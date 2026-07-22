//! Deterministic streaming writers for V8 containers.

use std::{error::Error, fmt, io::Write};

use crate::format15;

const ZERO_BUFFER: [u8; 8192] = [0; 8192];

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Format15Element {
    /// Raw logical element-header bytes, including the 20-byte prefix and
    /// UTF-16LE name region.
    pub header: Vec<u8>,
    /// `None` writes the absent-data sentinel; `Some([])` writes an explicit
    /// empty data block.
    pub data: Option<Vec<u8>>,
}

impl Format15Element {
    #[must_use]
    pub fn named(name: &str, data: Option<Vec<u8>>) -> Self {
        Self {
            header: make_element_header(name),
            data,
        }
    }

    #[must_use]
    pub fn preserved(header: Vec<u8>, data: Option<Vec<u8>>) -> Self {
        Self { header, data }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Format15Document {
    pub page_size: u32,
    pub storage_version: u32,
    pub reserved: u32,
    pub elements: Vec<Format15Element>,
}

impl Format15Document {
    #[must_use]
    pub fn new(storage_version: u32, elements: Vec<Format15Element>) -> Self {
        Self {
            page_size: format15::DEFAULT_PAGE_SIZE,
            storage_version,
            reserved: 0,
            elements,
        }
    }

    #[must_use]
    pub fn from_container(container: &format15::Container) -> Self {
        Self {
            page_size: container.file_header.page_size,
            storage_version: container.file_header.storage_version,
            reserved: container.file_header.reserved,
            elements: container
                .elements
                .iter()
                .map(|element| Format15Element {
                    header: element.raw_header.clone(),
                    data: element.data.clone(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct WriteReport {
    pub bytes_written: u64,
    pub entries_written: usize,
    pub pages_written: usize,
}

pub fn write_format15<W: Write>(
    target: &mut W,
    document: &Format15Document,
) -> Result<WriteReport, WriterError> {
    validate_document(document)?;
    let plan = plan_format15(document)?;

    write_all(target, &format15::SENTINEL.to_le_bytes(), "file marker")?;
    write_all(target, &document.page_size.to_le_bytes(), "file page size")?;
    write_all(
        target,
        &document.storage_version.to_le_bytes(),
        "storage version",
    )?;
    write_all(
        target,
        &document.reserved.to_le_bytes(),
        "file reserved word",
    )?;

    let mut toc = Vec::with_capacity(document.elements.len() * format15::ELEMENT_ADDRESS_SIZE);
    for element in &plan.elements {
        write_u32(&mut toc, checked_address(element.header.pages[0].address)?);
        write_u32(
            &mut toc,
            match &element.data {
                Some(chain) => checked_address(chain.pages[0].address)?,
                None => format15::SENTINEL,
            },
        );
        write_u32(&mut toc, format15::SENTINEL);
    }
    write_chain(target, &plan.toc, &toc)?;

    for (element, element_plan) in document.elements.iter().zip(&plan.elements) {
        write_chain(target, &element_plan.header, &element.header)?;
        if let (Some(chain), Some(data)) = (&element_plan.data, &element.data) {
            write_chain(target, chain, data)?;
        }
    }

    Ok(WriteReport {
        bytes_written: plan.bytes_written,
        entries_written: document.elements.len(),
        pages_written: plan.pages_written,
    })
}

pub fn write_format15_to_vec(document: &Format15Document) -> Result<Vec<u8>, WriterError> {
    let plan = plan_format15(document)?;
    let capacity =
        usize::try_from(plan.bytes_written).map_err(|_| WriterError::AddressSpaceExceeded {
            attempted_end: plan.bytes_written,
            maximum: u64::from(format15::SENTINEL),
        })?;
    let mut bytes = Vec::with_capacity(capacity);
    let report = write_format15(&mut bytes, document)?;
    debug_assert_eq!(report.bytes_written as usize, bytes.len());
    Ok(bytes)
}

#[must_use]
pub fn make_element_header(name: &str) -> Vec<u8> {
    let mut header = vec![0; format15::ELEMENT_HEADER_PREFIX_SIZE];
    header.extend(name.encode_utf16().flat_map(u16::to_le_bytes));
    header.extend_from_slice(&[0; 4]);
    header
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum WriterError {
    ZeroPageSize,
    PageSizeUsesSentinel {
        page_size: u32,
    },
    ElementCountOverflow {
        count: usize,
    },
    LogicalSizeOverflow {
        kind: &'static str,
        size: usize,
    },
    AddressSpaceExceeded {
        attempted_end: u64,
        maximum: u64,
    },
    InvalidElementHeader {
        index: usize,
        reason: &'static str,
    },
    Io {
        operation: &'static str,
        kind: std::io::ErrorKind,
    },
    InternalPlanMismatch,
}

impl fmt::Display for WriterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroPageSize => write!(formatter, "Format15 page size must be non-zero"),
            Self::PageSizeUsesSentinel { page_size } => write!(
                formatter,
                "Format15 page size {page_size} reaches the reserved sentinel"
            ),
            Self::ElementCountOverflow { count } => write!(
                formatter,
                "Format15 element count {count} overflows the TOC size"
            ),
            Self::LogicalSizeOverflow { kind, size } => write!(
                formatter,
                "Format15 {kind} logical size {size} does not fit in 32 bits"
            ),
            Self::AddressSpaceExceeded {
                attempted_end,
                maximum,
            } => write!(
                formatter,
                "Format15 deterministic allocation ends at {attempted_end}, beyond maximum {maximum}"
            ),
            Self::InvalidElementHeader { index, reason } => {
                write!(
                    formatter,
                    "Format15 element {index} header is invalid: {reason}"
                )
            }
            Self::Io { operation, kind } => {
                write!(formatter, "Format15 writer {operation} failed: {kind}")
            }
            Self::InternalPlanMismatch => write!(formatter, "Format15 writer plan mismatch"),
        }
    }
}

impl Error for WriterError {}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ChainPolicy {
    Compact,
    Padded,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct PlannedPage {
    address: u64,
    page_size: u32,
    data_length: usize,
    next_page_address: u32,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct PlannedChain {
    data_size: u32,
    pages: Vec<PlannedPage>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct PlannedElement {
    header: PlannedChain,
    data: Option<PlannedChain>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct Format15Plan {
    toc: PlannedChain,
    elements: Vec<PlannedElement>,
    bytes_written: u64,
    pages_written: usize,
}

fn validate_document(document: &Format15Document) -> Result<(), WriterError> {
    if document.page_size == 0 {
        return Err(WriterError::ZeroPageSize);
    }
    if document.page_size >= format15::SENTINEL {
        return Err(WriterError::PageSizeUsesSentinel {
            page_size: document.page_size,
        });
    }
    for (index, element) in document.elements.iter().enumerate() {
        validate_element_header(&element.header, index)?;
    }
    Ok(())
}

fn validate_element_header(header: &[u8], index: usize) -> Result<(), WriterError> {
    if header.len() < format15::ELEMENT_HEADER_PREFIX_SIZE {
        return Err(WriterError::InvalidElementHeader {
            index,
            reason: "shorter than the 20-byte prefix",
        });
    }
    let name = &header[format15::ELEMENT_HEADER_PREFIX_SIZE..];
    if !name.len().is_multiple_of(2) {
        return Err(WriterError::InvalidElementHeader {
            index,
            reason: "UTF-16LE name region has odd length",
        });
    }
    let units = name
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect::<Vec<_>>();
    String::from_utf16(&units).map_err(|_| WriterError::InvalidElementHeader {
        index,
        reason: "name is invalid UTF-16LE",
    })?;
    Ok(())
}

fn plan_format15(document: &Format15Document) -> Result<Format15Plan, WriterError> {
    validate_document(document)?;
    let toc_size = document
        .elements
        .len()
        .checked_mul(format15::ELEMENT_ADDRESS_SIZE)
        .ok_or(WriterError::ElementCountOverflow {
            count: document.elements.len(),
        })?;
    let mut cursor = format15::FILE_HEADER_SIZE as u64;
    let toc = plan_chain(
        &mut cursor,
        toc_size,
        document.page_size,
        ChainPolicy::Compact,
        "TOC",
    )?;
    let mut elements = Vec::with_capacity(document.elements.len());
    for element in &document.elements {
        let header = plan_chain(
            &mut cursor,
            element.header.len(),
            document.page_size,
            ChainPolicy::Compact,
            "element header",
        )?;
        let data = element
            .data
            .as_ref()
            .map(|data| {
                plan_chain(
                    &mut cursor,
                    data.len(),
                    document.page_size,
                    ChainPolicy::Padded,
                    "element data",
                )
            })
            .transpose()?;
        elements.push(PlannedElement { header, data });
    }
    let pages_written = toc.pages.len()
        + elements
            .iter()
            .map(|element| {
                element.header.pages.len()
                    + element.data.as_ref().map_or(0, |chain| chain.pages.len())
            })
            .sum::<usize>();
    Ok(Format15Plan {
        toc,
        elements,
        bytes_written: cursor,
        pages_written,
    })
}

fn plan_chain(
    cursor: &mut u64,
    logical_size: usize,
    configured_page_size: u32,
    policy: ChainPolicy,
    kind: &'static str,
) -> Result<PlannedChain, WriterError> {
    let data_size = u32::try_from(logical_size).map_err(|_| WriterError::LogicalSizeOverflow {
        kind,
        size: logical_size,
    })?;
    let page_capacity = configured_page_size as usize;
    let page_count = if logical_size == 0 {
        1
    } else {
        logical_size.div_ceil(page_capacity)
    };
    let mut pages = Vec::with_capacity(page_count);
    let mut remaining = logical_size;
    for _ in 0..page_count {
        let data_length = remaining.min(page_capacity);
        let page_size = match policy {
            ChainPolicy::Compact => u32::try_from(data_length).expect("bounded by u32 page size"),
            ChainPolicy::Padded => configured_page_size,
        };
        let address = *cursor;
        if address >= u64::from(format15::SENTINEL) {
            return Err(WriterError::AddressSpaceExceeded {
                attempted_end: address,
                maximum: u64::from(format15::SENTINEL),
            });
        }
        let extent = format15::BLOCK_HEADER_SIZE as u64 + u64::from(page_size);
        let end = address
            .checked_add(extent)
            .ok_or(WriterError::AddressSpaceExceeded {
                attempted_end: u64::MAX,
                maximum: u64::from(format15::SENTINEL),
            })?;
        if end > u64::from(format15::SENTINEL) {
            return Err(WriterError::AddressSpaceExceeded {
                attempted_end: end,
                maximum: u64::from(format15::SENTINEL),
            });
        }
        pages.push(PlannedPage {
            address,
            page_size,
            data_length,
            next_page_address: format15::SENTINEL,
        });
        *cursor = end;
        remaining -= data_length;
    }
    for index in 0..pages.len().saturating_sub(1) {
        pages[index].next_page_address = checked_address(pages[index + 1].address)?;
    }
    Ok(PlannedChain { data_size, pages })
}

fn write_chain<W: Write>(
    target: &mut W,
    chain: &PlannedChain,
    data: &[u8],
) -> Result<(), WriterError> {
    if data.len() != chain.data_size as usize {
        return Err(WriterError::InternalPlanMismatch);
    }
    let mut offset = 0_usize;
    for (index, page) in chain.pages.iter().enumerate() {
        let first_size = if index == 0 { chain.data_size } else { 0 };
        let header = format!(
            "\r\n{first_size:08x} {page_size:08x} {next:08x} \r\n",
            page_size = page.page_size,
            next = page.next_page_address,
        );
        if header.len() != format15::BLOCK_HEADER_SIZE {
            return Err(WriterError::InternalPlanMismatch);
        }
        write_all(target, header.as_bytes(), "block header")?;
        let end = offset
            .checked_add(page.data_length)
            .ok_or(WriterError::InternalPlanMismatch)?;
        write_all(target, &data[offset..end], "block payload")?;
        write_zeros(target, page.page_size as usize - page.data_length)?;
        offset = end;
    }
    if offset != data.len() {
        return Err(WriterError::InternalPlanMismatch);
    }
    Ok(())
}

fn write_zeros<W: Write>(target: &mut W, mut length: usize) -> Result<(), WriterError> {
    while length > 0 {
        let chunk = length.min(ZERO_BUFFER.len());
        write_all(target, &ZERO_BUFFER[..chunk], "page padding")?;
        length -= chunk;
    }
    Ok(())
}

fn write_all<W: Write>(
    target: &mut W,
    bytes: &[u8],
    operation: &'static str,
) -> Result<(), WriterError> {
    target.write_all(bytes).map_err(|error| WriterError::Io {
        operation,
        kind: error.kind(),
    })
}

fn checked_address(address: u64) -> Result<u32, WriterError> {
    if address >= u64::from(format15::SENTINEL) {
        return Err(WriterError::AddressSpaceExceeded {
            attempted_end: address,
            maximum: u64::from(format15::SENTINEL),
        });
    }
    Ok(address as u32)
}

fn write_u32(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::{
        Format15Document, Format15Element, WriterError, make_element_header, write_format15_to_vec,
    };
    use crate::format15;

    #[test]
    fn boundary_sizes_roundtrip_through_deterministic_page_chains() {
        let sizes = [0_usize, 1, 511, 512, 513, 4097];
        let elements = sizes
            .iter()
            .enumerate()
            .map(|(index, size)| {
                Format15Element::named(
                    &format!("entry-{index}"),
                    Some((0..*size).map(|value| value as u8).collect()),
                )
            })
            .collect();
        let document = Format15Document::new(5, elements);

        let bytes = write_format15_to_vec(&document).unwrap();
        let parsed = format15::parse(&bytes).unwrap();

        assert_eq!(parsed.elements.len(), sizes.len());
        for (index, size) in sizes.into_iter().enumerate() {
            assert_eq!(parsed.elements[index].data.as_ref().unwrap().len(), size);
            assert_eq!(
                parsed.elements[index].data_pages.as_ref().unwrap().len(),
                size.max(1).div_ceil(512)
            );
        }
        assert_eq!(parsed.elements[4].data_pages.as_ref().unwrap().len(), 2);
        assert_eq!(parsed.elements[5].data_pages.as_ref().unwrap().len(), 9);
    }

    #[test]
    fn absent_and_explicit_empty_data_remain_distinct() {
        let document = Format15Document::new(
            1,
            vec![
                Format15Element::named("absent", None),
                Format15Element::named("empty", Some(Vec::new())),
            ],
        );

        let parsed = format15::parse(&write_format15_to_vec(&document).unwrap()).unwrap();

        assert_eq!(parsed.elements[0].address.data_address, None);
        assert_eq!(parsed.elements[0].data, None);
        assert!(parsed.elements[1].address.data_address.is_some());
        assert_eq!(parsed.elements[1].data, Some(Vec::new()));
        assert_eq!(parsed.elements[1].data_pages.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn canonical_rewrite_is_byte_identical_and_preserves_raw_element_headers() {
        let mut header = make_element_header("preserved");
        header[0..4].copy_from_slice(&[1, 2, 3, 4]);
        let document = Format15Document::new(
            2,
            vec![Format15Element::preserved(
                header.clone(),
                Some(b"payload".to_vec()),
            )],
        );

        let first = write_format15_to_vec(&document).unwrap();
        let second = write_format15_to_vec(&document).unwrap();
        assert_eq!(first, second);
        let parsed = format15::parse(&first).unwrap();
        assert_eq!(parsed.elements[0].raw_header, header);

        let rewritten = write_format15_to_vec(&Format15Document::from_container(&parsed)).unwrap();
        assert_eq!(rewritten, first);
    }

    #[test]
    fn empty_container_roundtrips_with_one_zero_length_toc_page() {
        let bytes = write_format15_to_vec(&Format15Document::new(0, Vec::new())).unwrap();
        let parsed = format15::parse(&bytes).unwrap();

        assert!(parsed.elements.is_empty());
        assert_eq!(parsed.toc_block.data_size, 0);
        assert_eq!(parsed.toc_block.page_size, 0);
        assert_eq!(parsed.toc_pages.len(), 1);
    }

    #[test]
    fn invalid_options_and_headers_fail_before_output() {
        let mut zero = Format15Document::new(1, Vec::new());
        zero.page_size = 0;
        assert_eq!(
            write_format15_to_vec(&zero).unwrap_err(),
            WriterError::ZeroPageSize
        );

        let invalid = Format15Document::new(1, vec![Format15Element::preserved(vec![0; 19], None)]);
        assert_eq!(
            write_format15_to_vec(&invalid).unwrap_err(),
            WriterError::InvalidElementHeader {
                index: 0,
                reason: "shorter than the 20-byte prefix",
            }
        );
    }
}
