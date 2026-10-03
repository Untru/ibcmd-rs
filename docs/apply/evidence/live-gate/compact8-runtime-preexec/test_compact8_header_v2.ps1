$ErrorActionPreference='Stop'
function Assert-Compact8RuntimeClosure {}
$lab='F:/ibcmd/lab/05/wave3/load';$db='ibcmd_rs_05_load_w3_compact8_20261002';$prefix='pure-header'
$live=[IO.File]::ReadAllText("$lab/compact5-load-5-r1.recovery.live.json")|ConvertFrom-Json
$live.payload.identity.database=$db
$script:envelope=$live|ConvertTo-Json -Depth 30
$saved=[IO.File]::ReadAllText("$lab/logs/compact5-load-5-header-1.result.json")|ConvertFrom-Json
$actual=@($saved.Stdout -split '\r?\n'|Where-Object{$_ -like 'ibcmd-rs:live:*'})
if($actual.Count -ne 1){throw 'expected actual retained single header'}
$script:first=$actual[0];$script:output='';$script:calls=0
function ExtractHeader($path){$t=$null;$e=$null;$ast=[Management.Automation.Language.Parser]::ParseFile($path,[ref]$t,[ref]$e);if($e.Count){throw 'parser'};$f=@($ast.FindAll({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq 'Header'},$true));if($f.Count -ne 1){throw 'actual Header function'};return $f[0].Extent.Text}
Invoke-Expression ((ExtractHeader "$lab/compact6_checkpoint.ps1").Replace('function Header(', 'function HeaderOriginal('))
Invoke-Expression (ExtractHeader "$lab/compact8-runtime-v1/compact8_checkpoint_v2.ps1")
function Run($name,$exe,$argv){$script:calls++;if($exe -cne 'sqlcmd'){throw 'unexpected mock command'};[pscustomobject]@{ExitCode=0;Stdout=$script:output;Stderr=''}}
function Get-Content {param($Path,[switch]$Raw);if($Path -cne "$lab/mock.recovery.live.json"){throw 'unexpected mock artifact read'};return $script:envelope}
function Output($sets,$index=-1,$row=0,$value=''){
 $rows=@();for($i=0;$i -lt $sets;$i++){$fields=$script:first.Split('|');$fields[0]="ibcmd-rs:live:$($live.payload.recovery_token):$($i+1)";if($index -ge 0 -and $row -eq $i){$fields[$index]=$value};$rows+=($fields -join '|')}
 return (@('ONLINE|MULTI_USER|FULL|0')+$rows) -join "`n"
}
$foreign='00000000-0000-0000-0000-000000000000'
$script:output=Output 1 30 0 $foreign
HeaderOriginal 'mock' 1
'RED verified original actual Header admits foreign canonical database GUID'
foreach($sets in @(1,2)){
 $script:output=Output $sets;Header 'mock' $sets
 foreach($index in @(30,31,33)){
  $fields=$script:first.Split('|');$mixed=$fields[$index].Trim().ToLowerInvariant();$mixed=$mixed.Substring(0,8).ToUpperInvariant()+$mixed.Substring(8)
  $script:output=Output $sets $index ($sets-1) $mixed;Header 'mock' $sets
  $script:output=Output $sets $index ($sets-1) $foreign
  $caught=$false;try{Header 'mock' $sets}catch{if($_.Exception.Message -cne 'header identity lineage differs from LIVE envelope'){throw};$caught=$true};if(!$caught){throw 'foreign lineage admitted'}
  "PASS actual Header sets=$sets index=$index foreign canonical UUID refused; matching mixedcase accepted"
 }
}
$script:output=Output 1 31 0 'not-a-guid';$caught=$false;try{Header 'mock' 1}catch{$caught=$true};if(!$caught){throw 'malformed UUID admitted'}
$script:output=Output 2;$caught=$false;try{Header 'mock' 1}catch{$caught=$true};if(!$caught){throw 'wrong count admitted'}
$script:output=(Output 1).Replace(':1|',':2|');$caught=$false;try{Header 'mock' 1}catch{$caught=$true};if(!$caught){throw 'wrong order admitted'}
$bad=$live|ConvertTo-Json -Depth 30|ConvertFrom-Json;$bad.payload.identity.database='ibcmd_rs_05_load_w3_compact5_20261001';$script:envelope=$bad|ConvertTo-Json -Depth 30;$script:output=Output 1
$caught=$false;try{Header 'mock' 1}catch{$caught=$true};if(!$caught){throw 'foreign envelope database admitted'}
'PASS actual Header malformed/count/order/foreignDB refused; realStarts=realSignals=SQLcalls=registryWrites=Moves=0'
