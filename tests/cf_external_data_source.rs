//! Generated public CF routes for #435. No foreign fixture or platform oracle.
//! Enum codes and XML2.21 are source compatibility tests, not native acceptance.

mod generated {
    use ibcmd_rs as product;
    include!("common/eds_generated.rs");
}

use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

use ibcmd_rs::cli::InfobaseConfigSourceVersion;
use ibcmd_rs::{
    metadata_model::{brace::parse_row, export::configuration_objects},
    mssql::base_free_cf::base_free_entries,
};

struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "ibcmd-generated-eds435-{}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn sources(&self, version: &str, mode: &str) -> PathBuf {
        let root = self.0.join("input");
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("ExternalDataSources")).unwrap();
        fs::create_dir(root.join("ScheduledJobs")).unwrap();
        fs::create_dir(root.join("CommonModules")).unwrap();
        fs::write(
            root.join("Configuration.xml"),
            generated::configuration(version).replace(
                "<ChildObjects>",
                "<ChildObjects><CommonModule>Handler435</CommonModule>",
            ),
        )
        .unwrap();
        fs::write(
            root.join("ExternalDataSources/Source435.xml"),
            generated::external_data_source(version, mode, [2, 0, 1]),
        )
        .unwrap();
        fs::write(
            root.join("ScheduledJobs/Job435.xml"),
            generated::scheduled_job(version).replace(
                "<MethodName/>",
                "<MethodName>CommonModule.Handler435.Run</MethodName>",
            ),
        )
        .unwrap();
        fs::write(
            root.join("CommonModules/Handler435.xml"),
            generated::handler_module(version),
        )
        .unwrap();
        root
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn bootstrap(source: &Path, cf: &Path, version: &str) -> std::process::Output {
    let platform = if version == "2.20" {
        "8.3.27.2214"
    } else {
        "8.5.1.1529"
    };
    Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "bootstrap", "--base-free", "--platform", platform])
        .arg(source)
        .arg(cf)
        .env("PATH", "")
        .output()
        .unwrap()
}
fn export(cf: &Path, source: &Path, version: &str, strict: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"));
    command.args(["cf", "export", "--source-version", version]);
    if strict {
        command.args(["--fail-on-opaque", "--index"]);
    }
    command
        .arg(cf)
        .arg(source)
        .env("PATH", "")
        .output()
        .unwrap()
}
fn compatibility_export(cf: &Path, source: &Path, version: &str) {
    let output = export(cf, source, version, false);
    succeeded(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], true, "{report}");
    let storage = &report["export"]["storage"];
    assert_eq!(storage["failed"], 0, "{report}");
    let entries = storage["entries"].as_array().unwrap();
    let mut opaque = 0;
    for entry in entries {
        let name = entry["logical_name"].as_str().unwrap();
        match entry["disposition"].as_str().unwrap() {
            "opaque" => {
                assert!(
                    ["root", "version", "versions"].contains(&name),
                    "unexpected opaque metadata: {entry}"
                );
                opaque += 1;
            }
            "supported" => {}
            other => panic!("unexpected disposition {other}: {entry}"),
        }
    }
    assert_eq!(storage["opaque"].as_u64().unwrap(), opaque);
    for id in [
        generated::CONFIGURATION_UUID,
        generated::OBJECT_UUID,
        generated::JOB_UUID,
        generated::MODULE_UUID,
    ] {
        let item = entries
            .iter()
            .find(|entry| entry["logical_name"] == id)
            .unwrap();
        assert_eq!(item["disposition"], "supported", "{item}");
    }
}
fn succeeded(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
fn version(value: &str) -> InfobaseConfigSourceVersion {
    if value == "2.20" {
        InfobaseConfigSourceVersion::V2_20
    } else {
        InfobaseConfigSourceVersion::V2_21
    }
}

#[test]
fn public_base_free_bootstrap_exports_all_eds_types_and_rebuilds_them_both_profiles() {
    for dialect in ["2.20", "2.21"] {
        for mode in ["Automatic", "Managed", "AutomaticAndManaged"] {
            let tree = Tree::new();
            let source = tree.sources(dialect, mode);
            let cf = tree.0.join("generated.cf");
            let exported = tree.0.join("exported");
            let result = bootstrap(&source, &cf, dialect);
            succeeded(&result);
            let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
            assert_eq!(report["source_version"], dialect);
            compatibility_export(&cf, &exported, dialect);
            let path = exported.join("ExternalDataSources/Source435.xml");
            let xml = fs::read_to_string(&path).unwrap();
            assert!(xml.contains(&format!(
                "<ExternalDataSource uuid=\"{}\">",
                generated::OBJECT_UUID
            )));
            assert_eq!(xml.matches("<xr:GeneratedType ").count(), 3);
            for ((type_id, value_id), category) in
                generated::PAIRS
                    .iter()
                    .zip(["Manager", "TablesManager", "CubesManager"])
            {
                assert!(xml.contains(&format!("category=\"{category}\"")));
                assert!(xml.contains(&format!("<xr:TypeId>{type_id}</xr:TypeId>")));
                assert!(xml.contains(&format!("<xr:ValueId>{value_id}</xr:ValueId>")));
            }
            assert!(xml.contains(&format!(
                "<DataLockControlMode>{mode}</DataLockControlMode>"
            )));
            assert!(xml.contains("<ChildObjects/>"));
            assert!(xml.contains("line1\nquoted \"text\""));
            assert!(exported.join("ScheduledJobs/Job435.xml").is_file());
            let job = fs::read_to_string(exported.join("ScheduledJobs/Job435.xml")).unwrap();
            assert!(job.contains("<MethodName>CommonModule.Handler435.Run</MethodName>"));
            assert!(exported.join("CommonModules/Handler435.xml").is_file());
            assert!(!exported.join("SettingsStorages/Source435.xml").exists());
            assert!(!exported.join("ScheduledJobs/Source435.xml").exists());
            let second = tree.0.join("second.cf");
            let twice = tree.0.join("twice");
            succeeded(&bootstrap(&exported, &second, dialect));
            compatibility_export(&second, &twice, dialect);
            assert_eq!(
                fs::read(&path).unwrap(),
                fs::read(twice.join("ExternalDataSources/Source435.xml")).unwrap()
            );
        }
    }
}

#[test]
fn strict_public_export_refuses_structural_opaque_without_publishing_an_index() {
    let tree = Tree::new();
    let source = tree.sources("2.20", "Automatic");
    let cf = tree.0.join("strict.cf");
    succeeded(&bootstrap(&source, &cf, "2.20"));
    let target = tree.0.join("strict-refused");
    let output = export(&cf, &target, "2.20", true);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(report["ok"], false, "{report}");
    assert_eq!(report["export"]["storage"]["failed"], 0, "{report}");
    assert!(report["export"]["storage"]["opaque"].as_u64().unwrap() > 0);
    assert!(
        report["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error["code"] == "opaque_export_refused")
    );
    assert!(
        !target.join(".ibcmd/index.tsv").exists(),
        "strict refusal must not certify an export index"
    );
}

#[test]
fn generated_root_binary_section_keeps_declared_eds_uuid_separate_from_job() {
    for dialect in ["2.20", "2.21"] {
        let tree = Tree::new();
        let source = tree.sources(dialect, "Automatic");
        let entries = base_free_entries(&source, version(dialect)).unwrap();
        let mut plain = Vec::new();
        flate2::read::DeflateDecoder::new(
            entries
                .get(generated::CONFIGURATION_UUID)
                .unwrap()
                .as_slice(),
        )
        .read_to_end(&mut plain)
        .unwrap();
        let objects = configuration_objects(&parse_row(&plain).unwrap()).unwrap();
        assert!(objects.contains(&("ExternalDataSource".into(), generated::OBJECT_UUID.into())));
        assert!(objects.contains(&("ScheduledJob".into(), generated::JOB_UUID.into())));
        assert!(objects.contains(&("CommonModule".into(), generated::MODULE_UUID.into())));
        assert_eq!(objects.len(), 3);
        assert!(entries.contains_key(generated::OBJECT_UUID));
        assert!(entries.contains_key(generated::JOB_UUID));
        assert_eq!(generated::NAME, "Source435");
    }
}

#[test]
fn public_bootstrap_refuses_wrong_category_unknown_property_and_nonempty_eds_without_publication() {
    for mutation in ["duplicate-category", "future-property", "nonempty"] {
        let tree = Tree::new();
        let root = tree.sources("2.20", "Automatic");
        let file = root.join("ExternalDataSources/Source435.xml");
        let xml = fs::read_to_string(&file).unwrap();
        let changed = match mutation {
            "duplicate-category" => {
                xml.replace("category=\"CubesManager\"", "category=\"TablesManager\"")
            }
            "future-property" => xml.replace(
                "</Properties>",
                "<Future>do not discard</Future></Properties>",
            ),
            _ => xml.replace(
                "<ChildObjects/>",
                "<ChildObjects><Table>Missing</Table></ChildObjects>",
            ),
        };
        fs::write(&file, changed).unwrap();
        let cf = tree.0.join("refused.cf");
        let result = bootstrap(&root, &cf, "2.20");
        assert!(!result.status.success(), "{mutation}");
        let report: serde_json::Value =
            serde_json::from_slice(&result.stderr).unwrap_or_else(|_| {
                panic!(
                    "non-product refusal: {}",
                    String::from_utf8_lossy(&result.stderr)
                )
            });
        assert_eq!(report["ok"], false);
        assert_eq!(report["storage_entries"], 0);
        assert!(report["publication"].is_null());
        assert_eq!(report["errors"][0]["code"], "base_free_compile_failed");
        assert!(
            report["errors"][0]["message"]
                .as_str()
                .unwrap()
                .contains("ExternalDataSource")
        );
        assert!(!cf.exists(), "{mutation} must refuse before publication");
    }
}
