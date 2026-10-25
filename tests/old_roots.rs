//! Configurations saved with an older root tuple (`{66,…}`, `{59,…}`). The
//! fixtures are platform-built configurations with the tuple rewritten into
//! the older shape (`_onecdec/make_old_root_fixtures.py`, which names the
//! evidence: an older tuple is a prefix of the 68 one, the rest reads as the
//! all-default reference). Before, such a configuration exported a 15-line
//! Configuration.xml stub and its forms declared `dcssch` whatever its
//! compatibility mode (MONITOR-TRIAL-2.cf, Src/1Cv8.cf).

use crate::common;

use std::fs;

fn same_file(case: &str, exported: &str, expected: &std::path::Path) {
    let out = common::temp_dir(&format!("old-root-{case}"));
    let run = common::export(
        &common::fixture("old_roots").join(case).join("input.cf"),
        &out,
    );
    assert!(
        run.status.success(),
        "{case}: {}",
        String::from_utf8_lossy(&run.stdout)
    );
    let expected = fs::read(expected).unwrap();
    let actual = fs::read(out.join(exported)).unwrap_or_default();
    assert!(
        expected == actual,
        "{case}: {exported} differs\n--- native\n{}\n--- ours\n{}",
        String::from_utf8_lossy(&expected),
        String::from_utf8_lossy(&actual)
    );
}

#[test]
fn a_66_tuple_reads_as_the_68_one() {
    same_file(
        "v66",
        "Configuration.xml",
        &common::fixture("config_compat/c19_e12/Configuration.xml"),
    );
}

#[test]
fn a_59_tuple_reads_as_the_68_one() {
    same_file(
        "v59",
        "Configuration.xml",
        &common::fixture("config_compat/c19_e12/Configuration.xml"),
    );
}

#[test]
fn a_52_tuple_with_the_older_header_and_identities_reads_as_the_68_one() {
    // A root saved in 8.3.9 mode: 46 members, header `{1,{0,0,<id>},…}`,
    // section identities `{0,0,<id>}`. The platform loads the fixture and
    // dumps this very Configuration.xml.
    same_file(
        "v52",
        "Configuration.xml",
        &common::fixture("config_compat/c19_e12/Configuration.xml"),
    );
}

#[test]
fn a_59_tuple_gives_its_forms_the_compatibility_mode() {
    same_file(
        "v59_form",
        "CommonForms/Форма/Ext/Form.xml",
        &common::fixture("compat_forms/cf_8_3_18/Form.xml"),
    );
}

#[test]
fn an_extension_root_in_the_66_shape_is_written() {
    // ИР 7.77 (`{66,…}`) lost its whole extension Configuration.xml: the
    // writer looked for `{68,` or `{76,` only.
    let case = "ext_v66";
    let out = common::temp_dir(&format!("old-root-{case}"));
    let run = common::export(
        &common::fixture("old_roots").join(case).join("input.cfe"),
        &out,
    );
    assert!(
        run.status.success(),
        "{case}: {}",
        String::from_utf8_lossy(&run.stdout)
    );
    let expected = fs::read(common::fixture("extension_roots/values/Configuration.xml")).unwrap();
    let actual = fs::read(out.join("Configuration.xml")).unwrap_or_default();
    assert!(
        expected == actual,
        "--- native\n{}\n--- ours\n{}",
        String::from_utf8_lossy(&expected),
        String::from_utf8_lossy(&actual)
    );
}
