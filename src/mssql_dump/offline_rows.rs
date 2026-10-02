//! `mssql-dump-config --rows-dir`: the Config table read from a folder of
//! stored rows instead of SQL Server.
//!
//! The folder holds one `<FileName>__part<N>.bin` file per stored part, the
//! BinaryData column exactly as the table keeps it (raw deflate for most
//! rows) -- the format of `F:\ibcmd\lab\rawrows\*\Config` and of the row sets
//! `audit-empty-stage --rows-out` writes. While a folder is active every
//! Config read of the export is answered from it; any other query the export
//! would send to the server is refused, so an offline run never touches SQL.
//!
//! The folder is listed once. A query looks its names up (or walks the rows
//! once when it filters by shape), and the selected parts are read in
//! parallel, in the order the query returns them.
//!
//! The rows may as well be held in memory ([`OfflineRows::from_memory`]):
//! the state a stage would leave in the storage, built by
//! [`OfflineRows::with_staged`], is exported through the same reads without a
//! folder on disk.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use anyhow::{Context, Result, anyhow, bail};
use rayon::prelude::*;

use super::config_rows::{BinaryConfigRow, ConfigRowHeader};
use super::dynamic_generation::{
    StorageGenerationOverlay, dynamic_generation_history, is_dynamic_generation_alias,
    storage_generation_overlay, storage_generation_overlay_for,
};

/// Where the bytes of one stored part are.
#[derive(Debug, Clone)]
enum PartSource {
    File(PathBuf),
    Memory(Arc<Vec<u8>>),
}

/// One stored part.
#[derive(Debug, Clone)]
struct StoredPart {
    source: PartSource,
    bytes: u64,
}

impl StoredPart {
    fn memory(bytes: Arc<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.len() as u64,
            source: PartSource::Memory(bytes),
        }
    }

    fn read(&self) -> Result<Vec<u8>> {
        match &self.source {
            PartSource::File(path) => {
                fs::read(path).with_context(|| format!("failed to read {}", path.display()))
            }
            PartSource::Memory(bytes) => Ok(bytes.as_ref().clone()),
        }
    }
}

/// The parts of one stored row, in part order (0, 1, ...).
type Parts = Vec<StoredPart>;

/// The rows of one folder, by stored file name.
#[derive(Debug)]
pub(super) struct OfflineRows {
    rows: BTreeMap<String, Parts>,
    /// The published view under the last overlay a query saw.
    overlay_view: Mutex<Option<Arc<OverlayView>>>,
}

/// The names a table publishes while a dynamic generation's overlay is
/// installed: its aliases under their published names, without the rows they
/// replace and without any other generation's aliases, as the SQL view reads
/// it. Published name -> stored name.
#[derive(Debug)]
struct OverlayView {
    table: String,
    overlay: Arc<StorageGenerationOverlay>,
    published: BTreeMap<String, String>,
}

/// How a query sees the table.
enum View {
    /// No overlay: the stored names are the published names.
    Plain,
    Overlay(Arc<OverlayView>),
}

static ACTIVE: RwLock<Option<Arc<OfflineRows>>> = RwLock::new(None);

/// Keeps a folder active; the export reads SQL again once it is dropped.
pub(super) struct OfflineRowsGuard {
    _private: (),
}

impl Drop for OfflineRowsGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = ACTIVE.write() {
            *active = None;
        }
    }
}

/// Makes `dir` the Config table of this process until the guard drops.
pub(super) fn activate(dir: &Path) -> Result<OfflineRowsGuard> {
    activate_rows(OfflineRows::load(dir)?)
}

/// Makes `rows` the Config table of this process until the guard drops.
pub(super) fn activate_rows(rows: OfflineRows) -> Result<OfflineRowsGuard> {
    let mut active = ACTIVE
        .write()
        .map_err(|_| anyhow!("offline rows lock is poisoned"))?;
    *active = Some(Arc::new(rows));
    Ok(OfflineRowsGuard { _private: () })
}

/// The active folder, if an offline export is running.
pub(super) fn active() -> Option<Arc<OfflineRows>> {
    ACTIVE.read().ok()?.clone()
}

/// The error a query without an offline answer gets.
pub(super) fn refuse(what: &str) -> anyhow::Error {
    anyhow!(
        "offline export (--rows-dir) reads only the Config table from its folder; \
         it has no answer for {what}"
    )
}

/// `<FileName>__part<N>.bin` -> (FileName, N).
fn parse_part_file_name(name: &str) -> Option<(String, i32)> {
    let stem = name.strip_suffix(".bin")?;
    let (file_name, part) = stem.rsplit_once("__part")?;
    if file_name.is_empty() || part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((file_name.to_string(), part.parse().ok()?))
}

/// The stored bytes of one row: its parts concatenated in order.
fn read_parts(parts: &[StoredPart]) -> Result<Vec<u8>> {
    if let [part] = parts {
        return part.read();
    }
    let mut binary = Vec::with_capacity(parts.iter().map(|part| part.bytes).sum::<u64>() as usize);
    for part in parts {
        binary.extend(part.read()?);
    }
    Ok(binary)
}

impl OfflineRows {
    /// The rows of a folder of `<FileName>__part<N>.bin` files.
    pub(super) fn load(dir: &Path) -> Result<Self> {
        let mut found = BTreeMap::<String, BTreeMap<i32, StoredPart>>::new();
        let entries =
            fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))?;
        for entry in entries {
            let entry = entry.with_context(|| format!("failed to read {}", dir.display()))?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let name = entry.file_name();
            let Some((file_name, part_no)) = name.to_str().and_then(parse_part_file_name) else {
                continue;
            };
            let bytes = entry.metadata()?.len();
            found.entry(file_name).or_default().insert(
                part_no,
                StoredPart {
                    source: PartSource::File(entry.path()),
                    bytes,
                },
            );
        }
        if found.is_empty() {
            bail!("{} holds no <FileName>__part<N>.bin rows", dir.display());
        }
        let mut rows = BTreeMap::new();
        for (file_name, parts) in found {
            for (expected, part_no) in parts.keys().enumerate() {
                if *part_no != expected as i32 {
                    bail!(
                        "{}: row {file_name} has part {part_no} where part {expected} was expected",
                        dir.display()
                    );
                }
            }
            rows.insert(file_name, parts.into_values().collect());
        }
        Ok(Self {
            rows,
            overlay_view: Mutex::new(None),
        })
    }

    /// Rows held in memory, one part each: file name -> stored bytes.
    pub(super) fn from_memory(rows: impl IntoIterator<Item = (String, Arc<Vec<u8>>)>) -> Self {
        Self {
            rows: rows
                .into_iter()
                .map(|(file_name, bytes)| (file_name, vec![StoredPart::memory(bytes)]))
                .collect(),
            overlay_view: Mutex::new(None),
        }
    }

    /// The number of stored rows.
    pub(super) fn len(&self) -> usize {
        self.rows.len()
    }

    /// The configuration a stage of `staged` rows leaves in this storage once
    /// it is applied: what the storage publishes (an active dynamic
    /// generation's aliases under their plain names, see
    /// [`dynamic_generation`]), with each staged row in place of the stored
    /// row of its name. The apply drops the aliases of what it replaces, so a
    /// staged row is never shadowed by one; the generation history itself is
    /// left out, since nothing is aliased any more. `removed` are the names the
    /// stage's `deleted` row lists: the apply removes those rows.
    pub(super) fn with_staged(
        self,
        staged: impl IntoIterator<Item = (String, Arc<Vec<u8>>)>,
        removed: &[String],
    ) -> Result<Self> {
        let mut rows = self.rows;
        if let Some(marker) = rows.remove(super::DYNAMIC_UPDATE_MARKER_ROW) {
            let history = dynamic_generation_history(&read_parts(&marker)?).ok_or_else(|| {
                anyhow!(
                    "{} is not a generation history",
                    super::DYNAMIC_UPDATE_MARKER_ROW
                )
            })?;
            let overlay = storage_generation_overlay(&history, rows.keys().map(String::as_str));
            let mut published = BTreeMap::new();
            for (file_name, parts) in std::mem::take(&mut rows) {
                if let Some(name) = overlay.published_name(&file_name) {
                    published.insert(name.to_string(), parts);
                } else if !is_dynamic_generation_alias(&file_name) && !overlay.hides(&file_name) {
                    published.insert(file_name, parts);
                }
            }
            rows = published;
        }
        for file_name in removed {
            rows.remove(file_name);
        }
        for (file_name, bytes) in staged {
            rows.insert(file_name, vec![StoredPart::memory(bytes)]);
        }
        // What the `versions` row of the state does not list is not part of
        // the configuration: the rows an online update that removed an object
        // left behind, as the export of a table leaves them out.
        if let Some(versions) = rows.get("versions")
            && let Ok(unlisted) = super::config_dump_info::unlisted_entries(
                &read_parts(versions)?,
                rows.keys().map(String::as_str),
            )
        {
            for file_name in unlisted {
                rows.remove(&file_name);
            }
        }
        Ok(Self {
            rows,
            overlay_view: Mutex::new(None),
        })
    }

    fn check_table(&self, table: &str) -> Result<()> {
        if table
            .trim_matches(['[', ']'])
            .eq_ignore_ascii_case("Config")
        {
            Ok(())
        } else {
            Err(refuse(&format!("the {table} table")))
        }
    }

    /// The view a query on `table` of `database` reads: the stored rows as
    /// they are, or the published view of the overlay installed for the table,
    /// built once per overlay and kept until another one is installed.
    fn view(&self, database: &str, table: &str) -> View {
        let Some(overlay) = storage_generation_overlay_for(database, table) else {
            return View::Plain;
        };
        let mut cached = self
            .overlay_view
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(view) = cached.as_ref()
            && view.table == table
            && view.overlay == overlay
        {
            return View::Overlay(view.clone());
        }
        let mut published = BTreeMap::new();
        for file_name in self.rows.keys() {
            if let Some(name) = overlay.published_name(file_name) {
                published.insert(name.to_string(), file_name.clone());
            } else if !is_dynamic_generation_alias(file_name) && !overlay.hides(file_name) {
                published.insert(file_name.clone(), file_name.clone());
            }
        }
        let view = Arc::new(OverlayView {
            table: table.to_string(),
            overlay,
            published,
        });
        *cached = Some(view.clone());
        View::Overlay(view)
    }

    /// The rows of a view whose published name passes `keep`, in published
    /// name order.
    fn select<'a>(
        &'a self,
        view: &'a View,
        keep: impl Fn(&str) -> bool,
    ) -> Vec<(&'a str, &'a Parts)> {
        match view {
            View::Plain => self
                .rows
                .iter()
                .filter(|(name, _)| keep(name))
                .map(|(name, parts)| (name.as_str(), parts))
                .collect(),
            View::Overlay(view) => view
                .published
                .iter()
                .filter(|(name, _)| keep(name))
                .filter_map(|(name, stored)| Some((name.as_str(), self.rows.get(stored)?)))
                .collect(),
        }
    }

    /// The rows of a view with these published names, looked up, in name
    /// order; a name the view lacks is skipped, as `WHERE FileName IN (...)`
    /// skips it.
    fn lookup<'a>(
        &'a self,
        view: &'a View,
        names: &'a BTreeSet<String>,
    ) -> Vec<(&'a str, &'a Parts)> {
        names
            .iter()
            .filter_map(|name| {
                let parts = match view {
                    View::Plain => self.rows.get(name)?,
                    View::Overlay(view) => self.rows.get(view.published.get(name)?)?,
                };
                Some((name.as_str(), parts))
            })
            .collect()
    }

    /// Reads and assembles the selected rows in parallel, keeping their order.
    fn read(selected: Vec<(&str, &Parts)>) -> Result<Vec<BinaryConfigRow>> {
        crate::parallel::install(|| {
            selected
                .par_iter()
                .map(|(file_name, parts)| {
                    let binary = read_parts(parts)?;
                    Ok(BinaryConfigRow {
                        file_name: file_name.to_string(),
                        part_no: 0,
                        data_size: binary.len() as i64,
                        binary,
                    })
                })
                .collect::<Result<Vec<_>>>()
        })?
    }

    /// `SELECT FileName, PartNo, DataSize ... ORDER BY FileName, PartNo`.
    pub(super) fn headers(
        &self,
        database: &str,
        table: &str,
        selected: &BTreeSet<String>,
    ) -> Result<Vec<ConfigRowHeader>> {
        self.check_table(table)?;
        let view = self.view(database, table);
        let rows = if selected.is_empty() {
            self.select(&view, |_| true)
        } else {
            self.lookup(&view, selected)
        };
        let mut headers = Vec::with_capacity(rows.len());
        for (file_name, parts) in rows {
            let data_size = parts.iter().map(|part| part.bytes).sum::<u64>() as i64;
            for part_no in 0..parts.len() {
                headers.push(ConfigRowHeader {
                    file_name: file_name.to_string(),
                    part_no: part_no as i32,
                    data_size,
                });
            }
        }
        Ok(headers)
    }

    /// The assembled rows (parts concatenated) whose published name passes
    /// `keep`, in file name order.
    pub(super) fn rows(
        &self,
        database: &str,
        table: &str,
        keep: impl Fn(&str) -> bool,
    ) -> Result<Vec<BinaryConfigRow>> {
        self.check_table(table)?;
        let view = self.view(database, table);
        Self::read(self.select(&view, keep))
    }

    /// Rows by exact name (all rows when `names` is empty); a name the folder
    /// lacks is skipped, as `WHERE FileName IN (...)` skips it.
    pub(super) fn rows_named(
        &self,
        database: &str,
        table: &str,
        names: &BTreeSet<String>,
    ) -> Result<Vec<BinaryConfigRow>> {
        self.check_table(table)?;
        let view = self.view(database, table);
        if names.is_empty() {
            return Self::read(self.select(&view, |_| true));
        }
        Self::read(self.lookup(&view, names))
    }

    /// Part 0 of every row, as stored (`fetch_config_part0_rows_bcp`).
    pub(super) fn part0_rows(&self) -> Result<HashMap<String, Vec<u8>>> {
        let parts = self
            .rows
            .iter()
            .filter_map(|(file_name, parts)| Some((file_name, parts.first()?)))
            .collect::<Vec<_>>();
        let read = crate::parallel::install(|| {
            parts
                .par_iter()
                .map(|(file_name, part)| Ok(((*file_name).clone(), part.read()?)))
                .collect::<Result<Vec<_>>>()
        })??;
        Ok(read.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    use super::{OfflineRows, parse_part_file_name, read_parts};

    const GENERATION: &str = "06cb0442-0c47-4fad-986a-f08f28287c1b";

    fn row(bytes: &[u8]) -> Arc<Vec<u8>> {
        Arc::new(bytes.to_vec())
    }

    fn names(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    /// The stored rows by name, without going through a table view (another
    /// test may have installed a generation overlay for `Config`).
    fn contents(rows: &OfflineRows) -> BTreeMap<String, Vec<u8>> {
        rows.rows
            .iter()
            .map(|(name, parts)| (name.clone(), read_parts(parts).unwrap()))
            .collect()
    }

    /// A storage an online update touched: `a.0` and `versions` have an alias
    /// holding the text the infobase reads, `c.0` too.
    fn generation_rows() -> OfflineRows {
        OfflineRows::from_memory([
            (
                "DynamicallyUpdated".to_string(),
                row(format!("{{1,1,{GENERATION}}}").as_bytes()),
            ),
            ("a".to_string(), row(b"a plain")),
            ("a.0".to_string(), row(b"a.0 before the online update")),
            (
                format!("a_dynupdate_{GENERATION}.0"),
                row(b"a.0 after the online update"),
            ),
            ("c.0".to_string(), row(b"c.0 before")),
            (format!("c_dynupdate_{GENERATION}.0"), row(b"c.0 after")),
            ("versions".to_string(), row(b"versions before")),
            (
                format!("versions_dynupdate_{GENERATION}"),
                row(b"versions after"),
            ),
            ("z".to_string(), row(b"untouched")),
        ])
    }

    #[test]
    fn part_file_names_split_at_the_last_part_marker() {
        assert_eq!(
            parse_part_file_name("0a1b.0__part0.bin"),
            Some(("0a1b.0".to_string(), 0))
        );
        assert_eq!(
            parse_part_file_name("x__party__part12.bin"),
            Some(("x__party".to_string(), 12))
        );
        assert_eq!(parse_part_file_name("versions.bin"), None);
        assert_eq!(parse_part_file_name("root__part0.txt"), None);
    }

    #[test]
    fn rows_held_in_memory_answer_the_reads_a_folder_does() {
        let rows = OfflineRows::from_memory([
            ("a".to_string(), row(b"first")),
            ("b.0".to_string(), row(b"second")),
        ]);
        assert_eq!(rows.len(), 2);
        let read = rows
            .rows_named("", "Config", &names(&["b.0", "missing"]))
            .unwrap();
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].file_name, "b.0");
        assert_eq!(read[0].binary, b"second");
        assert_eq!(read[0].data_size, 6);
        let headers = rows.headers("", "Config", &BTreeSet::new()).unwrap();
        assert_eq!(
            headers
                .iter()
                .map(|header| (header.file_name.as_str(), header.part_no, header.data_size))
                .collect::<Vec<_>>(),
            vec![("a", 0, 5), ("b.0", 0, 6)]
        );
        let metadata = rows.rows("", "Config", |name| !name.contains('.')).unwrap();
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata[0].file_name, "a");
        assert_eq!(
            rows.part0_rows().unwrap().get("a").map(Vec::as_slice),
            Some(&b"first"[..])
        );
        assert!(
            rows.rows_named("", "ConfigSave", &names(&["a"])).is_err(),
            "only the Config table is served"
        );
    }

    #[test]
    fn a_staged_row_takes_the_place_of_the_row_and_of_its_alias() {
        let state = generation_rows()
            .with_staged(
                [
                    ("a.0".to_string(), row(b"a.0 staged")),
                    ("versions".to_string(), row(b"versions staged")),
                    ("new.0".to_string(), row(b"a row the storage lacked")),
                ],
                &[],
            )
            .unwrap();
        let got = contents(&state);
        assert_eq!(got["a.0"], b"a.0 staged");
        assert_eq!(got["versions"], b"versions staged");
        assert_eq!(got["new.0"], b"a row the storage lacked");
        // What the online update changed and the stage leaves alone stays as
        // the infobase reads it.
        assert_eq!(got["c.0"], b"c.0 after");
        assert_eq!(got["a"], b"a plain");
        assert_eq!(got["z"], b"untouched");
        // Neither an alias nor the history is left.
        assert_eq!(
            got.keys().map(String::as_str).collect::<Vec<_>>(),
            ["a", "a.0", "c.0", "new.0", "versions", "z"]
        );
    }

    #[test]
    fn a_storage_without_a_generation_is_only_overlaid() {
        let rows = OfflineRows::from_memory([
            ("a".to_string(), row(b"old")),
            ("b".to_string(), row(b"kept")),
        ]);
        let state = rows
            .with_staged([("a".to_string(), row(b"new"))], &[])
            .unwrap();
        let got = contents(&state);
        assert_eq!(got["a"], b"new");
        assert_eq!(got["b"], b"kept");
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn rows_the_versions_of_the_state_does_not_list_are_left_out() {
        // The generation and the two listed names; `stale` is a row an online
        // update that removed its object left behind.
        let generation = uuid::Uuid::new_v4().hyphenated().to_string();
        let a = uuid::Uuid::new_v4().hyphenated().to_string();
        let b = uuid::Uuid::new_v4().hyphenated().to_string();
        let versions = format!("\u{feff}{{1,3,\"\",{generation},\"a\",{a},\"a.0\",{b}}}");
        let versions = crate::module_blob::deflate_raw(versions.as_bytes()).unwrap();
        let rows = OfflineRows::from_memory([
            ("a".to_string(), row(b"a")),
            ("a.0".to_string(), row(b"a.0")),
            ("stale".to_string(), row(b"left behind")),
        ]);
        let state = rows
            .with_staged([("versions".to_string(), Arc::new(versions))], &[])
            .unwrap();
        let got = contents(&state);
        assert_eq!(
            got.keys().map(String::as_str).collect::<Vec<_>>(),
            ["a", "a.0", "versions"]
        );
    }

    #[test]
    fn the_rows_a_stage_deletes_are_not_in_the_state() {
        let rows = OfflineRows::from_memory([
            ("a".to_string(), row(b"old")),
            ("gone".to_string(), row(b"removed by the tree")),
            ("gone.0".to_string(), row(b"removed with it")),
            ("z".to_string(), row(b"kept")),
        ]);
        let state = rows
            .with_staged(
                [("a".to_string(), row(b"new"))],
                &[
                    "gone".to_string(),
                    "gone.0".to_string(),
                    "never there".to_string(),
                ],
            )
            .unwrap();
        let got = contents(&state);
        assert_eq!(
            got.keys().map(String::as_str).collect::<Vec<_>>(),
            ["a", "z"]
        );
    }

    #[test]
    fn an_unreadable_generation_history_is_an_error() {
        let rows = OfflineRows::from_memory([(
            "DynamicallyUpdated".to_string(),
            row(b"{1,2,not-a-uuid}"),
        )]);
        let error = rows
            .with_staged(Vec::<(String, Arc<Vec<u8>>)>::new(), &[])
            .unwrap_err();
        assert!(
            error.to_string().contains("generation history"),
            "{error:#}"
        );
    }

    /// One test at a time may hold a folder active.
    static FOLDER: Mutex<()> = Mutex::new(());

    const BASE_GENERATION: &str = "848a0a59-8803-445f-b89d-3cfae4f98bbd";

    fn versions_row(names: &[&str], generation: &str) -> Vec<u8> {
        let version = "00000000-0000-0000-0000-000000000001";
        let mut text = format!("{{1,{},\"\",{generation}", names.len());
        for name in names {
            text.push_str(&format!(",\"{name}\",{version}"));
        }
        text.push('}');
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(text.as_bytes()).unwrap();
        encoder.finish().unwrap()
    }

    fn folder_of(rows: &[(String, Vec<u8>)]) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ibcmd-rs-offline-rows-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        for (name, bytes) in rows {
            std::fs::write(dir.join(format!("{name}__part0.bin")), bytes).unwrap();
        }
        dir
    }

    fn owned_rows(names: &[(&str, Vec<u8>)]) -> Vec<(String, Vec<u8>)> {
        names
            .iter()
            .map(|(name, bytes)| ((*name).to_owned(), bytes.clone()))
            .collect()
    }

    fn published_by(dir: &std::path::Path, main_configuration: bool) -> BTreeSet<String> {
        let _active = super::activate(dir).unwrap();
        let sql = crate::sql::SqlExec::detached("the rows come from a folder");
        let headers =
            crate::mssql_dump::fetch::fetch_row_headers(&sql, "db", "Config", &BTreeSet::new())
                .unwrap();
        // Resolved, not installed: the installed overlays are shared by the
        // tests of the process.
        crate::mssql_dump::resolve_storage_overlay(
            &sql,
            "db",
            "Config",
            &BTreeSet::new(),
            headers,
            main_configuration,
        )
        .unwrap()
        .1
        .into_iter()
        .map(|header| header.file_name)
        .collect()
    }

    /// A folder of rows takes the generation overlay and never looks for staged
    /// rows, whether or not the run asks for the main configuration.
    #[test]
    fn a_folder_of_rows_takes_the_generation_overlay_and_no_staged_rows() {
        let _one = FOLDER
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let alias = format!("a_dynupdate_{GENERATION}");
        let published_versions = format!("versions_dynupdate_{GENERATION}");
        let dir = folder_of(&owned_rows(&[
            ("a", vec![1]),
            (&alias, vec![2]),
            ("b", vec![3]),
            ("gone", vec![4]),
            (
                "versions",
                versions_row(&["a", "b", "gone"], BASE_GENERATION),
            ),
            (&published_versions, versions_row(&["a", "b"], GENERATION)),
            (
                "DynamicallyUpdated",
                format!("\u{feff}{{1,1,{GENERATION}}}").into_bytes(),
            ),
        ]));
        for main_configuration in [false, true] {
            assert_eq!(
                published_by(&dir, main_configuration),
                names(&["DynamicallyUpdated", "a", "b", "versions"]),
                "the alias is published; its plain row and the unlisted `gone` are not"
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// No marker: nothing moves, aliases of unlisted generations included.
    #[test]
    fn a_folder_of_rows_without_a_marker_keeps_every_row() {
        let _one = FOLDER
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let alias = format!("a_dynupdate_{GENERATION}");
        let dir = folder_of(&owned_rows(&[
            ("a", vec![1]),
            (&alias, vec![2]),
            ("gone", vec![4]),
            ("versions", versions_row(&["a"], BASE_GENERATION)),
        ]));
        for main_configuration in [false, true] {
            assert_eq!(
                published_by(&dir, main_configuration),
                names(&["a", &alias, "gone", "versions"])
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The rows of an object an online update changed: the plain rows keep the
    /// text before it, the aliases hold the text the infobase reads.
    fn aliased_object_folder() -> std::path::PathBuf {
        let object = "a627e390-8fad-4a95-afe6-674f54813188";
        folder_of(&owned_rows(&[
            (object, vec![1]),
            (&format!("{object}.0"), vec![1, 1]),
            (&format!("{object}_dynupdate_{GENERATION}"), vec![2, 2, 2]),
            (
                &format!("{object}_dynupdate_{GENERATION}.0"),
                vec![2, 2, 2, 2],
            ),
            ("other", vec![5]),
            (
                "versions",
                versions_row(&[object, &format!("{object}.0"), "other"], BASE_GENERATION),
            ),
            (
                &format!("versions_dynupdate_{GENERATION}"),
                versions_row(&[object, &format!("{object}.0"), "other"], GENERATION),
            ),
            (
                "DynamicallyUpdated",
                format!("\u{feff}{{1,1,{GENERATION}}}").into_bytes(),
            ),
        ]))
    }

    /// #409 F-1: a run that selects an object by name gets the object's alias
    /// rows under its published names, not nothing.
    #[test]
    fn a_selected_run_lists_the_alias_rows_of_an_aliased_object() {
        let _one = FOLDER
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let object = "a627e390-8fad-4a95-afe6-674f54813188";
        let dir = aliased_object_folder();
        let active = super::activate(&dir).unwrap();
        let sql = crate::sql::SqlExec::detached("the rows come from a folder");
        let selected = names(&[object, &format!("{object}.0")]);
        let headers =
            crate::mssql_dump::fetch::fetch_row_headers(&sql, "f1_selected", "Config", &selected)
                .unwrap();
        let (_, published) = crate::mssql_dump::resolve_storage_overlay(
            &sql,
            "f1_selected",
            "Config",
            &selected,
            headers,
            false,
        )
        .unwrap();
        let mut got = published
            .iter()
            .map(|header| (header.file_name.clone(), header.data_size))
            .collect::<Vec<_>>();
        got.sort();
        drop(active);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            got,
            vec![(object.to_owned(), 3), (format!("{object}.0"), 4)],
            "the selected object is published from its alias rows"
        );
    }

    /// #409 F-2: the overlay one database installs is not the view of another
    /// database that has the same table.
    #[test]
    fn an_installed_overlay_is_the_view_of_its_database_only() {
        let _one = FOLDER
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = aliased_object_folder();
        let active = super::activate(&dir).unwrap();
        let views = crate::mssql_dump::dynamic_generation::StorageViewScope::begin("f2_installed");
        let sql = crate::sql::SqlExec::detached("the rows come from a folder");
        let headers = crate::mssql_dump::fetch::fetch_row_headers(
            &sql,
            "f2_installed",
            "Config",
            &BTreeSet::new(),
        )
        .unwrap();
        crate::mssql_dump::install_storage_overlay(
            &sql,
            "f2_installed",
            "Config",
            &BTreeSet::new(),
            headers,
            false,
        )
        .unwrap();
        let own = crate::mssql_dump::qualified_storage_table("f2_installed", "Config");
        let other = crate::mssql_dump::qualified_storage_table("f2_other", "Config");
        drop(views);
        drop(active);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(
            own.contains("_dynupdate_"),
            "its own view reads the generation: {own}"
        );
        assert_eq!(
            other, "[f2_other].dbo.[Config]",
            "another database reads its plain table"
        );
    }

    /// #409 F-2: an export does not leave its overlay behind for the reads
    /// that follow it in the same process (the activation's among them).
    #[test]
    fn an_export_leaves_no_overlay_behind() {
        let _one = FOLDER
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = aliased_object_folder();
        let out = std::env::temp_dir().join(format!(
            "ibcmd-rs-f2-export-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let result = crate::mssql_dump::dump_config(&crate::cli::MssqlDumpConfigArgs {
            sqlcmd: None,
            bcp_executable: None,
            runtime_journal: None,
            server: "localhost".to_owned(),
            sql_user: None,
            sql_pwd: None,
            sql_pwd_env: "IBCMD_DB_PSW".to_owned(),
            database: "f2_export".to_owned(),
            rows_dir: Some(dir.clone()),
            model_export: false,
            legacy_export: false,
            output_dir: out.clone(),
            overwrite: false,
            include_config_save: false,
            main_configuration: false,
            file_names: Vec::new(),
            file_name_lists: Vec::new(),
            objects: Vec::new(),
            inflate: false,
            extract_module_text: false,
            extract_metadata_xml: false,
            require_complete_root_metadata: false,
            require_complete_source_assets: false,
            collect_all_source_asset_diagnostics: false,
            platform: None,
            source_version: crate::legacy_version::InfobaseConfigSourceVersion::V2_20,
            no_binary_rows: true,
            write_binary_rows: false,
            write_manifest: false,
            base: None,
            sync: false,
        });
        let after = crate::mssql_dump::qualified_storage_table("f2_export", "Config");
        std::fs::remove_dir_all(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&out);
        result.unwrap();
        assert_eq!(
            after, "[f2_export].dbo.[Config]",
            "the export's overlay must not outlive the export"
        );
    }
}
