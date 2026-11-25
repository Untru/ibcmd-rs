use super::*;

#[test]
fn zero_code_preserves_a_present_command_picture() {
    let context = DescriptorContext::with_files(std::path::Path::new("."), "2.20", &[])
        .expect("empty descriptor index");
    let xml = crate::metadata_model::xml::parse_element_tree(
        b"<Picture><Ref>0</Ref><LoadTransparent>false</LoadTransparent></Picture>",
    )
    .expect("picture XML");
    let actual = picture(Some(&xml), &context).expect("recognize native zero picture code");
    let expected = brace_list![
        Brace::num(4),
        Brace::num(1),
        brace_list![Brace::num(0)],
        Brace::str(""),
        Brace::num(-1),
        Brace::num(-1),
        Brace::num(0),
        Brace::num(0),
        Brace::str(""),
    ];
    assert_eq!(actual, expected);
}
