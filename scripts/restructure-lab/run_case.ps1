# One traced native apply on a ddl-track clone, with before/after evidence.
#   pwsh -NoProfile -File run_case.ps1 -Database <db> -Label <case label> -PrevSnap <label of the before snapshot>
# Steps: light snapshot of the staged state, COPY_ONLY backup (twin source), XE session, native
# `config apply --force --dynamic=disable`, XE stop, full snapshot, XE events, snapshot diff.
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Label,
    [Parameter(Mandatory = $true)][string]$PrevSnap,
    [string]$Dynamic = 'disable',
    [ValidateSet('8327', '85')][string]$Platform = '8327',
    [switch]$NoBackup
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'; $env:PYTHONIOENCODING = 'utf-8'
if ($Database -notmatch '^ibcmd_rs_04_ddl_[a-z0-9_]+$') { throw "lab databases only: ibcmd_rs_04_ddl_* (got $Database)" }
. "$PSScriptRoot\xe.ps1"
$lab = if ($env:DDL_LAB) { $env:DDL_LAB } else { 'F:\ibcmd\lab\04\restructure' }
$tools = $PSScriptRoot
$xeName = "ibcmd_rs_04_ddl_$Label"
$sw = [Diagnostics.Stopwatch]::StartNew()
function Step($m) { "{0,7:n1}s  {1}" -f $sw.Elapsed.TotalSeconds, $m }

python "$tools\snapshot.py" $Database "${Label}_staged" --light; Step "staged snapshot (light) done"
if (-not $NoBackup) {
    $bak = "$lab\bak\${Database}_${Label}_staged.bak"
    Invoke-Sql "SET NOCOUNT ON; BACKUP DATABASE [$Database] TO DISK = N'$bak' WITH COPY_ONLY, COMPRESSION, INIT, STATS = 100;" | Select-Object -Last 1
    Step "backup $bak"
}
Start-DdlXe -Name $xeName -Db $Database | Out-Null
Step "xe started $xeName"
$others = (Get-Process ibcmd -ErrorAction SilentlyContinue | ForEach-Object { "$($_.Id)" }) -join ","
Step "other ibcmd processes before apply: [$others]"
$t0 = Get-Date
$ibcmd = if ($Platform -eq '85') { 'C:\Program Files\1cv8\8.5.1.1150\bin\ibcmd.exe' } else { 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe' }
$rc = Invoke-NativeApply -Db $Database -Dynamic $Dynamic -TimeoutSec 3000 -Ibcmd $ibcmd -User (Get-LabUser $Platform)
$t1 = Get-Date
Step ("native apply exit={0} wall={1:n1}s ({2:s} .. {3:s})" -f $rc, ($t1 - $t0).TotalSeconds, $t0, $t1)
$others2 = (Get-Process ibcmd -ErrorAction SilentlyContinue | ForEach-Object { "$($_.Id)" }) -join ","
Step "ibcmd processes after apply: [$others2]"
Stop-DdlXe $xeName | Out-Null
Step "xe stopped"
Copy-Item "$lab\logs\native-apply-$Database.out" "$lab\logs\native-apply-$Label.out.txt" -Force
Copy-Item "$lab\logs\native-apply-$Database.err" "$lab\logs\native-apply-$Label.err.txt" -Force
python "$tools\snapshot.py" $Database "${Label}_after"; Step "after snapshot done"
python "$tools\xe_read.py" "C:\temp\ibcmd_rs_04\ddl\$xeName*.xel" "$lab\xe\$Label"; Step "xe events read"
python "$tools\snapdiff.py" $Database $PrevSnap $Database "${Label}_after" --max 80 | Out-File -Encoding utf8 "$lab\out\diff_$Label.txt"
Step "diff written: $lab\out\diff_$Label.txt"
# keep the .xel small: the events are in xe\<label>\events.jsonl; move the raw file next to them
Move-Item "C:\temp\ibcmd_rs_04\ddl\$xeName*.xel" "$lab\xe\$Label\" -Force
Step "done; exit code of native apply = $rc"
exit $rc

