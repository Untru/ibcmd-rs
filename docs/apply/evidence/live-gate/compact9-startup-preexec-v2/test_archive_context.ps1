$ErrorActionPreference='Stop'
. "$PSScriptRoot/archive_worker.ps1"
$lease=[Convert]::ToBase64String([Text.Encoding]::ASCII.GetBytes("track=load9-983ad8f3eae1 since=2026-10-02T10:00:00`r`n"))
$script:Compact9Leases=@{worker=[pscustomobject]@{operation=('a'*32);sequence=3;lease=$lease;potential_held=$true;acquire_completed=$true;acquire_exit=0;released=$false;failure=$null}}
$script:proof=$script:Compact9Leases.worker.PSObject.Copy();$script:proof|Add-Member NoteProperty name 'worker'
$script:proofRaw=[Text.Encoding]::UTF8.GetBytes(($script:proof|ConvertTo-Json -Compress))
$script:parent=[pscustomobject]@{ProcessId=$PID;ParentProcessId=400;CreationDate=[datetime]::Parse('2026-10-02T07:00:00.1234560Z').ToUniversalTime();ExecutablePath='C:\Program Files\PowerShell\7\pwsh.exe';CommandLine='pwsh -File F:\ibcmd\lab\05\wave3\load\compact9-runtime-v2\compact9_lifetime_v2.ps1'}
$script:mode='';$script:authorityCalls=0
function Get-ChildItem {param($LiteralPath,[switch]$Force);$one=[pscustomobject]@{FullName=($script:Compact9ProofRoot+'\lock-'+('b'*32)+'.json');Name=('lock-'+('b'*32)+'.json')};if($script:mode -eq 'duplicate'){return @($one,$one)};return $one}
function Read-Compact9ArchiveBounded {param($Path,$Limit);return ,$script:proofRaw}
function Read-Compact9ArchiveOwner {return ,[Convert]::FromBase64String($lease)}
function Get-CimInstance {param($ClassName,$Filter);if($Filter -cne "ProcessId=$PID"){throw 'parent filter'};return $script:parent}
$original=${function:Get-Compact9ArchiveWorkerAuthority}
function Get-Compact9ArchiveWorkerAuthority($Encoded){$script:authorityCalls++;if($script:mode -eq 'late-parent'){$script:parent.ParentProcessId=401};& $original $Encoded}
$encoded=Get-Compact9ArchiveWorkerContext
$decoded=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($encoded))|ConvertFrom-Json
if($decoded.parent_pid -ne $PID -or $decoded.operation -cne ('a'*32) -or $script:authorityCalls -ne 1){throw 'actual issuer did not validate bound authority'}
'PASS actual parent issuer validates own proof/raw/current parent before emitting context; no filesystem/OS actions'
foreach($mode in @('duplicate','late-parent','nonzero')){
 $script:mode=$mode;if($mode -eq 'nonzero'){$script:Compact9Leases.worker.acquire_exit=1}
 $caught=$false;try{Get-Compact9ArchiveWorkerContext|Out-Null}catch{$caught=$true}
 if(!$caught){throw "issuer refusal failed $mode"}
 "PASS actual issuer $mode refusal; acquires/signals/writes0"
}
