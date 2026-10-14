# wsl-network.ps1 v1.0.33
# Run in an elevated PowerShell. Modify only the Oaclis ports and rules.
param([Parameter(Mandatory=$true)][string]$LanIp, [Parameter(Mandatory=$true)][string]$WslIp)
$ErrorActionPreference = 'Stop'
if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Aprire PowerShell come amministratore.' }
[void][System.Net.IPAddress]::Parse($LanIp)
[void][System.Net.IPAddress]::Parse($WslIp)
Set-Service iphlpsvc -StartupType Automatic
Start-Service iphlpsvc
foreach ($mapping in @(@{External=2222; Internal=22}, @{External=23232; Internal=23232})) {
    # Do not replace a mapping belonging to another application.
    $lines = netsh interface portproxy show v4tov4
    $match = $lines | Where-Object { $_ -match ('^\s*' + [regex]::Escape($LanIp) + '\s+' + $mapping.External + '\s+') }
    if ($match) {
        $fields = ($match.Trim() -split '\s+')
        if ([int]$fields[3] -ne $mapping.Internal) { throw "Porta $($mapping.External) gia usata da un altro inoltro." }
        netsh interface portproxy delete v4tov4 listenaddress=$LanIp listenport=$($mapping.External)
        if ($LASTEXITCODE) { throw 'Rimozione inoltro non riuscita.' }
    }
    netsh interface portproxy add v4tov4 listenaddress=$LanIp listenport=$($mapping.External) connectaddress=$WslIp connectport=$($mapping.Internal)
    if ($LASTEXITCODE) { throw 'Creazione inoltro non riuscita.' }
}
foreach ($rule in @(@{Name='Oaclis-WSL-SSH'; Protocol='TCP'; Port=2222}, @{Name='Oaclis-LAN-Control'; Protocol='TCP'; Port=23232}, @{Name='Oaclis-LAN-Discovery'; Protocol='UDP'; Port=23232})) {
    $existing = Get-NetFirewallRule -Name $rule.Name -ErrorAction SilentlyContinue
    if ($existing) {
        if ($existing.DisplayName -notlike 'Oaclis*') { throw 'Una regola omonima appartiene a un altra applicazione.' }
        $existing | Get-NetFirewallAddressFilter | Set-NetFirewallAddressFilter -LocalAddress $LanIp -RemoteAddress LocalSubnet
        $existing | Get-NetFirewallPortFilter | Set-NetFirewallPortFilter -Protocol $rule.Protocol -LocalPort $rule.Port
        $existing | Set-NetFirewallRule -Enabled True -Direction Inbound -Action Allow
    } else {
        New-NetFirewallRule -Name $rule.Name -DisplayName $rule.Name -Direction Inbound -Action Allow -Protocol $rule.Protocol -LocalAddress $LanIp -LocalPort $rule.Port -RemoteAddress LocalSubnet -Profile Any | Out-Null
    }
}
Write-Host 'Rete Oaclis configurata. Tornare al browser.'
