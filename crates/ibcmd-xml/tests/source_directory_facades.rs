use ibcmd_xml::{
    XmlReader,
    source_tree::{
        SourceKind, SourcePath, classify_source_path, inspect_source_uuid,
        validate_source_path_safety,
    },
};
use std::io::{BufReader, Cursor, Seek, SeekFrom};

#[test]
fn path_safety_and_resource_policy_are_separate() {
    let component = "Имя".repeat(100);
    let long = format!(
        "{}/{component}.xml",
        "safe/".repeat(90).trim_end_matches('/')
    );
    assert!(validate_source_path_safety(&long).is_ok());
    assert!(SourcePath::new(&long).is_err());
    for bad in [
        "../file.xml",
        "one/../../file.xml",
        "/root/file",
        r"C:\root\file",
        r"\\server\share\file",
        "one//file",
        "one/CON.txt",
        "one/NUL",
        "one/file.",
        "one/file ",
        "one/.git/config",
        "one/target/file",
        "one/name:stream",
        "one/../file",
        "one/na\0me",
    ] {
        assert!(validate_source_path_safety(bad).is_err(), "{bad:?}");
    }
    assert_eq!(
        classify_source_path("Configuration.xml"),
        SourceKind::ConfigurationRoot
    );
    assert_eq!(
        classify_source_path("Catalogs/Test/Ext/Module.bsl"),
        SourceKind::Module
    );
    assert_eq!(
        classify_source_path("Catalogs/Test/Forms/Main/Ext/Form.xml"),
        SourceKind::Form
    );
}

#[test]
fn streamed_uuid_inspection_validates_the_complete_source_before_early_identity() {
    let uuid = "11111111-1111-1111-1111-111111111111";
    let root = format!(r#"<MetaDataObject uuid="{uuid}"><Catalog/></MetaDataObject>"#);
    let child = format!(r#"<MetaDataObject><Catalog uuid="{uuid}"/></MetaDataObject>"#);
    let long_path = format!("{}/Catalog.xml", "safe/".repeat(90).trim_end_matches('/'));
    for source in [&root, &child] {
        for capacity in [1, 2, 3, 7, 65_536] {
            let actual = inspect_source_uuid(
                &long_path,
                BufReader::with_capacity(capacity, Cursor::new(source.as_bytes())),
            )
            .unwrap()
            .unwrap();
            assert_eq!(actual.to_string(), uuid);
        }
    }
    for source in [
        format!("{root}<extra/>"),
        root.replace("</MetaDataObject>", "\0</MetaDataObject>"),
        root.replace("<Catalog/>", "<Catalog>"),
        child.replace(
            "/></MetaDataObject>",
            &format!(r#"/><Other uuid="{uuid}"/></MetaDataObject>"#),
        ),
        root.replace(uuid, "invalid"),
    ] {
        assert!(inspect_source_uuid("Catalog.xml", Cursor::new(source)).is_err());
    }
}

#[test]
fn seekable_inspection_starts_at_current_position_without_buffering_the_prefix() {
    let mut input = Cursor::new(b"not XML\xef\xbb\xbf<?xml version=\"1.0\"?><root/>");
    input.seek(SeekFrom::Start(7)).unwrap();
    assert_eq!(XmlReader::inspect_reader(input).unwrap().raw(), "root");
    let mut input = Cursor::new(b"ignored<root>\n<bad></root>");
    input.seek(SeekFrom::Start(7)).unwrap();
    let actual = XmlReader::inspect_reader(input).unwrap_err();
    let expected = XmlReader::from_slice(b"<root>\n<bad></root>").unwrap_err();
    assert_eq!(actual, expected);
}
