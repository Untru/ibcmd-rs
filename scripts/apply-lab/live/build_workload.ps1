# Builds the bounded BSP invoice/report workload EPF on a disposable F file infobase.
# Runtime client launch remains the unchanged owned obs.ps1 protocol.
param(
    [string]$Platform = 'C:\Program Files\1cv8\8.3.27.2214\bin',
    [string]$LabRoot = 'F:\ibcmd\lab\05\wave3\load',
    [int]$TimeoutSec = 180
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
. "$PSScriptRoot\workload_lab.ps1"
$LabRoot = Resolve-LoadLabRoot $LabRoot
$root = Join-Path $LabRoot 'observer'
$src = Join-Path $PSScriptRoot 'workload\src\IbcmdRsObserver.xml'
$v8 = Join-Path $Platform '1cv8.exe'
$base = Join-Path $root 'epfbase'
$log = Join-Path $root 'build.log'
$epf = Join-Path $root 'IbcmdRsObserver.epf'
New-Item -ItemType Directory -Force -Path $root, (Join-Path $LabRoot 'obs') | Out-Null
if (-not (Test-Path -LiteralPath (Join-Path $base '1Cv8.1CD'))) {
    New-Item -ItemType Directory -Force -Path $base | Out-Null
    $p = Start-Process -FilePath $v8 -ArgumentList @('CREATEINFOBASE', "File=`"$base`"", '/Out', "`"$log`"") -PassThru -WindowStyle Hidden
    if (-not $p.WaitForExit($TimeoutSec * 1000)) { $p.Kill(); $p.WaitForExit(5000) | Out-Null; throw 'CREATEINFOBASE timed out' }
    "createinfobase exit=$($p.ExitCode)"
    if ($p.ExitCode -ne 0) { throw "CREATEINFOBASE failed ($($p.ExitCode))" }
}
if (Test-Path -LiteralPath $epf) { Remove-Item -LiteralPath $epf }
$args1 = @('DESIGNER', '/F', "`"$base`"", '/DisableStartupMessages', '/DisableStartupDialogs',
    '/LoadExternalDataProcessorOrReportFromFiles', "`"$src`"", "`"$epf`"", '/Out', "`"$log`"", '-NoTruncate')
$p = Start-Process -FilePath $v8 -ArgumentList $args1 -PassThru -WindowStyle Hidden
if (-not $p.WaitForExit($TimeoutSec * 1000)) { $p.Kill(); $p.WaitForExit(5000) | Out-Null; throw 'DESIGNER timed out' }
"designer exit=$($p.ExitCode)"
if ($p.ExitCode -ne 0) { throw "DESIGNER failed ($($p.ExitCode))" }
if (Test-Path -LiteralPath $log) { Get-Content -LiteralPath $log -Encoding UTF8 -Tail 30 }
if (Test-Path -LiteralPath $epf) { "built: $epf ($((Get-Item -LiteralPath $epf).Length) bytes)" } else { throw 'epf was not produced' }
