SET NOCOUNT ON;
SET QUOTED_IDENTIFIER ON;

WITH e AS (
    SELECT object_name, CAST(event_data AS xml) AS x
    FROM sys.fn_xe_file_target_read_file(
        N'D:\SQL\ibcmd_rs_activation_ext_oracle*.xel', NULL, NULL, NULL)
    WHERE CONVERT(nvarchar(max), event_data) LIKE N'%ConfigCAS%'
       OR CONVERT(nvarchar(max), event_data) LIKE N'%ExtensionsInfo%'
       OR CONVERT(nvarchar(max), event_data) LIKE N'%ExtensionsRestruct%'
       OR CONVERT(nvarchar(max), event_data) LIKE N'%Params%'
       OR CONVERT(nvarchar(max), event_data) LIKE N'%Files%'
)
SELECT
    object_name,
    x.value('(/event/@timestamp)[1]', 'datetime2') AS utc_time,
    x.value('(/event/action[@name="client_app_name"]/value)[1]', 'nvarchar(256)') AS app,
    x.value('(/event/data[@name="statement"]/value)[1]', 'nvarchar(4000)') AS statement,
    x.value('(/event/action[@name="sql_text"]/value)[1]', 'nvarchar(4000)') AS sql_text
FROM e
ORDER BY utc_time;
