# Windows PowerShell 5.1. Prints the configuration-generation state of a lab database (read-only):
# ordinary `versions` generation (Config, ConfigSave), every `versions_dynupdate_*` alias, the two DynamicallyUpdated
# markers, and the counts of alias rows. Connects INTO the database: run it when no live activation is in progress.
#   powershell -NoProfile -File gen-state.ps1 -Database <db>
param([Parameter(Mandatory = $true)][string]$Database)
$ErrorActionPreference = 'Stop'
if ($Database -notmatch '^ibcmd_rs_0[45]_[a-z0-9_]+$') { throw 'lab databases only' }
Add-Type -AssemblyName System.Data
$conn = New-Object System.Data.SqlClient.SqlConnection "Server=localhost;Database=$Database;Integrated Security=SSPI;TrustServerCertificate=True;Application Name=ibcmd-rs-lab-genstate;Connect Timeout=10"
$conn.Open()

function Read-Rows([string]$sql) {
    $cmd = $conn.CreateCommand(); $cmd.CommandText = $sql; $cmd.CommandTimeout = 60
    $r = $cmd.ExecuteReader(); $rows = @()
    while ($r.Read()) {
        $o = @{}
        for ($i = 0; $i -lt $r.FieldCount; $i++) { $o[$r.GetName($i)] = $r.GetValue($i) }
        $rows += , $o
    }
    $r.Close(); $rows
}
function Header-Of([byte[]]$blob) {
    if ($null -eq $blob -or $blob.Length -eq 0) { return '(empty)' }
    try {
        $ms = New-Object System.IO.MemoryStream(, $blob)
        $ds = New-Object System.IO.Compression.DeflateStream($ms, [System.IO.Compression.CompressionMode]::Decompress)
        $buf = New-Object byte[] 400
        $n = $ds.Read($buf, 0, 400)
        $txt = [System.Text.Encoding]::UTF8.GetString($buf, 0, $n).TrimStart([char]0xFEFF)
        $m = [regex]::Match($txt, '^\{1,(\d+),"",([0-9a-fA-F-]{36})')
        if ($m.Success) { return "gen=$($m.Groups[2].Value) rows=$($m.Groups[1].Value)" }
        return "header?=" + $txt.Substring(0, [Math]::Min(80, $txt.Length))
    } catch { return "inflate-failed: $($_.Exception.Message)" }
}
function Text-Of([byte[]]$blob) {
    try {
        $ms = New-Object System.IO.MemoryStream(, $blob)
        $ds = New-Object System.IO.Compression.DeflateStream($ms, [System.IO.Compression.CompressionMode]::Decompress)
        $buf = New-Object byte[] 400
        $n = $ds.Read($buf, 0, 400)
        return [System.Text.Encoding]::UTF8.GetString($buf, 0, $n).TrimStart([char]0xFEFF).Replace("`r", '').Replace("`n", ' ')
    } catch { return "inflate-failed: $($_.Exception.Message)" }
}
foreach ($table in 'Config', 'ConfigSave') {
    foreach ($row in (Read-Rows "SELECT FileName, PartNo, DataSize, BinaryData FROM dbo.$table WHERE FileName IN (N'root', N'version') OR FileName LIKE N'root[_]dynupdate[_]%' ORDER BY FileName")) {
        "{0,-11} {1,-58} part={2} size={3} text={4}" -f $table, $row.FileName, $row.PartNo, $row.DataSize, (Text-Of $row.BinaryData)
    }
}
foreach ($table in 'Config', 'ConfigSave') {
    foreach ($row in (Read-Rows "SELECT FileName, PartNo, DataSize, BinaryData FROM dbo.$table WHERE FileName = N'versions' OR FileName LIKE N'versions[_]dynupdate[_]%' ORDER BY FileName")) {
        "{0,-11} {1,-58} part={2} size={3} {4}" -f $table, $row.FileName, $row.PartNo, $row.DataSize, (Header-Of $row.BinaryData)
    }
}
foreach ($table in 'Config', 'Params') {
    foreach ($row in (Read-Rows "SELECT FileName, PartNo, DataSize, BinaryData FROM dbo.$table WHERE FileName = N'DynamicallyUpdated'")) {
        $txt = [System.Text.Encoding]::UTF8.GetString($row.BinaryData).TrimStart([char]0xFEFF)
        "{0,-11} DynamicallyUpdated size={1} {2}" -f $table, $row.DataSize, $txt
    }
}
foreach ($table in 'Config', 'ConfigSave', 'Params') {
    $n = (Read-Rows "SELECT COUNT(*) c, SUM(CASE WHEN FileName LIKE N'%[_]dynupdate[_]%' THEN 1 ELSE 0 END) a FROM dbo.$table")[0]
    "{0,-11} rows={1} alias_rows={2}" -f $table, $n.c, $n.a
}
$conn.Close()
