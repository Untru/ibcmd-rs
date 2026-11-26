# What the worker lab cluster is doing: state, processes, listeners, and (when it runs) the cluster, its infobases, working
# processes and sessions as rac sees them. Read-only.
#   pwsh -NoProfile -File status.ps1
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
. "$PSScriptRoot\lib.ps1"

$state = Read-State
$procs = @(Get-ClusterProcesses)
$listeners = @(Get-ClusterListeners)
if ($state) { Say "state: started by $($state.track) at $($state.started), ragent pid $($state.ragent_pid), platform $($state.platform)" } else { Say 'state: none (not started by these scripts)' }
Say ("processes: {0}{1}" -f $procs.Count, $(if ($procs.Count) { ' -- ' + (($procs | Sort-Object ProcessId | ForEach-Object { "$($_.Name)#$($_.ProcessId)" }) -join ' ') } else { '' }))
Say ("listening on its ports: {0}" -f $(if ($listeners.Count) { ($listeners | ForEach-Object { $_.LocalPort } | Sort-Object -Unique) -join ',' } else { 'none' }))
if ($procs.Count -and (Get-NetTCPConnection -State Listen -LocalPort $script:RasPort -ErrorAction SilentlyContinue)) {
    $cluster = Get-ClusterId
    Say "cluster: $cluster (rac $($script:RasAddress); clients: Srvr=`"$($script:Srvr)`")"
    if ($cluster) {
        Say 'infobases:'
        Invoke-Rac infobase summary list "--cluster=$cluster" | Where-Object { $_ -match '^(infobase|name)\s*:' } | ForEach-Object { "  $_" }
        Say 'working processes:'
        Invoke-Rac process list "--cluster=$cluster" | Where-Object { $_ -match '^(process|pid|port|running|use|started-at|available-perfomance|infobase)\s*:' } | ForEach-Object { "  $_" }
        Say 'sessions:'
        Invoke-Rac session list "--cluster=$cluster" | Where-Object { $_ -match '^(session|infobase|user-name|app-id|started-at|process)\s*:' } | ForEach-Object { "  $_" }
    }
}
