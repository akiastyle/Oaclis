# wsl-discovery.ps1 v1.0.40
# Runs with Oaclis, without a service or scheduled task.
param([string]$WslIp, [string]$Instance)
$ErrorActionPreference = 'Stop'
$listener = [System.Net.Sockets.UdpClient]::new(23232)
$listener.Client.ReceiveTimeout = 500
$forward = [System.Net.Sockets.UdpClient]::new()
$forward.EnableBroadcast = $true
$last = [datetime]::MinValue
$failures = 0
try {
    while ($true) {
        if (([datetime]::UtcNow - $last).TotalSeconds -ge 5) {
            $last = [datetime]::UtcNow
            try {
                $hello = Invoke-WebRequest -Uri 'http://localhost:23232/peer/hello' -UseBasicParsing -TimeoutSec 3
                $packet = $hello.Content | ConvertFrom-Json
                $card = $packet.hello | ConvertFrom-Json
                if ($card.instance -ne $Instance) { break }
                $bytes = [Text.Encoding]::UTF8.GetBytes($hello.Content)
                $addresses = Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.InterfaceAlias -notmatch 'vEthernet|Loopback' -and $_.AddressState -eq 'Preferred' -and $_.IPAddress -notlike '169.254.*' }
                foreach ($address in $addresses) {
                    $parts = $address.IPAddress.Split('.')
                    $value = ([uint64]$parts[0] -shl 24) -bor ([uint64]$parts[1] -shl 16) -bor ([uint64]$parts[2] -shl 8) -bor [uint64]$parts[3]
                    $mask = ([uint64]4294967295 -shl (32 - $address.PrefixLength)) -band [uint64]4294967295
                    $broadcast = $value -bor ((-bnot $mask) -band [uint64]4294967295)
                    $ip = '{0}.{1}.{2}.{3}' -f (($broadcast -shr 24) -band 255), (($broadcast -shr 16) -band 255), (($broadcast -shr 8) -band 255), ($broadcast -band 255)
                    [void]$forward.Send($bytes, $bytes.Length, $ip, 23232)
                }
                $failures = 0
            } catch { $failures++; if ($failures -ge 3) { break } }
        }
        try {
            $remote = [System.Net.IPEndPoint]::new([System.Net.IPAddress]::Any, 0)
            $bytes = $listener.Receive([ref]$remote)
            if ($bytes.Length -le 8192) { [void]$forward.Send($bytes, $bytes.Length, $WslIp, 23232) }
        } catch [System.Net.Sockets.SocketException] {
            if ($_.Exception.SocketErrorCode -ne [System.Net.Sockets.SocketError]::TimedOut) { throw }
        }
    }
} finally { $listener.Dispose(); $forward.Dispose() }
