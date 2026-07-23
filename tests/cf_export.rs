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
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
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
