$ErrorActionPreference='Stop'
. "$PSScriptRoot/compact8_prior_archive.ps1"
$script:savedState=Get-Content -LiteralPath "$PSScriptRoot/case5-stopped-state.json" -Raw|ConvertFrom-Json -DateKind String
$script:archive="$script:ArchiveRoot\evidence\compact6-case5-prior";$script:mode='';$script:census=@();$script:listeners=@();$script:uncertain=$false;$script:mutations=0
$script:raw=@(0..596|ForEach-Object {@{file="$script:archive\file$_";bytes=1;sha256=('A'*64)}})+@(0..15|ForEach-Object {@{file="$script:ArchiveRoot\logs\prior-file$_";bytes=1;sha256=('A'*64)}})
function Test-Path {param($LiteralPath,$PathType);if($LiteralPath -in @("$script:ArchiveCluster\srvinfo","$script:ArchiveCluster\logs")){return $script:mode -ceq 'archive-replay'};if($LiteralPath -ceq "$script:ArchiveCluster\state.json"){return $script:mode -ceq 'active-state'};return $true}
function Get-Item {param($LiteralPath,[switch]$Force);if($LiteralPath -ceq $script:archive){$d=[IO.DirectoryInfo]::new($LiteralPath);$d|Add-Member NoteProperty Attributes ([IO.FileAttributes]::Directory) -Force;return $d};[pscustomobject]@{Length=$(if($LiteralPath -like '*\file*' -or $LiteralPath -like '*\prior-file*'){1}else{100});Attributes=$(if($script:mode -ceq 'reparse' -and $LiteralPath -ceq 'F:\ibcmd\lab\05\wave3'){[IO.FileAttributes]::ReparsePoint}else{[IO.FileAttributes]::Normal})}}
function Get-ChildItem {param($LiteralPath,[switch]$Force,[switch]$Recurse);if($LiteralPath.EndsWith('compact6-child-receipts')){if($script:mode -ceq 'old6-receipt'){[pscustomobject]@{Name='unknown.json'}};return};if($LiteralPath -ceq $script:archive){$n=if($script:mode -ceq 'archive-extra'){597}else{596};0..$n|ForEach-Object {[pscustomobject]@{FullName="$script:archive\file$_";Attributes=[IO.FileAttributes]::Normal}}}}
function Get-Content {param($LiteralPath,[switch]$Raw);if($script:mode -ceq 'foreign-lease'){return 'track=meta since=fresh'};return 'track=load since=fresh'}
function Get-FileHash {param($LiteralPath);$sha='A'*64;if($LiteralPath.EndsWith('stopped-state-20261002001827166.json')){$sha='5B09714CAF28B294D13D2BF85A0BC0A5344D870E508E3931C984EF17FFF4943A'};if($LiteralPath.EndsWith('compact6-runtime-resource-proof.json')){$sha='72D05BACA921E350773C05CDD5F1616A18FE0961282FB81CC3950EAAB0B1E450'};if($script:mode -ceq 'raw-change' -and $LiteralPath.EndsWith('\file1')){$sha='0'*64};[pscustomobject]@{Hash=$sha}}
function Read-Compact8ArchiveJson($Path){
 if($Path.EndsWith('compact6-runtime-resource-proof.json')){$r=@($script:raw);if($script:mode -ceq 'foreign-input'){$r[0]=@{file='D:\foreign';bytes=1;sha256=('A'*64)}};return [pscustomobject]@{database='ibcmd_rs_05_load_w3_compact6_20261002';manifest_sha256='D26E884FB0C84580C69B03BCCAD1B867922DC5633E33A53E0EAA09BF2658C7B2';restore_dispatched=($script:mode -ceq 'old-restore');cluster_started=$false;registered=$false;phase1_executed=$false;cycle2_executed=$false;raw=$r}}
 if($Path.EndsWith('cleanup-status.json')){return [pscustomobject]@{guarded_stop_clean=$true;resources_may_need_exact_owned_cleanup=$false;registration_state='unregistered';worker_lease_held_or_unconfirmed=$false;no_purge=$true}}
 if($Path.EndsWith('release.json')){return [pscustomobject]@{ExitCode=0}}
 return $script:savedState
}
function Test-LiveUncertainChild {return $script:uncertain}
function Get-CimInstance {param($ClassName);$script:census}
function Get-NetTCPConnection {param($State);$script:listeners}
function Move-Item {$script:mutations++;throw 'No archive move permitted'}
function Copy-Item {$script:mutations++;throw 'No archive copy permitted'}
function Remove-Item {$script:mutations++;throw 'No archive deletion permitted'}
function New-Item {$script:mutations++;throw 'No archive creation permitted'}
Assert-Compact8PriorArchive|Out-Null
'PASS exact 613 raw/597 retained archive validation is read-only'
foreach($mode in @('archive-replay','active-state','old6-receipt','foreign-lease','saved-PID','listener','reparse','raw-change','archive-extra','foreign-input','old-restore','unknown-child')){
 $script:mode=$mode;$script:census=@();$script:listeners=@();$script:uncertain=$mode -ceq 'unknown-child'
 if($mode -ceq 'saved-PID'){$script:census=@([pscustomobject]@{ProcessId=99324;CommandLine='foreign reused PID'})}
 if($mode -ceq 'listener'){$script:listeners=@([pscustomobject]@{LocalPort=5561})}
 $caught=$false;try{Assert-Compact8PriorArchive|Out-Null}catch{$caught=$true}
 if(!$caught -or $script:mutations){throw "Readonly prior proof refusal failed $mode"}
 "PASS $mode refusal; moves/copies/deletes/creates0"
}
'ALL PASS; realStarts=realSignals=SQLcalls=registryWrites=Moves=Deletes=0'
$script:mode='';$script:uncertain=$false;$script:listeners=@()
$script:census=@([pscustomobject]@{ProcessId=99324;ParentProcessId=40344;CreationDate=[datetime]::Parse('2026-10-02T06:38:27.2300620Z').ToUniversalTime();ExecutablePath='C:\Program Files\PowerShell\7\pwsh.exe';CommandLine='pwsh -File F:\foreign\read-only.ps1'})
$positive=Assert-Compact8ArchiveAuthority
if($positive.foreign_reused_readonly.Count -ne 1 -or $positive.signal_authority -or $positive.stop_authority){throw 'actual authority function foreign exclusion failed'}
'PASS actual authority saved Case5 raw + complete synthetic foreign reused PID; no signal authority'
$script:leaseReads=0
function Get-Content {param($LiteralPath,[switch]$Raw);$script:leaseReads++;if($script:mode -ceq 'lease-drift' -and $script:leaseReads -gt 1){return 'track=load since=replaced'};return 'track=load since=fresh'}
$script:stateReads=0
function Get-FileHash {param($LiteralPath);$sha='A'*64;if($LiteralPath.EndsWith('stopped-state-20261002001827166.json')){$script:stateReads++;$sha='5B09714CAF28B294D13D2BF85A0BC0A5344D870E508E3931C984EF17FFF4943A';if($script:mode -ceq 'state-drift' -and $script:stateReads -gt 2){$sha='B'*64}};if($LiteralPath.EndsWith('compact6-runtime-resource-proof.json')){$sha='72D05BACA921E350773C05CDD5F1616A18FE0961282FB81CC3950EAAB0B1E450'};[pscustomobject]@{Hash=$sha}}
$script:childReads=0
function Test-LiveUncertainChild {$script:childReads++;return ($script:mode -ceq 'late-child' -and $script:childReads -gt 1)}
foreach($mode in @('lease-drift','state-drift','late-child')){
 $script:mode=$mode;$script:leaseReads=0;$script:stateReads=0;$script:childReads=0
 $caught=$false;try{Assert-Compact8PriorArchive|Out-Null}catch{$caught=$true}
 if(!$caught -or $script:mutations){throw "actual before/after authority drift failed $mode"}
 "PASS actual prior-archive $mode refusal; starts/signals/moves/deletes/acquires0"
}
