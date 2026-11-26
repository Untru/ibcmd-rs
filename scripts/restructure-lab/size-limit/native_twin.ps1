# The native side of the twin protocol of docs/apply/restructuring.md 12.6 for the twins of the track "ui" (S1-J):
# the ddl kit's scripts check the database prefix ibcmd_rs_04_ddl_, so these two thin wrappers run the same native
# commands for ibcmd_rs_04_ui_* with the ddl kit's helpers (xe.ps1, native_lock.ps1) and this track's lock name.
#
#   pwsh -NoProfile -File native_twin.ps1 apply  -Database ibcmd_rs_04_ui_s1j_e_nat     # native config apply, under the `native` lock
#   pwsh -NoProfile -File native_twin.ps1 export -Database ibcmd_rs_04_ui_s1j_e_own -Out <dir>   # native config export (read-only)
param(
    [Parameter(Mandatory = $true, Position = 0)][ValidateSet('apply', 'export')][string]$Action,
    [Parameter(Mandatory = $true)][string]$Database,
    [string]$Out = '',
    [int]$TimeoutSec = 3000
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
if ($Database -notmatch '^ibcmd_rs_04_ui_[a-z0-9_]+$') { throw "lab databases of the track ui only (got $Database)" }
$env:DDL_LOCK_TRACK = 'ui'
if (-not $env:DDL_LAB) { $env:DDL_LAB = 'F:\ibcmd\lab\04\ui-codec\s1j\ddlkit' }
New-Item -ItemType Directory -Force $env:DDL_LAB, "$env:DDL_LAB\logs" | Out-Null
$kit = $PSScriptRoot + '\..'
. "$kit\xe.ps1"
$ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
$sw = [Diagnostics.Stopwatch]::StartNew()
if ($Action -eq 'apply') {
    $rows = sqlcmd -S localhost -E -C -h -1 -W -d $Database -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM ConfigSave"
    "ConfigSave rows before: $rows"
    $rc = Invoke-NativeApply -Db $Database -Dynamic 'disable' -TimeoutSec $TimeoutSec -Ibcmd $ibcmd -User (Get-LabUser '8327') -Tag "$Database-apply"
    "native apply exit=$rc in {0:n1}s" -f $sw.Elapsed.TotalSeconds
    Get-Content "$env:DDL_LAB\logs\native-apply-$Database-apply.out" -Tail 6 -ErrorAction SilentlyContinue
    exit $rc
}
if (-not $Out) { throw '-Out is required for export' }
$data = "$env:DDL_LAB\ibdata\$Database"
New-Item -ItemType Directory -Force $data, $Out | Out-Null
$exportArgs = @('infobase', 'config', 'export', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Database",
                "--data=$data", "--user=$(Get-LabUser '8327')", '--threads=4', $Out)
$rc = Invoke-NativeCommand -Ibcmd $ibcmd -Arguments $exportArgs -Log "$env:DDL_LAB\logs\native-export-$Database" -TimeoutSec $TimeoutSec -NoLock
"export exit=$rc in {0:n1}s: {1}" -f $sw.Elapsed.TotalSeconds, ((Get-Content "$env:DDL_LAB\logs\native-export-$Database.out" -Tail 2 -ErrorAction SilentlyContinue) -join ' | ')
exit $rc
