# Builds F:\ibcmd\lab\05\online\observer\IbcmdRsObserver.epf from observer\src with a Designer batch run
# against a throw-away FILE infobase (observer\epfbase); no cluster, no SQL database is touched.
param(
    [string]$Platform = 'C:\Program Files\1cv8\8.3.27.2214\bin',
    [int]$TimeoutSec = 180
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$root = 'F:\ibcmd\lab\05\online\observer'
$v8 = Join-Path $Platform '1cv8.exe'
$base = Join-Path $root 'epfbase'
$log = Join-Path $root 'build.log'
$epf = Join-Path $root 'IbcmdRsObserver.epf'
if (-not (Test-Path -LiteralPath (Join-Path $base '1Cv8.1CD'))) {
    New-Item -ItemType Directory -Force -Path $base | Out-Null
    $p = Start-Process -FilePath $v8 -ArgumentList @('CREATEINFOBASE', "File=`"$base`"", '/Out', "`"$log`"") -PassThru -WindowStyle Hidden
    if (-not $p.WaitForExit($TimeoutSec * 1000)) { $p.Kill(); throw 'CREATEINFOBASE timed out' }
    "createinfobase exit=$($p.ExitCode)"
}
if (Test-Path -LiteralPath $epf) { Remove-Item -LiteralPath $epf }
$args1 = @('DESIGNER', '/F', "`"$base`"", '/DisableStartupMessages', '/DisableStartupDialogs',
    '/LoadExternalDataProcessorOrReportFromFiles', "`"$(Join-Path $root 'src\IbcmdRsObserver.xml')`"", "`"$epf`"",
    '/Out', "`"$log`"", '-NoTruncate')
$p = Start-Process -FilePath $v8 -ArgumentList $args1 -PassThru -WindowStyle Hidden
if (-not $p.WaitForExit($TimeoutSec * 1000)) { $p.Kill(); throw 'DESIGNER timed out' }
"designer exit=$($p.ExitCode)"
if (Test-Path -LiteralPath $log) { Get-Content -LiteralPath $log -Encoding UTF8 -Tail 30 }
if (Test-Path -LiteralPath $epf) { "built: $epf ($((Get-Item -LiteralPath $epf).Length) bytes)" } else { throw 'epf was not produced' }
