//! Form events 8.3.27.2214 has no name for keep their identifier in the 2.20
//! dialect: 8.5 spells them `OnHover` / `OnSelectedRowsSetChange`, 8.3.27
//! writes the uuid (ИТК: 21 and 3 bindings, every one by uuid). The fixture is
//! built and dumped by 8.3.27.2214 (`_onecdec/make_form_events_fixture.py`).

use crate::common;

use std::fs;

#[test]
fn table_events_without_an_8_3_27_name_keep_their_identifier() {
    let dir = common::fixture("form_events");
    let out = common::temp_dir("form-events");
    let run = common::export(&dir.join("input.cfe"), &out);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stdout)
    );
    let expected = fs::read(dir.join("Form.xml")).unwrap();
    let actual =
        fs::read(out.join("CommonForms/ТестРасширение_ФормаСобытий/Ext/Form.xml")).unwrap();
    assert!(
        expected == actual,
        "--- native\n{}\n--- ours\n{}",
        String::from_utf8_lossy(&expected),
        String::from_utf8_lossy(&actual)
    );
}

#[test]
fn an_8_5_form_body_reads_as_8_3_27_reads_it() {
    // Saved by 8.5.1.1529, dumped by 8.3.27.2214: the 2.20 dialect writes the
    // down-converted body, not the properties only 8.5 has (ИТК `_open`:
    // `WindowOpeningMode LockOwner`, `ShowCommandBar`, `ButtonImportance` on
    // ~180 forms). Fixture: `_onecdec/make_v85_form_fixture.py`.
    let dir = common::fixture("v85_form");
    let out = common::temp_dir("v85-form");
    let run = common::export(&dir.join("input.cfe"), &out);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stdout)
    );
    let expected = fs::read(dir.join("Form.xml")).unwrap();
    let actual = fs::read(out.join("CommonForms/ТестРасширение_Форма85/Ext/Form.xml")).unwrap();
    assert!(
        expected == actual,
        "--- native\n{}\n--- ours\n{}",
        String::from_utf8_lossy(&expected),
        String::from_utf8_lossy(&actual)
    );
}

#[test]
fn an_8_5_report_record_reads_as_8_3_27_reads_it() {
    // The 8.5 report record carries `AuxiliaryVariantForm`, which 8.3.27 does
    // not write; a report without children still writes `<ChildObjects/>`.
    let dir = common::fixture("v85_form");
    let out = common::temp_dir("v85-report");
    let run = common::export(&dir.join("input.cfe"), &out);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stdout)
    );
    let expected = fs::read(dir.join("Report.xml")).unwrap();
    let actual = fs::read(out.join("Reports/ТестРасширение_Отчет.xml")).unwrap();
    assert!(
        expected == actual,
        "--- native\n{}\n--- ours\n{}",
        String::from_utf8_lossy(&expected),
        String::from_utf8_lossy(&actual)
    );
}

#[test]
fn any_ib_ref_is_any_ref_below_compatibility_8_3_23() {
    // The same form in an extension of compatibility 8.3.22: 8.3.27.2214
    // writes the AnyIBRef type set as cfg:AnyRef below 8.3.23 (probed
    // 8.3.10/21/22 AnyRef, 8.3.23/24/27 AnyIBRef; ИТК 8.3.10 and 1Cv8 8.3.21
    // write AnyRef throughout).
    let dir = common::fixture("form_events_8_3_22");
    let out = common::temp_dir("form-events-8-3-22");
    let run = common::export(&dir.join("input.cfe"), &out);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stdout)
    );
    let expected = fs::read(dir.join("Form.xml")).unwrap();
    let actual =
        fs::read(out.join("CommonForms/ТестРасширение_ФормаСобытий/Ext/Form.xml")).unwrap();
    assert!(
        expected == actual,
        "--- native\n{}\n--- ours\n{}",
        String::from_utf8_lossy(&expected),
        String::from_utf8_lossy(&actual)
    );
}
