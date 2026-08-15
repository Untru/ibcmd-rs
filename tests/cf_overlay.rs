use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use ibcmd_cf::{
    archive::decode_archive_uniform,
    overlay::{OverlayCodec, PublishOverlayError, publish_overlay_new},
    payload::{PayloadEncoding, decode_payload, encode_payload},
};
use ibcmd_core::{
    artifact::StorageProfileId,
    limits::ResourceLimits,
    storage::{
        MultipartIdentity, StorageEntry, StorageKey, StoragePatch, StoragePatchEntry,
        StoragePatchOutcome, StoragePatchTarget, StorageProvenance,
    },
};
use ibcmd_rs::module_blob::unpack_module_blob_text;
use ibcmd_v8::writer::{Format15Document, Format15Element, write_format15_to_vec};
use serde_json::Value;

const PROFILE: &str = "storage:mssql-config-configsave";
const MODULE_KEY: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa.0";
const ASSET_KEY: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb.0";
const INTERFACE_KEY: &str = "cccccccc-cccc-4ccc-8ccc-cccccccccccc.0";

// Retained clean-room evidence for a compiled DCS Template body: the packed
// body is the platform's own single raw-deflate stream and the manifest
// records the SHA-256 of both the packed bytes and the one-inflate plaintext.
const DCS_CORPUS_MANIFEST: &str =
    include_str!("fixtures/native-evidence/8.3.27.2214/dcs-area-style-item-uuid/manifest.json");
const DCS_CORPUS_PACKED_BODY_B64: &str = include_str!(
    "fixtures/native-evidence/8.3.27.2214/dcs-area-style-item-uuid/raw-packed.bin.b64"
);
const DCS_CORPUS_UNPACKED_BODY_B64: &str = include_str!(
    "fixtures/native-evidence/8.3.27.2214/dcs-area-style-item-uuid/raw-unpacked.bin.b64"
);

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

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
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
        let path = std::env::temp_dir().join(format!(
            "ibcmd-rs-cf-overlay-{}-{nonce}",
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

fn deflate(bytes: &[u8]) -> Vec<u8> {
    encode_payload(
        PayloadEncoding::RawDeflate,
        bytes,
        ResourceLimits::default(),
    )
    .unwrap()
}

fn base_archive() -> Vec<u8> {
    let versions = format!(
        "\u{feff}{{1,7,\"\",11111111-1111-4111-8111-111111111111,\"root\",22222222-2222-4222-8222-222222222222,\"version\",33333333-3333-4333-8333-333333333333,\"versions\",44444444-4444-4444-8444-444444444444,\"{MODULE_KEY}\",55555555-5555-4555-8555-555555555555,\"{ASSET_KEY}\",66666666-6666-4666-8666-666666666666,\"{INTERFACE_KEY}\",77777777-7777-4777-8777-777777777777,\"unknown.0\",88888888-8888-4888-8888-888888888888}}"
    );
    let command_interface =
        b"{7,1,1,{0,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},{0,{0,{\"B\",0},0}},0,0,0,0,0}";
    write_format15_to_vec(&Format15Document::new(
        7,
        vec![
            Format15Element::named("unknown.0", Some(deflate(b"opaque bytes"))),
            Format15Element::named(MODULE_KEY, Some(deflate(b"old module"))),
            Format15Element::named(ASSET_KEY, Some(deflate(b"old asset"))),
            Format15Element::named(INTERFACE_KEY, Some(deflate(command_interface))),
            Format15Element::named("versions", Some(deflate(versions.as_bytes()))),
        ],
    ))
    .unwrap()
}

fn decode(bytes: &[u8], provenance: &str) -> ibcmd_cf::archive::CfArchive {
    decode_archive_uniform(
        Cursor::new(bytes),
        ResourceLimits::default(),
        StorageProfileId::parse(PROFILE).unwrap(),
        StorageProvenance::new(provenance).unwrap(),
        PayloadEncoding::RawDeflate,
    )
    .unwrap()
}

#[test]
fn cli_overlays_module_raw_asset_and_needs_base_without_platform() {
    let temp = TempDirectory::new();
    let base_path = temp.path().join("base.cf");
    let output_path = temp.path().join("overlay.cf");
    let module_path = temp.path().join("Module.bsl");
    let asset_path = temp.path().join("asset.bin");
    let interface_path = temp.path().join("CommandInterface.xml");
    fs::write(&base_path, base_archive()).unwrap();
    fs::write(&module_path, b"Procedure Offline()\nEndProcedure").unwrap();
    fs::write(&asset_path, b"new exact asset bytes").unwrap();
    fs::write(
        &interface_path,
        br#"<?xml version="1.0" encoding="UTF-8"?>
<CommandInterface xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
  <CommandsVisibility>
    <Command name="Catalog.Products.StandardCommand.OpenList">
      <Visibility><xr:Common>true</xr:Common></Visibility>
    </Command>
  </CommandsVisibility>
</CommandInterface>"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "overlay"])
        .arg(&base_path)
        .arg(&output_path)
        .arg("--module")
        .arg(format!("{MODULE_KEY}={}", module_path.display()))
        .arg("--raw-asset")
        .arg(format!("{ASSET_KEY}={}", asset_path.display()))
        .arg("--command-interface")
        .arg(format!("{INTERFACE_KEY}={}", interface_path.display()))
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
    assert_eq!(report["command"], "overlay");
    assert_eq!(report["ok"], true);
    assert_eq!(report["overlay"]["requested_entries"], 3);
    assert_eq!(report["overlay"]["preserved_entries"], 1);
    assert_eq!(report["overlay"]["versions_updated"], true);
    assert_eq!(report["publication"]["entries_validated"], 5);
    assert!(
        report["overlay"]["changes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|change| change["source"] == "needs_base")
    );

    let base = decode(&fs::read(&base_path).unwrap(), "base");
    let overlaid = decode(&fs::read(&output_path).unwrap(), "output");
    assert_eq!(
        base.image()
            .entries()
            .iter()
            .map(|entry| entry.logical_key().as_str())
            .collect::<Vec<_>>(),
        overlaid
            .image()
            .entries()
            .iter()
            .map(|entry| entry.logical_key().as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        base.entry("unknown.0").unwrap().packed_payload(),
        overlaid.entry("unknown.0").unwrap().packed_payload()
    );
    assert_eq!(
        unpack_module_blob_text(overlaid.entry(MODULE_KEY).unwrap().packed_payload()).unwrap(),
        b"Procedure Offline()\nEndProcedure"
    );
    assert_eq!(
        overlaid.entry(ASSET_KEY).unwrap().unpacked_payload(),
        b"new exact asset bytes"
    );
    let command_interface = String::from_utf8(
        overlaid
            .entry(INTERFACE_KEY)
            .unwrap()
            .unpacked_payload()
            .to_vec(),
    )
    .unwrap();
    assert!(command_interface.contains("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"));
    assert!(command_interface.contains("{\"B\",1}"));
    let versions = String::from_utf8(
        overlaid
            .entry("versions")
            .unwrap()
            .unpacked_payload()
            .to_vec(),
    )
    .unwrap();
    assert!(versions.contains("\"unknown.0\",88888888-8888-4888-8888-888888888888"));
    assert_ne!(
        base.entry("versions").unwrap().packed_payload(),
        overlaid.entry("versions").unwrap().packed_payload()
    );
}

/// A compiled DCS Template body (the corpus's platform-produced raw-deflate
/// stream) overlaid via `--compiled-asset` must land in the CF verbatim: the
/// stored physical payload is byte-identical to the input file, and a single
/// inflate yields the XML plaintext whose SHA-256 the corpus manifest records
/// as `unpacked_body`. The `--raw-asset` path would instead deflate the body a
/// second time, producing the double-compressed payload the platform accepts
/// but cannot export ("Stream format error").
#[test]
fn cli_overlay_compiled_asset_stores_corpus_dcs_body_verbatim() {
    let manifest: Value = serde_json::from_str(DCS_CORPUS_MANIFEST).unwrap();
    let compiled = decode_base64(DCS_CORPUS_PACKED_BODY_B64);
    assert_eq!(
        sha256_hex(&compiled),
        manifest["retained"]["packed_body"]["sha256"]
            .as_str()
            .unwrap(),
        "retained raw-packed.bin.b64 fixture no longer matches its manifest sha256"
    );

    let temp = TempDirectory::new();
    let base_path = temp.path().join("base.cf");
    let output_path = temp.path().join("overlay.cf");
    let body_path = temp.path().join("compiled-dcs-body.bin");
    fs::write(&base_path, base_archive()).unwrap();
    fs::write(&body_path, &compiled).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "overlay"])
        .arg(&base_path)
        .arg(&output_path)
        .arg("--compiled-asset")
        .arg(format!("{ASSET_KEY}={}", body_path.display()))
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
    assert_eq!(report["command"], "overlay");
    assert_eq!(report["ok"], true);
    assert_eq!(report["overlay"]["requested_entries"], 1);

    let overlaid = decode(&fs::read(&output_path).unwrap(), "compiled-asset-output");
    let entry = overlaid.entry(ASSET_KEY).unwrap();
    assert_eq!(
        entry.packed_payload(),
        compiled.as_slice(),
        "compiled body must be stored verbatim, without a second deflate layer"
    );
    let inflated_once = decode_payload(
        PayloadEncoding::RawDeflate,
        entry.packed_payload(),
        ResourceLimits::default(),
    )
    .unwrap()
    .into_bytes();
    assert_eq!(
        sha256_hex(&inflated_once),
        manifest["retained"]["unpacked_body"]["sha256"]
            .as_str()
            .unwrap(),
        "one inflate of the stored payload must yield the manifest unpacked body"
    );
    assert!(
        String::from_utf8_lossy(&inflated_once)
            .contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"),
        "one inflate must already expose the XML plaintext"
    );
    assert_eq!(entry.unpacked_payload(), inflated_once.as_slice());
}

/// Plain (not raw-deflated) bytes handed to `--compiled-asset` must be
/// rejected fail-closed with the dedicated diagnostic code before any output
/// is written, and the message must point at `--raw-asset` as the family for
/// plain source bytes. The corpus's unpacked body doubles as the realistic
/// wrong input: it is exactly what an extraction produces.
#[test]
fn cli_overlay_compiled_asset_rejects_plain_bytes_with_raw_asset_hint() {
    let temp = TempDirectory::new();
    let base_path = temp.path().join("base.cf");
    let output_path = temp.path().join("overlay.cf");
    let plain_path = temp.path().join("plain-dcs-body.bin");
    fs::write(&base_path, base_archive()).unwrap();
    fs::write(&plain_path, decode_base64(DCS_CORPUS_UNPACKED_BODY_B64)).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "overlay"])
        .arg(&base_path)
        .arg(&output_path)
        .arg("--compiled-asset")
        .arg(format!("{ASSET_KEY}={}", plain_path.display()))
        .args(["--source-version", "2.20"])
        .env("PATH", "")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "plain bytes must not be accepted: stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let report: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(report["command"], "overlay");
    assert_eq!(report["ok"], false);
    let error = &report["errors"][0];
    assert_eq!(error["code"], "invalid_compiled_asset");
    let message = error["message"].as_str().unwrap();
    assert!(
        message.contains("--raw-asset"),
        "error must point at --raw-asset for plain bytes: {message}"
    );
    assert!(
        message.contains(ASSET_KEY),
        "error must name the rejected storage key: {message}"
    );
    assert!(!output_path.exists());
}

struct CountingCodec<'a>(&'a AtomicUsize);

impl OverlayCodec for CountingCodec<'_> {
    fn resolve_needs_base(
        &mut self,
        _target: &StoragePatchTarget,
        _required: &StorageKey,
        _base: &StorageEntry,
    ) -> Result<Vec<u8>, String> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(Vec::new())
    }

    fn update_versions(
        &mut self,
        _base: &StorageEntry,
        _changed_keys: &[String],
    ) -> Result<Vec<u8>, String> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(Vec::new())
    }
}

#[test]
fn unsupported_preflight_never_creates_destination() {
    let temp = TempDirectory::new();
    let destination = temp.path().join("must-not-exist.cf");
    let archive = decode(&base_archive(), "unsupported-base");
    let patch = StoragePatch::new(vec![StoragePatchEntry::new(
        StoragePatchTarget::new(
            StorageKey::new(MODULE_KEY).unwrap(),
            MultipartIdentity::single(),
            StorageProvenance::new("unsupported test").unwrap(),
        ),
        StoragePatchOutcome::unsupported("clean-room unsupported family").unwrap(),
    )])
    .unwrap();
    let calls = AtomicUsize::new(0);
    let mut codec = CountingCodec(&calls);

    let error = publish_overlay_new(
        &archive,
        &patch,
        &mut codec,
        &destination,
        ResourceLimits::default(),
    )
    .unwrap_err();

    assert!(matches!(error, PublishOverlayError::Overlay(_)));
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert!(!destination.exists());
    assert!(fs::read_dir(temp.path()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")
    }));
}
