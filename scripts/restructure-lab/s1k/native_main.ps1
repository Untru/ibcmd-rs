# native_main.ps1 -Database <db> -Action ImportFiles|Apply|ImportApply|Export [-BaseDir <dir> -Files a.xml,b.xml] [-Out <dir>] [-Platform 8327|85]
# Native ibcmd on the MAIN configuration of an ext-track lab clone (the ext track's tool; paths under S1K_LAB). Writes (ImportFiles, Apply,
# ImportApply) run inside the native write lock (heavy-lock.ps1 acquire ext -Name native ... release), ONE command per hold -- except
# ImportApply, the import and the apply that follows it on the same clone in one hold (as the import track's lab does: the queue of the
# lock is long, and a stage waiting for its own apply holds up nobody); Export is read-only.
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][ValidateSet('ImportFiles', 'Apply', 'ImportApply', 'Export')][string]$Action,
    [string]$BaseDir = '',
    [string]$Files = '',
    [string]$Out = '',
    [ValidateSet('8327', '85')][string]$Platform = '8327',
    [string]$User = 'Администратор',
    [int]$TimeoutMin = 40,
    [int]$LockTimeoutMin = 240
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
if ($Database -notmatch '^ibcmd_rs_05_ext_') { throw "database $Database is not an ext-track clone" }
$ibcmd = if ($Platform -eq '8327') { 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe' } else { 'C:\Program Files\1cv8\8.5.1.1150\bin\ibcmd.exe' }
$lab = if ($env:S1K_LAB) { $env:S1K_LAB } else { 'F:\ibcmd\lab\05\ext\s1k' }
$data = "$lab\ibdata\$Database"
New-Item -ItemType Directory -Force $data | Out-Null
$logDir = "$lab\act"
New-Item -ItemType Directory -Force $logDir | Out-Null
$emptyStdin = "$logDir\empty_stdin.txt"
if (-not (Test-Path $emptyStdin)) { New-Item -ItemType File -Force $emptyStdin | Out-Null }
$stamp = Get-Date -Format 'HHmmss'
$common = @('--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Database", "--user=`"$User`"", "--data=$data")
$importArgs = @('infobase', 'config', 'import', 'files') + $common + @("--base-dir=`"$BaseDir`"", '--partial') + ($Files -split ',')
$applyArgs = @('infobase', 'config', 'apply') + $common + @('--force', '--dynamic=disable')
$steps = @()
if ($Action -in 'ImportFiles', 'ImportApply') { $steps += , @('ImportFiles', $importArgs) }
if ($Action -in 'Apply', 'ImportApply') { $steps += , @('Apply', $applyArgs) }
if ($Action -eq 'Export') {
    New-Item -ItemType Directory -Force $Out | Out-Null
    $steps += , @('Export', (@('infobase', 'config', 'export') + $common + @('--force', $Out)))
}
$lock = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$writes = $Action -ne 'Export'
if ($writes) {
    & pwsh -NoProfile -File $lock acquire ext -Name native -TimeoutMin $LockTimeoutMin
    if ($LASTEXITCODE -ne 0) { throw 'native lock not acquired' }
}
$failed = $false
try {
    foreach ($step in $steps) {
        $name = $step[0]
        $args1 = $step[1]
        $log = Join-Path $logDir "native_${name}_${Database}_$stamp.log"
        $t0 = Get-Date
        $p = Start-Process -FilePath $ibcmd -ArgumentList $args1 -NoNewWindow -PassThru -RedirectStandardOutput $log -RedirectStandardError "$log.err" -RedirectStandardInput $emptyStdin
        if (-not $p.WaitForExit($TimeoutMin * 60 * 1000)) { $p.Kill(); "TIMEOUT after $TimeoutMin min"; $failed = $true }
        else { "$name exit {0}, {1:N1} s" -f $p.ExitCode, ((Get-Date) - $t0).TotalSeconds; if ($p.ExitCode -ne 0) { $failed = $true } }
        Get-Content -LiteralPath $log -Encoding UTF8 -ErrorAction SilentlyContinue | Select-Object -Last 12
        Get-Content -LiteralPath "$log.err" -Encoding UTF8 -ErrorAction SilentlyContinue | Select-Object -Last 12
        if ($failed) { break }
    }
}
finally {
    if ($writes) { & pwsh -NoProfile -File $lock release ext -Name native }
}
if ($failed) { exit 1 }
