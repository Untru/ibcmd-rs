use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ibcmd-rs-cf-bootstrap-{}-{nonce}",
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

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bootstrap/8.3.27.1989/xcf-2.20/minimal")
}

fn fixture_manifest() -> Value {
    serde_json::from_slice(
        &fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/bootstrap/manifest.json"),
        )
        .unwrap(),
    )
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
fn checked_in_source_tree_bootstraps_and_exports_without_missing_or_extra_files() {
    let manifest = fixture_manifest();
    let fixture = &manifest["fixtures"][0];
    assert_eq!(manifest["schema_version"], 1);
    assert_eq!(fixture["provenance"], "hand-authored-clean-room");
    assert_eq!(fixture["contains_application_code"], false);
    let expected_sources = fixture["expected_source_files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|path| path.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(relative_files(&fixture_root()), expected_sources);

    let temp = TempDirectory::new();
    let cf = temp.path().join("configuration.cf");
    let exported = temp.path().join("exported");

    let bootstrap = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "bootstrap"])
        .arg(fixture_root())
        .arg(&cf)
        .args([
            "--source-version",
            "2.20",
            "--target-profile",
            "platform-8.3.27.1989",
            "--revision",
            "format16",
        ])
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(
        bootstrap.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&bootstrap.stdout),
        String::from_utf8_lossy(&bootstrap.stderr)
    );
    let report: Value = serde_json::from_slice(&bootstrap.stdout).unwrap();
    assert_eq!(report["command"], "bootstrap");
    assert_eq!(report["ok"], true);
    assert_eq!(report["source_files"], 3);
    assert_eq!(report["storage_entries"], 6);

    let inspect = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "inspect"])
        .arg(&cf)
        .args(["--profile", "storage:mssql-config-configsave"])
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(
        inspect.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&inspect.stdout),
        String::from_utf8_lossy(&inspect.stderr)
    );
    let report: Value = serde_json::from_slice(&inspect.stdout).unwrap();
    let actual_entries = report["elements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    let expected_entries = fixture["expected_storage_entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry.as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(actual_entries, expected_entries);

    let export = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "export"])
        .arg(&cf)
        .arg(&exported)
        .args([
            "--source-version",
            "2.20",
            "--profile",
            "storage:mssql-config-configsave",
        ])
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(
        export.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&export.stdout),
        String::from_utf8_lossy(&export.stderr)
    );
    let report: Value = serde_json::from_slice(&export.stdout).unwrap();
    assert_eq!(report["command"], "export");
    assert_eq!(report["ok"], true);
    assert_eq!(report["export"]["storage"]["failed"], 0);
    assert_eq!(relative_files(&exported), expected_sources);
}

#[test]
fn unsupported_source_prevents_publication() {
    let temp = TempDirectory::new();
    let source = temp.path().join("source");
    copy_tree(&fixture_root(), &source);
    fs::write(source.join("unsupported.dat"), b"must not disappear").unwrap();
    let cf = temp.path().join("blocked.cf");

    let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "bootstrap"])
        .arg(&source)
        .arg(&cf)
        .args(["--source-version", "2.20"])
        .env("PATH", "")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(report["errors"][0]["code"], "bootstrap_compile_failed");
    assert!(!cf.exists());
}

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_tree(&source_path, &target_path);
        } else {
            fs::copy(source_path, target_path).unwrap();
        }
    }
}
