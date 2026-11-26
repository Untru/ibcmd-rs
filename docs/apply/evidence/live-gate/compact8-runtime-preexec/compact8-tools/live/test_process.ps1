# Pure OS mocks only: no native/SQL/cluster process is launched or signalled.
$ErrorActionPreference='Stop'
. "$PSScriptRoot/process.ps1"
function Assert-Compact8RuntimeClosure {}
$savedReceipts=$env:IBCMD_RS_LIVE_CHILD_RECEIPT_ROOT
try {
$env:IBCMD_RS_LIVE_CHILD_RECEIPT_ROOT='D:\foreign';$caught=$false
try{Invoke-LiveBounded never-spawn @() 1|Out-Null}catch{$caught=$_.Exception.Message -match 'unknown child receipt context'}
if(!$caught){throw 'foreign receipt context accepted'}
$env:IBCMD_RS_LIVE_CHILD_RECEIPT_ROOT=''
function Get-LiveChildReceiptRoot{return $null}
function Save-LiveUncertainChild($State){$script:receipt=$State}
$birth=[DateTime]'2026-10-01T00:00:00Z';$fakeExe='C:\mock\child.exe'
function Reset([string]$Mode,[string]$Fault=''){$script:mode=$Mode;$script:fault=$Fault;$script:LiveBoundedUncertainChild=$false;$script:receipt=$null;$script:cimCalls=0;$script:kills=0;$script:disposed=0;$script:waits=[Collections.Generic.List[int]]::new()}
function Get-CimInstance{param($ClassName,$Filter);$script:cimCalls++;$x=[pscustomobject]@{ProcessId=777777;CreationDate=$birth;ExecutablePath=$fakeExe;CommandLine='mock direct child';ParentProcessId=$PID}
 if($script:mode -eq 'race-exit'){$script:process.HasExited=$true;return $null}
 if($script:mode -eq 'missing'){return $null}
 if($script:mode -eq 'unbound'){$x.ExecutablePath='C:\foreign.exe'}
 if($script:cimCalls -gt 1){switch($script:fault){'birthday'{$x.CreationDate=$birth.AddSeconds(1)}'exe'{$x.ExecutablePath='C:\foreign.exe'}'cmd'{$x.CommandLine='changed'}'parent'{$x.ParentProcessId=1}'missing'{return $null}}};return $x
}
function New-LiveBoundedProcess($StartInfo){
 $script:process=[pscustomobject]@{Id=777777;StartTime=$birth;ExitTime=$birth.AddSeconds(1);HasExited=($script:mode -eq 'rapid');ExitCode=7;MainModule=[pscustomobject]@{FileName=$fakeExe};StandardOutput=[pscustomobject]@{};StandardError=[pscustomobject]@{};StandardInput=[pscustomobject]@{}}
 $script:process.StandardInput|Add-Member ScriptMethod Close {}
 $script:process.StandardOutput|Add-Member ScriptMethod ReadToEndAsync {if($script:mode -ceq 'pipe-timeout'){$pipe=[pscustomobject]@{};$pipe|Add-Member ScriptMethod Wait {param($Milliseconds);if($Milliseconds -ne 5000){throw 'unbounded pipe wait'};return $false};$pipe|Add-Member ScriptMethod GetAwaiter {throw 'GetResult reached before pipe completion'};return $pipe};[Threading.Tasks.Task]::FromResult([string]'mock stdout')}
 $script:process.StandardError|Add-Member ScriptMethod ReadToEndAsync {[Threading.Tasks.Task]::FromResult([string]'mock stderr')}
 $script:process|Add-Member ScriptMethod Start {return $true}
 $script:process|Add-Member ScriptMethod WaitForExit {param([int]$Milliseconds);$script:waits.Add($Milliseconds);if($script:mode -in @('normal','rapid','race-exit','pipe-timeout')){$this.HasExited=$true;return $true};if($script:waits.Count -eq 1){return $false};if($script:mode -eq 'reap-fails'){return $false};$this.HasExited=$true;return $true}
 $script:process|Add-Member ScriptMethod Kill {if($args.Count){throw 'tree kill reached'};$script:kills++}
 $script:process|Add-Member ScriptMethod Dispose {$script:disposed++}
 return $script:process
}
foreach($mode in @('normal','rapid','race-exit')){
 Reset $mode;$r=Invoke-LiveBounded child @('arg') 1
 if($r.ExitCode -ne 7 -or $r.Stdout -cne 'mock stdout' -or $r.Stderr -cne 'mock stderr' -or $script:kills -or $script:receipt -or (Test-LiveUncertainChild) -or $script:disposed -ne 1){throw "ordinary/rapid $mode API changed"}
 if($mode -eq 'rapid' -and $script:cimCalls){throw 'rapid exit unnecessarily required CIM authority'}
 "PASS $mode child preserves result schema/nonzero output, signals0"
}
foreach($mode in @('missing','unbound')){
 Reset $mode;$caught=$false;try{Invoke-LiveBounded child @('arg') 1|Out-Null}catch{$caught=$_.Exception.Message -match '^LIVE_CHILD_EXECUTION_UNCERTAIN '}
 if(!$caught -or $script:kills -or !(Test-LiveUncertainChild) -or $script:receipt.execution_state -cne 'unknown_running' -or $script:receipt.pid -ne 777777 -or $script:disposed -ne 1){throw "initial $mode unsafe"};"PASS initial$mode live refusal + unresolved receipt, signals0"
}
Reset timeout;$caught=$false;try{Invoke-LiveBounded child @('arg') 1|Out-Null}catch{$caught=$_.Exception.Message -match '^LIVE_CHILD_EXECUTION_UNCERTAIN '}
if(!$caught -or $script:kills -ne 1 -or ($script:waits -join ',') -cne '1000,5000' -or $script:receipt.execution_state -cne 'timeout_descendants_unproved' -or !$script:receipt.direct_child_exit_proved -or !(Test-LiveUncertainChild)){throw 'exact bounded timeout/reap'}
'PASS exact direct child Kill(no args), bounded5000 reap; wrapper descendants remain unproved, lifecycle retained'
foreach($fault in @('birthday','exe','cmd','parent','missing')){
 Reset timeout $fault;$caught=$false;try{Invoke-LiveBounded child @('arg') 1|Out-Null}catch{$caught=$_.Exception.Message -match '^LIVE_CHILD_EXECUTION_UNCERTAIN '}
 if(!$caught -or $script:kills -or !(Test-LiveUncertainChild) -or $script:receipt.execution_state -cne 'unknown_running' -or ($script:waits -join ',') -cne '1000'){throw "timeout$fault unsafe"}
 if($script:receipt.Contains('command') -or $script:receipt.Contains('arguments') -or $script:receipt.command_sha256 -cnotmatch '^[A-F0-9]{64}$'){throw 'unsanitized child receipt'}
 "PASS timeout$fault live refusal/retained identity, signals0"
}
Reset reap-fails;$caught=$false;try{Invoke-LiveBounded child @('arg') 1|Out-Null}catch{$caught=$_.Exception.Message -match '^LIVE_CHILD_EXECUTION_UNCERTAIN '}
if(!$caught -or $script:kills -ne 1 -or ($script:waits -join ',') -cne '1000,5000' -or !(Test-LiveUncertainChild)){throw 'unreaped child silently complete'}
'PASS unreaped child unresolved, original direct identity retained, no descendants signalled'
Reset pipe-timeout;$caught=$false;try{Invoke-LiveBounded child @('arg') 1|Out-Null}catch{$caught=$_.Exception.Message -match '^LIVE_CHILD_EXECUTION_UNCERTAIN '}
if(!$caught -or $script:kills -or !(Test-LiveUncertainChild) -or $script:receipt.execution_state -cne 'pipe_timeout_descendants_unproved' -or !$script:receipt.direct_child_exit_proved){throw 'inherited pipe completion unproved but lifetime released'}
'PASS inherited output pipe deadline5000 retains unresolved descendants/lifetime; no unbounded GetResult or signal'
'PASS pure child mocks; realStarts=realSignals=SQLcalls=registryWrites=0'
}finally{$env:IBCMD_RS_LIVE_CHILD_RECEIPT_ROOT=$savedReceipts}
