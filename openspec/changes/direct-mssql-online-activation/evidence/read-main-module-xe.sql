SET NOCOUNT ON;
SET QUOTED_IDENTIFIER ON;

WITH source_events AS (
    SELECT object_name, CAST(event_data AS xml) AS event_xml
    FROM sys.fn_xe_file_target_read_file(
        N'C:\temp\ibcmd_rs_activation_oracle_main_module_20260829*.xel',
        NULL, NULL, NULL)
), parsed AS (
    SELECT
        object_name,
        event_xml.value('(event/@timestamp)[1]', 'datetime2') AS utc_time,
        event_xml.value(
            '(event/action[@name="client_app_name"]/value/text())[1]',
            'nvarchar(256)') AS client_app,
        event_xml.value(
            '(event/action[@name="session_id"]/value/text())[1]',
            'int') AS session_id,
        COALESCE(
            NULLIF(event_xml.value(
                '(event/data[@name="statement"]/value/text())[1]',
                'nvarchar(max)'), N''),
            NULLIF(event_xml.value(
                '(event/data[@name="batch_text"]/value/text())[1]',
                'nvarchar(max)'), N''),
            event_xml.value(
                '(event/action[@name="sql_text"]/value/text())[1]',
                'nvarchar(max)')) AS sql_text,
        event_xml.value(
            '(event/data[@name="duration"]/value/text())[1]',
            'bigint') AS duration_us,
        event_xml.value(
            '(event/data[@name="row_count"]/value/text())[1]',
            'bigint') AS row_count
    FROM source_events
)
SELECT object_name, utc_time, client_app, session_id, duration_us, row_count,
       sql_text
FROM parsed
ORDER BY utc_time
FOR JSON PATH;
