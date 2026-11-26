# Windows PowerShell 5.1 (System.Data.SqlClient). Samples the STATE of one lab database from master only
# (never opens a connection into the database: a connection there could take the SINGLE_USER slot of a live activation).
#   powershell -NoProfile -File sql-timeline.ps1 -Database <db> -Out <file.tsv> [-IntervalMs 100] [-MaxSeconds 900] [-StopFile <path>]
# One TSV row per change (and a heartbeat every 5 s): utc, state, user_access, 1C SQL connections, other user sessions,
# tail: log-backup history rows for the database (count).
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Out,
    [int]$IntervalMs = 100,
    [int]$MaxSeconds = 900,
    [string]$StopFile = ''
)
$ErrorActionPreference = 'Stop'
if ($Database -notmatch '^ibcmd_rs_0[45]_[a-z0-9_]+$') { throw 'lab databases only' }
Add-Type -AssemblyName System.Data
$cs = 'Server=localhost;Database=master;Integrated Security=SSPI;TrustServerCertificate=True;Application Name=ibcmd-rs-lab-sampler;Connect Timeout=5'
$conn = New-Object System.Data.SqlClient.SqlConnection $cs
$conn.Open()
$cmd = $conn.CreateCommand()
$cmd.CommandText = @"
SELECT CONVERT(varchar(30), SYSUTCDATETIME(), 126) AS t,
       d.state_desc, d.user_access_desc,
       (SELECT COUNT(*) FROM sys.dm_exec_sessions s WHERE s.is_user_process = 1 AND s.database_id = d.database_id AND s.program_name = N'1CV83 Server') AS c1c,
       (SELECT COUNT(*) FROM sys.dm_exec_sessions s WHERE s.is_user_process = 1 AND s.database_id = d.database_id AND s.program_name <> N'1CV83 Server') AS cother
FROM sys.databases d WHERE d.name = @db
"@
$null = $cmd.Parameters.Add('@db', [System.Data.SqlDbType]::NVarChar, 128)
$cmd.Parameters['@db'].Value = $Database
$writer = New-Object System.IO.StreamWriter($Out, $false, (New-Object System.Text.UTF8Encoding($false)))
$writer.AutoFlush = $true
$writer.WriteLine("utc`tstate`tuser_access`tsql_1c_connections`tsql_other_sessions")
$last = ''
$lastBeat = Get-Date
$deadline = (Get-Date).AddSeconds($MaxSeconds)
while ((Get-Date) -lt $deadline) {
    if ($StopFile -and (Test-Path -LiteralPath $StopFile)) { break }
    try {
        $r = $cmd.ExecuteReader()
        if ($r.Read()) {
            $key = "{0}`t{1}`t{2}`t{3}" -f $r['state_desc'], $r['user_access_desc'], $r['c1c'], $r['cother']
            if ($key -ne $last -or ((Get-Date) - $lastBeat).TotalSeconds -ge 5) {
                $writer.WriteLine(("{0}`t{1}" -f $r['t'], $key))
                $last = $key
                $lastBeat = Get-Date
            }
        }
        $r.Close()
    } catch {
        $msg = $_.Exception.Message -replace '\s+', ' '
        $writer.WriteLine([DateTime]::UtcNow.ToString('o') + "`tERROR`t" + $msg)
        Start-Sleep -Milliseconds 500
        try { $conn.Close(); $conn.Open() } catch { }
    }
    Start-Sleep -Milliseconds $IntervalMs
}
$writer.Close()
$conn.Close()
