# Windows PowerShell 5.1: writes the raw BinaryData of Config rows to files (read-only), one file per row name.
param([Parameter(Mandatory = $true)][string]$Database, [Parameter(Mandatory = $true)][string]$OutDir, [Parameter(Mandatory = $true)][string[]]$Names)
if ($Database -notmatch '^ibcmd_rs_0[45]_[a-z0-9_]+$') { throw 'lab databases only' }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
Add-Type -AssemblyName System.Data
$conn = New-Object System.Data.SqlClient.SqlConnection "Server=localhost;Database=$Database;Integrated Security=SSPI;TrustServerCertificate=True;Application Name=ibcmd-rs-lab-rowhex;Connect Timeout=10"
$conn.Open()
foreach ($n in ($Names -split ",")) {
    $cmd = $conn.CreateCommand(); $cmd.CommandText = 'SELECT BinaryData FROM dbo.Config WHERE FileName = @n AND PartNo = 0'
    $null = $cmd.Parameters.AddWithValue('@n', $n)
    $v = $cmd.ExecuteScalar()
    if ($v -ne $null) { [System.IO.File]::WriteAllBytes((Join-Path $OutDir ($n + '.bin')), [byte[]]$v); "$n : $(([byte[]]$v).Length) bytes" } else { "$n : missing" }
}
$conn.Close()
