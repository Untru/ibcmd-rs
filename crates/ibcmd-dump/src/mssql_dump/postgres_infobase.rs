//! The export of an infobase on PostgreSQL (`--dbms=PostgreSQL`, #369): the
//! rows of its `config` table, with a completed import stage of
//! `configsave` folded in as for a file infobase, become the offline rows
//! every read of the export answers from.
//!
//! The tables are the SQL Server ones under lower-case names (8.5.1.1529
//! `infobase create --dbms=PostgreSQL`): `config`, `configsave`, columns
//! `filename` (1C's `mvarchar`), `creation`, `modified`, `attributes`,
//! `datasize`, `binarydata` (`bytea`), `partno`; the stored bytes are the
//! same raw-deflate rows.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, bail};

use super::offline_rows::OfflineRows;
use crate::sql::postgres::{PgConnection, PgTarget, decode_bytea};

/// A single export operation's retained PostgreSQL rows. Clones share the same
/// single-use slot; neither a target key nor credentials enter shared state.
#[derive(Clone)]
pub struct PostgresExportSnapshot {
    rows: Arc<Mutex<Option<Arc<OfflineRows>>>>,
}

impl std::fmt::Debug for PostgresExportSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresExportSnapshot")
            .finish_non_exhaustive()
    }
}

impl PostgresExportSnapshot {
    fn new(rows: Arc<OfflineRows>) -> Self {
        Self {
            rows: Arc::new(Mutex::new(Some(rows))),
        }
    }

    fn take_rows(&self) -> Result<Option<Arc<OfflineRows>>> {
        Ok(self
            .rows
            .lock()
            .map_err(|_| anyhow::anyhow!("PostgreSQL snapshot lock is poisoned"))?
            .take())
    }

    /// Move the snapshot out of a resolved connection before export preflight.
    /// All connection clones then lose access, including when preflight fails.
    #[doc(hidden)]
    pub fn transfer(&self) -> Result<Option<Self>> {
        Ok(self.take_rows()?.map(Self::new))
    }

    #[cfg(test)]
    pub(crate) fn synthetic(label: &[u8]) -> Self {
        Self::new(Arc::new(OfflineRows::from_memory([(
            "root".to_owned(),
            Arc::new(label.to_vec()),
        )])))
    }
}

/// Read one snapshot for this probe and retain it only in its returned carrier.
pub(super) fn load_for_probe(
    target: &PgTarget,
) -> Result<(Arc<OfflineRows>, PostgresExportSnapshot)> {
    let rows = Arc::new(load(target, false)?);
    let snapshot = PostgresExportSnapshot::new(Arc::clone(&rows));
    Ok((rows, snapshot))
}

/// Consume this export's snapshot once, or read a fresh one when it had no probe.
pub(super) fn load_published(
    target: &PgTarget,
    snapshot: Option<&PostgresExportSnapshot>,
) -> Result<Arc<OfflineRows>> {
    if let Some(snapshot) = snapshot
        && let Some(rows) = snapshot.take_rows()?
    {
        return Ok(rows);
    }
    Ok(Arc::new(load(target, false)?))
}

/// The configuration the database publishes, as offline rows; with
/// `stored_only` its `config` table alone (`config save --db`).
pub(crate) fn load(target: &PgTarget, stored_only: bool) -> Result<OfflineRows> {
    let mut connection = PgConnection::connect(target)?;
    connection.query(
        "BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY",
        &mut |_| Ok(()),
    )?;
    // CONFIG, the CONFIGSAVE census and its lazy payload must belong to the
    // same database generation even when an import/apply commits meanwhile.
    match read_published(&mut connection, stored_only) {
        Ok(rows) => {
            connection.query("COMMIT", &mut |_| Ok(()))?;
            Ok(rows)
        }
        Err(error) => {
            let _ = connection.query("ROLLBACK", &mut |_| Ok(()));
            Err(error)
        }
    }
}

fn read_published(connection: &mut PgConnection, stored_only: bool) -> Result<OfflineRows> {
    let config = read_table(connection, "config")?;
    if stored_only {
        return super::file_infobase::publish(config, &BTreeSet::new(), || Ok(BTreeMap::new()));
    }
    let mut saved_names = BTreeSet::new();
    connection.query(
        "SELECT DISTINCT filename::text FROM configsave",
        &mut |row| {
            saved_names.insert(text(&row, 0)?);
            Ok(())
        },
    )?;
    super::file_infobase::publish(config, &saved_names, || {
        read_table(connection, "configsave")
    })
}

/// Every row of `table`, its parts joined.
fn read_table(connection: &mut PgConnection, table: &str) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut rows = Vec::new();
    connection
        .query(
            &format!("SELECT filename::text, partno, encode(binarydata, 'hex') FROM {table}"),
            &mut |row| {
                let name = text(&row, 0)?;
                let part: u32 = text(&row, 1)?
                    .parse()
                    .with_context(|| format!("{table}: part number of {name}"))?;
                let binary = match row.get(2).and_then(Option::as_deref) {
                    Some(value) => decode_bytea(value)?,
                    None => Vec::new(),
                };
                rows.push((name, part, binary));
                Ok(())
            },
        )
        .with_context(|| format!("failed to read the {table} table"))?;
    super::file_infobase::assemble_parts(rows, table)
}

fn text(row: &[Option<Vec<u8>>], column: usize) -> Result<String> {
    match row.get(column) {
        Some(Some(value)) => Ok(String::from_utf8(value.clone())?),
        Some(None) => bail!("NULL in column {column}"),
        None => bail!("no column {column}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_export_publishes_the_rows_its_platform_probe_read() {
        let target = PgTarget::new("127.0.0.1:1", "probed", None, None).unwrap();
        let rows = Arc::new(OfflineRows::from_memory([(
            "root".to_owned(),
            Arc::new(b"probe".to_vec()),
        )]));
        let snapshot = PostgresExportSnapshot::new(Arc::clone(&rows));
        let transferred = snapshot.transfer().unwrap().unwrap();
        // Port 1 refuses a connection: the rows can only be the probe's.
        let published = load_published(&target, Some(&transferred)).unwrap();
        assert!(Arc::ptr_eq(&published, &rows));
        assert!(snapshot.take_rows().unwrap().is_none());
        assert!(load_published(&target, Some(&transferred)).is_err());
    }
    fn snapshot_isolation_target() -> PgTarget {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        PgTarget::new(&address.to_string(), "isolated", None, None).unwrap()
    }

    fn snapshot_isolation_rows(label: &[u8]) -> Arc<OfflineRows> {
        Arc::new(OfflineRows::from_memory([(
            "root".to_owned(),
            Arc::new(label.to_vec()),
        )]))
    }

    #[test]
    fn snapshot_isolation_explicit_export_cannot_reuse_another_commands_probe() {
        let target = snapshot_isolation_target();
        let previous = snapshot_isolation_rows(b"previous command");
        let previous = PostgresExportSnapshot::new(previous);
        assert!(
            load_published(&target, None).is_err(),
            "an explicit-platform command must read its own database snapshot"
        );
        assert!(previous.take_rows().unwrap().is_some());
    }

    #[test]
    fn snapshot_isolation_failed_probe_cannot_publish_previous_rows() {
        let target = snapshot_isolation_target();
        let previous = snapshot_isolation_rows(b"previous command");
        let previous = PostgresExportSnapshot::new(previous);
        assert!(load_for_probe(&target).is_err());
        assert!(
            load_published(&target, None).is_err(),
            "a failed probe must not leave a previous command's rows eligible"
        );
        assert!(previous.take_rows().unwrap().is_some());
    }

    #[test]
    fn snapshot_isolation_two_pending_probes_keep_their_own_rows() {
        let first_target = snapshot_isolation_target();
        let mut second_target = first_target.clone();
        second_target.password = Some("different synthetic login".into());
        let first = snapshot_isolation_rows(b"first command");
        let second = snapshot_isolation_rows(b"second command");
        let first_snapshot = PostgresExportSnapshot::new(Arc::clone(&first));
        let second_snapshot = PostgresExportSnapshot::new(Arc::clone(&second));
        let published = load_published(&first_target, Some(&first_snapshot)).unwrap();
        assert!(
            Arc::ptr_eq(&published, &first),
            "a different pending operation must not replace the first snapshot"
        );
        let published = load_published(&second_target, Some(&second_snapshot)).unwrap();
        assert!(Arc::ptr_eq(&published, &second));
    }

    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    #[test]
    fn postgres_snapshot_transfer_keeps_one_database_transaction_and_same_rows() {
        let (ok, queries) = capture(false, false, true);
        assert!(ok);
        assert_eq!(queries.len(), 5);
        assert_eq!(
            queries.first().map(String::as_str),
            Some("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        );
        assert_eq!(queries.last().map(String::as_str), Some("COMMIT"));
    }

    #[test]
    fn postgres_snapshot_clones_consume_one_slot_and_debug_hides_payloads() {
        let snapshot = PostgresExportSnapshot::synthetic(b"synthetic private row");
        assert!(!format!("{snapshot:?}").contains("synthetic private row"));
        let clone = snapshot.clone();
        let transferred = snapshot.transfer().unwrap().unwrap();
        assert!(clone.transfer().unwrap().is_none());
        assert!(transferred.take_rows().unwrap().is_some());
        assert!(transferred.take_rows().unwrap().is_none());
    }

    fn message(stream: &mut TcpStream, tag: u8, body: &[u8]) {
        stream.write_all(&[tag]).unwrap();
        stream
            .write_all(&((body.len() + 4) as u32).to_be_bytes())
            .unwrap();
        stream.write_all(body).unwrap();
    }

    fn data_row(stream: &mut TcpStream, columns: &[&[u8]]) {
        let mut body = (columns.len() as u16).to_be_bytes().to_vec();
        for column in columns {
            body.extend_from_slice(&(column.len() as u32).to_be_bytes());
            body.extend_from_slice(column);
        }
        message(stream, b'D', &body);
    }

    fn capture(stored_only: bool, fail_config: bool, retain_snapshot: bool) -> (bool, Vec<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .unwrap();
            let mut length = [0; 4];
            stream.read_exact(&mut length).unwrap();
            let mut startup = vec![0; u32::from_be_bytes(length) as usize - 4];
            stream.read_exact(&mut startup).unwrap();
            message(&mut stream, b'R', &0u32.to_be_bytes());
            message(&mut stream, b'Z', b"I");
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
                let query = String::from_utf8(body[..body.len() - 1].to_vec()).unwrap();
                if query.starts_with("SELECT filename")
                    && query.ends_with("FROM config")
                    && fail_config
                {
                    message(&mut stream, b'E', b"SERROR\0Msynthetic read failure\0\0");
                } else {
                    if query.starts_with("SELECT DISTINCT") {
                        data_row(&mut stream, &[b"versions"]);
                    } else if query.ends_with("FROM configsave") {
                        // An invalid inventory still exercises the lazy payload read;
                        // publication safely falls back to the stored configuration.
                        data_row(&mut stream, &[b"versions", b"0", b"00"]);
                    }
                    message(&mut stream, b'C', b"OK\0");
                }
                message(&mut stream, b'Z', b"I");
                queries.push(query);
            }
            queries
        });
        let target = PgTarget::new(&address.to_string(), "synthetic", None, None).unwrap();
        let ok = if retain_snapshot {
            (|| -> Result<()> {
                let (rows, snapshot) = load_for_probe(&target)?;
                let clone = snapshot.clone();
                let transferred = snapshot.transfer()?.unwrap();
                assert!(clone.transfer()?.is_none());
                let published = load_published(&target, Some(&transferred))?;
                assert!(Arc::ptr_eq(&published, &rows));
                assert!(transferred.take_rows()?.is_none());
                Ok(())
            })()
            .is_ok()
        } else {
            load(&target, stored_only).is_ok()
        };
        (ok, server.join().unwrap())
    }

    #[test]
    fn postgres_export_uses_one_read_only_repeatable_read_snapshot() {
        let (ok, queries) = capture(false, false, false);
        assert!(ok);
        assert_eq!(
            queries.first().map(String::as_str),
            Some("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        );
        assert!(queries[1].ends_with("FROM config"));
        assert_eq!(queries[2], "SELECT DISTINCT filename::text FROM configsave");
        assert!(queries[3].ends_with("FROM configsave"));
        assert_eq!(queries.last().map(String::as_str), Some("COMMIT"));
        assert_eq!(queries.len(), 5);
    }

    #[test]
    fn postgres_stored_only_still_uses_a_read_only_snapshot() {
        let (ok, queries) = capture(true, false, false);
        assert!(ok);
        assert_eq!(
            queries.first().map(String::as_str),
            Some("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        );
        assert_eq!(queries.last().map(String::as_str), Some("COMMIT"));
        assert_eq!(queries.len(), 3);
    }

    #[test]
    fn postgres_read_failure_rolls_back_without_publishing() {
        let (ok, queries) = capture(false, true, false);
        assert!(!ok);
        assert_eq!(queries.last().map(String::as_str), Some("ROLLBACK"));
    }
}
