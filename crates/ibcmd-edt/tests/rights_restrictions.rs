//! Multiple restrictions on one right remain ordered, editable current values.
use formats_xml::registry::SidecarFormat;
use formats_xml::rights::{read, write};
use morph1c_core::version::{ERP, SSL};

fn edt_sample_with_restriction() -> Vec<u8> {
    concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
        "<Rights xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns=\"http://v8.1c.ru/8.2/roles\" xsi:type=\"Rights\">\r\n",
        "\t<setForNewObjects>false</setForNewObjects>\r\n",
        "\t<setForAttributesByDefault>true</setForAttributesByDefault>\r\n",
        "\t<independentRightsOfChildObjects>false</independentRightsOfChildObjects>\r\n",
        "\t<object>\r\n\t\t<name>Catalog.Example</name>\r\n",
        "\t\t<right>\r\n\t\t\t<name>Read</name>\r\n\t\t\t<value>true</value>\r\n",
        "\t\t\t<restrictionByCondition>\r\n\t\t\t\t<field>Ссылка</field>\r\n",
        "\t\t\t\t<condition>FIRST &amp;X\r\nOR Y</condition>\r\n",
        "\t\t\t</restrictionByCondition>\r\n\t\t</right>\r\n",
        "\t</object>\r\n",
        "\t<restrictionTemplate>\r\n\t\t<name>ExampleTemplate</name>\r\n",
        "\t\t<condition>TEMPLATE\r\nNEXT</condition>\r\n\t</restrictionTemplate>\r\n",
        "</Rights>\r\n"
    ).as_bytes().to_vec()
}

// Independently authored input: repeated fields, an absent field and an
// explicitly empty field are distinct ordered restrictions on one right.
fn edt_sample_with_multiple_restrictions() -> Vec<u8> {
    let source = String::from_utf8(edt_sample_with_restriction()).unwrap();
    let extra = concat!(
        "\t\t\t<restrictionByCondition>\r\n",
        "\t\t\t\t<field>Ссылка</field>\r\n",
        "\t\t\t\t<condition>SECOND &amp;VALUE\r\nOR OTHER</condition>\r\n",
        "\t\t\t</restrictionByCondition>\r\n",
        "\t\t\t<restrictionByCondition>\r\n",
        "\t\t\t\t<condition></condition>\r\n",
        "\t\t\t</restrictionByCondition>\r\n",
        "\t\t\t<restrictionByCondition>\r\n",
        "\t\t\t\t<field></field>\r\n",
        "\t\t\t\t<condition>FOURTH</condition>\r\n",
        "\t\t\t</restrictionByCondition>\r\n",
        "\t\t</right>\r\n"
    );
    source.replacen("\t\t</right>\r\n", extra, 1).into_bytes()
}

#[test]
fn multiple_restrictions_preserve_order_duplicates_and_optional_fields() {
    let source = edt_sample_with_multiple_restrictions();
    let table = read(&source, SidecarFormat::EdtRights).unwrap();
    let restrictions = &table.objects[0].rights[0].restrictions;
    assert_eq!(restrictions.len(), 4);
    assert_eq!(restrictions[0].field.as_deref(), Some("Ссылка"));
    assert_eq!(restrictions[1].field.as_deref(), Some("Ссылка"));
    assert_eq!(restrictions[1].condition, "SECOND &amp;VALUE\r\nOR OTHER");
    assert_eq!(restrictions[2].field, None);
    assert_eq!(restrictions[2].condition, "");
    assert_eq!(restrictions[3].field.as_deref(), Some(""));
    assert_eq!(restrictions[3].condition, "FOURTH");
    assert_eq!(write(&table, SidecarFormat::EdtRights).unwrap(), source);
    for version in [ERP, SSL] {
        let designer = morph1c_core::version::with_roundtrip_target(version, || {
            write(&table, SidecarFormat::DesignerRights)
        })
        .unwrap();
        let restored = read(&designer, SidecarFormat::DesignerRights).unwrap();
        assert_eq!(restored, table);
        assert_eq!(write(&restored, SidecarFormat::EdtRights).unwrap(), source);
        assert_eq!(
            morph1c_core::version::with_roundtrip_target(version, || {
                write(&restored, SidecarFormat::DesignerRights)
            })
            .unwrap(),
            designer
        );
    }
}

#[test]
fn multiple_restrictions_emit_current_values_and_current_order() {
    let source = edt_sample_with_multiple_restrictions();
    let mut table = read(&source, SidecarFormat::EdtRights).unwrap();
    let original = table.objects[0].rights[0].restrictions.clone();
    table.objects[0].rights[0].restrictions[1].condition = "CHANGED &lt;VALUE\r\nNEXT".into();
    table.objects[0].rights[0].restrictions.swap(0, 3);
    for (format, version) in [
        (SidecarFormat::EdtRights, ERP),
        (SidecarFormat::DesignerRights, ERP),
        (SidecarFormat::DesignerRights, SSL),
    ] {
        let output =
            morph1c_core::version::with_roundtrip_target(version, || write(&table, format))
                .unwrap();
        let restored = read(&output, format).unwrap();
        assert_eq!(restored, table);
        let restrictions = &restored.objects[0].rights[0].restrictions;
        assert_eq!(restrictions[0], original[3]);
        assert_eq!(restrictions[3], original[0]);
        assert_eq!(restrictions[2], original[2]);
        assert_eq!(restrictions[1].condition, "CHANGED &lt;VALUE\r\nNEXT");
        assert!(
            !String::from_utf8(output)
                .unwrap()
                .contains("SECOND &amp;VALUE")
        );
    }
}

#[test]
fn multiple_restrictions_still_reject_unknown_and_malformed_blocks() {
    let source = String::from_utf8(edt_sample_with_multiple_restrictions()).unwrap();
    for malformed in [
        source.replacen("SECOND &amp;VALUE", "<unexpected/>", 1),
        source.replacen("\t\t\t\t<condition>SECOND", "\t\t\t\t<unknown>SECOND", 1),
        source.replacen("\t\t\t</restrictionByCondition>\r\n", "", 1),
        source.replacen(
            "\t\t</right>\r\n",
            "\t\t\t<unexpected/>\r\n\t\t</right>\r\n",
            1,
        ),
    ] {
        assert!(read(malformed.as_bytes(), SidecarFormat::EdtRights).is_err());
    }
}

#[test]
fn restrictions_append_delete_and_field_presence_use_current_model() {
    use morph1c_core::spec::metadata::role::RightRestriction;
    let mut table = read(
        &edt_sample_with_multiple_restrictions(),
        SidecarFormat::EdtRights,
    )
    .unwrap();
    let before = serde_json::to_vec(&table).unwrap();
    let restrictions = &mut table.objects[0].rights[0].restrictions;
    restrictions[0].field = None;
    restrictions[2].field = Some("CURRENT_FIELD".into());
    restrictions.push(RightRestriction {
        field: Some("".into()),
        condition: "APPENDED &gt;VALUE".into(),
    });
    restrictions.remove(1);
    assert_ne!(serde_json::to_vec(&table).unwrap(), before);
    for format in [SidecarFormat::EdtRights, SidecarFormat::DesignerRights] {
        let output = write(&table, format).unwrap();
        let restored = read(&output, format).unwrap();
        assert_eq!(restored, table);
        let restrictions = &restored.objects[0].rights[0].restrictions;
        assert_eq!(restrictions.len(), 4);
        assert_eq!(restrictions[0].field, None);
        assert_eq!(restrictions[1].field.as_deref(), Some("CURRENT_FIELD"));
        assert_eq!(restrictions[3].field.as_deref(), Some(""));
        assert_eq!(restrictions[3].condition, "APPENDED &gt;VALUE");
        assert!(
            !String::from_utf8(output)
                .unwrap()
                .contains("SECOND &amp;VALUE")
        );
    }
    table.objects[0].rights[0].restrictions.clear();
    for format in [SidecarFormat::EdtRights, SidecarFormat::DesignerRights] {
        let output = write(&table, format).unwrap();
        assert!(
            !String::from_utf8(output.clone())
                .unwrap()
                .contains("restrictionByCondition")
        );
        assert_eq!(read(&output, format).unwrap(), table);
    }
}
