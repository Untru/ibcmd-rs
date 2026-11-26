//! Partial staging never supplies a sparse tree to a full importer.
#![cfg(unix)]
use ibcmd_rs::{
    file_import::{self, Selection},
    platform,
    stored_objects::{ConfigSource, StoredConfiguration},
};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
#[path = "support/owned_scratch.rs"]
mod owned_scratch;
use owned_scratch::OwnedScratch;

// Cancellation scopes are process-global. Serialize whole test bodies, not
// only the flag mutation, so another export never observes a sibling's scope.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
const MODULE: &str = "CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl";
const DESCRIPTOR: &str = "CommonModules/ФайловаяБазаПроверка.xml";
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/file-infobase")
}
fn setup() -> (OwnedScratch, PathBuf, PathBuf) {
    let root = OwnedScratch::new("issue382-partial");
    let db = root.join("db");
    let tree = root.join("src");
    fs::create_dir_all(&db).unwrap();
    fs::create_dir_all(&tree).unwrap();
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(fs::File::open(fixture().join("tiny.1CD.gz")).unwrap())
        .read_to_end(&mut bytes)
        .unwrap();
    fs::write(db.join("1Cv8.1CD"), bytes).unwrap();
    for entry in walkdir::WalkDir::new(fixture().join("tiny-xml"))
        .into_iter()
        .map(Result::unwrap)
        .filter(|e| e.file_type().is_file())
    {
        let path = tree.join(
            entry
                .path()
                .strip_prefix(fixture().join("tiny-xml"))
                .unwrap(),
        );
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::copy(entry.path(), path).unwrap();
    }
    (root, db, tree)
}
fn selected(path: &str) -> Selection {
    Selection {
        files: vec![path.into()],
        ..Selection::default()
    }
}
fn prepare(db: &Path, tree: &Path, path: &str) -> file_import::Plan {
    file_import::prepare(
        db,
        tree,
        &selected(path),
        platform::parse_flag("8.5.1").unwrap(),
    )
    .unwrap()
}
fn edit(tree: &Path, path: &str, from: &str, to: &str) {
    let text = fs::read_to_string(tree.join(path)).unwrap();
    assert!(text.contains(from));
    fs::write(tree.join(path), text.replace(from, to)).unwrap();
}
fn read(db: &Path, path: &str) -> Vec<u8> {
    let mut config = StoredConfiguration::new(
        ConfigSource::FileDb(db.into()),
        platform::parse_flag("8.5.1").unwrap(),
    );
    let (files, path) = config.read_file(path).unwrap();
    files.files[&path].clone()
}
#[test]
fn selected_module_preserves_unselected_files_and_previous_staging() {
    let _serial = serial();
    let (_scratch, db, tree) = setup();
    edit(
        &tree,
        DESCRIPTOR,
        "<Comment/>",
        "<Comment>first stage</Comment>",
    );
    let first = prepare(&db, &tree, DESCRIPTOR);
    assert!(first.summary().ready);
    first.publish(&db).unwrap();
    let staged_descriptor = read(&db, DESCRIPTOR);
    // A stale unselected descriptor and an unrelated source edit must not undo stage one.
    edit(
        &tree,
        DESCRIPTOR,
        "<Comment>first stage</Comment>",
        "<Comment/>",
    );
    edit(
        &tree,
        "Languages/English.xml",
        "<Comment/>",
        "<Comment>unselected</Comment>",
    );
    let expected = fs::read_to_string(tree.join(MODULE)).unwrap() + "\r\n// selected edit\r\n";
    fs::write(tree.join(MODULE), &expected).unwrap();
    let before = fs::read(db.join("1Cv8.1CD")).unwrap();
    let plan = prepare(&db, &tree, MODULE);
    assert!(plan.summary().ready);
    assert_eq!(plan.summary().files, vec![MODULE]);
    assert_eq!(
        fs::read(db.join("1Cv8.1CD")).unwrap(),
        before,
        "preflight wrote the target"
    );
    plan.publish(&db).unwrap();
    assert_eq!(read(&db, MODULE), expected.as_bytes());
    assert_eq!(read(&db, DESCRIPTOR), staged_descriptor);
    assert_eq!(
        read(&db, "Languages/English.xml"),
        fs::read(fixture().join("tiny-xml/Languages/English.xml")).unwrap()
    );
    assert_eq!(fs::read(ibcmd_filedb::backup_file(&db)).unwrap(), before);
}
#[test]
fn changed_target_source_or_connection_refuses_before_writes() {
    let _serial = serial();
    for scenario in 0..3 {
        let (_scratch, db, tree) = setup();
        let text = fs::read_to_string(tree.join(MODULE)).unwrap() + "\r\n// edit\r\n";
        fs::write(tree.join(MODULE), text).unwrap();
        let plan = prepare(&db, &tree, MODULE);
        if scenario == 0 {
            let changed = fs::read_to_string(tree.join(MODULE)).unwrap() + "// concurrent";
            fs::write(tree.join(MODULE), changed).unwrap();
        }
        if scenario == 1 {
            let session = ibcmd_filedb::open_for_write(&db).unwrap();
            session.replace("CONFIGSAVE", &[]).unwrap();
        }
        let before = fs::read(db.join("1Cv8.1CD")).unwrap();
        let other = (scenario == 2).then(setup);
        let current = other.as_ref().map(|(_, db, _)| db).unwrap_or(&db);
        assert!(plan.publish(current).is_err());
        assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), before);
    }
}
#[test]
fn new_or_missing_files_and_incomplete_stage_refuse_before_writes() {
    let _serial = serial();
    for scenario in 0..3 {
        let (_scratch, db, tree) = setup();
        let before = fs::read(db.join("1Cv8.1CD")).unwrap();
        let selected_path = if scenario == 0 {
            let new = "CommonModules/New.xml";
            fs::copy(tree.join(DESCRIPTOR), tree.join(new)).unwrap();
            new
        } else {
            MODULE
        };
        if scenario == 1 {
            fs::rename(tree.join(MODULE), tree.join("kept-original.bsl")).unwrap();
        }
        if scenario == 2 {
            let mut session = ibcmd_filedb::open_for_write(&db).unwrap();
            let mut rows = session.config_rows("CONFIG").unwrap();
            rows.retain(|row| row.file_name != "versions");
            session.replace("CONFIGSAVE", &rows).unwrap();
        }
        let expected = if scenario == 2 {
            fs::read(db.join("1Cv8.1CD")).unwrap()
        } else {
            before
        };
        assert!(
            file_import::prepare(
                &db,
                &tree,
                &selected(selected_path),
                platform::parse_flag("8.5.1").unwrap()
            )
            .is_err()
        );
        assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), expected);
    }
}

#[test]
fn cancellation_preserves_database_and_existing_backup() {
    let _serial = serial();
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let (_scratch, db, tree) = setup();
    fs::write(
        tree.join(MODULE),
        fs::read_to_string(tree.join(MODULE)).unwrap() + "\r\n// cancelled\r\n",
    )
    .unwrap();
    let plan = prepare(&db, &tree, MODULE);
    let before = fs::read(db.join("1Cv8.1CD")).unwrap();
    fs::write(
        ibcmd_filedb::backup_file(&db),
        b"previous recovery artifact",
    )
    .unwrap();
    let flag = Arc::new(AtomicBool::new(false));
    let _scope = ibcmd_rs::cancel::scope(flag.clone());
    flag.store(true, Ordering::SeqCst);
    let error = plan.publish(&db).unwrap_err();
    assert!(ibcmd_rs::cancel::is_cancelled(&error));
    assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), before);
    assert_eq!(
        fs::read(ibcmd_filedb::backup_file(&db)).unwrap(),
        b"previous recovery artifact"
    );
}
#[test]
fn late_publication_rejection_preserves_the_previous_backup() {
    let _serial = serial();
    let (_scratch, db, _) = setup();
    let before = fs::read(db.join("1Cv8.1CD")).unwrap();
    fs::write(ibcmd_filedb::backup_file(&db), b"previous backup").unwrap();
    let mut checks = 0;
    let session = ibcmd_filedb::open_for_write(&db).unwrap();
    let result = session.replace_checked("CONFIGSAVE", &[], || {
        checks += 1;
        if checks == 2 {
            Err(ibcmd_filedb::Error::Refused(
                "cancelled before publication".into(),
            ))
        } else {
            Ok(())
        }
    });
    assert!(result.is_err());
    assert_eq!(checks, 2);
    assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), before);
    assert_eq!(
        fs::read(ibcmd_filedb::backup_file(&db)).unwrap(),
        b"previous backup"
    );
}
#[test]
fn interruption_child() {
    let _serial = serial();
    let Some(db) = std::env::var_os("ISSUE382_INTERRUPTION_CHILD") else {
        return;
    };
    let session = ibcmd_filedb::open_for_write(Path::new(&db)).unwrap();
    let mut checks = 0;
    session
        .replace_checked("CONFIGSAVE", &[], || {
            checks += 1;
            if checks == 2 {
                std::process::exit(73)
            }
            Ok(())
        })
        .unwrap();
    panic!("interruption callback was not reached");
}
#[test]
fn process_interruption_preserves_published_files_and_refuses_stale_work() {
    let _serial = serial();
    let (_scratch, db, _) = setup();
    let before = fs::read(db.join("1Cv8.1CD")).unwrap();
    fs::write(ibcmd_filedb::backup_file(&db), b"old backup").unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "interruption_child", "--nocapture"])
        .env("ISSUE382_INTERRUPTION_CHILD", &db)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(73));
    assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), before);
    assert_eq!(
        fs::read(ibcmd_filedb::backup_file(&db)).unwrap(),
        b"old backup"
    );
    let session = ibcmd_filedb::open_for_write(&db).unwrap();
    assert!(session.replace("CONFIGSAVE", &[]).is_err());
    assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), before);
}

#[cfg(unix)]
#[test]
fn operator_guards_check_consumed_identity_and_final_publication_policy() {
    let _serial = serial();
    use std::os::unix::fs::MetadataExt;
    let (_scratch, db, tree) = setup();
    fs::write(
        tree.join(MODULE),
        fs::read_to_string(tree.join(MODULE)).unwrap() + "\r\n// guarded\r\n",
    )
    .unwrap();
    let pinned = fs::metadata(db.join("1Cv8.1CD")).unwrap();
    let plan = file_import::prepare_guarded(
        &db,
        &tree,
        &selected(MODULE),
        platform::parse_flag("8.5.1").unwrap(),
        &mut |metadata| {
            assert_eq!(
                (metadata.dev(), metadata.ino()),
                (pinned.dev(), pinned.ino())
            );
            Ok(())
        },
    )
    .unwrap();
    let before = fs::read(db.join("1Cv8.1CD")).unwrap();
    fs::write(ibcmd_filedb::backup_file(&db), b"previous backup").unwrap();
    let mut calls = 0;
    let result = plan.publish_guarded(&db, &mut |metadata| {
        calls += 1;
        assert_eq!(
            (metadata.dev(), metadata.ino()),
            (pinned.dev(), pinned.ino())
        );
        if calls == 3 {
            anyhow::bail!("operator policy revoked before publication");
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(calls, 3);
    assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), before);
    assert_eq!(
        fs::read(ibcmd_filedb::backup_file(&db)).unwrap(),
        b"previous backup"
    );
    fs::rename(db.join("1Cv8.1CD"), db.join("pinned-original.1CD")).unwrap();
    fs::write(db.join("1Cv8.1CD"), &before).unwrap();
    let error = file_import::prepare_guarded(
        &db,
        &tree,
        &selected(MODULE),
        platform::parse_flag("8.5.1").unwrap(),
        &mut |metadata| {
            if (metadata.dev(), metadata.ino()) != (pinned.dev(), pinned.ino()) {
                anyhow::bail!("startup target identity changed");
            }
            Ok(())
        },
    )
    .err()
    .unwrap();
    assert!(
        error
            .to_string()
            .contains("startup target identity changed")
    );
}
#[test]
fn stage_versions_cannot_name_an_absent_row() {
    let _serial = serial();
    let (_scratch, db, tree) = setup();
    let mut session = ibcmd_filedb::open_for_write(&db).unwrap();
    let mut rows = session.config_rows("CONFIG").unwrap();
    let versions = rows
        .iter_mut()
        .find(|row| row.file_name == "versions")
        .unwrap();
    let mut bytes = Vec::new();
    flate2::read::DeflateDecoder::new(versions.binary.as_slice())
        .read_to_end(&mut bytes)
        .unwrap();
    let mut text = String::from_utf8(bytes).unwrap();
    assert_eq!(text.pop(), Some('}'));
    text.push_str(
        ",\"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee\",00000000-0000-0000-0000-000000000001}",
    );
    use std::io::Write;
    let mut encoder =
        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(text.as_bytes()).unwrap();
    versions.binary = encoder.finish().unwrap();
    versions.data_size = versions.binary.len() as u64;
    session.replace("CONFIGSAVE", &rows).unwrap();
    let before = fs::read(db.join("1Cv8.1CD")).unwrap();
    let error = file_import::prepare(
        &db,
        &tree,
        &selected(MODULE),
        platform::parse_flag("8.5.1").unwrap(),
    )
    .err()
    .unwrap();
    assert!(
        format!("{error:#}").contains("unknown versions"),
        "{error:#}"
    );
    assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), before);
}

#[test]
fn cancellation_at_the_final_policy_gate_is_checked_after_long_checks() {
    let _serial = serial();
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let (_scratch, db, tree) = setup();
    fs::write(
        tree.join(MODULE),
        fs::read_to_string(tree.join(MODULE)).unwrap() + "\r\n// final cancel\r\n",
    )
    .unwrap();
    let plan = prepare(&db, &tree, MODULE);
    let before = fs::read(db.join("1Cv8.1CD")).unwrap();
    fs::write(ibcmd_filedb::backup_file(&db), b"old backup").unwrap();
    let flag = Arc::new(AtomicBool::new(false));
    let _scope = ibcmd_rs::cancel::scope(flag.clone());
    let mut calls = 0;
    let error = plan
        .publish_guarded(&db, &mut |_| {
            calls += 1;
            if calls == 3 {
                flag.store(true, Ordering::SeqCst);
            }
            Ok(())
        })
        .unwrap_err();
    assert!(ibcmd_rs::cancel::is_cancelled(&error));
    assert_eq!(calls, 3);
    assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), before);
    assert_eq!(
        fs::read(ibcmd_filedb::backup_file(&db)).unwrap(),
        b"old backup"
    );
}

#[test]
fn changed_import_uses_native_sync_versions_and_refuses_stale_staging() {
    let _serial = serial();
    let (_scratch, db, tree) = setup();
    fs::write(
        tree.join(MODULE),
        fs::read_to_string(tree.join(MODULE)).unwrap() + "\r\n// local change\r\n",
    )
    .unwrap();
    let selection = Selection {
        changed: true,
        ..Selection::default()
    };
    let plan = file_import::prepare(
        &db,
        &tree,
        &selection,
        platform::parse_flag("8.5.1").unwrap(),
    )
    .unwrap();
    assert_eq!(plan.summary().files, vec![MODULE]);
    // Publish another staged version after the synchronization baseline.
    prepare(&db, &tree, MODULE).publish(&db).unwrap();
    fs::write(
        tree.join(MODULE),
        fs::read(fixture().join("tiny-xml").join(MODULE)).unwrap(),
    )
    .unwrap();
    let before = fs::read(db.join("1Cv8.1CD")).unwrap();
    let error = file_import::prepare(
        &db,
        &tree,
        &selection,
        platform::parse_flag("8.5.1").unwrap(),
    )
    .err()
    .unwrap();
    assert!(
        format!("{error:#}").contains("target changed since the synchronized export"),
        "{error:#}"
    );
    assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), before);
}

#[test]
fn bounded_plan_retains_cancellation_control_for_publish() {
    let _serial = serial();
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let (_scratch, db, tree) = setup();
    fs::write(
        tree.join(MODULE),
        fs::read_to_string(tree.join(MODULE)).unwrap() + "\r\n// bounded\r\n",
    )
    .unwrap();
    let original = fs::read(db.join("1Cv8.1CD")).unwrap();
    let cancelled = Arc::new(AtomicBool::new(false));
    let callback = cancelled.clone();
    let plan = file_import::prepare_guarded_with_limits(
        &db,
        &tree,
        &selected(MODULE),
        platform::parse_flag("8.5.1").unwrap(),
        &mut |_| Ok(()),
        file_import::ReadControl {
            limits: ibcmd_filedb::ReadLimits {
                max_records: 1_000_000,
                max_binary_bytes: 1 << 30,
                max_object_bytes: 1 << 30,
            },
            check: Arc::new(move || {
                if callback.load(Ordering::SeqCst) {
                    Err(ibcmd_filedb::Error::Refused("adapter cancelled".into()))
                } else {
                    Ok(())
                }
            }),
        },
    )
    .unwrap();
    cancelled.store(true, Ordering::SeqCst);
    assert!(
        plan.publish_guarded(&db, &mut |_| Ok(()))
            .unwrap_err()
            .to_string()
            .contains("adapter cancelled")
    );
    assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), original);
}

#[test]
fn bounded_plan_checks_retained_callback_after_final_operator_guard() {
    let _serial = serial();
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let (_scratch, db, tree) = setup();
    fs::write(
        tree.join(MODULE),
        fs::read_to_string(tree.join(MODULE)).unwrap() + "\r\n// deadline\r\n",
    )
    .unwrap();
    let original = fs::read(db.join("1Cv8.1CD")).unwrap();
    fs::write(ibcmd_filedb::backup_file(&db), b"older recovery").unwrap();
    let cancelled = Arc::new(AtomicBool::new(false));
    let callback = cancelled.clone();
    let plan = file_import::prepare_guarded_with_limits(
        &db,
        &tree,
        &selected(MODULE),
        platform::parse_flag("8.5.1").unwrap(),
        &mut |_| Ok(()),
        file_import::ReadControl {
            limits: ibcmd_filedb::ReadLimits {
                max_records: 1_000_000,
                max_binary_bytes: 1 << 30,
                max_object_bytes: 1 << 30,
            },
            check: Arc::new(move || {
                if callback.load(Ordering::SeqCst) {
                    Err(ibcmd_filedb::Error::Refused("adapter deadline".into()))
                } else {
                    Ok(())
                }
            }),
        },
    )
    .unwrap();
    let mut gates = 0;
    let error = plan
        .publish_guarded(&db, &mut |_| {
            gates += 1;
            if gates == 3 {
                cancelled.store(true, Ordering::SeqCst);
            }
            Ok(())
        })
        .unwrap_err();
    assert!(error.to_string().contains("adapter deadline"));
    assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), original);
    assert_eq!(
        fs::read(ibcmd_filedb::backup_file(&db)).unwrap(),
        b"older recovery"
    );
}
