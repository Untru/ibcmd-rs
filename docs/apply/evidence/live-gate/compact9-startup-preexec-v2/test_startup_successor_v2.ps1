$ErrorActionPreference='Stop'
function Actual([string]$Path,[string]$Name){
 $errors=$null;$ast=[Management.Automation.Language.Parser]::ParseFile($Path,[ref]$null,[ref]$errors)
 if($errors.Count){throw ($errors|Out-String)}
 $f=@($ast.FindAll({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq $Name},$false));if($f.Count -ne 1){throw 'function unavailable'};$f[0].Extent.Text.Replace("`r`n","`n")
}
$old='F:/ibcmd/lab/05/wave3/load/compact9-runtime-v1';$new=$PSScriptRoot
foreach($name in @('Private-Key','Private-Identity','Private-Same','Private-Owned','Private-RequireListeners','Private-RequireLease')){
 if((Actual "$old/compact9-tools/cluster/private-lib.ps1" $name) -cne (Actual "$new/compact9-tools/cluster/private-lib.ps1" $name)){throw 'ownership/stop admission delta'}
}
foreach($name in @('Private-ListenerRefusal','Private-StartupListeners','Private-StartupObservation')){
 if((Actual 'F:/ibcmd/src/ibcmd-rs-05-load-wave3/scripts/apply-lab/cluster/private-lib.ps1' $name) -cne (Actual "$new/compact9-tools/cluster/private-lib.ps1" $name)){throw 'reviewed diagnostic function differs'}
}
$before=[IO.File]::ReadAllText("$old/compact9_lifetime_v2.ps1").Replace('compact9-runtime-v1','compact9-runtime-v2').Replace('compact9-runtime-frozen-v1','compact9-runtime-frozen-v2')
$expected=$before.Replace("'-TimeoutSec','60') 180","'-TimeoutSec','120') 180")
$actual=[IO.File]::ReadAllText("$new/compact9_lifetime_v2.ps1")
if($expected.Replace("`r`n","`n") -cne $actual.Replace("`r`n","`n")){throw 'lifetime delta beyond route/startup120'}
$ast=[Management.Automation.Language.Parser]::ParseFile("$new/compact9_lifetime_v2.ps1",[ref]$null,[ref]$null)
$calls=@($ast.FindAll({param($n)$n -is [Management.Automation.Language.CommandAst] -and $n.GetCommandName() -ceq 'Run' -and $n.Extent.Text -match 'cluster/start.ps1'},$true))
if($calls.Count -ne 1 -or $calls[0].Extent.Text -notmatch "'-TimeoutSec','120'\) 180$"){throw 'actual caller budget boundary'}
if([IO.File]::ReadAllText("$new/compact9-tools/cluster/private-start.ps1") -notmatch 'ValidateRange\(5,120\)'){throw 'existing startup maximum changed'}
foreach($name in @('Stop-ClusterProcesses','Private-RequireNames')){if((Actual "$old/compact9-tools/cluster/private-lib.ps1" $name) -cne (Actual "$new/compact9-tools/cluster/private-lib.ps1" $name)){throw 'unrelated authority delta'}}
'PASS reviewed diagnostic AST exact; existing ownership/immediate-stop/registry predicates exact; actual lifetime diff only V2 routing + caller120 under same180 outer bound; no runtime actions'
