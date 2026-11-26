# Smoke test of the worker lab cluster: start it, register ONE lab clone in it, connect one COM session and one thin client (1cv8c),
# see both in `rac session list`, unregister, stop, and show that no process of the cluster remains and that nothing of the other
# clusters changed.
#   pwsh -NoProfile -File smoke.ps1 [-Database ibcmd_rs_05_apply_wlab1_20260930] [-Track apply] [-SkipThinClient]
# The clone is restored (restore-clone.ps1) when it does not exist; it is NOT dropped here (drop-lab-dbs.ps1 does it when it allows).
# Exit codes: 0 all checks passed, 1 a check failed, 3 the platform refused for LICENSING reasons: the cluster is stopped and the
# run ends -- a licence refusal is reported, never worked around.
param(
    [ValidatePattern('^ibcmd_rs_0[45]_[a-z0-9_]+$')][string]$Database = 'ibcmd_rs_05_apply_wlab1_20260930',
    [ValidatePattern('^[a-z0-9_-]{2,20}$')][string]$Track = 'apply',
    [switch]$SkipThinClient
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
. "$PSScriptRoot\lib.ps1"
$tools = 'F:\ibcmd\lab\04\tools'
$stamp = Get-Date -Format 'yyyyMMdd_HHmmss'
New-Item -ItemType Directory -Force -Path $script:Logs | Out-Null
$log = Join-Path $script:Logs "smoke_$stamp.log"
$failed = New-Object System.Collections.Generic.List[string]
function Log([string]$text) { $line = Say $text; $line | Tee-Object -FilePath $log -Append }
function Check([string]$name, [bool]$ok, [string]$detail = '') {
    if ($ok) { Log "PASS  $name $detail" } else { Log "FAIL  $name $detail"; $failed.Add($name) }
}
function Stop-AndExit([int]$code, [string]$why) {
    Log $why
    & pwsh -NoProfile -File "$PSScriptRoot\stop.ps1" 2>&1 | ForEach-Object { Log "  stop: $_" }
    exit $code
}
function Read-Others {
    # the state of the OTHER clusters, read only: their server processes (id, name, start time) and the names of the infobases behind
    # the service RAS of 8.3.27 (2545) -- rac cluster list / infobase summary list, nothing else
    $snap = @(Get-OtherServerSnapshot)
    $procs = @($snap | Where-Object { $_ -notmatch ' rphost\.exe ' })
    $rphost = @($snap | Where-Object { $_ -match ' rphost\.exe ' }).Count
    $rac = 'C:\Program Files\1cv8\8.3.27.2214\bin\rac.exe'
    $cl = ((& $rac localhost:2545 cluster list) | Select-String '^cluster\s*:' | ForEach-Object { ($_ -replace '^cluster\s*:\s*', '').Trim() }) -join ','
    $names = @(& $rac localhost:2545 infobase summary list "--cluster=$(($cl -split ',')[0])" | Select-String '^name\s*:' | ForEach-Object { ($_ -replace '^name\s*:\s*', '').Trim() } | Sort-Object)
    [pscustomobject]@{ Procs = $procs; Rphosts = $rphost; Cluster = $cl; Infobases = $names }
}

Log "smoke of the worker lab cluster ($($script:Platform)), clone $Database"
$others0 = Read-Others
Log ("other clusters before: {0} server processes (+{1} rphost), 8.3.27 service cluster {2} with {3} infobases" -f $others0.Procs.Count, $others0.Rphosts, $others0.Cluster, $others0.Infobases.Count)

# the clone
$exists = [int](sqlcmd -S localhost -E -C -h -1 -W -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM sys.databases WHERE name = N'$Database'" | Select-Object -First 1)
if (-not $exists) {
    Log "restore $Database"
    & pwsh -NoProfile -File "$tools\restore-clone.ps1" -Corpus bsp8327 -Name $Database -Track $Track -Purpose 'worker lab smoke' 2>&1 | Select-Object -Last 1 | ForEach-Object { Log $_ }
}

# start
$sw = [Diagnostics.Stopwatch]::StartNew()
$startOutput = & pwsh -NoProfile -File "$PSScriptRoot\start.ps1" -Track $Track 2>&1
$startExit = $LASTEXITCODE
$startOutput | ForEach-Object { Log "  start: $_" }
Check 'the cluster starts' ($startExit -eq 0) "(exit $startExit, $([math]::Round($sw.Elapsed.TotalSeconds,1)) s)"
if ($startExit -ne 0) { exit 1 }
try {
    $cluster = Get-ClusterId
    Check 'rac sees a cluster of its own, not one of the services' ($cluster -and $cluster -ne $others0.Cluster) "($cluster)"

    # register the clone (register-ib.ps1 is the only place that reads the SQL login)
    $reg = & pwsh -NoProfile -File "$tools\register-ib.ps1" register -Database $Database -Cluster worker -Track $Track 2>&1
    Check 'the clone is registered in the worker cluster' ($LASTEXITCODE -eq 0 -and "$reg" -match 'registered:') "($(($reg | Out-String).Trim()))"
    $ib = $null; $cur = $null
    foreach ($line in Invoke-Rac infobase summary list "--cluster=$cluster") {
        if ($line -match '^infobase\s*:\s*(\S+)') { $cur = $Matches[1] } elseif ($line -match '^name\s*:\s*(\S+)' -and $Matches[1] -eq $Database) { $ib = $cur }
    }
    Check 'the infobase is listed by rac of the worker cluster' ([bool]$ib) "($ib)"
    $default = @(& pwsh -NoProfile -File "$tools\register-ib.ps1" list 2>&1)
    Check 'the service cluster does not list it' ($default -notcontains $Database)

    # one COM session
    $release = Join-Path $script:Root "release_$stamp.flag"
    $sessionOut = Join-Path $script:Logs "session_$stamp.out"
    $com = Start-Process -FilePath powershell.exe -PassThru -WindowStyle Hidden -RedirectStandardOutput $sessionOut -RedirectStandardError "$sessionOut.err" `
        -ArgumentList @('-NoProfile', '-File', "$PSScriptRoot\session.ps1", '-Srvr', $script:Srvr, '-Database', $Database, '-ReleaseFile', $release, '-HoldSec', '150')
    $end = (Get-Date).AddSeconds(150); $text = ''
    while ((Get-Date) -lt $end) {
        $text = (Get-Content $sessionOut -Encoding UTF8 -ErrorAction SilentlyContinue) -join ' '
        if ($text -match 'READY|ERROR') { break }
        Start-Sleep -Seconds 2
    }
    if ($text -match 'ERROR' -and $text -match '(?i)лиценз|licen[cs]e|ключ|HASP') { Stop-AndExit 3 "LICENSE: the platform refused the COM session: $text -- stopped; report it, do not work round it" }
    Check 'the COM session connects' ($text -match 'READY') "($text)"
    $sessions = @(Invoke-Rac session list "--cluster=$cluster")
    $comListed = ($sessions -join "`n") -match 'app-id\s*:\s*COMConnection'
    Check 'rac session list shows the COM session' $comListed
    Log ("  rac session list: " + ((($sessions | Select-String -Pattern '^(session-id|user-name|app-id|infobase|process)\s') | ForEach-Object { $_.Line.Trim() -replace '\s+', ' ' }) -join '; '))
    Log ("  rac process list: " + ((Invoke-Rac process list "--cluster=$cluster" | Select-String -Pattern '^(pid|port|running|use)\s') | ForEach-Object { $_.Line.Trim() -replace '\s+', ' ' }) -join '; ')
    New-Item -ItemType File -Path $release -Force | Out-Null
    [void]$com.WaitForExit(20000)
    if (-not $com.HasExited) { Stop-Process -Id $com.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item -LiteralPath $release -Force -ErrorAction SilentlyContinue

    # one thin client
    if (-not $SkipThinClient) {
        $thin = Join-Path $script:Bin '1cv8c.exe'
        $thinLog = Join-Path $script:Logs "thin_$stamp.out"
        $thinArgs = "ENTERPRISE /S`"$($script:Srvr)\$Database`" /N`"Администратор`" /P`"`" /DisableStartupMessages /DisableStartupDialogs /L ru /Out`"$thinLog`" -NoTruncate"
        $client = Start-Process -FilePath $thin -ArgumentList $thinArgs -PassThru -WindowStyle Minimized
        $end = (Get-Date).AddSeconds(150); $seen = $false; $sid = $null
        while ((Get-Date) -lt $end -and -not $seen) {
            $cur = @(Invoke-Rac session list "--cluster=$cluster")
            $block = $null; $blocks = ($cur -join "`n") -split "\n\s*\n"
            foreach ($b in $blocks) { if ($b -match 'app-id\s*:\s*1CV8C' -and $b -match [regex]::Escape($ib)) { $block = $b } }
            if ($block) { $seen = $true; if ($block -match 'session\s+:\s*(\S+)') { $sid = $Matches[1] } } else { Start-Sleep -Seconds 3 }
        }
        Check 'the thin client (1cv8c) connects and rac session list shows it' $seen "(session $sid)"
        Get-Content $thinLog -Encoding UTF8 -ErrorAction SilentlyContinue | Select-Object -First 3 | ForEach-Object { Log "  thin client log: $_" }
        if (-not $seen -and (Get-Content $thinLog -Raw -ErrorAction SilentlyContinue) -match '(?i)лиценз|licen[cs]e|ключ|HASP') {
            Stop-Process -Id $client.Id -Force -ErrorAction SilentlyContinue
            Stop-AndExit 3 'LICENSE: the platform refused the thin client -- stopped; report it, do not work round it'
        }
        Stop-Process -Id $client.Id -Force -ErrorAction SilentlyContinue
        if ($sid) { [void](Invoke-Rac session terminate "--cluster=$cluster" "--session=$sid") }
    }

    # no session is left, then the registration goes
    $end = (Get-Date).AddSeconds(60)
    while ((Get-Date) -lt $end -and @(Invoke-Rac session list "--cluster=$cluster" | Select-String '^session\s*:').Count) { Start-Sleep -Seconds 2 }
    Check 'no session is left' (@(Invoke-Rac session list "--cluster=$cluster" | Select-String '^session\s*:').Count -eq 0)
    $unreg = & pwsh -NoProfile -File "$tools\register-ib.ps1" unregister -Database $Database -Cluster worker -Track $Track 2>&1
    Check 'the clone is unregistered from the worker cluster' ($LASTEXITCODE -eq 0 -and "$unreg" -match 'unregistered:') "($(($unreg | Out-String).Trim()))"
}
finally {
    $stopOut = & pwsh -NoProfile -File "$PSScriptRoot\stop.ps1" 2>&1
    $stopExit = $LASTEXITCODE
    $stopOut | ForEach-Object { Log "  stop: $_" }
}
Check 'the cluster stops and none of its processes remains' ($stopExit -eq 0 -and @(Get-ClusterProcesses).Count -eq 0) "(stop exit $stopExit)"
Check 'none of its ports is listening' (@(Get-ClusterListeners).Count -eq 0)
Check 'the state file is gone' (-not (Test-Path -LiteralPath $script:StateFile))

# the other clusters are as they were
$others1 = Read-Others
Check 'the server processes of the other clusters are the same ones (id, name, start time)' (($others1.Procs -join "`n") -eq ($others0.Procs -join "`n")) "($($others1.Procs.Count) of $($others0.Procs.Count))"
Check 'the 8.3.27 service cluster and its infobases are unchanged' ($others1.Cluster -eq $others0.Cluster -and ($others1.Infobases -join ',') -eq ($others0.Infobases -join ',')) "($($others1.Infobases.Count) infobases)"
Log ("rphost of the other clusters: {0} before, {1} after (they come and go with the users' work)" -f $others0.Rphosts, $others1.Rphosts)

if ($failed.Count) { Log "RED: $($failed.Count) check(s) failed: $($failed -join '; ')"; exit 1 }
Log 'GREEN'
exit 0
