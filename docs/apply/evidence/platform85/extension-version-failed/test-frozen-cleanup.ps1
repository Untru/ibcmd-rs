# Actual cleanup functions/sequence/finally, fake bounded children only.
$ErrorActionPreference='Stop'
. "$PSScriptRoot\cleanup_extension_v4.ps1" -DefinitionsOnly
$script:Bin='mock-bin'
function RequireCleanupClosure{if($script:badClosure){throw 'mock frozen dependency drift'}}
function RequireLabPaths85($Paths){}
function Test-LiveUncertainChild{return $script:uncertain}
function ExactLease{if($script:badLease){throw 'mock exact lease drift'}}
function State85{if($script:stopped){return $null};return $script:savedState}
function Same85($p,$a){return !$script:anchorDrift}
function Owned85($state){return @()}
function RequireOwnedNames85($Names){if($script:unowned){throw 'mock ownership drift'}}
function Get-CimInstance{param($ClassName,$Filter);if($Filter -ceq 'ProcessId=44392'){if($script:clientPresent){return @{ProcessId=44392}};return $null};if($Filter){return @{ProcessId=1}};return @()}
function Test-Path{param($LiteralPath);return $false}
function Set-Content{param($LiteralPath,$Encoding,[Parameter(ValueFromPipeline)]$Value);process{}}
function Start-Sleep{param($Milliseconds)}
function SaveCleanup{$script:saved=@{held=$held;registered=$registered;clean=$clean}}
function Invoke-LiveBounded($Exe,$Arguments,$Seconds){
 $name=if($Arguments -contains 'summary'){'registry'}elseif($Arguments -contains 'terminate'){'terminate'}elseif($Arguments -contains 'session'){'sessions'}elseif($Arguments -contains '-Q'){'idle-sql'}elseif($Arguments -contains 'unregister'){'unregister'}elseif($Arguments -like '*snapshot_storage.py'){'snapshot'}elseif($Arguments -like '*stop.ps1'){'stop'}elseif($Arguments -contains 'release'){'release'}else{throw 'unexpected fake child'}
 $script:calls+=,$name
 if($name -ceq 'terminate'){
  if(!$script:snapshotSeen){throw 'termination before full postfailure snapshot'}
  if(($Arguments -join ',') -cne "localhost:6545,session,terminate,--cluster=$cluster,--session=$session"){throw 'broad or wrong termination arguments'}
  $script:terminated=$true
 }
 if($name -ceq 'idle-sql' -and (($Arguments -join ',') -notmatch 'sys.dm_tran_database_transactions' -or ($Arguments -join ',') -match '(?i)\bKILL\b')){throw 'idle query lost zeroTX/readonly scope'}
 if($name -ceq 'snapshot'){$script:snapshotSeen=$true}
 if($name -ceq 'unregister'){$script:unregistered=$true}
 if($name -ceq 'stop'){$script:stopped=$true}
 if($script:fault -ceq "late-$name"){$script:uncertain=$true}
 $out=''
 if($name -ceq 'registry' -and !$script:unregistered){$id=if($script:wrongIB -or ($script:fault -ceq 'registry-drift-after-snapshot' -and $script:snapshotSeen)){'foreign-id'}else{$ib};$out="infobase : $id`nname : $db`n"}
 if($name -ceq 'sessions' -and !$script:terminated){$out=$script:sessionText;if($script:fault -ceq 'session-drift-after-snapshot' -and $script:snapshotSeen){$out=$out.Replace($session,'foreign-session')}}
 if($name -ceq 'sessions' -and $script:terminated -and $script:fault -ceq 'remaining-session'){$out=$script:sessionText}
 return [pscustomobject]@{ExitCode=$(if($script:fault -ceq "nonzero-$name"){1}else{0});Stdout=$out;Stderr=''}
}
$baseline="session : $session`nsession-id : 1`ninfobase : $ib`nstarted-at : 2026-10-02T00:25:15`napp-id : 1CV8C`nhibernate : yes`nconnection : 00000000-0000-0000-0000-000000000000`nprocess : 00000000-0000-0000-0000-000000000000`n"
$cases=0
foreach($scenario in @('clean','client-present','anchor-drift','lease-drift','closure-drift','unowned','initial-receipt','wrong-IB','wrong-SID','wrong-session-UUID','wrong-start','wrong-app','live-connection','live-process','active-session','foreign-second-session','duplicate-field','registry-drift-after-snapshot','session-drift-after-snapshot','nonzero-idle-sql','late-snapshot','late-terminate','nonzero-terminate','remaining-session','late-unregister','late-stop','late-release')){
 $script:fault=$scenario;$script:clientPresent=$scenario -ceq 'client-present';$script:anchorDrift=$scenario -ceq 'anchor-drift';$script:badLease=$scenario -ceq 'lease-drift';$script:badClosure=$scenario -ceq 'closure-drift';$script:unowned=$scenario -ceq 'unowned';$script:wrongIB=$scenario -ceq 'wrong-IB';$script:uncertain=$scenario -ceq 'initial-receipt'
 $script:held=$true;$script:registered=$true;$script:clean=$false;$script:step=0;$script:snapshotSeen=$false;$script:terminated=$false;$script:unregistered=$false;$script:stopped=$false;$script:calls=@();$script:saved=$null
 $script:savedState=[pscustomobject]@{cluster=$cluster;root="$lab\cluster";track='p85';platform='8.5.1.1150';anchors=@(@{pid=1;born='saved'})}
 $script:sessionText=$baseline
 if($scenario -ceq 'wrong-SID'){$script:sessionText=$baseline.Replace('session-id : 1','session-id : 2')}
 if($scenario -ceq 'wrong-session-UUID'){$script:sessionText=$baseline.Replace($session,'foreign-session')}
 if($scenario -ceq 'wrong-start'){$script:sessionText=$baseline.Replace('00:25:15','00:25:16')}
 if($scenario -ceq 'wrong-app'){$script:sessionText=$baseline.Replace('1CV8C','Designer')}
 if($scenario -ceq 'live-connection'){$script:sessionText=$baseline.Replace('connection : 00000000-0000-0000-0000-000000000000','connection : 00000000-0000-0000-0000-000000000001')}
 if($scenario -ceq 'live-process'){$script:sessionText=$baseline.Replace('process : 00000000-0000-0000-0000-000000000000','process : 00000000-0000-0000-0000-000000000001')}
 if($scenario -ceq 'active-session'){$script:sessionText=$baseline.Replace('hibernate : yes','hibernate : no')}
 if($scenario -ceq 'foreign-second-session'){$script:sessionText=$baseline+"`n"+$baseline}
 if($scenario -ceq 'duplicate-field'){$script:sessionText=$baseline+"session-id : 1`n"}
 $failed=$false;try{Cleanup}catch{$failed=$true}
 if($scenario -ceq 'clean'){
  if($failed -or $script:saved.held -or $script:saved.registered -or !$script:saved.clean -or ($script:calls -join ',') -cne 'registry,sessions,idle-sql,snapshot,registry,idle-sql,sessions,terminate,sessions,registry,idle-sql,unregister,registry,sessions,stop,release'){throw 'actual clean sequence/flags differs'}
 }else{
  if(!$failed -or !$script:saved.held){throw "actual refusal lost worker $scenario"}
  if($scenario -notin @('late-unregister','late-stop','late-release') -and $script:calls -contains 'unregister'){throw "unsafe unregister $scenario"}
  if($scenario -notin @('late-stop','late-release') -and $script:calls -contains 'stop'){throw "unsafe stop $scenario"}
  if($scenario -ne 'late-release' -and $script:calls -contains 'release'){throw "unsafe worker release $scenario"}
  if($scenario -notin @('late-terminate','nonzero-terminate','remaining-session','late-unregister','late-stop','late-release') -and $script:calls -contains 'terminate'){throw "unsafe session termination $scenario"}
 }
 $cases++
}
"PASS actual exact cleanup sequence/finally $cases scenarios; real file/SQL/process/FIFO/session actions=0"
