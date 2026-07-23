//! Base-free assembly and atomic publication of a new CF archive.
//!
//! The input is a completely compiled neutral [`StoragePatch`].  No base CF,
//! platform executable, EDT/JVM component, or opaque container template is
//! consulted.  Patch preflight and payload decoding finish before an output
//! file is created.

use std::{
    error::Error,
    ffi::OsString,
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
};

use ibcmd_core::{
    artifact::StorageProfileId,
    limits::ResourceLimits,
    storage::{
        CompressionKind, MultipartIdentity, OpaqueStorageMetadata, StorageBuildError, StorageEntry,
        StorageImage, StorageKey, StorageName, StorageOrigin, StoragePatch,
        StoragePatchPreflightError, StoragePayloads, StorageProvenance,
    },
};
use ibcmd_v8::{
    format::Revision,
    format15, format16,
    writer::{
        Format15Document, Format15Element, Format16Document, Format16Element, Format16Payload,
        make_element_header, write_format15,
    },
};

use crate::{
    archive::{CfDataState, CfEntryAttributes, CfEntryAttributesError, decode_archive_uniform},
    payload::{PayloadDecodeError, PayloadDecoder, PayloadEncoding},
    preamble::{
        Format16ArchiveWriterError, PreambleMode, SemanticPreamble, SemanticPreambleEntry,
        write_format16_archive,
    },
    writer::CfWriteReport,
};

const TEMPORARY_ATTEMPTS: u32 = 1_024;
const REQUIRED_SPECIAL_ENTRIES: [&str; 3] = ["root", "version", "versions"];

/// Independent physical-container coordinates for a new CF artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapCfProfile {
    revision: Revision,
    page_size: u32,
    storage_version: u32,
    reserved: u32,
    storage_profile: StorageProfileId,
}

impl BootstrapCfProfile {
    /// Creates the conservative clean-room layout used by the standalone
    /// bootstrap path.  Container revision is deliberately independent from
    /// XML dialect and platform-build profile.
    pub fn new(
        revision: Revision,
        storage_version: u32,
        storage_profile: StorageProfileId,
    ) -> Self {
        let page_size = match revision {
            Revision::Format15 => format15::DEFAULT_PAGE_SIZE,
            Revision::Format16 => format16::DEFAULT_PAGE_SIZE,
        };
        Self {
            revision,
            page_size,
            storage_version,
            reserved: 0,
            storage_profile,
        }
    }

    /// Overrides the independently selected physical page size.
    #[must_use]
    pub const fn with_page_size(mut self, page_size: u32) -> Self {
        self.page_size = page_size;
        self
    }

    /// Overrides the retained container reserved word.
    #[must_use]
    pub const fn with_reserved(mut self, reserved: u32) -> Self {
        self.reserved = reserved;
        self
    }

    #[must_use]
    pub const fn revision(&self) -> Revision {
        self.revision
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
    pub const fn storage_profile(&self) -> &StorageProfileId {
        &self.storage_profile
    }
}

/// A complete, preflighted image ready for deterministic CF serialization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapArtifact {
    profile: BootstrapCfProfile,
    image: StorageImage,
}

impl BootstrapArtifact {
    #[must_use]
    pub const fn profile(&self) -> &BootstrapCfProfile {
        &self.profile
    }

    #[must_use]
    pub const fn image(&self) -> &StorageImage {
        &self.image
    }
}

/// Converts an all-compiled patch into a complete ordered storage image.
///
/// The raw-DEFLATE payload of every target is decoded under shared resource
/// limits before the returned artifact can be written.  Blocked, multipart,
/// malformed, or special-entry-incomplete patches fail closed.
pub fn assemble_bootstrap_artifact(
    patch: StoragePatch,
    profile: BootstrapCfProfile,
    limits: ResourceLimits,
) -> Result<BootstrapArtifact, BootstrapError> {
    if profile.page_size == 0 {
        return Err(BootstrapError::ZeroPageSize);
    }
    patch.preflight().map_err(BootstrapError::PatchBlocked)?;
    require_special_entries(&patch)?;

    let address_bytes = match profile.revision {
        Revision::Format15 => format15::ELEMENT_ADDRESS_SIZE,
        Revision::Format16 => format16::ELEMENT_ADDRESS_SIZE,
    };
    let origin_profile = profile.storage_profile.clone();
    let mut decoder = PayloadDecoder::new(limits);
    let mut entries = Vec::with_capacity(patch.len());
    for (index, compiled) in patch
        .into_compiled()
        .map_err(BootstrapError::PatchBlocked)?
        .into_iter()
        .enumerate()
    {
        let (target, payload) = compiled.into_parts();
        let key = target.key().as_str();
        if target.multipart() != MultipartIdentity::single() {
            return Err(BootstrapError::MultipartTarget {
                index,
                key: key.to_owned(),
                part_index: target.multipart().part_index(),
                part_count: target.multipart().part_count(),
            });
        }
        let unpacked = decoder
            .decode(PayloadEncoding::RawDeflate, payload.bytes())
            .map_err(|source| BootstrapError::Payload {
                index,
                key: key.to_owned(),
                source,
            })?
            .into_bytes();
        let attributes = CfEntryAttributes::new(CfDataState::Present, vec![0; address_bytes])
            .map_err(BootstrapError::Attributes)?
            .encode();
        let entry = StorageEntry::new(
            StorageName::new(key).map_err(BootstrapError::Storage)?,
            StorageKey::new(key).map_err(BootstrapError::Storage)?,
            MultipartIdentity::single(),
            OpaqueStorageMetadata::new(attributes, make_element_header(key))
                .map_err(BootstrapError::Storage)?,
            StoragePayloads::new(payload.bytes().to_vec(), unpacked)
                .map_err(BootstrapError::Storage)?,
            CompressionKind::raw_deflate(),
            StorageOrigin::new(origin_profile.clone(), target.provenance().clone()),
        )
        .map_err(BootstrapError::Storage)?;
        entries.push(entry);
    }
    let image = StorageImage::new(entries).map_err(BootstrapError::Storage)?;
    Ok(BootstrapArtifact { profile, image })
}

fn require_special_entries(patch: &StoragePatch) -> Result<(), BootstrapError> {
    for required in REQUIRED_SPECIAL_ENTRIES {
        if !patch
            .entries()
            .iter()
            .any(|entry| entry.target().key().as_str() == required)
        {
            return Err(BootstrapError::MissingSpecialEntry { key: required });
        }
    }
    Ok(())
}

/// Serializes a preflighted artifact without consulting a base CF.
pub fn write_bootstrap_artifact<W: Write>(
    target: &mut W,
    artifact: &BootstrapArtifact,
) -> Result<CfWriteReport, BootstrapError> {
    match artifact.profile.revision {
        Revision::Format15 => {
            let mut document = Format15Document::new(
                artifact.profile.storage_version,
                artifact
                    .image
                    .entries()
                    .iter()
                    .map(|entry| {
                        Format15Element::preserved(
                            entry.raw_header().to_vec(),
                            Some(entry.packed_payload().to_vec()),
                        )
                    })
                    .collect(),
            );
            document.page_size = artifact.profile.page_size;
            document.reserved = artifact.profile.reserved;
            let report = write_format15(target, &document).map_err(BootstrapError::Format15)?;
            Ok(CfWriteReport {
                revision: Revision::Format15,
                bytes_written: report.bytes_written,
                entries_written: report.entries_written,
            })
        }
        Revision::Format16 => {
            let mut preamble = SemanticPreamble::new(
                artifact.profile.storage_version,
                REQUIRED_SPECIAL_ENTRIES
                    .into_iter()
                    .map(|name| SemanticPreambleEntry::new(name, None))
                    .collect(),
            );
            preamble.page_size = format15::DEFAULT_PAGE_SIZE;
            preamble.reserved = artifact.profile.reserved;
            let mut document = Format16Document::new(
                artifact.profile.storage_version,
                artifact
                    .image
                    .entries()
                    .iter()
                    .map(|entry| {
                        Format16Element::preserved(
                            entry.raw_header().to_vec(),
                            Some(Format16Payload::from_bytes(entry.packed_payload())),
                        )
                    })
                    .collect(),
            );
            document.page_size = artifact.profile.page_size;
            document.reserved = artifact.profile.reserved;
            let report =
                write_format16_archive(target, &PreambleMode::generate(preamble), &mut document)
                    .map_err(BootstrapError::Format16)?;
            Ok(CfWriteReport {
                revision: Revision::Format16,
                bytes_written: report.bytes_written(),
                entries_written: report.primary.entries_written,
            })
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootstrapValidationReport {
    pub revision: Revision,
    pub entries_validated: usize,
}

/// Reopens a generated artifact and proves exact ordered payload coverage.
pub fn validate_bootstrap_artifact<R: Read + Seek>(
    source: R,
    artifact: &BootstrapArtifact,
    limits: ResourceLimits,
) -> Result<BootstrapValidationReport, BootstrapError> {
    let provenance = StorageProvenance::new("standalone CF bootstrap reopen validation")
        .expect("static bootstrap provenance is valid");
    let archive = decode_archive_uniform(
        source,
        limits,
        artifact.profile.storage_profile.clone(),
        provenance,
        PayloadEncoding::RawDeflate,
    )
    .map_err(BootstrapError::Reopen)?;
    if archive.metadata().revision() != artifact.profile.revision {
        return Err(BootstrapError::LayoutMismatch { field: "revision" });
    }
    if archive.metadata().page_size() != artifact.profile.page_size {
        return Err(BootstrapError::LayoutMismatch { field: "page_size" });
    }
    if archive.metadata().storage_version() != artifact.profile.storage_version {
        return Err(BootstrapError::LayoutMismatch {
            field: "storage_version",
        });
    }
    if archive.metadata().reserved() != artifact.profile.reserved {
        return Err(BootstrapError::LayoutMismatch { field: "reserved" });
    }
    let actual = archive.image().entries();
    let expected = artifact.image.entries();
    if actual.len() != expected.len() {
        return Err(BootstrapError::EntryCountMismatch {
            expected: expected.len(),
            actual: actual.len(),
        });
    }
    for (index, (expected, actual)) in expected.iter().zip(actual).enumerate() {
        let key = expected.logical_key().as_str();
        if actual.logical_key() != expected.logical_key() {
            return Err(BootstrapError::EntryMismatch {
                index,
                key: key.to_owned(),
                field: "logical_key",
            });
        }
        if actual.packed_payload() != expected.packed_payload() {
            return Err(BootstrapError::EntryMismatch {
                index,
                key: key.to_owned(),
                field: "packed_payload",
            });
        }
        if actual.unpacked_payload() != expected.unpacked_payload() {
            return Err(BootstrapError::EntryMismatch {
                index,
                key: key.to_owned(),
                field: "unpacked_payload",
            });
        }
    }
    Ok(BootstrapValidationReport {
        revision: artifact.profile.revision,
        entries_validated: expected.len(),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomicBootstrapReport {
    pub write: CfWriteReport,
    pub validation: BootstrapValidationReport,
    pub published_bytes: u64,
}

/// Preflights, writes, reopens, validates, and race-safely publishes a new CF.
/// Existing destinations are never overwritten.
pub fn publish_bootstrap_patch_new(
    patch: StoragePatch,
    profile: BootstrapCfProfile,
    destination: impl AsRef<Path>,
    limits: ResourceLimits,
) -> Result<AtomicBootstrapReport, BootstrapError> {
    // Crucially, complete patch/payload preflight happens before even looking
    // for a temporary output name.
    let artifact = assemble_bootstrap_artifact(patch, profile, limits)?;
    let destination = destination.as_ref();
    destination_absent(destination)?;
    let filename = destination
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or(BootstrapError::InvalidDestination)?;
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let (temporary_path, mut temporary) = create_temporary(parent, filename)?;
    let mut guard = TemporaryGuard::new(temporary_path.clone());

    let write = write_bootstrap_artifact(&mut temporary, &artifact)?;
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
    let validation = validate_bootstrap_artifact(reopened, &artifact, limits)?;
    if write.bytes_written != published_bytes {
        return Err(BootstrapError::LayoutMismatch {
            field: "reported_output_length",
        });
    }

    match fs::hard_link(&temporary_path, destination) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(BootstrapError::ExistingDestination);
        }
        Err(error) => return Err(atomic_io("publish temporary output", error)),
    }
    fs::remove_file(&temporary_path)
        .map_err(|error| atomic_io("remove published temporary link", error))?;
    guard.published = true;
    Ok(AtomicBootstrapReport {
        write,
        validation,
        published_bytes,
    })
}

fn destination_absent(path: &Path) -> Result<(), BootstrapError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(BootstrapError::ExistingDestination),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(atomic_io("inspect destination", error)),
    }
}

fn create_temporary(
    parent: &Path,
    filename: &std::ffi::OsStr,
) -> Result<(PathBuf, File), BootstrapError> {
    for attempt in 0..TEMPORARY_ATTEMPTS {
        let mut candidate_name = OsString::from(".");
        candidate_name.push(filename);
        candidate_name.push(format!(
            ".ibcmd-cf-bootstrap-{}-{attempt}.tmp",
            std::process::id()
        ));
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
    Err(BootstrapError::TemporaryNameExhausted)
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

fn atomic_io(operation: &'static str, error: std::io::Error) -> BootstrapError {
    BootstrapError::Io {
        operation,
        kind: error.kind(),
    }
}

#[derive(Debug)]
pub enum BootstrapError {
    PatchBlocked(StoragePatchPreflightError),
    MissingSpecialEntry {
        key: &'static str,
    },
    MultipartTarget {
        index: usize,
        key: String,
        part_index: u32,
        part_count: u32,
    },
    ZeroPageSize,
    Payload {
        index: usize,
        key: String,
        source: PayloadDecodeError,
    },
    Attributes(CfEntryAttributesError),
    Storage(StorageBuildError),
    Format15(ibcmd_v8::writer::WriterError),
    Format16(Format16ArchiveWriterError),
    Reopen(crate::archive::ArchiveDecodeError),
    LayoutMismatch {
        field: &'static str,
    },
    EntryCountMismatch {
        expected: usize,
        actual: usize,
    },
    EntryMismatch {
        index: usize,
        key: String,
        field: &'static str,
    },
    ExistingDestination,
    InvalidDestination,
    TemporaryNameExhausted,
    Io {
        operation: &'static str,
        kind: std::io::ErrorKind,
    },
}

impl fmt::Display for BootstrapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PatchBlocked(source) => write!(formatter, "bootstrap patch is blocked: {source}"),
            Self::MissingSpecialEntry { key } => {
                write!(formatter, "bootstrap patch has no required `{key}` entry")
            }
            Self::MultipartTarget {
                index,
                key,
                part_index,
                part_count,
            } => write!(
                formatter,
                "bootstrap target {index} (`{key}`) is multipart part {part_index}/{part_count}"
            ),
            Self::ZeroPageSize => formatter.write_str("bootstrap CF page size must be non-zero"),
            Self::Payload { index, key, source } => write!(
                formatter,
                "bootstrap target {index} (`{key}`) is not a valid raw-DEFLATE payload: {source}"
            ),
            Self::Attributes(source) => {
                write!(formatter, "invalid bootstrap CF attributes: {source}")
            }
            Self::Storage(source) => write!(formatter, "invalid bootstrap storage image: {source}"),
            Self::Format15(source) => source.fmt(formatter),
            Self::Format16(source) => source.fmt(formatter),
            Self::Reopen(source) => write!(formatter, "generated CF cannot be reopened: {source}"),
            Self::LayoutMismatch { field } => {
                write!(formatter, "generated CF layout changed `{field}`")
            }
            Self::EntryCountMismatch { expected, actual } => write!(
                formatter,
                "generated CF has {actual} entries instead of {expected}"
            ),
            Self::EntryMismatch { index, key, field } => write!(
                formatter,
                "generated CF entry {index} (`{key}`) changed `{field}`"
            ),
            Self::ExistingDestination => formatter.write_str("CF destination already exists"),
            Self::InvalidDestination => {
                formatter.write_str("CF destination has no usable filename")
            }
            Self::TemporaryNameExhausted => {
                formatter.write_str("cannot reserve a temporary bootstrap CF filename")
            }
            Self::Io { operation, kind } => {
                write!(formatter, "atomic bootstrap CF {operation} failed: {kind}")
            }
        }
    }
}

impl Error for BootstrapError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::PatchBlocked(source) => Some(source),
            Self::Payload { source, .. } => Some(source),
            Self::Attributes(source) => Some(source),
            Self::Storage(source) => Some(source),
            Self::Format15(source) => Some(source),
            Self::Format16(source) => Some(source),
            Self::Reopen(source) => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::Cursor,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use ibcmd_core::storage::{StoragePatchEntry, StoragePatchOutcome, StoragePatchTarget};
    use ibcmd_v8::format16;

    use crate::payload::{PayloadEncoding, encode_payload};

    use super::*;

    struct TempDirectory(PathBuf);

    impl TempDirectory {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "ibcmd-cf-bootstrap-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn profile(revision: Revision) -> BootstrapCfProfile {
        BootstrapCfProfile::new(
            revision,
            5,
            StorageProfileId::parse("storage:cf-bootstrap-test").unwrap(),
        )
    }

    fn target(key: &str) -> StoragePatchTarget {
        StoragePatchTarget::new(
            StorageKey::new(key).unwrap(),
            MultipartIdentity::single(),
            StorageProvenance::new(&format!("fixture:{key}")).unwrap(),
        )
    }

    fn compiled_patch() -> StoragePatch {
        let limits = ResourceLimits::default();
        StoragePatch::new(
            [
                (
                    "11111111-1111-4111-8111-111111111111",
                    b"configuration".as_slice(),
                ),
                ("root", b"root".as_slice()),
                ("version", b"version".as_slice()),
                ("versions", b"versions".as_slice()),
            ]
            .into_iter()
            .map(|(key, bytes)| {
                let packed = encode_payload(PayloadEncoding::RawDeflate, bytes, limits).unwrap();
                StoragePatchEntry::new(target(key), StoragePatchOutcome::compiled(packed).unwrap())
            })
            .collect(),
        )
        .unwrap()
    }

    #[test]
    fn both_revisions_assemble_write_and_reopen_without_a_base() {
        for revision in [Revision::Format15, Revision::Format16] {
            let artifact = assemble_bootstrap_artifact(
                compiled_patch(),
                profile(revision),
                ResourceLimits::default(),
            )
            .unwrap();
            assert_eq!(artifact.image().len(), 4);
            assert!(
                artifact
                    .image()
                    .entries()
                    .iter()
                    .all(|entry| entry.compression().as_str() == "raw-deflate")
            );

            let mut bytes = Vec::new();
            let report = write_bootstrap_artifact(&mut bytes, &artifact).unwrap();
            assert_eq!(report.revision, revision);
            assert_eq!(report.entries_written, 4);
            assert_eq!(report.bytes_written, bytes.len() as u64);
            let validation = validate_bootstrap_artifact(
                Cursor::new(&bytes),
                &artifact,
                ResourceLimits::default(),
            )
            .unwrap();
            assert_eq!(validation.entries_validated, 4);
            if revision == Revision::Format16 {
                let parsed = format16::parse(&bytes).unwrap();
                assert_eq!(parsed.base_offset, format16::BASE_OFFSET);
                assert_eq!(
                    parsed
                        .preamble
                        .as_ref()
                        .unwrap()
                        .elements
                        .iter()
                        .map(|entry| entry.name.as_str())
                        .collect::<Vec<_>>(),
                    REQUIRED_SPECIAL_ENTRIES
                );
            }
        }
    }

    #[test]
    fn blocked_or_invalid_patch_fails_before_output_exists() {
        let temp = TempDirectory::new("blocked");
        let output = temp.0.join("blocked.cf");
        let mut entries = compiled_patch().into_entries();
        entries[0] = StoragePatchEntry::new(
            target("11111111-1111-4111-8111-111111111111"),
            StoragePatchOutcome::unsupported("fixture blocker").unwrap(),
        );
        let error = publish_bootstrap_patch_new(
            StoragePatch::new(entries).unwrap(),
            profile(Revision::Format15),
            &output,
            ResourceLimits::default(),
        )
        .unwrap_err();
        assert!(matches!(error, BootstrapError::PatchBlocked(_)));
        assert!(!output.exists());
        assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 0);

        let mut entries = compiled_patch().into_entries();
        entries[0] = StoragePatchEntry::new(
            target("11111111-1111-4111-8111-111111111111"),
            StoragePatchOutcome::compiled(b"not-deflate".to_vec()).unwrap(),
        );
        let error = publish_bootstrap_patch_new(
            StoragePatch::new(entries).unwrap(),
            profile(Revision::Format15),
            &output,
            ResourceLimits::default(),
        )
        .unwrap_err();
        assert!(matches!(error, BootstrapError::Payload { .. }));
        assert!(!output.exists());
        assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 0);
    }

    #[test]
    fn atomic_publication_is_validated_and_never_clobbers() {
        let temp = TempDirectory::new("publish");
        for revision in [Revision::Format15, Revision::Format16] {
            let output = temp.0.join(format!("{revision:?}.cf"));
            let report = publish_bootstrap_patch_new(
                compiled_patch(),
                profile(revision),
                &output,
                ResourceLimits::default(),
            )
            .unwrap();
            assert_eq!(report.write.entries_written, 4);
            assert_eq!(report.validation.entries_validated, 4);
            assert_eq!(report.published_bytes, fs::metadata(&output).unwrap().len());

            let original = fs::read(&output).unwrap();
            assert!(matches!(
                publish_bootstrap_patch_new(
                    compiled_patch(),
                    profile(revision),
                    &output,
                    ResourceLimits::default(),
                ),
                Err(BootstrapError::ExistingDestination)
            ));
            assert_eq!(fs::read(&output).unwrap(), original);
        }
        assert!(!fs::read_dir(&temp.0).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".tmp")
        }));
    }

    #[test]
    fn missing_special_entry_and_zero_page_size_fail_closed() {
        let mut entries = compiled_patch().into_entries();
        entries.retain(|entry| entry.target().key().as_str() != "versions");
        assert!(matches!(
            assemble_bootstrap_artifact(
                StoragePatch::new(entries).unwrap(),
                profile(Revision::Format15),
                ResourceLimits::default(),
            ),
            Err(BootstrapError::MissingSpecialEntry { key: "versions" })
        ));
        assert!(matches!(
            assemble_bootstrap_artifact(
                compiled_patch(),
                profile(Revision::Format15).with_page_size(0),
                ResourceLimits::default(),
            ),
            Err(BootstrapError::ZeroPageSize)
        ));
    }
}
