# Config rows of two lab databases: the rows (FileName, PartNo, Creation, Modified, Attributes, DataSize,
# BinaryData) that one has and the other does not, listed by name.
#   pwsh -NoProfile -File compare_config.ps1 -A <db> -B <db> [-Table Config] [-Out <file>]
# Read-only.
param(
    [Parameter(Mandatory = $true)][string]$A,
    [Parameter(Mandatory = $true)][string]$B,
    [string]$Table = 'Config',
    [switch]$IgnoreTimes,
    [string]$Out = ''
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
Add-Type -AssemblyName System.Data
function Invoke-Rows([string]$Db, [string]$Sql) {
    $c = New-Object System.Data.SqlClient.SqlConnection "Server=localhost;Database=$Db;Integrated Security=SSPI;TrustServerCertificate=True"
    $c.Open()
    try {
        $cmd = $c.CreateCommand(); $cmd.CommandText = $Sql; $cmd.CommandTimeout = 1200
        $r = $cmd.ExecuteReader()
        $rows = @()
        while ($r.Read()) { $rows += , @($r.GetValue(0), $r.GetValue(1)) }
        return $rows
    } finally { $c.Close() }
}
$cols = if ($IgnoreTimes) { 'FileName, PartNo, Attributes, DataSize, BinaryData' } else { 'FileName, PartNo, Creation, Modified, Attributes, DataSize, BinaryData' }
$lines = @()
foreach ($pair in @(@($A, $B, 'only in A'), @($B, $A, 'only in B'))) {
    $rows = Invoke-Rows $pair[0] "SELECT FileName, PartNo FROM (SELECT $cols FROM [$($pair[0])].dbo.$Table EXCEPT SELECT $cols FROM [$($pair[1])].dbo.$Table) d ORDER BY FileName, PartNo"
    $lines += "== $($pair[2]) ($($rows.Count) rows)"
    foreach ($r in $rows) { $lines += "  $($r[0]) [$($r[1])]" }
}
$lines | ForEach-Object { $_ }
if ($Out) { $lines | Set-Content -LiteralPath $Out -Encoding UTF8 }
