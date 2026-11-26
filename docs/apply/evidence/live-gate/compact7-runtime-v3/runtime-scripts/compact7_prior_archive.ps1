param([switch]$Validate)
$ErrorActionPreference='Stop'
$script:ArchiveRoot='F:\ibcmd\lab\05\wave3\load'
$script:ArchiveCluster="$script:ArchiveRoot\cluster"

function Assert-Compact7PriorArchive {
    $before=Assert-Compact7ArchiveAuthority
    $proofPath="$script:ArchiveRoot\compact6-runtime-resource-proof.json"
    if((Get-FileHash -LiteralPath $proofPath).Hash -cne '72D05BACA921E350773C05CDD5F1616A18FE0961282FB81CC3950EAAB0B1E450'){throw 'Prior resource-only proof changed'}
    $proof=Read-Compact7ArchiveJson $proofPath
    if($proof.database -cne 'ibcmd_rs_05_load_w3_compact6_20261002' -or $proof.manifest_sha256 -cne 'D26E884FB0C84580C69B03BCCAD1B867922DC5633E33A53E0EAA09BF2658C7B2' -or $proof.restore_dispatched -ne $false -or $proof.cluster_started -ne $false -or $proof.registered -ne $false -or $proof.phase1_executed -ne $false -or $proof.cycle2_executed -ne $false -or $proof.raw.Count -ne 613){throw 'Prior compact6 resource-only identity differs'}
    foreach($name in @('srvinfo','logs')){if(Test-Path -LiteralPath "$script:ArchiveCluster\$name"){throw 'Fresh root already has registry/logs; archive replay forbidden'}}
    if(!(Test-Path -LiteralPath "$script:ArchiveRoot\compact6-child-receipts") -or @(Get-ChildItem -LiteralPath "$script:ArchiveRoot\compact6-child-receipts" -Force).Count){throw 'Prior compact6 unresolved child receipt retained'}
    $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase);$total=0L
    foreach($entry in $proof.raw){
        [void](Assert-Compact7ArchivePath $entry.file)
        if(!$seen.Add($entry.file) -or $entry.bytes -lt 0 -or $entry.bytes -gt 1073741824 -or $entry.sha256 -cnotmatch '^[0-9A-F]{64}$'){throw 'Prior raw entry identity/budget'}
        $total+=$entry.bytes;if($total -gt 1073741824){throw 'Prior raw total budget'}
        if((Get-Item -LiteralPath $entry.file).Length -ne $entry.bytes -or (Get-FileHash -LiteralPath $entry.file).Hash -cne $entry.sha256){throw 'Prior immutable raw bytes changed'}
    }
    $archive="$script:ArchiveRoot\evidence\compact6-case5-prior"
    [void](Assert-Compact7ArchivePath $archive)
    $pending=[Collections.Generic.Stack[object]]::new();$pending.Push((Get-Item -LiteralPath $archive -Force));$files=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase);$nodes=0
    while($pending.Count){
        $node=$pending.Pop();$nodes++;if($nodes -gt 2048){throw 'Prior archive tree budget'}
        [void](Assert-Compact7ArchivePath $node.FullName)
        if($node.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Prior archive tree reparse'}
        if($node -is [IO.DirectoryInfo]){foreach($child in Get-ChildItem -LiteralPath $node.FullName -Force){$pending.Push($child)}}else{if(!$files.Add($node.FullName)){throw 'Prior archive duplicate file'}}
    }
    $expected=@($proof.raw|Where-Object {$_.file.StartsWith($archive+'\',[StringComparison]::Ordinal)})
    if($expected.Count -ne 597 -or $files.Count -ne 597){throw 'Prior archive exact inventory differs'}
    foreach($entry in $expected){if(!$files.Contains($entry.file)){throw 'Prior archive missing file'}}
    $after=Assert-Compact7ArchiveAuthority
    if($after.lease -cne $before.lease -or $after.state_sha256 -cne $before.state_sha256){throw 'Prior archive authority drift'}
    'PASS read-only prior compact6/613 raw and existing case5/597 archive; no archive replay/move/delete'
}

function Assert-Compact7ArchivePath([string]$Path,[switch]$Tree){
 if(![IO.Path]::IsPathFullyQualified($Path)){throw 'archive fully qualified path required'}
 $full=[IO.Path]::GetFullPath($Path).TrimEnd('\')
 if($full -cne $Path.TrimEnd('\') -or !$full.StartsWith($script:ArchiveRoot+'\',[StringComparison]::Ordinal)){throw 'archive exact canonical containment required'}
 foreach($probe in @($full)+@(& {for($q=[IO.Path]::GetDirectoryName($full);$q;$q=[IO.Path]::GetDirectoryName($q)){$q}})){
  if((Test-Path -LiteralPath $probe) -and ((Get-Item -LiteralPath $probe -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'archive reparse ancestry refused'}
 }
 if($Tree){foreach($entry in @(Get-ChildItem -LiteralPath $full -Recurse -Force)){if($entry.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'archive reparse content refused'}}}
 return $full
}
function Read-Compact7ArchiveJson($Path){
 [void](Assert-Compact7ArchivePath $Path)
 if((Get-Item -LiteralPath $Path).Length -gt 1048576){throw 'archive proof exceeds budget'}
 Get-Content -LiteralPath $Path -Raw|ConvertFrom-Json -DateKind String
}
function Assert-Compact7ArchiveAuthority {
 $leasePath='F:\ibcmd\lab\04\locks\worker\owner.txt'
 if(!(Test-Path -LiteralPath $leasePath)){throw 'archive worker lease absent'}
 $lease=(Get-Content -LiteralPath $leasePath -Raw).Trim();if($lease -notmatch '^track=load since=\S+$'){throw 'archive foreign worker lease'}
 if(Test-LiveUncertainChild){throw 'archive unknown child receipt retained'}
 if(!(Test-Path -LiteralPath "$script:ArchiveRoot\compact5-child-receipts") -or @(Get-ChildItem -LiteralPath "$script:ArchiveRoot\compact5-child-receipts" -Force).Count){throw 'case5 unknown child receipt retained'}
 if(Test-Path -LiteralPath "$script:ArchiveCluster\state.json"){throw 'archive active state refused'}
 $cleanup=Read-Compact7ArchiveJson "$script:ArchiveRoot\logs\compact5-lifetime-cleanup-status.json"
 if(!$cleanup.guarded_stop_clean -or $cleanup.resources_may_need_exact_owned_cleanup -or $cleanup.registration_state -cne 'unregistered' -or $cleanup.worker_lease_held_or_unconfirmed -or !$cleanup.no_purge){throw 'case5 cleanup authority incomplete'}
 foreach($name in @('heavy','worker')){$r=Read-Compact7ArchiveJson "$script:ArchiveRoot\logs\compact5-lifetime-$name-release.json";if($r.ExitCode -ne 0){throw 'case5 prior lease release unproved'}}
 $state=Read-Compact7ArchiveJson "$script:ArchiveCluster\stopped-state-20261002001827166.json"
 if($state.root -cne $script:ArchiveCluster -or $state.cluster -cne 'd97ba4fe-773c-451b-a210-82ca1dc4f067' -or $state.worker_lease -cne 'track=load since=2026-10-02T00:13:41'){throw 'case5 stopped state identity differs'}
 if((Get-FileHash -LiteralPath "$script:ArchiveCluster\stopped-state-20261002001827166.json").Hash -cne '5B09714CAF28B294D13D2BF85A0BC0A5344D870E508E3931C984EF17FFF4943A'){throw 'case5 saved immutable state SHA differs'}
 $census=@(Get-CimInstance Win32_Process)
 $known=@($state.anchors)+@($state.known)
 if(@($state.anchors).Count -ne 2 -or @($state.known).Count -ne 6){throw 'case5 saved ancestry inventory differs'}
 foreach($p in $census){if($p.ProcessId -in $known.pid -or ([string]$p.CommandLine).IndexOf($script:ArchiveCluster,[StringComparison]::OrdinalIgnoreCase) -ge 0){throw 'case5 saved PID/reused PID/root process present; no archive'}}
 if(@(Get-NetTCPConnection -State Listen|Where-Object{$_.LocalPort -in @(5540,5541,5545)+(5560..5591)}).Count){throw 'private listeners present; no archive'}
 return [pscustomobject]@{lease=$lease;state_sha256=(Get-FileHash -LiteralPath "$script:ArchiveCluster\stopped-state-20261002001827166.json").Hash;known_pids=@($known.pid|Sort-Object -Unique);saved_pids_and_root_absent=$true;listener_count=0;sanitized_census=@($census|ForEach-Object{[ordered]@{pid=$_.ProcessId;parent=$_.ParentProcessId;born=$(if($_.CreationDate){$_.CreationDate.ToUniversalTime().ToString('o')}else{''});executable=$_.ExecutablePath;command_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes([string]$_.CommandLine)))}})}
}

if($Validate){. 'F:/ibcmd/lab/05/wave3/load/compact7-runtime-v3/compact7-tools/live/process.ps1';Assert-Compact7PriorArchive}
