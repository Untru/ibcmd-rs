# The worker lab cluster (0.5): constants and helpers shared by start.ps1, stop.ps1, status.ps1, smoke.ps1 and with-cluster.ps1.
# Dot-source it: . "$PSScriptRoot\lib.ps1"
#
# A SEPARATE console `ragent` (which starts its own `rmngr` and `rphost`s) and a console `ras` of platform 8.3.27.2214, on ports of
# their own, with a cluster registry of their own. They are plain processes started by these scripts -- no Windows service is
# installed or touched -- and no existing cluster or infobase (Pavel's 1540/2540/3540 services, the lab's 8.3.27/8.5 clusters) is
# read, changed or re-registered. This file never reads D:\EDT\config\.env: only register-ib.ps1 does.
#
#   ports   ragent 5540 (agent), 5541 (cluster manager, `Srvr=localhost:5541`), RAS 5545, working processes 5560-5591
#   data    F:\ibcmd\lab\05\cluster\srvinfo (the cluster registry), logs\, state.json (what is running, who started it)

$script:Platform = '8.3.27.2214'
$script:Bin = "C:\Program Files\1cv8\$($script:Platform)\bin"
$script:Ragent = Join-Path $script:Bin 'ragent.exe'
$script:RasExe = Join-Path $script:Bin 'ras.exe'
$script:Rac = Join-Path $script:Bin 'rac.exe'
$script:Root = 'F:\ibcmd\lab\05\cluster'
$script:PrivateContext = $null
if ($env:IBCMD_RS_WORKER_LAB_ROOT) {
    $overrideRoot = [IO.Path]::GetFullPath($env:IBCMD_RS_WORKER_LAB_ROOT)
    if ($overrideRoot -in @('F:\ibcmd\lab\05\wave3\load\cluster','F:\ibcmd\lab\05\wave3\metadata\cluster')) {
        . "$PSScriptRoot\private-lib.ps1"
        return
    }
    if ($overrideRoot -ne 'F:\ibcmd\lab\05\wave1\live\cluster') { throw 'worker root override is limited to the owned LIVE wave1 cluster' }
    $script:Root = $overrideRoot
}
$script:Srvinfo = Join-Path $script:Root 'srvinfo'
$script:Logs = Join-Path $script:Root 'logs'
$script:StateFile = Join-Path $script:Root 'state.json'
$script:AgentPort = 5540
$script:RegPort = 5541
$script:RasPort = 5545
$script:RangeFrom = 5560
$script:RangeTo = 5591
$script:RasAddress = "localhost:$($script:RasPort)"
$script:Srvr = "localhost:$($script:RegPort)"
# every port this cluster may listen on
$script:AllPorts = @($script:AgentPort, $script:RegPort, $script:RasPort) + @($script:RangeFrom..$script:RangeTo)
# the 1C server processes; nothing else is ever looked at or stopped
$script:ServerNames = @('ragent.exe', 'rmngr.exe', 'rphost.exe', 'ras.exe')

function Say([string]$text) { "$(Get-Date -Format s) $text" }

function Get-ServerProcesses {
    Get-CimInstance Win32_Process | Where-Object { $script:ServerNames -contains $_.Name }
}

# True for a process that belongs to THIS cluster: it names our data directory or one of our fixed ports on its command line.
# The ports of the services are 1540/1541/1545, 2540/2541/2545, 3540/3541/3545 (and their ranges 15xx/25xx/35xx): none of them
# can match. A descendant of our ragent is added by Get-ClusterProcesses.
function Test-OursCommandLine([string]$commandLine) {
    if (-not $commandLine) { return $false }
    if ($commandLine.IndexOf($script:Srvinfo, [StringComparison]::OrdinalIgnoreCase) -ge 0) { return $true }
    foreach ($p in @($script:AgentPort, $script:RegPort)) {
        if ($commandLine -match "(?i)(^|\s)-{1,2}(reg)?port[= ]$p(\s|$)") { return $true }
    }
    if ($commandLine -match "(?i)(^|\s)--port=$($script:RasPort)(\s|$)") { return $true }
    if ($commandLine -match "(?i)-range[= ]$($script:RangeFrom):$($script:RangeTo)(\s|$)") { return $true }
    if ($commandLine -match "(?i)-regport[= ]$($script:RegPort)(\s|$)") { return $true }
    return $false
}

function Get-ClusterProcesses {
    $all = @(Get-ServerProcesses)
    $ours = @{}
    foreach ($p in $all) { if (Test-OursCommandLine $p.CommandLine) { $ours[[int]$p.ProcessId] = $p } }
    # descendants of what is ours (the working processes started by our manager)
    $changed = $true
    while ($changed) {
        $changed = $false
        foreach ($p in $all) {
            if (-not $ours.ContainsKey([int]$p.ProcessId) -and $ours.ContainsKey([int]$p.ParentProcessId)) { $ours[[int]$p.ProcessId] = $p; $changed = $true }
        }
    }
    @($ours.Values)
}

# The listeners on our ports, whoever owns them.
function Get-ClusterListeners {
    @(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue | Where-Object { $script:AllPorts -contains $_.LocalPort })
}

function Read-State {
    if (Test-Path -LiteralPath $script:StateFile) { Get-Content -LiteralPath $script:StateFile -Raw -Encoding UTF8 | ConvertFrom-Json } else { $null }
}

function Write-State($state) {
    New-Item -ItemType Directory -Force -Path $script:Root | Out-Null
    $state | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $script:StateFile -Encoding UTF8
}

# rac against OUR ras, never another one.
function Invoke-Rac {
    . "$PSScriptRoot\..\live\process.ps1"
    Invoke-LiveRac -Executable $script:Rac -Arguments (@($script:RasAddress) + @($args))
}

function Get-ClusterId {
    ((Invoke-Rac cluster list) | Select-String '^cluster\s*:' | Select-Object -First 1) -replace '^cluster\s*:\s*', ''
}

# Processes of the services and of everything else that is not ours: id and start time, to show that they were not touched.
function Get-OtherServerSnapshot {
    $ours = @((Get-ClusterProcesses) | ForEach-Object { [int]$_.ProcessId })
    @(Get-ServerProcesses | Where-Object { $ours -notcontains [int]$_.ProcessId } |
        Sort-Object ProcessId | ForEach-Object { '{0} {1} {2:o}' -f $_.ProcessId, $_.Name, $_.CreationDate })
}

# Stop every process of this cluster (working processes, manager, RAS, agent), bounded; true when none remains.
function Stop-ClusterProcesses([int]$TimeoutSeconds = 40) {
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ($true) {
        $mine = @(Get-ClusterProcesses)
        if (-not $mine.Count) { return $true }
        if ((Get-Date) -gt $deadline) { return $false }
        # the deepest first: rphost, rmngr, ras, ragent
        $rank = @{ 'rphost.exe' = 0; 'rmngr.exe' = 1; 'ras.exe' = 2; 'ragent.exe' = 3 }
        foreach ($p in ($mine | Sort-Object { $rank[$_.Name] })) {
            Stop-Process -Id ([int]$p.ProcessId) -Force -ErrorAction SilentlyContinue
        }
        Start-Sleep -Milliseconds 700
    }
}
