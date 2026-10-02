//! A configuration of platform 8.5 as a `.cf` (Untru/ibcmd-rs#354): read in
//! XML 2.21 and written back for 8.5.1.1150, offline.
//!
//! The fixture is the one 8.5 configuration on record as a `.cf`:
//! `home_page/one_column_v85/input.cf`, the clean-room base with two common
//! forms on its home page, built from XML 2.20 by 8.5.1.1529 at
//! compatibility 8.3.27 and saved by it (`/DumpCfg`); beside it the
//! platform's own `Ext/HomePageWorkArea.xml`, dumped by 8.3.27.2214 after
//! loading that file (`_onecdec/make_home_page_fixtures.py`). No 2.21 dump of
//! it by 8.5 is on record. See `docs/evidence/cf-platform-8.5.md`.

mod common;

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use flate2::read::DeflateDecoder;
use ibcmd_cf::archive::decode_packed_archive;
use ibcmd_core::{artifact::StorageProfileId, limits::ResourceLimits};
use serde_json::Value;

const CASE: &str = "home_page/one_column_v85";
const CONFIGURATION_ROW: &str = "ba46775a-8ecc-49a2-8dc9-f4173b708a73";

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "ibcmd-rs-cf-platform-8-5-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn ibcmd_rs(args: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(args)
        // nothing may be launched and no server reached
        .env("PATH", "")
        .output()
        .expect("run ibcmd-rs")
}

fn ok(run: &Output) -> Value {
    assert!(
        run.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    serde_json::from_slice(&run.stdout).unwrap()
}

/// `cf export --platform 8.5.1.1150`: XML 2.21.
fn export_v85(input: &Path, output: &Path) -> Value {
    ok(&ibcmd_rs(&[
        "cf".as_ref(),
        "export".as_ref(),
        input.as_os_str(),
        output.as_os_str(),
        "--platform".as_ref(),
        "8.5.1.1150".as_ref(),
    ]))
}

/// The records of a `.cf`, inflated, by name.
fn records(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let file = fs::File::open(path).unwrap();
    let limits = ResourceLimits::for_input_bytes(file.metadata().unwrap().len());
    let archive = decode_packed_archive(
        file,
        limits,
        StorageProfileId::parse("storage:test").unwrap(),
    )
    .unwrap();
    let (_, _, entries) = archive.into_parts();
    entries
        .into_iter()
        .map(|entry| {
            let (name, packed) = entry.into_parts();
            let mut plain = Vec::new();
            DeflateDecoder::new(packed.as_slice())
                .read_to_end(&mut plain)
                .unwrap();
            (name, plain)
        })
        .collect()
}

/// `ConfigDumpInfo.xml` without its `configVersion` values, which name the
/// generation of each row (`versions`), new on every save.
fn without_config_versions(bytes: &[u8]) -> String {
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(at) = rest.find("configVersion=\"") {
        let value = at + "configVersion=\"".len();
        out.push_str(&rest[..value]);
        let end = rest[value..].find('"').unwrap();
        rest = &rest[value + end..];
    }
    out.push_str(rest);
    out
}

/// The two trees file for file, `ConfigDumpInfo.xml` without generations.
fn assert_same_tree(expected: &Path, actual: &Path) {
    let (expected, actual) = (common::files(expected), common::files(actual));
    assert_eq!(
        expected.keys().collect::<Vec<_>>(),
        actual.keys().collect::<Vec<_>>()
    );
    for (path, bytes) in &expected {
        if path == "ConfigDumpInfo.xml" {
            assert_eq!(
                without_config_versions(bytes),
                without_config_versions(&actual[path]),
                "{path}"
            );
        } else {
            assert!(
                bytes == &actual[path],
                "{path}\n--- exported\n{}\n--- rebuilt\n{}",
                String::from_utf8_lossy(bytes),
                String::from_utf8_lossy(&actual[path])
            );
        }
    }
}

#[test]
fn cf_saved_by_8_5_exports_in_xml_2_21() {
    let scratch = Scratch::new("export");
    let tree = scratch.join("tree");
    let report = export_v85(&common::fixture(CASE).join("input.cf"), &tree);
    assert_eq!(report["source_version"], "2.21");
    // Every record but the three service rows is written as XML; the
    // Configuration row of an 8.5 save at compatibility 8.3.27 among them.
    let entries = report["export"]["storage"]["entries"].as_array().unwrap();
    let opaque = entries
        .iter()
        .filter(|entry| entry["disposition"] != "supported")
        .map(|entry| entry["logical_name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(opaque, ["root", "version", "versions"]);
    let files = common::files(&tree);
    assert_eq!(
        files.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "CommonForms/ФормаСлева.xml",
            "CommonForms/ФормаСлева/Ext/Form.xml",
            "CommonForms/ФормаСлева/Ext/Form/Module.bsl",
            "CommonForms/ФормаСправа.xml",
            "CommonForms/ФормаСправа/Ext/Form.xml",
            "CommonForms/ФормаСправа/Ext/Form/Module.bsl",
            "ConfigDumpInfo.xml",
            "Configuration.xml",
            "Ext/ClientApplicationInterface.xml",
            "Ext/HomePageWorkArea.xml",
            "Languages/Русский.xml",
        ]
    );
    // The platform's file, dumped by 8.3.27 (2.20); 2.21 differs in the
    // version attribute alone.
    let native = fs::read_to_string(common::fixture(CASE).join("HomePageWorkArea.xml")).unwrap();
    assert_eq!(
        String::from_utf8(files["Ext/HomePageWorkArea.xml"].clone()).unwrap(),
        native.replace("version=\"2.20\"", "version=\"2.21\"")
    );
    // The 8.5 tuple's own members, at the values 8.5.1.1529 stored for a tree
    // that names none of them.
    let configuration = String::from_utf8(files["Configuration.xml"].clone()).unwrap();
    for element in [
        "<CompatibilityMode>Version8_3_27</CompatibilityMode>",
        "<InterfaceCompatibilityMode>TaxiEnableVersion8_2</InterfaceCompatibilityMode>",
        "<ClientApplicationWindowsOpenVariant>OpenDataInDialogs</ClientApplicationWindowsOpenVariant>",
        "<Version85InterfaceMigrationMode>DontUse</Version85InterfaceMigrationMode>",
    ] {
        assert!(configuration.contains(element), "{element}");
    }
}

#[test]
fn xml_2_21_bootstraps_into_the_cf_8_5_saved() {
    let scratch = Scratch::new("bootstrap");
    let original = common::fixture(CASE).join("input.cf");
    let tree = scratch.join("tree");
    export_v85(&original, &tree);

    let built = scratch.join("built.cf");
    let report = ok(&ibcmd_rs(&[
        "cf".as_ref(),
        "bootstrap".as_ref(),
        "--base-free".as_ref(),
        "--platform".as_ref(),
        "8.5.1.1150".as_ref(),
        tree.as_os_str(),
        built.as_os_str(),
    ]));
    assert_eq!(report["source_version"], "2.21");
    assert_eq!(report["target_profile"], "platform-8.5.1.1150");
    assert_eq!(report["revision"], "format15");

    // (a) the built file exports to the same tree
    let rebuilt = scratch.join("rebuilt");
    export_v85(&built, &rebuilt);
    assert_same_tree(&tree, &rebuilt);

    // (b) the platform's records
    let (theirs, ours) = (records(&original), records(&built));
    assert_eq!(
        theirs.keys().collect::<Vec<_>>(),
        ours.keys().collect::<Vec<_>>()
    );
    // `version` lists the features as the platform's file does: none, at
    // compatibility 8.3.27, in the 8.3 format.
    assert_eq!(
        ours["version"],
        "\u{feff}{\r\n{216,0,\r\n{80327,0}\r\n}\r\n}".as_bytes()
    );
    assert_eq!(ours["version"], theirs["version"]);
    let differing = theirs
        .iter()
        .filter(|(name, bytes)| ours[*name] != **bytes)
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();
    // `versions`: one fresh generation uuid per row, by design; the same rows.
    // The Configuration row: the footer the platform writes on save and a
    // load from XML does not (see the evidence note).
    assert_eq!(differing, [CONFIGURATION_ROW, "versions"]);
    let names = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes)
            .split('"')
            .skip(1)
            .step_by(2)
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&ours["versions"]), names(&theirs["versions"]));
    let (ours_root, theirs_root) = (
        String::from_utf8_lossy(&ours[CONFIGURATION_ROW]).into_owned(),
        String::from_utf8_lossy(&theirs[CONFIGURATION_ROW]).into_owned(),
    );
    assert_eq!(
        ours_root.replace(
            "{0,\"\",\"\"}\r\n}\r\n}",
            "{1,\"\",\"\"},\r\n{-809843968}\r\n}\r\n}"
        ),
        theirs_root
    );
}

#[cfg(not(feature = "platform-oracle"))]
#[test]
fn base_free_bootstrap_ignores_research_entry_overrides() {
    let scratch = Scratch::new("research-overrides");
    let tree = scratch.join("tree");
    export_v85(&common::fixture(CASE).join("input.cf"), &tree);
    let built = scratch.join("built.cf");
    let run = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "bootstrap", "--base-free", "--platform", "8.5.1.1150"])
        .arg(&tree)
        .arg(&built)
        .env("PATH", "")
        .env(
            "IBCMD_RS_BASE_FREE_ENTRIES_FROM",
            scratch.join("missing.cf"),
        )
        .env(
            "IBCMD_RS_BASE_FREE_ENTRIES_ONLY",
            scratch.join("missing.txt"),
        )
        .env("IBCMD_RS_BASE_FREE_KEEP_OURS", "root")
        .output()
        .unwrap();
    ok(&run);
    let rebuilt = scratch.join("rebuilt");
    export_v85(&built, &rebuilt);
    assert_same_tree(&tree, &rebuilt);
}
