//! Generated clean-room package admission through both actual public routes.
//! No native executable, foreign fixture, database or ready archive is used.
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

use ibcmd_rs::metadata_model::brace::{Brace, parse_row};
use ibcmd_xml::{
    XmlReader,
    metadata::{PackageIntent, inspect_package_intent},
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
    // Both compilers receive a complete, independently authored root. The
    // legacy base-free writer requires all seven declared section identities.
    let contained = [
        "9cd510cd-abfc-11d4-9434-004095e12fc7",
        "9fcd25a0-4822-11d4-9414-008048da11f9",
        "e3687481-0a87-462c-a166-9f34594f9bba",
        "9de14907-ec23-4a07-96f0-85521cb6b53b",
        "51f2d5d8-ea4d-4064-8892-82951750031e",
        "e68182ea-4237-4383-967f-90c1e3370bc7",
        "fb282519-d103-4dd3-bc12-cb271d631dfc",
    ]
    .iter()
    .enumerate()
    .map(|(index, class)| {
        format!(
            "<xr:ContainedObject><xr:ClassId>{class}</xr:ClassId><xr:ObjectId>20000000-0000-4000-8000-{:012}</xr:ObjectId></xr:ContainedObject>",
            index + 1,
        )
    })
    .collect::<String>();
    metadata(
        "Configuration",
        "<DefaultRunMode>ManagedApplication</DefaultRunMode><ScriptVariant>English</ScriptVariant><CompatibilityMode>Version8_3_24</CompatibilityMode>",
    )
    .replacen(
        "version=\"2.20\"",
        "xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" version=\"2.20\"",
        1,
    )
    .replacen(
        "<Properties>",
        &format!("<InternalInfo>{contained}</InternalInfo><Properties>"),
        1,
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
        r#"<a:MetaDataObject xmlns:a="{MD}" version="2.20"><b:Configuration xmlns:b="{MD}" uuid="{ROOT_UUID}"><b:Properties><c:NamePrefix xmlns:c="{MD}">Own_</c:NamePrefix><c:ConfigurationExtensionCompatibilityMode xmlns:c="{MD}">Version8_3_27</c:ConfigurationExtensionCompatibilityMode><c:ConfigurationExtensionPurpose xmlns:c="{MD}">Customization</c:ConfigurationExtensionPurpose></b:Properties></b:Configuration></a:MetaDataObject>"#
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
            metadata(
                "Configuration",
                "<ConfigurationExtensionCompatibilityMode xmlns=\"urn:foreign\">Version8_3_27</ConfigurationExtensionCompatibilityMode>",
            ),
            metadata(
                "Configuration",
                "<ConfigurationExtensionPurpose xmlns=\"urn:foreign\">Customization</ConfigurationExtensionPurpose>",
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

fn success(result: Output) -> Value {
    assert!(
        result.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr),
    );
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["ok"], true, "{report}");
    report
}

fn export(cf: &Path, output: &Path, version: &str) {
    let report = success(
        Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
            .args(["cf", "export"])
            .arg(cf)
            .arg(output)
            .args(["--source-version", version])
            .env("PATH", "")
            .output()
            .unwrap(),
    );
    assert_eq!(report["command"], "export");
    assert_eq!(report["export"]["storage"]["failed"], 0, "{report}");
}

fn stored_configuration_tuple(cf: &Path, output: &Path) -> Vec<Brace> {
    let report = success(
        Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
            .args(["cf", "extract"])
            .arg(cf)
            .arg(ROOT_UUID)
            .arg(output)
            .args(["--compression", "raw-deflate"])
            .env("PATH", "")
            .output()
            .unwrap(),
    );
    assert_eq!(report["command"], "extract");
    let row = parse_row(&fs::read(output.join("unpacked.bin")).unwrap()).unwrap();
    assert_eq!(row.at(&[0]).unwrap().as_atom(), Some("2"));
    assert_eq!(row.at(&[1, 0]).unwrap().as_atom(), Some(ROOT_UUID));
    assert_eq!(row.at(&[2]).unwrap().as_atom(), Some("7"));
    let tuple = row.at(&[3, 1, 1]).unwrap().as_list().unwrap();
    match tuple[0].as_atom().unwrap() {
        "67" => assert_eq!(tuple.len(), 60),
        "68" => assert_eq!(tuple.len(), 61),
        tag => panic!("unexpected ordinary generated root tuple {tag}"),
    }
    tuple.to_vec()
}

fn mobile_block(enabled: &[u32]) -> String {
    let entries = ibcmd_schema::configuration_mobile::FUNCTIONALITIES
        .iter()
        .map(|(id, name)| format!(
            "<app:functionality><app:functionality>{name}</app:functionality><app:use>{}</app:use></app:functionality>",
            if enabled.contains(id) { "true" } else { "false" },
        ))
        .collect::<String>();
    format!(
        "<UsedMobileApplicationFunctionalities xmlns:app=\"http://v8.1c.ru/8.2/managed-application/core\">{entries}</UsedMobileApplicationFunctionalities>"
    )
}

fn assert_mobile_tuple(tuple: &[Brace], enabled: &[u32]) {
    let table = tuple[53].as_list().unwrap();
    assert_eq!(table[0].as_atom(), Some("2"));
    let short = tuple[0].as_atom() == Some("67");
    let pairs = if short { 37 } else { 38 };
    assert_eq!(table[1].as_atom().unwrap().parse::<usize>().unwrap(), pairs);
    assert_eq!(table.len(), pairs + 3);
    assert_eq!(table[2 + 28].as_list().unwrap()[0].as_atom(), Some("32"));
    let mut actual_enabled = Vec::new();
    for (pair, (expected_id, _)) in table[2..2 + pairs]
        .iter()
        .zip(&ibcmd_schema::configuration_mobile::FUNCTIONALITIES)
    {
        let pair = pair.as_list().unwrap();
        assert_eq!(pair.len(), 2);
        let id = pair[0].as_atom().unwrap().parse::<u32>().unwrap();
        assert_eq!(id, *expected_id);
        match pair[1].as_atom().unwrap() {
            "0" => assert!(!enabled.contains(&id)),
            "1" => {
                assert!(enabled.contains(&id));
                actual_enabled.push(id);
            }
            flag => panic!("nonboolean stored mobile flag {flag}"),
        }
    }
    let tail = table[2 + pairs].as_atom().unwrap();
    if short && tail == "1" {
        actual_enabled.push(41);
    }
    assert_eq!(
        tail,
        if short && enabled.contains(&41) {
            "1"
        } else {
            "0"
        }
    );
    assert_eq!(actual_enabled, enabled);
}

#[test]
fn own_mobile_values_keep_exact_tuple_and_native_xml_through_both_compilers() {
    let all = ibcmd_schema::configuration_mobile::FUNCTIONALITIES
        .iter()
        .map(|(id, _)| *id)
        .collect::<Vec<_>>();
    for enabled in [vec![], vec![1, 18, 35, 41], all] {
        for base_free in [false, true] {
            let scratch = Scratch::new();
            let xml = configuration()
                .replace(
                    "<ScriptVariant>English</ScriptVariant>",
                    "<ScriptVariant>Russian</ScriptVariant>",
                )
                .replacen(
                    "<Comment/>",
                    &format!("<Comment/>{}", mobile_block(&enabled)),
                    1,
                );
            let source = scratch.source("Configuration.xml", &xml);
            let initial = scratch.0.join("mobile.cf");
            success(bootstrap(&source, &initial, base_free, "5"));
            assert_mobile_tuple(
                &stored_configuration_tuple(&initial, &scratch.0.join("mobile-tuple")),
                &enabled,
            );
            for version in ["2.20", "2.21"] {
                let native = scratch.0.join(format!("native-mobile-{version}"));
                export(&initial, &native, version);
                let expected = fs::read(native.join("Configuration.xml")).unwrap();
                let document = XmlReader::from_slice(&expected).unwrap();
                assert_eq!(
                    ibcmd_xml::metadata::parse_configuration_mobile_functionalities(&document)
                        .unwrap(),
                    Some(enabled.clone())
                );
                let expected_text = std::str::from_utf8(&expected).unwrap();
                for (name, id) in [
                    ("Location", 1),
                    ("Camera", 18),
                    ("NFC", 35),
                    ("TextToSpeech", 41),
                ] {
                    assert!(expected_text.contains(&format!("<app:functionality>{name}</app:functionality>\r\n\t\t\t\t\t<app:use>{}</app:use>", enabled.contains(&id))), "{expected_text}");
                }
                let rebuilt = scratch.0.join(format!("rebuilt-mobile-{version}.cf"));
                let mut command = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"));
                command.args(["cf", "bootstrap"]);
                if base_free {
                    command.arg("--base-free");
                }
                success(
                    command
                        .arg(&native)
                        .arg(&rebuilt)
                        .args(["--source-version", version, "--storage-version", "5"])
                        .env("PATH", "")
                        .output()
                        .unwrap(),
                );
                assert_mobile_tuple(
                    &stored_configuration_tuple(
                        &rebuilt,
                        &scratch.0.join(format!("rebuilt-mobile-tuple-{version}")),
                    ),
                    &enabled,
                );
                let returned = scratch.0.join(format!("returned-mobile-{version}"));
                export(&rebuilt, &returned, version);
                assert_eq!(
                    fs::read(returned.join("Configuration.xml")).unwrap(),
                    expected
                );
            }
        }
    }
}

#[test]
fn malformed_mobile_values_refuse_both_routes_before_fresh_or_existing_publication() {
    let valid = configuration().replacen(
        "<Comment/>",
        &format!("<Comment/>{}", mobile_block(&[1, 41])),
        1,
    );
    let first = "<app:functionality><app:functionality>Biometrics</app:functionality><app:use>false</app:use></app:functionality>";
    let second = "<app:functionality><app:functionality>Location</app:functionality><app:use>true</app:use></app:functionality>";
    let invalid = [
        valid.replacen(first, "", 1),
        valid.replacen(second, first, 1),
        valid.replacen(
            &(first.to_owned() + second),
            &(second.to_owned() + first),
            1,
        ),
        valid.replacen("Biometrics", "FutureFunctionality", 1),
        valid.replacen("<app:use>false</app:use>", "<app:use>0</app:use>", 1),
        valid.replacen("<app:use>false</app:use>", "<app:use/>", 1),
        valid.replacen(
            "<app:use>false</app:use>",
            "<app:use><app:future/></app:use>",
            1,
        ),
        valid.replacen(
            "<app:use>false</app:use>",
            "<app:use>false</app:use><app:future/>",
            1,
        ),
        valid.replacen("<app:use>", "<app:use unexpected=\"yes\">", 1),
        valid.replacen("<app:use>", "<app:use xmlns:app=\"urn:foreign\">", 1),
        valid.replace(
            "http://v8.1c.ru/8.2/managed-application/core",
            "urn:foreign",
        ),
        valid.replacen(
            "</Properties>",
            &format!("{}</Properties>", mobile_block(&[])),
            1,
        ),
    ];
    for xml in invalid {
        for base_free in [false, true] {
            for existing in [false, true] {
                let scratch = Scratch::new();
                let source = scratch.source("Configuration.xml", &xml);
                let output = scratch.0.join("blocked.cf");
                if existing {
                    fs::write(&output, b"original owned output").unwrap();
                }
                let result = bootstrap(&source, &output, base_free, "5");
                assert!(!result.status.success());
                let report: Value = serde_json::from_slice(&result.stderr).unwrap();
                assert_eq!(report["ok"], false);
                assert_eq!(report["storage_entries"], 0);
                assert!(report["publication"].is_null());
                let diagnostic = report["errors"].to_string();
                assert!(
                    diagnostic.contains("UsedMobileApplicationFunctionalities")
                        || diagnostic.contains("mobile functionality"),
                    "refused by an unrelated rule: {report}",
                );
                assert_eq!(
                    fs::read_to_string(source.join("Configuration.xml")).unwrap(),
                    xml
                );
                if existing {
                    assert_eq!(fs::read(&output).unwrap(), b"original owned output");
                } else {
                    assert!(!output.exists());
                }
            }
        }
    }
}

#[test]
fn mobile_namespace_aliases_reach_both_public_compilers() {
    let xml = configuration()
        .replacen(
            "<Comment/>",
            &format!("<Comment/>{}", mobile_block(&[1, 18, 35, 41])),
            1,
        )
        .replace("app:", "alias:")
        .replace("xmlns:app=", "xmlns:alias=");
    for base_free in [false, true] {
        let scratch = Scratch::new();
        let source = scratch.source("Configuration.xml", &xml);
        let output = scratch.0.join("aliases.cf");
        success(bootstrap(&source, &output, base_free, "5"));
        assert_mobile_tuple(
            &stored_configuration_tuple(&output, &scratch.0.join("aliases-tuple")),
            &[1, 18, 35, 41],
        );
    }
}

#[test]
fn ordinary_prefix_and_shared_compatibility_survive_public_native_rebuild() {
    // NamePrefix is owned by the ordinary root compiler, including nonempty
    // values. Native ordinary export also emits its shared compatibility field.
    // Exercise both actual compiler routes and both XML dialects without a
    // platform process or a prebuilt archive; do not erase these properties.
    for base_free in [false, true] {
        for prefix in ["", "Own_"] {
            let scratch = Scratch::new();
            // Use the measured native factory syntax in this new complete
            // roundtrip control. The original English admission fixture above
            // remains unchanged; syntax-coordinate coverage is separate.
            let xml = configuration().replace("<ScriptVariant>English</ScriptVariant>", "<ScriptVariant>Russian</ScriptVariant>").replacen(
                "<Comment/>",
                &format!("<Comment/><NamePrefix>{prefix}</NamePrefix><ConfigurationExtensionCompatibilityMode>Version8_3_24</ConfigurationExtensionCompatibilityMode>"),
                1,
            );
            let input = XmlReader::from_slice(xml.as_bytes()).unwrap();
            assert_eq!(
                inspect_package_intent(&input).unwrap(),
                Some(PackageIntent::Configuration)
            );
            let source = scratch.source("Configuration.xml", &xml);
            let initial = scratch.0.join("ordinary.cf");
            let report = success(bootstrap(&source, &initial, base_free, "5"));
            assert_eq!(report["storage_entries"], 4);
            let stored = stored_configuration_tuple(&initial, &scratch.0.join("stored-initial"));
            assert_eq!(stored[2].as_str(), Some(prefix));
            assert_eq!(stored[26].as_atom(), Some("80324"));
            assert_eq!(stored[43].as_atom(), Some("80324"));
            for version in ["2.20", "2.21"] {
                let native = scratch.0.join(format!("native-{version}"));
                export(&initial, &native, version);
                let expected = fs::read(native.join("Configuration.xml")).unwrap();
                let native_document = XmlReader::from_slice(&expected).unwrap();
                assert_eq!(
                    inspect_package_intent(&native_document).unwrap(),
                    Some(PackageIntent::Configuration)
                );
                let text = std::str::from_utf8(&expected).unwrap();
                let prefix_element = if prefix.is_empty() {
                    "<NamePrefix/>".to_owned()
                } else {
                    format!("<NamePrefix>{prefix}</NamePrefix>")
                };
                assert!(text.contains(&prefix_element), "{text}");
                // These are independent coordinates: the CF above still
                // contains authored 80324, while native XML of an older root
                // projects the reading platform's edition (proved tuples in
                // config_compat and the retained 8.3/8.5 export controls).
                let reading_edition = match version {
                    "2.20" => "Version8_3_27",
                    "2.21" => "Version8_5_1",
                    _ => unreachable!(),
                };
                assert!(text.contains(&format!("<ConfigurationExtensionCompatibilityMode>{reading_edition}</ConfigurationExtensionCompatibilityMode>")), "{text}");
                assert!(
                    text.contains("<CompatibilityMode>Version8_3_24</CompatibilityMode>"),
                    "{text}"
                );
                assert!(
                    text.contains("<ScriptVariant>Russian</ScriptVariant>"),
                    "{text}"
                );
                assert!(!text.contains("<ConfigurationExtensionPurpose>"));

                let rebuilt = scratch.0.join(format!("rebuilt-{version}.cf"));
                let mut command = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"));
                command.args(["cf", "bootstrap"]);
                if base_free {
                    command.arg("--base-free");
                }
                let report = success(
                    command
                        .arg(&native)
                        .arg(&rebuilt)
                        .args(["--source-version", version, "--storage-version", "5"])
                        .env("PATH", "")
                        .output()
                        .unwrap(),
                );
                assert_eq!(report["storage_entries"], 4);
                let stored = stored_configuration_tuple(
                    &rebuilt,
                    &scratch.0.join(format!("stored-rebuilt-{version}")),
                );
                assert_eq!(stored[2].as_str(), Some(prefix));
                assert_eq!(stored[26].as_atom(), Some("80324"));
                // Legacy {68} transports the source's projected shared value;
                // base-free {67} predates it and retains the own stored value.
                assert_eq!(
                    stored[43].as_atom(),
                    Some(if base_free {
                        "80324"
                    } else if version == "2.20" {
                        "80327"
                    } else {
                        "80501"
                    })
                );
                let returned = scratch.0.join(format!("returned-{version}"));
                export(&rebuilt, &returned, version);
                // Independently authored properties must survive the actual
                // native writer, package selection and selected compiler.
                assert_eq!(
                    fs::read(returned.join("Configuration.xml")).unwrap(),
                    expected
                );
            }
        }
    }
}

#[test]
fn extension_purpose_and_mapping_still_refuse_with_shared_ordinary_properties() {
    for base_free in [false, true] {
        for marker in [
            "<ConfigurationExtensionPurpose>Patch</ConfigurationExtensionPurpose>",
            "<ConfigurationExtensionPurpose>Customization</ConfigurationExtensionPurpose>",
            "<ConfigurationExtensionPurpose>AddOn</ConfigurationExtensionPurpose>",
            "<KeepMappingToExtendedConfigurationObjectsByIDs>false</KeepMappingToExtendedConfigurationObjectsByIDs>",
            "<ObjectBelonging>Adopted</ObjectBelonging>",
        ] {
            let scratch = Scratch::new();
            let xml = configuration().replacen("<Comment/>", &format!("<Comment/><NamePrefix>Own_</NamePrefix><ConfigurationExtensionCompatibilityMode>Version8_3_24</ConfigurationExtensionCompatibilityMode>{marker}"), 1);
            let source = scratch.source("Renamed.xml", &xml);
            let output = scratch.0.join("blocked.cf");
            refusal(
                bootstrap(&source, &output, base_free, "5"),
                &output,
                "bootstrap_package_not_supported",
                "Extension",
            );
        }
    }
}

// Independently authored values of the old-layout XML 2.21 projection. Do not
// build this fixture from the production policy being checked.
fn older_v85_configuration() -> String {
    let defaults = "<AuxiliaryReportForm/><AuxiliaryReportVariantForm/><AuxiliaryReportSettingsForm/><AuxiliaryDynamicListSettingsForm/><AuxiliaryDataHistoryChangeHistoryForm/><AuxiliaryDataHistoryVersionDataForm/><AuxiliaryDataHistoryVersionDifferencesForm/><AuxiliaryCollaborationSystemUsersChoiceForm/><MainClientApplicationWindowInterfaceVariant>NavigationLeft</MainClientApplicationWindowInterfaceVariant><ClientApplicationTheme>Auto</ClientApplicationTheme><ClientApplicationWindowsOpenVariant>OpenDataInDialogs</ClientApplicationWindowsOpenVariant><Caption/><ShortCaption/><Version85InterfaceMigrationMode>DontUse</Version85InterfaceMigrationMode>";
    configuration()
        .replace("version=\"2.20\"", "version=\"2.21\"")
        .replace(
            "<ScriptVariant>English</ScriptVariant>",
            "<ScriptVariant>Russian</ScriptVariant>",
        )
        .replace("<Comment/>", &format!("<Comment/>{defaults}"))
}

fn older_v85_bootstrap(source: &Path, output: &Path, base_free: bool, version: &str) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"));
    command.args(["cf", "bootstrap"]);
    if base_free {
        command.arg("--base-free");
    }
    command
        .arg(source)
        .arg(output)
        .args(["--source-version", version, "--storage-version", "5"])
        .env("PATH", "")
        .output()
        .unwrap()
}

#[test]
fn older_v85_projected_defaults_keep_complete_native_xml_through_both_routes() {
    for base_free in [false, true] {
        for alias in [false, true] {
            let scratch = Scratch::new();
            let xml = if alias {
                older_v85_configuration()
                    .replace("<Caption/>", &format!("<own:Caption xmlns:own=\"{MD}\"/>"))
            } else {
                older_v85_configuration()
            };
            let source = scratch.source("Configuration.xml", &xml);
            let first = scratch.0.join("first.cf");
            let report = success(older_v85_bootstrap(&source, &first, base_free, "2.21"));
            assert_eq!(report["target_profile"], "platform-8.3.27.1989");
            let tuple = stored_configuration_tuple(&first, &scratch.0.join("first-tuple"));
            assert_eq!(
                tuple[0].as_atom(),
                Some(if base_free { "67" } else { "68" })
            );
            assert_mobile_tuple(&tuple, &[]);
            let native = scratch.0.join("native");
            export(&first, &native, "2.21");
            let expected = fs::read(native.join("Configuration.xml")).unwrap();
            let expected_text = std::str::from_utf8(&expected).unwrap();
            for (name, value) in [
                ("AuxiliaryReportForm", ""),
                ("AuxiliaryReportVariantForm", ""),
                ("AuxiliaryReportSettingsForm", ""),
                ("AuxiliaryDynamicListSettingsForm", ""),
                ("AuxiliaryDataHistoryChangeHistoryForm", ""),
                ("AuxiliaryDataHistoryVersionDataForm", ""),
                ("AuxiliaryDataHistoryVersionDifferencesForm", ""),
                ("AuxiliaryCollaborationSystemUsersChoiceForm", ""),
                (
                    "MainClientApplicationWindowInterfaceVariant",
                    "NavigationLeft",
                ),
                ("ClientApplicationTheme", "Auto"),
                ("ClientApplicationWindowsOpenVariant", "OpenDataInDialogs"),
                ("Caption", ""),
                ("ShortCaption", ""),
                ("Version85InterfaceMigrationMode", "DontUse"),
            ] {
                let element = if value.is_empty() {
                    format!("<{name}/>")
                } else {
                    format!("<{name}>{value}</{name}>")
                };
                assert!(
                    expected_text.contains(&element),
                    "{element}: {expected_text}"
                );
            }
            assert!(expected_text.contains("<AuxiliaryReportForm/>"));
            assert!(expected_text.contains("<ClientApplicationWindowsOpenVariant>OpenDataInDialogs</ClientApplicationWindowsOpenVariant>"));
            assert!(expected_text.contains(
                "<Version85InterfaceMigrationMode>DontUse</Version85InterfaceMigrationMode>"
            ));
            let second = scratch.0.join("second.cf");
            success(older_v85_bootstrap(&native, &second, base_free, "2.21"));
            let returned = scratch.0.join("returned");
            export(&second, &returned, "2.21");
            assert_eq!(
                fs::read(returned.join("Configuration.xml")).unwrap(),
                expected
            );
            assert_mobile_tuple(
                &stored_configuration_tuple(&second, &scratch.0.join("second-tuple")),
                &[],
            );
        }
    }
}

#[test]
fn older_v85_nondefault_and_malformed_values_refuse_before_any_publication() {
    let valid = older_v85_configuration();
    let mut malformed = Vec::new();
    for name in [
        "AuxiliaryReportForm",
        "AuxiliaryReportVariantForm",
        "AuxiliaryReportSettingsForm",
        "AuxiliaryDynamicListSettingsForm",
        "AuxiliaryDataHistoryChangeHistoryForm",
        "AuxiliaryDataHistoryVersionDataForm",
        "AuxiliaryDataHistoryVersionDifferencesForm",
        "AuxiliaryCollaborationSystemUsersChoiceForm",
        "Caption",
        "ShortCaption",
    ] {
        let empty = format!("<{name}/>");
        for value in [
            format!("<{name}>AuthoredValue</{name}>"),
            format!("<{name}><Value/></{name}>"),
            format!("<{name} future=\"true\"/>"),
            format!("<{name} xmlns=\"urn:foreign\"/>"),
            format!("<{name}/><{name}/>"),
        ] {
            malformed.push(valid.replacen(&empty, &value, 1));
        }
    }
    for (name, old, new) in [
        (
            "MainClientApplicationWindowInterfaceVariant",
            "NavigationLeft",
            "NavigationTop",
        ),
        ("ClientApplicationTheme", "Auto", "Dark"),
        (
            "ClientApplicationWindowsOpenVariant",
            "OpenDataInDialogs",
            "OpenDataInTabs",
        ),
        ("Version85InterfaceMigrationMode", "DontUse", "Use"),
    ] {
        let original = format!("<{name}>{old}</{name}>");
        for value in [
            format!("<{name}>{new}</{name}>"),
            format!("<{name}/>"),
            format!("<{name} future=\"true\">{old}</{name}>"),
            format!("<{name} xmlns=\"urn:foreign\">{old}</{name}>"),
            format!("{original}{original}"),
        ] {
            malformed.push(valid.replacen(&original, &value, 1));
        }
    }
    malformed.push(valid.replacen("<Properties>", "<Properties future=\"true\">", 1));
    for base_free in [false, true] {
        for xml in &malformed {
            let scratch = Scratch::new();
            let source = scratch.source("Configuration.xml", xml);
            for existing in [false, true] {
                let output = scratch
                    .0
                    .join(if existing { "existing.cf" } else { "fresh.cf" });
                let previous = b"exact previous output: never replace on refusal";
                if existing {
                    fs::write(&output, previous).unwrap();
                }
                let result = older_v85_bootstrap(&source, &output, base_free, "2.21");
                assert!(!result.status.success(), "admitted: {xml}");
                let report: Value = serde_json::from_slice(&result.stderr).unwrap();
                assert_eq!(report["ok"], false, "{report}");
                assert!(report["publication"].is_null(), "{report}");
                assert!(
                    report["errors"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|error| error["message"].as_str().unwrap().contains("Configuration")),
                    "{report}"
                );
                if existing {
                    assert_eq!(fs::read(&output).unwrap(), previous);
                } else {
                    assert!(!output.exists());
                }
            }
        }
    }
}

#[test]
fn older_v85_vocabulary_does_not_change_the_xml_2_20_contract() {
    let xml = older_v85_configuration().replace("version=\"2.21\"", "version=\"2.20\"");
    for base_free in [false, true] {
        let scratch = Scratch::new();
        let source = scratch.source("Configuration.xml", &xml);
        let output = scratch.0.join("refused.cf");
        assert!(
            !older_v85_bootstrap(&source, &output, base_free, "2.20")
                .status
                .success()
        );
        assert!(!output.exists());
    }
}
