//! Shared file backend contract for editor and agent consumers (#382).
use ibcmd_rs::{
    server::infobases::{self, EntryParams, Login, Session},
    settings::{Settings, SettingsSources},
    stored_objects::{ConfigSource, StatusKind},
};
use serde_json::json;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[path = "support/owned_scratch.rs"]
mod owned_scratch;
use owned_scratch::OwnedScratch;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/file-infobase")
        .join(name)
}
fn scratch() -> OwnedScratch {
    OwnedScratch::new("issue382-backend")
}

#[test]
fn file_session_reads_tree_files_and_status_without_sql_or_writes() {
    let dir = scratch();
    let db = dir.join("db");
    fs::create_dir_all(&db).unwrap();
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(fs::File::open(fixture("tiny.1CD.gz")).unwrap())
        .read_to_end(&mut bytes)
        .unwrap();
    fs::write(db.join("1Cv8.1CD"), &bytes).unwrap();
    let settings_file = dir.join("ibcmd-rs.toml");
    fs::write(
        &settings_file,
        "[[database]]\nname='demo'\nplatform='8.5.1'\ndb-path='db'\nagent-write=true\n",
    )
    .unwrap();
    let env = |key: &str| {
        (key == "IBCMD_RS_CONFIG").then(|| settings_file.to_string_lossy().into_owned())
    };
    let settings = Settings::from_sources(&SettingsSources {
        env: &env,
        executable: None,
        current_dir: None,
        app_data: None,
        native_config: None,
    })
    .unwrap();
    let binding = &settings.files()[0].databases()[0];
    assert_eq!(binding.db_path.as_deref(), Some(db.as_path()));
    assert!(binding.agent_write);
    let entries = infobases::entries(&settings, &[]).unwrap();
    assert_eq!(entries[0].source, "file-db");
    let mut session = Session::open(&entries[0], None, Login::default(), &settings).unwrap();
    assert!(matches!(
        session.configuration.source(),
        ConfigSource::FileDb(_)
    ));
    assert!(session.login.user.is_none());
    assert_eq!(
        session.configuration.children(None).unwrap()[0].id,
        "Configuration"
    );
    let module = "CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl";
    let (files, key) = session.configuration.read_file(module).unwrap();
    assert_eq!(
        files.files.get(&key).unwrap(),
        &fs::read(fixture("tiny-xml").join(module)).unwrap()
    );
    let status = session
        .configuration
        .status(None, &fixture("tiny-xml"), &|_, _, _| {})
        .unwrap();
    assert!(
        status
            .iter()
            .all(|status| status.status == StatusKind::Same),
        "{status:?}"
    );
    assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), bytes);
    assert!(!ibcmd_filedb::backup_file(&db).exists());
}
#[test]
fn invalid_file_selectors_are_rejected_before_target_open() {
    for value in [
        json!({"name":"demo", "dbPath":"missing", "rowsDir":"missing"}),
        json!({"name":"demo", "dbPath":"missing", "server":"sql"}),
        json!({"name":"demo_*", "dbPath":"missing"}),
        json!({"name":"demo", "dbPath":""}),
    ] {
        let entry: EntryParams = serde_json::from_value(value).unwrap();
        assert!(
            infobases::entries(&Settings::default(), &[entry])
                .unwrap_err()
                .to_string()
                .contains("dbPath")
        );
    }
}
#[test]
fn legacy_sql_and_rows_entries_keep_their_sources() {
    for (params, source) in [
        (json!({"name":"demo","server":"sql"}), "mssql"),
        (json!({"name":"rows","rowsDir":"missing"}), "rows-dir"),
    ] {
        let params: EntryParams = serde_json::from_value(params).unwrap();
        assert_eq!(
            infobases::entries(&Settings::default(), &[params]).unwrap()[0].source,
            source
        );
    }
}

#[test]
fn agent_write_is_explicit_and_cannot_authorize_a_mask() {
    let dir = scratch();
    let file = dir.join("ibcmd-rs.toml");
    let env = |key: &str| (key == "IBCMD_RS_CONFIG").then(|| file.to_string_lossy().into_owned());
    let sources = SettingsSources {
        env: &env,
        executable: None,
        current_dir: None,
        app_data: None,
        native_config: None,
    };
    fs::write(
        &file,
        "[[database]]\nname='demo'\nplatform='8.5.1'\ndb-path='db'\n",
    )
    .unwrap();
    assert!(!Settings::from_sources(&sources).unwrap().files()[0].databases()[0].agent_write);
    for text in [
        "[[database]]\nname='demo_*'\nplatform='8.5.1'\nagent-write=true\n",
        "[[database]]\nname='demo'\nplatform='8.5.1'\ndb-path='db'\nrows-dir='rows'\n",
        "[[database]]\nname='demo'\nplatform='8.5.1'\ndb-path=''\n",
    ] {
        fs::write(&file, text).unwrap();
        assert!(Settings::from_sources(&sources).is_err());
    }
}

#[test]
fn session_rechecks_public_entry_invariants_before_opening_the_target() {
    let params: EntryParams =
        serde_json::from_value(json!({"name":"demo", "dbPath":"missing", "platform":"8.5.1"}))
            .unwrap();
    let entry = infobases::entries(&Settings::default(), &[params])
        .unwrap()
        .remove(0);
    for mode in 0..4 {
        let mut bad = entry.clone();
        match mode {
            0 => bad.source = "mssql",
            1 => bad.rows_dir = Some("missing".into()),
            2 => bad.server = Some("sql".into()),
            _ => bad.mask = true,
        }
        let error = Session::open(&bad, None, Login::default(), &Settings::default())
            .err()
            .unwrap();
        assert!(error.to_string().contains("dbPath requires"), "{error:#}");
    }
}

#[cfg(unix)]
#[test]
fn descriptor_reader_consumes_the_open_inode_after_path_rebinding() {
    use std::os::unix::fs::MetadataExt;
    let dir = scratch();
    let path = dir.join("original.1CD");
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(fs::File::open(fixture("tiny.1CD.gz")).unwrap())
        .read_to_end(&mut bytes)
        .unwrap();
    fs::write(&path, bytes).unwrap();
    let file = fs::File::open(&path).unwrap();
    let identity = file.metadata().unwrap();
    fs::rename(&path, dir.join("preserved.1CD")).unwrap();
    fs::write(&path, b"not a database").unwrap();
    let mut reader = ibcmd_filedb::FileDb::from_file(file).unwrap();
    assert_eq!(
        (reader.metadata().dev(), reader.metadata().ino()),
        (identity.dev(), identity.ino())
    );
    assert!(!reader.config_rows("CONFIG").unwrap().is_empty());
    assert_eq!(fs::read(&path).unwrap(), b"not a database");
}
