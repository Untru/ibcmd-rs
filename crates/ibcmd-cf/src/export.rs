//! Provider-neutral planning and reporting for offline storage export.
//!
//! This module deliberately knows nothing about SQL, XML, or the filesystem.
//! It presents validated [`StorageImage`] records to an exporter in stable
//! source order and supplies a common machine-readable disposition report.

use std::{
    borrow::Cow,
    collections::BTreeMap,
    error::Error,
    fmt::{self, Display, Formatter},
};

use ibcmd_core::storage::{StorageEntry, StorageImage};
use serde::Serialize;

/// One logical storage record and all of its physical parts.
#[derive(Clone, Debug)]
pub struct StorageExportRecord<'a> {
    logical_name: &'a str,
    logical_key: &'a str,
    parts: Vec<&'a StorageEntry>,
}

impl<'a> StorageExportRecord<'a> {
    /// Returns the display name retained by the source adapter.
    #[must_use]
    pub const fn logical_name(&self) -> &'a str {
        self.logical_name
    }

    /// Returns the stable logical identity retained by the source adapter.
    #[must_use]
    pub const fn logical_key(&self) -> &'a str {
        self.logical_key
    }

    /// Returns physical parts in their validated multipart order.
    #[must_use]
    pub fn parts(&self) -> &[&'a StorageEntry] {
        &self.parts
    }

    /// Returns the number of physical parts.
    #[must_use]
    pub fn part_count(&self) -> usize {
        self.parts.len()
    }

    /// Returns the aggregate exact packed byte count.
    pub fn packed_bytes(&self) -> Result<usize, StorageExportPlanError> {
        self.parts.iter().try_fold(0_usize, |total, entry| {
            total
                .checked_add(entry.packed_payload().len())
                .ok_or_else(|| StorageExportPlanError::PackedSizeOverflow {
                    logical_key: self.logical_key.to_owned(),
                })
        })
    }

    /// Returns the exact provider-packed representation without copying a
    /// single-part record and with one bounded concatenation for multipart.
    pub fn packed_payload(&self) -> Result<Cow<'a, [u8]>, StorageExportPlanError> {
        if let [entry] = self.parts.as_slice() {
            return Ok(Cow::Borrowed(entry.packed_payload()));
        }

        let mut payload = Vec::with_capacity(self.packed_bytes()?);
        for entry in &self.parts {
            payload.extend_from_slice(entry.packed_payload());
        }
        Ok(Cow::Owned(payload))
    }
}

/// Logical export records derived from a validated neutral storage image.
#[derive(Clone, Debug)]
pub struct StorageExportPlan<'a> {
    physical_entries: usize,
    records: Vec<StorageExportRecord<'a>>,
}

impl<'a> StorageExportPlan<'a> {
    /// Groups multipart entries by logical key while preserving the first
    /// source occurrence of every record and the validated order of its parts.
    #[must_use]
    pub fn from_image(image: &'a StorageImage) -> Self {
        let mut record_indexes = BTreeMap::<&str, usize>::new();
        let mut records = Vec::<StorageExportRecord<'a>>::new();
        for entry in image.entries() {
            let key = entry.logical_key().as_str();
            match record_indexes.get(key).copied() {
                Some(index) => records[index].parts.push(entry),
                None => {
                    record_indexes.insert(key, records.len());
                    records.push(StorageExportRecord {
                        logical_name: entry.logical_name().as_str(),
                        logical_key: key,
                        parts: vec![entry],
                    });
                }
            }
        }
        Self {
            physical_entries: image.len(),
            records,
        }
    }

    /// Returns the physical entry count before multipart grouping.
    #[must_use]
    pub const fn physical_entries(&self) -> usize {
        self.physical_entries
    }

    /// Returns logical records in stable source order.
    #[must_use]
    pub fn records(&self) -> &[StorageExportRecord<'a>] {
        &self.records
    }
}

/// Failure while materializing a logical storage record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageExportPlanError {
    /// The aggregate packed payload length does not fit into `usize`.
    PackedSizeOverflow { logical_key: String },
}

impl Display for StorageExportPlanError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::PackedSizeOverflow { logical_key } => write!(
                formatter,
                "packed payload size overflow for storage record `{logical_key}`"
            ),
        }
    }
}

impl Error for StorageExportPlanError {}

/// Outcome of applying known family decoders to one logical record.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageExportDisposition {
    Supported,
    Opaque,
    Failed,
}

/// Stable per-record export result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StorageExportEntryReport {
    pub logical_name: String,
    pub logical_key: String,
    pub part_count: usize,
    pub packed_bytes: usize,
    pub disposition: StorageExportDisposition,
    pub outputs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl StorageExportEntryReport {
    #[must_use]
    pub fn supported(
        record: &StorageExportRecord<'_>,
        packed_bytes: usize,
        outputs: Vec<String>,
    ) -> Self {
        Self::new(
            record,
            packed_bytes,
            StorageExportDisposition::Supported,
            outputs,
            None,
        )
    }

    #[must_use]
    pub fn opaque(
        record: &StorageExportRecord<'_>,
        packed_bytes: usize,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            record,
            packed_bytes,
            StorageExportDisposition::Opaque,
            Vec::new(),
            Some(message.into()),
        )
    }

    #[must_use]
    pub fn failed(
        record: &StorageExportRecord<'_>,
        packed_bytes: usize,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            record,
            packed_bytes,
            StorageExportDisposition::Failed,
            Vec::new(),
            Some(message.into()),
        )
    }

    fn new(
        record: &StorageExportRecord<'_>,
        packed_bytes: usize,
        disposition: StorageExportDisposition,
        outputs: Vec<String>,
        message: Option<String>,
    ) -> Self {
        Self {
            logical_name: record.logical_name().to_owned(),
            logical_key: record.logical_key().to_owned(),
            part_count: record.part_count(),
            packed_bytes,
            disposition,
            outputs,
            message,
        }
    }
}

/// Aggregate provider-neutral storage export report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StorageExportReport {
    pub schema_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_profile: Option<String>,
    pub physical_entries: usize,
    pub logical_entries: usize,
    pub supported: usize,
    pub opaque: usize,
    pub failed: usize,
    pub entries: Vec<StorageExportEntryReport>,
}

impl StorageExportReport {
    #[must_use]
    pub fn new(
        image: &StorageImage,
        plan: &StorageExportPlan<'_>,
        entries: Vec<StorageExportEntryReport>,
    ) -> Self {
        let supported = entries
            .iter()
            .filter(|entry| entry.disposition == StorageExportDisposition::Supported)
            .count();
        let opaque = entries
            .iter()
            .filter(|entry| entry.disposition == StorageExportDisposition::Opaque)
            .count();
        let failed = entries
            .iter()
            .filter(|entry| entry.disposition == StorageExportDisposition::Failed)
            .count();
        Self {
            schema_version: 1,
            source_profile: image
                .source_profile()
                .map(|profile| profile.as_str().to_owned()),
            physical_entries: plan.physical_entries(),
            logical_entries: plan.records().len(),
            supported,
            opaque,
            failed,
            entries,
        }
    }
}

#[cfg(test)]
mod tests {
    use ibcmd_core::{
        artifact::StorageProfileId,
        storage::{
            CompressionKind, MultipartIdentity, OpaqueStorageMetadata, StorageEntry, StorageImage,
            StorageKey, StorageName, StorageOrigin, StoragePayloads, StorageProvenance,
        },
    };

    use super::*;

    fn entry(key: &str, part_index: u32, part_count: u32, bytes: &[u8]) -> StorageEntry {
        StorageEntry::new(
            StorageName::new(key).unwrap(),
            StorageKey::new(key).unwrap(),
            MultipartIdentity::new(part_index, part_count).unwrap(),
            OpaqueStorageMetadata::default(),
            StoragePayloads::new(bytes.to_vec(), bytes.to_vec()).unwrap(),
            CompressionKind::stored(),
            StorageOrigin::new(
                StorageProfileId::parse("storage:cf-export-test").unwrap(),
                StorageProvenance::new("clean-room unit fixture").unwrap(),
            ),
        )
        .unwrap()
    }

    #[test]
    fn plan_groups_interleaved_parts_in_first_source_order() {
        let image = StorageImage::new(vec![
            entry("a", 0, 2, b"ab"),
            entry("b", 0, 1, b"x"),
            entry("a", 1, 2, b"cd"),
        ])
        .unwrap();
        let plan = StorageExportPlan::from_image(&image);
        assert_eq!(plan.physical_entries(), 3);
        assert_eq!(plan.records().len(), 2);
        assert_eq!(plan.records()[0].logical_key(), "a");
        assert_eq!(
            plan.records()[0].packed_payload().unwrap().as_ref(),
            b"abcd"
        );
        assert_eq!(plan.records()[1].packed_payload().unwrap().as_ref(), b"x");
    }

    #[test]
    fn report_counts_every_disposition() {
        let image = StorageImage::new(vec![entry("a", 0, 1, b"a")]).unwrap();
        let plan = StorageExportPlan::from_image(&image);
        let record = &plan.records()[0];
        let report = StorageExportReport::new(
            &image,
            &plan,
            vec![StorageExportEntryReport::opaque(record, 1, "unknown")],
        );
        assert_eq!(
            report.source_profile.as_deref(),
            Some("storage:cf-export-test")
        );
        assert_eq!((report.supported, report.opaque, report.failed), (0, 1, 0));
    }
}
