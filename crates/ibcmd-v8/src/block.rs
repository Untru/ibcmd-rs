//! Safe chained-page reader for the 32-bit V8 block layout.
//!
//! A reader instance tracks page extents across every chain it reads. This
//! makes aliases and partial overlaps fail closed instead of letting two
//! logical entries silently share mutable binary storage.

use std::{collections::BTreeSet, error::Error, fmt};

pub const FORMAT15_BLOCK_HEADER_SIZE: usize = 31;
pub const FORMAT15_SENTINEL: u32 = 0x7fff_ffff;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BlockHeader {
    pub raw: [u8; FORMAT15_BLOCK_HEADER_SIZE],
    pub data_size: u32,
    pub page_size: u32,
    pub next_page_address: u32,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BlockPage {
    pub address: u32,
    pub header: BlockHeader,
    /// Number of logical bytes consumed from this page. The declared page
    /// size may be larger on the final page because the remaining bytes are
    /// padding.
    pub data_length: u32,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BlockChain {
    pub data_size: u32,
    pub pages: Vec<BlockPage>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BlockError {
    AddressOutOfRange {
        address: u32,
        required_end: u64,
        input_length: usize,
    },
    PageOutOfRange {
        address: u32,
        page_size: u32,
        required_end: u64,
        input_length: usize,
    },
    InvalidHeader {
        address: u32,
    },
    InvalidHexField {
        address: u32,
        field_offset: usize,
    },
    Cycle {
        start_address: u32,
        address: u32,
    },
    RepeatedAddress {
        address: u32,
        claimed_by: u32,
    },
    Overlap {
        address: u32,
        end: u64,
        conflicting_address: u32,
        conflicting_end: u64,
    },
    SizeMismatch {
        start_address: u32,
        declared_size: u32,
        collected_size: u32,
        next_page_address: Option<u32>,
    },
}

impl fmt::Display for BlockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AddressOutOfRange {
                address,
                required_end,
                input_length,
            } => write!(
                formatter,
                "block address 0x{address:08x} is out of range: header ends at {required_end}, input length {input_length}"
            ),
            Self::PageOutOfRange {
                address,
                page_size,
                required_end,
                input_length,
            } => write!(
                formatter,
                "block page at 0x{address:08x} with size {page_size} is out of range: page ends at {required_end}, input length {input_length}"
            ),
            Self::InvalidHeader { address } => {
                write!(formatter, "invalid block header at 0x{address:08x}")
            }
            Self::InvalidHexField {
                address,
                field_offset,
            } => write!(
                formatter,
                "invalid hexadecimal block-header field at 0x{address:08x}+{field_offset}"
            ),
            Self::Cycle {
                start_address,
                address,
            } => write!(
                formatter,
                "block chain at 0x{start_address:08x} contains a cycle through 0x{address:08x}"
            ),
            Self::RepeatedAddress {
                address,
                claimed_by,
            } => write!(
                formatter,
                "block page address 0x{address:08x} is already claimed by block 0x{claimed_by:08x}"
            ),
            Self::Overlap {
                address,
                end,
                conflicting_address,
                conflicting_end,
            } => write!(
                formatter,
                "block extent 0x{address:08x}..0x{end:x} overlaps 0x{conflicting_address:08x}..0x{conflicting_end:x}"
            ),
            Self::SizeMismatch {
                start_address,
                declared_size,
                collected_size,
                next_page_address,
            } => match next_page_address {
                Some(next) => write!(
                    formatter,
                    "block chain at 0x{start_address:08x} declares {declared_size} bytes, collected {collected_size}, but continues at 0x{next:08x}"
                ),
                None => write!(
                    formatter,
                    "block chain at 0x{start_address:08x} declares {declared_size} bytes, but ended after {collected_size}"
                ),
            },
        }
    }
}

impl Error for BlockError {}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct ClaimedRange {
    start: u32,
    end: u64,
    owner: u32,
}

pub struct BlockReader<'a> {
    bytes: &'a [u8],
    claimed: Vec<ClaimedRange>,
}

impl<'a> BlockReader<'a> {
    #[must_use]
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            claimed: Vec::new(),
        }
    }

    /// Reserve a non-block range such as the file header so block addresses
    /// cannot point into it.
    pub fn reserve(&mut self, address: u32, length: u32) -> Result<(), BlockError> {
        let end = u64::from(address) + u64::from(length);
        if end > self.bytes.len() as u64 {
            return Err(BlockError::AddressOutOfRange {
                address,
                required_end: end,
                input_length: self.bytes.len(),
            });
        }
        self.ensure_unclaimed(address, end, &[])?;
        self.claimed.push(ClaimedRange {
            start: address,
            end,
            owner: address,
        });
        Ok(())
    }

    pub fn read_chain(&mut self, start_address: u32) -> Result<BlockChain, BlockError> {
        let mut address = start_address;
        let mut seen = BTreeSet::new();
        let mut local_ranges = Vec::new();
        let mut pages = Vec::new();
        let mut data = Vec::new();
        let mut declared_size = None;

        loop {
            if !seen.insert(address) {
                return Err(BlockError::Cycle {
                    start_address,
                    address,
                });
            }

            let header_end = u64::from(address) + FORMAT15_BLOCK_HEADER_SIZE as u64;
            self.ensure_unclaimed(address, header_end, &local_ranges)?;
            let header = read_header(self.bytes, address)?;
            let page_end = header_end + u64::from(header.page_size);
            if page_end > self.bytes.len() as u64 {
                return Err(BlockError::PageOutOfRange {
                    address,
                    page_size: header.page_size,
                    required_end: page_end,
                    input_length: self.bytes.len(),
                });
            }
            self.ensure_unclaimed(address, page_end, &local_ranges)?;

            let total = *declared_size.get_or_insert(header.data_size);
            let collected = u32::try_from(data.len()).expect("Format15 data size is u32");
            let remaining = total.saturating_sub(collected);
            let data_length = remaining.min(header.page_size);
            let payload_start = usize::try_from(header_end)
                .expect("Format15 address and header size fit usize after range check");
            let payload_end = payload_start + data_length as usize;
            data.extend_from_slice(&self.bytes[payload_start..payload_end]);

            local_ranges.push(ClaimedRange {
                start: address,
                end: page_end,
                owner: start_address,
            });
            pages.push(BlockPage {
                address,
                header: header.clone(),
                data_length,
            });

            let collected = u32::try_from(data.len()).expect("Format15 data size is u32");
            let next =
                (header.next_page_address != FORMAT15_SENTINEL).then_some(header.next_page_address);
            if collected == total {
                if next.is_some() {
                    return Err(BlockError::SizeMismatch {
                        start_address,
                        declared_size: total,
                        collected_size: collected,
                        next_page_address: next,
                    });
                }
                self.claimed.extend(local_ranges);
                return Ok(BlockChain {
                    data_size: total,
                    pages,
                    data,
                });
            }

            let Some(next_address) = next else {
                return Err(BlockError::SizeMismatch {
                    start_address,
                    declared_size: total,
                    collected_size: collected,
                    next_page_address: None,
                });
            };
            if data_length == 0 {
                return Err(BlockError::SizeMismatch {
                    start_address,
                    declared_size: total,
                    collected_size: collected,
                    next_page_address: Some(next_address),
                });
            }
            address = next_address;
        }
    }

    fn ensure_unclaimed(
        &self,
        address: u32,
        end: u64,
        local: &[ClaimedRange],
    ) -> Result<(), BlockError> {
        for claimed in self.claimed.iter().chain(local) {
            if address == claimed.start {
                return Err(BlockError::RepeatedAddress {
                    address,
                    claimed_by: claimed.owner,
                });
            }
            if u64::from(address) < claimed.end && u64::from(claimed.start) < end {
                return Err(BlockError::Overlap {
                    address,
                    end,
                    conflicting_address: claimed.start,
                    conflicting_end: claimed.end,
                });
            }
        }
        Ok(())
    }
}

fn read_header(bytes: &[u8], address: u32) -> Result<BlockHeader, BlockError> {
    let start = address as usize;
    let required_end = u64::from(address) + FORMAT15_BLOCK_HEADER_SIZE as u64;
    let end = start
        .checked_add(FORMAT15_BLOCK_HEADER_SIZE)
        .filter(|end| *end <= bytes.len())
        .ok_or(BlockError::AddressOutOfRange {
            address,
            required_end,
            input_length: bytes.len(),
        })?;
    let raw: [u8; FORMAT15_BLOCK_HEADER_SIZE] = bytes[start..end]
        .try_into()
        .expect("checked Format15 block-header length");
    if raw[0..2] != *b"\r\n"
        || raw[10] != b' '
        || raw[19] != b' '
        || raw[28] != b' '
        || raw[29..31] != *b"\r\n"
    {
        return Err(BlockError::InvalidHeader { address });
    }
    Ok(BlockHeader {
        data_size: parse_hex_u32(&raw[2..10], address, 2)?,
        page_size: parse_hex_u32(&raw[11..19], address, 11)?,
        next_page_address: parse_hex_u32(&raw[20..28], address, 20)?,
        raw,
    })
}

fn parse_hex_u32(bytes: &[u8], address: u32, field_offset: usize) -> Result<u32, BlockError> {
    let mut value = 0_u32;
    for byte in bytes {
        let digit = match byte {
            b'0'..=b'9' => u32::from(byte - b'0'),
            b'a'..=b'f' => u32::from(byte - b'a' + 10),
            b'A'..=b'F' => u32::from(byte - b'A' + 10),
            _ => {
                return Err(BlockError::InvalidHexField {
                    address,
                    field_offset,
                });
            }
        };
        value = (value << 4) | digit;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::{BlockError, BlockReader, FORMAT15_BLOCK_HEADER_SIZE, FORMAT15_SENTINEL};

    fn put_page(
        bytes: &mut Vec<u8>,
        address: u32,
        data_size: u32,
        page_size: u32,
        next: u32,
        payload: &[u8],
    ) {
        assert!(payload.len() <= page_size as usize);
        let start = address as usize;
        let end = start + FORMAT15_BLOCK_HEADER_SIZE + page_size as usize;
        bytes.resize(bytes.len().max(end), 0);
        let header = format!("\r\n{data_size:08x} {page_size:08x} {next:08x} \r\n");
        assert_eq!(header.len(), FORMAT15_BLOCK_HEADER_SIZE);
        bytes[start..start + FORMAT15_BLOCK_HEADER_SIZE].copy_from_slice(header.as_bytes());
        bytes[start + FORMAT15_BLOCK_HEADER_SIZE
            ..start + FORMAT15_BLOCK_HEADER_SIZE + payload.len()]
            .copy_from_slice(payload);
    }

    #[test]
    fn reads_three_pages_and_preserves_every_raw_header() {
        let mut bytes = Vec::new();
        put_page(&mut bytes, 0, 8, 3, 40, b"abc");
        put_page(&mut bytes, 40, 0, 2, 80, b"de");
        put_page(&mut bytes, 80, 0, 4, FORMAT15_SENTINEL, b"fgh");

        let chain = BlockReader::new(&bytes).read_chain(0).unwrap();

        assert_eq!(chain.data_size, 8);
        assert_eq!(chain.data, b"abcdefgh");
        assert_eq!(chain.pages.len(), 3);
        assert_eq!(
            chain
                .pages
                .iter()
                .map(|page| (page.address, page.data_length))
                .collect::<Vec<_>>(),
            [(0, 3), (40, 2), (80, 3)]
        );
        assert!(
            chain
                .pages
                .iter()
                .all(|page| page.header.raw[0..2] == *b"\r\n")
        );
    }

    #[test]
    fn reports_cycle_at_the_repeated_page_address() {
        let mut bytes = Vec::new();
        put_page(&mut bytes, 0, 9, 3, 40, b"abc");
        put_page(&mut bytes, 40, 0, 3, 0, b"def");

        assert_eq!(
            BlockReader::new(&bytes).read_chain(0).unwrap_err(),
            BlockError::Cycle {
                start_address: 0,
                address: 0,
            }
        );
    }

    #[test]
    fn reports_partial_page_overlap_before_parsing_the_alias() {
        let mut bytes = Vec::new();
        put_page(&mut bytes, 0, 6, 3, 20, b"abc");

        assert_eq!(
            BlockReader::new(&bytes).read_chain(0).unwrap_err(),
            BlockError::Overlap {
                address: 20,
                end: 51,
                conflicting_address: 0,
                conflicting_end: 34,
            }
        );
    }

    #[test]
    fn reports_reuse_of_a_page_across_chains() {
        let mut bytes = Vec::new();
        put_page(&mut bytes, 0, 3, 3, FORMAT15_SENTINEL, b"abc");
        let mut reader = BlockReader::new(&bytes);
        reader.read_chain(0).unwrap();

        assert_eq!(
            reader.read_chain(0).unwrap_err(),
            BlockError::RepeatedAddress {
                address: 0,
                claimed_by: 0,
            }
        );
    }

    #[test]
    fn reports_out_of_range_address_and_page_extent() {
        let bytes = vec![0; 64];
        assert_eq!(
            BlockReader::new(&bytes).read_chain(60).unwrap_err(),
            BlockError::AddressOutOfRange {
                address: 60,
                required_end: 91,
                input_length: 64,
            }
        );

        let mut truncated = Vec::new();
        put_page(&mut truncated, 0, 4, 4, FORMAT15_SENTINEL, b"data");
        truncated.pop();
        assert_eq!(
            BlockReader::new(&truncated).read_chain(0).unwrap_err(),
            BlockError::PageOutOfRange {
                address: 0,
                page_size: 4,
                required_end: 35,
                input_length: 34,
            }
        );
    }

    #[test]
    fn reports_early_end_and_unexpected_continuation_as_size_mismatches() {
        let mut early = Vec::new();
        put_page(&mut early, 0, 6, 3, FORMAT15_SENTINEL, b"abc");
        assert_eq!(
            BlockReader::new(&early).read_chain(0).unwrap_err(),
            BlockError::SizeMismatch {
                start_address: 0,
                declared_size: 6,
                collected_size: 3,
                next_page_address: None,
            }
        );

        let mut extra = Vec::new();
        put_page(&mut extra, 0, 3, 3, 40, b"abc");
        assert_eq!(
            BlockReader::new(&extra).read_chain(0).unwrap_err(),
            BlockError::SizeMismatch {
                start_address: 0,
                declared_size: 3,
                collected_size: 3,
                next_page_address: Some(40),
            }
        );
    }

    #[test]
    fn accepts_a_payload_that_ends_exactly_at_the_input_boundary() {
        let mut bytes = Vec::new();
        put_page(&mut bytes, 0, 4, 4, FORMAT15_SENTINEL, b"data");

        let chain = BlockReader::new(&bytes).read_chain(0).unwrap();

        assert_eq!(chain.data, b"data");
        assert_eq!(bytes.len(), FORMAT15_BLOCK_HEADER_SIZE + 4);
    }
}
