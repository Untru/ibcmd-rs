//! Source-codec resource tests; synthetic opaque bytes are not an SDK-validity claim.
#[path = "../vendor/morph1c/crates/pipeline/src/source_read.rs"]
mod source_read;

use morph1c_core::ir::{
    FormBody, HelpPage, HelpResource, MetadataObject, NamedFormBody, ObjectKind, PropertyValue,
    Token, Uuid,
};
use morph1c_core::version::FormatVersion;
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
use sha2::{Digest, Sha256};

fn options() -> ConvertOptions {
    ConvertOptions::default().with_target_version(FormatVersion::new(2, 21))
}
fn configuration() -> morph1c_core::ir::Configuration {
    read_config(
        Format::Designer,
        std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &options(),
    )
    .unwrap()
    .0
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn large_ordinary_help_resource_and_page_roundtrip_both_dialects() {
    let size = 32 * 1024 * 1024 + 1;
    let opaque: Vec<_> = (0..size).map(|index| (index % 251) as u8).collect();
    let opaque_hash = hash(&opaque);
    let page = "x".repeat(size);
    let mut cfg = configuration();
    let root = cfg
        .objects
        .iter_mut()
        .find(|object| object.kind.as_str() == "Configuration")
        .unwrap();
    root.help = vec![HelpPage {
        lang: "ru".into(),
        body: page.clone(),
    }];
    root.help_resources = vec![HelpResource {
        rel_path: "nested/opaque.bin".into(),
        bytes: opaque.clone(),
    }];
    let mut form = MetadataObject::new(
        ObjectKind::new("CommonForm"),
        "LargeOrdinary",
        Uuid([0x46; 16]),
    );
    let spec = morph1c_core::spec::registry::spec_for("CommonForm").unwrap();
    let form_type = spec
        .fields()
        .iter()
        .find(|field| field.name == "formType")
        .unwrap()
        .id;
    form.properties
        .push((form_type, PropertyValue::Enum(Token::new("Ordinary"))));
    form.form_bodies.push(NamedFormBody {
        name: form.name.clone(),
        body: FormBody::new(),
        ordinary_body: Some(opaque),
        module: None,
        help: vec![],
        help_resources: vec![],
    });
    cfg.objects.push(form);
    for format in [Format::Designer, Format::Edt] {
        let directory = tempfile::tempdir().unwrap();
        write_config(format, &cfg, directory.path()).unwrap();
        let (body, help, page_path) = match format {
            Format::Designer => (
                "CommonForms/LargeOrdinary/Ext/Form.bin",
                "Ext/Help/_files/nested/opaque.bin",
                "Ext/Help/ru.html",
            ),
            Format::Edt => (
                "CommonForms/LargeOrdinary/Form.oform",
                "Configuration/Help/_files/nested/opaque.bin",
                "Configuration/Help/ru.html",
            ),
            _ => unreachable!(),
        };
        assert_eq!(
            hash(&std::fs::read(directory.path().join(body)).unwrap()),
            opaque_hash
        );
        assert_eq!(
            hash(&std::fs::read(directory.path().join(help)).unwrap()),
            opaque_hash
        );
        let encoded_page = std::fs::read(directory.path().join(page_path)).unwrap();
        assert_eq!(
            encoded_page.len(),
            size + if format == Format::Designer { 3 } else { 0 }
        );
        cfg = read_config(format, directory.path(), &options()).unwrap().0;
        let root = cfg
            .objects
            .iter()
            .find(|object| object.kind.as_str() == "Configuration")
            .unwrap();
        assert_eq!(root.help[0].body.len(), page.len());
        assert!(root.help[0].body == page);
        assert_eq!(hash(&root.help_resources[0].bytes), opaque_hash);
        let form = cfg
            .objects
            .iter()
            .find(|object| object.name == "LargeOrdinary")
            .unwrap();
        assert_eq!(
            hash(form.form_bodies[0].ordinary_body.as_ref().unwrap()),
            opaque_hash
        );
        assert_eq!(form.form_bodies[0].body, FormBody::new());
        assert!(form.form_bodies[0].module.is_none());
    }
}

#[test]
fn regular_source_reader_rejects_nonregular_leaf_and_preserves_empty_help_resource() {
    let directory = tempfile::tempdir().unwrap();
    assert!(source_read::read_regular_source(directory.path()).is_err());
    let empty = directory.path().join("empty.bin");
    std::fs::write(&empty, b"").unwrap();
    assert!(source_read::read_regular_source(&empty).unwrap().is_empty());
    let bytes = (0..(64 * 1024 + 17))
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    std::fs::write(&empty, &bytes).unwrap();
    assert!(source_read::read_regular_source(&empty).unwrap() == bytes);
}

#[test]
fn source_reader_rejects_leaf_and_ancestor_links() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("target.bin");
    std::fs::write(&target, b"payload").unwrap();
    let link = directory.path().join("link.bin");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &link).unwrap();
    #[cfg(windows)]
    if let Err(error) = std::os::windows::fs::symlink_file(&target, &link) {
        if error.raw_os_error() == Some(1314) {
            eprintln!("SKIP filesystem link controls: Windows requires symlink privilege");
            return;
        }
        panic!("cannot create source link control: {error}");
    }
    assert!(source_read::read_regular_source(&link).is_err());
    let folder = directory.path().join("folder");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("nested.bin"), b"payload").unwrap();
    let folder_link = directory.path().join("folder-link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&folder, &folder_link).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&folder, &folder_link).unwrap();
    assert!(source_read::read_regular_source(&folder_link.join("nested.bin")).is_err());
}
