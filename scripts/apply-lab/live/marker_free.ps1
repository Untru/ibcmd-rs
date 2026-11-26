# Makes a lab clone of the БСП corpus MARKER-FREE: the corpus backup carries a native online (dynamic) generation, and the live and worker
# modes refuse such a database (#408 step 1: they would discard it). Deleting the two `DynamicallyUpdated` markers and the `_dynupdate_`
# alias rows leaves the ORDINARY generation, a complete and consistent configuration (the state before the native online update) -- the
# way the #344 marker-free clone was made. Lab databases only; no session may be connected.
#   pwsh -NoProfile -File marker_free.ps1 -Database ibcmd_rs_05_apply_wlab1_20260930
param([Parameter(Mandatory = $true)][ValidatePattern('^ibcmd_rs_0[45]_[a-z0-9_]+$')][string]$Database)
$ErrorActionPreference = 'Stop'
$sessions = [int](sqlcmd -S localhost -E -C -h -1 -W -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM sys.dm_exec_sessions WHERE is_user_process = 1 AND database_id = DB_ID(N'$Database')" | Select-Object -First 1)
if ($sessions) { throw "$sessions session(s) are connected to $Database" }
$sql = @"
SET NOCOUNT ON; SET XACT_ABORT ON;
USE [$Database];
BEGIN TRANSACTION;
DELETE FROM dbo.Config WHERE FileName = N'DynamicallyUpdated';
DELETE FROM dbo.Params WHERE FileName = N'DynamicallyUpdated';
DELETE FROM dbo.Config WHERE FileName LIKE N'%[_]dynupdate[_]%';
COMMIT TRANSACTION;
SELECT (SELECT COUNT(*) FROM dbo.Config WHERE FileName = N'DynamicallyUpdated' OR FileName LIKE N'%[_]dynupdate[_]%') + (SELECT COUNT(*) FROM dbo.Params WHERE FileName = N'DynamicallyUpdated');
"@
$left = sqlcmd -S localhost -E -C -h -1 -W -b -Q $sql
"markers and alias rows left: $(($left | Select-Object -Last 1).Trim())"
