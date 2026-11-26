# Does a staged change survive the native apply and the native export? (issue #388)
#
#   pwsh -NoProfile -File run_survival.ps1 -Database ibcmd_rs_04_import_bsp_bf -Tree F:\...\tree\combo2 -Mode bf -Tag bf1 [-Exe ...]
#
# Stages the tree in the given mode (patch = drop-in default, bf = --base-free, native = native `config import`)
# into the clone, runs the native `config apply --force --dynamic=disable`, exports the clone with the NATIVE
# `config export` into out\export\<tag>, and compares that export with the tree (`ibcmd-rs source-diff`).
# Every native write runs inside the lab's native lock. Lab databases only (ibcmd_rs_04_import_*).
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Tree,
    [Parameter(Mandatory = $true)][ValidateSet('patch', 'bf', 'native')][string]$Mode,
    [Parameter(Mandatory = $true)][string]$Tag,
    [string]$Exe = '',
    [string]$Dynamic = 'disable',
    [int]$MinRows = 9800,
    [switch]$SkipExport
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'; $env:PYTHONIOENCODING = 'utf-8'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
. "$here\native.ps1"
$lab = $script:Lab
$sw = [Diagnostics.Stopwatch]::StartNew()
$report = [ordered]@{ tag = $Tag; database = $Database; mode = $Mode; tree = $Tree }

# 1. stage, 2. native apply. A native import and its apply are one hold of the native lock.
$r = $null; $a = $null
switch ($Mode) {
    'patch' { $r = Invoke-OursImport -Db $Database -Tree $Tree -Tag "$Tag-stage" -Exe $Exe }
    'bf' { $r = Invoke-OursImport -Db $Database -Tree $Tree -Tag "$Tag-stage" -BaseFree -Exe $Exe }
}
if ($Mode -eq 'native') {
    # One command per hold of the native lock (the lock is a FIFO queue since 14:15): the import, then the apply.
    # A native import can stage a partial set (9 597-9 635 rows were seen under load): it is repeated until
    # `native_complete.py` finds ConfigSave complete (the lab's criterion), as the ddl track did.
    $complete = $false
    foreach ($attempt in 1..3) {
        $r = Invoke-NativeImport -Db $Database -Tree $Tree -Tag "$Tag-stage$attempt"
        $check = (python "$here\native_complete.py" $Database) -join ' '
        "stage (native) #${attempt}: exit=$($r.Exit) rows=$($r.Rows) $($r.Seconds)s | $check"
        if ($r.Exit -eq 0 -and $check -like 'complete*') { $complete = $true; break }
    }
    $r.Check = $check
    if ($complete) { $a = Invoke-NativeApply -Db $Database -Dynamic $Dynamic -Tag $Tag }
} elseif ($r.Exit -eq 0) {
    $a = Invoke-NativeApply -Db $Database -Dynamic $Dynamic -Tag $Tag
}
$report.stage = $r
"stage ($Mode): exit=$($r.Exit) rows=$($r.Rows) $($r.Seconds)s $($r.Tail)"
if ($r.Exit -ne 0) { $report | ConvertTo-Json -Depth 5 | Set-Content "$lab\out\survival-$Tag.json"; exit 1 }
$report.apply = $a
"apply: exit=$($a.Exit) $($a.Seconds)s $($a.Tail)"
$rows = sqlcmd -S localhost -E -C -h -1 -W -d $Database -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM Config; SELECT COUNT(*) FROM ConfigSave"
$report.config_rows_after = $rows -join ','
"after apply: Config,ConfigSave = $($rows -join ',')"
if ($a.Exit -ne 0) { $report | ConvertTo-Json -Depth 5 | Set-Content "$lab\out\survival-$Tag.json"; exit 2 }
if ($SkipExport) { $report | ConvertTo-Json -Depth 5 | Set-Content "$lab\out\survival-$Tag.json"; exit 0 }

# 3. native export and the diff with the tree
$exportDir = "$lab\out\export\$Tag"
if (Test-Path $exportDir) { throw "$exportDir exists" }
$e = Invoke-NativeExport -Db $Database -OutDir $exportDir -Tag $Tag
$report.export = $e
"export: exit=$($e.Exit) $($e.Seconds)s"
if ($e.Exit -ne 0) { $report | ConvertTo-Json -Depth 5 | Set-Content "$lab\out\survival-$Tag.json"; exit 3 }
$diff = "$lab\out\export\$Tag.diff.json"
$exeOurs = if ($Exe) { $Exe } else { "$lab\bin\ibcmd-rs-v0.exe" }
& $exeOurs source-diff -o $diff $Tree $exportDir 2>&1 | Out-Null
$d = Get-Content $diff -Raw | ConvertFrom-Json
$report.diff_summary = $d.summary
"source-diff tree vs native export: $($d.summary | ConvertTo-Json -Compress)"
$d.differences | Where-Object { $_.status -ne 'unchanged' } | Select-Object -First 40 | ForEach-Object { "  {0,-10} {1}" -f $_.status, $_.path }
$report.total_seconds = [math]::Round($sw.Elapsed.TotalSeconds, 1)
$report | ConvertTo-Json -Depth 5 | Set-Content "$lab\out\survival-$Tag.json"
