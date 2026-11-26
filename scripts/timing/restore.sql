-- Timing runs: (re)set a disposable timing copy to the backup's state before
-- each load measurement. Only names starting with ibcmd_rs_tm_ are accepted;
-- an existing timing copy is replaced (it is the previous measurement's copy
-- of the same backup). Files on F:, SIMPLE recovery.
-- usage: sqlcmd ... -v DB="ibcmd_rs_tm_..." BAK="F:\...\x.bak" DATA="<logical>" LOG="<logical>" -i restore.sql
SET NOCOUNT ON;
SET XACT_ABORT ON;
USE master;
IF N'$(DB)' NOT LIKE N'ibcmd[_]rs[_]tm[_]%'
    THROW 57100, 'Timing copies must be named ibcmd_rs_tm_*', 1;
IF DB_ID(N'$(DB)') IS NOT NULL
    ALTER DATABASE [$(DB)] SET SINGLE_USER WITH ROLLBACK IMMEDIATE;
RESTORE DATABASE [$(DB)]
    FROM DISK = N'$(BAK)'
    WITH REPLACE, STATS = 50,
         MOVE N'$(DATA)' TO N'F:\ibcmd\lab\timing\sqldata\$(DB).mdf',
         MOVE N'$(LOG)' TO N'F:\ibcmd\lab\timing\sqldata\$(DB)_log.ldf';
ALTER DATABASE [$(DB)] SET MULTI_USER;
ALTER DATABASE [$(DB)] SET RECOVERY SIMPLE;
SELECT name, state_desc, recovery_model_desc FROM sys.databases WHERE name = N'$(DB)';
