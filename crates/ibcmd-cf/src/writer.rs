//! Deterministic lossless CF repack and validated atomic publication.
//!
//! Repack preserves ordered logical headers and exact packed bytes. The
//! declared compression is checked against the retained unpacked bytes before
//! any output is emitted. Atomic publication writes in the destination
//! directory, synchronizes, reopens and validates the temporary artifact, and
//! only then renames it into place.

use std::{
    error::Error,
    ffi::OsString,
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use ibcmd_core::{
    limits::ResourceLimits,
    storage::{MultipartIdentity, StorageEntry, StorageImage},
};
use ibcmd_v8::{
    format::Revision,
    reader::{ReaderError, StreamingReader},
    writer::{
        Format15Document, Format15Element, Format16Document, Format16Element, Format16Payload,
        Format16WriterError, WriterError, write_format15,
    },
};

use crate::{
    archive::{
        CfArchive, CfArchiveMetadata, CfDataState, CfEntryAttributes, CfEntryAttributesError,
    },
    payload::{PayloadDecodeError, PayloadDecoder, PayloadEncoding},
    preamble::{Format16ArchiveWriterError, PreambleError, PreambleMode, write_format16_archive},
};

const TEMPORARY_ATTEMPTS: u32 = 1_024;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CfWriteReport {
    pub revision: Revision,
    pub bytes_written: u64,
    pub entries_written: usize,
}

#[derive(Debug)]
pub enum CfWriteError {
    Attributes {
        index: usize,
        name: String,
        source: CfEntryAttributesError,
    },
    PhysicalIdentity {
        index: usize,
        name: String,
        reason: &'static str,
    },
    HeaderNameMismatch {
        index: usize,
        expected: String,
        actual: String,
    },
    UnsupportedCompression {
        index: usize,
        name: String,
        compression: String,
    },
    AbsentPayload {
        index: usize,
        name: String,
    },
    PayloadDecode {
        index: usize,
        name: String,
        source: PayloadDecodeError,
    },
    PayloadMismatch {
        index: usize,
        name: String,
    },
    Format15(WriterError),
    Format16(Format16ArchiveWriterError),
}

impl fmt::Display for CfWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Attributes {
                index,
                name,
                source,
            } => write!(
                formatter,
                "CF entry {index} (`{name}`) has invalid retained attributes: {source}"
            ),
            Self::PhysicalIdentity {
                index,
                name,
                reason,
            } => write!(
                formatter,
                "CF entry {index} (`{name}`) cannot be emitted: {reason}"
            ),
            Self::HeaderNameMismatch {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "CF entry {index} logical name `{expected}` does not match raw header name `{actual}`"
            ),
            Self::UnsupportedCompression {
                index,
                name,
                compression,
            } => write!(
                formatter,
                "CF entry {index} (`{name}`) uses unsupported compression `{compression}`"
            ),
            Self::AbsentPayload { index, name } => write!(
                formatter,
                "absent CF entry {index} (`{name}`) retains non-empty payload bytes or compression"
            ),
            Self::PayloadDecode {
                index,
                name,
                source,
            } => write!(
                formatter,
                "CF entry {index} (`{name}`) packed payload is invalid: {source}"
            ),
            Self::PayloadMismatch { index, name } => write!(
                formatter,
                "CF entry {index} (`{name}`) packed payload does not decode to its retained unpacked bytes"
            ),
            Self::Format15(source) => source.fmt(formatter),
            Self::Format16(source) => source.fmt(formatter),
        }
    }
}

impl Error for CfWriteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Attributes { source, .. } => Some(source),
            Self::PayloadDecode { source, .. } => Some(source),
            Self::Format15(source) => Some(source),
            Self::Format16(source) => Some(source),
            Self::PhysicalIdentity { .. }
            | Self::HeaderNameMismatch { .. }
            | Self::UnsupportedCompression { .. }
            | Self::AbsentPayload { .. }
            | Self::PayloadMismatch { .. } => None,
        }
    }
}

impl From<WriterError> for CfWriteError {
    fn from(source: WriterError) -> Self {
        Self::Format15(source)
    }
}

impl From<Format16WriterError> for CfWriteError {
    fn from(source: Format16WriterError) -> Self {
        Self::Format16(Format16ArchiveWriterError::Primary(source))
    }
}

impl From<Format16ArchiveWriterError> for CfWriteError {
    fn from(source: Format16ArchiveWriterError) -> Self {
        Self::Format16(source)
    }
}

impl From<PreambleError> for CfWriteError {
    fn from(source: PreambleError) -> Self {
        Self::Format16(Format16ArchiveWriterError::Preamble(source))
    }
}

#[derive(Clone, Copy)]
struct ProjectedEntry<'a> {
    entry: &'a StorageEntry,
    state: CfDataState,
    encoding: PayloadEncoding,
}

/// Repack one decoded archive into a deterministic byte stream.
pub fn write_archive<W: Write>(
    target: &mut W,
    archive: &CfArchive,
    limits: ResourceLimits,
) -> Result<CfWriteReport, CfWriteError> {
    write_storage_image(target, archive.metadata(), archive.image(), limits)
}

/// Write a validated storage image using layout metadata retained from a base
/// archive. This entry point is also suitable for a future overlay image.
pub fn write_storage_image<W: Write>(
    target: &mut W,
    metadata: &CfArchiveMetadata,
    image: &StorageImage,
    limits: ResourceLimits,
) -> Result<CfWriteReport, CfWriteError> {
    let entries = project_entries(image, limits)?;
    match metadata.revision() {
        Revision::Format15 => write_format15_image(target, metadata, &entries),
        Revision::Format16 => write_format16_image(target, metadata, &entries),
    }
}

fn write_format15_image<W: Write>(
    target: &mut W,
    metadata: &CfArchiveMetadata,
    entries: &[ProjectedEntry<'_>],
) -> Result<CfWriteReport, CfWriteError> {
    let mut document = Format15Document::new(
        metadata.storage_version(),
        entries
            .iter()
            .map(|projected| {
                Format15Element::preserved(
                    projected.entry.raw_header().to_vec(),
                    (projected.state == CfDataState::Present)
                        .then(|| projected.entry.packed_payload().to_vec()),
                )
            })
            .collect(),
    );
    document.page_size = metadata.page_size();
    document.reserved = metadata.reserved();
    let report = write_format15(target, &document)?;
    Ok(CfWriteReport {
        revision: Revision::Format15,
        bytes_written: report.bytes_written,
        entries_written: report.entries_written,
    })
}

fn write_format16_image<W: Write>(
    target: &mut W,
    metadata: &CfArchiveMetadata,
    entries: &[ProjectedEntry<'_>],
) -> Result<CfWriteReport, CfWriteError> {
    let preamble = PreambleMode::preserve(metadata.raw_preamble().to_vec())?;
    let mut document = Format16Document::new(
        metadata.storage_version(),
        entries
            .iter()
            .map(|projected| {
                Format16Element::preserved(
                    projected.entry.raw_header().to_vec(),
                    (projected.state == CfDataState::Present)
                        .then(|| Format16Payload::from_bytes(projected.entry.packed_payload())),
                )
            })
            .collect(),
    );
    document.page_size = metadata.page_size();
    document.reserved = metadata.reserved();
    let report = write_format16_archive(target, &preamble, &mut document)?;
    Ok(CfWriteReport {
        revision: Revision::Format16,
        bytes_written: report.bytes_written(),
        entries_written: report.primary.entries_written,
    })
}

fn project_entries(
    image: &StorageImage,
    limits: ResourceLimits,
) -> Result<Vec<ProjectedEntry<'_>>, CfWriteError> {
    let mut decoder = PayloadDecoder::new(limits);
    let mut projected = Vec::with_capacity(image.len());
    for (index, entry) in image.entries().iter().enumerate() {
        let name = entry.logical_name().as_str();
        if entry.logical_key().as_str() != name {
            return Err(CfWriteError::PhysicalIdentity {
                index,
                name: name.to_owned(),
                reason: "logical key differs from the physical CF name",
            });
        }
        if entry.multipart() != MultipartIdentity::single() {
            return Err(CfWriteError::PhysicalIdentity {
                index,
                name: name.to_owned(),
                reason: "one CF TOC record must be a single physical part",
            });
        }
        let header_name = decode_header_name(entry.raw_header()).map_err(|reason| {
            CfWriteError::PhysicalIdentity {
                index,
                name: name.to_owned(),
                reason,
            }
        })?;
        if header_name != name {
            return Err(CfWriteError::HeaderNameMismatch {
                index,
                expected: name.to_owned(),
                actual: header_name,
            });
        }
        let attributes = CfEntryAttributes::decode(entry.attributes()).map_err(|source| {
            CfWriteError::Attributes {
                index,
                name: name.to_owned(),
                source,
            }
        })?;
        let encoding =
            entry_encoding(entry).ok_or_else(|| CfWriteError::UnsupportedCompression {
                index,
                name: name.to_owned(),
                compression: entry.compression().as_str().to_owned(),
            })?;
        if attributes.data_state() == CfDataState::Absent {
            if !entry.packed_payload().is_empty()
                || !entry.unpacked_payload().is_empty()
                || encoding != PayloadEncoding::Stored
            {
                return Err(CfWriteError::AbsentPayload {
                    index,
                    name: name.to_owned(),
                });
            }
        } else {
            let decoded = decoder
                .decode(encoding, entry.packed_payload())
                .map_err(|source| CfWriteError::PayloadDecode {
                    index,
                    name: name.to_owned(),
                    source,
                })?;
            if decoded.bytes() != entry.unpacked_payload() {
                return Err(CfWriteError::PayloadMismatch {
                    index,
                    name: name.to_owned(),
                });
            }
        }
        projected.push(ProjectedEntry {
            entry,
            state: attributes.data_state(),
            encoding,
        });
    }
    Ok(projected)
}

fn entry_encoding(entry: &StorageEntry) -> Option<PayloadEncoding> {
    match entry.compression().as_str() {
        "stored" => Some(PayloadEncoding::Stored),
        "raw-deflate" => Some(PayloadEncoding::RawDeflate),
        _ => None,
    }
}

fn decode_header_name(header: &[u8]) -> Result<String, &'static str> {
    const PREFIX: usize = 20;
    if header.len() < PREFIX {
        return Err("raw logical header is shorter than its 20-byte prefix");
    }
    let name = &header[PREFIX..];
    if !name.len().is_multiple_of(2) {
        return Err("raw logical header has an odd UTF-16LE name region");
    }
    let units = name
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect::<Vec<_>>();
    String::from_utf16(&units).map_err(|_| "raw logical header name is invalid UTF-16LE")
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct RepackValidationReport {
    pub revision: Revision,
    pub entries_validated: usize,
}

#[derive(Debug)]
pub enum RepackValidationError {
    Source(CfWriteError),
    Reader(ReaderError),
    PreambleRead {
        kind: std::io::ErrorKind,
    },
    LayoutMismatch {
        field: &'static str,
    },
    EntryCountMismatch {
        expected: usize,
        actual: usize,
    },
    EntryMismatch {
        index: usize,
        name: String,
        field: &'static str,
    },
    PayloadDecode {
        index: usize,
        name: String,
        source: PayloadDecodeError,
    },
}

impl fmt::Display for RepackValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(source) => write!(formatter, "invalid expected CF image: {source}"),
            Self::Reader(source) => write!(formatter, "repacked CF cannot be indexed: {source}"),
            Self::PreambleRead { kind } => {
                write!(formatter, "repacked CF preamble cannot be read: {kind}")
            }
            Self::LayoutMismatch { field } => {
                write!(formatter, "repacked CF layout changed `{field}`")
            }
            Self::EntryCountMismatch { expected, actual } => write!(
                formatter,
                "repacked CF has {actual} entries instead of {expected}"
            ),
            Self::EntryMismatch { index, name, field } => write!(
                formatter,
                "repacked CF entry {index} (`{name}`) changed `{field}`"
            ),
            Self::PayloadDecode {
                index,
                name,
                source,
            } => write!(
                formatter,
                "repacked CF entry {index} (`{name}`) cannot be decoded: {source}"
            ),
        }
    }
}

impl Error for RepackValidationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Source(source) => Some(source),
            Self::Reader(source) => Some(source),
            Self::PayloadDecode { source, .. } => Some(source),
            Self::PreambleRead { .. }
            | Self::LayoutMismatch { .. }
            | Self::EntryCountMismatch { .. }
            | Self::EntryMismatch { .. } => None,
        }
    }
}

/// Reopen-style semantic validation used before atomic publication.
pub fn validate_repacked_archive<R: Read + Seek>(
    source: R,
    metadata: &CfArchiveMetadata,
    image: &StorageImage,
    limits: ResourceLimits,
) -> Result<RepackValidationReport, RepackValidationError> {
    let expected = project_entries(image, limits).map_err(RepackValidationError::Source)?;
    let mut reader =
        StreamingReader::open(source, limits).map_err(RepackValidationError::Reader)?;
    validate_layout(&mut reader, metadata)?;
    if reader.index().entries.len() != expected.len() {
        return Err(RepackValidationError::EntryCountMismatch {
            expected: expected.len(),
            actual: reader.index().entries.len(),
        });
    }

    let mut decoder = PayloadDecoder::new(limits);
    for (index, projected) in expected.iter().enumerate() {
        let native = reader.index().entries[index].clone();
        let name = projected.entry.logical_name().as_str();
        if native.name != name {
            return Err(entry_mismatch(index, name, "logical_name"));
        }
        let header = reader
            .read_entry_header(index)
            .map_err(RepackValidationError::Reader)?;
        if header != projected.entry.raw_header() {
            return Err(entry_mismatch(index, name, "raw_header"));
        }
        let packed = reader
            .read_entry_data(index)
            .map_err(RepackValidationError::Reader)?;
        match (projected.state, packed) {
            (CfDataState::Absent, None) => {}
            (CfDataState::Present, Some(packed)) => {
                if packed != projected.entry.packed_payload() {
                    return Err(entry_mismatch(index, name, "packed_payload"));
                }
                let decoded = decoder
                    .decode(projected.encoding, &packed)
                    .map_err(|source| RepackValidationError::PayloadDecode {
                        index,
                        name: name.to_owned(),
                        source,
                    })?;
                if decoded.bytes() != projected.entry.unpacked_payload() {
                    return Err(entry_mismatch(index, name, "unpacked_payload"));
                }
            }
            _ => return Err(entry_mismatch(index, name, "data_state")),
        }
    }
    Ok(RepackValidationReport {
        revision: metadata.revision(),
        entries_validated: expected.len(),
    })
}

fn validate_layout<R: Read + Seek>(
    reader: &mut StreamingReader<R>,
    metadata: &CfArchiveMetadata,
) -> Result<(), RepackValidationError> {
    let index = reader.index();
    if index.revision != metadata.revision() {
        return Err(RepackValidationError::LayoutMismatch { field: "revision" });
    }
    if index.base_offset != metadata.base_offset() {
        return Err(RepackValidationError::LayoutMismatch {
            field: "base_offset",
        });
    }
    if index.raw_file_header != metadata.raw_file_header() {
        return Err(RepackValidationError::LayoutMismatch {
            field: "file_header",
        });
    }
    if index.storage_version != metadata.storage_version() {
        return Err(RepackValidationError::LayoutMismatch {
            field: "storage_version",
        });
    }
    let preamble_length =
        usize::try_from(index.base_offset).map_err(|_| RepackValidationError::LayoutMismatch {
            field: "base_offset",
        })?;
    if preamble_length != metadata.raw_preamble().len() {
        return Err(RepackValidationError::LayoutMismatch { field: "preamble" });
    }
    if preamble_length != 0 {
        let mut preamble = vec![0; preamble_length];
        reader
            .source_mut()
            .seek(SeekFrom::Start(0))
            .and_then(|_| reader.source_mut().read_exact(&mut preamble))
            .map_err(|error| RepackValidationError::PreambleRead { kind: error.kind() })?;
        if preamble != metadata.raw_preamble() {
            return Err(RepackValidationError::LayoutMismatch { field: "preamble" });
        }
    }
    Ok(())
}

fn entry_mismatch(index: usize, name: &str, field: &'static str) -> RepackValidationError {
    RepackValidationError::EntryMismatch {
        index,
        name: name.to_owned(),
        field,
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct AtomicRepackReport {
    pub write: CfWriteReport,
    pub validation: RepackValidationReport,
    pub published_bytes: u64,
}

#[derive(Debug)]
pub enum AtomicRepackError {
    ExistingDestination,
    InvalidDestination,
    TemporaryNameExhausted,
    Io {
        operation: &'static str,
        kind: std::io::ErrorKind,
    },
    Write(CfWriteError),
    Validation(RepackValidationError),
}

impl fmt::Display for AtomicRepackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExistingDestination => formatter.write_str("CF destination already exists"),
            Self::InvalidDestination => {
                formatter.write_str("CF destination has no usable filename")
            }
            Self::TemporaryNameExhausted => {
                formatter.write_str("cannot reserve a temporary CF filename")
            }
            Self::Io { operation, kind } => {
                write!(formatter, "atomic CF {operation} failed: {kind}")
            }
            Self::Write(source) => write!(formatter, "temporary CF write failed: {source}"),
            Self::Validation(source) => {
                write!(formatter, "temporary CF validation failed: {source}")
            }
        }
    }
}

impl Error for AtomicRepackError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Write(source) => Some(source),
            Self::Validation(source) => Some(source),
            Self::ExistingDestination
            | Self::InvalidDestination
            | Self::TemporaryNameExhausted
            | Self::Io { .. } => None,
        }
    }
}

/// Publish a repacked archive to a new path without exposing partial output.
/// Existing destinations are never intentionally replaced.
pub fn publish_repacked_new(
    archive: &CfArchive,
    destination: impl AsRef<Path>,
    limits: ResourceLimits,
) -> Result<AtomicRepackReport, AtomicRepackError> {
    publish_storage_image_new(archive.metadata(), archive.image(), destination, limits)
}

/// Atomic counterpart of [`write_storage_image`] for a new destination.
pub fn publish_storage_image_new(
    metadata: &CfArchiveMetadata,
    image: &StorageImage,
    destination: impl AsRef<Path>,
    limits: ResourceLimits,
) -> Result<AtomicRepackReport, AtomicRepackError> {
    let destination = destination.as_ref();
    destination_absent(destination)?;
    let filename = destination
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or(AtomicRepackError::InvalidDestination)?;
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let (temporary_path, mut temporary) = create_temporary(parent, filename)?;
    let mut guard = TemporaryGuard::new(temporary_path.clone());

    let write = write_storage_image(&mut temporary, metadata, image, limits)
        .map_err(AtomicRepackError::Write)?;
    temporary
        .flush()
        .map_err(|error| atomic_io("flush temporary output", error))?;
    temporary
        .sync_all()
        .map_err(|error| atomic_io("synchronize temporary output", error))?;
    let published_bytes = temporary
        .metadata()
        .map_err(|error| atomic_io("inspect temporary output", error))?
        .len();
    drop(temporary);

    let reopened =
        File::open(&temporary_path).map_err(|error| atomic_io("reopen temporary output", error))?;
    let validation = validate_repacked_archive(reopened, metadata, image, limits)
        .map_err(AtomicRepackError::Validation)?;
    if write.bytes_written != published_bytes {
        return Err(AtomicRepackError::Validation(
            RepackValidationError::LayoutMismatch {
                field: "reported_output_length",
            },
        ));
    }

    match fs::hard_link(&temporary_path, destination) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(AtomicRepackError::ExistingDestination);
        }
        Err(error) => return Err(atomic_io("publish temporary output", error)),
    }
    fs::remove_file(&temporary_path)
        .map_err(|error| atomic_io("remove published temporary link", error))?;
    guard.published = true;
    Ok(AtomicRepackReport {
        write,
        validation,
        published_bytes,
    })
}

fn destination_absent(path: &Path) -> Result<(), AtomicRepackError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(AtomicRepackError::ExistingDestination),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(atomic_io("inspect destination", error)),
    }
}

fn create_temporary(
    parent: &Path,
    filename: &std::ffi::OsStr,
) -> Result<(PathBuf, File), AtomicRepackError> {
    for attempt in 0..TEMPORARY_ATTEMPTS {
        let mut candidate_name = OsString::from(".");
        candidate_name.push(filename);
        candidate_name.push(format!(".ibcmd-cf-{}-{attempt}.tmp", std::process::id()));
        let candidate = parent.join(candidate_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(atomic_io("create temporary output", error)),
        }
    }
    Err(AtomicRepackError::TemporaryNameExhausted)
}

fn atomic_io(operation: &'static str, error: std::io::Error) -> AtomicRepackError {
    AtomicRepackError::Io {
        operation,
        kind: error.kind(),
    }
}

struct TemporaryGuard {
    path: PathBuf,
    published: bool,
}

impl TemporaryGuard {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            published: false,
        }
    }
}

impl Drop for TemporaryGuard {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_file(&self.path);
        }
    }
}
