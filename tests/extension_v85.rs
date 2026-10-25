//! `cf export --source-version 2.21` of a configuration extension: the whole
//! tree as platform 8.5.1.1529 dumps it. Each case is an extension fixture
//! already checked in for 8.3.27 (its `input.cfe` stays where it is), loaded
//! by 8.5.1.1529 over its base configuration and dumped by that platform
//! (`_onecdec/make_v85_extension_fixtures.py`).

use crate::common;

use std::{path::Path, process::Command};

fn assert_case(case: &str, input: &str) {
    let expected = common::fixture("v85_extension").join(case);
    let out = common::temp_dir(&format!("v85-extension-{case}"));
    let run = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "export"])
        .arg(common::fixture(input))
        .arg(&out)
        .args(["--platform", "8.5.1.1529", "--overwrite"])
        .output()
        .expect("run ibcmd-rs");
    assert!(
        run.status.success(),
        "{case}: {}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let (mut native, ours) = (common::files(&expected), common::files(Path::new(&out)));
    // Historical references came from Designer/SQL. Native file-CFE export
    // omits this service file; retain every metadata/body byte comparison.
    assert!(
        native.remove("ConfigDumpInfo.xml").is_some(),
        "{case}: missing Designer reference"
    );
    assert!(
        !ours.contains_key("ConfigDumpInfo.xml"),
        "{case}: file export wrote database dump info"
    );
    assert!(!native.is_empty(), "{case}: no native tree");
    let missing: Vec<_> = native
        .keys()
        .filter(|rel| !ours.contains_key(*rel))
        .collect();
    let extra: Vec<_> = ours
        .keys()
        .filter(|rel| !native.contains_key(*rel))
        .collect();
    let differ: Vec<_> = native
        .iter()
        .filter(|(rel, bytes)| ours.get(*rel).is_some_and(|actual| actual != *bytes))
        .map(|(rel, bytes)| {
            format!(
                "{rel}\n--- native\n{}\n--- ours\n{}",
                String::from_utf8_lossy(bytes),
                String::from_utf8_lossy(&ours[rel])
            )
        })
        .collect();
    assert!(
        missing.is_empty() && extra.is_empty() && differ.is_empty(),
        "{case}: missing {missing:?} extra {extra:?}\n{}",
        differ.join("\n")
    );
}

#[test]
fn clean_room_extension() {
    assert_case("test_extension", "test_extension/input.cfe");
}

#[test]
fn root_values_away_from_their_defaults() {
    assert_case("roots_values", "extension_roots/values/input.cfe");
}

#[test]
fn root_spellings_of_a_patch_without_mapping_by_ids() {
    assert_case("roots_spellings", "extension_roots/spellings/input.cfe");
}

#[test]
fn extended_root_modules_and_command_interfaces() {
    assert_case("roots_modules", "extension_roots/modules/input.cfe");
}

#[test]
fn own_role_among_the_default_roles() {
    assert_case("roots_roles", "extension_roots/roles/input.cfe");
}

#[test]
fn root_saved_by_8_5() {
    assert_case("roots_values_v85", "extension_roots/values_v85/input.cfe");
}

#[test]
fn root_captions_saved_by_8_5() {
    // Saved by 8.5 itself with both captions set: members 64/65 of `{76,…}`.
    assert_case(
        "roots_captions_v85",
        "v85_extension_inputs/roots_captions_v85/input.cfe",
    );
}

#[test]
fn common_form_saved_by_8_5() {
    assert_case("v85_form", "v85_form/input.cfe");
}

#[test]
fn own_form_events() {
    assert_case("form_events", "form_events/input.cfe");
}

#[test]
fn template_fonts() {
    assert_case("template_fonts", "template_fonts/input.cfe");
}

#[test]
fn adopted_common_modules() {
    for case in ["module_all", "module_b0", "module_b1", "module_b2"] {
        assert_case(
            &format!("adopted_{case}"),
            &format!("adopted/{case}/input.cfe"),
        );
    }
}

#[test]
fn adopted_catalogs_with_extended_modules() {
    assert_case(
        "adopted_catalog_modules",
        "adopted/catalog_modules/input.cfe",
    );
    assert_case(
        "adopted_catalog_object_module",
        "adopted/catalog_object_module/input.cfe",
    );
}

#[test]
fn adopted_form_interceptors() {
    assert_case("adopted_form_events", "adopted/form_events/input.cfe");
    assert_case(
        "adopted_form_events_shared",
        "adopted/form_events_shared/input.cfe",
    );
}

#[test]
fn adopted_document_with_adopted_children() {
    assert_case(
        "adopted_document_children",
        "adopted/document_children/input.cfe",
    );
}
