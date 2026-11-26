//! CFE bootstrap and strict export without a base file, database or platform.
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn command(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(args)
        .env("PATH", "")
        .output()
        .unwrap()
}
fn report(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["ok"], true);
    value
}
#[test]
fn source_only_cfe_roundtrips_both_dialects_and_keeps_package_identity() {
    let scratch = Scratch(std::env::temp_dir().join(format!(
        "ibcmd-cfe-cli-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    )));
    fs::create_dir(&scratch.0).unwrap();
    for (platform, fixture) in [("8.3.27", "native-8.3.27"), ("8.5.1.1150", "native-8.5.1")] {
        let source = scratch.0.join(fixture);
        fs::create_dir_all(source.join("Languages")).unwrap();
        let oracle = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/native-evidence/extension-empty")
            .join(fixture);
        fs::copy(
            oracle.join("Configuration.xml"),
            source.join("Configuration.xml"),
        )
        .unwrap();
        fs::copy(
            oracle.join("Language.xml"),
            source.join("Languages/Русский.xml"),
        )
        .unwrap();
        let mut outputs = Vec::new();
        for base_free in [false, true] {
            let output = scratch.0.join(format!("{fixture}-{base_free}.cfe"));
            let mut args = vec![
                "cf".as_ref(),
                "bootstrap".as_ref(),
                source.as_os_str(),
                output.as_os_str(),
                "--platform".as_ref(),
                platform.as_ref(),
            ];
            if base_free {
                args.push("--base-free".as_ref());
            }
            let built = report(command(&args));
            assert_eq!(built["storage_entries"], 3);
            assert_eq!(built["source_files"], 2);
            assert_eq!(built["metadata_files"], 2);
            assert_eq!(built["asset_files"], 0);
            assert_eq!(built["revision"], "format15");
            let inspect = report(command(&[
                "cf".as_ref(),
                "inspect".as_ref(),
                output.as_os_str(),
            ]));
            let names = inspect["elements"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["name"].as_str().unwrap())
                .collect::<Vec<_>>();
            assert!(names.contains(&"configinfo"));
            for name in ["root", "version", "versions"] {
                assert!(!names.contains(&name));
            }
            let back = scratch.0.join(format!("back-{fixture}-{base_free}"));
            let exported = report(command(&[
                "cf".as_ref(),
                "export".as_ref(),
                output.as_os_str(),
                back.as_os_str(),
                "--platform".as_ref(),
                platform.as_ref(),
                "--fail-on-opaque".as_ref(),
            ]));
            assert_eq!(exported["export"]["storage"]["failed"], 0);
            assert_eq!(exported["export"]["storage"]["opaque"], 0);
            for p in ["Configuration.xml", "Languages/Русский.xml"] {
                assert_eq!(
                    fs::read(back.join(p)).unwrap(),
                    fs::read(source.join(p)).unwrap()
                );
            }
            assert!(!back.join("ConfigDumpInfo.xml").exists());
            outputs.push(fs::read(output).unwrap());
        }
        assert_eq!(
            outputs[0], outputs[1],
            "the two source-only routes must compile the same CFE"
        );
    }
}
