# The route through OUR import for a combination case whose platform route ran before (mix_case.ps1), with ONE native write per case
# (the check 7) instead of four: the native lock is a queue of all tracks, and the case's staged state and the platform's result are
# still there -- the staged state as bak\ibcmd_rs_04_ddl_s2_<case>_base_<case>_staged.bak, the platform's result as the snapshot
# <nat> / nat_after (the tables' data of the native twin is gone with its database; the pass with mix_case.ps1 compared it to the drop-in
# twin, 0 rows on either side, so the drop-in twin stands in for it below).
#   pwsh -NoProfile -File mix_ours.ps1 -Case m2 [-Exe <ibcmd-rs.exe>] [-Cleanup] [-Phase all|prepare|finish]
#   1. the drop-in twin: `ibcmd-rs infobase config apply --recovery-backup` on a clone restored from the staged backup; the checks 2, 5, 6 of
#      its snapshot against the platform's (snapshot nat_after)
#   2. the route through our import: a clone restored from pristine БСП, `ibcmd-rs infobase config import <the full tree>` (the base tree, the
#      edited descriptors, the forms without the fields bound to removed attributes), then the drop-in apply; the check 3 (EXCEPT both ways of
#      the rebuilt tables) against the drop-in twin, the checks 2, 5, 6 of its snapshot against the platform's, the check 4 counted (informative:
#      our import re-compiles rows, #395), the check 7 (the platform's apply afterwards: the one native write), the check 8 (the platform's
#      exports of both, source-diff)
# Everything goes to out\ours_<Case>\ and the lines that matter to out\ours_<Case>\summary.txt.
param(
    [Parameter(Mandatory = $true)][string]$Case,
    [string]$Exe = 'F:\ibcmd\lab\04\restructure\bin\ibcmd-rs-mix2.exe',
    [switch]$Cleanup,
    # the drop-in twin is there and applied already (a case that ran with mix_case.ps1 and was not cleaned)
    [switch]$KeepOwn,
    # our twin is there and applied already (mix_case.ps1 -Ours): only the checks are made
    [switch]$KeepOurs,
    # prepare: everything but the native write (the drop-in twin, our import, the drop-in apply, the checks 2-6 and the count of 4);
    # finish: the check 7 (the one native write, queued once), the exports and the check 8, the cleanup; all: both
    [ValidateSet('all', 'prepare', 'finish')][string]$Phase = 'all'
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'; $env:PYTHONIOENCODING = 'utf-8'
$kit = $PSScriptRoot
. "$kit\native_lock.ps1"
. "$kit\verdicts.ps1"
$lab = if ($env:DDL_LAB) { $env:DDL_LAB } else { 'F:\ibcmd\lab\04\restructure' }
$base = "ibcmd_rs_04_ddl_s2_${Case}_base"
$nat = "ibcmd_rs_04_ddl_s2_${Case}_nat"
$own = "ibcmd_rs_04_ddl_s2_${Case}_own"
$oursDb = "ibcmd_rs_04_ddl_s2_${Case}_ours"
$o = "$lab\out\ours_$Case"
New-Item -ItemType Directory -Force $o | Out-Null
$summary = "$o\summary.txt"
if ($Phase -ne 'finish') { "case $Case, the route through our import, binary $Exe" | Set-Content $summary -Encoding UTF8 }
function Log($m) { $line = "[{0}] {1}" -f (Get-Date -Format s), $m; $line; $line | Add-Content "$o\log.txt" -Encoding UTF8 }
function Note($m) { Log $m; $m | Add-Content $summary -Encoding UTF8 }
$bak = "$lab\bak\${base}_${Case}_staged.bak"
if (-not $KeepOwn -and -not (Test-Path $bak)) { throw "no staged backup $bak" }

# checks 2, 5, 6 of a snapshot of $db (label $label) against the platform's snapshot
function Compare-WithNative($db, $label, $tag) {
    Push-Location $kit
    try {
        python snapshot.py $db $label | Select-Object -Last 1
        python snapdiff.py $nat nat_after $db $label > "$o\${tag}_diff.txt"
        python dbschema_cmp.py $nat nat_after $db $label > "$o\${tag}_dbschema.txt"
        python si_diff.py $nat nat_after $db $label > "$o\${tag}_si.txt"
    } finally { Pop-Location }
    foreach ($f in "${tag}_diff", "${tag}_dbschema", "${tag}_si") {
        Get-Content "$o\$f.txt" | Select-String -Pattern '^(changed|\d+ of 16|DBNames text|entries that differ|only in|added|removed)' | ForEach-Object { Note "${tag}: $($_.Line)" }
    }
}

$oursData = "$lab\ibdata\$oursDb"
$ownData = "$lab\ibdata\$own"
if ($Phase -ne 'finish') {
if (-not $KeepOwn) {
    Log 'the drop-in twin from the staged backup'
    pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus bak -Bak $bak -Name $own -Track ddl -Purpose "S1 mix $Case : the drop-in twin, the reference of the route through our import" | Select-Object -Last 1
    & $Exe mssql-config-apply --platform-profile platform-8.3.27.2214 --database $own --allow-restructure s1 --allow-non-lab --dry-run --report "$o\dry.json" > "$o\dry.txt" 2> "$o\dry.err"
    Note "dry run exit $LASTEXITCODE"
    if ($LASTEXITCODE -ne 0) { Get-Content "$o\dry.err" -Tail 6; throw 'the dry run refused' }
    Note ("plan: " + ((Get-Content "$o\dry.json" -Raw -Encoding UTF8 | ConvertFrom-Json).structure.objects -join ' | '))
    New-Item -ItemType Directory -Force $ownData | Out-Null
    $backup = "$lab\bak\ours_${Case}_own_$([Guid]::NewGuid().ToString('N').Substring(0, 8)).bak"
    $args1 = @('infobase', 'config', 'apply', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$own", "--data=$ownData",
               '--user=Администратор', '--force', '--dynamic=disable', "--recovery-backup=$backup")
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $rc = Invoke-NativeCommand -Ibcmd $Exe -Arguments $args1 -Log "$o\apply" -TimeoutSec 3600 -NoLock
    Note "drop-in exit $rc in $([math]::Round($sw.Elapsed.TotalSeconds, 1))s"
    if (Test-Path $backup) { Remove-Item -Force $backup }
    if ($rc -ne 0) { Get-Content "$o\apply.err" -Tail 4 -Encoding UTF8; throw 'the drop-in apply failed' }
    Compare-WithNative $own 'own_after' 'own'
}
$dryJson = if ($KeepOwn) { "$lab\out\mix_$Case\dry.json" } else { "$o\dry.json" }
$tables = @((Get-Content $dryJson -Raw -Encoding UTF8 | ConvertFrom-Json).structure.tables) -join ','

if (-not $KeepOurs) {
Log 'the route through our import'
$tree = "$lab\tree_full\$Case"
New-Item -ItemType Directory -Force "$lab\tree_full" | Out-Null
# MIX_BASE_BAK / MIX_BASE_TREE: the base that already had a native apply and its native export (the cases that create an object)
$baseTree = if ($env:MIX_BASE_TREE) { $env:MIX_BASE_TREE } else { 'F:\ibcmd\lab\04\import\tree\base' }
$baseArgs = if ($env:MIX_BASE_BAK) { @('-Corpus', 'bak', '-Bak', $env:MIX_BASE_BAK) } else { @('-Corpus', 'bsp8327') }
robocopy $baseTree $tree /MIR /NFL /NDL /NJH /NJS /NP /MT:16 | Out-Null
robocopy "$lab\tree_s2\$Case\stage" $tree /E /NFL /NDL /NJH /NJS /NP | Out-Null
if (Test-Path "$lab\tree_s2\$Case\stage_full") { robocopy "$lab\tree_s2\$Case\stage_full" $tree /E /NFL /NDL /NJH /NJS /NP | Out-Null }
pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 @baseArgs -Name $oursDb -Track ddl -Purpose "S1 mix $Case : our import of the full tree, then the drop-in apply" | Select-Object -Last 1
New-Item -ItemType Directory -Force $oursData | Out-Null
$sw = [Diagnostics.Stopwatch]::StartNew()
$importOut = & $Exe infobase config import --dbms=MSSQLServer --db-server=localhost "--db-name=$oursDb" "--data=$oursData" "--report=$o\ours_import.json" $tree 2>&1
$importExit = $LASTEXITCODE
$importOut | ForEach-Object { "$_" } | Set-Content "$o\ours_import.log" -Encoding UTF8
$rows = sqlcmd -S localhost -E -C -h -1 -W -d $oursDb -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM ConfigSave"
Note "our import exit $importExit in $([math]::Round($sw.Elapsed.TotalSeconds, 1))s, $($rows.Trim()) rows in ConfigSave"
if ($importExit -ne 0) { $importOut | Select-Object -Last 12 | ForEach-Object { Note "  ours import: $_" }; throw 'our import failed' }
$backupOurs = "$lab\bak\ours_${Case}_ours_$([Guid]::NewGuid().ToString('N').Substring(0, 8)).bak"
$args2 = @('infobase', 'config', 'apply', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$oursDb", "--data=$oursData",
           '--user=Администратор', '--force', '--dynamic=disable', "--recovery-backup=$backupOurs")
$sw = [Diagnostics.Stopwatch]::StartNew()
$rc2 = Invoke-NativeCommand -Ibcmd $Exe -Arguments $args2 -Log "$o\ours_apply" -TimeoutSec 3600 -NoLock
Note "ours: drop-in exit $rc2 in $([math]::Round($sw.Elapsed.TotalSeconds, 1))s"
Get-Content "$o\ours_apply.out" -Tail 3 -Encoding UTF8 -ErrorAction SilentlyContinue | ForEach-Object { Note "  ours apply.out: $_" }
Get-Content "$o\ours_apply.err" -Tail 4 -Encoding UTF8 -ErrorAction SilentlyContinue | Where-Object { $_.Trim() } | ForEach-Object { Note "  ours apply.err: $_" }
if (Test-Path $backupOurs) { Remove-Item -Force $backupOurs }
if ($rc2 -ne 0) { throw 'the drop-in apply on our import failed' }

}
pwsh -NoProfile -File "$kit\compare_tables.ps1" -A $own -B $oursDb -Tables $tables -Out "$o\ours_except.txt" *> $null
Assert-LabCommand $LASTEXITCODE 'rebuilt-table comparison'
Assert-LabExcept "$o\ours_except.txt" @($tables -split ',')
$except = Get-Content "$o\ours_except.txt" | Select-String -Pattern 'only in A [1-9]|only in B [1-9]'
Note ("ours: check 3 (EXCEPT both ways, $(@($tables -split ',').Count) tables): " + $(if ($except) { "DIFFERENCES: $($except.Count) tables" } else { 'no row differs' }))
Compare-WithNative $oursDb 'ours_after' 'ours'
Push-Location $kit
python config_compare.py $own $oursDb > "$o\ours_config.txt" 2>&1
Pop-Location
Get-Content "$o\ours_config.txt" | Select-String -Pattern '^(rows|equal bytes|different text by kind)' | ForEach-Object { Note "ours: check 4 (Config rows, informative until #395): $($_.Line)" }
}
if ($Phase -ne 'prepare') {
Log 'check 7: the platform apply on our twin'
pwsh -NoProfile -File "$kit\apply_only.ps1" -Database $oursDb *> "$o\ours_noop.txt"
Assert-LabNoop $LASTEXITCODE "$o\ours_noop.txt"
Get-Content "$o\ours_noop.txt" | Select-String -Pattern 'не требуется|exit=' | ForEach-Object { Note "ours: check 7: $($_.Line)" }
Log 'check 8: exports'
foreach ($db in $own, $oursDb) {
    Remove-Item -Recurse -Force "$lab\export\$db" -ErrorAction SilentlyContinue
    pwsh -NoProfile -File "$kit\export_tree.ps1" -Database $db -Out "$lab\export\$db" 2>&1 | Select-Object -Last 1 | ForEach-Object { Note "ours: export $db : $_" }
    Assert-LabCommand $LASTEXITCODE "native export of $db"
}
& $Exe source-diff "$lab\export\$own" "$lab\export\$oursDb" > "$o\ours_export_diff.json" 2>&1
Assert-LabExport $LASTEXITCODE "$o\ours_export_diff.json" -AllowDifferences
$diff = python -c "import json; d=json.load(open(r'$o\ours_export_diff.json',encoding='utf-8')); print(d['summary'])"
Note "ours: check 8 (source-diff of the drop-in twin and ours): $diff"
python "$kit\diff_paths.py" "$o\ours_export_diff.json" "$lab\tree_s2\$Case\forms.txt" "$lab\export\$oursDb" "$lab\tree_s2\$Case\stage_full" | ForEach-Object { Note "ours: check 8: $_" }
if ($LASTEXITCODE -ne 0) { throw 'check 8: exported differences exceed the declared forms and ConfigDumpInfo.xml' }

if ($Cleanup) {
    Log 'cleanup'
    pwsh -NoProfile -File F:\ibcmd\lab\04\tools\drop-lab-dbs.ps1 -Track ddl -Names "$base,$nat,$own,$oursDb" -MinIdleMinutes 1 -Execute 2>&1 | Select-Object -Last 4
    foreach ($db in $nat, $own, $oursDb) {
        foreach ($root in 'export', 'ibdata') { if (Test-Path "$lab\$root\$db") { Remove-Item -Recurse -Force "$lab\$root\$db" } }
    }
}
}
Note "done $Case ($Phase)"
