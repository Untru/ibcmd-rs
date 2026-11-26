# Native partial import of a few files into a ddl-track clone (stages them in ConfigSave), under the lab
# "native" lock.
#   pwsh -NoProfile -File import_files.ps1 -Database <db> -BaseDir <dir> -Files Catalogs/X.xml[,more] [-Platform 8327|85]
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$BaseDir,
    [Parameter(Mandatory = $true)][string]$Files,
    [ValidateSet('8327', '85')][string]$Platform = '8327',
    [int]$TimeoutSec = 3600
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
if ($Database -notmatch '^ibcmd_rs_04_ddl_[a-z0-9_]+$') { throw "lab databases only: ibcmd_rs_04_ddl_* (got $Database)" }
. "$PSScriptRoot\native_lock.ps1"
$lab = if ($env:DDL_LAB) { $env:DDL_LAB } else { 'F:\ibcmd\lab\04\restructure' }
$ibcmd = if ($Platform -eq '85') { 'C:\Program Files\1cv8\8.5.1.1150\bin\ibcmd.exe' } else { 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe' }
$data = "$lab\ibdata\$Database"
New-Item -ItemType Directory -Force $data, "$lab\logs" | Out-Null
$importArgs = @('infobase', 'config', 'import', 'files', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Database",
                "--data=$data", "--user=$(Get-LabUser $Platform)", "--base-dir=$BaseDir", '--partial') + ($Files -split ',')
$sw = [Diagnostics.Stopwatch]::StartNew()
$rc = Invoke-NativeCommand -Ibcmd $ibcmd -Arguments $importArgs -Log "$lab\logs\native-import-files-$Database" -TimeoutSec $TimeoutSec
"import files exit=$rc in {0:n1}s: {1}" -f $sw.Elapsed.TotalSeconds, ((Get-Content "$lab\logs\native-import-files-$Database.out" -Tail 3 -ErrorAction SilentlyContinue) -join ' | ')
$rows = sqlcmd -S localhost -E -C -h -1 -W -d $Database -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM ConfigSave"
"ConfigSave rows after import: $rows"
exit $rc
