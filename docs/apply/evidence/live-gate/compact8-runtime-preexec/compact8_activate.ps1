param([Parameter(Mandatory)][ValidatePattern('^compact8-[a-z0-9-]+$')][string]$Label,[Parameter(Mandatory)][string]$Binary,[Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{40}$')][string]$SourceHead,[Parameter(Mandatory)][ValidatePattern('^[0-9A-F]{64}$')][string]$BinarySha256)
$ErrorActionPreference='Stop';$lab='F:/ibcmd/lab/05/wave3/load';$wt='F:/ibcmd/src/ibcmd-rs-05-load-wave3'
. "$lab/compact8-runtime-v1/compact8-tools/live/process.ps1"
[void](. "$lab/compact8-runtime-v1/compact8-tools/live/workload_lab.ps1")
[void](Resolve-LoadLabRoot $lab)
if(Test-LiveUncertainChild){throw 'unresolved prior child; no new activation'}
$binding=Get-Content "$lab/compact8-binding.json" -Raw|ConvertFrom-Json
if($binding.database -cne 'ibcmd_rs_05_load_w3_compact8_20261002'){throw 'fresh case DB identity'}
$bin=$Binary
foreach($suffix in @('trn','sql','recovery.json','recovery.live.json')){if(Test-Path "$lab/$Label.$suffix"){throw 'fresh artifacts required'}}
if((Get-FileHash $bin).Hash -cne $BinarySha256){throw 'binary provenance'}
$argv=@('mssql-activate-staged-main','--platform-profile','platform-8.3.27.2214','--sqlcmd-trust-cert','--server','localhost','--database',$binding.database,'--mode','live','--live-checkpoint','--live-compact-recovery','--allow-non-lab','--rac','C:/Program Files/1cv8/8.3.27.2214/bin/rac.exe','--ras-endpoint','localhost:5545','--cluster-id',$binding.cluster_uuid,'--infobase-id',$binding.infobase_uuid,'--infobase-user','Администратор','--tail-log-output',"$lab/$Label.trn",'--script-output',"$lab/$Label.sql",'--recovery-output',"$lab/$Label.recovery.json")
$lock=Invoke-LiveBounded pwsh @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','acquire','load8-f8bd7d065a2c','-Name','native','-TimeoutMin','10') 620
$lock|ConvertTo-Json|Set-Content "$lab/logs/$Label-activate-lock.json";if($lock.ExitCode){throw 'lock'}
try{
@{executable=$bin;arguments=$argv;source_head=$SourceHead;sha256=(Get-FileHash $bin).Hash;started_utc=[DateTime]::UtcNow.ToString('o')}|ConvertTo-Json|Set-Content "$lab/logs/$Label-activate.command.json"
$r=Invoke-LiveBounded $bin $argv 90;$r|Add-Member -NotePropertyName ended_utc -NotePropertyValue ([DateTime]::UtcNow.ToString('o'));$r|ConvertTo-Json|Set-Content "$lab/logs/$Label-activate.result.json"
$r.Stdout;$r.Stderr; exit $r.ExitCode
}finally{if(Test-LiveUncertainChild){throw 'unresolved child; native FIFO retained'};$release=Invoke-LiveBounded pwsh @('-NoProfile','-File','F:/ibcmd/lab/04/tools/heavy-lock.ps1','release','load8-f8bd7d065a2c','-Name','native') 15;if($release.ExitCode){throw 'native FIFO release failed'}}


