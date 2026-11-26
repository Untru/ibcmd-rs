# One `ibcmd-rs mssql-apply-source-change` run with wall-clock bookends.
#   pwsh -NoProfile -File apply.ps1 -Database <db> -Src o2 -Mode online -Run online -Tag apply-v2 [-DryRun] [-TailLog <sql-local path>]
# Output (F:\ibcmd\lab\05\online\runs\<Run>\): <Tag>.json (stdout), <Tag>.err (stderr), <Tag>.sql (script), <Tag>.recovery.json,
# <Tag>.meta.txt (start/end UTC, exit code, wall ms). Exit code = the tool's.
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Src,
    [Parameter(Mandatory = $true)][ValidateSet('exclusive', 'online', 'live', 'worker')][string]$Mode,
    [Parameter(Mandatory = $true)][string]$Run,
    [Parameter(Mandatory = $true)][string]$Tag,
    [switch]$DryRun,
    [string]$TailLog = '',
    [ValidateSet('A', 'B', 'C')][string]$ModuleKey = 'A'
)
. F:\ibcmd\lab\05\online\tools\lab-env.ps1
$runDir = Join-Path $Lab "runs\$Run"
New-Item -ItemType Directory -Force -Path $runDir | Out-Null
$cluster = Get-ClusterId
$infobase = Get-InfobaseId $Database
$modulePath = if ($ModuleKey -eq 'C') { 'CommonModules/ОплатаСервисаКлиентПереопределяемый/Ext/Module.bsl' } elseif ($ModuleKey -eq 'B') { 'CommonModules/РаботаСКлассификаторамиКлиентСервер/Ext/Module.bsl' } else { 'CommonModules/ОбсужденияСлужебныйКлиентСервер/Ext/Module.bsl' }
$toolArgs = @(
    'mssql-apply-source-change',
    '--platform-profile', 'platform-8.3.27.2214',
    '--server', 'localhost',
    '--database', $Database,
    '--sqlcmd-trust-cert',
    '--source-root', (Join-Path $Lab "src\$Src"),
    '--path', $modulePath,
    '--mode', $Mode,
    '--script-output', (Join-Path $runDir "$Tag.sql"),
    '--recovery-output', (Join-Path $runDir "$Tag.recovery.json"),
    '--rac', $Rac,
    '--ras-endpoint', $Ras,
    '--cluster-id', $cluster,
    '--infobase-id', $infobase,
    '--infobase-user', 'Администратор'
)
if ($DryRun) { $toolArgs += '--dry-run' } else { $toolArgs += '--allow-non-lab' }
if ($Mode -eq 'live') {
    if (-not $TailLog) { throw '-TailLog is required for live' }
    $toolArgs += @('--tail-log-output', $TailLog)
}
$out = Join-Path $runDir "$Tag.json"
$err = Join-Path $runDir "$Tag.err"
$start = Now-Iso
$sw = [Diagnostics.Stopwatch]::StartNew()
$p = Start-Process -FilePath $Tool -ArgumentList ($toolArgs | ForEach-Object { if ($_ -match '\s') { "`"$_`"" } else { $_ } }) `
    -RedirectStandardOutput $out -RedirectStandardError $err -PassThru -NoNewWindow
if (-not $p.WaitForExit(900000)) { $p.Kill(); throw "apply timed out (15 min): $Tag" }
$sw.Stop()
$end = Now-Iso
$meta = "run=$Run`ntag=$Tag`nmode=$Mode`ndry_run=$($DryRun.IsPresent)`ndatabase=$Database`nsrc=$Src`nstart_utc=$start`nend_utc=$end`nwall_ms=$($sw.ElapsedMilliseconds)`nexit=$($p.ExitCode)"
Set-Content -LiteralPath (Join-Path $runDir "$Tag.meta.txt") -Value $meta -Encoding utf8
$meta
exit $p.ExitCode
