# read-only: which Config rows mention the given guids (decoded text search)?  find_refs.ps1 -Database <db> -Guids 'g1|g2|...' [-Table Config]
param([Parameter(Mandatory = $true)][string]$Database, [Parameter(Mandatory = $true)][string]$Guids, [string]$Table = 'Config')
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$needles = @($Guids -split '\|')
$cn = New-Object System.Data.SqlClient.SqlConnection "Server=localhost;Database=$Database;Integrated Security=true;TrustServerCertificate=true;Application Name=apply-trace-check"
$cn.Open()
$cmd = $cn.CreateCommand()
$cmd.CommandTimeout = 600
$cmd.CommandText = "SELECT FileName, BinaryData FROM $Table WHERE PartNo = 0"
$r = $cmd.ExecuteReader()
$rows = 0
while ($r.Read()) {
    $rows++
    $name = $r.GetString(0)
    $b = [byte[]]$r.GetValue(1)
    try {
        $ms = New-Object System.IO.MemoryStream(, $b)
        $ds = New-Object System.IO.Compression.DeflateStream($ms, [System.IO.Compression.CompressionMode]::Decompress)
        $o = New-Object System.IO.MemoryStream
        $ds.CopyTo($o)
        $bytes = $o.ToArray()
    } catch { $bytes = $b }
    $text = [Text.Encoding]::Latin1.GetString($bytes)
    foreach ($g in $needles) {
        if ($text.IndexOf($g, [StringComparison]::OrdinalIgnoreCase) -ge 0) { '{0}`t{1}`t{2}' -f $g.Substring(0, 8), $name, $bytes.Length }
    }
}
$r.Close()
$cn.Close()
"rows scanned: $rows"
