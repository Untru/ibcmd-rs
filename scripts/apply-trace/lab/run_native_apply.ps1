# One native `infobase config apply` under the capture kit.
#   pwsh -NoProfile -File run_native_apply.ps1 -Database <db> -Tag <tag> [-Dynamic disable|force|auto] [-Platform 8.3.27.2214] [-Note ..]
# Locks: `heavy` for the whole capture (apply is multi-connection and takes minutes), and, since 2026-09-29 11:30, the
# `native` lock for the native command only (taken right before it, released right after it).
# Output: F:\ibcmd\lab\04\trace\captures\<yyyyMMdd-HHmmss>-<Tag>\
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Tag,
    [ValidateSet('disable', 'force', 'auto')][string]$Dynamic = 'disable',
    [string]$Platform = '8.3.27.2214',
    [string]$User = 'Администратор',
    [string]$Note = '',
    [int]$TimeoutMinutes = 45,
    [switch]$NoHeavyLock,
    [switch]$NoUser,
    [string]$BeforeSnapshot = ''
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$kit = 'F:\ibcmd\src\ibcmd-rs-m-simple\scripts\apply-trace'
$lab = 'F:\ibcmd\lab\04\trace'
$lockScript = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$ibcmdExe = "C:\Program Files\1cv8\$Platform\bin\ibcmd.exe"
$dataDir = "$lab\ibdata\$Database"
New-Item -ItemType Directory -Force -Path $dataDir | Out-Null
$ibArgs = @('infobase', 'config', 'apply', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Database", "--data=$dataDir")
if (-not $NoUser) { $ibArgs += "--user=$User" }
$ibArgs += @('--force', "--dynamic=$Dynamic")
$held = $false
if (-not $NoHeavyLock) {
    & pwsh -NoProfile -File $lockScript acquire trace -TimeoutMin 90
    if ($LASTEXITCODE -ne 0) { throw 'heavy lock not acquired' }
    $held = $true
}
try {
    $cap = @{ Database = $Database; Tag = $Tag; OutRoot = "$lab\captures"; BlobStore = "$lab\blobs"; Track = 'trace'; TimeoutMinutes = $TimeoutMinutes;
        Note = $Note; Exe = $ibcmdExe; ArgumentList = $ibArgs
        BeforeCommand = { & pwsh -NoProfile -File $lockScript acquire trace -Name native -TimeoutMin 120; if ($LASTEXITCODE -ne 0) { throw 'native lock not acquired' } }
        AfterCommand  = { & pwsh -NoProfile -File $lockScript release trace -Name native } }
    if ($BeforeSnapshot) { $cap['BeforeSnapshot'] = $BeforeSnapshot }
    & (Join-Path $kit 'capture.ps1') @cap
} finally {
    if ($held) { & pwsh -NoProfile -File $lockScript release trace }
}
