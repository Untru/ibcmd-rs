//! Mapping of a native CF archive into the neutral ordered `StorageImage`.
//!
//! Physical CF names remain exact logical keys, including native suffixes such
//! as `.3`. Multipart identity is a separate axis; one CF TOC record maps to
//! one physical single-part entry and duplicate names are rejected rather than
//! being guessed into a multipart sequence.

use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    io::{Read, Seek, SeekFrom},
};

use ibcmd_core::{
    artifact::StorageProfileId,
    limits::ResourceLimits,
    storage::{
        CompressionKind, MultipartIdentity, OpaqueStorageMetadata, StorageBuildError, StorageEntry,
        StorageImage, StorageKey, StorageName, StorageOrigin, StoragePayloads, StorageProvenance,
    },
};
use ibcmd_v8::{
    format::Revision,
    reader::{ContainerIndex, EntryIndex, ReaderError, StreamingReader},
};

use crate::payload::{PayloadDecodeError, PayloadDecoder, PayloadEncoding};

const ENTRY_ATTRIBUTES_MAGIC: &[u8] = b"ibcmd-cf-entry-v1\0";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CfDataState {
    Absent,
    Present,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CfEntryAttributes {
    data_state: CfDataState,
    raw_address: Vec<u8>,
}

impl CfEntryAttributes {
    pub fn new(
        data_state: CfDataState,
        raw_address: Vec<u8>,
    ) -> Result<Self, CfEntryAttributesError> {
        if raw_address.len() > usize::from(u16::MAX) {
            return Err(CfEntryAttributesError::AddressTooLong {
                actual: raw_address.len(),
            });
        }
        Ok(Self {
            data_state,
            raw_address,
        })
    }

    #[must_use]
    pub const fn data_state(&self) -> CfDataState {
        self.data_state
    }

    #[must_use]
    pub fn raw_address(&self) -> &[u8] {
        &self.raw_address
    }

    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes =
            Vec::with_capacity(ENTRY_ATTRIBUTES_MAGIC.len() + 3 + self.raw_address.len());
        bytes.extend_from_slice(ENTRY_ATTRIBUTES_MAGIC);
        bytes.push(match self.data_state {
            CfDataState::Absent => 0,
            CfDataState::Present => 1,
        });
        debug_assert!(u16::try_from(self.raw_address.len()).is_ok());
        bytes.extend_from_slice(&(self.raw_address.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&self.raw_address);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CfEntryAttributesError> {
        let prefix_end = ENTRY_ATTRIBUTES_MAGIC.len();
        if bytes.get(..prefix_end) != Some(ENTRY_ATTRIBUTES_MAGIC) {
            return Err(CfEntryAttributesError::InvalidMagic);
        }
        let state = *bytes
            .get(prefix_end)
            .ok_or(CfEntryAttributesError::Truncated)?;
        let data_state = match state {
            0 => CfDataState::Absent,
            1 => CfDataState::Present,
            value => return Err(CfEntryAttributesError::InvalidDataState { value }),
        };
        let length_end = prefix_end + 3;
        let length_bytes: [u8; 2] = bytes
            .get(prefix_end + 1..length_end)
            .ok_or(CfEntryAttributesError::Truncated)?
            .try_into()
            .expect("checked attribute length field");
        let declared = usize::from(u16::from_le_bytes(length_bytes));
        let raw_address = bytes
            .get(length_end..)
            .ok_or(CfEntryAttributesError::Truncated)?;
        if raw_address.len() != declared {
            return Err(CfEntryAttributesError::AddressLengthMismatch {
                declared,
                actual: raw_address.len(),
            });
        }
        Ok(Self {
            data_state,
            raw_address: raw_address.to_vec(),
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CfEntryAttributesError {
    InvalidMagic,
    Truncated,
    InvalidDataState { value: u8 },
    AddressTooLong { actual: usize },
    AddressLengthMismatch { declared: usize, actual: usize },
}

impl fmt::Display for CfEntryAttributesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic => write!(formatter, "storage entry has no CF attribute marker"),
            Self::Truncated => write!(formatter, "CF storage entry attributes are truncated"),
            Self::InvalidDataState { value } => {
                write!(
                    formatter,
                    "CF storage entry has invalid data-state byte {value}"
                )
            }
            Self::AddressTooLong { actual } => write!(
                formatter,
                "CF storage entry address record has {actual} bytes, exceeding 65535"
            ),
            Self::AddressLengthMismatch { declared, actual } => write!(
                formatter,
                "CF storage entry declares a {declared}-byte address record but retains {actual} bytes"
            ),
        }
    }
}

impl Error for CfEntryAttributesError {}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CfArchiveMetadata {
    revision: Revision,
    base_offset: u64,
    stream_length: u64,
    page_size: u32,
    storage_version: u32,
    reserved: u32,
    raw_file_header: Vec<u8>,
    raw_preamble: Vec<u8>,
}

impl CfArchiveMetadata {
    #[must_use]
    pub const fn revision(&self) -> Revision {
        self.revision
    }

    #[must_use]
    pub const fn base_offset(&self) -> u64 {
        self.base_offset
    }

    #[must_use]
    pub const fn stream_length(&self) -> u64 {
        self.stream_length
    }

    #[must_use]
    pub const fn page_size(&self) -> u32 {
        self.page_size
    }

    #[must_use]
    pub const fn storage_version(&self) -> u32 {
        self.storage_version
    }

    #[must_use]
    pub const fn reserved(&self) -> u32 {
        self.reserved
    }

    #[must_use]
    pub fn raw_file_header(&self) -> &[u8] {
        &self.raw_file_header
    }

    #[must_use]
    pub fn raw_preamble(&self) -> &[u8] {
        &self.raw_preamble
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CfArchive {
    metadata: CfArchiveMetadata,
    image: StorageImage,
}

impl CfArchive {
    #[must_use]
    pub const fn metadata(&self) -> &CfArchiveMetadata {
        &self.metadata
    }

    #[must_use]
    pub const fn image(&self) -> &StorageImage {
        &self.image
    }

    #[must_use]
    pub fn entry(&self, logical_key: &str) -> Option<&StorageEntry> {
        self.image
            .entries()
            .iter()
            .find(|entry| entry.logical_key().as_str() == logical_key)
    }

    #[must_use]
    pub fn into_parts(self) -> (CfArchiveMetadata, StorageImage) {
        (self.metadata, self.image)
    }
}

/// Decodes a CF archive using an explicit per-entry encoding resolver.
///
/// Compression is never inferred from payload bytes or a dotted name. A
/// profile adapter must classify every present entry as stored or raw DEFLATE.
pub fn decode_archive<R, C>(
    source: R,
    limits: ResourceLimits,
    source_profile: StorageProfileId,
    provenance: StorageProvenance,
    mut classify: C,
) -> Result<CfArchive, ArchiveDecodeError>
where
    R: Read + Seek,
    C: FnMut(&EntryIndex) -> PayloadEncoding,
{
    let mut reader = StreamingReader::open(source, limits).map_err(ArchiveDecodeError::Reader)?;
    reject_duplicate_names(reader.index())?;
    let metadata = read_metadata(&mut reader)?;
    let mut decoder = PayloadDecoder::new(limits);
    let mut entries = Vec::with_capacity(reader.index().entries.len());

    for index in 0..reader.index().entries.len() {
        let native = reader.index().entries[index].clone();
        let raw_header = reader
            .read_entry_header(index)
            .map_err(ArchiveDecodeError::Reader)?;
        let packed = reader
            .read_entry_data(index)
            .map_err(ArchiveDecodeError::Reader)?;
        let data_state = if packed.is_some() {
            CfDataState::Present
        } else {
            CfDataState::Absent
        };
        let attributes = CfEntryAttributes::new(data_state, native.raw_address.clone())
            .map_err(|source| ArchiveDecodeError::Attributes {
                index,
                name: native.name.clone(),
                source,
            })?
            .encode();
        let (packed, unpacked, compression) = match packed {
            None => (Vec::new(), Vec::new(), CompressionKind::stored()),
            Some(packed) => {
                let encoding = classify(&native);
                let decoded = decoder.decode(encoding, &packed).map_err(|source| {
                    ArchiveDecodeError::Payload {
                        index,
                        name: native.name.clone(),
                        source,
                    }
                })?;
                let compression = match encoding {
                    PayloadEncoding::Stored => CompressionKind::stored(),
                    PayloadEncoding::RawDeflate => CompressionKind::raw_deflate(),
                };
                (packed, decoded.into_bytes(), compression)
            }
        };
        let entry = StorageEntry::new(
            StorageName::new(&native.name).map_err(|source| ArchiveDecodeError::StorageEntry {
                index,
                name: native.name.clone(),
                source,
            })?,
            StorageKey::new(&native.name).map_err(|source| ArchiveDecodeError::StorageEntry {
                index,
                name: native.name.clone(),
                source,
            })?,
            MultipartIdentity::single(),
            OpaqueStorageMetadata::new(attributes, raw_header).map_err(|source| {
                ArchiveDecodeError::StorageEntry {
                    index,
                    name: native.name.clone(),
                    source,
                }
            })?,
            StoragePayloads::new(packed, unpacked).map_err(|source| {
                ArchiveDecodeError::StorageEntry {
                    index,
                    name: native.name.clone(),
                    source,
                }
            })?,
            compression,
            StorageOrigin::new(source_profile.clone(), provenance.clone()),
        )
        .map_err(|source| ArchiveDecodeError::StorageEntry {
            index,
            name: native.name,
            source,
        })?;
        entries.push(entry);
    }

    let image = StorageImage::new(entries).map_err(ArchiveDecodeError::StorageImage)?;
    Ok(CfArchive { metadata, image })
}

/// Convenience decoder for profiles where all present top-level entries use
/// one explicitly selected representation.
pub fn decode_archive_uniform<R: Read + Seek>(
    source: R,
    limits: ResourceLimits,
    source_profile: StorageProfileId,
    provenance: StorageProvenance,
    encoding: PayloadEncoding,
) -> Result<CfArchive, ArchiveDecodeError> {
    decode_archive(source, limits, source_profile, provenance, |_| encoding)
}

fn reject_duplicate_names(index: &ContainerIndex) -> Result<(), ArchiveDecodeError> {
    let mut first = BTreeMap::<&str, usize>::new();
    for (entry_index, entry) in index.entries.iter().enumerate() {
        if let Some(first_index) = first.insert(&entry.name, entry_index) {
            return Err(ArchiveDecodeError::DuplicateName {
                name: entry.name.clone(),
                first_index,
                duplicate_index: entry_index,
            });
        }
    }
    Ok(())
}

fn read_metadata<R: Read + Seek>(
    reader: &mut StreamingReader<R>,
) -> Result<CfArchiveMetadata, ArchiveDecodeError> {
    let index = reader.index();
    let revision = index.revision;
    let base_offset = index.base_offset;
    let stream_length = index.stream_length;
    let storage_version = index.storage_version;
    let raw_file_header = index.raw_file_header.clone();
    let (page_offset, reserved_offset) = match revision {
        Revision::Format15 => (4, 12),
        Revision::Format16 => (8, 16),
    };
    let page_size = u32::from_le_bytes(
        raw_file_header[page_offset..page_offset + 4]
            .try_into()
            .expect("indexed file header contains page size"),
    );
    let reserved = u32::from_le_bytes(
        raw_file_header[reserved_offset..reserved_offset + 4]
            .try_into()
            .expect("indexed file header contains reserved word"),
    );
    let preamble_length = usize::try_from(base_offset)
        .map_err(|_| ArchiveDecodeError::PreambleLengthOverflow { base_offset })?;
    let mut raw_preamble = vec![0; preamble_length];
    if preamble_length != 0 {
        reader
            .source_mut()
            .seek(SeekFrom::Start(0))
            .map_err(|error| ArchiveDecodeError::Io {
                operation: "seek to preamble",
                position: 0,
                kind: error.kind(),
            })?;
        reader
            .source_mut()
            .read_exact(&mut raw_preamble)
            .map_err(|error| ArchiveDecodeError::Io {
                operation: "read preamble",
                position: 0,
                kind: error.kind(),
            })?;
    }
    Ok(CfArchiveMetadata {
        revision,
        base_offset,
        stream_length,
        page_size,
        storage_version,
        reserved,
        raw_file_header,
        raw_preamble,
    })
}

#[derive(Debug)]
pub enum ArchiveDecodeError {
    Reader(ReaderError),
    DuplicateName {
        name: String,
        first_index: usize,
        duplicate_index: usize,
    },
    Payload {
        index: usize,
        name: String,
        source: PayloadDecodeError,
    },
    StorageEntry {
        index: usize,
        name: String,
        source: StorageBuildError,
    },
    StorageImage(StorageBuildError),
    Attributes {
        index: usize,
        name: String,
        source: CfEntryAttributesError,
    },
    PreambleLengthOverflow {
        base_offset: u64,
    },
    Io {
        operation: &'static str,
        position: u64,
        kind: std::io::ErrorKind,
    },
}

impl fmt::Display for ArchiveDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reader(source) => source.fmt(formatter),
            Self::DuplicateName {
                name,
                first_index,
                duplicate_index,
            } => write!(
                formatter,
                "CF archive entry `{name}` is duplicated at TOC indexes {first_index} and {duplicate_index}"
            ),
            Self::Payload {
                index,
                name,
                source,
            } => write!(
                formatter,
                "CF archive payload {index} (`{name}`) cannot be decoded: {source}"
            ),
            Self::StorageEntry {
                index,
                name,
                source,
            } => write!(
                formatter,
                "CF archive entry {index} (`{name}`) cannot enter StorageImage: {source}"
            ),
            Self::StorageImage(source) => {
                write!(
                    formatter,
                    "decoded CF archive is not a valid StorageImage: {source}"
                )
            }
            Self::Attributes {
                index,
                name,
                source,
            } => write!(
                formatter,
                "CF archive entry {index} (`{name}`) attributes cannot be retained: {source}"
            ),
            Self::PreambleLengthOverflow { base_offset } => write!(
                formatter,
                "CF archive base offset {base_offset} does not fit in memory address space"
            ),
            Self::Io {
                operation,
                position,
                kind,
            } => write!(
                formatter,
                "CF archive {operation} failed at offset {position}: {kind}"
            ),
        }
    }
}

impl Error for ArchiveDecodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Reader(source) => Some(source),
            Self::Payload { source, .. } => Some(source),
            Self::StorageEntry { source, .. } | Self::StorageImage(source) => Some(source),
            Self::Attributes { source, .. } => Some(source),
            Self::DuplicateName { .. } | Self::PreambleLengthOverflow { .. } | Self::Io { .. } => {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use ibcmd_core::{
        artifact::StorageProfileId, limits::ResourceLimits, storage::StorageProvenance,
    };
    use ibcmd_v8::{
        format::Revision,
        writer::{Format15Document, Format15Element, write_format15_to_vec},
    };

    use crate::payload::{PayloadEncoding, encode_payload};

    use super::{
        ArchiveDecodeError, CfDataState, CfEntryAttributes, decode_archive, decode_archive_uniform,
    };

    fn origin(revision: &str) -> (StorageProfileId, StorageProvenance) {
        (
            StorageProfileId::parse(&format!("storage:cf-{revision}-clean-room")).unwrap(),
            StorageProvenance::new("checked-in clean-room CF fixture").unwrap(),
        )
    }

    #[test]
    fn both_clean_room_revisions_map_deterministically_in_source_order() {
        let fixtures = [
            (
                "format15",
                decode_base64(include_str!(
                    "../../../tests/fixtures/cf/format15-clean-room.cf.b64"
                )),
                Revision::Format15,
                0,
            ),
            (
                "format16",
                decode_base64(include_str!(
                    "../../../tests/fixtures/cf/format16-clean-room.cf.b64"
                )),
                Revision::Format16,
                ibcmd_v8::format16::BASE_OFFSET,
            ),
        ];
        for (name, bytes, expected_revision, expected_base) in fixtures {
            let (profile, provenance) = origin(name);
            let first = decode_archive_uniform(
                Cursor::new(&bytes),
                ResourceLimits::default(),
                profile.clone(),
                provenance.clone(),
                PayloadEncoding::RawDeflate,
            )
            .unwrap();
            let second = decode_archive_uniform(
                Cursor::new(&bytes),
                ResourceLimits::default(),
                profile,
                provenance,
                PayloadEncoding::RawDeflate,
            )
            .unwrap();

            assert_eq!(first.metadata().revision(), expected_revision);
            assert_eq!(first.metadata().base_offset(), expected_base as u64);
            assert_eq!(first.metadata().raw_preamble().len(), expected_base);
            assert_eq!(first.image().sha256(), second.image().sha256());
            assert_eq!(
                first
                    .image()
                    .entries()
                    .iter()
                    .map(|entry| entry.logical_key().as_str())
                    .collect::<Vec<_>>(),
                [
                    "11111111-1111-4111-8111-111111111111",
                    "22222222-2222-4222-8222-222222222222",
                    "root",
                    "version",
                    "versions",
                ]
            );
            for required in [
                "11111111-1111-4111-8111-111111111111",
                "root",
                "version",
                "versions",
            ] {
                let entry = first.entry(required).unwrap();
                assert_eq!(entry.multipart().part_count(), 1);
                assert_eq!(entry.compression().as_str(), "raw-deflate");
                assert_ne!(entry.packed_payload(), entry.unpacked_payload());
                let attributes = CfEntryAttributes::decode(entry.attributes()).unwrap();
                assert_eq!(attributes.data_state(), CfDataState::Present);
                assert_eq!(
                    attributes.raw_address().len(),
                    if expected_revision == Revision::Format15 {
                        12
                    } else {
                        24
                    }
                );
            }
        }
    }

    #[test]
    fn suffix_absent_state_and_mixed_explicit_encodings_remain_distinct() {
        let limits = ResourceLimits::default();
        let compressed = encode_payload(PayloadEncoding::RawDeflate, b"inflated", limits).unwrap();
        let bytes = write_format15_to_vec(&Format15Document::new(
            5,
            vec![
                Format15Element::named(
                    "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa.3",
                    Some(b"stored".to_vec()),
                ),
                Format15Element::named("compressed", Some(compressed.clone())),
                Format15Element::named("absent", None),
            ],
        ))
        .unwrap();
        let (profile, provenance) = origin("mixed");
        let archive = decode_archive(Cursor::new(bytes), limits, profile, provenance, |entry| {
            if entry.name == "compressed" {
                PayloadEncoding::RawDeflate
            } else {
                PayloadEncoding::Stored
            }
        })
        .unwrap();

        let suffixed = archive
            .entry("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa.3")
            .unwrap();
        assert_eq!(
            suffixed.logical_name().as_str(),
            suffixed.logical_key().as_str()
        );
        assert_eq!(suffixed.multipart().part_count(), 1);
        assert_eq!(suffixed.unpacked_payload(), b"stored");
        assert_eq!(
            archive.entry("compressed").unwrap().packed_payload(),
            compressed
        );
        assert_eq!(
            archive.entry("compressed").unwrap().unpacked_payload(),
            b"inflated"
        );
        let absent = archive.entry("absent").unwrap();
        assert!(absent.packed_payload().is_empty());
        assert_eq!(
            CfEntryAttributes::decode(absent.attributes())
                .unwrap()
                .data_state(),
            CfDataState::Absent
        );
    }

    #[test]
    fn duplicate_native_names_fail_before_storage_image_construction() {
        let bytes = write_format15_to_vec(&Format15Document::new(
            5,
            vec![
                Format15Element::named("root", Some(b"first".to_vec())),
                Format15Element::named("root", Some(b"second".to_vec())),
            ],
        ))
        .unwrap();
        let (profile, provenance) = origin("duplicate");
        let error = decode_archive_uniform(
            Cursor::new(bytes),
            ResourceLimits::default(),
            profile,
            provenance,
            PayloadEncoding::Stored,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ArchiveDecodeError::DuplicateName {
                name,
                first_index: 0,
                duplicate_index: 1,
            } if name == "root"
        ));
    }

    #[test]
    fn public_cf_attributes_reject_unencodable_address_length() {
        let error = CfEntryAttributes::new(CfDataState::Present, vec![0; 65_536]).unwrap_err();
        assert!(error.to_string().contains("exceeding 65535"));
    }

    fn decode_base64(source: &str) -> Vec<u8> {
        let mut output = Vec::new();
        let mut buffer = 0_u32;
        let mut bits = 0_u8;
        for byte in source.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
            if byte == b'=' {
                break;
            }
            let value = match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                _ => panic!("invalid fixture Base64 byte"),
            };
            buffer = (buffer << 6) | u32::from(value);
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                output.push(((buffer >> bits) & 0xff) as u8);
                buffer &= if bits == 0 { 0 } else { (1_u32 << bits) - 1 };
            }
        }
        output
    }
}
