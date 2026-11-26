function Test-Compact9ResourceUnconfirmed{return $false}
$ErrorActionPreference='Stop'
function Assert-Compact9RuntimeClosure {}
$lab='F:/ibcmd/lab/05/wave3/load';$prefix='pure';$bin='never-spawn';$db='mock';$rac='never-rac';$native='C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe';$argv=@('mock');$Label='pure';$TimeoutSec=1
function Extract([string]$Path,[string]$Name){$tokens=$null;$errors=$null;$ast=[Management.Automation.Language.Parser]::ParseFile($Path,[ref]$tokens,[ref]$errors);if($errors){throw 'parse'};$def=@($ast.FindAll({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq $Name},$true));if($def.Count -ne 1){throw 'function missing/ambiguous'};return $def[0].Extent.Text}
Invoke-Expression (Extract "$lab/compact9-runtime-v1/compact9_checkpoint_v2.ps1" 'InvokeContinuation')
Invoke-Expression (Extract "$lab/compact9-runtime-v1/compact9_native.ps1" 'Invoke-Compact9Native')
Invoke-Expression (Extract "$lab/compact9-runtime-v1/compact9_native.ps1" 'Complete-Compact9NativeHold')
function Test-LiveUncertainChild{return [bool]$script:uncertain}
function Save-LiveUncertainChild($State){$script:savedUncertain=$State}
$script:uncertain=$false
$script:events=[Collections.Generic.List[string]]::new();$script:held=$false;$script:throwChild=$false
function Run($name,$exe,$argv,$seconds){
 if($argv -contains 'acquire'){$script:held=$true;$script:events.Add('acquire');return [pscustomobject]@{ExitCode=0}}
 if(!$script:held -or $argv[0] -cne 'mssql-live-continue'){throw 'continuation outside native FIFO'};$script:events.Add('child');if($script:throwChild){throw 'mock child failure'};return [pscustomobject]@{ExitCode=0;Stdout='unexpected warm completion'}
}
function Invoke-LiveBounded($Executable,$Arguments,$TimeoutSeconds){if(!$script:held -or $Arguments -notcontains 'release' -or $TimeoutSeconds -ne 15){throw 'unbounded/unowned release'};$script:held=$false;$script:events.Add('release');return [pscustomobject]@{ExitCode=0}}
foreach($suffix in @('warm','continue','noop','failed')){
 $script:events.Clear();$script:throwChild=$suffix -eq 'failed';$caught=$false
 try{[void](InvokeContinuation 'mock' $suffix)}catch{if($_.Exception.Message -cne 'mock child failure'){throw};$caught=$true}
 if(($script:events -join ',') -cne 'acquire,child,release' -or $script:held -or $caught -ne $script:throwChild){throw 'FIFO/finally sequence'}
 "PASS continuation $suffix acquire/child/release, unexpected warm exit0 remains protected"
}
function Set-Content{param($LiteralPath,$Encoding,[Parameter(ValueFromPipeline)]$Value);process{try{$script:lastTimes=$Value|ConvertFrom-Json -DateKind String}catch{$script:lastTimes=$Value};$script:writes[$LiteralPath]=$script:lastTimes}}
$script:writes=@{}
$script:mode='timeout';$script:fault='';$script:cimCalls=0;$script:kills=0;$script:starts=0;$script:disposed=0;$script:waits=[Collections.Generic.List[int]]::new()
$birth=[DateTime]'2026-10-01T00:00:00Z'
function Get-CimInstance{param($ClassName,$Filter);$script:cimCalls++;if($script:fault -ceq 'initial-missing'){return $null};$entry=[pscustomobject]@{ProcessId=777777;CreationDate=$birth;ExecutablePath=$native;CommandLine='exact mock';ParentProcessId=$PID};if($script:cimCalls -gt 1){switch($script:fault){'birthday'{$entry.CreationDate=$birth.AddSeconds(1)}'exe'{$entry.ExecutablePath='C:\foreign.exe'}'cmd'{$entry.CommandLine='foreign'}'parent'{$entry.ParentProcessId=1}'missing'{return $null}}};return $entry}
function New-Compact9NativeProcess($StartInfo){
 $obj=[pscustomobject]@{Id=777777;StartTime=$birth;ExitTime=$birth.AddSeconds(1);HasExited=($script:mode -in @('rapid','stdin-close-rapid'));ExitCode=0;StandardOutput=[pscustomobject]@{};StandardError=[pscustomobject]@{};StandardInput=[pscustomobject]@{}}
 $obj.StandardInput|Add-Member ScriptMethod Close {if($script:mode.StartsWith('stdin-close')){throw 'mock native stdin close fault'}}
 $obj.StandardOutput|Add-Member ScriptMethod ReadToEndAsync {if($script:mode -ceq 'pipe-timeout'){$pipe=[pscustomobject]@{};$pipe|Add-Member ScriptMethod Wait {param($Milliseconds);if($Milliseconds -ne 5000){throw 'unbounded native pipe'};return $false};return $pipe};[Threading.Tasks.Task]::FromResult([string]'mock stdout')}
 $obj.StandardError|Add-Member ScriptMethod ReadToEndAsync {[Threading.Tasks.Task]::FromResult([string]'')}
 $obj|Add-Member ScriptMethod Start {$script:starts++;return $true}
 $obj|Add-Member ScriptMethod WaitForExit {param([int]$Milliseconds);$script:waits.Add($Milliseconds);if($script:mode -in @('normal','rapid','pipe-timeout','stdin-close-rapid')){$this.HasExited=$true;return $true};if($script:waits.Count -eq 1){return $false};return $script:mode -ne 'reap-fails'}
 $obj|Add-Member ScriptMethod Kill {if($args.Count){throw 'unchecked tree kill'};$script:kills++;$this.HasExited=($script:mode -ne 'reap-fails')}
 $obj|Add-Member ScriptMethod Dispose {$script:disposed++}
 return $obj
}
function ResetMock($Mode,$Fault=''){$script:mode=$Mode;$script:fault=$Fault;$script:cimCalls=0;$script:kills=0;$script:starts=0;$script:disposed=0;$script:waits.Clear();$script:lastTimes=$null;$script:uncertain=$false}
ResetMock normal;$result=Invoke-Compact9Native
if($result.ExitCode -ne 0 -or $result.Stdout -cne 'mock stdout' -or $script:kills -or ($script:waits -join ',') -cne '1000' -or $script:disposed -ne 1){throw 'ordinary execution mock'}
'PASS ordinary native child + bounded pipe reads'
ResetMock timeout;$caught=$false;try{[void](Invoke-Compact9Native)}catch{$caught=$_.Exception.Message -match 'exact spawned native child deadline'}
if(!$caught -or $script:kills -ne 1 -or ($script:waits -join ',') -cne '1000,5000' -or !$script:lastTimes.reap_completed -or !$script:lastTimes.spawn_birth -or !$script:lastTimes.timeout -or $script:disposed -ne 1){throw 'timeout identity/reap proof'}
'PASS exact native timeout single-PID Kill(no args), reap5000 and retained times'
foreach($fault in @('birthday','exe','cmd','parent','missing')){
 ResetMock timeout $fault;$caught=$false;try{[void](Invoke-Compact9Native)}catch{$caught=$_.Exception.Message -match 'identity changed; no signal'}
 if(!$caught -or $script:kills -or ($script:waits -join ',') -cne '1000' -or $script:disposed -ne 1){throw "unsafe timeout$fault"};"PASS timeout$fault refusal, signals0"
}
ResetMock reap-fails;$caught=$false;try{[void](Invoke-Compact9Native)}catch{$caught=$_.Exception.Message -match 'reap exceeded5000ms'}
if(!$caught -or $script:kills -ne 1 -or ($script:waits -join ',') -cne '1000,5000' -or $script:lastTimes.reap_completed -ne $false){throw 'unbounded reap'}
'PASS bounded reap failure retains exact identity, no tree signal'

# Exercise the actual release function, not a reconstructed expectation.
foreach($case in @('stdin-close-live','stdin-close-rapid')){
 ResetMock $case;try{[void](Invoke-Compact9Native)}catch{}
 if($script:NativeWriterCompletionProved -or $script:lastTimes.pid -ne 777777 -or $script:kills){throw "stdin native fault lost original child$case"}
 $script:events.Clear();$script:held=$true;$caught=$false
 try{Complete-Compact9NativeHold}catch{$caught=$_.Exception.Message -match 'FIFO retained'}
 if(!$caught -or $script:events.Count -or !$script:held){throw "stdin native fault released hold$case"}
 "PASS actual native $case retains original PID/unknown pipes and FIFO; signals/release0"
}
foreach($case in @('initial-missing','birthday','reap-fails','timeout-complete','pipe-timeout')){
 if($case -ceq 'initial-missing'){ResetMock timeout 'initial-missing'}elseif($case -ceq 'reap-fails'){ResetMock reap-fails}elseif($case -ceq 'pipe-timeout'){ResetMock pipe-timeout}elseif($case -ceq 'timeout-complete'){ResetMock timeout}else{ResetMock timeout $case}
 try{[void](Invoke-Compact9Native)}catch{}
 if($script:NativeWriterCompletionProved){throw "unproved writer$case marked completed"}
 $script:events.Clear();$script:held=$true;$caught=$false
 try{Complete-Compact9NativeHold}catch{$caught=$_.Exception.Message -match 'FIFO retained'}
 if(!$caught -or $script:events.Count -or !$script:held -or !$script:lastTimes.native_hold_retained){throw "unproved native$case released FIFO"}
 "PASS native$case retains FIFO, releaseCalls0"
}
ResetMock rapid 'initial-missing';$r=Invoke-Compact9Native
if($r.ExitCode -ne 0 -or !$script:NativeChildExitProved -or $script:kills){throw 'rapid native result not preserved'}
$script:held=$true;$script:events.Clear();Complete-Compact9NativeHold
if($script:held -or ($script:events -join ',') -cne 'release'){throw 'proven rapid native exit did not release'}
'PASS rapid native exit without CIM releases safely, signals0'
ResetMock normal;[void](Invoke-Compact9Native);$script:uncertain=$true;$script:held=$true;$script:events.Clear();$caught=$false
try{Complete-Compact9NativeHold}catch{$caught=$_.Exception.Message -match 'FIFO retained'}
if(!$caught -or $script:events.Count -or !$script:held){throw 'unresolved descendant receipt released FIFO'}
'PASS unresolved descendant receipt retains FIFO despite direct native exit'
# Execute the shipped lifetime finally block under mocks. RAS/SQL inventories
# being empty cannot override the unresolved-writer receipt or native preimage.
Invoke-Expression (Extract "$lab/compact9-runtime-v1/compact9_lifetime_v2.ps1" 'Test-Compact9UnresolvedWriter')
Invoke-Expression (Extract "$lab/compact9-runtime-v1/compact9_lifetime_v2.ps1" 'Assert-Compact9CleanupAuthority')
function Test-Path{param($LiteralPath);return $script:fakeNativeProofPresent}
function Get-Item{param($LiteralPath);[pscustomobject]@{Length=100}}
function Get-Content{param($LiteralPath,[switch]$Raw);return '{"direct_child_exit_proved":false,"writer_completion_proved":false}'}
$tokens=$null;$errors=$null;$tree=[Management.Automation.Language.Parser]::ParseFile("$lab/compact9-runtime-v1/compact9_lifetime_v2.ps1",[ref]$tokens,[ref]$errors)
$main=@($tree.FindAll({param($n)$n -is [Management.Automation.Language.TryStatementAst] -and $n.Body.Extent.Text.Contains('$existing=Invoke-LiveBounded sqlcmd')},$true));if($main.Count -ne 1){throw 'main lifecycle body'}
$body=$main[0].Finally.Extent.Text;$cleanup=[Scriptblock]::Create($body.Substring(1,$body.Length-2))

Invoke-Expression (Extract "$lab/compact9-runtime-v1/compact9_lifetime_v2.ps1" 'Native')
function Get-Content{param($LiteralPath,[switch]$Raw);if($LiteralPath.EndsWith('/OWNED.json')){return '{"processes":[]}'};return '{"direct_child_exit_proved":false,"writer_completion_proved":false}'}
function Run($name,$exe,$argv,$seconds){
 $script:cleanupCalls.Add($name)
 if($name.EndsWith('-native-acquire')){return [pscustomobject]@{ExitCode=0;Stdout='';Stderr=''}}
 if(($script:cleanupFault -ceq 'ras' -and $name.EndsWith('-cleanup-empty')) -or ($script:cleanupFault -ceq 'sql' -and $name.EndsWith('-cleanup-sql-work')) -or ($script:cleanupFault -ceq 'unregister' -and $name.EndsWith('-unregister')) -or ($script:cleanupFault -ceq 'stop' -and $name.EndsWith('-stop'))){$script:uncertain=$true;throw 'mock newly uncertain cleanup child'}
 return [pscustomobject]@{ExitCode=0;Stdout='';Stderr=''}
}
function Invoke-LiveBounded($Executable,$Arguments,$TimeoutSeconds){
 $script:cleanupCalls.Add('release-'+$Arguments[-1])
 if($script:cleanupFault -ceq 'heavy' -and $Arguments[-1] -ceq 'heavy'){$script:uncertain=$true;throw 'mock newly uncertain heavy release'}
 return [pscustomobject]@{ExitCode=0;Stdout='';Stderr=''}
}
foreach($reason in @('receipt','native-exit-unproved','ras','sql','unregister','stop','heavy')){
 $prefix='pure-'+$reason;$ib='owned-ib';$cluster='owned-cluster';$script:uncertain=$reason -ceq 'receipt';$script:fakeNativeProofPresent=$reason -ceq 'native-exit-unproved';$heldWorker=$true;$heldHeavy=$reason -ceq 'heavy';$registered=$reason -notin @('stop','heavy');$started=$reason -cne 'heavy';$taskReceipts='mock';$script:cleanupFault=$reason;$script:cleanupCalls=[Collections.Generic.List[string]]::new();$script:writes=@{};$caught=$false
 try{& $cleanup}catch{$caught=$_.Exception.Message -match 'unresolved owned writer/child'}
 $status=$script:writes["$lab/logs/$prefix-cleanup-status.json"]
 $retained=$script:writes["$lab/logs/$prefix-worker-retained.json"]
 if(!$caught -or !$status.worker_lease_held_or_unconfirmed -or !$retained.worker_lease_retained -or $retained.worker_release_attempted -or 'release-worker' -in $script:cleanupCalls){throw "lifecycle released worker after${reason}: $($script:cleanupCalls -join ',')"}
 if($reason -in @('receipt','native-exit-unproved','ras','sql') -and @($script:cleanupCalls|Where-Object{$_.EndsWith('-unregister') -or $_.EndsWith('-stop') -or $_ -eq 'release-native'}).Count){throw "cleanup crossed newlyunknown$reason guard"}
 if($reason -ceq 'unregister' -and @($script:cleanupCalls|Where-Object{$_.EndsWith('-stop') -or $_ -eq 'release-native'}).Count){throw 'uncertain unregister released native or stoppedcluster'}
 "PASS actual finally$reason keeps native/worker; recheck after cleanup failure and before next transition; calls=$($script:cleanupCalls -join ',')"
}
$prefix='pure-clean';$script:uncertain=$false;$script:fakeNativeProofPresent=$false;$heldWorker=$true;$heldHeavy=$false;$registered=$true;$started=$true;$script:cleanupFault='clean';$script:cleanupCalls.Clear();$script:writes=@{}
& $cleanup
$status=$script:writes["$lab/logs/$prefix-cleanup-status.json"]
if(!$status.guarded_stop_clean -or $status.worker_lease_held_or_unconfirmed -or $status.resources_may_need_exact_owned_cleanup -or ($script:cleanupCalls -join ',') -cne 'pure-clean-cleanup-empty,pure-clean-cleanup-sql-work,pure-clean-unregister-native-acquire,pure-clean-unregister,release-native,pure-clean-stop,release-worker'){throw 'proven clean lifecycle success changed'}
'PASS actual clean finally keeps normal unregister/stop/native/worker release sequence'
'PASS pure mocks: realStarts=realSignals=SQLcalls=registryWrites=0'

# The nested native completion fallback must name this exact new case.
$script:fallbackObserved='';$script:uncertain=$false
function Test-Path {param($LiteralPath);$script:fallbackObserved=$LiteralPath;return $LiteralPath -ceq "$lab/compact9-load-9-native-import.child-times.json"}
function Get-Content {param($LiteralPath,[switch]$Raw);return '{"writer_completion_proved":false}'}
if(!(Test-Compact9UnresolvedWriter) -or $script:fallbackObserved -cne "$lab/compact9-load-9-native-import.child-times.json"){throw 'wrong-case native fallback would release owned lifecycle'}
'PASS actual case8 nested-native fallback retains lifecycle with no sticky receipt; no actions'
