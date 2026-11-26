//! The tests of `compiler::bodies::form` (in `ibcmd-compiler`) that read a
//! compiled body back through the root's exporter
//! (`mssql_dump::extract_form_body_xml`).

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::compiler::bodies::form::{
        ManagedFormCodecProfile, compile_managed_form, decode_managed_form,
    };

    // The codec's own constants are private to it.
    const UTF8_BOM: &[u8; 3] = b"\xef\xbb\xbf";
    const FORM_TRAILING_SECTIONS: usize = 4;

    const SIMPLE_FORM: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<Title><v8:item><v8:lang>en</v8:lang><v8:content>Standalone</v8:content></v8:item></Title>
	<Width>480</Width>
	<Height>320</Height>
	<AutoTitle>false</AutoTitle>
	<AutoCommandBar name="FormCommandBar" id="-1"/>
	<Commands>
		<Command name="Do" id="1"/>
	</Commands>
	<Attributes>
		<Attribute name="Description" id="2">
			<Type><v8:Type>xs:string</v8:Type></Type>
		</Attribute>
	</Attributes>
	<Parameters>
		<Parameter name="Key"><Type><v8:Type>xs:string</v8:Type></Type></Parameter>
	</Parameters>
	<ChildItems>
		<UsualGroup name="Main" id="10">
			<ChildItems>
				<Button name="Run" id="11"><CommandName>Form.Command.Do</CommandName></Button>
				<InputField name="Description" id="12"><DataPath>Description</DataPath></InputField>
			</ChildItems>
		</UsualGroup>
	</ChildItems>
</Form>"#;

    #[test]
    fn marker_50_form_compiles_deterministically_without_base_blob() {
        let profile = ManagedFormCodecProfile::fixture();
        let module = b"&AtClient\r\nProcedure Run(Command)\r\nEndProcedure";
        let first = compile_managed_form(&profile, SIMPLE_FORM, Some(module), None).unwrap();
        let second = compile_managed_form(&profile, SIMPLE_FORM, Some(module), None).unwrap();
        assert_eq!(first, second);

        let decoded = decode_managed_form(&profile, &first).unwrap();
        assert_eq!(decoded.module_text(), std::str::from_utf8(module).unwrap());
        assert!(decoded.plaintext().starts_with(UTF8_BOM));
        assert_eq!(decoded.parsed().trailing_fields, FORM_TRAILING_SECTIONS);
        assert!(decoded.parsed().layout.starts_with("{50,"));
        assert!(decoded.parsed().layout.contains("\"Main\""));
        assert!(decoded.parsed().layout.contains("\"Run\""));

        let extracted = crate::mssql_dump::extract_form_body_xml(&first, &BTreeMap::new())
            .expect("marker-50 body must remain readable by the export adapter");
        for expected in [
            "<Width>480</Width>",
            "<Height>320</Height>",
            // A command carrying nothing is an empty element (fixture
            // `empty_command`), so only the opening is pinned.
            "<Command name=\"Do\" id=\"1\"",
            "<UsualGroup name=\"Main\" id=\"10\">",
            "<Button name=\"Run\" id=\"11\">",
            "<CommandName>Form.Command.Do</CommandName>",
            "<Attribute name=\"Description\" id=\"2\">",
            "<Parameter name=\"Key\">",
            "<InputField name=\"Description\" id=\"12\">",
            "<DataPath>Description</DataPath>",
        ] {
            assert!(extracted.contains(expected), "{expected}: {extracted}");
        }
    }

    #[test]
    fn representative_container_field_table_and_addition_matrix_roundtrips() {
        let profile = ManagedFormCodecProfile::fixture();
        let xml = br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<CommandBar name="Actions" id="1"><ChildItems><Button name="Action" id="2"/></ChildItems></CommandBar>
		<Pages name="Tabs" id="3"><ChildItems><Page name="General" id="4"><ChildItems>
			<CheckBoxField name="Enabled" id="5"/>
			<TextDocumentField name="Notes" id="6"/>
			<PictureDecoration name="Logo" id="7"/>
		</ChildItems></Page></ChildItems></Pages>
		<Table name="Rows" id="8"><ChildItems>
			<LabelField name="RowLabel" id="9"/>
			<InputField name="RowValue" id="10"/>
		</ChildItems></Table>
		<SearchStringAddition name="Search" id="11"/>
		<ViewStatusAddition name="Status" id="12"/>
		<SearchControlAddition name="SearchControl" id="13"/>
	</ChildItems>
</Form>"#;

        let blob = compile_managed_form(&profile, xml, None, None).unwrap();
        let extracted = crate::mssql_dump::extract_form_body_xml(&blob, &BTreeMap::new())
            .expect("representative marker-50 matrix must be exportable");
        for tag in [
            "CommandBar",
            "Button",
            "Pages",
            "Page",
            "CheckBoxField",
            "TextDocumentField",
            "PictureDecoration",
            "Table",
            "LabelField",
            "InputField",
            "SearchStringAddition",
            "ViewStatusAddition",
            "SearchControlAddition",
        ] {
            assert!(
                extracted.contains(&format!("<{tag} ")),
                "missing {tag}: {extracted}"
            );
        }
    }
}
