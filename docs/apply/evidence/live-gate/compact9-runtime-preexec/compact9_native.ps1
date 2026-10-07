# One bounded native write per FIFO hold, to a restored Track load clone only.
param(
    [Parameter(Mandatory)][ValidateSet('import', 'force')][string]$Action,
    [Parameter(Mandatory)][ValidateSet('ibcmd_rs_05_load_w3_compact9_20261002')][string]$Database,
    [Parameter(Mandatory)][string]$LabRoot,
    [Parameter(Mandatory)][ValidatePattern('^[a-z0-9-]+$')][string]$Label,
    [string]$Tree = '',
    [ValidateSet('disable','force')][string]$DynamicMode = 'force',
    [int]$TimeoutSec = 120
)
$ErrorActionPreference = 'Stop'
. "F:/ibcmd/lab/05/wave3/load/compact9-runtime-v1/compact9-tools/live/process.ps1"
. "F:/ibcmd/lab/05/wave3/load/compact9-runtime-v1/compact9-tools/live/workload_lab.ps1"
$lab = Resolve-LoadLabRoot $LabRoot
if(Test-LiveUncertainChild){throw 'unresolved prior child; no new native write'}
$artifacts = @('command.json', 'lock.log', 'log', 'child-times.json') | ForEach-Object { Join-Path $lab "$Label.$_" }
foreach ($artifact in $artifacts) {
    if (Test-Path -LiteralPath $artifact) { throw 'native label artifacts already exist; use a fresh label' }
}
$owned = Import-Csv -LiteralPath 'F:\ibcmd\lab\04\databases.tsv' -Delimiter "`t" |
    Where-Object { $_.database -ceq $Database -and $_.track -ceq 'load' -and (Test-LoadRestoreOrigin $_.notes $lab) }
if (-not $owned) { throw 'database is not a restored Track load clone in the ownership manifest' }
$argv = @('infobase','config', $(if ($Action -eq 'import') {'import'} else {'apply'}))
if ($Action -eq 'import') { $argv += 'files' }
$argv += @('--dbms=MSSQLServer','--db-server=localhost',"--db-name=$Database", "--data=$lab\ibdata\$Database",'--user=Администратор')
if ($Action -eq 'import') {
    $treePath = [IO.Path]::GetFullPath($Tree)
    if (-not $treePath.StartsWith($lab + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'source tree must be owned by this lab' }
    $argv += @("--base-dir=$treePath", '--partial', 'CommonModules/ОбсужденияСлужебныйКлиентСервер/Ext/Module.bsl')
} else { $argv += @('--force',"--dynamic=$DynamicMode") }
New-Item -ItemType Directory -Force $lab | Out-Null
$native = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
@{ executable=$native; arguments=$argv; deadline_seconds=$TimeoutSec } | ConvertTo-Json -Depth 4 |
    Set-Content -LiteralPath "$lab\$Label.command.json" -Encoding utf8
$lockTool = 'F:/ibcmd/lab/04/tools/heavy-lock.ps1'
$lock = Invoke-LiveBounded pwsh @('-NoProfile','-File',$lockTool,'acquire','load9-983ad8f3eae1','-Name','native','-TimeoutMin','10') 620
$lock.Stdout + $lock.Stderr | Set-Content -LiteralPath "$lab\$Label.lock.log" -Encoding utf8
if ($lock.ExitCode -ne 0) { throw 'native FIFO acquisition failed' }
function New-Compact9NativeProcess($StartInfo){$process=[Diagnostics.Process]::new();$process.StartInfo=$StartInfo;return $process}
function Invoke-Compact9Native {
 $psi=[Diagnostics.ProcessStartInfo]::new($native);$psi.UseShellExecute=$false;$psi.CreateNoWindow=$true;$psi.RedirectStandardOutput=$true;$psi.RedirectStandardError=$true;$psi.RedirectStandardInput=$true
 foreach($arg in $argv){$psi.ArgumentList.Add($arg)}
 $p=New-Compact9NativeProcess $psi;$started=$false;$reapAttempted=$false;$pipesUnproved=$false;$script:NativeChildExitProved=$false;$times=@{executable=$native;arguments=$argv;deadline_seconds=$TimeoutSec}
 try {
  Assert-Compact9RuntimeClosure
  if(!$p.Start()){throw 'native child failed start'};$started=$true;$times.pid=$p.Id;$times.actual_child_start_utc=$p.StartTime.ToUniversalTime().ToString('o')
  $spawn=Get-CimInstance Win32_Process -Filter "ProcessId=$($p.Id)";if(!$spawn -or $spawn.ExecutablePath -cne $native -or $spawn.ParentProcessId -ne $PID){if(!$p.HasExited){throw 'exact spawned native identity unavailable'}}else{$times.spawn_birth=$spawn.CreationDate.ToUniversalTime().ToString('o');$times.spawn_executable=$spawn.ExecutablePath;$times.spawn_command_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($spawn.CommandLine)))}
  $pipesUnproved=$true;$out=$p.StandardOutput.ReadToEndAsync();$err=$p.StandardError.ReadToEndAsync();$p.StandardInput.Close()
  if(!$p.WaitForExit($TimeoutSec*1000)){$times.timeout=$true;$now=Get-CimInstance Win32_Process -Filter "ProcessId=$($p.Id)";if(!$now -or $now.CreationDate.ToUniversalTime().Ticks -ne $spawn.CreationDate.ToUniversalTime().Ticks -or $now.ExecutablePath -cne $spawn.ExecutablePath -or $now.CommandLine -cne $spawn.CommandLine -or $now.ParentProcessId -ne $spawn.ParentProcessId){throw 'native timeout identity changed; no signal permitted'};if(!$p.HasExited){$p.Kill()};$reapAttempted=$true;$times.reap_completed=$p.WaitForExit(5000);if(!$times.reap_completed){throw 'native exact child reap exceeded5000ms; retained identity requires cleanup'};$script:NativeChildExitProved=$true;throw 'exact spawned native child deadline; preserve failure'}
  $script:NativeChildExitProved=$true
  if(!$out.Wait(5000) -or !$err.Wait(5000)){throw 'native output pipes did not close within deadline'}
  $pipesUnproved=$false
  [pscustomobject]@{ExitCode=$p.ExitCode;Stdout=$out.GetAwaiter().GetResult();Stderr=$err.GetAwaiter().GetResult()}
 } catch {
  $times.failure=$_.Exception.Message;throw
 } finally {
  if(!$started){$script:NativeChildExitProved=$true}elseif(!$script:NativeChildExitProved -and !$reapAttempted -and $p.HasExited){$script:NativeChildExitProved=$p.WaitForExit(5000)}
  $times.direct_child_exit_proved=$script:NativeChildExitProved
  $script:NativeWriterCompletionProved=(!$started -or ($script:NativeChildExitProved -and !$pipesUnproved -and !$times.timeout));$times.writer_completion_proved=$script:NativeWriterCompletionProved
  if(!$script:NativeWriterCompletionProved){$script:LiveBoundedUncertainChild=$true}
  if($times.ContainsKey('pid') -and $p.HasExited){$times.actual_child_exit_utc=$p.ExitTime.ToUniversalTime().ToString('o');$times.exit_code=$p.ExitCode}
  $times|ConvertTo-Json -Depth 5|Set-Content -LiteralPath "$lab/$Label.child-times.json" -Encoding utf8
  $p.Dispose()
 }
}
function Complete-Compact9NativeHold {
    if(!$script:NativeWriterCompletionProved -or (Test-LiveUncertainChild)){$script:LiveBoundedUncertainChild=$true;try{Save-LiveUncertainChild ([ordered]@{execution_state='native_writer_completion_unproved';child_times="$lab/$Label.child-times.json";direct_child_exit_proved=$script:NativeChildExitProved})}catch{};@{native_hold_retained=$true;child_exit_proved=$script:NativeChildExitProved;child_times="$lab/$Label.child-times.json";reason='direct writer completion unproved; no new writer or lifecycle cleanup'}|ConvertTo-Json|Set-Content -LiteralPath "$lab/$Label.native-hold-retained.json";throw 'native writer completion unproved; native FIFO retained'}
    $release=Invoke-LiveBounded pwsh @('-NoProfile','-File',$lockTool,'release','load9-983ad8f3eae1','-Name','native') 15
    if ($release.ExitCode -ne 0) { throw 'native FIFO release failed' }
}
try {
    $result = Invoke-Compact9Native
    $result.Stdout + $result.Stderr | Set-Content -LiteralPath "$lab\$Label.log" -Encoding utf8
    if ($result.ExitCode -ne 0) { throw "native $Action failed, exit $($result.ExitCode); see $Label.log" }
    "native $Action PASS: $Label"
} finally {
    Complete-Compact9NativeHold
}




