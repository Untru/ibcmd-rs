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

pub(crate) fn import_edt(
    config: &ConnectionConfig,
    source: std::path::PathBuf,
    version: crate::legacy_version::InfobaseConfigSourceVersion,
    stage: crate::file_infobase::Stage,
) -> Result<InfobaseConfigImportReport> {
    let target = config.postgres()?.context("no PostgreSQL database")?;
    let mut connection = PgConnection::connect(&target)?;
    begin_stage(&mut connection)?;
    if !read_rows(&mut connection, "config")?.is_empty() {
        anyhow::bail!("EDT staging currently requires empty CONFIG");
    }
    let before = write_stage(&mut connection, &stage.rows)
        .with_context(|| format!("не удалось записать configsave базы {}", target.database))?;
    let after = stage.rows.len();
    let mut report = stage.report(
        config,
        source,
        Some(version),
        "postgres-configsave-stage",
        (before, after),
        None,
    );
    report.format = "edt";
    Ok(report)
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
        let mut before = 0;
        connection.query("SELECT count(*) FROM configsave", &mut |row| {
            before = row
                .first()
                .and_then(Option::as_deref)
                .and_then(|value| std::str::from_utf8(value).ok()?.parse().ok())
                .unwrap_or(0);
            Ok(())
        })?;
        connection.query("DELETE FROM configsave", &mut |_| Ok(()))?;
        for row in rows {
            connection.query(&insert_statement(row), &mut |_| Ok(()))?;
        }
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
            loop {
                let mut header = [0; 5];
                stream.read_exact(&mut header).unwrap();
                let mut body =
                    vec![0; u32::from_be_bytes(header[1..].try_into().unwrap()) as usize - 4];
                stream.read_exact(&mut body).unwrap();
                if header[0] == b'X' {
                    break;
                }
                assert_eq!(header[0], b'Q');
                queries.push(String::from_utf8(body[..body.len() - 1].to_vec()).unwrap());
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
        assert_locked_stage(&wire_queries(|config| import(config, &args)));
    }

    #[test]
    fn a_cf_load_locks_both_tables_before_reading_its_base() {
        let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/file-infobase/tiny.cf");
        let queries = wire_queries(|config| {
            let report = load_cf(config, &input)?;
            assert_eq!(report.operation, "infobase config load");
            assert_eq!(report.format, "cf");
            Ok(report)
        });
        assert_locked_stage(&queries);
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
