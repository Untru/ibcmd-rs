//! Modules delivered without source text. 8.3.27.2214 dumps each as
//! `<Module>.bin`, the inflated storage entry byte for byte.

use crate::common;

use std::fs;

#[test]
fn a_closed_module_of_storage_revision_0_is_written_as_bin() {
    // MONITOR-TRIAL-2.cf keeps 204 closed modules in revision-0 inner
    // containers; only revisions 1 and 2 (ИТК) were recognised, so none was
    // written. Fixture: `_onecdec/make_closed_module_fixture.py`.
    let dir = common::fixture("closed_module");
    let out = common::temp_dir("closed-module");
    let run = common::export(&dir.join("input.cfe"), &out);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stdout)
    );
    let expected = fs::read(dir.join("Module.bin")).unwrap();
    let actual = fs::read(out.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bin"))
        .unwrap_or_default();
    assert!(
        expected == actual,
        "Module.bin differs ({} bytes)",
        actual.len()
    );
}
