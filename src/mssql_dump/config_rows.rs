use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::StorageProfileId;
use ibcmd_core::storage::{
    CompressionKind, MultipartIdentity, OpaqueStorageMetadata, StorageBuildError, StorageEntry,
    StorageImage, StorageKey, StorageName, StorageOrigin, StoragePayloads, StorageProvenance,
};

#[derive(Debug, Clone)]
pub(super) struct ConfigRow {
    pub(super) file_name: String,
    pub(super) part_no: i32,
    pub(super) data_size: i64,
    pub(super) binary_hex: String,
}

#[derive(Debug, Clone)]
pub(super) struct ConfigRowHeader {
    pub(super) file_name: String,
    pub(super) part_no: i32,
    pub(super) data_size: i64,
}

#[derive(Debug)]
pub(super) struct ConfigChunkRow {
    pub(super) file_name: String,
    pub(super) part_no: i32,
    pub(super) data_size: i64,
    pub(super) chunk_index: i32,
    pub(super) binary_hex: String,
}

#[derive(Debug)]
pub(super) struct BinaryConfigRow {
    pub(super) file_name: String,
    pub(super) part_no: i32,
    pub(super) data_size: i64,
    pub(super) binary: Vec<u8>,
}

/// Typed failure returned by the transitional MSSQL-row storage adapter.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug)]
pub(super) enum BinaryRowsToStorageImageError {
    NegativePart {
        key: String,
        part_no: i32,
    },
    TooManyParts {
        key: String,
        count: usize,
    },
    DuplicatePart {
        key: String,
        part_no: u32,
    },
    PartGap {
        key: String,
        expected: u32,
        actual: u32,
    },
    NegativeDataSize {
        key: String,
        data_size: i64,
    },
    InconsistentDataSize {
        key: String,
        expected: u64,
        actual: u64,
    },
    PayloadSizeOverflow {
        key: String,
    },
    PayloadSizeMismatch {
        key: String,
        declared: u64,
        actual: u64,
    },
    Storage(StorageBuildError),
}

impl Display for BinaryRowsToStorageImageError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegativePart { key, part_no } => {
                write!(
                    formatter,
                    "MSSQL storage row `{key}` has negative part {part_no}"
                )
            }
            Self::TooManyParts { key, count } => {
                write!(formatter, "MSSQL storage row `{key}` has {count} parts")
            }
            Self::DuplicatePart { key, part_no } => write!(
                formatter,
                "MSSQL storage row `{key}` contains duplicate part {part_no}"
            ),
            Self::PartGap {
                key,
                expected,
                actual,
            } => write!(
                formatter,
                "MSSQL storage row `{key}` part sequence expected {expected}, got {actual}"
            ),
            Self::NegativeDataSize { key, data_size } => write!(
                formatter,
                "MSSQL storage row `{key}` has negative DataSize {data_size}"
            ),
            Self::InconsistentDataSize {
                key,
                expected,
                actual,
            } => write!(
                formatter,
                "MSSQL storage row `{key}` declares both {expected} and {actual} payload bytes"
            ),
            Self::PayloadSizeOverflow { key } => {
                write!(formatter, "MSSQL storage row `{key}` payload size overflow")
            }
            Self::PayloadSizeMismatch {
                key,
                declared,
                actual,
            } => write!(
                formatter,
                "MSSQL storage row `{key}` declares {declared} payload bytes but contains {actual}"
            ),
            Self::Storage(error) => error.fmt(formatter),
        }
    }
}

impl Error for BinaryRowsToStorageImageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            _ => None,
        }
    }
}

impl From<StorageBuildError> for BinaryRowsToStorageImageError {
    fn from(error: StorageBuildError) -> Self {
        Self::Storage(error)
    }
}

/// Converts legacy MSSQL binary rows into a deterministic neutral image.
///
/// The legacy BCP projection has no storage attributes or raw element header,
/// so the adapter retains those as explicitly empty opaque values and does not
/// infer an inner codec. Rows are canonicalized by logical key and part number;
/// existing [`ConfigRow`] consumers remain on their unchanged path.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn storage_image_from_binary_rows(
    rows: &[BinaryConfigRow],
    source_profile: StorageProfileId,
    provenance: StorageProvenance,
) -> Result<StorageImage, BinaryRowsToStorageImageError> {
    let mut ordered = rows.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.file_name
            .cmp(&right.file_name)
            .then_with(|| left.part_no.cmp(&right.part_no))
    });

    let mut entries = Vec::with_capacity(ordered.len());
    let mut group_start = 0;
    while group_start < ordered.len() {
        let key = ordered[group_start].file_name.as_str();
        let mut group_end = group_start + 1;
        while group_end < ordered.len() && ordered[group_end].file_name == key {
            group_end += 1;
        }
        let group = &ordered[group_start..group_end];
        let part_count = u32::try_from(group.len()).map_err(|_| {
            BinaryRowsToStorageImageError::TooManyParts {
                key: key.to_owned(),
                count: group.len(),
            }
        })?;

        let declared_size = u64::try_from(group[0].data_size).map_err(|_| {
            BinaryRowsToStorageImageError::NegativeDataSize {
                key: key.to_owned(),
                data_size: group[0].data_size,
            }
        })?;
        let mut actual_size = 0_u64;
        let mut previous_part = None::<u32>;
        for (expected_index, row) in group.iter().enumerate() {
            let part_index = u32::try_from(row.part_no).map_err(|_| {
                BinaryRowsToStorageImageError::NegativePart {
                    key: key.to_owned(),
                    part_no: row.part_no,
                }
            })?;
            if previous_part == Some(part_index) {
                return Err(BinaryRowsToStorageImageError::DuplicatePart {
                    key: key.to_owned(),
                    part_no: part_index,
                });
            }
            let expected_index = u32::try_from(expected_index).map_err(|_| {
                BinaryRowsToStorageImageError::TooManyParts {
                    key: key.to_owned(),
                    count: group.len(),
                }
            })?;
            if part_index != expected_index {
                return Err(BinaryRowsToStorageImageError::PartGap {
                    key: key.to_owned(),
                    expected: expected_index,
                    actual: part_index,
                });
            }
            previous_part = Some(part_index);

            let row_size = u64::try_from(row.data_size).map_err(|_| {
                BinaryRowsToStorageImageError::NegativeDataSize {
                    key: key.to_owned(),
                    data_size: row.data_size,
                }
            })?;
            if row_size != declared_size {
                return Err(BinaryRowsToStorageImageError::InconsistentDataSize {
                    key: key.to_owned(),
                    expected: declared_size,
                    actual: row_size,
                });
            }
            let binary_len = u64::try_from(row.binary.len()).map_err(|_| {
                BinaryRowsToStorageImageError::PayloadSizeOverflow {
                    key: key.to_owned(),
                }
            })?;
            actual_size = actual_size.checked_add(binary_len).ok_or_else(|| {
                BinaryRowsToStorageImageError::PayloadSizeOverflow {
                    key: key.to_owned(),
                }
            })?;
        }
        if actual_size != declared_size {
            return Err(BinaryRowsToStorageImageError::PayloadSizeMismatch {
                key: key.to_owned(),
                declared: declared_size,
                actual: actual_size,
            });
        }

        for row in group {
            let part_index = u32::try_from(row.part_no).expect("part index validated above");
            let payload = row.binary.clone();
            entries.push(StorageEntry::new(
                StorageName::new(&row.file_name)?,
                StorageKey::new(&row.file_name)?,
                MultipartIdentity::new(part_index, part_count)?,
                OpaqueStorageMetadata::empty(),
                StoragePayloads::new(payload.clone(), payload)?,
                CompressionKind::stored(),
                StorageOrigin::new(source_profile.clone(), provenance.clone()),
            )?);
        }
        group_start = group_end;
    }

    StorageImage::new(entries).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(file_name: &str, part_no: i32, data_size: i64, binary: &[u8]) -> BinaryConfigRow {
        BinaryConfigRow {
            file_name: file_name.to_owned(),
            part_no,
            data_size,
            binary: binary.to_vec(),
        }
    }

    fn image(rows: &[BinaryConfigRow]) -> Result<StorageImage, BinaryRowsToStorageImageError> {
        storage_image_from_binary_rows(
            rows,
            StorageProfileId::parse("storage:mssql-test").unwrap(),
            StorageProvenance::new("legacy:mssql-config-row").unwrap(),
        )
    }

    #[test]
    fn shuffled_binary_rows_produce_the_same_key_part_ordered_image() {
        let rows = vec![
            row("beta", 0, 1, b"z"),
            row("alpha", 1, 2, b"b"),
            row("alpha", 0, 2, b"a"),
        ];
        let shuffled = vec![
            row("alpha", 0, 2, b"a"),
            row("beta", 0, 1, b"z"),
            row("alpha", 1, 2, b"b"),
        ];

        let first = image(&rows).unwrap();
        let second = image(&shuffled).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.sha256(), second.sha256());
        let identities = first
            .entries()
            .iter()
            .map(|entry| {
                (
                    entry.logical_key().as_str(),
                    entry.multipart().part_index(),
                    entry.multipart().part_count(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            identities,
            vec![("alpha", 0, 2), ("alpha", 1, 2), ("beta", 0, 1)]
        );
        assert!(first.entries().iter().all(|entry| {
            entry.opaque_metadata().attributes().is_empty()
                && entry.opaque_metadata().raw_header().is_empty()
                && entry.compression().is_stored()
        }));
    }

    #[test]
    fn binary_row_adapter_rejects_duplicate_gap_and_payload_mismatch() {
        let duplicate = vec![row("x", 0, 2, b"a"), row("x", 0, 2, b"b")];
        assert!(matches!(
            image(&duplicate),
            Err(BinaryRowsToStorageImageError::DuplicatePart { .. })
        ));

        let gap = vec![row("x", 0, 2, b"a"), row("x", 2, 2, b"b")];
        assert!(matches!(
            image(&gap),
            Err(BinaryRowsToStorageImageError::PartGap { .. })
        ));

        let mismatch = vec![row("x", 0, 2, b"a")];
        assert!(matches!(
            image(&mismatch),
            Err(BinaryRowsToStorageImageError::PayloadSizeMismatch { .. })
        ));

        let inconsistent = vec![row("x", 0, 2, b"a"), row("x", 1, 3, b"b")];
        assert!(matches!(
            image(&inconsistent),
            Err(BinaryRowsToStorageImageError::InconsistentDataSize { .. })
        ));
    }
}
