//! A configuration naming its default style (root tuple field 9). The
//! fixture is platform-built (`_onecdec/make_default_style_fixture.py`).
//! Before, the evidenced default block required field 9 to be the nil uuid:
//! Configuration.xml was not written at all, and with that lifted it printed
//! `<DefaultStyle/>`.

use crate::common;

use std::fs;

#[test]
fn the_default_style_is_printed() {
    let dir = common::fixture("default_style");
    let out = common::temp_dir("default-style");
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
