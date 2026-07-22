//! Deterministic streaming writers for V8 containers.

use std::{
    error::Error,
    fmt,
    io::{Cursor, Read, Write},
};

use crate::{format15, format16};

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
    /// Extra zero bytes reserved in the final physical page. This is used by
    /// higher-level formats whose next container starts at a fixed offset.
    pub tail_padding: u32,
    pub elements: Vec<Format15Element>,
}

impl Format15Document {
    #[must_use]
    pub fn new(storage_version: u32, elements: Vec<Format15Element>) -> Self {
        Self {
            page_size: format15::DEFAULT_PAGE_SIZE,
            storage_version,
            reserved: 0,
            tail_padding: 0,
            elements,
        }
    }

    #[must_use]
    pub fn from_container(container: &format15::Container) -> Self {
        Self {
            page_size: container.file_header.page_size,
            storage_version: container.file_header.storage_version,
            reserved: container.file_header.reserved,
            tail_padding: 0,
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

    #[must_use]
    pub fn with_tail_padding(mut self, tail_padding: u32) -> Self {
        self.tail_padding = tail_padding;
        self
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

/// Returns the exact number of bytes the deterministic Format15 writer will
/// emit, without allocating or reading any payload bytes.
pub fn format15_serialized_len(document: &Format15Document) -> Result<u64, WriterError> {
    Ok(plan_format15(document)?.bytes_written)
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
    let mut plan = Format15Plan {
        toc,
        elements,
        bytes_written: cursor,
        pages_written,
    };
    apply_format15_tail_padding(&mut plan, document.tail_padding)?;
    Ok(plan)
}

fn apply_format15_tail_padding(
    plan: &mut Format15Plan,
    tail_padding: u32,
) -> Result<(), WriterError> {
    if tail_padding == 0 {
        return Ok(());
    }
    let final_page = plan
        .elements
        .last_mut()
        .map(|element| {
            element.data.as_mut().map_or_else(
                || element.header.pages.last_mut().unwrap(),
                |data| data.pages.last_mut().unwrap(),
            )
        })
        .unwrap_or_else(|| plan.toc.pages.last_mut().unwrap());
    final_page.page_size = final_page.page_size.checked_add(tail_padding).ok_or(
        WriterError::PageSizeUsesSentinel {
            page_size: format15::SENTINEL,
        },
    )?;
    if final_page.page_size >= format15::SENTINEL {
        return Err(WriterError::PageSizeUsesSentinel {
            page_size: final_page.page_size,
        });
    }
    plan.bytes_written = plan
        .bytes_written
        .checked_add(u64::from(tail_padding))
        .ok_or(WriterError::AddressSpaceExceeded {
            attempted_end: u64::MAX,
            maximum: u64::from(format15::SENTINEL),
        })?;
    if plan.bytes_written > u64::from(format15::SENTINEL) {
        return Err(WriterError::AddressSpaceExceeded {
            attempted_end: plan.bytes_written,
            maximum: u64::from(format15::SENTINEL),
        });
    }
    Ok(())
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

/// A declared-length payload consumed exactly once by the Format16 writer.
///
/// The source may be a file, a decoder, or any other `Read` implementation;
/// payload bytes are copied through a fixed-size buffer and are never joined
/// into one aggregate allocation by the writer.
pub struct Format16Payload<'a> {
    length: u64,
    reader: Box<dyn Read + 'a>,
}

impl fmt::Debug for Format16Payload<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Format16Payload")
            .field("length", &self.length)
            .finish_non_exhaustive()
    }
}

impl<'a> Format16Payload<'a> {
    #[must_use]
    pub fn from_bytes(bytes: &'a [u8]) -> Self {
        Self {
            length: u64::try_from(bytes.len()).expect("slice length fits in u64"),
            reader: Box::new(Cursor::new(bytes)),
        }
    }

    #[must_use]
    pub fn from_reader<R>(length: u64, reader: R) -> Self
    where
        R: Read + 'a,
    {
        Self {
            length,
            reader: Box::new(reader),
        }
    }

    #[must_use]
    pub const fn len(&self) -> u64 {
        self.length
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }
}

#[derive(Debug)]
pub struct Format16Element<'a> {
    /// Raw logical element-header bytes, including the 20-byte prefix and
    /// UTF-16LE name region.
    pub header: Vec<u8>,
    /// `None` writes the absent-data sentinel; a zero-length payload writes an
    /// explicit empty data block.
    pub data: Option<Format16Payload<'a>>,
}

impl<'a> Format16Element<'a> {
    #[must_use]
    pub fn named(name: &str, data: Option<Format16Payload<'a>>) -> Self {
        Self {
            header: make_element_header(name),
            data,
        }
    }

    #[must_use]
    pub fn preserved(header: Vec<u8>, data: Option<Format16Payload<'a>>) -> Self {
        Self { header, data }
    }
}

#[derive(Debug)]
pub struct Format16Document<'a> {
    pub page_size: u32,
    pub storage_version: u32,
    pub reserved: u32,
    pub elements: Vec<Format16Element<'a>>,
}

impl<'a> Format16Document<'a> {
    #[must_use]
    pub fn new(storage_version: u32, elements: Vec<Format16Element<'a>>) -> Self {
        Self {
            page_size: format16::DEFAULT_PAGE_SIZE,
            storage_version,
            reserved: 0,
            elements,
        }
    }

    #[must_use]
    pub fn from_container(container: &'a format16::Container) -> Self {
        Self {
            page_size: container.file_header.page_size,
            storage_version: container.file_header.storage_version,
            reserved: container.file_header.reserved,
            elements: container
                .elements
                .iter()
                .map(|element| Format16Element {
                    header: element.raw_header.clone(),
                    data: element.data.as_deref().map(Format16Payload::from_bytes),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Format16WriteReport {
    pub bytes_written: u64,
    pub entries_written: usize,
    pub pages_written: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Format16WriterError {
    ZeroPageSize,
    ElementCountOverflow {
        count: usize,
    },
    AddressSpaceExceeded {
        attempted_end: u64,
    },
    AddressSpaceOverflow,
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

impl fmt::Display for Format16WriterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroPageSize => write!(formatter, "Format16 page size must be non-zero"),
            Self::ElementCountOverflow { count } => write!(
                formatter,
                "Format16 element count {count} overflows the TOC size"
            ),
            Self::AddressSpaceExceeded { attempted_end } => write!(
                formatter,
                "Format16 deterministic allocation reaches reserved address {attempted_end}"
            ),
            Self::AddressSpaceOverflow => {
                write!(
                    formatter,
                    "Format16 deterministic allocation overflows 64 bits"
                )
            }
            Self::InvalidElementHeader { index, reason } => write!(
                formatter,
                "Format16 element {index} header is invalid: {reason}"
            ),
            Self::Io { operation, kind } => {
                write!(formatter, "Format16 writer {operation} failed: {kind}")
            }
            Self::InternalPlanMismatch => write!(formatter, "Format16 writer plan mismatch"),
        }
    }
}

impl Error for Format16WriterError {}

/// Writes a primary Format16 container. Addresses in its TOC and page chains
/// are relative to the first byte written by this function.
pub fn write_format16_primary<W: Write>(
    target: &mut W,
    document: &mut Format16Document<'_>,
) -> Result<Format16WriteReport, Format16WriterError> {
    let plan = plan_format16(document)?;

    write_all16(target, &format16::SENTINEL.to_le_bytes(), "file marker")?;
    write_all16(target, &document.page_size.to_le_bytes(), "file page size")?;
    write_all16(
        target,
        &document.storage_version.to_le_bytes(),
        "storage version",
    )?;
    write_all16(
        target,
        &document.reserved.to_le_bytes(),
        "file reserved word",
    )?;

    let mut toc = Format16TocReader::new(&plan.elements);
    write_wide_chain(target, &plan.toc, &mut toc, "TOC")?;

    for (element, element_plan) in document.elements.iter_mut().zip(&plan.elements) {
        let mut header = Cursor::new(element.header.as_slice());
        write_wide_chain(target, &element_plan.header, &mut header, "element header")?;
        if let (Some(chain), Some(data)) = (&element_plan.data, &mut element.data) {
            write_wide_chain(target, chain, &mut data.reader, "element payload")?;
        } else if element_plan.data.is_some() != element.data.is_some() {
            return Err(Format16WriterError::InternalPlanMismatch);
        }
    }

    Ok(Format16WriteReport {
        bytes_written: plan.bytes_written,
        entries_written: document.elements.len(),
        pages_written: plan.pages_written,
    })
}

/// Returns the exact primary-container size without reading payload sources.
pub fn format16_serialized_len(
    document: &Format16Document<'_>,
) -> Result<u64, Format16WriterError> {
    Ok(plan_format16(document)?.bytes_written)
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum WideChainPolicy {
    Compact,
    Padded,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct WidePlannedChain {
    start_address: u64,
    data_size: u64,
    page_capacity: u64,
    page_count: u64,
    policy: WideChainPolicy,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct WidePlannedElement {
    header: WidePlannedChain,
    data: Option<WidePlannedChain>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct Format16Plan {
    toc: WidePlannedChain,
    elements: Vec<WidePlannedElement>,
    bytes_written: u64,
    pages_written: u64,
}

fn plan_format16(document: &Format16Document<'_>) -> Result<Format16Plan, Format16WriterError> {
    if document.page_size == 0 {
        return Err(Format16WriterError::ZeroPageSize);
    }
    for (index, element) in document.elements.iter().enumerate() {
        validate_format16_element_header(&element.header, index)?;
    }

    let toc_size = document
        .elements
        .len()
        .checked_mul(format16::ELEMENT_ADDRESS_SIZE)
        .and_then(|size| u64::try_from(size).ok())
        .ok_or(Format16WriterError::ElementCountOverflow {
            count: document.elements.len(),
        })?;
    let mut cursor = format16::FILE_HEADER_SIZE as u64;
    let toc = plan_wide_chain(
        &mut cursor,
        toc_size,
        document.page_size,
        WideChainPolicy::Padded,
    )?;
    let mut elements = Vec::with_capacity(document.elements.len());
    let mut pages_written = toc.page_count;
    for element in &document.elements {
        let header_size = u64::try_from(element.header.len())
            .map_err(|_| Format16WriterError::AddressSpaceOverflow)?;
        let header = plan_wide_chain(
            &mut cursor,
            header_size,
            document.page_size,
            WideChainPolicy::Compact,
        )?;
        let data = element
            .data
            .as_ref()
            .map(|payload| {
                plan_wide_chain(
                    &mut cursor,
                    payload.len(),
                    document.page_size,
                    WideChainPolicy::Padded,
                )
            })
            .transpose()?;
        pages_written = pages_written
            .checked_add(header.page_count)
            .and_then(|count| data.map_or(Some(count), |chain| count.checked_add(chain.page_count)))
            .ok_or(Format16WriterError::AddressSpaceOverflow)?;
        elements.push(WidePlannedElement { header, data });
    }
    Ok(Format16Plan {
        toc,
        elements,
        bytes_written: cursor,
        pages_written,
    })
}

fn plan_wide_chain(
    cursor: &mut u64,
    data_size: u64,
    configured_page_size: u32,
    policy: WideChainPolicy,
) -> Result<WidePlannedChain, Format16WriterError> {
    let page_capacity = u64::from(configured_page_size);
    let page_count = if data_size == 0 {
        1
    } else {
        (data_size - 1) / page_capacity + 1
    };
    if *cursor == format16::SENTINEL {
        return Err(Format16WriterError::AddressSpaceExceeded {
            attempted_end: *cursor,
        });
    }
    let header_bytes = page_count
        .checked_mul(format16::BLOCK_HEADER_SIZE as u64)
        .ok_or(Format16WriterError::AddressSpaceOverflow)?;
    let payload_bytes = match policy {
        WideChainPolicy::Compact => data_size,
        WideChainPolicy::Padded => page_count
            .checked_mul(page_capacity)
            .ok_or(Format16WriterError::AddressSpaceOverflow)?,
    };
    let extent = header_bytes
        .checked_add(payload_bytes)
        .ok_or(Format16WriterError::AddressSpaceOverflow)?;
    let start_address = *cursor;
    *cursor = cursor
        .checked_add(extent)
        .ok_or(Format16WriterError::AddressSpaceOverflow)?;
    Ok(WidePlannedChain {
        start_address,
        data_size,
        page_capacity,
        page_count,
        policy,
    })
}

fn validate_format16_element_header(
    header: &[u8],
    index: usize,
) -> Result<(), Format16WriterError> {
    if header.len() < format16::ELEMENT_HEADER_PREFIX_SIZE {
        return Err(Format16WriterError::InvalidElementHeader {
            index,
            reason: "shorter than the 20-byte prefix",
        });
    }
    let name = &header[format16::ELEMENT_HEADER_PREFIX_SIZE..];
    if !name.len().is_multiple_of(2) {
        return Err(Format16WriterError::InvalidElementHeader {
            index,
            reason: "UTF-16LE name region has odd length",
        });
    }
    let units = name
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect::<Vec<_>>();
    String::from_utf16(&units).map_err(|_| Format16WriterError::InvalidElementHeader {
        index,
        reason: "name is invalid UTF-16LE",
    })?;
    Ok(())
}

struct Format16TocReader<'a> {
    elements: &'a [WidePlannedElement],
    position: u64,
}

impl<'a> Format16TocReader<'a> {
    const fn new(elements: &'a [WidePlannedElement]) -> Self {
        Self {
            elements,
            position: 0,
        }
    }
}

impl Read for Format16TocReader<'_> {
    fn read(&mut self, target: &mut [u8]) -> std::io::Result<usize> {
        let total = self
            .elements
            .len()
            .saturating_mul(format16::ELEMENT_ADDRESS_SIZE);
        let position = usize::try_from(self.position).unwrap_or(usize::MAX);
        if position >= total || target.is_empty() {
            return Ok(0);
        }
        let mut written = 0;
        while written < target.len() {
            let Some(absolute) = position.checked_add(written) else {
                break;
            };
            if absolute >= total {
                break;
            }
            let element_index = absolute / format16::ELEMENT_ADDRESS_SIZE;
            let record_offset = absolute % format16::ELEMENT_ADDRESS_SIZE;
            let record = format16_toc_record(&self.elements[element_index]);
            let count = (format16::ELEMENT_ADDRESS_SIZE - record_offset)
                .min(target.len() - written)
                .min(total - absolute);
            target[written..written + count]
                .copy_from_slice(&record[record_offset..record_offset + count]);
            written += count;
        }
        self.position += written as u64;
        Ok(written)
    }
}

fn format16_toc_record(element: &WidePlannedElement) -> [u8; format16::ELEMENT_ADDRESS_SIZE] {
    let mut record = [0; format16::ELEMENT_ADDRESS_SIZE];
    record[0..8].copy_from_slice(&element.header.start_address.to_le_bytes());
    record[8..16].copy_from_slice(
        &element
            .data
            .map_or(format16::SENTINEL, |chain| chain.start_address)
            .to_le_bytes(),
    );
    record[16..24].copy_from_slice(&format16::SENTINEL.to_le_bytes());
    record
}

fn write_wide_chain<W: Write, R: Read + ?Sized>(
    target: &mut W,
    chain: &WidePlannedChain,
    source: &mut R,
    source_operation: &'static str,
) -> Result<(), Format16WriterError> {
    let mut remaining = chain.data_size;
    let mut address = chain.start_address;
    let mut buffer = [0_u8; 8192];
    for page_index in 0..chain.page_count {
        let data_length = remaining.min(chain.page_capacity);
        let page_size = match chain.policy {
            WideChainPolicy::Compact => data_length,
            WideChainPolicy::Padded => chain.page_capacity,
        };
        let next_address = if page_index + 1 == chain.page_count {
            format16::SENTINEL
        } else {
            address
                .checked_add(format16::BLOCK_HEADER_SIZE as u64)
                .and_then(|value| value.checked_add(page_size))
                .ok_or(Format16WriterError::AddressSpaceOverflow)?
        };
        let first_size = if page_index == 0 { chain.data_size } else { 0 };
        let header = format!("\r\n{first_size:016x} {page_size:016x} {next_address:016x} \r\n");
        if header.len() != format16::BLOCK_HEADER_SIZE {
            return Err(Format16WriterError::InternalPlanMismatch);
        }
        write_all16(target, header.as_bytes(), "block header")?;
        let mut page_remaining = data_length;
        while page_remaining > 0 {
            let count = usize::try_from(page_remaining.min(buffer.len() as u64)).unwrap();
            source
                .read_exact(&mut buffer[..count])
                .map_err(|error| Format16WriterError::Io {
                    operation: source_operation,
                    kind: error.kind(),
                })?;
            write_all16(target, &buffer[..count], "block payload")?;
            page_remaining -= count as u64;
        }
        write_zeros16(target, page_size - data_length)?;
        remaining -= data_length;
        address = next_address;
    }
    if remaining != 0 {
        return Err(Format16WriterError::InternalPlanMismatch);
    }
    Ok(())
}

fn write_zeros16<W: Write>(target: &mut W, mut length: u64) -> Result<(), Format16WriterError> {
    while length > 0 {
        let count = usize::try_from(length.min(ZERO_BUFFER.len() as u64)).unwrap();
        write_all16(target, &ZERO_BUFFER[..count], "page padding")?;
        length -= count as u64;
    }
    Ok(())
}

fn write_all16<W: Write>(
    target: &mut W,
    bytes: &[u8],
    operation: &'static str,
) -> Result<(), Format16WriterError> {
    target
        .write_all(bytes)
        .map_err(|error| Format16WriterError::Io {
            operation,
            kind: error.kind(),
        })
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use super::{
        Format15Document, Format15Element, Format16Document, Format16Element, Format16Payload,
        WriterError, format15_serialized_len, format16_serialized_len, make_element_header,
        write_format15_to_vec, write_format16_primary,
    };
    use crate::{format15, format16};

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

    #[test]
    fn format15_tail_padding_extends_only_the_final_physical_page() {
        let base = Format15Document::new(
            5,
            vec![Format15Element::named("last", Some(b"data".to_vec()))],
        );
        let base_size = format15_serialized_len(&base).unwrap();
        let padded = base.with_tail_padding(137);
        let bytes = write_format15_to_vec(&padded).unwrap();
        let parsed = format15::parse(&bytes).unwrap();

        assert_eq!(bytes.len() as u64, base_size + 137);
        assert_eq!(
            parsed.element("last").unwrap().data.as_deref(),
            Some(b"data".as_slice())
        );
        assert_eq!(
            parsed
                .element("last")
                .unwrap()
                .data_block
                .as_ref()
                .unwrap()
                .page_size,
            format15::DEFAULT_PAGE_SIZE + 137
        );
    }

    fn format16_document<'a>(payloads: &'a [Vec<u8>]) -> Format16Document<'a> {
        let mut preserved = make_element_header("preserved");
        preserved[0..4].copy_from_slice(&[1, 2, 3, 4]);
        Format16Document::new(
            5,
            vec![
                Format16Element::named("absent", None),
                Format16Element::named("empty", Some(Format16Payload::from_bytes(&payloads[0]))),
                Format16Element::named("one", Some(Format16Payload::from_bytes(&payloads[1]))),
                Format16Element::named("511", Some(Format16Payload::from_bytes(&payloads[2]))),
                Format16Element::named("512", Some(Format16Payload::from_bytes(&payloads[3]))),
                Format16Element::named("513", Some(Format16Payload::from_bytes(&payloads[4]))),
                Format16Element::preserved(
                    preserved,
                    Some(Format16Payload::from_bytes(&payloads[5])),
                ),
            ],
        )
    }

    #[test]
    fn format16_boundary_sizes_roundtrip_and_rewrite_deterministically() {
        let payloads = [0_usize, 1, 511, 512, 513, 4097]
            .into_iter()
            .map(|size| (0..size).map(|value| value as u8).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let mut first_document = format16_document(&payloads);
        let mut first = Vec::new();
        write_format16_primary(&mut first, &mut first_document).unwrap();
        let mut second_document = format16_document(&payloads);
        let mut second = Vec::new();
        write_format16_primary(&mut second, &mut second_document).unwrap();

        assert_eq!(first, second);
        let parsed = format16::parse_primary(&first).unwrap();
        assert_eq!(parsed.elements[0].data, None);
        for (index, payload) in payloads.iter().enumerate() {
            assert_eq!(
                parsed.elements[index + 1].data.as_deref(),
                Some(payload.as_slice())
            );
        }
        assert_eq!(parsed.elements[6].raw_header[0..4], [1, 2, 3, 4]);
        assert_eq!(parsed.elements[5].data_pages.as_ref().unwrap().len(), 2);
        assert_eq!(parsed.elements[6].data_pages.as_ref().unwrap().len(), 9);
    }

    #[derive(Debug)]
    struct VirtualPayload {
        remaining: u64,
        max_request: usize,
    }

    impl Read for VirtualPayload {
        fn read(&mut self, target: &mut [u8]) -> std::io::Result<usize> {
            self.max_request = self.max_request.max(target.len());
            let count = usize::try_from(self.remaining.min(target.len() as u64)).unwrap();
            target[..count].fill(0x5a);
            self.remaining -= count as u64;
            Ok(count)
        }
    }

    #[derive(Debug, Default)]
    struct CountingWriter(u64);

    impl Write for CountingWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 += bytes.len() as u64;
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn format16_streams_large_payload_and_plans_beyond_32_bit_offsets() {
        const STREAMED_SIZE: u64 = 64 * 1024 * 1024;
        let mut source = VirtualPayload {
            remaining: STREAMED_SIZE,
            max_request: 0,
        };
        let mut output = CountingWriter::default();
        {
            let mut document = Format16Document::new(
                5,
                vec![Format16Element::named(
                    "virtual",
                    Some(Format16Payload::from_reader(STREAMED_SIZE, &mut source)),
                )],
            );
            document.page_size = 1024 * 1024;
            let report = write_format16_primary(&mut output, &mut document).unwrap();
            assert_eq!(report.bytes_written, output.0);
        }
        assert_eq!(source.remaining, 0);
        assert!(source.max_request <= 8192);

        let mut huge = Format16Document::new(
            5,
            vec![Format16Element::named(
                "wide",
                Some(Format16Payload::from_reader(
                    u64::from(u32::MAX) + 4096,
                    std::io::empty(),
                )),
            )],
        );
        huge.page_size = 1024 * 1024;
        assert!(format16_serialized_len(&huge).unwrap() > u64::from(u32::MAX));
    }
}
