# Samples the sessions and connections of ONE lab infobase through RAS once a second and writes a row per change.
#   pwsh -NoProfile -File rac-timeline.ps1 -Database <db> -Out <file.tsv> [-IntervalMs 1000] [-MaxSeconds 900] [-StopFile <path>]
# Columns: utc, kind (session|connection|process), key, fields... Only rows of the infobase; the process list is
# reduced to the working processes that serve it.
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Out,
    [int]$IntervalMs = 1000,
    [int]$MaxSeconds = 900,
    [string]$StopFile = ''
)
. F:\ibcmd\lab\05\online\tools\lab-env.ps1
if ($Database -notmatch '^ibcmd_rs_0[45]_[a-z0-9_]+$') { throw 'lab databases only' }
$cluster = Get-ClusterId
$ib = Get-InfobaseId $Database
"utc`tkind`tkey`tapp`tuser`tstarted_at`tlast_active_at`tprocess`tconnection`tpid`textra" | Set-Content -LiteralPath $Out -Encoding utf8
$prev = @{}
$deadline = (Get-Date).AddSeconds($MaxSeconds)
while ((Get-Date) -lt $deadline) {
    if ($StopFile -and (Test-Path -LiteralPath $StopFile)) { break }
    $now = Now-Iso
    try {
        $sessions = ConvertFrom-RacBlocks (& $Rac $Ras session list "--cluster=$cluster" "--infobase=$ib")
        $connections = ConvertFrom-RacBlocks (& $Rac $Ras connection list "--cluster=$cluster" "--infobase=$ib")
        $procs = @{}
        foreach ($p in (ConvertFrom-RacBlocks (& $Rac $Ras process list "--cluster=$cluster"))) { $procs[$p['process']] = $p['pid'] }
        $cur = @{}
        foreach ($s in $sessions) {
            $key = "session`t$($s['session'])"
            $cur[$key] = "$($s['app-id'])`t$($s['user-name'])`t$($s['started-at'])`t$($s['last-active-at'])`t$($s['process'])`t$($s['connection'])`t$($procs[$s['process']])`tcalls=$($s['calls-last-5min']) blocked=$($s['blocked-by-dbms'])/$($s['blocked-by-ls']) hibernate=$($s['hibernate'])"
        }
        foreach ($c in $connections) {
            $key = "connection`t$($c['connection'])"
            $cur[$key] = "$($c['application'])`t$($c['user-name'])`t$($c['connected-at'])`t`t$($c['process'])`t$($c['connection'])`t$($procs[$c['process']])`tdb-proc-took=$($c['db-proc-took']) blocked=$($c['blocked-by-dbms'])"
        }
        # rows that are new or whose static part changed; last-active-at changes every second and is left out of the key
        foreach ($k in $cur.Keys) {
            $fields = $cur[$k] -split "`t"
            $sig = ($fields[0..2] + $fields[4..$($fields.Count - 1)]) -join '|'
            if (-not $prev.ContainsKey($k) -or $prev[$k] -ne $sig) {
                Add-Content -LiteralPath $Out -Encoding utf8 -Value "$now`t$k`t$($cur[$k])"
                $prev[$k] = $sig
            }
        }
        foreach ($k in @($prev.Keys)) {
            if (-not $cur.ContainsKey($k)) {
                Add-Content -LiteralPath $Out -Encoding utf8 -Value "$now`t$k`tGONE"
                $prev.Remove($k)
            }
        }
    } catch {
        Add-Content -LiteralPath $Out -Encoding utf8 -Value "$now`tERROR`t$($_.Exception.Message -replace '\s+', ' ')"
    }
    Start-Sleep -Milliseconds $IntervalMs
}
