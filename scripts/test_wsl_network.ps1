# test_wsl_network.ps1 v1.3.61
# Isolated tests: no real portproxy, firewall or service mutations.
param([string]$Source)
$ErrorActionPreference = 'Stop'
if (-not $Source) { $Source = Get-Content -Raw (Join-Path $PSScriptRoot '../run.ps1') }
$tokens = $null; $parseErrors = $null
$ast = [Management.Automation.Language.Parser]::ParseInput($Source, [ref]$tokens, [ref]$parseErrors)
if ($parseErrors.Count) { throw ($parseErrors | Out-String) }
foreach ($name in @('Get-OaclisWslNetwork', 'Test-OaclisTcp', 'Save-Scenario')) {
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
