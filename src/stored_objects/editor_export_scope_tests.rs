//! Tiny fixtures, removed with their case; work budgets are counts, never
//! wall-clock assertions.
use super::*;
use std::io::Read;

struct Case {
    root: crate::test_scratch::TestScratch,
    db: PathBuf,
    source: PathBuf,
    stage: PathBuf,
}
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/file-infobase")
        .join(name)
}
fn unpack(path: &Path) -> Vec<u8> {
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(fs::File::open(path).unwrap())
        .read_to_end(&mut bytes)
        .unwrap();
    bytes
}
fn case() -> Case {
    let root = crate::test_scratch::TestScratch::adopt(
        std::env::temp_dir().join(format!("editor-export-read-scope-{}", uuid::Uuid::new_v4())),
    );
    fs::create_dir_all(root.join("db")).unwrap();
    let db = root.join("db/1Cv8.1CD");
    fs::write(&db, unpack(&fixture("tiny.1CD.gz"))).unwrap();
    let source = root.join("source");
    fs::create_dir(&source).unwrap();
    fs::write(
        source.join("ConfigDumpInfo.xml"),
        fs::read(fixture("tiny-xml/ConfigDumpInfo.xml")).unwrap(),
    )
    .unwrap();
    let stage = root.join("stage");
    fs::create_dir(&stage).unwrap();
    Case {
        root,
        db,
        source,
        stage,
    }
}
fn config(path: &Path) -> StoredConfiguration {
    StoredConfiguration::new(
        ConfigSource::FileDb(path.to_owned()),
        crate::platform::parse("8.5.1").unwrap(),
    )
}
#[test]
fn selected_file_database_uses_two_fresh_loads_not_one_per_phase() {
    let c = case();
    let mut configuration = config(&c.db);
    let before = crate::mssql_dump::file_db_fresh_load_count();
    let rows = std::cell::RefCell::new(Vec::new());
    let plan = configuration
        .prepare_editor_export(
            &["CommonModule.ФайловаяБазаПроверка".into()],
            false,
            &c.source,
            &c.stage,
            &|done, _, what| {
                if done == 0 && what.starts_with("export of ") {
                    rows.borrow_mut().push(what.to_owned());
                }
            },
        )
        .unwrap();
    assert_eq!(
        plan.files.len(),
        3,
        "only selected descriptor/module plus inventory"
    );
    assert_eq!(rows.borrow().as_slice(), ["export of 1 objects (2 rows)"]);
    let emitted = EXPORTED_FILE_PATHS.with(|paths| paths.borrow().clone());
    let selected_paths = plan
        .files
        .iter()
        .filter(|p| p.as_str() != "ConfigDumpInfo.xml")
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        emitted, selected_paths,
        "converter itself must emit only selected files, not a full export later partitioned"
    );
    assert_eq!(
        crate::mssql_dump::file_db_fresh_load_count() - before,
        2,
        "one immutable raw read for all planning/conversion phases plus genuinely fresh final inventory"
    );
    for file in &plan.files {
        assert_eq!(
            fs::read(c.stage.join(file)).unwrap(),
            fs::read(fixture("tiny-xml").join(file)).unwrap()
        );
    }
}

fn changed_bytes() -> Vec<u8> {
    unpack(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/native-export-commands/change/database.1CD.gz"),
    )
}
fn version_rows(path: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    crate::mssql_dump::with_file_db_table(path, |table| {
        table.names()?;
        table.rows(&BTreeSet::from(["versions".into()]))
    })
}
#[test]
fn scopes_are_canonical_operation_local_and_do_not_replace_another_database() {
    let a = case();
    let b = case();
    fs::write(&b.db, changed_bytes()).unwrap();
    let _engine = engine();
    let before = crate::mssql_dump::file_db_fresh_load_count();
    let scope = crate::mssql_dump::FileDbReadScope::begin(a.db.parent().unwrap()).unwrap();
    let first = version_rows(&a.db).unwrap();
    let other = version_rows(&b.db).unwrap();
    assert_ne!(first, other);
    assert_eq!(first, version_rows(a.db.parent().unwrap()).unwrap());
    assert!(
        crate::mssql_dump::FileDbReadScope::begin(&a.db).is_err(),
        "nesting could make final inventory stale"
    );
    assert_eq!(
        crate::mssql_dump::file_db_fresh_load_count() - before,
        2,
        "other DB is fresh; directory/file aliases reuse the original scope"
    );
    drop(scope);
    assert!(!crate::mssql_dump::file_db_read_scope_active());
    assert_eq!(first, version_rows(&a.db).unwrap());
    assert_eq!(
        crate::mssql_dump::file_db_fresh_load_count() - before,
        3,
        "scope must never survive the operation"
    );
}
#[test]
fn cancellation_on_a_scoped_hit_preserves_type_and_drops_operation_rows() {
    const CHILD: &str = "IBCMD_EDITOR_READ_SCOPE_CANCEL_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","stored_objects::editor_export::read_scope_tests::cancellation_on_a_scoped_hit_preserves_type_and_drops_operation_rows","--nocapture"]).env(CHILD,"1").output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let c = case();
    let before = crate::mssql_dump::file_db_fresh_load_count();
    let result: Result<()> = (|| {
        let _engine = engine();
        let _read_scope = crate::mssql_dump::FileDbReadScope::begin(&c.db)?;
        let _cancel = crate::cancel::scope(Arc::new(std::sync::atomic::AtomicBool::new(true)));
        version_rows(&c.db)?;
        Ok(())
    })();
    assert!(crate::cancel::is_cancelled(&result.unwrap_err()));
    assert!(!crate::mssql_dump::file_db_read_scope_active());
    assert_eq!(
        crate::mssql_dump::file_db_fresh_load_count() - before,
        1,
        "cancelled cache hit must not reload or swallow cancellation"
    );
    fs::write(&c.db, changed_bytes()).unwrap();
    let actual = config(&c.db).native_config_dump_info().unwrap();
    let expected = fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/native-export-commands/change/full/ConfigDumpInfo.xml"),
    )
    .unwrap();
    assert_eq!(
        actual, expected,
        "the next request must read current rows after cancellation"
    );
}
#[test]
fn invalid_selection_cannot_leak_rows_to_the_next_request() {
    let c = case();
    let mut configuration = config(&c.db);
    let result = configuration.prepare_editor_export(
        &["CommonModule.Missing".into()],
        false,
        &c.source,
        &c.stage,
        &|_, _, _| {},
    );
    assert!(result.is_err());
    assert!(!crate::mssql_dump::file_db_read_scope_active());
    fs::write(&c.db, changed_bytes()).unwrap();
    let next = c.root.join("next-stage");
    fs::create_dir(&next).unwrap();
    configuration
        .prepare_editor_export(
            &["CommonModule.ФайловаяБазаПроверка".into()],
            false,
            &c.source,
            &next,
            &|_, _, _| {},
        )
        .unwrap();
    let module = "CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl";
    assert_eq!(
        fs::read(next.join(module)).unwrap(),
        fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/native-export-commands/change/full")
                .join(module)
        )
        .unwrap()
    );
}
#[test]
fn final_inventory_is_fresh_and_rejects_mid_operation_configuration_change() {
    let c = case();
    let mut configuration = config(&c.db);
    let replacement = changed_bytes();
    let replaced = std::cell::Cell::new(false);
    let source_before = fs::read(c.source.join("ConfigDumpInfo.xml")).unwrap();
    let before = crate::mssql_dump::file_db_fresh_load_count();
    let result = configuration.prepare_editor_export(
        &["CommonModule.ФайловаяБазаПроверка".into()],
        false,
        &c.source,
        &c.stage,
        &|_, _, what| {
            if what == "editor export: verify inventory" {
                assert!(!crate::mssql_dump::file_db_read_scope_active());
                fs::write(&c.db, &replacement).unwrap();
                replaced.set(true);
            }
        },
    );
    assert!(replaced.get());
    assert!(
        result
            .err()
            .unwrap()
            .to_string()
            .contains("Database changed")
    );
    assert_eq!(crate::mssql_dump::file_db_fresh_load_count() - before, 2);
    assert!(!crate::mssql_dump::file_db_read_scope_active());
    assert!(
        !c.stage.join("ConfigDumpInfo.xml").exists(),
        "failed plan must not advertise a completed inventory"
    );
    assert_eq!(
        fs::read(c.source.join("ConfigDumpInfo.xml")).unwrap(),
        source_before
    );
}
#[test]
fn scoped_reads_keep_completed_and_incomplete_configsave_publication_semantics() {
    let c = case();
    let changed = case();
    fs::write(&changed.db, changed_bytes()).unwrap();
    let mut reader = ibcmd_filedb::FileDb::open(&changed.db).unwrap();
    let mut staged = reader.config_rows("CONFIGSAVE").unwrap();
    assert!(
        !staged.is_empty(),
        "native change fixture carries a completed CONFIGSAVE stage"
    );
    drop(reader);
    ibcmd_filedb::replace_config_table(&c.db, "CONFIGSAVE", &staged).unwrap();
    let module = "CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl";
    let expected = fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/native-export-commands/change/full")
            .join(module),
    )
    .unwrap();
    let before = crate::mssql_dump::file_db_fresh_load_count();
    config(&c.db)
        .prepare_editor_export(
            &["CommonModule.ФайловаяБазаПроверка".into()],
            false,
            &c.source,
            &c.stage,
            &|_, _, _| {},
        )
        .unwrap();
    assert_eq!(fs::read(c.stage.join(module)).unwrap(), expected);
    assert_eq!(crate::mssql_dump::file_db_fresh_load_count() - before, 2);
    staged.push(ibcmd_filedb::ConfigRow {
        file_name: "commit".into(),
        part_no: 0,
        creation: ibcmd_filedb::Timestamp::now_utc(),
        modified: ibcmd_filedb::Timestamp::now_utc(),
        attributes: 0,
        data_size: 1,
        binary: vec![0],
    });
    ibcmd_filedb::replace_config_table(&c.db, "CONFIGSAVE", &staged).unwrap();
    let incomplete = c.root.join("incomplete-stage");
    fs::create_dir(&incomplete).unwrap();
    config(&c.db)
        .prepare_editor_export(
            &["CommonModule.ФайловаяБазаПроверка".into()],
            false,
            &c.source,
            &incomplete,
            &|_, _, _| {},
        )
        .unwrap();
    assert_eq!(
        fs::read(incomplete.join(module)).unwrap(),
        fs::read(fixture("tiny-xml").join(module)).unwrap()
    );
    assert!(!crate::mssql_dump::file_db_read_scope_active());
}

#[test]
fn strict_sql_main_selection_keeps_its_scope_with_an_immutable_offline_snapshot() {
    use crate::metadata_model::brace::{Brace, parse_row, serialize_row};
    let c = case();
    let mut db = ibcmd_filedb::FileDb::open(&c.db).unwrap();
    let mut rows = db.config_rows("CONFIG").unwrap();
    drop(db);
    let descriptor = rows
        .iter_mut()
        .find(|row| row.file_name == "6d1c2a0e-1cd0-4f00-9a00-000000000001")
        .unwrap();
    let mut parsed =
        parse_row(&crate::module_blob::inflate_raw(&descriptor.binary).unwrap()).unwrap();
    parsed.as_list_mut().unwrap()[1].as_list_mut().unwrap()[8] = Brace::num(99);
    descriptor.binary = crate::module_blob::deflate_raw(&serialize_row(&parsed)).unwrap();
    let mut config = StoredConfiguration::from_rows_snapshot(
        ConfigSource::Database {
            sql: SqlExec::detached("offline snapshot answers every query; never connect"),
            database: "editor-selected-main".into(),
        },
        crate::platform::parse("8.5.1").unwrap(),
        rows.into_iter()
            .map(|row| (row.file_name, Arc::new(row.binary)))
            .collect(),
    );
    let names = ["Language.English".into()];
    config
        .main_view_mut()
        .export(&names, &|_, _, _| {})
        .unwrap();
    let plan = config
        .prepare_editor_export(&names, false, &c.source, &c.stage, &|_, _, _| {})
        .unwrap();
    assert_eq!(plan.files, ["ConfigDumpInfo.xml", "Languages/English.xml"]);
    assert_eq!(
        fs::read(c.stage.join("Languages/English.xml")).unwrap(),
        fs::read(fixture("tiny-xml/Languages/English.xml")).unwrap()
    );
    assert_eq!(
        EXPORTED_FILE_PATHS.with(|paths| paths.borrow().clone()),
        ["Languages/English.xml"],
        "SQL main overlay must not expand the conversion or its proof to unselected rows"
    );
    let rejected = c.root.join("rejected-stage");
    fs::create_dir(&rejected).unwrap();
    assert!(
        config
            .prepare_editor_export(
                &[names[0].clone(), "CommonModule.ФайловаяБазаПроверка".into()],
                false,
                &c.source,
                &rejected,
                &|_, _, _| {}
            )
            .is_err()
    );
    assert!(fs::read_dir(rejected).unwrap().next().is_none());
}

#[test]
fn selected_validation_preserves_each_backends_original_configuration_writer() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/external/upgrade/exchange_plan/current.cf");
    let limits =
        ibcmd_core::limits::ResourceLimits::for_input_bytes(fs::metadata(&path).unwrap().len());
    let profile = ibcmd_core::artifact::StorageProfileId::parse("storage:cf-cli").unwrap();
    let archive =
        ibcmd_cf::archive::decode_packed_archive(fs::File::open(path).unwrap(), limits, profile)
            .unwrap();
    let rows = archive
        .entries()
        .iter()
        .map(|entry| (entry.name().to_owned(), Arc::new(entry.payload().to_vec())))
        .collect::<BTreeMap<_, _>>();
    let mut descriptors = Vec::new();
    for sql in [false, true] {
        let c = case();
        let source = if sql {
            ConfigSource::Database {
                sql: SqlExec::detached("memory snapshot answers every query"),
                database: "editor-writer-scope".into(),
            }
        } else {
            ConfigSource::RowsDir(c.root.join("unused-rows"))
        };
        let mut config = StoredConfiguration::from_rows_snapshot(
            source.clone(),
            crate::platform::parse("8.3.27").unwrap(),
            rows.clone(),
        );
        let names = ["Configuration".to_owned()];
        let observed = config
            .main_view_mut()
            .export(&names, &|_, _, _| {})
            .unwrap()
            .remove(0);
        // A diagnostic cache is not a typed-value proof; point reads must
        // revalidate without changing either backend's established writer.
        let (read, path) = config
            .main_view_mut()
            .read_file("Configuration.xml")
            .unwrap();
        assert_eq!(
            read.files[&path], observed.files[&path],
            "warm point read sql={sql}"
        );
        let mut cold = StoredConfiguration::from_rows_snapshot(
            source,
            crate::platform::parse("8.3.27").unwrap(),
            rows.clone(),
        );
        let (read, path) = cold.main_view_mut().read_file("Configuration.xml").unwrap();
        assert_eq!(
            read.files[&path], observed.files[&path],
            "cold point read sql={sql}"
        );
        fs::write(
            c.source.join("ConfigDumpInfo.xml"),
            config.native_config_dump_info().unwrap(),
        )
        .unwrap();
        config
            .prepare_editor_export(&names, false, &c.source, &c.stage, &|_, _, _| {})
            .unwrap();
        for (path, expected) in &observed.files {
            assert_eq!(
                &fs::read(c.stage.join(path)).unwrap(),
                expected,
                "writer drift for sql={sql}: {path}"
            );
        }
        descriptors.push(observed.files["Configuration.xml"].clone());
    }
    assert_ne!(
        descriptors[0], descriptors[1],
        "fixture must actually distinguish the pre-existing model/legacy writer defaults"
    );
}
