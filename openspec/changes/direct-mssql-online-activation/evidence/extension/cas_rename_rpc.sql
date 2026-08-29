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
        x.value('(/event/data[@name="statement"]/value)[1]', 'nvarchar(max)') AS statement
    FROM e
)
SELECT utc_time, statement
FROM p
WHERE object_name = N'rpc_completed'
  AND (
       statement LIKE N'exec sp_executesql N''DELETE FROM configcas WHERE FileName%'
    OR statement LIKE N'exec sp_executesql N''UPDATE configcas SET FileName%'
  )
ORDER BY utc_time;
