# One native `infobase config import files --partial` (native staging of selected files into ConfigSave)
# under the capture kit. Locks: `heavy` for the capture, the `native` lock (since 2026-09-29 11:30) for the native
# command only. Afterwards the number of ConfigSave rows is printed and recorded: a native import once staged a
# partial ConfigSave under CPU contention, so check it before applying.
#   pwsh -NoProfile -File run_native_import_files.ps1 -Database <db> -Tag <tag> -BaseDir <tree> -Files 'Catalogs/X.xml|Configuration.xml' [-Note ..]
# Output: F:\ibcmd\lab\04\trace\captures\<yyyyMMdd-HHmmss>-<Tag>\
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Tag,
    [Parameter(Mandatory = $true)][string]$BaseDir,
    [Parameter(Mandatory = $true)][string]$Files,      # relative to BaseDir, separated by |
    [string]$Platform = '8.3.27.2214',
    [string]$User = 'Администратор',
    [string]$Note = '',
    [int]$TimeoutMinutes = 60,
    [switch]$NoHeavyLock
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$kit = 'F:\ibcmd\src\ibcmd-rs-m-simple\scripts\apply-trace'
$lab = 'F:\ibcmd\lab\04\trace'
$lockScript = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
$ibcmdExe = "C:\Program Files\1cv8\$Platform\bin\ibcmd.exe"
$dataDir = "$lab\ibdata\$Database"
New-Item -ItemType Directory -Force -Path $dataDir | Out-Null
$ibArgs = @('infobase', 'config', 'import', 'files', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Database", "--data=$dataDir",
    "--user=$User", "--base-dir=$BaseDir", '--partial') + @($Files -split '\|')
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
    & (Join-Path $kit 'capture.ps1') @cap
    $n = & sqlcmd -S localhost -E -C -d $Database -h -1 -W -Q 'SET NOCOUNT ON; SELECT COUNT(*) FROM ConfigSave'
    "ConfigSave rows after the native import: $n"
    Add-Content -LiteralPath "$lab\captures\configsave-rows-after-native-import.tsv" -Value ("{0}`t{1}`t{2}`t{3}" -f (Get-Date -Format s), $Database, $Tag, $n)
} finally {
    if ($held) { & pwsh -NoProfile -File $lockScript release trace }
}
