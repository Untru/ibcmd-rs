# Config rows of two lab databases: the rows (FileName, PartNo, Creation, Modified, Attributes, DataSize,
# BinaryData) that one has and the other does not, listed by name.
#   pwsh -NoProfile -File compare_config.ps1 -A <db> -B <db> [-Table Config] [-Out <file>] [-ExactSummary]
# -ExactSummary compares every column, emits only counts and exits 1 on differences.
# Read-only.
param(
    [Parameter(Mandatory = $true)][string]$A,
    [Parameter(Mandatory = $true)][string]$B,
    [string]$Table = 'Config',
    [switch]$IgnoreTimes,
    [switch]$ExactSummary,
    [string]$Out = ''
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
Add-Type -AssemblyName System.Data
function Invoke-Rows([string]$Db, [string]$Sql, [string]$Peer) {
    $options = [System.Data.SqlClient.SqlConnectionStringBuilder]::new("Server=localhost;Integrated Security=SSPI;TrustServerCertificate=True")
    $options['Initial Catalog'] = $Db
    $c = [System.Data.SqlClient.SqlConnection]::new($options.ConnectionString)
    $c.Open()
    try {
        $cmd = $c.CreateCommand(); $cmd.CommandText = $Sql; $cmd.CommandTimeout = 1200
        if ($Peer) { [void]$cmd.Parameters.AddWithValue('@peer', $Peer) }
        $r = $cmd.ExecuteReader()
        $rows = @()
        while ($r.Read()) { $rows += , @($r.GetValue(0), $r.GetValue(1)) }
        return $rows
    } finally { $c.Close() }
}
$cols = if ($IgnoreTimes) { 'FileName, PartNo, Attributes, DataSize, BinaryData' } else { 'FileName, PartNo, Creation, Modified, Attributes, DataSize, BinaryData' }
if ($ExactSummary) {
    if ($IgnoreTimes -or $A -eq $B) {
        [Console]::Error.WriteLine('Exact summary requires distinct database twins and includes timestamps.')
        exit 1
    }
    # EXCEPT uses the database collation for text and pads binary values with zeroes.
    $cols = 'CONVERT(varbinary(max), FileName) AS NameBytes, DATALENGTH(FileName) AS NameSize, PartNo, Creation, Modified, Attributes, DataSize, DATALENGTH(BinaryData) AS BlobSize, BinaryData'
}
$lines = @()
$different = $false
try {
    foreach ($pair in @(@($A, $B, 'only in A'), @($B, $A, 'only in B'))) {
        $left = '[' + $pair[0].Replace(']', ']]') + ']'
        $right = '[' + $pair[1].Replace(']', ']]') + ']'
        $tab = '[' + $Table.Replace(']', ']]') + ']'
        $diff = "SELECT $cols FROM $left.dbo.$tab EXCEPT SELECT $cols FROM $right.dbo.$tab"
        if ($ExactSummary) {
            $query = "IF DB_ID(@peer) IS NULL OR DB_ID(@peer) = DB_ID() THROW 50001, 'Expected distinct twins', 1;
                      SELECT COUNT_BIG(*), (SELECT COUNT_BIG(*) FROM $left.dbo.$tab) FROM ($diff) d"
            $result = @(Invoke-Rows $pair[0] $query $pair[1])
            $count, $total = $result[0]
            if ($total -eq 0) { throw 'Empty table' }
            $lines += "$($pair[2]): $count of $total rows"
            $different = $different -or $count -ne 0
        } else {
            $rows = @(Invoke-Rows $pair[0] "SELECT FileName, PartNo FROM ($diff) d ORDER BY FileName, PartNo")
            $lines += "== $($pair[2]) ($($rows.Count) rows)"
            foreach ($r in $rows) { $lines += "  $($r[0]) [$($r[1])]" }
        }
    }
    if ($Out) { $lines | Set-Content -LiteralPath $Out -Encoding UTF8 }
} catch {
    if (!$ExactSummary) { throw }
    [Console]::Error.WriteLine('4. unable to compare populated SQL tables or write summary')
    exit 1
}
$lines
if ($ExactSummary -and $different) { exit 1 }
