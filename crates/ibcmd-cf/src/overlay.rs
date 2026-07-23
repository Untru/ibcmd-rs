//! Lossless same-profile overlay of a neutral storage patch onto a base CF.
//!
//! The adapter performs complete structural preflight before invoking codecs.
//! Compiled payloads replace only their exact physical targets, `NeedsBase`
//! entries are resolved through an explicit caller-owned codec, and an
//! `Unsupported` outcome blocks the whole operation.  Untouched entries retain
//! their order, headers, attributes, payload bytes, compression and origin.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt::{self, Display, Formatter},
    path::Path,
};

use ibcmd_core::{
    limits::ResourceLimits,
    storage::{
        CompressionKind, Sha256Digest, StorageBuildError, StorageEntry, StorageImage, StorageKey,
        StoragePatch, StoragePatchOutcome, StoragePatchTarget, StoragePayloads,
    },
};
use serde::Serialize;

use crate::{
    archive::CfArchive,
    payload::{PayloadDecodeError, PayloadEncoding, decode_payload},
    writer::{AtomicRepackError, AtomicRepackReport, publish_storage_image_new},
};

const VERSIONS_KEY: &str = "versions";

/// Caller-owned bridge to source-family codecs that remain outside `ibcmd-cf`.
///
/// Both methods receive exact packed bytes retained from the base archive.  A
/// returned payload must be in the same representation as the corresponding
/// base entry (`stored` or complete raw DEFLATE).
pub trait OverlayCodec {
    /// Resolves a compiler outcome that explicitly requires one base entry.
    fn resolve_needs_base(
        &mut self,
        target: &StoragePatchTarget,
        required: &StorageKey,
        base: &StorageEntry,
    ) -> Result<Vec<u8>, String>;

    /// Updates the native `versions` service entry for all changed logical keys.
    fn update_versions(
        &mut self,
        base: &StorageEntry,
        changed_keys: &[String],
    ) -> Result<Vec<u8>, String>;
}

/// One payload changed by an overlay plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OverlayChangeReport {
    pub logical_key: String,
    pub part_index: u32,
    pub source: OverlayChangeSource,
    pub before_sha256: String,
    pub after_sha256: String,
}

/// How a replacement payload was obtained.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayChangeSource {
    Compiled,
    NeedsBase,
    Versions,
}

/// Stable summary of an in-memory overlay.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OverlayReport {
    pub schema_version: u32,
    pub base_entries: usize,
    pub output_entries: usize,
    pub preserved_entries: usize,
    pub requested_entries: usize,
    pub versions_updated: bool,
    pub patch_sha256: String,
    pub output_image_sha256: String,
    pub changes: Vec<OverlayChangeReport>,
}

/// A structural reason why no overlay work may start.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum OverlayBlocker {
    Unsupported {
        logical_key: String,
        part_index: u32,
        reason: String,
    },
    MissingTarget {
        logical_key: String,
        part_index: u32,
    },
    MultipartMismatch {
        logical_key: String,
        part_index: u32,
        expected_part_count: u32,
        actual_part_count: u32,
    },
    MissingRequiredBase {
        logical_key: String,
        required: String,
    },
    AmbiguousRequiredBase {
        logical_key: String,
        required: String,
        candidates: usize,
    },
    MissingVersions,
    MultipartVersions {
        parts: usize,
    },
}

impl Display for OverlayBlocker {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported {
                logical_key,
                part_index,
                reason,
            } => write!(
                formatter,
                "overlay target `{logical_key}` part {part_index} is unsupported: {reason}"
            ),
            Self::MissingTarget {
                logical_key,
                part_index,
            } => write!(
                formatter,
                "overlay target `{logical_key}` part {part_index} is absent from the base"
            ),
            Self::MultipartMismatch {
                logical_key,
                part_index,
                expected_part_count,
                actual_part_count,
            } => write!(
                formatter,
                "overlay target `{logical_key}` part {part_index} declares {actual_part_count} parts but the base declares {expected_part_count}"
            ),
            Self::MissingRequiredBase {
                logical_key,
                required,
            } => write!(
                formatter,
                "overlay target `{logical_key}` requires missing base entry `{required}`"
            ),
            Self::AmbiguousRequiredBase {
                logical_key,
                required,
                candidates,
            } => write!(
                formatter,
                "overlay target `{logical_key}` requires base key `{required}` with {candidates} physical candidates"
            ),
            Self::MissingVersions => formatter.write_str(
                "overlay changes storage entries but the base has no `versions` service entry",
            ),
            Self::MultipartVersions { parts } => write!(
                formatter,
                "overlay requires one physical `versions` entry but the base contains {parts} parts"
            ),
        }
    }
}

/// Complete ordered blocker set returned before any codec is invoked.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OverlayPreflightError {
    blockers: Vec<OverlayBlocker>,
}

impl OverlayPreflightError {
    #[must_use]
    pub fn blockers(&self) -> &[OverlayBlocker] {
        &self.blockers
    }
}

impl Display for OverlayPreflightError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "CF overlay preflight found {} blocker(s)",
            self.blockers.len()
        )
    }
}

impl Error for OverlayPreflightError {}

/// Failure while materializing a preflighted overlay image.
#[derive(Debug)]
pub enum OverlayError {
    Preflight(OverlayPreflightError),
    NeedsBaseCodec {
        logical_key: String,
        required: String,
        message: String,
    },
    VersionsCodec {
        message: String,
    },
    UnsupportedCompression {
        logical_key: String,
        compression: String,
    },
    PayloadDecode {
        logical_key: String,
        source: PayloadDecodeError,
    },
    Storage(StorageBuildError),
}

impl Display for OverlayError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preflight(source) => source.fmt(formatter),
            Self::NeedsBaseCodec {
                logical_key,
                required,
                message,
            } => write!(
                formatter,
                "failed to resolve overlay target `{logical_key}` from base `{required}`: {message}"
            ),
            Self::VersionsCodec { message } => {
                write!(
                    formatter,
                    "failed to update overlay `versions` entry: {message}"
                )
            }
            Self::UnsupportedCompression {
                logical_key,
                compression,
            } => write!(
                formatter,
                "overlay target `{logical_key}` uses unsupported compression `{compression}`"
            ),
            Self::PayloadDecode {
                logical_key,
                source,
            } => write!(
                formatter,
                "replacement payload for `{logical_key}` cannot be decoded: {source}"
            ),
            Self::Storage(source) => write!(formatter, "invalid overlay storage image: {source}"),
        }
    }
}

impl Error for OverlayError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Preflight(source) => Some(source),
            Self::PayloadDecode { source, .. } => Some(source),
            Self::Storage(source) => Some(source),
            Self::NeedsBaseCodec { .. }
            | Self::VersionsCodec { .. }
            | Self::UnsupportedCompression { .. } => None,
        }
    }
}

impl From<StorageBuildError> for OverlayError {
    fn from(source: StorageBuildError) -> Self {
        Self::Storage(source)
    }
}

/// Successful atomic overlay publication.
#[derive(Debug)]
pub struct PublishedOverlay {
    pub overlay: OverlayReport,
    pub publication: AtomicRepackReport,
}

/// Failure before or during atomic publication.
#[derive(Debug)]
pub enum PublishOverlayError {
    Overlay(OverlayError),
    Publication(AtomicRepackError),
}

impl Display for PublishOverlayError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overlay(source) => source.fmt(formatter),
            Self::Publication(source) => source.fmt(formatter),
        }
    }
}

impl Error for PublishOverlayError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Overlay(source) => Some(source),
            Self::Publication(source) => Some(source),
        }
    }
}

/// Applies a patch entirely in memory after a complete structural preflight.
pub fn apply_overlay<C: OverlayCodec>(
    base: &StorageImage,
    patch: &StoragePatch,
    codec: &mut C,
    limits: ResourceLimits,
) -> Result<(StorageImage, OverlayReport), OverlayError> {
    let plan = preflight(base, patch).map_err(OverlayError::Preflight)?;
    let mut entries = base.entries().to_vec();
    let mut changes = Vec::with_capacity(patch.len().saturating_add(1));

    for planned in &plan.entries {
        let patch_entry = &patch.entries()[planned.patch_index];
        let packed = match patch_entry.outcome() {
            StoragePatchOutcome::Compiled(payload) => payload.bytes().to_vec(),
            StoragePatchOutcome::NeedsBase { required, .. } => codec
                .resolve_needs_base(
                    patch_entry.target(),
                    required,
                    &entries[planned.required_base_index.expect("preflighted base")],
                )
                .map_err(|message| OverlayError::NeedsBaseCodec {
                    logical_key: patch_entry.target().key().as_str().to_owned(),
                    required: required.as_str().to_owned(),
                    message,
                })?,
            StoragePatchOutcome::Unsupported { .. } => {
                unreachable!("unsupported outcomes are rejected by preflight")
            }
        };
        let source = match patch_entry.outcome() {
            StoragePatchOutcome::Compiled(_) => OverlayChangeSource::Compiled,
            StoragePatchOutcome::NeedsBase { .. } => OverlayChangeSource::NeedsBase,
            StoragePatchOutcome::Unsupported { .. } => unreachable!(),
        };
        replace_payload(
            &mut entries,
            planned.target_index,
            packed,
            source,
            limits,
            &mut changes,
        )?;
    }

    if let Some(versions_index) = plan.versions_index {
        let packed = codec
            .update_versions(&entries[versions_index], &plan.changed_keys)
            .map_err(|message| OverlayError::VersionsCodec { message })?;
        replace_payload(
            &mut entries,
            versions_index,
            packed,
            OverlayChangeSource::Versions,
            limits,
            &mut changes,
        )?;
    }

    let image = StorageImage::new(entries)?;
    let changed_physical = changes
        .iter()
        .map(|change| (change.logical_key.as_str(), change.part_index))
        .collect::<BTreeSet<_>>()
        .len();
    let report = OverlayReport {
        schema_version: 1,
        base_entries: base.len(),
        output_entries: image.len(),
        preserved_entries: image.len().saturating_sub(changed_physical),
        requested_entries: patch.len(),
        versions_updated: plan.versions_index.is_some(),
        patch_sha256: patch.sha256().to_string(),
        output_image_sha256: image.sha256().to_string(),
        changes,
    };
    Ok((image, report))
}

/// Applies, writes, synchronizes, reopens, validates and atomically publishes
/// an overlay. Existing destinations are never overwritten.
pub fn publish_overlay_new<C: OverlayCodec>(
    base: &CfArchive,
    patch: &StoragePatch,
    codec: &mut C,
    destination: impl AsRef<Path>,
    limits: ResourceLimits,
) -> Result<PublishedOverlay, PublishOverlayError> {
    let (image, overlay) =
        apply_overlay(base.image(), patch, codec, limits).map_err(PublishOverlayError::Overlay)?;
    let publication = publish_storage_image_new(base.metadata(), &image, destination, limits)
        .map_err(PublishOverlayError::Publication)?;
    Ok(PublishedOverlay {
        overlay,
        publication,
    })
}

#[derive(Debug)]
struct OverlayPlanEntry {
    patch_index: usize,
    target_index: usize,
    required_base_index: Option<usize>,
}

#[derive(Debug)]
struct OverlayPlan {
    entries: Vec<OverlayPlanEntry>,
    changed_keys: Vec<String>,
    versions_index: Option<usize>,
}

fn preflight(
    base: &StorageImage,
    patch: &StoragePatch,
) -> Result<OverlayPlan, OverlayPreflightError> {
    let mut exact = BTreeMap::<(&str, u32), usize>::new();
    let mut by_key = BTreeMap::<&str, Vec<usize>>::new();
    for (index, entry) in base.entries().iter().enumerate() {
        exact.insert(
            (entry.logical_key().as_str(), entry.multipart().part_index()),
            index,
        );
        by_key
            .entry(entry.logical_key().as_str())
            .or_default()
            .push(index);
    }

    let mut blockers = Vec::new();
    let mut entries = Vec::with_capacity(patch.len());
    let mut changed_keys = BTreeSet::<String>::new();
    for (patch_index, entry) in patch.entries().iter().enumerate() {
        let target = entry.target();
        let key = target.key().as_str();
        let part = target.multipart().part_index();
        if key != VERSIONS_KEY {
            changed_keys.insert(key.to_owned());
        }
        if let StoragePatchOutcome::Unsupported { reason } = entry.outcome() {
            blockers.push(OverlayBlocker::Unsupported {
                logical_key: key.to_owned(),
                part_index: part,
                reason: reason.as_str().to_owned(),
            });
        }

        let Some(target_index) = exact.get(&(key, part)).copied() else {
            blockers.push(OverlayBlocker::MissingTarget {
                logical_key: key.to_owned(),
                part_index: part,
            });
            continue;
        };
        let base_target = &base.entries()[target_index];
        if base_target.multipart().part_count() != target.multipart().part_count() {
            blockers.push(OverlayBlocker::MultipartMismatch {
                logical_key: key.to_owned(),
                part_index: part,
                expected_part_count: base_target.multipart().part_count(),
                actual_part_count: target.multipart().part_count(),
            });
            continue;
        }

        let required_base_index = match entry.outcome() {
            StoragePatchOutcome::NeedsBase { required, .. } => {
                match by_key.get(required.as_str()) {
                    None => {
                        blockers.push(OverlayBlocker::MissingRequiredBase {
                            logical_key: key.to_owned(),
                            required: required.as_str().to_owned(),
                        });
                        continue;
                    }
                    Some(candidates) if candidates.len() != 1 => {
                        blockers.push(OverlayBlocker::AmbiguousRequiredBase {
                            logical_key: key.to_owned(),
                            required: required.as_str().to_owned(),
                            candidates: candidates.len(),
                        });
                        continue;
                    }
                    Some(candidates) => Some(candidates[0]),
                }
            }
            StoragePatchOutcome::Compiled(_) | StoragePatchOutcome::Unsupported { .. } => None,
        };
        entries.push(OverlayPlanEntry {
            patch_index,
            target_index,
            required_base_index,
        });
    }

    let versions_index = if changed_keys.is_empty() {
        None
    } else {
        match by_key.get(VERSIONS_KEY) {
            None => {
                blockers.push(OverlayBlocker::MissingVersions);
                None
            }
            Some(candidates) if candidates.len() != 1 => {
                blockers.push(OverlayBlocker::MultipartVersions {
                    parts: candidates.len(),
                });
                None
            }
            Some(candidates) => Some(candidates[0]),
        }
    };

    if blockers.is_empty() {
        Ok(OverlayPlan {
            entries,
            changed_keys: changed_keys.into_iter().collect(),
            versions_index,
        })
    } else {
        Err(OverlayPreflightError { blockers })
    }
}

fn replace_payload(
    entries: &mut [StorageEntry],
    index: usize,
    packed: Vec<u8>,
    source: OverlayChangeSource,
    limits: ResourceLimits,
    changes: &mut Vec<OverlayChangeReport>,
) -> Result<(), OverlayError> {
    let base = &entries[index];
    let encoding = compression_encoding(base.logical_key().as_str(), base.compression())?;
    let unpacked = decode_payload(encoding, &packed, limits)
        .map_err(|source| OverlayError::PayloadDecode {
            logical_key: base.logical_key().as_str().to_owned(),
            source,
        })?
        .into_bytes();
    let before_sha256 = Sha256Digest::for_bytes(base.packed_payload());
    let after_sha256 = Sha256Digest::for_bytes(&packed);
    let replacement = StorageEntry::new(
        base.logical_name().clone(),
        base.logical_key().clone(),
        base.multipart(),
        base.opaque_metadata().clone(),
        StoragePayloads::new(packed, unpacked)?,
        base.compression().clone(),
        base.origin().clone(),
    )?;
    changes.push(OverlayChangeReport {
        logical_key: base.logical_key().as_str().to_owned(),
        part_index: base.multipart().part_index(),
        source,
        before_sha256: before_sha256.to_string(),
        after_sha256: after_sha256.to_string(),
    });
    entries[index] = replacement;
    Ok(())
}

fn compression_encoding(
    logical_key: &str,
    compression: &CompressionKind,
) -> Result<PayloadEncoding, OverlayError> {
    match compression.as_str() {
        "stored" => Ok(PayloadEncoding::Stored),
        "raw-deflate" => Ok(PayloadEncoding::RawDeflate),
        other => Err(OverlayError::UnsupportedCompression {
            logical_key: logical_key.to_owned(),
            compression: other.to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use ibcmd_core::{
        artifact::StorageProfileId,
        storage::{
            MultipartIdentity, OpaqueStorageMetadata, StorageName, StorageOrigin,
            StoragePatchEntry, StoragePatchOutcome, StorageProvenance,
        },
    };

    use super::*;

    fn storage_entry(key: &str, bytes: &[u8]) -> StorageEntry {
        StorageEntry::new(
            StorageName::new(key).unwrap(),
            StorageKey::new(key).unwrap(),
            MultipartIdentity::single(),
            OpaqueStorageMetadata::empty(),
            StoragePayloads::new(bytes.to_vec(), bytes.to_vec()).unwrap(),
            CompressionKind::stored(),
            StorageOrigin::new(
                StorageProfileId::parse("storage:overlay-test").unwrap(),
                StorageProvenance::new("clean-room overlay fixture").unwrap(),
            ),
        )
        .unwrap()
    }

    fn target(key: &str) -> StoragePatchTarget {
        StoragePatchTarget::new(
            StorageKey::new(key).unwrap(),
            MultipartIdentity::single(),
            StorageProvenance::new(&format!("source/{key}")).unwrap(),
        )
    }

    struct TestCodec {
        calls: Arc<AtomicUsize>,
    }

    impl OverlayCodec for TestCodec {
        fn resolve_needs_base(
            &mut self,
            _target: &StoragePatchTarget,
            _required: &StorageKey,
            base: &StorageEntry,
        ) -> Result<Vec<u8>, String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            let mut bytes = base.packed_payload().to_vec();
            bytes.extend_from_slice(b"+xml");
            Ok(bytes)
        }

        fn update_versions(
            &mut self,
            base: &StorageEntry,
            changed_keys: &[String],
        ) -> Result<Vec<u8>, String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            let mut bytes = base.packed_payload().to_vec();
            bytes.extend_from_slice(changed_keys.join(",").as_bytes());
            Ok(bytes)
        }
    }

    #[test]
    fn replaces_compiled_and_base_dependent_entries_and_preserves_order() {
        let base = StorageImage::new(vec![
            storage_entry("unknown", b"opaque"),
            storage_entry("module.0", b"old-module"),
            storage_entry("metadata.0", b"old-metadata"),
            storage_entry("versions", b"old-versions:"),
        ])
        .unwrap();
        let patch = StoragePatch::new(vec![
            StoragePatchEntry::new(
                target("module.0"),
                StoragePatchOutcome::compiled(b"new-module".to_vec()).unwrap(),
            ),
            StoragePatchEntry::new(
                target("metadata.0"),
                StoragePatchOutcome::needs_base(
                    StorageKey::new("metadata.0").unwrap(),
                    "metadata XML needs its base row",
                )
                .unwrap(),
            ),
        ])
        .unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let mut codec = TestCodec {
            calls: Arc::clone(&calls),
        };

        let (image, report) =
            apply_overlay(&base, &patch, &mut codec, ResourceLimits::default()).unwrap();

        assert_eq!(calls.load(Ordering::Relaxed), 2);
        assert_eq!(image.entries()[0], base.entries()[0]);
        assert_eq!(image.entries()[1].packed_payload(), b"new-module");
        assert_eq!(image.entries()[2].packed_payload(), b"old-metadata+xml");
        assert_eq!(
            image.entries()[3].packed_payload(),
            b"old-versions:metadata.0,module.0"
        );
        assert_eq!(report.preserved_entries, 1);
        assert!(report.versions_updated);
        assert_eq!(
            report
                .changes
                .iter()
                .map(|entry| entry.source)
                .collect::<Vec<_>>(),
            [
                OverlayChangeSource::Compiled,
                OverlayChangeSource::NeedsBase,
                OverlayChangeSource::Versions,
            ]
        );
    }

    #[test]
    fn unsupported_blocks_every_codec_before_materialization() {
        let base = StorageImage::new(vec![
            storage_entry("asset", b"old"),
            storage_entry("versions", b"versions"),
        ])
        .unwrap();
        let patch = StoragePatch::new(vec![StoragePatchEntry::new(
            target("asset"),
            StoragePatchOutcome::unsupported("codec is unavailable").unwrap(),
        )])
        .unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let mut codec = TestCodec {
            calls: Arc::clone(&calls),
        };

        let error =
            apply_overlay(&base, &patch, &mut codec, ResourceLimits::default()).unwrap_err();

        assert!(matches!(error, OverlayError::Preflight(_)));
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert_eq!(base.entries()[0].packed_payload(), b"old");
    }
}
