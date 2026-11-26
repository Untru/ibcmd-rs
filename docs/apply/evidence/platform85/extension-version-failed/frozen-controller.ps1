# Native-only oracle. No product extension-write/profile seam.
param([ValidatePattern('^[A-Fa-f0-9]{64}$')][string]$FrozenManifestSha,[switch]$SelfTest)
$ErrorActionPreference='Stop'
$lab='F:\ibcmd\lab\05\wave3\platform85'
$wt='F:\ibcmd\src\ibcmd-rs-05-platform85-wave3'
$kit="$lab\tools\extension-v2-kit"
. "$kit\lib.ps1"
$db='ibcmd_rs_05_p85_w3_ext_version_native_20261001'
$tag='ext-version-native-v2';$run="$lab\$tag"
$epf="$lab\observer\IbcmdRsObserver.extension-version.epf"
$epfSha='49C96B0A53AEF8D75E22A3D49442D90524C00EA07085CEDF676007AFCD52F610'
$labels=@('ext-version-native-old-v2','ext-version-native-new-v2')
$clients=@();$worker=$false;$heavy=$false;$started=$false;$registered=$false;$registrationAttempted=$false;$nativeRunning=$false
$deadline=[datetime]::MaxValue
$lock='F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$dependency="$kit\process.ps1"
$acceptedDependencySha='F376181945FB172BC97E27CE90AB7FC77FE4B89D41C9EC21E7C5DFFB62D5BC7D'
$script:stepNumber=0;$script:potentialNativeHold=$false
function RequireControlClosure {
 $path="$lab\logs\extension-control-frozen-v4.json"
 RequireLabPaths85 @($path)
 if(!$FrozenManifestSha -or (Get-Item -LiteralPath $path).Length -gt 4MB -or (Get-FileHash -LiteralPath $path).Hash -ine $FrozenManifestSha){throw 'reviewed complete controller closure required'}
 $manifest=Get-Content -LiteralPath $path -Raw|ConvertFrom-Json
 if($manifest.scope -cne 'native-only initial extension Version transition' -or @($manifest.files).Count -gt 2500){throw 'controller closure scope/budget differs'}
 foreach($entry in $manifest.files){
  $canonical=[IO.Path]::GetFullPath($entry.file)
  if($canonical -cne $entry.file){throw 'controller closure path is not exact canonical'}
  for($probe=$canonical;$probe;$probe=[IO.Path]::GetDirectoryName($probe)){
   if((Get-Item -LiteralPath $probe -Force).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'controller dependency reparse ancestry'}
  }
  if((Get-Item -LiteralPath $canonical).Length -ne $entry.length -or (Get-FileHash -LiteralPath $canonical).Hash -cne $entry.sha256){throw 'frozen controller dependency/source/executable drift'}
 }
 foreach($entry in $manifest.executables.PSObject.Properties){
  if(@(Get-Command $entry.Name -CommandType Application)[0].Source -cne $entry.Value){throw 'effective child executable resolution drift'}
 }
}
function AssertControllerAuthority {
 if(Test-LiveUncertainChild){throw 'unresolved owned child/writer; no next action or unregister/stop/native/worker release'}
}
function RunChild([string]$label,[string]$exe,[string[]]$argv,[int]$seconds,[switch]$IndependentHeavyRelease){
 if(!$IndependentHeavyRelease){AssertControllerAuthority}
 RequireControlClosure
 # Hashing/queue preparation consumes time too. Refuse new work immediately
 # before dispatch, but keep a FIFO wait's own bound and guarded cleanup.
 if($script:deadline -ne [datetime]::MaxValue -and !$IndependentHeavyRelease -and $label -notmatch '(?:release|stop|unregister)$|cleanup'){
  $left=Remaining ([int]::MaxValue)
  if($label -notmatch '-acquire$'){$seconds=[Math]::Min($seconds,$left)}
 }
 $script:stepNumber++;$stem=('{0:D3}-{1}' -f $script:stepNumber,$label)
 $path="$run\$stem.json";if((Test-Path -LiteralPath $path) -or (Test-Path -LiteralPath "$run\$stem.command.json")){throw 'fresh child evidence required'}
 @{executable=$exe;arguments=$argv;deadline_seconds=$seconds;started_utc=[datetime]::UtcNow.ToString('o')}|ConvertTo-Json -Depth 5|Set-Content "$run\$stem.command.json"
 try{
  $result=Invoke-LiveBounded $exe $argv $seconds
  $result|ConvertTo-Json -Depth 5|Set-Content $path
  if($result.ExitCode -ne 0){throw "bounded child $label returned nonzero; no retry"}
  if(!$IndependentHeavyRelease){AssertControllerAuthority}
  return $result
 }catch{$_|Out-String|Set-Content "$run\$stem.failure.txt";throw}
}
function CompleteControllerNativeHold([string]$mode){
 if(!$script:potentialNativeHold){return}
 AssertControllerAuthority
 [void](RunChild "native-$mode-release" pwsh @('-NoProfile','-File',$lock,'release',"p85-ext-$mode-v2",'-Name','native') 15)
 AssertControllerAuthority;$script:potentialNativeHold=$false;$script:nativeRunning=$false
}
function RequireLiveControllerContext([string]$mode){
 AssertControllerAuthority
 $state=State85;if(!$state){throw 'private lifetime absent after FIFO wait'}
 $snapshot=Snapshot85 $state "extension-before-native-$mode"
 AssertControllerAuthority
 $snapshot|ConvertTo-Json|Set-Content "$run\before-native-$mode.json"
 if($snapshot.names.Count -ne 1 -or $snapshot.names[0] -cne $db){throw 'native target is not sole private owned registration'}
 if(!$script:clients.Count){throw 'actual old observer is absent; no native Version transition'}
 foreach($client in $script:clients){
  $fresh=Get-CimInstance Win32_Process -Filter "ProcessId=$($client.identity.ProcessId)"
  if(!(SameProcess $client.identity $fresh) -or !$client.session){throw 'bound observer producer absent/drifted after FIFO wait'}
  $lines=@(Get-Content -LiteralPath "$lab\obs\$($client.label).log" -Encoding utf8)
  if(@($lines|Where-Object{$_ -match '\|error\|'}).Count){throw 'actual observer reported error; no native action'}
  $polls=@($lines|Where-Object{$_ -match '\|poll\|'});if(!$polls.Count){throw 'actual observer has no poll after FIFO wait'}
  $last=CheckPoll $polls[-1] $client.label
  if($last[3] -cne $client.session['session-id']){throw 'actual journal SID changed after FIFO wait'}
 }
}
function RequireVersionOnlyFixture {
 RequireLabPaths85 @("$lab\logs\extension-settled-fixture-delta-v2.json")
 if((Get-Item -LiteralPath "$lab\logs\extension-settled-fixture-delta-v2.json").Length -gt 1MB){throw 'Version fixture inventory exceeds bound'}
 $fixture=Get-Content "$lab\logs\extension-settled-fixture-delta-v2.json" -Raw|ConvertFrom-Json
 if($fixture.actual_old_version -cne '1.8.3.0' -or $fixture.new_version -cne '1.8.3.1' -or $fixture.CFE_built -ne $false -or $fixture.changes.Count -ne 1 -or $fixture.changes[0].path -cne 'Configuration.xml' -or $fixture.files_count -ne 633){throw 'actual settled Version-only fixture record differs'}
 $a="$lab\extension-version-settled-A";$b="$lab\extension-version-settled-B-v2"
 RequireLabPaths85 @($a,$b)
 foreach($item in @(Get-Item -LiteralPath $a)+@(Get-ChildItem -LiteralPath $a -Force -Recurse)+@(Get-Item -LiteralPath $b)+@(Get-ChildItem -LiteralPath $b -Force -Recurse)){if($item.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'fixture tree reparse'}}
 if(@($fixture.records).Count -ne 633 -or @($fixture.records.path|Sort-Object -Unique).Count -ne 633){throw 'fixture record inventory missing or duplicated'}
 foreach($root in @($a,$b)){
  $actual=@(Get-ChildItem -LiteralPath $root -File -Recurse|ForEach-Object{[IO.Path]::GetRelativePath($root,$_.FullName).Replace('\','/')}|Sort-Object)
  $expected=@($fixture.records.path|Sort-Object)
  if($actual.Count -ne 633 -or (Compare-Object $actual $expected -CaseSensitive)){throw 'fixture file inventory differs'}
 }
 foreach($record in $fixture.records){
  if($record.path.Contains('..') -or [IO.Path]::IsPathFullyQualified($record.path)){throw 'fixture path not canonical relative'}
  $left=Join-Path $a $record.path;$right=Join-Path $b $record.path
  if((Get-FileHash -LiteralPath $left).Hash -cne $record.before_sha256 -or (Get-FileHash -LiteralPath $right).Hash -cne $record.after_sha256){throw 'fixture source hash drift'}
  if($record.path -cne 'Configuration.xml' -and $record.before_sha256 -cne $record.after_sha256){throw 'fixture changes more than Version'}
 }
 $plainA=[IO.File]::ReadAllText("$a\Configuration.xml");$plainB=[IO.File]::ReadAllText("$b\Configuration.xml")
 if([regex]::Matches($plainA,'<Version>1\.8\.3\.0</Version>').Count -ne 1 -or $plainA.Replace('<Version>1.8.3.0</Version>','<Version>1.8.3.1</Version>') -cne $plainB){throw 'Version scalar transformation differs'}
}
function SameProcess($a,$b){
 $a -and $b -and $a.ProcessId -eq $b.ProcessId -and $a.ParentProcessId -eq $b.ParentProcessId -and
 $a.CreationDate.ToUniversalTime().Ticks -eq $b.CreationDate.ToUniversalTime().Ticks -and
 $a.ExecutablePath -ceq $b.ExecutablePath -and $a.CommandLine -ceq $b.CommandLine
}
function CheckPoll([string]$line,[string]$label){
 $p=$line -split '\|',7
 if($p.Count -ne 7 -or $p[0] -notmatch '^\d+$' -or $p[1] -cne $label -or $p[2] -cne 'poll' -or
 $p[3] -notmatch '^\d+$' -or $p[4] -notmatch '^1\.8\.3\.[01]$' -or $p[5] -notmatch '^1\.8\.3\.[01]$' -or
 $p[6] -cne 'расширение=ServiceDesk'){throw 'not an exact extension Version poll'}
 return $p
}
function Remaining([int]$maximum){
 $seconds=[int][math]::Floor(($script:deadline-[datetime]::UtcNow).TotalSeconds)
 if($seconds -lt 5){throw 'lifecycle action budget exhausted; no new action, guarded finally required'}
 return [math]::Min($maximum,$seconds)
}
function RequireArchiveState($prior){
 if($prior.root -cne $script:Root -or $prior.track -cne 'p85' -or $prior.platform -cne '8.5.1.1150'){throw 'archive is not this exact private85 lifetime'}
 if(@($prior.anchors).Count -ne 2 -or @($prior.known).Count -lt 2){throw 'prior private85 complete identity history required before archive'}
 foreach($anchor in $prior.anchors){
  $bound=@($prior.known|Where-Object{$_.pid -eq $anchor.pid -and $_.born -ceq $anchor.born -and $_.executable -ceq $anchor.executable -and $_.command -ceq $anchor.command})
  if($bound.Count -ne 1){throw 'prior anchor missing from persistent identity union'}
 }
 foreach($known in (@($prior.anchors)+@($prior.known))){
  if(!$known){continue}
  $live=Get-CimInstance Win32_Process -Filter "ProcessId=$($known.pid)"
  if($live -and $live.ProcessId -eq $known.pid -and $live.CreationDate.ToUniversalTime().Ticks -eq ([datetime]$known.born).ToUniversalTime().Ticks){throw 'prior saved exact identity remains; no archive'}
 }
}
if($SelfTest){
 $a=[pscustomobject]@{ProcessId=11;ParentProcessId=22;CreationDate=[datetime]::UtcNow;ExecutablePath='owned.exe';CommandLine='whole command'}
 if(!(SameProcess $a $a)){throw 'same identity'};$count=1
 foreach($key in @('ProcessId','ParentProcessId','CreationDate','ExecutablePath','CommandLine')){
  $b=$a.PSObject.Copy();switch($key){ProcessId{$b.ProcessId++} ParentProcessId{$b.ParentProcessId++} CreationDate{$b.CreationDate=$b.CreationDate.AddTicks(1)} ExecutablePath{$b.ExecutablePath='foreign.exe'} CommandLine{$b.CommandLine+=' other'}}
  if(SameProcess $a $b){throw "identity drift admitted: $key"};$count++
 }
 $valid='63926474816420|label|poll|1|1.8.3.0|1.8.3.1|расширение=ServiceDesk'
 [void](CheckPoll $valid label);$count++
 foreach($bad in @($valid.Replace('|label|','|other|'),$valid.Replace('|poll|','|error|'),$valid.Replace('1.8.3.0','1.8.3.10'),$valid.Replace('ServiceDesk','Other'),($valid+'|extra'))){
  $refused=$false;try{[void](CheckPoll $bad label)}catch{$refused=$true};if(!$refused){throw 'malformed poll admitted'};$count++
 }
 $script:deadline=[datetime]::UtcNow.AddSeconds(30);if((Remaining 10) -ne 10){throw 'positive bounded budget'};$count++
 $script:deadline=[datetime]::UtcNow.AddSeconds(-1);$refused=$false;try{[void](Remaining 10)}catch{$refused=$true};if(!$refused){throw 'expired budget admitted'};$count++
 $script:fakeArchiveProcess=$null
 function Get-CimInstance{param($ClassName,$Filter);return $script:fakeArchiveProcess}
 $history=@([pscustomobject]@{pid=11;born='2026-10-01T19:00:00Z';executable='owned-agent';command='exact-root'},[pscustomobject]@{pid=22;born='2026-10-01T19:00:01Z';executable='owned-ras';command='exact-root'})
 $prior=[pscustomobject]@{root=$script:Root;track='p85';platform='8.5.1.1150';anchors=$history;known=$history}
 RequireArchiveState $prior;$count++
 foreach($key in @('root','track','platform')){$bad=$prior.PSObject.Copy();$bad.$key='foreign';$refused=$false;try{RequireArchiveState $bad}catch{$refused=$true};if(!$refused){throw 'foreign archive admitted'};$count++}
 foreach($key in @('anchors','known')){$bad=$prior.PSObject.Copy();$bad.$key=@();$refused=$false;try{RequireArchiveState $bad}catch{$refused=$true};if(!$refused){throw 'missing saved ownership admitted'};$count++}
 $script:fakeArchiveProcess=[pscustomobject]@{ProcessId=11;CreationDate=[datetimeoffset]::Parse('2026-10-01T22:00:00+03:00').UtcDateTime}
 $refused=$false;try{RequireArchiveState $prior}catch{$refused=$true};if(!$refused){throw 'UTC-equivalent live historical PID admitted'};$count++
 $script:fakeArchiveProcess.CreationDate=$script:fakeArchiveProcess.CreationDate.AddSeconds(1);RequireArchiveState $prior;$count++
 "readonly extension controller mocks PASS $count; no process/DB/UI operations";exit 0
}
if((Get-FileHash -LiteralPath $dependency).Hash -cne $acceptedDependencySha){throw 'accepted bounded helper source drift; no execution'}
RequireControlClosure
$receiptRoot="$lab\extension-v2-child-receipts"
RequireLabPaths85 @($receiptRoot)
if(!(Test-Path -LiteralPath $receiptRoot)){New-Item -ItemType Directory -Path $receiptRoot|Out-Null}
$env:IBCMD_RS_LIVE_CHILD_RECEIPT_ROOT=$receiptRoot
AssertControllerAuthority
RequireVersionOnlyFixture
RequireOwnedNames85 @($db)
RequireLabPaths85 @($run,$epf,"$lab\extension-version-settled-B-v2\Configuration.xml")
if(Test-Path -LiteralPath $run){throw 'fresh control folder required'}
foreach($label in $labels){RequireFreshObserver85 "$lab\obs" $label}
foreach($suffix in @('cold_prestart','warm_before','staged','applied','cohort')){RequireLabPaths85 @("$lab\snapshots\ext_version_native_v2_$suffix");if(Test-Path -LiteralPath "$lab\snapshots\ext_version_native_v2_$suffix"){throw 'snapshot evidence already exists; no overwrite'}}
foreach($mode in @('import','apply','export')){RequireLabPaths85 @("$lab\ibdata\$tag-$mode");if(Test-Path -LiteralPath "$lab\ibdata\$tag-$mode"){throw 'native mode data evidence already exists; no overwrite'}}
if((Get-FileHash -LiteralPath $epf).Hash -cne $epfSha){throw 'observer source/build provenance mismatch'}
if((Get-FileHash -LiteralPath "$lab\extension-version-settled-B-v2\Configuration.xml").Hash -cne 'B8E20A7D3C18BF63DCB9E84BF5A05D649009AD7C5BE09F47B220636A4B379ECB'){throw 'Version1.8.3.1 fixture drift'}
$env:TEMP="$lab\tmp";$env:TMP=$env:TEMP
New-Item -ItemType Directory -Path $run | Out-Null
function Kit([string]$scriptName,[string[]]$arguments=@(),[switch]$Cleanup){
 $seconds=if($Cleanup){120}else{Remaining 240}
 [void](RunChild "kit-$scriptName" pwsh (@('-NoProfile','-File',"$kit\$scriptName.ps1")+$arguments) $seconds)
}
function Snapshot([string]$suffix){
 [void](RunChild "snapshot-$suffix" python @("$lab\tools\snapshot_storage.py",$db,"ext_version_native_v2_$suffix") (Remaining 120))
}
function Native([ValidateSet('import','apply','export')][string]$mode){
 AssertControllerAuthority;[void](Remaining 60);RequireVersionOnlyFixture
 $exe='C:\Program Files\1cv8\8.5.1.1150\bin\ibcmd.exe'
 $args1=@('infobase','config',$mode)
 if($mode -eq 'import'){$args1+='files'}
 $args1+=@('--dbms=MSSQLServer','--db-server=localhost',"--db-name=$db",'--user=Администратор (обычное приложение)','--extension=ServiceDesk',"--data=$lab\ibdata\$tag-$mode")
 if($mode -eq 'import'){$args1+=@("--base-dir=$lab\extension-version-settled-B-v2",'--partial','Configuration.xml')}
 elseif($mode -eq 'apply'){$args1+=@('--force','--dynamic=force')}
 else{$args1+="$run\export"}
 $script:potentialNativeHold=$true
 [void](RunChild "native-$mode-acquire" pwsh @('-NoProfile','-File',$lock,'acquire',"p85-ext-$mode-v2",'-Name','native','-TimeoutMin','10') 620)
 try{
  # A spent work budget after the queue wait refuses the writer, then guarded finally releases.
  AssertControllerAuthority;RequireControlClosure;RequireVersionOnlyFixture;RequireLiveControllerContext $mode;$seconds=Remaining 60
  @{executable_sha256=(Get-FileHash -LiteralPath $exe).Hash;scope='native-only actual extension Version transition; no product admission';arguments=$args1}|ConvertTo-Json -Depth 5|Set-Content "$run\native-$mode-provenance.json"
  $script:nativeRunning=$true
  [void](RunChild "native-$mode" $exe $args1 $seconds)
 }finally{
  CompleteControllerNativeHold $mode
 }
}
function StartObserver([string]$label){
 $seconds=Remaining 180
 $startupError=$null
 try{[void](RunChild "$label-start" pwsh @('-NoProfile','-File',"$kit\obs.ps1",'start','-Database',$db,'-Label',$label,'-TimeoutSec',"$seconds") ($seconds+30))}catch{$startupError=$_}
 $pidPath="$lab\obs\$label.pid"
 if(Test-Path $pidPath){
  $id=[int](Get-Content $pidPath);$saved=Get-CimInstance Win32_Process -Filter "ProcessId=$id"
  if($saved){
   RequireObserverCommand85 $saved.CommandLine $label
   $identityPath="$lab\obs\$label.identity.json";RequireLabPaths85 @($identityPath)
   $identity=Get-Content -LiteralPath $identityPath -Raw|ConvertFrom-Json
   if($saved.ExecutablePath -cne "$script:Bin\1cv8c.exe" -or $saved.CreationDate.ToUniversalTime().Ticks -ne ([datetime]$identity.born).ToUniversalTime().Ticks -or $saved.CommandLine -cne $identity.command){throw 'observer saved spawn executable/birth/command differs'}
   $script:clients+=@{label=$label;identity=$saved;session=$null}
  }
 }
 if($startupError){throw $startupError};AssertControllerAuthority
 $pollDeadline=[datetime]::UtcNow.AddSeconds((Remaining 10))
 do{
  $lines=@(Get-Content "$lab\obs\$label.log" -Encoding utf8)
  if(@($lines|Where-Object{$_ -match '\|error\|'}).Count){throw 'actual extension observer reported error; no native write'}
  $polls=@($lines|Where-Object{$_ -match '\|poll\|'})
  if(!$polls.Count){Start-Sleep -Milliseconds 100}
 }while(!$polls.Count -and [datetime]::UtcNow -lt $pollDeadline)
 if(!$polls.Count){throw 'actual Version readiness absent'}
 $first=CheckPoll $polls[0] $label
 $bound=@($script:clients|Where-Object{$_.label -ceq $label});if($bound.Count -ne 1){throw 'exact live observer producer absent'}
 $raw=@(Rac85 @('session','list',"--cluster=$((State85).cluster)")) -join "`n"
 $raw|Set-Content "$run\$label-session-binding.log"
 $blocks=@(($raw -split '(?:\r?\n){2,}')|Where-Object{$_ -match ('(?m)^session-id\s*:\s*'+[regex]::Escape($first[3])+'\s*$')})
 if($blocks.Count -ne 1){throw 'session id ambiguous/absent'}
 $fields=@{};foreach($key in @('session','infobase','session-id','started-at','app-id')){if($blocks[0] -notmatch ('(?m)^'+$key+'\s*:\s*(\S+)\s*$')){throw "session binding field absent: $key"};$fields[$key]=$Matches[1]}
 if($fields['app-id'] -cne '1CV8C'){throw 'session app-id differs'}
 [void][guid]::Parse($fields.session);[void][guid]::Parse($fields.infobase)
 $registry=@(Rac85 @('infobase','summary','list',"--cluster=$((State85).cluster)")) -join "`n"
 $ibBlocks=@(($registry -split '(?:\r?\n){2,}')|Where-Object{$_ -match ('(?m)^name\s*:\s*'+[regex]::Escape($db)+'\s*$')})
 if($ibBlocks.Count -ne 1 -or $ibBlocks[0] -notmatch ('(?m)^infobase\s*:\s*'+[regex]::Escape($fields.infobase)+'\s*$')){throw 'session is not exact target registration'}
 $pollTime=[datetime]::new(([long]$first[0])*10000,[DateTimeKind]::Utc)
 if([math]::Abs(($pollTime-[datetime]::Parse($fields['started-at']).ToUniversalTime()).TotalSeconds) -gt 180){throw 'session start unbound to actual poll'}
 $bound[0].session=$fields
 @{label=$label;first_poll=$first}|ConvertTo-Json|Set-Content "$run\$label-first.json"
 return $first
}
function AssertActualJournals{
 $report=@()
 foreach($label in $labels){
  $client=@($clients|Where-Object{$_.label -ceq $label});if($client.Count -ne 1 -or !$client[0].session){throw 'journal producer has no exact session binding'}
  $path="$lab\obs\$label.log";$lines=@(Get-Content -LiteralPath $path -Encoding utf8)
  if(!$lines.Count -or @($lines|Where-Object{$_ -match '\|error\|'}).Count){throw 'journal is empty or contains actual errors'}
  $polls=@();foreach($line in @($lines|Select-Object -Skip 1)){$p=CheckPoll $line $label;if($p[3] -cne $client[0].session['session-id']){throw 'journal session changed'};$polls+=,[object[]]$p}
  if($polls.Count -lt 5){throw 'fewer than five actual Version polls'}
  if($label -ceq $labels[1] -and @($polls|Where-Object{$_[4] -cne '1.8.3.1' -or $_[5] -cne '1.8.3.1'}).Count){throw 'new session Version not consistently1.8.3.1'}
  $report+=@{label=$label;polls=$polls.Count;first_tick=$polls[0][0];last_tick=$polls[-1][0];actual_version_pairs=@($polls|ForEach-Object{"$($_[4])/$($_[5])"}|Sort-Object -Unique);session=$client[0].session;process_pid=$client[0].identity.ProcessId;process_birth=$client[0].identity.CreationDate.ToUniversalTime().ToString('o');journal_sha256=(Get-FileHash -LiteralPath $path).Hash}
 }
 $overlap=[long]([math]::Min([long]$report[0].last_tick,[long]$report[1].last_tick)-[math]::Max([long]$report[0].first_tick,[long]$report[1].first_tick))
 if($overlap -lt 5000){throw 'less than five seconds actual concurrent old/new Version evidence'}
 @{observations=$report;concurrent_overlap_ms=$overlap;scope='actual applied-session Version versus database Version only; no client-code marker or refresh claim'}|ConvertTo-Json -Depth 8|Set-Content "$run\actual-version-journals.json"
}
try{
 # Acquisition may have succeeded even when its child result is uncertain.
 $worker=$true;[void](RunChild worker-acquire pwsh @('-NoProfile','-File',$lock,'acquire','p85-ext-version-v2','-Name','worker','-TimeoutMin','20') 1240)
 RequireControlClosure
 $heavy=$true;[void](RunChild heavy-acquire pwsh @('-NoProfile','-File',$lock,'acquire','p85-ext-version-v2','-Name','heavy','-TimeoutMin','20') 1240)
 RequireControlClosure
 $script:deadline=[datetime]::UtcNow.AddSeconds(600)
 if(State85){throw 'existing private ownership state; no start or cleanup of prior lifetime'}
 # A new private registry must replace only fully stopped owned history.
 $oldStates=@(Get-ChildItem "$lab\cluster" -Filter 'stopped-state-*.json' -File|Sort-Object Name -Descending|Select-Object -First 1)
 if($oldStates.Count){RequireLabPaths85 @($oldStates[0].FullName);$prior=Get-Content $oldStates[0].FullName -Raw|ConvertFrom-Json;RequireArchiveState $prior}
 elseif((Test-Path "$lab\cluster\srvinfo") -or (Test-Path "$lab\cluster\logs")){throw 'unbound historical registry/logs; no archive'}
 foreach($process in @(Get-CimInstance Win32_Process)){if($process.CommandLine -and $process.CommandLine.IndexOf($script:Root,[StringComparison]::OrdinalIgnoreCase) -ge 0 -and $process.Name -in @('ragent.exe','ras.exe','rmngr.exe','rphost.exe','dbda.exe')){throw 'private-root marked server still present; no archive/start'}}
 foreach($name in @('srvinfo','logs')){
  $path="$lab\cluster\$name";if(Test-Path $path){RequireLabPaths85 @($path,"$run\prior-$name");foreach($entry in @(Get-Item $path)+@(Get-ChildItem -LiteralPath $path -Force -Recurse)){if($entry.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'archive tree reparse'}};Move-Item -LiteralPath $path -Destination "$run\prior-$name"}
 }
 $current="$lab\observer\IbcmdRsObserver.epf";RequireLabPaths85 @($current)
 if((Get-FileHash $current).Hash -cne '4B837D367DA885C29E02CA35BAE123335D118C83CC2F1820FE35C24B341BEA7B'){throw 'current form proof EPF differs; no overwrite'}
 if((Get-FileHash "$lab\observer\IbcmdRsObserver.form-candidate.epf").Hash -cne '4B837D367DA885C29E02CA35BAE123335D118C83CC2F1820FE35C24B341BEA7B'){throw 'retained form proof EPF missing'}
 Copy-Item -LiteralPath $epf -Destination $current
 Snapshot cold_prestart
 [void](RunChild cold-baseline-compare python @("$lab\tools\compare_full_snapshots.py",'--before',"$lab\snapshots\ext_version_native_clean_fixture_v2\inventory.json",'--after',"$lab\snapshots\ext_version_native_v2_cold_prestart\inventory.json") (Remaining 60))
 $seconds=Remaining 45;$started=$true;Kit start @('-TimeoutSec',"$seconds")
 [void](Remaining 30);$registrationAttempted=$true
 [void](RunChild register pwsh @('-NoProfile','-File','F:\ibcmd\lab\04\tools\register-ib.ps1','register','-Database',$db,'-Platform','8.5','-Track','p85','-Cluster','p85worker') (Remaining 120));$registered=$true
 $before=Snapshot85 (State85) 'extension-before-old';$before|ConvertTo-Json|Set-Content "$run\before-old.json"
 $old=StartObserver $labels[0]
 if($old[4] -cne '1.8.3.0' -or $old[5] -cne '1.8.3.0'){throw 'actual old Version1.8.3.0 readiness absent; native write refused'}
 Snapshot warm_before;Native import;Snapshot staged;Native apply;Snapshot applied
 $new=StartObserver $labels[1]
 if($new[4] -cne '1.8.3.1' -or $new[5] -cne '1.8.3.1'){throw 'new session Version1.8.3.1 absent; retain oracle, no admission'}
 [void](Remaining 10);Start-Sleep -Seconds 10
 Snapshot cohort;Native export;AssertActualJournals
 'Native-only oracle complete; old-session values require actual journal analysis; no product extension admission'|Set-Content "$run\oracle-only.txt"
}catch{
 $_|Out-String|Set-Content "$run\primary-failure.txt";throw
}finally{
 $clean=$false;$cleanupPhase='entry';$cleanupFailure=$null;$workerReleaseAttempted=$false
 try{
  AssertControllerAuthority
  if($started){
   if($registrationAttempted){
    $cleanupPhase='pre-signal-superset';AssertControllerAuthority
    $snap=Snapshot85 (State85) 'extension-before-signal';AssertControllerAuthority
    $snap|ConvertTo-Json|Set-Content "$run\before-signal.json"
    if($snap.names.Count -ne 1 -or $snap.names[0] -cne $db){throw 'private registry not exactly target; no signal'}
   }
   foreach($client in $clients){
    $cleanupPhase='owned-client-close';AssertControllerAuthority
    $fresh=Get-CimInstance Win32_Process -Filter "ProcessId=$($client.identity.ProcessId)"
    if($fresh){
     if(!(SameProcess $client.identity $fresh)){throw 'observer identity changed; no signal'}
     [void](RunChild "$($client.label)-stop" pwsh @('-NoProfile','-File',"$kit\obs.ps1",'stop','-Label',$client.label) 30)
     AssertControllerAuthority
    }
    for($n=0;$n -lt 20;$n++){if(!(Get-CimInstance Win32_Process -Filter "ProcessId=$($client.identity.ProcessId)")){break};Start-Sleep -Milliseconds 250}
    if(Get-CimInstance Win32_Process -Filter "ProcessId=$($client.identity.ProcessId)"){throw 'observer stop not positively complete'}
   }
   if($registrationAttempted){
    $cleanupPhase='ras-empty';$empty=$false
    for($n=0;$n -lt 30;$n++){
     AssertControllerAuthority
     $sessions=@(Rac85 @('session','list',"--cluster=$((State85).cluster)")) -join "`n"
     AssertControllerAuthority;$sessions|Set-Content "$run\cleanup-sessions-$n.log"
     if(!$sessions.Trim()){$empty=$true;break};Start-Sleep -Milliseconds 500
    }
    if(!$empty){throw 'RAS not proved empty after exact client close; retain registration/worker'}
   }
   if($registered){
    $cleanupPhase='unregister';AssertControllerAuthority
    [void](RunChild unregister pwsh @('-NoProfile','-File','F:\ibcmd\lab\04\tools\register-ib.ps1','unregister','-Database',$db,'-Platform','8.5','-Track','p85','-Cluster','p85worker') 120)
    AssertControllerAuthority;$registered=$false
   }elseif($registrationAttempted){throw 'registration attempt outcome unconfirmed; no cluster stop'}
   $cleanupPhase='stop';AssertControllerAuthority;Kit stop -Cleanup;AssertControllerAuthority;$clean=$true
  }
 }catch{$_|Out-String|Set-Content "$run\cleanup-$cleanupPhase-retained.txt";$cleanupFailure=$_}
 $releaseErrors=@()
 if($heavy){
  try{[void](RunChild heavy-release pwsh @('-NoProfile','-File',$lock,'release','p85-ext-version-v2','-Name','heavy') 15 -IndependentHeavyRelease);$heavy=$false}catch{$releaseErrors+=@($_|Out-String)}
 }
 if($worker){
  try{
   AssertControllerAuthority;if($started -and !$clean){throw 'own lifecycle not proved clean; worker retained'}
   if($script:potentialNativeHold -or $script:nativeRunning){throw 'native writer/lease outcome unconfirmed; worker retained'}
   $workerReleaseAttempted=$true
   [void](RunChild worker-release pwsh @('-NoProfile','-File',$lock,'release','p85-ext-version-v2','-Name','worker') 15)
   AssertControllerAuthority;$worker=$false
  }catch{$_|Out-String|Set-Content "$run\worker-retained.txt";$cleanupFailure=$_}
 }
 @{registered=$registered;registration_attempted=$registrationAttempted;started=$started;guarded_stop_clean=$clean;native_running_unresolved=$nativeRunning;native_hold_held_or_unconfirmed=$script:potentialNativeHold;worker_held_or_unconfirmed=$worker;worker_release_attempted=$workerReleaseAttempted;uncertain_child=[bool](Test-LiveUncertainChild);extension_profile_supported=$false;extension_CFE_built=$false}|ConvertTo-Json|Set-Content "$run\final-state.json"
 if($releaseErrors.Count){$releaseErrors|Set-Content "$run\release-errors.txt"}
 if($cleanupFailure){throw $cleanupFailure}
 if($releaseErrors.Count){throw 'one or more FIFO release outcomes need exact owned proof'}
}
