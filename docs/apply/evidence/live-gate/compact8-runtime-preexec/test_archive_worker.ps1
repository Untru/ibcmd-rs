$ErrorActionPreference='Stop'
. "$PSScriptRoot/archive_worker.ps1"
$script:originalBounded=${function:Read-Compact8ArchiveBounded}
$script:originalOwner=${function:Read-Compact8ArchiveOwner}
$script:count=0
function Reject([scriptblock]$Body,[string]$Reason){$caught=$false;try{& $Body|Out-Null}catch{$caught=$true;if($_.Exception.Message -notlike "*$Reason*"){throw}};if(!$caught){throw "expected refusal $Reason"};$script:count++}
$bytes=[Text.Encoding]::ASCII.GetBytes("track=load8-f8bd7d065a2c since=2026-10-02T10:00:00`r`n")
$proof=[pscustomobject]@{operation=('a'*32);sequence=1;name='worker';potential_held=$true;acquire_completed=$true;acquire_exit=0;lease=[Convert]::ToBase64String($bytes);released=$false;failure=$null}
$script:proofRaw=[Text.Encoding]::UTF8.GetBytes(($proof|ConvertTo-Json -Compress))
$parent=[pscustomobject]@{ProcessId=500;ParentProcessId=400;CreationDate=[datetime]::Parse('2026-10-02T07:00:00.1234567Z').ToUniversalTime();ExecutablePath='C:\Program Files\PowerShell\7\pwsh.exe';CommandLine='pwsh -File F:\ibcmd\lab\05\wave3\load\compact8-runtime-v1\compact8_lifetime_v2.ps1'}
$ctx=[pscustomobject]@{operation=$proof.operation;lease=$proof.lease;proof_leaf=('lock-'+('b'*32)+'.json');proof_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($script:proofRaw));parent_pid=500;parent_parent=400;parent_born=$parent.CreationDate.ToUniversalTime().ToString('o');parent_exe=$parent.ExecutablePath;parent_command_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($parent.CommandLine)))}
$positive=Assert-Compact8ArchiveWorkerBinding $ctx $proof $bytes $parent
if($positive.raw_lease_base64 -cne $proof.lease -or $positive.signal_authority){throw 'valid context failed'};$script:count++
foreach($field in @('operation','lease','name','sequence','potential_held','acquire_completed','acquire_exit','released','failure')){
 $bad=$proof.PSObject.Copy()
 switch($field){operation{$bad.$field='c'*32};lease{$bad.$field=[Convert]::ToBase64String([byte[]](1,2))};name{$bad.$field='heavy'};sequence{$bad.$field=0};potential_held{$bad.$field=$false};acquire_completed{$bad.$field=$false};acquire_exit{$bad.$field=1};released{$bad.$field=$true};failure{$bad.$field='uncertain'}}
 Reject {Assert-Compact8ArchiveWorkerBinding $ctx $bad $bytes $parent} 'acquire proof'
}
foreach($field in @('ProcessId','ParentProcessId','CreationDate','ExecutablePath','CommandLine')){
 $bad=$parent.PSObject.Copy()
 switch($field){ProcessId{$bad.$field=501};ParentProcessId{$bad.$field=401};CreationDate{$bad.$field=$bad.$field.AddTicks(1)};ExecutablePath{$bad.$field='C:\foreign\pwsh.exe'};CommandLine{$bad.$field='different'}}
 Reject {Assert-Compact8ArchiveWorkerBinding $ctx $proof $bytes $bad} 'controller identity'
}
Reject {Assert-Compact8ArchiveWorkerBinding $ctx $proof $bytes $null} 'controller identity'
Reject {Assert-Compact8ArchiveWorkerBinding $ctx $proof ([Text.Encoding]::ASCII.GetBytes('track=load since=2026-10-02T10:00:00')) $parent} 'unique lease'
Reject {Assert-Compact8ArchiveWorkerBinding $ctx $proof ([byte[]]::new(129)) $parent} 'length'
Reject {Assert-Compact8ArchiveWorkerBinding $ctx $proof ([byte[]](255)) $parent} 'ASCII'
# Exercise the actual child reader/validator dispatch with mocked file bytes/CIM only.
$script:ownerBytes=$bytes;$script:currentParent=$parent;$script:readCount=0
function Read-Compact8ArchiveBounded {param($Path,$Limit);$script:readCount++;return ,$script:proofRaw}
function Read-Compact8ArchiveOwner {return ,$script:ownerBytes}
function Get-CimInstance {param($ClassName,$Filter);if($Filter -cne 'ProcessId=500'){throw 'unexpected parent query'};return $script:currentParent}
$encoded=[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes(($ctx|ConvertTo-Json -Compress)))
$result=Get-Compact8ArchiveWorkerAuthority $encoded
if($result.operation -cne ('a'*32) -or $script:readCount -ne 1){throw 'actual authority route failed'};$script:count++
$script:proofRaw=[byte[]](1,2,3);Reject {Get-Compact8ArchiveWorkerAuthority $encoded} 'proof bytes'
Reject {Get-Compact8ArchiveWorkerAuthority ''} 'context required'
Reject {Get-Compact8ArchiveWorkerAuthority ('x'*4097)} 'bounded'
# Actual metadata/single-owner guards refuse before any stream is allocated/opened.
$script:mode='large'
function Get-Item {param($LiteralPath,[switch]$Force);$f=[IO.FileInfo]::new($LiteralPath);$f|Add-Member NoteProperty Attributes ([IO.FileAttributes]::Normal) -Force;$f|Add-Member NoteProperty Length $(if($script:mode -eq 'large'){129}else{0}) -Force;return $f}
Reject {& $script:originalBounded 'F:\ibcmd\lab\04\locks\worker\owner.txt' 128} 'metadata budget'
$script:mode='zero';Reject {& $script:originalBounded 'F:\ibcmd\lab\04\locks\worker\owner.txt' 128} 'metadata budget'
function Get-ChildItem {param($LiteralPath,[switch]$Force);$owner=[IO.FileInfo]::new('F:\ibcmd\lab\04\locks\worker\owner.txt');$owner|Add-Member NoteProperty Attributes ([IO.FileAttributes]::Normal) -Force;if($script:mode -eq 'extra'){return @($owner,[IO.FileInfo]::new('F:\ibcmd\lab\04\locks\worker\other'))};if($script:mode -eq 'reparse'){$owner|Add-Member NoteProperty Attributes ([IO.FileAttributes]::ReparsePoint) -Force};return $owner}
$script:mode='extra';Reject {& $script:originalOwner} 'sole ordinary'
$script:mode='reparse';Reject {& $script:originalOwner} 'sole ordinary'
"PASS $script:count archive worker binding cases; realReads/acquires/signals/moves/deletes0"
