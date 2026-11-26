//! Adopted objects of an extension: `ObjectBelonging`, the extended
//! configuration object, only the controlled properties, and the modules and
//! forms the extension extends as `xr:PropertyState`. Each fixture is an
//! extension built by 8.3.27.2214 over a base configuration, beside that
//! platform's own dump of the adopted objects
//! (`_onecdec/make_adopted_fixtures.py`).

use crate::common;

use std::{fs, path::Path};

fn assert_case(case: &str) {
    let dir = common::fixture("adopted").join(case);
    let out = common::temp_dir(&format!("adopted-{case}"));
    let run = common::export(&dir.join("input.cfe"), &out);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stdout)
    );
    let mut compared = 0;
    for (rel, expected) in common::files(&dir) {
        if rel == "input.cfe" {
            continue;
        }
        let actual = fs::read(Path::new(&out).join(&rel)).unwrap_or_default();
        assert!(
            expected == actual,
            "{case}: {rel} differs\n--- native\n{}\n--- ours\n{}",
            String::from_utf8_lossy(&expected),
            String::from_utf8_lossy(&actual)
        );
        compared += 1;
    }
    assert!(compared > 0, "{case}: nothing compared");
}

#[test]
fn common_module_with_every_flag_controlled_and_the_module_extended() {
    assert_case("module_all");
}

#[test]
fn common_module_flag_partitions() {
    assert_case("module_b0");
    assert_case("module_b1");
    assert_case("module_b2");
}

#[test]
fn catalog_with_its_object_module_extended() {
    assert_case("catalog_object_module");
}

#[test]
fn catalog_with_both_modules_and_a_form_extended() {
    assert_case("catalog_modules");
}

#[test]
fn adopted_form_intercepting_base_events_before_after_and_instead() {
    // Its `<Events>` carry `callType`, and `<BaseForm>` is the base form as
    // the extension adopted it, whose own events the platform writes `Before`.
    assert_case("form_events");
}

#[test]
fn one_handler_intercepting_two_events_under_two_call_types() {
    // `Расш_Общий` is `Before` on OnOpen and `After` on BeforeClose: keyed by
    // handler alone, both lost their callType.
    assert_case("form_events_shared");
}

#[test]
fn adopted_language_of_the_clean_room_extension() {
    let dir = common::fixture("test_extension");
    let out = common::temp_dir("adopted-language");
    let run = common::export(&dir.join("input.cfe"), &out);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stdout)
    );
    let expected = fs::read(dir.join("expected/Languages/Русский.xml")).unwrap();
    let actual = fs::read(out.join("Languages/Русский.xml")).unwrap();
    assert!(
        expected == actual,
        "--- native\n{}\n--- ours\n{}",
        String::from_utf8_lossy(&expected),
        String::from_utf8_lossy(&actual)
    );
}

#[test]
fn adopted_document_with_adopted_attributes_and_tabular_section() {
    // The owner-graph check refused the whole document (a real extension: 65 such
    // catalogs and documents): the adopted children's headers repeat the
    // adoption property uuids. Their XML carries only the adopted members.
    // Fixture: `_onecdec/make_adopted_children_fixture.py`.
    assert_case("document_children");
}
