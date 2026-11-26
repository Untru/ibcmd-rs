//! A form's choice list of dates. The fixture is the clean-room configuration
//! with one common form whose date input field lists three dates, built and
//! dumped by 8.3.27.2214 (`_onecdec/make_choice_list_dates_fixture.py`).

use crate::common;

use std::fs;

#[test]
fn a_choice_list_of_dates_is_written() {
    // The items store `{"D",YYYYMMDDhhmmss}` under the nil identifier pair;
    // refusing them made the list opaque and withheld the whole Form.xml
    // (1Cv8_обф.cf: Reports/узПланированиеПроекта ФормаЗадачиУправляемая).
    let dir = common::fixture("choice_list_dates");
    let out = common::temp_dir("choice-list-dates");
    let run = common::export(&dir.join("input.cf"), &out);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stdout)
    );
    let expected = fs::read(dir.join("Form.xml")).unwrap();
    let actual = fs::read(out.join("CommonForms/ФормаДаты/Ext/Form.xml")).unwrap_or_default();
    assert!(
        expected == actual,
        "--- native\n{}\n--- ours\n{}",
        String::from_utf8_lossy(&expected),
        String::from_utf8_lossy(&actual)
    );
}
