# read-only: dump the decoded bytes of rows of a file table: dump_rows.ps1 -Database <db> -Names 'a|b' -Out <dir> [-Table Config]
param([Parameter(Mandatory = $true)][string]$Database, [Parameter(Mandatory = $true)][string]$Names, [Parameter(Mandatory = $true)][string]$Out, [string]$Table = 'Config')
$ErrorActionPreference = 'Stop'
$cn = New-Object System.Data.SqlClient.SqlConnection "Server=localhost;Database=$Database;Integrated Security=true;TrustServerCertificate=true;Application Name=apply-trace-check"
$cn.Open()
New-Item -ItemType Directory -Force -Path $Out | Out-Null
foreach ($n in ($Names -split '\|')) {
    $cmd = $cn.CreateCommand()
    $cmd.CommandText = "SELECT BinaryData FROM $Table WHERE FileName = @n ORDER BY PartNo"
    [void]$cmd.Parameters.AddWithValue('@n', $n)
    $ms = New-Object System.IO.MemoryStream
    $r = $cmd.ExecuteReader()
    while ($r.Read()) { $b = [byte[]]$r.GetValue(0); $ms.Write($b, 0, $b.Length) }
    $r.Close()
    $ms.Position = 0
    $o = New-Object System.IO.MemoryStream
    try { $ds = New-Object System.IO.Compression.DeflateStream($ms, [System.IO.Compression.CompressionMode]::Decompress); $ds.CopyTo($o); $bytes = $o.ToArray() } catch { $bytes = $ms.ToArray() }
    [IO.File]::WriteAllBytes((Join-Path $Out $n), $bytes)
    '{0} {1}' -f $n, $bytes.Length
}
$cn.Close()
