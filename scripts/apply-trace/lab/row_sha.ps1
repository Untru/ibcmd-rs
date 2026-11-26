# decoded sha256 of Config rows in a live database (read-only): row_sha.ps1 -Database <db> -Names 'a|b|c'
param([Parameter(Mandatory = $true)][string]$Database, [Parameter(Mandatory = $true)][string]$Names, [string]$Table = 'Config')
$ErrorActionPreference = 'Stop'
$cn = New-Object System.Data.SqlClient.SqlConnection "Server=localhost;Database=$Database;Integrated Security=true;TrustServerCertificate=true;Application Name=apply-trace-check"
$cn.Open()
foreach ($n in ($Names -split '[|,]')) {
    $cmd = $cn.CreateCommand()
    $cmd.CommandText = "SELECT BinaryData FROM $Table WHERE FileName = @n ORDER BY PartNo"
    [void]$cmd.Parameters.AddWithValue('@n', $n)
    $ms = New-Object System.IO.MemoryStream
    $r = $cmd.ExecuteReader()
    while ($r.Read()) { $b = [byte[]]$r.GetValue(0); $ms.Write($b, 0, $b.Length) }
    $r.Close()
    $ms.Position = 0
    $out = New-Object System.IO.MemoryStream
    try { $ds = New-Object System.IO.Compression.DeflateStream($ms, [System.IO.Compression.CompressionMode]::Decompress); $ds.CopyTo($out); $bytes = $out.ToArray() } catch { $bytes = $ms.ToArray() }
    $sha = [System.BitConverter]::ToString([System.Security.Cryptography.SHA256]::Create().ComputeHash($bytes)).Replace('-', '').ToLowerInvariant()
    '{0}`t{1}`t{2}' -f $n, $bytes.Length, $sha
}
$cn.Close()
