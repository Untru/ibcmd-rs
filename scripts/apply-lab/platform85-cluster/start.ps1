param([ValidateSet('p85')][string]$Track = 'p85', [ValidateRange(5,120)][int]$TimeoutSec = 60)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\lib.ps1"
foreach ($name in @('ragent.exe','ras.exe','rac.exe')) { if (-not (Test-Path -LiteralPath "$script:Bin\$name")) { throw "missing exact-build $name" } }
if (State85) { throw 'state exists; stop the previous owned cluster first' }
if (Test-Path -LiteralPath $script:Data) { throw 'fresh private srvinfo required; preserve and inspect old registry before a new run' }
if (@(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue | Where-Object { $script:Ports -contains $_.LocalPort }).Count) { throw 'private port occupied; nothing started' }
New-Item -ItemType Directory -Path $script:Data -Force | Out-Null
# Child-side temporary work is confined to the same F-owned run directory.
$env:TEMP=Join-Path $script:Root 'temp'
$env:TMP=$env:TEMP
New-Item -ItemType Directory -Path $env:TEMP -Force | Out-Null
Get-CimInstance Win32_Process | Where-Object { $_.Name -in @('ragent.exe','rmngr.exe','rphost.exe','ras.exe','dbda.exe') } | ForEach-Object { Identity85 $_ } | ConvertTo-Json -Depth 4 | Set-Content "$script:Root\foreign-before.json"
$state = [ordered]@{ track='p85'; platform='8.5.1.1150'; root=$script:Root; cluster=''; anchors=@(); started=(Get-Date).ToString('o') }
function Wait85([int]$Port) {
    $end=(Get-Date).AddSeconds($TimeoutSec)
    do { if (Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction SilentlyContinue) { return }; Start-Sleep -Milliseconds 300 } while ((Get-Date) -lt $end)
    throw "private listener $Port timed out; preserve state and use stop.ps1"
}
# Persist each anchor immediately; failures retain ownership evidence for guarded cleanup.
$p=Start-Process -FilePath "$script:Bin\ragent.exe" -ArgumentList @('-agent','-port','6540','-regport','6541','-range','6560:6591','-d',$script:Data) -WindowStyle Hidden -PassThru
$state.anchors += Identity85 (Get-CimInstance Win32_Process -Filter "ProcessId=$($p.Id)")
Save85 $state
Wait85 6540; Wait85 6541
$p=Start-Process -FilePath "$script:Bin\ras.exe" -ArgumentList @('cluster','--port=6545','localhost:6540') -WindowStyle Hidden -PassThru
$state.anchors += Identity85 (Get-CimInstance Win32_Process -Filter "ProcessId=$($p.Id)")
Save85 $state
Wait85 6545
$ids=@(Rac85 @('cluster','list') | Select-String '^cluster\s*:' | ForEach-Object { ($_ -replace '^cluster\s*:\s*','').Trim() })
if ($ids.Count -ne 1 -or -not [guid]::TryParse($ids[0],[ref]([guid]::Empty))) { throw 'expected exactly one valid private cluster UUID' }
$state.cluster=$ids[0]; Save85 $state
$snapshot=Snapshot85 $state 'start'
if ($snapshot.names.Count) { throw 'fresh cluster unexpectedly registered; no process signal permitted' }
"started private 8.5 cluster $($state.cluster) (6541/6545), roots $($state.anchors.pid -join ','); $($snapshot.path)"
