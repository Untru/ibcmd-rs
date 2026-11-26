# S1-K, the twin protocol (docs/apply/restructuring.md, 12.6) for one case: the native side (native `import files --partial` of the
# edited files + native `config apply`) against OUR side (the twin our import staged in import_phase.ps1, then OUR apply: the
# drop-in `ibcmd infobase config apply --recovery-backup=<file>`; the dry run and the rehearsal before it are the research command's,
# `mssql-config-apply --allow-restructure s1`, which shares the apply).
#
#   pwsh -NoProfile -File twin_case.ps1 -Case b1 -Bin <ibcmd-rs.exe> [-Job jobs\s2_b1.bsl] [-Skip export,session,noop]
#   -NativeOnly / -SkipNative   the native side alone / the rest alone (native_side.ps1 runs the native sides while the own side stages)
#   -StageOwnNative   HARNESS VALIDATION ONLY: stage our twin with the native partial import instead of our import (it says nothing
#                     about our import; it exercises the rest of the chain)
#
# Checks: 10 (a rehearsal changes nothing), 3 (EXCEPT both ways: the rebuilt tables, Config, every table of the extension schema),
# 4 (Config rows; part of 3), 5 (DBSchema, DBNames), 6 (the 16 .si rows), 7 (a native apply on our twin), 8 (native export of both,
# ibcmd-rs source-diff), 9 (a cluster session on both, outputs compared). Check 1 (the offline plan) is the corpus tests'; 2 is the
# snapshot diff printed here; 11 and 12 are the refusal (check11.ps1, case i1) and the injected failure (check12.ps1, on the dry-run
# script and the way back the real apply takes: out\<case>\script_dry.sql, staged.bak).
param(
    [Parameter(Mandatory = $true)][string]$Case,
    [Parameter(Mandatory = $true)][string]$Bin,
    [switch]$StageOwnNative,
    # the native side alone (the native twin: restore, native import of the edited files, native apply, snapshot), so that it can
    # wait in the queue of the native lock while the own side is still being staged; the own side follows with -SkipNative
    [switch]$NativeOnly,
    [switch]$SkipNative,
    [string]$Job = '',
    [string[]]$Skip = @()
)
$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'
$env:PYTHONDONTWRITEBYTECODE = '1'
$lab = if ($env:S1K_LAB) { $env:S1K_LAB } else { 'F:\ibcmd\lab\05\ext\s1k' }
$env:S1K_LAB = $lab
$env:DDL_LAB = "$lab\snap"
$kit = $PSScriptRoot
$rk = Split-Path $kit -Parent
$o = "$lab\out\$Case"
$nat = "ibcmd_rs_05_ext_s1k_${Case}_nat"
$own = "ibcmd_rs_05_ext_s1k_${Case}_own"
function Log($m) { "[{0}] {1}" -f (Get-Date -Format s), $m }
function Exists($db) { (sqlcmd -S localhost -E -C -h -1 -W -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM sys.databases WHERE name = N'$db'").Trim() -ne '0' }
function Restore($db, $why) {
    if (-not (Exists $db)) {
        pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus bak -Bak "$lab\bak\base_applied.bak" -Name $db -Track ext -Purpose $why | Select-Object -Last 1
    }
}
$files = (Get-Content "$o\files.txt" -Encoding UTF8 | Where-Object { $_ }) -join ','
if (-not $files) { throw "no edit of case $Case in $o (run import_phase.ps1 first)" }

if (-not $SkipNative) {
    Log "native side ($nat)"
    Restore $nat "S1-K case $Case (native)"
    if ((sqlcmd -S localhost -E -C -d $nat -h -1 -W -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM ConfigSave").Trim() -ne '0') { "the native twin has a stage already" }
    else {
        # the import and the apply that follows it on this clone in ONE hold of the native lock (its queue is long)
        pwsh -NoProfile -File "$kit\native_main.ps1" -Database $nat -Action ImportApply -BaseDir "$o\stage" -Files $files | Select-String -Pattern 'exit|успешно|ошибк' | ForEach-Object { $_.Line }
    }
    python "$rk\snapshot.py" $nat "${Case}_nat_after" | Select-Object -Last 1
}
if ($NativeOnly) { Log "native side of $Case done"; return }

if ($StageOwnNative) {
    Log 'HARNESS VALIDATION: staging our twin natively'
    Restore $own "S1-K case $Case (own, staged natively for the harness)"
    pwsh -NoProfile -File "$kit\native_main.ps1" -Database $own -Action ImportFiles -BaseDir "$o\stage" -Files $files | Select-String -Pattern 'exit' | ForEach-Object { $_.Line }
}
python "$rk\snapshot.py" $own "${Case}_staged" | Select-Object -Last 1

$common = @('mssql-config-apply', '--platform-profile', 'platform-8.3.27.2214', '--database', $own, '--allow-restructure', 's1', '--allow-non-lab')
Remove-Item "$o\script_dry.sql", "$o\script_rehearse.sql", "$o\script_real.sql", "$o\staged.bak" -Force -ErrorAction SilentlyContinue   # an artifact of an earlier run is never overwritten
Remove-Item "$o\recovery" -Recurse -Force -ErrorAction SilentlyContinue
Log 'our apply: dry run'
& $Bin @common --dry-run --report "$o\dry.json" --script-output "$o\script_dry.sql" > "$o\dry.txt" 2> "$o\dry.err"
"dry exit $LASTEXITCODE"
if ($LASTEXITCODE -ne 0) { Get-Content "$o\dry.txt", "$o\dry.err" -Tail 6 | ForEach-Object { if ($_.Length -gt 400) { $_.Substring(0, 400) } else { $_ } }; throw 'the dry run refused' }
Log 'our apply: rehearsal (check 10)'
& $Bin @common --rehearse --i-have-a-backup --report "$o\rehearse.json" --script-output "$o\script_rehearse.sql" > "$o\rehearse.txt" 2> "$o\rehearse.err"
"rehearse exit $LASTEXITCODE"
python "$rk\snapshot.py" $own "${Case}_rehearsed" | Select-Object -Last 1
python "$rk\snapdiff.py" $own "${Case}_staged" $own "${Case}_rehearsed" > "$o\rehearse_diff.txt"
"check 10: " + ((Get-Content "$o\rehearse_diff.txt" -Encoding UTF8 | Select-String -Pattern '^changed \(0\)|^\(0\)' ) -join ' ')
Log 'our apply: real run (the drop-in `infobase config apply --recovery-backup`)'
# The way back the apply takes just before it writes is a full backup of the staged state: check12.ps1 restores its fresh twin from it.
$staged = "$o\staged.bak"
if (Test-Path -LiteralPath $staged) { Remove-Item -LiteralPath $staged -Force }
$data = "$lab\ibdata\$own"
New-Item -ItemType Directory -Force $data | Out-Null
& $Bin infobase config apply --dbms=MSSQLServer --db-server=localhost "--db-name=$own" "--data=$data" --force --dynamic=disable --exclusivity=assumed --platform=8.3.27 "--recovery-backup=$staged" "--report=$o\real.json" > "$o\real.txt" 2> "$o\real.err"
"real exit $LASTEXITCODE"
if ($LASTEXITCODE -ne 0) { Get-Content "$o\real.txt", "$o\real.err" -Tail 5 | ForEach-Object { if ($_.Length -gt 400) { $_.Substring(0, 400) } else { $_ } }; throw 'the real run failed' }
$report = Get-Content "$o\real.json" -Raw -Encoding UTF8 | ConvertFrom-Json
# the drop-in wraps the apply's report: {operation, ok, nothing_to_apply, apply: {...}}
$apply = if ($report.apply) { $report.apply } else { $report }
"structure: " + (($apply.structure.objects) -join ' | ')
python "$rk\snapshot.py" $own "${Case}_own_after" | Select-Object -Last 1

if ($SkipNative) {
    # the native side runs apart (native_side.ps1) and may still be waiting for the native lock; everything above needed none of it
    $deadline = (Get-Date).AddMinutes(360)
    while (-not (Test-Path "$o\native_side.log") -or -not (Select-String -Path "$o\native_side.log" -Pattern 'native side of .* done' -Quiet)) {
        if ((Get-Date) -gt $deadline) { throw "the native side of $Case never finished" }
        Start-Sleep -Seconds 20
    }
    Log "native side of $Case is done"
}
Log 'checks 2-6'
python "$rk\snapdiff.py" $nat "${Case}_nat_after" $own "${Case}_own_after" --max 12 > "$o\nat_vs_own.txt"
Get-Content "$o\nat_vs_own.txt" -Encoding UTF8 | Select-String -Pattern '^(changed|added|removed) \(|^  T |^\(\d+\)' | ForEach-Object { $_.Line }
$tables = @($apply.structure.tables)   # Config is check 4 (dates and the generation differ when the twins were staged apart)
python "$kit\compare_twins.py" $nat $own @tables 2>&1 | Tee-Object "$o\check3.txt" | Select-Object -Last 6
python "$kit\check4.py" $nat $own 2>&1 | Tee-Object "$o\check4.txt"
python "$kit\check_register.py" $nat $own 2>&1 | Tee-Object "$o\check_register.txt"
python "$kit\checks56.py" $nat "${Case}_nat_after" $own "${Case}_own_after" 2>&1 | Tee-Object "$o\check56.txt"

if ($Skip -notcontains 'noop') {
    Log 'check 7: a native config apply on our twin'
    pwsh -NoProfile -File "$kit\native_main.ps1" -Database $own -Action Apply *> "$o\native_noop.txt"
    "check 7: " + ((Get-Content "$o\native_noop.txt" -Encoding UTF8 | Select-String -Pattern 'не требуется|exit') -join ' | ')
}
if ($Skip -notcontains 'export') {
    Log 'check 8: native exports and source-diff'
    foreach ($db in $nat, $own) {
        Remove-Item -Recurse -Force "$lab\export\$db" -ErrorAction SilentlyContinue
        pwsh -NoProfile -File "$kit\native_main.ps1" -Database $db -Action Export -Out "$lab\export\$db" | Select-Object -First 1
    }
    python "$kit\check8.py" "$lab\export\$nat" "$lab\export\$own" 2>&1 | Tee-Object "$o\check8.txt"
    Remove-Item -Recurse -Force "$lab\export\$nat", "$lab\export\$own" -ErrorAction SilentlyContinue
}
if ($Job -and $Skip -notcontains 'session') {
    Log 'check 9: cluster session'
    $reg = 'F:\ibcmd\lab\04\tools\register-ib.ps1'
    foreach ($side in 'nat', 'own') {
        $db = if ($side -eq 'nat') { $nat } else { $own }
        pwsh -NoProfile -File $reg register -Database $db -Platform 8.3.27 -Track ext 2>&1 | Select-Object -Last 1
        try {
            pwsh -NoProfile -File "$rk\session_job.ps1" -Job $Job -Database $db -Epf F:\ibcmd\lab\04\restructure\probe\ddl_probe.epf -TimeoutSec 600 > "$o\session_$side.txt" 2>&1
        } finally {
            pwsh -NoProfile -File $reg unregister -Database $db -Platform 8.3.27 -Track ext 2>&1 | Select-Object -Last 1
        }
    }
    "check 9: session outputs identical: " + ((Get-Content "$o\session_nat.txt" -Raw) -eq (Get-Content "$o\session_own.txt" -Raw)) + " (" + (Get-Content "$o\session_own.txt").Count + " lines)"
}
Log "done $Case"
