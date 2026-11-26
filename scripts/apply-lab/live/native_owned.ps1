# One bounded native write per FIFO hold, to a restored Track live clone only.
param(
    [Parameter(Mandatory)][ValidateSet('import', 'force')][string]$Action,
    [Parameter(Mandatory)][ValidatePattern('^ibcmd_rs_05_live_[a-z0-9_]+$')][string]$Database,
    [Parameter(Mandatory)][string]$LabRoot,
    [Parameter(Mandatory)][ValidatePattern('^[a-z0-9-]+$')][string]$Label,
    [string]$Tree = '',
    [int]$TimeoutSec = 120
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\process.ps1"
$lab = [IO.Path]::GetFullPath($LabRoot).TrimEnd('\')
if (-not $lab.StartsWith('F:\ibcmd\lab\05\', [StringComparison]::OrdinalIgnoreCase)) { throw 'output must stay in the F: live lab' }
$owned = Import-Csv -LiteralPath 'F:\ibcmd\lab\04\databases.tsv' -Delimiter "`t" |
    Where-Object { $_.database -eq $Database -and $_.track -eq 'live' -and $_.notes -like 'from F:\ibcmd\lab\dbbak\*' }
if (-not $owned) { throw 'database is not a restored Track live clone in the ownership manifest' }
$argv = @('infobase','config', $(if ($Action -eq 'import') {'import'} else {'apply'}))
if ($Action -eq 'import') { $argv += 'files' }
$argv += @('--dbms=MSSQLServer','--db-server=localhost',"--db-name=$Database", "--data=$lab\ibdata\$Database",'--user=Администратор')
if ($Action -eq 'import') {
    $treePath = [IO.Path]::GetFullPath($Tree)
    if (-not $treePath.StartsWith($lab + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'source tree must be owned by this lab' }
    $argv += @("--base-dir=$treePath", '--partial', 'CommonModules/ОбсужденияСлужебныйКлиентСервер/Ext/Module.bsl')
} else { $argv += @('--force','--dynamic=disable') }
New-Item -ItemType Directory -Force $lab | Out-Null
$native = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
@{ executable=$native; arguments=$argv; deadline_seconds=$TimeoutSec } | ConvertTo-Json -Depth 4 |
    Set-Content -LiteralPath "$lab\$Label.command.json" -Encoding utf8
$lockTool = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$lock = Invoke-LiveBounded pwsh @('-NoProfile','-File',$lockTool,'acquire','live','-Name','native','-TimeoutMin','10') 620
$lock.Stdout + $lock.Stderr | Set-Content -LiteralPath "$lab\$Label.lock.log" -Encoding utf8
if ($lock.ExitCode -ne 0) { throw 'native FIFO acquisition failed' }
try {
    $result = Invoke-LiveBounded $native $argv $TimeoutSec
    $result.Stdout + $result.Stderr | Set-Content -LiteralPath "$lab\$Label.log" -Encoding utf8
    if ($result.ExitCode -ne 0) { throw "native $Action failed, exit $($result.ExitCode); see $Label.log" }
    "native $Action PASS: $Label"
} finally {
    & pwsh -NoProfile -File $lockTool release live -Name native
    if ($LASTEXITCODE -ne 0) { throw 'native FIFO release failed' }
}
