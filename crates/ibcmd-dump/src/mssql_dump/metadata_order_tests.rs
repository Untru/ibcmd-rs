use super::{
    ConfigurationContainedObject, GeneratedTypeEntry,
    format_schema_ordered_generated_types_internal_info_xml,
    insert_configuration_internal_info_xml,
};

fn generated(category: &'static str) -> GeneratedTypeEntry {
    GeneratedTypeEntry {
        name: format!("Catalog{category}.Test"),
        category,
        type_id: format!("type-{category}"),
        value_id: format!("value-{category}"),
    }
}

#[test]
fn catalog_writer_uses_bundled_produced_types_order() {
    let xml = format_schema_ordered_generated_types_internal_info_xml(
        "CATALOG_TYPES",
        &[
            generated("Manager"),
            generated("Selection"),
            generated("Object"),
            generated("List"),
            generated("Ref"),
        ],
    )
    .unwrap();
    let positions = ["Object", "Ref", "Selection", "List", "Manager"].map(|category| {
        xml.find(&format!("category=\"{category}\""))
            .expect("generated type category")
    });
    assert!(positions.windows(2).all(|window| window[0] < window[1]));
}

#[test]
fn document_writer_uses_the_same_verified_base_type_sequence() {
    let xml = format_schema_ordered_generated_types_internal_info_xml(
        "DOCUMENT_TYPES",
        &[
            generated("List"),
            generated("Ref"),
            generated("Manager"),
            generated("Object"),
            generated("Selection"),
        ],
    )
    .unwrap();
    let positions = ["Object", "Ref", "Selection", "List", "Manager"].map(|category| {
        xml.find(&format!("category=\"{category}\""))
            .expect("generated type category")
    });
    assert!(positions.windows(2).all(|window| window[0] < window[1]));
}

#[test]
fn configuration_internal_info_uses_verified_section_rule() {
    let mut xml =
        "<Configuration>\r\n\t\t<Properties>\r\n\t\t</Properties>\r\n</Configuration>".to_owned();
    insert_configuration_internal_info_xml(
        &mut xml,
        &[ConfigurationContainedObject {
            class_id: "class".to_owned(),
            object_id: "object".to_owned(),
        }],
    )
    .unwrap();
    assert!(
        xml.find("<InternalInfo>").unwrap() < xml.find("<Properties>").unwrap(),
        "verified InternalInfo section must precede Properties"
    );
    assert!(xml.contains("<xr:ContainedObject>"));
}

#[test]
fn unknown_generated_type_category_fails_without_emitting_guessed_xml() {
    let result = format_schema_ordered_generated_types_internal_info_xml(
        "CATALOG_TYPES",
        &[generated("Invented")],
    );
    assert!(matches!(
        result,
        Err(ibcmd_xml::MetadataOrderError::AmbiguousProducedType { .. })
    ));
}
