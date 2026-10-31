//! Structural coverage invariant, not an owner resolver or write authority.
//!
//! Inputs are canonical BODY keys from independently justified engine witnesses.
//! CONFIG's historical projection must precede consulting CONFIGSAVE versions;
//! original staging coverage must precede incoming compiler inventory. Passing
//! raw physical keys or an already stage-pruned baseline is not a valid proof.
//! This helper has no I/O and is not yet wired to SQL preparation/publication.
use super::readset::Check;
use anyhow::{Result, ensure};
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
pub struct Limits {
    pub max_witness_keys: usize,
    pub max_key_bytes: usize,
}
/// Reject omission/addition before a stage inventory can prune its own evidence.
/// This proves set preservation only; actual engine provenance, exact selected
/// owner/file-role bindings, service/versions rules and payload checks are separate.
pub fn require_preserved(
    projected_before_stage: &BTreeSet<String>,
    original_stage: &BTreeSet<String>,
    effective: &BTreeSet<String>,
    compiled: &BTreeSet<String>,
    limits: Limits,
    check: &Check,
) -> Result<()> {
    check()?;
    ensure!(
        limits.max_witness_keys > 0 && limits.max_key_bytes > 0,
        "SQL coverage limits are invalid"
    );
    let mut count = 0usize;
    let mut bytes = 0usize;
    for set in [projected_before_stage, original_stage, effective, compiled] {
        for key in set {
            check()?;
            ensure!(!key.is_empty(), "SQL coverage key is empty");
            count = count
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("SQL coverage count overflow"))?;
            bytes = bytes
                .checked_add(key.len())
                .ok_or_else(|| anyhow::anyhow!("SQL coverage size overflow"))?;
            ensure!(
                count <= limits.max_witness_keys && bytes <= limits.max_key_bytes,
                "SQL coverage witness budget exceeded"
            );
        }
    }
    let mut expected = projected_before_stage.union(original_stage);
    for key in effective {
        check()?;
        ensure!(
            expected.next() == Some(key),
            "SQL effective view changed original body coverage"
        );
    }
    check()?;
    ensure!(
        expected.next().is_none(),
        "SQL effective view omitted original body coverage"
    );
    let mut expected = effective.iter();
    for key in compiled {
        check()?;
        ensure!(
            expected.next() == Some(key),
            "SQL compiler changed existing body coverage"
        );
    }
    check()?;
    ensure!(
        expected.next().is_none(),
        "SQL compiler omitted existing body coverage"
    );
    Ok(())
}
