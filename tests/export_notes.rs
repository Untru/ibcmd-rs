//! What the export report says about the parts it did not write. The
//! containers are platform-built fixtures damaged on purpose
//! (`_onecdec/make_export_notes_fixtures.py`).

use crate::common;

fn entry_message(input: &str, tag: &str, entry: &str) -> (String, String) {
    let out = common::temp_dir(tag);
    let run = common::export(&common::fixture("export_notes").join(input), &out);
    let report: serde_json::Value = serde_json::from_slice(if run.stdout.is_empty() {
        &run.stderr
    } else {
        &run.stdout
    })
    .unwrap();
    let found = report["export"]["storage"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|candidate| candidate["logical_name"] == entry)
        .unwrap_or_else(|| panic!("{entry} is not in the report"));
    (
        found["disposition"].as_str().unwrap_or_default().to_owned(),
        found["message"].as_str().unwrap_or_default().to_owned(),
    )
}

#[test]
fn a_metadata_object_not_written_is_named_with_its_reason() {
    // Before: "no legacy family decoder recognized this storage entry" --
    // true of root/version, misleading for an object that was recognized and
    // refused (1Cv8_обф.cf: Catalogs/узЗадачи.xml and two more).
    let (disposition, message) = entry_message(
        "metadata_miss.cf",
        "notes-metadata",
        "5b6f3a52-0c0e-4d0b-9d49-3f7f2f8e1a12",
    );
    assert_eq!(disposition, "opaque");
    assert!(
        message.contains("CommonForm 5b6f3a52-0c0e-4d0b-9d49-3f7f2f8e1a12 not written (header"),
        "{message}"
    );
}

#[test]
fn an_unreadable_object_keeps_the_kind_its_configuration_gives_it() {
    // The configuration root lists ФормаСправа among its common forms; its
    // own row, unreadable, used to be guessed a common picture, and its form
    // body was written as CommonPictures/ФормаСправа/Ext/Picture.xml.
    let out = common::temp_dir("notes-kind");
    let run = common::export(&common::fixture("export_notes").join("wrong_kind.cf"), &out);
    assert!(
        run.status.code().is_some(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(!out.join("CommonPictures").exists());
}

#[test]
fn a_withheld_form_body_is_named() {
    // A form whose Form.xml is withheld as opaque used to leave no trace in
    // the report: its entry read "supported" when the module was written.
    let (_, message) = entry_message(
        "withheld_form.cf",
        "notes-form",
        "5b6f3a52-0c0e-4d0b-9d49-3f7f2f8e1a21.0",
    );
    assert!(
        message.contains("CommonForms/ФормаДаты/Ext/Form.xml withheld (")
            && message.contains("choice_list"),
        "{message}"
    );
}

#[test]
fn an_unresolved_default_form_withholds_configuration_xml_and_says_so() {
    // Field 37 names no object of the container: the property used to vanish
    // silently from an otherwise written Configuration.xml. It fails closed,
    // as the reader always did, and the report names the file.
    let out = common::temp_dir("notes-search-form");
    let run = common::export(
        &common::fixture("export_notes").join("unresolved_search_form.cf"),
        &out,
    );
    let report = String::from_utf8_lossy(&run.stdout).into_owned();
    assert!(!out.join("Configuration.xml").exists(), "{report}");
    assert!(
        report.contains("Configuration.xml not written ("),
        "{report}"
    );
}
