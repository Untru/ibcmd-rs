# One case of the S1 combination matrix, end to end through the drop-in route (`ibcmd-rs infobase config apply --recovery-backup=<file>`):
# stage the case on a pristine БСП clone with the platform's partial import (stage_case.ps1, tree_s2\<case> made by edit_cases_s4.py), and then
#   a case that must work:  the platform's apply on the native twin (snapshot nat_after), a dry run of ours (the plan), the drop-in, then the checks of
#                           docs/apply/restructuring.md 12.6 that matter here: 2-6 (twin_check.ps1: tables, EXCEPT both ways, Config, DBSchema and DBNames,
#                           the .si rows), 7 (the platform's apply afterwards) and 8 (the platform's exports of both, source-diff)
#   a case that must be refused (-Refused): the drop-in must exit 1 and leave the database exactly as it was (the digest of check 12 before and after)
#   pwsh -NoProfile -File mix_case.ps1 -Case m1 [-Refused] [-Exe <ibcmd-rs.exe>] [-Cleanup] [-SkipStage]
# Everything goes to out\mix_<Case>\ and the lines that matter to out\mix_<Case>\summary.txt.
param(
    [Parameter(Mandatory = $true)][string]$Case,
    [switch]$Refused,
    [string]$Exe = 'F:\ibcmd\lab\04\restructure\bin\ibcmd-rs-mix.exe',
    [switch]$Cleanup,
    [switch]$SkipStage,
    [switch]$SkipNative,
    [switch]$NativeOnly,
    # The route through OUR import as well: a third twin (_ours) is staged by `ibcmd-rs infobase config import` of the full tree (the base
    # tree, the edited descriptors, the forms without the fields bound to removed attributes), then the drop-in apply, then the checks 3, 7 and
    # 8 against the native twin (the check 4 differs by construction until #395: our import re-compiles about 3 000 rows).
    [switch]$Ours
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'
$kit = $PSScriptRoot
. "$kit\native_lock.ps1"
. "$kit\verdicts.ps1"
$lab = if ($env:DDL_LAB) { $env:DDL_LAB } else { 'F:\ibcmd\lab\04\restructure' }
$base = "ibcmd_rs_04_ddl_s2_${Case}_base"
$oursDb = "ibcmd_rs_04_ddl_s2_${Case}_ours"
$nat = "ibcmd_rs_04_ddl_s2_${Case}_nat"
$own = "ibcmd_rs_04_ddl_s2_${Case}_own"
$o = "$lab\out\mix_$Case"
New-Item -ItemType Directory -Force $o | Out-Null
$summary = "$o\summary.txt"
"case $Case ($(if ($Refused) { 'must be refused' } else { 'must work' })), binary $Exe" | Set-Content $summary -Encoding UTF8
function Log($m) { $line = "[{0}] {1}" -f (Get-Date -Format s), $m; $line; $line | Add-Content "$o\log.txt" -Encoding UTF8 }
function Note($m) { Log $m; $m | Add-Content $summary -Encoding UTF8 }

$digestSql = @"
SET NOCOUNT ON;
SELECT 'Config rows', COUNT_BIG(*) FROM dbo.Config
UNION ALL SELECT 'ConfigSave rows', COUNT_BIG(*) FROM dbo.ConfigSave
UNION ALL SELECT 'Params rows', COUNT_BIG(*) FROM dbo.Params
UNION ALL SELECT 'Config checksum', CHECKSUM_AGG(CHECKSUM(FileName, PartNo, DataSize, Creation, Modified)) FROM dbo.Config
UNION ALL SELECT 'ConfigSave checksum', CHECKSUM_AGG(CHECKSUM(FileName, PartNo, DataSize)) FROM dbo.ConfigSave
UNION ALL SELECT 'Params checksum', CHECKSUM_AGG(CHECKSUM(FileName, PartNo, DataSize, Modified)) FROM dbo.Params
UNION ALL SELECT 'SchemaStorage 0 status', Status FROM dbo.SchemaStorage WHERE SchemaID = 0
UNION ALL SELECT 'SchemaStorage 0 schema', CHECKSUM(CONVERT(varchar(64), HASHBYTES('SHA2_256', CurrentSchema), 2)) FROM dbo.SchemaStorage WHERE SchemaID = 0
UNION ALL SELECT 'DBSchema', CHECKSUM(CONVERT(varchar(64), HASHBYTES('SHA2_256', SerializedData), 2)) FROM dbo.DBSchema
UNION ALL SELECT 'DBNames', CHECKSUM(CONVERT(varchar(64), HASHBYTES('SHA2_256', BinaryData), 2)) FROM dbo.Params WHERE FileName = N'DBNames' AND PartNo = 0
UNION ALL SELECT 'columns', CHECKSUM_AGG(CHECKSUM(t.name, c.name, c.max_length, c.is_nullable, c.column_id)) FROM sys.tables t JOIN sys.columns c ON c.object_id = t.object_id
UNION ALL SELECT 'indexes', CHECKSUM_AGG(CHECKSUM(t.name, i.name, i.type, i.is_unique)) FROM sys.tables t JOIN sys.indexes i ON i.object_id = t.object_id
UNION ALL SELECT 'tables *NG', COUNT(*) FROM sys.tables WHERE name LIKE N'%NG'
UNION ALL SELECT 'tables', COUNT(*) FROM sys.tables;
"@
function Get-Digest($db) { sqlcmd -S localhost -E -C -b -d $db -h -1 -W -s '|' -Q $digestSql }

if (-not $SkipStage) {
    Log 'stage the case (stage_case.ps1)'
    pwsh -NoProfile -File "$kit\stage_case.ps1" -Case $Case *> "$o\stage.log"
    if ($LASTEXITCODE -ne 0) { Get-Content "$o\stage.log" -Tail 5; throw 'stage_case failed' }
    Note ("staged: " + ((Get-Content "$o\stage.log" | Select-String 'done:') -join ' '))
}
Push-Location $kit
try {
    if (-not $Refused -and -not $SkipNative) {
        Log 'the platform apply on the native twin'
        pwsh -NoProfile -File "$kit\apply_only.ps1" -Database $nat *> "$o\native_apply.txt"
        $nativeExit = ((Get-Content "$o\native_apply.txt" | Select-String 'exit=(-?\d+)' | Select-Object -Last 1).Matches.Groups[1].Value)
        Note "native apply exit $nativeExit"
        if ($nativeExit -ne '0') { Get-Content "$o\native_apply.txt" -Tail 8; throw 'the platform apply failed: the case is not one the platform accepts' }
        python snapshot.py $nat nat_after | Select-Object -Last 1
        python snapdiff.py $base "${Case}_staged" $nat nat_after > "$o\native_diff.txt"
    }
} finally { Pop-Location }

if ($NativeOnly) { Note "native side only: done $Case"; return }
Log 'a dry run of ours (the plan)'
& $Exe mssql-config-apply --platform-profile platform-8.3.27.2214 --database $own --allow-restructure s1 --allow-non-lab --dry-run --report "$o\dry.json" > "$o\dry.txt" 2> "$o\dry.err"
$dryExit = $LASTEXITCODE
Note "dry run exit $dryExit"
if ($dryExit -eq 0) {
    $dry = Get-Content "$o\dry.json" -Raw -Encoding UTF8 | ConvertFrom-Json
    Note ("plan: " + (($dry.structure.objects) -join ' | '))
    Note ("tables: " + @($dry.structure.tables).Count)
    Note ("caches: " + (($dry.structure.caches) -join ' | '))
} else {
    $errs = Get-Content "$o\dry.err" -Encoding UTF8 -ErrorAction SilentlyContinue | Where-Object { $_.Trim() } | Select-Object -First 4
    $errs | ForEach-Object { Note "  dry.err: $_" }
    if (Test-Path "$o\dry.json") {
        $dry = Get-Content "$o\dry.json" -Raw -Encoding UTF8 | ConvertFrom-Json
        @($dry.gate.blockers | Where-Object { $_.row -eq '' } | ForEach-Object { $_.reason }) | ForEach-Object { Note "  blocker: $_" }
    }
}

$data = "$lab\ibdata\$own"
New-Item -ItemType Directory -Force $data | Out-Null
$backup = "$lab\bak\mix_${Case}_$([Guid]::NewGuid().ToString('N').Substring(0, 8)).bak"
$applyArgs = @('infobase', 'config', 'apply', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$own", "--data=$data",
               '--user=Администратор', '--force', '--dynamic=disable', "--recovery-backup=$backup")
$before = Get-Digest $own
Log 'the drop-in: ibcmd-rs infobase config apply --recovery-backup'
$sw = [Diagnostics.Stopwatch]::StartNew()
$rc = Invoke-NativeCommand -Ibcmd $Exe -Arguments $applyArgs -Log "$o\apply" -TimeoutSec 3600 -NoLock
$seconds = [math]::Round($sw.Elapsed.TotalSeconds, 1)
$backupSize = if (Test-Path $backup) { [math]::Round((Get-Item $backup).Length / 1MB) } else { 0 }
Note "drop-in exit $rc in ${seconds}s, backup file $(if ($backupSize) { "$backupSize MB" } else { 'not made' })"
Get-Content "$o\apply.out" -Tail 6 -Encoding UTF8 -ErrorAction SilentlyContinue | ForEach-Object { Note "  apply.out: $_" }
Get-Content "$o\apply.err" -Tail 4 -Encoding UTF8 -ErrorAction SilentlyContinue | Where-Object { $_.Trim() } | ForEach-Object { Note "  apply.err: $_" }
if ($backupSize) { Remove-Item -Force $backup }

if ($Refused) {
    $after = Get-Digest $own
    $same = ($before -join "`n") -eq ($after -join "`n")
    Note "refused as required: exit $rc (want 1); the database digest unchanged: $same"
    if (-not $same) { $before | Add-Content "$o\digest_before.txt"; $after | Add-Content "$o\digest_after.txt" }
    if ($dryExit -ne 1 -or $rc -ne 1 -or -not $same) { throw 'refusal acceptance failed (dry/apply must exit 1, digest must be unchanged)' }
} else {
    if ($rc -ne 0) { throw 'the drop-in apply failed' }
    Log 'checks 2-6'
    $env:TWIN_OUT = "$lab\out"
    pwsh -NoProfile -File "$kit\twin_check.ps1" -Case $Case -Nat $nat -Own $own -Report "$o\dry.json" -NatLabel nat_after *> "$o\twin_check.txt"
    Assert-LabCommand $LASTEXITCODE 'twin checks'
    $expectedTables = @((Get-Content "$o\dry.json" -Raw -Encoding UTF8 | ConvertFrom-Json).structure.tables)
    Assert-LabExcept "$lab\out\${Case}_twin\except.txt" $expectedTables
    Assert-LabConfig "$lab\out\${Case}_twin\config.txt"
    Get-Content "$o\twin_check.txt" | Select-String -Pattern '^(rebuilt|changed|\d+ of 16|DBNames text|entries that differ|only in)' | ForEach-Object { Note $_.Line }
    # twin_check prints the first lines of the .si comparison only; its own file has the count (S1-F: 15 of 16, c4629235 in the approximate order)
    Get-Content "$lab\out\${Case}_twin\si.txt" -ErrorAction SilentlyContinue | Select-String -Pattern '\d+ of \d+ `\.si` rows' | ForEach-Object { Note $_.Line }
    $except = Get-Content "$o\twin_check.txt" | Select-String -Pattern 'only in A [1-9]|only in B [1-9]'
    Note ("check 3 (EXCEPT both ways): " + $(if ($except) { "DIFFERENCES: $($except.Count) tables" } else { 'no row differs' }))
    $config = Get-Content "$o\twin_check.txt" | Select-String -Pattern '^== only in [AB] \(([1-9]\d*) rows\)'
    Note ("check 4 (Config rows): " + $(if ($config) { 'DIFFERENCES' } else { 'no row differs' }))
    Log 'check 7: the platform apply on our twin'
    pwsh -NoProfile -File "$kit\apply_only.ps1" -Database $own *> "$o\native_noop.txt"
    Assert-LabNoop $LASTEXITCODE "$o\native_noop.txt"
    Get-Content "$o\native_noop.txt" | Select-String -Pattern 'не требуется|exit=' | ForEach-Object { Note "check 7: $($_.Line)" }
    Log 'check 8: exports'
    foreach ($db in $nat, $own) {
        Remove-Item -Recurse -Force "$lab\export\$db" -ErrorAction SilentlyContinue
        pwsh -NoProfile -File "$kit\export_tree.ps1" -Database $db -Out "$lab\export\$db" 2>&1 | Select-Object -Last 1 | ForEach-Object { Note "export $db : $_" }
        Assert-LabCommand $LASTEXITCODE "native export of $db"
    }
    & $Exe source-diff "$lab\export\$nat" "$lab\export\$own" > "$o\export_diff.json" 2>&1
    Assert-LabExport $LASTEXITCODE "$o\export_diff.json"
    $diff = python -c "import json; d=json.load(open(r'$o\export_diff.json',encoding='utf-8')); print(d['summary'])"
    Note "check 8 (source-diff): $diff"
}
if ($Ours -and -not $Refused) {
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
    $oursData = "$lab\ibdata\$oursDb"
    New-Item -ItemType Directory -Force $oursData | Out-Null
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $importOut = & $Exe infobase config import --dbms=MSSQLServer --db-server=localhost "--db-name=$oursDb" "--data=$oursData" "--report=$o\ours_import.json" $tree 2>&1
    $importExit = $LASTEXITCODE
    $importOut | ForEach-Object { "$_" } | Set-Content "$o\ours_import.log" -Encoding UTF8
    $rows = sqlcmd -S localhost -E -C -h -1 -W -d $oursDb -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM ConfigSave"
    Note "our import exit $importExit in $([math]::Round($sw.Elapsed.TotalSeconds, 1))s, $($rows.Trim()) rows in ConfigSave"
    if ($importExit -ne 0) { $importOut | Select-Object -Last 12 | ForEach-Object { Note "  ours import: $_" }; throw 'our import failed' }
    else {
        $backupOurs = "$lab\bak\mix_${Case}_ours_$([Guid]::NewGuid().ToString('N').Substring(0, 8)).bak"
        $oursArgs = @('infobase', 'config', 'apply', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$oursDb", "--data=$oursData",
                      '--user=Администратор', '--force', '--dynamic=disable', "--recovery-backup=$backupOurs")
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $rc2 = Invoke-NativeCommand -Ibcmd $Exe -Arguments $oursArgs -Log "$o\ours_apply" -TimeoutSec 3600 -NoLock
        Note "ours: drop-in exit $rc2 in $([math]::Round($sw.Elapsed.TotalSeconds, 1))s"
        Get-Content "$o\ours_apply.out" -Tail 3 -Encoding UTF8 -ErrorAction SilentlyContinue | ForEach-Object { Note "  ours apply.out: $_" }
        Get-Content "$o\ours_apply.err" -Tail 4 -Encoding UTF8 -ErrorAction SilentlyContinue | Where-Object { $_.Trim() } | ForEach-Object { Note "  ours apply.err: $_" }
        if (Test-Path $backupOurs) { Remove-Item -Force $backupOurs }
        Assert-LabCommand $rc2 'drop-in apply after our import'
        if ($rc2 -eq 0) {
            $tables = @((Get-Content "$o\dry.json" -Raw -Encoding UTF8 | ConvertFrom-Json).structure.tables) -join ','
            pwsh -NoProfile -File "$kit\compare_tables.ps1" -A $nat -B $oursDb -Tables $tables -Out "$o\ours_except.txt" *> $null
            Assert-LabCommand $LASTEXITCODE 'rebuilt-table comparison'
            Assert-LabExcept "$o\ours_except.txt" @($tables -split ',')
            $except2 = Get-Content "$o\ours_except.txt" | Select-String -Pattern 'only in A [1-9]|only in B [1-9]'
            Note ("ours: check 3 (EXCEPT both ways): " + $(if ($except2) { "DIFFERENCES: $($except2.Count) tables" } else { 'no row differs' }))
            pwsh -NoProfile -File "$kit\apply_only.ps1" -Database $oursDb *> "$o\ours_noop.txt"
            Assert-LabNoop $LASTEXITCODE "$o\ours_noop.txt"
            Get-Content "$o\ours_noop.txt" | Select-String -Pattern 'не требуется|exit=' | ForEach-Object { Note "ours: check 7: $($_.Line)" }
            Remove-Item -Recurse -Force "$lab\export\$oursDb" -ErrorAction SilentlyContinue
            pwsh -NoProfile -File "$kit\export_tree.ps1" -Database $oursDb -Out "$lab\export\$oursDb" 2>&1 | Select-Object -Last 1 | ForEach-Object { Note "ours: export : $_" }
            Assert-LabCommand $LASTEXITCODE "native export of $oursDb"
            & $Exe source-diff "$lab\export\$nat" "$lab\export\$oursDb" > "$o\ours_export_diff.json" 2>&1
            Assert-LabExport $LASTEXITCODE "$o\ours_export_diff.json" -AllowDifferences
            $diff2 = python -c "import json; d=json.load(open(r'$o\ours_export_diff.json',encoding='utf-8')); print(d['summary'])"
            Note "ours: check 8 (source-diff against the platform's twin): $diff2"
            python "$kit\diff_paths.py" "$o\ours_export_diff.json" "$lab\tree_s2\$Case\forms.txt" "$lab\export\$oursDb" "$lab\tree_s2\$Case\stage_full" | ForEach-Object { Note "ours: check 8: $_" }
            Assert-LabCommand $LASTEXITCODE 'expected export paths after our import'
        }
    }
}
if ($Cleanup) {
    Log 'cleanup'
    pwsh -NoProfile -File F:\ibcmd\lab\04\tools\drop-lab-dbs.ps1 -Track ddl -Names "$base,$nat,$own,$oursDb" -MinIdleMinutes 1 -Execute 2>&1 | Select-Object -Last 4
    foreach ($db in $nat, $own, $oursDb) {
        foreach ($root in 'export', 'ibdata') { if (Test-Path "$lab\$root\$db") { Remove-Item -Recurse -Force "$lab\$root\$db" } }
    }
    if (Test-Path "$o\recovery") { Remove-Item -Recurse -Force "$o\recovery" }
}
Note "done $Case"
