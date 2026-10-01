use ibcmd_edt::{ReaderLimits, read_project, read_xml_source};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("crates/ibcmd-edt/tests/fixtures/subsystem-ci/src")
}

struct Temp(PathBuf);
impl Temp {
    fn new(label: &str) -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ibcmd-edt-cli-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(input: &Path, output: &Path, from: &str, to: &str, extra: &[&str]) -> Output {
    let profile = |format| {
        if format == "xml" {
            "xml-2.21"
        } else {
            "edt-2025.2.3-xml-2.21"
        }
    };
    run_profiles(input, output, from, to, (profile(from), profile(to)), extra)
}

fn run_profiles(
    input: &Path,
    output: &Path,
    from: &str,
    to: &str,
    profiles: (&str, &str),
    extra: &[&str],
) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .arg("convert")
        .arg(input)
        .arg(output)
        .args([
            "--source-format",
            from,
            "--target-format",
            to,
            "--source-profile",
            profiles.0,
            "--target-profile",
            profiles.1,
        ])
        .args(extra)
        .env("PATH", "")
        .output()
        .unwrap()
}

fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn edt_conversion_is_offline_preflighted_and_exact_on_unchanged_return() {
    let temp = Temp::new("roundtrip");
    let project = temp.0.join("project");
    let preview = success(run(&fixture(), &project, "xml", "edt", &["--dry-run"]));
    assert_eq!(preview["ok"], true);
    assert_eq!(preview["output_published"], false);
    assert_eq!(preview["phases"][4]["status"], "completed");
    assert_eq!(preview["phases"][5]["status"], "skipped_dry_run");
    assert!(!project.exists());
    let published = success(run(&fixture(), &project, "xml", "edt", &[]));
    assert_eq!(published["output_published"], true);
    assert_eq!(published["publication"]["artifact"], "edt");
    assert!(published["edt"]["files"].as_array().unwrap().len() > 1);
    read_project(&project, ReaderLimits::default()).unwrap();
    let returned = temp.0.join("returned");
    let report = success(run(&project, &returned, "edt", "xml", &[]));
    assert_eq!(report["publication"]["artifact"], "xml");
    let source = read_xml_source(fixture(), ReaderLimits::default()).unwrap();
    assert_eq!(
        source,
        read_xml_source(&returned, ReaderLimits::default()).unwrap()
    );
    assert!(returned.join("ConfigDumpInfo.xml").is_file());
}

#[test]
fn edt_input_and_output_report_paths_cannot_mutate_artifacts() {
    let temp = Temp::new("report-paths");
    let project = temp.0.join("project");
    success(run(&fixture(), &project, "xml", "edt", &[]));
    let output = temp.0.join("xml");
    let report_path = project.join("src/report.json");
    let failed = run(
        &project,
        &output,
        "edt",
        "xml",
        &["--report", report_path.to_str().unwrap()],
    );
    assert!(!failed.status.success());
    assert!(!output.exists());
    assert!(!report_path.exists());
    let report: Value = serde_json::from_slice(&failed.stderr).unwrap();
    assert_eq!(
        report["errors"][0]["code"],
        "conversion.report-path-conflict"
    );
    let nested = project.join("converted");
    assert!(!run(&project, &nested, "edt", "xml", &[]).status.success());
    assert!(!nested.exists());
    assert!(
        !run(&fixture(), &project, "xml", "edt", &[])
            .status
            .success()
    );
}

#[test]
fn edt_profile_is_not_an_xml_profile_and_unknown_migration_fails_closed() {
    let temp = Temp::new("profiles");
    let output = temp.0.join("project");
    let mismatched = run_profiles(
        &fixture(),
        &output,
        "xml",
        "edt",
        ("edt-2025.2.3-xml-2.21", "edt-2025.2.3-xml-2.21"),
        &[],
    );
    assert!(!mismatched.status.success());
    let report: Value = serde_json::from_slice(&mismatched.stderr).unwrap();
    assert_eq!(
        report["errors"][0]["code"],
        "conversion.profile-format-mismatch"
    );
    assert!(!output.exists());
    let downgraded = run_profiles(
        &fixture(),
        &output,
        "xml",
        "edt",
        ("xml-2.21", "edt-2025.2.3-xml-2.20"),
        &[],
    );
    assert!(!downgraded.status.success());
    let report: Value = serde_json::from_slice(&downgraded.stderr).unwrap();
    assert_eq!(
        report["errors"][0]["code"],
        "conversion.edt-migration-unsupported"
    );
    assert!(!output.exists());
}
