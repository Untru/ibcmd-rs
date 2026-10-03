# Timeline of one lab database during a live switch: two samplers in one script.
#   pwsh -NoProfile -File timeline.ps1 -Database <db> -Out <prefix> [-SqlIntervalMs 100] [-RacIntervalMs 1000] [-MaxSeconds 600] [-StopFile <path>]
# <prefix>.sql.tsv  from master (NEVER connects into the database: a connection there could take the SINGLE_USER slot of the switch): a row
#                   per change of state / user_access / number of `1CV83 Server` connections / other sessions, and a heartbeat every 5 s;
# <prefix>.rac.tsv  through the RAS of the worker lab cluster, once per interval: the user sessions of the infobase (session, app-id,
#                   user, started-at, last-active-at, calls) -- a row when a session appears or disappears, and its last-active-at every
#                   interval (so that "made a call after T" can be read off).
# Times are UTC in ISO 8601 with milliseconds.
param(
    [Parameter(Mandatory = $true)][ValidatePattern('^ibcmd_rs_05_[a-z0-9_]+$')][string]$Database,
    [Parameter(Mandatory = $true)][string]$Out,
    [int]$SqlIntervalMs = 100,
    [int]$RacIntervalMs = 1000,
    [int]$MaxSeconds = 600,
    [string]$StopFile = ''
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\process.ps1"
$rac = 'C:\Program Files\1cv8\8.3.27.2214\bin\rac.exe'
$ras = 'localhost:5545'
$deadline = (Get-Date).AddSeconds($MaxSeconds)
$cluster = ((Invoke-LiveRac $rac @($ras,'cluster','list')) | Select-String '^cluster\s*:' | Select-Object -First 1) -replace '^cluster\s*:\s*', ''
$infobase = $null; $cur = $null
foreach ($line in Invoke-LiveRac $rac @($ras,'infobase','summary','list',"--cluster=$cluster")) {
    if ($line -match '^infobase\s*:\s*(\S+)') { $cur = $Matches[1] } elseif ($line -match '^name\s*:\s*(\S+)' -and $Matches[1] -eq $Database) { $infobase = $cur }
}
if (-not $infobase) { throw "$Database is not registered in the worker lab cluster" }

$conn = New-Object System.Data.SqlClient.SqlConnection('Server=localhost;Database=master;Integrated Security=SSPI;TrustServerCertificate=True;Application Name=ibcmd-rs-lab-sampler;Connect Timeout=5')
$conn.Open()
$cmd = $conn.CreateCommand(); $cmd.CommandTimeout = 5
$cmd.CommandText = @"
SELECT CONVERT(varchar(30), SYSUTCDATETIME(), 126), d.state_desc, d.user_access_desc,
  (SELECT COUNT(*) FROM sys.dm_exec_sessions s WHERE s.is_user_process = 1 AND s.database_id = d.database_id AND s.program_name = N'1CV83 Server'),
  (SELECT COUNT(*) FROM sys.dm_exec_sessions s WHERE s.is_user_process = 1 AND s.database_id = d.database_id AND s.program_name <> N'1CV83 Server')
FROM sys.databases d WHERE d.name = @db
"@
$null = $cmd.Parameters.Add('@db', [System.Data.SqlDbType]::NVarChar, 128); $cmd.Parameters['@db'].Value = $Database
$sql = New-Object System.IO.StreamWriter("$Out.sql.tsv", $false, (New-Object System.Text.UTF8Encoding($false))); $sql.AutoFlush = $true
$sql.WriteLine("utc`tstate`tuser_access`tsql_1c_connections`tsql_other_sessions")
$racOut = New-Object System.IO.StreamWriter("$Out.rac.tsv", $false, (New-Object System.Text.UTF8Encoding($false))); $racOut.AutoFlush = $true
$racOut.WriteLine("utc`tsession`tapp`tuser`tstarted_at`tlast_active_at`tcalls_5min`tprocess")

$lastSql = ''; $lastBeat = Get-Date; $nextRac = Get-Date; $known = @{}
while ((Get-Date) -lt $deadline) {
    if ($StopFile -and (Test-Path -LiteralPath $StopFile)) { break }
    try {
        $r = $cmd.ExecuteReader()
        if ($r.Read()) {
            $key = "{0}`t{1}`t{2}`t{3}" -f $r.GetValue(1), $r.GetValue(2), $r.GetValue(3), $r.GetValue(4)
            if ($key -ne $lastSql -or ((Get-Date) - $lastBeat).TotalSeconds -ge 5) { $sql.WriteLine(("{0}`t{1}" -f $r.GetValue(0), $key)); $lastSql = $key; $lastBeat = Get-Date }
        }
        $r.Close()
    } catch {
        $sql.WriteLine([DateTime]::UtcNow.ToString('o') + "`tERROR`t" + ($_.Exception.Message -replace '\s+', ' '))
        Start-Sleep -Milliseconds 300
        try { $conn.Close(); $conn.Open() } catch { }
    }
    if ((Get-Date) -ge $nextRac) {
        $nextRac = (Get-Date).AddMilliseconds($RacIntervalMs)
        $now = [DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ss.fff')
        $seen = @{}
        $block = @{}
        $lines = @(Invoke-LiveRac $rac @($ras,'session','list',"--cluster=$cluster","--infobase=$infobase")) + ''
        foreach ($line in $lines) {
            if ([string]::IsNullOrWhiteSpace($line)) {
                if ($block.ContainsKey('session')) {
                    $id = $block['session']; $seen[$id] = $true
                    $racOut.WriteLine(("{0}`t{1}`t{2}`t{3}`t{4}`t{5}`t{6}`t{7}" -f $now, $id, $block['app-id'], $block['user-name'], $block['started-at'], $block['last-active-at'], $block['calls-last-5min'], $block['process']))
                    $known[$id] = $true
                }
                $block = @{}
            } elseif ($line -match '^([\w-]+)\s*:\s*(.*)$') { $block[$Matches[1]] = $Matches[2].Trim().Trim('"') }
        }
        foreach ($id in @($known.Keys)) { if (-not $seen.ContainsKey($id)) { $racOut.WriteLine(("{0}`t{1}`tGONE" -f $now, $id)); $known.Remove($id) } }
    }
    Start-Sleep -Milliseconds $SqlIntervalMs
}
$sql.Close(); $racOut.Close(); $conn.Close()
