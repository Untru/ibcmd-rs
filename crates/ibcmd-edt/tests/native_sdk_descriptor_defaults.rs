use formats_xml::configuration::{ConfigDialect, decode_child_objects, emit_child_objects};
use morph1c_core::{
    engine::Decoded,
    ir::{MetadataObject, ObjectKind, PropertyValue, Uuid},
    version::{FormatVersion, with_roundtrip_target},
};
use morph1c_pipeline::{
    ConvertOptions, Format, read_config, registry::FormatRegistry, write_config,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn read(format: Format, kind: &str, source: &[u8]) -> Result<MetadataObject, String> {
    (FormatRegistry::for_format(format)
        .unwrap()
        .get(kind)
        .unwrap()
        .read)(source)
}

fn write(format: Format, object: &MetadataObject, version: FormatVersion) -> Vec<u8> {
    with_roundtrip_target(version, || {
        (FormatRegistry::for_format(format)
            .unwrap()
            .get(object.kind.as_str())
            .unwrap()
            .write)(object)
    })
    .unwrap()
}

#[test]
fn configuration_roster_keeps_every_inline_language_in_source_order() {
    for names in [
        vec![],
        vec!["Russian"],
        vec!["English", "Russian", "French"],
    ] {
        let mut source = "<Configuration>".to_string();
        for name in &names {
            source.push_str(&format!("<languages><name>{name}</name></languages>"));
        }
        source.push_str(
            "<catalogs>Catalog.Z</catalogs><catalogs>Catalog.A</catalogs></Configuration>",
        );
        let document = formats_xml::parse(source.as_bytes()).unwrap();
        let Decoded::Present(roster) = decode_child_objects(ConfigDialect::Edt, &document.root)
        else {
            panic!("valid child roster did not decode");
        };
        let native = emit_child_objects(ConfigDialect::Designer, &roster).unwrap();
        let children = &native[0].children;
        assert_eq!(children.len(), names.len() + 2);
        for (element, name) in children.iter().zip(&names) {
            assert_eq!(element.local, "Language");
            assert_eq!(element.text.as_deref(), Some(*name));
        }
        assert_eq!(children[names.len()].text.as_deref(), Some("Z"));
        assert_eq!(children[names.len() + 1].text.as_deref(), Some("A"));
        let returned = emit_child_objects(ConfigDialect::Edt, &roster).unwrap();
        assert_eq!(returned.len(), 2);
        assert_eq!(returned[0].text.as_deref(), Some("Catalog.Z"));
    }
}

fn chart_of_accounts(synonym: &str) -> Vec<u8> {
    let attributes = ["TurnoversOnly", "Predefined", "ExtDimensionType", "LineNumber"]
        .map(|name| format!("<standardAttributes><name>{name}</name><fillValue xsi:type='core:UndefinedValue'/><minValue xsi:type='core:UndefinedValue'/><maxValue xsi:type='core:UndefinedValue'/></standardAttributes>"))
        .join("");
    format!(
        "<mdclass:ChartOfAccounts xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance' xmlns:core='http://g5.1c.ru/v8/dt/mcore' xmlns:mdclass='http://g5.1c.ru/v8/dt/metadata/mdclass' uuid='11111111-1111-1111-1111-111111111111'><name>Plan</name><standardTabularSections><name>ExtDimensionTypes</name>{synonym}{attributes}</standardTabularSections></mdclass:ChartOfAccounts>"
    ).into_bytes()
}

#[test]
fn sparse_standard_section_default_custom_and_empty_synonyms_survive_both_formats() {
    use morph1c_core::spec::metadata::chart_of_accounts::F_STANDARD_TABULAR_SECTIONS;
    for (source_leaf, expected) in [
        ("", vec![]),
        (
            "<synonym><key></key><value>Виды субконто</value></synonym>",
            vec![("", "Виды субконто")],
        ),
        (
            "<synonym><key>ru</key><value>Текущий</value></synonym><synonym><key>en</key><value>Current</value></synonym>",
            vec![("ru", "Текущий"), ("en", "Current")],
        ),
        ("<synonym/>", vec![]),
    ] {
        let object = read(
            Format::Edt,
            "ChartOfAccounts",
            &chart_of_accounts(source_leaf),
        )
        .unwrap();
        let value = object.get(F_STANDARD_TABULAR_SECTIONS).unwrap().clone();
        let PropertyValue::List(sections) = &value else {
            panic!("section list")
        };
        let PropertyValue::List(section) = &sections[0] else {
            panic!("section record")
        };
        let PropertyValue::Localized(synonyms) = &section[0] else {
            panic!("localized synonym")
        };
        assert_eq!(
            synonyms
                .iter()
                .map(|(lang, text)| (lang.as_str(), text.as_str()))
                .collect::<Vec<_>>(),
            expected
        );
        for version in [FormatVersion::new(2, 20), FormatVersion::new(2, 21)] {
            let native = write(Format::Designer, &object, version);
            let native_object = read(Format::Designer, "ChartOfAccounts", &native).unwrap();
            assert_eq!(native_object.get(F_STANDARD_TABULAR_SECTIONS), Some(&value));
            let edt = write(Format::Edt, &native_object, version);
            assert_eq!(
                read(Format::Edt, "ChartOfAccounts", &edt)
                    .unwrap()
                    .get(F_STANDARD_TABULAR_SECTIONS),
                Some(&value)
            );
        }
    }
    let invalid = String::from_utf8(chart_of_accounts(""))
        .unwrap()
        .replace("<name>LineNumber</name>", "<name>Unknown</name>");
    assert!(read(Format::Edt, "ChartOfAccounts", invalid.as_bytes()).is_err());
}

#[test]
fn empty_data_processor_section_keeps_native_child_container() {
    let mut object = MetadataObject::new(ObjectKind::new("DataProcessor"), "Empty", Uuid([1; 16]));
    object.children.push(MetadataObject::new(
        ObjectKind::new("DataProcessor.TabularSection"),
        "Rows",
        Uuid([2; 16]),
    ));
    for version in [FormatVersion::new(2, 20), FormatVersion::new(2, 21)] {
        let native = write(Format::Designer, &object, version);
        let document = formats_xml::parse(&native).unwrap();
        let section = document
            .root
            .child("DataProcessor")
            .unwrap()
            .child("ChildObjects")
            .unwrap()
            .child("TabularSection")
            .unwrap();
        assert!(section.child("ChildObjects").unwrap().children.is_empty());
        let decoded = read(Format::Designer, "DataProcessor", &native).unwrap();
        assert_eq!(decoded.children[0].name, "Rows");
        assert_eq!(decoded.children[0].uuid, Uuid([2; 16]));
        assert!(decoded.children[0].children.is_empty());
        let edt = write(Format::Edt, &decoded, version);
        let returned = read(Format::Edt, "DataProcessor", &edt).unwrap();
        assert_eq!(returned.children, decoded.children);
    }
}

fn row(kind: &str, name: &str) -> PropertyValue {
    PropertyValue::List(vec![
        PropertyValue::Str(kind.into()),
        PropertyValue::Str(name.into()),
    ])
}

fn root_semantics(roster: PropertyValue) -> Vec<u8> {
    use morph1c_core::{ir::Configuration, spec::metadata::configuration as cfg};
    let mut root = MetadataObject::new(ObjectKind::new("Configuration"), "Root", Uuid([1; 16]));
    root.properties.push((cfg::F_CHILD_OBJECTS, roster));
    let configuration = Configuration {
        source_version: None,
        properties: vec![],
        objects: vec![root],
    };
    serde_json::to_vec(
        &morph1c_core::ir::semantic_view::ConfigurationSemanticView {
            configuration: &configuration,
            template_body: morph1c_pipeline::dcs_template_semantic_body,
        },
    )
    .unwrap()
}

#[test]
fn bot_kind_placement_preserves_current_names_and_other_kind_interleaving() {
    use formats_xml::configuration::emit_child_objects_versioned;
    let source = PropertyValue::List(vec![
        row("DefinedType", "D"),
        row("CommonCommand", "C"),
        row("InformationRegister", "I"),
        row("ChartOfAccounts", "P"),
        row("AccountingRegister", "R"),
        row("PaletteColor", "Z"),
        row("PaletteColor", "A"),
        row("Bot", "B2"),
        row("Bot", "B1"),
    ]);
    for version in [FormatVersion::new(2, 20), FormatVersion::new(2, 21)] {
        let output =
            emit_child_objects_versioned(ConfigDialect::Designer, &source, version).unwrap();
        let names = output[0]
            .children
            .iter()
            .map(|element| element.text.as_deref().unwrap())
            .collect::<Vec<_>>();
        if version.minor == 21 {
            assert_eq!(names, ["D", "Z", "A", "B2", "B1", "C", "I", "P", "R"]);
        } else {
            assert_eq!(names, ["D", "B2", "B1", "C", "I", "P", "R", "Z", "A"]);
        }
        let returned = PropertyValue::List(
            output[0]
                .children
                .iter()
                .map(|element| row(&element.local, element.text.as_deref().unwrap()))
                .collect(),
        );
        assert_eq!(root_semantics(source.clone()), root_semantics(returned));
    }
    let mut edited = source.clone();
    let PropertyValue::List(rows) = &mut edited else {
        unreachable!()
    };
    rows.swap(7, 8);
    assert_ne!(root_semantics(source.clone()), root_semantics(edited));
    let mut edited = source.clone();
    let PropertyValue::List(rows) = &mut edited else {
        unreachable!()
    };
    rows[7] = row("Bot", "Current");
    assert_ne!(root_semantics(source), root_semantics(edited));
}

fn load_configuration(
    format: Format,
    path: &Path,
    options: &ConvertOptions,
) -> Result<morph1c_core::ir::Configuration, morph1c_pipeline::ConvertError> {
    read_config(format, path, options).map(|(configuration, _)| configuration)
}

fn seed() -> morph1c_core::ir::Configuration {
    load_configuration(
        Format::Designer,
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &ConvertOptions::default(),
    )
    .unwrap()
}

#[test]
#[ignore = "prepares new disposable F lab inputs for native standard-section import research"]
fn prepare_native_standard_section_synonym_probe() {
    use morph1c_core::spec::metadata::{chart_of_accounts as coa, configuration as cfg};
    let output =
        PathBuf::from(std::env::var_os("IBCMD_SYNONYM_PROBE_INPUT").expect("fresh F input"));
    assert!(output.is_absolute());
    assert!(
        output
            .to_string_lossy()
            .to_ascii_lowercase()
            .starts_with("f:")
    );
    std::fs::create_dir(&output).unwrap();
    let cases = [
        ("AbsentBlock", None),
        ("Empty", Some("")),
        (
            "Builtin",
            Some("<synonym><key></key><value>Виды субконто</value></synonym>"),
        ),
        (
            "CustomNeutral",
            Some("<synonym><key></key><value>Текущий заголовок</value></synonym>"),
        ),
        (
            "CustomRussian",
            Some("<synonym><key>ru</key><value>Новый русский</value></synonym>"),
        ),
        (
            "Multilingual",
            Some(
                "<synonym><key>ru</key><value>Текущий русский</value></synonym><synonym><key>en</key><value>Current English</value></synonym>",
            ),
        ),
    ];
    let mut receipts = vec![];
    for version in [FormatVersion::new(2, 20), FormatVersion::new(2, 21)] {
        let mut configuration = seed();
        configuration
            .objects
            .retain(|object| matches!(object.kind.as_str(), "Configuration" | "Language"));
        for (index, (name, leaf)) in cases.iter().enumerate() {
            let mut object = read(
                Format::Edt,
                "ChartOfAccounts",
                &chart_of_accounts(leaf.unwrap_or("")),
            )
            .unwrap();
            object.name = (*name).into();
            object.uuid = Uuid([index as u8 + 40; 16]);
            object.internal_info = Some(morph1c_core::ir::InternalInfo {
                generated_types: [
                    "Object",
                    "Ref",
                    "Selection",
                    "List",
                    "Manager",
                    "ExtDimensionTypes",
                    "ExtDimensionTypesRow",
                ]
                .iter()
                .enumerate()
                .map(|(role, category)| {
                    let mut type_id = [0_u8; 16];
                    type_id[0] = index as u8 + 80;
                    type_id[1] = role as u8 + 1;
                    let mut value_id = type_id;
                    value_id[2] = 1;
                    morph1c_core::ir::GeneratedType {
                        category: (*category).into(),
                        type_id: Uuid(type_id),
                        value_id: Uuid(value_id),
                    }
                })
                .collect(),
            });
            if leaf.is_none() {
                object
                    .properties
                    .retain(|(field, _)| *field != coa::F_STANDARD_TABULAR_SECTIONS);
            }
            configuration.objects.push(object);
        }
        let root = configuration
            .objects
            .iter_mut()
            .find(|object| object.kind.as_str() == "Configuration")
            .unwrap();
        let roster = root
            .properties
            .iter_mut()
            .find(|(field, _)| *field == cfg::F_CHILD_OBJECTS)
            .unwrap();
        roster.1 = PropertyValue::List(
            std::iter::once(row("Language", "Русский"))
                .chain(cases.iter().map(|(name, _)| row("ChartOfAccounts", name)))
                .collect(),
        );
        let target = output.join(format!("native-{}", version.minor));
        write_configuration(Format::Designer, &configuration, &target, version);
        let verified = load_configuration(
            Format::Designer,
            &target,
            &ConvertOptions::default().with_target_version(version),
        )
        .unwrap();
        for (name, _) in cases {
            let original = configuration
                .objects
                .iter()
                .find(|object| object.name == name)
                .unwrap();
            let reread = verified
                .objects
                .iter()
                .find(|object| object.name == name)
                .unwrap();
            assert_eq!(
                reread.get(coa::F_STANDARD_TABULAR_SECTIONS),
                original.get(coa::F_STANDARD_TABULAR_SECTIONS)
            );
            let path = target.join(format!("ChartsOfAccounts/{name}.xml"));
            receipts.push(serde_json::json!({"version":version.minor,"name":name,"input":path,"sha256":format!("{:x}", Sha256::digest(std::fs::read(&path).unwrap()))}));
        }
    }
    std::fs::write(output.join("preparation.json"), serde_json::to_vec_pretty(&serde_json::json!({"scope":"synthetic typed-current inputs only; native import/export outcomes pending", "rows":receipts})).unwrap()).unwrap();
}

fn write_configuration(
    format: Format,
    configuration: &morph1c_core::ir::Configuration,
    target: &Path,
    version: FormatVersion,
) {
    with_roundtrip_target(version, || write_config(format, configuration, target)).unwrap();
}

fn style_seed() -> morph1c_core::ir::Configuration {
    use morph1c_core::{
        ir::{ColorStyle, StyleRecord, StyleRecordValue},
        spec::metadata::configuration as cfg,
    };
    let mut configuration = seed();
    let root = configuration
        .objects
        .iter_mut()
        .find(|object| object.kind.as_str() == "Configuration")
        .unwrap();
    let (_, PropertyValue::List(rows)) = root
        .properties
        .iter_mut()
        .find(|(field, _)| *field == cfg::F_CHILD_OBJECTS)
        .unwrap()
    else {
        panic!("roster")
    };
    rows.push(row("Style", "Current"));
    let mut style = MetadataObject::new(ObjectKind::new("Style"), "Current", Uuid([8; 16]));
    style.style_records.push(StyleRecord {
        name: "FormBackColor".into(),
        value: StyleRecordValue::Color(ColorStyle::Ref("Web.Cream".into())),
    });
    configuration.objects.push(style);
    configuration
}

#[test]
fn style_namespace_uses_target_profile_and_validates_source_presence_exactly() {
    for version in [FormatVersion::new(2, 20), FormatVersion::new(2, 21)] {
        let root = tempfile::tempdir().unwrap();
        let native = root.path().join("native");
        write_configuration(Format::Designer, &style_seed(), &native, version);
        let path = native.join("Styles/Current/Ext/Style.xml");
        let source = std::fs::read(&path).unwrap();
        let text = String::from_utf8(source.clone()).unwrap();
        assert_eq!(text.contains("xmlns:pal="), version.minor == 21);
        let options = ConvertOptions::default().with_target_version(version);
        let decoded = load_configuration(Format::Designer, &native, &options).unwrap();
        let edt = root.path().join("edt");
        write_configuration(Format::Edt, &decoded, &edt, version);
        let returned = load_configuration(Format::Edt, &edt, &options).unwrap();
        let current = root.path().join("returned");
        write_configuration(Format::Designer, &returned, &current, version);
        assert_eq!(
            std::fs::read(current.join("Styles/Current/Ext/Style.xml")).unwrap(),
            source
        );
        if version.minor == 21 {
            std::fs::write(
                &path,
                text.replace(
                    "xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\" ",
                    "",
                ),
            )
            .unwrap();
            let absent = load_configuration(Format::Designer, &native, &options).unwrap();
            let own = root.path().join("own-absent-palette");
            write_configuration(Format::Designer, &absent, &own, version);
            assert_eq!(
                std::fs::read(own.join("Styles/Current/Ext/Style.xml")).unwrap(),
                std::fs::read(&path).unwrap()
            );
            std::fs::write(
                &path,
                text.replace("http://v8.1c.ru/8.1/data/ui/colors/palette", "urn:wrong"),
            )
            .unwrap();
            assert!(read_config(Format::Designer, &native, &options).is_err());
        }
        std::fs::write(&path, text.replace("version=", "unknown='yes' version=")).unwrap();
        assert!(read_config(Format::Designer, &native, &options).is_err());
    }
}

#[test]
fn empty_style_sidecars_survive_both_dialects_and_current_item_edits() {
    use morph1c_core::ir::{ColorStyle, StyleRecord, StyleRecordValue};
    for version in [FormatVersion::new(2, 20), FormatVersion::new(2, 21)] {
        for format in [Format::Designer, Format::Edt] {
            for self_closing in [false, true] {
                let temporary = tempfile::tempdir().unwrap();
                let mut configuration = style_seed();
                let style = configuration
                    .objects
                    .iter_mut()
                    .find(|o| o.kind.as_str() == "Style")
                    .unwrap();
                style.style_records.clear();
                style.style_sidecar_present = true;
                style.style_empty_root = Some((format == Format::Designer, self_closing));
                let input = temporary.path().join("input");
                write_configuration(format, &configuration, &input, version);
                let relative = if format == Format::Designer {
                    "Styles/Current/Ext/Style.xml"
                } else {
                    "Styles/Current/Style.style"
                };
                let source = std::fs::read(input.join(relative)).unwrap();
                assert_eq!(
                    source.ends_with(b"/>") || source.ends_with(b"/>\r\n"),
                    self_closing
                );
                let options = ConvertOptions::default().with_target_version(version);
                let mut decoded = load_configuration(format, &input, &options).unwrap();
                let style = decoded
                    .objects
                    .iter()
                    .find(|o| o.kind.as_str() == "Style")
                    .unwrap();
                assert!(style.style_records.is_empty() && style.style_sidecar_present);
                let own = temporary.path().join("own");
                write_configuration(format, &decoded, &own, version);
                assert_eq!(std::fs::read(own.join(relative)).unwrap(), source);

                let opposite = if format == Format::Designer {
                    Format::Edt
                } else {
                    Format::Designer
                };
                let other = temporary.path().join("other");
                write_configuration(opposite, &decoded, &other, version);
                let returned = load_configuration(opposite, &other, &options).unwrap();
                let returned_style = returned
                    .objects
                    .iter()
                    .find(|o| o.kind.as_str() == "Style")
                    .unwrap();
                assert!(
                    returned_style.style_records.is_empty() && returned_style.style_sidecar_present
                );

                let current = StyleRecord {
                    name: "FormBackColor".into(),
                    value: StyleRecordValue::Color(ColorStyle::Ref("Web.Cream".into())),
                };
                decoded
                    .objects
                    .iter_mut()
                    .find(|o| o.kind.as_str() == "Style")
                    .unwrap()
                    .style_records
                    .push(current.clone());
                let changed = temporary.path().join("changed");
                write_configuration(format, &decoded, &changed, version);
                let changed = load_configuration(format, &changed, &options).unwrap();
                assert_eq!(
                    changed
                        .objects
                        .iter()
                        .find(|o| o.kind.as_str() == "Style")
                        .unwrap()
                        .style_records,
                    vec![current]
                );
            }
            let temporary = tempfile::tempdir().unwrap();
            let mut absent = style_seed();
            absent
                .objects
                .iter_mut()
                .find(|o| o.kind.as_str() == "Style")
                .unwrap()
                .style_records
                .clear();
            write_configuration(format, &absent, &temporary.path().join("absent"), version);
            let loaded = load_configuration(
                format,
                &temporary.path().join("absent"),
                &ConvertOptions::default().with_target_version(version),
            )
            .unwrap();
            assert!(
                !loaded
                    .objects
                    .iter()
                    .find(|o| o.kind.as_str() == "Style")
                    .unwrap()
                    .style_sidecar_present
            );
        }
    }
}

#[test]
#[ignore = "requires immutable genuine UH83/UH85 source/SDK captures"]
fn genuine_root_metadata_and_style_match_native_bytes_in_both_profiles() {
    let lab = PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").expect("lab"));
    let output = PathBuf::from(std::env::var_os("IBCMD_DESCRIPTOR_REPORT").expect("report"));
    assert!(!output.exists());
    let hash = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    let mut results = vec![];
    for (corpus, oracle, reference, minor) in [
        ("uha83", "oracle-uha83-r1", "native-reference-uha83-r1", 20),
        (
            "uha85",
            "oracle-uha85-r2",
            "native-reference-uha85-affinity-r1",
            21,
        ),
    ] {
        let version = FormatVersion::new(2, minor);
        let source_root = lab
            .join(oracle)
            .join("authentic-workspace/OracleConfiguration/src");
        let sdk_root = lab.join(reference).join("native-xml");
        let report = lab.join(format!("convert-{corpus}-b4863be7-full-pair-r1/direct-native-sdk-configuration-data-comparison.json"));
        let comparison_bytes = std::fs::read(report).unwrap();
        let comparison: serde_json::Value = serde_json::from_slice(&comparison_bytes).unwrap();
        let paths = comparison["different"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|row| row["path"].as_str())
            .filter(|path| {
                *path == "Configuration.xml"
                    || path.starts_with("ChartsOfAccounts/") && !path.contains("/Ext/")
                    || path.starts_with("DataProcessors/") && !path.contains("/Ext/")
                    || path.starts_with("Styles/")
            })
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), 5);
        for path in paths {
            let native_path = Path::new(path);
            let expected = std::fs::read(sdk_root.join(path)).unwrap();
            let (source_path, actual, source_before) = if path.starts_with("Styles/") {
                let source_path = source_root.join("Styles/Основной/Style.style");
                let bytes = std::fs::read(&source_path).unwrap();
                let root = tempfile::tempdir().unwrap();
                let input = root.path().join("edt");
                write_configuration(Format::Edt, &style_seed(), &input, version);
                std::fs::write(input.join("Styles/Current/Style.style"), &bytes).unwrap();
                let configuration = load_configuration(
                    Format::Edt,
                    &input,
                    &ConvertOptions::default().with_target_version(version),
                )
                .unwrap();
                let out = root.path().join("native");
                write_configuration(Format::Designer, &configuration, &out, version);
                (
                    source_path,
                    std::fs::read(out.join("Styles/Current/Ext/Style.xml")).unwrap(),
                    bytes,
                )
            } else {
                let source_path = if path == "Configuration.xml" {
                    source_root.join("Configuration/Configuration.mdo")
                } else {
                    let name = native_path.file_stem().unwrap().to_str().unwrap();
                    source_root
                        .join(native_path.parent().unwrap())
                        .join(name)
                        .join(format!("{name}.mdo"))
                };
                let source = std::fs::read(&source_path).unwrap();
                let kind = if path == "Configuration.xml" {
                    "Configuration"
                } else if path.starts_with("ChartsOfAccounts/") {
                    "ChartOfAccounts"
                } else {
                    "DataProcessor"
                };
                let object = read(Format::Edt, kind, &source).unwrap();
                (
                    source_path,
                    write(Format::Designer, &object, version),
                    source,
                )
            };
            let source = std::fs::read(&source_path).unwrap();
            assert_eq!(
                source, source_before,
                "genuine source changed during descriptor conversion"
            );
            results.push(serde_json::json!({"corpus":corpus,"path":path,"source_sha256":hash(&source_before),"expected_sha256":hash(&expected),"actual_sha256":hash(&actual),"exact":actual==expected,"source_unchanged":true,"comparison_sha256":hash(&comparison_bytes)}));
            assert_eq!(std::fs::read(sdk_root.join(path)).unwrap(), expected);
        }
    }
    use std::io::Write;
    let mut report_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .unwrap();
    report_file.write_all(&serde_json::to_vec_pretty(&serde_json::json!({"scope":"10 root metadata/style pairs only; not full acceptance","executable_sha256":hash(&std::fs::read(std::env::current_exe().unwrap()).unwrap()),"rows":results})).unwrap()).unwrap();
    assert!(
        results.iter().all(|row| row["exact"] == true),
        "genuine descriptor comparison failed; inspect scoped report"
    );
}
