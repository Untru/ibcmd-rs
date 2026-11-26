//! Ext/HomePageWorkArea.xml read from the configuration's home page blob. The
//! fixtures are the clean-room configuration with two common forms on its home
//! page, built and dumped by 8.3.27.2214 (`one_column_v85` saved by
//! 8.5.1.1529), `_onecdec/make_home_page_fixtures.py`.

use crate::common;

use std::fs;

fn home_page_case(case: &str) {
    let dir = common::fixture("home_page").join(case);
    let out = common::temp_dir(&format!("home-page-{case}"));
    let run = common::export(&dir.join("input.cf"), &out);
    assert!(
        run.status.success(),
        "{case}: {}",
        String::from_utf8_lossy(&run.stdout)
    );
    let expected = fs::read(dir.join("HomePageWorkArea.xml")).unwrap();
    let actual = fs::read(out.join("Ext/HomePageWorkArea.xml")).unwrap_or_default();
    assert!(
        expected == actual,
        "{case}\n--- native\n{}\n--- ours\n{}",
        String::from_utf8_lossy(&expected),
        String::from_utf8_lossy(&actual)
    );
}

#[test]
fn every_work_area_template_is_named() {
    // The blob stores the template as 0 OneColumn, 1 TwoColumnsEqualWidth,
    // 2 TwoColumnsVariableWidth; a one-column page prints its items under
    // `<Column>`.
    home_page_case("one_column");
    home_page_case("two_equal");
    home_page_case("two_variable");
    home_page_case("one_column_v85");
}

#[test]
fn mobile_command_interface_placement_follows_the_columns() {
    // The field after the columns: 0 Top, 1 Bottom, 2 None (not printed). An
    // empty column is `<RightColumn/>`.
    home_page_case("one_column_top");
    home_page_case("two_equal_bottom");
}
