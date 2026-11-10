# gaze-monitors.ps1 v1.0.0
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'Rilevamento monitor disponibile solo su Windows.' }
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class OaclisMonitorDpi {
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
}
'@
[void][OaclisMonitorDpi]::SetThreadDpiAwarenessContext([IntPtr](-4))
Add-Type -AssemblyName System.Windows.Forms
$monitors = @([System.Windows.Forms.Screen]::AllScreens | ForEach-Object {
    [pscustomobject]@{id=$_.DeviceName;x=$_.Bounds.X;y=$_.Bounds.Y;width=$_.Bounds.Width;height=$_.Bounds.Height;primary=$_.Primary}
})
ConvertTo-Json -InputObject $monitors -Compress
