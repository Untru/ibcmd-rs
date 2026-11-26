# Fast loop on a debug clone: native import of a tree, then native apply (no trace).
#   pwsh -NoProfile -File try_native.ps1 -Database <db> -Tree <dir> [-NoApply] [-Platform 8327|85]
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Tree,
    [switch]$NoApply,
    [ValidateSet('8327', '85')][string]$Platform = '8327'
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
if ($Database -notmatch '^ibcmd_rs_04_ddl_[a-z0-9_]+$') { throw "lab databases only: ibcmd_rs_04_ddl_* (got $Database)" }
. "$PSScriptRoot\native_lock.ps1"
$ibcmd = if ($Platform -eq '85') { 'C:\Program Files\1cv8\8.5.1.1150\bin\ibcmd.exe' } else { 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe' }
$labRoot = if ($env:DDL_LAB) { $env:DDL_LAB } else { 'F:\ibcmd\lab\04\restructure' }
$data = "$labRoot\ibdata\$Database"
New-Item -ItemType Directory -Force $data | Out-Null
$user = Get-LabUser $Platform
$common = @('--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Database", "--data=$data", "--user=$user")
$sw = [Diagnostics.Stopwatch]::StartNew()
# each native command runs under the lab "native" lock, held for that command only
$rc = Invoke-NativeCommand -Ibcmd $ibcmd -Arguments (@('infobase', 'config', 'import') + $common + @($Tree)) -Log "$labRoot\logs\try-import-$Database"
"import exit=$rc in {0:n1}s: {1}" -f $sw.Elapsed.TotalSeconds, ((Get-Content "$labRoot\logs\try-import-$Database.out" -Tail 2 -ErrorAction SilentlyContinue) -join ' | ')
$rows = sqlcmd -S localhost -E -C -h -1 -W -d $Database -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM ConfigSave"
"ConfigSave rows after import: $rows (a complete stage of this configuration is 9842)"
if ($rc -ne 0) { exit 1 }
if ($NoApply) { exit 0 }
$sw.Restart()
$rc = Invoke-NativeCommand -Ibcmd $ibcmd -Arguments (@('infobase', 'config', 'apply') + $common + @('--force', '--dynamic=disable')) -Log "$labRoot\logs\try-apply-$Database"
"apply exit=$rc in {0:n1}s: {1}" -f $sw.Elapsed.TotalSeconds, ((Get-Content "$labRoot\logs\try-apply-$Database.out" -Tail 3 -ErrorAction SilentlyContinue) -join ' | ')
exit $rc
