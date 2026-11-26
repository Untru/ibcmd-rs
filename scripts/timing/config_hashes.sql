-- Read-only: every Config row of a timing copy with its attributes, size and
-- SHA-256, to compare what two loaders published.
-- usage: run_timing's Sql substitution or sqlcmd with DB replaced.
SET NOCOUNT ON;
USE [$(DB)];
SELECT FileName, PartNo, Attributes, DataSize,
       CONVERT(varchar(64), HASHBYTES('SHA2_256', BinaryData), 2) AS sha256
FROM dbo.Config
ORDER BY FileName, PartNo;
