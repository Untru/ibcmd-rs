//! Shared native version contract for CLI and RPC consumers.
mod common;

use ibcmd_rs::native_export_status::{ChangeKind, ConfigDumpInfoSnapshot};
fn tiny() -> Vec<u8> {
    include_bytes!("fixtures/file-infobase/tiny-xml/ConfigDumpInfo.xml").to_vec()
}
#[test]
fn versions_are_observed_even_when_source_xml_was_not_changed() {
    let baseline = tiny();
    let old = ConfigDumpInfoSnapshot::parse(&baseline).unwrap();
    let text = String::from_utf8(baseline).unwrap();
    // No source-file input is involved: native import can restamp these rows
    // while retaining exactly the same Configuration/Language XML bytes.
    let changed = text
        .replace("a86111168911c14b90b4fc704b63ebe600000000", &"0".repeat(40))
        .replace("446fd6627b646645af71333b00c13f6f00000000", &"1".repeat(40));
    assert_ne!(changed, text);
    let current = ConfigDumpInfoSnapshot::parse(changed.as_bytes()).unwrap();
    let result = current.compare(&old);
    assert!(!result.full_required);
    assert_eq!(result.changes.len(), 2);
    assert!(
        result
            .changes
            .iter()
            .all(|entry| entry.kind == ChangeKind::Modified)
    );
    assert_eq!(result.changes[0].name, "Configuration.Configuration");
    assert_eq!(result.changes[1].name, "Language.English");
    assert!(
        result
            .changes
            .iter()
            .all(|entry| entry.reference_version.as_deref() != Some(entry.config_version.as_str()))
    );
}
#[test]
fn native_header_only_difference_is_not_a_version_change() {
    let bytes = tiny();
    let current = ConfigDumpInfoSnapshot::parse(&bytes).unwrap();
    let older = String::from_utf8(bytes)
        .unwrap()
        .replace("version=\"2.21\"", "version=\"2.20\"");
    let status = current.compare(&ConfigDumpInfoSnapshot::parse(older.as_bytes()).unwrap());
    assert!(!status.full_required);
    assert!(status.changes.is_empty());
    assert_eq!(status.text(false), "");
}
#[test]
fn rename_or_deleted_reference_requires_full_export() {
    let bytes = tiny();
    let current = ConfigDumpInfoSnapshot::parse(&bytes).unwrap();
    let renamed = String::from_utf8(bytes).unwrap().replace(
        "CommonModule.ФайловаяБазаПроверка\"",
        "CommonModule.PreviousName\"",
    );
    let status = current.compare(&ConfigDumpInfoSnapshot::parse(renamed.as_bytes()).unwrap());
    assert!(status.full_required);
    assert_eq!(status.text(false), "modified: all\n");
    assert_eq!(status.text(true), "M: all\n");
}
#[test]
fn malformed_inventory_is_rejected() {
    assert!(ConfigDumpInfoSnapshot::parse(b"<ConfigDumpInfo/>").is_err());
    assert!(
        ConfigDumpInfoSnapshot::parse(b"<ConfigDumpInfo><Anything/></ConfigDumpInfo>").is_err()
    );
    assert!(
        ConfigDumpInfoSnapshot::parse(
            b"<ConfigDumpInfo><ConfigVersions/><ConfigVersions/></ConfigDumpInfo>"
        )
        .is_err()
    );
    assert!(ConfigDumpInfoSnapshot::parse(b"not XML").is_err());
    assert!(ConfigDumpInfoSnapshot::parse(b"<ConfigDumpInfo><ConfigVersions><Metadata name=\"X\"/></ConfigVersions></ConfigDumpInfo>").is_err());
}

#[test]
fn inventory_reader_refuses_diagnostic_writes_before_reading_any_source() {
    use clap::Parser;
    use ibcmd_rs::cli::{Cli, Commands};
    let root = std::env::temp_dir().join(format!("inventory-no-write-{}", uuid::Uuid::new_v4()));
    let _scratch = crate::common::Scratch::adopt(&root);
    let journal = root.join("journal.json");
    let cli = Cli::try_parse_from([
        std::ffi::OsString::from("ibcmd-rs"),
        "mssql-dump-config".into(),
        "--rows-dir".into(),
        root.join("absent-rows").into_os_string(),
        "--runtime-journal".into(),
        journal.clone().into_os_string(),
        "--extract-metadata-xml".into(),
        "--extract-module-text".into(),
        "--no-binary-rows".into(),
        "-o".into(),
        root.join("out").into_os_string(),
    ])
    .unwrap();
    let Commands::MssqlDumpConfig(args) = cli.command else {
        panic!("dump args expected")
    };
    let error = ibcmd_rs::mssql_dump::read_config_dump_info(&args).unwrap_err();
    assert!(error.to_string().contains("runtime_journal"));
    assert!(!root.exists());
}

#[test]
fn owner_mapping_uses_canonical_descriptors_and_exposes_missing_coverage() {
    let snapshot = ConfigDumpInfoSnapshot::parse(&tiny()).unwrap();
    let owners = snapshot.owners();
    assert!(owners.unmapped_ids.is_empty());
    assert_eq!(owners.by_id.len(), snapshot.entries.len());
    let module = owners
        .by_id
        .get("6d1c2a0e-1cd0-4f00-9a00-000000000001.0")
        .unwrap();
    assert_eq!(module.object, "CommonModule.ФайловаяБазаПроверка");
    assert_eq!(module.root, module.object);
    assert_eq!(
        owners
            .by_id
            .get("afb733a6-eae8-418b-b73c-fb9eb581e093")
            .unwrap()
            .object,
        "Configuration"
    );
    let orphan=ConfigDumpInfoSnapshot::parse(b"<ConfigDumpInfo><ConfigVersions><Metadata name=\"FutureKind.X.Module\" id=\"00000000-0000-0000-0000-000000000001.0\" configVersion=\"v\"/></ConfigVersions></ConfigDumpInfo>").unwrap();
    assert_eq!(orphan.owners().unmapped_ids.len(), 1);
}

#[test]
#[ignore = "optional private read-only corpus: set IBCMD_RS_NATIVE_STATUS_PRIVATE_XML"]
fn private_inventory_owner_coverage_is_complete() {
    let path = std::env::var_os("IBCMD_RS_NATIVE_STATUS_PRIVATE_XML")
        .expect("explicit private fixture path");
    let bytes = std::fs::read(path).unwrap();
    let snapshot = ConfigDumpInfoSnapshot::parse(&bytes).unwrap();
    let owners = snapshot.owners();
    println!(
        "version_entries={} mapped={} unmapped={}",
        snapshot.entries.len(),
        owners.by_id.len(),
        owners.unmapped_ids.len()
    );
    assert!(
        owners.unmapped_ids.is_empty(),
        "unmapped count={}",
        owners.unmapped_ids.len()
    );
    assert_eq!(owners.by_id.len(), snapshot.entries.len());
}

#[test]
fn nested_owner_is_never_silently_collapsed_to_its_root() {
    let xml=b"<ConfigDumpInfo><ConfigVersions><Metadata name=\"Catalog.X\" id=\"00000000-0000-0000-0000-000000000001\" configVersion=\"v\"/><Metadata name=\"Catalog.X.FutureKind.Y\" id=\"00000000-0000-0000-0000-000000000002\" configVersion=\"v\"/><Metadata name=\"Catalog.X.Form.F.Form\" id=\"00000000-0000-0000-0000-000000000003.0\" configVersion=\"v\"/></ConfigVersions></ConfigDumpInfo>";
    let owners = ConfigDumpInfoSnapshot::parse(xml).unwrap().owners();
    assert_eq!(
        owners.unmapped_ids,
        vec!["00000000-0000-0000-0000-000000000002"]
    );
    // A recognized nested owner is retained even in an incomplete snapshot.
    let form = owners
        .by_id
        .get("00000000-0000-0000-0000-000000000003.0")
        .unwrap();
    assert_eq!(form.object, "Catalog.X.Form.F");
    assert_eq!(form.root, "Catalog.X");
}

#[test]
fn inline_command_module_preserves_native_owner_and_rolls_into_owning_xml() {
    let xml=b"<ConfigDumpInfo><ConfigVersions><Metadata name=\"Catalog.X\" id=\"00000000-0000-0000-0000-000000000001\" configVersion=\"v\"/><Metadata name=\"Catalog.X.Command.C.CommandModule\" id=\"00000000-0000-0000-0000-000000000002.0\" configVersion=\"v\"/></ConfigVersions></ConfigDumpInfo>";
    let owners = ConfigDumpInfoSnapshot::parse(xml).unwrap().owners();
    assert!(owners.unmapped_ids.is_empty());
    let command = owners
        .by_id
        .get("00000000-0000-0000-0000-000000000002.0")
        .unwrap();
    assert_eq!(command.object, "Catalog.X");
    assert_eq!(command.root, "Catalog.X");
    assert_eq!(command.native_object, "Catalog.X.Command.C");
}

#[test]
fn metadata_name_command_is_not_mistaken_for_an_inline_command_kind() {
    let xml=b"<ConfigDumpInfo><ConfigVersions><Metadata name=\"Catalog.Command\" id=\"00000000-0000-0000-0000-000000000001\" configVersion=\"v\"/><Metadata name=\"Catalog.Command.ObjectModule\" id=\"00000000-0000-0000-0000-000000000001.0\" configVersion=\"v\"/></ConfigVersions></ConfigDumpInfo>";
    let owners = ConfigDumpInfoSnapshot::parse(xml).unwrap().owners();
    assert!(owners.unmapped_ids.is_empty());
    assert!(
        owners
            .by_id
            .values()
            .all(|owner| owner.object == "Catalog.Command")
    );
}

#[test]
fn nonempty_malformed_stored_rows_never_become_an_empty_inventory() {
    use clap::Parser;
    use ibcmd_rs::cli::{Cli, Commands};
    let root = std::env::temp_dir().join(format!("inventory-bad-rows-{}", uuid::Uuid::new_v4()));
    let _scratch = crate::common::Scratch::adopt(&root);
    let rows = root.join("rows");
    std::fs::create_dir_all(&rows).unwrap();
    std::fs::write(rows.join("root__part0.bin"), [0xff, 0xff, 0xff]).unwrap();
    let out = root.join("out");
    let cli = Cli::try_parse_from([
        std::ffi::OsString::from("ibcmd-rs"),
        "mssql-dump-config".into(),
        "--rows-dir".into(),
        rows.into_os_string(),
        "--extract-metadata-xml".into(),
        "--extract-module-text".into(),
        "--no-binary-rows".into(),
        "-o".into(),
        out.clone().into_os_string(),
    ])
    .unwrap();
    let Commands::MssqlDumpConfig(args) = cli.command else {
        panic!("dump args expected")
    };
    assert!(ibcmd_rs::mssql_dump::read_config_dump_info(&args).is_err());
    assert!(!out.exists());
}

#[test]
fn inventory_reader_refuses_inflate_debug_directories_for_a_valid_source() {
    use clap::Parser;
    use ibcmd_rs::cli::{Cli, Commands};
    use std::io::Read;
    let root = std::env::temp_dir().join(format!("inventory-no-inflate-{}", uuid::Uuid::new_v4()));
    let _scratch = crate::common::Scratch::adopt(&root);
    let db = root.join("db");
    std::fs::create_dir_all(&db).unwrap();
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(
        std::fs::File::open(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/file-infobase/tiny.1CD.gz"),
        )
        .unwrap(),
    )
    .read_to_end(&mut bytes)
    .unwrap();
    std::fs::write(db.join("1Cv8.1CD"), bytes).unwrap();
    let out = root.join("out");
    let cli = Cli::try_parse_from([
        std::ffi::OsString::from("ibcmd-rs"),
        "mssql-dump-config".into(),
        "--file-infobase".into(),
        db.into_os_string(),
        "--inflate".into(),
        "--extract-metadata-xml".into(),
        "--extract-module-text".into(),
        "--no-binary-rows".into(),
        "-o".into(),
        out.clone().into_os_string(),
    ])
    .unwrap();
    let Commands::MssqlDumpConfig(args) = cli.command else {
        panic!("dump args expected")
    };
    let error = ibcmd_rs::mssql_dump::read_config_dump_info(&args).unwrap_err();
    assert!(error.to_string().contains("inflate"));
    assert!(!out.exists());
}
