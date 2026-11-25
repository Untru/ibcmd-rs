use super::native_embedded_spreadsheet;

#[test]
fn embedded_spreadsheet_preserves_explicit_empty_language_settings() {
    let xml = r#"<Attribute name="Sheet"><Settings>
        <mxl:languageSettings><mxl:currentLanguage/><mxl:defaultLanguage/></mxl:languageSettings>
        <mxl:columns><mxl:size>2</mxl:size></mxl:columns>
        <mxl:rowsItem><mxl:index>0</mxl:index><mxl:row><mxl:empty>true</mxl:empty></mxl:row></mxl:rowsItem>
        <mxl:height>1</mxl:height><mxl:vgRows>1</mxl:vgRows>
    </Settings></Attribute>"#;
    let compiled = native_embedded_spreadsheet(xml, "Sheet").unwrap();
    let language_record = r#"{8,1,12,{"","",0,0,0},"#;
    assert!(compiled.contains(language_record));
}
