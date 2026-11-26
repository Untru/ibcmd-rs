# The lab cases of #409 F-9 (the live switch checks neither the log backup chain nor the tail directory) and F-10 (it interrupts sessions
# with work in flight), against the WORKER LAB CLUSTER (scripts/apply-lab/cluster). The same script runs the old tool (-Label red: the
# defect is reproduced) and the fixed one (-Label green: the switch is refused before anything is staged, or run when the operator
# accepted the interruption).
#   pwsh -NoProfile -File f9_f10.ps1 -Exe <ibcmd-rs.exe> -Label red|green [-Cases chain,nodir,nowrite,sessions,accepted,activate,dryrun,clean]
# The worker cluster must run (start.ps1 -Track apply, under `heavy-lock.ps1 acquire apply -Name worker`) and the clone must be registered
# there (register-ib.ps1 register -Database <db> -Cluster worker). The clone is changed: each case promotes a new marker of
# ОбсужденияСлужебныйКлиентСервер (live_trees.py) and the recovery model / log chain are set by the case.
# Cases: chain (no log backup chain), nodir (the tail directory is not there), nowrite (the account cannot write there), sessions (a
# session holds an open transaction), accepted (the same with --interrupt-sessions; green only), activate (the direct route
# mssql-activate-staged-main with a broken chain; green only), dryrun (--dry-run: the report, no probe; green only), clean (nothing wrong).
# The clone must be marker-free (marker_free.ps1): live refuses a database with online generations.
# Output: F:\ibcmd\lab\05\live\out\f9f10_<label>.log
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][ValidateSet('red', 'green')][string]$Label,
    [string[]]$Cases = @('chain', 'nodir', 'nowrite', 'sessions', 'accepted', 'clean'),
    [ValidatePattern('^ibcmd_rs_05_[a-z0-9_]+$')][string]$Database = 'ibcmd_rs_05_apply_wlab1_20260930'
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$Cases = @($Cases | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
$lab = 'F:\ibcmd\lab\05\live'
$bin = 'C:\Program Files\1cv8\8.3.27.2214\bin'
$rac = Join-Path $bin 'rac.exe'
$ras = 'localhost:5545'
$module = 'ОбсужденияСлужебныйКлиентСервер'
$log = "$lab\out\f9f10_$Label.log"
New-Item -ItemType Directory -Force "$lab\out", "$lab\bak", "$lab\src" | Out-Null
$failed = New-Object System.Collections.Generic.List[string]
function Say($text) { $line = "$(Get-Date -Format s) $text"; $line | Tee-Object -FilePath $log -Append }
function Check($name, $ok, $detail = '') { if ($ok) { Say "PASS  $name $detail" } else { Say "FAIL  $name $detail"; $failed.Add($name) } }
function Sql($text) { $o = sqlcmd -S localhost -E -C -h -1 -W -s '|' -Q "SET NOCOUNT ON; $text" 2>&1; ($o | ForEach-Object { "$_" }) -join "`n" }
function Scalar($text) { ((Sql $text) -split "`n" | Where-Object { $_.Trim() } | Select-Object -First 1).Trim() }

# the worker cluster: cluster and infobase ids
$cluster = ((& $rac $ras cluster list) | Select-String '^cluster\s*:' | Select-Object -First 1) -replace '^cluster\s*:\s*', ''
if (-not $cluster) { throw "the worker lab cluster is not running (nothing answers at $ras)" }
$infobase = $null; $cur = $null
foreach ($line in & $rac $ras infobase summary list "--cluster=$cluster") {
    if ($line -match '^infobase\s*:\s*(\S+)') { $cur = $Matches[1] } elseif ($line -match '^name\s*:\s*(\S+)' -and $Matches[1] -eq $Database) { $infobase = $cur }
}
if (-not $infobase) { throw "$Database is not registered in the worker lab cluster" }

$stamp = Get-Date -Format 'HHmmss'
function Tail([string]$case) { "$lab\bak\${Label}_${stamp}_$case.trn" }
function New-Tag($case) { $tag = "$Label-$case-$stamp"; & python "$PSScriptRoot\live_trees.py" "$lab\src" $tag | Out-Null; $tag }

# the state that shows whether a promotion was committed and whether anything was staged
function Snapshot {
    [pscustomobject]@{
        Staged   = [int](Scalar "SELECT COUNT(*) FROM [$Database].dbo.ConfigSave")
        Versions = Scalar "SELECT CONVERT(varchar(16), HASHBYTES('SHA2_256', BinaryData), 2) FROM [$Database].dbo.Config WHERE FileName = N'versions' AND PartNo = 0"
        Markers  = [int](Scalar "SELECT COUNT(*) FROM [$Database].dbo.Config WHERE FileName = N'DynamicallyUpdated'")
        State    = Scalar "SELECT state_desc FROM sys.databases WHERE name = N'$Database'"
        Recovery = Scalar "SELECT recovery_model_desc FROM sys.databases WHERE name = N'$Database'"
        Chain    = Scalar "SELECT CASE WHEN last_log_backup_lsn IS NULL THEN 'no' ELSE 'yes' END FROM sys.database_recovery_status WHERE database_id = DB_ID(N'$Database')"
        Probe    = Scalar "IF OBJECT_ID(N'[$Database].dbo.IbcmdRsLiveProbe') IS NULL SELECT 0 ELSE SELECT COUNT(*) FROM [$Database].dbo.IbcmdRsLiveProbe"
    }
}
function Show($s) { "staged=$($s.Staged) markers=$($s.Markers) state=$($s.State) recovery=$($s.Recovery) chain=$($s.Chain) probe_rows=$($s.Probe) versions=$($s.Versions)" }

function Clear-Stage { Sql "DELETE FROM [$Database].dbo.ConfigSave" | Out-Null }
function Make-Chain {
    Sql "ALTER DATABASE [$Database] SET RECOVERY FULL; BACKUP DATABASE [$Database] TO DISK = N'$lab\bak\${Database}_$stamp.bak' WITH INIT, COMPRESSION, STATS = 100;" | Out-Null
}
function Break-Chain { Sql "ALTER DATABASE [$Database] SET RECOVERY SIMPLE; ALTER DATABASE [$Database] SET RECOVERY FULL;" | Out-Null }

# one live run of the tool
function Run-Live([string]$case, [string]$tag, [string]$tail, [string[]]$extra = @()) {
    $args1 = @('mssql-apply-source-change', '--platform-profile', 'platform-8.3.27.2214', '--server', 'localhost', '--database', $Database,
        '--sqlcmd-trust-cert', '--source-root', "$lab\src\$tag", '--path', "CommonModules/$module/Ext/Module.bsl", '--mode', 'live',
        '--tail-log-output', $tail, '--script-output', "$lab\out\${Label}_${stamp}_$case.sql", '--recovery-output', "$lab\out\${Label}_${stamp}_$case.recovery.json",
        '--rac', $rac, '--ras-endpoint', $ras, '--cluster-id', $cluster, '--infobase-id', $infobase, '--infobase-user', 'Администратор', '--allow-non-lab') + $extra
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $out = & $Exe @args1 2>"$lab\out\${Label}_${stamp}_$case.err"
    $code = $LASTEXITCODE
    $out | Set-Content "$lab\out\${Label}_${stamp}_$case.json" -Encoding UTF8
    $errText = (Get-Content "$lab\out\${Label}_${stamp}_$case.err" -Raw -ErrorAction SilentlyContinue)
    $err = if ($errText) { ("$errText" -replace '\s+', ' ').Trim() } else { '' }
    if ($err.Length -gt 700) { $err = $err.Substring(0, 700) + '...' }
    [pscustomobject]@{ Code = $code; Seconds = [math]::Round($sw.Elapsed.TotalSeconds, 1); Error = $err; Json = "$lab\out\${Label}_${stamp}_$case.json" }
}

function Hold-Transaction([string]$case) {
    $release = "$lab\out\${Label}_${stamp}_$case.release"
    $outf = "$lab\out\${Label}_${stamp}_$case.txn.out"
    Remove-Item -LiteralPath $release, $outf -Force -ErrorAction SilentlyContinue
    $p = Start-Process -FilePath pwsh -ArgumentList @('-NoProfile', '-File', "$PSScriptRoot\hold_txn.ps1", '-Database', $Database, '-ReleaseFile', $release, '-HoldSec', '600') `
        -RedirectStandardOutput $outf -RedirectStandardError "$outf.err" -WindowStyle Hidden -PassThru
    $end = (Get-Date).AddSeconds(60)
    while ((Get-Date) -lt $end) { if ((Get-Content $outf -ErrorAction SilentlyContinue) -match 'READY|ERROR') { break }; Start-Sleep -Milliseconds 300 }
    [pscustomobject]@{ Process = $p; Release = $release; Out = $outf }
}
function Release-Transaction($holder) {
    New-Item -ItemType File -Path $holder.Release -Force | Out-Null
    [void]$holder.Process.WaitForExit(30000)
    if (-not $holder.Process.HasExited) { Stop-Process -Id $holder.Process.Id -Force -ErrorAction SilentlyContinue }
    (Get-Content $holder.Out -ErrorAction SilentlyContinue) -join ' | '
}

Say "== f9_f10 $Label with $Exe on $Database (worker lab cluster $cluster)"
$version = (& $Exe --version 2>&1 | Select-Object -First 1)
Say "tool: $version"

foreach ($case in $Cases) {
    Say "---- case $case"
    if ($case -ne 'activate') { Clear-Stage }
    switch ($case) {
        'chain' {
            # F-9: FULL recovery, but no full backup since: BACKUP LOG fails with 4214
            Make-Chain; Break-Chain
            $before = Snapshot; Say ("before: " + (Show $before))
            $r = Run-Live 'chain' (New-Tag 'chain') (Tail 'chain')
            $after = Snapshot; Say ("after:  " + (Show $after)); Say "exit $($r.Code) in $($r.Seconds) s; error: $($r.Error)"
            $committed = ($after.Versions -ne $before.Versions)
            if ($Label -eq 'red') {
                Check 'F-9 chain: the promotion is committed although the tail-log backup cannot be taken' ($committed -and $r.Code -ne 0 -and $r.Error -match '4214|no current database backup|не существует резервной копии') "(committed=$committed)"
            } else {
                Check 'F-9 chain: refused before the stage, nothing changed' ($r.Code -ne 0 -and $r.Error -match 'log backup chain' -and -not $committed -and $after.Staged -eq 0 -and $after.Markers -eq $before.Markers) "(committed=$committed staged=$($after.Staged))"
            }
        }
        'nodir' {
            Make-Chain
            $before = Snapshot; Say ("before: " + (Show $before))
            $r = Run-Live 'nodir' (New-Tag 'nodir') "$lab\nodir_$stamp\${Label}_nodir.trn"
            $after = Snapshot; Say ("after:  " + (Show $after)); Say "exit $($r.Code) in $($r.Seconds) s; error: $($r.Error)"
            $committed = ($after.Versions -ne $before.Versions)
            if ($Label -eq 'red') {
                Check 'F-9 nodir: the promotion is committed although the tail directory does not exist' ($committed -and $r.Code -ne 0) "(committed=$committed)"
            } else {
                Check 'F-9 nodir: refused before the stage, nothing changed' ($r.Code -ne 0 -and $r.Error -match 'directory of the tail-log output does not exist' -and -not $committed -and $after.Staged -eq 0) "(committed=$committed staged=$($after.Staged))"
            }
        }
        'nowrite' {
            Make-Chain
            $before = Snapshot; Say ("before: " + (Show $before))
            $r = Run-Live 'nowrite' (New-Tag 'nowrite') "C:\Windows\System32\config\${Label}_nowrite.trn"
            $after = Snapshot; Say ("after:  " + (Show $after)); Say "exit $($r.Code) in $($r.Seconds) s; error: $($r.Error)"
            $committed = ($after.Versions -ne $before.Versions)
            if ($Label -eq 'red') {
                Check 'F-9 nowrite: the promotion is committed although the account cannot write to the directory' ($committed -and $r.Code -ne 0) "(committed=$committed)"
            } else {
                Check 'F-9 nowrite: refused before the stage, nothing changed' ($r.Code -ne 0 -and $r.Error -match 'cannot write a backup' -and -not $committed -and $after.Staged -eq 0) "(committed=$committed staged=$($after.Staged))"
            }
        }
        'sessions' {
            # F-10: a session holds a transaction with one row written
            Make-Chain
            $holder = Hold-Transaction 'sessions'
            Say ("holder: " + ((Get-Content $holder.Out -ErrorAction SilentlyContinue) -join ' | '))
            $before = Snapshot; Say ("before: " + (Show $before))
            $r = Run-Live 'sessions' (New-Tag 'sessions') (Tail 'sessions')
            $mid = Snapshot; Say ("after:  " + (Show $mid)); Say "exit $($r.Code) in $($r.Seconds) s; error: $($r.Error)"
            $held = Release-Transaction $holder; Say "the session: $held"
            $end = Snapshot; Say ("after the session ended: " + (Show $end))
            $committed = ($mid.Versions -ne $before.Versions)
            $rowSurvived = ($end.Probe -gt $before.Probe)
            if ($Label -eq 'red') {
                Check 'F-10: the switch went on, the session was cut off and its row is gone, with no warning' ($committed -and $held -match 'LOST' -and -not $rowSurvived) "(committed=$committed, held=$held)"
            } else {
                Check 'F-10: refused before the stage, the session names its work, nothing changed, the session commits' ($r.Code -ne 0 -and $r.Error -match 'open work' -and $r.Error -match 'session \d+' -and -not $committed -and $mid.Staged -eq 0 -and $held -match 'COMMITTED' -and $rowSurvived) "(committed=$committed, held=$held, row=$rowSurvived)"
            }
        }
        'accepted' {
            if ($Label -eq 'red') { Say 'skipped: the old tool has no --interrupt-sessions'; continue }
            Make-Chain
            $holder = Hold-Transaction 'accepted'
            $before = Snapshot; Say ("before: " + (Show $before))
            $r = Run-Live 'accepted' (New-Tag 'accepted') (Tail 'accepted') @('--interrupt-sessions')
            $mid = Snapshot; Say ("after:  " + (Show $mid)); Say "exit $($r.Code) in $($r.Seconds) s; error: $($r.Error)"
            $held = Release-Transaction $holder; Say "the session: $held"
            $committed = ($mid.Versions -ne $before.Versions)
            Check 'F-10 accepted: with --interrupt-sessions the switch is not refused for the open work and the promotion is committed' ($committed) "(exit $($r.Code), held=$held)"
            Check 'F-10 accepted: the interrupted session lost its row, as it was told' ($held -match 'LOST' -and (Snapshot).Probe -le $before.Probe) ''
        }
        'activate' {
            # the direct route: the stage is made by mssql-stage-source-objects, mssql-activate-staged-main asks the same gate
            if ($Label -eq 'red') { Say 'skipped: red is the old tool, whose activate-staged-main has no gate'; continue }
            Make-Chain; Break-Chain
            $tag = New-Tag 'activate'
            $stage = & $Exe mssql-stage-source-objects --server localhost --database $Database --source-root "$lab\src\$tag" --replace-config-save --allow-non-lab `
                --batch-size 1 --path-prefix "CommonModules/$module" --per-row 2>"$lab\out\${Label}_${stamp}_activate.stage.err"
            Check 'activate: the module is staged' ($LASTEXITCODE -eq 0) "(exit $LASTEXITCODE)"
            $before = Snapshot; Say ("before: " + (Show $before))
            $out = & $Exe mssql-activate-staged-main --platform-profile platform-8.3.27.2214 --server localhost --database $Database --sqlcmd-trust-cert --mode live `
                --tail-log-output (Tail 'activate') --script-output "$lab\out\${Label}_${stamp}_activate.sql" --recovery-output "$lab\out\${Label}_${stamp}_activate.recovery.json" `
                --rac $rac --ras-endpoint $ras --cluster-id $cluster --infobase-id $infobase --infobase-user Администратор --allow-non-lab 2>"$lab\out\${Label}_${stamp}_activate.err"
            $code = $LASTEXITCODE
            $errText = Get-Content "$lab\out\${Label}_${stamp}_activate.err" -Raw; $err = if ($errText) { ("$errText" -replace '\s+', ' ').Trim() } else { ''}; if ($err.Length -gt 500) { $err = $err.Substring(0, 500) + '...' }
            $after = Snapshot; Say ("after:  " + (Show $after)); Say "exit $code; error: $err"
            Check 'activate: refused by the gate of the direct route; the stage stays where it is (it was staged before), nothing is promoted' ($code -ne 0 -and $err -match 'log backup chain' -and $after.Versions -eq $before.Versions -and $after.Staged -gt 0) "(staged=$($after.Staged))"
        }
        'dryrun' {
            if ($Label -eq 'red') { Say 'skipped: red is the old tool'; continue }
            Make-Chain
            $tag = New-Tag 'dryrun'
            $before = Snapshot
            $r = Run-Live 'dryrun' $tag (Tail 'dryrun') @('--dry-run')
            $after = Snapshot
            $report = Get-Content $r.Json -Raw -Encoding UTF8 | ConvertFrom-Json
            $gate = $report.live_gate
            Say "exit $($r.Code); live_gate: recovery=$($gate.recovery_model) log_chain=$($gate.log_chain) write_probe=$($gate.write_probe) connections=$($gate.connections) 1c=$($gate.connections_1c) open_tx=$($gate.with_open_transaction) running=$($gate.with_running_request) directory=$($gate.tail_directory)"
            $probeFiles = @(Get-ChildItem -LiteralPath "$lab\bak" -Filter '*.ibcmdrsprobe' -ErrorAction SilentlyContinue).Count
            Check 'dry run: the gate reports what it found, writes no probe and changes nothing' ($r.Code -eq 0 -and $gate.log_chain -and -not $gate.write_probe -and $probeFiles -eq 0 -and $after.Versions -eq $before.Versions -and $after.Staged -eq 0) "(probe files: $probeFiles)"
        }
        'clean' {
            Make-Chain
            $before = Snapshot; Say ("before: " + (Show $before))
            $r = Run-Live 'clean' (New-Tag 'clean') (Tail 'clean')
            $after = Snapshot; Say ("after:  " + (Show $after)); Say "exit $($r.Code) in $($r.Seconds) s; error: $($r.Error)"
            $committed = ($after.Versions -ne $before.Versions)
            $tail = (Tail 'clean')
            $sets = [int](Scalar "SELECT COUNT(*) FROM msdb.dbo.backupset bs JOIN msdb.dbo.backupmediafamily mf ON mf.media_set_id = bs.media_set_id WHERE mf.physical_device_name = N'$tail' AND bs.type = 'L'")
            Check 'clean: nothing wrong, so the gate passes and the promotion is committed; the database is online' ($committed -and $after.State -eq 'ONLINE') "(exit $($r.Code); tail backup sets: $sets; exit 1 with 57234 is the F-5 readiness gate, which the old and the new tool share until F-5 is fixed)"
        }
    }
}
if ($failed.Count) { Say "RESULT: $($failed.Count) check(s) failed: $($failed -join '; ')"; exit 1 }
Say 'RESULT: all checks passed'
