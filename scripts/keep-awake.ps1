# keep-awake.ps1 v1.4.206
param([switch]$Check, [ValidateSet("always", "screen-off")][string]$Mode = "always")
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public class OaclisAwake { [DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint flags); [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr SendMessageTimeout(IntPtr hwnd, uint msg, IntPtr wparam, IntPtr lparam, uint flags, uint timeout, out IntPtr result); }'
try {
    $flags = if ($Mode -eq "always") { 2147483651 } else { 2147483649 }
    if ([OaclisAwake]::SetThreadExecutionState($flags) -eq 0) { throw 'Windows ha rifiutato il blocco standby' }
    if ($Mode -eq 'screen-off' -and -not $Check) {
        $result = [IntPtr]::Zero
        if ([OaclisAwake]::SendMessageTimeout([IntPtr]65535, 274, [IntPtr]61808, [IntPtr]2, 2, 2000, [ref]$result) -eq [IntPtr]::Zero) { throw 'Spegnimento monitor non riuscito' }
    }
    Write-Output 'ready'
    if (-not $Check) { [Console]::In.ReadToEnd() | Out-Null }
} finally { [OaclisAwake]::SetThreadExecutionState(2147483648) | Out-Null }
