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

const DCS_GENERATED_NAME: &str = "CatalogRef.FilterProbe";
const DCS_GENERATED_ID: &str = "488c0ffa-ef24-480c-a420-3bd2736317f9";

fn original_generated_dcs_source(name: &str) -> Vec<u8> {
    let xml = decode_base64_mime(include_str!(
        "../tests/fixtures/native-evidence/8.3.27.2214/dcs-typeid-reference/native-template.xml.b64"
    ))
    .unwrap();
    String::from_utf8(xml)
        .unwrap()
        .replace(DCS_GENERATED_NAME, name)
        .into_bytes()
}

fn original_generated_metadata(version: &str, name: &str, type_id: &str) -> String {
    let (kind, object_name) = if name == "DocumentTabularSection.Invoice.Rows" {
        ("Document", "Invoice")
    } else {
        ("Catalog", "FilterProbe")
    };
    metadata(kind, object_name, OBJECT, version, "").replace(
        "<Properties>",
        &format!(
            "<InternalInfo><GeneratedType name=\"{name}\"><TypeId>{type_id}</TypeId></GeneratedType></InternalInfo><Properties>"
        ),
    )
}

fn assert_original_generated_dcs_payload(source: &MetadataSourceContext, name: &str) -> Vec<u8> {
    use crate::compiler::bodies::dcs::{DcsCodecProfile, DcsTemplateKind, decode_dcs};
    use crate::compiler::bodies::template::{
        TemplateKind, TemplateSource, compile_evidenced_template_with_resolvers,
    };
    let xml = original_generated_dcs_source(name);
    let blob = crate::mssql::compile_dcs_template_body(&xml, Some(source)).unwrap();
    let resolver = |requested: &str| (requested == name).then(|| DCS_GENERATED_ID.to_owned());
    let expected = compile_evidenced_template_with_resolvers(
        TemplateKind::DataCompositionSchema,
        TemplateSource::Bytes(&xml),
        &BTreeMap::new(),
        &resolver,
    )
    .unwrap();
    assert_eq!(
        blob, expected,
        "complete DCS payload, not only lookup success"
    );
    let body = decode_dcs(&DcsCodecProfile::fixture(), DcsTemplateKind::Schema, &blob).unwrap();
    assert!(
        std::str::from_utf8(body.plaintext())
            .unwrap()
            .contains(DCS_GENERATED_ID)
    );
    let types = BTreeMap::from([(
        DCS_GENERATED_ID.to_owned(),
        crate::mssql_dump::DcsTypeResolution::Type {
            qname: format!("cfg:{name}"),
        },
    )]);
    let exported =
        crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &types,
            &BTreeMap::new(),
            &ibcmd_core::artifact::ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ibcmd_core::artifact::ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
    assert_eq!(exported, xml, "complete source exportback");
    source.require_original_reads().unwrap();
    source.require_original_unchanged().unwrap();
    blob
}

#[test]
fn original_generated_type_actual_dcs_payload_and_exportback_both_metadata_editions() {
    for version in ["2.20", "2.21"] {
        let f = Fixture::new();
        f.write(
            "Catalogs/FilterProbe.xml",
            original_generated_metadata(version, DCS_GENERATED_NAME, DCS_GENERATED_ID),
        );
        let source = f.context();
        assert_original_generated_dcs_payload(&source, DCS_GENERATED_NAME);
        assert_eq!(
            source.dcs_generated_type_id(DCS_GENERATED_NAME).as_deref(),
            Some(DCS_GENERATED_ID)
        );
    }
}

#[test]
fn original_generated_type_malformed_selected_xml_cannot_be_a_literal_dcs_success() {
    let xml = original_generated_dcs_source(DCS_GENERATED_NAME);
    for version in ["2.20", "2.21"] {
        let valid = original_generated_metadata(version, DCS_GENERATED_NAME, DCS_GENERATED_ID);
        let positive = Fixture::new();
        positive.write("Catalogs/FilterProbe.xml", &valid);
        assert_original_generated_dcs_payload(&positive.context(), DCS_GENERATED_NAME);
        for malformed in [
            valid.replace("</MetaDataObject>", ""),
            valid.replace("</TypeId>", "</Wrong>"),
            valid.replace(DCS_GENERATED_ID, "not-a-generated-uuid"),
            valid.replace(DCS_GENERATED_ID, ""),
            valid.replace(&format!("<TypeId>{DCS_GENERATED_ID}</TypeId>"), "<TypeId/>"),
            valid.replace(DCS_GENERATED_ID, "&unknown;"),
            valid.replace(DCS_GENERATED_NAME, "CatalogRef.Filter&unknown;"),
            format!("{valid}<Extra/>"),
        ] {
            let f = Fixture::new();
            f.write("Catalogs/FilterProbe.xml", malformed);
            let source = f.context();
            // The actual compiler invokes the actual resolver; no preparatory
            // missing read, mocked parser or pre-poisoned owner is involved.
            assert!(crate::mssql::compile_dcs_template_body(&xml, Some(&source)).is_err());
            assert!(source.require_original_reads().is_err());
            assert_eq!(source.dcs_generated_type_id(DCS_GENERATED_NAME), None);
            assert!(crate::mssql::compile_dcs_template_body(&xml, Some(&source)).is_err());
        }
    }
}

#[test]
fn original_generated_type_invalid_utf8_is_sticky_at_actual_dcs_gate() {
    let f = Fixture::new();
    let mut metadata =
        original_generated_metadata("2.20", DCS_GENERATED_NAME, DCS_GENERATED_ID).into_bytes();
    let at = metadata.iter().position(|byte| *byte == b'F').unwrap();
    metadata[at] = 0xff;
    f.write("Catalogs/FilterProbe.xml", metadata);
    let source = f.context();
    assert!(
        crate::mssql::compile_dcs_template_body(
            &original_generated_dcs_source(DCS_GENERATED_NAME),
            Some(&source)
        )
        .is_err()
    );
    assert!(source.require_original_reads().is_err());
}

#[test]
fn original_generated_type_valid_cdata_and_numeric_entity_use_same_parser() {
    for type_id in [
        format!("<![CDATA[{DCS_GENERATED_ID}]]>"),
        DCS_GENERATED_ID.replacen('4', "&#52;", 1),
    ] {
        let f = Fixture::new();
        f.write(
            "Catalogs/FilterProbe.xml",
            original_generated_metadata("2.21", DCS_GENERATED_NAME, &type_id),
        );
        assert_original_generated_dcs_payload(&f.context(), DCS_GENERATED_NAME);
    }
}

#[test]
fn original_generated_type_genuine_missing_and_unresolved_remain_literal_dcs_references() {
    for version in ["2.20", "2.21"] {
        for present in [false, true] {
            let f = Fixture::new();
            if present {
                f.write(
                    "Catalogs/FilterProbe.xml",
                    metadata("Catalog", "FilterProbe", OBJECT, version, ""),
                );
            }
            let source = f.context();
            let xml = original_generated_dcs_source(DCS_GENERATED_NAME);
            let expected = crate::mssql::compile_dcs_template_body(&xml, None).unwrap();
            assert_eq!(
                crate::mssql::compile_dcs_template_body(&xml, Some(&source)).unwrap(),
                expected
            );
            assert_eq!(source.dcs_generated_type_id(DCS_GENERATED_NAME), None);
            source.require_original_reads().unwrap();
            source.require_original_unchanged().unwrap();
        }
    }
}

#[test]
fn original_generated_type_unsupported_names_do_not_read_or_poison_optional_files() {
    let f = Fixture::new();
    let source = f.context();
    for name in ["UnmeasuredType.Probe", "DefinedType.Probe"] {
        let xml = original_generated_dcs_source(name);
        let expected = crate::mssql::compile_dcs_template_body(&xml, None).unwrap();
        assert_eq!(
            crate::mssql::compile_dcs_template_body(&xml, Some(&source)).unwrap(),
            expected
        );
        assert_eq!(source.dcs_generated_type_id(name), None);
        source.require_original_reads().unwrap();
    }
}

#[test]
fn original_generated_tabular_type_owner_and_dotted_alternate_are_both_supported() {
    let name = "DocumentTabularSection.Invoice.Rows";
    for version in ["2.20", "2.21"] {
        for (owner_has_type, owner_present) in [(true, true), (false, true), (false, false)] {
            let f = Fixture::new();
            if owner_present {
                let owner = if owner_has_type {
                    original_generated_metadata(version, name, DCS_GENERATED_ID)
                } else {
                    metadata("Document", "Invoice", OBJECT, version, "")
                };
                f.write("Documents/Invoice.xml", owner);
            }
            if !owner_has_type {
                f.write(
                    "Documents/Invoice.Rows.xml",
                    original_generated_metadata(version, name, DCS_GENERATED_ID),
                );
            }
            assert_original_generated_dcs_payload(&f.context(), name);
        }
    }
}

#[test]
fn original_generated_tabular_selected_malformed_owner_cannot_hide_behind_alternate() {
    let name = "DocumentTabularSection.Invoice.Rows";
    let f = Fixture::new();
    f.write(
        "Documents/Invoice.xml",
        original_generated_metadata("2.20", name, "invalid-uuid"),
    );
    f.write(
        "Documents/Invoice.Rows.xml",
        original_generated_metadata("2.20", name, DCS_GENERATED_ID),
    );
    let source = f.context();
    assert!(
        crate::mssql::compile_dcs_template_body(
            &original_generated_dcs_source(name),
            Some(&source)
        )
        .is_err()
    );
    assert!(source.require_original_reads().is_err());
}

#[test]
fn legacy_unbound_generated_type_optional_absence_and_error_projection_are_preserved() {
    let f = Fixture::new();
    let xml = original_generated_dcs_source(DCS_GENERATED_NAME);
    let expected = crate::mssql::compile_dcs_template_body(&xml, None).unwrap();
    let missing = MetadataSourceContext::new(f.root.clone());
    assert_eq!(
        crate::mssql::compile_dcs_template_body(&xml, Some(&missing)).unwrap(),
        expected
    );
    f.write(
        "Catalogs/FilterProbe.xml",
        original_generated_metadata("2.20", DCS_GENERATED_NAME, "invalid-uuid"),
    );
    let malformed = MetadataSourceContext::new(f.root.clone());
    assert!(
        malformed
            .resolve_metadata_type_id(DCS_GENERATED_NAME)
            .is_err()
    );
    assert_eq!(
        crate::mssql::compile_dcs_template_body(&xml, Some(&malformed)).unwrap(),
        expected
    );
    malformed.require_original_reads().unwrap();
}

#[test]
fn legacy_generated_parser_outcome_is_distinct_from_selected_original_complete_eof() {
    let f = Fixture::new();
    let incomplete = original_generated_metadata("2.20", DCS_GENERATED_NAME, DCS_GENERATED_ID)
        .replace("</MetaDataObject>", "");
    // The compatibility caller keeps its old parser outcome. This does not
    // authorize accepting the same incomplete selected original for DCS.
    assert_eq!(
        parse_generated_type_type_id(incomplete.as_bytes(), DCS_GENERATED_NAME).unwrap(),
        DCS_GENERATED_ID
    );
    f.write("Catalogs/FilterProbe.xml", incomplete);
    let source = f.context();
    assert!(
        crate::mssql::compile_dcs_template_body(
            &original_generated_dcs_source(DCS_GENERATED_NAME),
            Some(&source)
        )
        .is_err()
    );
    assert!(source.require_original_reads().is_err());
}
