# Extended Events helpers for the ddl track. Dot-source: . scripts\restructure-lab\xe.ps1
# Sessions are named ibcmd_rs_04_ddl_<n> and filtered to one database; files under
# C:\temp\ibcmd_rs_04\ddl\. Only sessions with that prefix are ever created/stopped/dropped.
# The lab folder is $env:DDL_LAB (default F:\ibcmd\lab\04\restructure).

. "$PSScriptRoot\native_lock.ps1"
$script:LabRoot = if ($env:DDL_LAB) { $env:DDL_LAB } else { 'F:\ibcmd\lab\04\restructure' }
$script:XeDir = 'C:\temp\ibcmd_rs_04\ddl'

function Invoke-Sql([string]$Sql, [string]$Db = 'master') {
    $f = Join-Path $env:TEMP ("ddlsql-{0}.sql" -f [guid]::NewGuid().ToString('N'))
    [System.IO.File]::WriteAllText($f, $Sql, [System.Text.UTF8Encoding]::new($true))
    try {
        & sqlcmd -S localhost -E -C -b -f 65001 -d $Db -i $f
        if ($LASTEXITCODE -ne 0) { throw "sqlcmd failed ($LASTEXITCODE)" }
    } finally { Remove-Item -LiteralPath $f -ErrorAction SilentlyContinue }
}

function Start-DdlXe([string]$Name, [string]$Db) {
    if ($Name -notmatch '^ibcmd_rs_04_ddl_[a-z0-9_]+$') { throw "session name must start with ibcmd_rs_04_ddl_ (got $Name)" }
    if ($Db -notmatch '^ibcmd_rs_04_ddl_[a-z0-9_]+$') { throw "database must start with ibcmd_rs_04_ddl_ (got $Db)" }
    New-Item -ItemType Directory -Force $script:XeDir | Out-Null
    $file = Join-Path $script:XeDir "$Name.xel"
    $act = 'ACTION (sqlserver.session_id, sqlserver.client_app_name, sqlserver.database_name, sqlserver.transaction_id, sqlserver.sql_text)'
    $flt = "WHERE ([sqlserver].[database_name] = N'$Db')"
    $sql = @"
SET NOCOUNT ON;
IF EXISTS (SELECT 1 FROM sys.server_event_sessions WHERE name = N'$Name') DROP EVENT SESSION [$Name] ON SERVER;
CREATE EVENT SESSION [$Name] ON SERVER
ADD EVENT sqlserver.sql_batch_completed ($act $flt),
ADD EVENT sqlserver.rpc_completed ($act $flt),
ADD EVENT sqlserver.sql_statement_completed ($act $flt),
ADD EVENT sqlserver.object_created ($act $flt),
ADD EVENT sqlserver.object_altered ($act $flt),
ADD EVENT sqlserver.object_deleted ($act $flt)
ADD TARGET package0.event_file (SET filename = N'$file', max_file_size = 2048, max_rollover_files = 2)
WITH (MAX_MEMORY = 128 MB, EVENT_RETENTION_MODE = NO_EVENT_LOSS, MAX_DISPATCH_LATENCY = 1 SECONDS, TRACK_CAUSALITY = ON, STARTUP_STATE = OFF);
ALTER EVENT SESSION [$Name] ON SERVER STATE = START;
"@
    Invoke-Sql $sql
    "xe started: $Name -> $file"
}

function Stop-DdlXe([string]$Name) {
    if ($Name -notmatch '^ibcmd_rs_04_ddl_[a-z0-9_]+$') { throw "not my session: $Name" }
    $sql = @"
SET NOCOUNT ON;
IF EXISTS (SELECT 1 FROM sys.dm_xe_sessions WHERE name = N'$Name') ALTER EVENT SESSION [$Name] ON SERVER STATE = STOP;
IF EXISTS (SELECT 1 FROM sys.server_event_sessions WHERE name = N'$Name') DROP EVENT SESSION [$Name] ON SERVER;
"@
    Invoke-Sql $sql
    "xe stopped and dropped: $Name"
}

function Get-DdlXeSessions {
    Invoke-Sql "SET NOCOUNT ON; SELECT name FROM sys.server_event_sessions WHERE name LIKE N'ibcmd_rs_04_ddl[_]%'; SELECT name, create_time FROM sys.dm_xe_sessions WHERE name LIKE N'ibcmd_rs_04_ddl[_]%';"
}

# native ibcmd config apply on one lab database, under the lab "native" lock (native_lock.ps1: closed stdin, timeout,
# output cap). Platform 8.3.27.2214 unless -Ibcmd/-User say otherwise. Returns the exit code; the platform's output
# is in logs\native-apply-<tag>.out / .err.
function Invoke-NativeApply([string]$Db, [string]$Dynamic = 'disable', [string]$Data = '', [int]$TimeoutSec = 3600,
                            [string]$Ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe', [string]$Tag = '',
                            [string]$User = 'Администратор') {
    if (-not $Data) { $Data = "$($script:LabRoot)\ibdata\$Db" }
    if (-not $Tag) { $Tag = $Db }
    New-Item -ItemType Directory -Force $Data | Out-Null
    $nativeArgs = @('infobase', 'config', 'apply', '--dbms=MSSQLServer', '--db-server=localhost', "--db-name=$Db",
                    "--data=$Data", "--user=$User", '--force', "--dynamic=$Dynamic")
    Invoke-NativeCommand -Ibcmd $Ibcmd -Arguments $nativeArgs -Log "$($script:LabRoot)\logs\native-apply-$Tag" -TimeoutSec $TimeoutSec
}
