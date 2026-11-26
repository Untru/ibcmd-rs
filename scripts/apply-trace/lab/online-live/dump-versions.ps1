# Windows PowerShell 5.1. Writes the inflated `versions` blobs of a lab database to text files (read-only).
#   powershell -NoProfile -File dump-versions.ps1 -Database <db> -OutDir <dir>
# Files: Config.versions.txt, Config.versions_dynupdate_<gen>.txt, ConfigSave.versions.txt
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$OutDir
)
$ErrorActionPreference = 'Stop'
if ($Database -notmatch '^ibcmd_rs_0[45]_[a-z0-9_]+$') { throw 'lab databases only' }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
Add-Type -AssemblyName System.Data
$conn = New-Object System.Data.SqlClient.SqlConnection "Server=localhost;Database=$Database;Integrated Security=SSPI;TrustServerCertificate=True;Application Name=ibcmd-rs-lab-dumpversions;Connect Timeout=10"
$conn.Open()
foreach ($table in 'Config', 'ConfigSave') {
    $cmd = $conn.CreateCommand()
    $cmd.CommandText = "SELECT FileName, BinaryData FROM dbo.$table WHERE PartNo = 0 AND (FileName = N'versions' OR FileName LIKE N'versions[_]dynupdate[_]%')"
    $r = $cmd.ExecuteReader()
    while ($r.Read()) {
        $blob = [byte[]]$r['BinaryData']
        $ms = New-Object System.IO.MemoryStream(, $blob)
        $ds = New-Object System.IO.Compression.DeflateStream($ms, [System.IO.Compression.CompressionMode]::Decompress)
        $out = New-Object System.IO.MemoryStream
        $ds.CopyTo($out)
        [System.IO.File]::WriteAllBytes((Join-Path $OutDir ("{0}.{1}.txt" -f $table, $r['FileName'])), $out.ToArray())
    }
    $r.Close()
}
$conn.Close()
Get-ChildItem -LiteralPath $OutDir | Select-Object Name, Length
