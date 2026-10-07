use formats_xml::form::{FormDialect, read_form, write_form};
use ibcmd_edt::{
    ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
};
use ibcmd_xml::source_tree::{SourceEntry, SourceTree};
use morph1c_core::ir::{
    FormBody, MetadataObject, NamedFormBody, ObjectKind, PropertyValue, Token, Uuid,
};
use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
use sha2::{Digest, Sha256};
const FORM: &str = "CommonForms/ChoiceValues/Ext/Form.xml";
const EDT_FORM: &str = "src/CommonForms/ChoiceValues/Form.form";
const RESOURCE: &str = "src/CommonForms/ChoiceValues/ibcmd-picture-semantics.v1.json";
fn read(dialect: FormDialect, bytes: &[u8], minor: u16) -> FormBody {
    with_source_version(Some(FormatVersion::new(2, minor)), || {
        read_form(dialect, bytes)
    })
    .unwrap()
}
fn write(dialect: FormDialect, body: &FormBody, minor: u16) -> Vec<u8> {
    with_roundtrip_target(FormatVersion::new(2, minor), || write_form(dialect, body)).unwrap()
}
fn fixture(minor: u16) -> FormBody {
    let xml=r#"<?xml version="1.0" encoding="UTF-8"?>
<form:Form xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:form="http://g5.1c.ru/v8/dt/form" xmlns:core="http://g5.1c.ru/v8/dt/mcore"><items xsi:type="form:FormField"><name>Input</name><id>1</id><type>InputField</type><extInfo xsi:type="form:InputFieldExtInfo"><choiceParameters><name>Filter.Test</name><value xsi:type="form:FormChoiceListDesTimeValue"><presentation><key>ru</key><value>Current root</value></presentation><presentation><key>en</key><value>Current root EN</value></presentation><value xsi:type="core:FixedArrayValue"><values xsi:type="form:FormChoiceListDesTimeValue"><presentation><key>ru</key><value>First</value></presentation><value xsi:type="core:NumberValue"><value>13</value></value><picture xsi:type="core:PictureRef"><picture>StdPicture.Warning</picture></picture></values><values xsi:type="form:FormChoiceListDesTimeValue"><presentation><key>en</key><value>Second</value></presentation><value xsi:type="core:NumberValue"><value>17</value></value><picture xsi:type="core:PictureRef"><picture>StdPicture.Information</picture></picture></values></value><picture xsi:type="core:PictureRef"><picture>StdPicture.Question</picture></picture></value></choiceParameters></extInfo></items></form:Form>
"#.replace('\n',"\r\n");
    read(FormDialect::Edt, xml.as_bytes(), minor)
}
fn entry<'a>(tree: &'a SourceTree, path: &str) -> &'a [u8] {
    tree.entries()
        .iter()
        .find(|e| e.path().as_str() == path)
        .unwrap()
        .bytes()
}
fn options(minor: u16) -> ConversionOptions {
    ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: format!("2.{minor}"),
        runtime_version: Some(if minor == 20 { "8.3.27" } else { "8.5.1" }.into()),
    }
}
fn source(minor: u16) -> SourceTree {
    source_body(minor, fixture(minor))
}
fn source_body(minor: u16, body: FormBody) -> SourceTree {
    let fixture_path = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/subsystem-ci/src"
    ));
    let mut cfg = read_config(Format::Designer, fixture_path, &ConvertOptions::default())
        .unwrap()
        .0;
    let mut obj = MetadataObject::new(
        ObjectKind::new("CommonForm"),
        "ChoiceValues",
        Uuid([77; 16]),
    );
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
        name: "ChoiceValues".into(),
        body,
        ordinary_body: None,
        module: None,
        help: vec![],
        help_resources: vec![],
    });
    cfg.objects.push(obj);
    let dir = tempfile::tempdir().unwrap();
    with_roundtrip_target(FormatVersion::new(2, minor), || {
        write_config(Format::Designer, &cfg, dir.path())
    })
    .unwrap();
    let root = dir.path().join("Configuration.xml");
    let bytes = String::from_utf8(std::fs::read(&root).unwrap())
        .unwrap()
        .replace(
            "</Language>",
            "</Language>\r\n\t\t\t<CommonForm>ChoiceValues</CommonForm>",
        );
    std::fs::write(root, bytes).unwrap();
    let tree = read_xml_source(dir.path(), ReaderLimits::default()).unwrap();
    let text = String::from_utf8(entry(&tree, FORM).to_vec()).unwrap();
    if !text.contains("<xr:Ref>StdPicture.Warning</xr:Ref>") {
        return tree;
    }
    // Use scoped reference container edit, independent of serializer indentation.
    let mut text = text;
    let begin = text.find("<xr:Ref>StdPicture.Warning</xr:Ref>").unwrap();
    let end = begin + text[begin..].find("</Picture>").unwrap();
    let changed = text[begin..end].replace(
        "<xr:LoadTransparent>true</xr:LoadTransparent>",
        "<xr:LoadTransparent>false</xr:LoadTransparent>",
    );
    text.replace_range(begin..end, &changed);
    SourceTree::new(
        tree.entries()
            .iter()
            .map(|e| {
                SourceEntry::from_bytes(
                    e.path().clone(),
                    if e.path().as_str() == FORM {
                        text.clone().into_bytes()
                    } else {
                        e.bytes().to_vec()
                    },
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
#[test]
fn presentation_reference_and_each_array_wrapper_are_current_semantic_data() {
    for minor in [20, 21] {
        let body = fixture(minor);
        let native = write(FormDialect::Designer, &body, minor);
        let returned = read(FormDialect::Designer, &native, minor);
        assert_eq!(
            serde_json::to_vec(&body).unwrap(),
            serde_json::to_vec(&returned).unwrap()
        );
        assert_eq!(write(FormDialect::Designer, &returned, minor), native);
        let edt = write(FormDialect::Edt, &returned, minor);
        assert_eq!(
            serde_json::to_vec(&read(FormDialect::Edt, &edt, minor)).unwrap(),
            serde_json::to_vec(&body).unwrap()
        );
        let changed = String::from_utf8(native)
            .unwrap()
            .replace(">First<", ">Edited<")
            .replace("StdPicture.Warning", "StdPicture.CurrentSymbol")
            .replace(">13<", ">23<");
        let current = read(FormDialect::Designer, changed.as_bytes(), minor);
        assert_ne!(
            serde_json::to_vec(&current).unwrap(),
            serde_json::to_vec(&body).unwrap()
        );
        let edt = String::from_utf8(write(FormDialect::Edt, &current, minor)).unwrap();
        assert!(
            edt.contains(">Edited<")
                && edt.contains("StdPicture.CurrentSymbol")
                && edt.contains(">23<")
        );
    }
}
#[test]
fn wrapper_unknown_duplicate_value_and_picture_namespaces_fail_closed() {
    let native = String::from_utf8(write(FormDialect::Designer, &fixture(21), 21)).unwrap();
    for altered in [
        native.replace("<Picture>", "<Picture unknown='true'>"),
        native.replace("</Picture>", "</Picture><Picture/>"),
        native.replace("<Presentation>", "<Presentation unknown='true'>"),
        native
            .replace("<xr:Ref>", "<wrong:Ref xmlns:wrong='urn:unknown'>")
            .replace("</xr:Ref>", "</wrong:Ref>"),
        native.replace(
            "<Value xsi:type=\"xs:decimal\">13</Value>",
            "<Value xsi:type=\"xs:decimal\">13</Value><Value xsi:nil=\"true\"/>",
        ),
    ] {
        assert_ne!(altered, native);
        assert!(
            with_source_version(Some(FormatVersion::new(2, 21)), || read_form(
                FormDialect::Designer,
                altered.as_bytes()
            ))
            .is_err()
        );
    }
}
#[test]
fn public_parameter_picture_carrier_survives_strip_and_rejects_stale_or_forged_edits() {
    for minor in [20, 21] {
        let original = source(minor);
        let generated = xml_to_edt(&original, &options(minor)).unwrap().tree;
        assert!(!entry(&generated, RESOURCE).is_empty());
        assert_eq!(
            edt_to_xml(
                &Project::from_tree(generated.clone()).unwrap(),
                &options(minor)
            )
            .unwrap()
            .tree,
            original
        );
        let stripped = SourceTree::new(
            generated
                .entries()
                .iter()
                .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
                .cloned()
                .collect(),
        )
        .unwrap();
        let returned = edt_to_xml(&Project::from_tree(stripped).unwrap(), &options(minor))
            .unwrap()
            .tree;
        let original_body = read(FormDialect::Designer, entry(&original, FORM), minor);
        let current_body = read(FormDialect::Designer, entry(&returned, FORM), minor);
        assert_eq!(
            serde_json::to_vec(&current_body).unwrap(),
            serde_json::to_vec(&original_body).unwrap()
        );
        let mut resource: serde_json::Value =
            serde_json::from_slice(entry(&generated, RESOURCE)).unwrap();
        assert_eq!(resource["form"]["records"].as_array().unwrap().len(), 1);
        resource["form"]["records"][0]["load_transparent"] = true.into();
        let edited = serde_json::to_vec(&resource).unwrap();
        let mut manifest: serde_json::Value =
            serde_json::from_slice(entry(&generated, ".ibcmd-provenance/manifest.json")).unwrap();
        manifest["generated"][RESOURCE] = format!("{:x}", Sha256::digest(&edited)).into();
        let alter = |strip: bool, path: &str, bytes: &[u8]| {
            SourceTree::new(
                generated
                    .entries()
                    .iter()
                    .filter(|e| !strip || !e.path().as_str().starts_with(".ibcmd-provenance/"))
                    .map(|e| {
                        SourceEntry::from_bytes(
                            e.path().clone(),
                            if e.path().as_str() == path {
                                bytes.to_vec()
                            } else if e.path().as_str() == ".ibcmd-provenance/manifest.json" {
                                serde_json::to_vec(&manifest).unwrap()
                            } else {
                                e.bytes().to_vec()
                            },
                        )
                        .unwrap()
                    })
                    .collect(),
            )
            .unwrap()
        };
        assert!(
            edt_to_xml(
                &Project::from_tree(alter(false, RESOURCE, &edited)).unwrap(),
                &options(minor)
            )
            .is_err()
        );
        let edited_result = edt_to_xml(
            &Project::from_tree(alter(true, RESOURCE, &edited)).unwrap(),
            &options(minor),
        )
        .unwrap();
        assert_ne!(entry(&edited_result.tree, FORM), entry(&original, FORM));
        for altered in [
            String::from_utf8(entry(&generated, EDT_FORM).to_vec())
                .unwrap()
                .replace("StdPicture.Warning", "StdPicture.CurrentRef"),
            String::from_utf8(entry(&generated, EDT_FORM).to_vec())
                .unwrap()
                .replace(">13<", ">29<"),
            String::from_utf8(entry(&generated, EDT_FORM).to_vec())
                .unwrap()
                .replace(">First<", ">CurrentName<"),
        ] {
            assert!(
                edt_to_xml(
                    &Project::from_tree(alter(true, EDT_FORM, altered.as_bytes())).unwrap(),
                    &options(minor)
                )
                .is_err()
            );
        }
    }
}

fn asset_source(minor: u16) -> SourceTree {
    let original = source(minor);
    let text = String::from_utf8(entry(&original, FORM).to_vec())
        .unwrap()
        .replace(
            "<xr:Ref>StdPicture.Question</xr:Ref>",
            "<xr:Abs>nested-current.png</xr:Abs>",
        );
    let mut entries: Vec<_> = original
        .entries()
        .iter()
        .map(|e| {
            SourceEntry::from_bytes(
                e.path().clone(),
                if e.path().as_str() == FORM {
                    text.as_bytes().to_vec()
                } else {
                    e.bytes().to_vec()
                },
            )
            .unwrap()
        })
        .collect();
    entries.push(
        SourceEntry::from_bytes(
            ibcmd_xml::source_tree::SourcePath::new(
                "CommonForms/ChoiceValues/Ext/nested-current.png",
            )
            .unwrap(),
            b"CURRENT IMAGE BYTES".to_vec(),
        )
        .unwrap(),
    );
    SourceTree::new(entries).unwrap()
}
fn altered_resource(
    generated: &SourceTree,
    value: &serde_json::Value,
    strip: bool,
    forge: bool,
) -> SourceTree {
    let bytes = serde_json::to_vec(value).unwrap();
    let mut manifest: serde_json::Value =
        serde_json::from_slice(entry(generated, ".ibcmd-provenance/manifest.json")).unwrap();
    if forge {
        manifest["generated"][RESOURCE] = format!("{:x}", Sha256::digest(&bytes)).into();
    }
    SourceTree::new(
        generated
            .entries()
            .iter()
            .filter(|e| !strip || !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .map(|e| {
                SourceEntry::from_bytes(
                    e.path().clone(),
                    if e.path().as_str() == RESOURCE {
                        bytes.clone()
                    } else if e.path().as_str() == ".ibcmd-provenance/manifest.json" {
                        serde_json::to_vec(&manifest).unwrap()
                    } else {
                        e.bytes().to_vec()
                    },
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
#[test]
fn declared_absolute_choice_asset_survives_both_profiles_strip_and_current_byte_edit() {
    for minor in [20, 21] {
        let original = asset_source(minor);
        let converted = xml_to_edt(&original, &options(minor)).unwrap();
        assert!(
            converted
                .extensions
                .iter()
                .any(|e| e.id == "ibcmd-picture-semantics/1" && e.references == 2)
        );
        let generated = converted.tree;
        assert_eq!(
            edt_to_xml(
                &Project::from_tree(generated.clone()).unwrap(),
                &options(minor)
            )
            .unwrap()
            .tree,
            original
        );
        let resource: serde_json::Value =
            serde_json::from_slice(entry(&generated, RESOURCE)).unwrap();
        assert_eq!(resource["form"]["assets"].as_array().unwrap().len(), 1);
        let raw = String::from_utf8(entry(&generated, RESOURCE).to_vec()).unwrap();
        let duplicate = raw.replacen(
            "\"path\": \"nested-current.png\"",
            "\"path\": \"nested-current.png\", \"path\": \"nested-current.png\"",
            1,
        );
        assert_ne!(duplicate, raw);
        assert!(formats_xml::form::read_picture_semantics_resource(duplicate.as_bytes()).is_err());
        let missing_asset = SourceTree::new(
            original
                .entries()
                .iter()
                .filter(|e| !e.path().as_str().ends_with("nested-current.png"))
                .cloned()
                .collect(),
        )
        .unwrap();
        assert!(xml_to_edt(&missing_asset, &options(minor)).is_err());

        assert_eq!(
            edt_to_xml(
                &Project::from_tree(altered_resource(&generated, &resource, true, false)).unwrap(),
                &options(minor)
            )
            .unwrap()
            .tree,
            original
        );
        let mut edited = resource.clone();
        let current = b"EDITED CURRENT IMAGE";
        edited["form"]["assets"][0]["bytes"] = serde_json::to_value(current.to_vec()).unwrap();
        edited["asset_checks"][0]["length"] = (current.len() as u64).into();
        edited["asset_checks"][0]["sha256"] = format!("{:x}", Sha256::digest(current)).into();
        let result = edt_to_xml(
            &Project::from_tree(altered_resource(&generated, &edited, true, false)).unwrap(),
            &options(minor),
        )
        .unwrap();
        assert_eq!(
            entry(
                &result.tree,
                "CommonForms/ChoiceValues/Ext/nested-current.png"
            ),
            current
        );
        assert!(
            edt_to_xml(
                &Project::from_tree(altered_resource(&generated, &edited, false, true)).unwrap(),
                &options(minor)
            )
            .is_err()
        );
        let mut bad = resource.clone();
        bad["asset_checks"][0]["sha256"] = "0".repeat(64).into();
        assert!(
            edt_to_xml(
                &Project::from_tree(altered_resource(&generated, &bad, true, false)).unwrap(),
                &options(minor)
            )
            .is_err()
        );
        for path in [
            "../outside.png",
            "/absolute.png",
            "bad\\name.png",
            "Form.xml",
        ] {
            let mut bad = resource.clone();
            bad["form"]["assets"][0]["path"] = path.into();
            assert!(
                edt_to_xml(
                    &Project::from_tree(altered_resource(&generated, &bad, true, false)).unwrap(),
                    &options(minor)
                )
                .is_err()
            );
        }
        for changed in [
            {
                let mut x = resource.clone();
                x["form"]["assets"][0]["unknown"] = true.into();
                x
            },
            {
                let mut x = resource.clone();
                let row = x["form"]["assets"][0].clone();
                x["form"]["assets"].as_array_mut().unwrap().push(row);
                x
            },
            {
                let mut x = resource.clone();
                x["form"]["records"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|row| !row["reference"].as_str().unwrap().starts_with("abs-file:"));
                x
            },
        ] {
            assert!(
                edt_to_xml(
                    &Project::from_tree(altered_resource(&generated, &changed, true, false))
                        .unwrap(),
                    &options(minor)
                )
                .is_err()
            );
        }
        let missing = SourceTree::new(
            generated
                .entries()
                .iter()
                .filter(|e| {
                    e.path().as_str() != RESOURCE
                        && !e.path().as_str().starts_with(".ibcmd-provenance/")
                })
                .cloned()
                .collect(),
        )
        .unwrap();
        assert!(edt_to_xml(&Project::from_tree(missing).unwrap(), &options(minor)).is_err());
    }
}
#[test]
fn directory_asset_resource_is_consumed_once_and_declared_native_path_is_restored() {
    for minor in [20, 21] {
        let original = asset_source(minor);
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("source");
        std::fs::create_dir(&root).unwrap();
        for e in original.entries() {
            let path = root.join(e.path().as_str());
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, e.bytes()).unwrap();
        }
        let conversion = ibcmd_edt::read_directory_source(&root)
            .unwrap()
            .xml_to_edt(&options(minor))
            .unwrap();
        assert!(
            conversion
                .extensions
                .iter()
                .any(|e| e.id == "ibcmd-picture-semantics/1"
                    && e.resources == 1
                    && e.references == 2)
        );
        let project = tmp.path().join("project");
        conversion.publish_new(&project).unwrap();
        let provenance = project.join(".ibcmd-provenance");
        assert!(provenance.starts_with(tmp.path()));
        std::fs::remove_dir_all(provenance).unwrap();
        let conversion = ibcmd_edt::read_directory_project(&project)
            .unwrap()
            .edt_to_xml(&options(minor))
            .unwrap();
        assert_eq!(conversion.accounting.iter().filter(|r|r.path==RESOURCE && r.disposition==ibcmd_edt::Disposition::Converted).count(),1);
        let target = tmp.path().join("target");
        conversion.publish_new(&target).unwrap();
        assert_eq!(
            read_xml_source(&target, ReaderLimits::default()).unwrap(),
            original
        );
    }
}

fn chart_ref_body() -> FormBody {
    let mut body=read(FormDialect::Edt,&String::from(r#"<?xml version="1.0" encoding="UTF-8"?>
<form:Form xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:form="http://g5.1c.ru/v8/dt/form"><attributes><name>Diagram</name><id>1</id><valueType><types>GanttChart</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type="form:GanttChartExtInfo"/></attributes></form:Form>"#).replace("\n","\r\n").into_bytes(),21);
    let chart=formats_xml::form::read_chart_sidecar(&String::from(r#"<?xml version="1.0" encoding="UTF-8"?>
<ganttchart:GanttChart xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:core="http://g5.1c.ru/v8/dt/mcore" xmlns:ganttchart="http://g5.1c.ru/v8/dt/ganttchart/model"><points><value><picture xsi:type="core:PictureRef"><picture>StdPicture.Save</picture></picture></value></points></ganttchart:GanttChart>"#).replace("\n","\r\n").into_bytes()).unwrap();
    body.data_attributes[0].chart_settings = Some(chart);
    body
}
fn chart_picture(body: &mut FormBody) -> &mut morph1c_core::ir::form::ChartPicture {
    use morph1c_core::ir::form::ChartValue;
    let chart = body.data_attributes[0].chart_settings.as_mut().unwrap();
    let ChartValue::Nested(points) = &mut chart
        .fields
        .iter_mut()
        .find(|(n, _)| n == "points")
        .unwrap()
        .1
    else {
        panic!()
    };
    let ChartValue::Nested(value) = &mut points.iter_mut().find(|(n, _)| n == "value").unwrap().1
    else {
        panic!()
    };
    let ChartValue::Picture(picture) =
        &mut value.iter_mut().find(|(n, _)| n == "picture").unwrap().1
    else {
        panic!()
    };
    picture
}
#[test]
fn chart_reference_uses_current_metadata_default_and_two_phase_picture_resource_binding() {
    use formats_xml::form::{
        bind_picture_semantics, project_picture_semantics, read_picture_semantics_resource,
        resolve_common_picture_transparency,
    };
    use morph1c_core::ir::form::ChartPicture;
    let id = Uuid([77; 16]);
    let mut body = chart_ref_body();
    *chart_picture(&mut body) = ChartPicture::Reference {
        reference: "CommonPicture.Current".into(),
        load_transparent: false,
    };
    let defaults = std::collections::BTreeMap::from([("CommonPicture.Current".into(), true)]);
    resolve_common_picture_transparency(&mut body, &defaults, false).unwrap();
    bind_picture_semantics(&mut body, id, false).unwrap();
    let (mut projected, resource) = project_picture_semantics(&body, id).unwrap();
    let resource = resource.unwrap();
    assert!(matches!(
        chart_picture(&mut projected),
        ChartPicture::Reference {
            load_transparent: true,
            ..
        }
    ));
    let model = read_picture_semantics_resource(&resource).unwrap();
    projected.picture_resource_selection = Some(
        model
            .records
            .iter()
            .map(|row| row.binding.clone())
            .collect(),
    );
    projected.picture_semantics = Some(model);
    bind_picture_semantics(&mut projected, id, true).unwrap();
    assert_eq!(
        serde_json::to_vec(&projected).unwrap(),
        serde_json::to_vec(&body).unwrap()
    );
    let mut current = chart_ref_body();
    *chart_picture(&mut current) = ChartPicture::Reference {
        reference: "CommonPicture.Current".into(),
        load_transparent: false,
    };
    resolve_common_picture_transparency(&mut current, &defaults, true).unwrap();
    assert!(matches!(
        chart_picture(&mut current),
        ChartPicture::Reference {
            load_transparent: true,
            ..
        }
    ));
    resolve_common_picture_transparency(
        &mut current,
        &std::collections::BTreeMap::from([("CommonPicture.Current".into(), false)]),
        true,
    )
    .unwrap();
    assert!(matches!(
        chart_picture(&mut current),
        ChartPicture::Reference {
            load_transparent: false,
            ..
        }
    ));
    assert!(
        resolve_common_picture_transparency(&mut current, &std::collections::BTreeMap::new(), true)
            .is_err()
    );
    for mutate in ["ref", "pixel", "path", "attribute"] {
        let mut projected = project_picture_semantics(&body, id).unwrap().0;
        let mut model = read_picture_semantics_resource(&resource).unwrap();
        match mutate {
            "ref" => {
                *chart_picture(&mut projected) = ChartPicture::Reference {
                    reference: "StdPicture.Current".into(),
                    load_transparent: true,
                }
            }
            "pixel" => {
                model.records[0].pixel =
                    Some(morph1c_core::ir::form::PictureSemanticPixel { x: 1, y: 2 })
            }
            "path" => {
                if let morph1c_core::ir::form::PictureSemanticBinding::Chart { path, .. } =
                    &mut model.records[0].binding
                {
                    path.push(morph1c_core::ir::form::PictureChartStep::Field(
                        "unknown".into(),
                    ));
                }
            }
            "attribute" => projected.data_attributes[0].id = 2,
            _ => unreachable!(),
        }
        projected.picture_resource_selection =
            Some(model.records.iter().map(|r| r.binding.clone()).collect());
        projected.picture_semantics = Some(model);
        let before = serde_json::to_vec(&projected).unwrap();
        assert!(bind_picture_semantics(&mut projected, id, true).is_err());
        assert_eq!(before, serde_json::to_vec(&projected).unwrap());
    }
}
#[test]
fn public_gantt_reference_carrier_preserves_current_bool_without_provenance() {
    use morph1c_core::ir::form::ChartPicture;
    for minor in [20, 21] {
        let mut body = chart_ref_body();
        *chart_picture(&mut body) = ChartPicture::Reference {
            reference: "StdPicture.Save".into(),
            load_transparent: false,
        };
        let original = source_body(minor, body);
        let generated = xml_to_edt(&original, &options(minor)).unwrap().tree;
        let resource: serde_json::Value =
            serde_json::from_slice(entry(&generated, RESOURCE)).unwrap();
        assert_eq!(resource["form"]["records"][0]["binding"]["kind"], "Chart");
        let returned = edt_to_xml(
            &Project::from_tree(altered_resource(&generated, &resource, true, false)).unwrap(),
            &options(minor),
        )
        .unwrap()
        .tree;
        assert_eq!(original, returned);
    }
}

#[test]
fn current_definition_glyph_is_visible_in_edt_and_owned_native_carrier_in_both_profiles() {
    for minor in [20, 21] {
        let original = asset_source(minor);
        let generated = xml_to_edt(&original, &options(minor)).unwrap().tree;
        let descriptor = String::from_utf8(entry(&generated, EDT_FORM).to_vec()).unwrap();
        let changed = descriptor.replace(
            "<picture xsi:type=\"form:FormPicture\"/>",
            "<picture xsi:type=\"form:FormPicture\"><glyph><x>2</x><y>4</y></glyph></picture>",
        );
        assert_ne!(changed, descriptor);
        let altered = SourceTree::new(
            generated
                .entries()
                .iter()
                .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
                .map(|e| {
                    SourceEntry::from_bytes(
                        e.path().clone(),
                        if e.path().as_str() == EDT_FORM {
                            changed.as_bytes().to_vec()
                        } else {
                            e.bytes().to_vec()
                        },
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        let native = edt_to_xml(
            &Project::from_tree(altered.clone()).unwrap(),
            &options(minor),
        )
        .unwrap()
        .tree;
        let path = "CommonForms/ChoiceValues/Ext/ibcmd-picture-semantics.v1.json";
        let resource: serde_json::Value = serde_json::from_slice(entry(&native, path)).unwrap();
        assert_eq!(resource["definition_glyphs"][0]["glyph"]["x"], 2);
        assert_eq!(entry(&native, FORM), entry(&original, FORM));
        let strip = SourceTree::new(
            native
                .entries()
                .iter()
                .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
                .cloned()
                .collect(),
        )
        .unwrap();
        let returned = xml_to_edt(&strip, &options(minor)).unwrap().tree;
        assert!(
            String::from_utf8(entry(&returned, EDT_FORM).to_vec())
                .unwrap()
                .contains("<x>2</x>")
        );
        let mut edited = resource.clone();
        edited["definition_glyphs"][0]["glyph"]["x"] = 3.into();
        let edit_bytes = serde_json::to_vec(&edited).unwrap();
        let modified = SourceTree::new(
            strip
                .entries()
                .iter()
                .map(|e| {
                    SourceEntry::from_bytes(
                        e.path().clone(),
                        if e.path().as_str() == path {
                            edit_bytes.clone()
                        } else {
                            e.bytes().to_vec()
                        },
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        assert!(
            String::from_utf8(
                entry(
                    &xml_to_edt(&modified, &options(minor)).unwrap().tree,
                    EDT_FORM
                )
                .to_vec()
            )
            .unwrap()
            .contains("<x>3</x>")
        );
        // Native outputs carry no provenance manifest. Forge the actual generated
        // EDT project, whose retained native source must not hide a current glyph edit.
        let descriptor = String::from_utf8(entry(&returned, EDT_FORM).to_vec()).unwrap();
        let edited_descriptor = descriptor.replace("<x>2</x>", "<x>3</x>");
        assert_ne!(descriptor, edited_descriptor);
        let mut manifest: serde_json::Value =
            serde_json::from_slice(entry(&returned, ".ibcmd-provenance/manifest.json")).unwrap();
        manifest["generated"][EDT_FORM] =
            format!("{:x}", Sha256::digest(edited_descriptor.as_bytes())).into();
        let forged = SourceTree::new(
            returned
                .entries()
                .iter()
                .map(|e| {
                    SourceEntry::from_bytes(
                        e.path().clone(),
                        if e.path().as_str() == EDT_FORM {
                            edited_descriptor.as_bytes().to_vec()
                        } else if e.path().as_str() == ".ibcmd-provenance/manifest.json" {
                            serde_json::to_vec(&manifest).unwrap()
                        } else {
                            e.bytes().to_vec()
                        },
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        assert!(edt_to_xml(&Project::from_tree(forged).unwrap(), &options(minor)).is_err());
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("project");
        std::fs::create_dir(&project).unwrap();
        for e in altered.entries() {
            let p = project.join(e.path().as_str());
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, e.bytes()).unwrap();
        }
        let conversion = ibcmd_edt::read_directory_project(&project)
            .unwrap()
            .edt_to_xml(&options(minor))
            .unwrap();
        let native_dir = tmp.path().join("native");
        conversion.publish_new(&native_dir).unwrap();
        assert!(!native_dir.join(".ibcmd-provenance").exists());
        let conversion = ibcmd_edt::read_directory_source(&native_dir)
            .unwrap()
            .xml_to_edt(&options(minor))
            .unwrap();
        let final_dir = tmp.path().join("final");
        conversion.publish_new(&final_dir).unwrap();
        assert!(
            std::fs::read_to_string(final_dir.join(EDT_FORM))
                .unwrap()
                .contains("<x>2</x>")
        );
    }
}

#[test]
fn chart_definition_current_bytes_point_and_glyph_survive_public_and_directory_both_profiles() {
    use morph1c_core::ir::form::ChartPicture;
    for minor in [20, 21] {
        let mut body = chart_ref_body();
        *chart_picture(&mut body) = ChartPicture::Definition {
            file_name: "current/chart-picture.bin".into(),
            bytes: vec![13, 17, 21, 0, 255],
            transparent_pixel: Some((13, 3)),
            glyph: Some((2, 4)),
        };
        let original = source_body(minor, body);
        assert_eq!(
            entry(
                &original,
                "CommonForms/ChoiceValues/Ext/current/chart-picture.bin"
            ),
            [13, 17, 21, 0, 255]
        );
        let native_resource = "CommonForms/ChoiceValues/Ext/ibcmd-picture-semantics.v1.json";
        let glyph: serde_json::Value =
            serde_json::from_slice(entry(&original, native_resource)).unwrap();
        assert_eq!(glyph["definition_glyphs"][0]["glyph"]["x"], 2);
        let generated = xml_to_edt(&original, &options(minor)).unwrap().tree;
        let resource: serde_json::Value =
            serde_json::from_slice(entry(&generated, RESOURCE)).unwrap();
        assert_eq!(
            resource["form"]["assets"][0]["path"],
            "current/chart-picture.bin"
        );
        let sidecar = "src/CommonForms/ChoiceValues/Attributes/Diagram/ExtInfo/GanttChart.chart";
        let old = String::from_utf8(entry(&generated, sidecar).to_vec()).unwrap();
        let updated = old
            .replace("<x>13</x>", "<x>14</x>")
            .replace("<x>2</x>", "<x>3</x>");
        assert_ne!(old, updated);
        let mut edited_point = resource.clone();
        edited_point["form"]["records"][0]["pixel"]["x"] = 14.into();
        let edit_project = |retain: bool| {
            let mut manifest: serde_json::Value =
                serde_json::from_slice(entry(&generated, ".ibcmd-provenance/manifest.json"))
                    .unwrap();
            let resource_bytes = serde_json::to_vec(&edited_point).unwrap();
            manifest["generated"][RESOURCE] =
                format!("{:x}", Sha256::digest(&resource_bytes)).into();
            manifest["generated"][sidecar] =
                format!("{:x}", Sha256::digest(updated.as_bytes())).into();
            SourceTree::new(
                generated
                    .entries()
                    .iter()
                    .filter(|e| retain || !e.path().as_str().starts_with(".ibcmd-provenance/"))
                    .map(|e| {
                        SourceEntry::from_bytes(
                            e.path().clone(),
                            if e.path().as_str() == RESOURCE {
                                resource_bytes.clone()
                            } else if e.path().as_str() == sidecar {
                                updated.as_bytes().to_vec()
                            } else if e.path().as_str() == ".ibcmd-provenance/manifest.json" {
                                serde_json::to_vec(&manifest).unwrap()
                            } else {
                                e.bytes().to_vec()
                            },
                        )
                        .unwrap()
                    })
                    .collect(),
            )
            .unwrap()
        };
        let edited_native = edt_to_xml(
            &Project::from_tree(edit_project(false)).unwrap(),
            &options(minor),
        )
        .unwrap()
        .tree;
        let native_text = String::from_utf8(entry(&edited_native, FORM).to_vec()).unwrap();
        assert!(native_text.contains("x=\"14\""));
        let edited_glyph: serde_json::Value =
            serde_json::from_slice(entry(&edited_native, native_resource)).unwrap();
        assert_eq!(edited_glyph["definition_glyphs"][0]["glyph"]["x"], 3);
        assert!(
            edt_to_xml(
                &Project::from_tree(edit_project(true)).unwrap(),
                &options(minor)
            )
            .is_err()
        );
        let begin = old.find("<picture ").unwrap();
        let end = begin + old[begin..].find("</picture>").unwrap() + "</picture>".len();
        let mut deleted = old.clone();
        deleted.replace_range(begin..end, "");
        let missing = SourceTree::new(
            generated
                .entries()
                .iter()
                .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
                .map(|e| {
                    SourceEntry::from_bytes(
                        e.path().clone(),
                        if e.path().as_str() == sidecar {
                            deleted.as_bytes().to_vec()
                        } else {
                            e.bytes().to_vec()
                        },
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        assert!(edt_to_xml(&Project::from_tree(missing).unwrap(), &options(minor)).is_err());
        let missing_asset = SourceTree::new(
            original
                .entries()
                .iter()
                .filter(|e| {
                    e.path().as_str() != "CommonForms/ChoiceValues/Ext/current/chart-picture.bin"
                })
                .cloned()
                .collect(),
        )
        .unwrap();
        assert!(xml_to_edt(&missing_asset, &options(minor)).is_err());
        let stripped = altered_resource(&generated, &resource, true, false);
        let returned = edt_to_xml(
            &Project::from_tree(stripped.clone()).unwrap(),
            &options(minor),
        )
        .unwrap()
        .tree;
        assert_eq!(returned, original);
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("project");
        std::fs::create_dir(&project).unwrap();
        for e in stripped.entries() {
            let p = project.join(e.path().as_str());
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, e.bytes()).unwrap();
        }
        let native = ibcmd_edt::read_directory_project(&project)
            .unwrap()
            .edt_to_xml(&options(minor))
            .unwrap();
        let output = tmp.path().join("native");
        native.publish_new(&output).unwrap();
        assert_eq!(
            read_xml_source(&output, ReaderLimits::default()).unwrap(),
            original
        );
        let mut changed = resource.clone();
        let bytes = vec![42u8, 19, 0, 255];
        changed["form"]["assets"][0]["bytes"] = serde_json::to_value(&bytes).unwrap();
        changed["asset_checks"][0]["length"] = bytes.len().into();
        changed["asset_checks"][0]["sha256"] = format!("{:x}", Sha256::digest(&bytes)).into();
        let modified = edt_to_xml(
            &Project::from_tree(altered_resource(&generated, &changed, true, false)).unwrap(),
            &options(minor),
        )
        .unwrap()
        .tree;
        assert_eq!(
            entry(
                &modified,
                "CommonForms/ChoiceValues/Ext/current/chart-picture.bin"
            ),
            bytes
        );
        assert!(
            edt_to_xml(
                &Project::from_tree(altered_resource(&generated, &changed, false, true)).unwrap(),
                &options(minor)
            )
            .is_err()
        );
        for mutation in ["delete", "path", "unknown", "duplicate", "sha"] {
            let mut bad = resource.clone();
            match mutation {
                "delete" => bad["form"]["assets"] = serde_json::json!([]),
                "path" => {
                    bad["form"]["assets"][0]["path"] = "../escape.bin".into();
                    bad["form"]["records"][0]["reference"] = "abs-file:../escape.bin".into();
                }
                "unknown" => bad["form"]["assets"][0]["unknown"] = true.into(),
                "duplicate" => {
                    let asset = bad["form"]["assets"][0].clone();
                    bad["form"]["assets"].as_array_mut().unwrap().push(asset);
                }
                "sha" => bad["asset_checks"][0]["sha256"] = "invalid".into(),
                _ => unreachable!(),
            }
            assert!(
                edt_to_xml(
                    &Project::from_tree(altered_resource(&generated, &bad, true, false)).unwrap(),
                    &options(minor)
                )
                .is_err(),
                "{mutation}"
            );
        }
    }
}
#[test]
fn chart_definition_current_graph_edits_replace_mirrored_asset_bytes() {
    use morph1c_core::ir::form::ChartPicture;
    let mut body = chart_ref_body();
    *chart_picture(&mut body) = ChartPicture::Definition {
        file_name: String::new(),
        bytes: vec![1, 2, 3],
        transparent_pixel: None,
        glyph: None,
    };
    formats_xml::form::bind_picture_semantics(&mut body, Uuid([77; 16]), false).unwrap();
    let model = body.picture_semantics.as_ref().unwrap();
    let first = model.assets[0].path.clone();
    assert!(first.starts_with("ibcmd-picture-assets/"));
    if let ChartPicture::Definition {
        bytes,
        transparent_pixel,
        glyph,
        ..
    } = chart_picture(&mut body)
    {
        *bytes = vec![9, 8];
        *transparent_pixel = Some((-1, -1));
        *glyph = Some((4, 5));
    }
    formats_xml::form::bind_picture_semantics(&mut body, Uuid([77; 16]), false).unwrap();
    assert_eq!(
        body.picture_semantics.as_ref().unwrap().assets[0].bytes,
        [9, 8]
    );
    assert_eq!(
        body.picture_semantics.as_ref().unwrap().assets[0].path,
        first
    );
    assert!(
        formats_xml::form::write_native_picture_resource(&body, Uuid([77; 16]))
            .unwrap()
            .is_some()
    );
    let projection = formats_xml::form::project_native_picture_glyphs(&body)
        .unwrap()
        .unwrap();
    let mut projection = projection;
    assert!(matches!(
        chart_picture(&mut projection),
        ChartPicture::Definition { glyph: None, .. }
    ));
    if let ChartPicture::Definition {
        glyph,
        transparent_pixel,
        ..
    } = chart_picture(&mut body)
    {
        *glyph = None;
        *transparent_pixel = None;
    }
    assert!(
        formats_xml::form::write_native_picture_resource(&body, Uuid([77; 16]))
            .unwrap()
            .is_none()
    );
}

#[test]
fn sdk_omitted_definition_points_use_current_carrier_and_source_presence_both_profiles() {
    use morph1c_core::ir::form::ChartPicture;
    for minor in [20, 21] {
        for point in [(-1, 3), (13, -1), (-1, -1)] {
            let mut body = chart_ref_body();
            *chart_picture(&mut body) = ChartPicture::Definition {
                file_name: "current/partial.bin".into(),
                bytes: vec![1, 9, 3],
                transparent_pixel: Some(point),
                glyph: None,
            };
            let original = source_body(minor, body);
            let text = String::from_utf8(entry(&original, FORM).to_vec()).unwrap();
            assert!(!text.contains("<xr:TransparentPixel"));
            let native_resource = "CommonForms/ChoiceValues/Ext/ibcmd-picture-semantics.v1.json";
            if point != (-1, -1) {
                let current: serde_json::Value =
                    serde_json::from_slice(entry(&original, native_resource)).unwrap();
                assert_eq!(current["definition_points"][0]["point"]["x"], point.0);
            } else {
                assert!(
                    !original
                        .entries()
                        .iter()
                        .any(|e| e.path().as_str() == native_resource)
                );
            }
            let generated = xml_to_edt(&original, &options(minor)).unwrap().tree;
            let resource: serde_json::Value =
                serde_json::from_slice(entry(&generated, RESOURCE)).unwrap();
            let returned = edt_to_xml(
                &Project::from_tree(altered_resource(&generated, &resource, true, false)).unwrap(),
                &options(minor),
            )
            .unwrap()
            .tree;
            assert_eq!(returned, original);
            // A literal native current partial Point is a distinct source spelling.
            let explicit=text.replace("<xr:LoadTransparent>true</xr:LoadTransparent>",&format!("<xr:LoadTransparent>true</xr:LoadTransparent><xr:TransparentPixel x=\"{}\" y=\"{}\"/>",point.0,point.1));
            let native = SourceTree::new(
                original
                    .entries()
                    .iter()
                    .filter(|e| e.path().as_str() != native_resource)
                    .map(|e| {
                        SourceEntry::from_bytes(
                            e.path().clone(),
                            if e.path().as_str() == FORM {
                                explicit.as_bytes().to_vec()
                            } else {
                                e.bytes().to_vec()
                            },
                        )
                        .unwrap()
                    })
                    .collect(),
            )
            .unwrap();
            let generated = xml_to_edt(&native, &options(minor)).unwrap().tree;
            let resource: serde_json::Value =
                serde_json::from_slice(entry(&generated, RESOURCE)).unwrap();
            assert_eq!(
                resource["native_point_presence"].as_array().unwrap().len(),
                1
            );
            let returned = edt_to_xml(
                &Project::from_tree(altered_resource(&generated, &resource, true, false)).unwrap(),
                &options(minor),
            )
            .unwrap()
            .tree;
            // XML indentation is lexical; original provenance retains exact bytes.
            let returned_text = String::from_utf8(entry(&returned, FORM).to_vec()).unwrap();
            assert!(returned_text.contains("<xr:TransparentPixel"));
            assert_eq!(
                read(FormDialect::Designer, entry(&returned, FORM), minor),
                read(FormDialect::Designer, entry(&native, FORM), minor)
            );
            let exact = edt_to_xml(&Project::from_tree(generated).unwrap(), &options(minor))
                .unwrap()
                .tree;
            assert_eq!(exact, native);
        }
    }
}

#[test]
fn choice_definition_partial_points_current_resource_and_forged_provenance_both_profiles() {
    for minor in [20, 21] {
        for point in [(-1, 3), (13, -1), (-1, -1)] {
            let original = asset_source(minor);
            let generated = xml_to_edt(&original, &options(minor)).unwrap().tree;
            let mut resource: serde_json::Value =
                serde_json::from_slice(entry(&generated, RESOURCE)).unwrap();
            let row = resource["form"]["records"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|r| r["reference"].as_str().unwrap().starts_with("abs-file:"))
                .unwrap();
            row["pixel"] = serde_json::json!({"x":point.0,"y":point.1});
            row["load_transparent"] = true.into();
            let text = String::from_utf8(entry(&generated, EDT_FORM).to_vec()).unwrap();
            let edited=text.replace("<picture xsi:type=\"form:FormPicture\"/>",&format!("<picture xsi:type=\"form:FormPicture\"><transparentPixel><x>{}</x><y>{}</y></transparentPixel></picture>",point.0,point.1));
            assert_ne!(edited, text);
            let base = altered_resource(&generated, &resource, true, false);
            let input = SourceTree::new(
                base.entries()
                    .iter()
                    .map(|e| {
                        SourceEntry::from_bytes(
                            e.path().clone(),
                            if e.path().as_str() == EDT_FORM {
                                edited.as_bytes().to_vec()
                            } else {
                                e.bytes().to_vec()
                            },
                        )
                        .unwrap()
                    })
                    .collect(),
            )
            .unwrap();
            let native = edt_to_xml(&Project::from_tree(input).unwrap(), &options(minor))
                .unwrap()
                .tree;
            let body = String::from_utf8(entry(&native, FORM).to_vec()).unwrap();
            assert!(!body.contains("<xr:TransparentPixel"));
            let path = "CommonForms/ChoiceValues/Ext/ibcmd-picture-semantics.v1.json";
            let rows: serde_json::Value = serde_json::from_slice(entry(&native, path)).unwrap();
            assert_eq!(rows["definition_points"][0]["point"]["x"], point.0);
            let project = xml_to_edt(&native, &options(minor)).unwrap().tree;
            let current: serde_json::Value =
                serde_json::from_slice(entry(&project, RESOURCE)).unwrap();
            let native_again = edt_to_xml(
                &Project::from_tree(altered_resource(&project, &current, true, false)).unwrap(),
                &options(minor),
            )
            .unwrap()
            .tree;
            assert_eq!(native_again, native);
            let mut changed = current.clone();
            let row = changed["form"]["records"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|r| r["reference"].as_str().unwrap().starts_with("abs-file:"))
                .unwrap();
            row["load_transparent"] = false.into();
            assert!(
                edt_to_xml(
                    &Project::from_tree(altered_resource(&project, &changed, false, true)).unwrap(),
                    &options(minor)
                )
                .is_err()
            );
        }
    }
}
