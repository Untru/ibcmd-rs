# What the import guard costs on ERP УХ (issue #388, checkpoint 2), offline: no database is touched.
#
#   pwsh -NoProfile -File run_uha_guard.ps1 -Exe F:\...\ibcmd-rs.exe -Tag uha1 [-BaseFree] [-NoVerify]
#
# A patch stage (or, with -BaseFree, a base-free one) of a УХ tree is prepared offline (`--script-only`, the target's
# Config rows read from a folder of <name>__part0.bin files, `IBCMD_RS_BASE_ROWS_DIR`) and, unless -NoVerify, the guard
# runs on it. The run holds the lab's heavy lock (any УХ run does). Wall seconds, process CPU seconds and the peak
# working set are recorded with the stage timing lines the stage prints; the rows file the bulk stage writes is deleted.
# Results: out\uha-guard\<tag>.json.
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Tag,
    [string]$Tree = 'E:\ibcmd_lab\parity\ibcmd_rs_uha_8327_native_20260919_20260919_uha_8327_85head\native',
    [string]$Rows = 'F:\ibcmd\lab\rawrows\uha\Config',
    [switch]$BaseFree,
    [switch]$NoVerify,
    [int]$TimeoutMin = 90
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$lab = if ($env:IMPORT_LAB) { $env:IMPORT_LAB } else { 'F:\ibcmd\lab\04\import' }
$outDir = "$lab\out\uha-guard"
$scratch = "$outDir\$Tag-scripts"
New-Item -ItemType Directory -Force $outDir, $scratch | Out-Null
$free = (Get-PSDrive F).Free / 1GB
if ($free -lt 25) { throw "F: has $([math]::Round($free,1)) GB free; not starting" }
if (-not (Test-Path "$Tree\Configuration.xml")) { throw "no tree at $Tree" }
if (-not (Test-Path $Rows)) { throw "no rows at $Rows" }

$lock = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$got = & pwsh -NoProfile -File $lock acquire import -TimeoutMin 240
if ($LASTEXITCODE -ne 0) { throw "heavy lock: $got" }
try {
    $arguments = @('mssql-stage-source-objects', '--database', 'ibcmd_rs_04_import_uha_offline', '--source-root', $Tree,
        '--replace-config-save', '--allow-non-lab', '--script-only', '--script-output', "$scratch\stage.sql", '--platform', '8.3.27')
    if ($BaseFree) { $arguments += '--base-free' }
    if (-not $NoVerify) { $arguments += '--verify' }
    # A base-free stage is for an empty infobase: it reads no target, so the rows folder is not offered to it.
    if (-not $BaseFree) { $env:IBCMD_RS_BASE_ROWS_DIR = $Rows }
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
    Remove-Item Env:IBCMD_RS_BASE_ROWS_DIR, Env:IBCMD_RS_STAGE_TIMING, Env:IBCMD_RS_VERBOSE -ErrorAction SilentlyContinue
    & pwsh -NoProfile -File $lock release import | Out-Null
}
$exit = $p.ExitCode
# The stage prints its report (a JSON with a line per staged object) on stdout; the guard's summary is its last field.
$verification = $null
$outFile = "$outDir\$Tag.out.txt"
if (Test-Path $outFile) {
    $stream = [IO.File]::OpenRead($outFile)
    try {
        $take = [Math]::Min(8192, $stream.Length)
        $stream.Seek(-$take, 'End') | Out-Null
        $bytes = New-Object byte[] $take
        $stream.Read($bytes, 0, $take) | Out-Null
        $tail = [Text.Encoding]::UTF8.GetString($bytes)
    } finally { $stream.Dispose() }
    if ($tail -match '(?s)"verification":\s*(\{[^{}]*\})') { $verification = $Matches[1] | ConvertFrom-Json }
    if ((Get-Item $outFile).Length -gt 20MB) { Remove-Item $outFile -Force }
}
$timing = @(Get-Content "$outDir\$Tag.err.txt" -ErrorAction SilentlyContinue | Where-Object { $_ -match 'stage timing' })
$stderrTail = @(Get-Content "$outDir\$Tag.err.txt" -Tail 6 -ErrorAction SilentlyContinue)
$report = [ordered]@{
    tag = $Tag; base_free = [bool]$BaseFree; verify = -not $NoVerify; exit = $exit
    wall_seconds = [math]::Round($wall, 1); cpu_seconds = if ($cpu) { [math]::Round($cpu, 1) } else { $null }
    peak_working_set_mb = [math]::Round($peak / 1MB); verification = $verification; stage_timing = $timing; stderr_tail = $stderrTail
}
$report | ConvertTo-Json -Depth 4 | Set-Content "$outDir\$Tag.json" -Encoding UTF8
Get-ChildItem $scratch -Recurse -File | Where-Object { $_.Length -gt 100MB } | Remove-Item -Force
"exit=$exit wall=$([math]::Round($wall,1))s cpu=$(if ($cpu) { [math]::Round($cpu,1) })s peak=$([math]::Round($peak / 1MB)) MB"
$timing | Select-Object -Last 12
