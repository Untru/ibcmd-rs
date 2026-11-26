-- #344 H1 platform-level check, preparation on the disposable clone ibcmd_rs_05_online_a1 ONLY (live needs FULL recovery + a full backup).
SET NOCOUNT ON;
ALTER DATABASE [ibcmd_rs_05_online_a1] SET RECOVERY FULL;
BACKUP DATABASE [ibcmd_rs_05_online_a1] TO DISK = N'F:\ibcmd\lab\05\online\bak\ibcmd_rs_05_online_a1_base.bak' WITH INIT, COMPRESSION, CHECKSUM, STATS = 50;
SELECT name, recovery_model_desc, state_desc FROM sys.databases WHERE name = N'ibcmd_rs_05_online_a1';
