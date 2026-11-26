use ibcmd_edt::{
    ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
};
use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
use sha2::{Digest, Sha256};
const NATIVE: &[u8] = b"{2,\"\",\"\",\n{\n{0},\n{0},\n{0},\n{0}\n},0}";
const CARRIER: &[u8] = b"{2,\"\",\"\",\n{\n{-1},\n{-1},\n{-1},\n{-1}\n},0}";
fn fixture() -> tempfile::TempDir {
    let cfg = read_config(
        Format::Designer,
        std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &ConvertOptions::default(),
    )
    .unwrap()
    .0;
    let dir = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &cfg, dir.path()).unwrap();
    dir
}
fn original44() -> Vec<u8> {
    [
        b"\xef\xbb\xbf".as_slice(),
        String::from_utf8(NATIVE.to_vec())
            .unwrap()
            .replace('\n', "\r\n")
            .as_bytes(),
    ]
    .concat()
}
fn options() -> ConversionOptions {
    ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    }
}
fn bytes<'a>(tree: &'a SourceTree, path: &str) -> &'a [u8] {
    tree.entries()
        .iter()
        .find(|e| e.path().as_str() == path)
        .unwrap()
        .bytes()
}
fn replace(tree: &SourceTree, path: &str, data: Vec<u8>) -> SourceTree {
    SourceTree::new(
        tree.entries()
            .iter()
            .map(|e| {
                if e.path().as_str() == path {
                    SourceEntry::from_bytes(SourcePath::new(path).unwrap(), data.clone()).unwrap()
                } else {
                    e.clone()
                }
            })
            .collect(),
    )
    .unwrap()
}
#[test]
fn public_empty_v2_keeps_all_original44_bytes_and_emits_model_equal_carrier() {
    let dir = fixture();
    std::fs::create_dir_all(dir.path().join("Ext")).unwrap();
    std::fs::write(
        dir.path().join("Ext/MobileClientSignature.bin"),
        original44(),
    )
    .unwrap();
    let original = read_xml_source(dir.path(), ReaderLimits::default()).unwrap();
    let opts = options();
    let generated = xml_to_edt(&original, &opts).unwrap().tree;
    assert_eq!(
        bytes(&generated, "src/Configuration/MobileClientSign.bin"),
        CARRIER
    );
    assert_eq!(
        bytes(&generated, "src/Configuration/MobileClientSignature.bin"),
        original44()
    );
    assert_eq!(
        edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &opts)
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
    assert_eq!(
        bytes(
            &edt_to_xml(&Project::from_tree(clean).unwrap(), &opts)
                .unwrap()
                .tree,
            "Ext/MobileClientSignature.bin"
        ),
        original44()
    );
    for edited in [
        String::from_utf8(CARRIER.to_vec())
            .unwrap()
            .replace("},0}", "},1}")
            .into_bytes(),
        b"{0,\"\",\"\"}".to_vec(),
        String::from_utf8(CARRIER.to_vec())
            .unwrap()
            .replace("{-1}", "{-2}")
            .into_bytes(),
    ] {
        let path = "src/Configuration/MobileClientSign.bin";
        let mut changed = replace(&generated, path, edited.clone());
        let manifest_path = ".ibcmd-provenance/manifest.json";
        let mut manifest: serde_json::Value =
            serde_json::from_slice(bytes(&changed, manifest_path)).unwrap();
        manifest["generated"][path] = serde_json::json!(format!("{:x}", Sha256::digest(edited)));
        changed = replace(
            &changed,
            manifest_path,
            serde_json::to_vec(&manifest).unwrap(),
        );
        assert!(edt_to_xml(&Project::from_tree(changed).unwrap(), &opts).is_err());
    }
}
#[test]
fn exact_carrier_only_and_nonempty_signature_body_remains_verbatim() {
    let cfg = read_config(
        Format::Designer,
        fixture().path(),
        &ConvertOptions::default(),
    )
    .unwrap()
    .0;
    let dir = tempfile::tempdir().unwrap();
    write_config(Format::Edt, &cfg, dir.path()).unwrap();
    let path = dir.path().join("Configuration/MobileClientSign.bin");
    for malformed in [
        String::from_utf8(CARRIER.to_vec())
            .unwrap()
            .replace("{-1}", "{- 1}"),
        String::from_utf8(CARRIER.to_vec()).unwrap() + "trailing",
        String::from_utf8(CARRIER.to_vec())
            .unwrap()
            .replace("{-1}", "{2}"),
    ] {
        std::fs::write(&path, malformed).unwrap();
        assert!(read_config(Format::Edt, dir.path(), &ConvertOptions::default()).is_err());
    }
    let full=b"{2,\"nonempty key {-1} retained verbatim\",\"digest\",{{1,{00000000-0000-0000-0000-000000000001,\"Enum.Value\"}},{0},{0},{0}},1}";
    std::fs::write(&path, full).unwrap();
    let loaded = read_config(Format::Edt, dir.path(), &ConvertOptions::default())
        .unwrap()
        .0;
    let blob = loaded
        .objects
        .iter()
        .flat_map(|o| &o.config_blobs)
        .find(|b| b.slot == "MobileClientSignature")
        .unwrap();
    assert_eq!(
        blob.mobile_signature_lexical.as_ref().unwrap().source_bytes,
        full
    );
    let out = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &loaded, out.path()).unwrap();
    assert_eq!(
        std::fs::read(out.path().join("Ext/MobileClientSignature.bin")).unwrap(),
        full
    );
    // Native negative counts are not accepted as the special EDT carrier.
    let native = fixture();
    std::fs::create_dir_all(native.path().join("Ext")).unwrap();
    std::fs::write(native.path().join("Ext/MobileClientSignature.bin"), CARRIER).unwrap();
    assert!(read_config(Format::Designer, native.path(), &ConvertOptions::default()).is_err());
}
#[test]
#[ignore = "requires immutable installed SDK mobile-signature experiment artifacts in F lab"]
fn genuine_empty_native_and_installed_model_save_match_exact_framers() {
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let actual = std::fs::read(
        lab.join("../04/release-20261001/rc/out/uha8327_db_r1/tree/Ext/MobileClientSignature.bin"),
    )
    .unwrap();
    assert_eq!(actual, original44());
    assert_eq!(
        std::fs::read(
            lab.join("mobile-carrier-headless-r1/installed-export/Ext/MobileClientSignature.bin")
        )
        .unwrap(),
        CARRIER
    );
    assert_eq!(
        std::fs::read(
            lab.join("mobile-carrier-canonical-native-r1/native-xml/Ext/MobileClientSignature.bin")
        )
        .unwrap(),
        NATIVE
    );
    assert_eq!(
        std::fs::read(lab.join("mobile-carrier-research-r1/negative-count-SDK-save.bin")).unwrap(),
        NATIVE
    );
    let shapes = lab.join("mobile-model-shape-probe-r1/partial-r4");
    assert!(
        std::fs::read_to_string(shapes.join("actual.stdout"))
            .unwrap()
            .contains("empty-true-carrier load=true full_model_equal=true")
    );
    assert_eq!(
        morph1c_pipeline::canonical_empty_mobile_signature(
            &std::fs::read(shapes.join("empty-true-carrier.bin")).unwrap(),
            Format::Edt
        )
        .unwrap()
        .unwrap(),
        std::fs::read(shapes.join("empty-true-carrier.sdk-save.bin")).unwrap()
    );
}

#[test]
fn authentic_edt_native_spelling_is_retained_only_for_unchanged_full_model() {
    let cfg = read_config(
        Format::Designer,
        fixture().path(),
        &ConvertOptions::default(),
    )
    .unwrap()
    .0;
    for source in [NATIVE.to_vec(), original44()] {
        let edt = tempfile::tempdir().unwrap();
        write_config(Format::Edt, &cfg, edt.path()).unwrap();
        std::fs::write(
            edt.path().join("Configuration/MobileClientSign.bin"),
            &source,
        )
        .unwrap();
        let mut loaded = read_config(Format::Edt, edt.path(), &ConvertOptions::default())
            .unwrap()
            .0;
        let native = tempfile::tempdir().unwrap();
        write_config(Format::Designer, &loaded, native.path()).unwrap();
        assert_eq!(
            std::fs::read(native.path().join("Ext/MobileClientSignature.bin")).unwrap(),
            source
        );
        let root = loaded
            .objects
            .iter_mut()
            .find(|o| o.kind.as_str() == "Configuration")
            .unwrap();
        let blob = root
            .config_blobs
            .iter_mut()
            .find(|b| b.slot == "MobileClientSignature")
            .unwrap();
        let before = serde_json::to_vec(blob).unwrap();
        let facet = blob.mobile_signature_lexical.take().unwrap();
        assert_eq!(
            serde_json::to_vec(blob).unwrap(),
            before,
            "lexical facet never enters semantic hash"
        );
        blob.mobile_signature_lexical = Some(facet);
        blob.bytes = String::from_utf8(NATIVE.to_vec())
            .unwrap()
            .replace("},0}", "},1}")
            .into_bytes();
        let edited = blob.bytes.clone();
        let native = tempfile::tempdir().unwrap();
        write_config(Format::Designer, &loaded, native.path()).unwrap();
        let returned = std::fs::read(native.path().join("Ext/MobileClientSignature.bin")).unwrap();
        assert_eq!(returned, edited);
        assert_ne!(
            returned, source,
            "edited model must never use stale44 facet"
        );
    }
}

#[test]
fn dual_names_require_the_same_complete_model_and_a_native_preferred_frame() {
    let cfg = read_config(
        Format::Designer,
        fixture().path(),
        &ConvertOptions::default(),
    )
    .unwrap()
    .0;
    let edt = tempfile::tempdir().unwrap();
    write_config(Format::Edt, &cfg, edt.path()).unwrap();
    let reader = edt.path().join("Configuration/MobileClientSign.bin");
    let preferred = edt.path().join("Configuration/MobileClientSignature.bin");
    std::fs::write(&reader, CARRIER).unwrap();
    for native in [NATIVE.to_vec(), original44()] {
        std::fs::write(&preferred, &native).unwrap();
        let model = read_config(Format::Edt, edt.path(), &ConvertOptions::default())
            .unwrap()
            .0;
        let out = tempfile::tempdir().unwrap();
        write_config(Format::Designer, &model, out.path()).unwrap();
        assert_eq!(
            std::fs::read(out.path().join("Ext/MobileClientSignature.bin")).unwrap(),
            native
        );
        let regenerated = tempfile::tempdir().unwrap();
        write_config(Format::Edt, &model, regenerated.path()).unwrap();
        assert_eq!(
            std::fs::read(
                regenerated
                    .path()
                    .join("Configuration/MobileClientSign.bin")
            )
            .unwrap(),
            CARRIER
        );
        assert_eq!(
            std::fs::read(
                regenerated
                    .path()
                    .join("Configuration/MobileClientSignature.bin")
            )
            .unwrap(),
            native
        );
    }
    for invalid in [
        CARRIER.to_vec(),
        b"{0,\"\",\"\"}".to_vec(),
        String::from_utf8(NATIVE.to_vec())
            .unwrap()
            .replace("},0}", "},1}")
            .into_bytes(),
        b"{2,\"not empty\",\"digest\",{{0},{0},{0},{0}},0}".to_vec(),
    ] {
        std::fs::write(&preferred, invalid).unwrap();
        assert!(read_config(Format::Edt, edt.path(), &ConvertOptions::default()).is_err());
    }
    std::fs::write(&preferred, NATIVE).unwrap();
    std::fs::remove_file(&reader).unwrap();
    assert!(read_config(Format::Edt, edt.path(), &ConvertOptions::default()).is_err());
    std::fs::write(&reader, b"{0,\"\",\"\"}").unwrap();
    assert!(read_config(Format::Edt, edt.path(), &ConvertOptions::default()).is_err());
}

#[test]
#[ignore = "requires bound genuine EDT raw export and fresh native SDK dual-name captures"]
fn genuine_dual_names_pass_unmodified_official_export_and_fresh_native_gate() {
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let capture = lab.join("mobile-dual-name-headless-r1");
    let project = capture.join("EmptyEdtDiagnosticControl/src/Configuration");
    assert_eq!(
        std::fs::read(project.join("MobileClientSign.bin")).unwrap(),
        CARRIER
    );
    assert_eq!(
        std::fs::read(project.join("MobileClientSignature.bin")).unwrap(),
        NATIVE
    );
    let binding: serde_json::Value =
        serde_json::from_slice(&std::fs::read(capture.join("bound-native-chain.json")).unwrap())
            .unwrap();
    assert_eq!(
        binding["status"],
        "DUAL_NAME_OFFICIAL_EMPTY_MODEL_CHAIN_CAPTURED"
    );
    assert_eq!(
        binding["native_reference_binding"]["native_build"],
        "8.3.27.2214"
    );
    assert_eq!(
        binding["raw_installed_export35_sha256"],
        format!("{:x}", Sha256::digest(NATIVE))
    );
    assert_eq!(
        binding["raw_native_export35_sha256"],
        format!("{:x}", Sha256::digest(NATIVE))
    );
    for path in [
        capture.join("installed-export/Ext/MobileClientSignature.bin"),
        lab.join("mobile-dual-name-native-r1/native-xml/Ext/MobileClientSignature.bin"),
    ] {
        assert_eq!(std::fs::read(path).unwrap(), NATIVE);
    }
    let native_result: serde_json::Value = serde_json::from_slice(
        &std::fs::read(lab.join("mobile-dual-name-native-r1/result.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(native_result["status"], "CAPTURED");
}

#[test]
fn empty_converted_boolean_is_preserved_and_cannot_hide_a_forged_edit() {
    use morph1c_pipeline::canonical_empty_mobile_signature;
    let flag_bytes = |data: &[u8], flag: bool| {
        let mut data = data.to_vec();
        let at = data.len() - 2;
        data[at] = if flag { b'1' } else { b'0' };
        data
    };
    assert_ne!(
        canonical_empty_mobile_signature(NATIVE, Format::Designer).unwrap(),
        canonical_empty_mobile_signature(&flag_bytes(NATIVE, true), Format::Designer).unwrap()
    );
    for flag in [false, true] {
        let original_bytes = flag_bytes(&original44(), flag);
        let dir = fixture();
        std::fs::create_dir_all(dir.path().join("Ext")).unwrap();
        std::fs::write(
            dir.path().join("Ext/MobileClientSignature.bin"),
            &original_bytes,
        )
        .unwrap();
        let original = read_xml_source(dir.path(), ReaderLimits::default()).unwrap();
        let opts = options();
        let generated = xml_to_edt(&original, &opts).unwrap().tree;
        let reader = "src/Configuration/MobileClientSign.bin";
        let preferred = "src/Configuration/MobileClientSignature.bin";
        assert_eq!(bytes(&generated, reader), flag_bytes(CARRIER, flag));
        assert_eq!(bytes(&generated, preferred), original_bytes);
        assert_eq!(
            edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &opts)
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
        assert_eq!(
            bytes(
                &edt_to_xml(&Project::from_tree(clean).unwrap(), &opts)
                    .unwrap()
                    .tree,
                "Ext/MobileClientSignature.bin"
            ),
            original_bytes
        );
        let mut changed = generated.clone();
        let manifest_path = ".ibcmd-provenance/manifest.json";
        let mut manifest: serde_json::Value =
            serde_json::from_slice(bytes(&changed, manifest_path)).unwrap();
        for (path, data) in [
            (reader, flag_bytes(CARRIER, !flag)),
            (preferred, flag_bytes(&original44(), !flag)),
        ] {
            manifest["generated"][path] = serde_json::json!(format!("{:x}", Sha256::digest(&data)));
            changed = replace(&changed, path, data);
        }
        changed = replace(
            &changed,
            manifest_path,
            serde_json::to_vec(&manifest).unwrap(),
        );
        assert!(edt_to_xml(&Project::from_tree(changed).unwrap(), &opts).is_err());
    }
    for flag in ["01", "10", "2", "-1", "true", "false"] {
        let bad = String::from_utf8(CARRIER.to_vec())
            .unwrap()
            .replace("},0}", &format!("}},{flag}}}"));
        assert!(canonical_empty_mobile_signature(bad.as_bytes(), Format::Edt).is_err());
    }
}
