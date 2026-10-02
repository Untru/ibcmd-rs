use formats_xml::form::{
    CHART_SEMANTICS_RESOURCE, FormDialect, apply_chart_semantics_resource,
    chart_semantics_resource_count, project_chart_semantics, read_chart_sidecar, read_form,
    write_chart_sidecar, write_form,
};
use ibcmd_edt::{
    ConversionOptions, Project, ReaderLimits, edt_to_xml, read_directory_project,
    read_directory_source, read_xml_source, xml_to_edt,
};
use ibcmd_xml::source_tree::{SourceEntry, SourceTree};
use morph1c_core::ir::{
    FormBody, MetadataObject, NamedFormBody, ObjectKind, PropertyValue, Token, Uuid,
    form::{ChartSettings, ChartValue},
};
use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
use sha2::{Digest, Sha256};

const UUID: Uuid = Uuid([63; 16]);
const FORM: &str = "CommonForms/ChartFlags/Ext/Form.xml";
const RESOURCE: &str = "CommonForms/ChartFlags/Ext/ibcmd-chart-semantics.v1.json";
const SIDECAR: &str = "src/CommonForms/ChartFlags/Attributes/Diagram/ExtInfo/Chart.chart";

fn fixture_bytes(bytes: &[u8]) -> Vec<u8> {
    String::from_utf8_lossy(bytes)
        .replace("\r\n", "\n")
        .replace("\n", "\r\n")
        .into_bytes()
}
fn body() -> FormBody {
    let chart = read_chart_sidecar(&fixture_bytes(br#"<?xml version="1.0" encoding="UTF-8"?>
<chart:Chart xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:chart="http://g5.1c.ru/v8/dt/chart/model" xmlns:core="http://g5.1c.ru/v8/dt/mcore">
<realDataItems><dataValue xsi:type="core:NumberValue"><value>37.5</value></dataValue><tooltip>first</tooltip><isToolTipFormatted>true</isToolTipFormatted></realDataItems>
<realDataItems><dataValue xsi:type="core:NumberValue"><value>42</value></dataValue><tooltip>second</tooltip></realDataItems>
<gaugeQualityBands><items><begin>1</begin><end>5</end><useTextString>true</useTextString><useToolTipString>true</useToolTipString></items></gaugeQualityBands>
</chart:Chart>"#)).unwrap();
    let mut body = read_form(FormDialect::Edt, &fixture_bytes(br#"<?xml version="1.0" encoding="UTF-8"?>
<form:Form xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:form="http://g5.1c.ru/v8/dt/form"><attributes><name>Diagram</name><id>1</id><valueType><types>Chart</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type="form:ChartExtInfo"/></attributes></form:Form>"#)).unwrap();
    body.data_attributes[0].chart_settings = Some(chart);
    body
}
fn chart(body: &mut FormBody) -> &mut ChartSettings {
    body.data_attributes[0].chart_settings.as_mut().unwrap()
}
fn rows(chart: &mut ChartSettings) -> &mut Vec<Vec<(String, ChartValue)>> {
    let ChartValue::Items(rows) = &mut chart
        .fields
        .iter_mut()
        .find(|(n, _)| n == "realDataItems")
        .unwrap()
        .1
    else {
        panic!()
    };
    rows
}
fn digest(body: &FormBody) -> Vec<u8> {
    serde_json::to_vec(body).unwrap()
}
fn entry<'a>(tree: &'a SourceTree, path: &str) -> &'a [u8] {
    tree.entries()
        .iter()
        .find(|e| e.path().as_str() == path)
        .unwrap()
        .bytes()
}
fn alter(tree: &SourceTree, path: &str, bytes: Option<&[u8]>, strip: bool) -> SourceTree {
    SourceTree::new(
        tree.entries()
            .iter()
            .filter(|e| {
                (!strip || !e.path().as_str().starts_with(".ibcmd-provenance/"))
                    && (bytes.is_some() || e.path().as_str() != path)
            })
            .map(|e| {
                SourceEntry::from_bytes(
                    e.path().clone(),
                    if e.path().as_str() == path {
                        bytes.unwrap().to_vec()
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
fn options(minor: u16) -> ConversionOptions {
    ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: format!("2.{minor}"),
        runtime_version: Some(if minor == 20 { "8.3.27" } else { "8.5.1" }.into()),
    }
}
fn original(minor: u16) -> SourceTree {
    let fixture = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/subsystem-ci/src"
    ));
    let mut cfg = read_config(Format::Designer, fixture, &ConvertOptions::default())
        .unwrap()
        .0;
    let mut object = MetadataObject::new(ObjectKind::new("CommonForm"), "ChartFlags", UUID);
    let id = morph1c_core::spec::registry::spec_for("CommonForm")
        .unwrap()
        .fields()
        .iter()
        .find(|f| f.name == "formType")
        .unwrap()
        .id;
    object
        .properties
        .push((id, PropertyValue::Enum(Token::new("Managed"))));
    object.form_bodies.push(NamedFormBody {
        name: "ChartFlags".into(),
        body: body(),
        ordinary_body: None,
        module: None,
        help: vec![],
        help_resources: vec![],
    });
    cfg.objects.push(object);
    let dir = tempfile::tempdir().unwrap();
    with_roundtrip_target(FormatVersion::new(2, minor), || {
        write_config(Format::Designer, &cfg, dir.path())
    })
    .unwrap();
    let path = dir.path().join("Configuration.xml");
    let root = String::from_utf8(std::fs::read(&path).unwrap())
        .unwrap()
        .replace(
            "</Language>",
            "</Language>\r\n\t\t\t<CommonForm>ChartFlags</CommonForm>",
        );
    std::fs::write(path, root).unwrap();
    read_xml_source(dir.path(), ReaderLimits::default()).unwrap()
}
fn decoded(tree: &SourceTree, minor: u16) -> FormBody {
    let mut body = with_source_version(Some(FormatVersion::new(2, minor)), || {
        read_form(FormDialect::Designer, entry(tree, FORM))
    })
    .unwrap();
    apply_chart_semantics_resource(&mut body, UUID, entry(tree, RESOURCE)).unwrap();
    body
}

#[test]
fn three_flags_are_semantic_current_values_and_native_projection_is_explicit() {
    for minor in [20, 21] {
        let body = body();
        assert_eq!(chart_semantics_resource_count(&body).unwrap(), Some(2));
        assert!(
            with_roundtrip_target(FormatVersion::new(2, minor), || write_form(
                FormDialect::Designer,
                &body
            ))
            .is_err()
        );
        let (native, resource) = project_chart_semantics(&body, UUID).unwrap().unwrap();
        assert_eq!(chart_semantics_resource_count(&native).unwrap(), None);
        assert!(project_chart_semantics(&native, UUID).unwrap().is_none());
        let bytes = with_roundtrip_target(FormatVersion::new(2, minor), || {
            write_form(FormDialect::Designer, &native)
        })
        .unwrap();
        let mut returned = with_source_version(Some(FormatVersion::new(2, minor)), || {
            read_form(FormDialect::Designer, &bytes)
        })
        .unwrap();
        apply_chart_semantics_resource(&mut returned, UUID, &resource).unwrap();
        assert_eq!(digest(&returned), digest(&body));
        let old = digest(&returned);
        rows(chart(&mut returned))[0]
            .iter_mut()
            .find(|(n, _)| n == "isToolTipFormatted")
            .unwrap()
            .1 = ChartValue::Bool(false);
        assert_ne!(digest(&returned), old);
    }
}
#[test]
fn resource_rejects_unknown_duplicate_or_stale_bindings_atomically() {
    let (native, resource) = project_chart_semantics(&body(), UUID).unwrap().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&resource).unwrap();
    let mut cases = Vec::new();
    let mut v = value.clone();
    v["unknown"] = true.into();
    cases.push(v);
    let mut v = value.clone();
    v["records"][0]["attribute"][0]["id"] = 99.into();
    cases.push(v);
    let mut v = value.clone();
    v["records"][1]["current_item_sha256"] = "0".repeat(64).into();
    cases.push(v);
    let mut v = value.clone();
    let row = v["records"][0].clone();
    v["records"].as_array_mut().unwrap().push(row);
    cases.push(v);
    let mut v = value.clone();
    v["records"][0]["flags"][0][0] = "unknown".into();
    cases.push(v);
    for v in cases {
        let mut current = native.clone();
        let before = digest(&current);
        assert!(
            apply_chart_semantics_resource(&mut current, UUID, &serde_json::to_vec(&v).unwrap())
                .is_err()
        );
        assert_eq!(digest(&current), before);
    }
    let duplicate = String::from_utf8(resource.clone())
        .unwrap()
        .replace("\"version\": 1", "\"version\": 1, \"version\": 1");
    assert!(
        apply_chart_semantics_resource(&mut native.clone(), UUID, duplicate.as_bytes()).is_err()
    );
    assert!(
        apply_chart_semantics_resource(&mut native.clone(), Uuid([64; 16]), &resource).is_err()
    );
    let mut changed = native.clone();
    rows(chart(&mut changed)).reverse();
    assert!(apply_chart_semantics_resource(&mut changed, UUID, &resource).is_err());
    let mut deleted = native;
    deleted.data_attributes.clear();
    assert!(apply_chart_semantics_resource(&mut deleted, UUID, &resource).is_err());
}

#[test]
fn public_exact_return_stripped_current_edits_and_forged_hashes() {
    for minor in [20, 21] {
        let options = options(minor);
        let original = original(minor);
        let converted = xml_to_edt(&original, &options).unwrap();
        assert!(
            converted
                .extensions
                .iter()
                .any(|e| e.id == "ibcmd-chart-semantics/1"
                    && e.resources == 1
                    && e.references == 2)
        );
        assert!(
            converted
                .accounting
                .iter()
                .any(|e| e.path.as_str() == RESOURCE
                    && e.disposition == ibcmd_edt::Disposition::Converted)
        );
        let generated = converted.tree;
        assert_eq!(
            edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
                .unwrap()
                .tree,
            original
        );
        let stripped = alter(&generated, "unused", Some(b""), true);
        assert_eq!(
            edt_to_xml(&Project::from_tree(stripped.clone()).unwrap(), &options)
                .unwrap()
                .tree,
            original
        );
        let mut edited = read_chart_sidecar(entry(&generated, SIDECAR)).unwrap();
        let current = rows(&mut edited);
        current.reverse();
        current[1]
            .iter_mut()
            .find(|(n, _)| n == "tooltip")
            .unwrap()
            .1 = ChartValue::Str("current edit".into());
        current[1]
            .iter_mut()
            .find(|(n, _)| n == "isToolTipFormatted")
            .unwrap()
            .1 = ChartValue::Bool(false);
        let bytes = write_chart_sidecar(&edited).unwrap();
        let changed = edt_to_xml(
            &Project::from_tree(alter(&stripped, SIDECAR, Some(&bytes), false)).unwrap(),
            &options,
        )
        .unwrap();
        assert_eq!(
            decoded(&changed.tree, minor).data_attributes[0]
                .chart_settings
                .as_ref()
                .unwrap(),
            &edited
        );
        assert_ne!(changed.tree, original);
        assert!(
            changed
                .extensions
                .iter()
                .any(|e| e.id == "ibcmd-chart-semantics/1" && e.references == 1)
        );
        let mut manifest: serde_json::Value =
            serde_json::from_slice(entry(&generated, ".ibcmd-provenance/manifest.json")).unwrap();
        manifest["generated"][SIDECAR] = format!("{:x}", Sha256::digest(&bytes)).into();
        let forged = alter(
            &alter(&generated, SIDECAR, Some(&bytes), false),
            ".ibcmd-provenance/manifest.json",
            Some(&serde_json::to_vec(&manifest).unwrap()),
            false,
        );
        assert!(edt_to_xml(&Project::from_tree(forged).unwrap(), &options).is_err());
        let missing = alter(&original, RESOURCE, None, false);
        let no_flags = xml_to_edt(&missing, &options).unwrap();
        assert!(
            !no_flags
                .extensions
                .iter()
                .any(|e| e.id == "ibcmd-chart-semantics/1")
        );
        let mut orphan = original.entries().to_vec();
        orphan.retain(|e| e.path().as_str() != FORM);
        assert!(xml_to_edt(&SourceTree::new(orphan).unwrap(), &options).is_err());
        let stale = String::from_utf8(entry(&original, FORM).to_vec())
            .unwrap()
            .replace(">first</", ">changed</");
        assert_ne!(stale.as_bytes(), entry(&original, FORM));
        assert!(
            xml_to_edt(
                &alter(&original, FORM, Some(stale.as_bytes()), false),
                &options
            )
            .is_err()
        );
    }
}
#[test]
fn public_current_unicode_number_uses_sdk_digits_without_replaying_previous_values() {
    for minor in [20, 21] {
        let options = options(minor);
        let native = original(minor);
        let generated = xml_to_edt(&native, &options).unwrap().tree;
        let stripped = alter(&generated, "unused", Some(b""), true);
        let current = String::from_utf8(entry(&stripped, SIDECAR).to_vec()).unwrap();
        let unicode = current.replace(">37.5<", ">٢.٧٥<");
        assert_ne!(unicode, current);
        let converted = edt_to_xml(
            &Project::from_tree(alter(&stripped, SIDECAR, Some(unicode.as_bytes()), false))
                .unwrap(),
            &options,
        )
        .unwrap();
        let output = String::from_utf8(entry(&converted.tree, FORM).to_vec()).unwrap();
        assert!(output.contains(">2.75<"));
        assert!(!output.contains(">37.5<"));
        assert!(!output.contains('٢'));
    }
}

#[test]
fn directory_resources_are_consumed_reemitted_and_accounted_once() {
    for minor in [20, 21] {
        let options = options(minor);
        let original = original(minor);
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("native");
        std::fs::create_dir(&source).unwrap();
        for e in original.entries() {
            let path = source.join(e.path().as_str());
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, e.bytes()).unwrap();
        }
        let edt = read_directory_source(&source)
            .unwrap()
            .xml_to_edt(&options)
            .unwrap();
        assert!(
            edt.extensions
                .iter()
                .any(|e| e.id == "ibcmd-chart-semantics/1"
                    && e.resources == 1
                    && e.references == 2)
        );
        assert_eq!(edt.accounting.iter().filter(|e| e.path == RESOURCE && e.disposition == ibcmd_edt::Disposition::Converted).count(), 1);
        let project = tmp.path().join("project");
        edt.publish_new(&project).unwrap();
        std::fs::remove_dir_all(project.join(".ibcmd-provenance")).unwrap();
        let native = read_directory_project(&project)
            .unwrap()
            .edt_to_xml(&options)
            .unwrap();
        assert!(
            native
                .extensions
                .iter()
                .any(|e| e.id == "ibcmd-chart-semantics/1"
                    && e.resources == 1
                    && e.references == 2)
        );
        let output = tmp.path().join("returned");
        native.publish_new(&output).unwrap();
        assert_eq!(
            read_xml_source(&output, ReaderLimits::default()).unwrap(),
            original
        );
        assert!(
            output
                .join("CommonForms/ChartFlags/Ext")
                .join(CHART_SEMANTICS_RESOURCE)
                .is_file()
        );
    }
}
