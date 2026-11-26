# Lab rule since 2026-09-29 11:30 (lab README, "Heavy operations and native writes"): one NATIVE ibcmd write
# (config import / config apply / infobase create) at a time across all tracks. Hold the lock for the native
# command only, never for the analysis around it.
#
#   . "$PSScriptRoot\native_lock.ps1"
#   $rc = Invoke-NativeCommand -Ibcmd $ibcmd -Arguments $args -Log F:\...\logs\import-x    # -> exit code
#   $rc = Invoke-NativeLocked { ...anything else that must not run beside another native write... }
#
# The lock script and the track name are configurable: $env:HEAVY_LOCK, $env:DDL_LOCK_TRACK.
$script:HeavyLockScript = if ($env:HEAVY_LOCK) { $env:HEAVY_LOCK } else { 'F:\ibcmd\lab\04\tools\heavy-lock.ps1' }
$script:LockTrack = if ($env:DDL_LOCK_TRACK) { $env:DDL_LOCK_TRACK } else { 'ddl' }

# Runs the script block under the "native" lock (acquire ... release in try/finally) and returns its output.
# Progress goes through Write-Host so it never mixes with the block's return value.
function Invoke-NativeLocked([scriptblock]$Body, [int]$TimeoutMin = 180) {
    # heavy-lock.ps1 queues a ticket and waits for its turn (FIFO since 14:15); hold the lock for one command only.
    $acq = & pwsh -NoProfile -File $script:HeavyLockScript acquire $script:LockTrack -Name native -TimeoutMin $TimeoutMin
    if ($LASTEXITCODE -ne 0) { throw "native lock not acquired: $($acq -join ' ')" }
    Write-Host "[native lock] $($acq -join ' ')"
    try {
        & $Body
    } finally {
        $rel = & pwsh -NoProfile -File $script:HeavyLockScript release $script:LockTrack -Name native
        Write-Host "[native lock] $($rel -join ' ')"
    }
}

# Runs one native ibcmd command under the lock with stdin closed, stdout/stderr in <Log>.out / <Log>.err, a
# timeout and a cap on the output (a native ibcmd that asks for a password on a closed stdin loops printing the
# prompt; one that asks on an open stdin waits for ever holding the lock -- both are killed). Returns the exit code.
function Invoke-NativeCommand(
    [string]$Ibcmd, [string[]]$Arguments, [string]$Log,
    [int]$TimeoutSec = 3600, [long]$MaxLogBytes = 16MB, [int]$LockTimeoutMin = 180, [switch]$NoLock) {
    $dir = Split-Path -Parent $Log
    New-Item -ItemType Directory -Force $dir | Out-Null
    $empty = Join-Path $dir 'empty.txt'
    if (-not (Test-Path -LiteralPath $empty)) { New-Item -ItemType File $empty | Out-Null }
    $line = ($Arguments | ForEach-Object { if ($_ -match '[\s"]') { '"' + ($_ -replace '"', '\"') + '"' } else { $_ } }) -join ' '
    $run = {
        $p = Start-Process -FilePath $Ibcmd -ArgumentList $line -NoNewWindow -PassThru `
            -RedirectStandardOutput "$Log.out" -RedirectStandardError "$Log.err" -RedirectStandardInput $empty
        $deadline = (Get-Date).AddSeconds($TimeoutSec)
        $seen = @{}
        while (-not $p.HasExited) {
            Get-Process ibcmd -ErrorAction SilentlyContinue | Where-Object { $_.Id -ne $p.Id } | ForEach-Object { $seen[$_.Id] = $_.StartTime.ToString('s') }
            if ((Get-Date) -gt $deadline) { $p.Kill(); throw "native command timeout ($TimeoutSec s): $line" }
            if ((Test-Path "$Log.out") -and (Get-Item "$Log.out").Length -gt $MaxLogBytes) { $p.Kill(); throw "native command output above $MaxLogBytes bytes (a password prompt loop?): $line" }
            Start-Sleep -Milliseconds 1000
        }
        $p.WaitForExit()
        $note = ($seen.GetEnumerator() | ForEach-Object { "pid=$($_.Key) started=$($_.Value)" }) -join '; '
        Set-Content -LiteralPath "$Log.concurrent.txt" -Value ("exit=$($p.ExitCode) others=[$note]")
        $p.ExitCode
    }
    # A native export writes nothing to the database and needs no lock; import, apply and create do.
    if ($NoLock) { & $run } else { Invoke-NativeLocked $run -TimeoutMin $LockTimeoutMin }
}

# The БСП user of a corpus: the 8.3.27 database has an empty-password "Администратор", the 8.5 one only
# "Администратор (обычное приложение)" (plain "Администратор" there asks for a password).
function Get-LabUser([string]$Platform) {
    if ($Platform -eq '85') { 'Администратор (обычное приложение)' } else { 'Администратор' }
}
