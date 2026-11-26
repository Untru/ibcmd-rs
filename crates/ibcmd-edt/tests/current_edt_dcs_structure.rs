//! Current DCS structural LF is lexical; canonical body bytes remain complete.
use morph1c_core::ir::{MetadataObject, ObjectKind, PropertyValue, Token, Uuid};
use morph1c_core::spec::metadata::report_template_ref::F_TEMPLATE_TYPE;
use morph1c_pipeline::{Format, attach_dcs_bodies, write_dcs_bodies};
const SOURCE: &[u8] = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<DataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\">\n\t<query>A\r\nB</query>\n\t<caption><![CDATA[C\nD]]></caption>\n\t<query>\n</query>\n</DataCompositionSchema>";
fn owner(common: bool) -> MetadataObject {
    let mut t = MetadataObject::new(
        ObjectKind::new(if common {
            "CommonTemplate"
        } else {
            "Report.TemplateRef"
        }),
        "Schema",
        Uuid([0x35; 16]),
    );
    t.properties.push((
        F_TEMPLATE_TYPE,
        PropertyValue::Enum(Token::new("DataCompositionSchema")),
    ));
    if common {
        t
    } else {
        let mut p = MetadataObject::new(ObjectKind::new("Report"), "Case", Uuid([0x36; 16]));
        p.children.push(t);
        p
    }
}
fn template_owner(obj: &MetadataObject, common: bool) -> &MetadataObject {
    if common { obj } else { &obj.children[0] }
}
fn template_owner_mut(obj: &mut MetadataObject, common: bool) -> &mut MetadataObject {
    if common { obj } else { &mut obj.children[0] }
}
#[test]
fn current_dcs_structural_framing_is_exact_and_canonical_across_targets() {
    for common in [false, true] {
        // Removed with `root`, also after a failure, unless IBCMD_TEST_KEEP_FIXTURES=1.
        let root = tempfile::Builder::new()
            .prefix("ibcmd-dcs-structure-")
            .disable_cleanup(keep_fixtures())
            .tempdir()
            .unwrap();
        let root = root.path().to_path_buf();
        let anchor = root.join("input/Case.mdo");
        let relative = if common {
            "Template.dcs"
        } else {
            "Templates/Schema/Template.dcs"
        };
        let input = anchor.parent().unwrap().join(relative);
        std::fs::create_dir_all(input.parent().unwrap()).unwrap();
        std::fs::write(&input, SOURCE).unwrap();
        let mut obj = owner(common);
        attach_dcs_bodies(Format::Edt, &anchor, &mut obj).unwrap();
        assert!(template_owner(&obj, common).source_dcs_edt_lf);
        let canonical = template_owner(&obj, common).templates[0]
            .body
            .clone()
            .unwrap();
        assert!(canonical.starts_with(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n"));
        assert!(
            canonical
                .windows(b"<query>A\nB</query>".len())
                .any(|x| x == b"<query>A\nB</query>")
        );
        assert!(
            canonical
                .windows(b"<![CDATA[C\nD]]>".len())
                .any(|x| x == b"<![CDATA[C\nD]]>")
        );
        assert!(
            canonical
                .windows(b"<query>\n</query>".len())
                .any(|x| x == b"<query>\n</query>")
        );
        let target = root.join("same/Case.mdo");
        write_dcs_bodies(Format::Edt, &target, &obj).unwrap();
        assert_eq!(
            std::fs::read(target.parent().unwrap().join(relative)).unwrap(),
            SOURCE
        );
        let native = root.join("native/Case.xml");
        write_dcs_bodies(Format::Designer, &native, &obj).unwrap();
        let native_rel = if common {
            "Case/Ext/Template.xml"
        } else {
            "Case/Templates/Schema/Ext/Template.xml"
        };
        let bytes = std::fs::read(native.parent().unwrap().join(native_rel)).unwrap();
        assert_eq!(&bytes[..3], b"\xef\xbb\xbf");
        assert_eq!(&bytes[3..], canonical);
        let semantic = serde_json::to_vec(&obj).unwrap();
        let restored: MetadataObject = serde_json::from_slice(&semantic).unwrap();
        assert!(!template_owner(&restored, common).source_dcs_edt_lf);
        assert_eq!(
            template_owner(&restored, common).templates[0]
                .body
                .as_ref()
                .unwrap(),
            &canonical
        );
        let t = template_owner_mut(&mut obj, common);
        t.templates[0].body = Some(
            String::from_utf8(canonical)
                .unwrap()
                .replace("A\nB", "Edited\nValue")
                .into_bytes(),
        );
        let edited = root.join("edited/Case.mdo");
        write_dcs_bodies(Format::Edt, &edited, &obj).unwrap();
        let bytes = std::fs::read(edited.parent().unwrap().join(relative)).unwrap();
        assert!(
            bytes
                .windows(b"Edited\r\nValue".len())
                .any(|x| x == b"Edited\r\nValue")
        );
        assert_eq!(std::fs::read(&input).unwrap(), SOURCE);
    }
}
#[test]
fn legacy_dcs_default_and_designer_lf_body_are_not_reframed() {
    let root = tempfile::Builder::new()
        .prefix("ibcmd-dcs-legacy-")
        .disable_cleanup(keep_fixtures())
        .tempdir()
        .unwrap();
    let root = root.path().to_path_buf();
    let mut obj = owner(true);
    let path = root.join("in/Case.xml");
    let body = root.join("in/Case/Ext/Template.xml");
    std::fs::create_dir_all(body.parent().unwrap()).unwrap();
    let mut raw = b"\xef\xbb\xbf".to_vec();
    raw.extend_from_slice(SOURCE);
    std::fs::write(body, &raw).unwrap();
    attach_dcs_bodies(Format::Designer, &path, &mut obj).unwrap();
    assert!(!obj.source_dcs_edt_lf);
    let out = root.join("out/Case.xml");
    write_dcs_bodies(Format::Designer, &out, &obj).unwrap();
    assert_eq!(
        std::fs::read(root.join("out/Case/Ext/Template.xml")).unwrap(),
        raw
    );
}

#[test]
fn mixed_content_and_space_preserve_are_exact_but_mixed_structural_eol_is_rejected() {
    let root = tempfile::Builder::new()
        .prefix("ibcmd-dcs-boundaries-")
        .disable_cleanup(keep_fixtures())
        .tempdir()
        .unwrap();
    let root = root.path().to_path_buf();
    let anchor = root.join("input/Case.mdo");
    let input = root.join("input/Template.dcs");
    std::fs::create_dir_all(input.parent().unwrap()).unwrap();
    let source = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<DataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\">\n<node>left<child/>\n<child/>right</node>\n<node xml:space=\"preserve\"><scope><child/>\n<child/></scope><scope xml:space=\"default\"><child/>\n<child/></scope></node>\n</DataCompositionSchema>";
    std::fs::write(&input, source).unwrap();
    let mut obj = owner(true);
    attach_dcs_bodies(Format::Edt, &anchor, &mut obj).unwrap();
    let body = obj.templates[0].body.as_ref().unwrap();
    assert!(
        body.windows(b"left<child/>\n<child/>right".len())
            .any(|w| w == b"left<child/>\n<child/>right")
    );
    assert!(
        body.windows(b"xml:space=\"preserve\"><scope><child/>\n<child/>".len())
            .any(|w| w == b"xml:space=\"preserve\"><scope><child/>\n<child/>")
    );
    assert!(
        body.windows(b"<scope><child/>\n<child/></scope>".len())
            .any(|w| w == b"<scope><child/>\n<child/></scope>")
    );
    assert!(
        body.windows(b"xml:space=\"default\"><child/>\r\n<child/>".len())
            .any(|w| w == b"xml:space=\"default\"><child/>\r\n<child/>")
    );
    let out = root.join("same/Case.mdo");
    write_dcs_bodies(Format::Edt, &out, &obj).unwrap();
    assert_eq!(
        std::fs::read(root.join("same/Template.dcs")).unwrap(),
        source
    );
    let mixed = String::from_utf8(source.to_vec())
        .unwrap()
        .replacen("</node>\n", "</node>\r\n", 1);
    assert_ne!(mixed.as_bytes(), source);
    std::fs::write(&input, mixed.as_bytes()).unwrap();
    let mut obj = owner(true);
    let err = attach_dcs_bodies(Format::Edt, &anchor, &mut obj).unwrap_err();
    assert!(err.to_string().contains("mixed structural EOL"));
    assert!(obj.templates.is_empty());
}

/// Whether the fixtures are kept for debugging: `IBCMD_TEST_KEEP_FIXTURES=1`.
fn keep_fixtures() -> bool {
    std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").is_some_and(|value| value == "1")
}
