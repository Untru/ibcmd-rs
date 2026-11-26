$ErrorActionPreference='Stop'
$saved=$env:IBCMD_RS_WORKER_LAB_ROOT
try{
 $env:IBCMD_RS_WORKER_LAB_ROOT='F:\ibcmd\lab\05\wave3\load\cluster'
 function Test-Path {param($LiteralPath,$PathType);return $false}
 . "$PSScriptRoot/compact8-tools/cluster/private-lib.ps1"
 if($script:PrivateMappings.Count -ne 1 -or $script:PrivateContext.track -cne 'load8-f8bd7d065a2c' -or $script:PrivateContext.manifest_track -cne 'load' -or $script:PrivateContext.prefix -cne 'ibcmd_rs_05_load_w3_'){throw 'exact nonce map/classification changed'}
 function Test-Path {param($LiteralPath,$PathType);return $true}
 $script:lease="track=load8-f8bd7d065a2c since=2026-10-02T10:00:00`r`n"
 function Get-Content {param($LiteralPath,[switch]$Raw);return $script:lease}
 $state=[pscustomobject]@{worker_lease=$script:lease.Trim()}
 Private-RequireLease $state
 function Import-Csv {param($LiteralPath,$Delimiter);[pscustomobject]@{database='ibcmd_rs_05_load_w3_compact8_20261002';track='load';notes='from F:\ibcmd\lab\05\wave3\load\f5-owned-full.bak'}}
 Private-RequireNames @('ibcmd_rs_05_load_w3_compact8_20261002')
 'PASS actual copied private context nonce + separate exact restore manifest Trackload classification'
 foreach($lease in @('track=load since=2026-10-02T10:00:00','track=load8-other since=2026-10-02T10:00:00','track=load8-f8bd7d065a2c since=2026-10-02T10:00:01')){
  $script:lease=$lease;$caught=$false;try{Private-RequireLease $state}catch{$caught=$true}
  if(!$caught){throw 'wrong nonce/time worker admitted'}
 }
 $caught=$false;try{Private-RequireNames @('ibcmd_rs_05_meta_w3_foreign')}catch{$caught=$true};if(!$caught){throw 'foreign DB classification admitted'}
 'PASS actual old/foreign/replaced lease + foreign DB refusal; acquisitions/SQL/signals0'
}finally{$env:IBCMD_RS_WORKER_LAB_ROOT=$saved}
