//! Lossless structural reader for the 32-bit V8 container family.
//!
//! The third 32-bit file-header word is deliberately retained as
//! `storage_version`. Existing module containers use observed values 0, 1 and
//! 2, while configuration envelopes use 5 and other values. The reader does
//! not reinterpret or normalize that word: raw headers remain the source of
//! truth until an artifact-specific layer supplies stronger semantics.

use std::{error::Error, fmt};

pub const SENTINEL: u32 = 0x7fff_ffff;
pub const DEFAULT_PAGE_SIZE: u32 = 512;
pub const FILE_HEADER_SIZE: usize = 16;
pub const BLOCK_HEADER_SIZE: usize = 31;
pub const ELEMENT_ADDRESS_SIZE: usize = 12;
pub const ELEMENT_HEADER_PREFIX_SIZE: usize = 20;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FileHeader {
    pub raw: [u8; FILE_HEADER_SIZE],
    pub next_page_address: u32,
    pub page_size: u32,
    pub storage_version: u32,
    pub reserved: u32,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BlockHeader {
    pub raw: [u8; BLOCK_HEADER_SIZE],
    pub data_size: u32,
    pub page_size: u32,
    pub next_page_address: u32,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ElementAddress {
    pub raw: [u8; ELEMENT_ADDRESS_SIZE],
    pub header_address: u32,
    pub data_address: Option<u32>,
    pub marker: u32,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Element {
    pub name: String,
    pub address: ElementAddress,
    pub header_block: BlockHeader,
    pub raw_header: Vec<u8>,
    pub data_block: Option<BlockHeader>,
    pub data: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Container {
    pub file_header: FileHeader,
    pub toc_block: BlockHeader,
    pub elements: Vec<Element>,
}

impl Container {
    pub fn parse(bytes: &[u8]) -> Result<Self, Format15Error> {
        parse(bytes)
    }

    #[must_use]
    pub fn element(&self, name: &str) -> Option<&Element> {
        self.elements.iter().find(|element| element.name == name)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Format15Error {
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
        found: u32,
    },
    InvalidBlockHeader {
        offset: usize,
    },
    InvalidHexField {
        offset: usize,
    },
    TocSizeNotDivisible {
        size: u32,
    },
    InvalidAddressTableMarker {
        index: usize,
        found: u32,
    },
    AbsentHeaderAddress {
        index: usize,
    },
    ChainedPagesUnsupported {
        offset: usize,
        next_page_address: u32,
    },
    BlockSizeMismatch {
        offset: usize,
        data_size: u32,
        page_size: u32,
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

impl fmt::Display for Format15Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputTooShort { minimum, actual } => write!(
                formatter,
                "container is too short: expected at least {minimum} bytes, got {actual}"
            ),
            Self::RangeOverflow { offset, length } => write!(
                formatter,
                "container range overflows usize: offset {offset}, length {length}"
            ),
            Self::RangeOutOfBounds {
                offset,
                length,
                input_length,
            } => write!(
                formatter,
                "container range exceeds input: offset {offset}, length {length}, input length {input_length}"
            ),
            Self::UnexpectedFileMarker { found } => write!(
                formatter,
                "unexpected file header next page marker 0x{found:08x}"
            ),
            Self::InvalidBlockHeader { offset } => {
                write!(formatter, "invalid block header at {offset}")
            }
            Self::InvalidHexField { offset } => {
                write!(
                    formatter,
                    "invalid hexadecimal block-header field at {offset}"
                )
            }
            Self::TocSizeNotDivisible { size } => write!(
                formatter,
                "TOC size {size} is not divisible by element address size {ELEMENT_ADDRESS_SIZE}"
            ),
            Self::InvalidAddressTableMarker { index, found } => write!(
                formatter,
                "invalid address table marker at entry {index}: 0x{found:08x}"
            ),
            Self::AbsentHeaderAddress { index } => {
                write!(formatter, "element {index} has an absent header address")
            }
            Self::ChainedPagesUnsupported {
                offset,
                next_page_address,
            } => write!(
                formatter,
                "multi-page V8 blocks are not supported yet: block at {offset} next page address 0x{next_page_address:08x}"
            ),
            Self::BlockSizeMismatch {
                offset,
                data_size,
                page_size,
            } => write!(
                formatter,
                "single-page block at {offset} declares data size {data_size} larger than page size {page_size}"
            ),
            Self::ElementHeaderTooShort { index, actual } => write!(
                formatter,
                "element {index} header is too short: expected at least {ELEMENT_HEADER_PREFIX_SIZE} bytes, got {actual}"
            ),
            Self::OddElementNameLength { index, actual } => write!(
                formatter,
                "element {index} UTF-16LE name region has odd length {actual}"
            ),
            Self::InvalidElementName { index } => {
                write!(formatter, "element {index} name is not valid UTF-16LE")
            }
        }
    }
}

impl Error for Format15Error {}

pub fn parse(bytes: &[u8]) -> Result<Container, Format15Error> {
    let minimum = FILE_HEADER_SIZE + BLOCK_HEADER_SIZE;
    if bytes.len() < minimum {
        return Err(Format15Error::InputTooShort {
            minimum,
            actual: bytes.len(),
        });
    }

    let file_header = read_file_header(bytes)?;
    if file_header.next_page_address != SENTINEL {
        return Err(Format15Error::UnexpectedFileMarker {
            found: file_header.next_page_address,
        });
    }

    let toc_block = read_block_header(bytes, FILE_HEADER_SIZE)?;
    let toc = read_single_page_payload(bytes, FILE_HEADER_SIZE, &toc_block)?;
    if toc.len() % ELEMENT_ADDRESS_SIZE != 0 {
        return Err(Format15Error::TocSizeNotDivisible {
            size: toc_block.data_size,
        });
    }

    let mut elements = Vec::with_capacity(toc.len() / ELEMENT_ADDRESS_SIZE);
    for (index, raw_address) in toc.chunks_exact(ELEMENT_ADDRESS_SIZE).enumerate() {
        let address = read_element_address(raw_address, index)?;
        let header_address = match address.header_address {
            SENTINEL => return Err(Format15Error::AbsentHeaderAddress { index }),
            value => value as usize,
        };
        let header_block = read_block_header(bytes, header_address)?;
        let raw_header = read_single_page_payload(bytes, header_address, &header_block)?;
        let name = element_name(&raw_header, index)?;

        let (data_block, data) = match address.data_address {
            None => (None, None),
            Some(data_address) => {
                let data_address = data_address as usize;
                let block = read_block_header(bytes, data_address)?;
                let payload = read_single_page_payload(bytes, data_address, &block)?;
                (Some(block), Some(payload))
            }
        };

        elements.push(Element {
            name,
            address,
            header_block,
            raw_header,
            data_block,
            data,
        });
    }

    Ok(Container {
        file_header,
        toc_block,
        elements,
    })
}

fn read_file_header(bytes: &[u8]) -> Result<FileHeader, Format15Error> {
    let raw: [u8; FILE_HEADER_SIZE] = checked_slice(bytes, 0, FILE_HEADER_SIZE)?
        .try_into()
        .expect("checked Format15 file-header length");
    Ok(FileHeader {
        next_page_address: u32::from_le_bytes(raw[0..4].try_into().unwrap()),
        page_size: u32::from_le_bytes(raw[4..8].try_into().unwrap()),
        storage_version: u32::from_le_bytes(raw[8..12].try_into().unwrap()),
        reserved: u32::from_le_bytes(raw[12..16].try_into().unwrap()),
        raw,
    })
}

fn read_block_header(bytes: &[u8], offset: usize) -> Result<BlockHeader, Format15Error> {
    let raw: [u8; BLOCK_HEADER_SIZE] = checked_slice(bytes, offset, BLOCK_HEADER_SIZE)?
        .try_into()
        .expect("checked Format15 block-header length");
    if raw[0..2] != *b"\r\n"
        || raw[10] != b' '
        || raw[19] != b' '
        || raw[28] != b' '
        || raw[29..31] != *b"\r\n"
    {
        return Err(Format15Error::InvalidBlockHeader { offset });
    }
    Ok(BlockHeader {
        data_size: parse_hex_u32(&raw[2..10], offset + 2)?,
        page_size: parse_hex_u32(&raw[11..19], offset + 11)?,
        next_page_address: parse_hex_u32(&raw[20..28], offset + 20)?,
        raw,
    })
}

fn read_element_address(bytes: &[u8], index: usize) -> Result<ElementAddress, Format15Error> {
    let raw: [u8; ELEMENT_ADDRESS_SIZE] = bytes
        .try_into()
        .expect("chunks_exact guarantees a Format15 address length");
    let header_address = u32::from_le_bytes(raw[0..4].try_into().unwrap());
    let raw_data_address = u32::from_le_bytes(raw[4..8].try_into().unwrap());
    let marker = u32::from_le_bytes(raw[8..12].try_into().unwrap());
    if marker != SENTINEL {
        return Err(Format15Error::InvalidAddressTableMarker {
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

fn read_single_page_payload(
    bytes: &[u8],
    offset: usize,
    header: &BlockHeader,
) -> Result<Vec<u8>, Format15Error> {
    if header.next_page_address != SENTINEL {
        return Err(Format15Error::ChainedPagesUnsupported {
            offset,
            next_page_address: header.next_page_address,
        });
    }
    if header.data_size > header.page_size {
        return Err(Format15Error::BlockSizeMismatch {
            offset,
            data_size: header.data_size,
            page_size: header.page_size,
        });
    }
    let payload_offset =
        offset
            .checked_add(BLOCK_HEADER_SIZE)
            .ok_or(Format15Error::RangeOverflow {
                offset,
                length: BLOCK_HEADER_SIZE,
            })?;
    checked_slice(bytes, payload_offset, header.page_size as usize)?;
    Ok(checked_slice(bytes, payload_offset, header.data_size as usize)?.to_vec())
}

fn element_name(header: &[u8], index: usize) -> Result<String, Format15Error> {
    if header.len() < ELEMENT_HEADER_PREFIX_SIZE {
        return Err(Format15Error::ElementHeaderTooShort {
            index,
            actual: header.len(),
        });
    }
    let name_region = &header[ELEMENT_HEADER_PREFIX_SIZE..];
    if !name_region.len().is_multiple_of(2) {
        return Err(Format15Error::OddElementNameLength {
            index,
            actual: name_region.len(),
        });
    }
    let units = name_region
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect::<Vec<_>>();
    String::from_utf16(&units).map_err(|_| Format15Error::InvalidElementName { index })
}

fn checked_slice(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8], Format15Error> {
    let end = offset
        .checked_add(length)
        .ok_or(Format15Error::RangeOverflow { offset, length })?;
    bytes
        .get(offset..end)
        .ok_or(Format15Error::RangeOutOfBounds {
            offset,
            length,
            input_length: bytes.len(),
        })
}

fn parse_hex_u32(bytes: &[u8], offset: usize) -> Result<u32, Format15Error> {
    let mut value = 0_u32;
    for byte in bytes {
        let digit = match byte {
            b'0'..=b'9' => u32::from(byte - b'0'),
            b'a'..=b'f' => u32::from(byte - b'a' + 10),
            b'A'..=b'F' => u32::from(byte - b'A' + 10),
            _ => return Err(Format15Error::InvalidHexField { offset }),
        };
        value = (value << 4) | digit;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::{BLOCK_HEADER_SIZE, ELEMENT_ADDRESS_SIZE, Format15Error, SENTINEL, parse};

    fn fixture() -> Vec<u8> {
        decode_base64(include_str!(
            "../../../tests/fixtures/cf/format15-clean-room.cf.b64"
        ))
    }

    #[test]
    fn parses_clean_room_fixture_and_preserves_order_and_raw_headers() {
        let bytes = fixture();
        let parsed = parse(&bytes).unwrap();

        assert_eq!(parsed.file_header.storage_version, 5);
        assert_eq!(parsed.file_header.raw, bytes[..16]);
        assert_eq!(parsed.toc_block.raw, bytes[16..16 + BLOCK_HEADER_SIZE]);
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
        assert!(parsed.elements.iter().all(|entry| entry.data.is_some()));
        assert_eq!(
            parsed.elements[0].address.raw,
            bytes[16 + BLOCK_HEADER_SIZE..16 + BLOCK_HEADER_SIZE + ELEMENT_ADDRESS_SIZE]
        );
        assert_eq!(
            parsed.elements[0].raw_header,
            &bytes[parsed.elements[0].address.header_address as usize + BLOCK_HEADER_SIZE
                ..parsed.elements[0].address.header_address as usize
                    + BLOCK_HEADER_SIZE
                    + parsed.elements[0].header_block.data_size as usize]
        );
    }

    #[test]
    fn preserves_observed_storage_version_words_zero_one_two_and_five() {
        for version in [0_u32, 1, 2, 5] {
            let mut bytes = fixture();
            bytes[8..12].copy_from_slice(&version.to_le_bytes());
            let parsed = parse(&bytes).unwrap();
            assert_eq!(parsed.file_header.storage_version, version);
            assert_eq!(&parsed.file_header.raw[8..12], &version.to_le_bytes());
        }
    }

    #[test]
    fn absent_data_sentinel_is_preserved_as_none() {
        let mut bytes = fixture();
        let first_address = 16 + BLOCK_HEADER_SIZE;
        bytes[first_address + 4..first_address + 8].copy_from_slice(&SENTINEL.to_le_bytes());

        let parsed = parse(&bytes).unwrap();

        assert_eq!(parsed.elements[0].address.data_address, None);
        assert_eq!(parsed.elements[0].data_block, None);
        assert_eq!(parsed.elements[0].data, None);
        assert!(parsed.elements[1].data.is_some());
    }

    #[test]
    fn chained_page_is_a_precise_typed_error_until_cf_003() {
        let mut bytes = fixture();
        let first_address = 16 + BLOCK_HEADER_SIZE;
        let data_address = u32::from_le_bytes(
            bytes[first_address + 4..first_address + 8]
                .try_into()
                .unwrap(),
        ) as usize;
        let next_page_address = data_address as u32 + 128;
        bytes[data_address + 20..data_address + 28]
            .copy_from_slice(format!("{next_page_address:08x}").as_bytes());

        assert_eq!(
            parse(&bytes).unwrap_err(),
            Format15Error::ChainedPagesUnsupported {
                offset: data_address,
                next_page_address,
            }
        );
    }

    #[test]
    fn invalid_address_marker_is_a_precise_typed_error() {
        let mut bytes = fixture();
        let first_address = 16 + BLOCK_HEADER_SIZE;
        bytes[first_address + 8..first_address + 12].copy_from_slice(&0_u32.to_le_bytes());

        assert_eq!(
            parse(&bytes).unwrap_err(),
            Format15Error::InvalidAddressTableMarker { index: 0, found: 0 }
        );
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
