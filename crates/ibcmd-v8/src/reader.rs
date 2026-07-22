//! Bounded `Read + Seek` indexer for Format15 and Format16 containers.
//!
//! Opening a reader loads the TOC and element-name headers, but only indexes
//! data page headers. Entry payload bytes are fetched on an explicit read.

use std::{
    collections::BTreeSet,
    error::Error,
    fmt,
    io::{self, Read, Seek, SeekFrom},
};

use ibcmd_core::limits::{ResourceLimitError, ResourceLimits};

use crate::{format::Revision, format15, format16};

const ELEMENT_HEADER_PREFIX_SIZE: usize = 20;
const PAGE_MULTIPLIER: usize = 16;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PageIndex {
    /// Address relative to the primary container base.
    pub address: u64,
    /// Absolute stream offset of the page payload.
    pub payload_offset: u64,
    pub page_size: u64,
    pub data_length: u64,
    pub next_page_address: u64,
    pub raw_header: Vec<u8>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ChainIndex {
    pub data_size: u64,
    pub pages: Vec<PageIndex>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct EntryIndex {
    pub name: String,
    pub raw_address: Vec<u8>,
    pub header: ChainIndex,
    pub data: Option<ChainIndex>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ContainerIndex {
    pub revision: Revision,
    pub base_offset: u64,
    pub stream_length: u64,
    pub raw_file_header: Vec<u8>,
    pub storage_version: u32,
    pub entries: Vec<EntryIndex>,
    pub indexed_pages: usize,
    pub encoded_payload_bytes: u64,
}

impl ContainerIndex {
    #[must_use]
    pub fn entry(&self, name: &str) -> Option<(usize, &EntryIndex)> {
        self.entries
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.name == name)
    }
}

pub struct StreamingReader<R> {
    source: R,
    index: ContainerIndex,
    limits: ResourceLimits,
}

impl<R: Read + Seek> StreamingReader<R> {
    pub fn open(mut source: R, limits: ResourceLimits) -> Result<Self, ReaderError> {
        let index = build_index(&mut source, limits)?;
        Ok(Self {
            source,
            index,
            limits,
        })
    }

    pub fn open_default(source: R) -> Result<Self, ReaderError> {
        Self::open(source, ResourceLimits::default())
    }

    pub const fn index(&self) -> &ContainerIndex {
        &self.index
    }

    pub const fn source(&self) -> &R {
        &self.source
    }

    pub fn source_mut(&mut self) -> &mut R {
        &mut self.source
    }

    pub fn into_inner(self) -> R {
        self.source
    }

    pub fn read_entry_header(&mut self, index: usize) -> Result<Vec<u8>, ReaderError> {
        let chain = self
            .index
            .entries
            .get(index)
            .ok_or(ReaderError::EntryIndexOutOfRange {
                index,
                count: self.index.entries.len(),
            })?
            .header
            .clone();
        read_chain_payload(&mut self.source, &chain, self.limits.max_encoded_bytes())
    }

    pub fn read_entry_data(&mut self, index: usize) -> Result<Option<Vec<u8>>, ReaderError> {
        let chain = self
            .index
            .entries
            .get(index)
            .ok_or(ReaderError::EntryIndexOutOfRange {
                index,
                count: self.index.entries.len(),
            })?
            .data
            .clone();
        chain
            .as_ref()
            .map(|chain| {
                read_chain_payload(&mut self.source, chain, self.limits.max_encoded_bytes())
            })
            .transpose()
    }

    pub fn read_named_data(&mut self, name: &str) -> Result<Option<Vec<u8>>, ReaderError> {
        let Some((index, _)) = self.index.entry(name) else {
            return Ok(None);
        };
        self.read_entry_data(index)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ReaderError {
    Io {
        operation: &'static str,
        position: u64,
        kind: io::ErrorKind,
    },
    InputTooShort {
        minimum: u64,
        actual: u64,
    },
    UnknownSignature {
        found: u32,
    },
    UnexpectedFileMarker {
        revision: Revision,
        found: u64,
    },
    RangeOverflow {
        address: u64,
        length: u64,
    },
    RangeOutOfBounds {
        address: u64,
        length: u64,
        stream_length: u64,
    },
    InvalidBlockHeader {
        address: u64,
    },
    InvalidHexField {
        address: u64,
        field_offset: usize,
    },
    Cycle {
        start_address: u64,
        address: u64,
    },
    RepeatedAddress {
        address: u64,
        claimed_by: u64,
    },
    Overlap {
        address: u64,
        end: u64,
        conflicting_address: u64,
        conflicting_end: u64,
    },
    SizeMismatch {
        start_address: u64,
        declared_size: u64,
        collected_size: u64,
        next_page_address: Option<u64>,
    },
    TocSizeNotDivisible {
        size: u64,
        record_size: usize,
    },
    InvalidAddressTableMarker {
        index: usize,
        found: u64,
    },
    AbsentHeaderAddress {
        index: usize,
    },
    ElementHeaderTooShort {
        index: usize,
        actual: usize,
    },
    OddElementNameLength {
        index: usize,
        actual: usize,
    },
    InvalidElementName {
        index: usize,
    },
    PageCountExceeded {
        maximum: usize,
        actual: usize,
    },
    StructuralBytesExceeded {
        maximum: u64,
        actual: u64,
    },
    EntryIndexOutOfRange {
        index: usize,
        count: usize,
    },
    Limit(ResourceLimitError),
}

impl fmt::Display for ReaderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                position,
                kind,
            } => write!(
                formatter,
                "stream {operation} failed at offset {position}: {kind}"
            ),
            Self::InputTooShort { minimum, actual } => write!(
                formatter,
                "stream is too short: expected at least {minimum} bytes, got {actual}"
            ),
            Self::UnknownSignature { found } => write!(
                formatter,
                "unknown V8 stream signature: first word is 0x{found:08x}"
            ),
            Self::UnexpectedFileMarker { revision, found } => write!(
                formatter,
                "unexpected {revision:?} file marker 0x{found:016x}"
            ),
            Self::RangeOverflow { address, length } => write!(
                formatter,
                "stream range overflows u64: address {address}, length {length}"
            ),
            Self::RangeOutOfBounds {
                address,
                length,
                stream_length,
            } => write!(
                formatter,
                "stream range is out of bounds: address {address}, length {length}, stream length {stream_length}"
            ),
            Self::InvalidBlockHeader { address } => {
                write!(formatter, "invalid streamed block header at {address}")
            }
            Self::InvalidHexField {
                address,
                field_offset,
            } => write!(
                formatter,
                "invalid streamed block hex field at {address}+{field_offset}"
            ),
            Self::Cycle {
                start_address,
                address,
            } => write!(
                formatter,
                "streamed chain at {start_address} cycles through {address}"
            ),
            Self::RepeatedAddress {
                address,
                claimed_by,
            } => write!(
                formatter,
                "streamed page {address} is already claimed by chain {claimed_by}"
            ),
            Self::Overlap {
                address,
                end,
                conflicting_address,
                conflicting_end,
            } => write!(
                formatter,
                "streamed page extent {address}..{end} overlaps {conflicting_address}..{conflicting_end}"
            ),
            Self::SizeMismatch {
                start_address,
                declared_size,
                collected_size,
                next_page_address,
            } => write!(
                formatter,
                "streamed chain at {start_address} declares {declared_size} bytes, indexed {collected_size}, next {next_page_address:?}"
            ),
            Self::TocSizeNotDivisible { size, record_size } => write!(
                formatter,
                "streamed TOC size {size} is not divisible by record size {record_size}"
            ),
            Self::InvalidAddressTableMarker { index, found } => write!(
                formatter,
                "invalid streamed address marker at entry {index}: 0x{found:016x}"
            ),
            Self::AbsentHeaderAddress { index } => {
                write!(formatter, "streamed entry {index} has no header address")
            }
            Self::ElementHeaderTooShort { index, actual } => write!(
                formatter,
                "streamed entry {index} header is too short: {actual} bytes"
            ),
            Self::OddElementNameLength { index, actual } => write!(
                formatter,
                "streamed entry {index} UTF-16LE name has odd byte length {actual}"
            ),
            Self::InvalidElementName { index } => {
                write!(formatter, "streamed entry {index} name is invalid UTF-16LE")
            }
            Self::PageCountExceeded { maximum, actual } => write!(
                formatter,
                "streamed page index count {actual} exceeds maximum {maximum}"
            ),
            Self::StructuralBytesExceeded { maximum, actual } => write!(
                formatter,
                "streamed structural bytes {actual} exceed maximum {maximum}"
            ),
            Self::EntryIndexOutOfRange { index, count } => write!(
                formatter,
                "entry index {index} is out of range for {count} entries"
            ),
            Self::Limit(error) => error.fmt(formatter),
        }
    }
}

impl Error for ReaderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Limit(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ResourceLimitError> for ReaderError {
    fn from(error: ResourceLimitError) -> Self {
        Self::Limit(error)
    }
}

#[derive(Clone, Copy)]
struct Layout {
    revision: Revision,
    base_offset: u64,
    file_header_size: usize,
    block_header_size: usize,
    field_width: usize,
    address_width: usize,
    record_size: usize,
    sentinel: u64,
}

impl Layout {
    fn format15() -> Self {
        Self {
            revision: Revision::Format15,
            base_offset: 0,
            file_header_size: format15::FILE_HEADER_SIZE,
            block_header_size: format15::BLOCK_HEADER_SIZE,
            field_width: 8,
            address_width: 4,
            record_size: format15::ELEMENT_ADDRESS_SIZE,
            sentinel: u64::from(format15::SENTINEL),
        }
    }

    fn format16() -> Self {
        Self {
            revision: Revision::Format16,
            base_offset: format16::BASE_OFFSET as u64,
            file_header_size: format16::FILE_HEADER_SIZE,
            block_header_size: format16::BLOCK_HEADER_SIZE,
            field_width: 16,
            address_width: 8,
            record_size: format16::ELEMENT_ADDRESS_SIZE,
            sentinel: format16::SENTINEL,
        }
    }
}

#[derive(Clone, Copy)]
struct BlockFields {
    data_size: u64,
    page_size: u64,
    next_page_address: u64,
}

#[derive(Clone, Copy)]
struct ClaimedRange {
    start: u64,
    end: u64,
    owner: u64,
}

struct IndexState {
    claims: Vec<ClaimedRange>,
    pages: usize,
    maximum_pages: usize,
}

fn build_index<R: Read + Seek>(
    source: &mut R,
    limits: ResourceLimits,
) -> Result<ContainerIndex, ReaderError> {
    let stream_length = stream_length(source)?;
    let layout = detect_layout(source, stream_length)?;
    let raw_file_header = read_vec_at(
        source,
        layout.base_offset,
        layout.file_header_size,
        stream_length,
    )?;
    let marker = read_address(&raw_file_header, 0, layout.address_width);
    if marker != layout.sentinel {
        return Err(ReaderError::UnexpectedFileMarker {
            revision: layout.revision,
            found: marker,
        });
    }
    let storage_offset = layout.address_width + 4;
    let storage_version = u32::from_le_bytes(
        raw_file_header[storage_offset..storage_offset + 4]
            .try_into()
            .unwrap(),
    );

    let maximum_pages = limits.max_entries().saturating_mul(PAGE_MULTIPLIER).max(1);
    let mut state = IndexState {
        claims: vec![ClaimedRange {
            start: layout.base_offset,
            end: layout.base_offset + layout.file_header_size as u64,
            owner: 0,
        }],
        pages: 0,
        maximum_pages,
    };
    let toc = index_chain(
        source,
        stream_length,
        layout,
        layout.file_header_size as u64,
        &mut state,
    )?;
    let mut structural_bytes = toc.data_size;
    ensure_structural_limit(structural_bytes, limits)?;
    let toc_bytes = read_chain_payload(source, &toc, limits.max_encoded_bytes())?;
    if toc_bytes.len() % layout.record_size != 0 {
        return Err(ReaderError::TocSizeNotDivisible {
            size: toc.data_size,
            record_size: layout.record_size,
        });
    }
    let entry_count = toc_bytes.len() / layout.record_size;
    if entry_count > limits.max_entries() {
        return Err(ResourceLimitError::EntryCountExceeded {
            maximum: limits.max_entries(),
            actual: entry_count,
        }
        .into());
    }

    let mut entries = Vec::with_capacity(entry_count);
    let mut encoded_payload_bytes = 0_u64;
    for (index, raw) in toc_bytes.chunks_exact(layout.record_size).enumerate() {
        let header_address = read_address(raw, 0, layout.address_width);
        let data_address = read_address(raw, layout.address_width, layout.address_width);
        let marker = read_address(raw, layout.address_width * 2, layout.address_width);
        if marker != layout.sentinel {
            return Err(ReaderError::InvalidAddressTableMarker {
                index,
                found: marker,
            });
        }
        if header_address == layout.sentinel {
            return Err(ReaderError::AbsentHeaderAddress { index });
        }

        let header = index_chain(source, stream_length, layout, header_address, &mut state)?;
        structural_bytes = structural_bytes.checked_add(header.data_size).ok_or(
            ReaderError::StructuralBytesExceeded {
                maximum: limits.max_encoded_bytes(),
                actual: u64::MAX,
            },
        )?;
        ensure_structural_limit(structural_bytes, limits)?;
        let header_bytes = read_chain_payload(source, &header, limits.max_encoded_bytes())?;
        let name = element_name(&header_bytes, index)?;

        let data = if data_address == layout.sentinel {
            None
        } else {
            let chain = index_chain(source, stream_length, layout, data_address, &mut state)?;
            encoded_payload_bytes = encoded_payload_bytes.checked_add(chain.data_size).ok_or(
                ResourceLimitError::EncodedBytesExceeded {
                    maximum: limits.max_encoded_bytes(),
                    actual: u64::MAX,
                },
            )?;
            if encoded_payload_bytes > limits.max_encoded_bytes() {
                return Err(ResourceLimitError::EncodedBytesExceeded {
                    maximum: limits.max_encoded_bytes(),
                    actual: encoded_payload_bytes,
                }
                .into());
            }
            Some(chain)
        };
        entries.push(EntryIndex {
            name,
            raw_address: raw.to_vec(),
            header,
            data,
        });
    }

    Ok(ContainerIndex {
        revision: layout.revision,
        base_offset: layout.base_offset,
        stream_length,
        raw_file_header,
        storage_version,
        entries,
        indexed_pages: state.pages,
        encoded_payload_bytes,
    })
}

fn detect_layout<R: Read + Seek>(
    source: &mut R,
    stream_length: u64,
) -> Result<Layout, ReaderError> {
    if stream_length < 4 {
        return Err(ReaderError::InputTooShort {
            minimum: 4,
            actual: stream_length,
        });
    }
    let first = read_vec_at(source, 0, 4, stream_length)?;
    let marker = u32::from_le_bytes(first.try_into().unwrap());
    if marker != format15::SENTINEL {
        return Err(ReaderError::UnknownSignature { found: marker });
    }

    let wide_position = format16::BASE_OFFSET as u64;
    if stream_length >= wide_position + 8 {
        let wide = read_vec_at(source, wide_position, 8, stream_length)?;
        if u64::from_le_bytes(wide.try_into().unwrap()) == format16::SENTINEL {
            return Ok(Layout::format16());
        }
    }
    Ok(Layout::format15())
}

fn index_chain<R: Read + Seek>(
    source: &mut R,
    stream_length: u64,
    layout: Layout,
    start_address: u64,
    state: &mut IndexState,
) -> Result<ChainIndex, ReaderError> {
    let mut address = start_address;
    let mut seen = BTreeSet::new();
    let mut local = Vec::new();
    let mut pages = Vec::new();
    let mut declared_size = None;
    let mut collected = 0_u64;

    loop {
        if !seen.insert(address) {
            return Err(ReaderError::Cycle {
                start_address,
                address,
            });
        }
        let physical =
            layout
                .base_offset
                .checked_add(address)
                .ok_or(ReaderError::RangeOverflow {
                    address: layout.base_offset,
                    length: address,
                })?;
        let header_end = physical
            .checked_add(layout.block_header_size as u64)
            .ok_or(ReaderError::RangeOverflow {
                address: physical,
                length: layout.block_header_size as u64,
            })?;
        ensure_unclaimed(physical, header_end, &state.claims, &local)?;
        let raw_header = read_vec_at(source, physical, layout.block_header_size, stream_length)?;
        let fields = parse_block_header(&raw_header, address, layout)?;
        let page_end =
            header_end
                .checked_add(fields.page_size)
                .ok_or(ReaderError::RangeOverflow {
                    address: header_end,
                    length: fields.page_size,
                })?;
        if page_end > stream_length {
            return Err(ReaderError::RangeOutOfBounds {
                address: physical,
                length: layout.block_header_size as u64 + fields.page_size,
                stream_length,
            });
        }
        ensure_unclaimed(physical, page_end, &state.claims, &local)?;

        state.pages = state
            .pages
            .checked_add(1)
            .ok_or(ReaderError::PageCountExceeded {
                maximum: state.maximum_pages,
                actual: usize::MAX,
            })?;
        if state.pages > state.maximum_pages {
            return Err(ReaderError::PageCountExceeded {
                maximum: state.maximum_pages,
                actual: state.pages,
            });
        }

        let total = *declared_size.get_or_insert(fields.data_size);
        let remaining = total.saturating_sub(collected);
        let data_length = remaining.min(fields.page_size);
        collected = collected
            .checked_add(data_length)
            .ok_or(ReaderError::SizeMismatch {
                start_address,
                declared_size: total,
                collected_size: u64::MAX,
                next_page_address: None,
            })?;
        local.push(ClaimedRange {
            start: physical,
            end: page_end,
            owner: start_address,
        });
        pages.push(PageIndex {
            address,
            payload_offset: header_end,
            page_size: fields.page_size,
            data_length,
            next_page_address: fields.next_page_address,
            raw_header,
        });

        let next =
            (fields.next_page_address != layout.sentinel).then_some(fields.next_page_address);
        if collected == total {
            if next.is_some() {
                return Err(ReaderError::SizeMismatch {
                    start_address,
                    declared_size: total,
                    collected_size: collected,
                    next_page_address: next,
                });
            }
            state.claims.extend(local);
            return Ok(ChainIndex {
                data_size: total,
                pages,
            });
        }
        let Some(next_address) = next else {
            return Err(ReaderError::SizeMismatch {
                start_address,
                declared_size: total,
                collected_size: collected,
                next_page_address: None,
            });
        };
        if data_length == 0 {
            return Err(ReaderError::SizeMismatch {
                start_address,
                declared_size: total,
                collected_size: collected,
                next_page_address: Some(next_address),
            });
        }
        address = next_address;
    }
}

fn parse_block_header(
    raw: &[u8],
    address: u64,
    layout: Layout,
) -> Result<BlockFields, ReaderError> {
    let width = layout.field_width;
    if raw.len() != 3 * width + 7
        || raw[0..2] != *b"\r\n"
        || raw[2 + width] != b' '
        || raw[3 + 2 * width] != b' '
        || raw[4 + 3 * width] != b' '
        || raw[5 + 3 * width..] != *b"\r\n"
    {
        return Err(ReaderError::InvalidBlockHeader { address });
    }
    Ok(BlockFields {
        data_size: parse_hex(&raw[2..2 + width], address, 2)?,
        page_size: parse_hex(&raw[3 + width..3 + 2 * width], address, 3 + width)?,
        next_page_address: parse_hex(&raw[4 + 2 * width..4 + 3 * width], address, 4 + 2 * width)?,
    })
}

fn parse_hex(bytes: &[u8], address: u64, field_offset: usize) -> Result<u64, ReaderError> {
    let mut value = 0_u64;
    for byte in bytes {
        let digit = match byte {
            b'0'..=b'9' => u64::from(byte - b'0'),
            b'a'..=b'f' => u64::from(byte - b'a' + 10),
            b'A'..=b'F' => u64::from(byte - b'A' + 10),
            _ => {
                return Err(ReaderError::InvalidHexField {
                    address,
                    field_offset,
                });
            }
        };
        value = (value << 4) | digit;
    }
    Ok(value)
}

fn read_address(bytes: &[u8], offset: usize, width: usize) -> u64 {
    match width {
        4 => u64::from(u32::from_le_bytes(
            bytes[offset..offset + 4].try_into().unwrap(),
        )),
        8 => u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()),
        _ => unreachable!("supported V8 address widths are 4 and 8"),
    }
}

fn element_name(header: &[u8], index: usize) -> Result<String, ReaderError> {
    if header.len() < ELEMENT_HEADER_PREFIX_SIZE {
        return Err(ReaderError::ElementHeaderTooShort {
            index,
            actual: header.len(),
        });
    }
    let name = &header[ELEMENT_HEADER_PREFIX_SIZE..];
    if !name.len().is_multiple_of(2) {
        return Err(ReaderError::OddElementNameLength {
            index,
            actual: name.len(),
        });
    }
    let units = name
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect::<Vec<_>>();
    String::from_utf16(&units).map_err(|_| ReaderError::InvalidElementName { index })
}

fn read_chain_payload<R: Read + Seek>(
    source: &mut R,
    chain: &ChainIndex,
    maximum: u64,
) -> Result<Vec<u8>, ReaderError> {
    if chain.data_size > maximum {
        return Err(ResourceLimitError::EncodedBytesExceeded {
            maximum,
            actual: chain.data_size,
        }
        .into());
    }
    let capacity = usize::try_from(chain.data_size).map_err(|_| {
        ReaderError::Limit(ResourceLimitError::EncodedBytesExceeded {
            maximum,
            actual: chain.data_size,
        })
    })?;
    let mut output = Vec::with_capacity(capacity);
    for page in &chain.pages {
        let length = usize::try_from(page.data_length).map_err(|_| {
            ReaderError::Limit(ResourceLimitError::EncodedBytesExceeded {
                maximum,
                actual: page.data_length,
            })
        })?;
        let start = output.len();
        let end = start
            .checked_add(length)
            .ok_or(ResourceLimitError::EncodedBytesExceeded {
                maximum,
                actual: u64::MAX,
            })?;
        output.resize(end, 0);
        read_exact_at(source, page.payload_offset, &mut output[start..end])?;
    }
    Ok(output)
}

fn ensure_structural_limit(actual: u64, limits: ResourceLimits) -> Result<(), ReaderError> {
    if actual > limits.max_encoded_bytes() {
        return Err(ReaderError::StructuralBytesExceeded {
            maximum: limits.max_encoded_bytes(),
            actual,
        });
    }
    Ok(())
}

fn ensure_unclaimed(
    address: u64,
    end: u64,
    global: &[ClaimedRange],
    local: &[ClaimedRange],
) -> Result<(), ReaderError> {
    for claimed in global.iter().chain(local) {
        if address == claimed.start {
            return Err(ReaderError::RepeatedAddress {
                address,
                claimed_by: claimed.owner,
            });
        }
        if address < claimed.end && claimed.start < end {
            return Err(ReaderError::Overlap {
                address,
                end,
                conflicting_address: claimed.start,
                conflicting_end: claimed.end,
            });
        }
    }
    Ok(())
}

fn stream_length<R: Seek>(source: &mut R) -> Result<u64, ReaderError> {
    source
        .seek(SeekFrom::End(0))
        .map_err(|error| ReaderError::Io {
            operation: "length seek",
            position: 0,
            kind: error.kind(),
        })
}

fn read_vec_at<R: Read + Seek>(
    source: &mut R,
    position: u64,
    length: usize,
    stream_length: u64,
) -> Result<Vec<u8>, ReaderError> {
    let length_u64 = length as u64;
    let end = position
        .checked_add(length_u64)
        .ok_or(ReaderError::RangeOverflow {
            address: position,
            length: length_u64,
        })?;
    if end > stream_length {
        return Err(ReaderError::RangeOutOfBounds {
            address: position,
            length: length_u64,
            stream_length,
        });
    }
    let mut bytes = vec![0; length];
    read_exact_at(source, position, &mut bytes)?;
    Ok(bytes)
}

fn read_exact_at<R: Read + Seek>(
    source: &mut R,
    position: u64,
    bytes: &mut [u8],
) -> Result<(), ReaderError> {
    source
        .seek(SeekFrom::Start(position))
        .map_err(|error| ReaderError::Io {
            operation: "seek",
            position,
            kind: error.kind(),
        })?;
    source.read_exact(bytes).map_err(|error| ReaderError::Io {
        operation: "read",
        position,
        kind: error.kind(),
    })
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read, Seek, SeekFrom};

    use ibcmd_core::limits::ResourceLimits;

    use super::StreamingReader;
    use crate::format15;

    #[derive(Default)]
    struct SparseSource {
        length: u64,
        position: u64,
        segments: Vec<(u64, Vec<u8>)>,
        bytes_read: u64,
        maximum_request: usize,
    }

    impl SparseSource {
        fn new(length: u64, segments: Vec<(u64, Vec<u8>)>) -> Self {
            Self {
                length,
                segments,
                ..Self::default()
            }
        }
    }

    impl Read for SparseSource {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let available = self.length.saturating_sub(self.position);
            let count = usize::try_from(available.min(buffer.len() as u64)).unwrap();
            buffer[..count].fill(0);
            for (start, data) in &self.segments {
                let segment_end = *start + data.len() as u64;
                let read_end = self.position + count as u64;
                let overlap_start = self.position.max(*start);
                let overlap_end = read_end.min(segment_end);
                if overlap_start < overlap_end {
                    let destination = (overlap_start - self.position) as usize;
                    let source = (overlap_start - *start) as usize;
                    let length = (overlap_end - overlap_start) as usize;
                    buffer[destination..destination + length]
                        .copy_from_slice(&data[source..source + length]);
                }
            }
            self.position += count as u64;
            self.bytes_read += count as u64;
            self.maximum_request = self.maximum_request.max(buffer.len());
            Ok(count)
        }
    }

    impl Seek for SparseSource {
        fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
            let next = match position {
                SeekFrom::Start(value) => i128::from(value),
                SeekFrom::End(value) => i128::from(self.length) + i128::from(value),
                SeekFrom::Current(value) => i128::from(self.position) + i128::from(value),
            };
            if next < 0 || next > i128::from(u64::MAX) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "invalid sparse seek",
                ));
            }
            self.position = next as u64;
            Ok(self.position)
        }
    }

    #[test]
    fn sparse_gigabyte_page_is_indexed_and_selected_without_full_read() {
        let page_size = 1_u64 << 30;
        let header_address = 100_u32;
        let data_address = 200_u32;
        let element_header = element_header("huge");
        let mut file_header = Vec::new();
        file_header.extend_from_slice(&format15::SENTINEL.to_le_bytes());
        file_header.extend_from_slice(&512_u32.to_le_bytes());
        file_header.extend_from_slice(&1_u32.to_le_bytes());
        file_header.extend_from_slice(&0_u32.to_le_bytes());

        let mut toc = Vec::new();
        toc.extend_from_slice(&header_address.to_le_bytes());
        toc.extend_from_slice(&data_address.to_le_bytes());
        toc.extend_from_slice(&format15::SENTINEL.to_le_bytes());
        let segments = vec![
            (0, file_header),
            (16, block_header(12, 12, u64::from(format15::SENTINEL), 8)),
            (47, toc),
            (
                u64::from(header_address),
                block_header(
                    element_header.len() as u64,
                    element_header.len() as u64,
                    u64::from(format15::SENTINEL),
                    8,
                ),
            ),
            (u64::from(header_address) + 31, element_header),
            (
                u64::from(data_address),
                block_header(4, page_size, u64::from(format15::SENTINEL), 8),
            ),
            (u64::from(data_address) + 31, b"lazy".to_vec()),
        ];
        let length = u64::from(data_address) + 31 + page_size;
        let limits = ResourceLimits::new(8, 64, 1_048_576, 1_048_576, 200).unwrap();
        let mut reader =
            StreamingReader::open(SparseSource::new(length, segments), limits).unwrap();

        assert_eq!(reader.index().entries[0].name, "huge");
        assert_eq!(reader.index().stream_length, length);
        assert!(reader.source().bytes_read < 512);
        assert!(reader.source().maximum_request < 128);

        assert_eq!(
            reader.read_named_data("huge").unwrap(),
            Some(b"lazy".to_vec())
        );
        assert!(reader.source().bytes_read < 512);
        assert!(reader.source().maximum_request < 128);
    }

    #[test]
    fn format16_fixture_is_indexed_and_one_payload_is_read_lazily() {
        let bytes = decode_base64(include_str!(
            "../../../tests/fixtures/cf/format16-clean-room.cf.b64"
        ));
        let expected = crate::format16::parse(&bytes)
            .unwrap()
            .element("root")
            .unwrap()
            .data
            .clone()
            .unwrap();
        let mut reader = StreamingReader::open_default(Cursor::new(bytes)).unwrap();

        assert_eq!(reader.index().revision, crate::format::Revision::Format16);
        assert_eq!(reader.index().entries.len(), 5);
        assert_eq!(reader.read_named_data("root").unwrap(), Some(expected));
        assert_eq!(reader.read_named_data("missing").unwrap(), None);
    }

    fn block_header(data: u64, page: u64, next: u64, width: usize) -> Vec<u8> {
        format!(
            "\r\n{data:0width$x} {page:0width$x} {next:0width$x} \r\n",
            width = width
        )
        .into_bytes()
    }

    fn element_header(name: &str) -> Vec<u8> {
        let mut bytes = vec![0; 20];
        bytes.extend(name.encode_utf16().flat_map(u16::to_le_bytes));
        bytes.extend_from_slice(&[0; 4]);
        bytes
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
