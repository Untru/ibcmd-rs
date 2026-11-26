use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use ibcmd_cf::payload::{PayloadEncoding, encode_payload};
use ibcmd_core::limits::ResourceLimits;
use ibcmd_v8::writer::{Format15Document, Format15Element, write_format15_to_vec};
use serde_json::Value;

struct TempFile(PathBuf);

impl TempFile {
    fn new(label: &str, bytes: &[u8]) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ibcmd-rs-cf-cli-{label}-{}-{nonce}.cf",
            std::process::id()
        ));
        fs::write(&path, bytes).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn valid_archive() -> Vec<u8> {
    let packed = encode_payload(
        PayloadEncoding::RawDeflate,
        b"standalone CF CLI",
        ResourceLimits::default(),
    )
    .unwrap();
    write_format15_to_vec(&Format15Document::new(
        7,
        vec![Format15Element::named("root", Some(packed))],
    ))
    .unwrap()
}

#[test]
fn inspect_succeeds_with_an_empty_path() {
    let archive = TempFile::new("valid", &valid_archive());
    let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "inspect"])
        .arg(archive.path())
        .env("PATH", "")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], true);
    assert_eq!(report["command"], "inspect");
    assert_eq!(report["layout"]["revision"], "format15");
    assert_eq!(report["elements"][0]["name"], "root");
}

#[test]
fn corrupt_archive_has_nonzero_status_and_json_error() {
    let mut bytes = valid_archive();
    bytes[0..4].copy_from_slice(&0_u32.to_le_bytes());
    let archive = TempFile::new("corrupt", &bytes);
    let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "verify"])
        .arg(archive.path())
        .env("PATH", "")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(report["ok"], false);
    assert_eq!(report["command"], "verify");
    assert_eq!(report["errors"][0]["code"], "invalid_archive");
}
