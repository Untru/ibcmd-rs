$ErrorActionPreference='Stop'
$root='F:/ibcmd/lab/05/wave3/load';$name='compact7-runtime-v3-executor'
$receipt="$root/$name-origin.json";$result="$root/$name-result.json"
foreach($path in @($receipt,$result,"$root/logs/$name.stdout.txt","$root/logs/$name.stderr.txt")){if(Test-Path -LiteralPath $path){throw 'Fresh one-shot executor artifacts required'}}
$exe='C:/Program Files/PowerShell/7/pwsh.exe'
$arguments=@('-NoProfile','-File','F:/ibcmd/lab/05/wave3/load/compact7-runtime-v3/compact7_lifetime_v2.ps1','-Binary','F:/ibcmd/lab/05/wave3/coordinator/ibcmd-rs-fbf2d743-debug.exe','-SourceHead','fbf2d7437b8cdcd184ae49bc96acdb2115ac97d2','-BinarySha256','81CC4526E860EDB9412D60ECFBDF2EED71376D40DBE2ADA648D4CCFFD7077AE0','-ManifestSha256','30F33F9BC97B11524412F87E72B12061A7D2C4D19C4D40CCB01B4F2224B63D8E','-ControllerHead','ad9f1e2dbda5de78a09af6a5fc06f6f775708922','-RuntimeManifestSha256','781CFD57A8E27E918D7D1FF167692A11F46A1E76173878E8835F171C6C87202A')
$p=Start-Process -FilePath $exe -ArgumentList $arguments -WindowStyle Hidden -PassThru -RedirectStandardOutput "$root/logs/$name.stdout.txt" -RedirectStandardError "$root/logs/$name.stderr.txt"
$identity=Get-CimInstance Win32_Process -Filter "ProcessId=$($p.Id)"
$origin=@{executor_pid=$PID;controller_pid=$p.Id;birth_utc=$p.StartTime.ToUniversalTime().ToString('o');birth_ticks=$p.StartTime.ToUniversalTime().Ticks;expected_executable=$exe;arguments=$arguments;launch_utc=[DateTime]::UtcNow.ToString('o');native_authorized_one_attempt=$true;timeout_seconds=7200}
if($identity){$origin.actual_executable=$identity.ExecutablePath;$origin.command_line=$identity.CommandLine;$origin.parent_pid=$identity.ParentProcessId;$origin.cim_birth_utc=$identity.CreationDate.ToUniversalTime().ToString('o')}
$origin|ConvertTo-Json -Depth 5|Set-Content -LiteralPath $receipt -Encoding utf8
'ORIGIN '+($origin|ConvertTo-Json -Depth 5 -Compress)
$deadline=[DateTime]::UtcNow.AddSeconds(7200)
while(!$p.WaitForExit(1000)){
 if([DateTime]::UtcNow -ge $deadline){@{controller_pid=$p.Id;direct_exit_proved=$false;unknown_running=$true;no_signal=$true;retain_owned_lifecycle=$true;ended_utc=[DateTime]::UtcNow.ToString('o')}|ConvertTo-Json|Set-Content -LiteralPath $result -Encoding utf8;throw 'Original controller deadline; no signal/release; lifecycle retained'}
}
@{controller_pid=$p.Id;direct_exit_proved=$true;exit_code=$p.ExitCode;ended_utc=[DateTime]::UtcNow.ToString('o');no_signal=$true}|ConvertTo-Json|Set-Content -LiteralPath $result -Encoding utf8
'CONTROLLER_EXIT '+$p.ExitCode
$rc=$p.ExitCode;$p.Dispose();exit $rc
