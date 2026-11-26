<#
.SYNOPSIS
  A bounded, read-only snapshot of the state of one 1C infobase in SQL Server:
  every table (row count, content checksum), the schema (columns, indexes,
  constraints, other objects, database settings) and, for the service tables,
  one line per row.

.DESCRIPTION
  Output directory layout (all UTF-8, TAB separated, \ \t \r \n escaped):
    meta.json          database, server, time, options, counters
    tables.tsv         schema, table, class (service|data), rows, used_kb, checksum_agg, checksum_sum, note
    columns.tsv        every column: type, nullability, identity, computed, default, collation
    indexes.tsv        every index: type, keys, included columns, filter, compression
    constraints.tsv    check constraints and foreign keys
    objects.tsv        views, procedures, functions, triggers, sequences, synonyms, user types (definition sha256)
    dbsettings.tsv     database options, scoped configurations, principals, files
    rows/<Table>.tsv   one line per row of a service table
                         file tables (Config, ConfigSave, ConfigCAS, ConfigCASSave, Params, Files, DepotFiles):
                           FileName, parts, size, stored bytes, sha256 of the stored bytes, creation/modified as
                           stored (the platform adds a year offset), enc (deflate|raw), decoded size and sha256,
                           kind (v8text|text|container|binary|empty), blob (Y = content kept), preview
                         other service tables (DBSchema, SchemaStorage, IBVersion, v8users, _ConfigChngR, ...):
                           key (primary key values), row sha256, a compact summary of the columns
  Row content that is worth keeping goes to a content-addressed blob store
  (<store>/<sha[0..1]>/<sha>.gz, gzip of the stored bytes) shared by every
  snapshot that uses the same -BlobStore.

  Service tables are the tables with a file layout (FileName + PartNo +
  BinaryData) and every table whose name has no digit (Config, Params,
  DBSchema, IBVersion, v8users, _SystemSettings, _ConfigChngR, ...); object
  tables (_Reference123, _InfoRg45, ...) are data tables: row count and checksum only.

.PARAMETER Reference
  A previous snapshot: file rows that differ from it (or are new) keep their
  content in the blob store, so a later diff can show what changed even when
  the row is gone or overwritten.

.EXAMPLE
  .\snapshot.ps1 -Database ibcmd_rs_04_trace_x -Out F:\lab\snap\before
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Database,
    [Parameter(Mandatory = $true)][string]$Out,
    [string]$Server = 'localhost',
    # content-addressed store of row bytes; default: <parent of -Out>\blobs
    [string]$BlobStore = '',
    # rows up to this stored size may keep their content
    [int]$ContentMaxKB = 512,
    # stop keeping new content after this many MB written to the store
    [int]$ContentBudgetMB = 1024,
    # rows up to this stored size are inflated for the decoded size/sha256/kind
    [int]$DecodeMaxMB = 32,
    # full: COUNT + checksums of every table; counts: metadata row counts only; none: like counts
    [ValidateSet('full', 'counts', 'none')][string]$DataChecksum = 'full',
    # tables larger than this (MB used) get no checksum scan
    [int]$ChecksumMaxMB = 4096,
    # service tables with more rows than this are not dumped row by row
    [int]$RowDumpMaxRows = 300000,
    [string]$Reference = '',
    [string]$Label = ''
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
. (Join-Path $PSScriptRoot 'lib\common.ps1')

$sw = [Diagnostics.Stopwatch]::StartNew()
New-Item -ItemType Directory -Force -Path $Out, (Join-Path $Out 'rows') | Out-Null
$Out = (Resolve-Path -LiteralPath $Out).Path
if (-not $BlobStore) { $BlobStore = Join-Path (Split-Path -Parent $Out) 'blobs' }
New-Item -ItemType Directory -Force -Path $BlobStore | Out-Null
$BlobStore = (Resolve-Path -LiteralPath $BlobStore).Path
$conn = Open-Sql -Server $Server -Database $Database -App 'ibcmd-rs apply-trace snapshot'
$pack = New-Object ApplyTraceKit.BlobPack($BlobStore)
try {
    Invoke-SqlNonQuery $conn 'SET LOCK_TIMEOUT 120000'
    $dbInfo = Invoke-SqlRows $conn 'SELECT DB_ID() AS id, DB_NAME() AS name, @@VERSION AS ver, CAST(SERVERPROPERTY(''Collation'') AS nvarchar(128)) AS srv_collation'
    if ($dbInfo[0]['name'] -ne $Database) { throw "connected to '$($dbInfo[0]['name'])', expected '$Database'" }
    Write-Log ("snapshot of {0} (id {1}) -> {2}" -f $Database, $dbInfo[0]['id'], $Out)

    # ------------------------------------------------------------------ helpers
    # Streams a query into a TSV file; $Columns are the result columns to write.
    function Export-Query {
        param([string]$Sql, [string]$Path, [string[]]$Columns)
        $cmd = $conn.CreateCommand()
        $cmd.CommandText = $Sql
        $cmd.CommandTimeout = 0
        $reader = $cmd.ExecuteReader()
        $w = New-Utf8Writer $Path
        $n = 0
        try {
            Write-TsvLine $w $Columns
            $vals = New-Object object[] $reader.FieldCount
            while ($reader.Read()) {
                [void]$reader.GetValues($vals)
                $f = New-Object object[] $vals.Length
                for ($i = 0; $i -lt $vals.Length; $i++) {
                    $v = $vals[$i]
                    if ($v -is [System.DBNull]) { $f[$i] = '' }
                    elseif ($v -is [byte[]]) { $f[$i] = Get-HexString $v }
                    elseif ($v -is [bool]) { $f[$i] = if ($v) { '1' } else { '0' } }
                    elseif ($v -is [DateTime]) { $f[$i] = $v.ToString('yyyy-MM-ddTHH:mm:ss.fffffff') }
                    else { $f[$i] = $v }
                }
                Write-TsvLine $w $f
                $n++
            }
        } finally { $reader.Close(); $w.Dispose() }
        return $n
    }

    # ------------------------------------------------------- 1. table inventory
    $inventorySql = @'
SELECT s.name AS sch, t.name AS tbl,
       CAST(ISNULL(ps.rows_meta, 0) AS bigint) AS rows_meta,
       CAST(ISNULL(ps.used_kb, 0) AS bigint) AS used_kb
FROM sys.tables t
JOIN sys.schemas s ON s.schema_id = t.schema_id
LEFT JOIN (SELECT object_id,
                  SUM(CASE WHEN index_id IN (0, 1) THEN row_count ELSE 0 END) AS rows_meta,
                  SUM(used_page_count) * 8 AS used_kb
           FROM sys.dm_db_partition_stats GROUP BY object_id) ps ON ps.object_id = t.object_id
ORDER BY s.name, t.name
'@
    $inventory = Invoke-SqlRows $conn $inventorySql
    Write-Log "$($inventory.Count) tables"

    # File tables: FileName + PartNo + BinaryData.
    $fileTableRows = Invoke-SqlRows $conn @'
SELECT s.name AS sch, t.name AS tbl FROM sys.tables t JOIN sys.schemas s ON s.schema_id = t.schema_id
WHERE EXISTS (SELECT 1 FROM sys.columns c WHERE c.object_id = t.object_id AND c.name = N'FileName')
  AND EXISTS (SELECT 1 FROM sys.columns c WHERE c.object_id = t.object_id AND c.name = N'PartNo')
  AND EXISTS (SELECT 1 FROM sys.columns c WHERE c.object_id = t.object_id AND c.name = N'BinaryData')
'@
    $fileTables = @(); foreach ($r in $fileTableRows) { $fileTables += $r['tbl'] }
    $fileSet = @{}; foreach ($t in $fileTables) { $fileSet[$t] = $true }

    function Get-TableClass([string]$tbl) {
        if ($fileSet.ContainsKey($tbl)) { return 'service' }
        if ($tbl -notmatch '\d') { return 'service' }
        return 'data'
    }

    # -------------------------------------------- 2. row counts and checksums
    $ck = @{}
    if ($DataChecksum -eq 'full') {
        $sqlCk = @'
SET NOCOUNT ON;
CREATE TABLE #ck (sch sysname, tbl sysname, cnt bigint NULL, ck int NULL, cks bigint NULL, err nvarchar(400) NULL);
DECLARE @sch sysname, @tbl sysname, @sql nvarchar(max);
DECLARE c CURSOR LOCAL FAST_FORWARD FOR
    SELECT s.name, t.name FROM sys.tables t JOIN sys.schemas s ON s.schema_id = t.schema_id
    LEFT JOIN (SELECT object_id, SUM(used_page_count) AS pages FROM sys.dm_db_partition_stats GROUP BY object_id) ps ON ps.object_id = t.object_id
    WHERE ISNULL(ps.pages, 0) * 8 <= @maxkb ORDER BY s.name, t.name;
OPEN c; FETCH NEXT FROM c INTO @sch, @tbl;
WHILE @@FETCH_STATUS = 0
BEGIN
    SET @sql = N'INSERT #ck (sch, tbl, cnt, ck, cks) SELECT @s, @t, COUNT_BIG(*), CHECKSUM_AGG(BINARY_CHECKSUM(*)), SUM(CAST(BINARY_CHECKSUM(*) AS bigint)) FROM ' + QUOTENAME(@sch) + N'.' + QUOTENAME(@tbl);
    BEGIN TRY EXEC sp_executesql @sql, N'@s sysname, @t sysname', @sch, @tbl; END TRY
    BEGIN CATCH INSERT #ck (sch, tbl, err) VALUES (@sch, @tbl, LEFT(ERROR_MESSAGE(), 400)); END CATCH
    FETCH NEXT FROM c INTO @sch, @tbl;
END
CLOSE c; DEALLOCATE c;
SELECT sch, tbl, cnt, ck, cks, err FROM #ck;
'@
        Write-Log 'row counts and checksums of every table (a full scan)'
        $cmd = $conn.CreateCommand(); $cmd.CommandText = $sqlCk; $cmd.CommandTimeout = 0
        [void]$cmd.Parameters.AddWithValue('@maxkb', [int64]$ChecksumMaxMB * 1024)
        $r = $cmd.ExecuteReader()
        while ($r.Read()) {
            $ck[$r.GetString(0) + '.' + $r.GetString(1)] = @{
                cnt = if ($r.IsDBNull(2)) { $null } else { $r.GetInt64(2) }
                ck  = if ($r.IsDBNull(3)) { '' } else { [string]$r.GetInt32(3) }
                cks = if ($r.IsDBNull(4)) { '' } else { [string]$r.GetInt64(4) }
                err = if ($r.IsDBNull(5)) { '' } else { $r.GetString(5) }
            }
        }
        $r.Close()
    }
    $tablesPath = Join-Path $Out 'tables.tsv'
    $w = New-Utf8Writer $tablesPath
    $rowsByTable = @{}
    try {
        Write-TsvLine $w @('schema', 'table', 'class', 'rows', 'used_kb', 'checksum_agg', 'checksum_sum', 'note')
        foreach ($t in $inventory) {
            $key = $t['sch'] + '.' + $t['tbl']
            $cls = Get-TableClass $t['tbl']
            $rows = $t['rows_meta']; $a = ''; $b = ''; $note = ''
            if ($ck.ContainsKey($key)) {
                $c = $ck[$key]
                if ($null -ne $c['cnt']) { $rows = $c['cnt'] }
                $a = $c['ck']; $b = $c['cks']; $note = $c['err']
            } elseif ($DataChecksum -eq 'full') { $note = 'no checksum: larger than -ChecksumMaxMB' }
            $rowsByTable[$t['tbl']] = $rows
            Write-TsvLine $w @($t['sch'], $t['tbl'], $cls, $rows, $t['used_kb'], $a, $b, $note)
        }
    } finally { $w.Dispose() }

    # ------------------------------------------------------------- 3. schema
    Write-Log 'schema: columns, indexes, constraints, objects, settings'
    $nCols = Export-Query -Path (Join-Path $Out 'columns.tsv') -Columns @('schema', 'table', 'ord', 'column', 'type', 'nullable', 'identity', 'computed', 'computed_def', 'default_def', 'collation') -Sql @'
SELECT s.name AS sch, t.name AS tbl, c.column_id AS ord, c.name AS col,
  CASE WHEN ty.name IN ('nvarchar', 'nchar') THEN ty.name + '(' + CASE WHEN c.max_length = -1 THEN 'max' ELSE CAST(c.max_length / 2 AS varchar(10)) END + ')'
       WHEN ty.name IN ('varchar', 'char', 'varbinary', 'binary') THEN ty.name + '(' + CASE WHEN c.max_length = -1 THEN 'max' ELSE CAST(c.max_length AS varchar(10)) END + ')'
       WHEN ty.name IN ('decimal', 'numeric') THEN ty.name + '(' + CAST(c.precision AS varchar(5)) + ',' + CAST(c.scale AS varchar(5)) + ')'
       WHEN ty.name IN ('datetime2', 'time', 'datetimeoffset') THEN ty.name + '(' + CAST(c.scale AS varchar(5)) + ')'
       ELSE ty.name END AS typ,
  c.is_nullable, c.is_identity, c.is_computed,
  ISNULL(cc.definition, '') AS computed_def, ISNULL(dc.definition, '') AS default_def, ISNULL(c.collation_name, '') AS coll
FROM sys.tables t
JOIN sys.schemas s ON s.schema_id = t.schema_id
JOIN sys.columns c ON c.object_id = t.object_id
JOIN sys.types ty ON ty.user_type_id = c.user_type_id
LEFT JOIN sys.computed_columns cc ON cc.object_id = c.object_id AND cc.column_id = c.column_id
LEFT JOIN sys.default_constraints dc ON dc.parent_object_id = c.object_id AND dc.parent_column_id = c.column_id
ORDER BY s.name, t.name, c.column_id
'@
    $nIdx = Export-Query -Path (Join-Path $Out 'indexes.tsv') -Columns @('schema', 'table', 'index', 'type', 'unique', 'primary_key', 'unique_constraint', 'disabled', 'fill_factor', 'filter', 'keys', 'included', 'compression') -Sql @'
SELECT s.name AS sch, t.name AS tbl, ISNULL(i.name, N'(heap)') AS idx, i.type_desc AS typ, i.is_unique, i.is_primary_key, i.is_unique_constraint,
  i.is_disabled, i.fill_factor, ISNULL(i.filter_definition, N'') AS filt,
  ISNULL(STUFF((SELECT N',' + c.name + CASE WHEN ic.is_descending_key = 1 THEN N' DESC' ELSE N'' END
                FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id
                WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.is_included_column = 0 ORDER BY ic.key_ordinal
                FOR XML PATH(N''), TYPE).value('.', 'nvarchar(max)'), 1, 1, N''), N'') AS keys,
  ISNULL(STUFF((SELECT N',' + c.name
                FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id
                WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.is_included_column = 1 ORDER BY ic.index_column_id
                FOR XML PATH(N''), TYPE).value('.', 'nvarchar(max)'), 1, 1, N''), N'') AS incl,
  ISNULL((SELECT TOP 1 p.data_compression_desc FROM sys.partitions p WHERE p.object_id = i.object_id AND p.index_id = i.index_id), N'') AS compression
FROM sys.tables t
JOIN sys.schemas s ON s.schema_id = t.schema_id
JOIN sys.indexes i ON i.object_id = t.object_id
ORDER BY s.name, t.name, i.index_id
'@
    $nCon = Export-Query -Path (Join-Path $Out 'constraints.tsv') -Columns @('kind', 'schema', 'table', 'name', 'definition') -Sql @'
SELECT N'CHECK' AS kind, s.name AS sch, t.name AS tbl, cc.name, cc.definition
FROM sys.check_constraints cc JOIN sys.tables t ON t.object_id = cc.parent_object_id JOIN sys.schemas s ON s.schema_id = t.schema_id
UNION ALL
SELECT N'FOREIGN KEY', s.name, t.name, fk.name,
  N'-> ' + rs.name + N'.' + rt.name + N' (' + ISNULL(STUFF((SELECT N',' + pc.name + N'=' + rc.name
     FROM sys.foreign_key_columns fkc JOIN sys.columns pc ON pc.object_id = fkc.parent_object_id AND pc.column_id = fkc.parent_column_id
     JOIN sys.columns rc ON rc.object_id = fkc.referenced_object_id AND rc.column_id = fkc.referenced_column_id
     WHERE fkc.constraint_object_id = fk.object_id ORDER BY fkc.constraint_column_id FOR XML PATH(N''), TYPE).value('.', 'nvarchar(max)'), 1, 1, N''), N'') + N')'
FROM sys.foreign_keys fk JOIN sys.tables t ON t.object_id = fk.parent_object_id JOIN sys.schemas s ON s.schema_id = t.schema_id
JOIN sys.tables rt ON rt.object_id = fk.referenced_object_id JOIN sys.schemas rs ON rs.schema_id = rt.schema_id
ORDER BY 1, 2, 3, 4
'@
    $nObj = Export-Query -Path (Join-Path $Out 'objects.tsv') -Columns @('kind', 'schema', 'name', 'definition_sha256') -Sql @'
SELECT o.type_desc AS kind, s.name AS sch, o.name, ISNULL(CONVERT(varchar(64), HASHBYTES('SHA2_256', m.definition), 2), N'') AS def_sha
FROM sys.objects o JOIN sys.schemas s ON s.schema_id = o.schema_id LEFT JOIN sys.sql_modules m ON m.object_id = o.object_id
WHERE o.is_ms_shipped = 0 AND o.type NOT IN ('U', 'PK', 'D', 'C', 'F', 'UQ', 'IT', 'S', 'SQ', 'TT')
UNION ALL
SELECT N'USER TYPE', s.name, ty.name, N'' FROM sys.types ty JOIN sys.schemas s ON s.schema_id = ty.schema_id WHERE ty.is_user_defined = 1
ORDER BY 1, 2, 3
'@
    # Database options, scoped configurations, principals, schemas and files (no sizes).
    $w = New-Utf8Writer (Join-Path $Out 'dbsettings.tsv')
    try {
        Write-TsvLine $w @('key', 'value')
        $skip = @('database_id', 'create_date', 'owner_sid', 'log_reuse_wait', 'log_reuse_wait_desc', 'source_database_id', 'family_guid', 'group_database_id', 'resource_pool_id', 'default_language_lcid', 'physical_database_name', 'replica_id')
        $db = Invoke-SqlRows $conn 'SELECT * FROM sys.databases WHERE database_id = DB_ID()'
        foreach ($k in $db[0].Keys) { if ($skip -notcontains $k) { Write-TsvLine $w @("option:$k", $db[0][$k]) } }
        foreach ($r in (Invoke-SqlRows $conn 'SELECT name, CAST(value AS nvarchar(200)) AS v FROM sys.database_scoped_configurations ORDER BY name')) { Write-TsvLine $w @("scoped:$($r['name'])", $r['v']) }
        foreach ($r in (Invoke-SqlRows $conn 'SELECT name, type_desc FROM sys.database_principals WHERE is_fixed_role = 0 ORDER BY name')) { Write-TsvLine $w @("principal:$($r['name'])", $r['type_desc']) }
        foreach ($r in (Invoke-SqlRows $conn 'SELECT name FROM sys.schemas ORDER BY name')) { Write-TsvLine $w @("schema:$($r['name'])", '') }
        foreach ($r in (Invoke-SqlRows $conn 'SELECT name, type_desc, growth, is_percent_growth, max_size FROM sys.database_files ORDER BY file_id')) { Write-TsvLine $w @("file:$($r['name'])", "$($r['type_desc']) growth=$($r['growth'])$(if ($r['is_percent_growth']) { '%' } else { ' pages' }) max_size=$($r['max_size'])") }
    } finally { $w.Dispose() }

    $yearOffset = $null
    if ($inventory | Where-Object { $_['tbl'] -eq '_YearOffset' }) { $yearOffset = Invoke-SqlScalar $conn 'SELECT TOP 1 [Offset] FROM dbo._YearOffset' }

    # ---------------------------------------------- 4. reference for content
    $refRows = @{}
    if ($Reference) {
        foreach ($t in $fileTables) {
            $p = Join-Path $Reference "rows\$t.tsv"
            if (Test-Path -LiteralPath $p) {
                $h = @{}
                Get-Content -LiteralPath $p -Encoding UTF8 | Select-Object -Skip 1 | ForEach-Object {
                    $f = $_ -split "`t"
                    if ($f.Count -ge 5) { $h[$f[0]] = $f[4] }
                }
                $refRows[$t] = $h
            }
        }
        Write-Log "reference $Reference loaded"
    }

    # --------------------------------------------- 5. file tables, row by row
    $script:budget = [int64]$ContentBudgetMB * 1MB
    $contentMax = [int64]$ContentMaxKB * 1KB
    $decodeMax = [int64]$DecodeMaxMB * 1MB
    $saveNames = @{}   # table -> names present in the staging table of the same snapshot
    $stats = [ordered]@{}
    $guidStart = '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}'

    function Test-KeepContent {
        param([string]$Table, [string]$Name, [string]$Sha, [int64]$Stored)
        if ($Stored -gt $contentMax) { return $false }
        if ($pack.AddedBytes -ge $script:budget) { return $false }
        if ($Table -in @('Params', 'Files', 'ConfigSave', 'ConfigCASSave', 'DepotFiles')) { return $true }
        # Config / ConfigCAS: rows about to be replaced (present in the staging table), service names,
        # transient names and rows that differ from the reference.
        $stage = if ($Table -eq 'Config') { 'ConfigSave' } elseif ($Table -eq 'ConfigCAS') { 'ConfigCASSave' } else { $null }
        if ($stage -and $saveNames.ContainsKey($stage) -and $saveNames[$stage].ContainsKey($Name)) { return $true }
        # ConfigCAS is content-addressed: only new or changed rows are worth keeping.
        if ($Table -eq 'Config') {
            if ($Name -notmatch $guidStart) { return $true }
            if ($Name -match 'dynupdate|\.new$|NEW$') { return $true }
        }
        if ($refRows.ContainsKey($Table)) {
            if (-not $refRows[$Table].ContainsKey($Name) -or $refRows[$Table][$Name] -ne $Sha) { return $true }
        }
        return $false
    }

    function Write-FileRow {
        param($Writer, [string]$Table, [string]$Name, [int]$Parts, [int64]$DataSize, [int]$Attr, $Created, $Modified, [byte[]]$Stored, [string]$Gaps)
        $sha = Get-Sha256Hex $Stored
        $enc = 'raw'; $dsize = $Stored.Length; $dsha = $sha; $kind = ''; $preview = ''
        if ($Stored.Length -le $decodeMax) {
            $dec = Expand-RawDeflate -Bytes $Stored
            if ($null -ne $dec -and $dec.Length -gt 0) {
                $enc = 'deflate'; $dsize = $dec.Length; $dsha = Get-Sha256Hex $dec
                $kind = Get-ContentKind $dec
                if ($kind -in @('v8text', 'text') -and $dec.Length -le 8192) { $preview = Get-TextPreview $dec }
            } else {
                $kind = Get-ContentKind $Stored
                if ($kind -in @('v8text', 'text') -and $Stored.Length -le 8192) { $preview = Get-TextPreview $Stored }
            }
        }
        $blob = 'N'
        if (Test-KeepContent -Table $Table -Name $Name -Sha $sha -Stored $Stored.Length) {
            [void]$pack.Add($sha, $Stored)
            $blob = 'Y'
        }
        Write-TsvLine $Writer @($Name, $Parts, $DataSize, $Stored.Length, $sha, $Attr, $Created.ToString('yyyy-MM-ddTHH:mm:ss.fffffff'), $Modified.ToString('yyyy-MM-ddTHH:mm:ss.fffffff'), $enc, $dsize, $dsha, $kind, $blob, $preview, $Gaps)
    }

    # Staging tables first: their names decide which Config/ConfigCAS rows keep their content.
    $orderedFileTables = @($fileTables | Sort-Object { switch ($_) { 'ConfigSave' { 0 } 'ConfigCASSave' { 0 } default { 1 } } }, { $_ })
    foreach ($t in $orderedFileTables) {
        $sch = ($inventory | Where-Object { $_['tbl'] -eq $t } | Select-Object -First 1)['sch']
        $count = if ($rowsByTable.ContainsKey($t)) { $rowsByTable[$t] } else { 0 }
        $path = Join-Path $Out "rows\$t.tsv"
        $w = New-Utf8Writer $path
        $names = @{}
        $nFiles = 0
        try {
            Write-TsvLine $w @('FileName', 'parts', 'size', 'stored', 'sha256', 'attributes', 'creation', 'modified', 'enc', 'decoded_size', 'decoded_sha256', 'kind', 'blob', 'preview', 'gaps')
            $cmd = $conn.CreateCommand()
            $cmd.CommandTimeout = 0
            $cmd.CommandText = "SELECT FileName, PartNo, Creation, Modified, Attributes, DataSize, BinaryData FROM [$sch].[$t] ORDER BY FileName, PartNo"
            $reader = $cmd.ExecuteReader()
            $curName = $null; $ms = $null; $parts = 0; $first = $null; $expectPart = 0; $gaps = ''
            try {
                while ($reader.Read()) {
                    $name = $reader.GetString(0)
                    if ($name -ne $curName) {
                        if ($null -ne $curName) {
                            Write-FileRow $w $t $curName $parts ([int64]$first.size) ([int]$first.attr) $first.created $first.modified $ms.ToArray() $gaps
                            $names[$curName] = $true; $nFiles++
                        }
                        $curName = $name; $ms = New-Object System.IO.MemoryStream; $parts = 0; $expectPart = 0; $gaps = ''
                        $first = @{ size = $reader.GetValue(5); attr = $reader.GetValue(4); created = $reader.GetDateTime(2); modified = $reader.GetDateTime(3) }
                    }
                    $partNo = $reader.GetInt32(1)
                    if ($partNo -ne $expectPart) { $gaps = "part $partNo after $($expectPart - 1)" }
                    $expectPart = $partNo + 1
                    $b = $reader.GetValue(6)
                    if ($b -isnot [System.DBNull]) { $bytes = [byte[]]$b; $ms.Write($bytes, 0, $bytes.Length) }
                    $parts++
                }
                if ($null -ne $curName) {
                    Write-FileRow $w $t $curName $parts ([int64]$first.size) ([int]$first.attr) $first.created $first.modified $ms.ToArray() $gaps
                    $names[$curName] = $true; $nFiles++
                }
            } finally { $reader.Close() }
        } finally { $w.Dispose() }
        if ($t -in @('ConfigSave', 'ConfigCASSave')) { $saveNames[$t] = $names }
        $stats[$t] = $nFiles
        Write-Log ("{0}: {1} files" -f $t, $nFiles)
    }

    # --------------------------- 6. the other service tables, generic dump
    function Get-CanonValue($v) {
        if ($v -is [System.DBNull]) { return 'NULL' }
        if ($v -is [byte[]]) { return $null }
        if ($v -is [DateTime]) { return $v.ToString('yyyy-MM-ddTHH:mm:ss.fffffff') }
        if ($v -is [decimal] -or $v -is [double] -or $v -is [single]) { return [string]::Format([Globalization.CultureInfo]::InvariantCulture, '{0}', $v) }
        if ($v -is [bool]) { return $(if ($v) { '1' } else { '0' }) }
        return [string]$v
    }
    $lobLimitKB = @{ DBSchema = 16384; SchemaStorage = 16384 }
    foreach ($inv in $inventory) {
        $t = $inv['tbl']; $sch = $inv['sch']
        if ((Get-TableClass $t) -ne 'service' -or $fileSet.ContainsKey($t)) { continue }
        $count = [int64]$rowsByTable[$t]
        $path = Join-Path $Out "rows\$t.tsv"
        if ($count -gt $RowDumpMaxRows) {
            $stats[$t] = "skipped ($count rows)"
            Write-Log "$t : $count rows, row dump skipped (-RowDumpMaxRows)"
            continue
        }
        $pkRows = Invoke-SqlRows $conn @'
SELECT c.name AS col FROM sys.indexes i JOIN sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id
JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id
WHERE i.object_id = OBJECT_ID(@o) AND i.is_primary_key = 1 ORDER BY ic.key_ordinal
'@ @{ '@o' = "[$sch].[$t]" }
        $pk = @(); foreach ($r in $pkRows) { $pk += $r['col'] }
        $lobMax = if ($lobLimitKB.ContainsKey($t)) { [int64]$lobLimitKB[$t] * 1KB } else { $contentMax }
        $w = New-Utf8Writer $path
        $n = 0
        try {
            Write-TsvLine $w @('key', 'row_sha256', 'summary')
            $cmd = $conn.CreateCommand(); $cmd.CommandTimeout = 0
            $orderBy = if ($pk.Count -gt 0) { ' ORDER BY ' + (($pk | ForEach-Object { "[$_]" }) -join ', ') } else { '' }
            $cmd.CommandText = "SELECT * FROM [$sch].[$t]$orderBy"
            $reader = $cmd.ExecuteReader()
            try {
                $names = @(0..($reader.FieldCount - 1) | ForEach-Object { $reader.GetName($_) })
                $vals = New-Object object[] $reader.FieldCount
                $shaAlg = [System.Security.Cryptography.SHA256]::Create()
                while ($reader.Read()) {
                    [void]$reader.GetValues($vals)
                    $canon = New-Object System.Text.StringBuilder
                    $summary = New-Object System.Text.StringBuilder
                    $keyParts = @()
                    for ($i = 0; $i -lt $vals.Length; $i++) {
                        $v = $vals[$i]
                        $cn = $names[$i]
                        if ($v -is [byte[]]) {
                            $bsha = Get-Sha256Hex $v
                            [void]$canon.Append("$cn=bin:$($v.Length):$bsha|")
                            if ($v.Length -le 32) { $shown = Get-HexString $v; $isKey = $true } else { $shown = "len:$($v.Length) sha:$($bsha.Substring(0, 12))"; $isKey = $false }
                            if ($v.Length -gt 32 -and $v.Length -le $lobMax) {
                                [void]$pack.Add($bsha, $v)
                                $shown += ' blob'
                            }
                            [void]$summary.Append("$cn=$shown; ")
                            if ($pk -contains $cn) { $keyParts += (Get-HexString $v) }
                        } else {
                            $s = Get-CanonValue $v
                            [void]$canon.Append("$cn=$s|")
                            if ($s.Length -gt 400) {
                                $sb = $script:Utf8NoBom.GetBytes($s)
                                $ssha = Get-Sha256Hex $sb
                                $short = "len:$($s.Length) sha:$($ssha.Substring(0, 12))"
                                if ($sb.Length -le $lobMax) {
                                    [void]$pack.Add($ssha, $sb)
                                    $short += ' blob'
                                }
                                [void]$summary.Append("$cn=$short; ")
                            } else { [void]$summary.Append("$cn=$s; ") }
                            if ($pk -contains $cn) { $keyParts += $s }
                        }
                    }
                    $rowSha = (Get-HexString $shaAlg.ComputeHash($script:Utf8NoBom.GetBytes($canon.ToString()))).ToLowerInvariant()
                    $key = if ($pk.Count -gt 0) { $keyParts -join '|' } else { 'row:' + $rowSha.Substring(0, 16) }
                    $sm = $summary.ToString()
                    if ($sm.Length -gt 900) { $sm = $sm.Substring(0, 900) + '...' }
                    Write-TsvLine $w @($key, $rowSha, $sm)
                    $n++
                }
            } finally { $reader.Close() }
        } finally { $w.Dispose() }
        $stats[$t] = $n
    }

    # ------------------------------------------------------------ 7. meta
    $meta = [ordered]@{
        kit_version   = $script:KitVersion
        label         = $Label
        database      = $Database
        database_id   = $dbInfo[0]['id']
        server        = $Server
        sql_version   = ($dbInfo[0]['ver'] -split "`n")[0].Trim()
        taken_utc     = [DateTime]::UtcNow.ToString('o')
        taken_local   = (Get-Date).ToString('o')
        year_offset   = $yearOffset
        options       = [ordered]@{ content_max_kb = $ContentMaxKB; content_budget_mb = $ContentBudgetMB; decode_max_mb = $DecodeMaxMB; data_checksum = $DataChecksum; checksum_max_mb = $ChecksumMaxMB; row_dump_max_rows = $RowDumpMaxRows; reference = $Reference }
        blob_store    = $BlobStore
        tables        = $inventory.Count
        columns       = $nCols
        indexes       = $nIdx
        constraints   = $nCon
        other_objects = $nObj
        service_tables = @($stats.Keys)
        service_rows  = $stats
        blobs_written = $pack.AddedCount
        blob_bytes_written = $pack.AddedBytes
        seconds       = [Math]::Round($sw.Elapsed.TotalSeconds, 1)
    }
    [System.IO.File]::WriteAllText((Join-Path $Out 'meta.json'), ($meta | ConvertTo-Json -Depth 6), $script:Utf8NoBom)
    Write-Log ("snapshot done in {0:N1} s: {1} tables, {2} service tables, {3} new blobs ({4:N1} MB)" -f $sw.Elapsed.TotalSeconds, $inventory.Count, $stats.Count, $pack.AddedCount, ($pack.AddedBytes / 1MB))
} finally {
    $pack.Dispose()
    $conn.Close()
}
