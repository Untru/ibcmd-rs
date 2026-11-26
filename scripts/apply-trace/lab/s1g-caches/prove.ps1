# The platform proof of a cache row in a trace-track lab database (S1-G, docs/apply/derived-caches.md 4):
#   1. a stand-alone server (ibsrv) starts on the database and a thin-client session runs the probe job;
#   2. the same job in a session of the 8.3.27 cluster (register-ib.ps1);
#   3. native `infobase config check`;
#   4. native `infobase config apply`, which must say the database configuration needs no update.
# The native commands run under the lab "native" lock (one command per hold).
#   pwsh -NoProfile -File prove.ps1 -Database ibcmd_rs_05_trace_x -Label x [-Job <file.bsl>] [-Steps standalone,cluster,check,apply]
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Label,
    [string]$Job = 'F:\ibcmd\lab\04\restructure\probe\jobs\newcat.bsl',
    [string[]]$Steps = @('standalone', 'cluster', 'check', 'apply'),
    [int]$Port = 5614
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$Steps = @($Steps | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
if ($Database -notmatch '^ibcmd_rs_05_trace_[a-z0-9_]+$') { throw "trace-track lab databases only (got $Database)" }
$k = 'F:\ibcmd\lab\05\s1g\kit'
$env:DDL_LOCK_TRACK = 'trace'
. 'F:\ibcmd\lab\04\restructure\tools\native_lock.ps1'
$ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
$out = "$k\out"
New-Item -ItemType Directory -Force $out, "$k\logs", "$k\ibdata\$Database" | Out-Null
$report = "$out\prove_$Label.txt"
Set-Content -LiteralPath $report -Encoding utf8 -Value ("prove {0} on {1} at {2}" -f $Label, $Database, (Get-Date -Format s))
function Note([string]$text) { Add-Content -LiteralPath $report -Encoding utf8 -Value $text; Write-Host $text }
$nativeBase = @('--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Database", "--data=$k\ibdata\$Database", '--user=Администратор')

if ($Steps -contains 'standalone') {
    Note '== 1 stand-alone server + thin client'
    try {
        # started apart (a hidden ibsrv inherits the pipe of a direct call and the call never returns), then polled
        $srvOut = "$k\logs\ibsrv_$Port.out"
        if (Test-Path $srvOut) { [IO.File]::Delete($srvOut) }
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $starter = Start-Process pwsh -ArgumentList @('-NoProfile', '-File', "$k\srv.ps1", 'start', '-DbName', $Database, '-Plat', '8327', '-Port', "$Port") -PassThru -WindowStyle Hidden -RedirectStandardOutput "$k\logs\srv_start_$Port.log" -RedirectStandardError "$k\logs\srv_start_$Port.err"
        $deadline = (Get-Date).AddSeconds(240)
        while ((Get-Date) -lt $deadline) {
            if ((Test-Path $srvOut) -and ((Get-Content $srvOut -Raw -ErrorAction SilentlyContinue) -match 'ready')) { break }
            if ($starter.HasExited -and $starter.ExitCode -ne 0) { throw "server start failed: $(Get-Content "$k\logs\srv_start_$Port.err" -Raw)" }
            Start-Sleep -Milliseconds 500
        }
        if (-not ((Test-Path $srvOut) -and ((Get-Content $srvOut -Raw) -match 'ready'))) { throw 'the server did not become ready in 240 s' }
        Note ("  server ready in {0:n1} s" -f $sw.Elapsed.TotalSeconds)
        $sw.Restart()
        $result = & pwsh -NoProfile -File "$k\job.ps1" -Job $Job -Port $Port -TimeoutSec 400 2>&1
        Note ("  job took {0:n1} s" -f $sw.Elapsed.TotalSeconds)
        $result | ForEach-Object { Note "  | $_" }
    } finally {
        & pwsh -NoProfile -File "$k\srv.ps1" stop -Port $Port | Out-Null
        Note '  server stopped'
    }
}
if ($Steps -contains 'cluster') {
    Note '== 2 cluster session'
    try {
        (& pwsh -NoProfile -File F:\ibcmd\lab\04\tools\register-ib.ps1 register -Database $Database -Platform 8.3.27 -Track trace) | ForEach-Object { Note "  reg: $_" }
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $result = & pwsh -NoProfile -File "$k\job.ps1" -Job $Job -Database $Database -TimeoutSec 400 2>&1
        Note ("  job took {0:n1} s" -f $sw.Elapsed.TotalSeconds)
        $result | ForEach-Object { Note "  | $_" }
    } finally {
        (& pwsh -NoProfile -File F:\ibcmd\lab\04\tools\register-ib.ps1 unregister -Database $Database -Platform 8.3.27 -Track trace) | ForEach-Object { Note "  reg: $_" }
    }
}
if ($Steps -contains 'check') {
    Note '== 3 native config check'
    $rc = Invoke-NativeCommand -Ibcmd $ibcmd -Arguments (@('infobase', 'config', 'check') + $nativeBase + @('--force')) -Log "$k\logs\check-$Label" -TimeoutSec 3000
    Note "  exit=$rc"
    Get-Content "$k\logs\check-$Label.out" -Tail 12 -ErrorAction SilentlyContinue | ForEach-Object { Note "  | $_" }
    Get-Content "$k\logs\check-$Label.err" -Tail 5 -ErrorAction SilentlyContinue | ForEach-Object { Note "  ! $_" }
}
if ($Steps -contains 'apply') {
    Note '== 4 native config apply'
    $rc = Invoke-NativeCommand -Ibcmd $ibcmd -Arguments (@('infobase', 'config', 'apply') + $nativeBase + @('--force', '--dynamic=disable')) -Log "$k\logs\apply-$Label" -TimeoutSec 3000
    Note "  exit=$rc"
    Get-Content "$k\logs\apply-$Label.out" -Tail 6 -ErrorAction SilentlyContinue | ForEach-Object { Note "  | $_" }
    Get-Content "$k\logs\apply-$Label.err" -Tail 5 -ErrorAction SilentlyContinue | ForEach-Object { Note "  ! $_" }
}
Note 'done'
