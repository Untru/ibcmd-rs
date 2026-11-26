use morph1c_pipeline::{
    ConvertOptions, Format, canonical_empty_mobile_signature, read_config, write_config,
};
use std::path::Path;

fn canonical(bytes: &[u8], format: Format) -> Vec<u8> {
    canonical_empty_mobile_signature(bytes, format)
        .unwrap()
        .unwrap()
}
fn fixture() -> morph1c_core::ir::Configuration {
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
fn partial(group: usize, converted: bool) -> Vec<u8> {
    let mut text = String::from("\u{feff}{2,\"ключ \"\"цитата\"\" {0}\",\"digest {-1}\",\r\n{\r\n");
    for index in 0..4 {
        if index > 0 {
            text.push_str(",\r\n");
        }
        if index == group {
            text.push_str(
                "{1,\r\n{ABCDEF01-2345-6789-ABCD-EF0123456789,\"имя \"\"поле\"\" {-1}\"}\r\n}",
            );
        } else {
            text.push_str("{0}");
        }
    }
    text.push_str(if converted { "\r\n},1}" } else { "\r\n},0}" });
    text.into_bytes()
}

#[test]
fn all_four_partial_groups_preserve_members_strings_and_native_spelling() {
    for group in 0..4 {
        for flag in [false, true] {
            let original = partial(group, flag);
            let dir = tempfile::tempdir().unwrap();
            write_config(Format::Edt, &fixture(), dir.path()).unwrap();
            std::fs::write(
                dir.path().join("Configuration/MobileClientSign.bin"),
                &original,
            )
            .unwrap();
            let source = read_config(Format::Edt, dir.path(), &ConvertOptions::default())
                .unwrap()
                .0;
            let edt = tempfile::tempdir().unwrap();
            write_config(Format::Edt, &source, edt.path()).unwrap();
            let carrier =
                std::fs::read(edt.path().join("Configuration/MobileClientSign.bin")).unwrap();
            let expected = String::from_utf8(original.clone())
                .unwrap()
                .replace("\r\n{0}", "\r\n{-1}");
            assert_eq!(
                carrier,
                expected.as_bytes(),
                "only declared group counts may change"
            );
            assert_eq!(
                std::fs::read(edt.path().join("Configuration/MobileClientSignature.bin")).unwrap(),
                original
            );
            assert_eq!(
                canonical(&carrier, Format::Edt),
                canonical(&original, Format::Designer)
            );
            let returned = read_config(Format::Edt, edt.path(), &ConvertOptions::default())
                .unwrap()
                .0;
            let native = tempfile::tempdir().unwrap();
            write_config(Format::Designer, &returned, native.path()).unwrap();
            assert_eq!(
                std::fs::read(native.path().join("Ext/MobileClientSignature.bin")).unwrap(),
                original
            );
            for edit in ["digest", "имя", "ABCDEF01"] {
                let changed = String::from_utf8(original.clone()).unwrap().replace(
                    edit,
                    match edit {
                        "digest" => "changed",
                        "имя" => "другое",
                        _ => "ABCDEF02",
                    },
                );
                assert_ne!(
                    canonical(changed.as_bytes(), Format::Designer),
                    canonical(&original, Format::Designer)
                );
            }
        }
    }
}

#[test]
fn counts_members_and_boolean_are_closed_without_nested_negative_search() {
    let valid = partial(0, false);
    assert!(
        canonical_empty_mobile_signature(&valid, Format::Designer)
            .unwrap()
            .is_some()
    );
    let text = String::from_utf8(valid).unwrap();
    for changed in [
        text.replace("{1,", "{2,"),
        text.replace("{1,", "{0,"),
        text.replace("{1,", "{-1,"),
        text.replace("{1,", "{-2,"),
        text.replace("{0}", "{- 1}"),
        text.replace("{0}", "{-2}"),
        text.replace("{1,", "{2147483648,"),
        text.replace("ABCDEF01-2345-6789-ABCD-EF0123456789", "bad-uuid"),
        text.replace("},0}", "},2}"),
        text.clone() + "trailing",
    ] {
        assert!(canonical_empty_mobile_signature(changed.as_bytes(), Format::Edt).is_err());
    }
    assert!(
        canonical_empty_mobile_signature(b"{0,\"opaque\",{-123}}", Format::Edt)
            .unwrap()
            .is_none()
    );
    assert!(
        canonical_empty_mobile_signature(b"{2,\"legacy incomplete body\"}", Format::Edt)
            .unwrap()
            .is_none()
    );
}

#[test]
#[ignore = "requires bound genuine seven SDK models and ten partial/empty shape probes in F lab"]
fn genuine_full_models_and_all_partial_shapes_match_official_serializer() {
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let genuine = lab.join("mobile-nonempty-model-research-r1/bounded-512-model-r4");
    for index in 0..7 {
        let source = std::fs::read(genuine.join(format!("{index:02}-source.bin"))).unwrap();
        let saved =
            std::fs::read(genuine.join(format!("{index:02}-source.bin.sdk-save.bin"))).unwrap();
        assert_eq!(
            canonical(&source, Format::Designer),
            saved,
            "complete genuine model {index}"
        );
        let input = tempfile::tempdir().unwrap();
        write_config(Format::Edt, &fixture(), input.path()).unwrap();
        std::fs::write(
            input.path().join("Configuration/MobileClientSign.bin"),
            &source,
        )
        .unwrap();
        let model = read_config(Format::Edt, input.path(), &ConvertOptions::default())
            .unwrap()
            .0;
        let output = tempfile::tempdir().unwrap();
        write_config(Format::Edt, &model, output.path()).unwrap();
        assert_eq!(
            std::fs::read(output.path().join("Configuration/MobileClientSign.bin")).unwrap(),
            source,
            "all populated genuine groups retain complete carrier bytes"
        );
        assert_eq!(
            std::fs::read(
                output
                    .path()
                    .join("Configuration/MobileClientSignature.bin")
            )
            .unwrap(),
            source
        );
    }
    let shapes = lab.join("mobile-model-shape-probe-r1/partial-r4");
    let log = std::fs::read_to_string(shapes.join("actual.stdout")).unwrap();
    for flag in [false, true] {
        for label in ["empty", "group0", "group1", "group2", "group3"] {
            let label = format!("{label}-{flag}");
            assert!(log.contains(&format!("{label}-carrier load=true full_model_equal=true")));
            let native = std::fs::read(shapes.join(format!("{label}-native.bin"))).unwrap();
            let carrier = std::fs::read(shapes.join(format!("{label}-carrier.bin"))).unwrap();
            let saved =
                std::fs::read(shapes.join(format!("{label}-carrier.sdk-save.bin"))).unwrap();
            assert_eq!(canonical(&native, Format::Designer), saved);
            assert_eq!(canonical(&carrier, Format::Edt), saved);
        }
    }
    let quoted = lab.join("mobile-model-shape-probe-r1/quoted-r5");
    assert!(
        std::fs::read_to_string(quoted.join("actual.stdout"))
            .unwrap()
            .contains("full_model_equal=true exact_official_save=true")
    );
    let saved = std::fs::read(quoted.join("quoted-carrier.sdk-save.bin")).unwrap();
    assert_eq!(
        canonical(
            &std::fs::read(quoted.join("quoted-native.bin")).unwrap(),
            Format::Designer
        ),
        saved
    );
    assert_eq!(
        canonical(
            &std::fs::read(quoted.join("quoted-carrier.bin")).unwrap(),
            Format::Edt
        ),
        saved
    );
}

#[test]
fn public_partial_roundtrip_is_exact_and_member_key_or_flag_edits_reject_stale_hashes() {
    use ibcmd_edt::{
        ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
    };
    use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
    use sha2::{Digest, Sha256};
    let dir = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &fixture(), dir.path()).unwrap();
    std::fs::create_dir_all(dir.path().join("Ext")).unwrap();
    std::fs::write(
        dir.path().join("Ext/MobileClientSignature.bin"),
        partial(2, true),
    )
    .unwrap();
    let options = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    let original = read_xml_source(dir.path(), ReaderLimits::default()).unwrap();
    let generated = xml_to_edt(&original, &options).unwrap().tree;
    assert_eq!(
        edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
            .unwrap()
            .tree,
        original
    );
    let clean = SourceTree::new(
        generated
            .entries()
            .iter()
            .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap();
    let returned = edt_to_xml(&Project::from_tree(clean).unwrap(), &options)
        .unwrap()
        .tree;
    assert_eq!(
        returned
            .entries()
            .iter()
            .find(|e| e.path().as_str() == "Ext/MobileClientSignature.bin")
            .unwrap()
            .bytes(),
        partial(2, true)
    );
    for (old, new) in [
        ("имя", "другое"),
        ("ключ", "новый"),
        ("ABCDEF01", "ABCDEF02"),
        ("},1}", "},0}"),
    ] {
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
        let mut entries = Vec::new();
        for entry in generated.entries() {
            let path = entry.path().as_str();
            if path == manifest_path {
                continue;
            }
            if matches!(
                path,
                "src/Configuration/MobileClientSign.bin"
                    | "src/Configuration/MobileClientSignature.bin"
            ) {
                let data = std::str::from_utf8(entry.bytes())
                    .unwrap()
                    .replace(old, new)
                    .into_bytes();
                manifest["generated"][path] =
                    serde_json::json!(format!("{:x}", Sha256::digest(&data)));
                entries.push(SourceEntry::from_bytes(entry.path().clone(), data).unwrap());
            } else {
                entries.push(entry.clone());
            }
        }
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
}
