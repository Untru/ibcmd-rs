use ibcmd_edt::{
    ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
};
use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
use morph1c_core::{
    ir::{FieldId, MetadataObject, ObjectKind, PropertyValue, Uuid},
    version::{FormatVersion, with_roundtrip_target},
};
use morph1c_pipeline::{Format, registry::FormatRegistry};
use sha2::{Digest, Sha256};

fn cases() -> [(&'static str, &'static str, FieldId, &'static str); 5] {
    use morph1c_core::spec::metadata::{catalog, enumeration, information_register};
    [
        (
            "Catalog",
            "Catalogs",
            catalog::F_AUX_FOLDER_FORM,
            "auxiliaryFolderForm",
        ),
        (
            "Enum",
            "Enums",
            enumeration::F_AUXILIARY_LIST_FORM,
            "auxiliaryListForm",
        ),
        (
            "Enum",
            "Enums",
            enumeration::F_AUXILIARY_CHOICE_FORM,
            "auxiliaryChoiceForm",
        ),
        (
            "InformationRegister",
            "InformationRegisters",
            information_register::F_AUXILIARY_LIST_FORM,
            "auxiliaryListForm",
        ),
        (
            "InformationRegister",
            "InformationRegisters",
            information_register::F_AUXILIARY_RECORD_FORM,
            "auxiliaryRecordForm",
        ),
    ]
}
fn object(kind: &str, field: FieldId) -> MetadataObject {
    let mut obj = MetadataObject::new(ObjectKind::new(kind), "AuxOwner", Uuid([0x45; 16]));
    obj.properties.push((
        field,
        PropertyValue::Str(format!("{kind}.AuxOwner.Form.AliasRole")),
    ));
    if kind == "Catalog" {
        use morph1c_core::spec::metadata::catalog;
        obj.properties
            .push((catalog::F_LEVEL_COUNT, PropertyValue::Int(0)));
        obj.properties.push((
            catalog::F_STANDARD_ATTRIBUTES,
            PropertyValue::List(Vec::new()),
        ));
    } else if kind == "InformationRegister" {
        obj.properties.push((
            morph1c_core::spec::metadata::information_register::F_STANDARD_ATTRIBUTES,
            PropertyValue::List(Vec::new()),
        ));
    }
    obj
}
fn write(format: Format, obj: &MetadataObject, version: FormatVersion) -> Vec<u8> {
    let reg = FormatRegistry::for_format(format).unwrap();
    with_roundtrip_target(version, || (reg.get(obj.kind.as_str()).unwrap().write)(obj)).unwrap()
}
fn read(format: Format, kind: &str, bytes: &[u8]) -> Result<MetadataObject, String> {
    (FormatRegistry::for_format(format)
        .unwrap()
        .get(kind)
        .unwrap()
        .read)(bytes)
}
#[test]
fn auxiliary_refs_keep_roles_and_edits_in_both_dialects() {
    for version in [FormatVersion::new(2, 20), FormatVersion::new(2, 21)] {
        for (kind, _, field, tag) in cases() {
            let obj = object(kind, field);
            for format in [Format::Edt, Format::Designer] {
                let bytes = write(format, &obj, version);
                let decoded = read(format, kind, &bytes).unwrap();
                assert_eq!(
                    decoded.properties.iter().find(|(id, _)| *id == field),
                    obj.properties.first()
                );
                let mut edited = decoded.clone();
                edited
                    .properties
                    .iter_mut()
                    .find(|(id, _)| *id == field)
                    .unwrap()
                    .1 = PropertyValue::Str(format!("{kind}.AuxOwner.Form.EditedAlias"));
                assert_ne!(
                    serde_json::to_vec(&decoded).unwrap(),
                    serde_json::to_vec(&edited).unwrap()
                );
                for target in [Format::Edt, Format::Designer] {
                    let result = read(target, kind, &write(target, &edited, version)).unwrap();
                    assert_eq!(
                        result.properties.iter().find(|(id, _)| *id == field),
                        edited.properties.iter().find(|(id, _)| *id == field)
                    );
                }
            }
            let edt = String::from_utf8(write(Format::Edt, &obj, version)).unwrap();
            let slot = format!("<{tag}>{kind}.AuxOwner.Form.AliasRole</{tag}>");
            assert!(edt.contains(&slot));
            for malformed in [
                edt.replace(&slot, &format!("{slot}{slot}")),
                edt.replace(
                    &slot,
                    &format!("<{tag} extra=\"true\">{kind}.AuxOwner.Form.AliasRole</{tag}>"),
                ),
                edt.replace(&slot, &format!("<{tag}><unknown/></{tag}>")),
            ] {
                assert!(read(Format::Edt, kind, malformed.as_bytes()).is_err());
            }
        }
    }
}
fn replace(tree: &SourceTree, path: &str, bytes: Vec<u8>) -> SourceTree {
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
fn auxiliary_reference_edits_reject_rehashed_stale_provenance() {
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
    for (kind, family, field, _) in cases() {
        let obj = object(kind, field);
        let path = format!("{family}/AuxOwner.xml");
        let source = replace(
            &base,
            &path,
            write(Format::Designer, &obj, FormatVersion::new(2, 21)),
        );
        let generated = xml_to_edt(&source, &opts).unwrap().tree;
        assert_eq!(
            edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &opts)
                .unwrap()
                .tree,
            source
        );
        let edt_path = format!("src/{family}/AuxOwner/AuxOwner.mdo");
        let mut edited = obj.clone();
        edited.properties[0].1 = PropertyValue::Str(format!("{kind}.AuxOwner.Form.EditedAlias"));
        let bytes = write(Format::Edt, &edited, FormatVersion::new(2, 21));
        let hash = format!("{:x}", Sha256::digest(&bytes));
        let changed = replace(&generated, &edt_path, bytes);
        let mut manifest: serde_json::Value = serde_json::from_slice(
            changed
                .entries()
                .iter()
                .find(|e| e.path().as_str() == ".ibcmd-provenance/manifest.json")
                .unwrap()
                .bytes(),
        )
        .unwrap();
        manifest["generated"][&edt_path] = serde_json::Value::String(hash);
        let forged = replace(
            &changed,
            ".ibcmd-provenance/manifest.json",
            serde_json::to_vec(&manifest).unwrap(),
        );
        assert!(edt_to_xml(&Project::from_tree(forged).unwrap(), &opts).is_err());
        let converted = edt_to_xml(&Project::from_tree(strip(&changed)).unwrap(), &opts).unwrap();
        let result = read(
            Format::Designer,
            kind,
            converted
                .tree
                .entries()
                .iter()
                .find(|e| e.path().as_str() == path)
                .unwrap()
                .bytes(),
        )
        .unwrap();
        assert_eq!(
            result.properties.iter().find(|(id, _)| *id == field),
            edited.properties.first()
        );
    }
}
#[test]
#[ignore = "requires immutable genuine UH83 EDT and native XML corpus"]
fn genuine_uha_auxiliary_refs_match_and_regenerate() {
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
    for (kind, family, name, fields) in [
        ("Catalog", "Catalogs", "ВидыОтчетов", vec![cases()[0].2]),
        (
            "Enum",
            "Enums",
            "УдалитьПредметыАренды",
            vec![cases()[1].2, cases()[2].2],
        ),
        (
            "InformationRegister",
            "InformationRegisters",
            "ИзмененныеОбъектыДляВыгрузки",
            vec![cases()[3].2],
        ),
    ] {
        let ebytes = bounded_read(&edt.join(format!("src/{family}/{name}/{name}.mdo")));
        let xbytes = bounded_read(&native.join(format!("{family}/{name}.xml")));
        let e = read(Format::Edt, kind, &ebytes).unwrap();
        // Verify the exact genuine native root slots independently. The original
        // Catalog also exercises the separately typed command-picture pixel codec.
        let native_doc = formats_xml::parse(&xbytes).unwrap();
        let properties = native_doc
            .root
            .children
            .iter()
            .find(|c| c.local == kind)
            .unwrap()
            .children
            .iter()
            .find(|c| c.local == "Properties")
            .unwrap();
        let x = Some(read(Format::Designer, kind, &xbytes).unwrap());
        for field in fields {
            let ev = e.properties.iter().find(|(id, _)| *id == field).unwrap();
            let tag = cases()
                .into_iter()
                .find(|(k, _, id, _)| *k == kind && *id == field)
                .unwrap()
                .3;
            let native_tag = format!("{}{}", tag[..1].to_ascii_uppercase(), &tag[1..]);
            let slot = properties
                .children
                .iter()
                .filter(|c| c.local == native_tag)
                .collect::<Vec<_>>();
            assert_eq!(slot.len(), 1);
            assert!(slot[0].attrs.is_empty() && slot[0].children.is_empty());
            assert_eq!(ev.1, PropertyValue::Str(slot[0].text.clone()));
            if let Some(x) = &x {
                assert_eq!(Some(ev), x.properties.iter().find(|(id, _)| *id == field));
            }
            for source in std::iter::once(&e).chain(x.as_ref()) {
                for target in [Format::Edt, Format::Designer] {
                    // Bare Designer form references require whole-project form
                    // attachment before EDT emission. This descriptor-only test
                    // checks root auxiliary references; keep that scope explicit.
                    let mut root_slots = source.clone();
                    root_slots.children.clear();
                    let decoded = read(
                        target,
                        kind,
                        &write(target, &root_slots, FormatVersion::new(2, 20)),
                    )
                    .unwrap();
                    assert_eq!(
                        decoded.properties.iter().find(|(id, _)| *id == field),
                        Some(ev)
                    );
                }
            }
        }
        // Full byte regeneration is recorded by the complete-kind lab census;
        // this paired test covers the exact typed auxiliary roles specifically.
    }
}

#[test]
#[ignore = "read-only complete three-kind EDT lab census"]
fn genuine_complete_auxiliary_kind_census() {
    use std::time::Instant;
    let root = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_CENSUS_ROOT").unwrap());
    let report = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_CENSUS_REPORT").unwrap());
    let dialect: u16 = std::env::var("IBCMD_EDT_CENSUS_DIALECT")
        .unwrap()
        .parse()
        .unwrap();
    let version = FormatVersion::new(2, dialect);
    let reg = FormatRegistry::for_format(Format::Edt).unwrap();
    let start = Instant::now();
    let mut rows = Vec::new();
    let mut failures = Vec::new();
    let mut semantic_diffs = Vec::new();
    let mut byte_diffs = Vec::new();
    let mut counts = std::collections::BTreeMap::new();
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let snapshot = tempfile::tempdir_in(&lab).unwrap();
    let file = snapshot.path().join("descriptor.mdo");
    for (kind, family) in [
        ("Catalog", "Catalogs"),
        ("Enum", "Enums"),
        ("InformationRegister", "InformationRegisters"),
    ] {
        let mut objects = std::fs::read_dir(root.join("src").join(family))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect::<Vec<_>>();
        objects.sort();
        let mut count = 0;
        for object in objects {
            assert!(object.is_dir());
            let path = object
                .join(object.file_name().unwrap())
                .with_extension("mdo");
            assert!(path.is_file(), "{}", path.display());
            std::fs::copy(&path, &file).unwrap();
            let tree = read_xml_source(snapshot.path(), ReaderLimits::default()).unwrap();
            let bytes = tree.entries()[0].bytes();
            let relative = path
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            rows.push(serde_json::json!({"path":relative,"sha256":format!("{:x}",Sha256::digest(bytes)),"bytes":bytes.len()}));
            count += 1;
            match (reg.get(kind).unwrap().read)(bytes) {
                Err(error) => failures.push(serde_json::json!({"path":relative,"error":error})),
                Ok(obj) => {
                    match with_roundtrip_target(version, || (reg.get(kind).unwrap().write)(&obj)) {
                        Err(error) => {
                            failures.push(serde_json::json!({"path":relative,"encode_error":error}))
                        }
                        Ok(output) => {
                            if bytes != output {
                                let witness = report
                                    .with_extension("witnesses")
                                    .join(byte_diffs.len().to_string());
                                std::fs::create_dir_all(&witness).unwrap();
                                std::fs::write(witness.join("source.mdo"), bytes).unwrap();
                                std::fs::write(witness.join("generated.mdo"), &output).unwrap();
                                std::fs::write(
                                    witness.join("sourcepath.json"),
                                    serde_json::to_vec(&relative).unwrap(),
                                )
                                .unwrap();
                                byte_diffs.push(serde_json::json!({"path":relative,"source_sha256":format!("{:x}",Sha256::digest(bytes)),"output_sha256":format!("{:x}",Sha256::digest(&output))}));
                            }
                            match (reg.get(kind).unwrap().read)(&output) {
                                Err(error)=>failures.push(serde_json::json!({"path":relative,"regenerated_read_error":error})),
                                Ok(decoded)=>if serde_json::to_vec(&obj).unwrap()!=serde_json::to_vec(&decoded).unwrap() {semantic_diffs.push(relative)},
                            }
                        }
                    }
                }
            }
            if count % 100 == 0 {
                eprintln!(
                    "{kind}: {count}, elapsed {:.1}s, failures {}",
                    start.elapsed().as_secs_f64(),
                    failures.len()
                );
            }
        }
        counts.insert(kind, count);
    }
    let result = serde_json::json!({"root":root,"dialect":dialect,"counts":counts,"elapsed_seconds":start.elapsed().as_secs_f64(),"files":rows,"failures":failures,"semantic_diffs":semantic_diffs,"byte_diffs":byte_diffs});
    std::fs::write(&report, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    assert!(
        failures.is_empty() && semantic_diffs.is_empty(),
        "failure counts read/write={} semantic={}; report {}",
        failures.len(),
        semantic_diffs.len(),
        report.display()
    );
}

#[test]
fn catalog_standard_description_multiline_is_typed_and_edited_semantics() {
    use formats_xml::std_attrs_generic::{self, GenDialect};
    use morph1c_core::{
        engine::Decoded,
        spec::{common::StdAttrsVariant, metadata::catalog},
    };
    let mut source = String::from("<root xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">");
    for name in [
        "PredefinedDataName",
        "Predefined",
        "Ref",
        "DeletionMark",
        "IsFolder",
        "Owner",
        "Parent",
        "Description",
        "Code",
    ] {
        source.push_str(&format!("<standardAttributes><name>{name}</name><fillValue xsi:type=\"core:UndefinedValue\"/>{}<minValue xsi:type=\"core:UndefinedValue\"/><maxValue xsi:type=\"core:UndefinedValue\"/></standardAttributes>",if name=="Description" {"<multiLine>true</multiLine>"} else {""}));
    }
    source.push_str("</root>");
    let parse = |s: &str| {
        let doc = formats_xml::parse(s.as_bytes()).unwrap();
        std_attrs_generic::decode(
            GenDialect::Edt,
            &catalog::STD_ATTRS,
            StdAttrsVariant::Root,
            &doc.root,
            FormatVersion::new(2, 20),
        )
    };
    let value = match parse(&source) {
        Decoded::Present(v) => v,
        Decoded::Error(e) => panic!("{e}"),
        _ => panic!("absent"),
    };
    let PropertyValue::List(records) = &value else {
        panic!("list")
    };
    let PropertyValue::List(description) = &records[7] else {
        panic!("record")
    };
    assert_eq!(description[12], PropertyValue::Bool(true));
    let mut obj = object("Catalog", catalog::F_AUX_FOLDER_FORM);
    obj.properties
        .iter_mut()
        .find(|(id, _)| *id == catalog::F_STANDARD_ATTRIBUTES)
        .unwrap()
        .1 = value;
    for dialect in [Format::Edt, Format::Designer] {
        let output = write(dialect, &obj, FormatVersion::new(2, 20));
        let readback = read(dialect, "Catalog", &output).unwrap();
        assert_eq!(
            readback
                .properties
                .iter()
                .find(|(id, _)| *id == catalog::F_STANDARD_ATTRIBUTES),
            obj.properties
                .iter()
                .find(|(id, _)| *id == catalog::F_STANDARD_ATTRIBUTES)
        );
    }
    let mut edited = obj.clone();
    if let PropertyValue::List(records) = &mut edited
        .properties
        .iter_mut()
        .find(|(id, _)| *id == catalog::F_STANDARD_ATTRIBUTES)
        .unwrap()
        .1
        && let PropertyValue::List(description) = &mut records[7]
    {
        description[12] = PropertyValue::Bool(false);
    }
    assert_ne!(
        serde_json::to_vec(&edited).unwrap(),
        serde_json::to_vec(&obj).unwrap()
    );
    let source = String::from_utf8(write(Format::Edt, &obj, FormatVersion::new(2, 20))).unwrap();
    for bad in [
        source.replace("<multiLine>true</multiLine>", "<multiLine>yes</multiLine>"),
        source.replace(
            "<multiLine>true</multiLine>",
            "<multiLine>true</multiLine><multiLine>false</multiLine>",
        ),
        source.replace(
            "<multiLine>true</multiLine>",
            "<multiLine extra=\"true\">true</multiLine>",
        ),
    ] {
        assert!(read(Format::Edt, "Catalog", bad.as_bytes()).is_err());
    }
}
