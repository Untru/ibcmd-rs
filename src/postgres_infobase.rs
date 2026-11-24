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
use crate::sql::postgres::{PgConnection, decode_bytea};

pub(crate) fn import(
    config: &ConnectionConfig,
    args: &InfobaseConfigImportArgs,
) -> Result<InfobaseConfigImportReport> {
    let target = config
        .postgres()?
        .ok_or_else(|| anyhow!("no PostgreSQL database"))?;
    let (source_root, source_version) = crate::file_infobase::import_source(config, args)?;
    let mut connection = PgConnection::connect(&target)?;
    begin_stage(&mut connection)?;
    let stage = crate::file_infobase::plan_stage(
        read_rows(&mut connection, "config")?,
        &source_root,
        source_version,
        args,
    )?;
    let before = write_stage(&mut connection, &stage.rows)
        .with_context(|| format!("не удалось записать configsave базы {}", target.database))?;
    let after = stage.rows.len();
    Ok(stage.report(
        config,
        source_root,
        Some(source_version),
        "postgres-configsave-stage",
        (before, after),
        None,
    ))
}

/// `infobase config load <file.cf>` into a PostgreSQL infobase (#353).
pub(crate) fn load_cf(
    config: &ConnectionConfig,
    input: &std::path::Path,
) -> Result<InfobaseConfigImportReport> {
    let target = config
        .postgres()?
        .ok_or_else(|| anyhow!("no PostgreSQL database"))?;
    let mut connection = PgConnection::connect(&target)?;
    begin_stage(&mut connection)?;
    let stage = crate::file_infobase::plan_cf_stage(read_rows(&mut connection, "config")?, input)?;
    let before = write_stage(&mut connection, &stage.rows)
        .with_context(|| format!("не удалось записать configsave базы {}", target.database))?;
    let after = stage.rows.len();
    Ok(stage.load_report(
        config,
        input,
        "postgres-configsave-stage",
        (before, after),
        None,
    ))
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
        let mut metadata = None;
        connection.query(
            "SELECT count(*), LOCALTIMESTAMP(0)::text FROM configsave",
            &mut |row| {
                let text = |i: usize| -> Result<&str> {
                    Ok(std::str::from_utf8(
                        row.get(i)
                            .and_then(Option::as_deref)
                            .context("missing stage metadata")?,
                    )?)
                };
                metadata = Some((text(0)?.parse::<usize>()?, text(1)?.to_owned()));
                Ok(())
            },
        )?;
        let (before, timestamp) = metadata.context("missing stage metadata row")?;
        connection.query("DELETE FROM configsave", &mut |_| Ok(()))?;
        connection.copy_in(
            "COPY configsave (filename, creation, modified, attributes, datasize, binarydata, partno) FROM STDIN WITH (FORMAT csv)",
            rows.iter().map(|row| copy_row(row, &timestamp)),
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

fn copy_row(row: &ConfigRow, timestamp: &str) -> Vec<u8> {
    use std::io::Write;

    let mut bytes = Vec::new();
    let name = row.file_name.replace('"', "\"\"");
    write!(
        bytes,
        "\"{name}\",{timestamp},{timestamp},{},{},\\x",
        row.attributes, row.data_size
    )
    .unwrap();
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in &row.binary {
        bytes.extend_from_slice(&[HEX[(byte >> 4) as usize], HEX[(byte & 15) as usize]]);
    }
    writeln!(bytes, ",{}", row.part_no).unwrap();
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    enum CopyReply {
        Complete,
        Error,
        Short,
    }

    fn wire_queries(
        reply: CopyReply,
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
            let mut row_count = 0;
            loop {
                let mut header = [0; 5];
                stream.read_exact(&mut header).unwrap();
                let mut body =
                    vec![0; u32::from_be_bytes(header[1..].try_into().unwrap()) as usize - 4];
                stream.read_exact(&mut body).unwrap();
                if header[0] == b'X' {
                    break;
                }
                if header[0] == b'd' {
                    // The tiny fixtures have no line breaks in their file names.
                    row_count += body.iter().filter(|&&byte| byte == b'\n').count();
                    continue;
                }
                if header[0] == b'c' {
                    match reply {
                        CopyReply::Error => {
                            send(&mut stream, b'E', b"SERROR\0Msynthetic COPY failure\0\0")
                        }
                        CopyReply::Short => send(&mut stream, b'C', b"COPY 1\0"),
                        CopyReply::Complete => {
                            send(&mut stream, b'C', format!("COPY {row_count}\0").as_bytes())
                        }
                    }
                } else {
                    assert_eq!(header[0], b'Q');
                    let query = String::from_utf8(body[..body.len() - 1].to_vec()).unwrap();
                    queries.push(query.clone());
                    if query.starts_with("COPY ") {
                        send(
                            &mut stream,
                            b'G',
                            &[0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                        );
                        continue;
                    }
                    if query.starts_with("SELECT count(*)") {
                        send(
                            &mut stream,
                            b'D',
                            b"\0\x02\0\0\0\x01\x30\0\0\0\x132026-01-01 00:00:00",
                        );
                    }
                    send(&mut stream, b'C', b"OK\0");
                }
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
        match reply {
            CopyReply::Complete => assert!(result.is_ok(), "{result:?}"),
            CopyReply::Error => {
                assert!(format!("{:#}", result.unwrap_err()).contains("synthetic COPY failure"))
            }
            CopyReply::Short => {
                assert!(format!("{:#}", result.unwrap_err()).contains("COPY acknowledged 1 of"))
            }
        }
        queries
    }

    fn assert_locked_stage(queries: &[String]) {
        assert_eq!(queries.first().map(String::as_str), Some("BEGIN"));
        assert_eq!(
            queries.get(1).map(String::as_str),
            Some("LOCK TABLE config, configsave IN SHARE ROW EXCLUSIVE MODE")
        );
        assert!(queries[2].ends_with("FROM config"));
        assert_eq!(
            queries
                .iter()
                .filter(|sql| sql.starts_with("COPY configsave "))
                .count(),
            1
        );
        assert!(
            !queries
                .iter()
                .any(|sql| sql.starts_with("INSERT INTO configsave"))
        );
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
        assert_locked_stage(&wire_queries(CopyReply::Complete, |config| {
            import(config, &args)
        }));
    }

    #[test]
    fn a_cf_load_locks_both_tables_before_reading_its_base() {
        let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/file-infobase/tiny.cf");
        let queries = wire_queries(CopyReply::Complete, |config| {
            let report = load_cf(config, &input)?;
            assert_eq!(report.operation, "infobase config load");
            assert_eq!(report.format, "cf");
            Ok(report)
        });
        assert_locked_stage(&queries);
    }

    #[test]
    fn a_rejected_copy_rolls_back_the_stage_replacement() {
        let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/file-infobase/tiny.cf");
        for reply in [CopyReply::Error, CopyReply::Short] {
            let queries = wire_queries(reply, |config| load_cf(config, &input));
            assert!(queries.iter().any(|sql| sql == "DELETE FROM configsave"));
            assert_eq!(queries.last().map(String::as_str), Some("ROLLBACK"));
            assert!(!queries.iter().any(|sql| sql == "COMMIT"));
        }
    }

    #[test]
    fn copy_csv_preserves_a_quoted_multiline_name_and_bytea() {
        let row = ConfigRow {
            file_name: "a,\"'\\b\nc".into(),
            part_no: 1,
            creation: Timestamp::now_utc(),
            modified: Timestamp::now_utc(),
            attributes: 0,
            data_size: 20,
            binary: vec![0, 0xab],
        };
        assert_eq!(
            copy_row(&row, "2026-01-01 00:00:00"),
            b"\"a,\"\"'\\b\nc\",2026-01-01 00:00:00,2026-01-01 00:00:00,0,20,\\x00ab,1\n"
        );
    }
}
