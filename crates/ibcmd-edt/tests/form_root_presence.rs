//! Authored root CI presence is independent of native's AutoCommandBar default.
//! Fixtures are owned synthetic XML; the original 3a9 failure remains in the lab.
use formats_xml::form::{
    apply_form_presence_resource, prepare_form_presence, read_form,
    validate_form_presence_resource, write_form, FormDialect,
};
use morph1c_core::{
    ir::{
        form::DataPathSpec, FormBody, FormCiItem, MetadataObject, NamedFormBody, ObjectKind,
        PropertyValue, Token, Uuid,
    },
    spec::forms::controls::form_field as ff,
    version::{with_roundtrip_target, with_source_version, FormatVersion},
};
use morph1c_pipeline::{read_config, write_config, ConvertOptions, Format};

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
        assert!(apply_form_presence_resource(
            &mut stale,
            Uuid([7; 16]),
            profile,
            old.resource.as_ref().unwrap(),
            None
        )
        .is_err());
        assert_eq!(before, semantic(&stale));
        native_roundtrip(&edited, profile);
        edited.command_interface = true;
        assert!(prepare_form_presence(
            &edited,
            Uuid([7; 16]),
            FormDialect::Designer,
            profile,
            None
        )
        .unwrap()
        .resource
        .is_none());
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
        assert!(validate_form_presence_resource(
            &projected,
            Uuid([7; 16]),
            FormatVersion::new(2, if minor == 20 { 21 } else { 20 }),
            &resource
        )
        .is_err());
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
        assert!(validate_form_presence_resource(
            &projected,
            Uuid([7; 16]),
            profile,
            duplicate.as_bytes()
        )
        .is_err());
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
            // The empty emitted form has no xsi values. The newly authored
            // typed field requires this namespace before its baseline is read.
            .replace(
                "<form:Form ",
                "<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" ",
            )
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
        let edt_src = edt_project_source(edt.path(), profile);
        with_roundtrip_target(profile, || write_config(Format::Edt, &restored, &edt_src)).unwrap();
        let again = with_source_version(Some(profile), || {
            read_config(Format::Edt, &edt_src, &ConvertOptions::default())
        })
        .unwrap()
        .0;
        assert_eq!(again.source_version, Some(profile));
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

fn config_with_form(body: &FormBody) -> morph1c_core::ir::Configuration {
    let source = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/subsystem-ci/src"
    ));
    let mut cfg = read_config(Format::Designer, source, &ConvertOptions::default())
        .unwrap()
        .0;
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
        body: body.clone(),
        ordinary_body: None,
        module: None,
        help: vec![],
        help_resources: vec![],
    });
    cfg.objects.push(obj);
    cfg
}
fn present_body(cfg: &morph1c_core::ir::Configuration) -> &FormBody {
    &cfg.objects
        .iter()
        .find(|o| o.name == "Presence")
        .unwrap()
        .form_bodies[0]
        .body
}
fn complete_standalone_picture_context(body: &mut FormBody) {
    // attach_form_body is a sidecar pass. read_config completion resolves
    // CURRENT CommonPicture defaults and binds the canonical picture model.
    // These fixtures declare no CommonPicture: use that real empty context
    // and the same production owners before comparing complete typed IR.
    formats_xml::form::resolve_common_picture_transparency(
        body,
        &std::collections::BTreeMap::new(),
        false,
    )
    .unwrap();
    formats_xml::form::bind_picture_semantics(body, Uuid([7; 16]), false).unwrap();
}
fn load_native(path: &std::path::Path, profile: FormatVersion) -> morph1c_core::ir::Configuration {
    with_source_version(Some(profile), || {
        read_config(Format::Designer, path, &ConvertOptions::default())
    })
    .unwrap()
    .0
}
fn edt_project_source(project: &std::path::Path, profile: FormatVersion) -> std::path::PathBuf {
    // Whole EDT reads obtain their real profile from PROJECT.PMF, not a form
    // sidecar or the caller's ambient scope. Complete that authored project
    // envelope before reread rather than relaxing closed resource validation.
    let runtime = morph1c_core::version::VERSION_TABLE
        .iter()
        .find(|pair| pair.format == profile)
        .unwrap()
        .platform;
    let dt_inf = project.join("DT-INF");
    std::fs::create_dir_all(&dt_inf).unwrap();
    std::fs::write(
        dt_inf.join("PROJECT.PMF"),
        format!("Manifest-Version: 1.0\r\nRuntime-Version: {runtime}\r\n\r\n"),
    )
    .unwrap();
    project.join("src")
}
fn bar_field(profile: FormatVersion, independent_extension: bool) -> FormBody {
    let field = if independent_extension {
        concat!("<items xsi:type=\"form:FormField\"><name>BarField</name><id>11</id><type>None</type>",
            "<extInfo xsi:type=\"form:LabelFieldExtInfo\"><useCopy>true</useCopy></extInfo></items>")
    } else {
        concat!("<items xsi:type=\"form:FormField\"><name>BarField</name><id>11</id>",
            "<dataPath xsi:type=\"form:DataPath\"><segments>List.Reference.Missing</segments></dataPath>",
            "<type>LabelField</type><extInfo xsi:type=\"form:LabelFieldExtInfo\"/></items>")
    };
    let xml = format!(concat!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
        "<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\">",
        "<autoCommandBar><name>FormCommandBar</name><id>-1</id>{}</autoCommandBar></form:Form>\r\n"), field);
    read(xml.as_bytes(), FormDialect::Edt, profile)
}
fn assert_whole_cycle(
    body: &FormBody,
    cfg: &morph1c_core::ir::Configuration,
    profile: FormatVersion,
) {
    let before = semantic(body);
    let native = tempfile::tempdir().unwrap();
    with_roundtrip_target(profile, || {
        write_config(Format::Designer, cfg, native.path())
    })
    .unwrap();
    let restored = load_native(native.path(), profile);
    assert_eq!(before, semantic(present_body(&restored)));
    let edt = tempfile::tempdir().unwrap();
    let edt_src = edt_project_source(edt.path(), profile);
    with_roundtrip_target(profile, || write_config(Format::Edt, &restored, &edt_src)).unwrap();
    let restored = with_source_version(Some(profile), || {
        read_config(Format::Edt, &edt_src, &ConvertOptions::default())
    })
    .unwrap()
    .0;
    assert_eq!(restored.source_version, Some(profile));
    assert_eq!(before, semantic(present_body(&restored)));
    let next = tempfile::tempdir().unwrap();
    with_roundtrip_target(profile, || {
        write_config(Format::Designer, &restored, next.path())
    })
    .unwrap();
    assert_eq!(
        before,
        semantic(present_body(&load_native(next.path(), profile)))
    );
    assert_eq!(before, semantic(body));
}
#[test]
fn root_bar_event_descendants_validate_original_wire_before_typed_restoration() {
    use morph1c_core::ir::FormEvent;
    for minor in [20, 21] {
        let profile = FormatVersion::new(2, minor);
        let mut body = bar_field(profile, true);
        let field = &mut body.auto_command_bar.as_mut().unwrap().items[0];
        assert!(field.field_type_none);
        assert_eq!(
            field.field_extension_kind.as_deref(),
            Some("form:LabelFieldExtInfo")
        );
        field.events.push(FormEvent {
            name: "StartChoice".into(),
            handler: "CurrentChoice".into(),
        });
        assert!(
            formats_xml::form::write_event_semantics_resource(&body, Uuid([7; 16]))
                .unwrap()
                .is_some()
        );
        let before = semantic(&body);
        let serialized: FormBody = serde_json::from_slice(&before).unwrap();
        assert_whole_cycle(&serialized, &config_with_form(&serialized), profile);
        let obj = config_with_form(&serialized).objects.pop().unwrap();
        let native = tempfile::tempdir().unwrap();
        let descriptor = native.path().join("CommonForms/Presence.xml");
        with_roundtrip_target(profile, || {
            morph1c_pipeline::write_form_bodies(Format::Designer, &descriptor, &obj)
        })
        .unwrap();
        let mut returned = obj.clone();
        returned.form_bodies.clear();
        with_source_version(Some(profile), || {
            morph1c_pipeline::attach_form_body(
                Format::Designer,
                "CommonForm",
                &descriptor,
                &mut returned,
            )
        })
        .unwrap();
        complete_standalone_picture_context(&mut returned.form_bodies[0].body);
        assert_eq!(before, semantic(&returned.form_bodies[0].body));
        let mut edited = serialized.clone();
        edited.auto_command_bar.as_mut().unwrap().items[0].events[0].handler =
            "EditedChoice".into();
        assert_whole_cycle(&edited, &config_with_form(&edited), profile);
    }
}
fn metadata_child(owner: &mut MetadataObject, name: &str, ty: &str, uuid: u8) {
    use morph1c_core::ir::{TypeRef, TypeSpec};
    let kind = format!("{}.Attribute", owner.kind.as_str());
    let mut child = MetadataObject::new(ObjectKind::new(&kind), name, Uuid([uuid; 16]));
    let id = morph1c_core::spec::registry::spec_for(&kind)
        .unwrap()
        .fields()
        .iter()
        .find(|f| f.name == "type")
        .unwrap()
        .id;
    child.properties.push((
        id,
        PropertyValue::Type(TypeSpec {
            parts: vec![TypeRef {
                id: ty.into(),
                qualifier: None,
            }],
        }),
    ));
    // A real Catalog.Attribute descriptor requires these three undefined
    // values. Define the full authored fixture before conversion and baseline.
    for name in ["minValue", "maxValue", "fillValue"] {
        let id = morph1c_core::spec::registry::spec_for(&kind)
            .unwrap()
            .fields()
            .iter()
            .find(|f| f.name == name)
            .unwrap()
            .id;
        child.properties.push((
            id,
            PropertyValue::Value(morph1c_core::ir::value::ValueSpec {
                kind: Default::default(),
                scalar: None,
            }),
        ));
    }
    owner.children.push(child);
}
#[test]
fn root_bar_rich_path_restoration_uses_complete_current_metadata_and_standalone_context() {
    use formats_xml::form::FormProjectionContext;
    for minor in [20, 21] {
        let profile = FormatVersion::new(2, minor);
        let mut body = bar_field(profile, false);
        let attr = concat!(
            "<attributes><name>List</name><valueType><types>DynamicList</types></valueType>",
            "<view><common>true</common></view><edit><common>true</common></edit>",
            "<extInfo xsi:type=\"form:DynamicListExtInfo\"><mainTable>Catalog.Source</mainTable>",
            "<autoFillAvailableFields>true</autoFillAvailableFields></extInfo></attributes>"
        );
        let xml = with_roundtrip_target(profile, || write_form(FormDialect::Edt, &body)).unwrap();
        let xml = String::from_utf8(xml)
            .unwrap()
            .replace("</form:Form>", &format!("{attr}</form:Form>"));
        body = read(xml.as_bytes(), FormDialect::Edt, profile);
        body.auto_command_bar.as_mut().unwrap().items[0]
            .properties
            .iter_mut()
            .find(|(id, _)| *id == ff::F_DATA_PATH)
            .unwrap()
            .1 = PropertyValue::DataPath(DataPathSpec {
            segments: vec!["List".into(), "Reference".into(), "Missing".into()],
            extra_paths: vec!["Other.B".into(), "Next.C".into()],
        });
        let mut cfg = config_with_form(&body);
        // These owners must be complete native metadata, not graph-only stubs.
        // Reuse all canonical properties from the verified owned Catalog fixture
        // under new identities before defining the conversion's authored input.
        let catalog_properties = cfg
            .objects
            .iter()
            .find(|o| o.kind.as_str() == "Catalog")
            .unwrap()
            .properties
            .clone();
        let mut source = MetadataObject::new(ObjectKind::new("Catalog"), "Source", Uuid([31; 16]));
        source.properties = catalog_properties.clone();
        metadata_child(&mut source, "Reference", "CatalogRef.Target", 32);
        let mut target = MetadataObject::new(ObjectKind::new("Catalog"), "Target", Uuid([33; 16]));
        target.properties = catalog_properties;
        metadata_child(&mut target, "Known", "Boolean", 34);
        cfg.objects.extend([source, target]);
        let context = FormProjectionContext::new(&cfg).unwrap();
        let package = prepare_form_presence(
            &body,
            Uuid([7; 16]),
            FormDialect::Designer,
            profile,
            Some(&context),
        )
        .unwrap();
        let without =
            prepare_form_presence(&body, Uuid([7; 16]), FormDialect::Designer, profile, None)
                .unwrap();
        assert_ne!(
            package.resource, without.resource,
            "fixture must witness metadata-dependent root wire digest"
        );
        assert!(String::from_utf8_lossy(&package.bytes).contains("~List.Reference.Missing"));
        assert_whole_cycle(&body, &cfg, profile);
        let obj = cfg.objects.iter().find(|o| o.name == "Presence").unwrap();
        let standalone = tempfile::tempdir().unwrap();
        let descriptor = standalone.path().join("CommonForms/Presence.xml");
        with_roundtrip_target(profile, || {
            morph1c_pipeline::write_form_bodies_with_context(
                Format::Designer,
                &descriptor,
                obj,
                &context,
            )
        })
        .unwrap();
        let mut reread = obj.clone();
        reread.form_bodies.clear();
        with_source_version(Some(profile), || {
            morph1c_pipeline::attach_form_body_with_context(
                Format::Designer,
                "CommonForm",
                &descriptor,
                &mut reread,
                &context,
            )
        })
        .unwrap();
        complete_standalone_picture_context(&mut reread.form_bodies[0].body);
        assert_eq!(semantic(&body), semantic(&reread.form_bodies[0].body));
        // A context-free restoration cannot reproduce this actual forward artifact.
        let mut no_context = obj.clone();
        no_context.form_bodies.clear();
        assert!(
            with_source_version(Some(profile), || morph1c_pipeline::attach_form_body(
                Format::Designer,
                "CommonForm",
                &descriptor,
                &mut no_context
            ))
            .is_err()
        );
        assert!(no_context.form_bodies.is_empty());
        let mut edited = cfg.clone();
        edited
            .objects
            .iter_mut()
            .find(|o| o.name == "Target")
            .unwrap()
            .children[0]
            .name = "Missing".into();
        assert_whole_cycle(&body, &edited, profile);
    }
}
#[test]
fn same_destination_updates_remove_owned_root_companions_and_preserve_unrelated_manifest() {
    for minor in [20, 21] {
        let profile = FormatVersion::new(2, minor);
        let body = fixture(profile, true, false);
        let mut cfg = config_with_form(&body);
        let native = tempfile::tempdir().unwrap();
        let descriptor = native.path().join("CommonForms/Presence.xml");
        let sidecar = native
            .path()
            .join("CommonForms/Presence/Ext")
            .join(formats_xml::form::FORM_PRESENCE_RESOURCE);
        let obj = cfg.objects.last().unwrap();
        with_roundtrip_target(profile, || {
            morph1c_pipeline::write_form_bodies(Format::Designer, &descriptor, obj)
        })
        .unwrap();
        assert!(sidecar.is_file());
        with_roundtrip_target(profile, || {
            write_config(Format::Designer, &cfg, native.path())
        })
        .unwrap();
        assert!(
            !sidecar.exists(),
            "whole manifest owns the same root intent"
        );
        assert_eq!(
            semantic(&body),
            semantic(present_body(&load_native(native.path(), profile)))
        );
        let manifest_path = native.path().join("ConfigDumpInfo.xml");
        let manifest = String::from_utf8(std::fs::read(&manifest_path).unwrap()).unwrap();
        let marker = "<!-- platform-owned-comment --><PlatformOwned value=\"keep\"/>";
        let manifest = manifest.replace("</ConfigDumpInfo>", &format!("{marker}</ConfigDumpInfo>"));
        std::fs::write(&manifest_path, manifest.as_bytes()).unwrap();
        cfg.objects.last_mut().unwrap().form_bodies[0]
            .body
            .command_interface = true;
        with_roundtrip_target(profile, || {
            write_config(Format::Designer, &cfg, native.path())
        })
        .unwrap();
        let updated = std::fs::read(&manifest_path).unwrap();
        let comment_start = manifest
            .find("<!-- ibcmd-configuration-semantics:")
            .unwrap();
        let comment_end = comment_start + manifest[comment_start..].find("-->").unwrap() + 3;
        let mut expected = manifest.as_bytes().to_vec();
        expected.drain(comment_start..comment_end);
        assert_eq!(
            updated, expected,
            "only the exact protocol comment may disappear"
        );
        assert!(String::from_utf8_lossy(&updated).contains(marker));
        assert!(!String::from_utf8_lossy(&updated).contains("ibcmd-configuration-semantics:"));
        assert_eq!(
            semantic(present_body(&cfg)),
            semantic(present_body(&load_native(native.path(), profile)))
        );
        cfg.objects.last_mut().unwrap().form_bodies[0]
            .body
            .command_interface = false;
        with_roundtrip_target(profile, || {
            write_config(Format::Designer, &cfg, native.path())
        })
        .unwrap();
        assert!(String::from_utf8_lossy(&std::fs::read(&manifest_path).unwrap()).contains(marker));
        assert_eq!(
            semantic(&body),
            semantic(present_body(&load_native(native.path(), profile)))
        );
        // Isolated standalone output has no manifest. False -> true removes its owned sidecar.
        let standalone = tempfile::tempdir().unwrap();
        let descriptor = standalone.path().join("CommonForms/Presence.xml");
        let sidecar = standalone
            .path()
            .join("CommonForms/Presence/Ext")
            .join(formats_xml::form::FORM_PRESENCE_RESOURCE);
        let mut obj = cfg.objects.last().unwrap().clone();
        with_roundtrip_target(profile, || {
            morph1c_pipeline::write_form_bodies(Format::Designer, &descriptor, &obj)
        })
        .unwrap();
        assert!(sidecar.exists());
        obj.form_bodies[0].body.command_interface = true;
        with_roundtrip_target(profile, || {
            morph1c_pipeline::write_form_bodies(Format::Designer, &descriptor, &obj)
        })
        .unwrap();
        assert!(!sidecar.exists());
        let before = semantic(&obj.form_bodies[0].body);
        obj.form_bodies.clear();
        with_source_version(Some(profile), || {
            morph1c_pipeline::attach_form_body(
                Format::Designer,
                "CommonForm",
                &descriptor,
                &mut obj,
            )
        })
        .unwrap();
        complete_standalone_picture_context(&mut obj.form_bodies[0].body);
        assert_eq!(before, semantic(&obj.form_bodies[0].body));
    }
}

#[test]
fn same_destination_rejects_foreign_or_unknown_owned_resources_before_publication() {
    for minor in [20, 21] {
        let profile = FormatVersion::new(2, minor);
        let body = fixture(profile, true, false);
        let cfg = config_with_form(&body);
        let native = tempfile::tempdir().unwrap();
        with_roundtrip_target(profile, || {
            write_config(Format::Designer, &cfg, native.path())
        })
        .unwrap();
        let manifest_path = native.path().join("ConfigDumpInfo.xml");
        let original = std::fs::read(&manifest_path).unwrap();
        let bad = String::from_utf8(original.clone()).unwrap().replace(
            "ibcmd-configuration-semantics:2:",
            "ibcmd-configuration-semantics:3:",
        );
        std::fs::write(&manifest_path, bad.as_bytes()).unwrap();
        let before_body =
            std::fs::read(native.path().join("CommonForms/Presence/Ext/Form.xml")).unwrap();
        let mut edited = cfg.clone();
        edited.objects.last_mut().unwrap().form_bodies[0]
            .body
            .command_interface = true;
        assert!(with_roundtrip_target(profile, || write_config(
            Format::Designer,
            &edited,
            native.path()
        ))
        .is_err());
        assert_eq!(std::fs::read(&manifest_path).unwrap(), bad.as_bytes());
        assert_eq!(
            std::fs::read(native.path().join("CommonForms/Presence/Ext/Form.xml")).unwrap(),
            before_body
        );
        std::fs::write(&manifest_path, &original).unwrap();
        let descriptor = native.path().join("CommonForms/Presence.xml");
        let sidecar = native
            .path()
            .join("CommonForms/Presence/Ext")
            .join(formats_xml::form::FORM_PRESENCE_RESOURCE);
        let package =
            prepare_form_presence(&body, Uuid([99; 16]), FormDialect::Designer, profile, None)
                .unwrap();
        std::fs::write(&sidecar, package.resource.unwrap()).unwrap();
        assert!(with_roundtrip_target(profile, || write_config(
            Format::Designer,
            &edited,
            native.path()
        ))
        .is_err());
        assert_eq!(std::fs::read(&manifest_path).unwrap(), original);
        assert_eq!(
            std::fs::read(native.path().join("CommonForms/Presence/Ext/Form.xml")).unwrap(),
            before_body
        );
        assert!(
            with_roundtrip_target(profile, || morph1c_pipeline::write_form_bodies(
                Format::Designer,
                &descriptor,
                edited.objects.last().unwrap()
            ))
            .is_err()
        );
        assert_eq!(
            std::fs::read(native.path().join("CommonForms/Presence/Ext/Form.xml")).unwrap(),
            before_body
        );
        let wrong_cfg = formats_xml::form::update_native_data_path_annotation(
            Some(&original),
            None,
            profile,
            Some(Uuid([99; 16])),
        );
        assert!(wrong_cfg.is_err());
        let empty = format!(
            "<ConfigDumpInfo xmlns=\"http://v8.1c.ru/8.3/xcf/dumpinfo\" version=\"2.{minor}\"/>"
        );
        let root_uuid = cfg
            .objects
            .iter()
            .find(|o| o.kind.as_str() == "Configuration")
            .map(|o| o.uuid);
        let expanded = formats_xml::form::update_native_data_path_annotation(
            Some(empty.as_bytes()),
            Some(&original),
            profile,
            root_uuid,
        )
        .unwrap()
        .unwrap();
        assert!(
            formats_xml::form::read_native_data_path_annotation(&expanded, profile)
                .unwrap()
                .is_some()
        );
    }
}

#[test]
fn owned_annotation_spans_preserve_bom_unicode_and_adjacent_markup_exactly() {
    for minor in [20, 21] {
        let profile = FormatVersion::new(2, minor);
        let cfg = config_with_form(&fixture(profile, true, false));
        let root_uuid = cfg
            .objects
            .iter()
            .find(|o| o.kind.as_str() == "Configuration")
            .map(|o| o.uuid);
        let generated = formats_xml::form::prepare_native_form_write_plan(&cfg, profile)
            .unwrap()
            .manifest
            .unwrap();
        let generated_text = String::from_utf8(generated.clone()).unwrap();
        let start = generated_text
            .find("<!-- ibcmd-configuration-semantics:")
            .unwrap();
        let end = start + generated_text[start..].find("-->").unwrap() + 3;
        let comment = &generated_text[start..end];
        let plain = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><ConfigDumpInfo xmlns=\"http://v8.1c.ru/8.3/xcf/dumpinfo\" version=\"2.{minor}\"><!-- чужое --><ConfigVersions/><Other>Кириллица</Other></ConfigDumpInfo>");
        for existing_bom in [false, true] {
            let mut existing = if existing_bom {
                b"\xef\xbb\xbf".to_vec()
            } else {
                vec![]
            };
            existing.extend_from_slice(plain.as_bytes());
            for current_bom in [false, true] {
                let current = if current_bom {
                    generated.as_slice()
                } else {
                    generated
                        .strip_prefix(b"\xef\xbb\xbf")
                        .unwrap_or(&generated)
                };
                let installed = formats_xml::form::update_native_data_path_annotation(
                    Some(&existing),
                    Some(current),
                    profile,
                    root_uuid,
                )
                .unwrap()
                .unwrap();
                let expected_text =
                    plain.replace("</ConfigDumpInfo>", &format!("{comment}</ConfigDumpInfo>"));
                let mut expected = if existing_bom {
                    b"\xef\xbb\xbf".to_vec()
                } else {
                    vec![]
                };
                expected.extend_from_slice(expected_text.as_bytes());
                assert_eq!(installed, expected);
                let removed = formats_xml::form::update_native_data_path_annotation(
                    Some(&installed),
                    None,
                    profile,
                    root_uuid,
                )
                .unwrap()
                .unwrap();
                assert_eq!(removed, existing, "only the exact comment span may change");
                let replaced = formats_xml::form::update_native_data_path_annotation(
                    Some(&installed),
                    Some(current),
                    profile,
                    root_uuid,
                )
                .unwrap()
                .unwrap();
                assert_eq!(replaced, installed);
            }
        }
    }
}
