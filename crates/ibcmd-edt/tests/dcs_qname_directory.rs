use ibcmd_edt::{ConversionOptions, read_directory_project, read_directory_source};
use morph1c_core::ir::semantic_view::ConfigurationSemanticView;
use morph1c_core::ir::{MetadataObject, ObjectKind, PropertyValue, Template, Token, Uuid};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

const URI: &str = "http://v8.1c.ru/8.1/data/enterprise/current-config";
fn source(inner: &str) -> Vec<u8> {
    format!("<DataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\"><query><![CDATA[x < y && z]]></query><v8:TypeSet xmlns:p=\"{URI}\">{inner}</v8:TypeSet></DataCompositionSchema>").into_bytes()
}
fn configuration(body: Vec<u8>) -> morph1c_core::ir::Configuration {
    let mut cfg = read_config(
        Format::Designer,
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &ConvertOptions::default(),
    )
    .unwrap()
    .0;
    let mut owner =
        MetadataObject::new(ObjectKind::new("CommonTemplate"), "Witness", Uuid([9; 16]));
    owner.properties.push((
        morph1c_core::spec::metadata::report_template_ref::F_TEMPLATE_TYPE,
        PropertyValue::Enum(Token::new("DataCompositionSchema")),
    ));
    owner.templates.push(Template {
        name: "Witness".into(),
        properties: vec![],
        body: Some(body),
        pages: vec![("ru".into(), b"page".to_vec())],
        resources: vec![("asset".into(), b"bytes".to_vec())],
    });
    cfg.objects.push(owner);
    cfg
}
fn options() -> ConversionOptions {
    ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    }
}
fn semantic(cfg: &morph1c_core::ir::Configuration) -> Vec<u8> {
    serde_json::to_vec(&ConfigurationSemanticView {
        configuration: cfg,
        template_body: morph1c_pipeline::dcs_template_semantic_body,
    })
    .unwrap()
}
fn files(root: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, dir: &Path, result: &mut std::collections::BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                visit(root, &p, result)
            } else {
                result.insert(
                    p.strip_prefix(root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .replace('\\', "/"),
                    fs::read(p).unwrap(),
                );
            }
        }
    }
    let mut result = std::collections::BTreeMap::new();
    visit(root, root, &mut result);
    result
}
#[test]
fn explicit_noop_view_keeps_every_wire_field_and_wrong_type_body_exact() {
    let mut cfg = configuration(source("p:AnyIBRef"));
    {
        use morph1c_core::ir::*;
        let obj = cfg.objects.last_mut().unwrap();
        obj.internal_info = Some(InternalInfo {
            generated_types: vec![],
        });
        obj.this_node = Some(Uuid([3; 16]));
        obj.rights = Some(morph1c_core::spec::metadata::role::RightsTable {
            set_for_new_objects: true,
            set_for_attributes_by_default: false,
            independent_rights_of_child_objects: true,
            objects: vec![],
            restriction_templates: vec![],
            source_layout: None,
        });
        obj.xdto_schema = Some(b"xdto".to_vec());
        obj.ws_definition = Some(WsDefinition {
            wsdl: b"wsdl".to_vec(),
            xsds: vec![("1.xsd".into(), b"xsd".to_vec())],
        });
        obj.flowchart = Some(b"flow".to_vec());
        let ci = CommandInterface {
            native_groups_order: None,
            subsystems_visibility: Vec::new(),
            commands: vec![],
            placement: vec![],
            order: vec![],
            subsystems_order: vec!["Subsystem.S".into()],
        };
        obj.command_interface = Some(ci.clone());
        obj.picture = Some(PictureBody {
            file_name: "picture.png".into(),
            bytes: b"picture".to_vec(),
        });
        obj.config_pictures.push(ConfigPicture {
            native_sentinel_explicit: false,
            slot: "Splash".into(),
            ext: "png".into(),
            bytes: b"splash".to_vec(),
        });
        obj.config_blobs.push(ConfigBlob {
            slot: "ParentConfigurations".into(),
            bytes: b"parent".to_vec(),
            mobile_signature_lexical: None,
        });
        obj.help.push(HelpPage {
            lang: "ru".into(),
            body: "help".into(),
        });
        obj.help_resources.push(HelpResource {
            rel_path: "image.png".into(),
            bytes: b"image".to_vec(),
        });
        obj.style_records.push(StyleRecord {
            name: "ControlBorder".into(),
            value: StyleRecordValue::BorderRef("Style.Border".into()),
        });
        obj.forms
            .push(FormItem::new(FormControlKind::new("Button"), "Button", 1));
        obj.form_bodies.push(NamedFormBody {
            ordinary_body: None,
            name: "Form".into(),
            body: FormBody::new(),
            module: Some("Procedure Test() EndProcedure".into()),
            help: vec![],
            help_resources: vec![],
        });
        obj.source_extensions
            .recalculation_refs
            .push("Recalculation.R".into());
        obj.standalone_content = Some(StandaloneContent {
            used: vec!["Catalog.C".into()],
            unused: vec![],
            priority: vec![],
            exchange_on_change_data: true,
            exchange_period: 1,
            transaction_count: 2,
            inactive_nodes_cleanup_timeout: 0,
        });
        obj.root_command_interface = Some(ci.clone());
        obj.main_section_command_interface = Some(ci);
        obj.home_page_work_area = Some(HomePageWorkArea {
            template: "TwoColumnsVariableWidth".into(),
            left: vec![],
            right: vec![],
        });
        obj.client_application_interface = Some(ClientApplicationInterface {
            top: Some(CaiGroup {
                id: "group".into(),
                panels: vec![],
            }),
            left: None,
        });
        obj.schedule = Some(Schedule::platform_default());
        obj.modules.push(Module::text(
            "ObjectModule",
            "Procedure Test() EndProcedure",
        ));
        let mut child =
            MetadataObject::new(ObjectKind::new("Catalog.Attribute"), "Child", Uuid([4; 16]));
        child.templates.push(obj.templates[0].clone());
        obj.children.push(child);
    }
    let noop = ConfigurationSemanticView {
        configuration: &cfg,
        template_body: |_, _| Ok(None),
    };
    assert_eq!(
        serde_json::to_vec(&cfg).unwrap(),
        serde_json::to_vec(&noop).unwrap()
    );
    let mut wrong = cfg.clone();
    wrong.objects.last_mut().unwrap().properties[0].1 =
        PropertyValue::Enum(Token::new("TextDocument"));
    assert_eq!(serde_json::to_vec(&wrong).unwrap(), semantic(&wrong));
    let mut ns = cfg.clone();
    ns.objects.last_mut().unwrap().templates[0].body = Some(
        String::from_utf8(source("p:AnyIBRef"))
            .unwrap()
            .replace(URI, "urn:other")
            .into_bytes(),
    );
    let mut changed = ns.clone();
    changed.objects.last_mut().unwrap().templates[0].body = Some(
        String::from_utf8(
            changed.objects.last().unwrap().templates[0]
                .body
                .clone()
                .unwrap(),
        )
        .unwrap()
        .replace("AnyIBRef", "AnyRef")
        .into_bytes(),
    );
    assert_ne!(semantic(&ns), semantic(&changed));
}
#[test]
fn current_selected_qname_only_preserves_ordered_other_content() {
    let plain = configuration(source("p:AnyIBRef<!--keep--><?audit one?>"));
    let split = configuration(source(
        "<![CDATA[p:AnyI]]><!--keep--><?audit one?>B&#x52;ef",
    ));
    assert_eq!(semantic(&plain), semantic(&split));
    for body in [
        source("<![CDATA[p:AnyI]]><?audit one?><!--keep-->B&#x52;ef"),
        source("p:Other<!--keep--><?audit one?>"),
        source("p:AnyIBRef<!--changed--><?audit one?>"),
    ] {
        assert_ne!(semantic(&plain), semantic(&configuration(body)));
    }
    let mut query = configuration(source("p:AnyIBRef<!--keep--><?audit one?>"));
    query.objects.last_mut().unwrap().templates[0].body = Some(
        String::from_utf8(
            query.objects.last().unwrap().templates[0]
                .body
                .clone()
                .unwrap(),
        )
        .unwrap()
        .replace("x < y", "x <= y")
        .into_bytes(),
    );
    assert_ne!(semantic(&plain), semantic(&query));
}
#[test]
fn unprefixed_qname_uses_actual_inline_default_binding() {
    let body = String::from_utf8(source("AnyIBRef"))
        .unwrap()
        .replace("xmlns:p=", "xmlns=");
    let expected = body.replace("AnyIBRef", "AnyRef");
    let temp = tempfile::tempdir().unwrap();
    let mut cfg = configuration(body.as_bytes().to_vec());
    cfg.objects.last_mut().unwrap().templates[0].pages.clear();
    cfg.objects.last_mut().unwrap().templates[0]
        .resources
        .clear();
    write_config(Format::Edt, &cfg, temp.path()).unwrap();
    assert_eq!(
        fs::read(temp.path().join("CommonTemplates/Witness/Template.dcs")).unwrap(),
        expected.as_bytes()
    );
    let returned = read_config(Format::Edt, temp.path(), &ConvertOptions::default())
        .unwrap()
        .0;
    let original = cfg.objects.last().unwrap();
    let restored = returned
        .objects
        .iter()
        .find(|o| o.name == "Witness")
        .unwrap();
    assert_eq!(
        morph1c_pipeline::dcs_template_semantic_body(original, &original.templates[0]).unwrap(),
        morph1c_pipeline::dcs_template_semantic_body(restored, &restored.templates[0]).unwrap()
    );
    assert!(
        String::from_utf8(
            morph1c_pipeline::dcs_template_semantic_body(original, &original.templates[0])
                .unwrap()
                .unwrap()
        )
        .unwrap()
        .contains(">AnyIBRef</v8:TypeSet>")
    );
}
#[test]
fn both_directory_directions_split_entity_cdata_stripped_and_exact_provenance() {
    for inner in [
        "p:AnyIBRef",
        "<![CDATA[p:AnyIBRef]]>",
        "<![CDATA[p:AnyI]]><!--keep-->B&#x52;ef",
        "p:Any&#73;B&#x52;ef",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let xml = temp.path().join("xml");
        let mut cfg = configuration(source(inner));
        cfg.objects.last_mut().unwrap().templates[0].pages.clear();
        cfg.objects.last_mut().unwrap().templates[0]
            .resources
            .clear();
        write_config(Format::Designer, &cfg, &xml).unwrap();
        let native = files(&xml);
        let edt = temp.path().join("edt");
        read_directory_source(&xml)
            .unwrap()
            .xml_to_edt(&options())
            .unwrap()
            .publish_new(&edt)
            .unwrap();
        let exact = temp.path().join("exact");
        read_directory_project(&edt)
            .unwrap()
            .edt_to_xml(&options())
            .unwrap()
            .publish_new(&exact)
            .unwrap();
        assert_eq!(native, files(&exact));
        fs::remove_dir_all(edt.join(".ibcmd-provenance")).unwrap();
        let returned = temp.path().join("returned");
        read_directory_project(&edt)
            .unwrap()
            .edt_to_xml(&options())
            .unwrap()
            .publish_new(&returned)
            .unwrap();
        let a = read_config(Format::Designer, &xml, &ConvertOptions::default())
            .unwrap()
            .0;
        let b = read_config(Format::Designer, &returned, &ConvertOptions::default())
            .unwrap()
            .0;
        assert_eq!(semantic(&a), semantic(&b));
        let second = temp.path().join("edt-again");
        read_directory_source(&returned)
            .unwrap()
            .xml_to_edt(&options())
            .unwrap()
            .publish_new(&second)
            .unwrap();
        fs::remove_dir_all(second.join(".ibcmd-provenance")).unwrap();
        let last = temp.path().join("last");
        read_directory_project(&second)
            .unwrap()
            .edt_to_xml(&options())
            .unwrap()
            .publish_new(&last)
            .unwrap();
        assert_eq!(files(&returned), files(&last));
    }
}
#[test]
fn edited_type_with_forged_generated_hash_never_restores_stale_source() {
    let temp = tempfile::tempdir().unwrap();
    let xml = temp.path().join("xml");
    let edt = temp.path().join("edt");
    let mut cfg = configuration(source("<![CDATA[p:AnyI]]><!--keep-->B&#x52;ef"));
    cfg.objects.last_mut().unwrap().templates[0].pages.clear();
    cfg.objects.last_mut().unwrap().templates[0]
        .resources
        .clear();
    write_config(Format::Designer, &cfg, &xml).unwrap();
    read_directory_source(&xml)
        .unwrap()
        .xml_to_edt(&options())
        .unwrap()
        .publish_new(&edt)
        .unwrap();
    let path = "src/CommonTemplates/Witness/Template.dcs";
    let edited = String::from_utf8(fs::read(edt.join(path)).unwrap())
        .unwrap()
        .replace("Any]]>", "Other]]>");
    fs::write(edt.join(path), &edited).unwrap();
    let manifest = edt.join(".ibcmd-provenance/manifest.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    value["generated"][path] = format!("{:x}", Sha256::digest(edited.as_bytes())).into();
    fs::write(manifest, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(
        read_directory_project(&edt)
            .unwrap()
            .edt_to_xml(&options())
            .is_err()
    );
    fs::remove_dir_all(edt.join(".ibcmd-provenance")).unwrap();
    let output = temp.path().join("edited");
    read_directory_project(&edt)
        .unwrap()
        .edt_to_xml(&options())
        .unwrap()
        .publish_new(&output)
        .unwrap();
    assert!(
        String::from_utf8(
            fs::read(output.join("CommonTemplates/Witness/Ext/Template.xml")).unwrap()
        )
        .unwrap()
        .contains("Other")
    );
}
