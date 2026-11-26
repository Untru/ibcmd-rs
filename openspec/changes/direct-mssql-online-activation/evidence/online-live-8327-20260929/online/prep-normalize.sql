-- #344 ONLINE evidence, preparation on the disposable clone ibcmd_rs_05_online_a1 ONLY.
-- The BSP 8.3.27 corpus carries the leftover of a native dynamic update (Config/Params DynamicallyUpdated markers and
-- five *_dynupdate_* alias rows for two objects). This returns the clone to a history-free ordinary state (what a database
-- that never had a dynamic update looks like) and clears the dirty ConfigSave left by the first, refused apply.
SET NOCOUNT ON; SET XACT_ABORT ON;
USE [ibcmd_rs_05_online_a1];
BEGIN TRANSACTION;
DELETE FROM dbo.ConfigSave;
IF @@ROWCOUNT <> 5 THROW 57901, 'ConfigSave was expected to hold the 5 staged rows of the refused apply', 1;
DELETE FROM dbo.Config WHERE FileName LIKE N'%[_]dynupdate[_]%';
IF @@ROWCOUNT <> 5 THROW 57902, 'expected exactly 5 alias rows', 1;
DELETE FROM dbo.Config WHERE FileName = N'DynamicallyUpdated';
IF @@ROWCOUNT <> 1 THROW 57903, 'Config marker', 1;
DELETE FROM dbo.Params WHERE FileName = N'DynamicallyUpdated';
IF @@ROWCOUNT <> 1 THROW 57904, 'Params marker', 1;
COMMIT TRANSACTION;
SELECT 'Config rows' AS what, COUNT(*) AS n FROM dbo.Config UNION ALL SELECT 'Params rows', COUNT(*) FROM dbo.Params UNION ALL SELECT 'ConfigSave rows', COUNT(*) FROM dbo.ConfigSave;
