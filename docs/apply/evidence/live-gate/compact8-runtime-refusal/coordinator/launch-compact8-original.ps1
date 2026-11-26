$ErrorActionPreference='Stop'
$lab='F:/ibcmd/lab/05/wave3/load'
$env:IBCMD_RS_COMPACT8_RUNTIME_MANIFEST_SHA='E1CBFE6576104C9B641DAD6D3A3B3908679F670FEB9FA348C4A7E69B50BEC9AD'
. "$lab/compact8-runtime-v1/runtime_closure.ps1"
Assert-Compact8RuntimeClosure -RetainedEvidence
$launch="$lab/compact8-controller-launch-original.json";$outcome="$lab/compact8-controller-original-outcome.json"
if((Test-Path -LiteralPath $launch) -or (Test-Path -LiteralPath $outcome)){throw 'original launch already exists; no retry'}
$p=[Diagnostics.Process]::new();$p.StartInfo=[Diagnostics.ProcessStartInfo]::new()
$p.StartInfo.FileName='C:/Program Files/PowerShell/7/pwsh.exe';$p.StartInfo.UseShellExecute=$false;$p.StartInfo.CreateNoWindow=$true
$p.StartInfo.RedirectStandardInput=$true;$p.StartInfo.RedirectStandardOutput=$true;$p.StartInfo.RedirectStandardError=$true
$argv=@('-NoProfile','-File',"$lab/compact8-runtime-v1/compact8_lifetime_v2.ps1",'-Binary','F:/ibcmd/lab/05/wave3/coordinator/final-packaging-v5/ibcmd-rs-7aebba15-release.exe','-SourceHead','7aebba150e20c9b47b5956f4ca09e6cc62b28bef','-BinarySha256','44513F752CB946644C856B69D740189A045452682989C6E7FC955A59035D0E38','-ManifestSha256','30F33F9BC97B11524412F87E72B12061A7D2C4D19C4D40CCB01B4F2224B63D8E','-ControllerHead','fc5c9e445cffc028aff686a93bcd499959996bdb','-RuntimeManifestSha256','E1CBFE6576104C9B641DAD6D3A3B3908679F670FEB9FA348C4A7E69B50BEC9AD')
foreach($arg in $argv){$p.StartInfo.ArgumentList.Add($arg)}
if(!$p.Start()){throw 'original controller start failed'}
$pidOwn=$p.Id;$birthday=$p.StartTime.ToUniversalTime().ToString('o');$stdout=$p.StandardOutput.ReadToEndAsync();$stderr=$p.StandardError.ReadToEndAsync()
$c=Get-CimInstance Win32_Process -Filter "ProcessId=$pidOwn"
if(!$c -or $c.ParentProcessId -ne $PID -or $c.ExecutablePath -ine $p.StartInfo.FileName.Replace('/','\') -or !$c.CommandLine -or !$c.CreationDate){throw 'original identity unavailable; retain direct handle, no signal'}
$record=[ordered]@{pid=$pidOwn;handle_birth_utc=$birthday;cim_birth_utc=$c.CreationDate.ToUniversalTime().ToString('o');executable=$c.ExecutablePath;command=$c.CommandLine;parent_pid=$c.ParentProcessId;arguments=$argv;stdin='redirected EOF';single_authorized_attempt=$true;started_utc=[DateTime]::UtcNow.ToString('o')}
[IO.File]::WriteAllText($launch,($record|ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
$record|ConvertTo-Json -Depth 5
$p.StandardInput.Close()
$deadline=[DateTime]::UtcNow.AddSeconds(7200)
while(!$p.WaitForExit(1000)){if([DateTime]::UtcNow -gt $deadline){throw 'outer observation deadline; unknown controller retained, no signal/retry'};if($stdout.IsCompleted -and $stderr.IsCompleted -and !$p.HasExited){throw 'unexpected pipes closed while original controller remains live; no signal/retry'}}
if(![Threading.Tasks.Task]::WaitAll([Threading.Tasks.Task[]]@($stdout,$stderr),5000)){throw 'outer pipes completion unproved; retain uncertainty'}
$textOut=$stdout.Result;$textErr=$stderr.Result
[IO.File]::WriteAllText("$lab/compact8-controller-original.stdout.txt",$textOut,[Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText("$lab/compact8-controller-original.stderr.txt",$textErr,[Text.UTF8Encoding]::new($false))
$result=[ordered]@{pid=$pidOwn;handle_birth_utc=$birthday;known_exit=$p.ExitCode;direct_exit_proved=$true;pipes_complete=$true;ended_utc=[DateTime]::UtcNow.ToString('o');automatic_retry=$false}
[IO.File]::WriteAllText($outcome,($result|ConvertTo-Json),[Text.UTF8Encoding]::new($false));$result|ConvertTo-Json
$p.Dispose()
