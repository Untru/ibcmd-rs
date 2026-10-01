use formats_xml::{
    configuration::{ConfigDialect, decode_share_types, emit_share_types},
    read::parse,
};
use morph1c_core::engine::Decoded;
use morph1c_core::{
    ir::{MetadataObject, PropertyValue},
    spec::metadata::{common_command as cc, configuration as cfg},
    version::{FormatVersion, with_roundtrip_target},
};
use morph1c_pipeline::{Format, registry::FormatRegistry};

fn read(format: Format, kind: &str, bytes: &[u8]) -> Result<MetadataObject, String> {
    (FormatRegistry::for_format(format)
        .unwrap()
        .get(kind)
        .unwrap()
        .read)(bytes)
}
fn write(format: Format, obj: &MetadataObject, version: FormatVersion) -> Vec<u8> {
    with_roundtrip_target(version, || {
        (FormatRegistry::for_format(format)
            .unwrap()
            .get(obj.kind.as_str())
            .unwrap()
            .write)(obj)
    })
    .unwrap()
}
fn value(bytes: &[u8], dialect: ConfigDialect) -> PropertyValue {
    match decode_share_types(dialect, &parse(bytes).unwrap().root) {
        Decoded::Present(value) => value,
        Decoded::Error(e) => panic!("decode: {e}"),
        _ => panic!("missing share types"),
    }
}
#[test]
fn ordered_share_records_keep_all_fields_and_reject_unknown_shapes() {
    let xml = br#"<root><allowedIncomingShareRequestTypes><ext>txt</ext></allowedIncomingShareRequestTypes><allowedIncomingShareRequestTypes><mime>a&amp;b</mime><uti>unicode</uti><ext>abc</ext><processingVariant>Edit</processingVariant><isCustom>true</isCustom></allowedIncomingShareRequestTypes></root>"#;
    let val = value(xml, ConfigDialect::Edt);
    let PropertyValue::List(rows) = &val else {
        panic!()
    };
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0],
        PropertyValue::List(
            ["", "", "txt", "0", "false"]
                .map(|s| PropertyValue::Str(s.into()))
                .to_vec()
        )
    );
    assert_eq!(
        rows[1],
        PropertyValue::List(
            ["a&b", "unicode", "abc", "1", "true"]
                .map(|s| PropertyValue::Str(s.into()))
                .to_vec()
        )
    );
    for dialect in [ConfigDialect::Edt, ConfigDialect::Designer] {
        let emitted = emit_share_types(dialect, &val).unwrap();
        assert_eq!(
            emitted.len(),
            if dialect == ConfigDialect::Edt { 2 } else { 1 }
        );
    }
    for bad in [
        "<mime>x</mime><mime>y</mime>",
        "<unknown>x</unknown>",
        "<processingVariant>Execute</processingVariant>",
        "<isCustom>1</isCustom>",
        "<mime extra=\"true\">x</mime>",
        "<mime><extra/></mime>",
        "<x:mime xmlns:x=\"urn:bad\">x</x:mime>",
    ] {
        let src = format!(
            "<root><allowedIncomingShareRequestTypes>{bad}</allowedIncomingShareRequestTypes></root>"
        );
        assert!(matches!(
            decode_share_types(ConfigDialect::Edt, &parse(src.as_bytes()).unwrap().root),
            Decoded::Error(_)
        ));
    }
    let bad = PropertyValue::List(vec![PropertyValue::List(vec![PropertyValue::Str(
        "x".into(),
    )])]);
    assert!(emit_share_types(ConfigDialect::Edt, &bad).is_err());
    let empty = value(
        b"<root><allowedIncomingShareRequestTypes/></root>",
        ConfigDialect::Edt,
    );
    let emitted = emit_share_types(ConfigDialect::Edt, &empty).unwrap();
    assert_eq!(emitted.len(), 1);
    assert!(emitted[0].self_closing);
    assert!(emitted[0].children.is_empty());
}

#[test]
fn common_command_help_pages_resources_and_share_edits_survive_without_provenance() {
    use morph1c_core::ir::{HelpPage, HelpResource, ObjectKind, Token, Uuid};
    use morph1c_pipeline::{ConvertOptions, read_config, write_config};
    let opts = ConvertOptions::default().with_target_version(FormatVersion::new(2, 21));
    let mut config = read_config(
        Format::Designer,
        std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &opts,
    )
    .unwrap()
    .0;
    let share = value(br#"<root><allowedIncomingShareRequestTypes><mime>application/example</mime><processingVariant>Edit</processingVariant><isCustom>true</isCustom></allowedIncomingShareRequestTypes></root>"#, ConfigDialect::Edt);
    let root = config
        .objects
        .iter_mut()
        .find(|o| o.kind.as_str() == "Configuration")
        .unwrap();
    root.properties
        .push((cfg::F_ALLOWED_INCOMING_SHARE_TYPES, share.clone()));
    let mut command = MetadataObject::new(
        ObjectKind::new("CommonCommand"),
        "HelpExample",
        Uuid([32; 16]),
    );
    command.properties.push((
        cc::F_GROUP,
        PropertyValue::Enum(Token::new("NavigationPanelOrdinary")),
    ));
    command
        .properties
        .push((cc::F_HELP, PropertyValue::Bool(true)));
    command.help.push(HelpPage {
        lang: "ru".into(),
        body: "<html>complete help\nsecond line</html>".into(),
    });
    command.help_resources.push(HelpResource {
        rel_path: "image.bin".into(),
        bytes: vec![0, 255, 13, 10],
    });
    config.objects.push(command);
    for missing_marker in [false, true] {
        let out = tempfile::tempdir().unwrap();
        write_config(Format::Edt, &config, out.path()).unwrap();
        if missing_marker {
            let path = out
                .path()
                .join("CommonCommands/HelpExample/HelpExample.mdo");
            let text = std::fs::read_to_string(&path).unwrap();
            let start = text.find("  <help>").unwrap();
            let end = text[start..].find("</help>").unwrap() + start + "</help>".len();
            std::fs::write(path, format!("{}{}", &text[..start], &text[end..])).unwrap();
        } else {
            std::fs::remove_file(out.path().join("CommonCommands/HelpExample/Help/ru.html"))
                .unwrap();
            std::fs::remove_file(
                out.path()
                    .join("CommonCommands/HelpExample/Help/_files/image.bin"),
            )
            .unwrap();
            std::fs::remove_dir(out.path().join("CommonCommands/HelpExample/Help/_files")).unwrap();
            std::fs::remove_dir(out.path().join("CommonCommands/HelpExample/Help")).unwrap();
        }
        let error = read_config(Format::Edt, out.path(), &opts)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("help marker and owned help pages disagree"),
            "{error}"
        );
    }
    for format in [Format::Edt, Format::Designer, Format::Edt] {
        let out = tempfile::tempdir().unwrap();
        write_config(format, &config, out.path()).unwrap();
        config = read_config(format, out.path(), &opts).unwrap().0;
        let root = config
            .objects
            .iter()
            .find(|o| o.kind.as_str() == "Configuration")
            .unwrap();
        assert_eq!(
            root.properties
                .iter()
                .find(|(id, _)| *id == cfg::F_ALLOWED_INCOMING_SHARE_TYPES)
                .unwrap()
                .1,
            share
        );
        let command = config
            .objects
            .iter()
            .find(|o| o.kind.as_str() == "CommonCommand")
            .unwrap();
        assert_eq!(command.help.len(), 1);
        assert_eq!(
            command.help[0].body,
            "<html>complete help\nsecond line</html>"
        );
        assert_eq!(command.help_resources.len(), 1);
        assert_eq!(command.help_resources[0].bytes, vec![0, 255, 13, 10]);
        assert_eq!(
            command
                .properties
                .iter()
                .find(|(id, _)| *id == cc::F_HELP)
                .unwrap()
                .1,
            PropertyValue::Bool(true)
        );
    }
    let root = config
        .objects
        .iter_mut()
        .find(|o| o.kind.as_str() == "Configuration")
        .unwrap();
    let PropertyValue::List(rows) = &mut root
        .properties
        .iter_mut()
        .find(|(id, _)| *id == cfg::F_ALLOWED_INCOMING_SHARE_TYPES)
        .unwrap()
        .1
    else {
        panic!()
    };
    let PropertyValue::List(fields) = &mut rows[0] else {
        panic!()
    };
    fields[0] = PropertyValue::Str("application/edited".into());
    let out = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &config, out.path()).unwrap();
    let changed = read_config(Format::Designer, out.path(), &opts).unwrap().0;
    assert_ne!(
        changed
            .objects
            .iter()
            .find(|o| o.kind.as_str() == "Configuration")
            .unwrap()
            .properties
            .iter()
            .find(|(id, _)| *id == cfg::F_ALLOWED_INCOMING_SHARE_TYPES)
            .unwrap()
            .1,
        share
    );
}

#[test]
#[ignore = "requires immutable authentic UH83/UH85 corpus at IBCMD_EDT_LAB"]
fn genuine_configuration_and_common_command_census_both_versions() {
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let command = "СверкаДокументовСКонтрагентами";
    for (run, minor) in [("oracle-uha83-r1", 20), ("oracle-uha85-r2", 21)] {
        let version = FormatVersion::new(2, minor);
        let edt = lab
            .join(run)
            .join("authentic-workspace/OracleConfiguration/src");
        let native = lab.join(run).join("edt-native-xml");
        let eb = std::fs::read(edt.join("Configuration/Configuration.mdo")).unwrap();
        let nb = std::fs::read(native.join("Configuration.xml")).unwrap();
        let e = read(Format::Edt, "Configuration", &eb).unwrap();
        let n = read(Format::Designer, "Configuration", &nb).unwrap();
        let ev = e
            .properties
            .iter()
            .find(|(id, _)| *id == cfg::F_ALLOWED_INCOMING_SHARE_TYPES)
            .unwrap();
        let nv = n
            .properties
            .iter()
            .find(|(id, _)| *id == cfg::F_ALLOWED_INCOMING_SHARE_TYPES)
            .unwrap();
        assert_eq!(ev, nv);
        assert!(matches!(&ev.1, PropertyValue::List(rows) if rows.len() == 4));
        assert_eq!(write(Format::Edt, &e, version), eb);
        assert_eq!(write(Format::Designer, &n, version), nb);
        let from_edt = write(Format::Designer, &e, version);
        let projected = value(&from_edt, ConfigDialect::Designer);
        assert_eq!(projected, ev.1);
        let from_native = write(Format::Edt, &n, version);
        assert_eq!(value(&from_native, ConfigDialect::Edt), ev.1);
        // Independently captured installed serializer bytes for this complete property.
        let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
        let block = |bytes: &[u8]| {
            let s = text(bytes);
            let i = s.find("<AllowedIncomingShareRequestTypes>").unwrap();
            let j = s[i..].find("</AllowedIncomingShareRequestTypes>").unwrap()
                + i
                + "</AllowedIncomingShareRequestTypes>".len();
            s[i..j].to_string()
        };
        assert_eq!(block(&from_edt), block(&nb));
        let cb =
            std::fs::read(edt.join(format!("CommonCommands/{command}/{command}.mdo"))).unwrap();
        let c = read(Format::Edt, "CommonCommand", &cb).unwrap();
        assert_eq!(
            c.properties
                .iter()
                .find(|(id, _)| *id == cc::F_HELP)
                .unwrap()
                .1,
            PropertyValue::Bool(true)
        );
        assert_eq!(write(Format::Edt, &c, version), cb);
        let bad = text(&cb).replace("<lang>ru</lang>", "<lang>ru</lang><unknown/>");
        assert!(read(Format::Edt, "CommonCommand", bad.as_bytes()).is_err());
        let mut count = 0;
        for format in [Format::Edt, Format::Designer] {
            let root = if format == Format::Edt { &edt } else { &native };
            for entry in std::fs::read_dir(root.join("CommonCommands")).unwrap() {
                let entry = entry.unwrap();
                let file = if format == Format::Edt {
                    entry.path().join(entry.file_name()).with_extension("mdo")
                } else {
                    entry.path()
                };
                if file
                    .extension()
                    .is_none_or(|e| e != if format == Format::Edt { "mdo" } else { "xml" })
                {
                    continue;
                }
                let bytes = std::fs::read(&file).unwrap();
                let obj = read(format, "CommonCommand", &bytes)
                    .unwrap_or_else(|e| panic!("{}: {e}", file.display()));
                assert_eq!(write(format, &obj, version), bytes, "{}", file.display());
                count += 1;
            }
        }
        assert_eq!(count, 1072);
    }
    for (run, minor) in [("oracle-bsp83-r1", 20), ("oracle-bsp85-r3", 21)] {
        let version = FormatVersion::new(2, minor);
        let root = lab.join(run);
        for (format, file) in [
            (
                Format::Edt,
                root.join(
                    "authentic-workspace/OracleConfiguration/src/Configuration/Configuration.mdo",
                ),
            ),
            (
                Format::Designer,
                root.join("edt-native-xml/Configuration.xml"),
            ),
        ] {
            let bytes = std::fs::read(&file).unwrap();
            let obj = read(format, "Configuration", &bytes).unwrap();
            assert_eq!(write(format, &obj, version), bytes, "{}", file.display());
        }
    }
}
