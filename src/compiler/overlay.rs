//! Ordered assembly of pure source outcomes into a neutral storage patch.

use ibcmd_core::storage::StoragePatch;

use super::{CompileAxes, CompileRequest, CompileResult, compile_source};

/// Compiles requests in caller-supplied order and validates the resulting patch.
///
/// Duplicate targets are rejected by [`StoragePatch::new`] and are propagated as
/// the original core build error through the compiler error boundary.
pub fn compile_overlay<'a, I>(axes: &CompileAxes, requests: I) -> CompileResult<StoragePatch>
where
    I: IntoIterator<Item = CompileRequest<'a>>,
{
    let entries = requests
        .into_iter()
        .map(|request| compile_source(axes, request))
        .collect::<CompileResult<Vec<_>>>()?;
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
            StorageProfileId::parse("storage:mssql-test").unwrap(),
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
}
