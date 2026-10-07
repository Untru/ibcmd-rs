param([Parameter(Mandatory)][string]$Binary,[Parameter(Mandatory)][string]$SourceHead,[Parameter(Mandatory)][string]$BinarySha256,[Parameter(Mandatory)][ValidatePattern('^[0-9A-F]{64}$')][string]$ManifestSha256,[Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{40}$')][string]$ControllerHead,[Parameter(Mandatory)][ValidatePattern('^[0-9A-F]{64}$')][string]$RuntimeManifestSha256)
$ErrorActionPreference='Stop';$lab='F:/ibcmd/lab/05/wave3/load';$wt='F:/ibcmd/src/ibcmd-rs-05-load-wave3';$db='ibcmd_rs_05_load_w3_compact9_20261002';$backup="$lab/f5-owned-full.bak";$prefix='compact9-lifetime';$heldWorker=$false;$heldHeavy=$false;$registered=$false;$started=$false;$registrationAttempted=$false;$ib=''
$env:IBCMD_RS_COMPACT9_RUNTIME_MANIFEST_SHA=$RuntimeManifestSha256
. 'F:/ibcmd/lab/05/wave3/load/compact9-runtime-v2/runtime_closure.ps1'
Assert-Compact9RuntimeClosure -RetainedEvidence

. "$lab/compact9-runtime-v2/compact9-tools/live/process.ps1"
. "$lab/compact7_closure_v2.ps1"
. "$PSScriptRoot/archive_worker.ps1"
. "$lab/compact9-runtime-v2/compact9-tools/live/workload_lab.ps1"
[void](Resolve-LoadLabRoot $lab)
$authority=Read-Compact9RuntimeJson 'F:/ibcmd/lab/05/wave3/coordinator/compact9-runtime-frozen-v2.json'
if($authority.acceptance_source_status -cne 'CURRENT_COMBINED_REVIEWED' -or $SourceHead -cne $authority.binary_source -or $BinarySha256 -cne $authority.binary_sha256 -or [IO.Path]::GetFullPath($Binary) -cne [IO.Path]::GetFullPath($authority.binary_file) -or (Get-FileHash -LiteralPath $Binary).Hash -cne $BinarySha256){throw 'exact current combined executable authority required'}
if((Get-FileHash -LiteralPath $backup).Hash -cne '2B258E55F57432839208C9CD3981CBEA0A2439F19FE165EAA5939D336D90098D'){throw 'approved own backup changed'}
if((Test-Path "$lab/compact9-binding.json") -or @(Get-ChildItem "$lab/logs" -Filter 'compact9-lifetime*').Count){throw 'fresh case evidence required'}
if(Test-Path "$lab/cluster/state.json"){throw 'active private state refused'}
$taskTemp="$lab/compact9-temp";if(Test-Path -LiteralPath $taskTemp){throw 'fresh F process temp required'};New-Item -ItemType Directory -Path $taskTemp|Out-Null;$env:TEMP=$taskTemp;$env:TMP=$taskTemp
$taskReceipts="$lab/compact9-child-receipts";if(Test-Path -LiteralPath $taskReceipts){throw 'fresh F child receipts required'};New-Item -ItemType Directory -Path $taskReceipts|Out-Null;$env:IBCMD_RS_LIVE_CHILD_RECEIPT_ROOT=$taskReceipts
$env:IBCMD_RS_WORKER_LAB_ROOT="$lab/cluster"
$taskLockProofs="$lab/compact9-lock-proofs";if(Test-Path -LiteralPath $taskLockProofs){throw 'fresh F lock proof directory required'};New-Item -ItemType Directory -Path $taskLockProofs|Out-Null
function Assert-Compact9Source {
 if(Test-LiveUncertainChild){throw 'unresolved child; source freeze unconfirmed, no new command'}
 $head=Invoke-LiveBounded git @('-C',$wt,'rev-parse','HEAD') 30
 $dirty=Invoke-LiveBounded git @('-C',$wt,'status','--porcelain') 30
 if($head.ExitCode -or $dirty.ExitCode -or $head.Stdout.Trim() -cne $ControllerHead -or $dirty.Stdout.Trim()){throw 'compact9 controller source HEAD/clean changed'}
}
function Run([string]$name,[string]$exe,[string[]]$argv,[int]$seconds=60){
 $path="$lab/logs/$name.json";if(Test-Path -LiteralPath $path){throw 'fresh result evidence required'}
 @{executable=$exe;arguments=$argv;started_utc=[DateTime]::UtcNow.ToString('o');deadline_seconds=$seconds}|ConvertTo-Json -Depth 5|Set-Content "$lab/logs/$name.command.json"
 try{$r=Invoke-LiveBounded $exe $argv $seconds;if(($r.Stdout+$r.Stderr) -match 'COMPACT9_RESOURCE_UNCONFIRMED'){$script:Compact9ResourceUnconfirmed=$true};$r|Add-Member -NotePropertyName ended_utc -NotePropertyValue ([DateTime]::UtcNow.ToString('o'));$r|ConvertTo-Json|Set-Content -LiteralPath $path;if($r.ExitCode){throw "bounded $name failed"};return $r}catch{$_|Out-String|Set-Content "$lab/logs/$name.error.txt";throw}
}
function Native([string]$name,[string]$exe,[string[]]$argv,[int]$seconds=120){
 if(Test-LiveUncertainChild){throw 'unresolved prior child; no new native writer'}
 [void](Run "$name-native-acquire" pwsh @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','acquire','load9-983ad8f3eae1','-Name','native','-TimeoutMin','10') 620)
 try{Run $name $exe $argv $seconds}finally{if(Test-LiveUncertainChild){throw 'unresolved child; native FIFO retained'};$release=Invoke-LiveBounded pwsh @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','release','load9-983ad8f3eae1','-Name','native') 15;if($release.ExitCode){throw 'native release failed'}}
}
function Test-Compact9UnresolvedWriter {
 if(Test-LiveUncertainChild){return $true}
 $path="$lab/compact9-load-9-native-import.child-times.json"
 if(Test-Path -LiteralPath $path){
  if((Get-Item -LiteralPath $path).Length -gt 8192){throw 'native child proof exceeds budget; retain lifecycle'}
  $times=Get-Content -LiteralPath $path -Raw|ConvertFrom-Json -DateKind String
  if($times.writer_completion_proved -ne $true){return $true}
 }
 return $false
}
function Assert-Compact9CleanupAuthority {
 if(Test-Compact9UnresolvedWriter){throw 'unresolved owned writer/child; no unregister/cluster stop/native or worker release'}
 if(Test-Compact9ResourceUnconfirmed){throw 'unconfirmed resource operation; no unregister/cluster stop/worker release'}
}
try{
 Assert-Compact9RuntimeClosure -RetainedEvidence
 Assert-Compact7Closure $ManifestSha256
 Assert-Compact9Source
 $existing=Invoke-LiveBounded sqlcmd @('-S','localhost','-E','-C','-b','-h','-1','-Q',"SET NOCOUNT ON; IF DB_ID(N'$db') IS NOT NULL THROW 57100,'fresh compact9 DB exists',1;") 15;if($existing.ExitCode){throw 'fresh database preflight refused'}
 $heldWorker=$true # Potential grant before bounded acquire dispatch; uncertainty retains lease.
 [void](Run "$prefix-worker-acquire" pwsh @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','acquire','load9-983ad8f3eae1','-Name','worker','-TimeoutMin','20') 1240)
 Assert-Compact9RuntimeClosure -RetainedEvidence
 Assert-Compact7Closure $ManifestSha256
 Assert-Compact9Source
 [void](Run "$prefix-prior-case5-readonly" pwsh @('-NoProfile','-File',"$lab/compact9-runtime-v2/compact9_prior_archive.ps1",'-Validate','-WorkerContext',(Get-Compact9ArchiveWorkerContext)) 120)
 if((Test-Path "$lab/cluster/srvinfo") -or (Test-Path "$lab/cluster/logs")){throw 'prior clean registry archive incomplete'}
 $heldHeavy=$true
 [void](Run "$prefix-heavy-acquire" pwsh @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','acquire','load9-983ad8f3eae1','-Name','heavy','-TimeoutMin','20') 1240)
 Assert-Compact9RuntimeClosure -RetainedEvidence
 Assert-Compact7Closure $ManifestSha256
 Assert-Compact9Source
 [void](Native "$prefix-restore" pwsh @('-NoProfile','-File','F:/ibcmd/lab/04/tools/restore-clone.ps1','-Corpus','bak','-Bak',$backup,'-Name',$db,'-Track','load','-Purpose','bounded matching current combined compact9 loaded checkpoint') 240)
 $o=Get-Content "$lab/OWNED.json" -Raw|ConvertFrom-Json -DateKind String
 if(@($o.databases|Where-Object{$_.name -ceq $db}).Count){throw 'OWNED already contains fresh clone'}
 $o.databases+=@{name=$db;state='restored_compact9_unregistered';service='localhost:5541';source_backup=$backup};$o|ConvertTo-Json -Depth 9|Set-Content "$lab/OWNED.json"
 # Startup is one bounded fresh attempt. Failure retains census/state; no automatic retry.
 $started=$true;[void](Run "$prefix-start" pwsh @('-NoProfile','-File',"$lab/compact9-runtime-v2/compact9-tools/cluster/start.ps1",'-Track','load9-983ad8f3eae1','-TimeoutSec','120') 180)
 $registrationAttempted=$true;[void](Native "$prefix-register" pwsh @('-NoProfile','-File',"$lab/compact9-runtime-v2/compact9-tools/cluster/private-register.ps1",'register','-Database',$db) 120);$registered=$true
 $state=Get-Content "$lab/cluster/state.json" -Raw|ConvertFrom-Json -DateKind String;$cluster=$state.cluster;[void][guid]::Parse($cluster)
 $r=Run "$prefix-binding" 'C:/Program Files/1cv8/8.3.27.2214/bin/rac.exe' @('localhost:5545','infobase','summary','list',"--cluster=$cluster") 10
 $blocks=@(($r.Stdout -split '(?:\r?\n){2,}')|Where-Object{$_ -match ('(?m)^name\s*:\s*'+[regex]::Escape($db)+'\s*$')})
 if($blocks.Count -ne 1 -or $blocks[0] -notmatch '(?m)^infobase\s*:\s*([0-9a-f-]{36})\s*$'){throw 'exact fresh registration binding absent'};$ib=$Matches[1];[void][guid]::Parse($ib)
 @{database=$db;cluster_uuid=$cluster;infobase_uuid=$ib;worker_lease=$state.worker_lease;origin=$backup;binary=$Binary;source_head=$SourceHead;binary_sha256=$BinarySha256}|ConvertTo-Json|Set-Content "$lab/compact9-binding.json"
 $o=Get-Content "$lab/OWNED.json" -Raw|ConvertFrom-Json -DateKind String;$entry=@($o.databases|Where-Object{$_.name -ceq $db});$entry[0].state='registered_compact9';$entry[0]|Add-Member -NotePropertyName infobase_uuid -NotePropertyValue $ib;$entry[0]|Add-Member -NotePropertyName cluster_uuid -NotePropertyValue $cluster;$o|ConvertTo-Json -Depth 9|Set-Content "$lab/OWNED.json"
 [void](Run "$prefix-initial-snapshot" pwsh @('-NoProfile','-File',"$lab/compact9-runtime-v2/compact9_snapshot.ps1",'-Label','compact9-initial') 180)
 $rows=Get-Content "$lab/snapshots/compact9-initial-storage.txt"
 if(@($rows|Where-Object{$_ -match '^Config\|[^|]*_dynupdate_' -or $_ -match '^(Config|Params)\|DynamicallyUpdated\|'}).Count){throw 'restored fixture is not settled marker-free; no normalization admission'}
 # Recovery-model/chain negative controls and ordinary FULL preparation are inside the checkpoint,
 # after its exact native staged preimage and before any positive phase1 retry.
 [void](Run "$prefix-checkpoint" pwsh @('-NoProfile','-File',"$lab/compact9-runtime-v2/compact9_checkpoint_v2.ps1",'-Binary',$Binary,'-SourceHead',$SourceHead,'-BinarySha256',$BinarySha256) 1800)
 Assert-Compact9Source
 Assert-Compact9RuntimeClosure -RetainedEvidence
 }finally{
 $clean=$false;$guardFailure=$null;$cleanupPhase='entry';$workerReleaseAttempted=$false
 try{
  Assert-Compact9CleanupAuthority
  if($registered){
   $cleanupPhase='owned-process-proof'
   if(!$ib){throw 'registration UUID unresolved; no unregister or stop'}
   $o=Get-Content "$lab/OWNED.json" -Raw|ConvertFrom-Json -DateKind String
   foreach($b in @($o.processes|Where-Object{$_.infobase_uuid -ceq $ib})){
    if(Get-CimInstance Win32_Process -Filter "ProcessId=$($b.pid)"){throw 'recorded cohort PID still present; no unregister/stop without exact close'}
   }
   $cleanupPhase='ras-empty';Assert-Compact9CleanupAuthority
   $r=Run "$prefix-cleanup-empty" 'C:/Program Files/1cv8/8.3.27.2214/bin/rac.exe' @('localhost:5545','session','list',"--cluster=$cluster","--infobase=$ib") 10
   Assert-Compact9CleanupAuthority
   if($r.Stdout.Trim() -or $r.Stderr.Trim()){throw 'target RAS not proved empty; retain registration/state'}
   $cleanupPhase='sql-work';Assert-Compact9CleanupAuthority
   $r=Run "$prefix-cleanup-sql-work" sqlcmd @('-S','localhost','-E','-C','-b','-Q',"IF EXISTS(SELECT 1 FROM sys.dm_exec_sessions s LEFT JOIN sys.dm_exec_requests r ON s.session_id=r.session_id WHERE s.session_id<>@@SPID AND (s.database_id=DB_ID(N'$db') OR r.database_id=DB_ID(N'$db')) AND (s.open_transaction_count>0 OR r.session_id IS NOT NULL)) THROW 57239,'compact9 SQL work remains; cleanup refused',1;") 15
   Assert-Compact9CleanupAuthority
   $cleanupPhase='unregister';Assert-Compact9CleanupAuthority
   [void](Native "$prefix-unregister" pwsh @('-NoProfile','-File',"$lab/compact9-runtime-v2/compact9-tools/cluster/private-register.ps1",'unregister','-Database',$db) 120)
   Assert-Compact9CleanupAuthority;$registered=$false
  }
  if($started -and !$registered){
   $cleanupPhase='stop';Assert-Compact9CleanupAuthority
   [void](Run "$prefix-stop" pwsh @('-NoProfile','-File',"$lab/compact9-runtime-v2/compact9-tools/cluster/stop.ps1") 180)
   Assert-Compact9CleanupAuthority;$clean=$true
  }
 }catch{
  $_|Out-String|Set-Content "$lab/logs/$prefix-cleanup-$cleanupPhase-retained.txt"
  try{Assert-Compact9CleanupAuthority}catch{$guardFailure=$_}
 }
 $releaseErrors=@()
 foreach($name in @('heavy','worker')){
  $held=if($name -ceq 'heavy'){$heldHeavy}else{$heldWorker}
  if(!$held){continue}
  if($name -ceq 'worker'){
   try{Assert-Compact9CleanupAuthority;if($heldHeavy){throw 'heavy potential/unconfirmed lease retained; worker retained'};if($started -and !$clean){throw 'private lifecycle not proved clean; worker lease retained'}}catch{
    $guardFailure=$_;@{phase=$cleanupPhase;worker_release_attempted=$false;worker_lease_retained=$true;reason=$_.Exception.Message}|ConvertTo-Json|Set-Content "$lab/logs/$prefix-worker-retained.json";continue
   }
   $workerReleaseAttempted=$true
  }
  try{
   $release=Invoke-LiveBounded pwsh @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','release','load9-983ad8f3eae1','-Name',$name) 15
   $release|ConvertTo-Json|Set-Content "$lab/logs/$prefix-$name-release.json"
   if($release.ExitCode){throw "$name release refused"}
   if($name -ceq 'worker'){Assert-Compact9CleanupAuthority}
   if($name -ceq 'heavy'){$heldHeavy=$false}else{$heldWorker=$false}
  }catch{$releaseErrors+=@($_|Out-String)}
 }
 try{Assert-Compact9CleanupAuthority}catch{$guardFailure=$_}
 if($guardFailure){@{phase=$cleanupPhase;writer_or_lifecycle_completion_unproved=$true;worker_release_attempted=$workerReleaseAttempted;worker_lease_state=$(if($workerReleaseAttempted -and $heldWorker){'release_outcome_unconfirmed'}elseif($heldWorker){'retained'}else{'released'});native_hold_may_be_retained=$true;receipt_root=$taskReceipts;reason=$guardFailure.Exception.Message}|ConvertTo-Json|Set-Content "$lab/logs/$prefix-unresolved-writer.json"}
 if($releaseErrors.Count){$releaseErrors|Set-Content "$lab/logs/$prefix-release-errors.txt"}
 @{registration_state=$(if($clean){'unregistered'}elseif($registered){'registered'}elseif($registrationAttempted){'unconfirmed'}else{'unregistered'});guarded_stop_clean=$clean;resources_may_need_exact_owned_cleanup=(!$clean -or [bool]$guardFailure);worker_lease_held_or_unconfirmed=$heldWorker;database_default60_cleanup_pending=$true;no_purge=$true}|ConvertTo-Json|Set-Content "$lab/logs/$prefix-cleanup-status.json"
 if($guardFailure){throw $guardFailure}
 if($releaseErrors.Count){throw 'one or more FIFO releases failed; inspect retained errors'}
}
Assert-Compact9RuntimeClosure -RetainedEvidence
