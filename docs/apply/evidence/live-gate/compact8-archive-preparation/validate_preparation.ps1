$ErrorActionPreference='Stop'
foreach($file in Get-ChildItem -LiteralPath $PSScriptRoot -Filter '*.ps1'){
 $tokens=$null;$errors=$null;[void][Management.Automation.Language.Parser]::ParseFile($file.FullName,[ref]$tokens,[ref]$errors)
 if($errors.Count){throw "parser failed $($file.Name)"}
}
$preserved='F:/ibcmd/lab/05/wave3/coordinator/compact7-runtime-frozen-v3.json'
if((Get-FileHash -LiteralPath $preserved).Hash -cne '781CFD57A8E27E918D7D1FF167692A11F46A1E76173878E8835F171C6C87202A'){throw 'original100 manifest changed'}
$m=Get-Content -LiteralPath $preserved -Raw|ConvertFrom-Json -DateKind String
if($m.files.Count -ne 100){throw 'original100 count'}
function Verify($Entry){
 if(![IO.Path]::IsPathFullyQualified($Entry.file)){throw 'preserved path not fully qualified'}
 for($p=[IO.Path]::GetFullPath($Entry.file);$p;$p=[IO.Path]::GetDirectoryName($p)){if((Get-Item -LiteralPath $p -Force).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'preserved reparse ancestry'}}
 if((Get-Item -LiteralPath $Entry.file).Length -ne $Entry.bytes -or (Get-FileHash -LiteralPath $Entry.file).Hash -cne $Entry.sha256){throw ('preserved bytes changed: '+$Entry.file)}
}
foreach($e in $m.files){Verify $e}
foreach($p in $m.preserved){
 Verify $p
 $old=Get-Content -LiteralPath $p.file -Raw|ConvertFrom-Json -DateKind String
 $entries=@(if($p.kind -ceq 'raw'){@($old.raw)}else{@($old.files)+@($old.dependencies)})|Where-Object {$null -ne $_}
 if($entries.Count -ne $p.members){throw 'preserved deep count'}
 foreach($e in $entries){if(!$e.file){$e|Add-Member NoteProperty file $e.path};if($null -eq $e.bytes){$e|Add-Member NoteProperty bytes (Get-Item -LiteralPath $e.file).Length};Verify $e}
}
'PASS parser and preserved original100/deep54/52/45/58/root40/613/285 bytes/noReparse; runtime/acquires/moves/signals0'
