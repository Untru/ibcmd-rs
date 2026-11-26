param([string]$ManifestSha,[switch]$DefinitionsOnly)
$ErrorActionPreference='Stop'
$lab='F:\ibcmd\lab\05\wave3\platform85';$kit="$lab\tools\extension-v2-kit"
$run="$lab\extension-v4-cleanup-v1";$db='ibcmd_rs_05_p85_w3_ext_version_native_20261001'
$cluster='48225f9f-89e8-427e-aae6-36010395cfc3';$ib='39794873-ac47-4cc7-86da-2dfb0b34a172'
$session='a7b6996c-cf01-4cba-9016-bb32df51a3ae';$lease='track=p85-ext-version-v2 since=2026-10-02T00:23:58'
$held=$true;$registered=$true;$clean=$false;$step=0
function RequireCleanupClosure {
 RequireLabPaths85 @("$lab\logs\extension-control-frozen-v4.json")
 if((Get-FileHash -LiteralPath "$lab\logs\extension-control-frozen-v4.json").Hash -cne 'D4E662C47961B66C268BD3A2E36FEE1B28BA6B2DE0CB4DA6D0F02E18AAB30B5E'){throw 'accepted V4 manifest drift'}
 $base=Get-Content -LiteralPath "$lab\logs\extension-control-frozen-v4.json" -Raw|ConvertFrom-Json
 foreach($f in @($base.files)+@($script:manifest.files)){
  $path=[IO.Path]::GetFullPath($f.file);if($path -cne $f.file){throw 'noncanonical cleanup dependency'}
  for($p=$path;$p;$p=[IO.Path]::GetDirectoryName($p)){if((Get-Item -LiteralPath $p -Force).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'cleanup dependency reparse ancestry'}}
  if((Get-Item -LiteralPath $path).Length -ne $f.length -or (Get-FileHash -LiteralPath $path).Hash -cne $f.sha256){throw 'frozen cleanup dependency/header/source drift'}
 }
 foreach($e in $base.executables.PSObject.Properties){if(@(Get-Command $e.Name -CommandType Application)[0].Source -cne $e.Value){throw 'effective cleanup executable drift'}}
}
function AssertCleanup {
 if(Test-LiveUncertainChild){throw 'uncertain child; retain exact worker and registration'}
 RequireCleanupClosure
}
function ExactLease {
 $path='F:\ibcmd\lab\04\locks\worker\owner.txt'
 for($p=$path;$p;$p=[IO.Path]::GetDirectoryName($p)){if((Get-Item -LiteralPath $p -Force).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'lease ancestry reparse'}}
 if((Get-Content -LiteralPath $path -Raw).Trim() -cne $lease){throw 'exact retained worker lease differs'}
}
function ClientsGone {
 if(Get-CimInstance Win32_Process -Filter 'ProcessId=44392'){throw 'saved client PID exists or was reused; no signal'}
 foreach($p in @(Get-CimInstance Win32_Process)){
  if($p.Name -in @('1cv8.exe','1cv8c.exe') -and $p.CommandLine -and $p.CommandLine.IndexOf($db,[StringComparison]::OrdinalIgnoreCase) -ge 0){throw 'target-looking client still exists'}
 }
}
function BoundContext {
 AssertCleanup;ExactLease;ClientsGone
 $s=State85
 if(!$s -or $s.cluster -cne $cluster -or $s.root -cne "$lab\cluster" -or $s.track -cne 'p85' -or $s.platform -cne '8.5.1.1150' -or ($s.anchors|ConvertTo-Json -Depth 5 -Compress) -cne ($script:savedState.anchors|ConvertTo-Json -Depth 5 -Compress)){throw 'exact original private cluster/anchors differ'}
 foreach($a in $s.anchors){$p=Get-CimInstance Win32_Process -Filter "ProcessId=$($a.pid)";if(!(Same85 $p $a)){throw 'original anchor identity absent/drift'}}
 [void]@(Owned85 $s)
 RequireOwnedNames85 @($db)
}
function Child([string]$Name,[string]$Exe,[string[]]$Arguments,[int]$Seconds=15){
 AssertCleanup;$script:step++;$stem=('{0:D3}-{1}' -f $script:step,$Name)
 $path="$run\$stem.json";RequireLabPaths85 @($path,"$run\$stem.command.json")
 if((Test-Path -LiteralPath $path) -or (Test-Path -LiteralPath "$run\$stem.command.json")){throw 'fresh cleanup evidence required'}
 @{executable=$Exe;arguments=$Arguments;deadline_seconds=$Seconds}|ConvertTo-Json -Depth 4|Set-Content -LiteralPath "$run\$stem.command.json"
 $r=Invoke-LiveBounded $Exe $Arguments $Seconds
 $r|ConvertTo-Json -Depth 4|Set-Content -LiteralPath $path
 AssertCleanup
 if($r.ExitCode -ne 0){throw "cleanup child $Name failed; no retry"}
 return $r
}
function Fields([string]$Text){
 $blocks=@($Text -split '(?:\r?\n){2,}'|Where-Object{$_.Trim()})
 if($blocks.Count -ne 1){throw 'exact sole saved session required'}
 $fields=@{}
 foreach($key in @('session','session-id','infobase','started-at','app-id','hibernate','connection','process')){
  $matches=@($blocks[0] -split '\r?\n'|Where-Object{$_ -match ('^'+[regex]::Escape($key)+'\s*:')})
  if($matches.Count -ne 1){throw 'missing/duplicate saved session field'}
  $fields[$key]=($matches[0] -replace ('^'+[regex]::Escape($key)+'\s*:\s*'),'').Trim()
 }
 return $fields
}
function ExactHibernate([string]$Text){
 $f=Fields $Text;$zero='00000000-0000-0000-0000-000000000000'
 if($f.session -cne $session -or $f['session-id'] -cne '1' -or $f.infobase -cne $ib -or $f['started-at'] -cne '2026-10-02T00:25:15' -or $f['app-id'] -cne '1CV8C' -or $f.hibernate -cne 'yes' -or $f.connection -cne $zero -or $f.process -cne $zero){throw 'saved own hibernated session identity/state differs'}
}
function Registry([switch]$Empty){
 BoundContext
 $r=Child registry "$script:Bin\rac.exe" @('localhost:6545','infobase','summary','list',"--cluster=$cluster")
 if($r.Stderr.Trim()){throw 'registry stderr refused'}
 if($Empty){if($r.Stdout.Trim()){throw 'registration remains after unregister'};return}
 $blocks=@($r.Stdout -split '(?:\r?\n){2,}'|Where-Object{$_.Trim()})
 if($blocks.Count -ne 1){throw 'sole owned registration required'}
 foreach($pair in @(@('name',$db),@('infobase',$ib))){
  $lines=@($blocks[0] -split '\r?\n'|Where-Object{$_ -match ('^'+$pair[0]+'\s*:')})
  if($lines.Count -ne 1 -or ($lines[0] -replace ('^'+$pair[0]+'\s*:\s*'),'').Trim().Trim('"') -cne $pair[1]){throw 'saved exact IB UUID/name differs'}
 }
}
function SessionList {
 BoundContext
 $r=Child sessions "$script:Bin\rac.exe" @('localhost:6545','session','list',"--cluster=$cluster")
 if($r.Stderr.Trim()){throw 'session inventory stderr refused'}
 return $r.Stdout
}
function IdleSql {
 BoundContext
 $query="IF DB_ID(N'$db') IS NULL THROW 57239,'owned DB absent',1; IF EXISTS(SELECT 1 FROM sys.dm_exec_sessions s LEFT JOIN sys.dm_exec_requests r ON s.session_id=r.session_id WHERE s.session_id<>@@SPID AND (s.database_id=DB_ID(N'$db') OR r.database_id=DB_ID(N'$db')) AND (s.open_transaction_count>0 OR r.session_id IS NOT NULL)) THROW 57239,'owned SQL work remains',1; IF EXISTS(SELECT 1 FROM sys.dm_tran_database_transactions d JOIN sys.dm_tran_session_transactions s ON s.transaction_id=d.transaction_id WHERE d.database_id=DB_ID(N'$db') AND s.session_id<>@@SPID) THROW 57239,'owned DB transaction remains',1;"
 [void](Child idle-sql sqlcmd @('-S','localhost','-E','-C','-b','-Q',$query))
}
function Cleanup {
 try{
  Registry;ExactHibernate (SessionList);IdleSql
  # Full six-table bytes/headers and all eight auxiliary tables before any termination.
  [void](Child postfailed-full-snapshot python @("$lab\tools\snapshot_storage.py",$db,'ext_version_native_v4_postfailed') 90)
  Registry;IdleSql;ExactHibernate (SessionList);BoundContext
  [void](Child terminate-exact-saved "$script:Bin\rac.exe" @('localhost:6545','session','terminate',"--cluster=$cluster","--session=$session"))
  $empty=$false
  for($i=0;$i -lt 6;$i++){if(!(SessionList).Trim()){$empty=$true;break};Start-Sleep -Milliseconds 250}
  if(!$empty){throw 'RAS not empty after exact saved termination; retain worker'}
  Registry;IdleSql;BoundContext
  [void](Child unregister-only pwsh @('-NoProfile','-File','F:\ibcmd\lab\04\tools\register-ib.ps1','unregister','-Database',$db,'-Platform','8.5','-Track','p85','-Cluster','p85worker') 120)
  AssertCleanup;$script:registered=$false
  Registry -Empty
  if((SessionList).Trim()){throw 'sessions after unregister; retain worker'}
  BoundContext
  [void](Child guarded-stop-no-purge pwsh @('-NoProfile','-File',"$kit\stop.ps1") 90)
  AssertCleanup
  if(Test-Path -LiteralPath "$lab\cluster\state.json"){throw 'active private state remains after stop'}
  $script:clean=$true;ExactLease;AssertCleanup
  [void](Child release-exact-worker pwsh @('-NoProfile','-File','F:\ibcmd\lab\04\tools\heavy-lock.ps1','release','p85-ext-version-v2','-Name','worker'))
  AssertCleanup;$script:held=$false
 }finally{SaveCleanup}
}
function SaveCleanup {
 @{worker_held_or_unconfirmed=$held;registered_or_unconfirmed=$registered;guarded_stop_clean=$clean;no_purge=$true;no_DB_drop=$true;no_import_retry=$true;receipt_root="$lab\extension-v2-child-receipts"}|ConvertTo-Json|Set-Content -LiteralPath "$run\final-state.json"
}
if($DefinitionsOnly){return}
if($ManifestSha -notmatch '^[a-fA-F0-9]{64}$'){throw 'reviewed cleanup SHA required'}
. "$kit\lib.ps1"
$env:IBCMD_RS_LIVE_CHILD_RECEIPT_ROOT="$lab\extension-v2-child-receipts"
$path="$lab\logs\extension-v4-cleanup-frozen-v1.json";RequireLabPaths85 @($path,$run)
if((Get-Item -LiteralPath $path).Length -gt 1MB -or (Get-FileHash -LiteralPath $path).Hash -ine $ManifestSha){throw 'cleanup manifest drift/budget'}
$script:manifest=Get-Content -LiteralPath $path -Raw|ConvertFrom-Json
if(@($script:manifest.files).Count -gt 100){throw 'cleanup closure bound'}
$script:savedState=Get-Content -LiteralPath "$lab\logs\extension-v4-cleanup-original-state.json" -Raw|ConvertFrom-Json
$previous=Get-Content -LiteralPath "$lab\ext-version-native-v2\final-state.json" -Raw|ConvertFrom-Json
if(!$previous.worker_held_or_unconfirmed -or !$previous.registered -or !$previous.started -or $previous.uncertain_child -or $previous.native_running_unresolved -or $previous.native_hold_held_or_unconfirmed -or $previous.guarded_stop_clean){throw 'scope is only known retained V4 hibernated session'}
$saved=Fields (Get-Content -LiteralPath "$lab\ext-version-native-v2\ext-version-native-old-v2-session-binding.log" -Raw)
if($saved.session -cne $session -or $saved.infobase -cne $ib -or $saved['session-id'] -cne '1' -or $saved['app-id'] -cne '1CV8C' -or $saved['started-at'] -cne '2026-10-02T00:25:15'){throw 'original live journal producer/session binding differs'}
AssertCleanup;ExactLease
if(Test-Path -LiteralPath $run){throw 'fresh cleanup output root required'}
if(Test-Path -LiteralPath "$lab\snapshots\ext_version_native_v4_postfailed"){throw 'fresh full postfailure snapshot required'}
New-Item -ItemType Directory -Path $run|Out-Null
$temp="$run\tmp";New-Item -ItemType Directory -Path $temp|Out-Null;$env:TEMP=$temp;$env:TMP=$temp;$env:PYTHONDONTWRITEBYTECODE='1'
$env:IBCMD_RS_LIVE_CHILD_RECEIPT_ROOT="$lab\extension-v2-child-receipts"
Cleanup
'Exact saved hibernated extension session cleanup complete; database retained.'
