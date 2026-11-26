# Runs a piece of 1C code in a session on a lab database served by ibsrv (srv.ps1) and prints its result.
#   pwsh -NoProfile -File session_job.ps1 -Job <file.bsl> [-Port 5514 | -Database <db of the cluster>] [-User Администратор] [-TimeoutSec 240]
# The job is BSL text run on the server with Выполнить(); it sets the variable Результат (a string). The
# client is the 8.3.27 thin client (1cv8c) connected over HTTP (/WS) to the stand-alone server on -Port,
# running the generic processing probe\ddl_probe.epf (built with the Designer from probe\src: its form
# reads <startup parameter>.bsl, executes it on the server and writes <startup parameter>.out).
# Only servers of ddl-track databases: srv.ps1 refuses any other database.
param(
    [Parameter(Mandatory = $true)][string]$Job,
    [int]$Port = 5514,
    [string]$Database = '',
    [string]$User = 'Администратор',
    [int]$TimeoutSec = 240,
    [string]$Epf = ''
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$lab = if ($env:DDL_LAB) { $env:DDL_LAB } else { 'F:\ibcmd\lab\04\restructure' }
if (-not $Epf) { $Epf = "$lab\probe\ddl_probe.epf" }
$bin = 'C:\Program Files\1cv8\8.3.27.2214\bin\1cv8c.exe'
$work = "$lab\probe\run"
New-Item -ItemType Directory -Force $work | Out-Null
$base = "$work\job_{0}" -f [guid]::NewGuid().ToString('N').Substring(0, 8)
Copy-Item -LiteralPath $Job -Destination "$base.bsl"
# An empty password is simply not passed (an empty argument would swallow the next switch).
# With -Database the session is opened in the 1C server cluster of 8.3.27 (the database registered with
# register-ib.ps1 of the lab tools); otherwise on the stand-alone server of -Port.
$connect = if ($Database) { @('/IBConnectionString', "Srvr=`"localhost:2541`";Ref=`"$Database`";") } else { @('/WS', "http://localhost:$Port/") }
$clientArgs = @('ENTERPRISE') + $connect + @("/N$User", '/DisableStartupMessages', '/DisableStartupDialogs',
                '/Execute', "`"$Epf`"", "/C`"$base`"", '/Out', "`"$base.log`"")
$p = Start-Process -FilePath $bin -ArgumentList $clientArgs -PassThru -WindowStyle Hidden
$deadline = (Get-Date).AddSeconds($TimeoutSec)
while (-not (Test-Path "$base.out")) {
    if ($p.HasExited -and -not (Test-Path "$base.out")) { break }
    if ((Get-Date) -gt $deadline) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue; throw "session job timeout ($TimeoutSec s); log: $base.log" }
    Start-Sleep -Milliseconds 500
}
if (Test-Path "$base.out") {
    Start-Sleep -Milliseconds 300
    Get-Content -LiteralPath "$base.out" -Encoding UTF8
    if (-not $p.HasExited) { $p.WaitForExit(15000) | Out-Null }
    if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }
} else {
    'client exited without a result; log:'
    Get-Content -LiteralPath "$base.log" -ErrorAction SilentlyContinue
    exit 1
}
