# keep-awake.ps1 v1.3.195
param([switch]$Check)
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public class OaclisAwake { [DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint flags); }'
try {
    if ([OaclisAwake]::SetThreadExecutionState(2147483649) -eq 0) { throw 'Windows ha rifiutato il blocco standby' }
    Write-Output 'ready'
    if (-not $Check) { [Console]::In.ReadToEnd() | Out-Null }
} finally { [OaclisAwake]::SetThreadExecutionState(2147483648) | Out-Null }
