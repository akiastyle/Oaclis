# test_wsl_network.ps1 v1.8.101
# Isolated tests: no real portproxy, firewall or service mutations.
param([string]$Source)
$ErrorActionPreference = 'Stop'
if (-not $Source) { $Source = Get-Content -Raw (Join-Path $PSScriptRoot '../run.ps1') }
$bootstrapRoot = Split-Path $PSScriptRoot -Parent
$tokens = $null; $parseErrors = $null
$ast = [Management.Automation.Language.Parser]::ParseInput($Source, [ref]$tokens, [ref]$parseErrors)
if ($parseErrors.Count) { throw ($parseErrors | Out-String) }
foreach ($name in @('Get-OaclisWslNetwork', 'Test-OaclisTcp', 'Save-Scenario', 'Initialize-OaclisWindowsAwake', 'Open-OaclisWebApp')) {
    $definition = $ast.FindAll({param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst]}, $true) | Where-Object Name -eq $name
    if (-not $definition) { throw "Funzione assente: $name" }
    Invoke-Expression $definition.Extent.Text
}
function Assert-Oaclis($Condition, [string]$Message) { if (-not $Condition) { throw $Message } }
function Get-NetIPConfiguration {
    @(
        [pscustomobject]@{NetAdapter=[pscustomobject]@{HardwareInterface=$false;Status='Up'};IPv4Address=@([pscustomobject]@{IPAddress='172.29.128.1'});IPv4DefaultGateway='virtual';NetIPv4Interface=[pscustomobject]@{InterfaceMetric=1}},
        [pscustomobject]@{NetAdapter=[pscustomobject]@{HardwareInterface=$true;Status='Up'};IPv4Address=@([pscustomobject]@{IPAddress='169.254.240.26'});IPv4DefaultGateway=$null;NetIPv4Interface=[pscustomobject]@{InterfaceMetric=1}},
        [pscustomobject]@{NetAdapter=[pscustomobject]@{HardwareInterface=$true;Status='Up'};IPv4Address=@([pscustomobject]@{IPAddress='192.168.1.167'});IPv4DefaultGateway='192.168.1.1';NetIPv4Interface=[pscustomobject]@{InterfaceMetric=10}}
    )
}
function wsl {
    Assert-Oaclis (($args -join ' ') -eq '-d Ubuntu ip -j -4 route get 1.1.1.1') ("Argomenti rilevamento inattesi: " + ($args -join "|"))
    $global:LASTEXITCODE = 0
    $script:routeFixture
}
$script:routeFixture = '[{"dst":"1.1.1.1","gateway":"172.29.128.1","dev":"eth0","prefsrc":"172.29.128.195"}]'
$network = Get-OaclisWslNetwork -Distribution Ubuntu
Assert-Oaclis ($network.lan_ip -eq '192.168.1.167') 'Non deve scegliere vEthernet o APIPA.'
Assert-Oaclis ($network.wsl_ip -eq '172.29.128.195') 'Deve scegliere IP WSL, non il gateway.'
Assert-Oaclis ($network.ssh_port -eq 2222 -and $network.control_port -eq 23232) 'Porte Oaclis errate.'
$script:routeFixture = '[{"prefsrc":"127.0.0.1"}]'
$rejected = $false
try { Get-OaclisWslNetwork -Distribution Ubuntu | Out-Null } catch { $rejected = $true }
Assert-Oaclis $rejected 'Una sorgente loopback deve essere rifiutata.'
$directory = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
$previous = $env:OACLIS_SETTINGS_DIR
try {
    [void][IO.Directory]::CreateDirectory($directory)
    $settingsPath = Join-Path $directory 'settings.json'
    [IO.File]::WriteAllText($settingsPath, '{"schema_version":1,"control_port":23232,"custom":"preserve"}')
    $env:OACLIS_SETTINGS_DIR = $directory
    $Check = $false
    Save-Scenario -Runtime wsl -Distribution Ubuntu -Network $network
    $saved = Get-Content -Raw $settingsPath | ConvertFrom-Json
    Assert-Oaclis ($saved.custom -eq 'preserve' -and $saved.wsl_network.wsl_ip -eq '172.29.128.195') 'Preservare le impostazioni e salvare la rete.'
    $before = [IO.File]::ReadAllText($settingsPath)
    $Check = $true
    Save-Scenario -Runtime wsl -Distribution Ubuntu -Network ([pscustomobject]@{wsl_ip='changed'})
    Assert-Oaclis ([IO.File]::ReadAllText($settingsPath) -eq $before) '-Check non deve scrivere settings.'
} finally {
    $env:OACLIS_SETTINGS_DIR = $previous
    [IO.Directory]::Delete($directory, $true)
}
$listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
try {
    $listener.Start()
    $port = $listener.LocalEndpoint.Port
    Assert-Oaclis (Test-OaclisTcp -Address '127.0.0.1' -Port $port) 'Il controllo TCP deve riconoscere un listener.'
} finally { $listener.Stop() }
Assert-Oaclis (-not (Test-OaclisTcp -Address '127.0.0.1' -Port $port)) 'Una porta chiusa non deve risultare disponibile.'
Write-Host 'WSL network: rilevamento, validazione, settings e TCP verificati; nessuna modifica alla rete Windows.'

# Windows bootstrap owns helper installation, browser opening and listener recovery.
$networkSource = Get-Content -Raw (Join-Path $PSScriptRoot 'wsl-network.ps1')
$networkAst = [Management.Automation.Language.Parser]::ParseInput($networkSource, [ref]$tokens, [ref]$parseErrors)
if ($parseErrors.Count) { throw ($parseErrors | Out-String) }
Invoke-Expression ($networkAst.FindAll({param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst]}, $true) | Where-Object Name -eq 'Repair-OaclisProxyListeners').Extent.Text
function Get-NetTCPConnection { if ($script:listenersReady) { [pscustomobject]@{State='Listen'} } }
function Restart-Service { $script:restarts++; if (-not $script:keepMissing) { $script:listenersReady = $true } }
function Start-Sleep {}
$script:listenersReady = $true; $script:restarts = 0; $script:keepMissing = $false
Repair-OaclisProxyListeners -LanIp '192.168.1.167'
Assert-Oaclis ($script:restarts -eq 0) 'Non riavviare IP Helper quando gli inoltri ascoltano.'
$script:listenersReady = $false
Repair-OaclisProxyListeners -LanIp '192.168.1.167'
Assert-Oaclis ($script:restarts -eq 1 -and $script:listenersReady) 'Gli inoltri senza listener devono ripristinare IP Helper.'
$script:listenersReady = $false; $script:keepMissing = $true; $rejected = $false
try { Repair-OaclisProxyListeners -LanIp '192.168.1.167' } catch { $rejected = $true }
Assert-Oaclis $rejected 'Non dichiarare pronta una rete che ancora non ascolta.'
function Invoke-WebRequest { $script:requests++; [pscustomobject]@{StatusCode=$script:httpStatus} }
function Start-Process { $script:opened = $args[0] }
$script:requests = 0; $script:httpStatus = 200; $script:opened = $null
Open-OaclisWebApp
Assert-Oaclis ($script:requests -eq 1 -and $script:opened -eq 'http://127.0.0.1:23232') 'Aprire il browser solo dopo la risposta della webapp.'
$script:httpStatus = 500; $script:opened = $null; $rejected = $false
try { Open-OaclisWebApp } catch { $rejected = $true }
Assert-Oaclis ($rejected -and -not $script:opened) 'Non aprire la webapp prima che sia pronta.'
$previous = $env:LOCALAPPDATA
$directory = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
try {
    $env:LOCALAPPDATA = $directory
    Initialize-OaclisWindowsAwake
    Assert-Oaclis (Test-Path (Join-Path $directory 'Oaclis/keep-awake.ps1')) 'Il supporto standby deve essere installato da PowerShell.'
} finally {
    $env:LOCALAPPDATA = $previous
    if (Test-Path $directory) { [IO.Directory]::Delete($directory, $true) }
}
Write-Host 'Bootstrap Windows: standby, browser e recupero listener verificati con fixture isolate.'
