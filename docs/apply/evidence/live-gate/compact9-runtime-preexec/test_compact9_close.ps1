$ErrorActionPreference='Stop'
function Assert-Compact9RuntimeClosure {};$lab='F:/ibcmd/lab/05/wave3/load';$prefix='pure-close';$ib='11111111-1111-1111-1111-111111111111';$cluster='22222222-2222-2222-2222-222222222222';$rac='never-rac';$birth=[datetime]'2026-10-02T00:00:00Z'
$ast=[Management.Automation.Language.Parser]::ParseFile("$lab/compact9-runtime-v1/compact9_checkpoint_v2.ps1",[ref]$null,[ref]$null)
foreach($name in @('VerifyProcess','CloseCohort')){$fn=$ast.Find({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq $name},$true);. ([scriptblock]::Create($fn.Extent.Text))}
function Test-LiveUncertainChild {$script:uncertain}
function Get-CimInstance {param($ClassName,$Filter);$p=[pscustomobject]@{CommandLine='exact';ExecutablePath='C:\owned.exe';ParentProcessId=123;CreationDate=$birth};switch($script:mode){'pid-command'{$p.CommandLine='foreign'}'pid-exe'{$p.ExecutablePath='C:\foreign.exe'}'pid-parent'{$p.ParentProcessId=999}'pid-birth'{$p.CreationDate=$birth.AddSeconds(1)}};return $p}
function Run($name,$exe,$argv,$seconds){if($argv -contains 'terminate'){$script:events+='terminate';if($argv -notcontains '--session=33333333-3333-3333-3333-333333333333'){throw 'Foreign UUID signalled'}}else{$script:events+='stop'};[pscustomobject]@{ExitCode=0}}
function Inventory($name){if($name.EndsWith('empty-after')){return ''};$id=if($script:mode -ceq 'wrong-session-id'){'99'}else{'42'};$base=if($script:mode -ceq 'wrong-ib'){'foreign'}else{$ib};$start=if($script:mode -ceq 'wrong-start'){'later'}else{'2026-10-02T00:00:00'};$app=if($script:mode -ceq 'wrong-app'){'Other'}else{'1CV8C'};return "session : 33333333-3333-3333-3333-333333333333`ninfobase : $base`nsession-id : $id`napp-id : $app`nstarted-at : $start`nhibernate : yes`n"}
function PublishOwnership {$script:events+='publish'}
function Reset($Mode){$script:mode=$Mode;$script:uncertain=$Mode -ceq 'unknown';$script:events=@();$script:owned=@(@{label='old-a';pid=777;command='exact';executable='C:\owned.exe';parent_pid=123;creation=$birth.ToUniversalTime().Ticks;session_uuid='33333333-3333-3333-3333-333333333333';session_id='42';started_at='2026-10-02T00:00:00'})}
Reset clean;CloseCohort 'close'
if(($events -join ',') -cne 'stop,terminate,publish' -or $owned.Count){throw 'Exact close success changed'}
'PASS actual CloseCohort exact PID/start/exe/command/parent + UUID/IB/session/app/start bound; only owned UUID terminated'
foreach($mode in @('unknown','pid-command','pid-exe','pid-parent','pid-birth','wrong-session-id','wrong-ib','wrong-start','wrong-app')){
 Reset $mode;$caught=$false;try{CloseCohort 'close'}catch{$caught=$true}
 if(!$caught -or $events -contains 'terminate' -or $events -contains 'publish' -or $owned.Count -ne 1){throw "Unsafe close $mode"}
 if($mode -in @('unknown','pid-command','pid-exe','pid-parent','pid-birth') -and $events.Count){throw 'Uncertain/reused PID stop reached'}
 "PASS $mode refused; termination0/retained ownership"
}
'ALL PASS exact close/unknown mocks; realStarts=realSignals=SQLcalls=registryWrites=0'
