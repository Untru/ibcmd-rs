# One native `ibcmd infobase create` under the capture kit (XE trace + snapshots + diff).
#   pwsh -NoProfile -File run_native_create.ps1 -Database <empty db> -Tag <tag> [-Platform 8.5.1.1150] [-Locale ru_RU] [-Note ..]
# Locks: the `native` lock only around the native command (since 2026-09-29 11:30; FIFO tickets since 14:15).
# The database must exist and be empty (F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus empty).
# Output: F:\ibcmd\lab\04\trace\captures\<yyyyMMdd-HHmmss>-<Tag>\
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Tag,
    [string]$Platform = '8.5.1.1150',
    [string]$Locale = 'ru_RU',
    [string]$Note = '',
    [int]$TimeoutMinutes = 20
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$kit = 'F:\ibcmd\src\ibcmd-rs-m-simple\scripts\apply-trace'
$lab = 'F:\ibcmd\lab\04\trace'
$lockScript = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$ibcmdExe = "C:\Program Files\1cv8\$Platform\bin\ibcmd.exe"
$dataDir = "$lab\ibdata\$Database"
New-Item -ItemType Directory -Force -Path $dataDir | Out-Null
$ibArgs = @('infobase', 'create', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Database", "--data=$dataDir", "--locale=$Locale")
$cap = @{ Database = $Database; Tag = $Tag; OutRoot = "$lab\captures"; BlobStore = "$lab\blobs"; Track = 'trace'; TimeoutMinutes = $TimeoutMinutes;
    Note = $Note; Exe = $ibcmdExe; ArgumentList = $ibArgs
    BeforeCommand = { & pwsh -NoProfile -File $lockScript acquire trace -Name native -TimeoutMin 120; if ($LASTEXITCODE -ne 0) { throw 'native lock not acquired' } }
    AfterCommand  = { & pwsh -NoProfile -File $lockScript release trace -Name native } }
& (Join-Path $kit 'capture.ps1') @cap
