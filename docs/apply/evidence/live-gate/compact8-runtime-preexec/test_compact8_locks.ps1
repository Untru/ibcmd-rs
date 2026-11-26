$ErrorActionPreference='Stop'
function Assert-Compact8RuntimeClosure {}
$lab='F:/ibcmd/lab/05/wave3/load';$prefix='pure-lock';$taskReceipts='mock';$script:uncertain=$false
function Test-LiveUncertainChild {return $script:uncertain}
function Invoke-LiveBounded {param($Executable,$Arguments,$TimeoutSeconds);$script:rawCalls+=@($Arguments[3]+'-'+$Arguments[-1]);if($Arguments[3] -ceq 'acquire'){$script:current=Owner 'same_track' 'new';if($script:mode -ceq 'unknown'){$script:uncertain=$true;throw 'unknown acquire completion'};return [pscustomobject]@{ExitCode=$(if($script:mode -ceq 'nonzero'){1}else{0});Stdout='';Stderr=''}};$script:current=Owner 'absent' '';return [pscustomobject]@{ExitCode=0;Stdout='';Stderr=''}}
. "$lab/compact8-runtime-v1/compact8_locks.ps1"
$script:ActualLockReader=${function:Read-Compact8LockOwner}
function Owner($Kind,$Token){[pscustomobject]@{kind=$Kind;bytes=[Text.Encoding]::ASCII.GetBytes("track=load8-f8bd7d065a2c since=$Token")}}
function Read-Compact8LockOwner($Name){$script:current}
function Save-Compact8LockProof($State){$State.sequence++;$script:proofs++;$script:proofCopies+=@(($State|ConvertTo-Json -Depth 4 -Compress))}
function Test-Compact8ResourceUnconfirmed {if($script:Compact8ResourceUnconfirmed){return $true};$decoded=@($script:proofCopies|ForEach-Object {$_|ConvertFrom-Json});return Get-Compact8ResourceUnconfirmedFromProofs $decoded}
function Test-Path {param($LiteralPath);return $false}
function Set-Content {param($LiteralPath,$Encoding,[Parameter(ValueFromPipeline)]$Value);process{try{$script:writes[$LiteralPath]=$Value|ConvertFrom-Json}catch{}}}
$ast=[Management.Automation.Language.Parser]::ParseFile("$lab/compact8-runtime-v1/compact8_lifetime_v2.ps1",[ref]$null,[ref]$null)
foreach($name in @('Test-Compact8UnresolvedWriter','Assert-Compact8CleanupAuthority')){$fn=$ast.Find({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq $name},$true);. ([scriptblock]::Create($fn.Extent.Text))}
$outer=@($ast.EndBlock.Statements|Where-Object {$_ -is [Management.Automation.Language.TryStatementAst]})[0]
$finallyText=$outer.Finally.Extent.Text;$finally=[scriptblock]::Create($finallyText.Substring(1,$finallyText.Length-2))
function Run($Name,$Executable,$Arguments,$Seconds){$r=Invoke-LiveBounded $Executable $Arguments $Seconds;if($r.ExitCode){throw 'completed nonzero acquire'};$r}
function Reset($Mode){$script:mode=$Mode;$script:uncertain=$false;$script:rawCalls=@();$script:proofs=0;$script:proofCopies=@();$script:Compact8ResourceUnconfirmed=$false;$script:Compact8Leases=@{};$script:current=Owner 'absent' '';$script:writes=@{};$script:heldWorker=$false;$script:heldHeavy=$false;$script:started=$false;$script:registered=$false;$script:registrationAttempted=$false;$script:ib=''}
function ActualWorkerAcquire {
 $stmts=$outer.Body.Statements;$i=@(0..($stmts.Count-1)|Where-Object {$stmts[$_].Extent.Text -match 'Run "\$prefix-worker-acquire"'})[0]
 return [scriptblock]::Create($stmts[$i-1].Extent.Text+"`n"+$stmts[$i].Extent.Text)
}
Reset normal;. (ActualWorkerAcquire)|Out-Null;. $finally
if($heldWorker -or ($rawCalls -join ',') -cne 'acquire-20,release-worker' -or !$Compact8Leases.worker.released){throw 'Actual clean acquire/finally changed'}
'PASS actual worker acquire/finally known success captures bytes and releases exact lease'
foreach($mode in @('preexisting','preexisting-LOAD8-F8BD7D065A2C','preexisting-Load8-f8bd7d065a2c','nonzero','unknown','replacement','foreign-replacement')){
 Reset $mode
 if($mode.StartsWith('preexisting')){
  $matches=@{1=$(if($mode.Contains('-')){$mode.Substring('preexisting-'.Length)}else{'load8-f8bd7d065a2c'})}
  $la=[Management.Automation.Language.Parser]::ParseFile("$lab/compact8-runtime-v1/compact8_locks.ps1",[ref]$null,[ref]$null)
  $classification=$la.Find({param($n)$n -is [Management.Automation.Language.IfStatementAst] -and $n.Extent.Text.Contains("`$Matches[1] -ieq 'load8-f8bd7d065a2c'")},$true)
  if(!$classification){throw 'Actual shared-helper-compatible classification missing'}
  $kind=& ([scriptblock]::Create($classification.Extent.Text))
  $script:current=Owner $kind 'foreign-preexisting'
 }
 try{. (ActualWorkerAcquire)|Out-Null}catch{}
 if($mode -ceq 'replacement'){$script:current=Owner 'same_track' 'replacement'}
 if($mode -ceq 'foreign-replacement'){$script:current=Owner 'foreign' 'replacement'}
 try{. $finally}catch{}
 if(!$heldWorker -or @($rawCalls|Where-Object {$_ -like 'release-*'}).Count){throw "Unsafe release $mode"}
 if($mode.StartsWith('preexisting') -and $rawCalls.Count){throw 'Preexisting SAME Trackload dispatched'}
 if($mode -ceq 'nonzero' -and (!$Compact8Leases.worker.acquire_completed -or $Compact8Leases.worker.lease)){throw 'Nonzero owner was inferred to belong to this attempt'}
 "PASS actual acquire/finally $mode retains potential hold; release0"
}
foreach($name in @('heavy','native')){
 Reset normal
 $args=@('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','acquire','load8-f8bd7d065a2c','-Name',$name,'-TimeoutMin','10')
 [void](Invoke-LiveBounded 'pwsh' $args 620)
 $script:current=Owner 'same_track' 'byte-drift';$caught=$false
 try{[void](Invoke-LiveBounded 'pwsh' @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','release','load8-f8bd7d065a2c','-Name',$name) 15)}catch{$caught=$true}
 if(!$caught -or @($rawCalls|Where-Object {$_ -like 'release-*'}).Count){throw "Byte drift $name released"}
 "PASS exact $name byte drift no release"
}
# Actual finally consumes serialized child resource proofs even when all children exited.
function Get-Content {param($LiteralPath,[switch]$Raw);return '{"processes":[]}'}
function Get-CimInstance {return $null}
$script:oldRun=${function:Run}
foreach($case in @('native-nonzero','native-release-nonzero','native-lease-drift','heavy-nonzero','heavy-release-nonzero')){
 Reset normal;. (ActualWorkerAcquire)|Out-Null
 $script:current=Owner 'absent' '';$script:rawCalls=@();$script:registered=$true;$script:started=$true;$ib='exact-own';$cluster='exact-cluster';$name=if($case.StartsWith('native')){'native'}else{'heavy'}
 if($name -ceq 'heavy'){$script:heldHeavy=$true}
 $script:mode=if($case.EndsWith('-nonzero') -and $case -notmatch 'release'){'nonzero'}else{'normal'}
 [void](Invoke-LiveBounded 'pwsh' @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','acquire','load8-f8bd7d065a2c','-Name',$name,'-TimeoutMin','10') 620)
 if($case -match 'release-nonzero'){
  $script:Compact8RawBounded={param($Executable,$Arguments,$TimeoutSeconds);$script:rawCalls+=@('release-'+$Arguments[-1]);[pscustomobject]@{ExitCode=1;Stdout='';Stderr='failed exact release'}}
  try{[void](Invoke-LiveBounded 'pwsh' @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','release','load8-f8bd7d065a2c','-Name',$name) 15)}catch{}
 }elseif($case -ceq 'native-lease-drift'){
  $script:current=Owner 'same_track' 'replacement'
  try{[void](Invoke-LiveBounded 'pwsh' @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','release','load8-f8bd7d065a2c','-Name',$name) 15)}catch{}
 }
 if($script:uncertain -or !(Test-Compact8ResourceUnconfirmed)){throw 'Dead child resource proof not visible to parent'}
 $script:rawCalls=@();$script:cleanupCalls=@()
 function Run($Name,$Executable,$Arguments,$Seconds){$script:cleanupCalls+=@($Name);throw 'cleanup dispatched despite unconfirmed resource'}
 try{. $finally}catch{}
 if(!$heldWorker -or $script:cleanupCalls.Count -or @($rawCalls|Where-Object {$_ -ceq 'release-worker' -or $_ -ceq 'release-native'}).Count){throw "Unsafe dead child cleanup $case"}
 "PASS actual finally $case serialized child proof retains worker; unregister/stop/native release0 with liveChild=false"
 $script:Compact8RawBounded={param($Executable,$Arguments,$TimeoutSeconds);$script:rawCalls+=@($Arguments[3]+'-'+$Arguments[-1]);if($Arguments[3] -ceq 'acquire'){$script:current=Owner 'same_track' 'new';return [pscustomobject]@{ExitCode=$(if($script:mode -ceq 'nonzero'){1}else{0});Stdout='';Stderr=''}};$script:current=Owner 'absent' '';[pscustomobject]@{ExitCode=0;Stdout='';Stderr=''}}
 ${function:Run}=$script:oldRun
}
Reset normal
[void](Invoke-LiveBounded 'pwsh' @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','acquire','load8-f8bd7d065a2c','-Name','native','-TimeoutMin','10') 620)
if(!(Test-Compact8ResourceUnconfirmed)){throw 'Outstanding native lease admitted cleanup'}
[void](Invoke-LiveBounded 'pwsh' @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','release','load8-f8bd7d065a2c','-Name','native') 15)
if(Test-Compact8ResourceUnconfirmed){throw 'Successful latest native release not recognized'}
$decoded=@($proofCopies|ForEach-Object {$_|ConvertFrom-Json});$caught=$false
try{Get-Compact8ResourceUnconfirmedFromProofs @($decoded[0],$decoded[0])|Out-Null}catch{$caught=$true}
if(!$caught){throw 'Duplicate sequence admitted'}
'PASS cross-child native acquired/released latest sequence and duplicate refusal'
Reset normal
$nativeAst=[Management.Automation.Language.Parser]::ParseFile("$lab/compact8-runtime-v1/compact8_native.ps1",[ref]$null,[ref]$null)
$nativeLock=@($nativeAst.EndBlock.Statements|Where-Object {$_ -is [Management.Automation.Language.AssignmentStatementAst] -and $_.Left.Extent.Text -cin @('$lockTool','$lock')})
if($nativeLock.Count -ne 2){throw 'Exact native acquisition callsite missing'}
foreach($statement in $nativeLock){. ([scriptblock]::Create($statement.Extent.Text))}
if($rawCalls.Count -ne 1 -or !$Compact8Leases.native.lease -or $proofCopies.Count -ne 1 -or !(Test-Compact8ResourceUnconfirmed)){throw 'Actual native callsite bypassed wrapper/proof'}
'PASS actual native lockTool/acquire callsite uses wrapper and serialized lease proof'
$nativeRelease=$nativeAst.Find({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq 'Complete-Compact8NativeHold'},$true)
. ([scriptblock]::Create($nativeRelease.Extent.Text));$script:NativeWriterCompletionProved=$true;Complete-Compact8NativeHold
if(!$Compact8Leases.native.released){throw 'Actual native release bypassed wrapper'}
Reset normal;$lockTool='F:\ibcmd\lab\04\tools\heavy-lock.ps1'
. ([scriptblock]::Create($nativeLock[1].Extent.Text));$script:NativeWriterCompletionProved=$true;Complete-Compact8NativeHold
if(!$Compact8Leases.native.released -or $proofCopies.Count -ne 2){throw 'Known backslash native acquire/release bypassed wrapper'}
'PASS actual native acquire/release both exact known path spellings retain lease/proof protocol'
# Production Run propagates a failed child proof publication marker independently of liveness.
$runFn=$ast.Find({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq 'Run'},$true);. ([scriptblock]::Create($runFn.Extent.Text))
function Invoke-LiveBounded {param($Executable,$Arguments,$TimeoutSeconds);[pscustomobject]@{ExitCode=1;Stdout='';Stderr='COMPACT8_RESOURCE_UNCONFIRMED proof publication failed'}}
$script:Compact8ResourceUnconfirmed=$false;try{Run 'pure-proof-publication' 'pwsh' @('unused') 1|Out-Null}catch{}
if(!$script:Compact8ResourceUnconfirmed -or $script:uncertain){throw 'Resource marker lost after exited child'}
'PASS actual Run sticky cross-child resource marker with known nonzero and dead child'
${function:Read-Compact8LockOwner}=$script:ActualLockReader
function Test-Path {param($LiteralPath);return $true}
function Get-Item {param($LiteralPath,[switch]$Force);[pscustomobject]@{FullName=$LiteralPath;Attributes=[IO.FileAttributes]::Normal;Length=100}}
function Get-ChildItem {param($LiteralPath,[switch]$Force);$one=[IO.FileInfo]::new($LiteralPath+'\owner.txt');$one|Add-Member NoteProperty Attributes ([IO.FileAttributes]::Normal) -Force;$one;[IO.DirectoryInfo]::new($LiteralPath+'\foreign-extra')}
foreach($name in @('worker','heavy','native')){
 $caught=$false;try{Read-Compact8LockOwner $name|Out-Null}catch{$caught=$_.Exception.Message -like 'exact single ordinary owner.txt*'}
 if(!$caught){throw 'Actual lock reader admitted extra directory'}
 "PASS actual $name reader extra member refused before owner read/release"
}
'ALL PASS actual new7 resource calls/finally; realStarts=realSignals=SQLcalls=registryWrites=acquires=releases=0'
