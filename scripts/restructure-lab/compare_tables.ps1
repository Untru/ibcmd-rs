# Data of tables of two lab databases with the same structure: rows both ways with EXCEPT, every column but the
# row version (timestamp). One line per table: rows in A, rows in B, rows only in A, rows only in B.
#   pwsh -NoProfile -File compare_tables.ps1 -A <db> -B <db> -Tables _Reference20,_Document39 [-Out <file>]
# Read-only. Databases are compared as they are (both must exist on localhost).
param(
    [Parameter(Mandatory = $true)][string]$A,
    [Parameter(Mandatory = $true)][string]$B,
    [Parameter(Mandatory = $true)][string]$Tables,
    [string]$Out = ''
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
Add-Type -AssemblyName System.Data
function Invoke-Scalar([string]$Db, [string]$Sql) {
    $c = New-Object System.Data.SqlClient.SqlConnection "Server=localhost;Database=$Db;Integrated Security=SSPI;TrustServerCertificate=True"
    $c.Open()
    try {
        $cmd = $c.CreateCommand(); $cmd.CommandText = $Sql; $cmd.CommandTimeout = 1200
        return $cmd.ExecuteScalar()
    } finally { $c.Close() }
}
$lines = @()
foreach ($table in ($Tables -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ })) {
    $cols = Invoke-Scalar $A "SELECT STRING_AGG(QUOTENAME(c.name), ', ') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo.$table') AND t.name <> N'timestamp'"
    if (-not $cols) { $lines += "$table`tmissing in $A"; continue }
    $na = Invoke-Scalar $A "SELECT COUNT_BIG(*) FROM dbo.$table"
    $nb = Invoke-Scalar $B "SELECT COUNT_BIG(*) FROM dbo.$table"
    $ab = Invoke-Scalar $A "SELECT COUNT_BIG(*) FROM (SELECT $cols FROM [$A].dbo.$table EXCEPT SELECT $cols FROM [$B].dbo.$table) d"
    $ba = Invoke-Scalar $A "SELECT COUNT_BIG(*) FROM (SELECT $cols FROM [$B].dbo.$table EXCEPT SELECT $cols FROM [$A].dbo.$table) d"
    $lines += "{0}`trows {1} / {2}`tonly in A {3}`tonly in B {4}" -f $table, $na, $nb, $ab, $ba
}
$lines | ForEach-Object { $_ }
if ($Out) { $lines | Set-Content -LiteralPath $Out -Encoding UTF8 }
