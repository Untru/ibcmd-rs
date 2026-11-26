# Our import + the drop-in S1 apply against the platform's twin, on the cases of the ddl track (issue #388 step 2; #391).
#
#   pwsh -NoProfile -File run_s1_case.ps1 -Case b1|c1|d1 -Exe F:\...\ibcmd-rs.exe
#   pwsh -NoProfile -File run_s1_case.ps1 -Case attrdel -Edits attrdel -Exe F:\...\ibcmd-rs.exe
#
# The cases are rcheck's (F:\ibcmd\lab\04\restructure-check\dn\<case>: `stage` holds the edited descriptors, `files.txt`
# lists them, `before` holds the originals; only read). The full tree of a case is the lab's БСП 8.3.27 base tree with
# those files put over it. With -Edits the tree is the base tree with the edits of `edits.py` applied, and the files
# the platform stages are the descriptors those edits touched.
#   native twin: a fresh clone, the platform's `config import files --partial` of the listed files, the platform's apply.
#   own twin:    a fresh clone, our drop-in `config import` of the full tree (a patch stage, the override on, the guard on),
#                our drop-in `config apply --recovery-backup=<file>`.
# Checks against the native twin, numbered as in rcheck's protocol (docs/apply/evidence/dropin-apply/s1-acceptance.md):
#   4  the Config rows: same names, same content (compare_config_content.py; the bytes differ by construction, since the
#      two stages are not the same deflate streams and generations)
#   7  the platform's apply on our twin says nothing is required
#   8  the platform's exports of both, `source-diff` (and of ours against the tree)
#   3  the data of the rebuilt tables (rcheck's compare_tables.ps1), when the apply reports tables
# Results: out\s1\<case>.json. Lab databases only (ibcmd_rs_04_import_*).
param(
    [Parameter(Mandatory = $true)][ValidatePattern('^[a-z0-9]+$')][string]$Case,
    [string]$Edits = '',
    [Parameter(Mandatory = $true)][string]$Exe,
    [string]$Cases = 'F:\ibcmd\lab\04\restructure-check\dn',
    [string]$Tag = '',
    # The twins of an earlier run (the platform's applied, ours staged and not applied): only our apply and the checks run.
    [switch]$ReuseTwins,
    # An applied native twin of an earlier run of the same case, and its export folder: only our twin is made.
    [string]$NativeTwin = '',
    [string]$NativeExport = ''
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'; $env:PYTHONIOENCODING = 'utf-8'; $env:PYTHONDONTWRITEBYTECODE = '1'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
. "$here\native.ps1"
$lab = $script:Lab
if (-not $Tag) { $Tag = "dn$Case" }
$nat = if ($NativeTwin) { $NativeTwin } else { "ibcmd_rs_04_import_${Tag}_nat" }
$own = "ibcmd_rs_04_import_${Tag}_own"
Assert-LabDb $nat; Assert-LabDb $own
$outDir = "$lab\out\s1"
New-Item -ItemType Directory -Force $outDir, "$lab\bak" | Out-Null
$free = (Get-PSDrive F).Free / 1GB
if ($free -lt 25) { throw "F: has $([math]::Round($free,1)) GB free; not starting" }
$sw = [Diagnostics.Stopwatch]::StartNew()
$report = [ordered]@{ case = $Case; tag = $Tag; exe = $Exe; native_twin = $nat; own_twin = $own }
function Log($m) { "[{0}] {1}" -f (Get-Date -Format s), $m }
function Save { $report | ConvertTo-Json -Depth 8 | Set-Content "$outDir\$Tag.json" -Encoding UTF8 }

# 1. the full tree of the case
$tree = "$lab\tree\dn_$Case"
robocopy "$lab\tree\base" $tree /MIR /NFL /NDL /NJH /NJS /NP /MT:8 | Out-Null
$notBase = @()
if ($Edits) {
    $stageDir = $tree
    if (Test-Path "$tree.touched.json") { Remove-Item "$tree.touched.json" -Force }
    python "$here\edits.py" apply $Edits --base "$lab\tree\base" --work $tree | Out-Null
    $touched = Get-Content "$tree.touched.json" -Raw -Encoding UTF8 | ConvertFrom-Json
    $files = @($touched | Where-Object { $_ -like '*.xml' -and $_ -notmatch '/(Forms|Templates)/' })
} else {
    $stageDir = "$Cases\$Case\stage"
    $files = @(Get-Content "$Cases\$Case\files.txt" -Encoding UTF8 | Where-Object { $_ })
    foreach ($f in $files) {
        $rel = $f -replace '/', '\'
        $before = "$Cases\$Case\before\$rel"
        if ((Test-Path $before) -and ((Get-FileHash $before).Hash -ne (Get-FileHash "$lab\tree\base\$rel").Hash)) { $notBase += $f }
        Copy-Item "$stageDir\$rel" "$tree\$rel" -Force
    }
}
Log "tree $tree ($($files.Count) staged files: $($files -join ', '))"
$report.edited_files = $files
$report.before_differs_from_base = $notBase
if ($notBase.Count) { Log "WARNING: the case's `before` differs from the lab base tree for: $($notBase -join ', ')" }

# 2. the native twin
if (-not $ReuseTwins) {
foreach ($db in $(if ($NativeTwin) { @($own) } else { @($nat, $own) })) {
    Log "restore $db"
    pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus bsp8327 -Name $db -Track import -Purpose "S1 case ${Case}: $(if ($db -eq $nat) { 'the platform twin' } else { 'our import and the drop-in apply' })" | Select-Object -Last 1
}
if (-not $NativeTwin) {
Log 'native import files --partial'
$ni = Invoke-NativeImportFiles -Db $nat -BaseDir $stageDir -Files $files -Tag "$Tag-nat"
$report.native_import = $ni
"native import: exit=$($ni.Exit) rows=$($ni.Rows) $($ni.Seconds)s"
if ($ni.Exit -ne 0) { Save; exit 1 }
Log 'native apply (native twin)'
$na = Invoke-NativeApply -Db $nat -Dynamic disable -Tag "$Tag-nat"
$report.native_apply = $na
"native apply: exit=$($na.Exit) $($na.Seconds)s"
if ($na.Exit -ne 0) { Save; exit 2 }
}
}

# 3. the own twin
if (-not $ReuseTwins) {
Log 'ours import (patch, override on, guard on)'
$oi = Invoke-OursImport -Db $own -Tree $tree -Tag "$Tag-own" -Exe $Exe
} else { $oi = @{ Exit = 0; Seconds = 0; Rows = (Get-ConfigSaveRows $own); Tail = 'reused' } }
$stage = Get-Content "$lab\out\import-$Tag-own.json" -Raw -ErrorAction SilentlyContinue | ConvertFrom-Json
$report.own_import = [ordered]@{
    exit = $oi.Exit; seconds = $oi.Seconds; rows = $oi.Rows; tail = $oi.Tail
    overrides = if ($stage) { $stage.overrides } else { $null }
    verification = if ($stage) { $stage.verification } else { $null }
}
"own import: exit=$($oi.Exit) rows=$($oi.Rows) $($oi.Seconds)s"
if ($stage -and $stage.overrides) { "  overrides: $($stage.overrides | ConvertTo-Json -Compress)" }
if ($stage -and $stage.verification) { "  guard: $($stage.verification.checked_files) files" }
if ($oi.Exit -ne 0) { Save; exit 3 }
$bak = "$lab\bak\${Tag}_own_before_apply.bak"
if (Test-Path $bak) { [IO.File]::Delete($bak) }
Log 'ours apply --recovery-backup'
$oa = Invoke-OursApply -Db $own -Tag "$Tag-own" -Exe $Exe -Extra @("--recovery-backup=$bak")
$rep = Get-Content "$lab\out\apply-$Tag-own.json" -Raw -ErrorAction SilentlyContinue | ConvertFrom-Json
$report.own_apply = [ordered]@{
    exit = $oa.Exit; seconds = $oa.Seconds; tail = $oa.Tail
    gate = if ($rep -and $rep.apply) { $rep.apply.gate } else { $null }
    backup = if ($rep -and $rep.apply) { $rep.apply.backup } else { $null }
    structure = if ($rep -and $rep.apply) { $rep.apply.structure } else { $null }
}
"own apply: exit=$($oa.Exit) $($oa.Seconds)s $($oa.Tail)"
if ($oa.Exit -ne 0) { Save; exit 4 }
$tables = if ($rep -and $rep.apply.structure.tables) { @($rep.apply.structure.tables) -join ',' } else { '' }

# 4. the checks
Log 'check 4: Config rows by content'
$c4 = python "$here\compare_config_content.py" $nat $own --out "$outDir\${Tag}_check4.txt"
$report.check4 = ($c4 | Select-Object -First 60) -join "`n"
$c4 | Select-Object -First 40 | ForEach-Object { "  $_" }
Log 'check 7: the platform applies on our twin'
$n7 = Invoke-NativeApply -Db $own -Dynamic disable -Tag "$Tag-own-noop"
$noop = Get-Content "$lab\logs\native-apply-$Tag-own-noop.out.txt", "$lab\logs\native-apply-$Tag-own-noop.err.txt" -ErrorAction SilentlyContinue -Encoding UTF8
$report.check7 = [ordered]@{ exit = $n7.Exit; seconds = $n7.Seconds; not_required = [bool]($noop | Select-String 'не требуется'); tail = $n7.Tail }
"check 7: exit=$($n7.Exit) not required: $($report.check7.not_required)"
Log 'check 8: native exports and source-diff'
$natExport = if ($NativeExport) { $NativeExport } else { "$lab\out\export\${Tag}_nat" }
foreach ($side in 'nat', 'own') {
    if ($side -eq 'nat' -and $NativeExport) { continue }
    $out = "$lab\out\export\${Tag}_$side"
    if (Test-Path $out) { throw "$out exists" }
    $e = Invoke-NativeExport -Db $(if ($side -eq 'nat') { $nat } else { $own }) -OutDir $out -Tag "$Tag-$side"
    $report["export_$side"] = $e
    "export $side`: exit=$($e.Exit) $($e.Seconds)s"
}
foreach ($pair in @(@('nat_vs_own', $natExport, "$lab\out\export\${Tag}_own"), @('tree_vs_own', $tree, "$lab\out\export\${Tag}_own"))) {
    $diff = "$lab\out\export\${Tag}_$($pair[0]).diff.json"
    & $Exe source-diff -o $diff $pair[1] $pair[2] 2>&1 | Out-Null
    $d = Get-Content $diff -Raw | ConvertFrom-Json
    $others = @($d.differences | Where-Object { $_.status -ne 'unchanged' -and $_.path -ne 'ConfigDumpInfo.xml' })
    $report["diff_$($pair[0])"] = [ordered]@{ summary = $d.summary; other_than_dump_info = @($others | ForEach-Object { "{0} {1}" -f $_.status, $_.path }) }
    "source-diff $($pair[0]): $($d.summary | ConvertTo-Json -Compress); other than ConfigDumpInfo.xml: $($others.Count)"
    $others | Select-Object -First 20 | ForEach-Object { "    {0} {1}" -f $_.status, $_.path }
}
if ($tables) {
    Log 'check 3: the rebuilt tables'
    $c3 = pwsh -NoProfile -File F:\ibcmd\src\ibcmd-rs-04-restructure\scripts\restructure-lab\compare_tables.ps1 -A $nat -B $own -Tables $tables -Out "$outDir\${Tag}_check3.txt"
    $report.check3 = ($c3 | Select-Object -First 40) -join "`n"
    $c3 | Select-Object -First 20 | ForEach-Object { "  $_" }
}
$report.total_seconds = [math]::Round($sw.Elapsed.TotalSeconds, 1)
Save
Log "done $Case in $($report.total_seconds) s"
