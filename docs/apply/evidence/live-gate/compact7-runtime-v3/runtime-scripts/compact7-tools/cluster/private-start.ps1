param([Parameter(Mandatory)][string]$Track,[ValidateRange(5,120)][int]$TimeoutSec=30)
$ErrorActionPreference='Stop'
. "$PSScriptRoot\lib.ps1"
if(!$script:PrivateContext -or $Track -cne $script:PrivateContext.track){throw 'private root/track mismatch'}
foreach($name in @('ragent.exe','ras.exe','rac.exe')){if(!(Test-Path -LiteralPath (Join-Path $script:Bin $name))){throw 'exact83 executable missing'}}
$lease=Private-Lease
if(Read-State){throw 'previous private ownership state exists'}
[void](Require-PrivatePath $script:Srvinfo);[void](Require-PrivatePath $script:Logs)
if(Test-Path -LiteralPath $script:Srvinfo){throw 'fresh srvinfo required; preserve previous registry'}
if((Get-ClusterListeners).Count){throw 'private ports occupied; nothing started'}
Private-RequireListeners $null
New-Item -ItemType Directory -Path $script:Srvinfo,$script:Logs -Force|Out-Null
$state=[pscustomobject]@{format=1;track=$Track;platform=$script:Platform;root=$script:Root;fresh=$true;cluster='';anchors=@();known=@();worker_lease=$lease;started=[DateTime]::UtcNow.ToString('o')}
function Wait-PrivatePort([int]$Port){
 $deadline=$script:PrivateStartupDeadline
 do{
  $saved=Read-State;Private-RequireLease $saved;Private-Remember $saved
  $listeners=@(Private-StartupListeners $saved $deadline)
  if($listeners|Where-Object{$_.LocalPort -eq $Port}){return}
  Start-Sleep -Milliseconds 300
 }while([DateTime]::UtcNow -lt $deadline)
 throw "private listener $Port timed out; ownership/state retained for guarded stop"
}
Get-OtherServerSnapshot|Set-Content (Join-Path $script:Root 'foreign-before.txt')
Assert-Compact7RuntimeClosure
$process=Start-Process -FilePath (Join-Path $script:Bin 'ragent.exe') -ArgumentList @('-agent','-port','5540','-regport','5541','-range','5560:5591','-d',$script:Srvinfo) -WindowStyle Hidden -PassThru
$identity=Get-CimInstance Win32_Process -Filter "ProcessId=$($process.Id)";if(!$identity){throw 'spawned agent unavailable'}
$state.anchors+=Private-Identity $identity;Write-State $state
$script:PrivateStartupDeadline=[DateTime]::UtcNow.AddSeconds($TimeoutSec)
Wait-PrivatePort 5540
# RAS must be available before testing whether a fresh registry has a cluster.
# Earlier probes waited for rmngr first and never inspected the empty registry.
$state=Read-State
Assert-Compact7RuntimeClosure
$process=Start-Process -FilePath (Join-Path $script:Bin 'ras.exe') -ArgumentList @('cluster','--port=5545','localhost:5540') -WindowStyle Hidden -PassThru
$identity=Get-CimInstance Win32_Process -Filter "ProcessId=$($process.Id)";if(!$identity){throw 'spawned RAS unavailable'}
$state.anchors+=Private-Identity $identity;Write-State $state
Wait-PrivatePort 5545
$version=@(Invoke-Rac agent version) -join "`n";if($version.Trim() -cne $script:Platform){throw 'private runtime version differs'}
$raw=@(Invoke-Rac cluster list);$text=$raw -join "`n"
$ids=@($raw|Select-String '^cluster\s*:'|ForEach-Object{($_ -replace '^cluster\s*:\s*','').Trim()})
$raw|Set-Content (Join-Path $script:Logs 'initial-clusters.log')
if(!$ids.Count){
 if($text.Trim()){throw 'unparsed initial cluster inventory; no bootstrap write'}
 Private-RequireLease (Read-State)
 # Native RAC help cluster confirms insert --host/--port/--name. Exactly one
 # explicit new cluster is permitted only on this proven fresh owned registry.
 Invoke-Rac cluster insert '--host=localhost' '--port=5541' "--name=ibcmd-rs-$Track-wave3"|Set-Content (Join-Path $script:Logs 'cluster-bootstrap.log')
}
$cluster=Get-ClusterId;$state=Read-State;$state.cluster=$cluster;Write-State $state
Wait-PrivatePort 5541
$state=Read-State;$snapshot=Private-Snapshot $state 'start'
if($snapshot.names.Count){throw 'fresh cluster unexpectedly registered; no process signal'}
$script:PrivateStartupDeadline=[DateTime]::MinValue
"started isolated83 $Track cluster $cluster; exact root $script:Root"
