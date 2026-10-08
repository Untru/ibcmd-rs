//! Generated clean-room package admission through both actual public routes.
//! No native executable, foreign fixture, database or ready archive is used.
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

use serde_json::Value;

const MD: &str = "http://v8.1c.ru/8.3/MDClasses";
const ROOT_UUID: &str = "10000000-0000-4000-8000-000000000435";

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "ibcmd-package-intent-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn source(&self, file: &str, xml: &str) -> PathBuf {
        let root = self.0.join("source");
        fs::create_dir(&root).unwrap();
        fs::write(root.join(file), xml).unwrap();
        root
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn metadata(kind: &str, properties: &str) -> String {
    format!(
        r#"<MetaDataObject xmlns="{MD}" version="2.20"><{kind} uuid="{ROOT_UUID}"><Properties><Name>GeneratedPackage</Name><Synonym/><Comment/>{properties}</Properties><ChildObjects/></{kind}></MetaDataObject>"#
    )
}

fn configuration() -> String {
    metadata(
        "Configuration",
        "<DefaultRunMode>ManagedApplication</DefaultRunMode><ScriptVariant>English</ScriptVariant><CompatibilityMode>Version8_3_24</CompatibilityMode>",
    )
}

fn bootstrap(source: &Path, output: &Path, base_free: bool, storage: &str) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"));
    command.args(["cf", "bootstrap"]);
    if base_free {
        command.args(["--base-free", "--platform", "8.3.27.2214"]);
    } else {
        command.args(["--source-version", "2.20"]);
    }
    command
        .arg(source)
        .arg(output)
        .args(["--storage-version", storage])
        .env("PATH", "")
        .output()
        .unwrap()
}

fn refusal(result: Output, output: &Path, code: &str, address: &str) {
    assert!(!result.status.success(), "unexpected bootstrap success");
    assert!(!output.exists(), "refused package was published");
    let report: Value = serde_json::from_slice(&result.stderr)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(&result.stderr)));
    assert_eq!(report["command"], "bootstrap");
    assert_eq!(report["ok"], false);
    assert_eq!(report["storage_entries"], 0);
    assert!(report["publication"].is_null());
    assert_eq!(report["errors"][0]["code"], code);
    assert!(
        report["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains(address)
    );
}

#[test]
fn extension_and_both_external_families_refuse_before_either_cf_compiler() {
    for base_free in [false, true] {
        for (kind, properties, address, suffix, storage) in [
            (
                "Configuration",
                "<ObjectBelonging>Adopted</ObjectBelonging><ConfigurationExtensionPurpose>Customization</ConfigurationExtensionPurpose><Version>1.0.0</Version>",
                "Extension",
                "cfe",
                "6",
            ),
            (
                "ExternalDataProcessor",
                "",
                "ExternalDataProcessor",
                "epf",
                "7",
            ),
            ("ExternalReport", "", "ExternalReport", "erf", "7"),
        ] {
            let scratch = Scratch::new();
            // The filename provides no package identity, even when it lies.
            let source = scratch.source("Configuration.xml", &metadata(kind, properties));
            for requested_suffix in [suffix, "cf"] {
                let output = scratch.0.join(format!("blocked.{requested_suffix}"));
                refusal(
                    bootstrap(&source, &output, base_free, storage),
                    &output,
                    "bootstrap_package_not_supported",
                    address,
                );
            }
        }
    }
}

#[test]
fn renamed_prefixed_extension_and_explicit_xml_file_are_addressed() {
    let xml = format!(
        r#"<a:MetaDataObject xmlns:a="{MD}" version="2.20"><b:Configuration xmlns:b="{MD}" uuid="{ROOT_UUID}"><b:Properties><c:NamePrefix xmlns:c="{MD}">Own_</c:NamePrefix></b:Properties></b:Configuration></a:MetaDataObject>"#
    );
    for base_free in [false, true] {
        let scratch = Scratch::new();
        let source = scratch.source("Renamed.xml", &xml);
        for input in [source.clone(), source.join("Renamed.xml")] {
            let output = scratch.0.join("blocked.cf");
            refusal(
                bootstrap(&input, &output, base_free, "5"),
                &output,
                "bootstrap_package_not_supported",
                "Renamed.xml",
            );
        }
    }
}

#[test]
fn root_and_extension_property_namespace_spoofing_never_select_cf() {
    for base_free in [false, true] {
        for xml in [
            metadata("ExternalReport", "").replace(MD, "urn:foreign"),
            format!(
                r#"<MetaDataObject xmlns="{MD}"><ExternalDataProcessor xmlns="urn:foreign"/></MetaDataObject>"#
            ),
            metadata(
                "Configuration",
                "<NamePrefix xmlns=\"urn:foreign\">Spoof_</NamePrefix>",
            ),
        ] {
            let scratch = Scratch::new();
            let source = scratch.source("Configuration.xml", &xml);
            let output = scratch.0.join("blocked.cf");
            refusal(
                bootstrap(&source, &output, base_free, "5"),
                &output,
                "bootstrap_package_invalid",
                "namespace",
            );
        }
    }
}

#[test]
fn multiple_package_roots_are_not_resolved_by_configuration_filename() {
    for base_free in [false, true] {
        let scratch = Scratch::new();
        let source = scratch.source("Configuration.xml", &configuration());
        fs::write(source.join("Other.xml"), metadata("ExternalReport", "")).unwrap();
        let output = scratch.0.join("blocked.cf");
        refusal(
            bootstrap(&source, &output, base_free, "5"),
            &output,
            "bootstrap_package_invalid",
            "ambiguous package roots",
        );
    }
}

#[test]
fn ordinary_configuration_cannot_be_published_under_external_suffix() {
    for base_free in [false, true] {
        for suffix in ["cfe", "epf", "erf"] {
            let scratch = Scratch::new();
            let source = scratch.source("Configuration.xml", &configuration());
            let output = scratch.0.join(format!("blocked.{suffix}"));
            refusal(
                bootstrap(&source, &output, base_free, "5"),
                &output,
                "bootstrap_package_invalid",
                "XML declares Configuration",
            );
        }
    }
}

#[test]
fn configuration_file_is_identified_without_treating_it_as_a_source_directory() {
    for base_free in [false, true] {
        let scratch = Scratch::new();
        let source = scratch.source("Configuration.xml", &configuration());
        let output = scratch.0.join("blocked.cf");
        refusal(
            bootstrap(&source.join("Configuration.xml"), &output, base_free, "5"),
            &output,
            "bootstrap_package_root_file_not_supported",
            "complete source directory",
        );
    }
}

#[test]
fn generated_ordinary_cf_still_publishes_and_has_the_same_service_inventory() {
    for base_free in [false, true] {
        let scratch = Scratch::new();
        let source = scratch.source("Configuration.xml", &configuration());
        let output = scratch.0.join("ordinary.cf");
        let result = bootstrap(&source, &output, base_free, "5");
        assert!(
            result.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr),
        );
        let report: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["ok"], true);
        assert_eq!(report["storage_entries"], 4);
        assert_eq!(report["storage_version"], 5);
        let inspect = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
            .args(["cf", "inspect"])
            .arg(&output)
            .args(["--profile", "storage:mssql-config-configsave"])
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(
            inspect.status.success(),
            "{}",
            String::from_utf8_lossy(&inspect.stderr)
        );
        let report: Value = serde_json::from_slice(&inspect.stdout).unwrap();
        let names = report["elements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names, [ROOT_UUID, "root", "version", "versions"]);
    }
}
