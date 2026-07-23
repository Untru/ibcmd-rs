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
    payload::{PayloadEncoding, encode_payload},
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

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
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
