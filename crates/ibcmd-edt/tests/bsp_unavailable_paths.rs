use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::{
    ir::{FormBody, FormControlKind, FormItem, PropertyValue},
    version::{FormatVersion, with_roundtrip_target, with_source_version},
};

fn body(query: &str, main_table: Option<&str>) -> FormBody {
    let escape = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let table = main_table
        .map(|t| format!("<mainTable>{}</mainTable>", escape(t)))
        .unwrap_or_default();
    let bytes = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\"><attributes><name>List</name><valueType><types>DynamicList</types></valueType><view><common>true</common></view><edit><common>true</common></edit><notDefaultUseAlwaysAttributes xsi:type=\"form:DataPath\"><segments>List.Missing</segments></notDefaultUseAlwaysAttributes><notDefaultUseAlwaysAttributes xsi:type=\"form:DataPath\"><segments>List.Selected</segments></notDefaultUseAlwaysAttributes><extInfo xsi:type=\"form:DynamicListExtInfo\"><queryText>{}</queryText>{table}<autoFillAvailableFields>true</autoFillAvailableFields><customQuery>true</customQuery></extInfo></attributes></form:Form>\r\n",
        escape(query)
    );
    with_source_version(Some(FormatVersion::new(2, 20)), || {
        read_form(FormDialect::Edt, bytes.as_bytes())
    })
    .unwrap()
}
fn native(body: &FormBody, version: FormatVersion) -> Vec<u8> {
    with_roundtrip_target(version, || write_form(FormDialect::Designer, body)).unwrap()
}
#[test]
fn final_query_batch_aliases_are_bounded_and_source_version_specific() {
    let form = body(
        "SELECT Old AS Missing INTO Temporary; SELECT Source.Real AS Selected FROM Source; DESTROY Temporary",
        None,
    );
    let text = String::from_utf8(native(&form, FormatVersion::new(2, 20))).unwrap();
    assert!(text.contains("<Field>~List.Missing</Field>"));
    assert!(text.contains("<Field>List.Selected</Field>"));
    assert!(
        !String::from_utf8(native(&form, FormatVersion::new(2, 21)))
            .unwrap()
            .contains("~List.Missing")
    );
    let quote = body(
        "SELECT \"SELECT Missing FROM fake\" AS Selected // Missing\nFROM Source",
        None,
    );
    assert!(
        String::from_utf8(native(&quote, FormatVersion::new(2, 20)))
            .unwrap()
            .contains("~List.Missing")
    );
    let union = body(
        "SELECT A AS Selected FROM Source UNION ALL SELECT B AS Missing FROM Other",
        None,
    );
    assert!(
        String::from_utf8(native(&union, FormatVersion::new(2, 20)))
            .unwrap()
            .contains("~List.Missing")
    );
    let wildcard = body("SELECT Source.* FROM Source", None);
    assert!(
        !String::from_utf8(native(&wildcard, FormatVersion::new(2, 20)))
            .unwrap()
            .contains("~List.Missing")
    );
    for query in [
        "SELECT A.X + B.Y FROM Source",
        "SELECT A.X Implicit FROM Source",
    ] {
        let unknown = body(query, None);
        assert!(
            !String::from_utf8(native(&unknown, FormatVersion::new(2, 20)))
                .unwrap()
                .contains("~List.Missing")
        );
    }
    let mut invalid = body("SELECT Selected FROM Source", None);
    invalid.data_attributes[0]
        .dynamic_list
        .as_mut()
        .unwrap()
        .query_text = Some("SELECT \"unterminated".into());
    assert!(
        with_roundtrip_target(FormatVersion::new(2, 20), || write_form(
            FormDialect::Designer,
            &invalid
        ))
        .is_err()
    );
    invalid.data_attributes[0]
        .dynamic_list
        .as_mut()
        .unwrap()
        .query_text = Some(format!(
        "SELECT {}x{} AS Selected",
        "(".repeat(65),
        ")".repeat(65)
    ));
    assert!(
        with_roundtrip_target(FormatVersion::new(2, 20), || write_form(
            FormDialect::Designer,
            &invalid
        ))
        .is_err()
    );
}
#[test]
fn platform_picture_carrier_and_native_spelling_are_independent() {
    use morph1c_core::spec::forms::controls::table as tb;
    for (main, marked) in [
        (None, true),
        (Some("Catalog.Data"), false),
        (Some("Enum.Values"), true),
        (Some("FilterCriterion.Selection"), true),
    ] {
        let mut form = body("SELECT A AS Selected FROM Source", main);
        let mut item = FormItem::new(FormControlKind::new("Table"), "ListTable", 1);
        item.properties.push((
            tb::F_ROW_PICTURE_DATA_PATH,
            PropertyValue::Ref("List.DefaultPicture".into()),
        ));
        form.items.push(item);
        let bytes = native(&form, FormatVersion::new(2, 20));
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert_eq!(
            text.contains("<RowPictureDataPath>~List.DefaultPicture</RowPictureDataPath>"),
            marked
        );
        let source = with_source_version(Some(FormatVersion::new(2, 20)), || {
            read_form(FormDialect::Designer, &bytes)
        })
        .unwrap();
        assert!(native(&source, FormatVersion::new(2, 20)) == bytes);
        let mut lexical = source.clone();
        lexical.data_attributes[0]
            .designer_unavailable_paths
            .clear();
        for item in &mut lexical.items {
            item.row_picture_path_unavailable = false;
        }
        assert_eq!(
            serde_json::to_vec(&lexical).unwrap(),
            serde_json::to_vec(&source).unwrap()
        );
        lexical.data_attributes[0].not_default_use_always[0] = "List.Edited".into();
        assert_ne!(
            serde_json::to_vec(&lexical).unwrap(),
            serde_json::to_vec(&source).unwrap()
        );
    }
}

#[test]
#[ignore = "Read-only exact final eight BSP SDK and original source witnesses on F"]
fn genuine_eight_sdk_differences_close_without_changing_native_source() {
    use std::path::Path;
    let lab = Path::new(r"F:\ibcmd\lab\07");
    let report: serde_json::Value = serde_json::from_slice(
        &std::fs::read(lab.join("accept-bsp83-a09-final-r1/direct-edt-native-sdk-comparison.json"))
            .unwrap(),
    )
    .unwrap();
    let rows = report["differing_rows"].as_array().unwrap();
    assert_eq!(rows.len(), 8);
    for row in rows {
        let rel = row["path"].as_str().unwrap();
        for root in [lab.join("accept-bsp83-a09-final-r1/ours-authentic-edt-xml"),Path::new(r"F:\ibcmd\lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2\native").to_owned()]{
            let bytes=std::fs::read(root.join(rel)).unwrap();assert!(bytes.len()<1024*1024);
            let form=with_source_version(Some(FormatVersion::new(2,20)),||read_form(FormDialect::Designer,&bytes)).unwrap();
            assert!(native(&form,FormatVersion::new(2,20))==bytes,"same-source bytes changed at {rel}");
        }
        // This is the fully attached typed body from the genuine first route,
        // including DCS sidecars. Source glyphs are absent in this output.
        let bytes = std::fs::read(
            lab.join("accept-bsp83-a09-final-r1/ours-authentic-edt-xml")
                .join(rel),
        )
        .unwrap();
        let mut form = with_source_version(Some(FormatVersion::new(2, 20)), || {
            read_form(FormDialect::Designer, &bytes)
        })
        .unwrap();
        form.designer_path_spelling = false;
        let generated = native(&form, FormatVersion::new(2, 20));
        let expected =
            std::fs::read(lab.join("native-reference-bsp83-r2/native-xml").join(rel)).unwrap();
        assert!(generated == expected, "SDK bytes still differ at {rel}");
    }
}

#[test]
fn public_native_sigil_roundtrip_is_exact_and_edited_path_rejects_forged_hash() {
    use ibcmd_edt::{
        ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
    };
    use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
    use morph1c_core::ir::{
        DcsListSettings, MetadataObject, NamedFormBody, ObjectKind, Token, Uuid,
    };
    use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
    use sha2::{Digest, Sha256};
    let fixture = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/subsystem-ci/src"
    ));
    let mut cfg = read_config(Format::Designer, fixture, &ConvertOptions::default())
        .unwrap()
        .0;
    let mut form = body("SELECT Source.Real AS Selected FROM Source", None);
    form.data_attributes[0]
        .designer_unavailable_paths
        .push("List.Missing".into());
    form.data_attributes[0]
        .dynamic_list
        .as_mut()
        .unwrap()
        .list_settings = Some(DcsListSettings {
        items_view_mode: Some("Normal".into()),
        ..Default::default()
    });
    let mut obj = MetadataObject::new(ObjectKind::new("CommonForm"), "Paths", Uuid([42; 16]));
    let form_type = morph1c_core::spec::registry::spec_for("CommonForm")
        .unwrap()
        .fields()
        .iter()
        .find(|f| f.name == "formType")
        .unwrap()
        .id;
    obj.properties
        .push((form_type, PropertyValue::Enum(Token::new("Managed"))));
    obj.form_bodies.push(NamedFormBody {
        name: "Paths".into(),
        body: form,
        ordinary_body: None,
        module: None,
        help: vec![],
        help_resources: vec![],
    });
    cfg.objects.push(obj);
    let dir = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &cfg, dir.path()).unwrap();
    let path = dir.path().join("Configuration.xml");
    let root = String::from_utf8(std::fs::read(&path).unwrap())
        .unwrap()
        .replace(
            "</Language>",
            "</Language>\r\n\t\t\t<CommonForm>Paths</CommonForm>",
        );
    std::fs::write(path, root).unwrap();
    let options = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    let original = read_xml_source(dir.path(), ReaderLimits::default()).unwrap();
    let generated = xml_to_edt(&original, &options).unwrap().tree;
    let returned = edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
        .unwrap()
        .tree;
    assert_eq!(returned, original);
    let path = "src/CommonForms/Paths/Form.form";
    let bytes = generated
        .entries()
        .iter()
        .find(|e| e.path().as_str() == path)
        .unwrap()
        .bytes();
    let edited = String::from_utf8(bytes.to_vec())
        .unwrap()
        .replace("List.Missing", "List.Edited")
        .into_bytes();
    assert!(edited != bytes);
    let hash = format!("{:x}", Sha256::digest(&edited));
    let mut manifest: serde_json::Value = serde_json::from_slice(
        generated
            .entries()
            .iter()
            .find(|e| e.path().as_str() == ".ibcmd-provenance/manifest.json")
            .unwrap()
            .bytes(),
    )
    .unwrap();
    manifest["generated"][path] = serde_json::Value::String(hash);
    let altered = SourceTree::new(
        generated
            .entries()
            .iter()
            .map(|e| {
                if e.path().as_str() == path {
                    SourceEntry::from_bytes(SourcePath::new(path).unwrap(), edited.clone()).unwrap()
                } else if e.path().as_str() == ".ibcmd-provenance/manifest.json" {
                    SourceEntry::from_bytes(
                        e.path().clone(),
                        serde_json::to_vec(&manifest).unwrap(),
                    )
                    .unwrap()
                } else {
                    e.clone()
                }
            })
            .collect(),
    )
    .unwrap();
    assert!(
        edt_to_xml(&Project::from_tree(altered).unwrap(), &options)
            .unwrap_err()
            .to_string()
            .contains("changed")
    );
}

#[test]
#[ignore = "Full genuine1108 typed attached first-route bodies vs nativeSDK on F"]
fn genuine_all_attached_bsp_forms_equal_sdk_after_projection() {
    use std::path::Path;
    let root = Path::new(r"F:\ibcmd\lab\07\accept-bsp83-a09-final-r1\ours-authentic-edt-xml");
    let sdk = Path::new(r"F:\ibcmd\lab\07\native-reference-bsp83-r2\native-xml");
    fn files(dir: &Path, out: &mut Vec<std::path::PathBuf>, depth: usize) {
        assert!(depth < 64);
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let t = entry.file_type().unwrap();
            assert!(!t.is_symlink());
            if t.is_dir() {
                files(&entry.path(), out, depth + 1)
            } else if entry.file_name() == "Form.xml" {
                out.push(entry.path());
                assert!(out.len() < 65536);
            }
        }
    }
    let mut paths = vec![];
    files(root, &mut paths, 0);
    paths.sort();
    assert_eq!(paths.len(), 1108);
    let mut differences = vec![];
    let mut changed = 0;
    for path in paths {
        let relative = path.strip_prefix(root).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.len() < 32 * 1024 * 1024);
        let mut form = with_source_version(Some(FormatVersion::new(2, 20)), || {
            read_form(FormDialect::Designer, &bytes)
        })
        .unwrap();
        assert!(
            native(&form, FormatVersion::new(2, 20)) == bytes,
            "source changed at {}",
            relative.display()
        );
        form.designer_path_spelling = false;
        let generated = native(&form, FormatVersion::new(2, 20));
        changed += usize::from(generated != bytes);
        let expected = std::fs::read(sdk.join(relative)).unwrap();
        if generated != expected {
            differences.push(relative.display().to_string());
        }
    }
    std::fs::write(r"F:\ibcmd\lab\07\bsp-all-availability-native-sdk-census.json",serde_json::to_vec_pretty(&serde_json::json!({"forms":1108,"source_passed":1108,"changed_files":changed,"differences":differences})).unwrap()).unwrap();
    assert!(differences.is_empty(), "SDK differences {differences:?}");
    assert_eq!(changed, 8);
}
