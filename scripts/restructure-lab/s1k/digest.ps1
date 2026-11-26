# The digest of a lab database that checks 11 and 12 compare before and after (dot-source this file):
# rows of Config / ConfigSave / Params, the hash and status of the schema storage, the hash of DBSchema and DBNames,
# a checksum of every column and every index of every table, the number of *NG tables (the digest of
# ../inject_failure.ps1 of the ddl track, which only takes its own databases).
function Get-LabDigest {
    param([Parameter(Mandatory = $true)][string]$Database)
    if ($Database -notmatch '^ibcmd_rs_05_ext_s1k_[a-z0-9_]+$') { throw "S1-K lab databases only (got $Database)" }
    $sql = @"
SET NOCOUNT ON;
SELECT 'Config rows', COUNT_BIG(*) FROM dbo.Config
UNION ALL SELECT 'ConfigSave rows', COUNT_BIG(*) FROM dbo.ConfigSave
UNION ALL SELECT 'Params rows', COUNT_BIG(*) FROM dbo.Params
UNION ALL SELECT 'Config checksum', CHECKSUM_AGG(CHECKSUM(FileName, PartNo, DataSize, Creation, Modified)) FROM dbo.Config
UNION ALL SELECT 'ConfigSave checksum', CHECKSUM_AGG(CHECKSUM(FileName, PartNo, DataSize)) FROM dbo.ConfigSave
UNION ALL SELECT 'Params checksum', CHECKSUM_AGG(CHECKSUM(FileName, PartNo, DataSize, Modified)) FROM dbo.Params
UNION ALL SELECT 'SchemaStorage 0 status', Status FROM dbo.SchemaStorage WHERE SchemaID = 0
UNION ALL SELECT 'SchemaStorage 0 schema', CHECKSUM(CONVERT(varchar(64), HASHBYTES('SHA2_256', CurrentSchema), 2)) FROM dbo.SchemaStorage WHERE SchemaID = 0
UNION ALL SELECT 'DBSchema', CHECKSUM(CONVERT(varchar(64), HASHBYTES('SHA2_256', SerializedData), 2)) FROM dbo.DBSchema
UNION ALL SELECT 'DBNames', CHECKSUM(CONVERT(varchar(64), HASHBYTES('SHA2_256', BinaryData), 2)) FROM dbo.Params WHERE FileName = N'DBNames' AND PartNo = 0
UNION ALL SELECT 'columns', CHECKSUM_AGG(CHECKSUM(t.name, c.name, c.max_length, c.is_nullable, c.column_id)) FROM sys.tables t JOIN sys.columns c ON c.object_id = t.object_id
UNION ALL SELECT 'indexes', CHECKSUM_AGG(CHECKSUM(t.name, i.name, i.type, i.is_unique)) FROM sys.tables t JOIN sys.indexes i ON i.object_id = t.object_id
UNION ALL SELECT 'tables *NG', COUNT(*) FROM sys.tables WHERE name LIKE N'%NG'
UNION ALL SELECT 'tables', COUNT(*) FROM sys.tables;
"@
    sqlcmd -S localhost -E -C -b -d $Database -h -1 -W -s '|' -Q $sql
}

function Test-LabDatabase {
    param([Parameter(Mandatory = $true)][string]$Database)
    (sqlcmd -S localhost -E -C -h -1 -W -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM sys.databases WHERE name = N'$Database'").Trim() -ne '0'
}
