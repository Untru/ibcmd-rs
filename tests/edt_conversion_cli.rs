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
fn edt_source_profile_detection_does_not_hide_unknown_namespace_cells() {
    for (label, namespaces) in [
        (
            "many-namespaces",
            (0..384)
                .map(|index| format!(" xmlns:source{index}=\"urn:ibcmd:source:{index}\""))
                .collect::<String>(),
        ),
        (
            "long-prefix",
            format!(" xmlns:{}=\"urn:ibcmd:source\"", "p".repeat(1_025)),
        ),
    ] {
        let temp = Temp::new(label);
        let source = temp.0.join("source");
        let fixture = read_xml_source(fixture(), ReaderLimits::default()).unwrap();
        for entry in fixture.entries() {
            let path = source.join(entry.path().as_str());
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, entry.bytes()).unwrap();
        }
        let descriptor = source.join("Configuration.xml");
        let original = fs::read_to_string(&descriptor).unwrap();
        assert!(original.contains("<MetaDataObject "));
        fs::write(
            &descriptor,
            original.replacen(
                "<MetaDataObject ",
                &format!("<MetaDataObject{namespaces} "),
                1,
            ),
        )
        .unwrap();
        let project = temp.0.join("project");
        let rejected = run(&source, &project, "xml", "edt", &["--dry-run"]);
        assert!(!rejected.status.success());
        let report: Value = serde_json::from_slice(&rejected.stderr).unwrap();
        assert_eq!(report["phases"][0]["status"], "completed");
        assert_eq!(
            report["errors"][0]["code"],
            "conversion.edt-encode-preflight-failed"
        );
        assert!(
            report["errors"][0]["message"]
                .as_str()
                .unwrap()
                .contains("source cell(s) not covered")
        );
        assert_eq!(report["output_published"], false);
        assert!(!project.exists());
    }
}

#[test]
fn edt_directory_conversion_accepts_a_standard_descriptor_above_65536_elements() {
    let temp = Temp::new("large-enum");
    let source = temp.0.join("source");
    let fixture = read_xml_source(fixture(), ReaderLimits::default()).unwrap();
    for entry in fixture.entries() {
        let path = source.join(entry.path().as_str());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, entry.bytes()).unwrap();
    }
    let configuration = source.join("Configuration.xml");
    let original = fs::read_to_string(&configuration).unwrap();
    fs::write(
        &configuration,
        original.replacen(
            "</ChildObjects>",
            "<Enum>LargeValues</Enum></ChildObjects>",
            1,
        ),
    )
    .unwrap();
    let root_start = original.find("<MetaDataObject ").unwrap();
    let root_end = root_start + original[root_start..].find('>').unwrap() + 1;
    let mut descriptor = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    descriptor.push_str(&original[root_start..root_end]);
    descriptor.push_str("<Enum uuid=\"40000000-0000-4000-8000-000000000000\"><Properties><Name>LargeValues</Name><Synonym/><Comment/></Properties><ChildObjects>");
    for index in 0..11_000 {
        descriptor.push_str(&format!("<EnumValue uuid=\"50000000-0000-4000-8000-{index:012x}\"><Properties><Name>Value{index}</Name><Synonym/><Comment/></Properties></EnumValue>"));
    }
    descriptor.push_str("</ChildObjects></Enum></MetaDataObject>");
    assert!(11_000 * 6 > 65_536);
    fs::create_dir(source.join("Enums")).unwrap();
    fs::write(source.join("Enums/LargeValues.xml"), descriptor).unwrap();
    let project = temp.0.join("project");
    let preview = success(run(&source, &project, "xml", "edt", &["--dry-run"]));
    assert_eq!(preview["output_published"], false);
    assert!(!project.exists());
    success(run(&source, &project, "xml", "edt", &[]));
    let returned = temp.0.join("returned");
    success(run(&project, &returned, "edt", "xml", &[]));
    for path in fixture
        .entries()
        .iter()
        .map(|entry| entry.path().as_str())
        .chain(["Enums/LargeValues.xml"])
    {
        assert_eq!(
            fs::read(source.join(path)).unwrap(),
            fs::read(returned.join(path)).unwrap(),
            "{path}"
        );
    }
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

#[test]
fn edt_xml_profile_checks_descriptors_and_accepts_unversioned_interface_bodies() {
    let temp = Temp::new("descriptor-profile");
    let source = temp.0.join("source");
    let fixture = read_xml_source(fixture(), ReaderLimits::default()).unwrap();
    for entry in fixture.entries() {
        let path = source.join(entry.path().as_str());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, entry.bytes()).unwrap();
    }
    let mut interface = String::from("\u{feff}");
    interface.push_str(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<ClientApplicationInterface xmlns=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:type=\"InterfaceLayouter\">\r\n",
    );
    // Platform-defined panel identities are fixture data; the CLI does not
    // expose or depend directly on the adapter's private intermediate model.
    for id in [
        "b553047f-c9aa-4157-978d-448ecad24248",
        "13322b22-3960-4d68-93a6-fe2dd7f28ca3",
        "c933ac92-92cd-459d-81cc-e0c8a83ced99",
        "cbab57f2-a0f3-4f0a-89ea-4cb19570ab75",
        "b2735bd3-d822-4430-ba59-c9e869693b24",
        "8e10648b-f52d-4ec2-b4dd-87de33778d95",
    ] {
        interface.push_str(&format!("\t<panelDef id=\"{id}\"/>\r\n"));
    }
    interface.push_str("</ClientApplicationInterface>");
    let body = source.join("Ext/ClientApplicationInterface.xml");
    fs::create_dir_all(body.parent().unwrap()).unwrap();
    fs::write(&body, &interface).unwrap();
    let output = temp.0.join("project");
    let report = success(run(&source, &output, "xml", "edt", &["--dry-run"]));
    assert_eq!(report["output_published"], false);
    assert!(!output.exists());

    // A real subordinate descriptor still has to match the selected profile.
    // Its physical folder can classify it as a form or template instead of
    // MetadataXml, which must not bypass the descriptor gate.
    let descriptor_path = source.join("Catalogs/Parent/Forms/ProfileMismatch.xml");
    fs::create_dir_all(descriptor_path.parent().unwrap()).unwrap();
    fs::write(
        &descriptor_path,
        "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" version=\"2.20\"><Form uuid=\"11111111-1111-4111-8111-111111111111\"><Properties><Name>ProfileMismatch</Name></Properties></Form></MetaDataObject>",
    )
    .unwrap();
    let failed = run(&source, &output, "xml", "edt", &["--dry-run"]);
    assert!(!failed.status.success());
    let report: Value = serde_json::from_slice(&failed.stderr).unwrap();
    assert_eq!(
        report["errors"][0]["code"],
        "conversion.xml-source-profile-mismatch"
    );
    assert_eq!(
        report["errors"][0]["path"],
        "Catalogs/Parent/Forms/ProfileMismatch.xml"
    );
    assert!(!output.exists());
    assert_eq!(fs::read_to_string(body).unwrap(), interface);
}
