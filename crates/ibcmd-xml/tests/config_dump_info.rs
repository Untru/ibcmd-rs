use ibcmd_xml::config_dump_info::{ConfigDumpInfo, Entry, NativeVersion, Part};

#[test]
fn native_empty_inventory_bytes_match_both_measured_platform_dialects() {
    for (version, expected) in [
        (
            NativeVersion::V2_20,
            include_bytes!("../../../tests/fixtures/native-export-commands/empty-info-2.20.xml")
                .as_slice(),
        ),
        (
            NativeVersion::V2_21,
            include_bytes!("../../../tests/fixtures/native-export-commands/empty-info-2.21.xml")
                .as_slice(),
        ),
    ] {
        let document = ConfigDumpInfo::native(version, Vec::new());
        assert_eq!(document.to_native_bytes().unwrap(), expected);
        assert_eq!(ConfigDumpInfo::parse(expected).unwrap(), document);
        assert_eq!(ConfigDumpInfo::parse(&expected[3..]).unwrap(), document);
    }
}

#[test]
fn native_row_and_part_spelling_matches_real_import_captures() {
    for bytes in [
        include_bytes!(
            "../../../tests/fixtures/native-export-commands/change/full/ConfigDumpInfo.xml"
        )
        .as_slice(),
        include_bytes!(
            "../../../tests/fixtures/native-export-commands/add/full/ConfigDumpInfo.xml"
        )
        .as_slice(),
        include_bytes!(
            "../../../tests/fixtures/native-export-commands/delete/full/ConfigDumpInfo.xml"
        )
        .as_slice(),
        include_bytes!(
            "../../../tests/fixtures/native-export-commands/rename/full/ConfigDumpInfo.xml"
        )
        .as_slice(),
    ] {
        let parsed = ConfigDumpInfo::parse(bytes).unwrap();
        assert_eq!(parsed.to_native_bytes().unwrap(), bytes);
    }
}

#[test]
fn attribute_values_are_typed_unescaped_and_native_writer_escapes_once() {
    let document = ConfigDumpInfo::native(
        NativeVersion::V2_21,
        vec![Entry {
            name: "Catalog.А&B\"<>'".into(),
            id: "row&1".into(),
            config_version: "version&1".into(),
            children: vec![Part {
                name: "Catalog.А&B\"<>'.Attribute.С".into(),
                id: "part<1>".into(),
            }],
        }],
    );
    let bytes = document.to_native_bytes().unwrap();
    let xml = std::str::from_utf8(&bytes).unwrap();
    assert!(xml.contains("name=\"Catalog.А&amp;B&quot;&lt;&gt;'\""));
    assert!(xml.contains("configVersion=\"version&amp;1\""));
    assert_eq!(ConfigDumpInfo::parse(&bytes).unwrap(), document);
    assert!(!bytes.ends_with(b"\n"));
    assert!(bytes.starts_with(b"\xef\xbb\xbf"));
    assert!(!xml.replace("\r\n", "").contains('\n'));
}

#[test]
fn nested_reference_parts_keep_their_rows_and_full_names() {
    let xml = br#"<ConfigDumpInfo><ConfigVersions><Metadata name="Catalog.C" id="row" configVersion="v"><Metadata name="Catalog.C.TabularSection.T" id="part"><Metadata name="Catalog.C.TabularSection.T.Attribute.A" id="child"/></Metadata></Metadata></ConfigVersions></ConfigDumpInfo>"#;
    let document = ConfigDumpInfo::parse(xml).unwrap();
    assert_eq!(document.entries.len(), 1);
    assert_eq!(
        document.entries[0]
            .children
            .iter()
            .map(|p| p.id.as_str())
            .collect::<Vec<_>>(),
        ["part", "child"]
    );
}

#[test]
fn malformed_inventories_fail_closed_in_the_codec() {
    for xml in [
        "",
        "<Other/>",
        "<ConfigDumpInfo/>",
        "<ConfigDumpInfo><ConfigVersions/><ConfigVersions/></ConfigDumpInfo>",
        "<ConfigDumpInfo><ConfigVersions>",
        "<ConfigDumpInfo><ConfigVersions></ConfigDumpInfo>",
        "<ConfigDumpInfo><ConfigVersions><Metadata name='N' id='a'/></ConfigVersions></ConfigDumpInfo>",
        "<ConfigDumpInfo><ConfigVersions><Metadata name='N' id='a' id='b' configVersion='v'/></ConfigVersions></ConfigDumpInfo>",
        "<ConfigDumpInfo><ConfigVersions><Metadata name='N' id='a' configVersion='v'/><Metadata name='Other' id='a' configVersion='v'/></ConfigVersions></ConfigDumpInfo>",
        "<ConfigDumpInfo><ConfigVersions><Metadata name='N' id='a' configVersion='v'><Metadata name='part'/></Metadata></ConfigVersions></ConfigDumpInfo>",
        "<ConfigDumpInfo><ConfigVersions/>unexpected</ConfigDumpInfo>",
        "<ConfigDumpInfo><ConfigVersions/><Other/></ConfigDumpInfo>",
        "<ConfigDumpInfo><ConfigVersions/></ConfigDumpInfo><ConfigDumpInfo><ConfigVersions/></ConfigDumpInfo>",
        "<!DOCTYPE ConfigDumpInfo><ConfigDumpInfo><ConfigVersions/></ConfigDumpInfo>",
        "<ConfigDumpInfo><ConfigVersions/><?xml version='1.0'?></ConfigDumpInfo>",
        "<ConfigDumpInfo><ConfigVersions><Metadata name='&unknown;' id='a' configVersion='v'/></ConfigVersions></ConfigDumpInfo>",
        "<ConfigDumpInfo><ConfigVersions><Metadata name='&#1;' id='a' configVersion='v'/></ConfigVersions></ConfigDumpInfo>",
    ] {
        assert!(
            ConfigDumpInfo::parse(xml.as_bytes()).is_err(),
            "accepted {xml}"
        );
    }
}

#[test]
fn lexical_xml_errors_cannot_hide_inside_comments_or_declarations() {
    for xml in [
        b"<ConfigDumpInfo><!-- a -- b --><ConfigVersions/></ConfigDumpInfo>".as_slice(),
        b"<ConfigDumpInfo><!-- \xff --><ConfigVersions/></ConfigDumpInfo>".as_slice(),
        b"<ConfigDumpInfo><!-- \x01 --><ConfigVersions/></ConfigDumpInfo>".as_slice(),
        b"<?xml version='1.0' unknown='x'?><ConfigDumpInfo><ConfigVersions/></ConfigDumpInfo>"
            .as_slice(),
        b"<?xml version='1.0' version='1.0'?><ConfigDumpInfo><ConfigVersions/></ConfigDumpInfo>"
            .as_slice(),
        b" <?xml version='1.0'?><ConfigDumpInfo><ConfigVersions/></ConfigDumpInfo>".as_slice(),
        b"<ConfigDumpInfo bad:name:extra='x'><ConfigVersions/></ConfigDumpInfo>".as_slice(),
    ] {
        assert!(ConfigDumpInfo::parse(xml).is_err());
    }
    assert!(ConfigDumpInfo::parse(b"<?xml version='1.0'?><!-- valid --><ConfigDumpInfo><ConfigVersions/></ConfigDumpInfo>").is_ok());
}

#[test]
fn native_serialization_refuses_unknown_dialects_invalid_characters_and_duplicate_rows() {
    let mut document = ConfigDumpInfo::native(NativeVersion::V2_20, Vec::new());
    document.format_version = Some("future".into());
    assert!(document.to_native_bytes().is_err());
    let entry = Entry {
        name: "Catalog.N".into(),
        id: "r".into(),
        config_version: "v".into(),
        children: Vec::new(),
    };
    let duplicate =
        ConfigDumpInfo::native(NativeVersion::V2_20, vec![entry.clone(), entry.clone()]);
    assert!(duplicate.to_native_bytes().is_err());
    let mut bad = entry;
    bad.name.push('\u{1}');
    assert!(
        ConfigDumpInfo::native(NativeVersion::V2_20, vec![bad])
            .to_native_bytes()
            .is_err()
    );
}

#[test]
#[ignore = "optional private read-only inventory: set IBCMD_RS_NATIVE_STATUS_PRIVATE_XML"]
fn private_native_inventory_roundtrips_without_lexical_normalization() {
    let path = std::env::var_os("IBCMD_RS_NATIVE_STATUS_PRIVATE_XML")
        .expect("explicit private inventory path");
    let bytes = std::fs::read(path).unwrap();
    let document = ConfigDumpInfo::parse(&bytes).unwrap();
    assert_eq!(document.to_native_bytes().unwrap(), bytes);
    println!(
        "version_entries={} bytes={}",
        document.entries.len(),
        bytes.len()
    );
}
