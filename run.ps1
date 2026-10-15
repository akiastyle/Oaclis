# run.ps1 v1.1.78
# Windows checks; an explicitly selected WSL prepares and installs Oaclis.
param([switch]$Check, [string]$Distro, [switch]$PrepareAwake)
$ErrorActionPreference = 'Stop'
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
function Save-Scenario {
    param([string]$Runtime, [string]$Distribution)
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
    $temporary = Join-Path $directory ([IO.Path]::GetRandomFileName())
    try {
        [IO.File]::WriteAllText($temporary, ($settings | ConvertTo-Json -Depth 100 -WarningAction Stop), [Text.UTF8Encoding]::new($false))
        if (Test-Path $path) { [IO.File]::Replace($temporary, $path, $null) } else { [IO.File]::Move($temporary, $path) }
    } finally { if (Test-Path $temporary) { [IO.File]::Delete($temporary) } }
    Write-Host "Settings: $path"
}
Write-Host 'Sistema: Windows'
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
if (Get-Command wsl -ErrorAction SilentlyContinue) {
    $distros = @(& wsl --list --quiet)
    if ($LASTEXITCODE -ne 0) { throw 'Impossibile leggere le distribuzioni WSL.' }
    $distros = @($distros | ForEach-Object { ($_ -replace "`0", '').Trim() } | Where-Object { $_ })
    Write-Host ('Distribuzioni WSL disponibili: ' + ($distros -join ', '))
} else { $distros = @() }
if ([string]::IsNullOrWhiteSpace($Distro)) {
    Save-Scenario -Runtime 'native' -Distribution $null
    Write-Host 'Nessuna dipendenza installata. Per scegliere esplicitamente WSL: .\run.ps1 -Distro Ubuntu'
    return
}
if ($Distro -notin $distros) { throw 'Distribuzione non presente. Scegliere un nome riportato sopra.' }
if (-not $Check) {
    $reply = Read-Host "Preparare e installare Oaclis in WSL ($Distro)? [s/N]"
    if ($reply -notin @('s', 'S')) { Write-Host 'Operazione annullata.'; return }
}
# Convert the repository path instead of assuming it is in a particular user's home.
$path = (& wsl -d $Distro -- wslpath -u $PSScriptRoot | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or -not $path) { throw 'Percorso repository non convertibile: eseguire bash run.sh direttamente nella distribuzione.' }
Save-Scenario -Runtime 'wsl' -Distribution $Distro
if ($Check) { & wsl -d $Distro -- env 'OACLIS_HOST_OS=windows' "OACLIS_WSL_DISTRO=$Distro" bash "$path/run.sh" --check }
else { & wsl -d $Distro -- env 'OACLIS_HOST_OS=windows' "OACLIS_WSL_DISTRO=$Distro" bash "$path/run.sh" }
if ($LASTEXITCODE -ne 0) { throw 'Preparazione incompleta. Leggere i controlli indicati sopra.' }
