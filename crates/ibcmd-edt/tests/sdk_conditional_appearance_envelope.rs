use formats_xml::form::{
    FormDialect, read_conditional_appearance_dcssca, read_form, read_list_settings_dcss,
    read_spreadsheet_mxlx, write_conditional_appearance_dcssca, write_list_settings_dcss,
    write_spreadsheet_mxlx,
};
use ibcmd_edt::{
    ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
};
use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
use morph1c_core::ir::form::{
    MxlLanguageInfo, MxlLanguageSettings, MxlNode, MxlSpreadsheetSettings,
};
use morph1c_core::ir::semantic_view::ConfigurationSemanticView;
use morph1c_core::ir::{
    Configuration, DcsItem, DcsListSettings, DcsSettingsGroup, FormBody, MetadataObject,
    NamedFormBody, ObjectKind, PropertyValue, Uuid,
};
use morph1c_core::version::{FormatVersion, with_roundtrip_target};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
use sha2::{Digest, Sha256};
use std::path::Path;

const SIDECAR: &str = "src/Reports/Appearance/Forms/List/ConditionalAppearance.dcssca";
fn items() -> Vec<DcsItem> {
    let head = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<ConditionalAppearance xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\">\r\n";
    let item = |value: &str| {
        format!(
            "\t<item>\r\n\t\t<selection/>\r\n\t\t<filter/>\r\n\t\t<appearance>\r\n\t\t\t<dcscor:item xsi:type=\"SettingsParameterValue\">\r\n\t\t\t\t<dcscor:parameter>Color</dcscor:parameter>\r\n\t\t\t\t<dcscor:value xsi:type=\"v8ui:Color\">{value}</dcscor:value>\r\n\t\t\t</dcscor:item>\r\n\t\t</appearance>\r\n\t</item>\r\n"
        )
    };
    read_conditional_appearance_dcssca(
        format!(
            "{head}{}{}</ConditionalAppearance>\r\n",
            item("style:SpecialTextColor"),
            item("web:Red")
        )
        .as_bytes(),
    )
    .unwrap()
    .0
}
fn configuration(body: FormBody, minor: u16) -> Configuration {
    let options = ConvertOptions::default().with_target_version(FormatVersion::new(2, minor));
    let mut config = read_config(
        Format::Designer,
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &options,
    )
    .unwrap()
    .0;
    let mut owner = MetadataObject::new(ObjectKind::new("Report"), "Appearance", Uuid([0x51; 16]));
    let mut reference =
        MetadataObject::new(ObjectKind::new("Report.FormRef"), "List", Uuid([0x52; 16]));
    reference.properties.push((
        morph1c_core::spec::metadata::report_form_ref::F_USE_PURPOSES,
        PropertyValue::List(vec![]),
    ));
    owner.children.push(reference);
    owner.form_bodies.push(NamedFormBody {
        name: "List".into(),
        body,
        ordinary_body: None,
        module: None,
        help: vec![],
        help_resources: vec![],
    });
    config.objects.push(owner);
    config
}
fn body(without: bool) -> FormBody {
    let mut body = FormBody::new();
    body.conditional_appearance = items();
    body.ca_envelope_without_lf_pal = without;
    body
}
fn semantic(config: &Configuration) -> Vec<u8> {
    serde_json::to_vec(&ConfigurationSemanticView {
        configuration: config,
        template_body: morph1c_pipeline::dcs_template_semantic_body,
    })
    .unwrap()
}
fn options(minor: u16) -> ConversionOptions {
    ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: format!("2.{minor}"),
        runtime_version: Some(if minor == 20 { "8.3.27" } else { "8.5.1" }.into()),
    }
}
fn source(minor: u16) -> SourceTree {
    source_with_body(minor, body(false))
}
fn source_with_body(minor: u16, body: FormBody) -> SourceTree {
    let directory = tempfile::tempdir().unwrap();
    let config = configuration(body, minor);
    with_roundtrip_target(FormatVersion::new(2, minor), || {
        write_config(Format::Designer, &config, directory.path()).unwrap()
    });
    let path = directory.path().join("Configuration.xml");
    let root = String::from_utf8(std::fs::read(&path).unwrap())
        .unwrap()
        .replace(
            "</Language>",
            "</Language>\r\n\t\t\t<Report>Appearance</Report>",
        );
    std::fs::write(path, root).unwrap();
    read_xml_source(directory.path(), ReaderLimits::default()).unwrap()
}
fn replace(tree: &SourceTree, path: &str, bytes: Vec<u8>) -> SourceTree {
    let mut entries = tree
        .entries()
        .iter()
        .filter(|entry| entry.path().as_str() != path)
        .cloned()
        .collect::<Vec<_>>();
    entries.push(SourceEntry::from_bytes(SourcePath::new(path).unwrap(), bytes).unwrap());
    SourceTree::new(entries).unwrap()
}
fn strip(tree: &SourceTree) -> SourceTree {
    SourceTree::new(
        tree.entries()
            .iter()
            .filter(|entry| !entry.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap()
}
fn sidecar(tree: &SourceTree) -> &[u8] {
    tree.entries()
        .iter()
        .find(|entry| entry.path().as_str() == SIDECAR)
        .unwrap()
        .bytes()
}

#[test]
fn namespace_flavors_reemit_exactly_and_only_the_flag_is_nonsemantic() {
    for minor in [20, 21] {
        let a = configuration(body(false), minor);
        let b = configuration(body(true), minor);
        assert_ne!(
            a.objects.last().unwrap().form_bodies[0].body,
            b.objects.last().unwrap().form_bodies[0].body
        );
        assert_eq!(semantic(&a), semantic(&b));
        let mut original = None;
        for without in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let config = configuration(body(without), minor);
            with_roundtrip_target(FormatVersion::new(2, minor), || {
                write_config(Format::Edt, &config, dir.path()).unwrap()
            });
            let path = dir
                .path()
                .join("Reports/Appearance/Forms/List/ConditionalAppearance.dcssca");
            let bytes = std::fs::read(path).unwrap();
            let (decoded, actual) = read_conditional_appearance_dcssca(&bytes).unwrap();
            assert_eq!(actual, without);
            assert_eq!(write_conditional_appearance_dcssca(&decoded, actual), bytes);
            let read = read_config(
                Format::Edt,
                dir.path(),
                &ConvertOptions::default().with_target_version(FormatVersion::new(2, minor)),
            )
            .unwrap()
            .0;
            assert_eq!(
                read.objects
                    .iter()
                    .find(|owner| owner.name == "Appearance")
                    .unwrap()
                    .form_bodies[0]
                    .body
                    .ca_envelope_without_lf_pal,
                without
            );
            let regenerated = tempfile::tempdir().unwrap();
            with_roundtrip_target(FormatVersion::new(2, minor), || {
                write_config(Format::Edt, &read, regenerated.path()).unwrap()
            });
            assert_eq!(
                std::fs::read(
                    regenerated
                        .path()
                        .join("Reports/Appearance/Forms/List/ConditionalAppearance.dcssca")
                )
                .unwrap(),
                bytes
            );
            if let Some(previous) = &original {
                assert_ne!(&bytes, previous);
            }
            original = Some(bytes);
        }
        for replacement in ["style:OtherColor", "web:Blue"] {
            let mut edited = body(false);
            let text = String::from_utf8(write_conditional_appearance_dcssca(
                &edited.conditional_appearance,
                false,
            ))
            .unwrap()
            .replace("style:SpecialTextColor", replacement);
            edited.conditional_appearance = read_conditional_appearance_dcssca(text.as_bytes())
                .unwrap()
                .0;
            assert_ne!(semantic(&configuration(edited, minor)), semantic(&a));
        }
        let mut changed = body(false);
        changed.conditional_appearance.reverse();
        assert_ne!(semantic(&configuration(changed, minor)), semantic(&a));
        let mut changed = body(false);
        changed.conditional_appearance.pop();
        assert_ne!(semantic(&configuration(changed, minor)), semantic(&a));
    }
}

#[test]
fn public_current_dcs_edits_cannot_replay_rehashed_original_source() {
    for minor in [20, 21] {
        let original = source(minor);
        let options = options(minor);
        let generated = xml_to_edt(&original, &options).unwrap().tree;
        assert_eq!(
            edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
                .unwrap()
                .tree,
            original
        );
        let (current_items, _) = read_conditional_appearance_dcssca(sidecar(&generated)).unwrap();
        let lexical = replace(
            &generated,
            SIDECAR,
            write_conditional_appearance_dcssca(&current_items, true),
        );
        // Raw-file accounting remains strict even for an unused namespace declaration.
        assert!(edt_to_xml(&Project::from_tree(lexical).unwrap(), &options).is_err());
        let edited = std::str::from_utf8(sidecar(&generated))
            .unwrap()
            .replace("style:SpecialTextColor", "web:Blue")
            .into_bytes();
        assert_ne!(edited, sidecar(&generated));
        let changed = replace(&generated, SIDECAR, edited.clone());
        let mut manifest: serde_json::Value = serde_json::from_slice(
            changed
                .entries()
                .iter()
                .find(|entry| entry.path().as_str() == ".ibcmd-provenance/manifest.json")
                .unwrap()
                .bytes(),
        )
        .unwrap();
        manifest["generated"][SIDECAR] =
            serde_json::Value::String(format!("{:x}", Sha256::digest(&edited)));
        let forged = replace(
            &changed,
            ".ibcmd-provenance/manifest.json",
            serde_json::to_vec(&manifest).unwrap(),
        );
        assert!(edt_to_xml(&Project::from_tree(forged).unwrap(), &options).is_err());
        let current = edt_to_xml(&Project::from_tree(strip(&changed)).unwrap(), &options)
            .unwrap()
            .tree;
        let returned = xml_to_edt(&current, &options).unwrap().tree;
        let (items, _) = read_conditional_appearance_dcssca(sidecar(&returned)).unwrap();
        let (expected, _) = read_conditional_appearance_dcssca(&edited).unwrap();
        assert_eq!(items, expected);
    }
}

#[test]
fn malformed_envelopes_and_unknown_body_are_still_rejected() {
    let valid = String::from_utf8(write_conditional_appearance_dcssca(&items(), false)).unwrap();
    for changed in [
        valid.replace(
            "http://v8.1c.ru/8.2/managed-application/logform",
            "urn:wrong",
        ),
        valid.replace(
            " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\"",
            "",
        ),
        valid.replace(
            "</ConditionalAppearance>",
            "<unknown/></ConditionalAppearance>",
        ),
    ] {
        assert_ne!(changed, valid);
        assert!(read_conditional_appearance_dcssca(changed.as_bytes()).is_err());
    }
}

const LIST_SIDECAR: &str =
    "src/Reports/Appearance/Forms/List/Attributes/Dynamic/ExtInfo/ListSettings.dcss";

#[test]
fn complete_appearance_accounting_accepts_lexical_changes_and_rejects_unknown_cells() {
    for minor in [20, 21] {
        let options = options(minor);
        let generated = strip(&xml_to_edt(&source(minor), &options).unwrap().tree);
        let original = std::str::from_utf8(sidecar(&generated)).unwrap();
        let changed = original
            .replace("<selection/>", "<selection></selection>")
            .replace("<filter/>", "<filter></filter>");
        assert_ne!(changed, original);
        let current = replace(&generated, SIDECAR, changed.into_bytes());
        let native = edt_to_xml(&Project::from_tree(current.clone()).unwrap(), &options)
            .unwrap()
            .tree;
        let returned = xml_to_edt(&native, &options).unwrap().tree;
        assert_eq!(
            read_conditional_appearance_dcssca(sidecar(&returned)).unwrap(),
            read_conditional_appearance_dcssca(sidecar(&current)).unwrap()
        );
        let directory = tempfile::tempdir().unwrap();
        ibcmd_xml::source_tree::publish_new(&current, directory.path().join("project")).unwrap();
        let native = ibcmd_edt::read_directory_project(directory.path().join("project"))
            .unwrap()
            .edt_to_xml(&options)
            .unwrap();
        native.publish_new(directory.path().join("native")).unwrap();
        let returned = ibcmd_edt::read_directory_source(directory.path().join("native"))
            .unwrap()
            .xml_to_edt(&options)
            .unwrap();
        returned
            .publish_new(directory.path().join("returned"))
            .unwrap();
        let returned =
            read_xml_source(directory.path().join("returned"), ReaderLimits::default()).unwrap();
        assert_eq!(
            read_conditional_appearance_dcssca(sidecar(&returned)).unwrap(),
            read_conditional_appearance_dcssca(sidecar(&current)).unwrap()
        );
        for invalid in [
            original.replace(
                "</ConditionalAppearance>",
                "<unknown/></ConditionalAppearance>",
            ),
            original.replace("<selection/>", "<selection unknown=\"true\"/>"),
            format!("{original}<another/>"),
            original.replace(
                "http://v8.1c.ru/8.1/data-composition-system/settings",
                "urn:wrong",
            ),
        ] {
            let input = replace(&generated, SIDECAR, invalid.into_bytes());
            assert!(
                Project::from_tree(input)
                    .and_then(|project| edt_to_xml(&project, &options))
                    .is_err()
            );
        }
    }
}
const MXL_SIDECAR: &str =
    "src/Reports/Appearance/Forms/List/Attributes/Sheet/ExtInfo/SpreadsheetData.mxlx";
const PAL_DECL: &str = " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\"";
fn list_settings(without: bool) -> DcsListSettings {
    DcsListSettings {
        envelope_without_pal: without,
        conditional_appearance: Some(DcsSettingsGroup {
            items: items(),
            ..Default::default()
        }),
        ..Default::default()
    }
}
fn spreadsheet_settings(without: bool) -> MxlSpreadsheetSettings {
    MxlSpreadsheetSettings {
        envelope_without_pal: without,
        language_settings: Some(MxlLanguageSettings {
            current_language: "ru".into(),
            default_language: "ru".into(),
            languages: vec![MxlLanguageInfo {
                id: "ru".into(),
                code: "Russian".into(),
                description: "Russian".into(),
            }],
        }),
        columns_size: "0".into(),
        rows_index: "0".into(),
        row_empty: true,
        template_mode: Some(true),
        vg_rows: "0".into(),
        full_body: None,
    }
}
fn attribute_body(without: bool) -> FormBody {
    let mut body = read_form(FormDialect::Edt, b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\"><attributes><name>Dynamic</name><id>1</id><valueType><types>DynamicList</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:DynamicListExtInfo\"/></attributes><attributes><name>Sheet</name><id>2</id><valueType><types>SpreadsheetDocument</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:SpreadsheetDocumentExtInfo\"/></attributes></form:Form>\r\n").unwrap();
    body.data_attributes[0]
        .dynamic_list
        .as_mut()
        .unwrap()
        .list_settings = Some(list_settings(without));
    body.data_attributes[1].spreadsheet_settings = Some(spreadsheet_settings(without));
    body
}
fn entry_bytes<'a>(tree: &'a SourceTree, path: &str) -> &'a [u8] {
    tree.entries()
        .iter()
        .find(|entry| entry.path().as_str() == path)
        .unwrap()
        .bytes()
}

#[test]
fn list_and_spreadsheet_namespace_flags_preserve_bytes_and_current_typed_content() {
    let list = list_settings(false);
    let mxl = spreadsheet_settings(false);
    let mut list_other = list.clone();
    list_other.envelope_without_pal = true;
    let mut mxl_other = mxl.clone();
    mxl_other.envelope_without_pal = true;
    assert_ne!(list, list_other);
    assert_ne!(mxl, mxl_other);
    assert_eq!(
        serde_json::to_vec(&list).unwrap(),
        serde_json::to_vec(&list_other).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(&mxl).unwrap(),
        serde_json::to_vec(&mxl_other).unwrap()
    );
    assert_eq!(
        String::from_utf8(write_list_settings_dcss(&list))
            .unwrap()
            .replace(PAL_DECL, "")
            .as_bytes(),
        write_list_settings_dcss(&list_other)
    );
    assert_eq!(
        String::from_utf8(write_spreadsheet_mxlx(&mxl))
            .unwrap()
            .replace(PAL_DECL, "")
            .as_bytes(),
        write_spreadsheet_mxlx(&mxl_other)
    );
    for without in [false, true] {
        let list = list_settings(without);
        let bytes = write_list_settings_dcss(&list);
        let decoded = read_list_settings_dcss(&bytes).unwrap();
        assert_eq!(decoded.envelope_without_pal, without);
        assert_eq!(write_list_settings_dcss(&decoded), bytes);
        let mxl = spreadsheet_settings(without);
        let bytes = write_spreadsheet_mxlx(&mxl);
        let decoded = read_spreadsheet_mxlx(&bytes).unwrap();
        assert_eq!(decoded.envelope_without_pal, without);
        assert_eq!(write_spreadsheet_mxlx(&decoded), bytes);
    }
    for minor in [20, 21] {
        let a = configuration(attribute_body(false), minor);
        let b = configuration(attribute_body(true), minor);
        assert_eq!(semantic(&a), semantic(&b));
        for without in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            with_roundtrip_target(FormatVersion::new(2, minor), || {
                write_config(
                    Format::Edt,
                    &configuration(attribute_body(without), minor),
                    directory.path(),
                )
                .unwrap()
            });
            let read = read_config(
                Format::Edt,
                directory.path(),
                &ConvertOptions::default().with_target_version(FormatVersion::new(2, minor)),
            )
            .unwrap()
            .0;
            let regenerate = tempfile::tempdir().unwrap();
            with_roundtrip_target(FormatVersion::new(2, minor), || {
                write_config(Format::Edt, &read, regenerate.path()).unwrap()
            });
            for path in [LIST_SIDECAR, MXL_SIDECAR] {
                let relative = path.strip_prefix("src/").unwrap();
                assert_eq!(
                    std::fs::read(directory.path().join(relative)).unwrap(),
                    std::fs::read(regenerate.path().join(relative)).unwrap()
                );
            }
        }
        for mutate in 0..7 {
            let mut edited = attribute_body(false);
            let ls = edited.data_attributes[0]
                .dynamic_list
                .as_mut()
                .unwrap()
                .list_settings
                .as_mut()
                .unwrap();
            match mutate {
                0 => ls.conditional_appearance.as_mut().unwrap().items.reverse(),
                1 => {
                    ls.conditional_appearance.as_mut().unwrap().items.pop();
                }
                2 => {
                    let text = String::from_utf8(write_list_settings_dcss(ls))
                        .unwrap()
                        .replace("style:SpecialTextColor", "web:Blue");
                    *ls = read_list_settings_dcss(text.as_bytes()).unwrap();
                }
                3 => {
                    edited.data_attributes[1]
                        .spreadsheet_settings
                        .as_mut()
                        .unwrap()
                        .columns_size = "1".into()
                }
                4 => {
                    edited.data_attributes[1]
                        .spreadsheet_settings
                        .as_mut()
                        .unwrap()
                        .row_empty = false
                }
                5 => {
                    edited.data_attributes[1]
                        .spreadsheet_settings
                        .as_mut()
                        .unwrap()
                        .language_settings
                        .as_mut()
                        .unwrap()
                        .current_language = "en".into()
                }
                _ => {
                    edited.data_attributes[1]
                        .spreadsheet_settings
                        .as_mut()
                        .unwrap()
                        .full_body = Some(vec![MxlNode {
                        prefix: String::new(),
                        namespace: "http://v8.1c.ru/8.2/data/spreadsheet".into(),
                        source_layout: None,
                        local: "columns".into(),
                        attrs: vec![],
                        children: vec![],
                        text: String::new(),
                        text_qname: None,
                        self_closing: true,
                    }])
                }
            }
            assert_ne!(semantic(&configuration(edited, minor)), semantic(&a));
        }
    }
}

#[test]
fn public_list_and_spreadsheet_edits_remain_authoritative_and_raw_accounting_is_strict() {
    for minor in [20, 21] {
        let original = source_with_body(minor, attribute_body(false));
        let options = options(minor);
        let generated = xml_to_edt(&original, &options).unwrap().tree;
        assert_eq!(
            edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
                .unwrap()
                .tree,
            original
        );
        for path in [LIST_SIDECAR, MXL_SIDECAR] {
            let lexical = std::str::from_utf8(entry_bytes(&generated, path))
                .unwrap()
                .replace(PAL_DECL, "")
                .into_bytes();
            assert_ne!(lexical, entry_bytes(&generated, path));
            assert!(
                edt_to_xml(
                    &Project::from_tree(replace(&generated, path, lexical)).unwrap(),
                    &options
                )
                .is_err()
            );
            let text = std::str::from_utf8(entry_bytes(&generated, path)).unwrap();
            let edited = if path == LIST_SIDECAR {
                text.replace("style:SpecialTextColor", "web:Blue")
            } else {
                text.replace("<size>0</size>", "<size>1</size>")
            }
            .into_bytes();
            assert_ne!(edited, entry_bytes(&generated, path));
            let changed = replace(&generated, path, edited.clone());
            let mut manifest: serde_json::Value =
                serde_json::from_slice(entry_bytes(&generated, ".ibcmd-provenance/manifest.json"))
                    .unwrap();
            manifest["generated"][path] =
                serde_json::Value::String(format!("{:x}", Sha256::digest(&edited)));
            let forged = replace(
                &changed,
                ".ibcmd-provenance/manifest.json",
                serde_json::to_vec(&manifest).unwrap(),
            );
            assert!(edt_to_xml(&Project::from_tree(forged).unwrap(), &options).is_err());
            let current = edt_to_xml(&Project::from_tree(strip(&changed)).unwrap(), &options)
                .unwrap()
                .tree;
            let returned = xml_to_edt(&current, &options).unwrap().tree;
            if path == LIST_SIDECAR {
                assert_eq!(
                    serde_json::to_vec(
                        &read_list_settings_dcss(entry_bytes(&returned, path)).unwrap()
                    )
                    .unwrap(),
                    serde_json::to_vec(&read_list_settings_dcss(&edited).unwrap()).unwrap()
                );
            } else {
                assert_eq!(
                    serde_json::to_vec(
                        &read_spreadsheet_mxlx(entry_bytes(&returned, path)).unwrap()
                    )
                    .unwrap(),
                    serde_json::to_vec(&read_spreadsheet_mxlx(&edited).unwrap()).unwrap()
                );
            }
        }
    }
}

#[test]
fn list_and_spreadsheet_wrong_namespace_or_unclaimed_root_attributes_are_rejected() {
    let list = String::from_utf8(write_list_settings_dcss(&list_settings(false))).unwrap();
    let mxl = String::from_utf8(write_spreadsheet_mxlx(&spreadsheet_settings(false))).unwrap();
    for original in [list, mxl] {
        for changed in [
            original.replace("http://v8.1c.ru/8.1/data/ui/colors/palette", "urn:wrong"),
            original.replace(" xmlns:pal=", " unknown=\"true\" xmlns:pal="),
        ] {
            assert_ne!(changed, original);
            if original.contains("<Settings ") {
                assert!(read_list_settings_dcss(changed.as_bytes()).is_err());
            } else {
                assert!(read_spreadsheet_mxlx(changed.as_bytes()).is_err());
            }
        }
    }
    let list = String::from_utf8(write_list_settings_dcss(&list_settings(false)))
        .unwrap()
        .replace("</Settings>", "<unknown/></Settings>");
    assert!(read_list_settings_dcss(list.as_bytes()).is_err());
}
