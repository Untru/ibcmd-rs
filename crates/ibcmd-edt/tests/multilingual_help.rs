use formats_xml::{help, read::parse};
use morph1c_core::{
    ir::{
        FormBody, HelpPage, HelpResource, MetadataObject, NamedFormBody, ObjectKind, PropertyValue,
        Token, Uuid,
    },
    spec::registry::spec_for,
    version::FormatVersion,
};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};

fn ids() -> PropertyValue {
    PropertyValue::List(["ru", "en"].map(|s| PropertyValue::Str(s.into())).to_vec())
}
fn pages() -> Vec<HelpPage> {
    ["ru", "en"]
        .map(|lang| HelpPage {
            lang: lang.into(),
            body: format!("<html>{lang}\ncomplete</html>"),
        })
        .to_vec()
}
fn prop(obj: &mut MetadataObject, name: &str, value: PropertyValue) {
    let field = spec_for(obj.kind.as_str())
        .unwrap()
        .fields()
        .iter()
        .find(|f| f.name == name)
        .unwrap()
        .id;
    obj.properties.push((field, value));
}

#[test]
fn ordered_help_languages_fully_claim_every_page_and_reject_unknown_shapes() {
    let src = b"<help><pages><lang>ru</lang></pages><pages><lang>en</lang></pages></help>";
    let doc = parse(src).unwrap();
    assert_eq!(help::decode(&doc.root).unwrap(), ids());
    let emitted = help::emit("", "help", &ids()).unwrap().unwrap();
    assert_eq!(emitted.children.len(), 2);
    assert_eq!(emitted.children[0].children[0].text.as_deref(), Some("ru"));
    assert_eq!(emitted.children[1].children[0].text.as_deref(), Some("en"));
    for bad in [
        "<pages><lang>ru</lang><lang>en</lang></pages>",
        "<pages><lang>ru</lang></pages><pages><lang>ru</lang></pages>",
        "<pages extra='x'><lang>ru</lang></pages>",
        "<pages><lang>ru<unknown/></lang></pages>",
        "<pages><lang>../ru</lang></pages>",
        "<pages><lang></lang></pages>",
        "<pages>data<lang>ru</lang></pages>",
        "<pages><xml:lang>ru</xml:lang></pages>",
        "<unknown><lang>ru</lang></unknown>",
    ] {
        let xml = format!("<help>{bad}</help>");
        assert!(
            help::decode(&parse(xml.as_bytes()).unwrap().root).is_err(),
            "{bad}"
        );
    }
    assert!(help::emit("", "help", &PropertyValue::Bool(true)).is_err());
    assert!(help::valid_language(&"я".repeat(65)));
}

fn config() -> morph1c_core::ir::Configuration {
    let mut cfg = read_config(
        Format::Designer,
        std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &opts(),
    )
    .unwrap()
    .0;
    cfg.objects
        .iter_mut()
        .find(|o| o.kind.as_str() == "Configuration")
        .unwrap()
        .help = pages();
    let mut command = MetadataObject::new(
        ObjectKind::new("CommonCommand"),
        "LanguageHelp",
        Uuid([43; 16]),
    );
    prop(
        &mut command,
        "group",
        PropertyValue::Enum(Token::new("NavigationPanelOrdinary")),
    );
    command.help = pages();
    command.help_resources.push(HelpResource {
        rel_path: "raw.bin".into(),
        bytes: vec![0, 255, 1],
    });
    cfg.objects.push(command);
    let mut owner = MetadataObject::new(
        ObjectKind::new("DataProcessor"),
        "HelpForms",
        Uuid([44; 16]),
    );
    for (name, ordinary, uuid) in [("Managed", false, 45), ("Ordinary", true, 46)] {
        let mut child = MetadataObject::new(
            ObjectKind::new("DataProcessor.FormRef"),
            name,
            Uuid([uuid; 16]),
        );
        prop(
            &mut child,
            "formType",
            PropertyValue::Enum(Token::new(if ordinary { "Ordinary" } else { "Managed" })),
        );
        prop(
            &mut child,
            "usePurposes",
            PropertyValue::List(vec![PropertyValue::Str("PersonalComputer".into())]),
        );
        owner.children.push(child);
        owner.form_bodies.push(NamedFormBody {
            name: name.into(),
            body: FormBody::new(),
            module: None,
            ordinary_body: ordinary.then(|| b"\xff\xff\xff\x7fcomplete embedded module".to_vec()),
            help: pages(),
            help_resources: vec![],
        });
    }
    cfg.objects.push(owner);
    cfg
}
fn opts() -> ConvertOptions {
    ConvertOptions::default().with_target_version(FormatVersion::new(2, 21))
}
#[test]
fn owned_pages_define_current_markers_order_and_edits_without_provenance() {
    let mut cfg = config();
    for format in [Format::Edt, Format::Designer, Format::Edt] {
        let dir = tempfile::tempdir().unwrap();
        write_config(format, &cfg, dir.path()).unwrap();
        let returned = read_config(format, dir.path(), &opts()).unwrap().0;
        for kind in ["Configuration", "CommonCommand"] {
            let obj = returned
                .objects
                .iter()
                .find(|o| o.kind.as_str() == kind)
                .unwrap();
            assert_eq!(obj.help, pages());
            let id = spec_for(kind)
                .unwrap()
                .fields()
                .iter()
                .find(|f| f.name == "help")
                .unwrap()
                .id;
            assert_eq!(
                obj.properties.iter().find(|(f, _)| *f == id).unwrap().1,
                ids()
            );
        }
        let owner = returned
            .objects
            .iter()
            .find(|o| o.name == "HelpForms")
            .unwrap();
        assert_eq!(owner.form_bodies.len(), 2);
        for body in &owner.form_bodies {
            assert_eq!(body.help, pages());
        }
        cfg = returned;
    }
    let command = cfg
        .objects
        .iter_mut()
        .find(|o| o.kind.as_str() == "CommonCommand")
        .unwrap();
    command.help.swap(0, 1);
    command.help[0].lang = "de".into();
    let dir = tempfile::tempdir().unwrap();
    write_config(Format::Edt, &cfg, dir.path()).unwrap();
    let returned = read_config(Format::Edt, dir.path(), &opts()).unwrap().0;
    let command = returned
        .objects
        .iter()
        .find(|o| o.kind.as_str() == "CommonCommand")
        .unwrap();
    assert_eq!(
        command
            .help
            .iter()
            .map(|p| p.lang.as_str())
            .collect::<Vec<_>>(),
        ["de", "ru"]
    );
    assert_eq!(command.help_resources[0].bytes, [0, 255, 1]);
    assert!(
        !dir.path()
            .join("CommonCommands/LanguageHelp/Help/en.html")
            .exists()
    );
}

#[test]
fn missing_extra_or_duplicate_language_declarations_cannot_drop_owned_pages() {
    for replacement in [
        "<help><pages><lang>ru</lang></pages></help>",
        "<help><pages><lang>ru</lang></pages><pages><lang>de</lang></pages></help>",
        "<help><pages><lang>ru</lang></pages><pages><lang>ru</lang></pages></help>",
    ] {
        let dir = tempfile::tempdir().unwrap();
        write_config(Format::Edt, &config(), dir.path()).unwrap();
        let path = dir
            .path()
            .join("CommonCommands/LanguageHelp/LanguageHelp.mdo");
        let xml = std::fs::read_to_string(&path).unwrap();
        let a = xml.find("<help>").unwrap();
        let z = xml[a..].find("</help>").unwrap() + a + "</help>".len();
        std::fs::write(path, format!("{}{replacement}{}", &xml[..a], &xml[z..])).unwrap();
        assert!(read_config(Format::Edt, dir.path(), &opts()).is_err());
    }
}

#[test]
fn safe_unicode_and_escaped_language_ids_are_not_arbitrarily_truncated() {
    let mut cfg = config();
    let name = format!("{}&a", "я".repeat(65));
    for obj in &mut cfg.objects {
        if !obj.help.is_empty() {
            obj.help[0].lang = name.clone();
        }
        for body in &mut obj.form_bodies {
            if !body.help.is_empty() {
                body.help[0].lang = name.clone();
            }
        }
    }
    for format in [Format::Designer, Format::Edt] {
        let dir = tempfile::tempdir().unwrap();
        write_config(format, &cfg, dir.path()).unwrap();
        cfg = read_config(format, dir.path(), &opts()).unwrap().0;
        assert_eq!(
            cfg.objects
                .iter()
                .find(|o| o.kind.as_str() == "CommonCommand")
                .unwrap()
                .help[0]
                .lang,
            name
        );
    }
}

#[test]
fn public_multilingual_conversion_retains_exact_native_source_and_current_pages() {
    use ibcmd_edt::{
        ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
    };
    let native = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &config(), native.path()).unwrap();
    let path = native.path().join("Configuration.xml");
    let xml = std::fs::read_to_string(&path).unwrap().replace("</Language>", "</Language>\r\n\t\t\t<CommonCommand>LanguageHelp</CommonCommand>\r\n\t\t\t<DataProcessor>HelpForms</DataProcessor>");
    std::fs::write(path, xml).unwrap();
    let source = read_xml_source(native.path(), ReaderLimits::default()).unwrap();
    let options = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    let edt = xml_to_edt(&source, &options).unwrap();
    let project = Project::from_tree(edt.tree.clone()).unwrap();
    let returned = edt_to_xml(&project, &options).unwrap();
    assert_eq!(returned.tree, source);
    let stripped = ibcmd_xml::source_tree::SourceTree::new(
        edt.tree
            .entries()
            .iter()
            .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap();
    let independent = edt_to_xml(&Project::from_tree(stripped).unwrap(), &options).unwrap();
    let pages = independent
        .tree
        .entries()
        .iter()
        .filter(|e| e.path().as_str().contains("LanguageHelp/Ext/Help/"))
        .count();
    assert_eq!(pages, 3); // two HTML pages and the full raw resource
    assert!(independent.tree.entries().iter().any(|e| {
        e.path().as_str() == "CommonCommands/LanguageHelp/Ext/Help.xml"
            && std::str::from_utf8(e.bytes())
                .unwrap()
                .contains("<Page>ru</Page>\r\n\t<Page>en</Page>")
    }));

    use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
    use sha2::{Digest, Sha256};
    let descriptor = "src/CommonCommands/LanguageHelp/LanguageHelp.mdo";
    let old_page = "src/CommonCommands/LanguageHelp/Help/en.html";
    let new_page = "src/CommonCommands/LanguageHelp/Help/de.html";
    let mut changed = Vec::new();
    for entry in edt.tree.entries() {
        if entry.path().as_str() == descriptor {
            let text = std::str::from_utf8(entry.bytes()).unwrap();
            assert!(text.contains("<lang>en</lang>"));
            changed.push(
                SourceEntry::from_bytes(
                    SourcePath::new(descriptor).unwrap(),
                    text.replace("<lang>en</lang>", "<lang>de</lang>")
                        .into_bytes(),
                )
                .unwrap(),
            );
        } else if entry.path().as_str() == old_page {
            changed.push(entry.with_path(SourcePath::new(new_page).unwrap()).unwrap());
        } else {
            changed.push(entry.clone());
        }
    }
    let manifest_path = ".ibcmd-provenance/manifest.json";
    let manifest = changed
        .iter()
        .position(|e| e.path().as_str() == manifest_path)
        .unwrap();
    let mut json: serde_json::Value = serde_json::from_slice(changed[manifest].bytes()).unwrap();
    json["generated"] = serde_json::Value::Object(
        changed
            .iter()
            .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .map(|e| {
                (
                    e.path().as_str().to_string(),
                    serde_json::Value::String(format!("{:x}", Sha256::digest(e.bytes()))),
                )
            })
            .collect(),
    );
    changed[manifest] = SourceEntry::from_bytes(
        SourcePath::new(manifest_path).unwrap(),
        serde_json::to_vec(&json).unwrap(),
    )
    .unwrap();
    let forged = SourceTree::new(changed.clone()).unwrap();
    assert!(edt_to_xml(&Project::from_tree(forged).unwrap(), &options).is_err());
    let current = SourceTree::new(
        changed
            .into_iter()
            .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .collect(),
    )
    .unwrap();
    let current = edt_to_xml(&Project::from_tree(current).unwrap(), &options).unwrap();
    assert!(
        current
            .tree
            .entries()
            .iter()
            .any(|e| e.path().as_str() == "CommonCommands/LanguageHelp/Ext/Help/de.html")
    );
    assert!(
        !current
            .tree
            .entries()
            .iter()
            .any(|e| e.path().as_str() == "CommonCommands/LanguageHelp/Ext/Help/en.html")
    );
}
