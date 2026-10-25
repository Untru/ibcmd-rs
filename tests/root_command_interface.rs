//! The configuration's own command interfaces. The fixture is the clean-room
//! configuration loaded with empty Ext/CommandInterface.xml and
//! Ext/MainSectionCommandInterface.xml, built and dumped by 8.3.27.2214
//! (`_onecdec/make_empty_root_interface_fixture.py`).

use crate::common;

use std::fs;

#[test]
fn an_empty_root_command_interface_is_named_but_not_written() {
    // The platform stores both as `{7,0,0,0,0,0,0}`, lists them in
    // ConfigDumpInfo.xml and writes neither file (as WMS5 showed upstream).
    let dir = common::fixture("empty_root_interface");
    let out = common::temp_dir("empty-root-interface");
    let run = common::export(&dir.join("input.cf"), &out);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stdout)
    );
    let mut ext = fs::read_dir(out.join("Ext"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    ext.sort();
    let expected = fs::read_to_string(dir.join("ext.txt")).unwrap();
    assert_eq!(ext, expected.lines().collect::<Vec<_>>());
    let expected = fs::read(dir.join("ConfigDumpInfo.xml")).unwrap();
    let actual = fs::read(out.join("ConfigDumpInfo.xml")).unwrap_or_default();
    assert!(
        expected == actual,
        "--- native\n{}\n--- ours\n{}",
        String::from_utf8_lossy(&expected),
        String::from_utf8_lossy(&actual)
    );
}
