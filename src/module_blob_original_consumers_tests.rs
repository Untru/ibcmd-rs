use super::*;
use crate::mssql::source_original_consumers_tests::{Fixture, OBJECT, metadata};
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
#[ignore = "ROOT: run alone in a fresh process without explicit constant override; no database/native actor"]
fn original_constants_both_actual_form_callbacks_are_operation_local() {
    assert!(std::env::var_os("IBCMD_RS_ALWAYS_USED_CONSTANTS").is_none());
    let f = Fixture::new();
    f.write(
        "Constants/Probe.xml",
        metadata("Constant", "Probe", OBJECT, "2.20", ""),
    );
    let first = f.context();
    let second = f.context();
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    first.install_original_constants(move || {
        count.fetch_add(1, Ordering::Relaxed);
        Ok(vec![OBJECT.into()])
    });
    second.install_original_constants(|| Ok(Vec::new()));
    for version in ["2.20", "2.21"] {
        let xml = format!(
            "<Form xmlns=\"http://v8.1c.ru/8.3/xcf/logform\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" version=\"{version}\"><Attributes><Attribute name=\"Set\" id=\"1\"><Type><v8:Type>cfg:ConstantsSet</v8:Type></Type></Attribute></Attributes></Form>"
        );
        let absent = parse_form_xml_body_properties(xml.as_bytes()).unwrap();
        let explicit_xml = xml.replace(
            "</Attribute>",
            "<UseAlways><Field>Set.Probe</Field></UseAlways></Attribute>",
        );
        let explicit = parse_form_xml_body_properties(explicit_xml.as_bytes()).unwrap();
        let absent_paths = NativeDataPaths {
            form: native_data_path_form(&absent, &BTreeMap::new()),
            source: Some(&first),
        };
        let explicit_paths = NativeDataPaths {
            form: native_data_path_form(&explicit, &BTreeMap::new()),
            source: Some(&first),
        };
        let flagged =
            native_form_attribute_use_always(&absent.attributes[0], &absent_paths).unwrap();
        assert!(flagged.contains(OBJECT));
        assert_eq!(
            native_form_attribute_use_always(&explicit.attributes[0], &explicit_paths).unwrap(),
            "{0,0}"
        );
        let clear_paths = NativeDataPaths {
            form: native_data_path_form(&absent, &BTreeMap::new()),
            source: Some(&second),
        };
        assert_eq!(
            native_form_attribute_use_always(&absent.attributes[0], &clear_paths).unwrap(),
            "{0,0}"
        );
        let explicit_clear = NativeDataPaths {
            form: native_data_path_form(&explicit, &BTreeMap::new()),
            source: Some(&second),
        };
        assert!(
            native_form_attribute_use_always(&explicit.attributes[0], &explicit_clear)
                .unwrap()
                .contains(OBJECT)
        );
    }
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    first.require_original_unchanged().unwrap();
    second.require_original_unchanged().unwrap();
}

#[test]
fn original_form_assets_listing_uses_exact_census_spelling_and_never_path_fallback() {
    let f = Fixture::new();
    let image = f.write(
        "Forms/F/Ext/Form/Items/Picture/image.png",
        b"retained picture",
    );
    let context = f.context();
    let directory = f.root.join("Forms/F/Ext/Form/Items/Picture");
    assert_eq!(context.source_files(&directory).unwrap(), [image.clone()]);
    assert_eq!(
        context
            .source_descendant_files(&f.root.join("Forms/F/Ext/Form/Items"))
            .unwrap(),
        [image.clone()]
    );
    assert_eq!(&*context.read_source(&image).unwrap(), b"retained picture");
    assert!(
        context
            .source_files(&f.root.join("forms/F/Ext/Form/Items/Picture"))
            .is_err()
    );
    assert!(
        context
            .source_directory_exists(&f.root.join("Missing"))
            .is_ok_and(|present| !present)
    );
    assert!(
        context
            .read_source(&f.root.join("Forms/F/Ext/Form/Items/Picture/missing.png"))
            .is_err()
    );
    assert!(context.require_original_reads().is_err());
}

#[test]
fn original_form_asset_fallback_preserves_unique_stem_and_refuses_ambiguity() {
    let f = Fixture::new();
    let png = f.write("Items/A/image.png", b"PNG");
    let source = f.context();
    let directory = f.root.join("Items");
    assert_eq!(
        resolve_form_item_asset_path_with_source(&directory, "A", "image.jpg", Some(&source))
            .unwrap(),
        Some(png)
    );
    drop(source);
    f.write("Items/A/image.bmp", b"BMP");
    let source = f.context();
    assert!(
        resolve_form_item_asset_path_with_source(&directory, "A", "image.jpg", Some(&source))
            .is_err()
    );
}

#[test]
fn original_option_adapter_failure_is_sticky_before_real_dcs_blob_success() {
    let f = Fixture::new();
    let style = "4a9d8536-ff59-4a90-a1cf-646d241dc53c";
    f.write(
        "StyleItems/CorpusAccent.xml",
        metadata("StyleItem", "CorpusAccent", style, "2.20", ""),
    );
    let source = f.context();
    assert_eq!(
        source.dcs_style_items().get(style).map(String::as_str),
        Some("CorpusAccent")
    );
    source.require_original_reads().unwrap();
    let xml = decode_base64_mime(include_str!("../tests/fixtures/native-evidence/8.3.27.2214/dcs-area-style-item-uuid/native-template.xml.b64")).unwrap();
    let accepted = crate::mssql::compile_dcs_template_body(&xml, Some(&source)).unwrap();
    let expected = crate::compiler::bodies::template::compile_evidenced_template_with_references(
        crate::compiler::bodies::template::TemplateKind::DataCompositionSchema,
        crate::compiler::bodies::template::TemplateSource::Bytes(&xml),
        &BTreeMap::from([(style.to_owned(), "CorpusAccent".to_owned())]),
    )
    .unwrap();
    assert_eq!(accepted, expected);
    let isolated = f.context();
    assert!(isolated.read_source(&f.root.join("Missing.xml")).is_err());
    assert!(crate::mssql::compile_dcs_template_body(&xml, Some(&isolated)).is_err());
    assert!(isolated.require_original_reads().is_err());
}
