# S1-K, the import phase of one case: the edit of the tree, OUR `ibcmd infobase config import` into a twin of the base, and what
# it left behind; then OUR apply as a dry run over that stage. Nothing native runs here.
#
#   pwsh -NoProfile -File import_phase.ps1 -Case b1 -Bin <ibcmd-rs.exe> [-ImportArgs '--base-free'] [-Keep]
#
# Lab (default F:\ibcmd\lab\05\ext\s1k, S1K_LAB): tree\ (a copy of the reference native export, made once with robocopy),
# bak\base_applied.bak (the БСП clone after a native baseline apply), out\<case>\ (the edit, the reports, the record).
# The twin is ibcmd_rs_05_ext_s1k_<case>_own; it is left for the apply phase (twin_case.ps1) and dropped by the caller.
param(
    [Parameter(Mandatory = $true)][string]$Case,
    [Parameter(Mandatory = $true)][string]$Bin,
    [string]$ImportArgs = '',
    [string]$Twin = ''
)
$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'
$env:PYTHONDONTWRITEBYTECODE = '1'
$lab = if ($env:S1K_LAB) { $env:S1K_LAB } else { 'F:\ibcmd\lab\05\ext\s1k' }
$kit = $PSScriptRoot
$env:DDL_LAB = "$lab\snap"   # the snapshot store of the kit (never the ddl track's)
$tree = "$lab\tree"
$out = "$lab\out\$Case"
$db = if ($Twin) { $Twin } else { "ibcmd_rs_05_ext_s1k_${Case}_own" }
if ($db -notmatch '^ibcmd_rs_05_ext_') { throw "not an ext-track database: $db" }
New-Item -ItemType Directory -Force $out | Out-Null
function Log($m) { "[{0}] {1}" -f (Get-Date -Format s), $m }

Log "edit ($Case)"
if (Test-Path "$out\stage") { Remove-Item -Recurse -Force "$out\stage", "$out\before" }
python "$kit\cases.py" edit $Case $out
if ($LASTEXITCODE -ne 0) { throw "the editor of case $Case failed" }
python "$kit\overlay.py" apply $out $tree

try {
    Log "twin $db"
    $exists = sqlcmd -S localhost -E -C -h -1 -W -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM sys.databases WHERE name = N'$db'"
    if ($exists.Trim() -eq '0') {
        pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus bak -Bak "$lab\bak\base_applied.bak" -Name $db -Track ext -Purpose "S1-K case $Case (own)" | Select-Object -Last 1
    } else { "exists already, left as it is" }
    python "$kit\..\snapshot.py" $db "${Case}_before" | Select-Object -Last 1

    Log "our import$(if ($ImportArgs) { ' (' + $ImportArgs + ')' })"
    $t0 = Get-Date
    $cliArgs = @('infobase', 'config', 'import', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$db", '--platform=8.3.27', "--report=$out\import.json") + ($ImportArgs -split ' ' | Where-Object { $_ }) + @($tree)
    & $Bin @cliArgs > "$out\import.out" 2> "$out\import.err"
    $importExit = $LASTEXITCODE
    $seconds = ((Get-Date) - $t0).TotalSeconds
    "import exit $importExit in {0:N1} s" -f $seconds
    Get-Content "$out\import.out", "$out\import.err" -Encoding UTF8 | Select-Object -Last 6 | ForEach-Object { if ($_.Length -gt 500) { $_.Substring(0, 500) } else { $_ } }

    Log 'what the import left'
    python "$kit\record.py" $db $out > "$out\record.json"
    Get-Content "$out\record.json" -Encoding UTF8 | Select-String -Pattern '"(configsave_rows|has_deleted_row|edited_staged|edited_differing|other_rows_differing_from_config|rows_new_to_config_count)"' | ForEach-Object { $_.Line.Trim() }

    Log 'our apply, dry run'
    & $Bin mssql-config-apply --platform-profile platform-8.3.27.2214 --database $db --allow-restructure s1 --i-have-a-backup --allow-non-lab --dry-run --report "$out\apply_dry.json" > "$out\apply_dry.out" 2> "$out\apply_dry.err"
    $dryExit = $LASTEXITCODE
    "apply dry-run exit $dryExit"
    $text = (Get-Content "$out\apply_dry.out", "$out\apply_dry.err" -Encoding UTF8 -Raw) -join "`n"
    $plain = [regex]::Replace($text, "`e\[[0-9;]*m", '')
    ($plain -split "`n" | Where-Object { $_ -match 'blocker|S1:|refus|nothing_to_apply|"structure"|needs the platform|not empty|error|Error' } | Select-Object -First 8) | ForEach-Object { if ($_.Length -gt 400) { $_.Substring(0, 400) } else { $_ } }

    $summary = [ordered]@{
        case = $Case; database = $db; import_exit = $importExit; import_seconds = [math]::Round($seconds, 1)
        apply_dry_exit = $dryExit
    }
    $summary | ConvertTo-Json | Set-Content -Encoding UTF8 "$out\phase1.json"
}
finally {
    python "$kit\overlay.py" restore $out $tree
    python "$kit\overlay.py" verify $out $tree | Select-Object -First 3
}
