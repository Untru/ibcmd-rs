use formats_xml::{metadata_picture_semantics as carrier, picture};
use ibcmd_edt::{
    ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
};
use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
use morph1c_core::{
    ir::{MetadataObject, ObjectKind, PropertyValue, Token, Uuid},
    spec::metadata::{catalog, catalog_command as cmd},
    version::{FormatVersion, with_roundtrip_target},
};
use morph1c_pipeline::{Format, registry::FormatRegistry};
use sha2::{Digest, Sha256};
fn owner() -> MetadataObject {
    let mut root =
        MetadataObject::new(ObjectKind::new("Catalog"), "PictureOwner", Uuid([0x18; 16]));
    root.properties
        .push((catalog::F_LEVEL_COUNT, PropertyValue::Int(0)));
    root.properties.push((
        catalog::F_STANDARD_ATTRIBUTES,
        PropertyValue::List(Vec::new()),
    ));
    let mut command = MetadataObject::new(
        ObjectKind::new("Catalog.Command"),
        "Print",
        Uuid([0x19; 16]),
    );
    command.properties.push((
        cmd::F_GROUP,
        PropertyValue::Enum(Token::new("FormNavigationPanelSeeAlso")),
    ));
    command.properties.push((
        cmd::F_PICTURE,
        picture::pack("CommonPicture.Print".into(), true, Some((12, 12))),
    ));
    root.children.push(command);
    root
}
fn write(format: Format, obj: &MetadataObject) -> Vec<u8> {
    with_roundtrip_target(FormatVersion::new(2, 21), || {
        (FormatRegistry::for_format(format)
            .unwrap()
            .get("Catalog")
            .unwrap()
            .write)(obj)
    })
    .unwrap()
}
fn read(format: Format, bytes: &[u8]) -> MetadataObject {
    (FormatRegistry::for_format(format)
        .unwrap()
        .get("Catalog")
        .unwrap()
        .read)(bytes)
    .unwrap()
}
#[test]
fn command_pixel_tuple_is_semantic_and_source_native_exact() {
    let obj = owner();
    let native = write(Format::Designer, &obj);
    let decoded = read(Format::Designer, &native);
    assert_eq!(
        decoded.children[0].get(cmd::F_PICTURE),
        obj.children[0].get(cmd::F_PICTURE)
    );
    assert_eq!(write(Format::Designer, &decoded), native);
    let reg = FormatRegistry::for_format(Format::Edt).unwrap();
    assert!(
        (reg.get("Catalog").unwrap().write)(&decoded).is_err(),
        "bare SDK PictureRef cannot silently discard data"
    );
    let (projected, resource) = carrier::project(&decoded).unwrap();
    let mut edt = read(Format::Edt, &write(Format::Edt, &projected));
    carrier::apply(&mut edt, &resource.unwrap()).unwrap();
    assert_eq!(
        serde_json::to_vec(&edt).unwrap(),
        serde_json::to_vec(&decoded).unwrap()
    );
    assert_eq!(write(Format::Designer, &edt), native);
    let mut edited = edt.clone();
    edited.children[0]
        .properties
        .iter_mut()
        .find(|(id, _)| *id == cmd::F_PICTURE)
        .unwrap()
        .1 = picture::pack("CommonPicture.Print".into(), false, Some((-1, 4)));
    assert_ne!(
        serde_json::to_vec(&edt).unwrap(),
        serde_json::to_vec(&edited).unwrap()
    );
    let (projected, resource) = carrier::project(&edited).unwrap();
    let mut readback = read(Format::Edt, &write(Format::Edt, &projected));
    carrier::apply(&mut readback, &resource.unwrap()).unwrap();
    assert_eq!(
        readback.children[0].get(cmd::F_PICTURE),
        edited.children[0].get(cmd::F_PICTURE)
    );
}
#[test]
fn metadata_carrier_binding_and_schema_fail_closed_atomically() {
    let original = owner();
    let (projection, bytes) = carrier::project(&original).unwrap();
    let bytes = bytes.unwrap();
    let model = read(Format::Edt, &write(Format::Edt, &projection));
    let valid: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let mut variants = Vec::new();
    for edit in [
        "schema",
        "owner_uuid",
        "command_uuid",
        "command_kind",
        "slot",
        "ref",
        "extra",
        "duplicate",
        "pixel",
    ] {
        let mut changed = valid.clone();
        match edit {
            "schema" => changed["schema"] = serde_json::json!("urn:unknown"),
            "owner_uuid" => changed["owner_uuid"] = serde_json::json!(vec![0_u8; 16]),
            "command_uuid" => {
                changed["pictures"][0]["command_uuid"] = serde_json::json!(vec![0_u8; 16])
            }
            "command_kind" => {
                changed["pictures"][0]["command_kind"] = serde_json::json!("Document.Command")
            }
            "slot" => changed["pictures"][0]["slot"] = serde_json::json!("Unknown"),
            "ref" => {
                changed["pictures"][0]["reference"] = serde_json::json!("CommonPicture.Edited")
            }
            "extra" => changed["pictures"][0]["unknown"] = serde_json::json!(true),
            "duplicate" => {
                let row = changed["pictures"][0].clone();
                changed["pictures"].as_array_mut().unwrap().push(row);
            }
            "pixel" => changed["pictures"][0]["pixel"]["x"] = serde_json::json!(1.5),
            _ => unreachable!(),
        }
        variants.push(serde_json::to_vec(&changed).unwrap());
    }
    for bytes in variants {
        let mut current = model.clone();
        assert!(carrier::apply(&mut current, &bytes).is_err());
        assert_eq!(
            serde_json::to_vec(&current).unwrap(),
            serde_json::to_vec(&model).unwrap()
        );
    }
    let mut deleted = model.clone();
    deleted.children.clear();
    assert!(carrier::apply(&mut deleted, &bytes).is_err());
    let mut edited = model.clone();
    edited.children[0]
        .properties
        .iter_mut()
        .find(|(id, _)| *id == cmd::F_PICTURE)
        .unwrap()
        .1 = picture::pack("StdPicture.Help".into(), true, None);
    assert!(carrier::apply(&mut edited, &bytes).is_err());
    let native = String::from_utf8(write(Format::Designer, &original)).unwrap();
    assert!(native.contains("<xr:TransparentPixel x=\"12\" y=\"12\"/>"));
    for malformed in [
        native.replace("x=\"12\"", "x=\"x\""),
        native.replace("y=\"12\"", "z=\"12\""),
        native.replace("x=\"12\"", "x=\"12\" extra=\"1\""),
        native.replace(
            "<xr:TransparentPixel x=\"12\" y=\"12\"/>",
            "<xr:TransparentPixel x=\"12\" y=\"12\"/><xr:TransparentPixel x=\"1\" y=\"2\"/>",
        ),
    ] {
        assert!(
            (FormatRegistry::for_format(Format::Designer)
                .unwrap()
                .get("Catalog")
                .unwrap()
                .read)(malformed.as_bytes())
            .is_err()
        );
    }
}
fn mutate(tree: &SourceTree, path: &str, bytes: Vec<u8>) -> SourceTree {
    let mut files = tree
        .entries()
        .iter()
        .filter(|e| e.path().as_str() != path)
        .cloned()
        .collect::<Vec<_>>();
    files.push(SourceEntry::from_bytes(SourcePath::new(path).unwrap(), bytes).unwrap());
    SourceTree::new(files).unwrap()
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
#[test]
fn public_carrier_preserves_pixels_without_provenance_and_rejects_rehashed_stale_edits() {
    let opts = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    let base = read_xml_source(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        ),
        ReaderLimits::default(),
    )
    .unwrap();
    let source = mutate(
        &base,
        "Catalogs/PictureOwner.xml",
        write(Format::Designer, &owner()),
    );
    let converted = xml_to_edt(&source, &opts).unwrap();
    assert_eq!(
        converted
            .extensions
            .iter()
            .find(|e| e.id == "ibcmd-metadata-picture-semantics/1")
            .map(|e| (e.resources, e.references)),
        Some((1, 1))
    );
    let generated = converted.tree;
    assert_eq!(
        edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &opts)
            .unwrap()
            .tree,
        source
    );
    let returned = edt_to_xml(&Project::from_tree(strip(&generated)).unwrap(), &opts).unwrap();
    assert_eq!(
        returned
            .tree
            .entries()
            .iter()
            .find(|e| e.path().as_str() == "Catalogs/PictureOwner.xml")
            .unwrap()
            .bytes(),
        source
            .entries()
            .iter()
            .find(|e| e.path().as_str() == "Catalogs/PictureOwner.xml")
            .unwrap()
            .bytes()
    );
    let resource_path = format!("src/Catalogs/PictureOwner/{}", carrier::RESOURCE);
    for edit in ["pixel", "lt", "ref"] {
        let mut resource: serde_json::Value = serde_json::from_slice(
            generated
                .entries()
                .iter()
                .find(|e| e.path().as_str() == resource_path)
                .unwrap()
                .bytes(),
        )
        .unwrap();
        match edit {
            "pixel" => resource["pictures"][0]["pixel"]["x"] = serde_json::json!(13),
            "lt" => resource["pictures"][0]["load_transparent"] = serde_json::json!(false),
            "ref" => {
                resource["pictures"][0]["reference"] = serde_json::json!("CommonPicture.Edited")
            }
            _ => unreachable!(),
        };
        let bytes = serde_json::to_vec(&resource).unwrap();
        let hash = format!("{:x}", Sha256::digest(&bytes));
        let changed = mutate(&generated, &resource_path, bytes);
        let mut manifest: serde_json::Value = serde_json::from_slice(
            changed
                .entries()
                .iter()
                .find(|e| e.path().as_str() == ".ibcmd-provenance/manifest.json")
                .unwrap()
                .bytes(),
        )
        .unwrap();
        manifest["generated"][&resource_path] = serde_json::json!(hash);
        let forged = mutate(
            &changed,
            ".ibcmd-provenance/manifest.json",
            serde_json::to_vec(&manifest).unwrap(),
        );
        assert!(edt_to_xml(&Project::from_tree(forged).unwrap(), &opts).is_err());
        let result = edt_to_xml(&Project::from_tree(strip(&changed)).unwrap(), &opts);
        if edit == "ref" {
            assert!(result.is_err());
        } else {
            let out = result.unwrap();
            let descriptor = out
                .tree
                .entries()
                .iter()
                .find(|e| e.path().as_str() == "Catalogs/PictureOwner.xml")
                .unwrap();
            let value = read(Format::Designer, descriptor.bytes());
            let (_, lt, pixel) =
                picture::unpack(value.children[0].get(cmd::F_PICTURE).unwrap()).unwrap();
            assert_eq!(
                (lt, pixel),
                if edit == "pixel" {
                    (true, Some((13, 12)))
                } else {
                    (false, Some((12, 12)))
                }
            );
        }
    }
    // A coordinated reference edit is new current semantics, not a stale
    // binding: descriptor and resource both name the same edited reference.
    let descriptor_path = "src/Catalogs/PictureOwner/PictureOwner.mdo";
    let descriptor = generated
        .entries()
        .iter()
        .find(|e| e.path().as_str() == descriptor_path)
        .unwrap();
    let changed_descriptor = String::from_utf8(descriptor.bytes().to_vec())
        .unwrap()
        .replace("CommonPicture.Print", "CommonPicture.Edited")
        .into_bytes();
    let mut resource: serde_json::Value = serde_json::from_slice(
        generated
            .entries()
            .iter()
            .find(|e| e.path().as_str() == resource_path)
            .unwrap()
            .bytes(),
    )
    .unwrap();
    resource["pictures"][0]["reference"] = serde_json::json!("CommonPicture.Edited");
    let resource_bytes = serde_json::to_vec(&resource).unwrap();
    let changed = mutate(
        &mutate(&generated, descriptor_path, changed_descriptor.clone()),
        &resource_path,
        resource_bytes.clone(),
    );
    let mut manifest: serde_json::Value = serde_json::from_slice(
        changed
            .entries()
            .iter()
            .find(|e| e.path().as_str() == ".ibcmd-provenance/manifest.json")
            .unwrap()
            .bytes(),
    )
    .unwrap();
    manifest["generated"][descriptor_path] =
        serde_json::json!(format!("{:x}", Sha256::digest(&changed_descriptor)));
    manifest["generated"][&resource_path] =
        serde_json::json!(format!("{:x}", Sha256::digest(&resource_bytes)));
    let forged = mutate(
        &changed,
        ".ibcmd-provenance/manifest.json",
        serde_json::to_vec(&manifest).unwrap(),
    );
    assert!(edt_to_xml(&Project::from_tree(forged).unwrap(), &opts).is_err());
    let returned = edt_to_xml(&Project::from_tree(strip(&changed)).unwrap(), &opts).unwrap();
    let value = read(
        Format::Designer,
        returned
            .tree
            .entries()
            .iter()
            .find(|e| e.path().as_str() == "Catalogs/PictureOwner.xml")
            .unwrap()
            .bytes(),
    );
    assert_eq!(
        picture::unpack(value.children[0].get(cmd::F_PICTURE).unwrap()).unwrap(),
        ("CommonPicture.Edited", true, Some((12, 12)))
    );
    let no_owner = SourceTree::new(
        strip(&generated)
            .entries()
            .iter()
            .filter(|e| e.path().as_str() != descriptor_path)
            .cloned()
            .collect(),
    )
    .unwrap();
    assert!(edt_to_xml(&Project::from_tree(no_owner).unwrap(), &opts).is_err());
    let missing = SourceTree::new(
        generated
            .entries()
            .iter()
            .filter(|e| e.path().as_str() != resource_path)
            .cloned()
            .collect(),
    )
    .unwrap();
    assert!(edt_to_xml(&Project::from_tree(missing).unwrap(), &opts).is_err());
}

#[test]
fn empty_reference_keeps_explicit_nondefault_transparency() {
    for (flag, pixel) in [(false, None), (true, Some((0, 0))), (false, Some((-1, 4)))] {
        let value = picture::pack(String::new(), flag, pixel);
        let mut obj = owner();
        obj.children[0]
            .properties
            .iter_mut()
            .find(|(id, _)| *id == cmd::F_PICTURE)
            .unwrap()
            .1 = value;
        let native = write(Format::Designer, &obj);
        let model = read(Format::Designer, &native);
        assert_eq!(write(Format::Designer, &model), native);
        let (projection, bytes) = carrier::project(&model).unwrap();
        let mut readback = read(Format::Edt, &write(Format::Edt, &projection));
        carrier::apply(&mut readback, &bytes.unwrap()).unwrap();
        assert_eq!(write(Format::Designer, &readback), native);
    }
}
#[test]
#[ignore = "requires immutable genuine UH83 metadata picture counterparts"]
fn genuine_catalog_command_resource_keeps_all_current_values() {
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let native = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_UHA_XML").unwrap());
    let edt = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_UHA_EDT").unwrap());
    let bounded_read = |path: &std::path::Path| {
        let tmp = tempfile::tempdir_in(&lab).unwrap();
        std::fs::copy(path, tmp.path().join(path.file_name().unwrap())).unwrap();
        read_xml_source(tmp.path(), ReaderLimits::default())
            .unwrap()
            .entries()[0]
            .bytes()
            .to_vec()
    };
    let native_bytes = bounded_read(&native.join("Catalogs/ВидыОтчетов.xml"));
    let edt_bytes = bounded_read(&edt.join("src/Catalogs/ВидыОтчетов/ВидыОтчетов.mdo"));
    let original = read(Format::Designer, &native_bytes);
    let authentic = read(Format::Edt, &edt_bytes);
    let actual = original
        .children
        .iter()
        .find(|c| {
            c.kind.as_str() == "Catalog.Command"
                && c.get(cmd::F_PICTURE)
                    .is_some_and(|v| picture::unpack(v).unwrap().2.is_some())
        })
        .unwrap();
    let (_, flag, pixel) = picture::unpack(actual.get(cmd::F_PICTURE).unwrap()).unwrap();
    assert!(flag && pixel.is_some());
    let counterpart = authentic
        .children
        .iter()
        .find(|c| c.uuid == actual.uuid)
        .unwrap();
    assert_eq!(
        picture::unpack(actual.get(cmd::F_PICTURE).unwrap())
            .unwrap()
            .0,
        picture::unpack(counterpart.get(cmd::F_PICTURE).unwrap())
            .unwrap()
            .0
    );
    // Genuine command UUID/name/values travel in a small otherwise synthetic
    // owner. The complete source trees stay immutable; no proxy picture is made.
    let mut mini = owner();
    mini.children = vec![actual.clone()];
    let source_native = write(Format::Designer, &mini);
    let source_model = read(Format::Designer, &source_native);
    let (projection, bytes) = carrier::project(&source_model).unwrap();
    let mut readback = read(Format::Edt, &write(Format::Edt, &projection));
    carrier::apply(&mut readback, &bytes.unwrap()).unwrap();
    assert_eq!(write(Format::Designer, &readback), source_native);
    let opts = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    let base = read_xml_source(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        ),
        ReaderLimits::default(),
    )
    .unwrap();
    let source = mutate(&base, "Catalogs/PictureOwner.xml", source_native);
    let output = xml_to_edt(&source, &opts).unwrap();
    let returned = edt_to_xml(&Project::from_tree(strip(&output.tree)).unwrap(), &opts).unwrap();
    assert_eq!(
        returned
            .tree
            .entries()
            .iter()
            .find(|e| e.path().as_str() == "Catalogs/PictureOwner.xml")
            .unwrap()
            .bytes(),
        source
            .entries()
            .iter()
            .find(|e| e.path().as_str() == "Catalogs/PictureOwner.xml")
            .unwrap()
            .bytes()
    );
    if let Some(destination) = std::env::var_os("IBCMD_METADATA_PICTURE_PROJECT") {
        ibcmd_xml::source_tree::publish_new(&strip(&output.tree), destination).unwrap();
    }
}
