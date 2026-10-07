# Fixed owned wave3 fixture. Each case has a separate bounded N=3 cohort,
# retained phase1 refusal, exact termination, and explicit fresh-client restart.
param([Parameter(Mandatory)][string]$Binary,[Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{40}$')][string]$SourceHead,[Parameter(Mandatory)][ValidatePattern('^[0-9A-F]{64}$')][string]$BinarySha256)
$Case=9
$ErrorActionPreference='Stop'
$wt='F:/ibcmd/src/ibcmd-rs-05-load-wave3';$lab='F:/ibcmd/lab/05/wave3/load'
. "$lab/compact9-runtime-v2/compact9-tools/live/process.ps1"
. "$lab/compact9-runtime-v2/compact9-tools/live/workload_lab.ps1"
. "$lab/compact9-runtime-v2/compact9_receipts.ps1"
[void](Resolve-LoadLabRoot $lab)
$binding=Get-Content "$lab/compact9-binding.json" -Raw|ConvertFrom-Json -DateKind String;$db=$binding.database;$ib=$binding.infobase_uuid;$cluster=$binding.cluster_uuid
if($db -cne 'ibcmd_rs_05_load_w3_compact9_20261002'){throw 'isolated fixture identity'}
$rac='C:/Program Files/1cv8/8.3.27.2214/bin/rac.exe';$bin=$Binary
$prefix="compact9-load-$Case";$script:owned=@()
if((Get-FileHash $bin).Hash -cne $BinarySha256){throw 'binary provenance changed'}
if(@(Get-ChildItem -LiteralPath "$lab/logs" -Filter "$prefix*").Count -or @(Get-ChildItem -LiteralPath "$lab/obs" -Filter "$prefix*").Count){throw 'fresh case evidence required'}
function Run($name,$exe,[string[]]$argv,$seconds=30){
 if((Test-Path "$lab/logs/$name.result.json") -or (Test-Path "$lab/logs/$name.command.json")){throw 'command/result evidence exists'}
 @{executable=$exe;arguments=$argv;started_utc=[DateTime]::UtcNow.ToString('o');deadline_seconds=$seconds}|ConvertTo-Json -Depth 5|Set-Content "$lab/logs/$name.command.json"
 try{$r=Invoke-LiveBounded $exe $argv $seconds}catch{
  @{ExitCode=$null;error=$_|Out-String;ended_utc=[DateTime]::UtcNow.ToString('o')}|ConvertTo-Json|Set-Content "$lab/logs/$name.result.json"
  throw
 }
 $r|Add-Member -NotePropertyName ended_utc -NotePropertyValue ([DateTime]::UtcNow.ToString('o'))
 $r|ConvertTo-Json|Set-Content "$lab/logs/$name.result.json";return $r
}
function Inventory($name){$r=Run $name $rac @('localhost:5545','session','list',"--cluster=$cluster","--infobase=$ib");if($r.ExitCode -or $r.Stderr.Trim()){throw 'RAC inventory failed'};return $r.Stdout}
function PublishOwnership{
 $script:owned|ConvertTo-Json -Depth 6|Set-Content "$lab/snapshots/$prefix-current-owned.json"
 $o=Get-Content "$lab/OWNED.json" -Raw|ConvertFrom-Json -DateKind String;$o.processes=$script:owned;$o.sessions=@($o.sessions|Where-Object{$_.infobase_uuid -ne $ib})+@($script:owned);$o|ConvertTo-Json -Depth 8|Set-Content "$lab/OWNED.json"
}
function StartCohort($suffix){
 if((Inventory "$prefix-$suffix-empty").Trim()){throw 'fixture has remaining sessions'}
 foreach($letter in @('a','b','c')){
  $label="$prefix-$suffix-$letter"
  $r=Run "$label-start" pwsh @('-NoProfile','-File',"$lab/compact9-runtime-v2/compact9-tools/live/obs.ps1",'start','-Database',$db,'-Label',$label,'-Mode','poll','-LabRoot',$lab,'-Srvr','localhost:5541','-TimeoutSec','45') 55
  $pidFile="$lab/obs/$label.pid";if(!(Test-Path $pidFile)){throw 'startup has no owned PID file'}
  $processId=[int](Get-Content $pidFile);$spawned=Get-CimInstance Win32_Process -Filter "ProcessId=$processId"
  if($spawned -and $spawned.Name -eq '1cv8c.exe' -and $spawned.ExecutablePath -ceq 'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8c.exe' -and $spawned.CommandLine -match [regex]::Escape("$label;poll;") -and $spawned.CommandLine -match [regex]::Escape($db)){
   $script:owned+=@{label=$label;pid=$processId;creation=$spawned.CreationDate.ToUniversalTime().Ticks;executable=$spawned.ExecutablePath;parent_pid=$spawned.ParentProcessId;command=$spawned.CommandLine;session_uuid='';session_id='';started_at='';infobase_uuid=$ib;epf_sha256=(Get-FileHash "$lab/observer/IbcmdRsObserver.epf").Hash;open=''};PublishOwnership
  }else{throw 'spawned client unavailable/unbound; preserve PID evidence'}
  if($r.ExitCode){throw 'client startup failed; not workload'}
  $processId=[int](Get-Content "$lab/obs/$label.pid");$p=Get-CimInstance Win32_Process -Filter "ProcessId=$processId"
  if(!$p -or $p.Name -ne '1cv8c.exe' -or $p.ExecutablePath -cne 'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8c.exe' -or $p.CommandLine -notmatch [regex]::Escape("$label;poll;") -or $p.CommandLine -notmatch [regex]::Escape($db)){throw 'client PID binding'}
  $first=Get-Content "$lab/obs/$label.log" -Head 1;$sid=($first -split '\|')[3]
  $text=Inventory "$label-binding";$blocks=@(($text -split '(?:\r?\n){2,}')|Where-Object{$_ -match "session-id\s*:\s*$sid\s" -and $_ -match "infobase\s*:\s*$ib" -and $_ -match 'app-id\s*:\s*1CV8C'})
  if($blocks.Count -ne 1 -or $blocks[0] -notmatch 'session\s*:\s*([0-9a-f-]{36})'){throw 'session UUID binding'};$uuid=$Matches[1]
  if($blocks[0] -notmatch 'started-at\s*:\s*(\S+)'){throw 'session start missing'};$started=$Matches[1]
  $openUtc=[DateTime]::new(([long]($first -split '\|')[0])*10000,[DateTimeKind]::Utc)
  if([Math]::Abs(($openUtc-([DateTime]::Parse($started).ToUniversalTime())).TotalSeconds) -gt 90){throw 'session ID/start reuse'}
  $bound=$script:owned|Where-Object{$_.label -ceq $label};$bound.session_uuid=$uuid;$bound.session_id=$sid;$bound.started_at=$started;$bound.open=$first;PublishOwnership
  New-Item -ItemType File "$lab/obs/$label.go"|Out-Null
 }
 $deadline=[DateTime]::UtcNow.AddSeconds(30)
 do{$ready=$true;foreach($b in $script:owned){if((Get-Compact9ConfirmedReceipts "$lab/obs/$($b.label).log" $b.label) -lt 5){$ready=$false}};if(!$ready){Start-Sleep -Milliseconds 500}}while(!$ready -and [DateTime]::UtcNow -lt $deadline)
 if(!$ready){throw 'cohort did not confirm real documents/reports within deadline'}
}
function VerifyProcess($b){$p=Get-CimInstance Win32_Process -Filter "ProcessId=$($b.pid)";if($p -and ($p.CommandLine -cne $b.command -or $p.ExecutablePath -cne $b.executable -or $p.ParentProcessId -ne $b.parent_pid -or $p.CreationDate.ToUniversalTime().Ticks -ne $b.creation)){throw 'owned PID reused'};return $p}
function CloseCohort($suffix){
 if(Test-LiveUncertainChild){throw 'unresolved child; owned cohort retained, no new close/termination command'}
 foreach($b in $script:owned){if(VerifyProcess $b){$r=Run "$prefix-$suffix-$($b.label)-stop" pwsh @('-NoProfile','-File',"$lab/compact9-runtime-v2/compact9-tools/live/obs.ps1",'stop','-Label',$b.label,'-LabRoot',$lab) 15;if($r.ExitCode){throw 'owned stop failed'}}}
 foreach($b in $script:owned){if(!$b.session_uuid){throw 'owned session UUID unresolved; process closed but registration retained'};$block=$null;$ready=$false;for($n=0;$n -lt 20;$n++){$text=Inventory "$prefix-$suffix-$($b.label)-inventory-$n";$blocks=@(($text -split '(?:\r?\n){2,}')|Where-Object{$_ -match [regex]::Escape($b.session_uuid)});if(!$blocks.Count){$ready=$true;$block=$null;break};if($blocks.Count -ne 1){throw 'duplicate owned UUID'};$block=$blocks[0];if($block -notmatch "infobase\s*:\s*$ib" -or $block -notmatch "session-id\s*:\s*$($b.session_id)\s" -or $block -notmatch 'app-id\s*:\s*1CV8C' -or $block -notmatch ('started-at\s*:\s*'+[regex]::Escape($b.started_at))){throw 'owned termination binding changed'};if($block -match 'hibernate\s*:\s*yes'){$ready=$true;break};Start-Sleep -Milliseconds 500};if(!$ready){throw 'owned session not inactive within deadline'};if($block){$r=Run "$prefix-$suffix-$($b.label)-terminate" $rac @('localhost:5545','session','terminate',"--cluster=$cluster","--session=$($b.session_uuid)");if($r.ExitCode){throw 'owned UUID termination failed'}}}
 if((Inventory "$prefix-$suffix-empty-after").Trim()){throw 'unexpected session remains'}
 $script:owned=@();PublishOwnership
}
function Snapshot($suffix){$r=Run "$prefix-$suffix-snapshot" pwsh @('-NoProfile','-File',"$lab/compact9-runtime-v2/compact9_snapshot.ps1",'-Label',"$prefix-$suffix") 180;if($r.ExitCode){throw 'snapshot failed'}}
function Header($label,$sets){
 $q="SET NOCOUNT ON; SELECT state_desc,user_access_desc,recovery_model_desc,(SELECT COUNT(*) FROM [$db].dbo.ConfigSave) FROM sys.databases WHERE name=N'$db'; RESTORE HEADERONLY FROM DISK=N'$lab/$label.trn';"
 $r=Run "$prefix-header-$sets" sqlcmd @('-S','localhost','-E','-C','-b','-f','65001','-W','-h','-1','-s','|','-w','65535','-Q',$q)
 if($r.ExitCode -or $r.Stdout -notmatch 'ONLINE\|MULTI_USER\|FULL\|0'){throw 'database/header read failed'}
 $rows=@($r.Stdout -split '\r?\n'|Where-Object{$_ -like 'ibcmd-rs:live:*'});$a=Get-Content "$lab/$label.recovery.live.json" -Raw|ConvertFrom-Json
 $identity=$a.payload.identity
 if($identity.database -cne $db){throw 'header envelope database identity'}
 $uuid='^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$'
 foreach($name in @('database_guid','recovery_fork','family_guid')){if([string]$identity.$name -cnotmatch $uuid){throw 'header envelope UUID identity'}}
 if($rows.Count -ne $sets){throw 'header set count'}
 for($i=0;$i -lt $sets;$i++){
  if(!$rows[$i].StartsWith("ibcmd-rs:live:$($a.payload.recovery_token):$($i+1)|")){throw 'header token/order'}
  $fields=$rows[$i].Split('|');if($fields.Count -lt 34){throw 'header identity fields missing'}
  foreach($pair in @(@(30,'database_guid'),@(31,'recovery_fork'),@(33,'family_guid'))){
   $value=$fields[$pair[0]].Trim();$expected=[string]$identity.($pair[1])
   if($value -cnotmatch $uuid -or ![string]::Equals($value,$expected,[StringComparison]::OrdinalIgnoreCase)){throw 'header identity lineage differs from LIVE envelope'}
  }
 }
}
function InvokeContinuation($label,$suffix){
 if(Test-LiveUncertainChild){throw 'unresolved prior child; no new continuation'}
 $lock=Run "$prefix-$suffix-native-lock" pwsh @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','acquire','load9-983ad8f3eae1','-Name','native','-TimeoutMin','10') 620
 if($lock.ExitCode){throw 'native lock'}
 try{Run "$prefix-$suffix" $bin @('mssql-live-continue','--artifact',"$lab/$label.recovery.live.json",'--server','localhost','--database',$db,'--allow-non-lab','--rac',$rac,'--ras-endpoint','localhost:5545','--infobase-user','Администратор') 60}
 finally{if(Test-LiveUncertainChild){throw 'unresolved child; native FIFO retained'};$release=Invoke-LiveBounded pwsh @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','release','load9-983ad8f3eae1','-Name','native') 15;if($release.ExitCode){throw 'native release'}}
}
function InvokeInvalidContinuation($artifact,$suffix){
 if(Test-LiveUncertainChild){throw 'unresolved prior child; no new continuation'}
 $lock=Run "$prefix-$suffix-native-lock" pwsh @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','acquire','load9-983ad8f3eae1','-Name','native','-TimeoutMin','10') 620
 if($lock.ExitCode){throw 'native lock'}
 try{Run "$prefix-$suffix" $bin @('mssql-live-continue','--artifact',$artifact,'--server','127.0.0.1:1','--database',$db,'--allow-non-lab','--rac','must-not-spawn-rac','--ras-endpoint','127.0.0.1:1','--sql-pwd-env','MUST_NOT_READ_COMPACT9_INVALID_PASSWORD') 60}
 finally{if(Test-LiveUncertainChild){throw 'unresolved child; native FIFO retained'};$release=Invoke-LiveBounded pwsh @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','release','load9-983ad8f3eae1','-Name','native') 15;if($release.ExitCode){throw 'native release'}}
}
try{
 StartCohort 'old';Snapshot 'load-before'
 $r=Run "$prefix-import" pwsh @('-NoProfile','-File',"$lab/compact9-runtime-v2/compact9_native.ps1",'import','-Database',$db,'-LabRoot',$lab,'-Label',"$prefix-native-import",'-Tree',"$lab/src/f5-$Case") 760;if($r.ExitCode){throw 'native partial staging failed'}
 Snapshot 'staged'
 # Fresh owned recovery-model/chain controls; one SQL command per native hold.
 foreach($negative in @('simple','copyonly')){
  if($negative -ceq 'copyonly'){
   $q="ALTER DATABASE [$db] SET RECOVERY FULL; BACKUP DATABASE [$db] TO DISK=N'$lab/compact9-copyonly-control.bak' WITH COPY_ONLY, COMPRESSION, CHECKSUM;"
   if(Test-Path "$lab/compact9-copyonly-control.bak"){throw 'fresh copyonly control backup required'}
   $lock=Run "$prefix-copyonly-lock" pwsh @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','acquire','load9-983ad8f3eae1','-Name','native','-TimeoutMin','10') 620;if($lock.ExitCode){throw 'native lock'}
   try{$r=Run "$prefix-copyonly-backup" sqlcmd @('-S','localhost','-E','-C','-b','-Q',$q) 180;if($r.ExitCode){throw 'copyonly control failed'}}finally{if(Test-LiveUncertainChild){throw 'unresolved child; native FIFO retained'};$release=Invoke-LiveBounded pwsh @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','release','load9-983ad8f3eae1','-Name','native') 15;if($release.ExitCode){throw 'native release'}}
  }
  Snapshot "$negative-before";$candidate="$prefix-negative-$negative"
  $r=Run "$prefix-negative-$negative" pwsh @('-NoProfile','-File',"$lab/compact9-runtime-v2/compact9_activate.ps1",'-Label',$candidate,'-Binary',$bin,'-SourceHead',$SourceHead,'-BinarySha256',$BinarySha256) 760
  $expected=if($negative -ceq 'simple'){'57231'}else{'57235'}
  if($r.ExitCode -eq 0 -or ($r.Stdout+$r.Stderr) -notmatch $expected -or (Test-Path "$lab/$candidate.trn")){throw 'setup negative did not refuse exactly before cycle1'}
  Snapshot "$negative-after";Compare-Compact9Storage $lab $prefix "$negative-before" "$negative-after"
 }
 if(Test-Path "$lab/compact9-ordinary-full-control.bak"){throw 'fresh ordinary backup required'}
 $lock=Run "$prefix-full-lock" pwsh @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','acquire','load9-983ad8f3eae1','-Name','native','-TimeoutMin','10') 620;if($lock.ExitCode){throw 'native lock'}
 try{$r=Run "$prefix-full-backup" sqlcmd @('-S','localhost','-E','-C','-b','-Q',"BACKUP DATABASE [$db] TO DISK=N'$lab/compact9-ordinary-full-control.bak' WITH COMPRESSION,CHECKSUM;") 180;if($r.ExitCode){throw 'ordinary full chain failed'}}finally{if(Test-LiveUncertainChild){throw 'unresolved child; native FIFO retained'};$release=Invoke-LiveBounded pwsh @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','release','load9-983ad8f3eae1','-Name','native') 15;if($release.ExitCode){throw 'native release'}}
 $r=Run "$prefix-full-header" sqlcmd @('-S','localhost','-E','-C','-b','-Q',"RESTORE HEADERONLY FROM DISK=N'$lab/compact9-ordinary-full-control.bak';") 30;if($r.ExitCode){throw 'ordinary full header proof failed'}
 @{copyonly_sha256=(Get-FileHash "$lab/compact9-copyonly-control.bak").Hash;ordinary_full_sha256=(Get-FileHash "$lab/compact9-ordinary-full-control.bak").Hash}|ConvertTo-Json|Set-Content "$lab/logs/$prefix-backup-hashes.json"
 $completed=$false;$label=$null
 for($attempt=1;$attempt -le 1;$attempt++){
  $candidate="$prefix-r$attempt";$r=Run "$prefix-activate-$attempt" pwsh @('-NoProfile','-File',"$lab/compact9-runtime-v2/compact9_activate.ps1",'-Label',$candidate,'-Binary',$bin,'-SourceHead',$SourceHead,'-BinarySha256',$BinarySha256) 760
  if($r.ExitCode -eq 0){$label=$candidate;$completed=$true;break}
  if(($r.Stdout+$r.Stderr) -notmatch '57238|57239' -or (Test-Path "$lab/$candidate.trn")){throw 'ambiguous activation: inspect artifact, no retry'}
  Snapshot "refused-$attempt"
  $before=@(Get-Content "$lab/snapshots/$prefix-staged-storage.txt"|Where-Object{$_ -match '^Config(?:Save)?\|'})
  $after=@(Get-Content "$lab/snapshots/$prefix-refused-$attempt-storage.txt"|Where-Object{$_ -match '^Config(?:Save)?\|'})
  if(($before -join "`n") -cne ($after -join "`n")){throw 'refusal changed Config/stage'}
 }
 if(!$completed){throw 'all bounded loaded activation attempts refused'}
 $result=Get-Content "$lab/logs/$label-activate.result.json" -Raw|ConvertFrom-Json -DateKind String;$report=$result.Stdout|ConvertFrom-Json
 if($report.live_continuation.state -cne 'continuation_required'){throw 'expected retained phase1 with nonempty cohort'}
 # Capture owned client state before any potentially blocked RAS inventory.
 foreach($b in $script:owned){if(VerifyProcess $b){$r=Run "$prefix-$($b.label)-windows" powershell @('-NoProfile','-File',"$wt/scripts/apply-trace/lab/online-live/winshot.ps1",'-ProcessId',"$($b.pid)",'-OutDir',"$lab/windows/$prefix-$($b.label)") 20;if($r.ExitCode){throw 'own window capture failed'}}}
 Header $label 1;Snapshot 'pending';
 $readback=Run "$prefix-compact-readback" python @("$lab/compact9-runtime-v2/read_compact9_recovery.py",'--manifest',"$lab/$label.recovery.json",'--live',"$lab/$label.recovery.live.json",'--storage',"$lab/snapshots/$prefix-staged-storage.txt",'--output',"$lab/logs/$prefix-compact-readback.json") 30;if($readback.ExitCode){throw 'compact recovery readback failed; retained phase1, no repeat'}
 $before=(Get-FileHash "$lab/$label.trn").Hash
 Snapshot 'warm-before';$r=InvokeContinuation $label 'warm-refusal';if($r.ExitCode -eq 0 -or $r.Stderr -notmatch 'RAS lists sessions'){throw 'expected warm refusal'}
 $after=(Get-FileHash "$lab/$label.trn").Hash;@{before=$before;after=$after;equal=($before -ceq $after)}|ConvertTo-Json|Set-Content "$lab/logs/$prefix-refusal-tail.json";if($before -cne $after){throw 'refusal changed tail'};Snapshot 'warm-after';Compare-Compact9Storage $lab $prefix 'warm-before' 'warm-after'
 $activity=@();for($n=0;$n -lt 3;$n++){$text=Inventory "$prefix-postreturn-$n";$activity+=@{utc=[DateTime]::UtcNow.ToString('o');inventory=$text;journal_end=@($script:owned|ForEach-Object{@{label=$_.label;tail=@(Get-Content "$lab/obs/$($_.label).log" -Tail 2)}})};if($n -lt 2){Start-Sleep -Seconds 5}}
 $activity|ConvertTo-Json -Depth 7|Set-Content "$lab/logs/$prefix-activity.json"
 $script:owned|ConvertTo-Json -Depth 6|Set-Content "$lab/snapshots/$prefix-old-owned.json";CloseCohort 'old-close'
 $r=InvokeContinuation $label 'continue';if($r.ExitCode){throw 'continuation failed'};$report=$r.Stdout|ConvertFrom-Json -DateKind String;if($report.state -cne 'complete' -or $report.cycle_2_executed -ne $true){throw 'cycle2 not established'}
 Header $label 2;Snapshot 'noop-before';$before=(Get-FileHash "$lab/$label.trn").Hash;$r=InvokeContinuation $label 'noop';if($r.ExitCode){throw 'repeat failed'};$report=$r.Stdout|ConvertFrom-Json -DateKind String;$after=(Get-FileHash "$lab/$label.trn").Hash
 @{before=$before;after=$after;equal=($before -ceq $after)}|ConvertTo-Json|Set-Content "$lab/logs/$prefix-noop-tail.json"
 Snapshot 'noop-after';Compare-Compact9Storage $lab $prefix 'noop-before' 'noop-after'
 if($report.state -cne 'already_complete' -or $report.cycle_2_executed -ne $false -or $before -cne $after){throw 'repeat changed tail/state'}
 $planPath="$lab/logs/$prefix-invalid-actual-plan.json"
 $r=Run "$prefix-invalid-copies" python @("$lab/compact9-runtime-v2/make_compact9_negatives.py",'--live',"$lab/$label.recovery.live.json",'--manifest',"$lab/$label.recovery.json",'--storage',"$lab/snapshots/$prefix-staged-storage.txt",'--output',$planPath) 30;if($r.ExitCode){throw 'actual package negative copies failed'}
 $invalidPlan=Get-Content -LiteralPath $planPath -Raw|ConvertFrom-Json -DateKind String
 $expected=@{'corrupt-pack'='recovery pack length/digest mismatch';'missing-pack'='ibcmd-recovery-.*\.pack';'wrong-envelope-digest'='compact LIVE envelope integrity mismatch';'wrong-sidecar-digest'='compact LIVE recovery sidecar digest differs';'foreign-sidecar-name'='compact LIVE recovery basename differs'}
 if(($invalidPlan.cases.name|Sort-Object) -join ',' -cne (($expected.Keys|Sort-Object) -join ',')){throw 'exact negative inventory required'}
 foreach($case in $invalidPlan.cases){
  $path=[IO.Path]::GetFullPath($case.artifact)
  if(![IO.Path]::IsPathFullyQualified($case.artifact) -or !$path.StartsWith([IO.Path]::GetFullPath("$lab/compact9-invalid-actual/").TrimEnd('\')+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -or (Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant() -cne $case.sha256){throw 'negative artifact path/hash changed'}
  $r=InvokeInvalidContinuation $path "invalid-$($case.name)"
  if($r.ExitCode -eq 0 -or $r.Stderr -notmatch $expected[$case.name] -or $r.Stderr -match 'TCP|connect.*failed|authentication|login failed'){throw 'input refusal before connection not established'}
 }
 foreach($entry in $invalidPlan.originals_sha256.PSObject.Properties){if((Get-FileHash -LiteralPath $entry.Name).Hash.ToLowerInvariant() -cne $entry.Value){throw 'original actual recovery package changed'}}
 StartCohort 'new';foreach($b in $script:owned){$ops=@(Get-Content "$lab/obs/$($b.label).log"|Where-Object{$_ -match '\|operation\|'});if(!$ops.Count -or @($ops|Where-Object{$_ -notmatch "\|LIVE-f5-$Case\|LIVE-f5-$Case\|"}).Count){throw 'fresh-client marker mismatch'}}
 $script:owned|ConvertTo-Json -Depth 6|Set-Content "$lab/snapshots/$prefix-new-owned.json";CloseCohort 'new-close';Snapshot 'complete'
 @{case=$Case;activation=$label;phase1_retained=$true;cycle2_complete_after_owned_close=$true;repeat_tail_equal=$true;fresh_clients=3;old_clients_may_stall=$true;readiness_changed=$false}|ConvertTo-Json|Set-Content "$lab/logs/$prefix-checkpoint.json"
 "PASS $prefix bounded loaded phase1/refusal/explicit restart; no warm admission claim"
}catch{
 $primaryFailure=$_
 $primaryFailure|Out-String|Set-Content "$lab/logs/$prefix-error.txt"
 # Preserve pending state; cleanup is bound exclusively to this explicit cohort.
 if($script:owned.Count){try{CloseCohort 'failure-close'}catch{$_|Out-String|Set-Content "$lab/logs/$prefix-cleanup-error.txt"}}
 throw $primaryFailure
}







