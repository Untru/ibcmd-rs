# Fresh wave3 8.3 contexts only. Ports identify conflicts, never ownership.
$script:PrivateMappings = @{
 'F:\ibcmd\lab\05\wave3\load\cluster' = @{ track='load9-983ad8f3eae1'; manifest_track='load'; prefix='ibcmd_rs_05_load_w3_' }
}
$script:Root = [IO.Path]::GetFullPath($env:IBCMD_RS_WORKER_LAB_ROOT).TrimEnd('\')
if (-not $script:PrivateMappings.ContainsKey($script:Root)) { throw 'unknown private83 context' }
$script:PrivateContext = $script:PrivateMappings[$script:Root]
$script:Platform='8.3.27.2214';$script:Bin="C:\Program Files\1cv8\$script:Platform\bin"
$script:StateFile=Join-Path $script:Root 'state.json';$script:Srvinfo=Join-Path $script:Root 'srvinfo';$script:Logs=Join-Path $script:Root 'logs'
$script:AgentPort=5540;$script:RegPort=5541;$script:RasPort=5545;$script:RangeFrom=5560;$script:RangeTo=5591
$script:RasAddress='localhost:5545';$script:Srvr='localhost:5541';$script:Rac=Join-Path $script:Bin 'rac.exe'
$script:AllPorts=@(5540,5541,5545)+@(5560..5591)
$script:ServerNames=@('ragent.exe','rmngr.exe','rphost.exe','ras.exe','dbda.exe')
$script:PrivateStartupDeadline=[DateTime]::MinValue
$script:PrivateStartupInitializedUtc=[DateTime]::MinValue
$script:PrivateStartupPhase='startup_listener_acquisition'
. "$PSScriptRoot\..\live\process.ps1"
function Require-PrivatePath([string]$Path) {
 if(!$script:PrivateMappings.ContainsKey($script:Root) -or $script:PrivateContext.track -cne $script:PrivateMappings[$script:Root].track -or $script:PrivateContext.prefix -cne $script:PrivateMappings[$script:Root].prefix){throw 'unknown private83 root/track mapping'}
 $resolved=[IO.Path]::GetFullPath($Path).TrimEnd('\')
 if($resolved -ne $script:Root -and -not $resolved.StartsWith($script:Root+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'private path escaped selected root'}
 for($probe=$resolved;$probe;$probe=[IO.Path]::GetDirectoryName($probe)){
  if((Test-Path -LiteralPath $probe) -and ((Get-Item -LiteralPath $probe -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'private path has reparse ancestry'}
 }
 return $resolved
}
[void](Require-PrivatePath $script:Root)
function Say([string]$Text){"$(Get-Date -Format s) $Text"}
function Get-ServerProcesses{@(Get-CimInstance Win32_Process|Where-Object{$script:ServerNames -contains $_.Name})}
function Get-ClusterListeners{@(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue|Where-Object{$script:AllPorts -contains $_.LocalPort})}
function Private-Key($Identity){"$($Identity.pid):$(([DateTime]$Identity.born).ToUniversalTime().Ticks)"}
function Private-Identity($Process,[string]$ParentKey=''){
 [pscustomobject]@{pid=[int]$Process.ProcessId;born=$Process.CreationDate.ToUniversalTime().ToString('o');executable=$Process.ExecutablePath;command=$Process.CommandLine;parent_key=$ParentKey}
}
function Private-Same($Process,$Identity){
 $Process -and $Identity -and [int]$Process.ProcessId -eq $Identity.pid -and
 $Process.CreationDate.ToUniversalTime().Ticks -eq ([DateTime]$Identity.born).ToUniversalTime().Ticks -and
 $Process.ExecutablePath -eq $Identity.executable -and $Process.CommandLine -ceq $Identity.command
}
function Read-State {
 [void](Require-PrivatePath $script:StateFile)
 if(-not(Test-Path -LiteralPath $script:StateFile)){return $null}
 $state=Get-Content -LiteralPath $script:StateFile -Raw|ConvertFrom-Json
 if($state.format -ne 1 -or $state.platform -cne $script:Platform -or $state.root -ne $script:Root -or $state.track -cne $script:PrivateContext.track -or $state.fresh -ne $true){throw 'invalid private83 state'}
 $anchors=@($state.anchors);if(!$anchors.Count -or $anchors.Count -gt 2){throw 'private anchors unavailable'}
 $agent='"'+$script:Bin+'\ragent.exe" -agent -port 5540 -regport 5541 -range 5560:5591 -d '+$script:Srvinfo
 $ras='"'+$script:Bin+'\ras.exe" cluster --port=5545 localhost:5540'
 foreach($anchor in $anchors){
  if($anchor.parent_key -or $anchor.command -cnotin @($agent,$ras)){throw 'anchor does not bind private root/ports'}
  $expected=if($anchor.command -ceq $agent){Join-Path $script:Bin 'ragent.exe'}else{Join-Path $script:Bin 'ras.exe'}
  if($anchor.executable -ne $expected){throw 'private anchor executable mismatch'}
 }
 if(@($anchors|Where-Object{$_.command -ceq $agent}).Count -ne 1 -or @($anchors|Where-Object{$_.command -ceq $ras}).Count -gt 1){throw 'duplicate/missing private anchor'}
 $map=@{};foreach($identity in (@($state.anchors)+@($state.known))){
  if(!$identity){continue};$key=Private-Key $identity
  if($identity.pid -le 0 -or [IO.Path]::GetFileName($identity.executable) -notin $script:ServerNames -or $identity.executable -ne (Join-Path $script:Bin ([IO.Path]::GetFileName($identity.executable)))){throw 'invalid private process identity'}
  if($map.ContainsKey($key) -and ($map[$key].command -cne $identity.command -or $map[$key].parent_key -cne $identity.parent_key)){throw 'conflicting private process identity'}
  $map[$key]=$identity
 }
 $rootKeys=@($state.anchors|ForEach-Object{Private-Key $_})
 foreach($identity in @($state.known)){
  if(!$identity){continue};$cursor=$identity;$seen=@{}
  while((Private-Key $cursor) -notin $rootKeys){
   $key=Private-Key $cursor;if($seen.ContainsKey($key) -or !$cursor.parent_key -or !$map.ContainsKey($cursor.parent_key)){throw 'private descendant has no retained ancestry'}
   $seen[$key]=$true;$parent=$map[$cursor.parent_key]
   if(([DateTime]$cursor.born).ToUniversalTime().Ticks -lt ([DateTime]$parent.born).ToUniversalTime().Ticks){throw 'ambiguous private descendant birthday'}
   $cursor=$parent
  }
 }
 return $state
}
function Write-State($State){[void](Require-PrivatePath $script:StateFile);$State|ConvertTo-Json -Depth 9|Set-Content -LiteralPath $script:StateFile -Encoding utf8}
function Private-Owned($State,[object[]]$Processes){
 if(!$State){return @()};$all=if($PSBoundParameters.ContainsKey('Processes')){@($Processes)}else{Get-ServerProcesses};$owned=@{}
 foreach($identity in (@($State.anchors)+@($State.known))){
  if(!$identity){continue};$process=@($all|Where-Object{$_.ProcessId -eq $identity.pid})
  if($process.Count -gt 1){throw 'duplicate private PID'}
  if($process.Count){if(!(Private-Same $process[0] $identity)){throw 'private PID reused; no signal permitted'};$owned[[int]$process[0].ProcessId]=$process[0]}
 }
 do{$added=$false;foreach($process in $all){
  if(!$owned.ContainsKey([int]$process.ProcessId) -and $owned.ContainsKey([int]$process.ParentProcessId)){
   if($process.ExecutablePath -ne (Join-Path $script:Bin $process.Name) -or $process.CreationDate.ToUniversalTime().Ticks -lt $owned[[int]$process.ParentProcessId].CreationDate.ToUniversalTime().Ticks){throw 'unknown private descendant executable/birthday'}
   $owned[[int]$process.ProcessId]=$process;$added=$true
  }
 }}while($added)
 return @($owned.Values)
}
function Get-ClusterProcesses{Private-Owned (Read-State)}
function Private-Remember($State){
 $identities=@{};foreach($identity in (@($State.anchors)+@($State.known))){if($identity){$identities[(Private-Key $identity)]=$identity}}
 $live=@(Private-Owned $State);$byPid=@{};foreach($process in $live){$byPid[[int]$process.ProcessId]=$process}
 foreach($process in $live){
  $identity=Private-Identity $process;$key=Private-Key $identity
  if(!$identities.ContainsKey($key)){
   if(!$byPid.ContainsKey([int]$process.ParentProcessId)){throw 'descendant lost parent before capture'}
   $identity.parent_key=Private-Key (Private-Identity $byPid[[int]$process.ParentProcessId]);$identities[$key]=$identity
  }
 }
 $State.known=@($identities.Values);Write-State $State
}
function Private-StateDigest {
 $path=Require-PrivatePath $script:StateFile
 $stream=[IO.File]::OpenRead($path)
 try{
  $buffer=[byte[]]::new(65537);$read=0
  while($read -lt $buffer.Length){$next=$stream.Read($buffer,$read,$buffer.Length-$read);if(!$next){break};$read+=$next}
  if(!$read -or $read -gt 65536){throw 'state exceeds refusal receipt byte budget or is empty'}
  return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([byte[]]$buffer[0..($read-1)]))
 }finally{$stream.Dispose()}
}
function Private-PublishRefusalReceipt($Receipt){
 $bytes=[Text.Encoding]::UTF8.GetBytes(($Receipt|ConvertTo-Json -Depth 7))
 if($bytes.Length -gt 65536){throw 'refusal receipt exceeds byte budget'}
 $path=Require-PrivatePath (Join-Path $script:Logs ('listener-refusal-'+[guid]::NewGuid().ToString('N')+'.json'))
 $file=[IO.File]::Open($path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
 try{$file.Write($bytes);$file.Flush($true)}finally{$file.Dispose()}
 return $path
}
function Private-ListenerRefusal($State,$FailedListener,$Listeners,$Processes,$OwnedIds,[string]$Reason='listener_unproved',$StartupObservation=$null){
 # Diagnostics consume only the existing listener/process censuses. They never
 # perform another census for admission, retry or process signal.
 try{
  $receipt=[ordered]@{format=1;captured_utc=[DateTime]::UtcNow.ToString('o');root=$script:Root;state_sha256=$(if($State){Private-StateDigest}else{$null});failed_listener=$FailedListener;listeners=@($Listeners|ForEach-Object{[ordered]@{port=$_.LocalPort;pid=$_.OwningProcess;address=$_.LocalAddress}});owned_ids=@($OwnedIds);process_census=@($Processes|ForEach-Object{
   [ordered]@{pid=$_.ProcessId;parent=$_.ParentProcessId;born=$_.CreationDate.ToUniversalTime().ToString('o');name=$_.Name;executable=$_.ExecutablePath;command_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes([string]$_.CommandLine)));root_marker=([string]$_.CommandLine).IndexOf($script:Root,[StringComparison]::OrdinalIgnoreCase) -ge 0}
  });admission='refused';reason=$Reason;additional_admission_censuses=0}
  if($StartupObservation){$receipt.startup=$StartupObservation}
  $path=Private-PublishRefusalReceipt $receipt
 }catch{
  if($Reason -ceq 'startup_deadline_expired'){throw 'private startup deadline expired; no admission or signal permitted; diagnostic receipt unavailable'}
  throw 'private port has foreign/unproved listener; no signal permitted; diagnostic receipt unavailable'
 }
 if($Reason -ceq 'startup_deadline_expired'){throw "private startup deadline expired; no admission or signal permitted; retained receipt $path"}
 throw "private port has foreign/unproved listener; no signal permitted; retained receipt $path"
}
function Private-StartupObservation([DateTime]$Deadline,[DateTime]$EntryUtc,[bool]$Unstable=$false){
 [ordered]@{deadline_utc=$Deadline.ToUniversalTime().ToString('o');initialized_utc=$(if($script:PrivateStartupInitializedUtc -ne [DateTime]::MinValue){$script:PrivateStartupInitializedUtc.ToUniversalTime().ToString('o')}else{$null});entry_utc=$EntryUtc.ToUniversalTime().ToString('o');observed_utc=[DateTime]::UtcNow.ToString('o');phase=$script:PrivateStartupPhase;unstable_listener_observation=$Unstable}
}
function Private-RelevantCensus($Processes,$Listeners){
 $ids=@($Listeners|ForEach-Object{$_.OwningProcess})
 @($Processes|Where-Object{$_.Name -in $script:ServerNames -or $_.ProcessId -in $ids})
}
function Private-StartupListeners($State,[DateTime]$Deadline){
 $entryUtc=[DateTime]::UtcNow
 if(!$State -or !$State.fresh){throw 'bounded fresh startup acquisition required'}
 if($Deadline -le $entryUtc){Private-ListenerRefusal $State $null @() @() @() 'startup_deadline_expired' (Private-StartupObservation $Deadline $entryUtc)}
 do{
  Private-RequireLease $State
  $first=@(Get-CimInstance Win32_Process)
  $listeners=@(Get-ClusterListeners)
  $second=@(Get-CimInstance Win32_Process)
  $ownedFirst=@(Private-Owned $State -Processes @($first|Where-Object{$_.Name -in $script:ServerNames}))
  $ownedSecond=@(Private-Owned $State -Processes @($second|Where-Object{$_.Name -in $script:ServerNames}))
  $unstable=$false
  foreach($listener in $listeners){
   $a=@($first|Where-Object{$_.ProcessId -eq $listener.OwningProcess});$b=@($second|Where-Object{$_.ProcessId -eq $listener.OwningProcess})
   if($a.Count -gt 1 -or $b.Count -gt 1){throw 'ambiguous startup listener PID'}
   foreach($sample in @(@{entries=$a;owned=$ownedFirst},@{entries=$b;owned=$ownedSecond})){
    foreach($entry in $sample.entries){
     if($entry.Name -notin $script:ServerNames -or $entry.ProcessId -notin $sample.owned.ProcessId){
      Private-ListenerRefusal $State ([ordered]@{port=$listener.LocalPort;pid=$listener.OwningProcess;address=$listener.LocalAddress}) $listeners (Private-RelevantCensus (@($first)+@($second)) $listeners) @($ownedSecond.ProcessId) 'listener_unproved' (Private-StartupObservation $Deadline $entryUtc)
     }
    }
   }
   if($a.Count -and $b.Count -and (!(Private-Same $b[0] (Private-Identity $a[0])) -or $a[0].ParentProcessId -ne $b[0].ParentProcessId)){Private-ListenerRefusal $State ([ordered]@{port=$listener.LocalPort;pid=$listener.OwningProcess;address=$listener.LocalAddress}) $listeners (Private-RelevantCensus (@($first)+@($second)) $listeners) @($ownedSecond.ProcessId) 'listener_identity_or_ancestry_drift' (Private-StartupObservation $Deadline $entryUtc)}
   if(!$a.Count -or !$b.Count){$unstable=$true}
  }
  foreach($sample in @(@{processes=$first;owned=$ownedFirst},@{processes=$second;owned=$ownedSecond})){
   foreach($process in $sample.processes){
    $marked=$process.CommandLine -and ($process.CommandLine.IndexOf($script:Root,[StringComparison]::OrdinalIgnoreCase) -ge 0 -or $process.CommandLine -match '(?i)(?:^|\s)(?:-port[= ]5540|-regport[= ]5541|--port=5545|-range[= ]5560:5591)(?:\s|$)')
    if($marked -and ($process.Name -notin $script:ServerNames -or $process.ProcessId -notin $sample.owned.ProcessId)){throw 'unknown private-looking startup process; no retry or signal permitted'}
   }
  }
  if(!$unstable -and [DateTime]::UtcNow -lt $Deadline){return $listeners}
  if([DateTime]::UtcNow -lt $Deadline){Start-Sleep -Milliseconds 100}
 }while([DateTime]::UtcNow -lt $Deadline)
 # Expiry does not identify a foreign listener. Preserve both original censuses,
 # including missing/unstable observations, without selecting an owned PID as a culprit.
 Private-ListenerRefusal $State $null $listeners (Private-RelevantCensus (@($first)+@($second)) $listeners) @($ownedSecond.ProcessId) 'startup_deadline_expired' (Private-StartupObservation $Deadline $entryUtc $unstable)
}
function Private-RequireListeners($State,[DateTime]$StartupDeadline=[DateTime]::MinValue){
 if($StartupDeadline -ne [DateTime]::MinValue){[void](Private-StartupListeners $State $StartupDeadline);return}
 # Take listeners first, then use ONE process census for both ancestry proof
 # and unknown-process refusal. A child born between independent process
 # censuses must not be labelled foreign merely because the earlier census
 # did not contain it. Every admitted PID still needs exact ancestry/identity.
 $listeners=@(Get-ClusterListeners);$processes=@(Get-ServerProcesses)
 $live=@(Private-Owned $State -Processes $processes);$ids=@($live.ProcessId)
 foreach($listener in $listeners){if($listener.OwningProcess -notin $ids){Private-ListenerRefusal $State ([ordered]@{port=$listener.LocalPort;pid=$listener.OwningProcess;address=$listener.LocalAddress}) $listeners $processes $ids}}
 foreach($process in $processes){
  $marked=$process.CommandLine -and ($process.CommandLine.IndexOf($script:Root,[StringComparison]::OrdinalIgnoreCase) -ge 0 -or $process.CommandLine -match '(?i)(?:^|\s)(?:-port[= ]5540|-regport[= ]5541|--port=5545|-range[= ]5560:5591)(?:\s|$)')
  if($marked -and $process.ProcessId -notin $ids){throw 'unknown private-looking process; ports/root are not ownership'}
 }
}
function Private-Lease {
 $path='F:\ibcmd\lab\04\locks\worker\owner.txt'
 if(!(Test-Path -LiteralPath $path)){throw 'whole lifecycle worker FIFO lease required'}
 $lease=(Get-Content -LiteralPath $path -Raw).Trim()
 if($lease -notmatch ('^track='+[regex]::Escape($script:PrivateContext.track)+' since=\S+$')){throw 'worker FIFO held by another track'}
 return $lease
}
function Private-RequireLease($State){if((Private-Lease) -cne $State.worker_lease){throw 'worker FIFO lease changed; no signal permitted'}}
function Invoke-Rac{
 $state=Read-State;if(!$state){throw 'no private state for RAS operation'}
 if($script:PrivateStartupDeadline -ne [DateTime]::MinValue){$script:PrivateStartupPhase='rac '+(@($args|Select-Object -First 2) -join ' ')}
 Private-RequireListeners $state -StartupDeadline $script:PrivateStartupDeadline
 $ras=@(Private-Owned $state|Where-Object{$_.Name -eq 'ras.exe'})
 if($ras.Count -ne 1 -or !(Get-ClusterListeners|Where-Object{$_.LocalPort -eq 5545 -and $_.OwningProcess -eq $ras[0].ProcessId})){throw 'private RAS listener identity unavailable'}
 $result=Invoke-LiveBounded -Executable $script:Rac -Arguments (@($script:RasAddress)+@($args)) -TimeoutSeconds 5
 if($result.ExitCode -ne 0 -or $result.Stderr){throw 'private RAS command failed/timed out; no lifecycle signal admitted'}
 $result.Stdout -split '\r?\n'
}
function Get-ClusterId{
 $state=Read-State;$ids=@(Invoke-Rac cluster list|Select-String '^cluster\s*:'|ForEach-Object{($_ -replace '^cluster\s*:\s*','').Trim()})
 if($ids.Count -ne 1 -or ![Guid]::TryParse($ids[0],[ref]([Guid]::Empty)) -or ($state.cluster -and $ids[0] -ne $state.cluster)){throw 'private cluster identity unavailable/drifted'}
 return $ids[0]
}
function Private-RequireNames([string[]]$Names){
 $ownBackups=([IO.Path]::GetDirectoryName($script:Root))+'\'
 $manifest=@(Import-Csv -LiteralPath 'F:\ibcmd\lab\04\databases.tsv' -Delimiter "`t"|Where-Object{
  if($_.track -cne $script:PrivateContext.manifest_track -or $_.notes -notmatch '^from (.+)$'){return $false}
  $origin=$Matches[1];if(![IO.Path]::IsPathFullyQualified($origin)){return $false}
  $origin=[IO.Path]::GetFullPath($origin)
  $origin.StartsWith('F:\ibcmd\lab\dbbak\',[StringComparison]::OrdinalIgnoreCase) -or $origin.StartsWith($ownBackups,[StringComparison]::OrdinalIgnoreCase)
 })
 foreach($name in $Names){if($name -cnotmatch ('^'+[regex]::Escape($script:PrivateContext.prefix)+'[a-z0-9_]+$') -or $name -cnotin @($manifest.database)){throw 'private registry contains foreign/unmanifested infobase; no signal permitted'}}
}
function Private-Snapshot($State,[string]$Name){
 $cluster=Get-ClusterId;if($cluster -ne $State.cluster){throw 'private cluster drift'}
 $path=Join-Path $script:Logs ($Name+'-'+(Get-Date -Format yyyyMMddHHmmssfff)+'.log');[void](Require-PrivatePath $path)
 $raw=@(Invoke-Rac infobase summary list "--cluster=$cluster");$raw|Set-Content -LiteralPath $path -Encoding utf8
 $text=$raw -join "`n";$names=@();$seen=@{}
 if($text.Trim()){
  foreach($block in @($text -split '(?:\r?\n){2,}'|Where-Object{$_.Trim()})){
   $nameFields=@($block -split '\r?\n'|Where-Object{$_ -match '^name\s*:'});$idFields=@($block -split '\r?\n'|Where-Object{$_ -match '^infobase\s*:'})
   if($nameFields.Count -ne 1 -or $idFields.Count -ne 1){throw 'unparsed private registration inventory'}
   $id=($idFields[0] -replace '^infobase\s*:\s*','').Trim();if(![Guid]::TryParse($id,[ref]([Guid]::Empty)) -or $seen.ContainsKey($id)){throw 'ambiguous private registration UUID'};$seen[$id]=$true
   $names+=($nameFields[0] -replace '^name\s*:\s*','').Trim().Trim('"')
  }
 }
 Private-RequireNames $names
 Invoke-Rac connection list "--cluster=$cluster"|Add-Content -LiteralPath $path -Encoding utf8
 Invoke-Rac process list "--cluster=$cluster"|Add-Content -LiteralPath $path -Encoding utf8
 [pscustomobject]@{path=$path;names=$names}
}
function Get-OtherServerSnapshot{
 $mine=@((Get-ClusterProcesses).ProcessId)
 @(Get-ServerProcesses|Where-Object{$_.ProcessId -notin $mine}|Sort-Object ProcessId|ForEach-Object{'{0} {1} {2:o}' -f $_.ProcessId,$_.Name,$_.CreationDate})
}
function Stop-ClusterProcesses{throw 'private83 signals require guarded stop.ps1'}
