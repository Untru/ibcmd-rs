# Acceptance of the patch stage that carries what the target's rows cannot (issue #388 step 2, #393 import side).
#
#   pwsh -NoProfile -File run_override_acceptance.ps1 -Exe F:\...\ibcmd-rs.exe -Database ibcmd_rs_04_import_bsp_add `
#        -Tree F:\...\tree\add5 -Tag add1 [-SkipApply] [-Platform 8.5] [-ApplyWith ours] [-ApplyArgs ...]
#
# A tree that differs from the clone's configuration goes through the drop-in `infobase config import` with its
# default flags (a patch stage that builds what the target's rows cannot carry, and the guard on). What it left in
# ConfigSave is summarized (`configsave_summary.py`: the `deleted` row, the dates, the parts, the counts); then the
# platform's own `config apply --force --dynamic=disable` and `config export` run inside the native lock, and the
# export is compared with the tree (`ibcmd-rs source-diff`; only ConfigDumpInfo.xml, which holds the fresh generation
# ids, may differ). -Platform 8.5 runs the native ibcmd of 8.5.1.1150 as the 8.5 administrator; -ApplyWith ours applies
# with the drop-in `infobase config apply` instead of the platform's. Results: out\override-acceptance\<tag>.json.
# Lab databases only (ibcmd_rs_04_import_*).
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Tree,
    [Parameter(Mandatory = $true)][string]$Tag,
    [switch]$SkipApply,
    [string]$Dynamic = 'disable',
    [ValidateSet('8.3.27', '8.5')][string]$Platform = '8.3.27',
    [ValidateSet('native', 'ours')][string]$ApplyWith = 'native',
    [string[]]$ApplyArgs = @(),
    # An edited fixture may retain its old dump-info. Supply the independent native export's file.
    [string]$DumpInfoReference = ''
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'; $env:PYTHONIOENCODING = 'utf-8'; $env:PYTHONDONTWRITEBYTECODE = '1'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
. "$here\native.ps1"
Set-NativePlatform $Platform
Assert-LabDb $Database
$lab = $script:Lab
$outDir = "$lab\out\override-acceptance"
New-Item -ItemType Directory -Force $outDir | Out-Null
foreach ($artifact in @("$outDir\$Tag.json", "$lab\out\import-$Tag-import.json", "$lab\out\export\$Tag", "$lab\out\export\$Tag.diff.json")) {
    if (Test-Path -LiteralPath $artifact) { throw "acceptance artifact already exists: $artifact" }
}
$free = (Get-PSDrive F).Free / 1GB
if ($free -lt 25) { throw "F: has $([math]::Round($free,1)) GB free; not starting" }
$sw = [Diagnostics.Stopwatch]::StartNew()
$report = [ordered]@{ tag = $Tag; database = $Database; tree = $Tree; exe = $Exe; platform = $Platform; apply_with = $ApplyWith }

# 1. ours
$r = Invoke-OursImport -Db $Database -Tree $Tree -Tag "$Tag-import" -Exe $Exe
$stage = Get-Content "$lab\out\import-$Tag-import.json" -Raw -ErrorAction SilentlyContinue | ConvertFrom-Json
$report.import = [ordered]@{
    exit = $r.Exit; seconds = $r.Seconds; rows = $r.Rows; tail = $r.Tail
    overrides = if ($stage) { $stage.overrides } else { $null }
    verification = if ($stage) { $stage.verification } else { $null }
    error = if ($stage -and ($stage.PSObject.Properties.Name -contains 'error')) { [string]$stage.error } else { $null }
}
"import: exit=$($r.Exit) rows=$($r.Rows) $($r.Seconds)s"
if ($stage -and $stage.overrides) { "  overrides: $($stage.overrides | ConvertTo-Json -Compress)" }
if ($stage -and $stage.verification) { "  guard: $($stage.verification.checked_files) files, $([math]::Round($stage.verification.total_seconds,1)) s" }
if ($r.Exit -ne 0) {
    $report.import.error | Out-String
    $report | ConvertTo-Json -Depth 6 | Set-Content "$outDir\$Tag.json" -Encoding UTF8
    exit 1
}
if (-not $stage.verification -or $stage.verification.checked_files -le 0 -or $stage.verification.checked_files -ne $stage.verification.identical_files) { throw 'stage guard did not pass' }
$summary = (python "$here\configsave_summary.py" $Database) -join "`n" | ConvertFrom-Json
$report.configsave = $summary
"configsave: rows=$($summary.rows) names=$($summary.names) years=$($summary.creation_years | ConvertTo-Json -Compress) parts>1=$(@($summary.parts_over_one).Count)"
if ($summary.deleted) { "  deleted ($($summary.deleted.count)): " + (($summary.deleted.names | ForEach-Object { $_[0] }) -join ' ') }
if ($SkipApply) { $report | ConvertTo-Json -Depth 6 | Set-Content "$outDir\$Tag.json" -Encoding UTF8; exit 0 }

# 2. apply (native, or ours), 3. native export, 4. the tree against the export
$a = if ($ApplyWith -eq 'ours') { Invoke-OursApply -Db $Database -Tag $Tag -Exe $Exe -Platform $Platform -Extra $ApplyArgs }
     else { Invoke-NativeApply -Db $Database -Dynamic $Dynamic -Tag $Tag }
$report.apply = $a
"apply: exit=$($a.Exit) $($a.Seconds)s $($a.Tail)"
if ($a.Exit -ne 0) { $report | ConvertTo-Json -Depth 6 | Set-Content "$outDir\$Tag.json" -Encoding UTF8; exit 2 }
$counts = sqlcmd -S localhost -E -C -h -1 -W -d $Database -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM Config; SELECT COUNT(*) FROM ConfigSave"
$report.config_rows_after = $counts -join ','
"after apply: Config,ConfigSave = $($counts -join ',')"
$exportDir = "$lab\out\export\$Tag"
if (Test-Path $exportDir) { throw "$exportDir exists" }
$e = Invoke-NativeExport -Db $Database -OutDir $exportDir -Tag $Tag
$report.export = $e
"export: exit=$($e.Exit) $($e.Seconds)s"
if ($e.Exit -ne 0) { $report | ConvertTo-Json -Depth 6 | Set-Content "$outDir\$Tag.json" -Encoding UTF8; exit 3 }
$diff = "$lab\out\export\$Tag.diff.json"
& $Exe source-diff -o $diff $Tree $exportDir 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'source-diff failed' }
$d = Get-Content $diff -Raw | ConvertFrom-Json
$report.diff_summary = $d.summary
"source-diff tree vs native export: $($d.summary | ConvertTo-Json -Compress)"
$others = @($d.differences | Where-Object { $_.status -ne 'unchanged' -and $_.path -ne 'ConfigDumpInfo.xml' })
$report.differing_files = @($others | ForEach-Object { "{0} {1}" -f $_.status, $_.path })
$others | Select-Object -First 30 | ForEach-Object { "  {0,-10} {1}" -f $_.status, $_.path }
$report.equal_but_dump_info = ($others.Count -eq 0)
if (-not $DumpInfoReference) { $DumpInfoReference = "$Tree\ConfigDumpInfo.xml" }
$report.dump_info_reference = $DumpInfoReference
$dumpInfo = python "$here\compare_dump_info.py" $DumpInfoReference "$exportDir\ConfigDumpInfo.xml" | ConvertFrom-Json
if ($LASTEXITCODE -notin @(0, 1) -or -not $dumpInfo) { throw 'dump-info comparison failed' }
$report.dump_info = $dumpInfo
$report.equal_except_config_version = ($others.Count -eq 0 -and $dumpInfo.equal_except_config_version)
$report.total_seconds = [math]::Round($sw.Elapsed.TotalSeconds, 1)
$report | ConvertTo-Json -Depth 6 | Set-Content "$outDir\$Tag.json" -Encoding UTF8
"export equals the tree (ConfigDumpInfo.xml aside): $($others.Count -eq 0)"
if (-not $report.equal_except_config_version) { throw 'native export differs from the source tree' }
