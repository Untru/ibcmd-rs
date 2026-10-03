use formats_xml::form::{
    FormDialect, read_form, read_spreadsheet_mxlx, write_form, write_spreadsheet_mxlx,
};
use morph1c_core::ir::form::MxlNode;
fn source() -> Vec<u8> {
    concat!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
    "<document xmlns=\"http://v8.1c.ru/8.2/data/spreadsheet\" xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\r\n",
    "\t<columnsItem index=\"2\">\r\n\t\t<cellsItem>\r\n\t\t\t<c>\r\n\t\t\t\t<v8:v xsi:type=\"xs:decimal\">3</v8:v>\r\n\t\t\t</c>\r\n\t\t</cellsItem>\r\n\t</columnsItem>\r\n</document>").as_bytes().to_vec()
}
fn body() -> morph1c_core::ir::FormBody {
    let fixture = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:form=\"http://g5.1c.ru/v8/dt/form\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"><attributes><name>Grid</name><id>1</id><valueType><types>SpreadsheetDocument</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:SpreadsheetDocumentExtInfo\"/></attributes></form:Form>\r\n";
    read_form(FormDialect::Edt, fixture.as_bytes()).unwrap()
}
fn find<'a>(nodes: &'a mut [MxlNode], name: &str) -> &'a mut MxlNode {
    let index = nodes.iter().position(|n| n.local == name);
    if let Some(i) = index {
        return &mut nodes[i];
    }
    for node in nodes {
        if !node.children.is_empty() {
            return find(&mut node.children, name);
        }
    }
    panic!("missing node")
}
#[test]
fn rich_spreadsheet_uses_expanded_names_and_current_values_both_directions() {
    for minor in [20, 21] {
        morph1c_core::version::with_source_version(
            Some(morph1c_core::version::FormatVersion::new(2, minor)),
            || {
                morph1c_core::version::with_roundtrip_target(
                    morph1c_core::version::FormatVersion::new(2, minor),
                    || {
                        let original = source();
                        let settings = read_spreadsheet_mxlx(&original).unwrap();
                        assert_eq!(write_spreadsheet_mxlx(&settings), original);
                        let mut current = body();
                        current.data_attributes[0].spreadsheet_settings = Some(settings.clone());
                        let native = write_form(FormDialect::Designer, &current).unwrap();
                        let text = std::str::from_utf8(&native).unwrap();
                        assert!(text.contains("<mxl:columnsItem index=\"2\">"));
                        assert!(text.contains("<v8:v xsi:type=\"xs:decimal\">3</v8:v>"));
                        let decoded = read_form(FormDialect::Designer, &native).unwrap();
                        let other = decoded.data_attributes[0]
                            .spreadsheet_settings
                            .as_ref()
                            .unwrap();
                        assert_eq!(
                            serde_json::to_value(&settings).unwrap(),
                            serde_json::to_value(other).unwrap()
                        );
                        assert_eq!(write_form(FormDialect::Designer, &decoded).unwrap(), native);
                        let node = find(
                            current.data_attributes[0]
                                .spreadsheet_settings
                                .as_mut()
                                .unwrap()
                                .full_body
                                .as_mut()
                                .unwrap(),
                            "v",
                        );
                        node.text = "17".into();
                        node.attrs[0].1 = "{http://www.w3.org/2001/XMLSchema}string".into();
                        let native = write_form(FormDialect::Designer, &current).unwrap();
                        assert!(
                            std::str::from_utf8(&native)
                                .unwrap()
                                .contains("xsi:type=\"xs:string\">17</v8:v>")
                        );
                        let decoded = read_form(FormDialect::Designer, &native).unwrap();
                        assert_eq!(
                            serde_json::to_value(&current.data_attributes[0].spreadsheet_settings)
                                .unwrap(),
                            serde_json::to_value(&decoded.data_attributes[0].spreadsheet_settings)
                                .unwrap()
                        );
                    },
                )
            },
        );
    }
}
#[test]
fn prefix_spelling_is_lexical_but_namespace_identity_and_attributes_are_current() {
    let original = source();
    let alias = String::from_utf8(original.clone())
        .unwrap()
        .replace("xmlns:v8=", "xmlns:c=")
        .replace("<v8:v", "<c:v")
        .replace("</v8:v", "</c:v");
    // mxlx's closed root frame uses its standard root declarations; a local alias remains legal.
    let alias = alias
        .replace(
            "xmlns:c=\"http://v8.1c.ru/8.1/data/core\"",
            "xmlns:v8=\"http://v8.1c.ru/8.1/data/core\"",
        )
        .replace("<c:v ", "<c:v xmlns:c=\"http://v8.1c.ru/8.1/data/core\" ");
    let a = read_spreadsheet_mxlx(&original).unwrap();
    let b = read_spreadsheet_mxlx(alias.as_bytes()).unwrap();
    assert_eq!(a.full_body, b.full_body);
    assert_eq!(
        serde_json::to_value(&a).unwrap(),
        serde_json::to_value(&b).unwrap()
    );
    assert_eq!(write_spreadsheet_mxlx(&b), alias.as_bytes());
    let wrong = alias.replace(
        "xmlns:c=\"http://v8.1c.ru/8.1/data/core\"",
        "xmlns:c=\"urn:other-current-namespace\"",
    );
    let other = read_spreadsheet_mxlx(wrong.as_bytes()).unwrap();
    assert_ne!(a.full_body, other.full_body);
    let mut current = body();
    current.data_attributes[0].spreadsheet_settings = Some(other.clone());
    let native = write_form(FormDialect::Designer, &current).unwrap();
    let decoded = read_form(FormDialect::Designer, &native).unwrap();
    assert_eq!(
        serde_json::to_value(&other).unwrap(),
        serde_json::to_value(&decoded.data_attributes[0].spreadsheet_settings).unwrap()
    );
}
#[test]
fn rich_spreadsheet_rejects_unbound_and_duplicate_expanded_attribute_names() {
    let source = String::from_utf8(source()).unwrap();
    assert!(
        read_spreadsheet_mxlx(
            source
                .replace("<v8:v", "<unbound:v")
                .replace("</v8:v", "</unbound:v")
                .as_bytes()
        )
        .is_err()
    );
    let duplicate = source.replace(
        "index=\"2\"",
        "xmlns:a=\"urn:attribute\" xmlns:b=\"urn:attribute\" a:index=\"2\" b:index=\"3\"",
    );
    assert!(read_spreadsheet_mxlx(duplicate.as_bytes()).is_err());
    assert!(
        read_spreadsheet_mxlx(source.replace("xs:decimal", "unbound:decimal").as_bytes()).is_err()
    );
    for bad in [
        ":decimal",
        "xs:",
        "xs:decimal:extra",
        "xs:9decimal",
        "xs:bad name",
    ] {
        assert!(read_spreadsheet_mxlx(source.replace("xs:decimal", bad).as_bytes()).is_err());
    }
    for declaration in [
        "xmlns:xml=\"urn:wrong\"",
        "xmlns:a=\"http://www.w3.org/XML/1998/namespace\"",
        "xmlns:xmlns=\"urn:wrong\"",
        "xmlns:a=\"http://www.w3.org/2000/xmlns/\"",
        "xmlns:a=\"\"",
        "xmlns:=\"urn:malformed\"",
        "xmlns:a=\"urn:a\" xmlns:a=\"urn:b\"",
    ] {
        let bad = source.replace("<v8:v ", &format!("<v8:v {declaration} "));
        assert!(read_spreadsheet_mxlx(bad.as_bytes()).is_err());
    }
    let valid = source.replace("<v8:v ", "<v8:v xml:lang=\"en\" ");
    let parsed = read_spreadsheet_mxlx(valid.as_bytes()).unwrap();
    assert_eq!(write_spreadsheet_mxlx(&parsed), valid.as_bytes());
}

#[test]
fn sidecar_namespace_scope_contains_only_actual_declarations_and_builtin_xml() {
    let original = String::from_utf8(source()).unwrap().replace(
        " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\"",
        "",
    );
    for prefix in ["mxl", "pal"] {
        let unbound = original
            .replace("<columnsItem", &format!("<{prefix}:columnsItem"))
            .replace("</columnsItem", &format!("</{prefix}:columnsItem"));
        assert!(read_spreadsheet_mxlx(unbound.as_bytes()).is_err());
    }
    let mut current = read_spreadsheet_mxlx(original.as_bytes()).unwrap();
    assert_eq!(write_spreadsheet_mxlx(&current), original.as_bytes());
    let node = find(current.full_body.as_mut().unwrap(), "v");
    node.namespace = "http://v8.1c.ru/8.1/data/ui/colors/palette".into();
    node.source_layout = None;
    let wire = write_spreadsheet_mxlx(&current);
    assert!(
        std::str::from_utf8(&wire)
            .unwrap()
            .contains("xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\"")
    );
    assert_eq!(
        current.full_body,
        read_spreadsheet_mxlx(&wire).unwrap().full_body
    );
    for minor in [20, 21] {
        let version = morph1c_core::version::FormatVersion::new(2, minor);
        morph1c_core::version::with_source_version(Some(version), || {
            morph1c_core::version::with_roundtrip_target(version, || {
                let mut form = body();
                form.data_attributes[0].spreadsheet_settings = Some(current.clone());
                let native = write_form(FormDialect::Designer, &form).unwrap();
                let decoded = read_form(FormDialect::Designer, &native).unwrap();
                assert_eq!(
                    current.full_body,
                    decoded.data_attributes[0]
                        .spreadsheet_settings
                        .as_ref()
                        .unwrap()
                        .full_body
                );
                let unbound = String::from_utf8(native).unwrap().replace(
                    " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\"",
                    "",
                );
                assert!(read_form(FormDialect::Designer, unbound.as_bytes()).is_err());
            })
        });
    }
}

#[test]
fn current_namespace_changes_do_not_rebind_other_current_names() {
    let mut current = read_spreadsheet_mxlx(&source()).unwrap();
    let node = find(current.full_body.as_mut().unwrap(), "v");
    node.namespace = "urn:current-element".into();
    node.attrs.push((
        "{http://v8.1c.ru/8.1/data/core}identity".into(),
        "present".into(),
    ));
    node.attrs.sort_by(|a, b| a.0.cmp(&b.0));
    let wire = write_spreadsheet_mxlx(&current);
    let decoded = read_spreadsheet_mxlx(&wire).unwrap();
    assert_eq!(current.full_body, decoded.full_body);
    let node = find(current.full_body.as_mut().unwrap(), "v");
    node.namespace.clear();
    node.attrs
        .iter_mut()
        .find(|(n, _)| n == "{http://www.w3.org/2001/XMLSchema-instance}type")
        .unwrap()
        .1 = "decimal".into();
    let wire = write_spreadsheet_mxlx(&current);
    assert_eq!(
        current.full_body,
        read_spreadsheet_mxlx(&wire).unwrap().full_body
    );
}

#[test]
fn schema_qname_text_keeps_expanded_identity_and_current_edits() {
    let source = String::from_utf8(source()).unwrap().replace(
        "<v8:v xsi:type=\"xs:decimal\">3</v8:v>",
        "<v8:Type xmlns:c=\"urn:current-type\">c:Record</v8:Type>",
    );
    let mut current = read_spreadsheet_mxlx(source.as_bytes()).unwrap();
    assert_eq!(write_spreadsheet_mxlx(&current), source.as_bytes());
    let alias = source
        .replace("xmlns:c=", "xmlns:d=")
        .replace(">c:Record<", ">d:Record<");
    assert_eq!(
        current.full_body,
        read_spreadsheet_mxlx(alias.as_bytes()).unwrap().full_body
    );
    let rebound = source.replace("urn:current-type", "urn:different-type");
    assert_ne!(
        current.full_body,
        read_spreadsheet_mxlx(rebound.as_bytes()).unwrap().full_body
    );
    let node = find(current.full_body.as_mut().unwrap(), "Type");
    assert!(node.text.is_empty());
    node.text_qname = Some(("urn:edited-type".into(), "EditedRecord".into()));
    let mut form = body();
    form.data_attributes[0].spreadsheet_settings = Some(current.clone());
    let native = write_form(FormDialect::Designer, &form).unwrap();
    let decoded = read_form(FormDialect::Designer, &native).unwrap();
    assert_eq!(
        current.full_body,
        decoded.data_attributes[0]
            .spreadsheet_settings
            .as_ref()
            .unwrap()
            .full_body
    );
    for text in ["c:", ":Record", "c:Record:extra", "missing:Record"] {
        assert!(read_spreadsheet_mxlx(source.replace("c:Record", text).as_bytes()).is_err());
    }
}

fn isolated_attribute(bytes: &[u8], owner: &str) -> Vec<u8> {
    let mut root = formats_xml::parse(bytes).unwrap().root;
    root.children.retain(|e| e.local == "Attributes");
    let attributes = root.children.first_mut().unwrap();
    attributes
        .children
        .retain(|e| e.attr("name").is_some_and(|a| a.value == owner));
    assert_eq!(attributes.children.len(), 1);
    fn output(el: &formats_xml::descriptor::Element) -> formats_xml::OutElement {
        let mut out = if el.children.is_empty() && el.text.is_empty() {
            formats_xml::OutElement::self_closing(&el.prefix, &el.local)
        } else if el.children.is_empty() {
            formats_xml::OutElement::leaf(&el.prefix, &el.local, &el.text)
        } else {
            formats_xml::OutElement::branch(&el.prefix, &el.local)
        };
        for attribute in &el.attrs {
            out = out.attr(&attribute.name, &attribute.value);
        }
        for child in &el.children {
            out.push(output(child));
        }
        out
    }
    formats_xml::emit::render(
        &formats_xml::Envelope {
            bom: true,
            eol: "\r\n",
            indent_unit: "\t",
            decl: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
            trailing_eol: true,
            escape_gt: true,
            escape_quot: false,
            text_eol: "\n",
        },
        &output(&root),
    )
}

fn settings_bytes(bytes: &[u8], owner: &str) -> Vec<u8> {
    use quick_xml::events::Event;
    let offset = usize::from(bytes.starts_with(&[0xef, 0xbb, 0xbf])) * 3;
    let mut reader = quick_xml::Reader::from_reader(&bytes[offset..]);
    let mut depth = 0usize;
    let mut owned = None;
    let mut capture = None;
    loop {
        let start = offset + reader.buffer_position() as usize;
        match reader.read_event().unwrap() {
            Event::Start(element) => {
                depth += 1;
                if element.local_name().as_ref() == b"Attribute"
                    && element.attributes().any(|a| {
                        let a = a.unwrap();
                        a.key.as_ref() == b"name"
                            && a.decode_and_unescape_value(reader.decoder()).unwrap() == owner
                    })
                {
                    owned = Some(depth);
                }
                if owned.is_some() && element.local_name().as_ref() == b"Settings" {
                    assert!(capture.is_none());
                    capture = Some((start, depth));
                }
            }
            Event::End(_) => {
                if let Some((begin, level)) = capture {
                    if depth == level {
                        return bytes[begin..offset + reader.buffer_position() as usize].to_vec();
                    }
                }
                if owned == Some(depth) {
                    owned = None;
                }
                depth -= 1;
            }
            Event::Eof => panic!("missing owned spreadsheet Settings"),
            _ => {}
        }
    }
}

#[test]
#[ignore = "requires hash-bound ALL10 genuine rich MXL sidecars/native SDK manifest"]
fn genuine_all_ten_spreadsheet_namespace_cases_match_native_sdk_bytes() {
    use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
    use sha2::{Digest, Sha256};
    let sha = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    let manifest = std::fs::read(std::env::var_os("IBCMD_MXL_MANIFEST").unwrap()).unwrap();
    let cases: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
    assert_eq!(cases["processed"], 10);
    let report = std::env::var_os("IBCMD_MXL_REPORT").unwrap();
    let mut rows = Vec::new();
    let mut failures = 0;
    for (index, row) in cases["rows"].as_array().unwrap().iter().enumerate() {
        let source = std::fs::read(row["source"].as_str().unwrap()).unwrap();
        let reference = std::fs::read(row["reference"].as_str().unwrap()).unwrap();
        assert_eq!(sha(&source), row["source_sha256"]);
        assert_eq!(sha(&reference), row["reference_sha256"]);
        let owner = row["owner"].as_str().unwrap();
        let version = match row["profile"].as_str().unwrap() {
            "83" => FormatVersion::new(2, 20),
            "85" => FormatVersion::new(2, 21),
            _ => panic!("unbound profile"),
        };
        let (own_source, own_native, exact, semantic) = with_source_version(Some(version), || {
            with_roundtrip_target(version, || {
                let settings = read_spreadsheet_mxlx(&source).unwrap();
                let isolated = isolated_attribute(&reference, owner);
                let original = read_form(FormDialect::Designer, &isolated).unwrap();
                assert_eq!(original.data_attributes.len(), 1);
                let own_native = settings_bytes(
                    &write_form(FormDialect::Designer, &original).unwrap(),
                    owner,
                ) == settings_bytes(&reference, owner);
                let mut current = original.clone();
                current.data_attributes[0].spreadsheet_settings = Some(settings.clone());
                let native = write_form(FormDialect::Designer, &current).unwrap();
                let actual = settings_bytes(&native, owner);
                let expected = settings_bytes(&reference, owner);
                let returned = read_form(FormDialect::Designer, &native).unwrap();
                (
                    write_spreadsheet_mxlx(&settings) == source,
                    own_native,
                    actual == expected,
                    returned.data_attributes[0]
                        .spreadsheet_settings
                        .as_ref()
                        .unwrap()
                        .full_body
                        == settings.full_body,
                )
            })
        });
        let unchanged = sha(&std::fs::read(row["source"].as_str().unwrap()).unwrap())
            == row["source_sha256"]
            && sha(&std::fs::read(row["reference"].as_str().unwrap()).unwrap())
                == row["reference_sha256"];
        if !(own_source && own_native && exact && semantic && unchanged) {
            failures += 1;
        }
        rows.push(serde_json::json!({"index":index,"profile":row["profile"],"path_sha256":row["path_sha256"],
            "own_source":own_source,"own_native":own_native,"exact":exact,"semantic":semantic,"source_unchanged":unchanged}));
    }
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(report)
        .unwrap();
    serde_json::to_writer_pretty(&mut output,&serde_json::json!({"complete":true,"processed":10,"failures":failures,"manifest_sha256":sha(&manifest),"rows":rows})).unwrap();
    assert_eq!(failures, 0);
}
