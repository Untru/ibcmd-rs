use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new(label: &str) -> Self {
        // The wall clock alone can collide when parallel tests hit the same
        // timer tick; a per-process sequence keeps the names unique.
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let nonce = format!(
            "{}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(format!(
            "ibcmd-rs-conversion-cli-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn migration_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/migrations/2.20-to-2.21")
}

fn bootstrap_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bootstrap/8.3.27.1989/xcf-2.20/minimal")
}

fn copy_migration_xml(source: &Path, target: &Path) {
    fs::create_dir(target).unwrap();
    for name in ["changed-catalog.xml", "unchanged-constant.xml"] {
        fs::copy(source.join(name), target.join(name)).unwrap();
    }
}

fn convert(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .arg("convert")
        .args(arguments)
        .env("PATH", "")
        .output()
        .unwrap()
}

fn relative_files(root: &Path) -> Vec<String> {
    fn visit(root: &Path, current: &Path, files: &mut Vec<String>) {
        for entry in fs::read_dir(current).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, files);
            } else {
                files.push(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }

    let mut files = Vec::new();
    visit(root, root, &mut files);
    files.sort();
    files
}

#[test]
fn xml_migration_dry_run_completes_preflight_without_output() {
    let temp = TempDirectory::new("dry-run");
    let source = temp.path().join("source");
    copy_migration_xml(&migration_fixture(), &source);
    let destination = temp.path().join("converted");
    let report_path = temp.path().join("report.json");
    let output = convert(&[
        source.to_str().unwrap(),
        destination.to_str().unwrap(),
        "--source-format",
        "xml",
        "--target-format",
        "xml",
        "--source-profile",
        "xml-2.20",
        "--target-profile",
        "xml-2.21",
        "--dry-run",
        "--report",
        report_path.to_str().unwrap(),
    ]);

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!destination.exists());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], true);
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["output_published"], false);
    assert_eq!(report["phases"][4]["status"], "completed");
    assert_eq!(report["phases"][5]["status"], "skipped_dry_run");
    assert_eq!(report["plan"]["steps"][0], "migration:xcf-2.20-to-2.21");
    assert_eq!(report["migrations"].as_array().unwrap().len(), 2);
    let file_report: Value = serde_json::from_slice(&fs::read(report_path).unwrap()).unwrap();
    assert_eq!(file_report["preflight"], report["preflight"]);
}

#[test]
fn xml_cf_xml_roundtrip_is_offline_and_atomic() {
    let temp = TempDirectory::new("xml-cf-xml");
    let cf = temp.path().join("configuration.cf");
    let restored = temp.path().join("restored");
    let input = bootstrap_fixture();

    let to_cf = convert(&[
        input.to_str().unwrap(),
        cf.to_str().unwrap(),
        "--source-format",
        "xml",
        "--target-format",
        "cf",
        "--source-profile",
        "xml-2.20",
        "--target-profile",
        "platform-8.3.27.1989",
        "--target-revision",
        "format16",
    ]);
    assert!(
        to_cf.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&to_cf.stdout),
        String::from_utf8_lossy(&to_cf.stderr)
    );
    let to_cf_report: Value = serde_json::from_slice(&to_cf.stdout).unwrap();
    assert_eq!(to_cf_report["output_published"], true);
    assert_eq!(to_cf_report["publication"]["cf_revision"], "format16");
    assert!(cf.is_file());

    let repacked = temp.path().join("repacked.cf");
    let cf_to_cf = convert(&[
        cf.to_str().unwrap(),
        repacked.to_str().unwrap(),
        "--source-format",
        "cf",
        "--target-format",
        "cf",
        "--source-profile",
        "platform-8.3.27.1989",
        "--target-profile",
        "platform-8.3.27.1989",
    ]);
    assert!(
        cf_to_cf.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&cf_to_cf.stdout),
        String::from_utf8_lossy(&cf_to_cf.stderr)
    );
    let cf_to_cf_report: Value = serde_json::from_slice(&cf_to_cf.stdout).unwrap();
    assert_eq!(cf_to_cf_report["plan"]["kind"], "lossless_repack");
    assert_eq!(cf_to_cf_report["output_published"], true);
    assert!(repacked.is_file());

    let to_xml = convert(&[
        cf.to_str().unwrap(),
        restored.to_str().unwrap(),
        "--source-format",
        "cf",
        "--target-format",
        "xml",
        "--source-profile",
        "platform-8.3.27.1989",
        "--target-profile",
        "xml-2.20",
    ]);
    assert!(
        to_xml.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&to_xml.stdout),
        String::from_utf8_lossy(&to_xml.stderr)
    );
    let to_xml_report: Value = serde_json::from_slice(&to_xml.stdout).unwrap();
    assert_eq!(to_xml_report["output_published"], true);
    assert_eq!(to_xml_report["preflight"]["opaque_entries"], 0);
    // The XML a CF exports to carries its `ConfigDumpInfo.xml`, as the
    // platform's own dump does, so the round trip adds exactly that file.
    let mut expected = relative_files(&input);
    expected.push("ConfigDumpInfo.xml".to_owned());
    expected.sort();
    assert_eq!(relative_files(&restored), expected);

    let collision = convert(&[
        input.to_str().unwrap(),
        cf.to_str().unwrap(),
        "--source-format",
        "xml",
        "--target-format",
        "cf",
        "--source-profile",
        "xml-2.20",
        "--target-profile",
        "platform-8.3.27.1989",
    ]);
    assert!(!collision.status.success());
    let collision_report: Value = serde_json::from_slice(&collision.stderr).unwrap();
    assert_eq!(
        collision_report["errors"][0]["code"],
        "conversion.destination-exists"
    );
}

/// Platform 8.5 (Untru/ibcmd-rs#354): the configuration 8.5.1.1529 saved
/// (`external/home_page/one_column_v85/input.cf`, compatibility 8.3.27) read
/// as XML 2.21, with a common module of the platform's own 2.21 spelling
/// (`external/v85_extension/test_extension`, dumped by 8.5.1.1529) added,
/// converted to a CF for 8.5.1.1150 and back: the same tree.
#[test]
fn xml_221_cf_851_xml_roundtrip_is_offline() {
    let temp = TempDirectory::new("xml-221-cf-851");
    let external = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/external");
    let input = temp.path().join("input");
    let exported = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "export"])
        .arg(external.join("home_page/one_column_v85/input.cf"))
        .arg(&input)
        .args(["--platform", "8.5.1.1150"])
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(exported.status.success());
    fs::remove_file(input.join("ConfigDumpInfo.xml")).unwrap();
    let module = "ТестРасширение_Модуль";
    let module_source = external.join("v85_extension/test_extension/CommonModules");
    fs::create_dir_all(input.join(format!("CommonModules/{module}/Ext"))).unwrap();
    for file in [format!("{module}.xml"), format!("{module}/Ext/Module.bsl")] {
        fs::copy(
            module_source.join(&file),
            input.join("CommonModules").join(&file),
        )
        .unwrap();
    }
    let configuration = input.join("Configuration.xml");
    let text = fs::read_to_string(&configuration).unwrap();
    let language = "\t\t\t<Language>Русский</Language>\r\n";
    assert!(text.contains(language));
    fs::write(
        &configuration,
        text.replace(
            language,
            &format!("{language}\t\t\t<CommonModule>{module}</CommonModule>\r\n"),
        ),
    )
    .unwrap();

    let cf = temp.path().join("configuration.cf");
    let to_cf = convert(&[
        input.to_str().unwrap(),
        cf.to_str().unwrap(),
        "--source-format",
        "xml",
        "--target-format",
        "cf",
        "--source-profile",
        "xml-2.21",
        "--target-profile",
        "platform-8.5.1.1150",
    ]);
    assert!(
        to_cf.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&to_cf.stdout),
        String::from_utf8_lossy(&to_cf.stderr)
    );
    let to_cf_report: Value = serde_json::from_slice(&to_cf.stdout).unwrap();
    assert_eq!(to_cf_report["output_published"], true);
    // The base-free stage's writer: Format15, as `cf bootstrap --base-free`.
    assert_eq!(to_cf_report["publication"]["cf_revision"], "format15");
    assert_eq!(
        to_cf_report["plan"]["steps"][0],
        "adapter:xml-to-cf-base-free"
    );

    let restored = temp.path().join("restored");
    let to_xml = convert(&[
        cf.to_str().unwrap(),
        restored.to_str().unwrap(),
        "--source-format",
        "cf",
        "--target-format",
        "xml",
        "--source-profile",
        "platform-8.5.1.1150",
        "--target-profile",
        "xml-2.21",
    ]);
    assert!(
        to_xml.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&to_xml.stdout),
        String::from_utf8_lossy(&to_xml.stderr)
    );
    let mut expected = relative_files(&input);
    expected.push("ConfigDumpInfo.xml".to_owned());
    expected.sort();
    assert_eq!(relative_files(&restored), expected);
    for file in relative_files(&input) {
        assert!(
            fs::read(input.join(&file)).unwrap() == fs::read(restored.join(&file)).unwrap(),
            "{file}"
        );
    }
    let dump_info = fs::read_to_string(restored.join("ConfigDumpInfo.xml")).unwrap();
    assert!(dump_info.contains(&format!("name=\"CommonModule.{module}\"")));
}

#[test]
fn format_and_profile_coordinates_are_not_inferred() {
    let temp = TempDirectory::new("profile-format");
    let destination = temp.path().join("converted");
    let input = migration_fixture();
    let output = convert(&[
        input.to_str().unwrap(),
        destination.to_str().unwrap(),
        "--source-format",
        "xml",
        "--target-format",
        "xml",
        "--source-profile",
        "platform-8.3.27.1989",
        "--target-profile",
        "xml-2.21",
    ]);

    assert!(!output.status.success());
    assert!(!destination.exists());
    let report: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(
        report["errors"][0]["code"],
        "conversion.profile-format-mismatch"
    );
    assert_eq!(report["phases"][0]["status"], "failed");
}

#[test]
fn report_cannot_replace_the_destination_even_in_dry_run() {
    let temp = TempDirectory::new("report-conflict");
    let destination = temp.path().join("converted");
    let input = migration_fixture();
    let output = convert(&[
        input.to_str().unwrap(),
        destination.to_str().unwrap(),
        "--source-format",
        "xml",
        "--target-format",
        "xml",
        "--source-profile",
        "xml-2.20",
        "--target-profile",
        "xml-2.21",
        "--dry-run",
        "--report",
        destination.to_str().unwrap(),
    ]);

    assert!(!output.status.success());
    assert!(!destination.exists());
    let report: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(
        report["errors"][0]["code"],
        "conversion.report-path-conflict"
    );
}

#[test]
fn cross_profile_unknown_assets_and_nested_outputs_fail_closed() {
    let temp = TempDirectory::new("fail-closed-paths");
    let source = temp.path().join("source");
    copy_migration_xml(&migration_fixture(), &source);
    fs::write(source.join("unknown.dat"), b"must not disappear").unwrap();
    let destination = temp.path().join("converted");
    let unknown = convert(&[
        source.to_str().unwrap(),
        destination.to_str().unwrap(),
        "--source-format",
        "xml",
        "--target-format",
        "xml",
        "--source-profile",
        "xml-2.20",
        "--target-profile",
        "xml-2.21",
    ]);
    assert!(!unknown.status.success());
    assert!(!destination.exists());
    let report: Value = serde_json::from_slice(&unknown.stderr).unwrap();
    assert_eq!(
        report["errors"][0]["code"],
        "conversion.xml-validation-failed"
    );

    let nested = source.join("generated");
    let nested_output = convert(&[
        source.to_str().unwrap(),
        nested.to_str().unwrap(),
        "--source-format",
        "xml",
        "--target-format",
        "xml",
        "--source-profile",
        "xml-2.20",
        "--target-profile",
        "xml-2.20",
    ]);
    assert!(!nested_output.status.success());
    assert!(!nested.exists());
    let report: Value = serde_json::from_slice(&nested_output.stderr).unwrap();
    assert_eq!(
        report["errors"][0]["code"],
        "conversion.artifact-path-conflict"
    );
}
