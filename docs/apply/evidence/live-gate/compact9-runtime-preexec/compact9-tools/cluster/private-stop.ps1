param([switch]$Purge)
$ErrorActionPreference='Stop'
. "$PSScriptRoot\lib.ps1"
if(!$script:PrivateContext){throw 'private stop requires exact wave3 context'}
$state=Read-State;if(!$state){throw 'no ownership state; no process signal or purge permitted'}
Private-RequireLease $state;Private-RequireListeners $state
if($state.cluster){$snapshot=Private-Snapshot $state 'before-stop';if($snapshot.names.Count){throw 'unregister all owned infobases before stopping private cluster'}}
Private-Remember $state
$state.known|ConvertTo-Json -Depth 6|Set-Content (Join-Path $script:Root 'stopping-identities.json')
$rank=@{'ragent.exe'=0;'ras.exe'=1;'rphost.exe'=2;'dbda.exe'=2;'rmngr.exe'=3}
for($pass=0;$pass -lt 5 -and @(Private-Owned $state).Count;$pass++){
 foreach($process in @(Private-Owned $state|Sort-Object{$rank[$_.Name]})){
  Private-RequireLease $state;Private-Remember $state;Private-RequireListeners $state
  $identity=@($state.known|Where-Object{$_.pid -eq $process.ProcessId -and ([DateTime]$_.born).ToUniversalTime().Ticks -eq $process.CreationDate.ToUniversalTime().Ticks})
  $now=Get-CimInstance Win32_Process -Filter "ProcessId=$($process.ProcessId)"
  if(!$now){continue};if($identity.Count -ne 1 -or !(Private-Same $now $identity[0])){throw 'identity changed immediately before signal'}
  Stop-Process -Id ([int]$now.ProcessId) -Force
  Start-Sleep -Milliseconds 500
 }
 Start-Sleep -Milliseconds 500
}
Private-RequireListeners $state
if(@(Private-Owned $state).Count -or (Get-ClusterListeners).Count){throw 'private cluster not clean; ownership state retained'}
Get-OtherServerSnapshot|Set-Content (Join-Path $script:Root 'foreign-after.txt')
if($Purge){
 # Verify both final absolute targets AND every contained reparse point before
 # either delete. A refusal preserves the state and all directories.
 $targets=@($script:Srvinfo,$script:Logs)|ForEach-Object{Require-PrivatePath $_}
 foreach($target in $targets){if(Test-Path -LiteralPath $target){foreach($item in Get-ChildItem -LiteralPath $target -Recurse -Force){[void](Require-PrivatePath $item.FullName)}}}
 foreach($target in $targets){if(Test-Path -LiteralPath $target){Remove-Item -LiteralPath $target -Recurse -Force}}
}
$archive=Join-Path $script:Root ('stopped-state-'+(Get-Date -Format yyyyMMddHHmmssfff)+'.json');[void](Require-PrivatePath $archive)
Move-Item -LiteralPath $script:StateFile -Destination $archive
'isolated83 clean; ownership state archived (no foreign process signalled)'
