use formats_xml::form::{
    FormDialect, bind_picture_semantics, project_picture_semantics, read_form,
    read_picture_semantics_resource, resolve_common_picture_transparency, write_form,
};
use morph1c_core::ir::{FormBody, FormCommand, FormControlKind, FormItem, PropertyValue, Uuid};
use morph1c_core::spec::forms::{command as fc, controls::button as bt};
use std::collections::BTreeMap;

const UUID: Uuid = Uuid([42; 16]);
fn picture(reference: &str, flag: bool, pixel: Option<(i64, i64)>) -> PropertyValue {
    let mut parts = vec![
        PropertyValue::Ref(reference.into()),
        PropertyValue::Bool(flag),
    ];
    if let Some((x, y)) = pixel {
        parts.push(PropertyValue::List(vec![
            PropertyValue::Int(x),
            PropertyValue::Int(y),
        ]));
    }
    PropertyValue::List(parts)
}
fn body() -> FormBody {
    let mut body = FormBody::new();
    let mut command = FormCommand::new("Command", 7);
    command.properties.push((
        fc::F_PICTURE,
        picture(
            "0:268b94cf-1d8b-4809-a21d-2538352896b9",
            true,
            Some((13, 1)),
        ),
    ));
    body.commands.push(command);
    let mut item = FormItem::new(FormControlKind::new("Button"), "Button", 5);
    item.properties.push((
        bt::F_BUTTON_TYPE,
        PropertyValue::Enum(morph1c_core::ir::Token::new("CommandBarButton")),
    ));
    item.properties
        .push((bt::F_PICTURE, picture("StdPicture.Save", false, None)));
    body.items.push(item);
    body
}
fn restore(
    mut projected: FormBody,
    bytes: &[u8],
    defaults: &BTreeMap<String, bool>,
) -> Result<FormBody, formats_xml::form::FormError> {
    let model = read_picture_semantics_resource(bytes)?;
    projected.picture_resource_selection = Some(
        model
            .records
            .iter()
            .map(|row| row.binding.clone())
            .collect(),
    );
    projected.picture_semantics = Some(model);
    resolve_common_picture_transparency(&mut projected, defaults, true)?;
    bind_picture_semantics(&mut projected, UUID, true)?;
    Ok(projected)
}
#[test]
fn typed_resource_preserves_ref_bool_pixel_and_all_values_in_fingerprint() {
    let mut original = body();
    bind_picture_semantics(&mut original, UUID, false).unwrap();
    let (projected, bytes) = project_picture_semantics(&original, UUID).unwrap();
    let bytes = bytes.unwrap();
    let model = read_picture_semantics_resource(&bytes).unwrap();
    assert_eq!(model.records.len(), 2);
    let restored = restore(projected, &bytes, &BTreeMap::new()).unwrap();
    assert_eq!(
        serde_json::to_vec(&original).unwrap(),
        serde_json::to_vec(&restored).unwrap()
    );
    let mut edited: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    edited["form"]["records"][0]["pixel"]["x"] = 14.into();
    let (projected, _) = project_picture_semantics(&original, UUID).unwrap();
    let restored = restore(
        projected,
        &serde_json::to_vec(&edited).unwrap(),
        &BTreeMap::new(),
    )
    .unwrap();
    assert_ne!(
        serde_json::to_vec(&original).unwrap(),
        serde_json::to_vec(&restored).unwrap()
    );
    assert_eq!(
        restored.picture_semantics.unwrap().records[0]
            .pixel
            .unwrap()
            .x,
        14
    );
}
#[test]
fn unknown_duplicate_wrong_form_ref_and_control_bindings_fail_closed() {
    let (projected, bytes) = project_picture_semantics(&body(), UUID).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes.unwrap()).unwrap();
    let mut cases = Vec::new();
    let mut v = value.clone();
    v["extra"] = true.into();
    cases.push(v);
    let mut v = value.clone();
    v["version"] = 2.into();
    cases.push(v);
    let mut v = value.clone();
    v["form"]["form_uuid"][0] = 43.into();
    cases.push(v);
    let mut v = value.clone();
    v["form"]["records"][0]["reference"] = "StdPicture.Save".into();
    cases.push(v);
    let mut v = value.clone();
    v["form"]["records"][0]["binding"]["id"] = 8.into();
    cases.push(v);
    let mut v = value.clone();
    v["form"]["records"][1]["binding"]["slot"] = "UnknownPicture".into();
    cases.push(v);
    let mut v = value.clone();
    v["form"]["records"][0]["load_transparent"] = false.into();
    cases.push(v);
    let mut v = value.clone();
    let row = v["form"]["records"][0].clone();
    v["form"]["records"].as_array_mut().unwrap().push(row);
    cases.push(v);
    for v in cases {
        assert!(
            restore(
                projected.clone(),
                &serde_json::to_vec(&v).unwrap(),
                &BTreeMap::new()
            )
            .is_err(),
            "accepted {v}"
        );
    }
}
#[test]
fn metadata_pixel_edits_keep_independent_resource_flag_and_current_semantics() {
    let mut native = FormBody::new();
    let mut command = FormCommand::new("Command", 7);
    command
        .properties
        .push((fc::F_PICTURE, picture("CommonPicture.Picture", true, None)));
    native.commands.push(command);
    let defaults = BTreeMap::from([("CommonPicture.Picture".into(), false)]);
    resolve_common_picture_transparency(&mut native, &defaults, false).unwrap();
    bind_picture_semantics(&mut native, UUID, false).unwrap();
    let (projected, bytes) = project_picture_semantics(&native, UUID).unwrap();
    let bytes = bytes.unwrap();
    for flag in [true, false, true] {
        let changed = BTreeMap::from([("CommonPicture.Picture".into(), flag)]);
        let restored = restore(projected.clone(), &bytes, &changed).unwrap();
        let mut expected = native.clone();
        resolve_common_picture_transparency(&mut expected, &changed, false).unwrap();
        bind_picture_semantics(&mut expected, UUID, false).unwrap();
        assert_eq!(
            serde_json::to_vec(&expected).unwrap(),
            serde_json::to_vec(&restored).unwrap()
        );
        assert!(
            read_picture_semantics_resource(
                &project_picture_semantics(&restored, UUID)
                    .unwrap()
                    .1
                    .unwrap()
            )
            .unwrap()
            .records[0]
                .load_transparent
        );
    }
}
#[test]
fn choices_bind_full_typed_value_and_presentation_not_picture_position() {
    use morph1c_core::ir::{Lang, ValueScalarKind, ValueSpec};
    use morph1c_core::spec::forms::controls::radio_button as rb;
    let mut native = FormBody::new();
    let mut item = FormItem::new(FormControlKind::new("RadioButtonField"), "Choice", 12);
    let choice = |text: &str, pixel: i64| {
        PropertyValue::List(vec![
            PropertyValue::Localized(vec![(Lang::new("ru"), text.into())]),
            PropertyValue::Value(ValueSpec {
                kind: ValueScalarKind::Str,
                scalar: Some(Box::new(PropertyValue::Str(text.into()))),
            }),
            picture("StdPicture.Save", true, Some((pixel, 0))),
        ])
    };
    item.ext_info.push((
        rb::F_EXT_CHOICE_LIST,
        PropertyValue::List(vec![choice("A", 1), choice("B", 2)]),
    ));
    native.items.push(item);
    bind_picture_semantics(&mut native, UUID, false).unwrap();
    let (mut projected, bytes) = project_picture_semantics(&native, UUID).unwrap();
    let PropertyValue::List(choices) = &mut projected.items[0].ext_info[0].1 else {
        panic!()
    };
    choices.swap(0, 1);
    let restored = restore(projected, &bytes.unwrap(), &BTreeMap::new()).unwrap();
    let model = restored.picture_semantics.unwrap();
    assert_eq!(model.records[0].pixel.unwrap().x, 2);
    assert_eq!(model.records[1].pixel.unwrap().x, 1);
}
#[test]
fn projection_is_sdk_compatible_and_native_current_values_remain_exact() {
    use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
    let original = body();
    let source = with_roundtrip_target(FormatVersion::new(2, 21), || {
        write_form(FormDialect::Designer, &original)
    })
    .unwrap();
    let native = with_source_version(Some(FormatVersion::new(2, 21)), || {
        read_form(FormDialect::Designer, &source)
    })
    .unwrap();
    let (projected, bytes) = project_picture_semantics(&native, UUID).unwrap();
    let edt = with_roundtrip_target(FormatVersion::new(2, 21), || {
        write_form(FormDialect::Edt, &projected)
    })
    .unwrap();
    let reread = with_source_version(Some(FormatVersion::new(2, 21)), || {
        read_form(FormDialect::Edt, &edt)
    })
    .unwrap();
    let restored = restore(reread, &bytes.unwrap(), &BTreeMap::new()).unwrap();
    let returned = with_roundtrip_target(FormatVersion::new(2, 21), || {
        write_form(FormDialect::Designer, &restored)
    })
    .unwrap();
    assert_eq!(
        String::from_utf8(returned).unwrap(),
        String::from_utf8(source).unwrap()
    );
}

#[test]
fn public_resource_without_provenance_returns_exact_native_and_edited_pixel_changes_semantics() {
    use ibcmd_edt::{
        ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
    };
    use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
    use morph1c_core::ir::{MetadataObject, NamedFormBody, ObjectKind, Token};
    use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
    use sha2::{Digest, Sha256};
    let fixture = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/subsystem-ci/src"
    ));
    let mut cfg = read_config(Format::Designer, fixture, &ConvertOptions::default())
        .unwrap()
        .0;
    let mut object = MetadataObject::new(ObjectKind::new("CommonForm"), "PictureReferences", UUID);
    let form_type = morph1c_core::spec::registry::spec_for("CommonForm")
        .unwrap()
        .fields()
        .iter()
        .find(|field| field.name == "formType")
        .unwrap()
        .id;
    object
        .properties
        .push((form_type, PropertyValue::Enum(Token::new("Managed"))));
    object.form_bodies.push(NamedFormBody {
        name: "PictureReferences".into(),
        body: body(),
        ordinary_body: None,
        module: None,
        help: vec![],
        help_resources: vec![],
    });
    cfg.objects.push(object);
    let dir = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &cfg, dir.path()).unwrap();
    let path = dir.path().join("Configuration.xml");
    let root = String::from_utf8(std::fs::read(&path).unwrap())
        .unwrap()
        .replace(
            "</Language>",
            "</Language>\r\n\t\t\t<CommonForm>PictureReferences</CommonForm>",
        );
    std::fs::write(path, root).unwrap();
    let original = read_xml_source(dir.path(), ReaderLimits::default()).unwrap();
    let options = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    let converted = xml_to_edt(&original, &options).unwrap();
    assert_eq!(converted.extensions.len(), 1);
    assert_eq!(converted.extensions[0].resources, 1);
    assert_eq!(converted.extensions[0].references, 2);
    let generated = converted.tree;
    let stripped = SourceTree::new(
        generated
            .entries()
            .iter()
            .filter(|entry| !entry.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap();
    let returned = edt_to_xml(&Project::from_tree(stripped.clone()).unwrap(), &options).unwrap();
    assert_eq!(returned.extensions[0].references, 2);
    assert_eq!(returned.tree, original);
    let resource_path = "src/CommonForms/PictureReferences/ibcmd-picture-semantics.v1.json";
    let resource = generated
        .entries()
        .iter()
        .find(|entry| entry.path().as_str() == resource_path)
        .unwrap();
    let mut edited: serde_json::Value = serde_json::from_slice(resource.bytes()).unwrap();
    edited["form"]["records"][0]["pixel"]["x"] = 14.into();
    let edited = serde_json::to_vec(&edited).unwrap();
    let mutate = |tree: &SourceTree, path: &str, bytes: &[u8]| {
        SourceTree::new(
            tree.entries()
                .iter()
                .map(|entry| {
                    if entry.path().as_str() == path {
                        SourceEntry::from_bytes(SourcePath::new(path).unwrap(), bytes.to_vec())
                            .unwrap()
                    } else {
                        entry.clone()
                    }
                })
                .collect(),
        )
        .unwrap()
    };
    let changed = edt_to_xml(
        &Project::from_tree(mutate(&stripped, resource_path, &edited)).unwrap(),
        &options,
    )
    .unwrap();
    let xml = changed
        .tree
        .entries()
        .iter()
        .find(|entry| entry.path().as_str() == "CommonForms/PictureReferences/Ext/Form.xml")
        .unwrap();
    assert!(String::from_utf8_lossy(xml.bytes()).contains("TransparentPixel x=\"14\" y=\"1\""));
    assert_ne!(changed.tree, original);
    let mut manifest: serde_json::Value = serde_json::from_slice(
        generated
            .entries()
            .iter()
            .find(|entry| entry.path().as_str() == ".ibcmd-provenance/manifest.json")
            .unwrap()
            .bytes(),
    )
    .unwrap();
    manifest["generated"][resource_path] = format!("{:x}", Sha256::digest(&edited)).into();
    let forged = mutate(
        &mutate(&generated, resource_path, &edited),
        ".ibcmd-provenance/manifest.json",
        &serde_json::to_vec(&manifest).unwrap(),
    );
    assert!(
        edt_to_xml(&Project::from_tree(forged).unwrap(), &options)
            .unwrap_err()
            .to_string()
            .contains("changed")
    );
}

#[test]
#[ignore = "Read-only three genuine UH UUID picture commands, isolated public adapter transport on F"]
fn genuine_uuid_picture_commands_keep_identity_and_pixels_in_public_stripped_transport() {
    use ibcmd_edt::{
        ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
    };
    use ibcmd_xml::source_tree::SourceTree;
    use morph1c_core::ir::{MetadataObject, NamedFormBody, ObjectKind, Token};
    use morph1c_core::version::{FormatVersion, with_source_version};
    use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
    use std::path::Path;
    let original_root = Path::new(r"F:\ibcmd\lab\04\release-20261001\rc\out\uha8327_db_r1\tree");
    let edt_root =
        Path::new(r"F:\ibcmd\lab\07\oracle-uha83-r1\authentic-workspace\OracleConfiguration\src");
    let sdk_root = Path::new(r"F:\ibcmd\lab\07\native-reference-uha83-r1\native-xml");
    for (processor, form_name, reference, x, y) in [
        (
            "СверткаВерсий",
            "ФормаУправляемая",
            "0:268b94cf-1d8b-4809-a21d-2538352896b9",
            13,
            1,
        ),
        (
            "СверкаВГО",
            "ФормаУправляемая",
            "0:7f6adcef-099d-4d35-8504-145bdea3620e",
            0,
            14,
        ),
        (
            "НастройкаМатрицыПолномочий",
            "Форма_Управляемая",
            "0:52958592-afcf-4b18-a10c-f10059f5a67b",
            2,
            10,
        ),
    ] {
        let rel = Path::new("DataProcessors")
            .join(processor)
            .join("Forms")
            .join(form_name);
        let descriptor =
            std::fs::read_to_string(original_root.join(&rel).with_extension("xml")).unwrap();
        let uuid = descriptor
            .split_once("<Form uuid=\"")
            .unwrap()
            .1
            .split_once('"')
            .unwrap()
            .0;
        let uuid = formats_xml::children::parse_uuid(uuid).unwrap();
        let read = |dialect, bytes: &[u8]| {
            with_source_version(Some(FormatVersion::new(2, 20)), || {
                read_form(dialect, bytes)
            })
            .unwrap()
        };
        let source = read(
            FormDialect::Designer,
            &std::fs::read(original_root.join(&rel).join("Ext/Form.xml")).unwrap(),
        );
        let genuine_edt = read(
            FormDialect::Edt,
            &std::fs::read(edt_root.join(&rel).join("Form.form")).unwrap(),
        );
        let genuine_sdk = read(
            FormDialect::Designer,
            &std::fs::read(sdk_root.join(&rel).join("Ext/Form.xml")).unwrap(),
        );
        let has_ref = |command: &FormCommand| {
            command.properties.iter().any(|(id,value)|*id==fc::F_PICTURE && matches!(value,PropertyValue::List(parts) if matches!(parts.first(),Some(PropertyValue::Ref(r)) if r==reference)))
        };
        let command = source
            .commands
            .iter()
            .find(|command| has_ref(command))
            .unwrap()
            .clone();
        let original_picture = command
            .properties
            .iter()
            .find(|(id, _)| *id == fc::F_PICTURE)
            .unwrap()
            .1
            .clone();
        assert_eq!(original_picture, picture(reference, true, Some((x, y))));
        for counterpart in [&genuine_edt, &genuine_sdk] {
            let other = counterpart
                .commands
                .iter()
                .find(|other| other.id == command.id && other.name == command.name)
                .unwrap();
            assert_eq!(
                other
                    .properties
                    .iter()
                    .find(|(id, _)| *id == fc::F_PICTURE)
                    .unwrap()
                    .1,
                picture(reference, false, None)
            );
        }
        // Isolate the complete genuine command in a declared fixture form. This
        // exercises public resource transport; it is not whole-UH SDK acceptance.
        let fixture = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        ));
        let mut cfg = read_config(Format::Designer, fixture, &ConvertOptions::default())
            .unwrap()
            .0;
        let mut object =
            MetadataObject::new(ObjectKind::new("CommonForm"), "GenuinePictureCommand", uuid);
        let form_type = morph1c_core::spec::registry::spec_for("CommonForm")
            .unwrap()
            .fields()
            .iter()
            .find(|field| field.name == "formType")
            .unwrap()
            .id;
        object
            .properties
            .push((form_type, PropertyValue::Enum(Token::new("Managed"))));
        let mut body = FormBody::new();
        body.commands.push(command);
        object.form_bodies.push(NamedFormBody {
            name: "GenuinePictureCommand".into(),
            body,
            ordinary_body: None,
            module: None,
            help: vec![],
            help_resources: vec![],
        });
        cfg.objects.push(object);
        let dir = tempfile::tempdir().unwrap();
        write_config(Format::Designer, &cfg, dir.path()).unwrap();
        let path = dir.path().join("Configuration.xml");
        let root = std::fs::read_to_string(&path).unwrap().replace(
            "</Language>",
            "</Language>\r\n\t\t\t<CommonForm>GenuinePictureCommand</CommonForm>",
        );
        std::fs::write(path, root).unwrap();
        let original = read_xml_source(dir.path(), ReaderLimits::default()).unwrap();
        let options = ConversionOptions {
            edt_version: "2025.2.3".into(),
            xml_dialect: "2.21".into(),
            runtime_version: Some("8.5.1".into()),
        };
        let generated = xml_to_edt(&original, &options).unwrap();
        assert_eq!(generated.extensions[0].references, 1);
        let stripped = SourceTree::new(
            generated
                .tree
                .entries()
                .iter()
                .filter(|entry| !entry.path().as_str().starts_with(".ibcmd-provenance/"))
                .cloned()
                .collect(),
        )
        .unwrap();
        assert_eq!(
            edt_to_xml(&Project::from_tree(stripped).unwrap(), &options)
                .unwrap()
                .tree,
            original
        );
    }
}

#[test]
#[ignore = "Read-only all 102 genuine UH per-use pixel carrier forms on F, including current typed metadata"]
fn genuine_all_per_use_carriers_restore_every_typed_ref_flag_and_pixel() {
    use morph1c_core::spec::metadata::common_picture::F_TRANSPARENT_PIXEL;
    use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
    use morph1c_pipeline::{Format, FormatRegistry};
    use sha2::{Digest, Sha256};
    use std::path::Path;
    let report: serde_json::Value = serde_json::from_slice(
        &std::fs::read(r"F:\ibcmd\lab\07\uha83-form-census-xml.json").unwrap(),
    )
    .unwrap();
    let root = Path::new(r"F:\ibcmd\lab\04\release-20261001\rc\out\uha8327_db_r1\tree");
    let edt =
        Path::new(r"F:\ibcmd\lab\07\oracle-uha83-r1\authentic-workspace\OracleConfiguration\src");
    let mut paths = Vec::new();
    for (kind, entries) in report["failures"].as_object().unwrap() {
        if [
            "<HeaderPicture>",
            "<Picture>",
            "<RowsPicture>",
            "<ValuesPicture>",
            "Command Picture",
        ]
        .iter()
        .any(|known| kind.contains(known))
        {
            paths.extend(
                entries
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|entry| entry.as_str().unwrap().to_owned()),
            );
        }
    }
    paths.sort();
    paths.dedup();
    assert_eq!(paths.len(), 102);
    let registry = FormatRegistry::for_format(Format::Edt).unwrap();
    let picture_reader = registry.get("CommonPicture").unwrap().read;
    let mut metadata = BTreeMap::new();
    let mut evidence = Vec::new();
    let mut pixels = 0;
    let start = std::time::Instant::now();
    for relative in paths {
        let path = root.join(&relative);
        let bytes = std::fs::read(&path).unwrap();
        let mut source = with_source_version(Some(FormatVersion::new(2, 20)), || {
            read_form(FormDialect::Designer, &bytes)
        })
        .unwrap();
        let descriptor = path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .with_extension("xml");
        let descriptor = std::fs::read_to_string(descriptor).unwrap();
        let uuid = descriptor
            .split_once(" uuid=\"")
            .unwrap()
            .1
            .split_once('"')
            .unwrap()
            .0;
        let uuid = formats_xml::children::parse_uuid(uuid).unwrap();
        bind_picture_semantics(&mut source, uuid, false).unwrap();
        for row in &source.picture_semantics.as_ref().unwrap().records {
            if let Some(name) = row.reference.strip_prefix("CommonPicture.")
                && !metadata.contains_key(&row.reference)
            {
                let bytes = std::fs::read(
                    edt.join("CommonPictures")
                        .join(name)
                        .join(format!("{name}.mdo")),
                )
                .unwrap();
                let picture =
                    with_source_version(Some(FormatVersion::new(2, 20)), || picture_reader(&bytes))
                        .unwrap();
                let present = match picture.get(F_TRANSPARENT_PIXEL) {
                    None => false,
                    Some(PropertyValue::List(values)) => !values.is_empty(),
                    Some(other) => panic!("invalid typed CommonPicture pixel {other:?}"),
                };
                metadata.insert(row.reference.clone(), present);
            }
        }
        resolve_common_picture_transparency(&mut source, &metadata, false).unwrap();
        bind_picture_semantics(&mut source, uuid, false).unwrap();
        let expected = source.picture_semantics.clone().unwrap();
        let pixel_count = expected
            .records
            .iter()
            .filter(|row| row.pixel.is_some())
            .count();
        pixels += pixel_count;
        let (projected, resource) = project_picture_semantics(&source, uuid).unwrap();
        let resource = resource.unwrap();
        let form = with_roundtrip_target(FormatVersion::new(2, 20), || {
            write_form(FormDialect::Edt, &projected)
        })
        .unwrap();
        let mut reread = with_source_version(Some(FormatVersion::new(2, 20)), || {
            read_form(FormDialect::Edt, &form)
        })
        .unwrap();
        let model = read_picture_semantics_resource(&resource).unwrap();
        reread.picture_resource_selection = Some(
            model
                .records
                .iter()
                .map(|row| row.binding.clone())
                .collect(),
        );
        reread.picture_semantics = Some(model);
        resolve_common_picture_transparency(&mut reread, &metadata, true).unwrap();
        bind_picture_semantics(&mut reread, uuid, true).unwrap();
        assert_eq!(
            reread.picture_semantics.as_ref().unwrap(),
            &expected,
            "carrier {relative}"
        );
        evidence.push(serde_json::json!({"path":relative,"native_sha256":format!("{:x}",Sha256::digest(&bytes)),"resource_sha256":format!("{:x}",Sha256::digest(&resource)),"all_reference_records":expected.records.len(),"per_use_pixels":pixel_count}));
    }
    assert_eq!(pixels, 248);
    let evidence = serde_json::json!({"scope":"all genuine 102 native UH per-use pixel carrier forms; typed EDT descriptor plus adapter resource regenerated then independently parsed; full reference/flag/pixel/binding model exact, not whole configuration SDK acceptance","forms":evidence,"total_pixels":pixels,"typed_common_picture_metadata":metadata.len(),"elapsed_seconds":start.elapsed().as_secs_f64()});
    std::fs::write(
        r"F:\ibcmd\lab\07\picture-all-carrier-census-r1.json",
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
}
