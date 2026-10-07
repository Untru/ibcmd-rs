param([string]$Lab='F:/ibcmd/lab/05/wave3/load')
$ErrorActionPreference='Stop';$wt='F:/ibcmd/src/ibcmd-rs-05-load-wave3';. "$wt/scripts/apply-lab/live/process.ps1"
$ib='910a5f6b-e788-4dbe-a6f9-aad6d808bd89';$db='ibcmd_rs_05_load_w3_work_20261001'
$r=Invoke-LiveBounded 'C:/Program Files/1cv8/8.3.27.2214/bin/rac.exe' @('localhost:2545','session','list','--cluster=24c580ef-d5de-4b78-b204-b94b64eb2fae',"--infobase=$ib") 10
$r|ConvertTo-Json|Set-Content "$Lab/logs/current-ownership-ras.json" -Encoding utf8;if($r.ExitCode){throw 'inventory failed'}
$records=@()
foreach($file in Get-ChildItem "$Lab/obs" -Filter '*.pid'){
$label=$file.BaseName;$id=[int](Get-Content -LiteralPath $file.FullName);$p=Get-CimInstance Win32_Process -Filter "ProcessId=$id";if(!$p){continue};if($p.CommandLine -notmatch [regex]::Escape($db) -or $p.CommandLine -notmatch [regex]::Escape("$label;poll;") -or $p.Name -ne '1cv8c.exe'){throw 'client PID reuse/ownership drift'}
$first=Get-Content "$Lab/obs/$label.log" -Head 1;$sid=($first -split '\|')[3];$b=@(($r.Stdout -split '(?:\r?\n){2,}')|Where-Object {$_ -match "session-id\s*:\s*$sid\s" -and $_ -match "infobase\s*:\s*$ib"});if($b.Count -ne 1 -or $b[0] -notmatch 'session\s*:\s*([0-9a-f-]{36})'){throw 'unique journal session missing'};$uuid=$Matches[1];if($b[0] -notmatch 'started-at\s*:\s*(\S+)'){throw 'missing start'};$start=$Matches[1];$journalUtc=[DateTime]::new(([long]($first -split '\|')[0])*10000,[DateTimeKind]::Utc);$startedUtc=([DateTime]::Parse($start)).ToUniversalTime();if([Math]::Abs(($journalUtc-$startedUtc).TotalSeconds) -gt 90){throw 'session ID reuse/start mismatch'}
$records+=@{label=$label;pid=$id;process_creation=$p.CreationDate.ToString('o');command_line=$p.CommandLine;session_id=$sid;session_uuid=$uuid;infobase_uuid=$ib;ras_started_at=$start;journal_open=$first;epf_sha256=(Get-FileHash "$Lab/observer/IbcmdRsObserver.epf").Hash}
}
$records|ConvertTo-Json -Depth 6|Set-Content "$Lab/current-ownership.json" -Encoding utf8
$o=Get-Content "$Lab/OWNED.json" -Raw|ConvertFrom-Json;$o.processes=$records;$o.sessions=$records;$o|ConvertTo-Json -Depth 8|Set-Content "$Lab/OWNED.json" -Encoding utf8
"ownership PASS count=$($records.Count)"
