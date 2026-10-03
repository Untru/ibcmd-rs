//! Windows counts physical filename limits differently from UTF-8 byte length.
#![cfg(windows)]

use ibcmd_edt::{ConversionOptions, read_directory_project, read_directory_source};
use morph1c_core::ir::{HelpPage, HelpResource};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};

#[test]
fn unicode_help_resource_above_255_utf8_bytes_survives_both_directory_directions() {
    let fixture = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/subsystem-ci/src"
    ));
    let mut config = read_config(Format::Designer, fixture, &ConvertOptions::default())
        .unwrap()
        .0;
    let owner = config
        .objects
        .iter_mut()
        .find(|object| object.kind.as_str() == "Catalog")
        .unwrap();
    owner.help = vec![HelpPage {
        lang: "ru".into(),
        body: "<html>resource</html>".into(),
    }];
    let filename = format!("{}.bin", "Ж".repeat(128));
    assert!(filename.len() > 255);
    assert!(filename.encode_utf16().count() < 255);
    let payload = b"\x00preserve complete Unicode resource\xff";
    owner.help_resources = vec![HelpResource {
        rel_path: filename.clone(),
        bytes: payload.to_vec(),
    }];
    let name = owner.name.clone();
    let temp = tempfile::tempdir().unwrap();
    let native = temp.path().join("native");
    write_config(Format::Designer, &config, &native).unwrap();
    let options = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    let project = temp.path().join("project");
    read_directory_source(&native)
        .unwrap()
        .xml_to_edt(&options)
        .unwrap()
        .publish_new(&project)
        .unwrap();
    let edt_path = format!("src/Catalogs/{name}/Help/_files/{filename}");
    assert_eq!(std::fs::read(project.join(edt_path)).unwrap(), payload);
    let restored = temp.path().join("restored");
    read_directory_project(&project)
        .unwrap()
        .edt_to_xml(&options)
        .unwrap()
        .publish_new(&restored)
        .unwrap();
    let native_path = format!("Catalogs/{name}/Ext/Help/_files/{filename}");
    assert_eq!(std::fs::read(restored.join(&native_path)).unwrap(), payload);
    // Also require actual typed return, independent of unchanged-source replay.
    std::fs::remove_dir_all(project.join(".ibcmd-provenance")).unwrap();
    let stripped = temp.path().join("stripped");
    read_directory_project(&project)
        .unwrap()
        .edt_to_xml(&options)
        .unwrap()
        .publish_new(&stripped)
        .unwrap();
    assert_eq!(std::fs::read(stripped.join(native_path)).unwrap(), payload);
}
