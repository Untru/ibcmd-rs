use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

use ibcmd_core::{
    artifact::ProfileId,
    diagnostic::{LossPolicy, ObjectPath, PathSegment, PropertyPath},
    identity::{LogicalIdentity, ObjectUuid},
    migration::{
        executor::{MigrationExecutionRequest, MigrationExecutor},
        graph::MigrationGraph,
        v2_21_to_v2_20::{INTERFACE_MODE_LOSS_CODE, SOURCE_PROFILE, TARGET_PROFILE, V2_21ToV2_20},
    },
    model::{CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, MetadataKind},
    provenance::{CanonicalAnchor, SourceProvenance},
    value::{CanonicalField, CanonicalText, CanonicalValue, EnumToken},
};
use ibcmd_rs::{
    compatibility::{
        EvidenceKind, EvidenceState, RouteSupport, VerificationStatus,
        current_compatibility_report, validate_repository_evidence,
    },
    compiler::readiness::bundled_manifest,
    profile_registry::load_bundled_profile_registry,
};
use serde_json::Value;

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ibcmd-rs-offline-matrix-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        fs::create_dir(path.join("empty-path")).unwrap();
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

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn bootstrap_fixture() -> PathBuf {
    repository_root().join("tests/fixtures/bootstrap/8.3.27.1989/xcf-2.20/minimal")
}

fn migration_fixture(direction: &str) -> PathBuf {
    repository_root()
        .join("tests/fixtures/migrations")
        .join(direction)
}

fn run_offline(temp: &TempDirectory, arguments: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"));
    command
        .args(arguments)
        .env("PATH", temp.path().join("empty-path"));
    for variable in [
        "IBCMD_PATH",
        "JAVA_HOME",
        "JDK_HOME",
        "ECLIPSE_HOME",
        "ONEC_HOME",
        "1C_HOME",
    ] {
        command.env_remove(variable);
    }
    command.output().unwrap()
}

fn successful_report(output: Output) -> Value {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], true);
    assert_eq!(report["errors"].as_array().unwrap().len(), 0);
    report
}

fn failed_report(output: Output) -> Value {
    assert!(!output.status.success());
    serde_json::from_slice(&output.stderr).unwrap()
}

fn relative_files(root: &Path) -> Vec<String> {
    fn visit(root: &Path, current: &Path, files: &mut Vec<String>) {
        for entry in fs::read_dir(current).unwrap() {
            let path = entry.unwrap().path();
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

fn copy_migration_cohort(source: &Path, target: &Path) {
    fs::create_dir(target).unwrap();
    for name in ["changed-catalog.xml", "unchanged-constant.xml"] {
        fs::copy(source.join(name), target.join(name)).unwrap();
    }
}

fn assert_completed_pipeline(report: &Value) {
    let phases = report["phases"].as_array().unwrap();
    assert_eq!(phases.len(), 6);
    assert!(phases.iter().all(|phase| phase["status"] == "completed"));
    assert_eq!(report["output_published"], true);
}

fn assert_no_migration_losses(report: &Value) {
    for migration in report["migrations"].as_array().unwrap() {
        let migration_report = &migration["report"];
        assert_eq!(migration_report["outcome"], "success");
        assert_eq!(migration_report["terminal_failure"], Value::Null);
        for step in migration_report["steps"].as_array().unwrap() {
            assert!(step["losses"].as_array().unwrap().is_empty());
        }
    }
}

#[test]
fn xml_cf_xml_and_same_profile_cf_repack_cover_both_revisions_offline() {
    let temp = TempDirectory::new("cf-routes");
    let source = bootstrap_fixture();
    let source_files = relative_files(&source);

    for revision in ["format15", "format16"] {
        let cf = temp.path().join(format!("configuration-{revision}.cf"));
        let repacked = temp.path().join(format!("repacked-{revision}.cf"));
        let restored = temp.path().join(format!("restored-{revision}"));

        let to_cf = successful_report(run_offline(
            &temp,
            &[
                "convert",
                source.to_str().unwrap(),
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
                revision,
            ],
        ));
        assert_completed_pipeline(&to_cf);
        assert_eq!(to_cf["publication"]["cf_revision"], revision);

        let repack = successful_report(run_offline(
            &temp,
            &[
                "convert",
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
            ],
        ));
        assert_completed_pipeline(&repack);
        assert_eq!(repack["plan"]["kind"], "lossless_repack");
        assert_eq!(repack["publication"]["cf_revision"], revision);

        let to_xml = successful_report(run_offline(
            &temp,
            &[
                "convert",
                repacked.to_str().unwrap(),
                restored.to_str().unwrap(),
                "--source-format",
                "cf",
                "--target-format",
                "xml",
                "--source-profile",
                "platform-8.3.27.1989",
                "--target-profile",
                "xml-2.20",
            ],
        ));
        assert_completed_pipeline(&to_xml);
        assert_eq!(to_xml["preflight"]["opaque_entries"], 0);
        assert_eq!(relative_files(&restored), source_files);
        assert_eq!(
            fs::read(restored.join("CommonModules/Portable/Ext/Module.bsl")).unwrap(),
            fs::read(source.join("CommonModules/Portable/Ext/Module.bsl")).unwrap()
        );
    }
}

#[test]
fn same_and_cross_profile_xml_routes_have_no_unexpected_losses() {
    let temp = TempDirectory::new("xml-routes");
    let source = temp.path().join("source-220");
    copy_migration_cohort(&migration_fixture("2.20-to-2.21"), &source);

    let same = temp.path().join("same-220");
    let same_report = successful_report(run_offline(
        &temp,
        &[
            "convert",
            source.to_str().unwrap(),
            same.to_str().unwrap(),
            "--source-format",
            "xml",
            "--target-format",
            "xml",
            "--source-profile",
            "xml-2.20",
            "--target-profile",
            "xml-2.20",
        ],
    ));
    assert_completed_pipeline(&same_report);
    assert_eq!(same_report["plan"]["steps"].as_array().unwrap().len(), 0);
    assert_no_migration_losses(&same_report);
    assert_eq!(relative_files(&same), relative_files(&source));

    let upgraded = temp.path().join("upgraded-221");
    let upgrade = successful_report(run_offline(
        &temp,
        &[
            "convert",
            source.to_str().unwrap(),
            upgraded.to_str().unwrap(),
            "--source-format",
            "xml",
            "--target-format",
            "xml",
            "--source-profile",
            "xml-2.20",
            "--target-profile",
            "xml-2.21",
        ],
    ));
    assert_completed_pipeline(&upgrade);
    assert_eq!(upgrade["plan"]["steps"][0], "migration:xcf-2.20-to-2.21");
    assert_no_migration_losses(&upgrade);

    let downgraded = temp.path().join("downgraded-220");
    let downgrade = successful_report(run_offline(
        &temp,
        &[
            "convert",
            upgraded.to_str().unwrap(),
            downgraded.to_str().unwrap(),
            "--source-format",
            "xml",
            "--target-format",
            "xml",
            "--source-profile",
            "xml-2.21",
            "--target-profile",
            "xml-2.20",
        ],
    ));
    assert_completed_pipeline(&downgrade);
    assert_eq!(downgrade["plan"]["steps"][0], "migration:xcf-2.21-to-2.20");
    assert_no_migration_losses(&downgrade);
    assert_eq!(relative_files(&downgraded), relative_files(&source));
}

#[test]
fn malformed_artifacts_and_unknown_profiles_never_publish() {
    let temp = TempDirectory::new("fail-closed");
    let source = bootstrap_fixture();
    let valid_cf = temp.path().join("valid.cf");
    successful_report(run_offline(
        &temp,
        &[
            "convert",
            source.to_str().unwrap(),
            valid_cf.to_str().unwrap(),
            "--source-format",
            "xml",
            "--target-format",
            "cf",
            "--source-profile",
            "xml-2.20",
            "--target-profile",
            "platform-8.3.27.1989",
        ],
    ));

    let mut truncated = fs::read(&valid_cf).unwrap();
    truncated.truncate(truncated.len() / 2);
    let malformed_cf = temp.path().join("truncated.cf");
    fs::write(&malformed_cf, truncated).unwrap();
    let malformed_output = temp.path().join("malformed-output");
    let malformed = failed_report(run_offline(
        &temp,
        &[
            "convert",
            malformed_cf.to_str().unwrap(),
            malformed_output.to_str().unwrap(),
            "--source-format",
            "cf",
            "--target-format",
            "xml",
            "--source-profile",
            "platform-8.3.27.1989",
            "--target-profile",
            "xml-2.20",
        ],
    ));
    assert_eq!(malformed["ok"], false);
    assert_eq!(
        malformed["errors"][0]["code"],
        "conversion.cf-decode-failed"
    );
    assert!(!malformed_output.exists());

    let unknown_output = temp.path().join("unknown-output.cf");
    let unknown = failed_report(run_offline(
        &temp,
        &[
            "convert",
            source.to_str().unwrap(),
            unknown_output.to_str().unwrap(),
            "--source-format",
            "xml",
            "--target-format",
            "cf",
            "--source-profile",
            "xml-2.20",
            "--target-profile",
            "platform-99.1.0",
        ],
    ));
    assert_eq!(unknown["errors"][0]["code"], "conversion.profile-not-found");
    assert!(!unknown_output.exists());
}

#[test]
fn default_loss_policy_blocks_the_evidenced_downgrade_loss() {
    let fixture: Value = serde_json::from_slice(
        &fs::read(migration_fixture("2.21-to-2.20").join("lossy-form.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(fixture["loss_code"], INTERFACE_MODE_LOSS_CODE);
    assert_eq!(fixture["default_policy"], "error");

    let object_path = fixture["object_path"].as_str().unwrap();
    let path = ObjectPath::new(vec![PathSegment::name(object_path).unwrap()]).unwrap();
    let mut parts = CanonicalObjectParts::new(
        LogicalIdentity::new(
            ObjectUuid::parse(fixture["uuid"].as_str().unwrap()).unwrap(),
            path.clone(),
        ),
        MetadataKind::new(fixture["family"].as_str().unwrap()).unwrap(),
        SourceProvenance::new(
            ProfileId::parse(SOURCE_PROFILE).unwrap(),
            CanonicalAnchor::new(path, PropertyPath::root()),
        ),
    );
    parts.properties = vec![
        CanonicalField::named(
            "Name",
            CanonicalValue::text(CanonicalText::new(fixture["name"].as_str().unwrap()).unwrap()),
        )
        .unwrap(),
        CanonicalField::named(
            fixture["property"].as_str().unwrap(),
            CanonicalValue::enum_token(EnumToken::new(fixture["value"].as_str().unwrap()).unwrap()),
        )
        .unwrap(),
    ];
    let configuration =
        CanonicalConfiguration::new(vec![CanonicalObject::new(parts).unwrap()]).unwrap();

    let profiles = load_bundled_profile_registry().unwrap();
    let graph = MigrationGraph::new(&profiles, vec![V2_21ToV2_20::verified_edge()]).unwrap();
    let source = profiles
        .get(&ProfileId::parse(SOURCE_PROFILE).unwrap())
        .unwrap();
    let target = profiles
        .get(&ProfileId::parse(TARGET_PROFILE).unwrap())
        .unwrap();
    let plan = graph.plan(source, target).unwrap();
    let error = MigrationExecutor::new(&graph)
        .execute(MigrationExecutionRequest::new(
            &plan,
            &configuration,
            LossPolicy::Error,
        ))
        .unwrap_err();
    let report = error.report().unwrap();
    assert!(!report.is_complete());
    assert_eq!(report.steps().len(), 1);
    assert!(
        serde_json::to_string(report)
            .unwrap()
            .contains(INTERFACE_MODE_LOSS_CODE)
    );
}

#[test]
fn release_readiness_is_backed_by_registered_green_evidence() {
    let matrix = current_compatibility_report().unwrap();
    validate_repository_evidence(&matrix, &repository_root()).unwrap();
    bundled_manifest().unwrap().validate().unwrap();

    let evidence = matrix
        .evidence
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect::<std::collections::BTreeMap<_, _>>();
    let verified = matrix
        .routes
        .iter()
        .filter(|route| route.status == VerificationStatus::Verified)
        .collect::<Vec<_>>();
    assert!(!verified.is_empty());
    for route in verified {
        assert_eq!(route.support, RouteSupport::Supported, "{}", route.id);
        assert!(route.evidence.iter().any(|id| {
            evidence.get(id.as_str()).is_some_and(|item| {
                item.kind == EvidenceKind::CargoTest && item.state == EvidenceState::Green
            })
        }));
    }

    let bootstrap: Value = serde_json::from_slice(
        &fs::read(repository_root().join("tests/fixtures/bootstrap/manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(bootstrap["schema_version"], 1);
    assert_eq!(bootstrap["fixtures"][0]["contains_application_code"], false);
    assert_eq!(
        bootstrap["fixtures"][0]["platform_profile"],
        "platform-8.3.27.1989"
    );
}

#[test]
fn accepted_roadmap_and_readme_report_full_completion() {
    let root = repository_root();
    let tasks =
        fs::read_to_string(root.join("openspec/changes/build-offline-converter/tasks.md")).unwrap();
    assert!(
        !tasks.contains("**Статус:** `[ ]`"),
        "the release gate cannot pass while an accepted leaf task is open"
    );

    let readme = fs::read_to_string(root.join("README.md")).unwrap();
    assert!(
        readme.contains(
            "<!-- offline-converter-progress: completed=56 total=56 updated=2026-07-23 -->"
        )
    );
    assert!(readme.contains("| **Overall** | **56/56** | **100%** |"));
}
