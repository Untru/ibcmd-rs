# New8 only. Read-only child authority bound to the successful parent acquire.
$script:Compact9ArchiveTrack='load9-983ad8f3eae1'
$script:Compact9ProofRoot='F:\ibcmd\lab\05\wave3\load\compact9-lock-proofs'
function Read-Compact9ArchiveBounded([string]$Path,[int]$Limit){
 if($Limit -notin @(128,8192) -or ![IO.Path]::IsPathFullyQualified($Path) -or [IO.Path]::GetFullPath($Path) -cne $Path){throw 'archive worker exact path/budget'}
 for($p=$Path;$p;$p=[IO.Path]::GetDirectoryName($p)){if((Get-Item -LiteralPath $p -Force).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'archive worker reparse ancestry'}}
 $item=Get-Item -LiteralPath $Path -Force
 if($item -isnot [IO.FileInfo] -or $item.Length -lt 1 -or $item.Length -gt $Limit){throw 'archive worker metadata budget before allocation'}
 $stream=[IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
 try{$buffer=[byte[]]::new($Limit+1);$n=0;while($n -lt $buffer.Length){$r=$stream.Read($buffer,$n,$buffer.Length-$n);if(!$r){break};$n+=$r};if(!$n -or $n -gt $Limit){throw 'archive worker bounded read'};return ,([byte[]]$buffer[0..($n-1)])}finally{$stream.Dispose()}
}
function Read-Compact9ArchiveOwner {
 $root='F:\ibcmd\lab\04\locks\worker'
 for($p=$root;$p;$p=[IO.Path]::GetDirectoryName($p)){if((Get-Item -LiteralPath $p -Force).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'archive owner reparse ancestry'}}
 $items=@(Get-ChildItem -LiteralPath $root -Force|Select-Object -First 2)
 if($items.Count -ne 1 -or $items[0] -isnot [IO.FileInfo] -or $items[0].Name -cne 'owner.txt' -or $items[0].FullName -cne "$root\owner.txt" -or ($items[0].Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'archive owner sole ordinary exact member required'}
 Read-Compact9ArchiveBounded "$root\owner.txt" 128
}
function Assert-Compact9ArchiveWorkerBinding($Context,$Proof,[byte[]]$Owner,$Parent){
 $required=@('operation','lease','proof_leaf','proof_sha256','parent_pid','parent_parent','parent_born','parent_exe','parent_command_sha256')
 if(@($Context.PSObject.Properties.Name).Count -ne $required.Count -or @($required|Where-Object{!$Context.PSObject.Properties[$_]}).Count){throw 'archive worker context schema'}
 if($Context.operation -cnotmatch '^[0-9a-f]{32}$' -or $Context.proof_leaf -cnotmatch '^lock-[0-9a-f]{32}\.json$' -or $Context.proof_sha256 -cnotmatch '^[0-9A-F]{64}$'){throw 'archive worker nonce/proof identity'}
 if($Owner.Length -lt 1 -or $Owner.Length -gt 128 -or @($Owner|Where-Object{$_ -gt 127}).Count){throw 'archive owner ASCII/length'}
 $text=[Text.Encoding]::ASCII.GetString($Owner)
 if($text -cnotmatch ('\Atrack='+[regex]::Escape($script:Compact9ArchiveTrack)+' since=\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\r?\n)?\z') -or [Convert]::ToBase64String($Owner) -cne $Context.lease){throw 'archive exact captured unique lease changed'}
 if($Proof.operation -cne $Context.operation -or $Proof.name -cne 'worker' -or ($Proof.sequence -isnot [int] -and $Proof.sequence -isnot [long]) -or $Proof.sequence -lt 1 -or $Proof.potential_held -isnot [bool] -or $Proof.potential_held -cne $true -or $Proof.acquire_completed -isnot [bool] -or $Proof.acquire_completed -cne $true -or ($Proof.acquire_exit -isnot [int] -and $Proof.acquire_exit -isnot [long]) -or $Proof.acquire_exit -ne 0 -or $Proof.released -isnot [bool] -or $Proof.released -cne $false -or $Proof.failure -or $Proof.lease -cne $Context.lease){throw 'archive original confirmed acquire proof differs'}
 if(!$Parent -or [long]$Context.parent_pid -le 0 -or [long]$Context.parent_parent -le 0 -or [long]$Parent.ProcessId -ne [long]$Context.parent_pid -or [long]$Parent.ParentProcessId -ne [long]$Context.parent_parent -or !$Parent.CreationDate -or $Parent.CreationDate.ToUniversalTime().ToString('o') -cne $Context.parent_born -or ![IO.Path]::IsPathFullyQualified([string]$Context.parent_exe) -or [string]$Parent.ExecutablePath -cne [string]$Context.parent_exe -or [string]::IsNullOrWhiteSpace([string]$Parent.CommandLine) -or ([string]$Parent.CommandLine).Length -gt 32768 -or $Context.parent_command_sha256 -cnotmatch '^[0-9A-F]{64}$' -or [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes([string]$Parent.CommandLine))) -cne $Context.parent_command_sha256){throw 'archive parent controller identity missing/drifted'}
 [pscustomobject]@{raw_lease_base64=$Context.lease;operation=$Context.operation;proof_sha256=$Context.proof_sha256;signal_authority=$false}
}
function Get-Compact9ArchiveWorkerAuthority([string]$Encoded){
 if([string]::IsNullOrEmpty($Encoded) -or $Encoded.Length -gt 4096){throw 'archive parent context required/bounded'}
 $context=[Text.UTF8Encoding]::new($false,$true).GetString([Convert]::FromBase64String($Encoded))|ConvertFrom-Json -DateKind String
 if($context.proof_leaf -cnotmatch '^lock-[0-9a-f]{32}\.json$'){throw 'archive exact proof leaf required'}
 $raw=Read-Compact9ArchiveBounded ($script:Compact9ProofRoot+'\'+$context.proof_leaf) 8192
 if([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($raw)) -cne $context.proof_sha256){throw 'archive original acquire proof bytes changed'}
 $proof=[Text.UTF8Encoding]::new($false,$true).GetString($raw)|ConvertFrom-Json
 $parent=Get-CimInstance Win32_Process -Filter "ProcessId=$($context.parent_pid)"
 Assert-Compact9ArchiveWorkerBinding $context $proof (Read-Compact9ArchiveOwner) $parent
}
function Get-Compact9ArchiveWorkerContext {
 $entry=$script:Compact9Leases['worker']
 if(!$entry -or !$entry.acquire_completed -or $entry.acquire_exit -ne 0 -or !$entry.lease -or !$entry.potential_held -or $entry.released -or $entry.failure){throw 'archive requires this parent confirmed worker acquire'}
 $files=@(Get-ChildItem -LiteralPath $script:Compact9ProofRoot -Force|Select-Object -First 513);if($files.Count -gt 512){throw 'archive acquire proof inventory budget'}
 $matches=@()
 foreach($f in $files){
  $raw=Read-Compact9ArchiveBounded $f.FullName 8192
  $p=[Text.UTF8Encoding]::new($false,$true).GetString($raw)|ConvertFrom-Json
  if($p.operation -ceq $entry.operation -and $p.sequence -eq $entry.sequence){$matches+=@{leaf=$f.Name;sha=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($raw))}}
 }
 if($matches.Count -ne 1){throw 'archive exact this-parent acquire proof absent/duplicate'}
 $parent=Get-CimInstance Win32_Process -Filter "ProcessId=$PID"
 if(!$parent.CreationDate -or !$parent.ExecutablePath -or !$parent.CommandLine){throw 'archive parent full identity unavailable'}
 $ctx=[pscustomobject]@{operation=$entry.operation;lease=$entry.lease;proof_leaf=$matches[0].leaf;proof_sha256=$matches[0].sha;parent_pid=[long]$PID;parent_parent=[long]$parent.ParentProcessId;parent_born=$parent.CreationDate.ToUniversalTime().ToString('o');parent_exe=$parent.ExecutablePath;parent_command_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes([string]$parent.CommandLine)))}
 $encoded=[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes(($ctx|ConvertTo-Json -Compress)))
 [void](Get-Compact9ArchiveWorkerAuthority $encoded)
 $encoded
}
