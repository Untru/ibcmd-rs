use ibcmd_edt::{
    ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
};
use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
use morph1c_core::{
    ir::ConfigPicture, spec::metadata::configuration as spec, version::FormatVersion,
};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
use sha2::{Digest, Sha256};

fn options(dialect: &str) -> ConversionOptions {
    ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: dialect.into(),
        runtime_version: Some(if dialect == "2.20" { "8.3.27" } else { "8.5.1" }.into()),
    }
}
fn model(
    dialect: &str,
    point: Option<(i64, i64)>,
    explicit: bool,
) -> morph1c_core::ir::Configuration {
    let version = FormatVersion::new(2, if dialect == "2.20" { 20 } else { 21 });
    let mut config = read_config(
        Format::Designer,
        std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &ConvertOptions::default().with_target_version(version),
    )
    .unwrap()
    .0;
    config.source_version = Some(version);
    let root = config
        .objects
        .iter_mut()
        .find(|o| o.kind.as_str() == "Configuration")
        .unwrap();
    for (slot, id) in [
        ("Logo", spec::F_LOGO),
        ("Splash", spec::F_SPLASH),
        ("MainSectionPicture", spec::F_MAIN_SECTION_PICTURE),
    ] {
        root.properties
            .push((id, formats_xml::md_picture::present(point)));
        root.config_pictures.push(ConfigPicture {
            slot: slot.into(),
            ext: "png".into(),
            bytes: b"immutable-image-bytes".to_vec(),
            native_sentinel_explicit: explicit,
        });
    }
    config
}
fn source(dialect: &str, point: Option<(i64, i64)>, explicit: bool) -> SourceTree {
    let t = tempfile::tempdir().unwrap();
    morph1c_core::version::with_roundtrip_target(
        FormatVersion::new(2, if dialect == "2.20" { 20 } else { 21 }),
        || write_config(Format::Designer, &model(dialect, point, explicit), t.path()),
    )
    .unwrap();
    read_xml_source(t.path(), ReaderLimits::default()).unwrap()
}
fn bytes<'a>(tree: &'a SourceTree, path: &str) -> &'a [u8] {
    tree.entries()
        .iter()
        .find(|e| e.path().as_str() == path)
        .unwrap()
        .bytes()
}
fn mutate(tree: &SourceTree, path: &str, data: Vec<u8>) -> SourceTree {
    let mut entries = tree
        .entries()
        .iter()
        .filter(|e| e.path().as_str() != path)
        .cloned()
        .collect::<Vec<_>>();
    entries.push(SourceEntry::from_bytes(SourcePath::new(path).unwrap(), data).unwrap());
    SourceTree::new(entries).unwrap()
}
fn strip(tree: &SourceTree) -> SourceTree {
    SourceTree::new(
        tree.entries()
            .iter()
            .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap()
}
fn delete(tree: &SourceTree, path: &str) -> SourceTree {
    SourceTree::new(
        tree.entries()
            .iter()
            .filter(|e| e.path().as_str() != path)
            .cloned()
            .collect(),
    )
    .unwrap()
}
fn forge(tree: &SourceTree, path: &str) -> SourceTree {
    let mut m: serde_json::Value =
        serde_json::from_slice(bytes(tree, ".ibcmd-provenance/manifest.json")).unwrap();
    m["generated"][path] = serde_json::json!(format!("{:x}", Sha256::digest(bytes(tree, path))));
    mutate(
        tree,
        ".ibcmd-provenance/manifest.json",
        serde_json::to_vec(&m).unwrap(),
    )
}
fn forge_all_current_semantics(current: &SourceTree) -> SourceTree {
    let temp = tempfile::tempdir().unwrap();
    let disk = temp.path().join("project");
    ibcmd_xml::source_tree::publish_new(&strip(current), &disk).unwrap();
    let model = read_config(
        Format::Edt,
        &disk.join("src"),
        &ConvertOptions::default().with_target_version(FormatVersion::new(2, 21)),
    )
    .unwrap()
    .0;
    let view = morph1c_core::ir::semantic_view::ConfigurationSemanticView {
        configuration: &model,
        template_body: morph1c_pipeline::dcs_template_semantic_body,
    };
    let mut manifest: serde_json::Value =
        serde_json::from_slice(bytes(current, ".ibcmd-provenance/manifest.json")).unwrap();
    for e in current
        .entries()
        .iter()
        .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
    {
        manifest["generated"][e.path().as_str()] =
            serde_json::json!(format!("{:x}", Sha256::digest(e.bytes())));
    }
    manifest["semantics"] = serde_json::json!(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&view).unwrap())
    ));
    mutate(
        current,
        ".ibcmd-provenance/manifest.json",
        serde_json::to_vec(&manifest).unwrap(),
    )
}

#[test]
fn all_root_slots_both_profiles_have_current_point_semantics_and_complete_inventory() {
    for dialect in ["2.20", "2.21"] {
        for point in [
            None,
            Some((13, 3)),
            Some((0, 0)),
            Some((-1, -1)),
            Some((-1, 3)),
        ] {
            let src = source(dialect, point, false);
            let o = options(dialect);
            let generated = xml_to_edt(&src, &o).unwrap();
            for slot in ["Logo", "Splash", "MainSectionPicture"] {
                assert!(
                    generated
                        .accounting
                        .iter()
                        .any(|e| e.path.as_str() == format!("Ext/{slot}.xml"))
                );
                assert_eq!(
                    bytes(&generated.tree, &format!("src/Configuration/{slot}.png")),
                    b"immutable-image-bytes"
                );
            }
            assert_eq!(
                edt_to_xml(&Project::from_tree(generated.tree.clone()).unwrap(), &o)
                    .unwrap()
                    .tree,
                src
            );
            let returned =
                edt_to_xml(&Project::from_tree(strip(&generated.tree)).unwrap(), &o).unwrap();
            assert_eq!(returned.tree, src);
            assert!(returned.extensions.is_empty());
            let second = xml_to_edt(&returned.tree, &o).unwrap();
            assert_eq!(
                edt_to_xml(&Project::from_tree(strip(&second.tree)).unwrap(), &o)
                    .unwrap()
                    .tree,
                src
            );
        }
    }
}
#[test]
fn native_explicit_sentinel_keeps_source_spelling_and_current_point_edit_wins() {
    let src = source("2.21", Some((-1, -1)), true);
    let opts = options("2.21");
    assert!(
        std::str::from_utf8(bytes(&src, "Ext/Logo.xml"))
            .unwrap()
            .contains("<xr:TransparentPixel x=\"-1\" y=\"-1\"/>")
    );
    let generated = xml_to_edt(&src, &opts).unwrap().tree;
    assert_eq!(
        edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &opts)
            .unwrap()
            .tree,
        src
    );
    let path = "src/Configuration/Configuration.mdo";
    let changed = mutate(
        &generated,
        path,
        String::from_utf8(bytes(&generated, path).to_vec())
            .unwrap()
            .replace("<x>-1</x>", "<x>13</x>")
            .replace("<y>-1</y>", "<y>3</y>")
            .into_bytes(),
    );
    assert!(edt_to_xml(&Project::from_tree(forge(&changed, path)).unwrap(), &opts).is_err());
    let out = edt_to_xml(&Project::from_tree(strip(&changed)).unwrap(), &opts)
        .unwrap()
        .tree;
    assert!(
        std::str::from_utf8(bytes(&out, "Ext/Logo.xml"))
            .unwrap()
            .contains("<xr:TransparentPixel x=\"13\" y=\"3\"/>")
    );
}
#[test]
fn point_delete_image_edit_and_abs_reference_tamper_do_not_restore_original() {
    for dialect in ["2.20", "2.21"] {
        let opts = options(dialect);
        let src = source(dialect, Some((13, 3)), false);
        let generated = xml_to_edt(&src, &opts).unwrap().tree;
        let path = "src/Configuration/Configuration.mdo";
        let text = String::from_utf8(bytes(&generated, path).to_vec()).unwrap();
        // Use exact parsed block boundaries rather than assuming indentation.
        let mut text = text;
        while let Some(a) = text.find("<transparentPixel>") {
            let z =
                a + text[a..].find("</transparentPixel>").unwrap() + "</transparentPixel>".len();
            text.replace_range(a..z, "");
        }
        let changed = mutate(&generated, path, text.into_bytes());
        assert!(edt_to_xml(&Project::from_tree(forge(&changed, path)).unwrap(), &opts).is_err());
        let out = edt_to_xml(&Project::from_tree(strip(&changed)).unwrap(), &opts)
            .unwrap()
            .tree;
        assert!(
            std::str::from_utf8(bytes(&out, "Ext/Logo.xml"))
                .unwrap()
                .contains("<xr:LoadTransparent>false</xr:LoadTransparent>")
        );
        let image = "src/Configuration/Logo.png";
        let changed = mutate(&generated, image, b"new-image-current".to_vec());
        assert!(edt_to_xml(&Project::from_tree(forge(&changed, image)).unwrap(), &opts).is_err());
        assert_eq!(
            bytes(
                &edt_to_xml(&Project::from_tree(strip(&changed)).unwrap(), &opts)
                    .unwrap()
                    .tree,
                "Ext/Logo/Picture.png"
            ),
            b"new-image-current"
        );
        let missing = delete(&generated, image);
        assert!(edt_to_xml(&Project::from_tree(missing).unwrap(), &opts).is_err());
        let reference = mutate(
            &src,
            "Ext/Logo.xml",
            String::from_utf8(bytes(&src, "Ext/Logo.xml").to_vec())
                .unwrap()
                .replace("Picture.png", "Absent.png")
                .into_bytes(),
        );
        assert!(xml_to_edt(&reference, &opts).is_err());
    }
}
#[test]
fn closed_md_picture_rejects_unknown_duplicate_and_malformed_point_slots() {
    for xml in [
        "<logo extra=\"x\"/>",
        "<logo><unknown/></logo>",
        "<logo><transparentPixel/><transparentPixel/></logo>",
        "<logo><transparentPixel><x>1</x><x>2</x></transparentPixel></logo>",
        "<logo><transparentPixel><x>broken</x></transparentPixel></logo>",
        "<logo><p:transparentPixel xmlns:p=\"urn:unknown\"/></logo>",
    ] {
        let doc = formats_xml::parse(xml.as_bytes()).unwrap();
        assert!(matches!(
            formats_xml::md_picture::decode(&doc.root),
            morph1c_core::engine::Decoded::Error(_)
        ));
    }
}

#[test]
fn official_sdk_both_point_xml_shapes_claim_exactly_the_same_current_value() {
    for (a, b) in [
        (
            "<logo><transparentPixel x=\"13\" y=\"3\"/></logo>",
            "<logo><transparentPixel><x>13</x><y>3</y></transparentPixel></logo>",
        ),
        (
            "<logo><transparentPixel y=\"3\"/></logo>",
            "<logo><transparentPixel><y>3</y></transparentPixel></logo>",
        ),
    ] {
        let (a, b) = (
            formats_xml::parse(a.as_bytes()).unwrap(),
            formats_xml::parse(b.as_bytes()).unwrap(),
        );
        let (va, vb) = (
            formats_xml::md_picture::decode(&a.root),
            formats_xml::md_picture::decode(&b.root),
        );
        match (va, vb) {
            (
                morph1c_core::engine::Decoded::Present(va),
                morph1c_core::engine::Decoded::Present(vb),
            ) => assert_eq!(va, vb),
            _ => panic!("closed point shape"),
        }
    }
    for bad in [
        "<logo><transparentPixel x=\"13\"><y>3</y></transparentPixel></logo>",
        "<logo><transparentPixel x=\"13\" unknown=\"x\"/></logo>",
        "<logo><transparentPixel x=\"wrong\"/></logo>",
    ] {
        assert!(matches!(
            formats_xml::md_picture::decode(&formats_xml::parse(bad.as_bytes()).unwrap().root),
            morph1c_core::engine::Decoded::Error(_)
        ));
    }
}

#[test]
fn even_rehashed_every_generated_file_and_current_semantics_cannot_replay_stale_root_points() {
    let opts = options("2.21");
    let src = source("2.21", Some((13, 3)), false);
    let generated = xml_to_edt(&src, &opts).unwrap().tree;
    let path = "src/Configuration/Configuration.mdo";
    let current = mutate(
        &generated,
        path,
        String::from_utf8(bytes(&generated, path).to_vec())
            .unwrap()
            .replace("<x>13</x>", "<x>27</x>")
            .into_bytes(),
    );
    let forged = forge_all_current_semantics(&current);
    assert!(edt_to_xml(&Project::from_tree(forged).unwrap(), &opts).is_err());
    let returned = edt_to_xml(&Project::from_tree(strip(&current)).unwrap(), &opts)
        .unwrap()
        .tree;
    assert!(
        std::str::from_utf8(bytes(&returned, "Ext/Logo.xml"))
            .unwrap()
            .contains("x=\"27\"")
    );
    let retained = ".ibcmd-provenance/xml/Ext/Logo.xml";
    let current = mutate(
        &generated,
        retained,
        String::from_utf8(bytes(&generated, retained).to_vec())
            .unwrap()
            .replace("x=\"13\"", "x=\"27\"")
            .into_bytes(),
    );
    let mut manifest: serde_json::Value =
        serde_json::from_slice(bytes(&current, ".ibcmd-provenance/manifest.json")).unwrap();
    manifest["original"]["Ext/Logo.xml"] =
        serde_json::json!(format!("{:x}", Sha256::digest(bytes(&current, retained))));
    let forged = mutate(
        &current,
        ".ibcmd-provenance/manifest.json",
        serde_json::to_vec(&manifest).unwrap(),
    );
    assert!(edt_to_xml(&Project::from_tree(forged).unwrap(), &opts).is_err());
}

fn glyph_source(dialect: &str) -> SourceTree {
    let temp = tempfile::tempdir().unwrap();
    let mut cfg = model(dialect, Some((13, 3)), false);
    let root = cfg
        .objects
        .iter_mut()
        .find(|o| o.kind.as_str() == "Configuration")
        .unwrap();
    for (_, v) in root
        .properties
        .iter_mut()
        .filter(|(id, _)| [spec::F_LOGO, spec::F_SPLASH, spec::F_MAIN_SECTION_PICTURE].contains(id))
    {
        *v = formats_xml::md_picture::present_with_glyph(Some((13, 3)), Some((2, 4)));
    }
    morph1c_core::version::with_roundtrip_target(
        FormatVersion::new(2, if dialect == "2.20" { 20 } else { 21 }),
        || write_config(Format::Designer, &cfg, temp.path()),
    )
    .unwrap();
    read_xml_source(temp.path(), ReaderLimits::default()).unwrap()
}
#[test]
fn persisted_glyph_has_closed_native_carrier_and_both_profiles_return_without_provenance() {
    for dialect in ["2.20", "2.21"] {
        let source = glyph_source(dialect);
        let opts = options(dialect);
        let resource = format!("Ext/{}", formats_xml::md_picture::RESOURCE);
        let converted = xml_to_edt(&source, &opts).unwrap();
        assert_eq!(
            converted
                .extensions
                .iter()
                .find(|e| e.id == "ibcmd-root-picture-semantics/1")
                .map(|e| (e.resources, e.references)),
            Some((1, 3))
        );
        let descriptor = std::str::from_utf8(bytes(
            &converted.tree,
            "src/Configuration/Configuration.mdo",
        ))
        .unwrap();
        assert_eq!(descriptor.matches("<glyph>").count(), 3);
        assert!(!converted.tree.entries().iter().any(|e| e.path().as_str()
            == format!("src/Configuration/{}", formats_xml::md_picture::RESOURCE)));
        let current = strip(&converted.tree);
        let returned = edt_to_xml(&Project::from_tree(current.clone()).unwrap(), &opts).unwrap();
        assert_eq!(returned.tree, source);
        assert_eq!(returned.extensions[0].id, "ibcmd-root-picture-semantics/1");
        assert!(
            returned
                .accounting
                .iter()
                .any(|e| e.path.as_str() == "src/Configuration/Configuration.mdo")
        );
        let second = xml_to_edt(&returned.tree, &opts).unwrap();
        assert_eq!(
            edt_to_xml(&Project::from_tree(strip(&second.tree)).unwrap(), &opts)
                .unwrap()
                .tree,
            source
        );
        assert!(
            bytes(&source, &resource)
                .windows(7)
                .any(|b| b == b"\"glyph\"")
        );
    }
}
#[test]
fn current_glyph_edit_delete_and_full_forgery_cannot_return_old_carrier() {
    let opts = options("2.21");
    let source = glyph_source("2.21");
    let generated = xml_to_edt(&source, &opts).unwrap().tree;
    let path = "src/Configuration/Configuration.mdo";
    let changed = mutate(
        &generated,
        path,
        String::from_utf8(bytes(&generated, path).to_vec())
            .unwrap()
            .replace("<x>2</x>", "<x>7</x>")
            .into_bytes(),
    );
    assert!(edt_to_xml(&Project::from_tree(forge(&changed, path)).unwrap(), &opts).is_err());
    assert!(
        edt_to_xml(
            &Project::from_tree(forge_all_current_semantics(&changed)).unwrap(),
            &opts
        )
        .is_err()
    );
    let returned = edt_to_xml(&Project::from_tree(strip(&changed)).unwrap(), &opts)
        .unwrap()
        .tree;
    let resource = format!("Ext/{}", formats_xml::md_picture::RESOURCE);
    let dto: serde_json::Value = serde_json::from_slice(bytes(&returned, &resource)).unwrap();
    assert_eq!(dto["pictures"][0]["glyph"]["x"], 7);
    let mut text = String::from_utf8(bytes(&generated, path).to_vec()).unwrap();
    while let Some(a) = text.find("<glyph>") {
        let z = a + text[a..].find("</glyph>").unwrap() + "</glyph>".len();
        text.replace_range(a..z, "");
    }
    let deleted = mutate(&generated, path, text.into_bytes());
    assert!(edt_to_xml(&Project::from_tree(forge(&deleted, path)).unwrap(), &opts).is_err());
    let returned = edt_to_xml(&Project::from_tree(strip(&deleted)).unwrap(), &opts).unwrap();
    assert!(returned.extensions.is_empty());
    assert!(
        !returned
            .tree
            .entries()
            .iter()
            .any(|e| e.path().as_str() == resource)
    );
    let resource_path = format!(".ibcmd-provenance/xml/{resource}");
    let mut dto: serde_json::Value =
        serde_json::from_slice(bytes(&generated, &resource_path)).unwrap();
    dto["pictures"][0]["glyph"]["x"] = serde_json::json!(17);
    let changed = mutate(
        &generated,
        &resource_path,
        serde_json::to_vec(&dto).unwrap(),
    );
    let mut manifest: serde_json::Value =
        serde_json::from_slice(bytes(&changed, ".ibcmd-provenance/manifest.json")).unwrap();
    manifest["original"][&resource] = serde_json::json!(format!(
        "{:x}",
        Sha256::digest(bytes(&changed, &resource_path))
    ));
    let forged = mutate(
        &changed,
        ".ibcmd-provenance/manifest.json",
        serde_json::to_vec(&manifest).unwrap(),
    );
    assert!(edt_to_xml(&Project::from_tree(forged).unwrap(), &opts).is_err());
}
#[test]
fn root_glyph_resource_rejects_unknown_duplicate_owner_and_orphan_bindings_atomically() {
    let source = glyph_source("2.21");
    let opts = options("2.21");
    let resource = format!("Ext/{}", formats_xml::md_picture::RESOURCE);
    let original: serde_json::Value = serde_json::from_slice(bytes(&source, &resource)).unwrap();
    for edit in ["unknown", "duplicate", "owner", "slot", "point_name"] {
        let mut dto = original.clone();
        match edit {
            "unknown" => dto["unknown"] = serde_json::json!(true),
            "duplicate" => {
                let row = dto["pictures"][0].clone();
                dto["pictures"].as_array_mut().unwrap().push(row);
            }
            "owner" => dto["owner_uuid"][0] = serde_json::json!(255),
            "slot" => dto["pictures"][0]["slot"] = serde_json::json!("Unknown"),
            "point_name" => {
                dto["pictures"][0]["glyph"]["name"] = serde_json::json!("not-a-Point-field")
            }
            _ => unreachable!(),
        };
        assert!(
            xml_to_edt(
                &mutate(&source, &resource, serde_json::to_vec(&dto).unwrap()),
                &opts
            )
            .is_err(),
            "{edit}"
        );
    }
    assert!(xml_to_edt(&delete(&source, "Ext/Logo/Picture.png"), &opts).is_err());
    // Current binary edits remain independent of glyph semantics.
    let source = mutate(&source, "Ext/Logo/Picture.png", b"CURRENT-image".to_vec());
    let converted = xml_to_edt(&source, &opts).unwrap();
    assert_eq!(
        edt_to_xml(&Project::from_tree(strip(&converted.tree)).unwrap(), &opts)
            .unwrap()
            .tree,
        source
    );
}

#[test]
#[ignore = "explicit lab-only mini capture for independent native SDK compatibility"]
fn capture_declared_native_root_picture_mini() {
    let destination = std::path::PathBuf::from(
        std::env::var("IBCMD_ROOT_PICTURE_CAPTURE").expect("fresh lab capture path"),
    );
    assert!(destination.starts_with(std::path::Path::new("F:/ibcmd/lab/07")));
    let png =
        std::fs::read(std::env::var("IBCMD_ROOT_PICTURE_PNG").expect("valid lab PNG")).unwrap();
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    std::fs::create_dir(&destination).unwrap();
    for dialect in ["2.20", "2.21"] {
        let mut src = glyph_source(dialect);
        for slot in ["Logo", "Splash", "MainSectionPicture"] {
            src = mutate(&src, &format!("Ext/{slot}/Picture.png"), png.clone());
        }
        ibcmd_xml::source_tree::publish_new(&src, destination.join(dialect)).unwrap();
    }
}

#[test]
fn original_sdk_eint_point_type_accepts_full_signed_range_and_rejects_overflow() {
    for tag in ["transparentPixel", "glyph"] {
        for shape in [
            format!(
                "<logo><{tag} x=\"{}\" y=\"{}\"/></logo>",
                i32::MIN,
                i32::MAX
            ),
            format!(
                "<logo><{tag}><x>{}</x><y>{}</y></{tag}></logo>",
                i32::MIN,
                i32::MAX
            ),
        ] {
            assert!(matches!(
                formats_xml::md_picture::decode(
                    &formats_xml::parse(shape.as_bytes()).unwrap().root
                ),
                morph1c_core::engine::Decoded::Present(_)
            ));
        }
        for coordinate in [i64::from(i32::MIN) - 1, i64::from(i32::MAX) + 1] {
            for shape in [
                format!("<logo><{tag} x=\"{coordinate}\"/></logo>"),
                format!("<logo><{tag}><y>{coordinate}</y></{tag}></logo>"),
            ] {
                assert!(matches!(
                    formats_xml::md_picture::decode(
                        &formats_xml::parse(shape.as_bytes()).unwrap().root
                    ),
                    morph1c_core::engine::Decoded::Error(_)
                ));
            }
            assert!(
                formats_xml::md_picture::encode(
                    "",
                    "logo",
                    &formats_xml::md_picture::present_with_glyph(Some((coordinate, 0)), None)
                )
                .is_err()
            );
        }
    }
    let src = glyph_source("2.21");
    let resource = format!("Ext/{}", formats_xml::md_picture::RESOURCE);
    let mut dto: serde_json::Value = serde_json::from_slice(bytes(&src, &resource)).unwrap();
    dto["pictures"][0]["glyph"]["x"] = serde_json::json!(i64::from(i32::MAX) + 1);
    assert!(
        xml_to_edt(
            &mutate(&src, &resource, serde_json::to_vec(&dto).unwrap()),
            &options("2.21")
        )
        .is_err()
    );
}
