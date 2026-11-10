# focus-probe.ps1 v1.0.0
param([string]$WindowHandle = '', [ValidateRange(500,60000)][int]$DelayMs = 500)
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'Eseguire questo script su Windows.' }
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.RegularExpressions;
public static class OaclisFocusProbe {
    public delegate bool EnumProc(IntPtr hwnd, IntPtr data);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct GuiInfo {
        public uint Size,Flags;
        public IntPtr Active,Focus,Capture,MenuOwner,MoveSize,Caret;
        public Rect CaretRect;
    }
    public class Window {
        public string Handle,Title,Process;
        public int Left,Top,Width,Height;
        public bool Foreground;
    }
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc callback,IntPtr data);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr hwnd);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr hwnd,StringBuilder text,int size);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd,StringBuilder text,int size);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr hwnd,out Rect rect);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint process);
    [DllImport("user32.dll")] public static extern bool GetGUIThreadInfo(uint thread,ref GuiInfo info);
    [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr hwnd,uint flags);
    [DllImport("user32.dll")] public static extern int GetSystemMetrics(int index);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr context);
    public static Window[] List() {
        var result=new List<Window>();
        EnumWindows((hwnd,data)=>{
            if(!IsWindowVisible(hwnd)||IsIconic(hwnd)) return true;
            var title=new StringBuilder(1024); GetWindowText(hwnd,title,title.Capacity);
            if(!Regex.IsMatch(title.ToString(),@"^(Oaclis|.+[\u00b7\u2022] Oaclis)( - (Google Chrome|Microsoft Edge|Mozilla Firefox|Brave|Opera|Vivaldi))?$",RegexOptions.IgnoreCase)) return true;
            var cls=new StringBuilder(128);GetClassName(hwnd,cls,cls.Capacity);
            if(cls.ToString()!="Chrome_WidgetWin_1"&&cls.ToString()!="MozillaWindowClass") return true;
            uint pid;GetWindowThreadProcessId(hwnd,out pid);
            string process;
            try {process=System.Diagnostics.Process.GetProcessById((int)pid).ProcessName;}
            catch(ArgumentException){return true;}
            catch(System.ComponentModel.Win32Exception){return true;}
            if(!Regex.IsMatch(process,@"^(chrome|msedge|firefox|brave|opera|vivaldi)$",RegexOptions.IgnoreCase)) return true;
            Rect r;if(!GetWindowRect(hwnd,out r))return true;
            result.Add(new Window{Handle="0x"+hwnd.ToInt64().ToString("X"),Title=title.ToString(),Process=process,Left=r.Left,Top=r.Top,Width=r.Right-r.Left,Height=r.Bottom-r.Top,Foreground=GetForegroundWindow()==hwnd});return true;
        },IntPtr.Zero);
        return result.ToArray();
    }
}
'@
[void][OaclisFocusProbe]::SetProcessDpiAwarenessContext([IntPtr](-4))
$windows = @([OaclisFocusProbe]::List())
if (!$WindowHandle) {
    [pscustomobject]@{Monitors=[OaclisFocusProbe]::GetSystemMetrics(80);VirtualLeft=[OaclisFocusProbe]::GetSystemMetrics(76);VirtualTop=[OaclisFocusProbe]::GetSystemMetrics(77);VirtualWidth=[OaclisFocusProbe]::GetSystemMetrics(78);VirtualHeight=[OaclisFocusProbe]::GetSystemMetrics(79);Windows=$windows} | ConvertTo-Json -Depth 4
    return
}
$target = $windows | Where-Object { $_.Handle -eq $WindowHandle } | Select-Object -First 1
if (!$target) { throw 'Handle non presente fra le finestre Oaclis visibili. Nessun focus modificato.' }
Write-Host "Fra $DelayMs ms provo il focus su $($target.Title)."
Start-Sleep -Milliseconds $DelayMs
# Recheck ownership and visibility immediately before using a possibly recycled handle.
if (!(@([OaclisFocusProbe]::List()) | Where-Object { $_.Handle -eq $WindowHandle })) { throw 'La finestra Oaclis non e piu disponibile.' }
$hwnd = [IntPtr]([Convert]::ToInt64($WindowHandle.Substring(2),16))
$before = [OaclisFocusProbe]::GetForegroundWindow()
$accepted = [OaclisFocusProbe]::SetForegroundWindow($hwnd)
# Activation is asynchronous; verify the foreground and the keyboard owner, not only the API return.
$deadline = [DateTime]::UtcNow.AddMilliseconds(700)
do {
    Start-Sleep -Milliseconds 25
    $after = [OaclisFocusProbe]::GetForegroundWindow()
    $gui = New-Object OaclisFocusProbe+GuiInfo
    $gui.Size = [Runtime.InteropServices.Marshal]::SizeOf($gui)
    [uint32]$owner = 0
    $thread = [OaclisFocusProbe]::GetWindowThreadProcessId($hwnd,[ref]$owner)
    $hasGui = [OaclisFocusProbe]::GetGUIThreadInfo($thread,[ref]$gui)
    $keyboard = $hasGui -and $gui.Focus -ne [IntPtr]::Zero -and [OaclisFocusProbe]::GetAncestor($gui.Focus,2) -eq $hwnd
} while (($after -ne $hwnd -or !$keyboard) -and [DateTime]::UtcNow -lt $deadline)
[pscustomobject]@{Title=$target.Title;Handle=$target.Handle;AlreadyForeground=$before -eq $hwnd;ApiAccepted=$accepted;Foreground=$after -eq $hwnd;KeyboardFocus=$keyboard;FocusHandle=('0x'+$gui.Focus.ToInt64().ToString('X'));Result=$(if($after -eq $hwnd -and $keyboard){'FOCUS_OK'}else{'FOCUS_NOT_CONFIRMED'})} | ConvertTo-Json
if ($after -ne $hwnd -or !$keyboard) { exit 2 }
