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

struct OriginalClassifierFixture {
    parent: std::path::PathBuf,
    root: std::path::PathBuf,
}
impl OriginalClassifierFixture {
    fn new(active_body: &[u8], proposed_body: &[u8]) -> Self {
        let parent = std::env::temp_dir();
        let root = parent.join(format!(
            "ibcmd-original-classifier-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&root).unwrap();
        for (side, body) in [("active", active_body), ("proposed", proposed_body)] {
            let tree = root.join(side);
            std::fs::create_dir_all(tree.join("CommonForms/F/Ext")).unwrap();
            std::fs::write(tree.join("CommonForms/F.xml"), b"<FormOwner/>").unwrap();
            std::fs::write(tree.join("CommonForms/F/Ext/Form.xml"), body).unwrap();
            std::fs::write(tree.join("unrelated.bin"), b"unchanged census member").unwrap();
        }
        Self { parent, root }
    }
    fn classify(&self) -> crate::mssql_source_change::SourceActivationInput {
        let active = HeldSourceRoot::open_compiler_operation(&self.root.join("active")).unwrap();
        let proposed = HeldSourceRoot::open_source_operation(
            &self.root.join("proposed"),
            "CommonForms/F/Ext/Form.xml",
        )
        .unwrap();
        // This is the production source-apply seam, before its NoOp return or
        // any staging. Neither side is a for_bytes/mock inventory.
        let classified = classify_original_source_change(
            &active,
            &proposed,
            "CommonForms/F/Ext/Form.xml",
            ActivationTarget::Main,
            ActivationMode::Online,
        )
        .unwrap();
        active.require_unchanged().unwrap();
        proposed.require_unchanged().unwrap();
        classified
    }
}
impl Drop for OriginalClassifierFixture {
    fn drop(&mut self) {
        assert_eq!(self.root.parent(), Some(self.parent.as_path()));
        assert!(
            self.root
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("ibcmd-original-classifier-")
        );
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}
#[test]
fn actual_original_classifier_formatting_only_returns_before_staging() {
    let fixture = OriginalClassifierFixture::new(
        b"<Form><Title>Demo</Title></Form>",
        b"<?xml version=\"1.0\"?>\n<Form>\n  <Title>Demo</Title>\n</Form>\n",
    );
    let classified = fixture.classify();
    assert!(classified.is_no_op()); // the actual source-apply no-stage branch
    assert!(classified.changed_paths().is_empty());
    assert_eq!(classified.verified_sources().len(), 1);
    assert_eq!(
        classified.verified_sources()[0].bytes(),
        b"<?xml version=\"1.0\"?>\n<Form>\n  <Title>Demo</Title>\n</Form>\n"
    );
}
#[test]
fn actual_original_classifier_title_change_requires_staging() {
    let fixture = OriginalClassifierFixture::new(
        b"<Form><Title>Before</Title></Form>",
        b"<Form><Title>After</Title></Form>",
    );
    let classified = fixture.classify();
    assert!(!classified.is_no_op());
    assert_eq!(classified.changed_paths(), &["CommonForms/F/Ext/Form.xml"]);
    assert_eq!(
        classified.verified_sources()[0].bytes(),
        b"<Form><Title>After</Title></Form>"
    );
}
#[test]
fn actual_original_classifier_same_digest_returns_before_staging() {
    let body = b"<Form><Title>Demo</Title></Form>";
    let fixture = OriginalClassifierFixture::new(body, body);
    let classified = fixture.classify();
    assert!(classified.is_no_op());
    assert!(classified.changed_paths().is_empty());
    assert_eq!(classified.verified_sources()[0].bytes(), body);
}

#[test]
fn actual_original_classifier_compares_outside_xml_without_authorizing_edits() {
    let body = b"<Form><Title>same</Title></Form>";
    let fixture = OriginalClassifierFixture::new(body, body);
    std::fs::write(
        fixture.root.join("active/Other.xml"),
        b"<Root><Value>old</Value></Root>",
    )
    .unwrap();
    std::fs::write(
        fixture.root.join("proposed/Other.xml"),
        b"<Root><Value>new</Value></Root>",
    )
    .unwrap();
    let active = HeldSourceRoot::open_compiler_operation(&fixture.root.join("active")).unwrap();
    let proposed = HeldSourceRoot::open_source_operation(
        &fixture.root.join("proposed"),
        "CommonForms/F/Ext/Form.xml",
    )
    .unwrap();
    assert!(matches!(classify_original_source_change(&active, &proposed,
        "CommonForms/F/Ext/Form.xml", ActivationTarget::Main, ActivationMode::Online),
        Err(crate::mssql_source_change::SourceChangeError::ChangesOutsideClosure(paths)) if paths == vec!["Other.xml"]));
    active.require_unchanged().unwrap();
    proposed.require_unchanged().unwrap();
}
#[test]
fn actual_original_classifier_outside_xml_formatting_preserves_no_op() {
    let body = b"<Form><Title>same</Title></Form>";
    let fixture = OriginalClassifierFixture::new(body, body);
    std::fs::write(
        fixture.root.join("active/Other.xml"),
        b"<Root><Value>same</Value></Root>",
    )
    .unwrap();
    std::fs::write(
        fixture.root.join("proposed/Other.xml"),
        b"<Root>\n<Value>same</Value>\n</Root>",
    )
    .unwrap();
    let classified = fixture.classify();
    assert!(classified.is_no_op());
    assert!(classified.changed_paths().is_empty());
}
