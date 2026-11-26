//! Tests added by the onecdec fork, kept apart from the upstream file.

use super::*;

/// Every home page the platform stored (fixtures `home_page/*`: its own XML
/// and the record it saved) compiles back to that record byte for byte --
/// the templates OneColumn (`<Column>`) and TwoColumnsEqualWidth, and
/// `<MACommandInterfaceDisplays>` (0 Top, 1 Bottom, 2 when absent), not only
/// TwoColumnsVariableWidth.
#[test]
fn compiles_every_stored_home_page_work_area() {
    let forms = [
        (
            "CommonForm.ФормаСлева",
            "5b6f3a52-0c0e-4d0b-9d49-3f7f2f8e1a11",
        ),
        (
            "CommonForm.ФормаСправа",
            "5b6f3a52-0c0e-4d0b-9d49-3f7f2f8e1a12",
        ),
    ];
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/external/home_page");
    for case in [
        "one_column",
        "one_column_top",
        "two_equal",
        "two_equal_bottom",
        "two_variable",
    ] {
        let mut xml = fs::read_to_string(fixtures.join(case).join("HomePageWorkArea.xml")).unwrap();
        for (name, uuid) in forms {
            xml = xml.replace(
                &format!("<Form>{name}</Form>"),
                &format!("<Form>{uuid}</Form>"),
            );
        }
        let root = crate::test_scratch::TestScratch::new(&format!("ibcmd-rs-home-page-{case}"));
        let body_path = root.join("Ext").join("HomePageWorkArea.xml");
        fs::create_dir_all(body_path.parent().unwrap()).unwrap();
        fs::write(&body_path, xml).unwrap();
        let properties = SimpleMetadataXmlProperties {
            kind: "Configuration".to_string(),
            uuid: "ffffffff-ffff-4fff-ffff-ffffffffffff".to_string(),
            name: "Main".to_string(),
            synonyms: Vec::new(),
            comment: String::new(),
        };
        let rows = prepare_configuration_interface_asset_body_row(
            &properties,
            body_path,
            "8",
            InterfaceAssetSource::HomePageWorkArea,
            "HomePageWorkArea",
            None,
            &legacy_non_xml_compile_axes(),
        )
        .unwrap_or_else(|error| panic!("{case}: {error:#}"));
        let compiled = crate::module_blob::inflate_raw(&rows[0].blob).unwrap();
        let stored = fs::read(fixtures.join(case).join("stored.txt")).unwrap();
        assert!(
            compiled == stored,
            "{case}\n--- stored\n{}\n--- compiled\n{}",
            String::from_utf8_lossy(&stored),
            String::from_utf8_lossy(&compiled)
        );
    }
}
