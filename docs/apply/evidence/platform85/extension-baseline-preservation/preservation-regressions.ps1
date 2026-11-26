# Actual preservation functions and phase statements, with zero real actions.
$ErrorActionPreference='Stop'
$file=Join-Path $PSScriptRoot 'preserve_extension_settled_A.ps1'
$tokens=$null;$errors=$null
$ast=[Management.Automation.Language.Parser]::ParseFile($file,[ref]$tokens,[ref]$errors)
if($errors.Count){throw 'preservation parser'}
foreach($name in @('OwnerContext','Boundary')){
 $n=@($ast.FindAll({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq $name},$true))
 if($n.Count -ne 1){throw 'actual guard missing'}
 . ([scriptblock]::Create($n[0].Extent.Text))
}
$lab='F:\ibcmd\lab\05\wave3\platform85';$db='ibcmd_rs_05_p85_w3_ext_version_native_20261001'
$backup="$lab\baselines\extension-settled-A-20261002.bak";$run="$lab\extension-baseline-preservation-v5"
$helper="$lab\tools\extension-v2-kit\process.ps1";$helperSha='F376181945FB172BC97E27CE90AB7FC77FE4B89D41C9EC21E7C5DFFB62D5BC7D'
$FrozenManifestSha='A'*64;$script:case='clean'
function Test-LiveUncertainChild{return $script:case -ceq 'uncertain'}
function Get-FileHash {param($LiteralPath);@{Hash=$(if($LiteralPath -ceq $helper){if($script:case -ceq 'helper-drift'){'wrong'}else{$helperSha}}else{$FrozenManifestSha})}}
function Get-Item {param($LiteralPath,[switch]$Force);@{Length=1;Attributes=$(if($script:case -ceq 'reparse'){[IO.FileAttributes]::ReparsePoint}else{0})}}
function Get-Content {param($LiteralPath,[switch]$Raw,$Encoding)
 if($LiteralPath -like '*frozen-v5.json'){
  return (@{files=@(@{file=$helper;bytes=1;sha256=$helperSha});executables=@{python='F:\mock\python.exe';sqlcmd='F:\mock\sqlcmd.exe'}}|ConvertTo-Json -Depth 5)
 }
 if($LiteralPath -like '*stopped-state*'){return '{"anchors":[{"pid":13404}],"known":[{"pid":96248}]}'}
 if($script:case -ceq 'unowned'){return 'foreign'}
 $track=if($script:case -ceq 'foreign-track'){'foreign'}else{'p85'}
 $state=if($script:case -ceq 'registered'){'registered'}else{'unregistered'}
 return "$db`t$track`t$state from the private wave3 8.5 worker lab cluster`tdate`tcluster registration"
}
function Get-Command {param($Name,$CommandType);@{Source=$(if($script:case -ceq 'executable-drift'){'F:\foreign.exe'}else{"F:\mock\$Name.exe"})}}
function Test-Path {param($LiteralPath);return (($script:case -ceq 'private-active' -and $LiteralPath -like '*state.json') -or $LiteralPath -ceq $helper)}
function Get-CimInstance {param($ClassName,$Filter)
 if($script:case -ceq 'producer-reuse' -and $Filter){return @{ProcessId=44392}}
 if($script:case -ceq 'saved-process-reuse' -and $Filter -eq 'ProcessId=13404'){return @{ProcessId=13404}}
 if($Filter){return $null}
 if($script:case -ceq 'target-client'){return @{Name='1cv8c.exe';CommandLine="Srvr=localhost;Ref=$db"}}
 if($script:case -ceq 'private-server'){return @{Name='rphost.exe';CommandLine="$lab\cluster"}}
 return @()
}
function Get-NetTCPConnection {param($State,$ErrorAction);if($script:case -ceq 'listener'){return @{LocalPort=6541}};return @()}
OwnerContext
$checks=1
foreach($case in @('uncertain','helper-drift','reparse','unowned','foreign-track','registered','executable-drift','private-active','producer-reuse','saved-process-reuse','target-client','private-server','listener')){
 $script:case=$case;$refused=$false;try{OwnerContext}catch{$refused=$true}
 if(!$refused){throw "actual source guard accepted $case"};$checks++
}
# Execute the actual top-level try BODY; phase mocks only return/throw/count.
$tries=@($ast.FindAll({param($n)$n -is [Management.Automation.Language.TryStatementAst]},$false))
if($tries.Count -ne 1){throw 'one actual preservation phase sequence'}
$text=$tries[0].Body.Extent.Text;$flow=[scriptblock]::Create($text.Substring(1,$text.Length-2))
function Boundary {param($path)}
function Idle {
 $script:idle++;$script:calls.Add('idle')
 if($script:case -ceq 'source-changed' -and $script:idle -eq 2){throw 'actual idle source recheck refused'}
}
function Snapshot {param($tag)
 $script:calls.Add($tag)
 if($script:case -ceq 'baseline-changed' -and $tag -like '*before*'){throw 'full physical baseline differs'}
}
function Child {param($label,$exe,$argv,$seconds)
 $script:calls.Add($label)
 if($label -ceq 'one-copy-only-backup'){
  if($seconds -ne 90 -or $argv[-1] -notmatch 'WITH COPY_ONLY,CHECKSUM,COMPRESSION,BUFFERCOUNT=4,MAXTRANSFERSIZE=65536' -or $argv[-1] -match 'INIT|FORMAT|RESTORE'){throw 'backup command shape'}
  if($script:case -ceq 'unknown-backup'){throw 'sticky unknown child'}
 }
 return @{ExitCode=0}
}
function New-Item {param($ItemType,$Path)}
function Set-Content {param($LiteralPath,$Encoding,[Parameter(ValueFromPipeline)]$Value)}
function Test-Path {param($LiteralPath);return $script:case -ceq 'destination-race' -and $LiteralPath -ceq $backup}
function Get-FileHash {param($LiteralPath);@{Hash='B'*64}}
function Get-Item {param($LiteralPath);@{Length=123}}
foreach($case in @('clean','source-changed','baseline-changed','destination-race','unknown-backup')){
 $script:case=$case;$script:idle=0;$script:calls=[Collections.Generic.List[string]]::new();$failed=$false
 try{& $flow}catch{$failed=$true}
 if($case -ceq 'clean'){
  if($failed -or ($script:calls -join '|') -cne 'idle|ext_version_settled_backup_before_v5|idle|one-copy-only-backup|idle|backup-header|backup-verifyonly|ext_version_settled_backup_after_v5|idle'){throw 'actual positive preservation order'}
 }else{
  if(!$failed){throw "actual phase failed to refuse $case"}
  if($case -cne 'unknown-backup' -and $script:calls.Contains('one-copy-only-backup')){throw 'backup attempted after preflight refusal'}
  if($case -ceq 'unknown-backup' -and $script:calls.Contains('backup-header')){throw 'next child after unknown backup'}
 }
 $checks++
}
"PASS actual source guards and preservation sequence $checks cases; real filesystem/process/SQL/backup/FIFO actions=0"
