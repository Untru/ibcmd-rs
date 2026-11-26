# One `ibcmd-rs mssql-activate-staged-main` run (the staged ConfigSave is activated in a FRESH process) with wall-clock bookends.
#   pwsh -NoProfile -File act.ps1 -Database <db> -Mode online -Run online -Tag act-v2 [-DryRun] [-TailLog <sql-local path>]
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][ValidateSet('exclusive', 'online', 'live', 'worker')][string]$Mode,
    [Parameter(Mandatory = $true)][string]$Run,
    [Parameter(Mandatory = $true)][string]$Tag,
    [switch]$DryRun,
    [string]$TailLog = ''
)
. F:\ibcmd\lab\05\online\tools\lab-env.ps1
$runDir = Join-Path $Lab "runs\$Run"
New-Item -ItemType Directory -Force -Path $runDir | Out-Null
$cluster = Get-ClusterId
$infobase = Get-InfobaseId $Database
$toolArgs = @(
    'mssql-activate-staged-main',
    '--platform-profile', 'platform-8.3.27.2214',
    '--sqlcmd-trust-cert',
    '--server', 'localhost',
    '--database', $Database,
    '--mode', $Mode,
    '--script-output', (Join-Path $runDir "$Tag.sql"),
    '--recovery-output', (Join-Path $runDir "$Tag.recovery.json"),
    '--rac', $Rac,
    '--ras-endpoint', $Ras,
    '--cluster-id', $cluster,
    '--infobase-id', $infobase,
    '--infobase-user', 'Администратор'
)
if ($DryRun) { $toolArgs += '--dry-run' }
$toolArgs += '--allow-non-lab'
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
if (-not $p.WaitForExit(900000)) { $p.Kill(); throw "activation timed out (15 min): $Tag" }
$sw.Stop()
$end = Now-Iso
$meta = "run=$Run`ntag=$Tag`nmode=$Mode`ndry_run=$($DryRun.IsPresent)`ndatabase=$Database`nstart_utc=$start`nend_utc=$end`nwall_ms=$($sw.ElapsedMilliseconds)`nexit=$($p.ExitCode)"
Set-Content -LiteralPath (Join-Path $runDir "$Tag.meta.txt") -Value $meta -Encoding utf8
$meta
exit $p.ExitCode
