# read-only: for every ConfigSave row of a database, is the decoded content equal to the Config row of the same name?
# stage_vs_config.ps1 -Database <db> [-Dump <dir>]   (Dump: write the decoded bytes of the differing rows there)
param([Parameter(Mandatory = $true)][string]$Database, [string]$Dump = '')
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$cn = New-Object System.Data.SqlClient.SqlConnection "Server=localhost;Database=$Database;Integrated Security=true;TrustServerCertificate=true;Application Name=apply-trace-check"
$cn.Open()
function Get-Decoded($table, $name) {
    $cmd = $cn.CreateCommand()
    $cmd.CommandText = "SELECT BinaryData FROM $table WHERE FileName = @n ORDER BY PartNo"
    [void]$cmd.Parameters.AddWithValue('@n', $name)
    $ms = New-Object System.IO.MemoryStream
    $r = $cmd.ExecuteReader()
    $any = $false
    while ($r.Read()) { $any = $true; $b = [byte[]]$r.GetValue(0); $ms.Write($b, 0, $b.Length) }
    $r.Close()
    if (-not $any) { return $null }
    $ms.Position = 0
    $out = New-Object System.IO.MemoryStream
    try { $ds = New-Object System.IO.Compression.DeflateStream($ms, [System.IO.Compression.CompressionMode]::Decompress); $ds.CopyTo($out); return $out.ToArray() } catch { return $ms.ToArray() }
}
$names = New-Object System.Collections.Generic.List[string]
$cmd = $cn.CreateCommand(); $cmd.CommandText = 'SELECT DISTINCT FileName FROM ConfigSave ORDER BY FileName'
$r = $cmd.ExecuteReader(); while ($r.Read()) { $names.Add($r.GetString(0)) }; $r.Close()
$sha = [System.Security.Cryptography.SHA256]::Create()
foreach ($n in $names) {
    $s = Get-Decoded 'ConfigSave' $n
    $c = Get-Decoded 'Config' $n
    if ($null -eq $c) { $state = 'NEW (no Config row)' }
    elseif ([System.BitConverter]::ToString($sha.ComputeHash($s)) -eq [System.BitConverter]::ToString($sha.ComputeHash($c))) { $state = 'same' }
    else { $state = 'DIFFERENT' }
    '{0}`t{1}`t{2}`t{3}' -f $n, $s.Length, $(if ($null -eq $c) { '-' } else { $c.Length }), $state
    if ($Dump -ne '' -and $state -ne 'same') {
        New-Item -ItemType Directory -Force -Path $Dump | Out-Null
        [IO.File]::WriteAllBytes((Join-Path $Dump ($n + '.stage')), $s)
        if ($null -ne $c) { [IO.File]::WriteAllBytes((Join-Path $Dump ($n + '.config')), $c) }
    }
}
$cn.Close()
