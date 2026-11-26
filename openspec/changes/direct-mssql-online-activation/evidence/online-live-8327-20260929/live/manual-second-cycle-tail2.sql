-- The second recovery cycle of render_live_recovery (src/mssql_main_activation.rs), run by hand after the tool aborted at the
-- readiness gate (code 57234). Same statements, same order; tail-log appended to the artifact of the first cycle.
SET NOCOUNT ON;
USE [master];
SELECT SYSUTCDATETIME() AS started_utc;
ALTER DATABASE [ibcmd_rs_05_online_b1] SET SINGLE_USER WITH ROLLBACK IMMEDIATE;
BACKUP LOG [ibcmd_rs_05_online_b1] TO DISK=N'F:\ibcmd\lab\05\online\bak\ibcmd_rs_05_online_b1_tail2.trn' WITH NORECOVERY, NOINIT, COMPRESSION, CHECKSUM;
RESTORE DATABASE [ibcmd_rs_05_online_b1] WITH RECOVERY;
ALTER DATABASE [ibcmd_rs_05_online_b1] SET MULTI_USER;
SELECT SYSUTCDATETIME() AS finished_utc;
