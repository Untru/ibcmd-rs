# A stand-alone server (ibsrv) for one lab database, under the Windows account of the caller (integrated SQL
# login): the way to open a 1C session on a lab database when the 1C server cluster (LocalSystem) has no SQL login.
#   pwsh -NoProfile -File srv.ps1 start -DbName <db> [-Plat 8327|85] [-Port 5514]
#   pwsh -NoProfile -File srv.ps1 stop  [-Port 5514]
#   pwsh -NoProfile -File srv.ps1 status [-Port 5514]
# Ports: HTTP <Port>, direct registration <Port>+1000, direct range <Port>+1100..+1130.
# The data directory <lab>\ibdata\srv_<Port> is preallocated by the platform (~0.8 GB); `stop` deletes it.
# Only databases named ibcmd_rs_04_ddl_* are accepted.
param(
    [Parameter(Mandatory = $true, Position = 0)][ValidateSet('start', 'stop', 'status')][string]$Action,
    [string]$DbName = '',
    [ValidateSet('8327', '85')][string]$Plat = '8327',
    [int]$Port = 5514
)
$ErrorActionPreference = 'Stop'
$root = if ($env:DDL_LAB) { $env:DDL_LAB } else { 'F:\ibcmd\lab\04\restructure' }
$pidFile = "$root\ibdata\srv_$Port.pid"
$dataDir = "$root\ibdata\srv_$Port"
switch ($Action) {
    'start' {
        if (-not $DbName) { throw 'start needs -DbName' }
        if ($DbName -notmatch '^ibcmd_rs_04_ddl_[a-z0-9_]+$') { throw "refusing: $DbName is not a ddl-track database" }
        $ibsrv = if ($Plat -eq '85') { 'C:\Program Files\1cv8\8.5.1.1150\bin\ibsrv.exe' } else { 'C:\Program Files\1cv8\8.3.27.2214\bin\ibsrv.exe' }
        New-Item -ItemType Directory -Force $dataDir, "$root\logs" | Out-Null
        $regport = $Port + 1000; $lo = $Port + 1100; $hi = $Port + 1130
        $srvArgs = @("--data=$dataDir", '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$DbName", "--name=$DbName",
                     "--http-port=$Port", "--direct-regport=$regport", "--direct-range=${lo}:${hi}", '--disable-ssh-gate', '--schedule-jobs=deny')
        $out = "$root\logs\ibsrv_$Port.out"; $err = "$root\logs\ibsrv_$Port.err"
        $p = Start-Process -FilePath $ibsrv -ArgumentList $srvArgs -PassThru -RedirectStandardOutput $out -RedirectStandardError $err -WindowStyle Hidden
        Set-Content $pidFile $p.Id
        $deadline = (Get-Date).AddSeconds(120)
        while ((Get-Date) -lt $deadline) {
            if ($p.HasExited) { throw "ibsrv exited: $(Get-Content $err -Raw)" }
            if ((Test-Path $out) -and ((Get-Content $out -Raw) -match 'ready')) { break }
            Start-Sleep -Milliseconds 500
        }
        "started ibsrv pid $($p.Id) on $DbName http://localhost:$Port/ direct localhost:$regport"
    }
    'stop' {
        if (Test-Path $pidFile) {
            $id = [int](Get-Content $pidFile)
            Stop-Process -Id $id -Force -ErrorAction SilentlyContinue
            Start-Sleep -Seconds 2
            Remove-Item $pidFile -Force -ErrorAction SilentlyContinue
        }
        if (Test-Path $dataDir) { Remove-Item -Recurse -Force $dataDir -ErrorAction SilentlyContinue }
        'stopped'
    }
    'status' {
        if (Test-Path $pidFile) {
            $id = [int](Get-Content $pidFile)
            if (Get-Process -Id $id -ErrorAction SilentlyContinue) { "running pid $id" } else { 'not running (stale pid file)' }
        } else { 'no pid file' }
    }
}
