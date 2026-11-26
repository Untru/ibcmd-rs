use ibcmd_v8::{
    format::{self, Revision},
    format15, format16,
    reader::StreamingReader,
    writer::{
        self, Format15Document, Format15Element, Format16Document, Format16Element, Format16Payload,
    },
};
use std::io::Cursor;

fn narrow() -> Vec<u8> {
    writer::write_format15_to_vec(&Format15Document::new(
        5,
        vec![Format15Element::named(
            "payload",
            Some(b"native-compatible".to_vec()),
        )],
    ))
    .unwrap()
}
fn with_free_pages(mut bytes: Vec<u8>, wide: bool, base: usize) -> (Vec<u8>, usize, usize) {
    let first = bytes.len() - base;
    let width = if wide { 16 } else { 8 };
    let sentinel = if wide {
        u64::MAX
    } else {
        u64::from(format15::SENTINEL)
    };
    let header_size = if wide { 55 } else { 31 };
    let second = first + header_size + 7;
    for (next, size) in [(second as u64, 7), (sentinel, 3)] {
        let header = format!("\r\n{sentinel:0width$x} {size:0width$x} {next:0width$x} \r\n");
        bytes.extend_from_slice(header.as_bytes());
        bytes.resize(bytes.len() + size, 0);
    }
    if wide {
        bytes[base..base + 8].copy_from_slice(&(first as u64).to_le_bytes());
    } else {
        bytes[..4].copy_from_slice(&(first as u32).to_le_bytes());
    }
    (bytes, first, second)
}
fn verify(bytes: Vec<u8>, revision: Revision) {
    assert_eq!(format::detect(&bytes).unwrap().revision, revision);
    let parsed = format::parse(&bytes).unwrap();
    assert_eq!(parsed.element_count(), 1);
    let mut stream = StreamingReader::open_default(Cursor::new(bytes)).unwrap();
    assert_eq!(stream.index().revision, revision);
    assert_eq!(
        stream.read_named_data("payload").unwrap().unwrap(),
        b"native-compatible"
    );
}
#[test]
fn narrow_nonempty_free_list_preserves_live_payload() {
    verify(with_free_pages(narrow(), false, 0).0, Revision::Format15);
}
#[test]
fn wide_nonempty_free_list_preserves_live_payload() {
    let mut bytes = writer::write_format15_to_vec(&Format15Document::new(0, vec![])).unwrap();
    bytes.resize(format16::BASE_OFFSET, 0);
    let mut doc = Format16Document::new(
        5,
        vec![Format16Element::named(
            "payload",
            Some(Format16Payload::from_bytes(b"native-compatible")),
        )],
    );
    writer::write_format16_primary(&mut bytes, &mut doc).unwrap();
    verify(
        with_free_pages(bytes, true, format16::BASE_OFFSET).0,
        Revision::Format16,
    );
}
#[test]
fn uppercase_free_page_sentinels_are_detected_by_both_readers() {
    let mut wide = writer::write_format15_to_vec(&Format15Document::new(0, vec![])).unwrap();
    wide.resize(format16::BASE_OFFSET, 0);
    let mut doc = Format16Document::new(
        5,
        vec![Format16Element::named(
            "payload",
            Some(Format16Payload::from_bytes(b"native-compatible")),
        )],
    );
    writer::write_format16_primary(&mut wide, &mut doc).unwrap();
    for (bytes, wide, base, revision) in [
        (narrow(), false, 0, Revision::Format15),
        (wide, true, format16::BASE_OFFSET, Revision::Format16),
    ] {
        let (mut bytes, first, second) = with_free_pages(bytes, wide, base);
        let length = if wide { 55 } else { 31 };
        for address in [first, second] {
            bytes[base + address..base + address + length].make_ascii_uppercase();
        }
        verify(bytes, revision);
    }
}

#[test]
fn free_page_cycles_are_rejected_by_both_readers() {
    let (mut bytes, first, second) = with_free_pages(narrow(), false, 0);
    bytes[second + 20..second + 28].copy_from_slice(format!("{first:08x}").as_bytes());
    assert!(format::parse(&bytes).is_err());
    assert!(StreamingReader::open_default(Cursor::new(bytes)).is_err());
}
#[test]
fn a_free_page_must_not_alias_live_pages_or_point_outside_the_file() {
    for head in [16u32, u32::MAX - 1] {
        let mut bytes = narrow();
        bytes[..4].copy_from_slice(&head.to_le_bytes());
        assert!(format::parse(&bytes).is_err());
        assert!(StreamingReader::open_default(Cursor::new(bytes)).is_err());
    }
    let (mut bytes, first, _) = with_free_pages(narrow(), false, 0);
    // Extend the first free page across EOF instead of relaxing page bounds.
    bytes[first + 11..first + 19].copy_from_slice(b"7fffffff");
    assert!(format::parse(&bytes).is_err());
    assert!(StreamingReader::open_default(Cursor::new(bytes)).is_err());
}
