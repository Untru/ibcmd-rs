use morph1c_core::{
    ir::{MetadataObject, ObjectKind, PropertyValue, Token, Uuid},
    spec::metadata::{web_service as ws, web_service_operation as op},
    version::{FormatVersion, with_roundtrip_target},
};
use morph1c_pipeline::{Format, registry::FormatRegistry};

fn read(format: Format, kind: &str, bytes: &[u8]) -> Result<MetadataObject, String> {
    (FormatRegistry::for_format(format)
        .unwrap()
        .get(kind)
        .unwrap()
        .read)(bytes)
}
fn write(format: Format, object: &MetadataObject, version: FormatVersion) -> Vec<u8> {
    let registry = FormatRegistry::for_format(format).unwrap();
    with_roundtrip_target(version, || {
        (registry.get(object.kind.as_str()).unwrap().write)(object)
    })
    .unwrap()
}
fn same_descriptor_children(left: &MetadataObject, right: &MetadataObject) -> bool {
    left.children.len() == right.children.len()
        && left.children.iter().zip(&right.children).all(|(a, b)| {
            a.kind == b.kind && a.name == b.name
                // Designer FormRef carries only a name. Its descriptor loads
                // UUID and properties in the whole-configuration reader.
                && (a.kind.as_str() == "FilterCriterion.FormRef"
                    || (a.uuid == b.uuid && a.properties == b.properties && a.children == b.children))
        })
}
fn web_service(mode: Option<&str>) -> MetadataObject {
    let mut object = MetadataObject::new(ObjectKind::new("WebService"), "Sparse", Uuid([1; 16]));
    object.properties = vec![
        (ws::F_NAMESPACE, PropertyValue::Str("urn:test".into())),
        (
            ws::F_DESCRIPTOR_FILE_NAME,
            PropertyValue::Str("sparse.1cws".into()),
        ),
        (ws::F_SESSION_MAX_AGE, PropertyValue::Int(20)),
    ];
    let mut child = MetadataObject::new(
        ObjectKind::new("WebService.Operation"),
        "Invoke",
        Uuid([2; 16]),
    );
    child.properties = vec![
        (
            op::F_XDTO_RETURNING_VALUE_TYPE,
            PropertyValue::List(vec![
                PropertyValue::Str("string".into()),
                PropertyValue::Str("http://www.w3.org/2001/XMLSchema".into()),
            ]),
        ),
        (op::F_PROCEDURE_NAME, PropertyValue::Str("Invoke".into())),
    ];
    if let Some(mode) = mode {
        child.properties.push((
            op::F_DATA_LOCK_CONTROL_MODE,
            PropertyValue::Enum(Token::new(mode)),
        ));
    }
    object.children.push(child);
    object
}

#[test]
fn sparse_web_service_default_and_explicit_modes_survive_both_dialects() {
    for version in [FormatVersion::new(2, 20), FormatVersion::new(2, 21)] {
        for mode in [
            None,
            Some("Automatic"),
            Some("Managed"),
            Some("AutomaticAndManaged"),
        ] {
            let source = web_service(mode);
            let expected = PropertyValue::Enum(Token::new(mode.unwrap_or("Automatic")));
            for format in [Format::Edt, Format::Designer] {
                let bytes = write(format, &source, version);
                let text = std::str::from_utf8(&bytes).unwrap();
                if format == Format::Edt {
                    assert_eq!(
                        text.contains("<dataLockControlMode>"),
                        mode.is_some_and(|m| m != "Automatic")
                    );
                } else {
                    assert!(text.contains(&format!(
                        "<DataLockControlMode>{}</DataLockControlMode>",
                        mode.unwrap_or("Automatic")
                    )));
                }
                let decoded = read(format, "WebService", &bytes).unwrap();
                assert_eq!(
                    decoded.children[0]
                        .properties
                        .iter()
                        .find(|(id, _)| *id == op::F_DATA_LOCK_CONTROL_MODE)
                        .map(|(_, value)| value.clone())
                        .unwrap_or_else(|| PropertyValue::Enum(Token::new("Automatic"))),
                    expected
                );
                for target in [Format::Edt, Format::Designer] {
                    let returned =
                        read(target, "WebService", &write(target, &decoded, version)).unwrap();
                    assert_eq!(
                        returned.children[0].properties,
                        decoded.children[0].properties
                    );
                }
            }
        }
    }
    let sparse = String::from_utf8(write(
        Format::Edt,
        &web_service(None),
        FormatVersion::new(2, 20),
    ))
    .unwrap();
    for invalid in [
        sparse.replace("<procedureName>Invoke</procedureName>", ""),
        sparse.replace("<procedureName>", "<procedureName extra='yes'>"),
        sparse.replace("</operations>", "<dataLockControlMode>Managed</dataLockControlMode><dataLockControlMode>Automatic</dataLockControlMode></operations>"),
    ] {
        assert!(read(Format::Edt, "WebService", invalid.as_bytes()).is_err());
    }
}

#[test]
fn filter_criterion_known_namespace_headers_are_checked_exactly() {
    let descriptor = "<mdclass:FilterCriterion xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance' xmlns:core='http://g5.1c.ru/v8/dt/mcore' xmlns:mdclass='http://g5.1c.ru/v8/dt/metadata/mdclass' uuid='11111111-1111-1111-1111-111111111111'><name>Known</name><type/></mdclass:FilterCriterion>";
    let object = read(Format::Edt, "FilterCriterion", descriptor.as_bytes()).unwrap();
    for format in [Format::Edt, Format::Designer] {
        let returned = read(
            format,
            "FilterCriterion",
            &write(format, &object, FormatVersion::new(2, 20)),
        )
        .unwrap();
        assert_eq!(returned.properties, object.properties);
    }
    for invalid in [
        descriptor.replace("http://www.w3.org/2001/XMLSchema-instance", "urn:wrong:xsi"),
        descriptor.replace("http://g5.1c.ru/v8/dt/mcore", "urn:wrong:core"),
        descriptor.replace("xmlns:core=", "xmlns:unknown="),
        descriptor.replace("<type/>", "<type extra='yes'/>"),
    ] {
        assert!(read(Format::Edt, "FilterCriterion", invalid.as_bytes()).is_err());
    }
}

#[test]
fn custom_xdto_return_and_parameter_use_native_physical_depths() {
    use morph1c_core::spec::metadata::web_service_operation_parameter as parameter;
    let mut source = web_service(Some("Managed"));
    source.children[0].properties[0].1 = PropertyValue::List(vec![
        PropertyValue::Str("Result".into()),
        PropertyValue::Str("urn:custom:return".into()),
    ]);
    let mut child = MetadataObject::new(
        ObjectKind::new("WebService.Operation.Parameter"),
        "Argument",
        Uuid([3; 16]),
    );
    child.properties.push((
        parameter::F_XDTO_VALUE_TYPE,
        PropertyValue::List(vec![
            PropertyValue::Str("Input".into()),
            PropertyValue::Str("urn:custom:parameter".into()),
        ]),
    ));
    source.children[0].children.push(child);
    for version in [FormatVersion::new(2, 20), FormatVersion::new(2, 21)] {
        let native = write(Format::Designer, &source, version);
        let text = std::str::from_utf8(&native).unwrap();
        assert!(text.contains("<XDTOReturningValueType xmlns:d6p1=\"urn:custom:return\">d6p1:Result</XDTOReturningValueType>"));
        assert!(text.contains(
            "<XDTOValueType xmlns:d8p1=\"urn:custom:parameter\">d8p1:Input</XDTOValueType>"
        ));
        let decoded = read(Format::Designer, "WebService", &native).unwrap();
        let edt = write(Format::Edt, &decoded, version);
        let returned = read(Format::Edt, "WebService", &edt).unwrap();
        assert!(returned.children == decoded.children);
        assert_eq!(write(Format::Designer, &returned, version), native);
        // Local prefix names do not change a QName's resolved value.
        assert!(
            read(
                Format::Designer,
                "WebService",
                text.replace("d8p1", "another").as_bytes()
            )
            .unwrap()
            .children
                == decoded.children
        );
        for invalid in [
            text.replace(">d8p1:Input<", ">undeclared:Input<"),
            text.replace(
                "<XDTOValueType xmlns:d8p1=",
                "<XDTOValueType extra='yes' xmlns:d8p1=",
            ),
        ] {
            assert!(read(Format::Designer, "WebService", invalid.as_bytes()).is_err());
        }
    }
}

#[test]
#[ignore = "requires immutable genuine UH EDT and XML corpus"]
fn genuine_sparse_metadata_census() {
    use std::{collections::BTreeMap, path::PathBuf};
    let edt = PathBuf::from(std::env::var_os("IBCMD_EDT_UHA_EDT").unwrap());
    let native = PathBuf::from(std::env::var_os("IBCMD_EDT_UHA_XML").unwrap());
    let version = FormatVersion::new(
        2,
        std::env::var("IBCMD_EDT_DIALECT_MINOR")
            .unwrap()
            .parse()
            .unwrap(),
    );
    let mut counts = BTreeMap::new();
    for (kind, family) in [
        ("FilterCriterion", "FilterCriteria"),
        ("WebService", "WebServices"),
    ] {
        let mut count = 0usize;
        for owner in std::fs::read_dir(edt.join("src").join(family)).unwrap() {
            let owner = owner.unwrap();
            if !owner.file_type().unwrap().is_dir() {
                continue;
            }
            let name = owner.file_name();
            let ep = owner.path().join(&name).with_extension("mdo");
            let np = native.join(family).join(&name).with_extension("xml");
            let ebytes = std::fs::read(&ep).unwrap();
            let xbytes = std::fs::read(&np).unwrap();
            let e = read(Format::Edt, kind, &ebytes).unwrap();
            let x = read(Format::Designer, kind, &xbytes).unwrap();
            assert!(
                e.properties == x.properties,
                "{kind} root property semantics"
            );
            assert!(
                same_descriptor_children(&e, &x),
                "{kind} descriptor child semantics"
            );
            assert!(
                write(Format::Edt, &e, version) == ebytes,
                "{kind} genuine EDT same-source"
            );
            let regenerated_xml = write(Format::Designer, &x, version);
            if regenerated_xml != xbytes
                && let Some(path) = std::env::var_os("IBCMD_EDT_DIFF_DIR")
            {
                let directory = PathBuf::from(path);
                std::fs::create_dir(&directory).unwrap();
                std::fs::write(directory.join("original.xml"), &xbytes).unwrap();
                std::fs::write(directory.join("regenerated.xml"), &regenerated_xml).unwrap();
            }
            assert!(regenerated_xml == xbytes, "{kind} genuine XML same-source");
            for source in [&e, &x] {
                // This focused test does not load external form descriptors.
                // Cross-codec checks cover descriptor-owned properties and
                // commands; original FormRefs are checked by exact R above.
                // Full directory acceptance separately loads all forms.
                let mut descriptor_owned = source.clone();
                descriptor_owned
                    .children
                    .retain(|child| child.kind.as_str() != "FilterCriterion.FormRef");
                for target in [Format::Edt, Format::Designer] {
                    let result =
                        read(target, kind, &write(target, &descriptor_owned, version)).unwrap();
                    assert!(result.properties == source.properties);
                    assert!(same_descriptor_children(&result, &descriptor_owned));
                }
            }
            count += 1;
        }
        counts.insert(kind, count);
    }
    assert!(counts.values().all(|count| *count > 0));
    println!("genuine metadata census: {counts:?}");
}
