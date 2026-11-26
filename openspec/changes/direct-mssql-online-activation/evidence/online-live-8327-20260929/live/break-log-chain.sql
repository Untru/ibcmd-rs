-- #344: break the log chain of the disposable clone ibcmd_rs_05_online_b1 (SIMPLE -> FULL without a new full backup), to test the preflight of live.
SET NOCOUNT ON;
ALTER DATABASE [ibcmd_rs_05_online_b1] SET RECOVERY SIMPLE;
ALTER DATABASE [ibcmd_rs_05_online_b1] SET RECOVERY FULL;
SELECT name, recovery_model_desc, log_reuse_wait_desc FROM sys.databases WHERE name = N'ibcmd_rs_05_online_b1';
SELECT last_log_backup_lsn FROM sys.database_recovery_status WHERE database_id = DB_ID(N'ibcmd_rs_05_online_b1');
