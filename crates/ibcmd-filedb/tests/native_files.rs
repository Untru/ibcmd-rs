//! The reader and the writer against files written by 1C:Enterprise
//! 8.5.1.1150 (`tests/fixtures/file-infobase` of the workspace).

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use ibcmd_filedb::{
    CONFIG_TABLES, ConfigRow, Error, FileDb, Timestamp, open_for_write, replace_config_table,
};

#[cfg(any(unix, windows))]
use ibcmd_filedb::{Collation, backup_file};

/// A scratch directory holding one unpacked fixture, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn unpack(name: &str) -> Scratch {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/file-infobase")
        .join(format!("{name}.1CD.gz"));
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(fs::File::open(source).unwrap())
        .read_to_end(&mut bytes)
        .unwrap();
    let dir = std::env::temp_dir().join(format!(
        "ibcmd-filedb-{}-{}-{name}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("1Cv8.1CD"), bytes).unwrap();
    Scratch(dir)
}

fn row(name: &str, part_no: u32, binary: Vec<u8>) -> ConfigRow {
    ConfigRow {
        file_name: name.into(),
        part_no,
        creation: Timestamp::now_utc(),
        modified: Timestamp::now_utc(),
        attributes: 0,
        data_size: binary.len() as u64,
        binary,
    }
}

#[test]
fn native_files_read_and_every_index_matches_its_records() {
    for (name, locale) in [
        ("empty", "en_US_POSIX"),
        ("tiny", "en_US_POSIX"),
        ("ru-empty", "ru_RU"),
    ] {
        let dir = unpack(name);
        let mut db = FileDb::open(&dir.0).unwrap();
        assert_eq!(db.locale(), locale);
        assert_eq!(db.page_size(), 8192);
        for table in CONFIG_TABLES {
            let rows = db.config_rows(table).unwrap();
            assert_eq!(
                db.verify_index(table).unwrap(),
                rows.len(),
                "{name} {table}"
            );
        }
    }
    let dir = unpack("tiny");
    let rows = FileDb::open(&dir.0).unwrap().config_rows("CONFIG").unwrap();
    let names: Vec<&str> = rows.iter().map(|r| r.file_name.as_str()).collect();
    for service in ["root", "version", "versions"] {
        assert!(names.contains(&service), "{service} in {names:?}");
    }
    assert!(rows.iter().all(|r| r.data_size == r.binary.len() as u64));
}

#[cfg(any(unix, windows))]
#[test]
fn a_table_written_back_reads_the_same_and_keeps_a_backup() {
    let dir = unpack("tiny");
    let data = dir.0.join("1Cv8.1CD");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&data, fs::Permissions::from_mode(0o640)).unwrap();
    }
    let original = fs::read(&data).unwrap();
    let config = FileDb::open(&dir.0).unwrap().config_rows("CONFIG").unwrap();
    let report = replace_config_table(&dir.0, "CONFIGSAVE", &config).unwrap();
    assert_eq!(report.rows_after, config.len());
    assert_eq!(report.backup, backup_file(&dir.0));
    assert_eq!(fs::read(backup_file(&dir.0)).unwrap(), original);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&data).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640, "the data file keeps its mode");
    }
    let mut db = FileDb::open(&dir.0).unwrap();
    assert_eq!(db.config_rows("CONFIGSAVE").unwrap(), config);
    assert_eq!(db.config_rows("CONFIG").unwrap(), config);
    db.verify_index("CONFIGSAVE").unwrap();
    drop(db);
    // Written again over the first write: the space is reused, not grown.
    let again = replace_config_table(&dir.0, "CONFIGSAVE", &config).unwrap();
    assert_eq!(again.pages_after, report.pages_after);
}

#[cfg(any(unix, windows))]
#[test]
fn objects_grow_to_second_level_tables_and_shrink_onto_free_chains() {
    let dir = unpack("tiny");
    let mut rows: Vec<ConfigRow> = (0..6000)
        .map(|i| {
            row(
                &format!("{i:08x}-0000-4000-8000-{:012x}.{}", i * 7919, i % 3),
                0,
                vec![(i % 251) as u8; 40 + i % 700],
            )
        })
        .collect();
    // Three rows of three parts each, as long rows are stored.
    for n in 0..3u8 {
        for part in 0..3 {
            rows.push(row(&format!("big{n}.0"), part, vec![n; 4_000_000]));
        }
    }
    let big = replace_config_table(&dir.0, "CONFIGSAVE", &rows).unwrap();
    assert!(big.pages_after > 1500, "{}", big.pages_after);
    let mut db = FileDb::open(&dir.0).unwrap();
    assert_eq!(db.config_rows("CONFIGSAVE").unwrap(), rows);
    assert_eq!(db.verify_index("CONFIGSAVE").unwrap(), rows.len());
    db.verify_index("CONFIG").unwrap();
    drop(db);
    let few = &rows[..10];
    let small = replace_config_table(&dir.0, "CONFIGSAVE", few).unwrap();
    assert_eq!(small.pages_after, big.pages_after, "objects never shrink");
    let mut db = FileDb::open(&dir.0).unwrap();
    assert_eq!(db.config_rows("CONFIGSAVE").unwrap(), few);
    db.verify_index("CONFIGSAVE").unwrap();
}

#[cfg(any(unix, windows))]
#[test]
fn a_russian_infobase_is_written_with_its_own_collation() {
    let dir = unpack("ru-empty");
    let session = open_for_write(&dir.0).unwrap();
    assert_eq!(session.collation(), Collation::Russian);
    let rows = vec![
        row("root", 0, vec![1]),
        row("Versions", 0, vec![2]),
        row("a2782760-dd4c-44d2-a785-814d6f610455.0", 0, vec![3]),
    ];
    session.replace("CONFIGSAVE", &rows).unwrap();
    let mut db = FileDb::open(&dir.0).unwrap();
    assert_eq!(db.verify_index("CONFIGSAVE").unwrap(), 3);
    db.verify_index("PARAMS").unwrap();
}

#[test]
fn an_infobase_in_an_unmeasured_locale_is_read_but_not_written() {
    let dir = unpack("de-empty");
    let mut db = FileDb::open(&dir.0).unwrap();
    assert_eq!(db.locale(), "de_DE");
    assert_eq!(db.collation(), None);
    assert!(!db.config_rows("PARAMS").unwrap().is_empty());
    drop(db);
    let before = fs::read(dir.0.join("1Cv8.1CD")).unwrap();
    let refused = replace_config_table(&dir.0, "CONFIGSAVE", &[row("root", 0, vec![1])]);
    assert!(matches!(refused, Err(Error::Refused(_))), "{refused:?}");
    assert_eq!(fs::read(dir.0.join("1Cv8.1CD")).unwrap(), before);
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
}

#[cfg(any(unix, windows))]
#[test]
fn a_row_the_index_cannot_key_changes_nothing() {
    let dir = unpack("tiny");
    let before = fs::read(dir.0.join("1Cv8.1CD")).unwrap();
    let refused = replace_config_table(&dir.0, "CONFIGSAVE", &[row("имя по-русски", 0, vec![0])]);
    assert!(refused.is_err());
    assert_eq!(fs::read(dir.0.join("1Cv8.1CD")).unwrap(), before);
    assert_eq!(
        fs::read_dir(&dir.0).unwrap().count(),
        1,
        "no work file, no backup"
    );
}

#[cfg(any(unix, windows))]
#[test]
fn a_work_file_left_behind_stops_the_write() {
    let dir = unpack("tiny");
    let work = dir.0.join("1Cv8.1CD.ibcmd-rs.tmp");
    fs::write(&work, b"left by an interrupted write").unwrap();
    let refused = replace_config_table(&dir.0, "CONFIGSAVE", &[row("root", 0, vec![1])]);
    assert!(matches!(refused, Err(Error::Refused(_))), "{refused:?}");
    assert_eq!(fs::read(&work).unwrap(), b"left by an interrupted write");
}

#[test]
fn a_corrupted_object_length_is_refused_without_allocating_it() {
    let dir = unpack("tiny");
    let data = dir.0.join("1Cv8.1CD");
    let mut bytes = fs::read(&data).unwrap();
    // The root object's length (page 2, offset 16).
    bytes[2 * 8192 + 16..2 * 8192 + 24].copy_from_slice(&0xFFFF_FFFF_FFFF_FF00u64.to_le_bytes());
    fs::write(&data, &bytes).unwrap();
    assert!(matches!(FileDb::open(&dir.0), Err(Error::Corrupt(_))));
    assert!(open_for_write(&dir.0).is_err());
}

#[cfg(any(unix, windows))]
#[test]
fn bounded_write_checks_identity_before_parsing_and_shares_budget_between_reads() {
    use ibcmd_filedb::{ReadLimits, open_for_write_with_limits};
    let dir = unpack("tiny");
    let original = fs::read(dir.0.join("1Cv8.1CD")).unwrap();
    let limits = ReadLimits {
        max_records: 100_000,
        max_binary_bytes: 1 << 30,
        max_object_bytes: 1 << 30,
    };
    fs::write(dir.0.join("1Cv8.1CD"), b"not a database").unwrap();
    let error = match open_for_write_with_limits(&dir.0, limits, || Ok(()), &mut |_| {
        Err(Error::Refused("startup identity rejected".into()))
    }) {
        Ok(_) => panic!("identity accepted"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("startup identity rejected"));
    fs::write(dir.0.join("1Cv8.1CD"), &original).unwrap();
    let bytes: u64 = FileDb::open(&dir.0)
        .unwrap()
        .config_rows("CONFIG")
        .unwrap()
        .iter()
        .map(|r| r.binary.len() as u64)
        .sum();
    assert!(bytes > 0);
    let mut session = open_for_write_with_limits(
        &dir.0,
        ReadLimits {
            max_binary_bytes: bytes,
            ..limits
        },
        || Ok(()),
        &mut |_| Ok(()),
    )
    .unwrap();
    session.config_rows("CONFIG").unwrap();
    assert!(
        session
            .config_rows("CONFIG")
            .unwrap_err()
            .to_string()
            .contains("materialization limit")
    );
    drop(session);
    assert_eq!(fs::read(dir.0.join("1Cv8.1CD")).unwrap(), original);
}

#[cfg(any(unix, windows))]
#[test]
fn bounded_write_cancels_snapshot_and_copy_without_publication() {
    use ibcmd_filedb::{ReadLimits, open_for_write_with_limits};
    use std::sync::{Arc, atomic::AtomicBool};
    let dir = unpack("tiny");
    let original = fs::read(dir.0.join("1Cv8.1CD")).unwrap();
    let cancelled = Arc::new(AtomicBool::new(false));
    let callback = cancelled.clone();
    let session = open_for_write_with_limits(
        &dir.0,
        ReadLimits {
            max_records: 1_000_000,
            max_binary_bytes: 1 << 30,
            max_object_bytes: 1 << 30,
        },
        move || {
            if callback.load(Ordering::SeqCst) {
                Err(Error::Refused("budget cancelled".into()))
            } else {
                Ok(())
            }
        },
        &mut |_| Ok(()),
    )
    .unwrap();
    cancelled.store(true, Ordering::SeqCst);
    assert!(
        session
            .snapshot()
            .unwrap_err()
            .to_string()
            .contains("budget cancelled")
    );
    assert!(
        session
            .replace("CONFIGSAVE", &[])
            .unwrap_err()
            .to_string()
            .contains("budget cancelled")
    );
    assert_eq!(fs::read(dir.0.join("1Cv8.1CD")).unwrap(), original);
    assert!(!backup_file(&dir.0).exists());
}

#[cfg(not(any(unix, windows)))]
#[test]
fn unsupported_writer_refuses_before_creating_artifacts() {
    let dir = unpack("tiny");
    let original = fs::read(dir.0.join("1Cv8.1CD")).unwrap();
    let error = match open_for_write(&dir.0) {
        Ok(_) => panic!("unsupported writer opened"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("not supported on this OS"));
    assert_eq!(fs::read(dir.0.join("1Cv8.1CD")).unwrap(), original);
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
}

#[test]
fn bounded_reader_counts_shared_blob_chains_before_copying() {
    let dir = unpack("empty");
    let rows: Vec<_> = (0..32)
        .map(|i| {
            row(
                &format!("budgetprobe{i:03}"),
                0,
                if i == 0 {
                    vec![42; 1024 * 1024]
                } else {
                    Vec::new()
                },
            )
        })
        .collect();
    replace_config_table(&dir.0, "CONFIGCAS", &rows).unwrap();
    let path = dir.0.join("1Cv8.1CD");
    let mut bytes = fs::read(&path).unwrap();
    let db = FileDb::open(&dir.0).unwrap();
    let table = db.table("CONFIGCAS").unwrap();
    let binary_offset = 1 + table
        .fields
        .iter()
        .take_while(|f| f.name != "BINARYDATA")
        .map(|f| f.size())
        .sum::<usize>();
    drop(db);
    let slots: Vec<_> = rows
        .iter()
        .map(|row| {
            let needle: Vec<u8> = row
                .file_name
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect();
            let name_at = bytes
                .windows(needle.len())
                .position(|v| v == needle)
                .unwrap();
            name_at - 3 + binary_offset
        })
        .collect();
    let shared_ref = bytes[slots[0]..slots[0] + 8].to_vec();
    for at in &slots[1..] {
        bytes[*at..*at + 8].copy_from_slice(&shared_ref);
    }
    fs::write(&path, bytes).unwrap();
    let result =
        FileDb::open(&dir.0)
            .unwrap()
            .config_rows_bounded("CONFIGCAS", 2 * 1024 * 1024, 32);
    let err = match result {
        Err(err) => err,
        Ok(rows) => panic!(
            "2 MiB budget accepted {} retained bytes",
            rows.iter().map(|row| row.binary.len()).sum::<usize>()
        ),
    };
    assert!(err.to_string().contains("retained"), "{err}");
}
