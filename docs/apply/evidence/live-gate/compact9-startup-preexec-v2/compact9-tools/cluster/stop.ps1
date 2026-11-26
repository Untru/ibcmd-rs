# Stop the worker lab cluster: every process of it (working processes, cluster manager, RAS, agent) and nothing else, then show that
# none is left and none of its ports is listening. Safe to run when it is not running.
#   pwsh -NoProfile -File stop.ps1 [-Purge]
# -Purge also deletes the cluster registry (F:\ibcmd\lab\05\cluster\srvinfo) and the logs, so that the next start is a new cluster;
# only when nothing of it runs, only under F:\ibcmd\lab\05\cluster.
# The processes are found by our data directory / our ports on the command line and by descent from our agent; the services of the
# other clusters run under another account, show no command line and are never candidates.
# Exit code 0: nothing of this cluster remains; 1: something does.
param([switch]$Purge)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
. "$PSScriptRoot\lib.ps1"
if ($script:PrivateContext) {
    & "$PSScriptRoot\private-stop.ps1" -Purge:$Purge
    exit 0
}

$state = Read-State
$before = @(Get-ClusterProcesses)
Say ("stopping: {0} process(es) of the worker lab cluster{1}" -f $before.Count, $(if ($state) { " (started by $($state.track) at $($state.started))" } else { '' }))
if ($before.Count) {
    # what was registered / connected, for the record (the infobases stay in the registry of this cluster only)
    try {
        $cluster = Get-ClusterId
        if ($cluster) {
            $names = @(Invoke-Rac infobase summary list "--cluster=$cluster" | Select-String '^name\s*:' | ForEach-Object { ($_ -replace '^name\s*:\s*', '').Trim() })
            $sessions = @(Invoke-Rac session list "--cluster=$cluster" | Select-String '^session\s*:').Count
            Say "registered infobases: $($names.Count) ($($names -join ', ')); sessions: $sessions"
        }
    } catch { Say "rac does not answer: $($_.Exception.Message)" }
}
$clean = Stop-ClusterProcesses 40
Start-Sleep -Milliseconds 500
$left = @(Get-ClusterProcesses)
$listening = @(Get-ClusterListeners)
Remove-Item -LiteralPath $script:StateFile -Force -ErrorAction SilentlyContinue
if ($left.Count -or $listening.Count -or -not $clean) {
    Say ("NOT CLEAN: {0} process(es) [{1}], {2} listener(s) [{3}]" -f $left.Count, (($left | ForEach-Object { "$($_.Name)#$($_.ProcessId)" }) -join ' '), $listening.Count, (($listening | ForEach-Object { $_.LocalPort }) -join ','))
    exit 1
}
Say 'stopped: no process of the worker lab cluster remains and none of its ports (5540, 5541, 5545, 5560-5591) is listening'
if ($Purge) {
    $target = [IO.Path]::GetFullPath($script:Srvinfo)
    if (-not $target.StartsWith('F:\ibcmd\lab\05\cluster\', [StringComparison]::OrdinalIgnoreCase)) { throw "refusing to delete $target" }
    Remove-Item -LiteralPath $script:Srvinfo -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $script:Logs -Recurse -Force -ErrorAction SilentlyContinue
    Say 'purged: the cluster registry and the logs are deleted'
}
exit 0
