# Run a script against the worker lab cluster and stop the cluster when the run ends, whatever the outcome (finished, failed, timed out).
#   pwsh -NoProfile -File with-cluster.ps1 -Track <track> -Run <script.ps1> [-RunArgs a,b,...] [-MaxMinutes 90] [-Purge]
# Starts the cluster (start.ps1), runs `pwsh -NoProfile -File <script> <args>` with these variables in its environment
#   IBCMD_WORKER_SRVR (localhost:5541)  IBCMD_WORKER_RAS (localhost:5545)  IBCMD_WORKER_CLUSTER (the cluster id)  IBCMD_WORKER_RAC
# waits for it at most -MaxMinutes (then ends it), and stops the cluster (stop.ps1; -Purge also deletes its registry).
# The exit code is the script's; 124 when it timed out; 1 when the cluster did not stop cleanly; 2/1 when it did not start.
# A run that kills this wrapper (a hard kill of the shell) leaves the cluster running: status.ps1 shows it, stop.ps1 ends it.
param(
    [Parameter(Mandatory = $true)][ValidatePattern('^[a-z0-9_-]{2,20}$')][string]$Track,
    [Parameter(Mandatory = $true)][string]$Run,
    [string[]]$RunArgs = @(),
    [int]$MaxMinutes = 90,
    [switch]$Purge
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
. "$PSScriptRoot\lib.ps1"
if (-not (Test-Path -LiteralPath $Run)) { throw "no such script: $Run" }

& pwsh -NoProfile -File "$PSScriptRoot\start.ps1" -Track $Track
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$code = 1
try {
    $env:IBCMD_WORKER_SRVR = $script:Srvr
    $env:IBCMD_WORKER_RAS = $script:RasAddress
    $env:IBCMD_WORKER_RAC = $script:Rac
    $env:IBCMD_WORKER_CLUSTER = Get-ClusterId
    $child = Start-Process -FilePath pwsh -ArgumentList (@('-NoProfile', '-File', $Run) + $RunArgs) -NoNewWindow -PassThru
    if ($child.WaitForExit($MaxMinutes * 60 * 1000)) {
        $code = $child.ExitCode
    } else {
        Say "timed out after $MaxMinutes min: ending the run"
        & taskkill /PID $child.Id /T /F | Out-Null
        $code = 124
    }
}
finally {
    $stopArgs = @('-NoProfile', '-File', "$PSScriptRoot\stop.ps1")
    if ($Purge) { $stopArgs += '-Purge' }
    & pwsh @stopArgs
    if ($LASTEXITCODE -ne 0) { $code = 1 }
}
exit $code
