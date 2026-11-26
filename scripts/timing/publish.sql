-- Timing runs: publish ConfigSave into Config of a disposable timing copy, the
-- way the real cycles did (bsp_r3, uha_r1, v85 bsp_r1/uha_r1): staged rows
-- replace every Config row with the same FileName (all parts), ConfigSave is
-- emptied, one transaction. Refuses any database whose name is not a timing
-- copy.   usage: sqlcmd ... -v DB="ibcmd_rs_tm_<bsp|uha>_<date>" -i publish.sql
SET NOCOUNT ON;
SET XACT_ABORT ON;
USE [$(DB)];
IF DB_NAME() NOT LIKE N'ibcmd[_]rs[_]tm[_]%'
    THROW 57000, 'Refusing to publish outside a timing copy', 1;
DECLARE @save_rows bigint = (SELECT COUNT_BIG(*) FROM dbo.ConfigSave);
IF @save_rows = 0
    THROW 57001, 'ConfigSave is empty; nothing to publish', 1;
BEGIN TRAN;
DELETE c FROM dbo.Config c
WHERE EXISTS (SELECT 1 FROM dbo.ConfigSave s WHERE s.FileName = c.FileName);
DECLARE @deleted bigint = @@ROWCOUNT;
INSERT INTO dbo.Config (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)
SELECT FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo
FROM dbo.ConfigSave;
DECLARE @inserted bigint = @@ROWCOUNT;
IF @inserted <> @save_rows
    THROW 57002, 'Inserted row count differs from the ConfigSave row count', 1;
DELETE FROM dbo.ConfigSave;
COMMIT;
SELECT @save_rows AS staged_rows, @deleted AS config_rows_deleted, @inserted AS config_rows_inserted,
       (SELECT COUNT_BIG(*) FROM dbo.Config) AS config_rows_after;
