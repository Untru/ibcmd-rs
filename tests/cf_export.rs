use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use ibcmd_cf::payload::{PayloadEncoding, encode_payload};
use ibcmd_core::limits::ResourceLimits;
use ibcmd_v8::writer::{Format15Document, Format15Element, write_format15_to_vec};
use serde_json::Value;

const FORMAT15: &str = include_str!("fixtures/cf/format15-clean-room.cf.b64");

fn decode_base64(source: &str) -> Vec<u8> {
    let mut output = Vec::new();
    let mut buffer = 0_u32;
    let mut bits = 0_u8;
    let mut saw_padding = false;
    for byte in source.bytes() {
        if byte.is_ascii_whitespace() {
            continue;
        }
        if byte == b'=' {
            saw_padding = true;
            continue;
        }
        assert!(!saw_padding, "non-padding data after Base64 padding");
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("invalid Base64 byte 0x{byte:02x}"),
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push(((buffer >> bits) & 0xff) as u8);
            buffer &= if bits == 0 { 0 } else { (1_u32 << bits) - 1 };
        }
    }
    assert!(bits == 0 || buffer == 0, "non-zero trailing Base64 bits");
    output
}

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        // The wall clock alone can collide when parallel tests hit the same
        // timer tick; a per-process sequence keeps the names unique.
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let nonce = format!(
            "{}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let path =
            std::env::temp_dir().join(format!("ibcmd-rs-cf-export-{}-{nonce}", std::process::id()));
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

fn xml_files(root: &Path) -> Vec<PathBuf> {
    fn visit(path: &Path, files: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                visit(&path, files);
            } else if path.extension().is_some_and(|extension| extension == "xml") {
                files.push(path);
            }
        }
    }

    let mut files = Vec::new();
    visit(root, &mut files);
    files.sort();
    files
}

fn full_configuration_archive() -> Vec<u8> {
    let uuid = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    let zero = "00000000-0000-0000-0000-000000000000";
    let configuration = format!(
        "{{2,{{{uuid}}},{{3,{{1,0,{uuid}}},\"OfflineDemo\",{{1,\"en\",\"Offline demo\"}},\"\",0,0,{zero},0}}}}"
    );
    let packed = encode_payload(
        PayloadEncoding::RawDeflate,
        configuration.as_bytes(),
        ResourceLimits::default(),
    )
    .unwrap();
    write_format15_to_vec(&Format15Document::new(
        7,
        vec![Format15Element::named(uuid, Some(packed))],
    ))
    .unwrap()
}

#[test]
fn clean_room_cf_exports_known_families_with_an_empty_path() {
    let temp = TempDirectory::new();
    let input = temp.path().join("clean-room.cf");
    let output_dir = temp.path().join("source");
    fs::write(&input, decode_base64(FORMAT15)).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "export"])
        .arg(&input)
        .arg(&output_dir)
        .args(["--source-version", "2.20"])
        .env("PATH", "")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["command"], "export");
    assert_eq!(report["ok"], true);
    assert_eq!(report["export"]["storage"]["physical_entries"], 5);
    assert_eq!(report["export"]["storage"]["logical_entries"], 5);
    assert_eq!(report["export"]["storage"]["supported"], 1);
    assert_eq!(report["export"]["storage"]["opaque"], 4);
    assert_eq!(report["export"]["storage"]["failed"], 0);
    assert_eq!(report["export"]["files_written"], 1);

    let files = xml_files(&output_dir);
    assert_eq!(files.len(), 1, "unexpected XML outputs: {files:?}");
    let xml = files
        .iter()
        .map(|path| fs::read_to_string(path).unwrap())
        .collect::<Vec<_>>();
    assert!(xml.iter().any(|text| text.contains("<Language")));
}

#[test]
fn complete_configuration_record_exports_through_the_same_family_decoder() {
    let temp = TempDirectory::new();
    let input = temp.path().join("configuration.cf");
    let output_dir = temp.path().join("source");
    fs::write(&input, full_configuration_archive()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "export"])
        .arg(&input)
        .arg(&output_dir)
        .args(["--source-version", "2.21"])
        .env("PATH", "")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["source_version"], "2.21");
    assert_eq!(report["export"]["storage"]["supported"], 1);
    assert_eq!(report["export"]["storage"]["opaque"], 0);
    assert_eq!(report["export"]["storage"]["failed"], 0);
    assert_eq!(report["export"]["files_written"], 1);
    let configuration = fs::read_to_string(output_dir.join("Configuration.xml")).unwrap();
    assert!(configuration.contains("<Configuration"));
    assert!(configuration.contains("version=\"2.21\""));
    assert!(configuration.contains("<Name>OfflineDemo</Name>"));
}

// CF-EXPORT-CONFIG-01: retained clean-room evidence for a Configuration whose
// composition (Language + StyleItem + Report) is known from real platform
// bytes, not a hypothesis. See
// docs/evidence/dcs-batch-evidence-2214-20260813.md and
// docs/evidence/dcs-style-link-probes-2214-20260814.md for the lab sessions
// that first observed the `cf export` gap this test guards.
const RETAINED_CONFIGURATION_CF: &str = include_str!(
    "fixtures/native-evidence/8.3.27.2214/dcs-area-style-item-uuid/configuration.cf.b64"
);
const RETAINED_NATIVE_CONFIGURATION_XML: &str = include_str!(
    "fixtures/native-evidence/8.3.27.2214/dcs-area-style-item-uuid/native-configuration.xml.b64"
);
const RETAINED_MANIFEST: &str =
    include_str!("fixtures/native-evidence/8.3.27.2214/dcs-area-style-item-uuid/manifest.json");

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

/// Slices out one exact top-level `<tag>...</tag>` block (inclusive), so
/// composition-shaped sections can be compared without depending on the rest
/// of `Configuration.xml` (Properties completeness beyond composition is a
/// separate, tracked gap; see the doc comment on
/// `retained_configuration_reproduces_native_internal_info_and_child_objects`).
fn xml_block<'a>(xml: &'a str, tag: &str) -> &'a str {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml
        .find(&open)
        .unwrap_or_else(|| panic!("missing <{tag}> in {xml}"));
    let end = xml[start..]
        .find(&close)
        .map(|offset| start + offset + close.len())
        .unwrap_or_else(|| panic!("missing </{tag}> in {xml}"));
    &xml[start..end]
}

/// `cf export` of a retained, byte-exact clean-room CF must reproduce the
/// native platform's `<InternalInfo>` (the fixed `ContainedObject` graph) and
/// `<ChildObjects>` (Language, StyleItem, Report in the evidenced order)
/// sections of `Configuration.xml` byte for byte.
///
/// This does not assert whole-file equality against
/// `native-configuration.xml.b64`: that native file also carries ~30
/// `<Properties>` children (e.g. `IncludeHelpInContents`,
/// `ObjectAutonumerationMode`, `InterfaceCompatibilityMode`,
/// `DatabaseTablespacesUseMode`, the `Default*Form` family,
/// `UsedMobileApplicationFunctionalities`) that the exporter does not yet
/// decode from the CF storage image. That gap is independent of composition
/// (`ChildObjects`/`InternalInfo`) and is out of scope for this fix; closing
/// it needs additional retained corpora with non-default values to safely
/// derive the remaining field positions without guessing.
#[test]
fn retained_configuration_reproduces_native_internal_info_and_child_objects() {
    let manifest: Value = serde_json::from_str(RETAINED_MANIFEST).unwrap();
    let cf_bytes = decode_base64(RETAINED_CONFIGURATION_CF);
    let native_bytes = decode_base64(RETAINED_NATIVE_CONFIGURATION_XML);
    assert_eq!(
        sha256_hex(&cf_bytes),
        manifest["retained"]["configuration"]["sha256"]
            .as_str()
            .unwrap(),
        "retained configuration.cf.b64 fixture no longer matches its manifest sha256"
    );
    assert_eq!(
        sha256_hex(&native_bytes),
        manifest["retained"]["native_configuration"]["sha256"]
            .as_str()
            .unwrap(),
        "retained native-configuration.xml.b64 fixture no longer matches its manifest sha256"
    );
    let native_xml = String::from_utf8(native_bytes).unwrap();

    let temp = TempDirectory::new();
    let input = temp.path().join("configuration.cf");
    let output_dir = temp.path().join("source");
    fs::write(&input, &cf_bytes).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "export"])
        .arg(&input)
        .arg(&output_dir)
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let exported_xml = fs::read_to_string(output_dir.join("Configuration.xml")).unwrap();

    assert_eq!(
        xml_block(&exported_xml, "InternalInfo"),
        xml_block(&native_xml, "InternalInfo"),
        "exported InternalInfo does not reproduce the native ContainedObject graph"
    );
    assert_eq!(
        xml_block(&exported_xml, "ChildObjects"),
        xml_block(&native_xml, "ChildObjects"),
        "exported ChildObjects does not reproduce the native Language/StyleItem/Report order"
    );
}

/// MINI-GATE-A-CONFIG-PROPS-01: full-file `Configuration.xml` byte parity
/// across three corpora sharing the base coordinate but differing in which
/// Properties fields are at their platform default. T1/T3 share the exact
/// `dcs-area-style-item-uuid` base recipe (T3 additionally has three
/// non-default enum-typed properties at the corpus-proven offsets); T2 is a
/// structurally different corpus (different Configuration Name/Synonym
/// header length, additional Catalogs/Report ChildObjects) whose Properties
/// span is nonetheless all-default -- exercising the evidenced-default
/// fail-closed comparison's header-length-shift tolerance, not just the
/// exact-offset case.
#[test]
fn retained_configuration_xml_matches_native_byte_for_byte_across_the_evidenced_cohort() {
    const T1_CF: &str = include_str!(
        "fixtures/native-evidence/8.3.27.2214/dcs-area-style-item-uuid/configuration.cf.b64"
    );
    const T1_NATIVE: &str = include_str!(
        "fixtures/native-evidence/8.3.27.2214/dcs-area-style-item-uuid/native-configuration.xml.b64"
    );
    const T2_CF: &str = include_str!(
        "fixtures/native-evidence/8.3.27.2214/dcs-form-list-settings-server-state/configuration.cf.b64"
    );
    const T2_NATIVE: &str = include_str!(
        "fixtures/native-evidence/8.3.27.2214/dcs-form-list-settings-server-state/native-configuration.xml.b64"
    );
    const T3_CF: &str = include_str!(
        "fixtures/native-evidence/8.3.27.2214/configuration-properties-enum-group/configuration.cf.b64"
    );
    const T3_NATIVE: &str = include_str!(
        "fixtures/native-evidence/8.3.27.2214/configuration-properties-enum-group/native-configuration.xml.b64"
    );

    for (name, cf_b64, native_b64) in [
        ("T1 dcs-area-style-item-uuid", T1_CF, T1_NATIVE),
        ("T2 dcs-form-list-settings-server-state", T2_CF, T2_NATIVE),
        ("T3 configuration-properties-enum-group", T3_CF, T3_NATIVE),
    ] {
        let cf_bytes = decode_base64(cf_b64);
        let native_bytes = decode_base64(native_b64);
        let native_xml = String::from_utf8(native_bytes).unwrap();

        let temp = TempDirectory::new();
        let input = temp.path().join("configuration.cf");
        let output_dir = temp.path().join("source");
        fs::write(&input, &cf_bytes).unwrap();

        // Only Configuration.xml is asserted here; the exit status is
        // deliberately not, so an unrelated per-row export gap in a corpus
        // cannot mask (or be masked by) this test's own byte-parity claim.
        let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
            .args(["cf", "export"])
            .arg(&input)
            .arg(&output_dir)
            .env("PATH", "")
            .output()
            .unwrap();

        let exported_xml =
            fs::read_to_string(output_dir.join("Configuration.xml")).unwrap_or_else(|error| {
                panic!(
                    "{name}: Configuration.xml missing: {error}\nstdout: {}\nstderr: {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                )
            });
        assert_eq!(
            exported_xml, native_xml,
            "{name}: exported Configuration.xml is not byte-exact against native"
        );
    }
}
