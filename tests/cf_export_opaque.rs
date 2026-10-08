//! Opt-in complete-image opaque accounting, without a platform or copied CF.

mod common;

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use ibcmd_cf::payload::{PayloadEncoding, encode_payload};
use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::{ObjectPath, PathSegment, PropertyPath};
use ibcmd_core::identity::{LogicalIdentity, ObjectUuid};
use ibcmd_core::limits::ResourceLimits;
use ibcmd_core::model::{
    CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, MetadataKind,
};
use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
use ibcmd_core::validate::validate_configuration;
use ibcmd_rs::compiler::graph::{ObjectStorageRoute, build_bootstrap_graph};
use ibcmd_rs::compiler::identity::collect_bootstrap_identities;
use ibcmd_rs::compiler::root::{
    ConfigurationBodyProperties, compile_configuration_body, compile_root,
};
use ibcmd_rs::compiler::version::{SpecialEntryProfile, compile_version};
use ibcmd_rs::profile_registry::load_bundled_profile_registry;
use ibcmd_v8::writer::{Format15Document, Format15Element, write_format15_to_vec};
use serde_json::Value;

const CONFIG: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const OPAQUE: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb.0";
const SECOND_OPAQUE: &str = "cccccccc-cccc-4ccc-8ccc-cccccccccccc.0";
const FIRST_VERSION: &str = "11111111-1111-4111-8111-111111111111";
const NEXT_VERSION: &str = "22222222-2222-4222-8222-222222222222";

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ibcmd-cf-opaque-{}-{nonce}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn packed(name: &str, text: &str) -> Format15Element {
    let payload = encode_payload(
        PayloadEncoding::RawDeflate,
        text.as_bytes(),
        ResourceLimits::default(),
    )
    .unwrap();
    Format15Element::named(name, Some(payload))
}

/// Use the production clean-room compiler's complete seven-section root, not
/// a permissive fallback header that cannot own a ConfigDumpInfo inventory.
fn canonical_configuration_entries(name: &str) -> Vec<Format15Element> {
    let profile_id = ProfileId::parse("platform-8.3.27.1989").unwrap();
    let path = ObjectPath::new(vec![PathSegment::name("Configuration").unwrap()]).unwrap();
    let configuration = CanonicalConfiguration::new(vec![
        CanonicalObject::new(CanonicalObjectParts::new(
            LogicalIdentity::new(ObjectUuid::parse(CONFIG).unwrap(), path.clone()),
            MetadataKind::new("Configuration").unwrap(),
            SourceProvenance::new(
                profile_id.clone(),
                CanonicalAnchor::new(path, PropertyPath::root()),
            ),
        ))
        .unwrap(),
    ])
    .unwrap();
    let validated = validate_configuration(&configuration).unwrap();
    let identities = collect_bootstrap_identities(&validated).unwrap();
    let graph = build_bootstrap_graph(
        &identities,
        profile_id.clone(),
        vec![ObjectStorageRoute::new(identities.configuration_uuid(), vec![]).unwrap()],
    )
    .unwrap();
    let profiles = load_bundled_profile_registry().unwrap();
    let profile = SpecialEntryProfile::from_effective(profiles.get(&profile_id).unwrap()).unwrap();
    let properties = ConfigurationBodyProperties::minimal(name, profile.compatibility());
    [
        compile_root(&graph, &profile).unwrap(),
        compile_version(&graph, &profile).unwrap(),
        compile_configuration_body(&identities, &graph, &profile, &properties).unwrap(),
    ]
    .into_iter()
    .map(|entry| {
        Format15Element::named(
            entry.target().key().as_str(),
            Some(entry.outcome().compiled_payload().unwrap().bytes().to_vec()),
        )
    })
    .collect()
}

fn configuration(name: &str) -> Format15Element {
    canonical_configuration_entries(name).pop().unwrap()
}

fn archive(name: &str, opaque: bool) -> Vec<u8> {
    let mut elements = vec![configuration(name)];
    if opaque {
        elements.push(packed(OPAQUE, "unrecognized clean-room body"));
    }
    write_format15_to_vec(&Format15Document::new(7, elements)).unwrap()
}

fn versioned_archive(
    name: &str,
    generation: u32,
    config_version: &str,
    body_version: &str,
) -> Vec<u8> {
    let versions = format!(
        "{{1,4,\"\",00000000-0000-0000-0000-{generation:012},\"{CONFIG}\",{config_version},\"{OPAQUE}\",{FIRST_VERSION},\"{SECOND_OPAQUE}\",{body_version}}}"
    );
    let mut elements = canonical_configuration_entries(name);
    elements.extend([
        packed(OPAQUE, "unrecognized unchanged clean-room body"),
        packed(
            SECOND_OPAQUE,
            &format!("unrecognized clean-room body {body_version}"),
        ),
        packed("versions", &versions),
    ]);
    write_format15_to_vec(&Format15Document::new(7, elements)).unwrap()
}

fn run(input: &Path, tree: &Path, profile: &str, flags: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "export"])
        .arg(input)
        .arg(tree)
        .args(["--source-version", profile])
        .args(flags)
        .env("PATH", "")
        .output()
        .unwrap()
}

fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], true, "{report}");
    report
}

fn refused(output: Output, expected_name: &str) -> Value {
    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "a refusal must not emit a success report"
    );
    let report: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(report["command"], "export");
    assert_eq!(report["ok"], false, "{report}");
    let storage = &report["export"]["storage"];
    let count = storage["opaque"].as_u64().unwrap();
    assert!(count > 0, "{report}");
    let entries = storage["entries"].as_array().unwrap();
    let entry = entries
        .iter()
        .find(|entry| entry["logical_name"] == expected_name)
        .unwrap();
    assert_eq!(entry["disposition"], "opaque", "{entry}");
    assert!(!entry["message"].as_str().unwrap().is_empty(), "{entry}");
    assert!(
        report["errors"].as_array().unwrap().iter().any(|error| {
            error["code"] == "opaque_export_refused"
                && error["message"]
                    .as_str()
                    .unwrap()
                    .contains(&format!("{count} opaque"))
        }),
        "{report}"
    );
    report
}

fn indexed(input: &Path, tree: &Path, profile: &str) {
    success(run(input, tree, profile, &["--index"]));
    assert!(tree.join(".ibcmd/index.tsv").is_file());
    fs::create_dir(tree.join(".git")).unwrap();
    fs::write(
        tree.join(".git/config"),
        "retained user repository metadata",
    )
    .unwrap();
    fs::create_dir(tree.join(".user")).unwrap();
    fs::write(tree.join(".user/notes.txt"), "retained unindexed user file").unwrap();
}

#[test]
fn known_only_strict_export_publishes_an_index_in_both_profiles() {
    let temp = TempDirectory::new();
    let input = temp.0.join("known.cf");
    fs::write(&input, archive("OfflineDemo", false)).unwrap();
    for profile in ["2.20", "2.21"] {
        let tree = temp.0.join(profile);
        let report = success(run(
            &input,
            &tree,
            profile,
            &["--fail-on-opaque", "--index"],
        ));
        assert_eq!(report["export"]["storage"]["opaque"], 0);
        assert_eq!(report["export"]["storage"]["failed"], 0);
        assert_eq!(report["export"]["storage"]["supported"], 1);
        assert!(
            fs::read_to_string(tree.join("Configuration.xml"))
                .unwrap()
                .contains("<Name>OfflineDemo</Name>")
        );
        assert!(tree.join(".ibcmd/index.tsv").is_file());
    }
}

#[test]
fn default_opaque_export_succeeds_but_strict_export_retains_reasons_and_fails() {
    let temp = TempDirectory::new();
    let input = temp.0.join("opaque.cf");
    fs::write(&input, archive("OfflineDemo", true)).unwrap();
    let report = success(run(&input, &temp.0.join("default"), "2.20", &[]));
    assert_eq!(report["export"]["storage"]["opaque"], 1);
    let strict = refused(
        run(
            &input,
            &temp.0.join("strict"),
            "2.20",
            &["--fail-on-opaque"],
        ),
        OPAQUE,
    );
    assert_eq!(strict["export"]["storage"]["opaque"], 1);
    assert_eq!(strict["export"]["storage"]["logical_entries"], 2);
}

#[test]
fn strict_refusal_does_not_publish_a_new_index() {
    let temp = TempDirectory::new();
    let input = temp.0.join("opaque.cf");
    let tree = temp.0.join("tree");
    fs::write(&input, archive("OfflineDemo", true)).unwrap();
    refused(
        run(&input, &tree, "2.21", &["--index", "--fail-on-opaque"]),
        OPAQUE,
    );
    assert!(!tree.join(".ibcmd/index.tsv").exists());
    assert!(tree.join("Configuration.xml").is_file());
}

#[test]
fn same_file_update_cannot_skip_strict_accounting_or_change_the_existing_tree() {
    let temp = TempDirectory::new();
    let input = temp.0.join("opaque.cf");
    let tree = temp.0.join("tree");
    fs::write(&input, archive("OfflineDemo", true)).unwrap();
    indexed(&input, &tree, "2.20");
    let before = common::files(&tree);
    let default = success(run(&input, &tree, "2.20", &["--update"]));
    assert_eq!(default["update"]["mode"], "unchanged");
    assert!(default["export"].is_null());
    let strict = refused(
        run(&input, &tree, "2.20", &["--update", "--fail-on-opaque"]),
        OPAQUE,
    );
    assert_eq!(strict["update"]["mode"], "full");
    assert_eq!(strict["export"]["storage"]["logical_entries"], 2);
    assert_eq!(
        common::files(&tree),
        before,
        "including index, .git and unindexed files"
    );
}

#[test]
fn resaved_incremental_and_full_updates_all_check_the_complete_current_image() {
    let temp = TempDirectory::new();
    let base = temp.0.join("base.cf");
    fs::write(
        &base,
        versioned_archive("OfflineDemo", 1, FIRST_VERSION, FIRST_VERSION),
    )
    .unwrap();
    for (scenario, expected_mode, name, config_version, body_version) in [
        (
            "resaved",
            "unchanged",
            "OfflineDemo",
            FIRST_VERSION,
            FIRST_VERSION,
        ),
        (
            "incremental",
            "incremental",
            "OfflineDemo",
            FIRST_VERSION,
            NEXT_VERSION,
        ),
        ("full", "full", "NewName", NEXT_VERSION, FIRST_VERSION),
    ] {
        let input = temp.0.join(format!("{scenario}.cf"));
        fs::write(
            &input,
            versioned_archive(name, 2, config_version, body_version),
        )
        .unwrap();
        assert_ne!(fs::read(&base).unwrap(), fs::read(&input).unwrap());
        let control = temp.0.join(format!("{scenario}-control"));
        indexed(&base, &control, "2.21");
        assert!(
            fs::read_to_string(control.join("Configuration.xml"))
                .unwrap()
                .contains("<Name>OfflineDemo</Name>"),
            "the clean-room base must expose its canonical Configuration owner"
        );
        let default = success(run(&input, &control, "2.21", &["--update"]));
        assert_eq!(
            default["update"]["mode"], expected_mode,
            "{scenario}: {default}"
        );
        if scenario == "incremental" {
            assert_eq!(
                default["update"]["changed_entries"],
                serde_json::json!([SECOND_OPAQUE])
            );
        }
        let tree = temp.0.join(format!("{scenario}-strict"));
        indexed(&base, &tree, "2.21");
        let before = common::files(&tree);
        let strict = refused(
            run(&input, &tree, "2.21", &["--update", "--fail-on-opaque"]),
            OPAQUE,
        );
        assert_eq!(strict["update"]["mode"], "full");
        assert_eq!(strict["export"]["storage"]["physical_entries"], 6);
        assert_eq!(strict["export"]["storage"]["failed"], 0);
        assert!(
            strict["export"]["storage"]["entries"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["logical_name"] == SECOND_OPAQUE
                    && entry["disposition"] == "opaque")
        );
        assert_eq!(
            common::files(&tree),
            before,
            "{scenario}: original files/index changed"
        );
    }
}

#[test]
fn strict_successful_update_replaces_metadata_and_moves_the_index() {
    let temp = TempDirectory::new();
    let base = temp.0.join("base.cf");
    let input = temp.0.join("new.cf");
    let tree = temp.0.join("tree");
    fs::write(&base, archive("OfflineDemo", false)).unwrap();
    fs::write(&input, archive("NewName", false)).unwrap();
    indexed(&base, &tree, "2.21");
    let old_index = fs::read(tree.join(".ibcmd/index.tsv")).unwrap();
    let report = success(run(
        &input,
        &tree,
        "2.21",
        &["--update", "--fail-on-opaque"],
    ));
    assert_eq!(report["update"]["mode"], "full");
    assert_eq!(report["export"]["storage"]["opaque"], 0);
    assert!(
        fs::read_to_string(tree.join("Configuration.xml"))
            .unwrap()
            .contains("<Name>NewName</Name>")
    );
    assert_ne!(fs::read(tree.join(".ibcmd/index.tsv")).unwrap(), old_index);
    assert_eq!(
        fs::read_to_string(tree.join(".user/notes.txt")).unwrap(),
        "retained unindexed user file"
    );
    assert_eq!(
        fs::read_to_string(tree.join(".git/config")).unwrap(),
        "retained user repository metadata"
    );
    let again = success(run(&input, &tree, "2.21", &["--update"]));
    assert_eq!(again["update"]["mode"], "unchanged");
    assert!(again["export"].is_null());
}
