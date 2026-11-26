//! The exact MCP export-arm helper, without changing transport/custody guards.
use super::*;
#[path = "../tests/common/mcp_export_fixture.rs"]
mod fixture;
use fixture::{Case, Scenario};
fn config(c: &Case) -> stored_objects::StoredConfiguration {
    let mut reader = ibcmd_filedb::FileDb::open(&c.database).unwrap();
    let rows = reader
        .config_rows("CONFIG")
        .unwrap()
        .into_iter()
        .map(|r| (r.file_name, Arc::new(r.binary)))
        .collect();
    let source = if c.rows_dir {
        stored_objects::ConfigSource::RowsDir(c.rows.clone())
    } else {
        stored_objects::ConfigSource::FileDb(c.database.clone())
    };
    stored_objects::StoredConfiguration::from_rows_snapshot(
        source,
        crate::platform::parse("8.5.1").unwrap(),
        rows,
    )
}
#[test]
fn actual_mcp_export_payload_refuses_loss_in_selected_data() {
    if crate::test_process::isolated_engine_test(
        "mcp::export_completeness_tests::actual_mcp_export_payload_refuses_loss_in_selected_data",
    ) {
        return;
    }
    let mut failures = Vec::new();
    for rows_dir in [false, true] {
        for scenario in [
            Scenario::InvalidDescriptor,
            Scenario::OpaqueForm,
            Scenario::NestedInvalid,
            Scenario::LegacyNestedInvalid,
        ] {
            let c = Case::new(scenario, rows_dir);
            let before = c.input();
            let mut view = config(&c);
            let result = export_payload(&mut view, &[c.object.clone()]);
            let no_partial_result = result.is_err();
            eprintln!(
                "actual MCP export payload {scenario:?}, rows_dir={rows_dir}: refused={no_partial_result}"
            );
            if !no_partial_result {
                failures.push(format!(
                    "{scenario:?}, rows_dir={rows_dir}: refused={no_partial_result}"
                ));
            }
            assert_eq!(
                c.input(),
                before,
                "DB, rows/source and sentinels must remain unchanged"
            );
        }
    }
    assert!(
        failures.is_empty(),
        "MCP returned successful incomplete file maps:\n{}",
        failures.join("\n")
    );
}
#[test]
fn mcp_payload_keeps_supported_selection_and_legacy_or_common_form_controls() {
    if crate::test_process::isolated_engine_test(
        "mcp::export_completeness_tests::mcp_payload_keeps_supported_selection_and_legacy_or_common_form_controls",
    ) {
        return;
    }
    for rows_dir in [false, true] {
        for scenario in [
            Scenario::InvalidDescriptor,
            Scenario::OpaqueForm,
            Scenario::LegacyCommand,
            Scenario::CommonForm,
            Scenario::LegacyNestedValid,
        ] {
            let c = Case::new(scenario, rows_dir);
            let before = c.input();
            let mut view = config(&c);
            let name = if matches!(scenario, Scenario::InvalidDescriptor | Scenario::OpaqueForm) {
                "Language.English".to_owned()
            } else {
                c.object.clone()
            };
            let expected = view.export(&[name.clone()], &|_, _, _| {}).unwrap();
            let result = export_payload(&mut view, &[name.clone()]).unwrap();
            assert_eq!(result["objects"][0]["object"], name);
            let files = result["objects"][0]["files"].as_object().unwrap();
            assert!(!files.is_empty());
            assert_eq!(files.len(), expected[0].files.len());
            for (path, bytes) in &expected[0].files {
                assert_eq!(
                    files[path]["content"].as_str().unwrap().as_bytes(),
                    bytes,
                    "{path}"
                );
            }
            if name == "Language.English" {
                assert_eq!(files.len(), 1);
                assert_eq!(
                    files["Languages/English.xml"]["content"],
                    fs::read_to_string(
                        Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("tests/fixtures/file-infobase/tiny-xml/Languages/English.xml")
                    )
                    .unwrap()
                );
            }
            if matches!(scenario, Scenario::CommonForm) {
                assert!(files.contains_key("CommonForms/OpaqueForm.xml"));
                assert!(files.contains_key("CommonForms/OpaqueForm/Ext/Form.xml"));
                assert!(files.contains_key("CommonForms/OpaqueForm/Ext/Form/Module.bsl"));
            }
            assert_eq!(c.input(), before);
        }
    }
}

#[test]
fn mcp_payload_retains_existing_home_page_configuration_bytes() {
    if crate::test_process::isolated_engine_test(
        "mcp::export_completeness_tests::mcp_payload_retains_existing_home_page_configuration_bytes",
    ) {
        return;
    }
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/external/home_page/one_column");
    let cf = root.join("input.cf");
    let limits =
        ibcmd_core::limits::ResourceLimits::for_input_bytes(fs::metadata(&cf).unwrap().len());
    let profile = ibcmd_core::artifact::StorageProfileId::parse("storage:cf-cli").unwrap();
    let archive =
        ibcmd_cf::archive::decode_packed_archive(fs::File::open(cf).unwrap(), limits, profile)
            .unwrap();
    let rows = archive
        .entries()
        .iter()
        .map(|entry| (entry.name().to_owned(), Arc::new(entry.payload().to_vec())))
        .collect();
    let mut view = stored_objects::StoredConfiguration::from_rows_snapshot(
        stored_objects::ConfigSource::RowsDir(root.join("unused")),
        crate::platform::parse("8.3.27").unwrap(),
        rows,
    );
    let names = ["Configuration".into()];
    let observed = view.export(&names, &|_, _, _| {}).unwrap();
    let result = export_payload(&mut view, &names).unwrap();
    let files = result["objects"][0]["files"].as_object().unwrap();
    assert_eq!(files.len(), observed[0].files.len());
    for (path, bytes) in &observed[0].files {
        assert_eq!(files[path]["encoding"], "utf-8");
        assert_eq!(
            files[path]["content"].as_str().unwrap().as_bytes(),
            bytes,
            "{path}"
        );
    }
    assert_eq!(
        files["Ext/HomePageWorkArea.xml"]["content"]
            .as_str()
            .unwrap()
            .as_bytes(),
        fs::read(root.join("HomePageWorkArea.xml")).unwrap()
    );
}
