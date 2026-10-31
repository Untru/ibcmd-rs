//! Raw compiler evidence for an opt-in candidate, never mutation authority.
//! Supported is not completeness. Reduced canonical indexes cannot prove owner
//! ambiguity or full parent chains; no production owner proof is built here.
pub(crate) mod engine_observation;

use crate::{
    mssql_dump::StorageImageSourceExportReport,
    mssql_source_change::{
        HeldSourceRoot, SourceFileDigest, SourceInventoryLimits, SourceTraversalLimits,
    },
    platform::PlatformSpec,
};
use anyhow::{Result, bail, ensure};
use ibcmd_cf::export::{StorageExportDisposition, StorageExportReport};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub(crate) type Check = Arc<dyn Fn() -> Result<()> + Send + Sync>;
#[derive(Clone, Copy)]
pub(crate) struct Limits {
    pub entries: usize,
    pub outputs: usize,
    pub text_bytes: usize,
    pub descriptor_bytes: u64,
    pub packed_bytes: u64,
    pub artifact_bytes: u64,
    pub tree: SourceInventoryLimits,
    pub traversal: SourceTraversalLimits,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            entries: 100_000,
            outputs: 200_000,
            text_bytes: 16 * 1024 * 1024,
            descriptor_bytes: 128 * 1024 * 1024,
            packed_bytes: 128 * 1024 * 1024,
            artifact_bytes: 256 * 1024 * 1024,
            tree: SourceInventoryLimits {
                max_files: 20_000,
                max_file_bytes: 16 * 1024 * 1024,
                max_total_bytes: 128 * 1024 * 1024,
            },
            traversal: SourceTraversalLimits {
                max_entries: 40_000,
                max_depth: 64,
            },
        }
    }
}
/// Trusted adapter binding data, not read from import parameters. No write API.
#[derive(Clone, Eq, PartialEq)]
pub(crate) struct TargetBinding {
    pub snapshot: [u8; 32],
    pub schema: [u8; 32],
    pub profile: [u8; 32],
    pub epoch: u64,
}
pub(crate) struct Request {
    pub target: TargetBinding,
    pub report_profile: String,
    pub report_schema: u32,
    pub limits: Limits,
    pub check: Check,
}
#[derive(Clone, Eq, PartialEq)]
pub(crate) struct Binding {
    pub target: TargetBinding,
    pub source_census: [u8; 32],
    pub selection: [u8; 32],
    pub resolved_selection: [u8; 32],
    pub platform: String,
    pub report_profile: String,
    pub report_schema: u32,
    pub old_packed: [u8; 32],
    pub new_packed: Option<[u8; 32]>,
    pub old_artifact: [u8; 32],
    pub new_artifact: Option<[u8; 32]>,
}
pub(crate) struct DescriptorCandidate {
    pub path: String,
    pub bytes: Arc<Vec<u8>>,
}
pub(crate) struct Side {
    pub(super) rows_digest: [u8; 32],
    pub(super) packed_keys: BTreeSet<String>,
    pub(super) report: StorageExportReport,
    pub(super) descriptors: Vec<DescriptorCandidate>,
}
pub(crate) struct RawCompilerEvidence {
    pub(super) engine_observations: Vec<engine_observation::Summary>,
    pub(super) binding: Binding,
    pub(super) old: Side,
    pub(super) new: Option<Side>,
    pub(super) resolved_selected: BTreeSet<String>,
}
/// References preserve original entry and output occurrence, never last-writer wins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EntryRef {
    pub entry: usize,
    pub output: usize,
}
pub(crate) struct OutputClaims {
    pub by_path: BTreeMap<String, Vec<EntryRef>>,
    pub by_key: BTreeMap<String, BTreeSet<String>>,
}
pub(crate) struct SelectedOutputPair {
    pub path: String,
    pub key: String,
    pub old: EntryRef,
    pub new: EntryRef,
}
pub(crate) struct SelectedOutputEvidence {
    pub pairs: Vec<SelectedOutputPair>,
    pub old_claiming_keys: usize,
    pub new_claiming_keys: usize,
}
impl SelectedOutputEvidence {
    /// There is deliberately no owner/completeness constructor in this slice.
    /// Even a stable exact path/key pair cannot become an authorized SQL plan.
    pub fn require_owner_completeness(&self) -> Result<()> {
        bail!("independent complete owner, parent and selected-role evidence is unavailable")
    }
}

pub(crate) struct Capture {
    observations: Vec<engine_observation::Summary>,
    request: Request,
    entries: usize,
    outputs: usize,
    text: usize,
    descriptors: u64,
    packed: u64,
}
impl Capture {
    pub fn new(request: Request) -> Result<Self> {
        (request.check)()?;
        ensure!(
            !request.report_profile.is_empty() && request.report_schema > 0,
            "compiler evidence profile is missing"
        );
        let l = request.limits;
        ensure!(
            request.report_profile.len() <= l.text_bytes,
            "compiler evidence profile text limit exceeded"
        );
        ensure!(
            l.entries > 0
                && l.outputs > 0
                && l.text_bytes > 0
                && l.descriptor_bytes > 0
                && l.packed_bytes > 0
                && l.artifact_bytes > 0
                && l.tree.max_files > 0
                && l.tree.max_file_bytes > 0
                && l.tree.max_total_bytes > 0
                && l.traversal.max_entries > 0
                && l.traversal.max_depth > 0,
            "compiler evidence limits are invalid"
        );
        Ok(Self {
            observations: Vec::new(),
            request,
            entries: 0,
            outputs: 0,
            text: 0,
            descriptors: 0,
            packed: 0,
        })
    }
    /// Reads only planner-owned scratch artifacts; not a path authorization API.
    pub fn artifact(&self, path: &std::path::Path) -> Result<[u8; 32]> {
        use std::io::Read;
        self.check()?;
        let mut file = std::fs::File::open(path)?;
        let size = file.metadata()?.len();
        ensure!(
            size <= self.request.limits.artifact_bytes,
            "compiler artifact byte limit exceeded"
        );
        let mut hash = Sha256::new();
        let mut total = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            self.check()?;
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            total = total
                .checked_add(n as u64)
                .ok_or_else(|| anyhow::anyhow!("compiler artifact size overflow"))?;
            ensure!(
                total <= self.request.limits.artifact_bytes,
                "compiler artifact byte limit exceeded"
            );
            hash.update(&buffer[..n]);
        }
        ensure!(total == size, "compiler artifact changed while read");
        self.check()?;
        Ok(hash.finalize().into())
    }
    pub fn check(&self) -> Result<()> {
        (self.request.check)()
    }
    fn text(&mut self, value: &str) -> Result<()> {
        self.check()?;
        self.text = self
            .text
            .checked_add(value.len())
            .ok_or_else(|| anyhow::anyhow!("compiler evidence text overflow"))?;
        ensure!(
            self.text <= self.request.limits.text_bytes,
            "compiler evidence text limit exceeded"
        );
        Ok(())
    }
    pub fn packed(
        &mut self,
        rows: &BTreeMap<String, Vec<u8>>,
        platform: PlatformSpec,
    ) -> Result<([u8; 32], BTreeSet<String>)> {
        ensure!(
            rows.len() <= self.request.limits.entries,
            "compiler evidence packed key limit exceeded"
        );
        let mut hash = Sha256::new();
        hash.update(b"ibcmd-import-packed-evidence-v1");
        hash.update((rows.len() as u64).to_le_bytes());
        let mut keys = BTreeSet::new();
        for (key, bytes) in rows {
            self.text(key)?;
            self.packed = self
                .packed
                .checked_add(bytes.len() as u64)
                .ok_or_else(|| anyhow::anyhow!("compiler packed size overflow"))?;
            ensure!(
                self.packed <= self.request.limits.packed_bytes,
                "compiler evidence packed byte limit exceeded"
            );
            feed(&mut hash, key.as_bytes(), &self.request.check)?;
            feed(&mut hash, bytes, &self.request.check)?;
            keys.insert(key.clone());
        }
        let digest = hash.finalize().into();
        let observation = engine_observation::observe(
            rows,
            digest,
            self.observations.len(),
            &self.request.target.schema,
            platform,
            self.request.limits,
            &self.request.check,
        )?;
        self.observations.push(observation);
        Ok((digest, keys))
    }
    pub fn take_observations(&mut self) -> Vec<engine_observation::Summary> {
        std::mem::take(&mut self.observations)
    }
    pub fn binding(
        &mut self,
        source: &HeldSourceRoot,
        selection: &super::Selection,
        platform: PlatformSpec,
        old: [u8; 32],
        old_artifact: [u8; 32],
    ) -> Result<Binding> {
        ensure!(
            source.baseline().files().len() <= self.request.limits.tree.max_files,
            "compiler source census file limit exceeded"
        );
        ensure!(
            selection
                .files
                .len()
                .checked_add(selection.objects.len())
                .is_some_and(|n| n <= self.request.limits.outputs),
            "compiler selection count limit exceeded"
        );
        let mut source_bytes = 0u64;
        let mut hash = Sha256::new();
        hash.update(b"ibcmd-import-source-evidence-v1");
        hash.update((source.baseline().files().len() as u64).to_le_bytes());
        for file in source.baseline().files() {
            self.check()?;
            source_bytes = source_bytes
                .checked_add(file.size_bytes())
                .ok_or_else(|| anyhow::anyhow!("compiler source census size overflow"))?;
            ensure!(
                file.size_bytes() <= self.request.limits.tree.max_file_bytes
                    && source_bytes <= self.request.limits.tree.max_total_bytes,
                "compiler source census byte limit exceeded"
            );
            self.text(file.path())?;
            feed(&mut hash, file.path().as_bytes(), &self.request.check)?;
            hash.update(file.size_bytes().to_le_bytes());
            hash.update(file.sha256());
        }
        let source_census = hash.finalize().into();
        let mut hash = Sha256::new();
        hash.update(b"ibcmd-import-selection-evidence-v1");
        hash.update([u8::from(selection.changed)]);
        for (tag, values) in [
            (b"files".as_slice(), &selection.files),
            (b"objects".as_slice(), &selection.objects),
        ] {
            feed(&mut hash, tag, &self.request.check)?;
            hash.update((values.len() as u64).to_le_bytes());
            for value in values {
                self.text(value)?;
                feed(&mut hash, value.as_bytes(), &self.request.check)?;
            }
        }
        self.check()?;
        Ok(Binding {
            target: self.request.target.clone(),
            source_census,
            selection: hash.finalize().into(),
            resolved_selection: [0; 32],
            platform: platform.display().to_string(),
            report_profile: self.request.report_profile.clone(),
            report_schema: self.request.report_schema,
            old_packed: old,
            new_packed: None,
            old_artifact,
            new_artifact: None,
        })
    }
    pub fn resolved_selection(&mut self, paths: &[String]) -> Result<[u8; 32]> {
        ensure!(
            paths.len() <= self.request.limits.outputs,
            "resolved compiler selection count limit exceeded"
        );
        let mut hash = Sha256::new();
        hash.update(b"ibcmd-import-resolved-selection-v1");
        hash.update((paths.len() as u64).to_le_bytes());
        let mut seen = BTreeSet::new();
        for path in paths {
            self.text(path)?;
            let exact = canonical(path)?;
            ensure!(
                &exact == path && seen.insert(exact.to_lowercase()),
                "resolved compiler selection is ambiguous"
            );
            feed(&mut hash, path.as_bytes(), &self.request.check)?;
        }
        self.check()?;
        Ok(hash.finalize().into())
    }
    /// Capture descriptors before any selected edit (old) or from the verified
    /// compiled export (new). Preserve raw candidates, not reduced ConfigIndex.
    pub fn side(
        &mut self,
        root: &std::path::Path,
        raw: StorageImageSourceExportReport,
        packed: ([u8; 32], BTreeSet<String>),
        platform: PlatformSpec,
    ) -> Result<Side> {
        self.check()?;
        ensure!(
            raw.source_version == platform.xml_version().as_str(),
            "compiler evidence XML profile changed"
        );
        ensure!(
            raw.storage.schema_version == self.request.report_schema
                && raw.storage.source_profile.as_deref()
                    == Some(self.request.report_profile.as_str()),
            "compiler evidence storage report profile is unsupported"
        );
        self.entries = self
            .entries
            .checked_add(raw.storage.entries.len())
            .ok_or_else(|| anyhow::anyhow!("compiler evidence entry overflow"))?;
        ensure!(
            self.entries <= self.request.limits.entries,
            "compiler evidence entry limit exceeded"
        );
        let mut reported = BTreeSet::new();
        for entry in &raw.storage.entries {
            self.text(&entry.logical_key)?;
            self.text(&entry.logical_name)?;
            if let Some(note) = &entry.message {
                self.text(note)?;
            }
            ensure!(
                !entry.logical_key.is_empty() && reported.insert(entry.logical_key.clone()),
                "compiler report has a missing or duplicate packed key"
            );
            self.outputs = self
                .outputs
                .checked_add(entry.outputs.len())
                .ok_or_else(|| anyhow::anyhow!("compiler evidence output overflow"))?;
            ensure!(
                self.outputs <= self.request.limits.outputs,
                "compiler evidence output limit exceeded"
            );
            for path in &entry.outputs {
                self.text(path)?;
            }
        }
        ensure!(
            reported == packed.1,
            "compiler report omitted or added a packed key"
        );
        let held = HeldSourceRoot::open_with_traversal_limits(
            root,
            self.request.limits.tree,
            self.request.limits.traversal,
        )?;
        let mut descriptors = Vec::new();
        for file in held.baseline().files() {
            self.check()?;
            if !crate::metadata_model::audit::is_descriptor_xml(file.path()) {
                continue;
            }
            self.descriptors = self
                .descriptors
                .checked_add(file.size_bytes())
                .ok_or_else(|| anyhow::anyhow!("compiler descriptor size overflow"))?;
            ensure!(
                self.descriptors <= self.request.limits.descriptor_bytes,
                "compiler descriptor evidence limit exceeded"
            );
            self.text(file.path())?;
            descriptors.push(DescriptorCandidate {
                path: file.path().to_string(),
                bytes: held.source_bytes(file.path())?,
            });
        }
        held.require_unchanged()?;
        self.check()?;
        Ok(Side {
            rows_digest: packed.0,
            packed_keys: packed.1,
            report: raw.storage,
            descriptors,
        })
    }
}
fn feed(hash: &mut Sha256, value: &[u8], check: &Check) -> Result<()> {
    hash.update((value.len() as u64).to_le_bytes());
    for chunk in value.chunks(64 * 1024) {
        check()?;
        hash.update(chunk);
    }
    check()?;
    Ok(())
}
fn canonical(path: &str) -> Result<String> {
    let normalized = SourceFileDigest::new(path, 0, [0; 32])
        .map_err(|_| anyhow::anyhow!("compiler report output path is unsafe"))?;
    Ok(normalized.path().to_string())
}
/// Evidence only. Raw duplicates remain in Side even if indexing refuses.
pub(crate) fn output_claims(side: &Side, limits: Limits, check: &Check) -> Result<OutputClaims> {
    check()?;
    ensure!(
        side.report.entries.len() <= limits.entries,
        "compiler output entry limit exceeded"
    );
    let mut by_path = BTreeMap::<String, Vec<EntryRef>>::new();
    let mut by_key = BTreeMap::new();
    let mut folded = BTreeMap::new();
    let mut edges = 0usize;
    let mut text = 0usize;
    for (entry_index, entry) in side.report.entries.iter().enumerate() {
        check()?;
        text = text
            .checked_add(entry.logical_key.len())
            .ok_or_else(|| anyhow::anyhow!("compiler output text overflow"))?;
        ensure!(
            text <= limits.text_bytes,
            "compiler output text limit exceeded"
        );
        ensure!(
            !entry.logical_key.is_empty() && !by_key.contains_key(&entry.logical_key),
            "duplicate compiler output entry"
        );
        let mut paths = BTreeSet::new();
        for (output_index, path) in entry.outputs.iter().enumerate() {
            check()?;
            edges = edges
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("compiler output count overflow"))?;
            text = text
                .checked_add(path.len())
                .ok_or_else(|| anyhow::anyhow!("compiler output text overflow"))?;
            ensure!(
                edges <= limits.outputs && text <= limits.text_bytes,
                "compiler output witness budget exceeded"
            );
            let path = canonical(path)?;
            let fold = path.to_lowercase();
            match folded.entry(fold) {
                std::collections::btree_map::Entry::Vacant(slot) => {
                    slot.insert(path.clone());
                }
                std::collections::btree_map::Entry::Occupied(slot) => {
                    ensure!(
                        slot.get() == &path,
                        "compiler output case alias is ambiguous"
                    );
                }
            }
            paths.insert(path.clone());
            by_path.entry(path).or_default().push(EntryRef {
                entry: entry_index,
                output: output_index,
            });
        }
        by_key.insert(entry.logical_key.clone(), paths);
    }
    for claims in by_path.values() {
        check()?;
        ensure!(
            claims.len() == 1,
            "compiler output path has duplicate claims"
        );
    }
    Ok(OutputClaims { by_path, by_key })
}
impl RawCompilerEvidence {
    pub fn engine_observations(&self) -> &[engine_observation::Summary] {
        &self.engine_observations
    }
    pub fn binding(&self) -> &Binding {
        &self.binding
    }
    pub fn old_report(&self) -> &StorageExportReport {
        &self.old.report
    }
    pub fn new_report(&self) -> Option<&StorageExportReport> {
        self.new.as_ref().map(|s| &s.report)
    }
    pub fn old_descriptors(&self) -> &[DescriptorCandidate] {
        &self.old.descriptors
    }
    pub fn new_descriptors(&self) -> Option<&[DescriptorCandidate]> {
        self.new.as_ref().map(|s| s.descriptors.as_slice())
    }
    /// Stable output claims are necessary but insufficient: this output has no
    /// owner, role, UUID, kind, parent-chain or completeness authority.
    pub fn selected_outputs(
        &self,
        expected: &Binding,
        selected: &[String],
        limits: Limits,
        check: &Check,
    ) -> Result<SelectedOutputEvidence> {
        check()?;
        ensure!(
            &self.binding == expected,
            "compiler evidence target, source, selection or profile binding changed"
        );
        ensure!(!selected.is_empty(), "compiler evidence selection is empty");
        ensure!(
            selected.len() <= limits.outputs,
            "compiler selected output limit exceeded"
        );
        let new = self
            .new
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("compiled export evidence is missing"))?;
        ensure!(
            self.binding.old_packed == self.old.rows_digest
                && self.binding.new_packed == Some(new.rows_digest),
            "compiler packed evidence binding changed"
        );
        ensure!(
            self.old.packed_keys == new.packed_keys,
            "compiler changed existing packed key coverage"
        );
        let old_claims = output_claims(&self.old, limits, check)?;
        let new_claims = output_claims(new, limits, check)?;
        let mut seen = BTreeSet::new();
        let mut pairs = Vec::new();
        let mut selected_bytes = 0usize;
        for path in selected {
            check()?;
            selected_bytes = selected_bytes
                .checked_add(path.len())
                .ok_or_else(|| anyhow::anyhow!("compiler selected output size overflow"))?;
            ensure!(
                selected_bytes <= limits.text_bytes,
                "compiler selected output text limit exceeded"
            );
            let path = canonical(path)?;
            ensure!(
                self.resolved_selected.contains(&path),
                "queried compiler output was not selected by the prepared plan"
            );
            ensure!(
                seen.insert(path.to_lowercase()),
                "compiler selection has duplicate aliases"
            );
            let old = *old_claims
                .by_path
                .get(&path)
                .and_then(|v| v.first())
                .ok_or_else(|| anyhow::anyhow!("selected old output evidence is missing"))?;
            let after = *new_claims
                .by_path
                .get(&path)
                .and_then(|v| v.first())
                .ok_or_else(|| anyhow::anyhow!("selected compiled output evidence is missing"))?;
            let old_entry = &self.old.report.entries[old.entry];
            let new_entry = &new.report.entries[after.entry];
            ensure!(
                old_entry.disposition == StorageExportDisposition::Supported
                    && new_entry.disposition == StorageExportDisposition::Supported,
                "selected compiler output is opaque or failed"
            );
            ensure!(
                old_entry.logical_key == new_entry.logical_key
                    && old_entry.logical_name == new_entry.logical_name,
                "selected compiler output key or logical name changed"
            );
            pairs.push(SelectedOutputPair {
                path,
                key: old_entry.logical_key.clone(),
                old,
                new: after,
            });
        }
        check()?;
        Ok(SelectedOutputEvidence {
            pairs,
            old_claiming_keys: old_claims
                .by_key
                .values()
                .filter(|paths| !paths.is_empty())
                .count(),
            new_claiming_keys: new_claims
                .by_key
                .values()
                .filter(|paths| !paths.is_empty())
                .count(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ibcmd_cf::export::StorageExportEntryReport;
    use std::sync::atomic::{AtomicUsize, Ordering};
    fn check() -> Check {
        Arc::new(|| Ok(()))
    }
    fn entry(key: &str, outputs: &[&str]) -> StorageExportEntryReport {
        StorageExportEntryReport {
            logical_name: key.into(),
            logical_key: key.into(),
            part_count: 1,
            packed_bytes: 3,
            disposition: StorageExportDisposition::Supported,
            outputs: outputs.iter().map(|s| s.to_string()).collect(),
            message: None,
        }
    }
    fn side(entries: Vec<StorageExportEntryReport>) -> Side {
        Side {
            rows_digest: [1; 32],
            packed_keys: entries.iter().map(|e| e.logical_key.clone()).collect(),
            report: StorageExportReport {
                schema_version: 1,
                source_profile: Some("synthetic-profile".into()),
                physical_entries: entries.len(),
                logical_entries: entries.len(),
                supported: entries.len(),
                opaque: 0,
                failed: 0,
                entries,
            },
            descriptors: Vec::new(),
        }
    }
    fn evidence(old: Side, new: Side) -> RawCompilerEvidence {
        let resolved_selected = old
            .report
            .entries
            .iter()
            .flat_map(|e| &e.outputs)
            .map(|p| canonical(p).unwrap())
            .collect();
        RawCompilerEvidence {
            engine_observations: Vec::new(),
            binding: Binding {
                target: TargetBinding {
                    snapshot: [2; 32],
                    schema: [3; 32],
                    profile: [4; 32],
                    epoch: 1,
                },
                source_census: [5; 32],
                selection: [6; 32],
                resolved_selection: [10; 32],
                platform: "synthetic-platform".into(),
                report_profile: "synthetic-profile".into(),
                report_schema: 1,
                old_packed: old.rows_digest,
                new_packed: Some(new.rows_digest),
                old_artifact: [7; 32],
                new_artifact: Some([8; 32]),
            },
            old,
            new: Some(new),
            resolved_selected,
        }
    }
    fn request() -> Request {
        Request {
            target: TargetBinding {
                snapshot: [2; 32],
                schema: [3; 32],
                profile: [4; 32],
                epoch: 1,
            },
            report_profile: "synthetic-profile".into(),
            report_schema: 1,
            limits: Limits::default(),
            check: check(),
        }
    }
    #[test]
    fn one_key_can_claim_multiple_distinct_outputs_without_gaining_owner_authority() {
        let raw = side(vec![entry(
            "body",
            &["Objects/A/Ext/Form.xml", "Objects/A/Ext/Form/Module.bsl"],
        )]);
        let index = output_claims(&raw, Limits::default(), &check()).unwrap();
        assert_eq!(index.by_path.len(), 2);
        assert_eq!(index.by_key["body"].len(), 2);
        let e = evidence(
            raw,
            side(vec![entry(
                "body",
                &["Objects/A/Ext/Form.xml", "Objects/A/Ext/Form/Module.bsl"],
            )]),
        );
        let pairs = e
            .selected_outputs(
                e.binding(),
                &["Objects/A/Ext/Form/Module.bsl".into()],
                Limits::default(),
                &check(),
            )
            .unwrap();
        assert_eq!(pairs.pairs.len(), 1);
        assert_eq!(pairs.pairs[0].key, "body");
        assert!(pairs.require_owner_completeness().is_err());
    }
    #[test]
    fn duplicate_claims_never_overwrite_raw_occurrences() {
        for entries in [
            vec![
                entry("A", &["Objects/X.xml"]),
                entry("B", &["Objects/X.xml"]),
            ],
            vec![entry("A", &["Objects/X.xml", "Objects/X.xml"])],
            vec![
                entry("A", &["Objects/X.xml"]),
                entry("A", &["Objects/Y.xml"]),
            ],
        ] {
            let raw = side(entries.clone());
            assert!(output_claims(&raw, Limits::default(), &check()).is_err());
            assert_eq!(raw.report.entries, entries);
        }
    }
    #[test]
    fn canonical_slash_and_unicode_case_collisions_refuse() {
        for paths in [
            ["Objects/X.xml", "Objects\\X.xml"],
            ["Objects/X.xml", "objects/x.xml"],
            ["Objects/Тест.xml", "Objects/ТЕСТ.xml"],
        ] {
            assert!(
                output_claims(&side(vec![entry("A", &paths)]), Limits::default(), &check())
                    .is_err()
            );
        }
        let index = output_claims(
            &side(vec![entry("A", &["Objects\\X.xml"])]),
            Limits::default(),
            &check(),
        )
        .unwrap();
        assert!(index.by_path.contains_key("Objects/X.xml"));
    }
    #[test]
    fn absolute_traversal_device_and_escaped_outputs_refuse() {
        for path in [
            "/outside.xml",
            "C:\\outside.xml",
            "../outside.xml",
            "Objects/../X.xml",
            "Objects/./X.xml",
            "Objects//X.xml",
            "Objects/NUL.xml",
            "Objects/x.xml ",
            "Objects/a:stream.xml",
        ] {
            assert!(
                output_claims(
                    &side(vec![entry("A", &[path])]),
                    Limits::default(),
                    &check()
                )
                .is_err()
            );
        }
    }
    #[test]
    fn supported_module_with_omitted_form_is_not_complete() {
        let mut raw = side(vec![entry("A", &["Objects/X/Ext/Form/Module.bsl"])]);
        raw.report.entries[0].message = Some("synthetic missing form output".into());
        let e = evidence(
            raw,
            side(vec![entry("A", &["Objects/X/Ext/Form/Module.bsl"])]),
        );
        assert!(
            e.selected_outputs(
                e.binding(),
                &["Objects/X/Ext/Form.xml".into()],
                Limits::default(),
                &check()
            )
            .is_err()
        );
        let partial = e
            .selected_outputs(
                e.binding(),
                &["Objects/X/Ext/Form/Module.bsl".into()],
                Limits::default(),
                &check(),
            )
            .unwrap();
        assert!(partial.require_owner_completeness().is_err());
        assert_eq!(
            e.old_report().entries[0].message.as_deref(),
            Some("synthetic missing form output")
        );
    }
    #[test]
    fn selected_opaque_failed_missing_or_rebound_output_refuses() {
        for disposition in [
            StorageExportDisposition::Opaque,
            StorageExportDisposition::Failed,
        ] {
            let mut new = side(vec![entry("A", &["Objects/X.xml"])]);
            new.report.entries[0].disposition = disposition;
            let e = evidence(side(vec![entry("A", &["Objects/X.xml"])]), new);
            assert!(
                e.selected_outputs(
                    e.binding(),
                    &["Objects/X.xml".into()],
                    Limits::default(),
                    &check()
                )
                .is_err()
            );
        }
        let e = evidence(
            side(vec![entry("A", &["Objects/X.xml"])]),
            side(vec![entry("A", &["Objects/Y.xml"])]),
        );
        assert!(
            e.selected_outputs(
                e.binding(),
                &["Objects/X.xml".into()],
                Limits::default(),
                &check()
            )
            .is_err()
        );
        let e = evidence(
            side(vec![entry("A", &["Objects/X.xml"])]),
            side(vec![entry("B", &["Objects/X.xml"])]),
        );
        assert!(
            e.selected_outputs(
                e.binding(),
                &["Objects/X.xml".into()],
                Limits::default(),
                &check()
            )
            .is_err()
        );
    }
    #[test]
    fn stale_snapshot_schema_profile_source_selection_epoch_and_packed_binding_refuse() {
        let e = evidence(
            side(vec![entry("A", &["Objects/X.xml"])]),
            side(vec![entry("A", &["Objects/X.xml"])]),
        );
        for n in 0..12 {
            let mut expected = e.binding().clone();
            match n {
                0 => expected.target.snapshot[0] ^= 1,
                1 => expected.target.schema[0] ^= 1,
                2 => expected.target.profile[0] ^= 1,
                3 => expected.target.epoch += 1,
                4 => expected.source_census[0] ^= 1,
                5 => expected.selection[0] ^= 1,
                6 => expected.old_packed[0] ^= 1,
                7 => expected.platform.push('x'),
                8 => expected.old_artifact[0] ^= 1,
                9 => expected.new_artifact = Some([9; 32]),
                10 => expected.report_profile.push('x'),
                _ => expected.report_schema += 1,
            }
            assert!(
                e.selected_outputs(
                    &expected,
                    &["Objects/X.xml".into()],
                    Limits::default(),
                    &check()
                )
                .is_err()
            );
        }
    }
    #[test]
    fn a_bound_evidence_cannot_return_selected_pairs_for_unchanged_b() {
        let mut e = evidence(
            side(vec![
                entry("A", &["Objects/A.xml"]),
                entry("B", &["Objects/B.xml"]),
            ]),
            side(vec![
                entry("A", &["Objects/A.xml"]),
                entry("B", &["Objects/B.xml"]),
            ]),
        );
        e.resolved_selected = BTreeSet::from(["Objects/A.xml".into()]);
        let error = e
            .selected_outputs(
                e.binding(),
                &["Objects/B.xml".into()],
                Limits::default(),
                &check(),
            )
            .err()
            .unwrap();
        assert_eq!(
            error.to_string(),
            "queried compiler output was not selected by the prepared plan"
        );
        assert!(
            e.selected_outputs(
                e.binding(),
                &["Objects/A.xml".into()],
                Limits::default(),
                &check()
            )
            .is_ok()
        );
    }
    #[test]
    fn stable_key_with_changed_raw_logical_name_is_refused() {
        let mut new = side(vec![entry("A", &["Objects/A.xml"])]);
        new.report.entries[0].logical_name = "renamed".into();
        let e = evidence(side(vec![entry("A", &["Objects/A.xml"])]), new);
        assert!(
            e.selected_outputs(
                e.binding(),
                &["Objects/A.xml".into()],
                Limits::default(),
                &check()
            )
            .is_err()
        );
    }
    #[test]
    fn witness_limits_and_typed_cancellation_cover_indexing_and_hashing() {
        let raw = side(vec![entry("A", &["Objects/X.xml", "Objects/Y.xml"])]);
        for l in [
            Limits {
                entries: 0,
                ..Limits::default()
            },
            Limits {
                outputs: 1,
                ..Limits::default()
            },
            Limits {
                text_bytes: 1,
                ..Limits::default()
            },
        ] {
            assert!(output_claims(&raw, l, &check()).is_err());
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        let cancel: Check = Arc::new(move || {
            if seen.fetch_add(1, Ordering::Relaxed) > 2 {
                Err(crate::cancel::Cancelled.into())
            } else {
                Ok(())
            }
        });
        assert!(crate::cancel::is_cancelled(
            &output_claims(&raw, Limits::default(), &cancel)
                .err()
                .unwrap()
        ));
        let mut r = request();
        r.limits.packed_bytes = 1;
        let mut capture = Capture::new(r).unwrap();
        assert!(
            capture
                .packed(
                    &BTreeMap::from([("A".into(), vec![1, 2])]),
                    crate::platform::parse_flag("8.5.1").unwrap()
                )
                .is_err()
        );
    }
    #[test]
    fn unknown_report_profile_and_missing_or_added_packed_keys_refuse_before_tree_io() {
        for mode in 0..3 {
            let mut capture = Capture::new(request()).unwrap();
            let mut raw = crate::mssql_dump::StorageImageSourceExportReport {
                output_dir: std::path::PathBuf::new(),
                source_version: "2.21".into(),
                files_written: 1,
                storage: side(vec![entry("A", &["Objects/X.xml"])]).report,
            };
            if mode == 0 {
                raw.storage.source_profile = Some("unknown".into());
            }
            let keys = if mode == 1 {
                BTreeSet::new()
            } else if mode == 2 {
                BTreeSet::from(["A".into(), "B".into()])
            } else {
                BTreeSet::from(["A".into()])
            };
            let error = capture
                .side(
                    std::path::Path::new("nonexistent-witness-tree"),
                    raw,
                    ([1; 32], keys),
                    crate::platform::parse_flag("8.5.1").unwrap(),
                )
                .err()
                .unwrap();
            assert_eq!(
                error.to_string(),
                if mode == 0 {
                    "compiler evidence storage report profile is unsupported"
                } else {
                    "compiler report omitted or added a packed key"
                }
            );
        }
    }
}
