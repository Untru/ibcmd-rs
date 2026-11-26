//! The tests of `compiler::bodies::mxl_native` (in `ibcmd-compiler`) that
//! read a written body back through the root's exporter
//! (`mssql_dump::try_extract_moxel_spreadsheet_xml`) or own a source tree
//! through the root's test scratch.

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use anyhow::{Result, anyhow};
    use ibcmd_metadata_source::MetadataSource;

    use crate::compiler::bodies::mxl_native::write_native_moxel_body;

    fn crlf(text: &str) -> String {
        text.replace('\n', "\r\n")
    }

    fn read_back(body: &str) -> String {
        let plain = format!("MOXCEL\0\u{8}\0\u{1}\0\u{c}\0\u{feff}{body}");
        let blob = crate::compiler::families::native::deflate_bytes(plain.as_bytes()).unwrap();
        crate::mssql_dump::try_extract_moxel_spreadsheet_xml(&blob, &BTreeMap::new()).unwrap()
    }

    /// БСП `DataProcessors/НастройкиПользователей/Templates/МакетОтчета`:
    /// the platform numbers its formats by first use in the body -- the two
    /// cell formats, then the column's -- where the XML lists the column's
    /// first, and keeps the default format out of the table.
    const REPORT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:style="http://v8.1c.ru/8.1/data/ui/style" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:v8ui="http://v8.1c.ru/8.1/data/ui" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
	<languageSettings>
		<currentLanguage>ru</currentLanguage>
		<defaultLanguage>ru</defaultLanguage>
		<languageInfo>
			<id>ru</id>
			<code>Русский</code>
			<description>Русский</description>
		</languageInfo>
	</languageSettings>
	<columns>
		<size>1</size>
		<columnsItem>
			<index>0</index>
			<column>
				<formatIndex>1</formatIndex>
			</column>
		</columnsItem>
	</columns>
	<rowsItem>
		<index>0</index>
		<row>
			<c>
				<c>
					<f>2</f>
					<parameter>Описание</parameter>
				</c>
			</c>
		</row>
	</rowsItem>
	<rowsItem>
		<index>1</index>
		<row>
			<empty>true</empty>
		</row>
	</rowsItem>
	<rowsItem>
		<index>2</index>
		<row>
			<c>
				<c>
					<f>3</f>
					<parameter>Название</parameter>
				</c>
			</c>
		</row>
	</rowsItem>
	<templateMode>true</templateMode>
	<defaultFormatIndex>4</defaultFormatIndex>
	<height>3</height>
	<vgRows>3</vgRows>
	<namedItem xsi:type="NamedItemCells">
		<name>Заголовок</name>
		<area>
			<type>Rows</type>
			<beginRow>0</beginRow>
			<endRow>0</endRow>
			<beginColumn>-1</beginColumn>
			<endColumn>-1</endColumn>
		</area>
	</namedItem>
	<namedItem xsi:type="NamedItemCells">
		<name>ПустаяСтрока</name>
		<area>
			<type>Rows</type>
			<beginRow>1</beginRow>
			<endRow>1</endRow>
			<beginColumn>-1</beginColumn>
			<endColumn>-1</endColumn>
		</area>
	</namedItem>
	<namedItem xsi:type="NamedItemCells">
		<name>СодержаниеОтчета</name>
		<area>
			<type>Rows</type>
			<beginRow>2</beginRow>
			<endRow>2</endRow>
			<beginColumn>-1</beginColumn>
			<endColumn>-1</endColumn>
		</area>
	</namedItem>
	<font faceName="Arial" height="8" bold="true" italic="false" underline="false" strikeout="false" kind="Absolute" scale="100"/>
	<format>
		<width>509</width>
	</format>
	<format>
		<font>0</font>
		<fillType>Parameter</fillType>
	</format>
	<format>
		<fillType>Parameter</fillType>
	</format>
	<format>
		<width>72</width>
	</format>
</document>"#;
    const REPORT_BODY: &str = r#"{8,1,12,
{"ru","ru",1,1,"ru","Русский","Русский",0},
{128,72},
{0},0,
{0,0},
{0,0},
{0,0},
{0,0},
{0,0},
{0,0},1,2,3,0,0,1,0,
{16,1,
{1,1,
{"","Описание"}
},0},1,0,0,2,0,1,0,
{16,2,
{1,1,
{"","Название"}
},0},
{1,0,00000000-0000-0000-0000-000000000000,1,0,3},3,0,0,0,0,0,0,0,0,
{0},
{0},
{0},
{3,"Заголовок",
{1,
{1,-1,0,-1,0,00000000-0000-0000-0000-000000000000},0},"ПустаяСтрока",
{1,
{1,-1,1,-1,1,00000000-0000-0000-0000-000000000000},0},"СодержаниеОтчета",
{1,
{1,-1,2,-1,2,00000000-0000-0000-0000-000000000000},0}
},"",
{
{0,6,6,
{"N",1000},7,
{"N",1000},8,
{"N",1000},9,
{"N",1000},10,
{"N",1000},11,
{"N",1000}
}
},
{0,-1,-1,-1,-1,00000000-0000-0000-0000-000000000000},0,0,0,0,0,0,0,1,0,1,3,
{32769,0,1},
{32768,1},
{128,509},1,
{7,0,575,80,0,0,0,700,0,0,0,0,0,0,0,0,"Arial",1,100},0,0,0,2,
{3,3,
{-1}
},
{3,3,
{-3}
},0,0,0,"",0,
{3,0,0,100,1,1,0,1,1,0,0,0,0,0,0,0,0,0,0,0,0,"",0,0,0,0,0,0,0},
{0},0,0,0,1,0,0,0}"#;

    #[test]
    fn writes_the_row_the_platform_stored_and_reads_it_back() {
        let xml = format!("\u{feff}{}", crlf(REPORT_XML));
        let body = write_native_moxel_body(xml.as_bytes(), None).unwrap();
        assert_eq!(body, crlf(REPORT_BODY));
        assert_eq!(read_back(&body), xml);
    }

    /// Документооборот КОРП `ПФ_MXL_ИтоговаяЗапись` keeps its repeating band
    /// in the first four scalars behind the print area; the exporter reads
    /// the row back to the same element.
    #[test]
    fn repeat_rows_fill_the_first_four_scalars_behind_the_print_area() {
        let xml = r#"<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:style="http://v8.1c.ru/8.1/data/ui/style">
	<columns><size>1</size></columns>
	<rowsItem><index>0</index><row><c><c><f>1</f></c></c></row></rowsItem>
	<repeatRows>
		<type>Rows</type>
		<beginRow>6</beginRow>
		<endRow>9</endRow>
		<beginColumn>0</beginColumn>
		<endColumn>0</endColumn>
	</repeatRows>
	<format><width>72</width></format>
</document>"#;
        let body = write_native_moxel_body(xml.as_bytes(), None)
            .unwrap()
            .replace("\r\n", "");
        assert!(
            body.contains("{0,-1,-1,-1,-1,00000000-0000-0000-0000-000000000000},6,9,0,0,0,0,"),
            "{body}"
        );
        let exported = read_back(&body);
        assert!(exported.contains("<repeatRows>\r\n\t\t<type>Rows</type>\r\n\t\t<beginRow>6</beginRow>\r\n\t\t<endRow>9</endRow>\r\n\t\t<beginColumn>0</beginColumn>\r\n\t\t<endColumn>0</endColumn>\r\n\t</repeatRows>"), "{exported}");

        let columns = xml.replace(
            "<beginColumn>0</beginColumn>",
            "<beginColumn>1</beginColumn>",
        );
        let error = write_native_moxel_body(columns.as_bytes(), None).unwrap_err();
        assert!(error.to_string().contains("names columns"), "{error:#}");
    }

    #[test]
    fn a_report_colour_and_its_own_value_are_two_palette_slots() {
        let xml = br#"<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:style="http://v8.1c.ru/8.1/data/ui/style">
	<columns><size>1</size></columns>
	<rowsItem><index>0</index><row><c><c><f>1</f></c></c><c><c><f>2</f></c></c></row></rowsItem>
	<format><backColor>style:ReportHeaderBackColor</backColor></format>
	<format><backColor>#F4ECC5</backColor></format>
</document>"#;
        let body = write_native_moxel_body(xml, None)
            .unwrap()
            .replace("\r\n", "");
        assert!(body.contains("4,{3,3,{-1}},{3,3,{-3}},{3,0,{12971252}},{3,0,{12971252}},"));
        assert!(body.contains("{1,2,{3,3,{-25}}}"));
        let exported = read_back(&body);
        assert!(exported.contains("<backColor>style:ReportHeaderBackColor</backColor>"));
        assert!(exported.contains("<backColor>#F4ECC5</backColor>"));
    }

    /// A source that answers the one style item the stub tree has; the
    /// moxel writer of a sheet without charts or configuration types reads
    /// nothing else.
    #[derive(Debug)]
    struct StubSource;

    const STUB_STYLE_ITEM: &str = "5A5A5A5A-1B1B-4C4C-8D8D-9E9E9E9E9E9E";

    impl MetadataSource for StubSource {
        fn resolve_style_item_uuid(&self, reference: &str) -> Result<String> {
            match reference {
                "StyleItem.Акцент" => Ok(STUB_STYLE_ITEM.to_string()),
                other => Err(anyhow!("the stub tree has no {other}")),
            }
        }
        fn source_root(&self) -> &std::path::Path {
            unreachable!("the writer reads no file")
        }
        fn read_source(
            &self,
            _: &std::path::Path,
        ) -> std::io::Result<ibcmd_metadata_source::SourceBytes> {
            unreachable!("the writer reads no file")
        }
        fn configuration_uuid(&self) -> Option<String> {
            unreachable!("the writer reads no configuration uuid")
        }
        fn configuration_object(
            &self,
            _: &str,
        ) -> Option<std::sync::Arc<ibcmd_form_native::ConfigurationObject>> {
            unreachable!("the writer reads no data path")
        }
        fn resolve_metadata_type_id(&self, _: &str) -> Result<String> {
            unreachable!("the sheet names no configuration type")
        }
        fn resolve_metadata_reference_uuid(&self, _: &str) -> Result<String> {
            unreachable!("the writer names no object")
        }
        fn resolve_common_picture_uuid(&self, _: &str) -> Result<String> {
            unreachable!("the sheet names no picture")
        }
        fn resolve_command_group_uuid(&self, _: &str) -> Result<String> {
            unreachable!("the writer names no command group")
        }
        fn dcs_generated_type_id(&self, _: &str) -> Option<String> {
            unreachable!("a sheet has no data-composition schema")
        }
        fn dcs_style_items(&self) -> &BTreeMap<String, String> {
            unreachable!("a sheet has no data-composition schema")
        }
        fn moxel_object_refs(&self) -> Result<BTreeMap<String, String>> {
            unreachable!("the sheet has no chart")
        }
        fn help_reference(&self, _: &str) -> Result<ibcmd_metadata_source::HelpReference> {
            unreachable!("a sheet has no help page")
        }
        fn interface_memo(
            &self,
        ) -> &std::sync::Mutex<ibcmd_metadata_source::InterfaceResolutionMemo> {
            unreachable!("a sheet has no command interface")
        }
        fn resolve_command_reference_uuid(&self, _: &str) -> Result<String> {
            unreachable!("the sheet names no command")
        }
        fn resolve_form_uuid(&self, _: &str) -> Result<String> {
            unreachable!("the sheet names no form")
        }
        fn resolve_metadata_child_uuid(&self, _: &str, _: &str, _: &str) -> Result<String> {
            unreachable!("the sheet names no child object")
        }
        fn resolve_predefined_item_id(&self, _: &str, _: &str) -> Result<String> {
            unreachable!("the sheet names no predefined item")
        }
        fn information_register_master_dimensions(&self, _: &str) -> Result<Vec<String>> {
            unreachable!("the sheet reads no register")
        }
        fn resolve_simple_metadata_uuid(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<String> {
            unreachable!("the sheet names no object")
        }
    }

    /// The writer reads the configuration only through `MetadataSource`: a
    /// stub that answers a style colour the way a tree on disk does gives the
    /// body the source context gives over that tree, and a resolver's error
    /// reaches the caller as the source gave it.
    #[test]
    fn a_stub_source_answers_the_writer_as_the_source_context_does() {
        let sheet = |colour: &str| {
            format!(
                r#"<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:style="http://v8.1c.ru/8.1/data/ui/style">
	<columns><size>1</size></columns>
	<rowsItem><index>0</index><row><c><c><f>1</f></c></c></row></rowsItem>
	<format><backColor>style:{colour}</backColor></format>
</document>"#
            )
        };
        let xml = sheet("Акцент");
        let through_stub = write_native_moxel_body(xml.as_bytes(), Some(&StubSource)).unwrap();
        assert!(
            through_stub
                .replace("\r\n", "")
                .contains("{3,3,{0,5a5a5a5a-1b1b-4c4c-8d8d-9e9e9e9e9e9e}}"),
            "{through_stub}"
        );

        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-moxel-seam-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("StyleItems")).unwrap();
        std::fs::write(
            root.join("StyleItems/Акцент.xml"),
            format!(
                "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\"><StyleItem uuid=\"{}\"><Properties><Name>Акцент</Name></Properties></StyleItem></MetaDataObject>",
                STUB_STYLE_ITEM.to_ascii_lowercase()
            ),
        )
        .unwrap();
        let context = crate::module_blob::MetadataSourceContext::new(root.clone());
        let through_context = write_native_moxel_body(xml.as_bytes(), Some(&context)).unwrap();
        assert_eq!(through_stub, through_context);

        // A style item neither has: the stub's own error, unchanged, and the
        // context's naming the file it could not read.
        let missing = sheet("Нет");
        let error = write_native_moxel_body(missing.as_bytes(), Some(&StubSource)).unwrap_err();
        assert_eq!(
            error.root_cause().to_string(),
            "the stub tree has no StyleItem.Нет"
        );
        let error = write_native_moxel_body(missing.as_bytes(), Some(&context)).unwrap_err();
        let chain = error.chain().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(
            chain[..chain.len() - 1],
            [format!(
                "failed to read StyleItem XML {}",
                root.join("StyleItems").join("Нет.xml").display()
            )],
            "{error:#}"
        );
        assert_eq!(
            error
                .root_cause()
                .downcast_ref::<std::io::Error>()
                .map(std::io::Error::kind),
            Some(std::io::ErrorKind::NotFound)
        );
    }
}
