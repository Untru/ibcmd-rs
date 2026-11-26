SET NOCOUNT ON;
SET QUOTED_IDENTIFIER ON;

WITH e AS (
    SELECT object_name, CAST(event_data AS xml) AS x
    FROM sys.fn_xe_file_target_read_file(
        N'D:\SQL\ibcmd_rs_activation_ext_oracle*.xel', NULL, NULL, NULL)
), p AS (
    SELECT
        object_name,
        x.value('(/event/@timestamp)[1]', 'datetime2') AS utc_time,
        x.value('(/event/action[@name="client_app_name"]/value)[1]', 'nvarchar(256)') AS app,
        x.value('(/event/data[@name="statement"]/value)[1]', 'nvarchar(4000)') AS statement
    FROM e
)
SELECT statement, COUNT_BIG(*) AS executions, MIN(utc_time) AS first_utc, MAX(utc_time) AS last_utc
FROM p
WHERE object_name = N'sp_statement_completed'
  AND app = N''
  AND utc_time >= '2026-08-29T08:54:29'
  AND (
       statement LIKE N'%ConfigCAS%'
    OR statement LIKE N'%ExtensionsInfo%'
    OR statement LIKE N'%ExtensionsRestruct%'
    OR statement LIKE N'%Params%'
    OR statement LIKE N'%Files%'
  )
GROUP BY statement
ORDER BY first_utc, statement;
