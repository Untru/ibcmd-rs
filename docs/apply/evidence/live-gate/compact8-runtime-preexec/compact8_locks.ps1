# New7 only. Shared Trackload requires a known successful acquire and exact
# raw lease capture; a nonzero result never grants release authority.
$script:Compact8RawBounded=${function:Invoke-LiveBounded}
if(!(Get-Variable Compact8ResourceUnconfirmed -Scope Script -ErrorAction SilentlyContinue)){$script:Compact8ResourceUnconfirmed=$false}
if(!(Get-Variable Compact8Leases -Scope Script -ErrorAction SilentlyContinue)){$script:Compact8Leases=@{}}
function Read-Compact8LockOwner([ValidateSet('worker','heavy','native')][string]$Name){
 $root='F:\ibcmd\lab\04\locks';$path=Join-Path $root $Name
 for($probe=$path;$probe;$probe=[IO.Path]::GetDirectoryName($probe)){if((Test-Path -LiteralPath $probe) -and ((Get-Item -LiteralPath $probe -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'lock reparse ancestry refused'}}
 if(!(Test-Path -LiteralPath $path)){return [pscustomobject]@{kind='absent';bytes=$null}}
 if([IO.Path]::GetFullPath((Get-Item -LiteralPath $path -Force).FullName) -cne $path){throw 'exact lock containment mismatch'}
 $entries=@(Get-ChildItem -LiteralPath $path -Force|Select-Object -First 2)
 if($entries.Count -ne 1 -or $entries[0].Name -cne 'owner.txt' -or $entries[0] -isnot [IO.FileInfo] -or ($entries[0].Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'exact single ordinary owner.txt required; no recursive release of extra members'}
 $owner=Join-Path $path 'owner.txt';if((Get-Item -LiteralPath $owner).Length -gt 4096){throw 'lock owner exceeds budget'}
 $f=[IO.File]::Open($owner,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
 try{$b=[byte[]]::new(4097);$n=0;while($n -lt $b.Length){$r=$f.Read($b,$n,$b.Length-$n);if(!$r){break};$n+=$r};if(!$n -or $n -gt 4096){throw 'lock owner bounded read'};$b=[byte[]]$b[0..($n-1)]}finally{$f.Dispose()}
 if(@($b|Where-Object {$_ -gt 127}).Count){throw 'lock owner nonASCII'}
 $text=[Text.Encoding]::ASCII.GetString($b)
 if($text -cnotmatch '\Atrack=([A-Za-z0-9-]+) since=\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\r?\n)?\z'){throw 'lock owner malformed'}
 [pscustomobject]@{kind=$(if($Matches[1] -ieq 'load8-f8bd7d065a2c'){'same_track'}else{'foreign'});bytes=$b;sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($b))}
}
function Save-Compact8LockProof($State){
 $root='F:\ibcmd\lab\05\wave3\load\compact8-lock-proofs'
 for($probe=$root;$probe;$probe=[IO.Path]::GetDirectoryName($probe)){if((Get-Item -LiteralPath $probe -Force).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'lock proof reparse ancestry'}}
 $State.sequence++
 $bytes=[Text.Encoding]::UTF8.GetBytes(($State|ConvertTo-Json -Depth 4 -Compress))
 if($bytes.Length -gt 8192){throw 'lock proof budget'}
 $f=[IO.File]::Open((Join-Path $root ('lock-'+[guid]::NewGuid().ToString('N')+'.json')),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
 try{$f.Write($bytes);$f.Flush($true)}finally{$f.Dispose()}
}
function Test-Compact8ResourceUnconfirmed {
 if($script:Compact8ResourceUnconfirmed){return $true}
 $root='F:\ibcmd\lab\05\wave3\load\compact8-lock-proofs'
 for($probe=$root;$probe;$probe=[IO.Path]::GetDirectoryName($probe)){if((Get-Item -LiteralPath $probe -Force).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'lock proof reparse ancestry; retain lifecycle'}}
 $files=@(Get-ChildItem -LiteralPath $root -Force|Select-Object -First 513);if($files.Count -gt 512){throw 'lock proof inventory budget; retain lifecycle'}
 $proofs=@()
 foreach($file in $files){
  if($file -isnot [IO.FileInfo] -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -or $file.Length -gt 8192){throw 'lock proof file/budget; retain lifecycle'}
  $f=[IO.File]::Open($file.FullName,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
  try{$b=[byte[]]::new(8193);$n=0;while($n -lt $b.Length){$r=$f.Read($b,$n,$b.Length-$n);if(!$r){break};$n+=$r};if(!$n -or $n -gt 8192){throw 'lock proof bounded read'};$text=[Text.UTF8Encoding]::new($false,$true).GetString($b,0,$n)}finally{$f.Dispose()}
  $p=$text|ConvertFrom-Json
  $proofs+=@($p)
 }
 return Get-Compact8ResourceUnconfirmedFromProofs $proofs
}
function Get-Compact8ResourceUnconfirmedFromProofs($Proofs){
 $latest=@{};$seen=@{}
 foreach($p in $Proofs){
  if($p.operation -cnotmatch '^[0-9a-f]{32}$' -or ($p.sequence -isnot [long] -and $p.sequence -isnot [int]) -or $p.sequence -lt 1 -or $p.name -cnotin @('worker','heavy','native') -or $p.potential_held -isnot [bool] -or $p.released -isnot [bool] -or $p.acquire_completed -isnot [bool]){throw 'lock proof identity malformed; retain lifecycle'}
  $key=$p.operation+':'+$p.sequence
  if($seen.ContainsKey($key)){throw 'duplicate lock proof sequence; retain lifecycle'};$seen[$key]=$true
  if(!$latest.ContainsKey($p.operation) -or $p.sequence -gt $latest[$p.operation].sequence){$latest[$p.operation]=$p}
 }
 foreach($p in $latest.Values){
  if($p.potential_held -and !$p.released -and ($p.name -ceq 'native' -or !$p.acquire_completed -or $p.acquire_exit -ne 0 -or !$p.lease -or $p.failure)){return $true}
 }
 return $false
}
function Invoke-LiveBounded {
 param([string]$Executable,[string[]]$Arguments,[int]$TimeoutSeconds=30)
 $index=[Array]::IndexOf($Arguments,'-File')
 if($index -lt 0 -or $Arguments.Count -le $index+1 -or $Arguments[$index+1] -cnotin @('F:/ibcmd/lab/04/tools/heavy-lock.ps1','F:\ibcmd\lab\04\tools\heavy-lock.ps1')){return & $script:Compact8RawBounded $Executable $Arguments $TimeoutSeconds}
 $action=$Arguments[$index+2];$track=$Arguments[$index+3];$nameIndex=[Array]::IndexOf($Arguments,'-Name')
 if($track -cne 'load8-f8bd7d065a2c' -or $action -cnotin @('acquire','release') -or $nameIndex -lt 0 -or $Arguments.Count -le $nameIndex+1 -or $Arguments[$nameIndex+1] -cnotin @('worker','heavy','native')){throw 'exact new7 lock command required'}
 $name=$Arguments[$nameIndex+1]
 if($action -ceq 'acquire'){
  if(Test-LiveUncertainChild){throw 'unknown prior child; no acquire'}
  $entry=[ordered]@{operation=[guid]::NewGuid().ToString('N');sequence=0;name=$name;potential_held=$true;acquire_completed=$false;acquire_exit=$null;lease=$null;released=$false;failure=$null}
  $script:Compact8Leases[$name]=$entry
  try{
   $before=Read-Compact8LockOwner $name
   if($before.kind -ceq 'same_track'){throw 'preexisting SAME Trackload lock refused before dispatch'}
   $result=& $script:Compact8RawBounded $Executable $Arguments $TimeoutSeconds
   $entry.acquire_completed=$true;$entry.acquire_exit=$result.ExitCode
   if(Test-LiveUncertainChild){throw 'acquire child completion uncertain'}
   if($result.ExitCode -eq 0){$current=Read-Compact8LockOwner $name;if($current.kind -cne 'same_track'){throw 'successful acquire has no exact Trackload lease'};$entry.lease=[Convert]::ToBase64String($current.bytes)}
   return $result
  }catch{$entry.failure=$_.Exception.Message;throw ('COMPACT8_RESOURCE_UNCONFIRMED '+$entry.failure)}
  finally{try{Save-Compact8LockProof $entry}catch{throw ('COMPACT8_RESOURCE_UNCONFIRMED proof publication failed: '+$_.Exception.Message)}}
 }
 $entry=$script:Compact8Leases[$name]
 if(!$entry -or !$entry.acquire_completed -or $entry.acquire_exit -ne 0 -or !$entry.lease){throw 'unbound/nonzero/unknown acquire; potential lock retained, no release'}
 try{
  if($name -cne 'heavy' -and (Test-LiveUncertainChild)){throw 'unknown child; native/worker retained'}
  $current=Read-Compact8LockOwner $name
  if($current.kind -cne 'same_track' -or [Convert]::ToBase64String($current.bytes) -cne $entry.lease){throw 'exact captured lease changed; no release'}
  $result=& $script:Compact8RawBounded $Executable $Arguments $TimeoutSeconds
  if(Test-LiveUncertainChild){throw 'release child completion uncertain; potential lock retained'}
  if($result.ExitCode -ne 0){throw 'release completed nonzero; potential lock retained for exact proof'}
  $after=Read-Compact8LockOwner $name
  if($after.kind -ceq 'same_track'){throw 'Trackload owner remains after release'}
  $entry.released=$true;$entry.potential_held=$false
  return $result
 }catch{$entry.failure=$_.Exception.Message;throw ('COMPACT8_RESOURCE_UNCONFIRMED '+$entry.failure)}
 finally{try{Save-Compact8LockProof $entry}catch{throw ('COMPACT8_RESOURCE_UNCONFIRMED proof publication failed: '+$_.Exception.Message)}}
}
