# desktop-test.ps1 v1.0.26
# Prepare the real Windows desktop, including when Oaclis runs inside WSL.
param([ValidateSet('Check','Prepare')][string]$Action = 'Check')
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'Questo supporto richiede Windows.' }
$clientPaths = @(
    (Join-Path $env:ProgramFiles 'RustDesk\rustdesk.exe'),
    (Join-Path $env:LOCALAPPDATA 'Programs\RustDesk\rustdesk.exe')
)
$client = $clientPaths | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
if (-not $client) {
    $command = Get-Command rustdesk.exe -ErrorAction SilentlyContinue
    if ($command) { $client = $command.Source }
}
if (-not $client -and $Action -eq 'Check') { Write-Host 'RustDesk da installare su Windows, non in WSL.'; exit 1 }
if (-not $client) {
    if (-not (Get-Command winget.exe -ErrorAction SilentlyContinue)) { throw 'Installare RustDesk dal sito ufficiale: gestore Windows non disponibile.' }
    & winget.exe install --id RustDesk.RustDesk --exact --source winget --accept-package-agreements --accept-source-agreements --disable-interactivity
    if ($LASTEXITCODE -ne 0) { throw 'Installazione RustDesk non completata. Verificare eventuale autorizzazione di Windows.' }
    $client = $clientPaths | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
    if (-not $client) { throw 'Installazione terminata ma client RustDesk non trovato.' }
}
Write-Host 'RustDesk presente su Windows. Il test mostrera il desktop Windows, non WSL.'
if ($Action -eq 'Prepare') {
    Write-Host 'Aprire RustDesk dal menu Start sui due computer. Dal master inserire ID del device e approvare sul device.'
    Write-Host 'Nel client scegliere il monitor e verificare mouse e tastiera. Nessuna password permanente configurata da Oaclis.'
}
