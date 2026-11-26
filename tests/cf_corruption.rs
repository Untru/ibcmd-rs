use std::io::Cursor;

use ibcmd_cf::{
    payload::{PayloadDecodeError, PayloadEncoding, decode_payload},
    preamble::{PreambleMode, SemanticPreamble, write_format16_archive_to_vec},
    tree::{TraversalAction, TreeError, traverse},
};
use ibcmd_core::limits::ResourceLimits;
use ibcmd_v8::{
    format::Revision,
    reader::{ReaderError, StreamingReader},
    writer::{
        Format15Document, Format15Element, Format16Document, Format16Element, Format16Payload,
        make_element_header, write_format15_to_vec,
    },
};

const TRUNCATED_HEADER: &str = include_str!("fixtures/malformed/cf/file-header-truncated.hex");
const UNKNOWN_SIGNATURE: &str =
    include_str!("fixtures/malformed/cf/file-header-unknown-signature.hex");
const INVALID_HEX: &str = include_str!("fixtures/malformed/cf/block-invalid-hex.hex");
const OUT_OF_RANGE: &str = include_str!("fixtures/malformed/cf/toc-offset-out-of-range.hex");
const SELF_CYCLE: &str = include_str!("fixtures/malformed/cf/chain-self-cycle.hex");
const ODD_UTF16: &str = include_str!("fixtures/malformed/cf/element-name-odd-utf16.hex");
const INVALID_UTF16: &str = include_str!("fixtures/malformed/cf/element-name-invalid-utf16.hex");
const INVALID_DEFLATE: &str = include_str!("fixtures/malformed/cf/payload-invalid-deflate.hex");
const TRUNCATED_NESTED: &str = include_str!("fixtures/malformed/cf/nested-truncated-child.hex");

fn limits() -> ResourceLimits {
    ResourceLimits::new(8, 64, 1_048_576, 1_048_576, 200).unwrap()
}

fn decode_hex(source: &str) -> Vec<u8> {
    source
        .lines()
        .flat_map(|line| {
            line.split('#')
                .next()
                .unwrap_or_default()
                .split_whitespace()
        })
        .map(|token| {
            assert_eq!(token.len(), 2, "invalid clean-room hex token `{token}`");
            u8::from_str_radix(token, 16)
                .unwrap_or_else(|error| panic!("invalid clean-room hex token `{token}`: {error}"))
        })
        .collect()
}

fn reader_error(seed: &str) -> ReaderError {
    match StreamingReader::open(Cursor::new(decode_hex(seed)), limits()) {
        Ok(_) => panic!("malformed corpus seed unexpectedly opened"),
        Err(error) => error,
    }
}

#[test]
fn checked_in_malformed_corpus_reaches_every_container_boundary() {
    assert!(matches!(
        reader_error(TRUNCATED_HEADER),
        ReaderError::InputTooShort { .. }
    ));
    assert!(matches!(
        reader_error(UNKNOWN_SIGNATURE),
        ReaderError::UnknownSignature { .. }
    ));
    assert!(matches!(
        reader_error(INVALID_HEX),
        ReaderError::InvalidHexField { .. }
    ));
    assert!(matches!(
        reader_error(OUT_OF_RANGE),
        ReaderError::RangeOutOfBounds { .. }
    ));
    assert!(matches!(
        reader_error(SELF_CYCLE),
        ReaderError::Cycle { .. }
    ));
    assert!(matches!(
        reader_error(ODD_UTF16),
        ReaderError::OddElementNameLength { .. }
    ));
    assert!(matches!(
        reader_error(INVALID_UTF16),
        ReaderError::InvalidElementName { .. }
    ));

    let mut reader =
        StreamingReader::open(Cursor::new(decode_hex(INVALID_DEFLATE)), limits()).unwrap();
    let payload = reader.read_entry_data(0).unwrap().unwrap();
    assert!(matches!(
        decode_payload(PayloadEncoding::RawDeflate, &payload, limits()).unwrap_err(),
        PayloadDecodeError::InvalidDeflate(_) | PayloadDecodeError::TruncatedDeflate
    ));

    let nested_error = traverse(
        Cursor::new(decode_hex(TRUNCATED_NESTED)),
        limits(),
        |_, _| TraversalAction::Container(PayloadEncoding::Stored),
        |_| {},
    )
    .unwrap_err();
    assert!(matches!(
        nested_error,
        TreeError::Reader {
            path,
            source: ReaderError::InputTooShort { .. },
        } if path == ["x"]
    ));
}

#[derive(Clone, Copy)]
struct DeterministicBytes(u64);

impl DeterministicBytes {
    fn next(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.0 = value;
        value
    }

    fn bytes(&mut self, length: usize) -> Vec<u8> {
        (0..length).map(|_| self.next() as u8).collect()
    }
}

#[test]
fn bounded_pseudorandom_input_never_panics_or_bypasses_limits() {
    let mut generator = DeterministicBytes(0x4f46_464c_494e_4543);
    for case in 0..512_usize {
        let length = match case {
            0 => 0,
            1 => 1,
            2 => 15,
            3 => 16,
            4 => 31,
            _ => (generator.next() as usize) % 4_097,
        };
        let input = generator.bytes(length);
        if let Ok(mut reader) = StreamingReader::open(Cursor::new(input), limits()) {
            assert!(reader.index().entries.len() <= 64);
            for index in 0..reader.index().entries.len() {
                let _ = reader.read_entry_header(index);
                let _ = reader.read_entry_data(index);
            }
        }
    }
}

#[derive(Debug)]
struct GeneratedCase {
    page_size: u32,
    storage_version: u32,
    names: Vec<String>,
    payloads: Vec<Option<Vec<u8>>>,
}

fn generated_case(generator: &mut DeterministicBytes, case: usize) -> GeneratedCase {
    const PAGE_SIZES: [u32; 5] = [31, 64, 511, 512, 1_024];
    let page_size = PAGE_SIZES[case % PAGE_SIZES.len()];
    let entry_count = (generator.next() as usize) % 7;
    let mut names = Vec::with_capacity(entry_count);
    let mut payloads = Vec::with_capacity(entry_count);
    let boundary_lengths = [
        0,
        1,
        page_size.saturating_sub(1) as usize,
        page_size as usize,
        page_size.saturating_add(1) as usize,
        511,
        512,
        513,
        1_025,
    ];
    for index in 0..entry_count {
        names.push(format!("case-{case}-Ж-{index}"));
        let payload = if generator.next().is_multiple_of(5) {
            None
        } else {
            let length =
                boundary_lengths[(generator.next() as usize + index) % boundary_lengths.len()];
            Some(generator.bytes(length))
        };
        payloads.push(payload);
    }
    GeneratedCase {
        page_size,
        storage_version: (generator.next() as u32) % 8,
        names,
        payloads,
    }
}

fn assert_parsed_case(bytes: Vec<u8>, revision: Revision, generated: &GeneratedCase) {
    let mut reader = StreamingReader::open(Cursor::new(bytes), limits()).unwrap();
    assert_eq!(reader.index().revision, revision);
    assert_eq!(reader.index().storage_version, generated.storage_version);
    assert_eq!(reader.index().entries.len(), generated.names.len());
    for index in 0..generated.names.len() {
        assert_eq!(reader.index().entries[index].name, generated.names[index]);
        assert_eq!(
            reader.read_entry_header(index).unwrap(),
            make_element_header(&generated.names[index])
        );
        assert_eq!(
            reader.read_entry_data(index).unwrap(),
            generated.payloads[index]
        );
    }
}

#[test]
fn generated_format15_and_format16_documents_satisfy_parse_build_properties() {
    let mut generator = DeterministicBytes(0x4346_5052_4f50_4552);
    for case_index in 0..128_usize {
        let generated = generated_case(&mut generator, case_index);
        let mut format15 = Format15Document::new(
            generated.storage_version,
            generated
                .names
                .iter()
                .zip(&generated.payloads)
                .map(|(name, payload)| Format15Element::named(name, payload.clone()))
                .collect(),
        );
        format15.page_size = generated.page_size;
        let format15_bytes = write_format15_to_vec(&format15).unwrap();
        assert_parsed_case(format15_bytes, Revision::Format15, &generated);

        let mut format16 = Format16Document::new(
            generated.storage_version,
            generated
                .names
                .iter()
                .zip(&generated.payloads)
                .map(|(name, payload)| {
                    Format16Element::named(
                        name,
                        payload.as_deref().map(Format16Payload::from_bytes),
                    )
                })
                .collect(),
        );
        format16.page_size = generated.page_size;
        let preamble = PreambleMode::generate(SemanticPreamble::new(5, Vec::new()));
        let format16_bytes = write_format16_archive_to_vec(&preamble, &mut format16).unwrap();
        assert_parsed_case(format16_bytes, Revision::Format16, &generated);
    }
}

fn stored_format15_container(name: &str, payload: &[u8]) -> Vec<u8> {
    write_format15_to_vec(&Format15Document::new(
        1,
        vec![Format15Element::named(name, Some(payload.to_vec()))],
    ))
    .unwrap()
}

#[test]
fn generated_nested_trees_roundtrip_through_the_bounded_visitor() {
    for depth in 0..=6 {
        let mut document = stored_format15_container("leaf", b"offline-leaf");
        for _ in 0..depth {
            document = stored_format15_container("nested", &document);
        }
        let mut leaves = Vec::new();
        let stats = traverse(
            Cursor::new(document),
            ResourceLimits::new(depth + 2, 64, 1_048_576, 1_048_576, 200).unwrap(),
            |_, entry| {
                if entry.name == "nested" {
                    TraversalAction::Container(PayloadEncoding::Stored)
                } else {
                    TraversalAction::Leaf(PayloadEncoding::Stored)
                }
            },
            |visit| {
                if visit.entry.name == "leaf" {
                    leaves.push(visit.bytes.to_vec());
                }
            },
        )
        .unwrap();
        assert_eq!(leaves, [b"offline-leaf".to_vec()]);
        assert_eq!(stats.containers, depth + 1);
        assert_eq!(stats.visited_entries, depth + 1);
    }
}
