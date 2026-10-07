# Execute original actual Child on a virtual filesystem; zero real actions.
$ErrorActionPreference='Stop';$tokens=$null;$errors=$null
$ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'preserve_extension_settled_A.ps1'),[ref]$tokens,[ref]$errors)
if($errors.Count){throw 'actual preservation parser'}
$fn=@($ast.FindAll({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq 'Child'},$true));if($fn.Count -ne 1){throw 'actual Child missing'}
. ([scriptblock]::Create($fn[0].Extent.Text))
$run='F:\ibcmd\lab\05\wave3\platform85\tmp\virtual-header-label-v5'
function OwnerContext {}
function Test-LiveUncertainChild {$false}
function Test-Path {param($LiteralPath);$script:files.ContainsKey($LiteralPath)}
function Set-Content {param($LiteralPath,$Encoding,[Parameter(ValueFromPipeline)]$Value);process{$script:files[$LiteralPath]=[string]$Value}}
function Invoke-LiveBounded {param($Executable,$Arguments,$TimeoutSeconds)
 $script:files[$script:payload]='[{"DatabaseName":"owned","IsCopyOnly":true}]'
 @{ExitCode=0;Stdout='';Stderr=''}
}
$script:files=@{};$script:payload=Join-Path $run 'backup-header.json'
[void](Child 'backup-header' 'virtual-python' @() 30)
if(($script:files[$script:payload]|ConvertFrom-Json).PSObject.Properties.Name -notcontains 'ExitCode'){throw 'RED failed to reproduce actual overwritten header'}
$script:files=@{};$script:payload=Join-Path $run 'backup-header-rows-readback-v5.json'
[void](Child 'backup-header-readback-v5' 'virtual-python' @() 30)
$rows=$script:files[$script:payload]|ConvertFrom-Json
if(@($rows).Count -ne 1 -or $rows[0].DatabaseName -cne 'owned' -or !$rows[0].IsCopyOnly){throw 'GREEN failed to retain separate payload'}
if(!($script:files[(Join-Path $run 'backup-header-readback-v5.json')]|ConvertFrom-Json).PSObject.Properties['ExitCode']){throw 'separate result receipt missing'}
'PASS actual Child RED collision reproduced; GREEN distinct payload/result retained; real filesystem/process/SQL actions=0'
