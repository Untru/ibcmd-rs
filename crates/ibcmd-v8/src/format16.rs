//! Lossless structural reader for the 64-bit V8 container family.

use std::{error::Error, fmt};

use crate::{
    block::{BlockError, BlockPage64, BlockReader64},
    format15,
};

pub use crate::block::BlockHeader64 as BlockHeader;

pub const BASE_OFFSET: usize = 0x1359;
pub const SENTINEL: u64 = crate::block::FORMAT16_SENTINEL;
pub const DEFAULT_PAGE_SIZE: u32 = 512;
pub const FILE_HEADER_SIZE: usize = 20;
pub const BLOCK_HEADER_SIZE: usize = crate::block::FORMAT16_BLOCK_HEADER_SIZE;
pub const ELEMENT_ADDRESS_SIZE: usize = 24;
pub const ELEMENT_HEADER_PREFIX_SIZE: usize = 20;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FileHeader {
    pub raw: [u8; FILE_HEADER_SIZE],
    pub next_page_address: u64,
    pub page_size: u32,
    pub storage_version: u32,
    pub reserved: u32,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ElementAddress {
    pub raw: [u8; ELEMENT_ADDRESS_SIZE],
    pub header_address: u64,
    pub data_address: Option<u64>,
    pub marker: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Element {
    pub name: String,
    pub address: ElementAddress,
    pub header_block: BlockHeader,
    pub header_pages: Vec<BlockPage64>,
    pub raw_header: Vec<u8>,
    pub data_block: Option<BlockHeader>,
    pub data_pages: Option<Vec<BlockPage64>>,
    pub data: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Container {
    /// Absolute byte offset of the primary 64-bit container in the source.
    pub base_offset: usize,
    /// Exact bytes preceding the primary container.
    pub raw_preamble: Vec<u8>,
    /// Parsed semantic preamble when `base_offset` is non-zero.
    pub preamble: Option<Box<format15::Container>>,
    pub file_header: FileHeader,
    pub toc_block: BlockHeader,
    pub toc_pages: Vec<BlockPage64>,
    pub elements: Vec<Element>,
}

impl Container {
    pub fn parse(bytes: &[u8]) -> Result<Self, Format16Error> {
        parse(bytes)
    }

    pub fn parse_primary(bytes: &[u8]) -> Result<Self, Format16Error> {
        parse_primary(bytes)
    }

    #[must_use]
    pub fn element(&self, name: &str) -> Option<&Element> {
        self.elements.iter().find(|element| element.name == name)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Format16Error {
    BaseOffsetOutOfRange {
        base_offset: usize,
        input_length: usize,
    },
    InvalidPreamble(format15::Format15Error),
    InputTooShort {
        minimum: usize,
        actual: usize,
    },
    RangeOverflow {
        offset: usize,
        length: usize,
    },
    RangeOutOfBounds {
        offset: usize,
        length: usize,
        input_length: usize,
    },
    UnexpectedFileMarker {
        found: u64,
    },
    Block(BlockError),
    TocSizeNotDivisible {
        size: u64,
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
}

impl fmt::Display for Format16Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BaseOffsetOutOfRange {
                base_offset,
                input_length,
            } => write!(
                formatter,
                "Format16 base offset {base_offset} exceeds input length {input_length}"
            ),
            Self::InvalidPreamble(error) => {
                write!(formatter, "invalid Format16 preamble: {error}")
            }
            Self::InputTooShort { minimum, actual } => write!(
                formatter,
                "Format16 primary container is too short: expected at least {minimum} bytes, got {actual}"
            ),
            Self::RangeOverflow { offset, length } => write!(
                formatter,
                "Format16 range overflows usize: offset {offset}, length {length}"
            ),
            Self::RangeOutOfBounds {
                offset,
                length,
                input_length,
            } => write!(
                formatter,
                "Format16 range exceeds primary container: offset {offset}, length {length}, input length {input_length}"
            ),
            Self::UnexpectedFileMarker { found } => write!(
                formatter,
                "unexpected Format16 file header next page marker 0x{found:016x}"
            ),
            Self::Block(error) => error.fmt(formatter),
            Self::TocSizeNotDivisible { size } => write!(
                formatter,
                "Format16 TOC size {size} is not divisible by element address size {ELEMENT_ADDRESS_SIZE}"
            ),
            Self::InvalidAddressTableMarker { index, found } => write!(
                formatter,
                "invalid Format16 address table marker at entry {index}: 0x{found:016x}"
            ),
            Self::AbsentHeaderAddress { index } => {
                write!(
                    formatter,
                    "Format16 element {index} has an absent header address"
                )
            }
            Self::ElementHeaderTooShort { index, actual } => write!(
                formatter,
                "Format16 element {index} header is too short: expected at least {ELEMENT_HEADER_PREFIX_SIZE} bytes, got {actual}"
            ),
            Self::OddElementNameLength { index, actual } => write!(
                formatter,
                "Format16 element {index} UTF-16LE name region has odd length {actual}"
            ),
            Self::InvalidElementName { index } => {
                write!(
                    formatter,
                    "Format16 element {index} name is not valid UTF-16LE"
                )
            }
        }
    }
}

impl Error for Format16Error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidPreamble(error) => Some(error),
            Self::Block(error) => Some(error),
            _ => None,
        }
    }
}

impl From<BlockError> for Format16Error {
    fn from(error: BlockError) -> Self {
        Self::Block(error)
    }
}

/// Parse a complete Format16 artifact with its standard `0x1359` preamble.
pub fn parse(bytes: &[u8]) -> Result<Container, Format16Error> {
    parse_at(bytes, BASE_OFFSET)
}

/// Parse only the primary 64-bit container. This is useful for nested tests
/// and writer validation; complete artifacts should use [`parse`].
pub fn parse_primary(bytes: &[u8]) -> Result<Container, Format16Error> {
    parse_at(bytes, 0)
}

pub(crate) fn parse_at(bytes: &[u8], base_offset: usize) -> Result<Container, Format16Error> {
    if base_offset > bytes.len() {
        return Err(Format16Error::BaseOffsetOutOfRange {
            base_offset,
            input_length: bytes.len(),
        });
    }

    let (raw_preamble, preamble) = if base_offset == 0 {
        (Vec::new(), None)
    } else {
        let raw = bytes[..base_offset].to_vec();
        let parsed = format15::parse(&raw).map_err(Format16Error::InvalidPreamble)?;
        (raw, Some(Box::new(parsed)))
    };
    let primary = &bytes[base_offset..];
    let minimum = FILE_HEADER_SIZE + BLOCK_HEADER_SIZE;
    if primary.len() < minimum {
        return Err(Format16Error::InputTooShort {
            minimum,
            actual: primary.len(),
        });
    }

    let file_header = read_file_header(primary)?;
    if file_header.next_page_address != SENTINEL {
        return Err(Format16Error::UnexpectedFileMarker {
            found: file_header.next_page_address,
        });
    }

    let mut block_reader = BlockReader64::new(primary);
    block_reader.reserve(0, FILE_HEADER_SIZE as u64)?;
    let toc_chain = block_reader.read_chain(FILE_HEADER_SIZE as u64)?;
    let toc_block = toc_chain.pages[0].header.clone();
    let toc_pages = toc_chain.pages;
    let toc = toc_chain.data;
    if toc.len() % ELEMENT_ADDRESS_SIZE != 0 {
        return Err(Format16Error::TocSizeNotDivisible {
            size: toc_block.data_size,
        });
    }

    let mut elements = Vec::with_capacity(toc.len() / ELEMENT_ADDRESS_SIZE);
    for (index, raw_address) in toc.chunks_exact(ELEMENT_ADDRESS_SIZE).enumerate() {
        let address = read_element_address(raw_address, index)?;
        let header_address = match address.header_address {
            SENTINEL => return Err(Format16Error::AbsentHeaderAddress { index }),
            value => value,
        };
        let header_chain = block_reader.read_chain(header_address)?;
        let header_block = header_chain.pages[0].header.clone();
        let header_pages = header_chain.pages;
        let raw_header = header_chain.data;
        let name = element_name(&raw_header, index)?;

        let (data_block, data_pages, data) = match address.data_address {
            None => (None, None, None),
            Some(data_address) => {
                let chain = block_reader.read_chain(data_address)?;
                let block = chain.pages[0].header.clone();
                (Some(block), Some(chain.pages), Some(chain.data))
            }
        };

        elements.push(Element {
            name,
            address,
            header_block,
            header_pages,
            raw_header,
            data_block,
            data_pages,
            data,
        });
    }

    Ok(Container {
        base_offset,
        raw_preamble,
        preamble,
        file_header,
        toc_block,
        toc_pages,
        elements,
    })
}

fn read_file_header(bytes: &[u8]) -> Result<FileHeader, Format16Error> {
    let raw: [u8; FILE_HEADER_SIZE] = checked_slice(bytes, 0, FILE_HEADER_SIZE)?
        .try_into()
        .expect("checked Format16 file-header length");
    Ok(FileHeader {
        next_page_address: u64::from_le_bytes(raw[0..8].try_into().unwrap()),
        page_size: u32::from_le_bytes(raw[8..12].try_into().unwrap()),
        storage_version: u32::from_le_bytes(raw[12..16].try_into().unwrap()),
        reserved: u32::from_le_bytes(raw[16..20].try_into().unwrap()),
        raw,
    })
}

fn read_element_address(bytes: &[u8], index: usize) -> Result<ElementAddress, Format16Error> {
    let raw: [u8; ELEMENT_ADDRESS_SIZE] = bytes
        .try_into()
        .expect("chunks_exact guarantees a Format16 address length");
    let header_address = u64::from_le_bytes(raw[0..8].try_into().unwrap());
    let raw_data_address = u64::from_le_bytes(raw[8..16].try_into().unwrap());
    let marker = u64::from_le_bytes(raw[16..24].try_into().unwrap());
    if marker != SENTINEL {
        return Err(Format16Error::InvalidAddressTableMarker {
            index,
            found: marker,
        });
    }
    Ok(ElementAddress {
        raw,
        header_address,
        data_address: (raw_data_address != SENTINEL).then_some(raw_data_address),
        marker,
    })
}

fn element_name(header: &[u8], index: usize) -> Result<String, Format16Error> {
    if header.len() < ELEMENT_HEADER_PREFIX_SIZE {
        return Err(Format16Error::ElementHeaderTooShort {
            index,
            actual: header.len(),
        });
    }
    let name_region = &header[ELEMENT_HEADER_PREFIX_SIZE..];
    if !name_region.len().is_multiple_of(2) {
        return Err(Format16Error::OddElementNameLength {
            index,
            actual: name_region.len(),
        });
    }
    let units = name_region
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect::<Vec<_>>();
    String::from_utf16(&units).map_err(|_| Format16Error::InvalidElementName { index })
}

fn checked_slice(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8], Format16Error> {
    let end = offset
        .checked_add(length)
        .ok_or(Format16Error::RangeOverflow { offset, length })?;
    bytes
        .get(offset..end)
        .ok_or(Format16Error::RangeOutOfBounds {
            offset,
            length,
            input_length: bytes.len(),
        })
}

#[cfg(test)]
mod tests {
    use crate::block::BlockError;

    use super::{
        BASE_OFFSET, BLOCK_HEADER_SIZE, FILE_HEADER_SIZE, Format16Error, SENTINEL, parse,
        parse_primary,
    };

    fn fixture() -> Vec<u8> {
        decode_base64(include_str!(
            "../../../tests/fixtures/cf/format16-clean-room.cf.b64"
        ))
    }

    #[test]
    fn parses_clean_room_fixture_preamble_and_ordered_primary_entries() {
        let bytes = fixture();
        let parsed = parse(&bytes).unwrap();

        assert_eq!(parsed.base_offset, BASE_OFFSET);
        assert_eq!(parsed.raw_preamble, bytes[..BASE_OFFSET]);
        assert_eq!(parsed.file_header.storage_version, 5);
        assert_eq!(parsed.file_header.raw, bytes[BASE_OFFSET..BASE_OFFSET + 20]);
        assert_eq!(parsed.toc_block.data_size, 5 * 24);
        assert_eq!(
            parsed
                .elements
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            [
                "11111111-1111-4111-8111-111111111111",
                "22222222-2222-4222-8222-222222222222",
                "root",
                "version",
                "versions"
            ]
        );
        let preamble = parsed.preamble.as_ref().unwrap();
        assert_eq!(preamble.elements.len(), 5);
        assert!(parsed.elements.iter().all(|entry| entry.data.is_some()));

        let primary = parse_primary(&bytes[BASE_OFFSET..]).unwrap();
        assert_eq!(primary.base_offset, 0);
        assert!(primary.raw_preamble.is_empty());
        assert_eq!(primary.elements, parsed.elements);
    }

    #[test]
    fn truncated_first_block_header_fails_at_exact_primary_coordinate() {
        let mut bytes = fixture();
        bytes.truncate(BASE_OFFSET + FILE_HEADER_SIZE + BLOCK_HEADER_SIZE - 1);

        assert_eq!(
            parse(&bytes).unwrap_err(),
            Format16Error::InputTooShort {
                minimum: FILE_HEADER_SIZE + BLOCK_HEADER_SIZE,
                actual: FILE_HEADER_SIZE + BLOCK_HEADER_SIZE - 1,
            }
        );
    }

    #[test]
    fn overflowing_wide_element_address_is_rejected_before_conversion_to_usize() {
        let mut bytes = fixture();
        let first_address = BASE_OFFSET + FILE_HEADER_SIZE + BLOCK_HEADER_SIZE;
        bytes[first_address..first_address + 8].copy_from_slice(&(u64::MAX - 10).to_le_bytes());

        assert_eq!(
            parse(&bytes).unwrap_err(),
            Format16Error::Block(BlockError::RangeOverflow {
                address: u64::MAX - 10,
                length: BLOCK_HEADER_SIZE as u64,
            })
        );
    }

    #[test]
    fn absent_data_sentinel_remains_distinct_from_empty_payload() {
        let mut bytes = fixture();
        let first_address = BASE_OFFSET + FILE_HEADER_SIZE + BLOCK_HEADER_SIZE;
        bytes[first_address + 8..first_address + 16].copy_from_slice(&SENTINEL.to_le_bytes());

        let parsed = parse(&bytes).unwrap();

        assert_eq!(parsed.elements[0].address.data_address, None);
        assert_eq!(parsed.elements[0].data, None);
        assert!(parsed.elements[1].data.is_some());
    }

    #[test]
    fn follows_two_page_wide_data_chain_with_relative_addresses() {
        let mut bytes = fixture();
        let first_address = BASE_OFFSET + FILE_HEADER_SIZE + BLOCK_HEADER_SIZE;
        let data_address = u64::from_le_bytes(
            bytes[first_address + 8..first_address + 16]
                .try_into()
                .unwrap(),
        );
        let physical = BASE_OFFSET + data_address as usize;
        let data_size = u64::from_str_radix(
            std::str::from_utf8(&bytes[physical + 2..physical + 18]).unwrap(),
            16,
        )
        .unwrap();
        assert!(data_size > 1);
        let split = data_size / 2;
        let original = bytes
            [physical + BLOCK_HEADER_SIZE..physical + BLOCK_HEADER_SIZE + data_size as usize]
            .to_vec();
        let next_page_address = (bytes.len() - BASE_OFFSET) as u64;
        bytes[physical + 19..physical + 35].copy_from_slice(format!("{split:016x}").as_bytes());
        bytes[physical + 36..physical + 52]
            .copy_from_slice(format!("{next_page_address:016x}").as_bytes());
        let remaining = data_size - split;
        bytes.extend_from_slice(
            format!(
                "\r\n{zero:016x} {remaining:016x} {SENTINEL:016x} \r\n",
                zero = 0
            )
            .as_bytes(),
        );
        bytes.extend_from_slice(&original[split as usize..]);

        let parsed = parse(&bytes).unwrap();

        assert_eq!(
            parsed.elements[0].data.as_deref(),
            Some(original.as_slice())
        );
        let pages = parsed.elements[0].data_pages.as_ref().unwrap();
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].address, data_address);
        assert_eq!(pages[1].address, next_page_address);
        assert_eq!(pages[0].data_length, split);
        assert_eq!(pages[1].data_length, remaining);
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
