# Native `config export` of a ddl-track clone into a folder (no lock: an export writes nothing to the database).
#   pwsh -NoProfile -File export_tree.ps1 -Database <db> -Out <dir> [-Platform 8327|85] [-Threads 4]
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Out,
    [ValidateSet('8327', '85')][string]$Platform = '8327',
    [int]$Threads = 4,
    [int]$TimeoutSec = 3600
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
if ($Database -notmatch '^ibcmd_rs_04_ddl_[a-z0-9_]+$') { throw "lab databases only: ibcmd_rs_04_ddl_* (got $Database)" }
. "$PSScriptRoot\native_lock.ps1"
$lab = if ($env:DDL_LAB) { $env:DDL_LAB } else { 'F:\ibcmd\lab\04\restructure' }
$ibcmd = if ($Platform -eq '85') { 'C:\Program Files\1cv8\8.5.1.1150\bin\ibcmd.exe' } else { 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe' }
$data = "$lab\ibdata\$Database"
New-Item -ItemType Directory -Force $data, $Out | Out-Null
$exportArgs = @('infobase', 'config', 'export', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Database",
                "--data=$data", "--user=$(Get-LabUser $Platform)", "--threads=$Threads", $Out)
$sw = [Diagnostics.Stopwatch]::StartNew()
$rc = Invoke-NativeCommand -Ibcmd $ibcmd -Arguments $exportArgs -Log "$lab\logs\native-export-$Database" -TimeoutSec $TimeoutSec -NoLock
"export exit=$rc in {0:n1}s: {1}" -f $sw.Elapsed.TotalSeconds, ((Get-Content "$lab\logs\native-export-$Database.out" -Tail 2 -ErrorAction SilentlyContinue) -join ' | ')
exit $rc
