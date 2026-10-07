# New runtime successor only. No acquisition or native work in these functions.
function Assert-Compact7RuntimePath([string]$Path){
 if(![IO.Path]::IsPathFullyQualified($Path)){throw 'runtime fully-qualified path required'}
 $full=[IO.Path]::GetFullPath($Path)
 for($probe=$full;$probe;$probe=[IO.Path]::GetDirectoryName($probe)){
  $item=Get-Item -LiteralPath $probe -Force
  if($item.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'runtime reparse ancestry refused'}
 }
 return $full
}
function Read-Compact7RuntimeJson([string]$Path){
 [void](Assert-Compact7RuntimePath $Path)
 $meta=Get-Item -LiteralPath $Path -Force
 if($meta -isnot [IO.FileInfo] -or $meta.Length -gt 1048576){throw 'runtime JSON metadata budget'}
 $stream=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
 try{$buffer=[byte[]]::new(1048577);$n=0;while($n -lt $buffer.Length){$r=$stream.Read($buffer,$n,$buffer.Length-$n);if(!$r){break};$n+=$r};if(!$n -or $n -gt 1048576){throw 'runtime JSON bounded read'};$text=[Text.UTF8Encoding]::new($false,$true).GetString($buffer,0,$n)}finally{$stream.Dispose()}
 return ($text|ConvertFrom-Json -DateKind String)
}
function Assert-Compact7RuntimeEntry($Entry){
 if($Entry.sha256 -cnotmatch '^[0-9A-F]{64}$' -or $Entry.bytes -lt 0 -or $Entry.bytes -gt 2147483648){throw 'runtime file identity/budget'}
 $full=Assert-Compact7RuntimePath $Entry.file
 $meta=Get-Item -LiteralPath $full -Force
 if($meta -isnot [IO.FileInfo] -or $meta.Length -ne $Entry.bytes -or (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash -cne $Entry.sha256){throw ('runtime frozen file changed: '+$full)}
}
function Assert-Compact7RuntimeClosure([switch]$RetainedEvidence){
 $path='F:/ibcmd/lab/05/wave3/coordinator/compact7-runtime-frozen-v3.json'
 $expected=$env:IBCMD_RS_COMPACT7_RUNTIME_MANIFEST_SHA
 if($expected -cnotmatch '^[0-9A-F]{64}$'){throw 'runtime approved manifest SHA required'}
 [void](Assert-Compact7RuntimePath $path)
 if((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -cne $expected){throw 'runtime manifest hash changed'}
 $m=Read-Compact7RuntimeJson $path
 if($m.format -ne 3 -or $m.runtime_executed -ne $false -or $m.database -cne 'ibcmd_rs_05_load_w3_compact7_20261002' -or $m.binary_sha256 -cne '81CC4526E860EDB9412D60ECFBDF2EED71376D40DBE2ADA648D4CCFFD7077AE0'){throw 'runtime exact scope/binary identity'}
 if($m.files.Count -lt 70 -or $m.files.Count -gt 160){throw 'runtime closure file budget'}
 $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
 foreach($e in $m.files){if(!$seen.Add([IO.Path]::GetFullPath($e.file))){throw 'runtime duplicate file'};Assert-Compact7RuntimeEntry $e}
 $required=@('pwsh','python','sqlcmd','git','ibcmd','1cv8','1cv8c','rac','ras','ragent','rmngr','rphost','dbda')
 if($m.executables.Count -lt 13 -or $m.executables.Count -gt 20 -or @($m.executables.role|Sort-Object -Unique).Count -ne $m.executables.Count){throw 'runtime executable pin inventory'}
 foreach($role in $required){if(@($m.executables|Where-Object {$_.role -ceq $role}).Count -ne 1){throw 'runtime required executable absent'}}
 foreach($exe in $m.executables){if(!$seen.Contains([IO.Path]::GetFullPath($exe.file))){throw 'runtime executable outside frozen files'}}
 if([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($env:PATH))) -cne $m.path_sha256){throw 'runtime effective PATH changed'}
 if($m.commands.Count -ne 4 -or @($m.commands.name|Sort-Object -Unique).Count -ne 4){throw 'runtime executable selection inventory'}
 foreach($c in $m.commands){if($c.name -cnotin @('pwsh','python','sqlcmd','git') -or (Get-Command $c.name -CommandType Application|Select-Object -First 1).Source -cne $c.path){throw 'runtime effective executable selection changed'}}
 foreach($v in $m.selection_environment){if([Environment]::GetEnvironmentVariable($v.name) -cne $v.value){throw 'runtime selection environment changed'}}
 if($RetainedEvidence){
  foreach($p in $m.preserved){
   Assert-Compact7RuntimeEntry $p
   $old=Read-Compact7RuntimeJson $p.file
   $entries=@(if($p.kind -ceq 'raw'){@($old.raw)}else{@($old.files)+@($old.dependencies)});$entries=@($entries|Where-Object {$null -ne $_})
   if($entries.Count -ne $p.members -or $entries.Count -gt 1024){throw 'retained runtime inventory changed'}
   foreach($entry in $entries){
    if(!$entry.file){$entry|Add-Member NoteProperty file $entry.path}
    if($null -eq $entry.bytes){$entry|Add-Member NoteProperty bytes (Get-Item -LiteralPath $entry.file -Force).Length}
    Assert-Compact7RuntimeEntry $entry
   }
  }
 }
}
