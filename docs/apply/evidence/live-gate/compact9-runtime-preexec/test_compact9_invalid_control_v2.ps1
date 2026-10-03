$ErrorActionPreference='Stop'
function Assert-Compact9RuntimeClosure {}
$lab='F:/ibcmd/lab/05/wave3/load';$prefix='pure';$bin='never-spawn';$db='mock';$label='pure-r1';$script:held=$false;$script:children=0;$script:releases=0;$script:uncertain=$false
$t=$null;$e=$null;$ast=[Management.Automation.Language.Parser]::ParseFile("$lab/compact9-runtime-v1/compact9_checkpoint_v2.ps1",[ref]$t,[ref]$e);if($e.Count){throw 'parser'}
$def=@($ast.FindAll({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq 'InvokeInvalidContinuation'},$true));if($def.Count -ne 1){throw 'function'};Invoke-Expression $def[0].Extent.Text
function Test-LiveUncertainChild{return $script:uncertain}
function Run($name,$exe,$argv,$seconds){
 if($argv -contains 'acquire'){if($script:held){throw 'overlap'};$script:held=$true;return [pscustomobject]@{ExitCode=0}}
 if($exe -ceq 'python'){return [pscustomobject]@{ExitCode=0}}
 if(!$script:held -or $argv -notcontains 'mssql-live-continue' -or $argv -notcontains '127.0.0.1:1' -or $argv -notcontains 'must-not-spawn-rac'){throw 'unprotected/connected negative command'}
 $script:children++;$key=$name.Substring('pure-invalid-'.Length);$errors=@{'corrupt-pack'='recovery pack length/digest mismatch';'missing-pack'='cannot open ibcmd-recovery-proof.pack';'wrong-envelope-digest'='compact LIVE envelope integrity mismatch';'wrong-sidecar-digest'='compact LIVE recovery sidecar digest differs';'foreign-sidecar-name'='compact LIVE recovery basename differs'}
 return [pscustomobject]@{ExitCode=1;Stderr=$errors[$key];Stdout=''}
}
function Invoke-LiveBounded($Executable,$Arguments,$TimeoutSeconds){if(!$script:held -or $Arguments -notcontains 'release' -or $TimeoutSeconds -ne 15){throw 'release'};$script:held=$false;$script:releases++;[pscustomobject]@{ExitCode=0}}
function Get-FileHash {param($LiteralPath);[pscustomobject]@{Hash=('A'*64)}}
function Get-Content {param($LiteralPath,[switch]$Raw);$cases=@('corrupt-pack','missing-pack','wrong-envelope-digest','wrong-sidecar-digest','foreign-sidecar-name')|ForEach-Object{@{name=$_;artifact="$lab/compact9-invalid-actual/$_/invalid.live.json";sha256=('a'*64)}};@{cases=@($cases);originals_sha256=@{"$lab/original.live.json"=('a'*64)}}|ConvertTo-Json -Depth 6}
$source=[IO.File]::ReadAllText("$lab/compact9-runtime-v1/compact9_checkpoint_v2.ps1");$start=$source.IndexOf(' $planPath=');$end=$source.IndexOf(" StartCohort 'new'",$start);if($start -lt 0 -or $end -lt 0){throw 'actual block'}
Invoke-Expression $source.Substring($start,$end-$start)
if($script:children -ne 5 -or $script:releases -ne 5 -or $script:held){throw 'negative block FIFO/coverage'}
'PASS actual negative block all5 unavailable endpoints/exact input refusals under FIFO; original SHA checks; realStarts=realSignals=SQLcalls=registryWrites=0'
$script:uncertain=$true;$caught=$false;try{InvokeInvalidContinuation 'never' 'never'|Out-Null}catch{$caught=$_.Exception.Message -match 'unresolved prior child'};if(!$caught -or $script:children -ne 5){throw 'uncertain child allowed new command'}
'PASS prior unknown child refuses negative execution before acquire/spawn'
