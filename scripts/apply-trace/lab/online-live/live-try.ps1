# One lean LIVE attempt on the LIVE clone: fresh single WARM session, apply (single process, markers must be absent), NEW session after.
#   pwsh -NoProfile -File live-try.ps1 -N 3
param([Parameter(Mandatory = $true)][int]$N)
. F:\ibcmd\lab\05\online\tools\lab-env.ps1
$db = 'ibcmd_rs_05_online_b1'
$src = "l$N"
& python "$Lab\tools\make_versions.py" "$src=ibcmd-live-v$N" | Out-Null
& pwsh -NoProfile -File "$Lab\tools\obs-stop.ps1" -All | Out-Null
foreach ($p in (Get-CimInstance Win32_Process -Filter "Name='1cv8c.exe'")) { if ($p.CommandLine -match $db) { Stop-Process -Id $p.ProcessId -Force } }
& pwsh -NoProfile -File "$Lab\tools\sessions-clean.ps1" -Database $db | Out-Null
$warm = "lv$N-warm"
& pwsh -NoProfile -File "$Lab\tools\obs-start.ps1" -Database $db -Label $warm -Mode poll -TimeoutSec 240
Start-Sleep -Seconds 20
& pwsh -NoProfile -File "$Lab\tools\apply.ps1" -Database $db -Src $src -Mode live -TailLog "$Lab\bak\ibcmd_rs_05_online_b1_tail$N.trn" -Run live -Tag "live-v$N"
"exit=$LASTEXITCODE"
