-- #344 LIVE evidence, preparation on the disposable clone ibcmd_rs_05_online_b1 ONLY:
-- live activation needs FULL (or BULK_LOGGED) recovery and an existing full backup to start the log chain.
SET NOCOUNT ON;
ALTER DATABASE [ibcmd_rs_05_online_b1] SET RECOVERY FULL;
BACKUP DATABASE [ibcmd_rs_05_online_b1] TO DISK = N'F:\ibcmd\lab\05\online\bak\ibcmd_rs_05_online_b1_base.bak' WITH INIT, COMPRESSION, CHECKSUM, STATS = 50;
SELECT name, recovery_model_desc, state_desc FROM sys.databases WHERE name = N'ibcmd_rs_05_online_b1';
