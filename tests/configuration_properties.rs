//! Configuration.xml properties read from the root's `{68,…}` tuple. The
//! fixture is a configuration built and dumped by 8.3.27.2214
//! (`_onecdec/make_search_form_fixture.py`).

use crate::common;

use std::fs;

#[test]
fn default_search_form_is_written_from_field_37() {
    // A configuration naming a DefaultSearchForm used to lose its whole
    // Configuration.xml: field 37 was only checked against the all-default
    // reference (1Cv8: CommonForm.ФормаПоиска).
    let dir = common::fixture("search_form");
    let out = common::temp_dir("search-form");
    let run = common::export(&dir.join("input.cf"), &out);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stdout)
    );
    let expected = fs::read(dir.join("Configuration.xml")).unwrap();
    let actual = fs::read(out.join("Configuration.xml")).unwrap_or_default();
    assert!(
        expected == actual,
        "--- native\n{}\n--- ours\n{}",
        String::from_utf8_lossy(&expected),
        String::from_utf8_lossy(&actual)
    );
}

fn config_case(case: &str) {
    let dir = common::fixture("config_compat").join(case);
    let out = common::temp_dir(&format!("config-compat-{case}"));
    let run = common::export(&dir.join("input.cf"), &out);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stdout)
    );
    let expected = fs::read(dir.join("Configuration.xml")).unwrap();
    let actual = fs::read(out.join("Configuration.xml")).unwrap_or_default();
    assert!(
        expected == actual,
        "{case}\n--- native\n{}\n--- ours\n{}",
        String::from_utf8_lossy(&expected),
        String::from_utf8_lossy(&actual)
    );
}

#[test]
fn a_configuration_prints_the_platform_edition_as_its_extension_compatibility() {
    // The tuple stores CompatibilityMode in field 26 and the requested
    // extension compatibility in field 43; 8.3.27.2214 prints Version8_3_27
    // whatever field 43 holds (10/27, 19/12, 21/24 here; 1Cv8 21/23).
    config_case("c10_e27");
    config_case("c19_e12");
    config_case("c21_e24");
}
