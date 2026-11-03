//! Read-only library diagnostics over a PROVIDED packed view, not SQL preflight.
//!
//! No RPC route or DB method calls this function. Caller-provided context hashes
//! and packed rows are provenance labels, never a verified snapshot/authority.
use super::{SourceAdmission, SourceLimits};
use crate::{
    file_import::Selection,
    import_plan::{self, witness},
    platform::PlatformSpec,
};
use anyhow::{Result, bail};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path, sync::Arc};

pub type Check = Arc<dyn Fn() -> Result<()> + Send + Sync>;
/// Operator/library data only; never deserialized from editor or MCP arguments.
pub struct ProvidedView {
    pub rows: BTreeMap<String, Vec<u8>>,
    pub snapshot_label: [u8; 32],
    pub schema_label: [u8; 32],
    pub profile_label: [u8; 32],
    pub epoch_label: u64,
    pub export_profile: String,
    pub export_schema: u32,
}
#[derive(Clone, Copy)]
pub struct Limits {
    pub source: SourceLimits,
    pub report_entries: usize,
    pub report_outputs: usize,
    pub report_text_bytes: usize,
    pub descriptor_bytes: u64,
    pub packed_bytes: u64,
    pub artifact_bytes: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            source: SourceLimits::default(),
            report_entries: 100_000,
            report_outputs: 200_000,
            report_text_bytes: 16 * 1024 * 1024,
            descriptor_bytes: 128 * 1024 * 1024,
            packed_bytes: 128 * 1024 * 1024,
            artifact_bytes: 256 * 1024 * 1024,
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    MissingReferences,
    IndependentOwnerCompletenessUnavailable,
    OutputMappingUnavailable,
}
#[derive(Debug)]
pub struct OutputLedger {
    pub pairs: usize,
    pub old_claiming_keys: usize,
    pub new_claiming_keys: usize,
    pub digest: [u8; 32],
}
#[derive(Debug)]
pub struct Diagnostic {
    pub refusal: Refusal,
    pub changed_files: usize,
    pub missing_references: usize,
    pub compiled_entries: Option<usize>,
    pub old_report_entries: usize,
    pub new_report_entries: Option<usize>,
    pub old_descriptors: usize,
    pub new_descriptors: Option<usize>,
    pub old_descriptor_digest: [u8; 32],
    pub new_descriptor_digest: Option<[u8; 32]>,
    pub binding_digest: [u8; 32],
    pub engine_observations: Vec<EngineObservation>,
    pub outputs: Option<OutputLedger>,
    pub database_accessed: bool,
    pub publication_allowed: bool,
}
/// Redacted passive evidence; no names, raw reports or authority are exposed.
#[derive(Debug)]
pub struct EngineObservation {
    pub canonical_map_digest: [u8; 32],
    pub ordered_view_digest: [u8; 32],
    pub candidates: usize,
    pub ambiguous_or_unbound: usize,
    pub audit_issues: usize,
    pub unsupported_dimensions: Vec<&'static str>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    Admission,
    Compilation,
    SourceChanged,
    Interrupted,
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Admission => "compiler diagnostic source admission refused",
            Self::Compilation => {
                "compiler diagnostic compilation or mandatory verification refused"
            }
            Self::SourceChanged => "compiler diagnostic source changed during preparation",
            Self::Interrupted => "compiler diagnostic guard interrupted preparation",
        })
    }
}
impl std::error::Error for Failure {}
fn safe_error(error: anyhow::Error, stage: Failure) -> anyhow::Error {
    if crate::cancel::is_cancelled(&error) {
        crate::cancel::Cancelled.into()
    } else {
        stage.into()
    }
}
/// Admit and compile diagnostics only. The provided view is not promoted to SQL
/// provenance. Mapping errors produce a refusal; a stable mapping still refuses owner
/// completeness. No diagnostic result can be submitted to a publication method.
pub fn inspect_provided_view(
    view: ProvidedView,
    root: &Path,
    selection: &Selection,
    platform: PlatformSpec,
    limits: Limits,
    check: Check,
) -> Result<Diagnostic> {
    // Retain cancellation's type without a path-bearing error chain at this
    // diagnostic log/serialization boundary.
    let check: Check = Arc::new(move || check().map_err(|e| safe_error(e, Failure::Interrupted)));
    check()?;
    let admitted = SourceAdmission::prepare(root, selection, Some(true), limits.source)
        .map_err(|e| safe_error(e, Failure::Admission))?;
    check()?;
    let controls = witness::Limits {
        entries: limits.report_entries,
        outputs: limits.report_outputs,
        text_bytes: limits.report_text_bytes,
        descriptor_bytes: limits.descriptor_bytes,
        packed_bytes: limits.packed_bytes,
        artifact_bytes: limits.artifact_bytes,
        tree: limits.source.inventory,
        traversal: limits.source.traversal,
    };
    let request = witness::Request {
        target: witness::TargetBinding {
            snapshot: view.snapshot_label,
            schema: view.schema_label,
            profile: view.profile_label,
            epoch: view.epoch_label,
        },
        report_profile: view.export_profile,
        report_schema: view.export_schema,
        limits: controls,
        check: check.clone(),
    };
    crate::stored_objects::exclusive(|| -> Result<Diagnostic> {
        let planned = import_plan::prepare_with_export_evidence(
            view.rows,
            admitted.source,
            selection,
            platform,
            request,
        )
        .map_err(|e| safe_error(e, Failure::Compilation))?;
        check()?;
        let evidence = &planned.evidence;
        let missing = planned.prepared.missing_references.len();
        let observations = evidence.engine_observations();
        let expected = evidence.binding();
        anyhow::ensure!(
            observations.len() == 1 + usize::from(expected.new_packed.is_some())
                && observations[0].canonical_map_digest == expected.old_packed
                && expected
                    .new_packed
                    .is_none_or(|digest| observations[1].canonical_map_digest == digest),
            "engine diagnostic observations do not bind the complete compiler views"
        );
        let (refusal, outputs) = if missing > 0 {
            (Refusal::MissingReferences, None)
        } else {
            let expected = evidence.binding().clone();
            match evidence.selected_outputs(&expected, &planned.prepared.files, controls, &check) {
                Err(error) if crate::cancel::is_cancelled(&error) => {
                    return Err(crate::cancel::Cancelled.into());
                }
                Err(error) if error.downcast_ref::<Failure>() == Some(&Failure::Interrupted) => {
                    return Err(error);
                }
                // No filtered subset or successful permission is returned when
                // a raw claim is ambiguous, unsupported or omitted.
                Err(_) => (Refusal::OutputMappingUnavailable, None),
                Ok(pairs) => {
                    // This is the real refusal boundary, not a discarded placeholder:
                    // a future proof-capable API must not silently enable this consumer.
                    match pairs.require_owner_completeness() {
                        Ok(()) => bail!("diagnostic consumer cannot accept publication authority"),
                        Err(error) if crate::cancel::is_cancelled(&error) => {
                            return Err(crate::cancel::Cancelled.into());
                        }
                        Err(_) => {}
                    }
                    let mut hash = Sha256::new();
                    hash.update(b"ibcmd-diagnostic-output-ledger-v1");
                    hash.update((pairs.pairs.len() as u64).to_le_bytes());
                    for pair in &pairs.pairs {
                        check()?;
                        feed(&mut hash, pair.path.as_bytes(), &check)?;
                        feed(&mut hash, pair.key.as_bytes(), &check)?;
                        for index in [
                            pair.old.entry,
                            pair.old.output,
                            pair.new.entry,
                            pair.new.output,
                        ] {
                            hash.update((index as u64).to_le_bytes());
                        }
                    }
                    let ledger = OutputLedger {
                        pairs: pairs.pairs.len(),
                        old_claiming_keys: pairs.old_claiming_keys,
                        new_claiming_keys: pairs.new_claiming_keys,
                        digest: hash.finalize().into(),
                    };
                    (
                        Refusal::IndependentOwnerCompletenessUnavailable,
                        Some(ledger),
                    )
                }
            }
        };
        let old_descriptors = evidence.old_descriptors();
        let new_descriptors = evidence.new_descriptors();
        let mut binding_hash = Sha256::new();
        binding_hash.update(b"ibcmd-diagnostic-binding-v1");
        let b = evidence.binding();
        for digest in [
            &b.target.snapshot,
            &b.target.schema,
            &b.target.profile,
            &b.source_census,
            &b.selection,
            &b.resolved_selection,
            &b.old_packed,
            &b.old_artifact,
        ] {
            binding_hash.update(digest);
        }
        binding_hash.update(b.target.epoch.to_le_bytes());
        feed(&mut binding_hash, b.platform.as_bytes(), &check)?;
        feed(&mut binding_hash, b.report_profile.as_bytes(), &check)?;
        binding_hash.update(b.report_schema.to_le_bytes());
        for digest in [b.new_packed, b.new_artifact] {
            binding_hash.update([u8::from(digest.is_some())]);
            if let Some(digest) = digest {
                binding_hash.update(digest);
            }
        }
        let result = Diagnostic {
            refusal,
            changed_files: planned.prepared.files.len(),
            missing_references: missing,
            compiled_entries: planned.prepared.rows.as_ref().map(BTreeMap::len),
            old_report_entries: evidence.old_report().entries.len(),
            new_report_entries: evidence.new_report().map(|r| r.entries.len()),
            old_descriptors: old_descriptors.len(),
            new_descriptors: new_descriptors.map(|values| values.len()),
            old_descriptor_digest: descriptor_digest(old_descriptors, &check)?,
            new_descriptor_digest: new_descriptors
                .map(|d| descriptor_digest(d, &check))
                .transpose()?,
            binding_digest: binding_hash.finalize().into(),
            engine_observations: evidence
                .engine_observations()
                .iter()
                .map(|o| EngineObservation {
                    canonical_map_digest: o.canonical_map_digest,
                    ordered_view_digest: o.ordered_view_digest,
                    candidates: o.candidates,
                    ambiguous_or_unbound: o.ambiguous_or_unbound,
                    audit_issues: o.audit_issues,
                    unsupported_dimensions: o.unsupported_dimensions.clone(),
                })
                .collect(),
            outputs,
            database_accessed: false,
            publication_allowed: false,
        };
        // The original user-source capability remains alive through the entire
        // diagnostic ledger and final fresh validation, not merely compilation.
        planned
            .prepared
            .source
            .require_unchanged()
            .map_err(|e| safe_error(e.into(), Failure::SourceChanged))?;
        check()?;
        Ok(result)
    })
}
fn descriptor_digest(values: &[witness::DescriptorCandidate], check: &Check) -> Result<[u8; 32]> {
    let mut hash = Sha256::new();
    hash.update(b"ibcmd-diagnostic-descriptors-v1");
    hash.update((values.len() as u64).to_le_bytes());
    for value in values {
        check()?;
        feed(&mut hash, value.path.as_bytes(), check)?;
        feed(&mut hash, value.bytes.as_slice(), check)?;
    }
    check()?;
    Ok(hash.finalize().into())
}
fn feed(hash: &mut Sha256, bytes: &[u8], check: &Check) -> Result<()> {
    hash.update((bytes.len() as u64).to_le_bytes());
    for chunk in bytes.chunks(64 * 1024) {
        check()?;
        hash.update(chunk);
    }
    check()?;
    Ok(())
}
