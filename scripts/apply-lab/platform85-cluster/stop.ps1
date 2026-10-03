param()
$ErrorActionPreference='Stop'
. "$PSScriptRoot\lib.ps1"
$state=State85
if (-not $state) { throw 'no ownership state; no process signal permitted' }
if ($state.cluster) {
    $snapshot=Snapshot85 $state 'before-stop'
    if ($snapshot.names.Count) { throw 'unregister owned infobases before stopping private cluster' }
}
Remember85 $state
$state.known | ConvertTo-Json -Depth 4 | Set-Content "$script:Root\stopping-identities.json"
# Freeze the spawning agent first AFTER retaining all descendants. The retained
# identities remain discoverable after its death; otherwise it respawns workers
# between their signal and the next inventory. No ports/name-only ownership.
$rank=@{ 'ragent.exe'=0; 'ras.exe'=1; 'rphost.exe'=2; 'dbda.exe'=2; 'rmngr.exe'=3 }
for ($pass=0; $pass -lt 5 -and @(Owned85 $state).Count; $pass++) {
    foreach ($p in (@(Owned85 $state) | Sort-Object { $rank[$_.Name] })) {
        # Refresh before stopping a root, so newly created descendants persist independently.
        Remember85 $state
        $identity=$state.known | Where-Object { $_.pid -eq $p.ProcessId } | Select-Object -First 1
        $now=Get-CimInstance Win32_Process -Filter "ProcessId=$($p.ProcessId)"
        if (-not $now) { continue }
        if (-not (Same85 $now $identity)) { throw 'identity changed before signal; refused' }
        Stop-Process -Id ([int]$now.ProcessId) -Force
        Start-Sleep -Milliseconds 500
    }
    Start-Sleep -Milliseconds 500
}
Start-Sleep -Seconds 1
if (@(Owned85 $state).Count -or @(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue | Where-Object { $script:Ports -contains $_.LocalPort }).Count) { throw 'private cluster not fully stopped; state retained' }
Get-CimInstance Win32_Process | Where-Object { $_.Name -in @('ragent.exe','rmngr.exe','rphost.exe','ras.exe','dbda.exe') } | ForEach-Object { Identity85 $_ } | ConvertTo-Json -Depth 4 | Set-Content "$script:Root\foreign-after.json"
Move-Item -LiteralPath $script:StatePath -Destination (Join-Path $script:Root ('stopped-state-' + (Get-Date -Format yyyyMMddHHmmssfff) + '.json'))
'private cluster stopped; registry and evidence retained'
