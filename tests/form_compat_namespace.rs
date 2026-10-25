//! A form's root element declares `xmlns:dcssch` only when the configuration
//! (or extension) it belongs to runs in compatibility mode 8.3.19 or later.
//! Each fixture is a file built by 8.3.27.2214 beside that platform's own dump
//! of its one common form (`_onecdec/make_compat_form_fixtures.py`).

use crate::common;

use std::fs;

fn exported_form(case: &str, input: &str, form: &str) -> (Vec<u8>, Vec<u8>) {
    let dir = common::fixture("compat_forms").join(case);
    let out = common::temp_dir(&format!("compat-{case}"));
    let run = common::export(&dir.join(input), &out);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let path = format!("CommonForms/{form}/Ext/Form.xml");
    (
        fs::read(dir.join("Form.xml")).unwrap(),
        fs::read(out.join(path)).unwrap(),
    )
}

fn assert_same(case: &str, input: &str, form: &str) {
    let (expected, actual) = exported_form(case, input, form);
    assert!(
        expected == actual,
        "{case}: Form.xml differs\n--- native\n{}\n--- ours\n{}",
        String::from_utf8_lossy(&expected),
        String::from_utf8_lossy(&actual)
    );
}

#[test]
fn configuration_below_8_3_19_writes_forms_without_dcssch() {
    assert_same("cf_8_3_18", "input.cf", "Форма");
}

#[test]
fn configuration_from_8_3_19_writes_forms_with_dcssch() {
    assert_same("cf_8_3_19", "input.cf", "Форма");
}

#[test]
fn extension_below_8_3_19_writes_forms_without_dcssch() {
    assert_same("cfe_8_3_18", "input.cfe", "ТестРасширение_Форма");
}

#[test]
fn extension_from_8_3_19_writes_forms_with_dcssch() {
    assert_same("cfe_8_3_19", "input.cfe", "ТестРасширение_Форма");
}
