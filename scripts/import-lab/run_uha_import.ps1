# The drop-in `infobase config import` of an ERP УХ tree into a УХ clone (issue #388, step 2): what a stage against a
# database costs on the largest corpus -- wall and CPU seconds, peak memory, the stage's own timing lines, what the
# comparison of the tree with the target and the guard found. The clone is a staging one that is never applied.
#
#   pwsh -NoProfile -File run_uha_import.ps1 -Exe F:\...\ibcmd-rs.exe -Database ibcmd_rs_04_import_uha_s1 -Tag uhaimp1
#        [-Tree E:\...\native] [-NoVerify] [-TimeoutMin 90]
#
# The run holds the lab's heavy lock (any УХ run does). Results: out\uha-import\<tag>.json.
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Tag,
    [string]$Tree = 'E:\ibcmd_lab\parity\ibcmd_rs_uha_8327_native_20260919_20260919_uha_8327_85head\native',
    [switch]$NoVerify,
    [int]$TimeoutMin = 90
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$lab = if ($env:IMPORT_LAB) { $env:IMPORT_LAB } else { 'F:\ibcmd\lab\04\import' }
if ($Database -notmatch '^ibcmd_rs_04_import_[a-z0-9_]+$') { throw "lab databases only: ibcmd_rs_04_import_* (got $Database)" }
$outDir = "$lab\out\uha-import"
New-Item -ItemType Directory -Force $outDir, "$lab\ibdata\$Database" | Out-Null
$free = (Get-PSDrive F).Free / 1GB
if ($free -lt 25) { throw "F: has $([math]::Round($free,1)) GB free; not starting" }
if (-not (Test-Path "$Tree\Configuration.xml")) { throw "no tree at $Tree" }

$lock = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$got = & pwsh -NoProfile -File $lock acquire import -TimeoutMin 240
if ($LASTEXITCODE -ne 0) { throw "heavy lock: $got" }
try {
    $arguments = @('infobase', 'config', 'import', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Database",
        "--data=$lab\ibdata\$Database", "--report=$outDir\$Tag.report.json")
    if ($NoVerify) { $arguments += '--no-verify' }
    $arguments += $Tree
    $env:IBCMD_RS_STAGE_TIMING = '1'
    $env:IBCMD_RS_VERBOSE = '1'
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $Exe -ArgumentList $arguments -NoNewWindow -PassThru `
        -RedirectStandardOutput "$outDir\$Tag.out.txt" -RedirectStandardError "$outDir\$Tag.err.txt"
    $peak = 0L
    while (-not $p.HasExited) {
        if ($p.WaitForExit(5000)) { break }
        try { $p.Refresh(); $peak = [Math]::Max($peak, $p.PeakWorkingSet64) } catch { }
        if ($sw.Elapsed.TotalMinutes -gt $TimeoutMin) { $p.Kill(); throw "timeout after $TimeoutMin min" }
    }
    $p.WaitForExit()
    $wall = $sw.Elapsed.TotalSeconds
    $cpu = try { $p.TotalProcessorTime.TotalSeconds } catch { $null }
} finally {
    Remove-Item Env:IBCMD_RS_STAGE_TIMING, Env:IBCMD_RS_VERBOSE -ErrorAction SilentlyContinue
    & pwsh -NoProfile -File $lock release import | Out-Null
}
$exit = $p.ExitCode
$report = $null
if (Test-Path "$outDir\$Tag.report.json") { $report = Get-Content "$outDir\$Tag.report.json" -Raw | ConvertFrom-Json }
$timing = @(Get-Content "$outDir\$Tag.err.txt" -ErrorAction SilentlyContinue | Where-Object { $_ -match 'stage timing' })
$result = [ordered]@{
    tag = $Tag; database = $Database; verify = -not $NoVerify; exit = $exit
    wall_seconds = [math]::Round($wall, 1); cpu_seconds = if ($cpu) { [math]::Round($cpu, 1) } else { $null }
    peak_working_set_mb = [math]::Round($peak / 1MB)
    stage_mode = if ($report) { $report.stage_mode } else { $null }
    staged_rows_after = if ($report) { $report.staged_rows_after } else { $null }
    overrides = if ($report) { $report.overrides } else { $null }
    verification = if ($report) { $report.verification } else { $null }
    error = if ($report -and ($report.PSObject.Properties.Name -contains 'error')) { [string]$report.error } else { $null }
    stage_timing = $timing
    stderr_tail = @(Get-Content "$outDir\$Tag.err.txt" -Tail 6 -ErrorAction SilentlyContinue)
}
$result | ConvertTo-Json -Depth 5 | Set-Content "$outDir\$Tag.json" -Encoding UTF8
"exit=$exit wall=$([math]::Round($wall,1))s cpu=$(if ($cpu) { [math]::Round($cpu,1) })s peak=$([math]::Round($peak / 1MB)) MB"
$timing | Select-Object -Last 14
