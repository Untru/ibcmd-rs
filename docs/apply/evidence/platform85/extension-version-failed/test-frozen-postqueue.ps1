# Execute actual acquisition statements and the actual closure guard with pure mocks.
# No child/process/FIFO/DB/file mutation is dispatched.
$ErrorActionPreference='Stop'
$tokens=$null;$errors=$null
$ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'extension_version_control_v2.ps1'),[ref]$tokens,[ref]$errors)
if($errors.Count){throw 'controller parser refused'}
$guard=@($ast.FindAll({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq 'RequireControlClosure'},$true))
if($guard.Count -ne 1){throw 'one actual closure guard required'}
Invoke-Expression $guard[0].Extent.Text
$life=@($ast.FindAll({param($n) $n -is [Management.Automation.Language.TryStatementAst] -and $n.Body.Extent.Text.Contains('$worker=$true;') -and $n.Body.Extent.Text.Contains('$heavy=$true;')},$true))
if($life.Count -ne 1){throw 'one actual lifetime required'}
$prefix=@()
foreach($statement in $life[0].Body.Statements){
 if($statement.Extent.Text.StartsWith('$script:deadline=')){break}
 $prefix+=,$statement.Extent.Text
}
if($prefix.Count -ne 6 -or $prefix[1] -notmatch 'RunChild worker-acquire' -or $prefix[2] -cne 'RequireControlClosure' -or $prefix[4] -notmatch 'RunChild heavy-acquire' -or $prefix[5] -cne 'RequireControlClosure'){throw 'actual post-grant guard order differs'}
$actual=[scriptblock]::Create(($prefix -join "`n")+"`n`$script:mutationReached=`$true")
$lab='F:\ibcmd\lab\05\wave3\platform85';$lock='mock';$FrozenManifestSha='A'*64
$member=[IO.Path]::GetFullPath("$lab\mock-member.ps1")
$manifest=@{scope='native-only initial extension Version transition';files=@(@{file=$member;length=12;sha256=('B'*64)});executables=@{}}|ConvertTo-Json -Depth 5
function RequireLabPaths85($Paths){}
function Get-Item{param($LiteralPath,[switch]$Force);return [pscustomobject]@{Attributes=[IO.FileAttributes]::Normal;Length=$(if($LiteralPath -ceq $member){12}else{1000})}}
function Get-Content{param($LiteralPath,[switch]$Raw);return $manifest}
function Get-FileHash{param($LiteralPath);return [pscustomobject]@{Hash=$(if($LiteralPath -ceq $member){$script:memberHash}else{$FrozenManifestSha})}}
function RunChild($Label,$Exe,$Arguments,$Seconds){
 $script:acquires+=,$Label
 if($Label -ceq $script:driftAt){$script:memberHash='C'*64}
 return [pscustomobject]@{ExitCode=0}
}
foreach($case in @('','worker-acquire','heavy-acquire')){
 $worker=$false;$heavy=$false;$script:memberHash='B'*64;$script:driftAt=$case;$script:acquires=@();$script:mutationReached=$false;$refused=$false
 try{. $actual}catch{if($_.Exception.Message -cne 'frozen controller dependency/source/executable drift'){throw};$refused=$true}
 if(!$case){if($refused -or !$script:mutationReached -or !$worker -or !$heavy -or $script:acquires.Count -ne 2){throw 'clean grant sequence refused'}}
 else{
  if(!$refused -or $script:mutationReached -or !$worker){throw 'changed hash after grant reached mutation or lost potential worker hold'}
  if($case -ceq 'worker-acquire' -and ($heavy -or $script:acquires.Count -ne 1)){throw 'worker drift continued into heavy acquire'}
  if($case -ceq 'heavy-acquire' -and (!$heavy -or $script:acquires.Count -ne 2)){throw 'heavy drift lost potential hold'}
 }
}
'PASS real closure hash refusal after worker/heavy grant and clean actual statement order, 3 cases; runtime/file/process/FIFO/DB actions=0'
