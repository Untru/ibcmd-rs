# 12.6 check 12: a failure inside the transaction takes everything back. Runs the generated apply script (from
# `mssql-restructure --through-apply --script-output`) on a FRESH twin of the staged state with a THROW injected as the
# last statement before COMMIT, and compares a digest of the database before and after.
#   pwsh -NoProfile -File inject_failure.ps1 -Script <script_real.sql> -From <db the script was made for> -To <fresh twin> [-Out <file>]
# The digest: rows of Config / ConfigSave / Params, the hash and status of the schema storage, the hash of DBSchema and
# DBNames, a checksum of every column and every index of every table, the number of *NG tables.
param(
    [Parameter(Mandatory = $true)][string]$Script,
    [Parameter(Mandatory = $true)][string]$From,
    [Parameter(Mandatory = $true)][string]$To,
    [string]$Out = ''
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
if ($To -notmatch '^ibcmd_rs_04_ddl_[a-z0-9_]+$') { throw "lab databases only (got $To)" }
$digestSql = @"
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
function Get-Digest {
    sqlcmd -S localhost -E -C -b -d $To -h -1 -W -s '|' -Q $digestSql
}
$text = Get-Content -LiteralPath $Script -Raw -Encoding UTF8
if ($text -notmatch [regex]::Escape("USE [$From];")) { throw "the script does not start with USE [$From]" }
$text = $text.Replace("USE [$From];", "USE [$To];")
$commit = "`nCOMMIT TRANSACTION;`nEND TRY"
if (-not $text.Contains($commit) -and -not $text.Contains("`r`nCOMMIT TRANSACTION;`r`nEND TRY")) { throw 'no COMMIT before END TRY' }
$text = $text -replace "(\r?\n)COMMIT TRANSACTION;(\r?\n)END TRY", "`$1THROW 51999, N'injected failure before COMMIT', 1;`$1COMMIT TRANSACTION;`$2END TRY"
$injected = [IO.Path]::ChangeExtension($Script, '.injected.sql')
[IO.File]::WriteAllText($injected, $text, (New-Object Text.UTF8Encoding($true)))
"before:"
$before = Get-Digest
$before
$sw = [Diagnostics.Stopwatch]::StartNew()
$run = sqlcmd -S localhost -E -C -f 65001 -d $To -i $injected 2>&1
# sp_rename warns about every renamed object; the error of the THROW is the line that matters
$said = (($run | ForEach-Object { "$_" }) | Where-Object { $_ -notmatch '^Внимание' -and $_.Trim() }) -join ' | '
$verdict = "the script ran for {0:n1} s and said: {1}" -f $sw.Elapsed.TotalSeconds, $said
$verdict
if ($said -notmatch 'injected failure before COMMIT') { throw 'the injected THROW was not reached' }
"after:"
$after = Get-Digest
$after
$same = (($before -join "`n") -eq ($after -join "`n"))
"digest unchanged: $same"
if ($Out) { @($verdict) + @('before:') + $before + @('after:') + $after + @("digest unchanged: $same") | Set-Content -LiteralPath $Out -Encoding UTF8 }
Remove-Item -LiteralPath $injected -Force
if (-not $same) { exit 1 }
