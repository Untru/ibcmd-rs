$ErrorActionPreference='Stop'
function Assert-Compact9RuntimeClosure {};$lab='F:/ibcmd/lab/05/wave3/load'
$ast=[Management.Automation.Language.Parser]::ParseFile("$lab/compact9-runtime-v1/compact9_checkpoint_v2.ps1",[ref]$null,[ref]$null)
$body=$ast.EndBlock.Statements|Where-Object {$_ -is [Management.Automation.Language.TryStatementAst]}
$rule=$body.Body.Statements|Where-Object {$_.Extent.Text.StartsWith("if(`$report.state -cne 'already_complete'")}
if(@($rule).Count -ne 1){throw 'Actual repeat assertion missing'}
$assertion=[scriptblock]::Create($rule.Extent.Text)
$report=[pscustomobject]@{state='already_complete';cycle_2_executed=$false};$before='A'*64;$after=$before;& $assertion
foreach($mode in @('third-cycle','tail-change','wrong-state')){
 $report=[pscustomobject]@{state='already_complete';cycle_2_executed=$false};$after=$before
 switch($mode){'third-cycle'{$report.cycle_2_executed=$true}'tail-change'{$after='B'*64}'wrong-state'{$report.state='complete'}}
 $caught=$false;try{& $assertion}catch{$caught=$true};if(!$caught){throw "Repeat admitted $mode"}
 "PASS actual repeat assertion refuses $mode"
}
. "$lab/compact9-runtime-v1/compact9_receipts.ps1"
$script:beforeRows=@('Config|a|0|creation|modified|0|1|1|A','ConfigSave|s|0|creation|modified|0|1|1|B','Params|p|0|creation|modified|0|1|1|C','Files|f|0|creation|modified|0|1|1|D');$script:afterRows=@($beforeRows);$script:comparison=$null
function Get-Content {param($LiteralPath);if($LiteralPath.Contains('-before-')){$script:beforeRows}else{$script:afterRows}}
function Get-FileHash {param($Path);[pscustomobject]@{Hash=('A'*64)}}
function Test-Path {param($LiteralPath);return $false}
function Set-Content {param($LiteralPath,[Parameter(ValueFromPipeline)]$Value);process{$script:comparison=$Value|ConvertFrom-Json}}
Compare-Compact9Storage $lab 'pure' 'before' 'after'
foreach($table in @('Config','ConfigSave','Params')){
 $script:afterRows=@($beforeRows|ForEach-Object {if($_.StartsWith($table+'|')){$_.Replace('|creation|','|foreign-creation|')}else{$_}})
 $caught=$false;try{Compare-Compact9Storage $lab 'pure' 'before' 'after'}catch{$caught=$true};if(!$caught){throw 'Header drift accepted'}
 "PASS actual fullheader refusal/repeat compare catches $table Creation drift"
}
$script:afterRows=@($beforeRows|ForEach-Object {if($_.StartsWith('Files|')){$_.Replace('|modified|','|operational-change|')}else{$_}})
Compare-Compact9Storage $lab 'pure' 'before' 'after'
if($comparison.tables.Files.exact_all_headers_and_data_sha_equal -ne $false){throw 'Operational Files drift falsely exact'}
'PASS operational Files difference retained explicitly, no false full equality'
$start=$ast.Find({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq 'StartCohort'},$true)
$statements=$start.Body.EndBlock.Statements
$readiness=($statements[($statements.Count-3)..($statements.Count-1)]|ForEach-Object {$_.Extent.Text}) -join "`n"
if(!$readiness.Contains('AddSeconds(30)') -or !$readiness.Contains('-lt 5')){throw 'Actual N3 bounded real receipt threshold changed'}
# Only shorten the clock to zero for a mocked negative; predicate is unchanged.
$readiness=$readiness.Replace('AddSeconds(30)','AddSeconds(0)')
$script:owned=@('a','b','c'|ForEach-Object {@{label=$_}});$script:counts=@{a=5;b=5;c=5};$script:seen=@()
function Get-Compact9ConfirmedReceipts($Path,$Label){$script:seen+=@($Label);return $script:counts[$Label]}
function Start-Sleep {param($Milliseconds)}
& ([scriptblock]::Create($readiness))
if(($seen -join ',') -cne 'a,b,c'){throw 'N3 predicate skipped a cohort'}
$script:counts.c=4;$caught=$false;try{& ([scriptblock]::Create($readiness))}catch{$caught=$true};if(!$caught){throw 'Under-threshold third cohort admitted'}
'PASS actual bounded N3 readiness requires at least5 confirmed document+report receipts per client; third client4 refused'
'ALL PASS postcondition mocks; realStarts=realSignals=SQLcalls=registryWrites=0; no runtime/readiness acceptance claim'
