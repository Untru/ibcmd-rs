//! A configuration role's rights. The fixtures are the clean-room configuration
//! with one role, built and dumped by 8.3.27.2214
//! (`_onecdec/make_role_rights_fixtures.py`).

use crate::common;

use std::fs;

fn rights_case(case: &str) {
    let dir = common::fixture("role_rights").join(case);
    let out = common::temp_dir(&format!("role-rights-{case}"));
    let run = common::export(&dir.join("input.cf"), &out);
    assert!(
        run.status.success(),
        "{case}: {}",
        String::from_utf8_lossy(&run.stdout)
    );
    let expected = fs::read(dir.join("Rights.xml")).unwrap();
    let actual = fs::read(out.join("Roles/РольПроверка/Ext/Rights.xml")).unwrap_or_default();
    assert!(
        expected == actual,
        "{case}\n--- native\n{}\n--- ours\n{}",
        String::from_utf8_lossy(&expected),
        String::from_utf8_lossy(&actual)
    );
}

#[test]
fn exclusive_mode_termination_diverging_from_the_new_objects_flag_is_printed() {
    // Both directions: the flag false and the right true (ИТК), the flag true
    // and the right false -- the second was printed on inference only.
    rights_case("new_false_emt_true");
    rights_case("new_true_emt_false");
}
