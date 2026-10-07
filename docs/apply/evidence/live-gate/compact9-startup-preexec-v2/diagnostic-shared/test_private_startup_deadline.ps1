# Actual production functions with a deterministic clock and in-memory censuses.
# No process, listener, lease, state-file or receipt-file operations are executed.
$ErrorActionPreference='Stop'
function Actual([string]$Name){
 $ast=[Management.Automation.Language.Parser]::ParseFile("$PSScriptRoot/private-lib.ps1",[ref]$null,[ref]$null)
 $found=@($ast.FindAll({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq $Name},$false))
 if($found.Count -ne 1){throw 'actual function missing'}
 $found[0].Extent.Text.Replace('[DateTime]::UtcNow','(Test-StartupUtcNow)')
}
foreach($name in @('Private-Key','Private-Identity','Private-Same','Private-Owned','Private-RelevantCensus','Private-StartupObservation','Private-ListenerRefusal','Private-StartupListeners')){. ([scriptblock]::Create((Actual $name)))}
$script:Root='F:\ibcmd\lab\05\wave3\load\cluster';$script:Bin='C:\Program Files\1cv8\8.3.27.2214\bin'
$script:ServerNames=@('ragent.exe','rmngr.exe','rphost.exe','ras.exe','dbda.exe')
$script:PrivateStartupPhase='waiting_port_5541';$script:PrivateStartupInitializedUtc=[DateTime]'2026-10-02T09:41:20Z'
$deadline=$script:PrivateStartupInitializedUtc.AddSeconds(60);$initialEntry=$deadline.AddTicks(-1)
function Process([int]$Id,[int]$Parent,[string]$Name){[pscustomobject]@{ProcessId=$Id;ParentProcessId=$Parent;Name=$Name;CreationDate=[DateTime]'2026-10-02T09:41:22Z';ExecutablePath="$script:Bin\$Name";CommandLine="owned $Name"}}
$agent=Process 700001 1 'ragent.exe';$child=Process 700002 700001 'dbda.exe';$ras=Process 700004 1 'ras.exe'
$state=[pscustomobject]@{fresh=$true;anchors=@((Private-Identity $agent),(Private-Identity $ras));known=@()}
function Test-StartupUtcNow{if($script:cimCalls -ge 2){return $script:after};return $script:entry}
function Private-RequireLease($State){$script:leaseCalls++}
function Get-CimInstance{param($Class);$script:cimCalls++;if($script:cimCalls -eq 1){return $script:first};if($script:cimCalls -eq 2){return $script:second};throw 'unexpected additional census'}
function Get-ClusterListeners{$script:listenerCalls++;return $script:listeners}
function Private-StateDigest{'A'*64}
function Private-PublishRefusalReceipt($Receipt){$script:receipt=$Receipt;'mock-no-file'}
function Start-Sleep{throw 'unexpected retry'}
function Reset([DateTime]$After){$script:after=$After;$script:entry=$initialEntry;$script:cimCalls=0;$script:leaseCalls=0;$script:listenerCalls=0;$script:receipt=$null;$script:first=@($agent,$ras,$child);$script:second=@($agent,$ras,$child);$script:listeners=@([pscustomobject]@{LocalPort=5561;OwningProcess=700002;LocalAddress='127.0.0.1'})}
function Refusal([string]$Reason,[int]$Censuses=2){
 $caught=$false;try{Private-StartupListeners $state $deadline|Out-Null}catch{$caught=$true;$message=$_.Exception.Message}
 if(!$caught -or !$script:receipt -or $script:receipt.reason -cne $Reason -or $script:cimCalls -ne $Censuses){throw "wrong refusal: $Reason; actual $message"}
 if($script:receipt.startup.deadline_utc -cne $deadline.ToUniversalTime().ToString('o') -or $script:receipt.startup.initialized_utc -cne $script:PrivateStartupInitializedUtc.ToUniversalTime().ToString('o') -or $script:receipt.startup.phase -cne 'waiting_port_5541' -or $script:receipt.additional_admission_censuses -ne 0){throw 'original deadline/phase lost'}
 if($Reason -ceq 'startup_deadline_expired' -and ($script:receipt.failed_listener -or $message -match 'foreign/unproved' -or $message -notmatch 'no admission or signal')){throw 'expiry misidentified a foreign listener'}
}
Reset $deadline.AddTicks(-1)
if(@(Private-StartupListeners $state $deadline).Count -ne 1 -or $script:cimCalls -ne 2 -or $script:receipt){throw 'stable before deadline changed'}
foreach($after in @($deadline,$deadline.AddTicks(1))){Reset $after;Refusal 'startup_deadline_expired';if($script:receipt.owned_ids -notcontains 700002 -or $script:receipt.startup.unstable_listener_observation){throw 'stable owned expiry classification wrong'}}
Reset $deadline;$script:entry=$deadline;Refusal 'startup_deadline_expired' 0
if($script:leaseCalls -or $script:listenerCalls -or $script:receipt.process_census.Count){throw 'expired entry performed acquisition'}
Reset $deadline;$script:second=@($agent);Refusal 'startup_deadline_expired'
if(!$script:receipt.startup.unstable_listener_observation){throw 'missing observation lost'}
Reset $deadline;$script:listeners=@();Refusal 'startup_deadline_expired'
if($script:receipt.listeners.Count){throw 'empty expiry census invented listener'}
foreach($after in @($deadline.AddTicks(-1),$deadline.AddTicks(1))){
 Reset $after;$foreign=Process 700003 9 'not-a-server.exe';$script:first+=@($foreign);$script:second+=@($foreign);$script:listeners[0].OwningProcess=700003
 Refusal 'listener_unproved';if($script:receipt.failed_listener.pid -ne 700003 -or 700003 -in $script:receipt.owned_ids){throw 'foreign ownership relaxed'}
}
foreach($field in @('CreationDate','CommandLine','ParentProcessId')){
 Reset $deadline.AddTicks(-1);$changed=Process 700002 700001 'dbda.exe'
 switch($field){'CreationDate'{$changed.CreationDate=$changed.CreationDate.AddTicks(10)} 'CommandLine'{$changed.CommandLine='changed'} 'ParentProcessId'{$changed.ParentProcessId=700004}}
 $script:second=@($agent,$ras,$changed);Refusal 'listener_identity_or_ancestry_drift'
}
# Invalid state remains an input refusal; it must not be reclassified as expiry.
Reset $deadline;$state.fresh=$false;$caught=$false;try{Private-StartupListeners $state $deadline|Out-Null}catch{$caught=$_.Exception.Message -ceq 'bounded fresh startup acquisition required'}
if(!$caught -or $script:cimCalls -or $script:receipt){throw 'invalid state classification changed'}
$state.fresh=$true
# The actual caller must also publish expiry if a complete observation has no
# requested port and its fixed sleep exhausts the original budget.
$startAst=[Management.Automation.Language.Parser]::ParseFile("$PSScriptRoot/private-start.ps1",[ref]$null,[ref]$null)
$wait=@($startAst.FindAll({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq 'Wait-PrivatePort'},$false))
if($wait.Count -ne 1){throw 'actual wait function unavailable'}
. ([scriptblock]::Create($wait[0].Extent.Text.Replace('[DateTime]::UtcNow','(Test-StartupUtcNow)')))
function Read-State{$state}
function Private-Remember($State){}
function Start-Sleep{param($Milliseconds);if($Milliseconds -ne 300){throw 'unexpected caller sleep'};$script:after=$deadline}
$script:PrivateStartupDeadline=$deadline;Reset $deadline.AddTicks(-1);$caught=$false
try{Wait-PrivatePort 5541}catch{$caught=$_.Exception.Message -match 'startup deadline expired'}
if(!$caught -or $script:cimCalls -ne 2 -or $script:receipt.reason -cne 'startup_deadline_expired' -or $script:receipt.startup.phase -cne 'waiting_port_5541' -or $script:receipt.startup.entry_utc -cne $deadline.ToUniversalTime().ToString('o')){throw 'caller timeout lost original deadline or reacquired censuses'}
'PASS 13 actual-function boundaries: stable, exact/+1 deadline, expired entry, missing/empty census, foreign before/after expiry, three identity drifts, invalid state, caller budget exhausted; starts=signals=files=leases=0'
