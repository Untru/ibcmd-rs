use formats_xml::form::{
    FormDialect, read_conditional_appearance_dcssca, read_form, read_list_settings_dcss,
    write_conditional_appearance_dcssca, write_form, write_list_settings_dcss,
};
use morph1c_core::ir::{DcsItem, DcsRightValue, FormBody};
use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
use quick_xml::{Reader, events::Event};

// Read actual emitted namespace/QName/depth without rewriting any source bytes.
fn type_heads(bytes: &[u8]) -> Vec<(usize, String, String)> {
    let mut reader = Reader::from_reader(bytes);
    let mut depth = 0;
    let mut heads = vec![];
    loop {
        match reader.read_event().unwrap() {
            Event::Start(start) => {
                depth += 1;
                if start.local_name().as_ref() == b"right" {
                    let attrs: Vec<_> = start.attributes().map(|a| a.unwrap()).collect();
                    if attrs
                        .iter()
                        .any(|a| a.key.as_ref() == b"xsi:type" && a.value.as_ref() == b"v8:Type")
                    {
                        let ns = attrs
                            .iter()
                            .find(|a| a.value.as_ref() == b"http://v8.1c.ru/8.2/data/types")
                            .unwrap();
                        let prefix = std::str::from_utf8(ns.key.as_ref())
                            .unwrap()
                            .strip_prefix("xmlns:")
                            .unwrap()
                            .to_owned();
                        let Event::Text(text) = reader.read_event().unwrap() else {
                            panic!("typed QName leaf");
                        };
                        heads.push((
                            depth,
                            prefix,
                            std::str::from_utf8(text.as_ref()).unwrap().to_owned(),
                        ));
                    }
                }
            }
            Event::End(_) => depth -= 1,
            Event::Eof => break,
            _ => {}
        }
    }
    heads
}
fn body() -> FormBody {
    let mut body = FormBody::new();
    body.conditional_appearance
        .push(DcsItem::ConditionalAppearance {
            used: None,
            selection: None,
            filter: vec![DcsItem::FilterComparison {
                used: None,
                left_field: "Object.Kind".into(),
                left_type: "dcscor:Field".into(),
                comparison_type: "Equal".into(),
                right: vec![DcsRightValue::TypeQName("source:Undefined".into())],
                presentation: None,
                view_mode: None,
                user_setting_id: None,
                user_setting_presentation: None,
            }],
            appearance: vec![],
            source_empty_appearance: false,
            presentation: None,
            view_mode: None,
            user_setting_id: None,
        });
    body
}
#[test]
fn same_origin_aliases_remain_exact_cross_origin_uses_final_depth_and_edits_win() {
    let native = write_form(FormDialect::Designer, &body()).unwrap();
    let original = read_form(FormDialect::Designer, &native).unwrap();
    assert_eq!(
        write_form(FormDialect::Designer, &original).unwrap(),
        native
    );
    assert!(
        type_heads(&native)
            .iter()
            .all(|(_, prefix, _)| prefix == "source")
    );
    let sidecar = write_conditional_appearance_dcssca(&original.conditional_appearance, false);
    let (items, envelope) = read_conditional_appearance_dcssca(&sidecar).unwrap();
    assert_eq!(
        write_conditional_appearance_dcssca(&items, envelope),
        sidecar
    );
    assert!(
        type_heads(&sidecar)
            .iter()
            .all(|(depth, prefix, value)| prefix == &format!("d{depth}p1")
                && value == &format!("{prefix}:Undefined"))
    );
    let mut returned = original.clone();
    returned.conditional_appearance = items;
    assert_eq!(
        serde_json::to_vec(&returned).unwrap(),
        serde_json::to_vec(&original).unwrap()
    );
    let output = write_form(FormDialect::Designer, &returned).unwrap();
    assert!(
        type_heads(&output)
            .iter()
            .all(|(depth, prefix, value)| prefix == &format!("d{depth}p1")
                && value == &format!("{prefix}:Undefined"))
    );
    assert!(!String::from_utf8_lossy(&output).contains("source_type_qname"));
    let DcsItem::ConditionalAppearance { filter, .. } = &mut returned.conditional_appearance[0]
    else {
        unreachable!()
    };
    let DcsItem::FilterComparison { right, .. } = &mut filter[0] else {
        unreachable!()
    };
    let DcsRightValue::TypeQName(qname) = &mut right[0] else {
        unreachable!()
    };
    qname.qname = qname.qname.replace(":Undefined", ":String");
    let edited = write_form(FormDialect::Designer, &returned).unwrap();
    let reread = read_form(FormDialect::Designer, &edited).unwrap();
    assert_ne!(
        serde_json::to_vec(&original).unwrap(),
        serde_json::to_vec(&reread).unwrap()
    );
    assert!(
        type_heads(&edited)
            .iter()
            .all(|(_, _, value)| value.ends_with(":String"))
    );
}

#[test]
#[ignore = "requires unchanged authentic EDT and independent SDK UH83 witnesses on F"]
fn genuine_list_settings_aliases_match_independent_sdk_at_actual_depth() {
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let rel = "DocumentJournals/ЧекиККМ/Forms/ФормаСписка";
    let native_path = lab
        .join("native-reference-uha83-r1/native-xml")
        .join(rel)
        .join("Ext/Form.xml");
    let edt_path = lab
        .join("oracle-uha83-r1/authentic-workspace/OracleConfiguration/src")
        .join(rel)
        .join("Attributes/ЧекиККМ/ExtInfo/ListSettings.dcss");
    let native = std::fs::read(&native_path).unwrap();
    let edt = std::fs::read(&edt_path).unwrap();
    let sdk_heads = type_heads(&native);
    let edt_heads = type_heads(&edt);
    assert_eq!(
        sdk_heads,
        vec![(10, "d10p1".into(), "d10p1:Undefined".into()); 2]
    );
    assert_eq!(
        edt_heads,
        vec![(6, "d6p1".into(), "d6p1:Undefined".into()); 2]
    );
    let mut body = with_source_version(Some(FormatVersion::new(2, 20)), || {
        read_form(FormDialect::Designer, &native)
    })
    .unwrap();
    let unchanged = with_roundtrip_target(FormatVersion::new(2, 20), || {
        write_form(FormDialect::Designer, &body)
    })
    .unwrap();
    assert!(
        unchanged == native,
        "native SDK source form must regenerate exactly"
    );
    let attribute = body
        .data_attributes
        .iter_mut()
        .find(|a| a.name.as_str() == "ЧекиККМ")
        .unwrap();
    let list = attribute.dynamic_list.as_mut().unwrap();
    let source = list.list_settings.as_ref().unwrap();
    assert_eq!(type_heads(&write_list_settings_dcss(source)), edt_heads);
    let decoded = read_list_settings_dcss(&edt).unwrap();
    assert_eq!(write_list_settings_dcss(&decoded), edt);
    list.list_settings = Some(decoded);
    let output = with_roundtrip_target(FormatVersion::new(2, 20), || {
        write_form(FormDialect::Designer, &body)
    })
    .unwrap();
    assert_eq!(type_heads(&output), sdk_heads);
    let reread = with_source_version(Some(FormatVersion::new(2, 20)), || {
        read_form(FormDialect::Designer, &output)
    })
    .unwrap();
    assert_eq!(reread.data_attributes.len(), body.data_attributes.len());
    assert_eq!(std::fs::read(native_path).unwrap(), native);
    assert_eq!(std::fs::read(edt_path).unwrap(), edt);
}
