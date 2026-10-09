//! Existing compiler-tree projection, not a new compiler or acceptance policy.
use super::*;
use crate::mssql_source_change::SourceFileDigest;
fn census(rows: &[(&str, &[u8])]) -> SourceInventory {
    SourceInventory::from_files(
        rows.iter()
            .map(|(path, bytes)| SourceFileDigest::for_bytes(*path, bytes).unwrap())
            .collect(),
    )
    .unwrap()
}
#[test]
fn actual_module_only_projection_refuses_any_other_change_before_compilation() {
    let module = "CommonForms/F/Ext/Form/Module.bsl";
    let form = "CommonForms/F/Ext/Form.xml";
    let before = census(&[
        (module, b"module"),
        (form, b"form"),
        ("CommonForms/F.xml", b"owner"),
    ]);
    let after = census(&[(module, b"module"), ("CommonForms/F.xml", b"owner")]);
    require_compile_projection(&before, &after, module).unwrap();
    for bad in [
        census(&[(module, b"mutant"), ("CommonForms/F.xml", b"owner")]),
        census(&[(module, b"module"), ("CommonForms/F.xml", b"other")]),
        census(&[(module, b"module")]),
        census(&[
            (module, b"module"),
            ("CommonForms/F.xml", b"owner"),
            ("extra.xml", b"new"),
        ]),
        before.clone(),
    ] {
        assert!(require_compile_projection(&before, &bad, module).is_err());
    }
    require_compile_projection(&after, &after, module).unwrap();
}
#[test]
fn digest_projection_keeps_values_but_carries_no_original_capability() {
    let original = census(&[("x.bin", b"complete")]);
    let projection = original.digest_projection();
    let member = projection.file("x.bin").unwrap().unwrap();
    assert_eq!(member.size_bytes(), 8);
    assert_eq!(
        member.sha256(),
        original.file("x.bin").unwrap().unwrap().sha256()
    );
    require_compile_projection(&original, &projection, "CommonModules/M/Ext/Module.bsl").unwrap();
}
