//! `MainFilterOnPeriod` of a recorder-subordinate periodic information
//! register: stored set, dumped `false` by 8.3.27.2214, in a configuration
//! and in an extension (`_onecdec/make_main_filter_fixture.py`).

use crate::common;

use std::fs;

fn assert_case(case: &str, input: &str) {
    let dir = common::fixture("main_filter").join(case);
    let out = common::temp_dir(&format!("main-filter-{case}"));
    let run = common::export(&dir.join(input), &out);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stdout)
    );
    let mut compared = 0;
    for (rel, expected) in common::files(&dir) {
        if rel.starts_with("input.") {
            continue;
        }
        let actual = fs::read(out.join(&rel)).unwrap_or_default();
        assert!(
            expected == actual,
            "{case}: {rel} differs\n--- native\n{}\n--- ours\n{}",
            String::from_utf8_lossy(&expected),
            String::from_utf8_lossy(&actual)
        );
        compared += 1;
    }
    assert!(compared > 0);
}

#[test]
fn a_recorder_subordinate_register_has_no_main_filter_on_period_in_a_configuration() {
    assert_case("cf", "input.cf");
}

#[test]
fn a_recorder_subordinate_register_has_no_main_filter_on_period_in_an_extension() {
    assert_case("cfe", "input.cfe");
}
