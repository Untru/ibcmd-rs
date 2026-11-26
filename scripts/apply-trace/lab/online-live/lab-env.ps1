# Dot-source: . F:\ibcmd\lab\05\online\tools\lab-env.ps1
# Shared paths and helpers of the #344 evidence runs (8.3.27.2214 cluster, registered lab clones only).
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$script:Lab = 'F:\ibcmd\lab\05\online'
$script:Bin = 'C:\Program Files\1cv8\8.3.27.2214\bin'
$script:Rac = Join-Path $Bin 'rac.exe'
$script:Ras = 'localhost:2545'
$script:Srvr = 'localhost:2541'
$script:Tool = 'F:\ibcmd\src\ibcmd-rs-m-simple\target\iter\ibcmd-rs.exe'
$script:Epf = Join-Path $Lab 'observer\IbcmdRsObserver.epf'

function Get-ClusterId {
    ((& $Rac $Ras cluster list) | Select-String '^cluster\s*:' | Select-Object -First 1) -replace '^cluster\s*:\s*', ''
}

# uuid of a registered infobase by name (only the lab ones are ever asked for)
function Get-InfobaseId([string]$Name) {
    $cluster = Get-ClusterId
    $uuid = $null
    foreach ($line in & $Rac $Ras infobase summary list "--cluster=$cluster") {
        if ($line -match '^infobase\s*:\s*(\S+)') { $uuid = $Matches[1] }
        elseif ($line -match '^name\s*:\s*(\S+)' -and $Matches[1] -eq $Name) { return $uuid }
    }
    throw "infobase $Name is not registered"
}

function Now-Iso { [DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ss.fffZ') }

# blocks of `rac ... list` output as hashtables
function ConvertFrom-RacBlocks([string[]]$Lines) {
    $blocks = @(); $cur = [ordered]@{}
    foreach ($line in $Lines) {
        if ([string]::IsNullOrWhiteSpace($line)) { if ($cur.Count) { $blocks += , $cur; $cur = [ordered]@{} }; continue }
        $i = $line.IndexOf(':')
        if ($i -gt 0) { $cur[$line.Substring(0, $i).Trim()] = $line.Substring($i + 1).Trim().Trim('"') }
    }
    if ($cur.Count) { $blocks += , $cur }
    $blocks
}

# sessions of ONE lab infobase (never lists foreign ones)
function Get-LabSessions([string]$InfobaseId) {
    $cluster = Get-ClusterId
    ConvertFrom-RacBlocks (& $Rac $Ras session list "--cluster=$cluster" "--infobase=$InfobaseId")
}
function Get-LabConnections([string]$InfobaseId) {
    $cluster = Get-ClusterId
    ConvertFrom-RacBlocks (& $Rac $Ras connection list "--cluster=$cluster" "--infobase=$InfobaseId")
}
