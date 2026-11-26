//! The built-in client against a real SQL Server (`--features
//! mssql-live-tests`). The server is `IBCMD_RS_TEST_SQL_SERVER` (default
//! `localhost`); the tests write only to tables of their own in tempdb,
//! named `ibcmd_rs_03_live_*`, and drop them. The SQL-login test runs when
//! `IBCMD_RS_TEST_SQL_USER` and `IBCMD_RS_TEST_SQL_PASSWORD` are set.

use crate::sql::{ScriptVariables, SqlExec, SqlLogin, SqlOptions, SqlParam, SqlTarget, SqlValue};

fn server() -> String {
    std::env::var("IBCMD_RS_TEST_SQL_SERVER").unwrap_or_else(|_| "localhost".to_owned())
}

fn integrated() -> SqlExec {
    let server = server();
    SqlExec::from_options(SqlOptions::integrated(&server, None)).unwrap()
}

fn scratch_table(name: &str) -> String {
    format!(
        "tempdb.dbo.[ibcmd_rs_03_live_{name}_{}]",
        std::process::id()
    )
}

#[test]
fn integrated_login_reaches_the_server_over_encrypted_tcp() {
    let sql = integrated();
    let client = sql.client().unwrap();
    let rows = client
        .query_rows(
            "SELECT net_transport, encrypt_option, SUSER_SNAME(), @@OPTIONS & 256, @@TEXTSIZE \
             FROM sys.dm_exec_connections WHERE session_id = @@SPID",
            &[],
        )
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].text(0).unwrap(), "TCP");
    assert_eq!(rows[0].text(1).unwrap(), "TRUE");
    assert!(!rows[0].text(2).unwrap().is_empty());
    // QUOTED_IDENTIFIER is off, as in a sqlcmd session.
    assert_eq!(rows[0].i64(3).unwrap(), 0);
    // No limit on the size of a returned varbinary(max) value.
    assert_eq!(rows[0].i64(4).unwrap(), -1);
}

#[test]
fn sql_login_reaches_the_server() {
    let (Ok(user), Ok(password)) = (
        std::env::var("IBCMD_RS_TEST_SQL_USER"),
        std::env::var("IBCMD_RS_TEST_SQL_PASSWORD"),
    ) else {
        eprintln!("IBCMD_RS_TEST_SQL_USER/PASSWORD not set; SQL login not tested");
        return;
    };
    let server = server();
    let sql = SqlExec::from_options(SqlOptions {
        user: Some(&user),
        password: Some(&password),
        ..SqlOptions::integrated(&server, None)
    })
    .unwrap();
    let login = sql
        .client()
        .unwrap()
        .query_scalar("SELECT SUSER_SNAME()", &[])
        .unwrap();
    assert_eq!(login, Some(SqlValue::Text(user.clone())));
    // A wrong password is refused at once, not retried for half a minute.
    let started = std::time::Instant::now();
    let wrong = SqlExec::from_options(SqlOptions {
        user: Some(&user),
        password: Some("certainly-not-the-password"),
        ..SqlOptions::integrated(&server, None)
    })
    .unwrap();
    let error = wrong
        .client()
        .unwrap()
        .query_scalar("SELECT 1", &[])
        .unwrap_err();
    assert!(format!("{error:#}").contains("18456"), "{error:#}");
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
    assert!(!format!("{error:#}").contains(&password));
}

#[test]
fn queries_leave_no_session_state_on_pooled_connections() {
    let sql = integrated();
    let client = sql.client().unwrap();
    for _ in 0..(client.max_connections() * 2) {
        let database = client
            .query_scalar(
                "USE tempdb; CREATE TABLE #t (a int); SELECT DB_NAME();",
                &[],
            )
            .unwrap();
        assert_eq!(database, Some(SqlValue::Text("tempdb".to_owned())));
        let after = client.query_scalar("SELECT DB_NAME()", &[]).unwrap();
        assert_ne!(after, Some(SqlValue::Text("tempdb".to_owned())));
    }
}

#[test]
fn for_json_documents_are_joined_across_rows() {
    let sql = integrated();
    let client = sql.client().unwrap();
    let json = client
        .query_json(
            "SELECT TOP (500) a.name, a.object_id FROM sys.all_objects a \
             ORDER BY a.object_id FOR JSON PATH",
        )
        .unwrap()
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 500);
    assert!(json.len() > 4000, "the document spans several rows");
    let empty = client
        .query_json("SELECT name FROM sys.all_objects WHERE 1 = 0 FOR JSON PATH")
        .unwrap();
    assert_eq!(empty, None);
}

#[test]
fn scripts_run_batch_by_batch_and_stop_at_the_first_error() {
    let sql = integrated();
    let client = sql.client().unwrap();
    let table = scratch_table("script");
    let script = format!(
        "SET NOCOUNT ON;\nIF OBJECT_ID(N'{table}', N'U') IS NOT NULL DROP TABLE {table};\n\
         CREATE TABLE {table} (a int NOT NULL);\nGO\nINSERT INTO {table} VALUES (1);\nGO 2\n\
         DECLARE @n int = (SELECT COUNT(*) FROM {table});\n\
         IF @n <> 2 THROW 50000, 'wrong count', 1;\n"
    );
    client.run_script(&script, ScriptVariables::Refuse).unwrap();
    let failing = format!(
        "INSERT INTO {table} VALUES (3);\nGO\nTHROW 50001, 'stop here', 1;\nGO\nINSERT INTO {table} VALUES (4);\n"
    );
    let error = client
        .run_script(&failing, ScriptVariables::Refuse)
        .unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("stop here"), "{message}");
    assert!(message.contains("batch 2 of 3"), "{message}");
    let rows = client
        .query_rows(&format!("SELECT a FROM {table} ORDER BY a"), &[])
        .unwrap();
    let values = rows
        .iter()
        .map(|row| row.i64(0).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(values, vec![1, 1, 3]);
    client.execute(&format!("DROP TABLE {table}"), &[]).unwrap();
}

#[test]
fn bulk_writes_carry_large_varbinary_values_byte_for_byte() {
    let sql = integrated();
    let client = sql.client().unwrap();
    let table = scratch_table("bulk");
    client
        .execute(
            &format!(
                "IF OBJECT_ID(N'{table}', N'U') IS NOT NULL DROP TABLE {table}; \
                 CREATE TABLE {table} (FileName nvarchar(128) NOT NULL, Kind tinyint NOT NULL, \
                 DataSize bigint NOT NULL, BinaryData varbinary(max) NOT NULL)"
            ),
            &[],
        )
        .unwrap();
    let sizes = [
        0usize,
        1,
        8000,
        8001,
        65_535,
        65_536,
        3 * 1024 * 1024,
        70_000_000,
    ];
    let blobs = sizes
        .iter()
        .enumerate()
        .map(|(index, size)| {
            (0..*size)
                .map(|offset| (offset * 31 + index) as u8)
                .collect::<Vec<u8>>()
        })
        .collect::<Vec<_>>();
    let names = (0..blobs.len())
        .map(|index| format!("Строка.{index}"))
        .collect::<Vec<_>>();
    let rows = blobs
        .iter()
        .zip(&names)
        .map(|(blob, name)| {
            vec![
                SqlParam::Text(name),
                SqlParam::U8(1),
                SqlParam::I64(blob.len() as i64),
                SqlParam::Binary(blob),
            ]
        })
        .collect::<Vec<_>>();
    let written = client
        .write_rows(
            &table,
            &["FileName", "Kind", "DataSize", "BinaryData"],
            &rows,
        )
        .unwrap();
    assert_eq!(written, blobs.len() as u64);
    let mut read = Vec::new();
    client
        .read_rows(
            &format!("SELECT FileName, DataSize, BinaryData FROM {table} ORDER BY DataSize"),
            &[],
            &mut |mut row| {
                read.push((row.take_text(0)?, row.i64(1)?, row.take_binary(2)?));
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(read.len(), blobs.len());
    for ((name, size, bytes), (expected_name, expected)) in
        read.iter().zip(names.iter().zip(&blobs))
    {
        assert_eq!(name, expected_name);
        assert_eq!(*size as usize, expected.len());
        assert!(bytes == expected, "row {name} differs");
    }
    client.execute(&format!("DROP TABLE {table}"), &[]).unwrap();
}

#[test]
fn a_parallel_bulk_write_lands_every_row() {
    let sql = integrated();
    let client = sql.client().unwrap();
    let table = scratch_table("parallel");
    client
        .execute(
            &format!(
                "IF OBJECT_ID(N'{table}', N'U') IS NOT NULL DROP TABLE {table}; \
                 CREATE TABLE {table} (FileName nvarchar(128) NOT NULL, BinaryData varbinary(max) NOT NULL)"
            ),
            &[],
        )
        .unwrap();
    let blob = vec![0x5au8; 40_000];
    let names = (0..3000)
        .map(|index| format!("r{index:05}"))
        .collect::<Vec<_>>();
    let rows = names
        .iter()
        .map(|name| vec![SqlParam::Text(name), SqlParam::Binary(&blob)])
        .collect::<Vec<_>>();
    let written = client
        .write_rows(&table, &["FileName", "BinaryData"], &rows)
        .unwrap();
    assert_eq!(written, 3000);
    let count = client
        .query_scalar(
            &format!("SELECT COUNT_BIG(DISTINCT FileName) FROM {table} WHERE DATALENGTH(BinaryData) = 40000"),
            &[],
        )
        .unwrap();
    assert_eq!(count, Some(SqlValue::Int(3000)));
    client.execute(&format!("DROP TABLE {table}"), &[]).unwrap();
}

#[test]
fn server_errors_name_the_message_and_leave_the_pool_usable() {
    let sql = integrated();
    let client = sql.client().unwrap();
    let error = client
        .query_rows("SELECT 1 AS a; THROW 50002, 'deliberate', 1;", &[])
        .unwrap_err();
    assert!(format!("{error:#}").contains("deliberate"));
    assert_eq!(
        client.query_scalar("SELECT 7", &[]).unwrap(),
        Some(SqlValue::Int(7))
    );
}

#[test]
fn an_unreachable_port_fails_instead_of_hanging() {
    let target = SqlTarget {
        server: "127.0.0.1,1".to_owned(),
        database: None,
        login: SqlLogin::Integrated,
        trust_server_certificate: true,
    };
    // Six attempts with growing pauses: about half a minute at most.
    let sql = SqlExec::sql_server(target).unwrap();
    let started = std::time::Instant::now();
    assert!(sql.client().unwrap().query_scalar("SELECT 1", &[]).is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(90));
}
