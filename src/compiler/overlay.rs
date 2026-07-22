//! Ordered assembly of pure source outcomes into a neutral storage patch.

use ibcmd_core::storage::{
    MAX_STORAGE_PATCH_ENTRIES, MAX_STORAGE_PATCH_RETAINED_BYTES, StoragePatch,
    StoragePatchBuildError,
};

use super::{CompileAxes, CompileError, CompileRequest, CompileResult, compile_source};

/// Compiles requests in caller-supplied order and validates the resulting patch.
///
/// Duplicate targets are rejected by [`StoragePatch::new`] and are propagated as
/// the original core build error through the compiler error boundary.
pub fn compile_overlay<'a, I>(axes: &CompileAxes, requests: I) -> CompileResult<StoragePatch>
where
    I: IntoIterator<Item = CompileRequest<'a>>,
{
    compile_overlay_with_limits(
        axes,
        requests,
        MAX_STORAGE_PATCH_ENTRIES,
        MAX_STORAGE_PATCH_RETAINED_BYTES,
    )
}

fn compile_overlay_with_limits<'a, I>(
    axes: &CompileAxes,
    requests: I,
    maximum_entries: usize,
    maximum_retained_bytes: usize,
) -> CompileResult<StoragePatch>
where
    I: IntoIterator<Item = CompileRequest<'a>>,
{
    let mut entries = Vec::new();
    let mut retained_bytes = 0_usize;
    for request in requests {
        if entries.len() == maximum_entries {
            return Err(CompileError::Patch(
                StoragePatchBuildError::TooManyEntries {
                    maximum: maximum_entries,
                    actual: entries.len() + 1,
                },
            ));
        }

        let entry = compile_source(axes, request)?;
        let actual = retained_bytes
            .checked_add(entry.retained_byte_len()?)
            .ok_or(CompileError::Patch(
                StoragePatchBuildError::RetainedByteCountOverflow,
            ))?;
        if actual > maximum_retained_bytes {
            return Err(CompileError::Patch(
                StoragePatchBuildError::RetainedBytesExceeded {
                    maximum: maximum_retained_bytes,
                    actual,
                },
            ));
        }
        retained_bytes = actual;
        entries.push(entry);
    }
    StoragePatch::new(entries).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use ibcmd_core::artifact::StorageProfileId;
    use ibcmd_core::storage::{
        MultipartIdentity, StorageKey, StoragePatchBuildError, StoragePatchOutcome,
        StoragePatchTarget, StorageProvenance,
    };
    use ibcmd_core::version::XmlDialect;

    use super::*;
    use crate::compiler::{CompileError, SourcePayload};

    fn axes() -> CompileAxes {
        CompileAxes::new(
            XmlDialect::parse("2.20").unwrap(),
            None,
            None,
            StorageProfileId::parse("storage:mssql-config-configsave").unwrap(),
            None,
        )
    }

    fn request(key: &str, bytes: &'static [u8]) -> CompileRequest<'static> {
        CompileRequest::new(
            StoragePatchTarget::new(
                StorageKey::new(key).unwrap(),
                MultipartIdentity::single(),
                StorageProvenance::new(&format!("source/{key}")).unwrap(),
            ),
            SourcePayload::RawDeflated { bytes },
        )
    }

    #[test]
    fn overlay_preserves_request_order() {
        let patch =
            compile_overlay(&axes(), [request("second", b"2"), request("first", b"1")]).unwrap();

        let keys = patch
            .entries()
            .iter()
            .map(|entry| entry.target().key().as_str())
            .collect::<Vec<_>>();
        assert_eq!(keys, ["second", "first"]);
        assert!(
            patch
                .entries()
                .iter()
                .all(|entry| matches!(entry.outcome(), StoragePatchOutcome::Compiled(_)))
        );
    }

    #[test]
    fn overlay_propagates_core_duplicate_target_error() {
        let error = compile_overlay(
            &axes(),
            [request("duplicate", b"1"), request("duplicate", b"2")],
        )
        .unwrap_err();

        assert!(matches!(
            error,
            CompileError::Patch(StoragePatchBuildError::DuplicateTarget {
                key,
                part_index: 0
            }) if key == "duplicate"
        ));
    }

    #[test]
    fn overlay_rejects_count_before_compiling_the_next_request() {
        let malformed = CompileRequest::new(
            StoragePatchTarget::new(
                StorageKey::new("second").unwrap(),
                MultipartIdentity::single(),
                StorageProvenance::new("source/second").unwrap(),
            ),
            SourcePayload::MetadataXml { xml: b"not XML" },
        );
        let error = compile_overlay_with_limits(
            &axes(),
            [request("first", b"1"), malformed],
            1,
            usize::MAX,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            CompileError::Patch(StoragePatchBuildError::TooManyEntries {
                maximum: 1,
                actual: 2
            })
        ));
    }

    #[test]
    fn overlay_rejects_retained_bytes_incrementally() {
        let error =
            compile_overlay_with_limits(&axes(), [request("first", b"payload")], usize::MAX, 1)
                .unwrap_err();

        assert!(matches!(
            error,
            CompileError::Patch(StoragePatchBuildError::RetainedBytesExceeded {
                maximum: 1,
                actual
            }) if actual > 1
        ));
    }
}
