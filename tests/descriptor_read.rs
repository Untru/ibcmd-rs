//! Public point reads must not turn present invalid values into guessed XML.
#[path = "common/mcp_export_fixture.rs"]
mod fixture;
use fixture::{Case, Scenario};
use ibcmd_rs::{
    metadata_model::brace::Brace,
    stored_objects::{ConfigSource, StoredConfiguration},
};
const DESCRIPTOR: &str = "Catalogs/CorpusList/Forms/ListForm.xml";
const MODULE: &str = "Catalogs/CorpusList/Forms/ListForm/Ext/Form/Module.bsl";
fn config(c: &Case) -> StoredConfiguration {
    StoredConfiguration::new(
        if c.rows_dir {
            ConfigSource::RowsDir(c.rows.clone())
        } else {
            ConfigSource::FileDb(c.database.clone())
        },
        ibcmd_rs::platform::parse("8.5.1").unwrap(),
    )
}
#[test]
fn present_invalid_requested_descriptor_is_refused_cold_and_cached() {
    let mut failures = Vec::new();
    for rows in [false, true] {
        for value in [
            Brace::num(99),
            Brace::atom("unknown"),
            Brace::str("unknown"),
            Brace::list(vec![Brace::num(99)]),
        ] {
            let c = Case::new(Scenario::NestedInvalid, rows);
            c.nested_value(value.clone());
            let before = c.input();
            for warm in [false, true] {
                let mut view = config(&c);
                if warm {
                    view.export(&[c.object.clone()], &|_, _, _| {}).unwrap();
                }
                for repeat in 0..2 {
                    let refused = view.read_file(DESCRIPTOR).is_err();
                    eprintln!(
                        "requested descriptor rows={rows} value={value:?} warm={warm} repeat={repeat}: refused={refused}"
                    );
                    if !refused {
                        failures.push(format!(
                            "rows={rows} value={value:?} warm={warm} repeat={repeat}"
                        ));
                    }
                    let (map, path) = view.read_file(MODULE).unwrap();
                    assert!(map.files.contains_key(&path));
                }
            }
            assert_eq!(c.input(), before);
        }
    }
    assert!(
        failures.is_empty(),
        "guessed descriptor XML returned: {failures:?}"
    );
}
#[test]
fn requested_read_preserves_independent_files_absence_and_legacy_bytes() {
    for rows in [false, true] {
        for (scenario, paths) in [
            (
                Scenario::NestedInvalid,
                vec![
                    "Catalogs/CorpusList.xml",
                    MODULE,
                    "Catalogs/CorpusList/Forms/ListForm/Ext/Form.xml",
                ],
            ),
            (
                Scenario::OpaqueForm,
                vec![
                    "CommonForms/OpaqueForm.xml",
                    "CommonForms/OpaqueForm/Ext/Form/Module.bsl",
                    "CommonForms/OpaqueForm/Ext/Form.xml",
                ],
            ),
            (
                Scenario::InvalidDescriptor,
                vec![
                    "CommonModules/ФайловаяБазаПроверка.xml",
                    "CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl",
                    "Languages/English.xml",
                ],
            ),
            (
                Scenario::LegacyCommand,
                vec!["CommonCommands/LegacyCommand.xml"],
            ),
        ] {
            let c = Case::new(scenario, rows);
            let before = c.input();
            let mut view = config(&c);
            for path in paths.into_iter().chain(
                ["Catalogs/CorpusList/Forms/ListForm/Ext/nonexistent.txt"]
                    .into_iter()
                    .filter(|_| matches!(scenario, Scenario::NestedInvalid)),
            ) {
                let name = ibcmd_rs::stored_objects::object_of_path(path).unwrap();
                let expected = view.export(&[name], &|_, _, _| {}).unwrap()[0]
                    .files
                    .get(path)
                    .cloned();
                let (map, normalized) = view.read_file(path).unwrap();
                assert_eq!(
                    map.files.get(&normalized).cloned(),
                    expected,
                    "{scenario:?} {path}"
                );
            }
            assert_eq!(c.input(), before);
        }
    }
}
#[test]
fn requested_descriptor_proof_is_invalidated_with_versions() {
    for rows in [false, true] {
        let c = Case::new(Scenario::NestedInvalid, rows);
        c.nested_value(Brace::flag(false));
        let mut view = config(&c);
        let (first, path) = view.read_file(DESCRIPTOR).unwrap();
        let first_version = first.version.clone();
        let first = first.files[&path].clone();
        c.nested_value(Brace::num(99));
        assert!(
            view.read_file(DESCRIPTOR).is_err(),
            "changed descriptor cannot reuse prior semantic proof"
        );
        c.nested_value(Brace::flag(true));
        let (last, path) = view.read_file(DESCRIPTOR).unwrap();
        assert_ne!(last.files[&path], first);
        assert_ne!(last.version, first_version);
    }
}

#[test]
fn documented_form12_keeps_valid_bytes_and_refuses_present_invalid_values() {
    let mut failures = Vec::new();
    for rows in [false, true] {
        for value in [
            Brace::flag(false),
            Brace::flag(true),
            Brace::num(99),
            Brace::atom("unknown"),
            Brace::str("unknown"),
            Brace::list(vec![Brace::num(99)]),
        ] {
            let c = Case::new(Scenario::NestedInvalid, rows);
            c.legacy_form(value.clone());
            let before = c.input();
            let valid = matches!(value.as_atom(), Some("0" | "1"));
            let expected = config(&c)
                .export(&[c.object.clone()], &|_, _, _| {})
                .unwrap()[0]
                .files
                .clone();
            if valid {
                let current = Case::new(Scenario::NestedInvalid, rows);
                current.nested_value(value.clone());
                let canonical = config(&current)
                    .export(&[current.object.clone()], &|_, _, _| {})
                    .unwrap();
                assert_eq!(
                    expected, canonical[0].files,
                    "documented12->13 must preserve the entire tree"
                );
            }
            for warm in [false, true] {
                let mut view = config(&c);
                if warm {
                    view.export(&["Catalog.CorpusList".into()], &|_, _, _| {})
                        .unwrap();
                }
                for repeat in 0..2 {
                    let result = view.read_file(DESCRIPTOR);
                    if valid {
                        let (map, path) = result.unwrap();
                        assert_eq!(map.files[&path], expected[DESCRIPTOR]);
                    } else if result.is_ok() {
                        failures.push(format!(
                            "read rows={rows} value={value:?} warm={warm} repeat={repeat}"
                        ));
                    }
                    let (map, path) = view.read_file(MODULE).unwrap();
                    assert_eq!(map.files[&path], expected[MODULE]);
                }
                let result = view.export_complete(&[c.object.clone()], &|_, _, _| {});
                if valid {
                    assert_eq!(result.unwrap()[0].files, expected);
                } else if result.is_ok() {
                    failures.push(format!("export rows={rows} value={value:?} warm={warm}"));
                }
            }
            assert_eq!(c.input(), before);
        }
    }
    assert!(
        failures.is_empty(),
        "documented legacy shape bypassed typed validation: {failures:?}"
    );
}
#[test]
fn documented_form12_also_validates_with_legacy_writer() {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "documented_form12_keeps_valid_bytes_and_refuses_present_invalid_values",
            "--nocapture",
        ])
        .env("IBCMD_RS_LEGACY_EXPORT", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
