//! Authored root CI presence is independent of native's AutoCommandBar default.
//! Fixtures are owned synthetic XML; the original 3a9 failure remains in the lab.
use formats_xml::form::{
    FormDialect, apply_form_presence_resource, prepare_form_presence, read_form,
    validate_form_presence_resource, write_form,
};
use morph1c_core::{
    ir::{
        FormBody, FormCiItem, MetadataObject, NamedFormBody, ObjectKind, PropertyValue, Token,
        Uuid, form::DataPathSpec,
    },
    spec::forms::controls::form_field as ff,
    version::{FormatVersion, with_roundtrip_target, with_source_version},
};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};

fn read(bytes: &[u8], dialect: FormDialect, profile: FormatVersion) -> FormBody {
    with_source_version(Some(profile), || read_form(dialect, bytes)).unwrap()
}
fn fixture(profile: FormatVersion, bar: bool, ci: bool) -> FormBody {
    let bar = if bar {
        "<autoCommandBar><name>FormCommandBar</name><id>-1</id></autoCommandBar>"
    } else {
        ""
    };
    let ci = if ci {
        "<commandInterface><navigationPanel/><commandBar/></commandInterface>"
    } else {
        ""
    };
    let xml = format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\">{}{}",
            "</form:Form>\r\n"
        ),
        bar, ci
    );
    read(xml.as_bytes(), FormDialect::Edt, profile)
}
fn semantic(body: &FormBody) -> Vec<u8> {
    serde_json::to_vec(body).unwrap()
}

fn native_roundtrip(body: &FormBody, profile: FormatVersion) -> FormBody {
    let package =
        prepare_form_presence(body, Uuid([7; 16]), FormDialect::Designer, profile, None).unwrap();
    let mut reread = read(&package.bytes, FormDialect::Designer, profile);
    if let Some(resource) = package.resource {
        apply_form_presence_resource(&mut reread, Uuid([7; 16]), profile, &resource, None).unwrap();
    }
    assert_eq!(semantic(body), semantic(&reread));
    reread
}

#[test]
fn authored_absent_and_explicit_ci_survive_serde_and_both_directions() {
    for minor in [20, 21] {
        let profile = FormatVersion::new(2, minor);
        for (bar, ci) in [(true, false), (true, true), (false, false), (false, true)] {
            let original = fixture(profile, bar, ci);
            let before = semantic(&original);
            let serialized: FormBody = serde_json::from_slice(&before).unwrap();
            assert_eq!(before, semantic(&serialized));
            let native = native_roundtrip(&serialized, profile);
            let edt =
                with_roundtrip_target(profile, || write_form(FormDialect::Edt, &native)).unwrap();
            let edt = read(&edt, FormDialect::Edt, profile);
            assert_eq!(before, semantic(&edt));
            native_roundtrip(&edt, profile);
            assert_eq!(before, semantic(&original));
        }
    }
}

#[test]
fn bare_native_writer_reports_actual_presence_loss_and_plain_combinations_stay_supported() {
    for minor in [20, 21] {
        let profile = FormatVersion::new(2, minor);
        for (bar, ci) in [(true, false), (false, true)] {
            let current = fixture(profile, bar, ci);
            let error =
                with_roundtrip_target(profile, || write_form(FormDialect::Designer, &current))
                    .unwrap_err();
            assert!(error.to_string().contains("prepare_form_presence"));
        }
        for (bar, ci) in [(true, true), (false, false)] {
            let current = fixture(profile, bar, ci);
            let package = prepare_form_presence(
                &current,
                Uuid([7; 16]),
                FormDialect::Designer,
                profile,
                None,
            )
            .unwrap();
            assert!(package.resource.is_none());
            assert_eq!(
                package.bytes,
                with_roundtrip_target(profile, || write_form(FormDialect::Designer, &current))
                    .unwrap()
            );
        }
    }
}

#[test]
fn current_bar_and_ci_edits_regenerate_and_stale_resource_rejection_is_atomic() {
    for minor in [20, 21] {
        let profile = FormatVersion::new(2, minor);
        let original = fixture(profile, true, false);
        let old = prepare_form_presence(
            &original,
            Uuid([7; 16]),
            FormDialect::Designer,
            profile,
            None,
        )
        .unwrap();
        let mut edited: FormBody = serde_json::from_slice(&semantic(&original)).unwrap();
        let bar = edited.auto_command_bar.as_mut().unwrap();
        bar.visible = false;
        bar.auto_fill = true;
        bar.horizontal_align = Some("Left".into());
        let new =
            prepare_form_presence(&edited, Uuid([7; 16]), FormDialect::Designer, profile, None)
                .unwrap();
        assert_ne!(old.resource, new.resource);
        let mut stale = read(&new.bytes, FormDialect::Designer, profile);
        let before = semantic(&stale);
        assert!(
            apply_form_presence_resource(
                &mut stale,
                Uuid([7; 16]),
                profile,
                old.resource.as_ref().unwrap(),
                None
            )
            .is_err()
        );
        assert_eq!(before, semantic(&stale));
        native_roundtrip(&edited, profile);
        edited.command_interface = true;
        assert!(
            prepare_form_presence(&edited, Uuid([7; 16]), FormDialect::Designer, profile, None)
                .unwrap()
                .resource
                .is_none()
        );
        native_roundtrip(&edited, profile);
        edited.auto_command_bar = None;
        native_roundtrip(&edited, profile);
        edited.form_ci_command_bar.push(FormCiItem {
            command: "CommonCommand.Example".into(),
            ty: "Added".into(),
            command_parameter: None,
            group: None,
            index: None,
            user_visible: Some(true),
            user_visible_roles: vec![],
        });
        native_roundtrip(&edited, profile);
        let edt = with_roundtrip_target(profile, || write_form(FormDialect::Edt, &edited)).unwrap();
        assert_eq!(
            semantic(&edited),
            semantic(&read(&edt, FormDialect::Edt, profile))
        );
        edited.form_ci_navigation_panel = edited.form_ci_command_bar.clone();
        edited.form_ci_command_bar.clear();
        native_roundtrip(&edited, profile);
        // This canonical combination cannot encode its populated panels in EDT.
        // Refuse contradictory authored IR instead of silently dropping values.
        edited.command_interface = false;
        assert!(
            prepare_form_presence(&edited, Uuid([7; 16]), FormDialect::Edt, profile, None).is_err()
        );
        assert!(with_roundtrip_target(profile, || write_form(FormDialect::Edt, &edited)).is_err());
    }
}

#[test]
fn closed_resource_rejects_foreign_stale_unknown_and_nonconsuming_values_without_assignment() {
    for minor in [20, 21] {
        let profile = FormatVersion::new(2, minor);
        let original = fixture(profile, true, false);
        let package = prepare_form_presence(
            &original,
            Uuid([7; 16]),
            FormDialect::Designer,
            profile,
            None,
        )
        .unwrap();
        let resource = package.resource.unwrap();
        let projected = read(&package.bytes, FormDialect::Designer, profile);
        for change in 0..6 {
            let mut model: serde_json::Value = serde_json::from_slice(&resource).unwrap();
            match change {
                0 => model["version"] = 3.into(),
                1 => model["schema"] = "foreign".into(),
                2 => model["root"]["opaque"] = "unknown".into(),
                3 => model["root"]["command_interface"] = true.into(),
                4 => model["projected_sha256"] = "a".repeat(64).into(),
                _ => model["root"] = serde_json::Value::Null,
            }
            let bad = serde_json::to_vec(&model).unwrap();
            let mut current = projected.clone();
            let before = semantic(&current);
            assert!(
                apply_form_presence_resource(&mut current, Uuid([7; 16]), profile, &bad, None)
                    .is_err()
            );
            assert_eq!(before, semantic(&current));
        }
        assert!(
            validate_form_presence_resource(&projected, Uuid([8; 16]), profile, &resource).is_err()
        );
        assert!(
            validate_form_presence_resource(
                &projected,
                Uuid([7; 16]),
                FormatVersion::new(2, if minor == 20 { 21 } else { 20 }),
                &resource
            )
            .is_err()
        );
        let mut deleted = projected.clone();
        deleted.auto_command_bar = None;
        assert!(
            validate_form_presence_resource(&deleted, Uuid([7; 16]), profile, &resource).is_err()
        );
        let mut edited = projected.clone();
        let ticket =
            validate_form_presence_resource(&edited, Uuid([7; 16]), profile, &resource).unwrap();
        edited.auto_command_bar.as_mut().unwrap().visible = false;
        let before = semantic(&edited);
        assert!(ticket.restore(&mut edited, profile, None).is_err());
        assert_eq!(before, semantic(&edited));
        let duplicate = String::from_utf8(resource)
            .unwrap()
            .replace("\"version\":2", "\"version\":2,\"version\":2");
        assert!(
            validate_form_presence_resource(
                &projected,
                Uuid([7; 16]),
                profile,
                duplicate.as_bytes()
            )
            .is_err()
        );
    }
}

#[test]
fn whole_configuration_uses_same_current_plan_and_restores_root_with_rich_paths() {
    for minor in [20, 21] {
        let profile = FormatVersion::new(2, minor);
        let source = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        ));
        let mut cfg = read_config(Format::Designer, source, &ConvertOptions::default())
            .unwrap()
            .0;
        let mut original = fixture(profile, true, false);
        let field = concat!(
            "<items xsi:type=\"form:FormField\"><name>Field</name><id>1</id>",
            "<dataPath xsi:type=\"form:DataPath\"><segments>List.Link</segments></dataPath>",
            "<type>LabelField</type><extInfo xsi:type=\"form:LabelFieldExtInfo\"/></items>"
        );
        let xml =
            with_roundtrip_target(profile, || write_form(FormDialect::Edt, &original)).unwrap();
        let xml = String::from_utf8(xml)
            .unwrap()
            .replace("</form:Form>", &format!("{field}</form:Form>"));
        original = read(xml.as_bytes(), FormDialect::Edt, profile);
        original.items[0]
            .properties
            .iter_mut()
            .find(|(id, _)| *id == ff::F_DATA_PATH)
            .unwrap()
            .1 = PropertyValue::DataPath(DataPathSpec {
            segments: vec!["List".into(), "A~literal".into()],
            extra_paths: vec!["Other.B".into(), "Other.B".into()],
        });
        let mut obj = MetadataObject::new(ObjectKind::new("CommonForm"), "Presence", Uuid([7; 16]));
        let id = morph1c_core::spec::registry::spec_for("CommonForm")
            .unwrap()
            .fields()
            .iter()
            .find(|f| f.name == "formType")
            .unwrap()
            .id;
        obj.properties
            .push((id, PropertyValue::Enum(Token::new("Managed"))));
        obj.form_bodies.push(NamedFormBody {
            name: "Presence".into(),
            body: original.clone(),
            ordinary_body: None,
            module: None,
            help: vec![],
            help_resources: vec![],
        });
        cfg.objects.push(obj);
        let before = semantic(&original);
        let plan = formats_xml::form::prepare_native_form_write_plan(&cfg, profile).unwrap();
        let native = tempfile::tempdir().unwrap();
        with_roundtrip_target(profile, || {
            write_config(Format::Designer, &cfg, native.path())
        })
        .unwrap();
        assert_eq!(
            before,
            semantic(&cfg.objects.last().unwrap().form_bodies[0].body)
        );
        let artifact =
            std::fs::read(native.path().join("CommonForms/Presence/Ext/Form.xml")).unwrap();
        assert_eq!(artifact, plan.form(Uuid([7; 16])).unwrap().bytes);
        let manifest = std::fs::read(native.path().join("ConfigDumpInfo.xml")).unwrap();
        assert_eq!(manifest, plan.manifest.unwrap());
        assert!(String::from_utf8_lossy(&manifest).contains("ibcmd-configuration-semantics:2:"));
        let restored = with_source_version(Some(profile), || {
            read_config(Format::Designer, native.path(), &ConvertOptions::default())
        })
        .unwrap()
        .0;
        let body = &restored
            .objects
            .iter()
            .find(|o| o.name == "Presence")
            .unwrap()
            .form_bodies[0]
            .body;
        assert_eq!(before, semantic(body));
        let edt = tempfile::tempdir().unwrap();
        with_roundtrip_target(profile, || write_config(Format::Edt, &restored, edt.path()))
            .unwrap();
        let again = with_source_version(Some(profile), || {
            read_config(Format::Edt, edt.path(), &ConvertOptions::default())
        })
        .unwrap()
        .0;
        let body = &again
            .objects
            .iter()
            .find(|o| o.name == "Presence")
            .unwrap()
            .form_bodies[0]
            .body;
        assert_eq!(before, semantic(body));
        let native_again = tempfile::tempdir().unwrap();
        with_roundtrip_target(profile, || {
            write_config(Format::Designer, &again, native_again.path())
        })
        .unwrap();
        let final_cfg = with_source_version(Some(profile), || {
            read_config(
                Format::Designer,
                native_again.path(),
                &ConvertOptions::default(),
            )
        })
        .unwrap()
        .0;
        assert_eq!(
            before,
            semantic(
                &final_cfg
                    .objects
                    .iter()
                    .find(|o| o.name == "Presence")
                    .unwrap()
                    .form_bodies[0]
                    .body
            )
        );
    }
}
