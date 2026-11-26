//! `infobase config import --dbms=PostgreSQL` (#370): the import of an XML
//! tree into an infobase on PostgreSQL.
//!
//! The platform's import there stages the configuration in `configsave` the
//! way it does in a file infobase (8.5.1.1529 into an empty database: every
//! row, then `deleted`, `root`, `version`, `versions`; attributes `0`, the
//! local time in `creation`/`modified`), and `config apply` takes the stage.
//! This import builds the same stage with the file infobase's builder
//! (`crate::file_infobase::plan_stage`) over the database's `config` rows,
//! checks it, and replaces `configsave` with it in one transaction.

use anyhow::{Context, Result, anyhow};
use ibcmd_filedb::{ConfigRow, Timestamp};

use crate::cli::InfobaseConfigImportArgs;
use crate::infobase::{ConnectionConfig, InfobaseConfigImportReport};
use crate::sql::SqlParam;
use crate::sql::postgres::{PgClient, PgConnection, copy_rows, decode_bytea};

pub(crate) fn import(
    config: &ConnectionConfig,
    args: &InfobaseConfigImportArgs,
) -> Result<InfobaseConfigImportReport> {
    let target = config
        .postgres()?
        .ok_or_else(|| anyhow!("no PostgreSQL database"))?;
    let (source_root, source_version) = crate::file_infobase::import_source(config, args)?;
    let database = target.database.clone();
    PgClient::new(target).with_session(|connection| {
        begin_stage(connection)?;
        let stage = crate::file_infobase::plan_stage(
            read_rows(connection, "config")?,
            &source_root,
            source_version,
            args,
        )?;
        let before = write_stage(connection, &stage.rows)
            .with_context(|| format!("не удалось записать configsave базы {}", database))?;
        let after = stage.rows.len();
        Ok(stage.report(
            config,
            source_root,
            Some(source_version),
            "postgres-configsave-stage",
            (before, after),
            None,
        ))
    })
}

/// `infobase config load <file.cf>` into a PostgreSQL infobase (#353).
pub(crate) fn load_cf(
    config: &ConnectionConfig,
    input: &std::path::Path,
) -> Result<InfobaseConfigImportReport> {
    let target = config
        .postgres()?
        .ok_or_else(|| anyhow!("no PostgreSQL database"))?;
    let database = target.database.clone();
    PgClient::new(target).with_session(|connection| {
        begin_stage(connection)?;
        let stage = crate::file_infobase::plan_cf_stage(read_rows(connection, "config")?, input)?;
        let before = write_stage(connection, &stage.rows)
            .with_context(|| format!("не удалось записать configsave базы {}", database))?;
        let after = stage.rows.len();
        Ok(stage.load_report(
            config,
            input,
            "postgres-configsave-stage",
            (before, after),
            None,
        ))
    })
}

/// Hold the configuration generation stable until its replacement stage commits.
fn begin_stage(connection: &mut PgConnection) -> Result<()> {
    connection.query("BEGIN", &mut |_| Ok(()))?;
    connection.query(
        "LOCK TABLE config, configsave IN SHARE ROW EXCLUSIVE MODE",
        &mut |_| Ok(()),
    )
}

/// Every part of every row of `table`.
fn read_rows(connection: &mut PgConnection, table: &str) -> Result<Vec<ConfigRow>> {
    let now = Timestamp::now_utc();
    let mut rows = Vec::new();
    connection
        .query(
            &format!(
                "SELECT filename::text, partno, datasize, encode(binarydata, 'hex') FROM {table}"
            ),
            &mut |row| {
                let text = |column: usize| -> Result<String> {
                    let value = row
                        .get(column)
                        .and_then(Option::as_deref)
                        .ok_or_else(|| anyhow!("{table}: NULL in column {column}"))?;
                    Ok(String::from_utf8(value.to_vec())?)
                };
                rows.push(ConfigRow {
                    file_name: text(0)?,
                    part_no: text(1)?.parse().context("part number")?,
                    creation: now,
                    modified: now,
                    attributes: 0,
                    data_size: text(2)?.parse().context("data size")?,
                    binary: match row.get(3).and_then(Option::as_deref) {
                        Some(value) => decode_bytea(value)?,
                        None => Vec::new(),
                    },
                });
                Ok(())
            },
        )
        .with_context(|| format!("failed to read the {table} table"))?;
    Ok(rows)
}

/// Replaces `configsave` with `rows` in one transaction; returns the rows it
/// held before.
fn write_stage(connection: &mut PgConnection, rows: &[ConfigRow]) -> Result<usize> {
    let written = (|| -> Result<usize> {
        let mut before = 0;
        let mut can_copy = false;
        connection.query(
            "SELECT count(*), has_database_privilege(current_database(), 'TEMP') FROM configsave",
            &mut |row| {
                let count = row
                    .first()
                    .and_then(Option::as_deref)
                    .context("missing configsave count")?;
                before = std::str::from_utf8(count)?.parse()?;
                can_copy = row.get(1).and_then(Option::as_deref) == Some(b"t");
                Ok(())
            },
        )?;
        connection.query("DELETE FROM configsave", &mut |_| Ok(()))?;
        if !can_copy {
            for row in rows {
                connection.query(&insert_statement(row), &mut |_| Ok(()))?;
            }
            return Ok(before);
        }
        // Keep the platform-specific mvarchar cast and the database's local
        // timestamp on the server. The wire representation uses core PG types.
        connection.query(
            "CREATE TEMP TABLE ibcmd_stage (name text, attributes int4, size int8, data bytea, part int4) ON COMMIT DROP",
            &mut |_| Ok(()),
        )?;
        copy_rows(
            connection,
            "pg_temp.ibcmd_stage",
            &["name", "attributes", "size", "data", "part"],
            rows.iter().map(|row| {
                [
                    Ok(SqlParam::Text(&row.file_name)),
                    i32::try_from(row.attributes)
                        .map(SqlParam::I32)
                        .map_err(Into::into),
                    i64::try_from(row.data_size)
                        .map(SqlParam::I64)
                        .map_err(Into::into),
                    Ok(SqlParam::Binary(&row.binary)),
                    i32::try_from(row.part_no)
                        .map(SqlParam::I32)
                        .map_err(Into::into),
                ]
            }),
        )?;
        connection.query(
            "INSERT INTO configsave (filename, creation, modified, attributes, datasize, binarydata, partno) \
             SELECT name::mvarchar, LOCALTIMESTAMP(0), LOCALTIMESTAMP(0), attributes, size, data, part FROM pg_temp.ibcmd_stage",
            &mut |_| Ok(()),
        )?;
        Ok(before)
    })();
    match written {
        Ok(before) => {
            connection.query("COMMIT", &mut |_| Ok(()))?;
            Ok(before)
        }
        Err(error) => {
            let _ = connection.query("ROLLBACK", &mut |_| Ok(()));
            Err(error)
        }
    }
}

fn insert_statement(row: &ConfigRow) -> String {
    let name = row.file_name.replace('\\', "\\\\").replace('\'', "''");
    let mut hex = String::with_capacity(row.binary.len() * 2);
    for byte in &row.binary {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!(
        "INSERT INTO configsave (filename, creation, modified, attributes, datasize, binarydata, partno) \
         VALUES (CAST(E'{}' AS mvarchar), LOCALTIMESTAMP(0), LOCALTIMESTAMP(0), {}, {}, decode('{hex}', 'hex'), {})",
        name, row.attributes, row.data_size, row.part_no
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire_queries(
        can_create_temp: bool,
        run: impl FnOnce(&ConnectionConfig) -> Result<InfobaseConfigImportReport>,
    ) -> Vec<String> {
        use std::io::{Read, Write};
        use std::net::{TcpListener, TcpStream};
        fn send(stream: &mut TcpStream, tag: u8, body: &[u8]) {
            stream.write_all(&[tag]).unwrap();
            stream
                .write_all(&((body.len() + 4) as u32).to_be_bytes())
                .unwrap();
            stream.write_all(body).unwrap();
        }
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .unwrap();
            let mut size = [0; 4];
            stream.read_exact(&mut size).unwrap();
            let mut startup = vec![0; u32::from_be_bytes(size) as usize - 4];
            stream.read_exact(&mut startup).unwrap();
            send(&mut stream, b'R', &0u32.to_be_bytes());
            send(&mut stream, b'Z', b"I");
            let mut queries = Vec::new();
            let mut copy_frames = Vec::new();
            loop {
                let mut header = [0; 5];
                stream.read_exact(&mut header).unwrap();
                let mut body =
                    vec![0; u32::from_be_bytes(header[1..].try_into().unwrap()) as usize - 4];
                stream.read_exact(&mut body).unwrap();
                if header[0] == b'X' {
                    break;
                }
                match header[0] {
                    b'd' => {
                        copy_frames.push(body);
                        continue;
                    }
                    b'c' => {
                        assert_eq!(
                            copy_frames.first().unwrap(),
                            b"PGCOPY\n\xff\r\n\0\0\0\0\0\0\0\0\0"
                        );
                        assert_eq!(copy_frames.pop().unwrap(), (-1i16).to_be_bytes());
                        assert!(copy_frames.len() > 1);
                        for row in &copy_frames[1..] {
                            let mut fields = row.as_slice();
                            let mut count = [0; 2];
                            fields.read_exact(&mut count).unwrap();
                            assert_eq!(i16::from_be_bytes(count), 5);
                            for expected in [None, Some(4), Some(8), None, Some(4)] {
                                let mut length = [0; 4];
                                fields.read_exact(&mut length).unwrap();
                                let length = usize::try_from(i32::from_be_bytes(length)).unwrap();
                                if let Some(expected) = expected {
                                    assert_eq!(length, expected);
                                }
                                fields = &fields[length..];
                            }
                            assert!(fields.is_empty());
                        }
                        send(&mut stream, b'C', b"COPY\0");
                        send(&mut stream, b'Z', b"T");
                        continue;
                    }
                    b'Q' => {}
                    tag => panic!("unexpected frontend message {tag}"),
                }
                queries.push(String::from_utf8(body[..body.len() - 1].to_vec()).unwrap());
                if queries.last().unwrap().starts_with("SELECT count(*)") {
                    let mut row = *b"\0\x02\0\0\0\x010\0\0\0\x01t";
                    if !can_create_temp {
                        *row.last_mut().unwrap() = b'f';
                    }
                    send(&mut stream, b'D', &row);
                }
                if queries.last().unwrap().starts_with("COPY ") {
                    send(&mut stream, b'G', &[1, 0, 5, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1]);
                    continue;
                }
                send(&mut stream, b'C', b"OK\0");
                send(&mut stream, b'Z', b"I");
            }
            queries
        });
        let version = crate::cli::InfobaseConfigSourceVersion::V2_21;
        let config = ConnectionConfig {
            dbms: "PostgreSQL".into(),
            db_server: address,
            db_name: "synthetic".into(),
            db_user: None,
            db_pwd: None,
            password_source: None,
            native_config: None,
            format: crate::cli::InfobaseConfigFormat::Xml,
            legacy_adapter: crate::adapters::mssql_legacy::MssqlLegacyAdapter::from_legacy_selector(
                version,
            ),
            xml_version_given: true,
            file_infobase: None,
            postgres_snapshot: None,
        };
        let result = run(&config);
        let queries = worker.join().unwrap();
        assert!(result.is_ok(), "{result:?}");
        queries
    }

    fn assert_locked_stage(queries: &[String]) {
        assert_eq!(queries.first().map(String::as_str), Some("BEGIN"));
        assert_eq!(
            queries.get(1).map(String::as_str),
            Some("LOCK TABLE config, configsave IN SHARE ROW EXCLUSIVE MODE")
        );
        assert!(queries[2].ends_with("FROM config"));
        assert!(queries.iter().any(|sql| sql
            == "COPY pg_temp.ibcmd_stage (name, attributes, size, data, part) FROM STDIN BINARY"));
        assert_eq!(queries.last().map(String::as_str), Some("COMMIT"));
    }

    #[test]
    fn an_import_locks_both_tables_before_reading_its_base() {
        use crate::cli::{
            InfobaseConfigFormat, InfobaseConfigSourceVersion, InfobaseImportStageMode,
            InfobaseImportVerify,
        };
        let version = InfobaseConfigSourceVersion::V2_21;
        let args = InfobaseConfigImportArgs {
            settings: None,
            native_config: None,
            format: Some(InfobaseConfigFormat::Xml),
            platform: None,
            source_version: Some(version),
            db_path: None,
            dbms: None,
            db_server: None,
            db_name: None,
            db_user: None,
            db_pwd: None,
            db_pwd_env: "UNUSED_SYNTHETIC_PG_PASSWORD".into(),
            user: None,
            password: None,
            password_env: "UNUSED_SYNTHETIC_IB_PASSWORD".into(),
            sqlcmd: None,
            replace_config_save: true,
            allow_non_lab: true,
            batch_size: None,
            path_prefix: vec![],
            files: vec![],
            native_files: None,
            script_output: None,
            stage_mode: InfobaseImportStageMode::Auto,
            verify: InfobaseImportVerify::Auto,
            source_dir: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/file-infobase/tiny-source"),
        };
        assert_locked_stage(&wire_queries(true, |config| import(config, &args)));
    }

    #[test]
    fn a_cf_load_locks_both_tables_before_reading_its_base() {
        let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/file-infobase/tiny.cf");
        let queries = wire_queries(true, |config| {
            let report = load_cf(config, &input)?;
            assert_eq!(report.operation, "infobase config load");
            assert_eq!(report.format, "cf");
            Ok(report)
        });
        assert_locked_stage(&queries);
    }
    #[test]
    fn a_role_without_temp_stages_rows_with_inserts() {
        let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/file-infobase/tiny.cf");
        let queries = wire_queries(false, |config| load_cf(config, &input));
        assert!(
            !queries
                .iter()
                .any(|query| query.starts_with("COPY ") || query.starts_with("CREATE TEMP"))
        );
        assert!(
            queries
                .iter()
                .any(|query| query.starts_with("INSERT INTO configsave"))
        );
        assert_eq!(queries.last().map(String::as_str), Some("COMMIT"));
    }

    #[test]
    fn row_names_quote_backslashes_independently_of_the_server_setting() {
        let row = ConfigRow {
            file_name: r"a\'b".into(),
            part_no: 0,
            creation: Timestamp::now_utc(),
            modified: Timestamp::now_utc(),
            attributes: 0,
            data_size: 0,
            binary: vec![],
        };
        assert!(insert_statement(&row).contains(r"CAST(E'a\\''b' AS mvarchar)"));
    }

    #[test]
    fn a_row_is_inserted_with_its_name_quoted_and_bytes_in_hex() {
        let row = ConfigRow {
            file_name: "a'b".to_string(),
            part_no: 1,
            creation: Timestamp::now_utc(),
            modified: Timestamp::now_utc(),
            attributes: 0,
            data_size: 20,
            binary: vec![0, 0xab],
        };
        assert_eq!(
            insert_statement(&row),
            "INSERT INTO configsave (filename, creation, modified, attributes, datasize, binarydata, partno) \
             VALUES (CAST(E'a''b' AS mvarchar), LOCALTIMESTAMP(0), LOCALTIMESTAMP(0), 0, 20, decode('00ab', 'hex'), 1)"
        );
    }
}
