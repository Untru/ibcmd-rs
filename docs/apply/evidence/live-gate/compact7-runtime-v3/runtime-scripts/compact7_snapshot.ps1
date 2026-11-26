param([Parameter(Mandatory)][ValidatePattern('^[a-z0-9-]+$')][string]$Label)
$ErrorActionPreference='Stop'
$wt='F:/ibcmd/src/ibcmd-rs-05-load-wave3';$lab='F:/ibcmd/lab/05/wave3/load';$binding=Get-Content "$lab/compact7-binding.json" -Raw|ConvertFrom-Json;$db=$binding.database;if($db -cne 'ibcmd_rs_05_load_w3_compact7_20261002'){throw 'isolated database binding'}
. "$lab/compact7-runtime-v3/compact7-tools/live/process.ps1"
if(@(Get-ChildItem -LiteralPath "$lab/snapshots" -Filter "$Label-*").Count){throw 'snapshot evidence already exists'}
$queries=[ordered]@{}
$queries['storage']=@"
SET NOCOUNT ON; USE [$db];
SELECT 'Config',FileName,PartNo,CONVERT(varchar(27),Creation,121),CONVERT(varchar(27),Modified,121),CONVERT(int,Attributes),DataSize,DATALENGTH(BinaryData),CONVERT(varchar(64),HASHBYTES('SHA2_256',BinaryData),2) FROM dbo.Config
UNION ALL SELECT 'ConfigSave',FileName,PartNo,CONVERT(varchar(27),Creation,121),CONVERT(varchar(27),Modified,121),CONVERT(int,Attributes),DataSize,DATALENGTH(BinaryData),CONVERT(varchar(64),HASHBYTES('SHA2_256',BinaryData),2) FROM dbo.ConfigSave
UNION ALL SELECT 'Params',FileName,PartNo,CONVERT(varchar(27),Creation,121),CONVERT(varchar(27),Modified,121),CONVERT(int,Attributes),DataSize,DATALENGTH(BinaryData),CONVERT(varchar(64),HASHBYTES('SHA2_256',BinaryData),2) FROM dbo.Params
UNION ALL SELECT 'Files',FileName,PartNo,CONVERT(varchar(27),Creation,121),CONVERT(varchar(27),Modified,121),CONVERT(int,Attributes),DataSize,DATALENGTH(BinaryData),CONVERT(varchar(64),HASHBYTES('SHA2_256',BinaryData),2) FROM dbo.Files ORDER BY 1,2,3;
"@
$queries['sql-users']="SET NOCOUNT ON; SELECT s.session_id,s.host_process_id,s.program_name,s.status,s.open_transaction_count,r.status,r.command,r.wait_type FROM sys.dm_exec_sessions s LEFT JOIN sys.dm_exec_requests r ON r.session_id=s.session_id WHERE s.database_id=DB_ID(N'$db') OR r.database_id=DB_ID(N'$db'); SELECT state_desc,user_access_desc,recovery_model_desc FROM sys.databases WHERE name=N'$db';"
$queries['markers']="SET NOCOUNT ON; USE [$db]; SELECT 'Config',FileName,PartNo,DataSize,CONVERT(varchar(max),BinaryData,2) FROM dbo.Config WHERE FileName=N'DynamicallyUpdated' OR FileName LIKE N'%[_]dynupdate[_]%' AND DataSize<4096 UNION ALL SELECT 'Params',FileName,PartNo,DataSize,CONVERT(varchar(max),BinaryData,2) FROM dbo.Params WHERE FileName=N'DynamicallyUpdated' OR FileName=N'siVersions' ORDER BY 1,2,3;"
foreach($name in $queries.Keys){$r=Invoke-LiveBounded sqlcmd @('-S','localhost','-E','-C','-b','-f','65001','-W','-h','-1','-s','|','-Q',$queries[$name]) 40;$r.Stdout|Set-Content "$lab/snapshots/$Label-$name.txt" -Encoding utf8;$r.Stderr|Set-Content "$lab/snapshots/$Label-$name.stderr" -Encoding utf8;if($r.ExitCode){throw "$name snapshot failed"}}
$r=Invoke-LiveBounded 'C:/Program Files/1cv8/8.3.27.2214/bin/rac.exe' @('localhost:5545','session','list',"--cluster=$($binding.cluster_uuid)","--infobase=$($binding.infobase_uuid)") 10
$r|ConvertTo-Json|Set-Content "$lab/snapshots/$Label-ras.json" -Encoding utf8;if($r.ExitCode){throw 'RAS snapshot failed'}
"snapshot PASS $Label"
