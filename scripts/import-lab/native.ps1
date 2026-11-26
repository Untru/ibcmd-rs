# Native ibcmd helpers of the import track (issue #388). Dot-source: . scripts\import-lab\native.ps1
# Lab only: databases ibcmd_rs_04_import_*; every native write (config import / config apply) runs
# inside the lab's native lock (heavy-lock.ps1 acquire import -Name native ... release), one at a time.
# Native ibcmd needs --user=Администратор on the BSP databases (without it, with stdin closed, it loops
# printing the user prompt).

$script:Lab = if ($env:IMPORT_LAB) { $env:IMPORT_LAB } else { 'F:\ibcmd\lab\04\import' }
$script:Ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
$script:NativeUser = 'Администратор'
$script:Lock = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'

# The platform the native ibcmd of the next calls belongs to: '8.3.27' (the default) or '8.5'. The 8.5 БСП clones keep
# the administrator as "Администратор (обычное приложение)" with an empty password.
function Set-NativePlatform([string]$Platform, [switch]$NoUser) {
    if ($Platform -eq '8.5') {
        $script:Ibcmd = 'C:\Program Files\1cv8\8.5.1.1150\bin\ibcmd.exe'
        $script:NativeUser = '"Администратор (обычное приложение)"'   # quoted: it holds spaces and Start-Process does not quote
    } else {
        $script:Ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
        $script:NativeUser = 'Администратор'
    }
    # The ERP УХ clones have no users: no --user at all.
    if ($NoUser) { $script:NativeUser = '' }
}

# The command line without an empty --user=.
function Drop-EmptyUser([string[]]$Arguments) { @($Arguments | Where-Object { $_ -ne '--user=' }) }

# ERP УХ commands run under the heavy lock (README of the lab): every command of a УХ case holds it for that one command,
# a native write holds it and then the native lock. Off by default (the БСП cases hold the native lock only).
$script:UseHeavy = $false
$script:HeavyHeld = $false
function Use-HeavyLab([bool]$On = $true) { $script:UseHeavy = $On }
function Invoke-WithHeavyLock([scriptblock]$Body, [int]$TimeoutMin = 240) {
    if (-not $script:UseHeavy -or $script:HeavyHeld) { & $Body; return }
    $out = & pwsh -NoProfile -File $script:Lock acquire import -TimeoutMin $TimeoutMin
    if ($LASTEXITCODE -ne 0) { throw "heavy lock: $out" }
    $script:HeavyHeld = $true
    try { & $Body } finally {
        $script:HeavyHeld = $false
        & pwsh -NoProfile -File $script:Lock release import | Out-Null
    }
}

function Assert-LabDb([string]$Db) {
    if ($Db -notmatch '^ibcmd_rs_04_import_[a-z0-9_]+$') { throw "lab databases only: ibcmd_rs_04_import_* (got $Db)" }
}

$script:NativeLockHeld = $false

# One native write at a time across the lab. Re-entrant: a caller that needs a native import and the apply
# that follows it on the same clone wraps both in one call and holds the lock once.
function Invoke-WithNativeLock([scriptblock]$Body, [int]$TimeoutMin = 180) {
    if ($script:NativeLockHeld) { & $Body; return }
    # PowerShell resolves variables dynamically. The heavy wrapper also has a $Body parameter:
    # capture under another name or its inner invocation calls the lock wrapper recursively.
    $nativeBody = $Body
    Invoke-WithHeavyLock {
        $out = & pwsh -NoProfile -File $script:Lock acquire import -Name native -TimeoutMin $TimeoutMin
        if ($LASTEXITCODE -ne 0) { throw "native lock: $out" }
        $script:NativeLockHeld = $true
        try { & $nativeBody } finally {
            $script:NativeLockHeld = $false
            & pwsh -NoProfile -File $script:Lock release import -Name native | Out-Null
        }
    }
}

function Get-ConfigSaveRows([string]$Db) {
    [int](sqlcmd -S localhost -E -C -h -1 -W -d $Db -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM ConfigSave")
}

# native `config import` of a tree; returns @{Exit; Seconds; Rows; Tail}
function Invoke-NativeImport([string]$Db, [string]$Tree, [string]$Tag = 'x', [int]$TimeoutSec = 1800) {
    Assert-LabDb $Db
    $data = "$($script:Lab)\ibdata\$Db"
    New-Item -ItemType Directory -Force $data, "$($script:Lab)\logs" | Out-Null
    if (-not (Test-Path "$($script:Lab)\logs\empty.txt")) { New-Item -ItemType File "$($script:Lab)\logs\empty.txt" | Out-Null }
    $res = @{}
    Invoke-WithNativeLock {
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $args = @('infobase', 'config', 'import', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Db",
                  "--data=$data", "--user=$($script:NativeUser)", $Tree)
        $p = Start-Process -FilePath $script:Ibcmd -ArgumentList (Drop-EmptyUser $args) -NoNewWindow -PassThru `
            -RedirectStandardOutput "$($script:Lab)\logs\native-import-$Tag.out.txt" `
            -RedirectStandardError "$($script:Lab)\logs\native-import-$Tag.err.txt" `
            -RedirectStandardInput "$($script:Lab)\logs\empty.txt"
        if (-not $p.WaitForExit($TimeoutSec * 1000)) { $p.Kill(); $res.Exit = -999 } else { $res.Exit = $p.ExitCode }
        $res.Seconds = [math]::Round($sw.Elapsed.TotalSeconds, 1)
    }
    $res.Rows = Get-ConfigSaveRows $Db
    $res.Tail = ((Get-Content "$($script:Lab)\logs\native-import-$Tag.out.txt", "$($script:Lab)\logs\native-import-$Tag.err.txt" -ErrorAction SilentlyContinue) | Select-Object -Last 3) -join ' | '
    $res
}

# native `config import files --partial`: the listed files (relative to BaseDir) only; returns @{Exit; Seconds; Rows; Tail}
function Invoke-NativeImportFiles([string]$Db, [string]$BaseDir, [string[]]$Files, [string]$Tag = 'x', [int]$TimeoutSec = 1800) {
    Assert-LabDb $Db
    $data = "$($script:Lab)\ibdata\$Db"
    New-Item -ItemType Directory -Force $data, "$($script:Lab)\logs" | Out-Null
    if (-not (Test-Path "$($script:Lab)\logs\empty.txt")) { New-Item -ItemType File "$($script:Lab)\logs\empty.txt" | Out-Null }
    $res = @{}
    Invoke-WithNativeLock {
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $args = @('infobase', 'config', 'import', 'files', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Db",
                  "--data=$data", "--user=$($script:NativeUser)", "--base-dir=$BaseDir", '--partial') + $Files
        $p = Start-Process -FilePath $script:Ibcmd -ArgumentList (Drop-EmptyUser $args) -NoNewWindow -PassThru `
            -RedirectStandardOutput "$($script:Lab)\logs\native-importfiles-$Tag.out.txt" `
            -RedirectStandardError "$($script:Lab)\logs\native-importfiles-$Tag.err.txt" `
            -RedirectStandardInput "$($script:Lab)\logs\empty.txt"
        if (-not $p.WaitForExit($TimeoutSec * 1000)) { $p.Kill(); $res.Exit = -999 } else { $res.Exit = $p.ExitCode }
        $res.Seconds = [math]::Round($sw.Elapsed.TotalSeconds, 1)
    }
    $res.Rows = Get-ConfigSaveRows $Db
    $res.Tail = ((Get-Content "$($script:Lab)\logs\native-importfiles-$Tag.out.txt", "$($script:Lab)\logs\native-importfiles-$Tag.err.txt" -ErrorAction SilentlyContinue) | Select-Object -Last 3) -join ' | '
    $res
}

# native `config apply --force --dynamic=<mode>`
function Invoke-NativeApply([string]$Db, [string]$Dynamic = 'disable', [string]$Tag = 'x', [int]$TimeoutSec = 3600) {
    Assert-LabDb $Db
    $data = "$($script:Lab)\ibdata\$Db"
    New-Item -ItemType Directory -Force $data, "$($script:Lab)\logs" | Out-Null
    if (-not (Test-Path "$($script:Lab)\logs\empty.txt")) { New-Item -ItemType File "$($script:Lab)\logs\empty.txt" | Out-Null }
    $res = @{}
    Invoke-WithNativeLock {
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $args = @('infobase', 'config', 'apply', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Db",
                  "--data=$data", "--user=$($script:NativeUser)", '--force', "--dynamic=$Dynamic")
        $p = Start-Process -FilePath $script:Ibcmd -ArgumentList (Drop-EmptyUser $args) -NoNewWindow -PassThru `
            -RedirectStandardOutput "$($script:Lab)\logs\native-apply-$Tag.out.txt" `
            -RedirectStandardError "$($script:Lab)\logs\native-apply-$Tag.err.txt" `
            -RedirectStandardInput "$($script:Lab)\logs\empty.txt"
        if (-not $p.WaitForExit($TimeoutSec * 1000)) { $p.Kill(); $res.Exit = -999 } else { $res.Exit = $p.ExitCode }
        $res.Seconds = [math]::Round($sw.Elapsed.TotalSeconds, 1)
    }
    $res.Tail = ((Get-Content "$($script:Lab)\logs\native-apply-$Tag.out.txt", "$($script:Lab)\logs\native-apply-$Tag.err.txt" -ErrorAction SilentlyContinue) | Select-Object -Last 4) -join ' | '
    $res
}

# native `config export` (a read: no lock)
function Invoke-NativeExport([string]$Db, [string]$OutDir, [string]$Tag = 'x', [int]$TimeoutSec = 1800) {
    Assert-LabDb $Db
    $data = "$($script:Lab)\ibdata\$Db"
    New-Item -ItemType Directory -Force $data, "$($script:Lab)\logs" | Out-Null
    if (-not (Test-Path "$($script:Lab)\logs\empty.txt")) { New-Item -ItemType File "$($script:Lab)\logs\empty.txt" | Out-Null }
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $args = @('infobase', 'config', 'export', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Db",
              "--data=$data", "--user=$($script:NativeUser)", $OutDir)
    $p = Start-Process -FilePath $script:Ibcmd -ArgumentList (Drop-EmptyUser $args) -NoNewWindow -PassThru `
        -RedirectStandardOutput "$($script:Lab)\logs\native-export-$Tag.out.txt" `
        -RedirectStandardError "$($script:Lab)\logs\native-export-$Tag.err.txt" `
        -RedirectStandardInput "$($script:Lab)\logs\empty.txt"
    $res = @{}
    if (-not $p.WaitForExit($TimeoutSec * 1000)) { $p.Kill(); $res.Exit = -999 } else { $res.Exit = $p.ExitCode }
    $res.Seconds = [math]::Round($sw.Elapsed.TotalSeconds, 1)
    $res
}

# ours: the drop-in `infobase config apply` (the own apply: it stops with "требуется штатный config apply: <reasons>"
# and exit 1 when the stage needs the platform's); returns @{Exit; Seconds; Tail}
function Invoke-OursApply([string]$Db, [string]$Tag = 'x', [string]$Exe = '', [string]$Platform = '8.3.27', [string[]]$Extra = @()) {
    Assert-LabDb $Db
    if (-not $Exe) { $Exe = "$($script:Lab)\bin\ibcmd-rs-v0.exe" }
    $data = "$($script:Lab)\ibdata\$Db"
    New-Item -ItemType Directory -Force $data, "$($script:Lab)\out", "$($script:Lab)\logs" | Out-Null
    $version = if ($Platform -eq '8.5') { '8.5.1' } else { $Platform }
    $args = @('infobase', 'config', 'apply', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Db",
              "--data=$data", '--force', '--dynamic=disable', '--exclusivity=assumed', "--platform=$version",
              "--report=$($script:Lab)\out\apply-$Tag.json") + $Extra
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $o = & $Exe @args 2>&1
    $rc = $LASTEXITCODE
    $o | ForEach-Object { "$_" } | Set-Content "$($script:Lab)\logs\ours-apply-$Tag.log" -Encoding UTF8
    @{ Exit = $rc; Seconds = [math]::Round($sw.Elapsed.TotalSeconds, 1); Tail = (($o | Select-Object -Last 4) -join ' | ') }
}

# ours: the drop-in `infobase config import` (Auto = patch on a database that holds the configuration)
function Invoke-OursImport([string]$Db, [string]$Tree, [string]$Tag = 'x', [switch]$BaseFree, [string]$Exe = '') {
    Assert-LabDb $Db
    if (-not $Exe) { $Exe = "$($script:Lab)\bin\ibcmd-rs-v0.exe" }
    $data = "$($script:Lab)\ibdata\$Db"
    New-Item -ItemType Directory -Force $data, "$($script:Lab)\out" | Out-Null
    $args = @('infobase', 'config', 'import', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Db",
              "--data=$data", "--report=$($script:Lab)\out\import-$Tag.json")
    if ($BaseFree) { $args += '--base-free' }
    $args += $Tree
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $o = & $Exe @args 2>&1
    $rc = $LASTEXITCODE
    New-Item -ItemType Directory -Force "$($script:Lab)\logs" | Out-Null
    $o | ForEach-Object { "$_" } | Set-Content "$($script:Lab)\logs\ours-import-$Tag.log" -Encoding UTF8
    $res = @{ Exit = $rc; Seconds = [math]::Round($sw.Elapsed.TotalSeconds, 1); Tail = (($o | Select-Object -Last 4) -join ' | ') }
    $rep = "$($script:Lab)\out\import-$Tag.json"
    if (Test-Path $rep) {
        $j = Get-Content $rep -Raw | ConvertFrom-Json
        $res.Mode = $j.stage_mode; $res.RowsAfter = $j.staged_rows_after
    }
    $res.Rows = Get-ConfigSaveRows $Db
    $res
}
