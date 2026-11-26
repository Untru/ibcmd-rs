# New9 only. Measured immutable directory inventory; no ownership/signalling authority.
$script:Compact9TreeRoot='F:\ibcmd\lab\05\wave3\load\evidence\compact6-case5-prior'
function Read-Compact9CapturedArchive {
 $path='F:\ibcmd\lab\05\wave3\load\compact8-prior-archive-census-v1.json'
 [void](Assert-Compact9ArchivePath $path)
 $meta=Get-Item -LiteralPath $path -Force
 if($meta -isnot [IO.FileInfo] -or $meta.Length -ne 1491071 -or $meta.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'captured tree metadata changed'}
 $stream=[IO.File]::Open($path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
 try{$buffer=[byte[]]::new(2097153);$n=0;while($n -lt $buffer.Length){$r=$stream.Read($buffer,$n,$buffer.Length-$n);if(!$r){break};$n+=$r};if($n -ne 1491071){throw 'captured tree bounded bytes changed'}
  $bytes=[byte[]]::new($n);[Array]::Copy($buffer,$bytes,$n);if([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)) -cne '8E1B6EBF466878BE2C69420C24E0A3A904BF7C86FBF5E8B536A4B2FC4B055D1B'){throw 'captured tree SHA changed'}
  $c=[Text.UTF8Encoding]::new($false,$true).GetString($bytes)|ConvertFrom-Json -DateKind String
 }finally{$stream.Dispose()}
 if($c.total_nodes -ne 5485 -or $c.ordinary_files -ne 597 -or $c.directories_including_root -ne 4888 -or $c.directories.Count -ne 4888 -or $c.verified_prior613.Count -ne 613 -or !$c.no_reparse -or !$c.exact597_file_set -or !$c.all613_bytes_equal){throw 'captured measured layout scope changed'}
 return $c
}
function Get-Compact9TreeEnumerator([string]$Path){return ,[IO.Directory]::EnumerateFileSystemEntries($Path).GetEnumerator()}
function Get-Compact9TreeAttributes([string]$Path){[IO.File]::GetAttributes($Path)}
function Assert-Compact9TreePathSyntax([string]$Path){
 if(![IO.Path]::IsPathFullyQualified($Path) -or [IO.Path]::GetFullPath($Path) -cne $Path -or ($Path -cne $script:Compact9TreeRoot -and !$Path.StartsWith($script:Compact9TreeRoot+'\',[StringComparison]::Ordinal))){throw 'tree canonical containment'}
}
function Assert-Compact9CapturedArchiveTree {
 $deadline=[DateTime]::UtcNow.AddSeconds(90)
 $capture=Read-Compact9CapturedArchive
 [void](Assert-Compact9ArchivePath $script:Compact9TreeRoot)
 $expectedDirs=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
 foreach($d in $capture.directories){Assert-Compact9TreePathSyntax $d.path;if(!$expectedDirs.Add($d.path)){throw 'captured duplicate directory'}}
 $expectedFiles=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
 foreach($e in $capture.verified_prior613){if($e.file.StartsWith($script:Compact9TreeRoot+'\',[StringComparison]::Ordinal)){Assert-Compact9TreePathSyntax $e.file;if(!$expectedFiles.Add($e.file)){throw 'captured duplicate file'}}}
 if(!$expectedDirs.Contains($script:Compact9TreeRoot) -or $expectedFiles.Count -ne 597){throw 'captured exact root/files absent'}
 $seenDirs=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal);$seenFiles=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
 $pending=[Collections.Generic.Stack[string]]::new();$pending.Push($script:Compact9TreeRoot);$discovered=1;$limit=65536
 while($pending.Count){
  $path=$pending.Pop();if([DateTime]::UtcNow -gt $deadline){throw 'tree metadata deadline'}
  Assert-Compact9TreePathSyntax $path
  $attrs=Get-Compact9TreeAttributes $path
  if($attrs -band [IO.FileAttributes]::ReparsePoint -or !($attrs -band [IO.FileAttributes]::Directory) -or !$expectedDirs.Contains($path) -or !$seenDirs.Add($path)){throw 'tree directory foreign/reparse/duplicate'}
  # Direct .NET lazy enumeration: enforce discovery limit BEFORE child metadata allocation/push.
  $iterator=Get-Compact9TreeEnumerator $path
  try{while($iterator.MoveNext()){
   $child=[string]$iterator.Current
   $discovered++;if($discovered -gt $limit){throw 'tree discovery budget'}
   if([DateTime]::UtcNow -gt $deadline){throw 'tree metadata deadline'}
   Assert-Compact9TreePathSyntax $child
   if([IO.Path]::GetDirectoryName($child) -cne $path){throw 'tree immediate child containment'}
   $attrs=Get-Compact9TreeAttributes $child
   if($attrs -band [IO.FileAttributes]::ReparsePoint){throw 'tree reparse child'}
   if($attrs -band [IO.FileAttributes]::Directory){if(!$expectedDirs.Contains($child)){throw 'tree foreign directory'};$pending.Push($child)}
   elseif(!$expectedFiles.Contains($child) -or !$seenFiles.Add($child)){throw 'tree foreign/duplicate file'}
  }}finally{$iterator.Dispose()}
 }
 if($discovered -ne 5485 -or $seenDirs.Count -ne 4888 -or $seenFiles.Count -ne 597 -or !$seenDirs.SetEquals($expectedDirs) -or !$seenFiles.SetEquals($expectedFiles)){throw 'tree exact measured membership changed'}
 foreach($e in $capture.verified_prior613){
  if([DateTime]::UtcNow -gt $deadline){throw 'tree raw verification deadline'}
  [void](Assert-Compact9ArchivePath $e.file)
  if((Get-Item -LiteralPath $e.file -Force).Length -ne $e.bytes -or (Get-FileHash -LiteralPath $e.file).Hash -cne $e.sha256){throw 'tree captured613 bytes changed'}
 }
 return [pscustomobject]@{total_nodes=$discovered;directories=$seenDirs.Count;ordinary_files=$seenFiles.Count;captured613_bytes_equal=$true;no_reparse=$true;signal_authority=$false;ownership_authority=$false;archive_moves=0}
}
