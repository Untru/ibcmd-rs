# A session of a lab database that holds WORK IN FLIGHT: it opens a transaction, inserts one row into dbo.IbcmdRsLiveProbe and holds
# the transaction open until a release file appears -- what the live switch would roll back (#409 F-10). Then it commits and says how
# that went: the row survives (COMMITTED) or the connection was taken away (LOST <error>).
#   pwsh -NoProfile -File hold_txn.ps1 -Database <db> -ReleaseFile <path> [-HoldSec 300] [-Program ibcmd-lab-writer] [-Server localhost]
# The program name is NOT "1CV83 Server", so the session is not one of the 1C connections the readiness gate waits for.
# dbo.IbcmdRsLiveProbe(id int, note nvarchar(100)) is created by the script in the lab clone when it is missing.
param(
    [Parameter(Mandatory = $true)][ValidatePattern('^ibcmd_rs_0[45]_[a-z0-9_]+$')][string]$Database,
    [Parameter(Mandatory = $true)][string]$ReleaseFile,
    [int]$HoldSec = 300,
    [string]$Program = 'ibcmd-lab-writer',
    [string]$Server = 'localhost',
    [string]$Note = 'work in flight'
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$connection = New-Object System.Data.SqlClient.SqlConnection("Server=$Server;Database=$Database;Integrated Security=SSPI;TrustServerCertificate=True;Application Name=$Program;Connect Timeout=15")
function Run([string]$text) { $c = $connection.CreateCommand(); $c.CommandText = $text; $c.CommandTimeout = 0; [void]$c.ExecuteNonQuery() }
try {
    $connection.Open()
    Run "IF OBJECT_ID(N'dbo.IbcmdRsLiveProbe') IS NULL CREATE TABLE dbo.IbcmdRsLiveProbe (id int NOT NULL, note nvarchar(100) NOT NULL)"
    $spid = $connection.CreateCommand(); $spid.CommandText = 'SELECT @@SPID'; $id = $spid.ExecuteScalar()
    $transaction = $connection.BeginTransaction()
    $c = $connection.CreateCommand(); $c.Transaction = $transaction
    $c.CommandText = "INSERT dbo.IbcmdRsLiveProbe (id, note) VALUES (@id, @note)"
    [void]$c.Parameters.AddWithValue('@id', [int]$id); [void]$c.Parameters.AddWithValue('@note', $Note)
    [void]$c.ExecuteNonQuery()
    "READY session $id holds an open transaction with one row in dbo.IbcmdRsLiveProbe"
    $end = (Get-Date).AddSeconds($HoldSec)
    while ((Get-Date) -lt $end -and -not (Test-Path -LiteralPath $ReleaseFile)) { Start-Sleep -Milliseconds 300 }
    try { $transaction.Commit(); 'COMMITTED' } catch { "LOST $($_.Exception.InnerException.Message)$($_.Exception.Message)".Trim() }
} catch {
    "ERROR $($_.Exception.Message)"
    exit 2
} finally {
    $connection.Dispose()
}
