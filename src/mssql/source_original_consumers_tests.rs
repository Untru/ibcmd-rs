//! Production-bound original-reader controls; no database or native actor.
use super::*;
use std::sync::Arc;

pub(crate) struct Fixture {
    pub(crate) top: PathBuf,
    pub(crate) root: PathBuf,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        let top =
            std::env::temp_dir().join(format!("ibcmd-original-consumers-{}", uuid::Uuid::new_v4()));
        let root = top.join("source");
        fs::create_dir_all(&root).unwrap();
        Self { top, root }
    }

    pub(crate) fn write(&self, relative: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        let path = self.root.join(relative);
        assert!(path.starts_with(&self.root));
        assert!(
            !Path::new(relative)
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        );
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }

    pub(crate) fn context(&self) -> MetadataSourceContext {
        MetadataSourceContext::with_original_source(Arc::new(
            crate::mssql_source_change::HeldSourceRoot::open_compiler_operation(&self.root)
                .unwrap(),
        ))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let name = self.top.file_name().unwrap().to_string_lossy();
        let suffix = name.strip_prefix("ibcmd-original-consumers-").unwrap();
        assert!(uuid::Uuid::parse_str(suffix).is_ok());
        assert_eq!(self.top.parent(), Some(std::env::temp_dir().as_path()));
        // Only the independently created, UUID-named fixture is removed.
        let _ = fs::remove_dir_all(&self.top);
    }
}

pub(crate) fn metadata(kind: &str, name: &str, uuid: &str, version: &str, extra: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" version=\"{version}\"><{kind} uuid=\"{uuid}\"><Properties><Name>{name}</Name>{extra}</Properties></{kind}></MetaDataObject>"
    )
}

pub(crate) const OBJECT: &str = "12345678-1234-4234-8234-123456789abc";

#[test]
fn original_scanner_reproduces_complete_legacy_members_kinds_and_hashes() {
    let f = Fixture::new();
    f.write(
        "Configuration.xml",
        metadata("Configuration", "C", OBJECT, "2.20", ""),
    );
    f.write(
        "Catalogs/A.xml",
        metadata("Catalog", "A", OBJECT, "2.20", ""),
    );
    f.write(
        "CommonModules/M/Ext/Module.bsl",
        "Procedure P()\nEndProcedure\n",
    );
    f.write("Templates/T/Ext/Template.bin", vec![71u8; 196_633]);
    let old = crate::source::scan_sources(&f.root).unwrap();
    let context = f.context();
    let current = context.scan_sources(&[]).unwrap();
    assert_eq!(current.files.len(), 4);
    for (expected, actual) in old.files.iter().zip(&current.files) {
        assert_eq!(actual.path, expected.path);
        assert_eq!(actual.size_bytes, expected.size_bytes);
        assert_eq!(actual.sha256, expected.sha256);
        assert_eq!(actual.kind, expected.kind);
        assert_eq!(actual.xml_root, expected.xml_root);
        assert_eq!(actual.object_hint, expected.object_hint);
    }
    assert_eq!(
        context
            .scan_sources(&["Catalogs/A".into()])
            .unwrap()
            .files
            .len(),
        1
    );
    assert!(context.scan_sources(&["catalogs/A".into()]).is_err());
    assert!(context.scan_sources(&["../outside".into()]).is_err());
    context.require_original_unchanged().unwrap();
}

#[test]
fn original_selection_retains_exact_owner_and_refuses_missing_or_alias_inputs() {
    let f = Fixture::new();
    f.write(
        "CommonModules/M.xml",
        metadata("CommonModule", "M", OBJECT, "2.20", ""),
    );
    f.write(
        "CommonModules/M/Ext/Module.bsl",
        "Procedure P()\nEndProcedure\n",
    );
    let context = f.context();
    let selected = files_stage::select_with_source(
        &f.root,
        &["CommonModules/M/Ext/Module.bsl".into()],
        Some(&context),
    )
    .unwrap();
    assert_eq!(selected.files(), ["CommonModules/M/Ext/Module.bsl"]);
    assert_eq!(selected.owners(), ["CommonModules/M.xml"]);
    for invalid in [
        "CommonModules/M/Ext/Missing.bsl",
        "commonmodules/M/Ext/Module.bsl",
        "../outside.bsl",
    ] {
        assert!(
            files_stage::select_with_source(&f.root, &[invalid.into()], Some(&context)).is_err(),
            "{invalid}"
        );
    }
    assert!(context.read_source(&f.top.join("outside.xml")).is_err());
    assert!(context.require_original_reads().is_err());
}

#[test]
fn original_version_validation_matches_both_profiles_and_refuses_wrong_or_missing() {
    for (version, expected, other) in [
        (
            "2.20",
            InfobaseConfigSourceVersion::V2_20,
            InfobaseConfigSourceVersion::V2_21,
        ),
        (
            "2.21",
            InfobaseConfigSourceVersion::V2_21,
            InfobaseConfigSourceVersion::V2_20,
        ),
    ] {
        let f = Fixture::new();
        let path = f.write(
            "CommonModules/M.xml",
            metadata("CommonModule", "M", OBJECT, version, ""),
        );
        let context = f.context();
        validate_selected_source_versions_with_source(
            std::slice::from_ref(&path),
            expected,
            Some(&context),
        )
        .unwrap();
        assert!(
            validate_selected_source_versions_with_source(&[path], other, Some(&context)).is_err()
        );
        assert!(
            validate_selected_source_versions_with_source(
                &[f.root.join("Missing.xml")],
                expected,
                Some(&context)
            )
            .is_err()
        );
    }
}

#[test]
fn original_layout_uses_each_current_configuration_and_parse_failures_are_errors() {
    for (compatibility, expected) in [("Version8_3_27", false), ("Version8_5_1", true)] {
        let f = Fixture::new();
        f.write(
            "Configuration.xml",
            metadata(
                "Configuration",
                "C",
                OBJECT,
                "2.21",
                &format!("<CompatibilityMode>{compatibility}</CompatibilityMode>"),
            ),
        );
        let context = f.context();
        assert_eq!(context.stores_layout_8_5_1().unwrap(), expected);
        assert_eq!(context.stores_layout_8_5_1().unwrap(), expected);
        assert_eq!(context.tree_version().unwrap().as_deref(), Some("2.21"));
    }
    let f = Fixture::new();
    f.write("Configuration.xml", b"<MetaDataObject><Configuration");
    let context = f.context();
    assert!(context.stores_layout_8_5_1().is_err());
    assert!(context.tree_version().is_err());
}

#[test]
fn original_role_resolver_keeps_full_uuid_and_never_masks_invalid_owner() {
    use crate::compiler::bodies::role_rights_writer::RoleRightsSource;
    for version in ["2.20", "2.21"] {
        let f = Fixture::new();
        f.write(
            "Catalogs/A.xml",
            metadata("Catalog", "A", OBJECT, version, ""),
        );
        f.write("Catalogs/B.xml", b"<MetaDataObject><Catalog");
        let context = f.context();
        assert_eq!(
            context
                .role_rights_source()
                .metadata_object_uuid("Catalog.A")
                .unwrap(),
            OBJECT
        );
        let rights = format!(
            "<Rights xmlns=\"http://v8.1c.ru/8.2/roles\" version=\"{version}\"><setForNewObjects>false</setForNewObjects><setForAttributesByDefault>false</setForAttributesByDefault><independentRightsOfChildObjects>false</independentRightsOfChildObjects><object><name>Catalog.A</name><right><name>Read</name><value>true</value></right></object></Rights>"
        );
        let old_source = MetadataSourceContext::new(f.root.clone());
        let expected =
            crate::module_blob::pack_role_rights_blob_base_free(rights.as_bytes(), &old_source)
                .unwrap();
        let actual =
            crate::module_blob::pack_role_rights_blob_base_free(rights.as_bytes(), &context)
                .unwrap();
        assert_eq!(actual.blob, expected.blob);
        assert_eq!(actual.plain_bytes, expected.plain_bytes);
        assert!(
            context
                .role_rights_source()
                .metadata_object_uuid("Catalog.B")
                .is_err()
        );
        assert!(
            context
                .role_rights_source()
                .metadata_object_uuid("Catalog.Missing")
                .is_err()
        );
        assert_eq!(
            context
                .role_rights_source()
                .metadata_object_uuid("Catalog.A")
                .unwrap(),
            OBJECT
        );
        assert!(context.require_original_reads().is_err());
    }
}

#[test]
fn original_descriptor_compatibility_and_predefined_reference_keep_current_values() {
    use crate::metadata_model::DescriptorContext;
    use crate::metadata_model::objects_parts::{Compat, compatibility};
    use crate::metadata_model::types::predefined_item_id;
    for version in ["2.20", "2.21"] {
        let f = Fixture::new();
        f.write(
            "Configuration.xml",
            metadata(
                "Configuration",
                "C",
                "42345678-1234-4234-8234-123456789abc",
                version,
                "<CompatibilityMode>Version8_3_27</CompatibilityMode>",
            ),
        );
        let catalog_xml = metadata("Catalog", "A", OBJECT, version, "");
        let path = f.write("Catalogs/A.xml", &catalog_xml);
        let predefined = "<PredefinedData><Item id=\"52345678-1234-4234-8234-123456789abc\"><Name>P</Name><ChildItems><Item id=\"62345678-1234-4234-8234-123456789abc\"><Name>Nested</Name></Item></ChildItems></Item></PredefinedData>";
        f.write("Catalogs/A/Ext/Predefined.xml", predefined);
        let old = DescriptorContext::new(&f.root, version).unwrap();
        assert_eq!(compatibility(&old), Compat(8, 3, 27));
        assert_eq!(
            predefined_item_id("Catalog", "A", "P", &old).unwrap(),
            "52345678-1234-4234-8234-123456789abc"
        );
        let source = f.context();
        let original = source
            .original_source()
            .unwrap()
            .read_original_path(&path)
            .unwrap();
        let mut current =
            DescriptorContext::with_files(&f.root, version, &[(path.clone(), original)]).unwrap();
        current.source = source.clone();
        assert_eq!(compatibility(&current), compatibility(&old));
        for item in ["P", "Nested"] {
            assert_eq!(
                predefined_item_id("Catalog", "A", item, &current).unwrap(),
                predefined_item_id("Catalog", "A", item, &old).unwrap()
            );
        }
        assert!(predefined_item_id("Catalog", "A", "Missing", &current).is_err());
        source.require_original_unchanged().unwrap();
        drop(current);
        drop(source);
        // The legacy path memo is deliberately left populated. A second
        // original owner must derive its current source facts independently.
        f.write(
            "Configuration.xml",
            metadata(
                "Configuration",
                "C",
                "42345678-1234-4234-8234-123456789abc",
                version,
                "<CompatibilityMode>Version8_5</CompatibilityMode>",
            ),
        );
        f.write(
            "Catalogs/A/Ext/Predefined.xml",
            predefined.replace(
                "52345678-1234-4234-8234-123456789abc",
                "72345678-1234-4234-8234-123456789abc",
            ),
        );
        let source = f.context();
        let original = source
            .original_source()
            .unwrap()
            .read_original_path(&path)
            .unwrap();
        let mut current =
            DescriptorContext::with_files(&f.root, version, &[(path, original)]).unwrap();
        current.source = source.clone();
        assert_eq!(compatibility(&current), Compat(8, 5, 0));
        assert_eq!(
            predefined_item_id("Catalog", "A", "P", &current).unwrap(),
            "72345678-1234-4234-8234-123456789abc"
        );
        source.require_original_unchanged().unwrap();
    }
}

#[test]
fn original_descriptor_optional_absence_and_invalid_configuration_are_distinct() {
    use crate::metadata_model::DescriptorContext;
    use crate::metadata_model::objects_parts::{Compat, compatibility};
    use crate::metadata_model::types::predefined_item_id;
    for version in ["2.20", "2.21"] {
        let f = Fixture::new();
        let path = f.write(
            "Catalogs/A.xml",
            metadata("Catalog", "A", OBJECT, version, ""),
        );
        let source = f.context();
        let original = source
            .original_source()
            .unwrap()
            .read_original_path(&path)
            .unwrap();
        let mut current =
            DescriptorContext::with_files(&f.root, version, &[(path, original)]).unwrap();
        current.source = source.clone();
        let [major, minor, patch] = current.platform().release();
        assert_eq!(compatibility(&current), Compat(major, minor, patch));
        assert!(predefined_item_id("Catalog", "A", "Missing", &current).is_err());
        source.require_original_reads().unwrap();
        drop(current);
        drop(source);
        f.write("Configuration.xml", b"<MetaDataObject><Configuration");
        let source = f.context();
        let path = f.root.join("Catalogs/A.xml");
        let original = source
            .original_source()
            .unwrap()
            .read_original_path(&path)
            .unwrap();
        let mut current =
            DescriptorContext::with_files(&f.root, version, &[(path, original)]).unwrap();
        current.source = source.clone();
        // The legacy return type is a value, so only the sticky success gate
        // can authorize it. A parser failure never authorizes the fallback.
        let _ = compatibility(&current);
        assert!(source.require_original_reads().is_err());
    }
}

#[test]
fn original_readiness_matches_actual_legacy_report_and_refuses_missing_original() {
    for version in ["2.20", "2.21"] {
        let f = Fixture::new();
        let xml = metadata(
            "CommonTemplate",
            "T",
            OBJECT,
            version,
            "<TemplateType>BinaryData</TemplateType>",
        );
        let path = f.write("CommonTemplates/T.xml", xml);
        f.write("CommonTemplates/T/Ext/Template.bin", b"full current body");
        let context = f.context();
        let old =
            source_bootstrap_readiness_report(&f.root, std::slice::from_ref(&path), &[]).unwrap();
        let current = source_bootstrap_readiness_report_with_source(
            &f.root,
            std::slice::from_ref(&path),
            &[],
            &context,
        )
        .unwrap();
        assert_eq!(current, old);
        assert_eq!(current.selected_objects, 1);
        assert!(
            current
                .rows
                .iter()
                .any(|r| r.config_file_name == format!("{OBJECT}.0"))
        );
        assert!(
            source_bootstrap_readiness_report_with_source(
                &f.root,
                &[f.root.join("Missing.xml")],
                &[],
                &context
            )
            .is_err()
        );
    }
}

#[test]
fn original_binary_template_compiler_preserves_complete_multichunk_payload() {
    for version in ["2.20", "2.21"] {
        let f = Fixture::new();
        let xml = metadata(
            "CommonTemplate",
            "T",
            OBJECT,
            version,
            "<TemplateType>BinaryData</TemplateType>",
        );
        let path = f.write("CommonTemplates/T.xml", &xml);
        let bytes = (0..196_673).map(|i| (i % 251) as u8).collect::<Vec<_>>();
        f.write("CommonTemplates/T/Ext/Template.bin", &bytes);
        let context = f.context();
        let props = parse_simple_metadata_xml_properties(xml.as_bytes()).unwrap();
        let axes = mssql_compile_axes_from_metadata_xml(xml.as_bytes()).unwrap();
        let expected = prepare_binary_template_body_row(
            &SqlExec::detached("no SQL"),
            "Db",
            &path,
            &props,
            TemplateKind::BinaryData,
            &axes,
        )
        .unwrap();
        let actual = prepare_binary_template_body_row_with_source(
            &SqlExec::detached("no SQL"),
            "Db",
            &path,
            &props,
            TemplateKind::BinaryData,
            &axes,
            Some(&context),
        )
        .unwrap();
        assert_eq!(actual.len(), 1);
        assert_eq!(actual[0].body_id, format!("{OBJECT}.0"));
        assert_eq!(actual[0].blob, expected[0].blob);
        assert_eq!(actual[0].blob_sha256, hex_sha256(&actual[0].blob));
        context.require_original_unchanged().unwrap();
    }
}

#[test]
fn original_chunk_copy_preserves_empty_directories_and_all_bytes_without_replacing_destination() {
    let f = Fixture::new();
    f.write(
        "Configuration.xml",
        metadata("Configuration", "C", OBJECT, "2.20", ""),
    );
    let bytes = (0..262_163).map(|i| (i % 253) as u8).collect::<Vec<_>>();
    f.write("Nested/Payload.bin", &bytes);
    fs::create_dir_all(f.root.join("Nested/Empty")).unwrap();
    let context = f.context();
    let destination = f.top.join("projection");
    context
        .original_source()
        .unwrap()
        .copy_to_new_projection(&destination)
        .unwrap();
    assert_eq!(
        fs::read(destination.join("Nested/Payload.bin")).unwrap(),
        bytes
    );
    assert!(destination.join("Nested/Empty").is_dir());
    assert_eq!(
        crate::source::scan_sources(&destination)
            .unwrap()
            .files
            .iter()
            .map(|m| (&m.path, &m.sha256))
            .collect::<Vec<_>>(),
        crate::source::scan_sources(&f.root)
            .unwrap()
            .files
            .iter()
            .map(|m| (&m.path, &m.sha256))
            .collect::<Vec<_>>()
    );
    assert!(
        context
            .original_source()
            .unwrap()
            .copy_to_new_projection(&destination)
            .is_err()
    );
    assert_eq!(
        fs::read(destination.join("Nested/Payload.bin")).unwrap(),
        bytes
    );
    context.require_original_unchanged().unwrap();
}

#[test]
fn original_drift_before_copy_creates_no_projection() {
    let f = Fixture::new();
    f.write("A.bin", b"original");
    let context = f.context();
    f.write("New.bin", b"drift");
    let destination = f.top.join("projection");
    assert!(
        context
            .original_source()
            .unwrap()
            .copy_to_new_projection(&destination)
            .is_err()
    );
    assert!(!destination.exists());
}

#[test]
fn original_prewrite_gate_runs_before_builder_and_keeps_existing_prefix_on_late_drift() {
    let f = Fixture::new();
    f.write("A.bin", b"original");
    let context = f.context();
    let first = f.top.join("scripts/first.sql");
    write_original_source_script(&context, &first, || "first accepted script".into()).unwrap();
    assert_eq!(fs::read(&first).unwrap(), b"first accepted script");
    f.write("New.bin", b"drift");
    let mut invoked = false;
    let second = f.top.join("never-created/second.sql");
    assert!(
        write_original_source_script(&context, &second, || {
            invoked = true;
            "late script".into()
        })
        .is_err()
    );
    assert!(!invoked);
    assert!(!second.parent().unwrap().exists());
    assert_eq!(fs::read(&first).unwrap(), b"first accepted script");
}

#[test]
fn original_prewrite_rechecks_fallible_builder_before_script_write() {
    let f = Fixture::new();
    f.write("A.bin", b"original");
    let context = f.context();
    let output = f.top.join("scripts/never-written.sql");
    let mut invoked = false;
    assert!(
        write_original_source_script(&context, &output, || {
            invoked = true;
            f.write("New.bin", b"new member during build");
            "must not be written".into()
        })
        .is_err()
    );
    assert!(invoked);
    // Directory creation already occurred before the builder. The result is
    // an addressed failure, not a claim of rolled-back filesystem effects.
    assert!(output.parent().unwrap().is_dir());
    assert!(!output.exists());
}

pub(crate) fn stage_args(f: &Fixture) -> MssqlStageSourceObjectsArgs {
    MssqlStageSourceObjectsArgs {
        server: "unused".into(),
        sql_user: None,
        sql_pwd: None,
        sql_pwd_env: "UNUSED".into(),
        database: "LabOriginal".into(),
        source_root: f.root.clone(),
        sqlcmd: None,
        replace_config_save: true,
        allow_non_lab: false,
        batch_size: None,
        platform: None,
        source_version: None,
        path_prefix: Vec::new(),
        files: Vec::new(),
        script_output: Some(f.top.join("bulk/stage.sql")),
        script_only: true,
        bulk: true,
        per_row: false,
        bcp_executable: None,
        base_free: false,
        verify: false,
    }
}

#[test]
fn original_actual_bulk_writer_produces_scripts_then_refuses_drift_before_any_output() {
    let f = Fixture::new();
    f.write("A.bin", b"original");
    let context = f.context();
    let args = stage_args(&f);
    let scripts = stage_source_rows_bulk(
        &args,
        &SqlExec::detached("no SQL allowed"),
        &[],
        &[],
        b"versions",
        &StageAdditions::default(),
        &context,
    )
    .unwrap();
    assert_eq!(scripts.len(), 2);
    for script in &scripts {
        assert!(!fs::read(script).unwrap().is_empty());
    }
    let before = scripts
        .iter()
        .map(|p| fs::read(p).unwrap())
        .collect::<Vec<_>>();
    f.write("New.bin", b"drift");
    let mut rejected = args;
    rejected.script_output = Some(f.top.join("must-not-exist/stage.sql"));
    assert!(
        stage_source_rows_bulk(
            &rejected,
            &SqlExec::detached("no SQL allowed"),
            &[],
            &[],
            b"versions",
            &StageAdditions::default(),
            &context
        )
        .is_err()
    );
    assert!(!f.top.join("must-not-exist").exists());
    assert_eq!(
        scripts
            .iter()
            .map(|p| fs::read(p).unwrap())
            .collect::<Vec<_>>(),
        before
    );
}

#[cfg(unix)]
#[test]
fn original_no_follow_refuses_links_before_copy_or_selection() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    f.write("A.bin", b"original");
    let context = f.context();
    symlink(f.top.join("outside"), f.root.join("link")).unwrap();
    assert!(context.scan_sources(&[]).is_err());
    assert!(
        context
            .original_source()
            .unwrap()
            .copy_to_new_projection(&f.top.join("projection"))
            .is_err()
    );
    assert!(!f.top.join("projection").exists());
}
