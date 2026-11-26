# Check 12 of the twin protocol (12.6) on trace-track databases: the ddl kit's inject_failure.ps1 (a THROW before COMMIT in the
# generated script, run on a fresh twin of the staged state, a digest of the database before and after) with its database-name
# guard widened to the trace track. Restores the fresh twin from the staged backup first.
#   pwsh -NoProfile -File twin_inject.ps1 -Case n5 -Run F:\ibcmd\lab\05\s1g\out\run_n5 -Own ibcmd_rs_05_trace_tw5_own -To ibcmd_rs_05_trace_tw5_inj
param(
    [Parameter(Mandatory = $true)][string]$Case,
    [Parameter(Mandatory = $true)][string]$Run,
    [Parameter(Mandatory = $true)][string]$Own,
    [Parameter(Mandatory = $true)][string]$To
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
if ($To -notmatch '^ibcmd_rs_05_trace_[a-z0-9_]+$') { throw "trace-track lab databases only (got $To)" }
$bak = "F:\ibcmd\lab\05\s1g\bak\${Case}_staged.bak"
$script = "$Run\script_real.sql"
if (-not (Test-Path -LiteralPath $script)) { throw "no script: $script" }
$exists = sqlcmd -S localhost -E -C -h -1 -W -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM sys.databases WHERE name = N'$To'"
if ("$exists".Trim() -eq '0') {
    pwsh -NoProfile -File 'F:\ibcmd\lab\04\tools\restore-clone.ps1' -Corpus bak -Bak $bak -Name $To -Track trace -Purpose "S1-F check 12: failure injection on a fresh twin of $Case" | Select-Object -Last 1
}
$kit = Join-Path (Split-Path (Split-Path (Split-Path $PSScriptRoot))) 'restructure-lab'
$patched = Join-Path $env:TEMP 'inject_failure_trace.ps1'
(Get-Content -LiteralPath "$kit\inject_failure.ps1" -Raw -Encoding UTF8).Replace('ibcmd_rs_04_ddl_', 'ibcmd_rs_05_trace_') |
    Set-Content -LiteralPath $patched -Encoding UTF8
pwsh -NoProfile -File $patched -Script $script -From $Own -To $To -Out "$Run\inject_failure.txt"
