use formats_xml::form::{
    DATA_PATH_SEMANTICS_RESOURCE, FormDialect, read_form, read_native_data_path_annotation,
};
use ibcmd_edt::{
    ConversionOptions, Disposition, Project, ReaderLimits, edt_to_xml, read_directory_project,
    read_directory_source, read_xml_source, xml_to_edt,
};
use ibcmd_xml::source_tree::{SourceEntry, SourceTree};
use morph1c_core::{
    ir::{
        MetadataObject, NamedFormBody, ObjectKind, PropertyValue, Token, Uuid, form::DataPathSpec,
    },
    spec::forms::controls::form_field as ff,
    version::{FormatVersion, with_roundtrip_target},
};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
use sha2::{Digest, Sha256};
const FORM: &str = "src/CommonForms/RichPaths/Form.form";
const RESOURCE: &str = "src/CommonForms/RichPaths/ibcmd-form-data-path-semantics.v1.json";
fn options(minor: u16) -> ConversionOptions {
    ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: format!("2.{minor}"),
        runtime_version: Some(if minor == 20 { "8.3.27" } else { "8.5.1" }.into()),
    }
}
fn path() -> DataPathSpec {
    DataPathSpec {
        segments: vec!["List".into(), "A~literal".into()],
        extra_paths: vec!["Other.B".into(), "Next.C".into(), "Other.B".into()],
    }
}
fn native(minor: u16) -> SourceTree {
    native_with(minor, path(), "")
}
fn native_with(minor: u16, current: DataPathSpec, attributes: &str) -> SourceTree {
    let fixture = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/subsystem-ci/src"
    ));
    let mut cfg = read_config(Format::Designer, fixture, &ConvertOptions::default())
        .unwrap()
        .0;
    let xml = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
        "<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\"><items xsi:type=\"form:FormField\"><name>Field</name><id>1</id><dataPath xsi:type=\"form:DataPath\"><segments>List.A</segments></dataPath><type>LabelField</type><extInfo xsi:type=\"form:LabelFieldExtInfo\"/></items></form:Form>\r\n"
    );
    let xml = xml.replace("</form:Form>", &format!("{attributes}</form:Form>"));
    let mut body = read_form(FormDialect::Edt, xml.as_bytes()).unwrap();
    body.items[0]
        .properties
        .iter_mut()
        .find(|(id, _)| *id == ff::F_DATA_PATH)
        .unwrap()
        .1 = PropertyValue::DataPath(current);
    let mut obj = MetadataObject::new(ObjectKind::new("CommonForm"), "RichPaths", Uuid([77; 16]));
    let field = morph1c_core::spec::registry::spec_for("CommonForm")
        .unwrap()
        .fields()
        .iter()
        .find(|f| f.name == "formType")
        .unwrap()
        .id;
    obj.properties
        .push((field, PropertyValue::Enum(Token::new("Managed"))));
    obj.form_bodies.push(NamedFormBody {
        name: "RichPaths".into(),
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
    let text = String::from_utf8(std::fs::read(&root).unwrap())
        .unwrap()
        .replace(
            "</Language>",
            "</Language>\r\n\t\t\t<CommonForm>RichPaths</CommonForm>",
        );
    std::fs::write(root, text).unwrap();
    read_xml_source(dir.path(), ReaderLimits::default()).unwrap()
}
fn bytes<'a>(tree: &'a SourceTree, path: &str) -> &'a [u8] {
    tree.entries()
        .iter()
        .find(|e| e.path().as_str() == path)
        .unwrap()
        .bytes()
}
fn altered(tree: &SourceTree, strip: bool, path: Option<(&str, Vec<u8>)>) -> SourceTree {
    SourceTree::new(
        tree.entries()
            .iter()
            .filter(|e| !strip || !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .map(|e| {
                SourceEntry::from_bytes(
                    e.path().clone(),
                    path.as_ref()
                        .filter(|(p, _)| *p == e.path().as_str())
                        .map_or_else(|| e.bytes().to_vec(), |(_, b)| b.clone()),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
fn current(tree: &SourceTree, minor: u16) -> DataPathSpec {
    let dir = tempfile::tempdir().unwrap();
    publish(tree, dir.path());
    let cfg = read_config(Format::Designer, dir.path(), &ConvertOptions::default())
        .unwrap()
        .0;
    assert_eq!(cfg.source_version, Some(FormatVersion::new(2, minor)));
    let form = &cfg
        .objects
        .iter()
        .find(|o| o.name == "RichPaths")
        .unwrap()
        .form_bodies[0]
        .body;
    match form.items[0].get(ff::F_DATA_PATH).unwrap() {
        PropertyValue::DataPath(path) => path.clone(),
        PropertyValue::Ref(path) => DataPathSpec::from(path.as_str()),
        _ => panic!("CURRENT typed path missing"),
    }
}
fn publish(tree: &SourceTree, root: &std::path::Path) {
    for entry in tree.entries() {
        let path = root.join(entry.path().as_str());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, entry.bytes()).unwrap();
    }
}
#[test]
fn source_tree_complete_return_stripped_current_and_converted_annotation_both_profiles() {
    for minor in [20, 21] {
        let source = native(minor);
        assert_eq!(current(&source, minor), path());
        let generated = xml_to_edt(&source, &options(minor)).unwrap();
        assert!(
            generated
                .accounting
                .iter()
                .any(|r| r.path.as_str() == "ConfigDumpInfo.xml"
                    && r.disposition == Disposition::Converted)
        );
        assert!(
            generated
                .extensions
                .iter()
                .any(|r| r.id == "ibcmd-form-data-path-semantics/1")
        );
        assert!(
            bytes(&generated.tree, RESOURCE)
                .windows(b"extra_paths".len())
                .any(|s| s == b"extra_paths")
        );
        assert!(
            String::from_utf8_lossy(bytes(&generated.tree, FORM))
                .contains("<segments>List.A~literal</segments>")
        );
        let returned = edt_to_xml(
            &Project::from_tree(generated.tree.clone()).unwrap(),
            &options(minor),
        )
        .unwrap()
        .tree;
        assert_eq!(returned, source);
        let stripped = altered(&generated.tree, true, None);
        let returned = edt_to_xml(&Project::from_tree(stripped).unwrap(), &options(minor))
            .unwrap()
            .tree;
        assert_eq!(current(&returned, minor), path());
        assert!(
            !returned
                .entries()
                .iter()
                .any(|e| e.path().as_str().ends_with(DATA_PATH_SEMANTICS_RESOURCE))
        );
    }
}
#[test]
fn directory_return_and_stripped_current_transport_both_profiles() {
    for minor in [20, 21] {
        let source = native(minor);
        let tmp = tempfile::tempdir().unwrap();
        let input = tmp.path().join("input");
        std::fs::create_dir(&input).unwrap();
        publish(&source, &input);
        let generated = read_directory_source(&input)
            .unwrap()
            .xml_to_edt(&options(minor))
            .unwrap();
        let project = tmp.path().join("project");
        generated.publish_new(&project).unwrap();
        let returned = read_directory_project(&project)
            .unwrap()
            .edt_to_xml(&options(minor))
            .unwrap();
        let output = tmp.path().join("output");
        returned.publish_new(&output).unwrap();
        assert_eq!(
            read_xml_source(&output, ReaderLimits::default()).unwrap(),
            source
        );
        let memory = xml_to_edt(&source, &options(minor)).unwrap().tree;
        let stripped = tmp.path().join("stripped");
        std::fs::create_dir(&stripped).unwrap();
        publish(&altered(&memory, true, None), &stripped);
        let current_output = tmp.path().join("current");
        read_directory_project(&stripped)
            .unwrap()
            .edt_to_xml(&options(minor))
            .unwrap()
            .publish_new(&current_output)
            .unwrap();
        assert_eq!(
            current(
                &read_xml_source(&current_output, ReaderLimits::default()).unwrap(),
                minor
            ),
            path()
        );
    }
}
#[test]
fn ordered_current_carrier_edits_win_and_forged_generated_hash_cannot_replay_old_values() {
    for minor in [20, 21] {
        let source = native(minor);
        let generated = xml_to_edt(&source, &options(minor)).unwrap().tree;
        let mut json: serde_json::Value =
            serde_json::from_slice(bytes(&generated, RESOURCE)).unwrap();
        let extras = json["records"][0]["current"]["extra_paths"]
            .as_array_mut()
            .unwrap();
        extras.remove(0);
        extras.push("CURRENT.D".into());
        extras.swap(0, 1);
        let changed = serde_json::to_vec(&json).unwrap();
        let mut expected = path();
        expected.extra_paths = vec!["Other.B".into(), "Next.C".into(), "CURRENT.D".into()];
        let stripped = altered(&generated, true, Some((RESOURCE, changed.clone())));
        let returned = edt_to_xml(&Project::from_tree(stripped).unwrap(), &options(minor))
            .unwrap()
            .tree;
        assert_eq!(current(&returned, minor), expected);
        let mut manifest: serde_json::Value =
            serde_json::from_slice(bytes(&generated, ".ibcmd-provenance/manifest.json")).unwrap();
        manifest["generated"][RESOURCE] = format!("{:x}", Sha256::digest(&changed)).into();
        let changed_tree = altered(&generated, false, Some((RESOURCE, changed)));
        let forged = altered(
            &changed_tree,
            false,
            Some((
                ".ibcmd-provenance/manifest.json",
                serde_json::to_vec(&manifest).unwrap(),
            )),
        );
        assert!(edt_to_xml(&Project::from_tree(forged).unwrap(), &options(minor)).is_err());
        let old_body = bytes(&generated, FORM);
        let changed_body = String::from_utf8(old_body.to_vec())
            .unwrap()
            .replace("List.A~literal", "CURRENT.X")
            .into_bytes();
        assert_ne!(changed_body, old_body);
        assert!(
            edt_to_xml(
                &Project::from_tree(altered(&generated, true, Some((FORM, changed_body)))).unwrap(),
                &options(minor)
            )
            .is_err()
        );
    }
}
fn base64(bytes: &[u8]) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bits = 0u32;
    let mut count = 0;
    let mut out = String::new();
    for b in bytes {
        bits = (bits << 8) | u32::from(*b);
        count += 8;
        while count >= 6 {
            count -= 6;
            out.push(A[((bits >> count) & 63) as usize] as char);
        }
    }
    if count > 0 {
        out.push(A[((bits << (6 - count)) & 63) as usize] as char);
    }
    while !out.len().is_multiple_of(4) {
        out.push('=');
    }
    out
}
fn comment(tree: &SourceTree, minor: u16, mutation: usize) -> Vec<u8> {
    let cdf = String::from_utf8(bytes(tree, "ConfigDumpInfo.xml").to_vec()).unwrap();
    let annotation = read_native_data_path_annotation(cdf.as_bytes(), FormatVersion::new(2, minor))
        .unwrap()
        .unwrap();
    let resource: serde_json::Value =
        serde_json::from_slice(&annotation.form_resource(Uuid([77; 16])).unwrap().unwrap())
            .unwrap();
    let root = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/subsystem-ci/src"
    ));
    let cfg = read_config(Format::Designer, root, &ConvertOptions::default())
        .unwrap()
        .0;
    let uuid = cfg
        .objects
        .iter()
        .find(|o| o.kind.as_str() == "Configuration")
        .unwrap()
        .uuid;
    let mut json = serde_json::json!({"schema":"urn:ibcmd:source-extension:configuration-semantics:1","version":1,"configuration_uuid":uuid,"data_paths":[resource]});
    match mutation {
        0 => json["configuration_uuid"] = serde_json::json!(Uuid([99; 16])),
        1 => {
            let row = json["data_paths"][0].clone();
            json["data_paths"].as_array_mut().unwrap().push(row);
        }
        2 => json["data_paths"][0]["form_uuid"] = serde_json::json!(Uuid([88; 16])),
        3 => json["data_paths"][0]["records"][0]["current"]["segments"][0] = "Unrelated".into(),
        4 => json["data_paths"][0]["unknown"] = true.into(),
        _ => {
            json["data_paths"][0]["profile"] =
                serde_json::json!([2, if minor == 20 { 21 } else { 20 }])
        }
    }
    let start = cdf.find("<!-- ibcmd-configuration-semantics:1:").unwrap();
    let end = start + cdf[start..].find("-->").unwrap() + 3;
    format!(
        "{}<!-- ibcmd-configuration-semantics:1:{} -->{}",
        &cdf[..start],
        base64(&serde_json::to_vec(&json).unwrap()),
        &cdf[end..]
    )
    .into_bytes()
}
#[test]
fn global_owner_payload_profile_duplicates_and_unknowns_fail_on_both_public_routes() {
    for minor in [20, 21] {
        let source = native(minor);
        for mutation in 0..6 {
            let altered = altered(
                &source,
                false,
                Some(("ConfigDumpInfo.xml", comment(&source, minor, mutation))),
            );
            assert!(
                xml_to_edt(&altered, &options(minor)).is_err(),
                "invalid CURRENT global annotation accepted"
            );
            let tmp = tempfile::tempdir().unwrap();
            publish(&altered, tmp.path());
            assert!(
                read_directory_source(tmp.path())
                    .unwrap()
                    .xml_to_edt(&options(minor))
                    .is_err()
            );
        }
    }
}

#[test]
fn reserved_annotation_family_unknown_and_malformed_versions_fail_on_both_routes() {
    for minor in [20, 21] {
        let source = native(minor);
        let cdf = String::from_utf8(bytes(&source, "ConfigDumpInfo.xml").to_vec()).unwrap();
        // v2 is now a supported closed protocol. Unknown and malformed version
        // headers remain a distinct rejection class from malformed v2 contents.
        for version in ["3:", "", "one:", "1", ":"] {
            let changed = cdf.replace(
                "ibcmd-configuration-semantics:1:",
                &format!("ibcmd-configuration-semantics:{version}"),
            );
            assert_ne!(changed.as_bytes(), cdf.as_bytes());
            let changed = altered(
                &source,
                false,
                Some(("ConfigDumpInfo.xml", changed.into_bytes())),
            );
            let error = xml_to_edt(&changed, &options(minor))
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("unknown/malformed reserved annotation version"),
                "{error}"
            );
            let tmp = tempfile::tempdir().unwrap();
            publish(&changed, tmp.path());
            let error = read_directory_source(tmp.path())
                .unwrap()
                .xml_to_edt(&options(minor))
                .err()
                .unwrap()
                .to_string();
            assert!(
                error.contains("unknown/malformed reserved annotation version"),
                "{error}"
            );
        }
    }
}
#[test]
fn v1_payload_relabelled_as_v2_rejects_missing_presence_collection_on_both_routes() {
    for minor in [20, 21] {
        let source = native(minor);
        let original = bytes(&source, "ConfigDumpInfo.xml");
        assert!(
            read_native_data_path_annotation(original, FormatVersion::new(2, minor))
                .unwrap()
                .is_some()
        );
        let original = String::from_utf8(original.to_vec()).unwrap();
        let relabelled = original.replace(
            "ibcmd-configuration-semantics:1:",
            "ibcmd-configuration-semantics:2:",
        );
        assert_ne!(relabelled.as_bytes(), original.as_bytes());
        let changed = altered(
            &source,
            false,
            Some(("ConfigDumpInfo.xml", relabelled.into_bytes())),
        );
        let error = xml_to_edt(&changed, &options(minor))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("missing field") && error.contains("form_presence"),
            "{error}"
        );
        let tmp = tempfile::tempdir().unwrap();
        publish(&changed, tmp.path());
        let error = read_directory_source(tmp.path())
            .unwrap()
            .xml_to_edt(&options(minor))
            .err()
            .unwrap()
            .to_string();
        assert!(
            error.contains("missing field") && error.contains("form_presence"),
            "{error}"
        );
    }
}
fn plain_native(minor: u16) -> (SourceTree, DataPathSpec) {
    let attributes = concat!(
        "<attributes><name>List</name><id>1</id><valueType><types>DynamicList</types></valueType>",
        "<view><common>true</common></view><edit><common>true</common></edit>",
        "<extInfo xsi:type=\"form:DynamicListExtInfo\"><queryText>SELECT Source.Real AS Selected FROM Source</queryText>",
        "<autoFillAvailableFields>true</autoFillAvailableFields><customQuery>true</customQuery></extInfo></attributes>"
    );
    let expected = DataPathSpec {
        segments: vec!["List".into(), "Selected".into()],
        extra_paths: vec!["Other.B".into(), "Next.C".into(), "Other.B".into()],
    };
    let source = native_with(minor, expected.clone(), attributes);
    let cdf = String::from_utf8(bytes(&source, "ConfigDumpInfo.xml").to_vec()).unwrap();
    let start = cdf.find("<!-- ibcmd-configuration-semantics:1:").unwrap();
    let end = start + cdf[start..].find("-->").unwrap() + 3;
    let cdf = format!(
        "{}<!-- unrelated retained storage note -->{}",
        &cdf[..start],
        &cdf[end..]
    );
    let cdf = cdf.replacen(
        "<Metadata ",
        "<Metadata configVersion=\"0123456789abcdef0123456789abcdef\" ",
        1,
    );
    assert!(
        read_native_data_path_annotation(cdf.as_bytes(), FormatVersion::new(2, minor))
            .unwrap()
            .is_none()
    );
    let source = altered(
        &source,
        false,
        Some(("ConfigDumpInfo.xml", cdf.into_bytes())),
    );
    let form_path = "CommonForms/RichPaths/Ext/Form.xml";
    let form = String::from_utf8(bytes(&source, form_path).to_vec()).unwrap();
    let changed = form.replace(
        "<DataPath>List.Selected</DataPath>",
        "<DataPath>List.Selected~Other.B~Next.C~Other.B</DataPath>",
    );
    assert_ne!(
        form, changed,
        "fixture must edit the standard native current path"
    );
    (
        altered(&source, false, Some((form_path, changed.into_bytes()))),
        expected,
    )
}
#[test]
fn plain_source_manifest_is_retained_exact_while_current_annotation_is_regenerated() {
    for minor in [20, 21] {
        let (source, expected) = plain_native(minor);
        assert_eq!(current(&source, minor), expected);
        let generated = xml_to_edt(&source, &options(minor)).unwrap();
        assert!(
            generated
                .accounting
                .iter()
                .any(|r| r.path.as_str() == "ConfigDumpInfo.xml"
                    && r.disposition == Disposition::Retained)
        );
        let returned = edt_to_xml(
            &Project::from_tree(generated.tree.clone()).unwrap(),
            &options(minor),
        )
        .unwrap()
        .tree;
        assert_eq!(
            returned, source,
            "whole provenance must retain the original manifest and all original bytes"
        );
        let stripped = altered(&generated.tree, true, None);
        let current_native = edt_to_xml(&Project::from_tree(stripped).unwrap(), &options(minor))
            .unwrap()
            .tree;
        assert_eq!(current(&current_native, minor), expected);
        let current_cdf = bytes(&current_native, "ConfigDumpInfo.xml");
        assert!(
            read_native_data_path_annotation(current_cdf, FormatVersion::new(2, minor))
                .unwrap()
                .is_some()
        );
        assert_ne!(current_cdf, bytes(&source, "ConfigDumpInfo.xml"));
        assert!(!String::from_utf8_lossy(current_cdf).contains("configVersion="));
        assert!(!String::from_utf8_lossy(current_cdf).contains("unrelated retained storage note"));
        let tmp = tempfile::tempdir().unwrap();
        let input = tmp.path().join("input");
        std::fs::create_dir(&input).unwrap();
        publish(&source, &input);
        let generated = read_directory_source(&input)
            .unwrap()
            .xml_to_edt(&options(minor))
            .unwrap();
        assert!(
            generated
                .accounting
                .iter()
                .any(|r| r.path == "ConfigDumpInfo.xml" && r.disposition == Disposition::Retained)
        );
        let project = tmp.path().join("project");
        generated.publish_new(&project).unwrap();
        let returned = read_directory_project(&project)
            .unwrap()
            .edt_to_xml(&options(minor))
            .unwrap();
        let output = tmp.path().join("output");
        returned.publish_new(&output).unwrap();
        assert_eq!(
            read_xml_source(&output, ReaderLimits::default()).unwrap(),
            source
        );
        let stripped_dir = tmp.path().join("stripped");
        std::fs::create_dir(&stripped_dir).unwrap();
        let memory = xml_to_edt(&source, &options(minor)).unwrap();
        publish(&altered(&memory.tree, true, None), &stripped_dir);
        let current_dir = tmp.path().join("current");
        read_directory_project(&stripped_dir)
            .unwrap()
            .edt_to_xml(&options(minor))
            .unwrap()
            .publish_new(&current_dir)
            .unwrap();
        let current_tree = read_xml_source(&current_dir, ReaderLimits::default()).unwrap();
        assert_eq!(current(&current_tree, minor), expected);
        assert!(
            read_native_data_path_annotation(
                bytes(&current_tree, "ConfigDumpInfo.xml"),
                FormatVersion::new(2, minor)
            )
            .unwrap()
            .is_some()
        );
    }
}
