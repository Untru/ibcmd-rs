# Case d of S1-G: stage the files of case_d\stage in a pristine БСП clone with the native partial import, snapshot,
# apply natively, snapshot again. Every native command runs under the lab "native" lock (one command per hold).
#   pwsh -NoProfile -File stage_d.ps1 -Database ibcmd_rs_05_trace_d_base [-Steps import,snap1,apply,snap2]
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [string[]]$Steps = @('import', 'snap1', 'apply', 'snap2')
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'
$Steps = @($Steps | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
if ($Database -notmatch '^ibcmd_rs_05_trace_[a-z0-9_]+$') { throw "trace-track lab databases only (got $Database)" }
$env:DDL_LOCK_TRACK = 'trace'
. 'F:\ibcmd\lab\04\restructure\tools\native_lock.ps1'
$k = 'F:\ibcmd\lab\05\s1g\kit'
$case = 'F:\ibcmd\lab\05\s1g\case_d'
$ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
$data = "$k\ibdata\$Database"
New-Item -ItemType Directory -Force $data, "$k\logs", "$k\out" | Out-Null
function Log($m) { "[{0}] {1}" -f (Get-Date -Format s), $m }
$nativeBase = @('--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Database", "--data=$data", '--user=Администратор')

if ($Steps -contains 'import') {
    Log 'native partial import of case d'
    $files = @(Get-Content "$case\files.txt" | Where-Object { $_ })
    $args = @('infobase', 'config', 'import', 'files') + $nativeBase + @("--base-dir=$case\stage", '--partial') + $files
    $rc = Invoke-NativeCommand -Ibcmd $ibcmd -Arguments $args -Log "$k\logs\import-$Database" -TimeoutSec 3600
    Log "import exit=$rc"
    Get-Content "$k\logs\import-$Database.out" -Tail 4 -ErrorAction SilentlyContinue
    Get-Content "$k\logs\import-$Database.err" -Tail 4 -ErrorAction SilentlyContinue
    if ($rc -ne 0) { throw 'native import failed' }
    sqlcmd -S localhost -E -C -h -1 -W -d $Database -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM ConfigSave"
}
if ($Steps -contains 'snap1') {
    Log 'snapshot d_staged'
    Push-Location $k
    python snapshot.py $Database d_staged --light
    Pop-Location
}
if ($Steps -contains 'apply') {
    Log 'native apply'
    $args = @('infobase', 'config', 'apply') + $nativeBase + @('--force', '--dynamic=disable')
    $rc = Invoke-NativeCommand -Ibcmd $ibcmd -Arguments $args -Log "$k\logs\apply-$Database" -TimeoutSec 3600
    Log "apply exit=$rc"
    Get-Content "$k\logs\apply-$Database.out" -Tail 6 -ErrorAction SilentlyContinue
    Get-Content "$k\logs\apply-$Database.err" -Tail 4 -ErrorAction SilentlyContinue
    if ($rc -ne 0) { throw 'native apply failed' }
}
if ($Steps -contains 'snap2') {
    Log 'snapshot d_after'
    Push-Location $k
    python snapshot.py $Database d_after --light
    Pop-Location
}
Log 'done'
