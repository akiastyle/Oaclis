# keep-awake.ps1 v1.7.264
param([switch]$Check, [ValidateSet('always', 'screen-off', 'normal')][string]$Mode = 'always')
$ErrorActionPreference = 'Stop'
function Invoke-Power([string[]]$Arguments) {
    $output = & powercfg.exe @Arguments 2>&1
    if ($LASTEXITCODE -ne 0) { throw 'Impossibile modificare le impostazioni energia di Windows.' }
    return ($output | Out-String)
}
$active = [regex]::Match((Invoke-Power -Arguments @('/getactivescheme')), '[a-fA-F0-9-]{36}').Value
if (-not $active) { throw 'Piano energia Windows non disponibile.' }
if ($Check) { Write-Output 'ready'; exit }
$directory = Join-Path $env:LOCALAPPDATA 'Oaclis'
if ((Test-Path $directory) -and ((Get-Item $directory).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Cartella impostazioni energia non regolare.' }
[void][IO.Directory]::CreateDirectory($directory)
$path = Join-Path $directory 'power-settings.json'
if ((Test-Path $path) -and ((Get-Item $path).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Impostazioni energia non regolari.' }
$settings = @(@('SUB_VIDEO', 'VIDEOIDLE'), @('SUB_SLEEP', 'STANDBYIDLE'), @('SUB_SLEEP', 'HIBERNATEIDLE'))
$saved = $null
if (Test-Path $path) {
    $saved = Get-Content -Raw -LiteralPath $path | ConvertFrom-Json
    if ($saved.schema -ne 1 -or $saved.scheme -notmatch '^[a-fA-F0-9-]{36}$' -or $saved.values.Count -ne 3) { throw 'Impostazioni energia salvate non valide.' }
    foreach ($value in $saved.values) { if ($value.Count -ne 2 -or @($value | Where-Object { $_ -isnot [long] -and $_ -isnot [int] -or $_ -lt 0 -or $_ -gt [uint32]::MaxValue }).Count) { throw 'Timeout energia salvati non validi.' } }
}
if ($Mode -ne 'normal' -and -not $saved) {
    $values = foreach ($setting in $settings) {
        $query = Invoke-Power -Arguments @('/query', $active, $setting[0], $setting[1])
        $indices = [regex]::Matches($query, '0x([a-fA-F0-9]+)')
        if ($indices.Count -lt 2) { throw 'Timeout energia Windows non disponibili.' }
        ,@([Convert]::ToInt64($indices[$indices.Count-2].Groups[1].Value,16), [Convert]::ToInt64($indices[$indices.Count-1].Groups[1].Value,16))
    }
    $saved = [pscustomobject]@{ schema=1; scheme=$active; values=@($values) }
    $temporary = Join-Path $directory ([IO.Path]::GetRandomFileName())
    try { [IO.File]::WriteAllText($temporary,($saved | ConvertTo-Json -Depth 5)); [IO.File]::Move($temporary,$path) }
    finally { if (Test-Path $temporary) { [IO.File]::Delete($temporary) } }
}
if ($saved) {
    if ($Mode -ne 'normal' -and $saved.scheme -ne $active) { throw 'Il piano energia è cambiato: seleziona prima Normale.' }
    try {
        for ($index=0; $index -lt $settings.Count; $index++) {
            for ($supply=0; $supply -lt 2; $supply++) {
                $value = if ($Mode -eq 'normal') { $saved.values[$index][$supply] } elseif ($index -eq 0 -and $Mode -eq 'screen-off') { 300 } else { 0 }
                $flag = if ($supply -eq 0) { '/setacvalueindex' } else { '/setdcvalueindex' }
                [void](Invoke-Power -Arguments @($flag, $saved.scheme, $settings[$index][0], $settings[$index][1], [string]$value))
            }
        }
        if ($active -eq $saved.scheme) { [void](Invoke-Power -Arguments @('/setactive', $active)) }
        if ($Mode -eq 'normal') { [IO.File]::Delete($path) }
    } catch { throw 'Impostazioni energia non applicate completamente. Riprova o seleziona Normale.' }
}
Write-Output 'ready'
