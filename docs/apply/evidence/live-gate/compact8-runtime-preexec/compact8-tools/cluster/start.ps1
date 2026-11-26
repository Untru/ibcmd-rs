# Start the worker lab cluster: a console ragent (+ its rmngr / rphost) and a console ras of platform 8.3.27.2214, on ports of their
# own and a cluster registry of their own (F:\ibcmd\lab\05\cluster\srvinfo). Not a Windows service; stop.ps1 ends it.
#   pwsh -NoProfile -File start.ps1 -Track <track> [-TimeoutSec 90]
# Refuses to start when this cluster already runs (state.json + a live ragent of ours), when any of its ports is taken by anyone,
# or when the platform is not installed. Nothing of any other cluster is read or changed. On a failure it stops what it started.
# Exit codes: 0 started, 2 refused (already running / port taken / no platform), 1 failed to come up.
param(
    [Parameter(Mandatory = $true)][ValidatePattern('^[a-z0-9_-]{2,20}$')][string]$Track,
    [int]$TimeoutSec = 90
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
. "$PSScriptRoot\lib.ps1"
if ($script:PrivateContext) {
    & "$PSScriptRoot\private-start.ps1" -Track $Track -TimeoutSec $TimeoutSec
    exit 0
}

foreach ($exe in @($script:Ragent, $script:RasExe, $script:Rac)) {
    if (-not (Test-Path -LiteralPath $exe)) { Say "refused: $exe is not installed"; exit 2 }
}
$state = Read-State
$mine = @(Get-ClusterProcesses)
if ($mine.Count) {
    $who = if ($state) { "started by $($state.track) at $($state.started)" } else { 'no state file' }
    Say "refused: the worker lab cluster already runs ($($mine.Count) process(es); $who). Stop it first: stop.ps1"
    exit 2
}
$taken = @(Get-ClusterListeners)
if ($taken.Count) {
    Say ("refused: port(s) {0} of the worker lab cluster are taken (pid {1}); nothing was started" -f (($taken | ForEach-Object { $_.LocalPort } | Sort-Object -Unique) -join ','), (($taken | ForEach-Object { $_.OwningProcess } | Sort-Object -Unique) -join ','))
    exit 2
}
New-Item -ItemType Directory -Force -Path $script:Root, $script:Srvinfo, $script:Logs | Out-Null
$started = @()

function Fail([string]$reason) {
    Say "failed: $reason"
    [void](Stop-ClusterProcesses)
    Remove-Item -LiteralPath $script:StateFile -Force -ErrorAction SilentlyContinue
    exit 1
}
function Wait-Port([int]$port, [int]$seconds) {
    $end = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $end) {
        if (Get-NetTCPConnection -State Listen -LocalPort $port -ErrorAction SilentlyContinue) { return $true }
        Start-Sleep -Milliseconds 500
    }
    $false
}

# 1. the agent: it starts the cluster manager (rmngr) and, on demand, the working processes (rphost)
$agentArgs = @('-agent', '-port', $script:AgentPort, '-regport', $script:RegPort, '-range', "$($script:RangeFrom):$($script:RangeTo)", '-d', $script:Srvinfo)
# No -RedirectStandard*: with a redirect, Start-Process makes the child inherit the caller's handles, and a caller that captures the
# output of this script through a pipe would then wait for the agent to exit. Without it (ShellExecute) nothing is inherited; the
# agent prints nothing, its own logs are in the data directory.
Assert-Compact8RuntimeClosure
$agent = Start-Process -FilePath $script:Ragent -ArgumentList $agentArgs -WindowStyle Hidden -PassThru
Say "ragent pid $($agent.Id): $($agentArgs -join ' ')"
if (-not (Wait-Port $script:AgentPort $TimeoutSec)) { Fail "the agent does not listen on $($script:AgentPort) within $TimeoutSec s" }
if (-not (Wait-Port $script:RegPort $TimeoutSec)) { Fail "the cluster manager does not listen on $($script:RegPort) within $TimeoutSec s" }

# 2. RAS, for rac (and for the tool's own verification and worker switch)
$rasArgs = @('cluster', "--port=$($script:RasPort)", "localhost:$($script:AgentPort)")
Assert-Compact8RuntimeClosure
$ras = Start-Process -FilePath $script:RasExe -ArgumentList $rasArgs -WindowStyle Hidden -PassThru
Say "ras pid $($ras.Id): $($rasArgs -join ' ')"
if (-not (Wait-Port $script:RasPort $TimeoutSec)) { Fail "RAS does not listen on $($script:RasPort) within $TimeoutSec s" }

# 3. the cluster answers rac
$cluster = $null
$end = (Get-Date).AddSeconds($TimeoutSec)
while ((Get-Date) -lt $end -and -not $cluster) {
    $cluster = Get-ClusterId
    if (-not $cluster) { Start-Sleep -Seconds 1 }
}
if (-not $cluster) { Fail "rac $($script:RasAddress) cluster list returns no cluster within $TimeoutSec s" }

Write-State ([ordered]@{
        track = $Track; started = (Get-Date -Format s); platform = $script:Platform
        ragent_pid = $agent.Id; ras_pid = $ras.Id; cluster = $cluster
        agent_port = $script:AgentPort; reg_port = $script:RegPort; ras_port = $script:RasPort
        range = "$($script:RangeFrom):$($script:RangeTo)"; srvinfo = $script:Srvinfo; srvr = $script:Srvr; ras = $script:RasAddress
    })
Say "started: cluster $cluster; clients connect with Srvr=`"$($script:Srvr)`";Ref=`"<db>`"; rac $($script:RasAddress)"
$procs = @(Get-ClusterProcesses)
Say ("processes of this cluster: " + (($procs | Sort-Object ProcessId | ForEach-Object { "$($_.Name)#$($_.ProcessId)" }) -join ' '))
exit 0
