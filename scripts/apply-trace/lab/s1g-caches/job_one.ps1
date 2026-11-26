# Registers a trace-track lab database in the 8.3.27 cluster, runs one session job on it, unregisters it.
#   pwsh -NoProfile -File job_one.ps1 -Database ibcmd_rs_05_trace_tw1_nat -Job f_cleanup.bsl [-Out <file>]
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Job,
    [string]$Out = ''
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
if ($Database -notmatch '^ibcmd_rs_05_trace_[a-z0-9_]+$') { throw "trace-track lab databases only (got $Database)" }
$reg = 'F:\ibcmd\lab\04\tools\register-ib.ps1'
pwsh -NoProfile -File $reg register -Database $Database -Platform 8.3.27 -Track trace 2>&1 | Select-Object -Last 1
try {
    $text = pwsh -NoProfile -File 'F:\ibcmd\lab\05\s1g\kit\job.ps1' -Job $Job -Database $Database -TimeoutSec 900 2>&1
} finally {
    pwsh -NoProfile -File $reg unregister -Database $Database -Platform 8.3.27 -Track trace 2>&1 | Select-Object -Last 1
}
$text
if ($Out) { $text | Set-Content -LiteralPath $Out -Encoding UTF8 }
