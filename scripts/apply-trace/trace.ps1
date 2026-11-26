<#
.SYNOPSIS
  Runs one command under an Extended Events trace of one SQL Server database
  and turns the trace into a bounded, ordered report.

.DESCRIPTION
  1. creates and starts the XE session ibcmd_rs_04_<Track>_<n> (statement,
     batch, RPC, user transaction and error events of the database, with
     session id, transaction id and a global event sequence number; target:
     event files under C:\temp\ibcmd_rs_04\<Track>\);
  2. runs the command (a script block, or -Exe with -ArgumentList) and saves
     its output to command.log;
  3. stops and drops the session, exports the raw events to events.xml.gz;
  4. runs trace_report.py: statement groups (normalized, in order of first
     appearance, with counts), an ordered log of the writes to the service
     tables (Config, Params, Files, ...), the DDL verbatim, transaction
     boundaries, durations, a phase timeline.

  The session is dropped even when the command fails; -Cleanup drops sessions
  that a killed run left behind.

.EXAMPLE
  $ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
  .\trace.ps1 -Database ibcmd_rs_04_trace_x -Tag apply -OutDir F:\lab\out\apply `
     -Command { & $ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=ibcmd_rs_04_trace_x --user=Администратор --force --dynamic=disable }

.NOTES
  Events are only those of the database (sqlserver.database_id); use
  -AllDatabases for commands that also work in master (infobase create).
  rpc_completed keeps at most the first 1,000,000 bytes of a binary parameter
  (SQL Server truncates the statement text at 2,000,000 characters);
  sql_batch_completed keeps the whole text.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Tag,
    [scriptblock]$Command,
    [string]$Exe,
    [string[]]$ArgumentList = @(),
    [string]$OutDir = '',
    [string]$Server = 'localhost',
    # session name ibcmd_rs_04_<Track>_<n>, event files in C:\temp\ibcmd_rs_04\<Track>\
    [string]$Track = 'trace',
    [int]$Number = 0,
    # also record sql_statement_completed (each statement inside a batch)
    [switch]$IncludeStatements,
    # no database filter: everything of non-system sessions (infobase create)
    [switch]$AllDatabases,
    # a raw XE predicate that replaces the database filter
    [string]$Predicate = '',
    # statements longer than this are cut in events.tsv.gz (their length is kept)
    [int]$MaxStatementKB = 1024,
    [switch]$KeepXel,
    # keep events.xml.gz (the raw events) next to the parsed events.tsv.gz
    [switch]$KeepRaw,
    [switch]$NoReport,
    # kill the command (and its child processes) after this many minutes; 0 = no limit
    [int]$TimeoutMinutes = 0,
    # script blocks run right before / after the command (After also on failure), outside its timeout
    [scriptblock]$BeforeCommand,
    [scriptblock]$AfterCommand,
    # drop sessions named ibcmd_rs_04_<Track>_* left behind by a killed run and exit
    [switch]$Cleanup,
    [string]$Note = ''
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
. (Join-Path $PSScriptRoot 'lib\common.ps1')

if ($Track -notmatch '^[a-z0-9]+$') { throw "-Track must match ^[a-z0-9]+$ (got '$Track')" }
$prefix = "ibcmd_rs_04_${Track}_"
$xelDir = "C:\temp\ibcmd_rs_04\$Track"

$session = $null
$master = Open-Sql -Server $Server -Database master
try {
    if ($Cleanup) {
        $stale = Invoke-SqlRows $master "SELECT name FROM sys.server_event_sessions WHERE name LIKE N'$prefix%'"
        foreach ($s in $stale) {
            $n = $s['name']
            $running = Invoke-SqlScalar $master "SELECT COUNT(*) FROM sys.dm_xe_sessions WHERE name = N'$n'"
            if ($running -gt 0) { Invoke-SqlNonQuery $master "ALTER EVENT SESSION [$n] ON SERVER STATE = STOP" }
            Invoke-SqlNonQuery $master "DROP EVENT SESSION [$n] ON SERVER"
            Write-Log "dropped stale session $n"
        }
        if (-not $stale -or $stale.Count -eq 0) { Write-Log "no sessions named $prefix*" }
        return
    }

    if (-not $Command -and -not $Exe) { throw 'give -Command <scriptblock> or -Exe <path> [-ArgumentList ...]' }
    $dbId = Invoke-SqlScalar $master 'SELECT DB_ID(@n)' @{ '@n' = $Database }
    if ($null -eq $dbId) { throw "database '$Database' not found on $Server" }
    if (-not $OutDir) { $OutDir = Join-Path (Get-Location) ('trace-{0}-{1}' -f $Tag, (Get-Date -Format 'yyyyMMdd-HHmmss')) }
    New-Item -ItemType Directory -Force -Path $OutDir, $xelDir | Out-Null
    $OutDir = (Resolve-Path -LiteralPath $OutDir).Path

    # Session name: the first free ibcmd_rs_04_<track>_<n>.
    if ($Number -le 0) {
        $used = @(); foreach ($r in (Invoke-SqlRows $master "SELECT name FROM sys.server_event_sessions WHERE name LIKE N'$prefix%'")) { $used += $r['name'] }
        $Number = 1
        while ($used -contains "$prefix$Number") { $Number++ }
    }
    $session = "$prefix$Number"
    $exists = Invoke-SqlScalar $master 'SELECT COUNT(*) FROM sys.server_event_sessions WHERE name = @n' @{ '@n' = $session }
    if ($exists -gt 0) { throw "event session $session exists already; run with -Cleanup or pick -Number" }

    # Which events, with which filter.
    $dbFilter = if ($Predicate) { $Predicate } elseif ($AllDatabases) { 'sqlserver.is_system = 0' } else { "sqlserver.database_id = $dbId" }
    $actions = 'package0.event_sequence, sqlserver.session_id, sqlserver.transaction_id, sqlserver.client_app_name, sqlserver.database_name'
    $events = @(
        "ADD EVENT sqlserver.rpc_completed (ACTION ($actions) WHERE $dbFilter)",
        "ADD EVENT sqlserver.sql_batch_completed (ACTION ($actions) WHERE $dbFilter)",
        "ADD EVENT sqlserver.sql_transaction (ACTION ($actions) WHERE ($dbFilter) AND transaction_type = 1)",
        "ADD EVENT sqlserver.begin_tran_completed (ACTION ($actions) WHERE $dbFilter)",
        "ADD EVENT sqlserver.commit_tran_completed (ACTION ($actions) WHERE $dbFilter)",
        "ADD EVENT sqlserver.rollback_tran_completed (ACTION ($actions) WHERE $dbFilter)",
        "ADD EVENT sqlserver.error_reported (ACTION ($actions, sqlserver.sql_text) WHERE ($dbFilter) AND severity >= 11)"
    )
    if ($IncludeStatements) {
        $events += "ADD EVENT sqlserver.sql_statement_completed (ACTION ($actions) WHERE $dbFilter)"
    }
    # a time stamp in the file name: files left by an earlier run with the same session name are never read
    $xelBase = Join-Path $xelDir ('{0}_{1}.xel' -f $session, (Get-Date -Format 'yyyyMMddHHmmss'))
    $xelPrefix = [System.IO.Path]::GetFileNameWithoutExtension($xelBase)
    $create = "CREATE EVENT SESSION [$session] ON SERVER`n" + ($events -join ",`n") + @"

ADD TARGET package0.event_file (SET filename = N'$xelBase', max_file_size = 256, max_rollover_files = 64)
WITH (MAX_MEMORY = 128 MB, MAX_EVENT_SIZE = 128 MB, EVENT_RETENTION_MODE = ALLOW_SINGLE_EVENT_LOSS,
      MAX_DISPATCH_LATENCY = 1 SECONDS, TRACK_CAUSALITY = OFF, STARTUP_STATE = OFF)
"@
    Set-Content -LiteralPath (Join-Path $OutDir 'session.sql') -Value $create -Encoding UTF8

    $sqlVersion = Invoke-SqlScalar $master 'SELECT @@VERSION'
    $startedUtc = $null; $endedUtc = $null; $observed = $null; $dropStats = $null; $eventCount = 0
    Invoke-SqlNonQuery $master $create
    try {
        Invoke-SqlNonQuery $master "ALTER EVENT SESSION [$session] ON SERVER STATE = START"
        $startedUtc = [DateTime]::UtcNow
        Write-Log "trace $session started on '$Database' (id $dbId); running the command"
        $logPath = Join-Path $OutDir 'command.log'
        $observed = Invoke-Observed -Command $Command -Exe $Exe -ArgumentList $ArgumentList -LogPath $logPath -TimeoutMinutes $TimeoutMinutes -Before $BeforeCommand -After $AfterCommand
        $endedUtc = [DateTime]::UtcNow
        Write-Log ("command finished: exit {0}, {1} s" -f $observed.ExitCode, $observed.Seconds)
        $ds = Invoke-SqlRows $master "SELECT dropped_event_count, dropped_buffer_count, largest_event_dropped_size, total_bytes_generated FROM sys.dm_xe_sessions WHERE name = N'$session'"
        if ($ds.Count -gt 0) { $dropStats = $ds[0] }
    } finally {
        try { Invoke-SqlNonQuery $master "ALTER EVENT SESSION [$session] ON SERVER STATE = STOP" } catch { Write-Log "stop failed: $_" }
    }

    # Export the raw events (they are in the files after STOP) to events.xml.gz.
    $xmlPath = Join-Path $OutDir 'events.xml.gz'
    $eventCount = Export-XelEvents -Conn $master -Pattern (($xelBase -replace '\.xel$', '') + '*.xel') -OutPath $xmlPath
    Write-Log "$eventCount events exported to events.xml.gz"

    $xelFiles = @(Get-ChildItem -LiteralPath $xelDir -Filter "$xelPrefix*.xel" -ErrorAction SilentlyContinue)
    $xelBytes = ($xelFiles | Measure-Object -Property Length -Sum).Sum
    $meta = [ordered]@{
        kit_version      = $script:KitVersion
        tag              = $Tag
        note             = $Note
        database         = $Database
        database_id      = $dbId
        server           = $Server
        sql_version      = ($sqlVersion -split "`n")[0].Trim()
        session          = $session
        predicate        = $dbFilter
        include_statements = [bool]$IncludeStatements
        started_utc      = $startedUtc.ToString('o')
        ended_utc        = $endedUtc.ToString('o')
        command          = if ($Command) { $Command.ToString().Trim() } else { ($Exe + ' ' + ($ArgumentList -join ' ')) }
        exit_code        = $observed.ExitCode
        command_seconds  = $observed.Seconds
        timed_out        = $observed.TimedOut
        events           = $eventCount
        dropped_events   = if ($dropStats) { [int64]$dropStats['dropped_event_count'] } else { $null }
        dropped_buffers  = if ($dropStats) { [int64]$dropStats['dropped_buffer_count'] } else { $null }
        largest_dropped  = if ($dropStats) { [int64]$dropStats['largest_event_dropped_size'] } else { $null }
        xel_bytes        = $xelBytes
        max_statement_kb = $MaxStatementKB
    }
    [System.IO.File]::WriteAllText((Join-Path $OutDir 'trace-meta.json'), ($meta | ConvertTo-Json -Depth 4), $script:Utf8NoBom)
    Invoke-SqlNonQuery $master "DROP EVENT SESSION [$session] ON SERVER"
    Write-Log "session $session dropped"
    if (-not $KeepXel) { $xelFiles | Remove-Item -Force -ErrorAction SilentlyContinue }
} finally {
    # Never leave the session behind, whatever failed above.
    try {
        if ($session) {
            $left = Invoke-SqlScalar $master 'SELECT COUNT(*) FROM sys.server_event_sessions WHERE name = @n' @{ '@n' = $session }
            if ($left -gt 0) {
                $run = Invoke-SqlScalar $master 'SELECT COUNT(*) FROM sys.dm_xe_sessions WHERE name = @n' @{ '@n' = $session }
                if ($run -gt 0) { Invoke-SqlNonQuery $master "ALTER EVENT SESSION [$session] ON SERVER STATE = STOP" }
                Invoke-SqlNonQuery $master "DROP EVENT SESSION [$session] ON SERVER"
                Write-Log "session $session dropped (cleanup)"
            }
        }
    } catch { Write-Log "cleanup of $session failed: $_" }
    $master.Close()
}

if (-not $NoReport) {
    $py = Get-Command python -ErrorAction SilentlyContinue
    if (-not $py) { Write-Log 'python not found: events.xml.gz is written, the report is skipped'; return }
    $env:PYTHONIOENCODING = 'utf-8'
    $reportArgs = @('--xml', $xmlPath, '--max-statement-kb', $MaxStatementKB, '--meta', (Join-Path $OutDir 'trace-meta.json'), '--out', $OutDir)
    if ($KeepRaw) { $reportArgs += '--keep-intermediate' }
    # without the database filter the trace holds every non-system session: keep those that touched the database
    if ($AllDatabases -or $Predicate) { $reportArgs += @('--focus-db', $Database) }
    & $py.Source (Join-Path $PSScriptRoot 'trace_report.py') @reportArgs
    if ($LASTEXITCODE -ne 0) { throw "trace_report.py failed ($LASTEXITCODE)" }
    # events.tsv (parsed, statements cut at -MaxStatementKB) replaces the raw XML unless -KeepRaw
    if (-not $KeepRaw) { Remove-Item -LiteralPath $xmlPath -Force -ErrorAction SilentlyContinue }
}
Write-Log "trace report: $OutDir"
