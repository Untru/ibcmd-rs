//! Passive adapter over provided assembled maps; never storage provenance.
use super::{Check, Limits};
use crate::{mssql_dump::import_evidence as engine, platform::PlatformSpec};
use anyhow::{Result, ensure};
use std::collections::BTreeMap;

/// Only counts and hashes cross the private observer boundary.
#[derive(Debug, Clone)]
pub(crate) struct Summary {
    pub canonical_map_digest: [u8; 32],
    pub ordered_view_digest: [u8; 32],
    pub candidates: usize,
    pub ambiguous_or_unbound: usize,
    pub audit_issues: usize,
    pub unsupported_dimensions: Vec<&'static str>,
}

fn partition(limits: Limits) -> Result<engine::Limits> {
    // Each view has TWO independent text representations. Four shares bound
    // old+new inflated+normalized together; unused capacity is never recycled.
    let result = engine::Limits {
        max_candidates: limits.entries / 2,
        max_label_bytes: limits.text_bytes / 2,
        max_packed_bytes: usize::try_from(limits.packed_bytes / 2)?,
        max_inflated_bytes: usize::try_from(limits.descriptor_bytes / 4)?,
        max_collection_items: limits.outputs / 2,
    };
    ensure!(
        result.max_candidates > 0
            && result.max_label_bytes > 0
            && result.max_packed_bytes > 0
            && result.max_inflated_bytes > 0
            && result.max_collection_items > 0,
        "engine diagnostic budget partition is empty"
    );
    Ok(result)
}

pub(super) fn observe(
    rows: &BTreeMap<String, Vec<u8>>,
    canonical_map_digest: [u8; 32],
    view_index: usize,
    schema: &[u8; 32],
    platform: PlatformSpec,
    limits: Limits,
    check: &Check,
) -> Result<Summary> {
    check()?;
    let view_label = match view_index {
        0 => "old-provided-view",
        1 => "compiled-unverified-view",
        _ => anyhow::bail!("engine diagnostic permits only old and compiled views"),
    };
    let budget = partition(limits)?;
    ensure!(
        rows.len() <= budget.max_candidates,
        "engine diagnostic candidate budget exceeded"
    );
    let mut candidates = Vec::with_capacity(rows.len());
    for (index, (key, packed)) in rows.iter().enumerate() {
        check()?;
        candidates.push(engine::PackedCandidate {
            occurrence_ordinal: u64::try_from(index)?,
            origin_label: "provided-effective-view",
            table_label: "unverified-assembled",
            stored_name: key,
            logical_key: key,
            packed,
        });
    }
    // An asserted schema label is not a measured SQL schema certificate.
    let schema_label = schema
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let platform_label = platform.display().to_string();
    let mut original_error = None;
    let mut callback = || match check() {
        Ok(()) => Ok(()),
        Err(error) => {
            original_error = Some(error);
            Err(engine::ObserveError::Cancelled)
        }
    };
    let observed = engine::observe_view(
        engine::ViewInput {
            view_label,
            schema_label: &schema_label,
            platform_label: &platform_label,
            candidates: &candidates,
        },
        budget,
        &mut callback,
    );
    // Restore the actual typed parent error before inspecting any result.
    if let Some(error) = original_error {
        return Err(error);
    }
    let observed = observed.map_err(|error| match error {
        engine::ObserveError::Cancelled => anyhow::Error::from(crate::cancel::Cancelled),
        engine::ObserveError::Limit => {
            anyhow::anyhow!("engine diagnostic observation budget exceeded")
        }
    })?;
    ensure!(
        observed.require_owner_completeness().is_err(),
        "passive diagnostic cannot accept engine authority"
    );
    let mut ambiguous_or_unbound = 0usize;
    let mut audit_issues = 0usize;
    for candidate in observed.candidates() {
        check()?;
        ambiguous_or_unbound += usize::from(candidate.ambiguous());
        audit_issues += usize::from(candidate.audit_issue().is_some());
    }
    check()?;
    Ok(Summary {
        canonical_map_digest,
        ordered_view_digest: *observed.ordered_view_sha256(),
        candidates: observed.candidates().len(),
        ambiguous_or_unbound,
        audit_issues,
        unsupported_dimensions: observed
            .unsupported_dimensions()
            .iter()
            .map(|dimension| match dimension {
                engine::UnsupportedDimension::RawProfileValidation => "raw_profile_validation",
                engine::UnsupportedDimension::FullParentBinding => "full_parent_binding",
                engine::UnsupportedDimension::BodyRoleBinding => "body_role_binding",
                engine::UnsupportedDimension::Versions => "versions",
                engine::UnsupportedDimension::StorageProjection => "storage_projection",
                engine::UnsupportedDimension::OwnerCompleteness => "owner_completeness",
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    fn platform() -> PlatformSpec {
        crate::platform::parse_flag("8.5.1").unwrap()
    }
    #[test]
    fn partitions_bound_both_views_and_both_text_representations() {
        let limits = Limits::default();
        let p = partition(limits).unwrap();
        assert!(p.max_candidates * 2 <= limits.entries);
        assert!(p.max_label_bytes * 2 <= limits.text_bytes);
        assert!(p.max_packed_bytes as u64 * 2 <= limits.packed_bytes);
        assert!(p.max_inflated_bytes as u64 * 4 <= limits.descriptor_bytes);
        assert!(p.max_collection_items * 2 <= limits.outputs);
        assert!(
            partition(Limits {
                descriptor_bytes: 3,
                ..limits
            })
            .is_err()
        );
    }
    #[test]
    fn ordered_observation_and_canonical_map_are_distinct_and_bound() {
        let rows = BTreeMap::from([("private-key".to_string(), vec![1, 2, 3])]);
        let check: Check = Arc::new(|| Ok(()));
        let old = observe(
            &rows,
            [7; 32],
            0,
            &[2; 32],
            platform(),
            Limits::default(),
            &check,
        )
        .unwrap();
        let new = observe(
            &rows,
            [7; 32],
            1,
            &[2; 32],
            platform(),
            Limits::default(),
            &check,
        )
        .unwrap();
        assert_eq!(old.canonical_map_digest, [7; 32]);
        assert_ne!(old.ordered_view_digest, old.canonical_map_digest);
        assert_ne!(old.ordered_view_digest, new.ordered_view_digest);
        assert_eq!(old.unsupported_dimensions.len(), 6);
        assert!(!format!("{old:?}").contains("private-key"));
        assert!(
            observe(
                &rows,
                [7; 32],
                2,
                &[2; 32],
                platform(),
                Limits::default(),
                &check
            )
            .is_err()
        );
    }
    #[test]
    fn typed_cancellation_from_engine_callback_is_preserved() {
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let check: Check = Arc::new(move || {
            if count.fetch_add(1, Ordering::SeqCst) >= 2 {
                return Err(crate::cancel::Cancelled.into());
            }
            Ok(())
        });
        let error = observe(
            &BTreeMap::from([("key".into(), vec![1])]),
            [0; 32],
            0,
            &[0; 32],
            platform(),
            Limits::default(),
            &check,
        )
        .unwrap_err();
        assert!(crate::cancel::is_cancelled(&error));
    }
    #[test]
    fn one_view_cannot_borrow_unused_budget_from_the_other() {
        let limits = Limits {
            entries: 2,
            ..Limits::default()
        };
        let check: Check = Arc::new(|| Ok(()));
        let rows = BTreeMap::from([("A".into(), vec![1]), ("B".into(), vec![2])]);
        assert!(observe(&rows, [0; 32], 0, &[0; 32], platform(), limits, &check).is_err());
        assert!(observe(&rows, [0; 32], 1, &[0; 32], platform(), limits, &check).is_err());
    }
    #[test]
    fn non_cancellation_callback_error_keeps_its_original_type() {
        #[derive(Debug)]
        struct Sentinel;
        impl std::fmt::Display for Sentinel {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("sentinel")
            }
        }
        impl std::error::Error for Sentinel {}
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let check: Check = Arc::new(move || {
            if count.fetch_add(1, Ordering::SeqCst) >= 2 {
                return Err(Sentinel.into());
            }
            Ok(())
        });
        let error = observe(
            &BTreeMap::from([("key".into(), vec![1])]),
            [0; 32],
            0,
            &[0; 32],
            platform(),
            Limits::default(),
            &check,
        )
        .unwrap_err();
        assert!(error.downcast_ref::<Sentinel>().is_some());
        assert!(!crate::cancel::is_cancelled(&error));
    }
    #[test]
    fn engine_inflate_limit_returns_no_partial_summary() {
        use std::io::Write;
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(&[b'x'; 64]).unwrap();
        let rows = BTreeMap::from([("key".into(), encoder.finish().unwrap())]);
        let check: Check = Arc::new(|| Ok(()));
        // Candidate count/labels/packed bytes fit. Only incremental engine
        // inflate exceeds the 16-byte per-representation per-view partition.
        let limits = Limits {
            descriptor_bytes: 64,
            ..Limits::default()
        };
        let error = observe(&rows, [0; 32], 0, &[0; 32], platform(), limits, &check).unwrap_err();
        assert_eq!(
            error.to_string(),
            "engine diagnostic observation budget exceeded"
        );
    }
}
