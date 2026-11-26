# Pure mock checks: no cluster/process/DB/registry starts, signals or writes.
$ErrorActionPreference='Stop'
$savedOverride=$env:IBCMD_RS_WORKER_LAB_ROOT
try{
$env:IBCMD_RS_WORKER_LAB_ROOT='F:\ibcmd\lab\05\wave3\load\cluster'
. "$PSScriptRoot\lib.ps1"
$when=[DateTime]'2026-10-01T00:00:00Z';$cluster='11111111-1111-1111-1111-111111111111'
function Fake([int]$Id,[int]$Parent,[string]$Name,[int]$Age=0){[pscustomobject]@{ProcessId=$Id;ParentProcessId=$Parent;Name=$Name;ExecutablePath=(Join-Path $script:Bin $Name);CreationDate=$when.AddSeconds($Age);CommandLine="private $Name"}}
$agent=Fake 700001 1 'ragent.exe';$agent.CommandLine='"'+$script:Bin+'\ragent.exe" -agent -port 5540 -regport 5541 -range 5560:5591 -d '+$script:Srvinfo
$ras=Fake 700002 1 'ras.exe';$ras.CommandLine='"'+$script:Bin+'\ras.exe" cluster --port=5545 localhost:5540'
$child=Fake 700003 700001 'rphost.exe' 1;$foreign=Fake 700004 9 'rphost.exe' 1
$global:Private83FakeProcesses=@($agent,$ras,$child,$foreign);$global:Private83FakeListeners=@();$global:Private83FakeJson=$null;$global:Private83FakeLease='track=load since=2026-10-01T00:00:00';$global:Private83Reparse='';$global:Private83SignalCount=0;$global:Private83StartCount=0;$global:Private83DeleteCount=0;$global:Private83MoveCount=0;$global:Private83DataExists=$false;$global:Private83Registration=''
$global:Private83CensusCount=0;$global:Private83SpawnAtListen=$null;$global:Private83CensusQueue=[Collections.Generic.Queue[object]]::new();$global:Private83CensusDelay=0
function Get-CimInstance{param($ClassName,$Filter);if($Filter -match '^ProcessId=(\d+)$'){@($global:Private83FakeProcesses|Where-Object{$_.ProcessId -eq [int]$Matches[1]})}else{$global:Private83CensusCount++;if($global:Private83CensusDelay){Start-Sleep -Milliseconds $global:Private83CensusDelay};if($global:Private83CensusQueue.Count){$global:Private83CensusQueue.Dequeue()}else{$global:Private83FakeProcesses}}}
function Get-NetTCPConnection{param($State,$LocalPort,$ErrorAction);if($global:Private83SpawnAtListen){$global:Private83FakeProcesses+=@($global:Private83SpawnAtListen);$global:Private83SpawnAtListen=$null};$global:Private83FakeListeners}
function Test-Path{param($LiteralPath);if($LiteralPath -eq $script:StateFile){return [bool]$global:Private83FakeJson};if($LiteralPath -eq $script:Srvinfo){return $global:Private83DataExists};if($LiteralPath -eq $global:Private83Reparse){return $true};if($LiteralPath -like 'F:\ibcmd\lab\04\locks\worker\*' -or $LiteralPath -like "$script:Bin\*.exe"){return $true};return $false}
function Get-Item{param($LiteralPath,[switch]$Force);[pscustomobject]@{Attributes=$(if($LiteralPath -eq $global:Private83Reparse){[IO.FileAttributes]::ReparsePoint}else{[IO.FileAttributes]::Directory})}}
function Get-Content{param($LiteralPath,[switch]$Raw);if($LiteralPath -eq $script:StateFile){return $global:Private83FakeJson};if($LiteralPath -like 'F:\ibcmd\lab\04\locks\worker\*'){return $global:Private83FakeLease};throw 'unexpected mock read'}
function Set-Content{param($LiteralPath,$Encoding,[Parameter(ValueFromPipeline)]$Value);begin{$values=@()}process{$values+=$Value}end{if($LiteralPath -eq $script:StateFile){$global:Private83FakeJson=$values -join "`n"}}}
function Add-Content{param($LiteralPath,$Encoding,[Parameter(ValueFromPipeline)]$Value)}
$global:Private83FakeOrigin='from F:\ibcmd\lab\05\wave3\load\own.bak'
function Import-Csv{param($LiteralPath,$Delimiter);@([pscustomobject]@{track='load';database='ibcmd_rs_05_load_w3_mock';notes=$global:Private83FakeOrigin},[pscustomobject]@{track='meta';database='ibcmd_rs_05_meta_w3_mock';notes='from F:\ibcmd\lab\dbbak\bsp.bak'})}
function Stop-Process{param($Id,[switch]$Force);$global:Private83SignalCount++;throw 'mock signal reached'}
function Start-Process{param($FilePath,$ArgumentList,$WindowStyle,[switch]$PassThru);$global:Private83StartCount++;throw 'mock start reached'}
function Remove-Item{param($LiteralPath,[switch]$Force,[switch]$Recurse);$global:Private83DeleteCount++;throw 'mock delete reached'}
function Move-Item{param($LiteralPath,$Destination);$global:Private83MoveCount++;throw 'mock archive reached'}
function Get-ChildItem{param($LiteralPath,[switch]$Recurse,[switch]$Force);if($global:Private83Reparse){[pscustomobject]@{FullName=$global:Private83Reparse}}}
$global:Private83RacFailure=''
function Invoke-LiveBounded{param($Executable,$Arguments,$TimeoutSeconds);if($global:Private83RacFailure -eq 'timeout'){throw 'mock bounded timeout'};$text='';if(($Arguments -join ' ') -like '*cluster list*'){$text="cluster : $cluster"};if(($Arguments -join ' ') -like '*infobase summary list*'){$text=$global:Private83Registration};[pscustomobject]@{ExitCode=$(if($global:Private83RacFailure -eq 'exit'){1}else{0});Stdout=$text;Stderr=$(if($global:Private83RacFailure -eq 'stderr'){'warning'}else{''})}}
function Refuse([scriptblock]$Run,[string]$Pattern){$caught=$false;$detail='no exception';try{& $Run|Out-Null}catch{$detail=$_.Exception.Message;$caught=$detail -match $Pattern;if(!$caught){$detail+="`n"+$_.ScriptStackTrace}};if(!$caught){throw "expected refusal: $Pattern; actual: $detail"}}
function Private-StateDigest{[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($global:Private83FakeJson)))}
function Private-PublishRefusalReceipt($Receipt){$global:Private83LastRefusal=$Receipt;return 'mock-receipt-no-file-write'}
$state=[pscustomobject]@{format=1;platform=$script:Platform;root=$script:Root;track='load';fresh=$true;cluster=$cluster;anchors=@((Private-Identity $agent),(Private-Identity $ras));known=@();worker_lease=$global:Private83FakeLease}
Write-State $state;$validated=Read-State
if(@(Private-Owned $validated).Count -ne 3){throw 'foreign included/own child missing'}
Private-Remember $validated;$retained=Read-State
$global:Private83FakeProcesses=@($ras,$child,$foreign)
if(@(Private-Owned $retained).Count -ne 2){throw 'orphan was lost after root exit'}
$child.CreationDate=$when.AddSeconds(20);Refuse {Private-Owned $retained} 'PID reused';$child.CreationDate=$when.AddSeconds(1)
$child.CommandLine='foreign replacement';Refuse {Private-Owned $retained} 'PID reused';$child.CommandLine='private rphost.exe'
$global:Private83FakeProcesses=@($agent,$ras,$child,$foreign);$child.ExecutablePath='C:\foreign\rphost.exe';Refuse {Private-Owned $state} 'descendant executable';$child.ExecutablePath=Join-Path $script:Bin 'rphost.exe'
$invalid=$retained|ConvertTo-Json -Depth 9|ConvertFrom-Json;$invalid.known=@(Private-Identity $foreign);Write-State $invalid;Refuse {Read-State} 'retained ancestry';Write-State $retained
$global:Private83FakeListeners=@([pscustomobject]@{LocalPort=5540;OwningProcess=700004});Refuse {Private-RequireListeners $retained} 'foreign/unproved';$global:Private83FakeListeners=@()
$foreign.CommandLine='foreign -regport 5541';Refuse {Private-RequireListeners $retained} 'unknown private-looking';$foreign.CommandLine='private rphost.exe'
# A proven child can appear during listener sampling. The old guard sampled
# ownership before listeners and falsely refused this child as foreign.
$global:Private83FakeProcesses=@($agent,$ras,$foreign);$global:Private83SpawnAtListen=$child
$global:Private83FakeListeners=@([pscustomobject]@{LocalPort=5560;OwningProcess=700003});$global:Private83CensusCount=0
Private-RequireListeners $state
if($global:Private83CensusCount -ne 1){throw 'listener guard used inconsistent process censuses'}
# An unrelated process born at the same boundary must still be refused.
$global:Private83FakeProcesses=@($agent,$ras,$child);$global:Private83SpawnAtListen=$foreign
$global:Private83FakeListeners=@([pscustomobject]@{LocalPort=5560;OwningProcess=700004})
Refuse {Private-RequireListeners $state} 'foreign/unproved'
# A listener sampled before its process exits must remain refused. The receipt
# captures that exact census; diagnostics must not resample it into admission.
$global:Private83FakeProcesses=@($agent,$ras,$child);$global:Private83FakeListeners=@([pscustomobject]@{LocalPort=5561;OwningProcess=700099;LocalAddress='127.0.0.1'});$global:Private83CensusCount=0;$global:Private83LastRefusal=$null
Refuse {Private-RequireListeners $state} 'foreign/unproved'
if($global:Private83CensusCount -ne 1 -or !$global:Private83LastRefusal -or $global:Private83LastRefusal.failed_listener.pid -ne 700099 -or $global:Private83LastRefusal.failed_listener.port -ne 5561 -or $global:Private83LastRefusal.process_census.Count -ne 3 -or 700099 -in $global:Private83LastRefusal.owned_ids -or $global:Private83LastRefusal.admission -cne 'refused' -or $global:Private83LastRefusal.additional_admission_censuses -ne 0 -or $global:Private83LastRefusal.state_sha256 -cne (Private-StateDigest)){throw 'transient listener census/refusal receipt incorrect'}
foreach($entry in $global:Private83LastRefusal.process_census){if($entry.Contains('command') -or $entry.command_sha256 -cnotmatch '^[A-F0-9]{64}$' -or !$entry.born -or !$entry.executable){throw 'receipt command/identity sanitization incorrect'}}
# Startup uses full C1 -> listeners -> full C2. A missing listener process
# cannot be admitted until a later complete, identical ancestry pair exists.
$global:Private83FakeProcesses=@($agent,$ras,$child)
$global:Private83FakeListeners=@([pscustomobject]@{LocalPort=5561;OwningProcess=700003;LocalAddress='127.0.0.1'})
$global:Private83CensusCount=0
$stable=@(Private-StartupListeners $state ([DateTime]::UtcNow.AddSeconds(2)))
if($stable.Count -ne 1 -or $global:Private83CensusCount -ne 2){throw 'startup did not use exact C1/listener/C2 pair'}
$global:Private83CensusQueue.Enqueue([object[]]@($agent,$ras))
$global:Private83CensusQueue.Enqueue([object[]]@($agent,$ras,$child))
$global:Private83CensusCount=0
$stable=@(Private-StartupListeners $state ([DateTime]::UtcNow.AddSeconds(2)))
if($stable.Count -ne 1 -or $global:Private83CensusCount -ne 4){throw 'missing startup observation admitted without a complete later pair'}
# An unknown/vanished listener is bounded and never positive; stop retains its
# immediate one-census refusal regardless of startup sampling support.
$global:Private83FakeListeners=@([pscustomobject]@{LocalPort=5561;OwningProcess=700099;LocalAddress='127.0.0.1'})
$global:Private83CensusCount=0
Refuse {Private-StartupListeners $state ([DateTime]::UtcNow.AddMilliseconds(180))} 'startup deadline expired'
if($global:Private83CensusCount -lt 2 -or $global:Private83LastRefusal.failed_listener -or $global:Private83LastRefusal.reason -cne 'startup_deadline_expired' -or !$global:Private83LastRefusal.startup.unstable_listener_observation -or $global:Private83LastRefusal.listeners[0].pid -ne 700099){throw 'unstable deadline did not retain exact refusal'}
$global:Private83CensusCount=0;Refuse {Private-RequireListeners $state} 'foreign/unproved'
if($global:Private83CensusCount -ne 1){throw 'stop guard acquired a startup resampling loop'}
# Full census captures a non-whitelisted listener even when the server-only
# inventory would have hidden it. No retry of foreign identities is allowed.
$unknown=Fake 700005 700001 'not-a-server.exe' 2
$global:Private83FakeProcesses=@($agent,$ras,$child,$unknown)
$global:Private83FakeListeners=@([pscustomobject]@{LocalPort=5561;OwningProcess=700005;LocalAddress='127.0.0.1'})
$global:Private83CensusCount=0
Refuse {Private-StartupListeners $state ([DateTime]::UtcNow.AddSeconds(2))} 'foreign/unproved'
if($global:Private83CensusCount -ne 2 -or !($global:Private83LastRefusal.process_census|Where-Object{$_.pid -eq 700005 -and $_.name -ceq 'not-a-server.exe'})){throw 'full census concealed non-whitelist listener'}
$global:Private83FakeProcesses=@($agent,$ras,$child,$foreign)
$global:Private83FakeListeners=@([pscustomobject]@{LocalPort=5561;OwningProcess=700004})
$global:Private83CensusCount=0
Refuse {Private-StartupListeners $state ([DateTime]::UtcNow.AddSeconds(2))} 'foreign/unproved'
if($global:Private83CensusCount -ne 2){throw 'foreign listener was resampled'}
$global:Private83FakeProcesses=@($agent,$ras,$child)
$global:Private83FakeListeners=@([pscustomobject]@{LocalPort=5561;OwningProcess=700003})
foreach($field in @('CreationDate','CommandLine','ParentProcessId')){
 $changed=Fake 700003 700001 'rphost.exe' 1
 switch($field){'CreationDate'{$changed.CreationDate=$when.AddSeconds(2)} 'CommandLine'{$changed.CommandLine='changed'} 'ParentProcessId'{$changed.ParentProcessId=700002}}
 $global:Private83CensusQueue.Enqueue([object[]]@($agent,$ras,$child))
 $global:Private83CensusQueue.Enqueue([object[]]@($agent,$ras,$changed))
 $global:Private83CensusCount=0
 Refuse {Private-StartupListeners $state ([DateTime]::UtcNow.AddSeconds(2))} 'foreign/unproved'
 if($global:Private83LastRefusal.reason -cne 'listener_identity_or_ancestry_drift'){throw 'identity drift receipt missing'}
 if($global:Private83CensusCount -ne 2){throw 'changed PID identity was resampled'}
}
$unknown.CommandLine='unknown '+$script:Root
$global:Private83FakeProcesses=@($agent,$ras,$child,$unknown);$global:Private83FakeListeners=@()
Refuse {Private-StartupListeners $state ([DateTime]::UtcNow.AddSeconds(2))} 'unknown private-looking'
$global:Private83FakeProcesses=@($agent,$ras,$child)
$global:Private83FakeListeners=@([pscustomobject]@{LocalPort=5561;OwningProcess=700003});$global:Private83CensusDelay=80
Refuse {Private-StartupListeners $state ([DateTime]::UtcNow.AddMilliseconds(100))} 'startup deadline expired'
$global:Private83CensusDelay=0
$global:Private83FakeProcesses=@($agent,$ras,$child,$foreign);$global:Private83FakeListeners=@()
Private-RequireNames @('ibcmd_rs_05_load_w3_mock')
foreach($origin in @('from F:..\..\lab\05\wave3\load\own.bak','from \ibcmd\lab\05\wave3\load\own.bak')){$global:Private83FakeOrigin=$origin;Refuse {Private-RequireNames @('ibcmd_rs_05_load_w3_mock')} 'foreign/unmanifested'};$global:Private83FakeOrigin='from F:\ibcmd\lab\05\wave3\load\own.bak'
foreach($name in @('bsp','ibcmd_rs_05_load_w3_absent','ibcmd_rs_05_meta_w3_mock')){Refuse {Private-RequireNames @($name)} 'foreign/unmanifested'}
$global:Private83FakeListeners=@([pscustomobject]@{LocalPort=5545;OwningProcess=700002})
$global:Private83Registration="infobase : 22222222-2222-2222-2222-222222222222`nname : ibcmd_rs_05_load_w3_absent";Refuse {Private-Snapshot $retained 'disconnected'} 'foreign/unmanifested'
$global:Private83Registration='unexpected protocol';Refuse {Private-Snapshot $retained 'invalid'} 'unparsed';$global:Private83Registration=''
foreach($failure in @('exit','stderr','timeout')){$global:Private83RacFailure=$failure;Refuse {Private-Snapshot $retained 'failed-rac'} 'failed/timed out|bounded timeout'};$global:Private83RacFailure=''
$global:Private83Reparse='F:\ibcmd\lab\05\wave3';Refuse {Require-PrivatePath $script:Srvinfo} 'reparse ancestry';$global:Private83Reparse=''
Refuse {Require-PrivatePath 'D:\foreign'} 'escaped';Refuse {Require-PrivatePath "$script:Root\..\foreign"} 'escaped'
$global:Private83FakeLease='track=meta since=2026-10-01T00:00:00';Refuse {Private-RequireLease $retained} 'another track';$global:Private83FakeLease='track=load since=2026-10-01T00:00:01';Refuse {Private-RequireLease $retained} 'lease changed';$global:Private83FakeLease=$retained.worker_lease
# Guarded entrypoints refuse without any native process start/signal/delete.
$global:Private83FakeJson=$null;Refuse {& "$PSScriptRoot\stop.ps1"} 'no ownership state'
$global:Private83FakeListeners=@([pscustomobject]@{LocalPort=5540;OwningProcess=700004});Refuse {& "$PSScriptRoot\start.ps1" -Track load -TimeoutSec 5} 'ports occupied';$global:Private83FakeListeners=@()
$global:Private83DataExists=$true;Refuse {& "$PSScriptRoot\start.ps1" -Track load -TimeoutSec 5} 'fresh srvinfo';$global:Private83DataExists=$false
Write-State $retained;$global:Private83FakeProcesses=@();$global:Private83FakeListeners=@();$retained.cluster='';Write-State $retained
$global:Private83Reparse=Join-Path $script:Logs 'linked-child';$global:Private83DataExists=$true;Refuse {& "$PSScriptRoot\stop.ps1" -Purge} 'reparse ancestry'
if(!$global:Private83FakeJson -or $global:Private83SignalCount -or $global:Private83StartCount -or $global:Private83DeleteCount -or $global:Private83MoveCount){throw 'refusal mutated lifecycle'}
$global:Private83Reparse='';$global:Private83DataExists=$false
$env:IBCMD_RS_WORKER_LAB_ROOT='F:\ibcmd\lab\05\wave3\metadata\cluster';. "$PSScriptRoot\lib.ps1"
if($script:PrivateContext.track -cne 'meta' -or $script:PrivateContext.prefix -cne 'ibcmd_rs_05_meta_w3_'){throw 'metadata mapping'};Private-RequireNames @('ibcmd_rs_05_meta_w3_mock');Refuse {Private-RequireNames @('ibcmd_rs_05_load_w3_mock')} 'foreign/unmanifested'
$env:IBCMD_RS_WORKER_LAB_ROOT='F:\ibcmd\lab\05\wave3\other\cluster';Refuse {. "$PSScriptRoot\lib.ps1"} 'override is limited'
$env:IBCMD_RS_WORKER_LAB_ROOT='';. "$PSScriptRoot\lib.ps1";if($script:PrivateContext -or $script:Root -ne 'F:\ibcmd\lab\05\cluster'){throw 'legacy default changed'}
$env:IBCMD_RS_WORKER_LAB_ROOT='F:\ibcmd\lab\05\wave1\live\cluster';. "$PSScriptRoot\lib.ps1";if($script:PrivateContext -or $script:Root -ne $env:IBCMD_RS_WORKER_LAB_ROOT){throw 'legacy wave1 changed'}
'PASS bounded full-C1/listeners/full-C2 startup, immediate stop, non-whitelist/missing/deadline/PID-drift; retained ancestry/UTC/orphan/foreign/PID/executable/command/lease/registry/path/reparse/refusal/context mocks; starts=signals=deletes=archives=0'
}finally{$env:IBCMD_RS_WORKER_LAB_ROOT=$savedOverride}
