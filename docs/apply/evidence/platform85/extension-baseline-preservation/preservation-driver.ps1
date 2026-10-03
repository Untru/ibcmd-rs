# ONE coordinator-authorized COPY_ONLY capture. No restore/registration/native/worker.
param([ValidatePattern('^[A-Fa-f0-9]{64}$')][string]$FrozenManifestSha,[switch]$Execute)
$ErrorActionPreference='Stop'
$lab='F:\ibcmd\lab\05\wave3\platform85'
$db='ibcmd_rs_05_p85_w3_ext_version_native_20261001'
$backup="$lab\baselines\extension-settled-A-20261002.bak"
$run="$lab\extension-baseline-preservation-v5"
$helper="$lab\tools\extension-v2-kit\process.ps1"
$helperSha='F376181945FB172BC97E27CE90AB7FC77FE4B89D41C9EC21E7C5DFFB62D5BC7D'
function Boundary([string]$path) {
 if(![IO.Path]::IsPathFullyQualified($path)){throw 'absolute preservation path required'}
 $canonical=[IO.Path]::GetFullPath($path)
 if($canonical -cne $path -or !$canonical.StartsWith($lab+'\',[StringComparison]::Ordinal)){throw 'exact own F lab child required'}
 for($p=$canonical;$p;$p=[IO.Path]::GetDirectoryName($p)){
  if((Test-Path -LiteralPath $p) -and ((Get-Item -LiteralPath $p -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'preservation path reparse ancestry'}
 }
}
function OwnerContext {
 if(Test-LiveUncertainChild){throw 'unresolved owned child; stop preservation, retain outputs'}
 $manifestPath="$lab\logs\extension-preservation-frozen-v5.json";Boundary $manifestPath
 if(!$FrozenManifestSha -or (Get-Item -LiteralPath $manifestPath).Length -gt 1MB -or (Get-FileHash -LiteralPath $manifestPath).Hash -ine $FrozenManifestSha){throw 'preservation frozen closure required'}
 $manifest=Get-Content -LiteralPath $manifestPath -Raw|ConvertFrom-Json
 foreach($e in $manifest.files){
  if([IO.Path]::GetFullPath($e.file) -cne $e.file){throw 'noncanonical preservation dependency'}
  for($p=$e.file;$p;$p=[IO.Path]::GetDirectoryName($p)){if((Get-Item -LiteralPath $p -Force).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'preservation dependency reparse ancestry'}}
  if((Get-Item -LiteralPath $e.file).Length -ne $e.bytes -or (Get-FileHash -LiteralPath $e.file).Hash -ine $e.sha256){throw 'preservation dependency drift'}
 }
 foreach($e in $manifest.executables.PSObject.Properties){if(@(Get-Command $e.Name -CommandType Application)[0].Source -cne $e.Value){throw 'effective preservation executable drift'}}
 foreach($path in @($backup,$run,$helper)){Boundary $path}
 if((Get-FileHash -LiteralPath $helper).Hash -cne $helperSha){throw 'accepted bounded helper drifted'}
 $ledger='F:\ibcmd\lab\04\databases.tsv'
 if((Get-Item -LiteralPath $ledger).Length -gt 4MB){throw 'ownership ledger budget'}
 $rows=@(Get-Content -LiteralPath $ledger -Encoding utf8 | Where-Object{($_ -split "`t")[0] -ceq $db})
 if(!$rows.Count -or @($rows | Where-Object{($_ -split "`t")[1] -cne 'p85'}).Count){throw 'exact p85 ownership absent/ambiguous'}
 if($rows[-1] -notmatch '\tunregistered from the private wave3 8\.5 worker lab cluster\t'){throw 'latest owned registration history is not unregistered'}
 if(Test-Path -LiteralPath "$lab\cluster\state.json"){throw 'private85 cluster active'}
 $stopped=Get-Content -LiteralPath "$lab\cluster\stopped-state-20261002004805224.json" -Raw|ConvertFrom-Json
 foreach($saved in @($stopped.anchors)+@($stopped.known)){
  if($saved -and (Get-CimInstance Win32_Process -Filter "ProcessId=$($saved.pid)")){throw 'saved private process present or PID reused'}
 }
 if(Get-CimInstance Win32_Process -Filter 'ProcessId=44392'){throw 'old producer present or PID reused'}
 foreach($p in @(Get-CimInstance Win32_Process)){
  if($p.CommandLine -and (($p.Name -in @('1cv8.exe','1cv8c.exe','1cv8s.exe') -and $p.CommandLine.IndexOf($db,[StringComparison]::OrdinalIgnoreCase) -ge 0) -or ($p.Name -in @('ragent.exe','ras.exe','rmngr.exe','rphost.exe','dbda.exe') -and $p.CommandLine.IndexOf("$lab\cluster",[StringComparison]::OrdinalIgnoreCase) -ge 0))){throw 'target client or private-root server present'}
 }
 if(@(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue | Where-Object{$_.LocalPort -in @(6540,6541,6545)}).Count){throw 'private85 listener remains'}
}
function Child([string]$label,[string]$exe,[string[]]$argv,[int]$seconds=30) {
 OwnerContext
 $target=Join-Path $run $label
 if(Test-Path -LiteralPath ($target+'.json')){throw 'preservation child result already exists'}
 @{executable=$exe;arguments=$argv;deadline_seconds=$seconds;started_utc=[datetime]::UtcNow.ToString('o')}|ConvertTo-Json -Depth 6|Set-Content -LiteralPath ($target+'.command.json') -Encoding utf8
 $r=Invoke-LiveBounded -Executable $exe -Arguments $argv -TimeoutSeconds $seconds
 $r|ConvertTo-Json -Depth 6|Set-Content -LiteralPath ($target+'.json') -Encoding utf8
 if($r.ExitCode){throw "preservation child refused: $label"}
 if(Test-LiveUncertainChild){throw 'late child uncertainty; retain all outputs'}
 return $r
}
function Idle {
 $q="IF DB_ID(N'$db') IS NULL THROW 57239,'owned source absent',1; IF (SELECT state FROM sys.databases WHERE name=N'$db')<>0 OR (SELECT user_access FROM sys.databases WHERE name=N'$db')<>0 THROW 57239,'source not online multi-user',1; IF EXISTS(SELECT 1 FROM sys.dm_exec_sessions s LEFT JOIN sys.dm_exec_requests r ON s.session_id=r.session_id WHERE s.session_id<>@@SPID AND (s.database_id=DB_ID(N'$db') OR r.database_id=DB_ID(N'$db'))) THROW 57239,'source SQL sessions remain',1; IF EXISTS(SELECT 1 FROM sys.dm_tran_database_transactions d JOIN sys.dm_tran_session_transactions s ON s.transaction_id=d.transaction_id WHERE d.database_id=DB_ID(N'$db') AND s.session_id<>@@SPID) THROW 57239,'source transaction remains',1; IF EXISTS(SELECT 1 FROM [$db].dbo.ConfigSave) OR EXISTS(SELECT 1 FROM [$db].dbo.ConfigCASSave) THROW 57239,'source pending stage',1; SELECT N'OWNED_IDLE';"
 $script:step++;[void](Child ('{0:d2}-idle' -f $script:step) sqlcmd @('-S','localhost','-E','-C','-b','-Q',$q))
}
function Snapshot([string]$tag) {
 $path="$lab\snapshots\$tag";Boundary $path
 if(Test-Path -LiteralPath $path){throw 'snapshot already exists'}
 [void](Child $tag python @("$lab\tools\snapshot_storage.py",$db,$tag) 90)
 [void](Child ($tag+'-compare') python @("$lab\tools\compare_full_snapshots.py",'--before',"$lab\snapshots\ext_version_native_v4_postfailed\inventory.json",'--after',"$path\inventory.json") 60)
}
Boundary $backup;Boundary $run;Boundary $helper
if((Get-FileHash -LiteralPath $helper).Hash -cne $helperSha){throw 'bounded helper SHA differs'}
. $helper
$env:IBCMD_RS_LIVE_CHILD_RECEIPT_ROOT="$lab\extension-v2-child-receipts"
OwnerContext
if(Test-Path -LiteralPath $backup){throw 'COPY_ONLY destination exists; no overwrite'}
if(Test-Path -LiteralPath $run){throw 'preservation results already exist'}
if((Get-PSDrive F).Free -lt 25GB){throw 'F free below25GB'}
if(!$Execute){throw 'explicit coordinator-authorized Execute required'}
New-Item -ItemType Directory -Path $run|Out-Null
$temp="$run\tmp";Boundary $temp;New-Item -ItemType Directory -Path $temp|Out-Null
$env:TEMP=$temp;$env:TMP=$temp;$env:PYTHONDONTWRITEBYTECODE='1'
$script:step=0
try {
 Idle;Snapshot 'ext_version_settled_backup_before_v5';Idle
 if(!(Test-Path -LiteralPath "$lab\baselines")){New-Item -ItemType Directory -Path "$lab\baselines"|Out-Null}
 Boundary $backup
 if(Test-Path -LiteralPath $backup){throw 'destination appeared before backup; no overwrite'}
 # One source, one device, small buffers. No INIT/FORMAT/restore/DB alteration.
 $sql="BACKUP DATABASE [$db] TO DISK=N'$backup' WITH COPY_ONLY,CHECKSUM,COMPRESSION,BUFFERCOUNT=4,MAXTRANSFERSIZE=65536;"
 [void](Child 'one-copy-only-backup' sqlcmd @('-S','localhost','-E','-C','-b','-Q',$sql) 90)
 Idle
 [void](Child 'backup-header' python @("$lab\tools\verify_extension_backup_header.py",$backup,"$run\backup-header.json") 30)
 [void](Child 'backup-verifyonly' sqlcmd @('-S','localhost','-E','-C','-b','-Q',"RESTORE VERIFYONLY FROM DISK=N'$backup' WITH CHECKSUM;") 60)
 Snapshot 'ext_version_settled_backup_after_v5';Idle
 @{backup=$backup;bytes=(Get-Item -LiteralPath $backup).Length;sha256=(Get-FileHash -LiteralPath $backup).Hash;source=$db;copy_only=$true;full_before_after_matches_postfailed=$true;no_restore_registration_native_worker=$true}|ConvertTo-Json|Set-Content -LiteralPath "$run\result.json" -Encoding utf8
}catch {
 $_|Out-String|Set-Content -LiteralPath "$run\failure.txt" -Encoding utf8
 throw
}
