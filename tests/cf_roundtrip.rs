use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use ibcmd_cf::{
    archive::{CfArchive, CfEntryAttributes, decode_archive_uniform},
    payload::PayloadEncoding,
    writer::{AtomicRepackError, publish_repacked_new, validate_repacked_archive, write_archive},
};
use ibcmd_core::{artifact::StorageProfileId, limits::ResourceLimits, storage::StorageProvenance};

const FORMAT15: &str = include_str!("fixtures/cf/format15-clean-room.cf.b64");
const FORMAT16: &str = include_str!("fixtures/cf/format16-clean-room.cf.b64");

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

fn decode(bytes: &[u8], label: &str) -> CfArchive {
    decode_archive_uniform(
        Cursor::new(bytes),
        ResourceLimits::default(),
        StorageProfileId::parse("storage:cf-roundtrip").unwrap(),
        StorageProvenance::new(label).unwrap(),
        PayloadEncoding::RawDeflate,
    )
    .unwrap()
}

fn assert_semantic_equal(expected: &CfArchive, actual: &CfArchive) {
    assert_eq!(expected.metadata().revision(), actual.metadata().revision());
    assert_eq!(
        expected.metadata().base_offset(),
        actual.metadata().base_offset()
    );
    assert_eq!(
        expected.metadata().page_size(),
        actual.metadata().page_size()
    );
    assert_eq!(
        expected.metadata().storage_version(),
        actual.metadata().storage_version()
    );
    assert_eq!(expected.metadata().reserved(), actual.metadata().reserved());
    assert_eq!(
        expected.metadata().raw_preamble(),
        actual.metadata().raw_preamble()
    );
    assert_eq!(expected.image().len(), actual.image().len());
    for (expected, actual) in expected
        .image()
        .entries()
        .iter()
        .zip(actual.image().entries())
    {
        assert_eq!(expected.logical_name(), actual.logical_name());
        assert_eq!(expected.logical_key(), actual.logical_key());
        assert_eq!(expected.multipart(), actual.multipart());
        assert_eq!(expected.raw_header(), actual.raw_header());
        assert_eq!(expected.packed_payload(), actual.packed_payload());
        assert_eq!(expected.unpacked_payload(), actual.unpacked_payload());
        assert_eq!(expected.compression(), actual.compression());
        assert_eq!(expected.source_profile(), actual.source_profile());
        assert_eq!(expected.provenance(), actual.provenance());
        assert_eq!(
            CfEntryAttributes::decode(expected.attributes())
                .unwrap()
                .data_state(),
            CfEntryAttributes::decode(actual.attributes())
                .unwrap()
                .data_state()
        );
    }
}

#[test]
fn both_revisions_repack_semantically_and_deterministically() {
    for (label, encoded) in [("format15", FORMAT15), ("format16", FORMAT16)] {
        let original_bytes = decode_base64(encoded);
        let original = decode(&original_bytes, label);
        let mut first = Vec::new();
        let first_report = write_archive(&mut first, &original, ResourceLimits::default()).unwrap();
        let validation = validate_repacked_archive(
            Cursor::new(&first),
            original.metadata(),
            original.image(),
            ResourceLimits::default(),
        )
        .unwrap();
        assert_eq!(validation.entries_validated, original.image().len());
        assert_eq!(first_report.bytes_written, first.len() as u64);

        let repacked = decode(&first, label);
        assert_semantic_equal(&original, &repacked);

        let mut second = Vec::new();
        write_archive(&mut second, &original, ResourceLimits::default()).unwrap();
        assert_eq!(first, second, "{label} repack is not deterministic");

        for (before, after) in original
            .image()
            .entries()
            .iter()
            .zip(repacked.image().entries())
        {
            assert_eq!(
                before.packed_payload(),
                after.packed_payload(),
                "{label} changed packed bytes for {}",
                before.logical_name()
            );
        }
    }
}

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ibcmd-rs-cf-roundtrip-{}-{nonce}",
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

#[test]
fn atomic_publish_reopens_validates_and_never_overwrites() {
    let original_bytes = decode_base64(FORMAT16);
    let original = decode(&original_bytes, "atomic-format16");
    let directory = TempDirectory::new();
    let first = directory.path().join("first.cf");
    let second = directory.path().join("second.cf");

    let report = publish_repacked_new(&original, &first, ResourceLimits::default()).unwrap();
    assert_eq!(report.write.bytes_written, report.published_bytes);
    assert_eq!(report.validation.entries_validated, original.image().len());
    publish_repacked_new(&original, &second, ResourceLimits::default()).unwrap();
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());

    let existing = directory.path().join("existing.cf");
    fs::write(&existing, b"keep me").unwrap();
    let error = publish_repacked_new(&original, &existing, ResourceLimits::default()).unwrap_err();
    assert!(matches!(error, AtomicRepackError::ExistingDestination));
    assert_eq!(fs::read(&existing).unwrap(), b"keep me");

    let leftovers = fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tmp"))
        .collect::<Vec<_>>();
    assert!(
        leftovers.is_empty(),
        "temporary files remain: {leftovers:?}"
    );
}
