use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};

const SQLSERVER_XEVENTS: &str = r#"/*
ibcmd-rs SQL Server trace template.

Replace:
  $(DATABASE_NAME) - target 1C database name
  $(SESSION_NAME)  - event session name, for example ibcmd_rs_trace

Run as a SQL Server administrator, then execute ibcmd/1cv8 load in another session.
*/

IF EXISTS (SELECT 1 FROM sys.server_event_sessions WHERE name = N'$(SESSION_NAME)')
    DROP EVENT SESSION [$(SESSION_NAME)] ON SERVER;
GO

CREATE EVENT SESSION [$(SESSION_NAME)] ON SERVER
ADD EVENT sqlserver.rpc_completed(
    ACTION(
        sqlserver.client_app_name,
        sqlserver.client_hostname,
        sqlserver.database_name,
        sqlserver.session_id,
        sqlserver.transaction_id,
        sqlserver.sql_text,
        sqlserver.username,
        package0.attach_activity_id,
        package0.attach_activity_id_xfer
    )
    WHERE ([sqlserver].[database_name] = N'$(DATABASE_NAME)')
),
ADD EVENT sqlserver.sql_batch_completed(
    ACTION(
        sqlserver.client_app_name,
        sqlserver.client_hostname,
        sqlserver.database_name,
        sqlserver.session_id,
        sqlserver.transaction_id,
        sqlserver.sql_text,
        sqlserver.username,
        package0.attach_activity_id,
        package0.attach_activity_id_xfer
    )
    WHERE ([sqlserver].[database_name] = N'$(DATABASE_NAME)')
),
ADD EVENT sqlserver.lock_deadlock(
    ACTION(
        sqlserver.client_app_name,
        sqlserver.database_name,
        sqlserver.session_id,
        sqlserver.transaction_id,
        sqlserver.sql_text,
        package0.attach_activity_id,
        package0.attach_activity_id_xfer
    )
    WHERE ([sqlserver].[database_name] = N'$(DATABASE_NAME)')
)
ADD TARGET package0.event_file(
    SET filename = N'C:\temp\ibcmd-rs-trace.xel',
        max_file_size = 512,
        max_rollover_files = 8
)
WITH (
    MAX_MEMORY = 64 MB,
    EVENT_RETENTION_MODE = ALLOW_SINGLE_EVENT_LOSS,
    MAX_DISPATCH_LATENCY = 5 SECONDS,
    TRACK_CAUSALITY = ON,
    STARTUP_STATE = OFF
);
GO

ALTER EVENT SESSION [$(SESSION_NAME)] ON SERVER STATE = START;
GO

-- Stop after the measured load:
-- ALTER EVENT SESSION [$(SESSION_NAME)] ON SERVER STATE = STOP;
-- SELECT CAST(event_data AS xml) AS event_xml
-- FROM sys.fn_xe_file_target_read_file(N'C:\temp\ibcmd-rs-trace*.xel', NULL, NULL, NULL);
"#;

const TECH_LOG_README: &str = r#"1C technical log capture notes
==============================

Goal
----
Capture platform-side phases while SQL Server Extended Events captures database calls.

Recommended fields to collect
-----------------------------
- Timestamp and duration.
- Process id, session id, connection id.
- SQL text or DBMSSQL events where available.
- Configuration load/update phases.
- Locks, waits, exceptions and transaction boundaries.

Suggested run protocol
----------------------
1. Start SQL Server Extended Events from `sqlserver-xevents.sql`.
2. Enable a dedicated 1C technical log folder for the test process.
3. Run `ibcmd-rs profile-run --capture-output -- ibcmd ...`.
4. Stop Extended Events.
5. Archive:
   - source manifest JSON,
   - load plan JSON,
   - profile-run JSON,
   - SQL Server `.xel`,
   - 1C technical log files,
   - exact platform version and database compatibility mode.

Do not run early write experiments against production databases.
"#;

pub fn write_trace_templates(output_dir: &Path, overwrite: bool) -> Result<()> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    write_one(
        &output_dir.join("sqlserver-xevents.sql"),
        SQLSERVER_XEVENTS,
        overwrite,
    )?;
    write_one(
        &output_dir.join("1c-tech-log-notes.txt"),
        TECH_LOG_README,
        overwrite,
    )?;
    Ok(())
}

fn write_one(path: &Path, content: &str, overwrite: bool) -> Result<()> {
    if path.exists() && !overwrite {
        return Err(anyhow!(
            "{} already exists, pass --overwrite to replace it",
            path.display()
        ));
    }
    fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))
}
