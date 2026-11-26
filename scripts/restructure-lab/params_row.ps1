# Read, replace or delete one row of Params in a ddl-track lab database (research kit; nothing else is touched).
#   pwsh -NoProfile -File params_row.ps1 dump    -Database <db> -Name <row> -Out <file>      the raw stored bytes
#   pwsh -NoProfile -File params_row.ps1 put     -Database <db> -Name <row> -Blob <file>     BinaryData and DataSize from the file
#   pwsh -NoProfile -File params_row.ps1 delete  -Database <db> -Name <row>
#   pwsh -NoProfile -File params_row.ps1 list    -Database <db> [-Name <like pattern>]
# Only databases ibcmd_rs_04_ddl_*; the row must exist for put/delete (PartNo 0).
param(
    [Parameter(Mandatory = $true, Position = 0)][ValidateSet('dump', 'put', 'delete', 'list')][string]$Action,
    [Parameter(Mandatory = $true)][string]$Database,
    [string]$Name = '',
    [string]$Out = '',
    [string]$Blob = ''
)
$ErrorActionPreference = 'Stop'
if ($Database -notmatch '^ibcmd_rs_04_ddl_[a-z0-9_]+$') { throw "lab databases only: ibcmd_rs_04_ddl_* (got $Database)" }
Add-Type -AssemblyName System.Data
$conn = New-Object System.Data.SqlClient.SqlConnection "Server=localhost;Database=$Database;Integrated Security=True;TrustServerCertificate=True;Encrypt=True;Connect Timeout=60;Pooling=False"
$conn.Open()
try {
    $cmd = $conn.CreateCommand()
    $cmd.CommandTimeout = 300
    switch ($Action) {
        'list' {
            $cmd.CommandText = if ($Name) { 'SELECT FileName, PartNo, DataSize FROM dbo.Params WHERE FileName LIKE @n ORDER BY FileName, PartNo' } else { 'SELECT FileName, PartNo, DataSize FROM dbo.Params ORDER BY FileName, PartNo' }
            if ($Name) { [void]$cmd.Parameters.AddWithValue('@n', $Name) }
            $r = $cmd.ExecuteReader()
            while ($r.Read()) { '{0}|{1}|{2}' -f $r.GetString(0), $r.GetValue(1), $r.GetValue(2) }
            $r.Close()
        }
        'dump' {
            if (-not $Name -or -not $Out) { throw 'dump needs -Name and -Out' }
            $cmd.CommandText = 'SELECT BinaryData FROM dbo.Params WHERE FileName = @n AND PartNo = 0'
            [void]$cmd.Parameters.AddWithValue('@n', $Name)
            $bytes = [byte[]]$cmd.ExecuteScalar()
            [IO.File]::WriteAllBytes($Out, $bytes)
            "dumped {0} bytes of {1}" -f $bytes.Length, $Name
        }
        'put' {
            if (-not $Name -or -not $Blob) { throw 'put needs -Name and -Blob' }
            $bytes = [IO.File]::ReadAllBytes($Blob)
            $cmd.CommandText = 'UPDATE dbo.Params SET BinaryData = @b, DataSize = @s WHERE FileName = @n AND PartNo = 0'
            [void]$cmd.Parameters.AddWithValue('@n', $Name)
            [void]$cmd.Parameters.AddWithValue('@s', [int]$bytes.Length)
            $p = $cmd.Parameters.Add('@b', [System.Data.SqlDbType]::VarBinary, -1); $p.Value = $bytes
            $rows = $cmd.ExecuteNonQuery()
            if ($rows -eq 0) {
                # the row was deleted before: insert it (timestamps as the platform stores them, year + 2000)
                $ins = $conn.CreateCommand()
                $ins.CommandText = 'INSERT INTO dbo.Params (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo) VALUES (@n, DATEADD(YEAR, 2000, SYSDATETIME()), DATEADD(YEAR, 2000, SYSDATETIME()), 0, @s, @b, 0)'
                [void]$ins.Parameters.AddWithValue('@n', $Name)
                [void]$ins.Parameters.AddWithValue('@s', [int]$bytes.Length)
                $q = $ins.Parameters.Add('@b', [System.Data.SqlDbType]::VarBinary, -1); $q.Value = $bytes
                [void]$ins.ExecuteNonQuery()
                "inserted {0} bytes as {1}" -f $bytes.Length, $Name
            } elseif ($rows -eq 1) {
                "put {0} bytes into {1}" -f $bytes.Length, $Name
            } else { throw "expected one row, updated $rows" }
        }
        'delete' {
            if (-not $Name) { throw 'delete needs -Name' }
            $cmd.CommandText = 'DELETE FROM dbo.Params WHERE FileName = @n'
            [void]$cmd.Parameters.AddWithValue('@n', $Name)
            $rows = $cmd.ExecuteNonQuery()
            "deleted $rows row(s) of $Name"
        }
    }
} finally { $conn.Close() }
