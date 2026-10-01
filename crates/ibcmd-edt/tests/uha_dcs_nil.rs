use formats_xml::form::{
    FormDialect, read_conditional_appearance_dcssca, read_form,
    write_conditional_appearance_dcssca, write_form,
};
use morph1c_core::ir::{DcsItem, DcsRightValue, FormBody};

fn nil_count(items: &[DcsItem]) -> usize {
    items
        .iter()
        .map(|item| match item {
            DcsItem::FilterComparison { right, .. } => right
                .iter()
                .filter(|v| **v == DcsRightValue::Undefined)
                .count(),
            DcsItem::FilterGroup { items, .. } => nil_count(items),
            DcsItem::ConditionalAppearance { filter, .. } => nil_count(filter),
            _ => 0,
        })
        .sum()
}
fn witness() -> Vec<DcsItem> {
    vec![DcsItem::ConditionalAppearance {
        used: None,
        selection: None,
        filter: vec![DcsItem::FilterComparison {
            used: None,
            left_field: "Object.Document".into(),
            left_type: "dcscor:Field".into(),
            comparison_type: "InList".into(),
            right: vec![
                DcsRightValue::DesignTimeValue("Документ.ВыбытиеИнвестиций.ПустаяСсылка".into()),
                DcsRightValue::Undefined,
                DcsRightValue::Boolean("true".into()),
            ],
            presentation: None,
            view_mode: None,
            user_setting_id: None,
            user_setting_presentation: None,
        }],
        appearance: vec![],
        presentation: None,
        view_mode: None,
        user_setting_id: None,
    }]
}
#[test]
fn undefined_right_preserves_order_and_explicit_presence_in_both_formats() {
    let items = witness();
    let edt = write_conditional_appearance_dcssca(&items, true);
    let (decoded, flavor) = read_conditional_appearance_dcssca(&edt).unwrap();
    assert!(flavor);
    assert_eq!(decoded, items);
    assert_eq!(nil_count(&decoded), 1);
    assert_eq!(write_conditional_appearance_dcssca(&decoded, flavor), edt);
    let mut body = FormBody::new();
    body.conditional_appearance = items.clone();
    let native = write_form(FormDialect::Designer, &body).unwrap();
    let returned = read_form(FormDialect::Designer, &native).unwrap();
    assert_eq!(returned.conditional_appearance, items);
    assert_eq!(
        write_form(FormDialect::Designer, &returned).unwrap(),
        native
    );
    let text = String::from_utf8(edt).unwrap();
    for replacement in [
        "<right/>",
        "<right xsi:nil=\"false\"/>",
        "<right xsi:nil=\"true\" xsi:type=\"xs:string\"/>",
        "<right xsi:nil=\"true\">hidden</right>",
        "<right xsi:nil=\"true\"><hidden/></right>",
        "<right xsi:nil=\"true\" unknown=\"true\"/>",
    ] {
        assert!(
            read_conditional_appearance_dcssca(
                text.replace("<right xsi:nil=\"true\"/>", replacement)
                    .as_bytes()
            )
            .is_err(),
            "{replacement}"
        );
    }
    let mut absent = items.clone();
    let DcsItem::ConditionalAppearance { filter, .. } = &mut absent[0] else {
        panic!("expected appearance")
    };
    let DcsItem::FilterComparison { right, .. } = &mut filter[0] else {
        panic!("expected comparison")
    };
    right.remove(1);
    assert_ne!(
        serde_json::to_vec(&items).unwrap(),
        serde_json::to_vec(&absent).unwrap()
    );
}
#[test]
#[ignore = "requires immutable genuine UH83 paired native/EDT sources in F laboratory"]
fn genuine_uha83_undefined_right_is_not_dropped_or_reordered() {
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let rel = "DataProcessors/СтруктураВладения/Forms/Форма";
    let native = std::fs::read(
        lab.join("../04/release-20261001/rc/out/uha8327_db_r1/tree")
            .join(rel)
            .join("Ext/Form.xml"),
    )
    .unwrap();
    let edt = std::fs::read(
        lab.join("oracle-uha83-r1/authentic-workspace/OracleConfiguration/src")
            .join(rel)
            .join("ConditionalAppearance.dcssca"),
    )
    .unwrap();
    let body = read_form(FormDialect::Designer, &native).unwrap();
    let (items, flavor) = read_conditional_appearance_dcssca(&edt).unwrap();
    assert_eq!(nil_count(&body.conditional_appearance), 1);
    assert_eq!(nil_count(&items), 1);
    assert_eq!(body.conditional_appearance, items);
    assert_eq!(write_conditional_appearance_dcssca(&items, flavor), edt);
    let generated = write_form(FormDialect::Designer, &body).unwrap();
    assert_eq!(
        read_form(FormDialect::Designer, &generated)
            .unwrap()
            .conditional_appearance,
        items
    );
}
