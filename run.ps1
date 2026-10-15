# run.ps1 v1.4.148
# Windows checks; an explicitly selected WSL prepares and installs Oaclis.
param([switch]$Check, [string]$Distro, [switch]$PrepareAwake, [switch]$PrepareNetwork)
$ErrorActionPreference = 'Stop'
if (($Check -and ($PrepareAwake -or $PrepareNetwork)) -or ($PrepareAwake -and $PrepareNetwork)) { throw 'Usare una sola modalita: -Check, -PrepareAwake oppure -PrepareNetwork.' }
if ($env:OS -ne 'Windows_NT') { throw 'Su macOS/Linux usare bash run.sh.' }
if ($PrepareAwake) {
    if ($Check) { throw 'Usare -Check oppure -PrepareAwake, non entrambi.' }
    if ([string]::IsNullOrWhiteSpace($Distro)) { throw 'Specificare la distribuzione Oaclis con -Distro, per esempio Ubuntu.' }
    $repo = (& wsl -d $Distro -- wslpath -u $PSScriptRoot | Out-String).Trim()
    if ($LASTEXITCODE -ne 0 -or -not $repo) { throw 'Repository non raggiungibile dalla distribuzione WSL.' }
    & wsl -d $Distro -- bash "$repo/run.sh" --prepare-awake
    if ($LASTEXITCODE -ne 0) { throw 'Preparazione Sempre attivo non riuscita; leggere il motivo sopra.' }
    return
}
function Get-OaclisWslNetwork {
    param([string]$Distribution)
    $interfaces = @(Get-NetIPConfiguration | Where-Object {
        $_.NetAdapter.HardwareInterface -and $_.NetAdapter.Status -eq 'Up' -and
        @($_.IPv4Address | Where-Object { $_.IPAddress -notmatch '^(127\.|169\.254\.)' }).Count -gt 0
    } | Sort-Object @{Expression={if ($_.IPv4DefaultGateway) { 0 } else { 1 }}}, @{Expression={$_.NetIPv4Interface.InterfaceMetric}})
    if (-not $interfaces.Count) { throw 'Nessun indirizzo IPv4 LAN su un adattatore fisico attivo.' }
    $lan = @($interfaces[0].IPv4Address | Where-Object { $_.IPAddress -notmatch '^(127\.|169\.254\.)' })[0].IPAddress
    # Route lookup is local: it selects the source address, without contacting the Internet.
    $route = (& wsl -d $Distribution -- ip -j -4 route get 1.1.1.1 | Out-String)
    if ($LASTEXITCODE -ne 0) { throw 'Impossibile rilevare la rete WSL: verificare che la distribuzione sia avviata.' }
    $routes = @($route | ConvertFrom-Json)
    $address = [string]$routes[0].prefsrc
    $parsed = $null
    if (-not [Net.IPAddress]::TryParse($address, [ref]$parsed) -or $parsed.AddressFamily -ne [Net.Sockets.AddressFamily]::InterNetwork -or $address -match '^(0\.|127\.|169\.254\.)') { throw 'Indirizzo sorgente WSL IPv4 non valido.' }
    if ($address -eq $lan) { throw 'WSL usa la rete condivisa/mirrored: non creare portproxy verso lo stesso IP. Configurare il firewall della modalita mirrored.' }
    [pscustomobject]@{lan_ip=$lan; wsl_ip=$address; ssh_port=2222; control_port=23232}
}
function Test-OaclisTcp {
    param([string]$Address, [int]$Port)
    $client = [Net.Sockets.TcpClient]::new()
    try {
        $connection = $client.ConnectAsync($Address, $Port)
        if (-not $connection.Wait(2000)) { return $false }
        return $client.Connected
    } catch { return $false }
    finally { $client.Dispose() }
}
function Initialize-OaclisWslNetwork {
    param($Network)
    if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Per configurare inoltri e firewall, riaprire PowerShell come amministratore e usare .\run.ps1 -Distro <nome> -PrepareNetwork.'
    }
    & (Join-Path $PSScriptRoot 'scripts/wsl-network.ps1') -LanIp $Network.lan_ip -WslIp $Network.wsl_ip
    foreach ($mapping in @(@{External=2222;Internal=22}, @{External=23232;Internal=23232})) {
        if (-not (Test-OaclisTcp -Address $Network.wsl_ip -Port $mapping.Internal)) {
            Write-Warning "WSL non ascolta sulla porta $($mapping.Internal). Avviare SSH (22) oppure Oaclis (23232) nella distribuzione."
        } elseif (-not (Test-OaclisTcp -Address $Network.lan_ip -Port $mapping.External)) {
            throw "L'inoltro LAN $($Network.lan_ip):$($mapping.External) non risponde, anche se WSL e raggiungibile."
        } else { Write-Host "[OK] $($Network.lan_ip):$($mapping.External) -> $($Network.wsl_ip):$($mapping.Internal)" }
    }
}
function Save-Scenario {
    param([string]$Runtime, [string]$Distribution, $Network)
    if ($Check) { return }
    $directory = if ($env:OACLIS_SETTINGS_DIR) { $env:OACLIS_SETTINGS_DIR } else { Join-Path $env:LOCALAPPDATA 'Oaclis' }
    if ((Test-Path $directory) -and ((Get-Item $directory).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Cartella settings non regolare.' }
    [void][IO.Directory]::CreateDirectory($directory)
    $path = Join-Path $directory 'settings.json'
    if (Test-Path $path) {
        if ((Get-Item $path).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'settings.json non regolare.' }
        $settings = Get-Content -Raw -Encoding UTF8 $path | ConvertFrom-Json
        if ($settings -isnot [pscustomobject] -or ($settings.schema_version -and $settings.schema_version -ne 1)) { throw 'Formato settings non supportato: file conservato.' }
    } else { $settings = [pscustomobject]@{} }
    $settings | Add-Member -NotePropertyName schema_version -NotePropertyValue 1 -Force
    if (-not $settings.PSObject.Properties['control_port']) { $settings | Add-Member -NotePropertyName control_port -NotePropertyValue 23232 }
    $runtimeOs = if ($Runtime -eq 'wsl') { 'linux' } else { 'windows' }
    $kernelWsl = if ($Runtime -eq 'wsl') { $null } else { $false }
    $environment = [ordered]@{os=$runtimeOs; host_os='windows'; runtime=$Runtime; container=[bool]$env:CONTAINER_SANDBOX_MOUNT_POINT; wsl_kernel=$kernelWsl; wsl_distribution=$Distribution}
    $settings | Add-Member -NotePropertyName environment -NotePropertyValue $environment -Force
    if ($Network) { $settings | Add-Member -NotePropertyName wsl_network -NotePropertyValue $Network -Force }
    $temporary = Join-Path $directory ([IO.Path]::GetRandomFileName())
    try {
        [IO.File]::WriteAllText($temporary, ($settings | ConvertTo-Json -Depth 100 -WarningAction Stop), [Text.UTF8Encoding]::new($false))
        if (Test-Path $path) { [IO.File]::Replace($temporary, $path, [NullString]::Value) } else { [IO.File]::Move($temporary, $path) }
    } finally { if (Test-Path $temporary) { [IO.File]::Delete($temporary) } }
    Write-Host "Settings: $path"
}
Write-Host 'Sistema: Windows'
if (-not $PrepareNetwork) {
# Probe only: the application owns the lifetime of the actual standby block.
if (-not ('OaclisSetupAwake' -as [type])) {
    Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public class OaclisSetupAwake { [DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint flags); }'
}
try {
    if ([OaclisSetupAwake]::SetThreadExecutionState(2147483649) -eq 0) {
        Write-Warning 'Windows non consente il blocco standby: Sempre attivo non disponibile.'
    } else { Write-Host '[OK] Blocco standby Windows disponibile (prova rilasciata).' }
} finally { [OaclisSetupAwake]::SetThreadExecutionState(2147483648) | Out-Null }
foreach ($name in @('git', 'wsl', 'docker', 'cargo', 'rustc', 'python', 'ssh', 'ssh-keygen', 'rsync')) {
    if (Get-Command $name -ErrorAction SilentlyContinue) { Write-Host "[OK] $name" }
    else { Write-Host "[Manca] $name" }
}
Write-Host 'Windows nativo: Oaclis non e ancora compatibile; questo script non installa dipendenze native inutilizzabili.'
}
if (Get-Command wsl -ErrorAction SilentlyContinue) {
    $distros = @(& wsl --list --quiet)
    if ($LASTEXITCODE -ne 0) { throw 'Impossibile leggere le distribuzioni WSL.' }
    $distros = @($distros | ForEach-Object { ($_ -replace "`0", '').Trim() } | Where-Object { $_ })
    Write-Host ('Distribuzioni WSL disponibili: ' + ($distros -join ', '))
} else { $distros = @() }
if ([string]::IsNullOrWhiteSpace($Distro)) {
    if ($PrepareNetwork) { throw 'Specificare la distribuzione: .\run.ps1 -Distro Ubuntu -PrepareNetwork.' }
    Save-Scenario -Runtime 'native' -Distribution $null
    Write-Host 'Nessuna dipendenza installata. Per scegliere esplicitamente WSL: .\run.ps1 -Distro Ubuntu'
    return
}
if ($Distro -notin $distros) { throw 'Distribuzione non presente. Scegliere un nome riportato sopra.' }
if (-not $Check -and -not $PrepareNetwork) {
    $reply = Read-Host "Preparare e installare Oaclis in WSL ($Distro)? [s/N]"
    if ($reply -notin @('s', 'S')) { Write-Host 'Operazione annullata.'; return }
}
$network = Get-OaclisWslNetwork -Distribution $Distro
Write-Host "LAN: $($network.lan_ip) | WSL: $($network.wsl_ip)"
if ($Check) {
    $proxies = @(& netsh interface portproxy show v4tov4)
    if ($LASTEXITCODE -ne 0) { throw 'Impossibile leggere gli inoltri Windows.' }
    foreach ($mapping in @(@{External=2222;Internal=22}, @{External=23232;Internal=23232})) {
        $pattern = '^\s*' + [regex]::Escape($network.lan_ip) + '\s+' + $mapping.External + '\s+' + [regex]::Escape($network.wsl_ip) + '\s+' + $mapping.Internal + '\s*$'
        if (-not ($proxies | Where-Object { $_ -match $pattern })) { Write-Warning "Inoltro $($mapping.External) mancante o vecchio: usare -PrepareNetwork." }
    }
} else {
    Initialize-OaclisWslNetwork -Network $network
    Save-Scenario -Runtime 'wsl' -Distribution $Distro -Network $network
    if ($PrepareNetwork) { Write-Host 'Rete aggiornata. Nessuna compilazione, installazione o sincronizzazione avviata.'; return }
}
# Convert the repository path instead of assuming it is in a particular user's home.
$path = (& wsl -d $Distro -- wslpath -u $PSScriptRoot | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or -not $path) { throw 'Percorso repository non convertibile: eseguire bash run.sh direttamente nella distribuzione.' }
if ($Check) { & wsl -d $Distro -- env 'OACLIS_HOST_OS=windows' "OACLIS_WSL_DISTRO=$Distro" bash "$path/run.sh" --check }
else { & wsl -d $Distro -- env 'OACLIS_HOST_OS=windows' "OACLIS_WSL_DISTRO=$Distro" bash "$path/run.sh" }
if ($LASTEXITCODE -ne 0) { throw 'Preparazione incompleta. Leggere i controlli indicati sopra.' }
