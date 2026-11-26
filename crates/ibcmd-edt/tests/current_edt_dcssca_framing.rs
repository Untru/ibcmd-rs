//! Witnessed current DCSSCA framing. Legacy API/default output remain CRLF.
use formats_xml::form::{
    read_conditional_appearance_dcssca, read_conditional_appearance_dcssca_with_framing,
    write_conditional_appearance_dcssca, write_conditional_appearance_dcssca_with_framing,
};
use morph1c_core::ir::{DcsCorValue, DcsItem, DcsSettingsParameterValue};

fn items() -> Vec<DcsItem> {
    vec![DcsItem::ConditionalAppearance {
        used: None,
        selection: None,
        filter: vec![],
        appearance: vec![DcsSettingsParameterValue {
            used: None,
            parameter: "Caption".into(),
            value: Some(DcsCorValue::Str("Before\nCurrent".into())),
            user_setting_id: None,
        }],
        source_empty_appearance: false,
        presentation: None,
        view_mode: None,
        user_setting_id: None,
    }]
}

#[test]
fn explicit_framing_api_preserves_lf_current_values_and_legacy_bytes() {
    for without in [false, true] {
        let source = items();
        let legacy = write_conditional_appearance_dcssca(&source, without);
        let (old_items, old_without) = read_conditional_appearance_dcssca(&legacy).unwrap();
        assert_eq!(old_items, source);
        assert_eq!(old_without, without);
        assert_eq!(
            write_conditional_appearance_dcssca(&old_items, old_without),
            legacy
        );
        let current = String::from_utf8(legacy)
            .unwrap()
            .replace("\r\n", "\n")
            .into_bytes();
        assert!(read_conditional_appearance_dcssca(&current).is_err());
        let mut document = read_conditional_appearance_dcssca_with_framing(&current).unwrap();
        assert!(document.source_edt_lf);
        assert_eq!(document.envelope_without_lf_pal, without);
        assert_eq!(document.items, source);
        assert_eq!(
            write_conditional_appearance_dcssca_with_framing(
                &document.items,
                without,
                document.source_edt_lf
            ),
            current
        );
        let DcsItem::ConditionalAppearance { appearance, .. } = &mut document.items[0] else {
            unreachable!()
        };
        appearance[0].value = Some(DcsCorValue::Str("Edited\r\nCurrent".into()));
        let edited =
            write_conditional_appearance_dcssca_with_framing(&document.items, without, true);
        assert_ne!(edited, current);
        let reread = read_conditional_appearance_dcssca_with_framing(&edited).unwrap();
        assert_eq!(reread.items, document.items);
        assert!(reread.source_edt_lf);
    }
}

#[test]
fn framing_api_still_rejects_bom_unknown_cells_and_half_namespace_flavors() {
    let current = String::from_utf8(write_conditional_appearance_dcssca_with_framing(
        &items(),
        false,
        true,
    ))
    .unwrap();
    for bad in [
        format!("\u{feff}{current}"),
        current.replace(
            "</ConditionalAppearance>",
            "<Unknown/></ConditionalAppearance>",
        ),
        current.replace("UTF-8", "UTF-16"),
        current.replacen(
            "<dcscor:parameter>",
            "<dcscor:parameter unknown=\"lost\">",
            1,
        ),
        current.replace(
            " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\"",
            "",
        ),
    ] {
        assert_ne!(bad, current, "negative fixture mutation must take effect");
        assert!(read_conditional_appearance_dcssca_with_framing(bad.as_bytes()).is_err());
    }
}

#[test]
fn pipeline_carries_sidecar_framing_independently_and_empty_items_emit_no_sidecar() {
    use morph1c_core::ir::{
        FormBody, MetadataObject, NamedFormBody, ObjectKind, PropertyValue, Token, Uuid,
    };
    use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
    use morph1c_pipeline::{Format, attach_form_body, write_form_bodies};
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        // Removed with `root`, also after a failure, unless IBCMD_TEST_KEEP_FIXTURES=1.
        let root = tempfile::Builder::new()
            .prefix("ibcmd-dcssca-framing-")
            .disable_cleanup(keep_fixtures())
            .tempdir()
            .unwrap();
        let root = root.path().to_path_buf();
        let anchor = root.join("Case/Case.mdo");
        let mut object =
            MetadataObject::new(ObjectKind::new("CommonForm"), "Case", Uuid([0x35; 16]));
        let form_type = morph1c_core::spec::registry::spec_for("CommonForm")
            .unwrap()
            .fields()
            .iter()
            .find(|f| f.name == "formType")
            .unwrap()
            .id;
        object
            .properties
            .push((form_type, PropertyValue::Enum(Token::new("Managed"))));
        let body = FormBody {
            conditional_appearance: items(),
            source_dcssca_lf: true,
            ..FormBody::new()
        };
        let semantic = serde_json::to_vec(&body).unwrap();
        let restored: FormBody = serde_json::from_slice(&semantic).unwrap();
        assert!(!restored.source_dcssca_lf);
        assert_eq!(restored.conditional_appearance, body.conditional_appearance);
        object.form_bodies.push(NamedFormBody {
            name: "Case".into(),
            body,
            ordinary_body: None,
            module: None,
            help: vec![],
            help_resources: vec![],
        });
        with_roundtrip_target(version, || write_form_bodies(Format::Edt, &anchor, &object))
            .unwrap();
        let sidecar = anchor
            .parent()
            .unwrap()
            .join("ConditionalAppearance.dcssca");
        let raw = std::fs::read(&sidecar).unwrap();
        assert_eq!(
            raw,
            write_conditional_appearance_dcssca_with_framing(&items(), false, true)
        );
        let form = std::fs::read(anchor.parent().unwrap().join("Form.form")).unwrap();
        assert!(form.starts_with(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n"));
        let mut returned = object.clone();
        returned.form_bodies.clear();
        with_source_version(Some(version), || {
            attach_form_body(Format::Edt, "CommonForm", &anchor, &mut returned)
        })
        .unwrap();
        assert!(returned.form_bodies[0].body.source_dcssca_lf);
        assert_eq!(returned.form_bodies[0].body.conditional_appearance, items());
        let next = root.join("next/Case/Case.mdo");
        with_roundtrip_target(version, || write_form_bodies(Format::Edt, &next, &returned))
            .unwrap();
        assert_eq!(
            std::fs::read(next.parent().unwrap().join("ConditionalAppearance.dcssca")).unwrap(),
            raw
        );
        returned.form_bodies[0].body.conditional_appearance.clear();
        let empty = root.join("empty/Case/Case.mdo");
        with_roundtrip_target(version, || {
            write_form_bodies(Format::Edt, &empty, &returned)
        })
        .unwrap();
        assert!(
            !empty
                .parent()
                .unwrap()
                .join("ConditionalAppearance.dcssca")
                .exists()
        );
    }
}

/// Whether the fixtures are kept for debugging: `IBCMD_TEST_KEEP_FIXTURES=1`.
fn keep_fixtures() -> bool {
    std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").is_some_and(|value| value == "1")
}
