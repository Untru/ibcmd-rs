//! File infobases (`1Cv8.1CD`, format 8.3.8) without the platform.
//!
//! Reading: [`FileDb::open`] and [`FileDb::config_rows`] give the rows of the
//! configuration tables (`CONFIG`, `CONFIGSAVE`, `PARAMS`, …).
//!
//! Writing: [`open_for_write`] locks a closed infobase and holds the lock
//! until [`WriteSession::replace`] has replaced every row of one configuration
//! table (`CONFIGSAVE` for an import). The change is made in a copy, read back
//! and checked, the original is kept as `<data file>.ibcmd-rs.bak` and the copy
//! takes its place by a rename.
//!
//! The format evidence (page, object, blob, record and index layouts measured
//! on files written by 1C:Enterprise 8.5.1.1150) is in
//! `openspec/changes/add-file-infobase-backend/evidence/1cd-format.md`.

#![deny(unsafe_code)]

mod blob;
mod config;
mod descr;
mod error;
mod extensions;
pub mod index;
mod lock;
mod object;
mod pager;
mod read_limits;
mod record;
#[cfg(windows)]
mod windows_security;
mod write_paths;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub use config::ConfigRow;
pub use descr::{Field, FieldKind, Table};
pub use error::{Error, Result};
pub use extensions::ExtensionRegistryRow;
pub use index::Collation;
pub use read_limits::ReadLimits;
pub use record::Timestamp;

use lock::Mode;
use pager::Pager;

/// The data file inside an infobase directory.
pub const DATA_FILE: &str = "1Cv8.1CD";
/// The suffix of the copy of the data file kept before the last write.
pub const BACKUP_SUFFIX: &str = ".ibcmd-rs.bak";
const WORK_SUFFIX: &str = ".ibcmd-rs.tmp";

/// The tables a write may replace.
pub const CONFIG_TABLES: [&str; 6] = [
    "CONFIG",
    "CONFIGSAVE",
    "PARAMS",
    "FILES",
    "CONFIGCAS",
    "CONFIGCASSAVE",
];

/// Resolves `--db-path`: the directory holding `1Cv8.1CD`, or the file itself
/// when the path names a `.1CD` file.
pub fn data_file(db_path: &Path) -> PathBuf {
    let names_file = !db_path.is_dir()
        && db_path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("1cd"));
    if names_file {
        db_path.to_path_buf()
    } else {
        db_path.join(DATA_FILE)
    }
}

/// `<data file><suffix>` beside the data file.
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().map(OsString::from).unwrap_or_default();
    name.push(suffix);
    path.with_file_name(name)
}

/// The copy [`WriteSession::replace`] keeps of the data file at `db_path`.
pub fn backup_file(db_path: &Path) -> PathBuf {
    sibling(&data_file(db_path), BACKUP_SUFFIX)
}

fn existing_data_file(db_path: &Path) -> Result<PathBuf> {
    let path = data_file(db_path);
    if !path.is_file() {
        return Err(Error::Refused(format!(
            "Файловая информационная база не найдена: нет файла {}",
            path.display()
        )));
    }
    Ok(path)
}

/// An open file infobase, read-locked for the lifetime of the value.
pub struct FileDb {
    pager: Pager<'static>,
    locale: String,
    tables: BTreeMap<String, Table>,
    metadata: fs::Metadata,
}

impl FileDb {
    /// Opens `1Cv8.1CD` (or the directory holding it) for reading.
    pub fn open(db_path: &Path) -> Result<FileDb> {
        let path = existing_data_file(db_path)?;
        let file = lock::open_locked(&path, Mode::Read)?;
        Self::from_locked_file(file)
    }

    /// Reads the caller's exact descriptor, without reopening its pathname.
    /// On Unix acquires the same read lock as open(). Other OSes are refused
    /// until descriptor sharing/identity has been measured.
    pub fn from_file(file: File) -> Result<FileDb> {
        lock::lock_read_descriptor(&file)?;
        Self::from_locked_file(file)
    }

    /// Metadata of the descriptor consumed by the reader, for target identity checks.
    pub fn metadata(&self) -> &fs::Metadata {
        &self.metadata
    }

    /// Read an exact descriptor under cumulative materialization limits. The
    /// callback runs before metadata/object/row reads and within long blob walks.
    pub fn from_file_with_limits(
        file: File,
        limits: ReadLimits,
        check: impl Fn() -> Result<()> + Send + Sync + 'static,
    ) -> Result<FileDb> {
        check()?;
        lock::lock_read_descriptor(&file)?;
        Self::from_locked_file_budget(file, Some(read_limits::ReadBudget::new(limits, check)))
    }

    fn from_locked_file(file: File) -> Result<FileDb> {
        Self::from_locked_file_budget(file, None)
    }
    fn from_locked_file_budget(
        file: File,
        budget: Option<read_limits::ReadBudget>,
    ) -> Result<FileDb> {
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(Error::Refused(
                "file database descriptor is not a regular file".into(),
            ));
        }
        let mut pager = Pager::new(file)?;
        pager.set_read_budget(budget);
        let (locale, tables) = descr::read_tables(&mut pager)?;
        Ok(FileDb {
            pager,
            locale,
            tables,
            metadata,
        })
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// The collation of the infobase, when it is one the keys are known for.
    pub fn collation(&self) -> Option<Collation> {
        Collation::for_locale(&self.locale)
    }

    pub fn page_size(&self) -> usize {
        self.pager.page_size()
    }

    pub fn page_count(&self) -> u32 {
        self.pager.page_count()
    }

    pub fn tables(&self) -> impl Iterator<Item = &Table> {
        self.tables.values()
    }

    /// The measured 8.3.27 extension registry, with next-generation states
    /// rejected before any image is selected.
    pub fn extension_registry(&mut self) -> Result<Vec<ExtensionRegistryRow>> {
        extensions::read_registry(&mut self.pager, &self.tables)
    }

    pub fn table(&self, name: &str) -> Option<&Table> {
        self.tables.get(name)
    }

    /// Every stored part of every row of a configuration table, in record
    /// order. Parts of one row are separate values with the same name.
    pub fn config_rows(&mut self, table: &str) -> Result<Vec<ConfigRow>> {
        read_rows(&mut self.pager, &self.tables, table, true)
    }

    /// Refuses oversized physical objects before allocating their contents.
    /// Free records and unused blob blocks count toward the physical budget.
    /// The same limit separately bounds retained binary values, including
    /// repeated references to a shared blob chain.
    pub fn config_rows_bounded(
        &mut self,
        name: &str,
        max_bytes: u64,
        max_rows: usize,
    ) -> Result<Vec<ConfigRow>> {
        let table = config_table(&self.tables, name)?;
        let records = object::Object::open(&mut self.pager, table.records_page)?;
        let blobs = object::Object::open(&mut self.pager, table.blob_page)?;
        let row_budget = max_rows
            .saturating_add(1)
            .saturating_mul(table.record_size()) as u64;
        if records.len > row_budget || records.len.saturating_add(blobs.len) > max_bytes {
            return Err(Error::Refused(format!(
                "{name}: physical extension storage exceeds its read budget"
            )));
        }
        Ok(
            config::read_records_bounded(&mut self.pager, &table, true, max_bytes)?
                .into_iter()
                .map(|record| record.row)
                .collect(),
        )
    }

    /// Row headers without the binary data.
    pub fn config_headers(&mut self, table: &str) -> Result<Vec<ConfigRow>> {
        read_rows(&mut self.pager, &self.tables, table, false)
    }

    /// `(record number, key)` of every index entry, in key order.
    pub fn index_entries(&mut self, table: &str) -> Result<Vec<(u32, Vec<u8>)>> {
        index_entries(&mut self.pager, &self.tables, table)
    }

    /// Checks that the index of a configuration table lists exactly the live
    /// records with the keys their names and part numbers give under the
    /// infobase's collation.
    pub fn verify_index(&mut self, table: &str) -> Result<usize> {
        let collation = known_collation(&self.locale)?;
        verify_index(&mut self.pager, &self.tables, table, collation)
    }
}

fn config_table(tables: &BTreeMap<String, Table>, name: &str) -> Result<Table> {
    let table = tables
        .get(name)
        .ok_or_else(|| Error::Refused(format!("В базе нет таблицы {name}")))?;
    config::check_layout(table)?;
    Ok(table.clone())
}

fn read_rows(
    pager: &mut Pager<'_>,
    tables: &BTreeMap<String, Table>,
    table: &str,
    with_binary: bool,
) -> Result<Vec<ConfigRow>> {
    let table = config_table(tables, table)?;
    Ok(config::read_records(pager, &table, with_binary)?
        .into_iter()
        .map(|r| r.row)
        .collect())
}

fn index_entries(
    pager: &mut Pager<'_>,
    tables: &BTreeMap<String, Table>,
    table: &str,
) -> Result<Vec<(u32, Vec<u8>)>> {
    let table = config_table(tables, table)?;
    let object = object::Object::open(pager, table.index_page)?;
    let data = object.read_all(pager)?;
    index::read_entries_checked(&data, pager.page_size(), &mut |key_len| {
        pager.check_read()?;
        if key_len != 0 {
            pager.materialize_record(0)?;
            pager
                .materialize_object((key_len * 2 + std::mem::size_of::<(u32, Vec<u8>)>()) as u64)?;
        }
        Ok(())
    })
}

fn verify_index(
    pager: &mut Pager<'_>,
    tables: &BTreeMap<String, Table>,
    table: &str,
    collation: Collation,
) -> Result<usize> {
    let t = config_table(tables, table)?;
    let records = config::read_records(pager, &t, false)?;
    let mut expected: Vec<(u32, Vec<u8>)> = records
        .iter()
        .map(|r| {
            Ok((
                r.number,
                index::config_key(collation, &r.row.file_name, r.row.part_no)?,
            ))
        })
        .collect::<Result<_>>()?;
    expected.sort_by(|a, b| a.1.cmp(&b.1));
    let actual = index_entries(pager, tables, table)?;
    if actual != expected {
        let first = actual
            .iter()
            .zip(&expected)
            .position(|(a, b)| a != b)
            .unwrap_or(actual.len().min(expected.len()));
        return Err(Error::Corrupt(format!(
            "Индекс таблицы {table} не совпадает с записями: {} ключей против {} записей, первое расхождение в позиции {first}",
            actual.len(),
            expected.len()
        )));
    }
    Ok(actual.len())
}

fn known_collation(locale: &str) -> Result<Collation> {
    Collation::for_locale(locale).ok_or_else(|| {
        Error::Refused(format!(
            "Коллация «{locale}» информационной базы не измерена: ibcmd-rs пишет только базы с \
             локалью en_US_POSIX или ru_RU (выгрузка работает с любой)"
        ))
    })
}

/// What a write did.
#[derive(Debug, Clone)]
pub struct ReplaceReport {
    pub table: String,
    pub rows_before: usize,
    pub rows_after: usize,
    pub bytes_after: u64,
    pub pages_before: u32,
    pub pages_after: u32,
    pub backup: PathBuf,
}

/// A closed file infobase, write-locked until the session ends: the platform
/// cannot open it, and what the caller reads through the session is what the
/// write replaces.
pub struct WriteSession {
    path: PathBuf,
    file: File,
    locale: String,
    collation: Collation,
    tables: BTreeMap<String, Table>,
    read_budget: Option<read_limits::ReadBudget>,
    paths: write_paths::WritePaths,
}

/// Locks the infobase for a write and checks it can be written: the
/// platform does not have it open, its collation is a measured one, and the
/// indexes of its configuration tables -- keys the platform wrote -- are the
/// keys this crate would write.
pub fn open_for_write(db_path: &Path) -> Result<WriteSession> {
    open_for_write_budget(db_path, None, &mut |_| Ok(()))
}

/// Bounded write-session preflight. The identity guard receives the actual
/// locked descriptor BEFORE parsing any database descriptors. The budget is
/// cumulative across metadata, indexes, repeated row reads and copy verification.
pub fn open_for_write_with_limits(
    db_path: &Path,
    limits: ReadLimits,
    check: impl Fn() -> Result<()> + Send + Sync + 'static,
    guard: &mut dyn FnMut(&fs::Metadata) -> Result<()>,
) -> Result<WriteSession> {
    check()?;
    open_for_write_budget(
        db_path,
        Some(read_limits::ReadBudget::new(limits, check)),
        guard,
    )
}

fn open_for_write_budget(
    db_path: &Path,
    read_budget: Option<read_limits::ReadBudget>,
    guard: &mut dyn FnMut(&fs::Metadata) -> Result<()>,
) -> Result<WriteSession> {
    let path = existing_data_file(db_path)?;
    let paths = write_paths::WritePaths::open(&path)?;
    let file = paths.open_locked(&path)?;
    guard(&file.metadata()?)?;
    if let Some(budget) = &read_budget {
        budget.check()?;
    }
    let (locale, tables) = {
        let mut pager = Pager::new(&file)?;
        pager.set_read_budget(read_budget.clone());
        let (locale, tables) = descr::read_tables(&mut pager)?;
        let collation = known_collation(&locale)?;
        for table in CONFIG_TABLES {
            if tables.contains_key(table) {
                verify_index(&mut pager, &tables, table, collation).map_err(|err| match err {
                    // A name whose key is not measured: e.g. the `*_dynupdate_*`
                    // rows of an unfinished dynamic update (83 characters).
                    Error::Refused(reason) => Error::Refused(format!(
                        "В таблице {table} базы есть строка, ключ индекса которой не измерен \
                         ({reason}); запись не выполняется. Такие строки оставляет, например, \
                         незавершённое динамическое обновление: завершите его штатной платформой"
                    )),
                    other => Error::Refused(format!(
                        "Индексы базы не совпадают с ключами, которые записал бы ibcmd-rs ({other}); \
                         запись не выполняется"
                    )),
                })?;
            }
        }
        (locale, tables)
    };
    let collation = known_collation(&locale)?;
    Ok(WriteSession {
        path,
        file,
        locale,
        collation,
        tables,
        read_budget,
        paths,
    })
}

impl WriteSession {
    fn pager<'a>(&self, file: &'a File) -> Result<Pager<'a>> {
        self.check_read()?;
        let mut pager = Pager::new(file)?;
        pager.set_read_budget(self.read_budget.clone());
        Ok(pager)
    }
    fn check_read(&self) -> Result<()> {
        self.read_budget
            .as_ref()
            .map_or(Ok(()), |budget| budget.check())
    }
    fn copy_checked(&self, source: &mut impl Read, target: &mut impl Write) -> Result<()> {
        let mut buffer = vec![0; 1 << 20];
        loop {
            self.check_read()?;
            let count = source.read(&mut buffer)?;
            if count == 0 {
                return Ok(());
            }
            target.write_all(&buffer[..count])?;
        }
    }

    /// Metadata from the descriptor holding this write lock.
    pub fn metadata(&self) -> std::io::Result<fs::Metadata> {
        self.file.metadata()
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn collation(&self) -> Collation {
        self.collation
    }

    /// Rows of a configuration table, read through the locked descriptor.
    pub fn config_rows(&mut self, table: &str) -> Result<Vec<ConfigRow>> {
        let mut pager = self.pager(&self.file)?;
        read_rows(&mut pager, &self.tables, table, true)
    }

    /// Row headers of a configuration table, without the binary data.
    pub fn config_headers(&mut self, table: &str) -> Result<Vec<ConfigRow>> {
        let mut pager = self.pager(&self.file)?;
        read_rows(&mut pager, &self.tables, table, false)
    }

    /// Digest of the locked data file including its filesystem identity.
    /// An opaque equality fingerprint under the held lock, not an atomic
    /// filesystem CAS or universal ABA guarantee. Windows admits local fixed
    /// NTFS only; Unix retains the existing dev/inode token construction.
    pub fn snapshot(&self) -> Result<String> {
        #[cfg(not(any(unix, windows)))]
        {
            Err(Error::Refused(
                "file import CAS identity is not supported on this OS".into(),
            ))
        }
        #[cfg(any(unix, windows))]
        {
            use sha2::{Digest, Sha256};
            self.check_read()?;
            self.check_binding()?;
            let mut hash = Sha256::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                let metadata = self.file.metadata()?;
                hash.update(metadata.dev().to_le_bytes());
                hash.update(metadata.ino().to_le_bytes());
            }
            #[cfg(windows)]
            let identity = {
                let identity = self.paths.snapshot_identity(&self.path, &self.file)?;
                hash.update(b"ibcmd-filedb:windows-ntfs-snapshot:v1\0");
                hash.update(identity);
                identity
            };
            let mut file = &self.file;
            file.seek(SeekFrom::Start(0))?;
            let mut buffer = vec![0; 1 << 20];
            loop {
                self.check_read()?;
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hash.update(&buffer[..n]);
            }
            self.check_read()?;
            self.check_binding()?;
            #[cfg(windows)]
            if self.paths.snapshot_identity(&self.path, &self.file)? != identity {
                return Err(Error::Refused(
                    "Windows snapshot descriptor identity changed".into(),
                ));
            }
            self.check_read()?;
            Ok(format!("{:x}", hash.finalize()))
        }
    }

    fn check_binding(&self) -> Result<()> {
        self.paths.require_binding(&self.path, &self.file)
    }

    /// Replaces every row of a configuration table and ends the session.
    ///
    /// The steps: copy the data file to a new work file (its mode and owner
    /// those of the original), check that the table's pages are its own, write
    /// the table into the copy, check that no other page changed and that the
    /// table and its index read back as written, keep the original as
    /// `<data file>.ibcmd-rs.bak` (a hard link when the file system allows),
    /// rename the copy over the data file. A failure before the rename leaves
    /// the infobase untouched.
    pub fn replace(self, table: &str, rows: &[ConfigRow]) -> Result<ReplaceReport> {
        self.replace_checked(table, rows, || Ok(()))
    }

    /// As replace, with a caller publication check before work and after verification.
    /// A rejected final check leaves the original data file in place.
    pub fn replace_checked(
        self,
        table: &str,
        rows: &[ConfigRow],
        mut check: impl FnMut() -> Result<()>,
    ) -> Result<ReplaceReport> {
        self.replace_guarded(table, rows, |_| check())
    }

    /// Supplies the actual locked descriptor metadata at both publication gates.
    pub fn replace_guarded(
        self,
        table: &str,
        rows: &[ConfigRow],
        mut check: impl FnMut(&fs::Metadata) -> Result<()>,
    ) -> Result<ReplaceReport> {
        check(&self.file.metadata()?)?;
        self.check_binding()?;
        if !CONFIG_TABLES.contains(&table) {
            return Err(Error::Refused(format!(
                "Запись таблицы {table} не поддерживается"
            )));
        }
        let target = config_table(&self.tables, table)?;
        let work = sibling(&self.path, WORK_SUFFIX);
        let backup = sibling(&self.path, BACKUP_SUFFIX);
        let mut copy = match self.paths.create_new(&work, true) {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(Error::Refused(format!(
                    "Рядом с базой остался рабочий файл {} прерванной записи: убедитесь, что \
                     загрузка не идёт, удалите его и повторите",
                    work.display()
                )));
            }
            Err(err) => return Err(Error::Io(err)),
        };
        let result = self.write_copy(&target, rows, &mut copy, &backup);
        let report = match result {
            Ok(report) => report,
            Err(err) => {
                drop(copy);
                let _ = self.paths.remove(&work);
                return Err(err);
            }
        };
        #[cfg(not(any(unix, windows)))]
        drop(copy);
        // Prepare recovery data without replacing an existing backup. Rejection
        // or interruption before the final check preserves both published files.
        let backup_work = sibling(&self.path, ".ibcmd-rs.bak.new");
        let prepare_backup = || -> Result<Option<File>> {
            self.check_binding()?;
            prepare_backup_file(
                &self.paths,
                &self.file,
                &backup_work,
                || self.paths.hard_link(&self.path, &backup_work),
                |out| {
                    let mut original = &self.file;
                    original.seek(SeekFrom::Start(0))?;
                    self.copy_checked(&mut original, out)
                },
            )
        };
        let backup_copy = match prepare_backup() {
            Ok(file) => file,
            Err(error) => {
                let _ = self.paths.remove(&work);
                return Err(error);
            }
        };
        if let Err(error) = check(&self.file.metadata()?).and_then(|()| {
            self.check_binding()?;
            #[cfg(any(unix, windows))]
            self.paths.require_leaf(&work, &copy)?;
            require_owner_and_mode(&self.file, &copy)?;
            if let Some(backup_copy) = &backup_copy {
                self.paths.require_leaf(&backup_work, backup_copy)?;
                require_owner_and_mode(&self.file, backup_copy)?;
            } else {
                self.paths.require_leaf(&backup_work, &self.file)?;
            }
            Ok(())
        }) {
            let _ = self.paths.remove(&work);
            let _ = self.paths.remove(&backup_work);
            return Err(error);
        }
        // Publish the database FIRST. Until this succeeds the previous backup
        // is untouched. A later backup rename failure retains .bak.new (the
        // complete pre-write database) and the previous .bak; no rollback can
        // destroy the only recovery generation.
        let WriteSession {
            path, file, paths, ..
        } = self;
        #[cfg(windows)]
        drop(file);
        // On error, RAII keeps the original descriptor locked until return.
        publish_files(&paths, &work, &path, &backup_work, &backup, |a, b| {
            paths.rename(a, b)
        })?;
        paths.sync();
        #[cfg(unix)]
        drop(file);
        Ok(report)
    }

    fn write_copy(
        &self,
        target: &Table,
        rows: &[ConfigRow],
        copy: &mut File,
        backup: &Path,
    ) -> Result<ReplaceReport> {
        let mut original = &self.file;
        original.seek(SeekFrom::Start(0))?;
        keep_owner_and_mode(&self.file, copy, &self.path)?;
        self.copy_checked(&mut original, copy)?;

        // The table's pages are its own: nothing else would change with them.
        let (pages_before, rows_before, others) = {
            let mut pager = self.pager(&self.file)?;
            // Owner of every page, as an index into `names`: a few bytes a page
            // whatever the size of the infobase.
            let mut names = vec![
                "списку свободных страниц".to_string(),
                "корневому объекту".to_string(),
            ];
            let mut owners = BTreeMap::<u32, u32>::new();
            let mut claim = |page: u32, owner: u32, names: &[String]| -> Result<()> {
                if let Some(first) = owners.insert(page, owner) {
                    return Err(Error::Corrupt(format!(
                        "Страница {page} принадлежит и {}, и {}: запись не выполняется",
                        names[first as usize], names[owner as usize]
                    )));
                }
                Ok(())
            };
            claim(1, 0, &names)?;
            for page in object::Object::open(&mut pager, 2)?.owned_pages() {
                claim(page, 1, &names)?;
            }
            let mut target_owner = u32::MAX;
            for t in self.tables.values() {
                let owner = names.len() as u32;
                names.push(format!("таблице {}", t.name));
                if t.name == target.name {
                    target_owner = owner;
                }
                for page in config::table_pages(&mut pager, t)? {
                    claim(page, owner, &names)?;
                }
            }
            let rows_before = config::read_records(&mut pager, target, false)?.len();
            let others: BTreeSet<u32> = owners
                .iter()
                .filter(|(_, owner)| **owner != target_owner)
                .map(|(page, _)| *page)
                .collect();
            (pager.page_count(), rows_before, others)
        };

        let mut pager = self.pager(&*copy)?;
        let written_pages = config::replace(&mut pager, target, rows, self.collation)?;
        pager.sync()?;
        let pages_after = pager.page_count();
        if let Some(page) = written_pages.iter().find(|page| others.contains(page)) {
            return Err(Error::Corrupt(format!(
                "Проверка записи: таблица {} заняла чужую страницу {page}",
                target.name
            )));
        }

        // Every page the write did not own is the original's, byte for byte.
        let mut before = self.pager(&self.file)?;
        let mut a = vec![0u8; pager.page_size()];
        let mut b = vec![0u8; pager.page_size()];
        for page in 1..pages_before {
            if written_pages.contains(&page) {
                continue;
            }
            before.read_page_into(page, &mut a)?;
            pager.read_page_into(page, &mut b)?;
            if a != b {
                return Err(Error::Corrupt(format!(
                    "Проверка записи: изменилась страница {page}, не принадлежащая таблице {}",
                    target.name
                )));
            }
        }

        // The table and its index read back as written, through the copy.
        let (_, tables) = descr::read_tables(&mut pager)?;
        let written = read_rows(&mut pager, &tables, &target.name, true)?;
        if written.len() != rows.len() || written.iter().zip(rows).any(|(a, b)| a != b) {
            return Err(Error::Corrupt(format!(
                "Проверка записи: таблица {} прочиталась не так, как записана",
                target.name
            )));
        }
        verify_index(&mut pager, &tables, &target.name, self.collation)?;
        drop(pager);

        Ok(ReplaceReport {
            table: target.name.clone(),
            rows_before,
            rows_after: rows.len(),
            bytes_after: rows.iter().map(|r| r.binary.len() as u64).sum(),
            pages_before,
            pages_after,
            backup: backup.to_path_buf(),
        })
    }
}

// A failed fallback copy must not leave this attempt's partial recovery
// file behind. create_new establishes ownership: a pre-existing file is NEVER
// removed, truncated or overwritten. The injected operations keep fault tests
// on the same path as production, including filesystems without hard links.
fn prepare_backup_file(
    paths: &write_paths::WritePaths,
    original: &File,
    path: &Path,
    hard_link: impl FnOnce() -> std::io::Result<()>,
    copy: impl FnOnce(&mut File) -> Result<()>,
) -> Result<Option<File>> {
    if hard_link().is_ok() {
        paths.require_leaf(path, original)?;
        return Ok(None);
    }
    let mut out = match paths.create_new(path, false) {
        Ok(out) => out,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(Error::Refused(format!(
                "pre-existing backup work file {}; inspect interrupted publication before retrying",
                path.display()
            )));
        }
        Err(error) => return Err(Error::Io(error)),
    };
    let result = keep_owner_and_mode(original, &out, path)
        .and_then(|()| copy(&mut out))
        .and_then(|()| require_owner_and_mode(original, &out))
        .and_then(|()| Ok(out.sync_all()?));
    match result {
        Ok(()) => Ok(Some(out)),
        Err(error) => {
            drop(out);
            let _ = paths.remove(path);
            Err(error)
        }
    }
}

// Two-step publication deliberately has a recoverable partial-success state:
// if updating backup fails, the new database is published and the old database
// remains in backup_work. Never silently discard either backup generation.
fn publish_files(
    paths: &write_paths::WritePaths,
    work: &Path,
    path: &Path,
    backup_work: &Path,
    backup: &Path,
    mut rename: impl FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<()> {
    if let Err(error) = rename(work, path) {
        let _ = paths.remove(work);
        let _ = paths.remove(backup_work);
        return Err(Error::Io(error));
    }
    if let Err(error) = rename(backup_work, backup) {
        return Err(Error::Published {
            recovery: backup_work.to_path_buf(),
            error,
        });
    }
    Ok(())
}

/// Before copying content, set the Unix owner/group/mode or require an already
/// matching Windows owner/group/DACL. Windows ACL differences are refused,
/// not rewritten; its readonly attribute is copied separately. SACL, auditing
/// and mandatory labels are outside this check's scope.
fn keep_owner_and_mode(original: &File, copy: &File, path: &Path) -> Result<()> {
    #[cfg(windows)]
    windows_security::require_same(original, copy)?;
    let metadata = original.metadata()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let current = copy.metadata()?;
        if (current.uid(), current.gid()) != (metadata.uid(), metadata.gid())
            && std::os::unix::fs::fchown(copy, Some(metadata.uid()), Some(metadata.gid())).is_err()
        {
            return Err(Error::Refused(format!(
                "Файл {} принадлежит пользователю {} (группа {}): запустите загрузку от его имени, \
                 чтобы у базы остались прежние владелец и права",
                path.display(),
                metadata.uid(),
                metadata.gid()
            )));
        }
    }
    copy.set_permissions(metadata.permissions())?;
    #[cfg(not(unix))]
    let _ = path;
    require_owner_and_mode(original, copy)
}

// Recheck the held source/work descriptors without changing their security.
fn require_owner_and_mode(original: &File, copy: &File) -> Result<()> {
    #[cfg(windows)]
    windows_security::require_same(original, copy)?;
    let original = original.metadata()?;
    let copy = copy.metadata()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if (original.uid(), original.gid(), original.mode())
            != (copy.uid(), copy.gid(), copy.mode())
        {
            return Err(Error::Refused(
                "file owner/group/mode changed before publication".into(),
            ));
        }
    }
    if original.permissions().readonly() != copy.permissions().readonly() {
        return Err(Error::Refused(
            "file readonly attribute changed before publication".into(),
        ));
    }
    Ok(())
}

/// Replaces every row of a configuration table of a closed file infobase:
/// [`open_for_write`], then [`WriteSession::replace`].
pub fn replace_config_table(
    db_path: &Path,
    table: &str,
    rows: &[ConfigRow],
) -> Result<ReplaceReport> {
    open_for_write(db_path)?.replace(table, rows)
}

/// Research aid: rebuilds the index of one configuration table in place, from
/// its live records. The infobase must be closed; no backup is made.
#[doc(hidden)]
pub fn rebuild_config_index_in_place(db_path: &Path, table: &str) -> Result<()> {
    let path = existing_data_file(db_path)?;
    let file = lock::open_locked(&path, Mode::Write)?;
    let mut pager = Pager::new(&file)?;
    let (locale, tables) = descr::read_tables(&mut pager)?;
    let t = config_table(&tables, table)?;
    config::reindex(&mut pager, &t, known_collation(&locale)?)?;
    pager.sync()
}

#[cfg(all(test, unix))]
mod publication_tests {
    use super::*;
    #[test]
    fn failed_backup_copy_cleans_only_owned_file_and_allows_retry() {
        for cancelled in [false, true] {
            let dir = std::env::temp_dir().join(format!(
                "filedb-backup-copy-{}-{cancelled}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&dir).unwrap();
            let data = dir.join("data");
            let backup = dir.join("backup");
            let pending = dir.join("pending");
            fs::write(&data, b"original database A").unwrap();
            fs::write(&backup, b"older backup B").unwrap();
            let original = File::open(&data).unwrap();
            let paths = write_paths::WritePaths::open(&data).unwrap();
            let no_link = || Err(std::io::Error::other("injected hard-link unavailable"));
            let error = prepare_backup_file(&paths, &original, &pending, no_link, |out| {
                out.write_all(b"partial")?;
                if cancelled {
                    Err(Error::Refused("injected cancellation".into()))
                } else {
                    Err(Error::Io(std::io::Error::other("injected copy EIO")))
                }
            })
            .unwrap_err();
            assert!(error.to_string().contains("injected"));
            assert!(!pending.exists());
            assert_eq!(fs::read(&data).unwrap(), b"original database A");
            assert_eq!(fs::read(&backup).unwrap(), b"older backup B");
            prepare_backup_file(&paths, &original, &pending, no_link, |out| {
                Ok(out.write_all(b"original database A")?)
            })
            .unwrap();
            assert_eq!(fs::read(&pending).unwrap(), b"original database A");
            let error = prepare_backup_file(&paths, &original, &pending, no_link, |_| {
                panic!("must not touch pre-existing file")
            })
            .unwrap_err();
            assert!(error.to_string().contains("pre-existing backup work file"));
            assert_eq!(fs::read(&pending).unwrap(), b"original database A");
            let work = dir.join("work");
            fs::write(&work, b"new database C").unwrap();
            drop(original);
            publish_files(&paths, &work, &data, &pending, &backup, |a, b| {
                fs::rename(a, b)
            })
            .unwrap();
            assert_eq!(fs::read(&data).unwrap(), b"new database C");
            assert_eq!(fs::read(&backup).unwrap(), b"original database A");
            assert!(!pending.exists());
            if std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").is_none() {
                fs::remove_dir_all(dir).unwrap();
            }
        }
    }
    #[test]
    fn each_publication_failure_preserves_both_backup_generations() {
        for fail_at in 1..=2 {
            let dir = std::env::temp_dir().join(format!(
                "filedb-publish-{}-{fail_at}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&dir).unwrap();
            let data = dir.join("data");
            let work = dir.join("work");
            let backup = dir.join("backup");
            let pending = dir.join("pending");
            fs::write(&data, b"A").unwrap();
            fs::write(&backup, b"B").unwrap();
            fs::write(&work, b"C").unwrap();
            fs::write(&pending, b"A").unwrap();
            let paths = write_paths::WritePaths::open(&data).unwrap();
            let mut step = 0;
            let error = publish_files(&paths, &work, &data, &pending, &backup, |a, b| {
                step += 1;
                if step == fail_at {
                    Err(std::io::Error::other("injected EIO"))
                } else {
                    fs::rename(a, b)
                }
            })
            .unwrap_err();
            assert_eq!(fs::read(&backup).unwrap(), b"B");
            if fail_at == 1 {
                assert_eq!(fs::read(&data).unwrap(), b"A");
                assert!(!pending.exists());
                assert!(!work.exists());
            } else {
                assert_eq!(fs::read(&data).unwrap(), b"C");
                assert_eq!(fs::read(&pending).unwrap(), b"A");
                assert!(matches!(error, Error::Published { .. }));
                assert!(error.to_string().contains("database was published"));
                assert!(!work.exists());
                // No rollback is attempted: even an unavailable filesystem
                // cannot erase recovery A or the previous generation B.
            }
            if std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").is_none() {
                fs::remove_dir_all(dir).unwrap();
            }
        }
    }
}
