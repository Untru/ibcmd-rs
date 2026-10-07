$ErrorActionPreference='Stop'
$lab='F:/ibcmd/lab/05/wave3/load'
$env:IBCMD_RS_COMPACT8_RUNTIME_MANIFEST_SHA='E1CBFE6576104C9B641DAD6D3A3B3908679F670FEB9FA348C4A7E69B50BEC9AD'
$env:IBCMD_RS_LIVE_CHILD_RECEIPT_ROOT="$lab/compact8-child-receipts"
. "$lab/compact8-runtime-v1/compact8-tools/live/process.ps1"
$outcome=Read-Compact8RuntimeJson "$lab/compact8-controller-original-outcome.json"
if($outcome.known_exit -ne 1 -or !$outcome.direct_exit_proved -or !$outcome.pipes_complete){throw 'original controller terminal proof differs'}
$db='ibcmd_rs_05_load_w3_compact8_20261002'
$r=Invoke-LiveBounded sqlcmd @('-S','localhost','-E','-C','-b','-h','-1','-Q',"SET NOCOUNT ON; SELECT CASE WHEN DB_ID(N'$db') IS NULL THEN N'DB_ABSENT' ELSE N'DB_PRESENT' END;") 20
if($r.ExitCode -or $r.Stdout.Trim() -cne 'DB_ABSENT'){throw 'fresh failed clone absent proof refused'}
$root="$lab/evidence/compact6-case5-prior"
$stack=[Collections.Generic.Stack[string]]::new();$stack.Push($root)
$files=0;$dirs=0;$bytes=0L;$maxNodes=65536
while($stack.Count){
 $path=$stack.Pop();$dirs++;if($files+$dirs -gt $maxNodes){throw 'readonly census metadata budget'}
 [void](Assert-Compact8RuntimePath $path)
 foreach($item in Get-ChildItem -LiteralPath $path -Force){
  if($item.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'readonly tree reparse'}
  if($item -is [IO.DirectoryInfo]){$stack.Push($item.FullName)}elseif($item -is [IO.FileInfo]){$files++;$bytes+=$item.Length}else{throw 'unknown filesystem member'}
 }
}
$state=Test-Path -LiteralPath "$lab/cluster/state.json"
$binding=Test-Path -LiteralPath "$lab/compact8-binding.json"
$unknown=Test-LiveUncertainChild
$own=@{};foreach($name in @('worker','heavy','native')){
 $p="F:/ibcmd/lab/04/locks/$name/owner.txt"
 $matching=$false
 if(Test-Path -LiteralPath $p){$m=Get-Item -LiteralPath $p -Force;if($m -isnot [IO.FileInfo] -or $m.Length -gt 128 -or $m.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'owner bounded metadata refused'};$raw=[IO.File]::ReadAllBytes($p);$matching=[Text.Encoding]::ASCII.GetString($raw) -match '(?i)track=load8-f8bd7d065a2c(?:\s|$)'}
 $own[$name]=$matching
}
$alive=@(Get-CimInstance Win32_Process -Filter 'ProcessId=112180')
if($state -or $binding -or $unknown -or @($own.Values|Where-Object{$_}).Count -or $alive.Count){throw 'terminal resources not established'}
$proof=[ordered]@{scope='READONLY terminal reconciliation; no retry/guard edit/archive move/delete';original_known_exit=1;database=$db;database_absent=$true;database_readonly_result=$r;private_active_state_absent=$true;registration_binding_absent=$true;original_controller_pid_absent=$true;unknown_receipts_absent=$true;own_fifo_holds=$own;archive_root=$root;archive_metadata_census=@{ordinary_files=$files;directories_including_root=$dirs;total_nodes=$files+$dirs;file_bytes=$bytes;no_reparse=$true;metadata_only=$true;original_limit=2048};restore_dispatched=$false;cluster_started=$false;registered=$false;heavy_acquire_dispatched=$false;native_write_dispatched=$false;phase1_executed=$false;cycle2_executed=$false;old_temp_preserved=$true}
$path="$lab/compact8-terminal-reconciliation-v1.json"
$f=[IO.File]::Open($path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None);try{$b=[Text.UTF8Encoding]::new($false).GetBytes(($proof|ConvertTo-Json -Depth 8));$f.Write($b)}finally{$f.Dispose()}
$proof|ConvertTo-Json -Depth 8
