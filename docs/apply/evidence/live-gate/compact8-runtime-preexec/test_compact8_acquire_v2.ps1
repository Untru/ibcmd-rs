function Test-Compact8ResourceUnconfirmed{return $false}
$ErrorActionPreference='Stop'
function Assert-Compact8RuntimeClosure {}
$lab='F:/ibcmd/lab/05/wave3/load';$prefix='pure-acquire';$taskReceipts='mock';$script:uncertain=$false;$script:releases=@();$script:mutations=0
$t=$null;$e=$null;$tree=[Management.Automation.Language.Parser]::ParseFile("$lab/compact8-runtime-v1/compact8_lifetime_v2.ps1",[ref]$t,[ref]$e);if($e.Count){throw 'parser'}
foreach($name in @('Test-Compact8UnresolvedWriter','Assert-Compact8CleanupAuthority')){$f=@($tree.FindAll({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq $name},$true));Invoke-Expression $f[0].Extent.Text}
$outer=@($tree.EndBlock.Statements|Where-Object{$_ -is [Management.Automation.Language.TryStatementAst]});if($outer.Count -ne 1){throw 'outer actual try'}
function Test-LiveUncertainChild{return $script:uncertain}
$script:case7File=$false
function Test-Path {param($LiteralPath);return $script:case7File -and $LiteralPath -ceq "$lab/compact8-load-7-native-import.child-times.json"}
function Get-Item {param($LiteralPath);if($LiteralPath -cne "$lab/compact8-load-7-native-import.child-times.json"){throw 'wrong native proof path'};[pscustomobject]@{Length=100}}
function Get-Content {param($LiteralPath,[switch]$Raw);if($LiteralPath -cne "$lab/compact8-load-7-native-import.child-times.json"){throw 'unexpected read'};return '{"writer_completion_proved":false,"direct_child_exit_proved":true}'}
function Set-Content {param($LiteralPath,$Encoding,[Parameter(ValueFromPipeline)]$Value);process{}}
function Run($name,$exe,$argv,$seconds){if($name -like '*acquire'){$script:uncertain=$true;throw 'mock granted then bounded child uncertain'};$script:mutations++;throw 'cleanup writer after uncertainty'}
function Invoke-LiveBounded($Executable,$Arguments,$TimeoutSeconds){$script:releases+=@($Arguments[-1]);if($script:releaseUnknown -and $Arguments[-1] -ceq 'worker'){$script:uncertain=$true};[pscustomobject]@{ExitCode=0;Stdout='released';Stderr=''}}
foreach($kind in @('worker','heavy')){
 $heldWorker=$kind -ceq 'heavy';$heldHeavy=$false;$started=$false;$registered=$false;$registrationAttempted=$false;$ib='';$script:uncertain=$false;$script:releases=@();$script:mutations=0
 $statements=$outer[0].Body.Statements;$matches=@(0..($statements.Count-1)|Where-Object{$statements[$_].Extent.Text -match ('Run "\$prefix-'+$kind+'-acquire"')});if($matches.Count -ne 1){throw 'actual acquire statement'};$i=$matches[0]
 if($statements[$i-1].Extent.Text -notmatch ('\$held'+$kind+'=\$true')){throw 'potential lease set after child'}
 $caught=$false;try{Invoke-Expression ($statements[$i-1].Extent.Text+"`n"+$statements[$i].Extent.Text)}catch{$caught=$true};if(!$caught -or !(Get-Variable "held$kind" -ValueOnly)){throw 'granted/unknown lease not retained'}
 $body=$outer[0].Finally.Extent.Text.Trim();try{Invoke-Expression $body.Substring(1,$body.Length-2)}catch{}
 if($kind -ceq 'heavy' -and ($script:releases -join ',') -cne 'heavy'){throw 'actual heavy finally not exercised'}
 if(!$heldWorker -or $script:mutations -or $script:releases -contains 'worker' -or $script:releases -contains 'native'){throw 'actual finally released unknown worker/native lease'}
 "PASS actual $kind acquire grants then unknown; flags set before dispatch, finally worker/native retained; heavy independent"
}
$heldWorker=$true;$heldHeavy=$false;$started=$true;$registered=$true;$registrationAttempted=$true;$ib='mock';$script:uncertain=$false;$script:case7File=$true;$script:releases=@();$script:mutations=0
$body=$outer[0].Finally.Extent.Text.Trim();try{Invoke-Expression $body.Substring(1,$body.Length-2)}catch{}
if(!$heldWorker -or $script:mutations -or $script:releases.Count -or $script:uncertain){throw 'case7 completion fallback missed; cleanup/release reached'}
'PASS actual finally case7 writer-completion=false without sticky receipt retains worker, unregister/stop/release0'
$script:case7File=$false
$heldWorker=$true;$heldHeavy=$false;$started=$false;$registered=$false;$script:uncertain=$false;$script:releases=@();$script:releaseUnknown=$true
$body=$outer[0].Finally.Extent.Text.Trim();try{Invoke-Expression $body.Substring(1,$body.Length-2)}catch{}
if(!$heldWorker -or !$script:uncertain -or ($script:releases -join ',') -cne 'worker'){throw 'late worker release uncertainty cleared potential lease'}
'PASS actual successful release with newly uncertain receipt retains unconfirmed lease flag before clearing'
'PASS pure actual acquire/finally AST mocks; realStarts=realSignals=SQLcalls=registryWrites=0'
