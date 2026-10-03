use morph1c_core::ir::{Configuration, MetadataObject, Module, ModuleBody, ObjectKind, Uuid};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
use sha2::{Digest, Sha256};
use std::path::Path;

fn fixture() -> Configuration {
    read_config(
        Format::Designer,
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &ConvertOptions::default(),
    )
    .unwrap()
    .0
}
fn module_shape(c: &Configuration) -> Vec<(String, String, ModuleBody)> {
    fn collect(o: &MetadataObject, path: &str, rows: &mut Vec<(String, String, ModuleBody)>) {
        for m in &o.modules {
            rows.push((path.into(), m.slot.clone(), m.body.clone()));
        }
        for c in &o.children {
            collect(c, &format!("{path}/{}", c.name), rows);
        }
        for f in &o.form_bodies {
            if let Some(m) = &f.module {
                rows.push((
                    format!("{path}/{}", f.name),
                    "FormModule".into(),
                    ModuleBody::Text(m.clone()),
                ));
            }
        }
    }
    let mut rows = vec![];
    for o in &c.objects {
        collect(o, &o.name, &mut rows);
    }
    rows
}
fn configuration_with_modules(text: &str) -> Configuration {
    let mut cfg = fixture();
    cfg.objects
        .iter_mut()
        .find(|o| o.kind.as_str() == "Configuration")
        .unwrap()
        .modules
        .push(Module::text("ManagedApplicationModule", text));
    let catalog = cfg
        .objects
        .iter_mut()
        .find(|o| o.kind.as_str() == "Catalog")
        .unwrap();
    catalog.modules.push(Module::text("ObjectModule", text));
    let mut cmd = MetadataObject::new(
        ObjectKind::new("Catalog.Command"),
        "Encoding",
        Uuid([97; 16]),
    );
    cmd.properties.push((
        morph1c_core::ir::FieldId(3),
        morph1c_core::ir::PropertyValue::Enum(morph1c_core::ir::Token::new("CommandBarImportant")),
    ));
    cmd.modules.push(Module::text("CommandModule", text));
    catalog.children.push(cmd);
    cfg
}
#[test]
fn root_object_and_command_preserve_canonical_prefix_in_both_directions() {
    for text in [
        "",
        " \t// plain\r\n",
        "\u{feff}// body prefix\r\n\u{feff}interior\u{feff}",
        "\u{feff}\u{feff}two body characters",
    ] {
        let source = configuration_with_modules(text);
        let expected = module_shape(&source);
        let dir = tempfile::tempdir().unwrap();
        for (i, format) in [Format::Designer, Format::Edt].into_iter().enumerate() {
            let path = dir.path().join(i.to_string());
            write_config(format, &source, &path).unwrap();
            let returned = read_config(format, &path, &ConvertOptions::default())
                .unwrap()
                .0;
            assert_eq!(module_shape(&returned), expected);
            let opposite = if format == Format::Designer {
                Format::Edt
            } else {
                Format::Designer
            };
            let target = dir.path().join(format!("opposite{i}"));
            write_config(opposite, &returned, &target).unwrap();
            let next = read_config(opposite, &target, &ConvertOptions::default())
                .unwrap()
                .0;
            assert_eq!(module_shape(&next), expected);
        }
    }
}
#[test]
fn protected_object_image_is_verbatim_and_invalid_command_text_rejects() {
    let mut cfg = configuration_with_modules("text");
    let image = vec![0xff, 0xff, 0xff, 0x7f, 0, 2, 0, 0, 0xef, 0xbb, 0xbf, 0xff];
    cfg.objects
        .iter_mut()
        .find(|o| o.kind.as_str() == "Catalog")
        .unwrap()
        .modules[0]
        .body = ModuleBody::Binary(image.clone());
    let dir = tempfile::tempdir().unwrap();
    for (i, format) in [Format::Designer, Format::Edt].into_iter().enumerate() {
        let path = dir.path().join(i.to_string());
        write_config(format, &cfg, &path).unwrap();
        let parsed = read_config(format, &path, &ConvertOptions::default())
            .unwrap()
            .0;
        assert_eq!(module_shape(&parsed), module_shape(&cfg));
        let catalog = cfg
            .objects
            .iter()
            .find(|o| o.kind.as_str() == "Catalog")
            .unwrap();
        let command_dir = path
            .join("Catalogs")
            .join(&catalog.name)
            .join("Commands/Encoding");
        let command = if format == Format::Designer {
            command_dir.join("Ext/CommandModule.bsl")
        } else {
            command_dir.join("CommandModule.bsl")
        };
        std::fs::write(command, [0xff, 0, 0xfe]).unwrap();
        assert!(read_config(format, &path, &ConvertOptions::default()).is_err());
    }
}
#[test]
fn public_unchanged_return_preserves_module_bytes_and_forged_edit_rejects() {
    use ibcmd_edt::{
        ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
    };
    use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
    let dir = tempfile::tempdir().unwrap();
    let cfg = configuration_with_modules("\u{feff} // body character\r\n\u{feff}end");
    write_config(Format::Designer, &cfg, dir.path()).unwrap();
    let options = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    let source = read_xml_source(dir.path(), ReaderLimits::default()).unwrap();
    let generated = xml_to_edt(&source, &options).unwrap().tree;
    assert_eq!(
        edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
            .unwrap()
            .tree,
        source
    );
    let entry = generated
        .entries()
        .iter()
        .find(|e| e.path().as_str().ends_with("ManagedApplicationModule.bsl"))
        .unwrap();
    let edited = b" // edited\r\n".to_vec();
    let manifest_path = ".ibcmd-provenance/manifest.json";
    let mut manifest: serde_json::Value = serde_json::from_slice(
        generated
            .entries()
            .iter()
            .find(|e| e.path().as_str() == manifest_path)
            .unwrap()
            .bytes(),
    )
    .unwrap();
    manifest["generated"][entry.path().as_str()] =
        serde_json::json!(format!("{:x}", Sha256::digest(&edited)));
    let mut entries = generated
        .entries()
        .iter()
        .filter(|e| e.path() != entry.path() && e.path().as_str() != manifest_path)
        .cloned()
        .collect::<Vec<_>>();
    entries.push(SourceEntry::from_bytes(entry.path().clone(), edited).unwrap());
    entries.push(
        SourceEntry::from_bytes(
            SourcePath::new(manifest_path).unwrap(),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap(),
    );
    assert!(
        edt_to_xml(
            &Project::from_tree(SourceTree::new(entries).unwrap()).unwrap(),
            &options
        )
        .is_err()
    );
}

#[test]
fn form_module_preserves_double_prefix_and_all_body_whitespace() {
    use morph1c_core::ir::{FormBody, NamedFormBody};
    use morph1c_pipeline::{attach_form_body, write_form_bodies};
    let dir = tempfile::tempdir().unwrap();
    for (i, format) in [Format::Designer, Format::Edt].into_iter().enumerate() {
        for (j, text) in [
            "",
            "\u{feff} \t// body\r\n\u{feff}trailing",
            "\r\n \t\u{feff}interior\r\n",
        ]
        .into_iter()
        .enumerate()
        {
            let parent = dir.path().join(format!("{i}-{j}"));
            std::fs::create_dir_all(&parent).unwrap();
            let anchor = parent.join(if format == Format::Designer {
                "Witness.xml"
            } else {
                "Witness.mdo"
            });
            let mut obj =
                MetadataObject::new(ObjectKind::new("CommonForm"), "Witness", Uuid([98; 16]));
            obj.form_bodies.push(NamedFormBody {
                name: "Witness".into(),
                body: FormBody::new(),
                ordinary_body: None,
                module: Some(text.into()),
                help: vec![],
                help_resources: vec![],
            });
            write_form_bodies(format, &anchor, &obj).unwrap();
            let mut parsed =
                MetadataObject::new(ObjectKind::new("CommonForm"), "Witness", Uuid([98; 16]));
            attach_form_body(format, "CommonForm", &anchor, &mut parsed).unwrap();
            assert_eq!(parsed.form_bodies[0].module.as_deref(), Some(text));
        }
    }
}
