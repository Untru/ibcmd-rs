# Native `config apply` of what is staged in a ddl-track clone, under the lab "native" lock; optional Extended Events
# trace of the database (-Trace <label>: xe\<label>\events.jsonl).
#   pwsh -NoProfile -File apply_only.ps1 -Database <db> [-Platform 8327|85] [-Dynamic disable|force] [-Trace <label>]
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [ValidateSet('8327', '85')][string]$Platform = '8327',
    [string]$Dynamic = 'disable',
    [string]$Trace = ''
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$env:PYTHONUTF8 = '1'; $env:PYTHONIOENCODING = 'utf-8'
if ($Database -notmatch '^ibcmd_rs_04_ddl_[a-z0-9_]+$') { throw "lab databases only: ibcmd_rs_04_ddl_* (got $Database)" }
. "$PSScriptRoot\xe.ps1"
$lab = $script:LabRoot
$ibcmd = if ($Platform -eq '85') { 'C:\Program Files\1cv8\8.5.1.1150\bin\ibcmd.exe' } else { 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe' }
$xeName = if ($Trace) { "ibcmd_rs_04_ddl_$Trace" } else { '' }
$rows = sqlcmd -S localhost -E -C -h -1 -W -d $Database -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM ConfigSave"
"ConfigSave rows before: $rows"
if ($xeName) { Start-DdlXe -Name $xeName -Db $Database | Out-Null }
$sw = [Diagnostics.Stopwatch]::StartNew()
$tag = if ($Trace) { $Trace } else { "$Database-apply-only" }
$rc = Invoke-NativeApply -Db $Database -Dynamic $Dynamic -TimeoutSec 3000 -Ibcmd $ibcmd -User (Get-LabUser $Platform) -Tag $tag
"native apply exit=$rc in {0:n1}s" -f $sw.Elapsed.TotalSeconds
Get-Content "$lab\logs\native-apply-$tag.out" -Tail 6 -ErrorAction SilentlyContinue
if ($xeName) {
    Stop-DdlXe $xeName | Out-Null
    python "$PSScriptRoot\xe_read.py" "C:\temp\ibcmd_rs_04\ddl\$xeName*.xel" "$lab\xe\$Trace"
    Move-Item "C:\temp\ibcmd_rs_04\ddl\$xeName*.xel" "$lab\xe\$Trace\" -Force
}
$rows = sqlcmd -S localhost -E -C -h -1 -W -d $Database -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM ConfigSave"
"ConfigSave rows after: $rows"
exit $rc
