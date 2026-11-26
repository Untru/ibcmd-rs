use ibcmd_cf::{
    archive::decode_archive_uniform,
    payload::PayloadEncoding,
    writer::{validate_repacked_archive, write_archive},
};
use ibcmd_core::{artifact::StorageProfileId, limits::ResourceLimits, storage::StorageProvenance};
use ibcmd_v8::{
    format15,
    writer::{Format15Document, Format15Element, write_format15_to_vec},
};
use std::io::Cursor;

#[test]
fn repack_discards_free_pages_and_validates_the_new_header_and_payload() {
    let mut source = write_format15_to_vec(&Format15Document::new(
        5,
        vec![Format15Element::named("test", Some(b"data".to_vec()))],
    ))
    .unwrap();
    let head = source.len() as u32;
    source.extend_from_slice(b"\r\n7fffffff 00000004 7fffffff \r\nfree");
    source[..4].copy_from_slice(&head.to_le_bytes());
    let archive = decode_archive_uniform(
        Cursor::new(source.clone()),
        ResourceLimits::default(),
        StorageProfileId::parse("storage:cf-cli").unwrap(),
        StorageProvenance::new("clean-room free-page regression").unwrap(),
        PayloadEncoding::Stored,
    )
    .unwrap();
    let mut output = Vec::new();
    write_archive(&mut output, &archive, ResourceLimits::default()).unwrap();
    assert!(output.len() < source.len());
    assert_eq!(&output[..4], format15::SENTINEL.to_le_bytes());
    let valid = validate_repacked_archive(
        Cursor::new(output.clone()),
        archive.metadata(),
        archive.image(),
        ResourceLimits::default(),
    )
    .unwrap();
    assert_eq!(valid.entries_validated, 1);
    // Other header coordinates remain verified, not silently normalized.
    output[12] ^= 1;
    assert!(
        validate_repacked_archive(
            Cursor::new(output),
            archive.metadata(),
            archive.image(),
            ResourceLimits::default()
        )
        .is_err()
    );
}
