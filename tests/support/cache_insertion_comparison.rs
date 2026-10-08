//! Closed diagnostic comparison. No insertion generator, source authority or production mutation.
use super::*;
use ibcmd_rs::metadata_model::brace::serialize;
use ibcmd_rs::restructure::caches::order;
use std::io::Write;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum BraceValue {
    Atom(String),
    Quoted(String),
    /// Existing canonical Brace serialization; literal source whitespace is compared separately.
    List(String),
}
fn value(node: &Brace) -> BraceValue {
    match node {
        Brace::Atom(s) => BraceValue::Atom(s.clone()),
        Brace::Str(s) => BraceValue::Quoted(s.clone()),
        Brace::List(_) => BraceValue::List(serialize(node)),
    }
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ByteEndpoint {
    Byte(u8),
    Eof,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct LiteralDifference {
    pub offset: usize,
    pub expected: ByteEndpoint,
    pub actual: ByteEndpoint,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct LiteralComparison {
    pub expected_length: usize,
    pub expected_sha256: String,
    pub actual_length: usize,
    pub actual_sha256: String,
    pub expected_bom: bool,
    pub actual_bom: bool,
    pub first: Option<LiteralDifference>,
}
pub fn literal(expected: &[u8], actual: &[u8]) -> LiteralComparison {
    let endpoint = |bytes: &[u8], at| {
        bytes
            .get(at)
            .map_or(ByteEndpoint::Eof, |b| ByteEndpoint::Byte(*b))
    };
    LiteralComparison {
        expected_length: expected.len(),
        expected_sha256: hash(expected),
        actual_length: actual.len(),
        actual_sha256: hash(actual),
        expected_bom: expected.starts_with(b"\xef\xbb\xbf"),
        actual_bom: actual.starts_with(b"\xef\xbb\xbf"),
        first: first_raw_difference(expected, actual).map(|offset| LiteralDifference {
            offset,
            expected: endpoint(expected, offset),
            actual: endpoint(actual, offset),
        }),
    }
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct KeyDifference {
    pub ordinal: usize,
    pub expected: Option<String>,
    pub actual: Option<String>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PropertyDifference {
    pub key: String,
    pub ordinal: usize,
    pub expected: Option<(String, BraceValue)>,
    pub actual: Option<(String, BraceValue)>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct PayloadComparison {
    pub expected_keys: usize,
    pub actual_keys: usize,
    pub missing_keys: Vec<String>,
    pub extra_keys: Vec<String>,
    pub first_key: Option<KeyDifference>,
    pub first_property: Option<PropertyDifference>,
    pub property_maps_equal: bool,
}
pub fn payload(
    expected: &ibcmd_rs::restructure::caches::help_props::HelpProps,
    actual: &ibcmd_rs::restructure::caches::help_props::HelpProps,
) -> Result<PayloadComparison> {
    // Existing counted preflight closes duplicate keys/properties even for caller-supplied DTOs.
    for input in [expected, actual] {
        help_preflight(&parse_row(&input.render())?)?;
    }
    let map = |input: &ibcmd_rs::restructure::caches::help_props::HelpProps| -> Result<BTreeMap<String,usize>> {
        input.entries.iter().enumerate().map(|(i,e)| Ok((uuid(&e.key)?,i))).collect()
    };
    let e = map(expected)?;
    let a = map(actual)?;
    let missing_keys = e
        .keys()
        .filter(|k| !a.contains_key(*k))
        .cloned()
        .collect::<Vec<_>>();
    let extra_keys = a
        .keys()
        .filter(|k| !e.contains_key(*k))
        .cloned()
        .collect::<Vec<_>>();
    let first_key = (0..expected.entries.len().max(actual.entries.len())).find_map(|ordinal| {
        let expected = expected.entries.get(ordinal).map(|e| e.key.clone());
        let actual = actual.entries.get(ordinal).map(|e| e.key.clone());
        (expected != actual).then_some(KeyDifference {
            ordinal,
            expected,
            actual,
        })
    });
    let mut first_property = None;
    // Comparator source ordinal only, never candidate insertion order.
    for entry in &expected.entries {
        let key = uuid(&entry.key)?;
        let Some(&i) = a.get(&key) else {
            continue;
        };
        let other = &actual.entries[i];
        for ordinal in 0..entry.props.len().max(other.props.len()) {
            if entry.props.get(ordinal) != other.props.get(ordinal) {
                first_property = Some(PropertyDifference {
                    key: entry.key.clone(),
                    ordinal,
                    expected: entry
                        .props
                        .get(ordinal)
                        .map(|(id, v)| (id.clone(), value(v))),
                    actual: other
                        .props
                        .get(ordinal)
                        .map(|(id, v)| (id.clone(), value(v))),
                });
                break;
            }
        }
        if first_property.is_some() {
            break;
        }
    }
    Ok(PayloadComparison {
        expected_keys: e.len(),
        actual_keys: a.len(),
        property_maps_equal: missing_keys.is_empty()
            && extra_keys.is_empty()
            && first_property.is_none(),
        missing_keys,
        extra_keys,
        first_key,
        first_property,
    })
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct OccurrenceView {
    pub source: RowOrigin,
    pub plain_sha256: String,
    pub path: Vec<usize>,
    pub registry_span: Option<(usize, usize)>,
}
impl From<&Occurrence> for OccurrenceView {
    fn from(o: &Occurrence) -> Self {
        Self {
            source: o.source.clone(),
            plain_sha256: o.plain_sha256.clone(),
            path: o.path.clone(),
            registry_span: o.registry_span,
        }
    }
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "domain", rename_all = "snake_case")]
pub enum TargetView {
    Metadata {
        uuid: String,
        registry_ordinal: Option<usize>,
    },
    TypeId {
        uuid: String,
        declaration: Option<usize>,
    },
    DesignTimeInstance {
        type_id: String,
        value_id: String,
        type_declaration: Option<usize>,
    },
    Unknown {
        raw: BraceValue,
    },
}
fn target(t: &Target) -> TargetView {
    match t {
        Target::Metadata {
            uuid,
            registry_ordinal,
        } => TargetView::Metadata {
            uuid: uuid.clone(),
            registry_ordinal: *registry_ordinal,
        },
        Target::TypeId { uuid, declaration } => TargetView::TypeId {
            uuid: uuid.clone(),
            declaration: *declaration,
        },
        Target::DesignTimeValue {
            type_id,
            value_id,
            type_declaration,
        } => TargetView::DesignTimeInstance {
            type_id: type_id.clone(),
            value_id: value_id.clone(),
            type_declaration: *type_declaration,
        },
        Target::Unknown { raw } => TargetView::Unknown { raw: value(raw) },
    }
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MemberView {
    Generated { target: TargetView },
    Primitive { tag: String, raw: BraceValue },
    Unknown { raw: BraceValue },
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EdgeView {
    Reference {
        owner: String,
        owner_identity_proven: bool,
        slot: Option<String>,
        target: TargetView,
        first_in_domain: bool,
        occurrence: OccurrenceView,
    },
    TypeSet {
        set: String,
        member_ordinal: usize,
        member: MemberView,
        first_in_domain: Option<bool>,
        occurrence: OccurrenceView,
    },
    EmptyValue {
        owner: String,
        value_kind: String,
        raw: BraceValue,
        occurrence: OccurrenceView,
    },
    NilAction {
        action: String,
        set: Option<String>,
        occurrence: OccurrenceView,
    },
}
fn observed_edges(g: &FactGraph) -> Vec<EdgeView> {
    g.events
        .iter()
        .filter_map(|e| match e {
            ObservationEvent::OwnerVisit { .. } | ObservationEvent::EmitKey { .. } => None,
            ObservationEvent::ReferenceVisit {
                owner,
                owner_identity_proven,
                slot,
                target: t,
                first_in_domain,
                occurrence,
            } => Some(EdgeView::Reference {
                owner: owner.clone(),
                owner_identity_proven: *owner_identity_proven,
                slot: slot.map(str::to_owned),
                target: target(t),
                first_in_domain: *first_in_domain,
                occurrence: occurrence.into(),
            }),
            ObservationEvent::TypeSetVisit {
                set,
                member_ordinal,
                member,
                first_in_domain,
                occurrence,
            } => Some(EdgeView::TypeSet {
                set: set.clone(),
                member_ordinal: *member_ordinal,
                member: match member {
                    TypeMemberFact::Generated(t) => MemberView::Generated { target: target(t) },
                    TypeMemberFact::Primitive { tag, raw } => MemberView::Primitive {
                        tag: tag.clone(),
                        raw: value(raw),
                    },
                    TypeMemberFact::Unknown { raw } => MemberView::Unknown { raw: value(raw) },
                },
                first_in_domain: *first_in_domain,
                occurrence: occurrence.into(),
            }),
            ObservationEvent::EmptyValue {
                owner,
                kind,
                raw,
                occurrence,
            } => Some(EdgeView::EmptyValue {
                owner: owner.clone(),
                value_kind: match kind {
                    EmptyValueKind::MetadataReference => "metadata_reference",
                    EmptyValueKind::DesignTimeReference => "design_time_reference",
                }
                .into(),
                raw: value(raw),
                occurrence: occurrence.into(),
            }),
            ObservationEvent::NilAction {
                action,
                set,
                occurrence,
            } => Some(EdgeView::NilAction {
                action: match action {
                    NilActionKind::ObservedAggregate => "observed_aggregate",
                    NilActionKind::ObservedSetLink => "observed_set_link",
                }
                .into(),
                set: set.clone(),
                occurrence: occurrence.into(),
            }),
        })
        .collect()
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct EdgeDifference {
    pub ordinal: usize,
    pub expected: Option<EdgeView>,
    pub actual: Option<EdgeView>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum EdgeComparison {
    Unavailable {
        reason: String,
    },
    ObservedOnly {
        expected_count: usize,
        actual_count: usize,
        first: Option<EdgeDifference>,
        expected_partial: Vec<String>,
        actual_partial: Vec<String>,
    },
}
pub fn edges(expected: Option<&FactGraph>, actual: Option<&FactGraph>) -> EdgeComparison {
    let (Some(e), Some(a)) = (expected, actual) else {
        return EdgeComparison::Unavailable {
            reason: "missing independent facts manifest; empty observations cannot prove NoRefs"
                .into(),
        };
    };
    let ev = observed_edges(e);
    let av = observed_edges(a);
    let first = (0..ev.len().max(av.len())).find_map(|ordinal| {
        (ev.get(ordinal) != av.get(ordinal)).then(|| EdgeDifference {
            ordinal,
            expected: ev.get(ordinal).cloned(),
            actual: av.get(ordinal).cloned(),
        })
    });
    EdgeComparison::ObservedOnly {
        expected_count: ev.len(),
        actual_count: av.len(),
        first,
        expected_partial: e.partial.iter().map(|p| p.reason.to_owned()).collect(),
        actual_partial: a.partial.iter().map(|p| p.reason.to_owned()).collect(),
    }
}
#[derive(Clone, Debug)]
pub struct ProposedEmit {
    pub key: String,
    pub event_id: String,
    pub occurrence: Occurrence,
}
#[derive(Clone, Debug)]
pub struct DiagnosticProposal {
    pub emits: Vec<ProposedEmit>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ProposedAssociation {
    pub key: String,
    pub event_id: String,
    pub proposed_ordinal: usize,
    pub uuid_hash: u32,
    pub bucket: u32,
    pub occurrence: OccurrenceView,
}
#[derive(Clone, Debug, Serialize)]
pub struct ProposalComparison {
    pub untrusted_diagnostic_only: bool,
    pub associations: Vec<ProposedAssociation>,
    pub model_order: Vec<String>,
    pub literal: LiteralComparison,
    pub payload: PayloadComparison,
}
#[derive(Clone, Debug, Serialize)]
pub struct CurrentComparison {
    pub trace_status: String,
    pub graph_completeness: String,
    pub first_visit_authority: Option<()>,
    pub native_acceptance: bool,
    pub current_roundtrip: LiteralComparison,
    pub proposal: Option<ProposalComparison>,
}

/// Key-source witness only, NEVER a proven insertion event or generator.
pub fn key_occurrence(inputs: &ProjectionInputs, key: &str) -> Result<Occurrence> {
    let c = inputs.project()?;
    key_occurrence_from(inputs, &c, key)
}
/// One bound projection for the complete witness roster; no file rehash per key/event.
pub fn key_witnesses(inputs: &ProjectionInputs) -> Result<BTreeMap<String, Occurrence>> {
    let c = inputs.project()?;
    c.help
        .entries
        .iter()
        .map(|e| Ok((uuid(&e.key)?, key_occurrence_from(inputs, &c, &e.key)?)))
        .collect()
}
fn key_occurrence_from(inputs: &ProjectionInputs, c: &Coverage, key: &str) -> Result<Occurrence> {
    let key = uuid(key)?;
    let i = c
        .help
        .entries
        .iter()
        .position(|e| uuid(&e.key).is_ok_and(|k| k == key))
        .context("key outside current coverage")?;
    let (role, path, span) = match c.origins[i] {
        KeyOrigin::MetadataRecord { record_ordinal } => {
            let r = &c.registry.records[record_ordinal];
            (
                RowRole::Registry,
                vec![2, 1 + 7 * record_ordinal],
                Some((r.start, r.end)),
            )
        }
        KeyOrigin::TypeSet { set_ordinal } => {
            (RowRole::TypeSets, vec![1, 1 + 2 * set_ordinal], None)
        }
        KeyOrigin::NilAggregate => {
            let offset = 1 + c.help.entries[..i]
                .iter()
                .map(|e| 2 + 2 * e.props.len())
                .sum::<usize>();
            let p = c.help.entries[i]
                .props
                .iter()
                .position(|(id, _)| id.parse::<usize>() == Ok(23))
                .context("nil property23 missing")?;
            (RowRole::HelpProps, vec![1, offset + 3 + 2 * p], None)
        }
    };
    let row = inputs
        .manifest
        .rows
        .iter()
        .find(|r| r.role == role)
        .context("missing bound role")?;
    Ok(Occurrence {
        source: row.origin.clone(),
        plain_sha256: row.plain_sha256.clone(),
        path,
        registry_span: span,
    })
}
pub fn compare_current(
    inputs: &ProjectionInputs,
    proposal: Option<&DiagnosticProposal>,
) -> Result<CurrentComparison> {
    let coverage = inputs.project()?;
    let original = coverage.help.clone();
    let current_roundtrip = literal(inputs.raw(RowRole::HelpProps), &original.render());
    let proposal = if let Some(proposal) = proposal {
        let mut keys = BTreeSet::new();
        let mut ids = BTreeSet::new();
        let mut entry_by_key = BTreeMap::new();
        for (i, e) in original.entries.iter().enumerate() {
            entry_by_key.insert(uuid(&e.key)?, i);
        }
        let mut occurrences = Vec::new();
        for emit in &proposal.emits {
            let key = uuid(&emit.key)?;
            ensure!(keys.insert(key.clone()), "duplicate proposed key");
            ensure!(
                !emit.event_id.is_empty()
                    && !emit.event_id.chars().any(char::is_control)
                    && ids.insert(emit.event_id.clone()),
                "invalid/duplicate proposed event id"
            );
            let occurrence = key_occurrence_from(inputs, &coverage, &key)?;
            ensure!(
                occurrence == emit.occurrence,
                "proposed key source witness differs"
            );
            occurrences.push(occurrence);
        }
        ensure!(
            keys.len() == entry_by_key.len() && keys.iter().all(|k| entry_by_key.contains_key(k)),
            "proposed key coverage is not bijective"
        );
        // Model invocation only after complete key/event/source validation, never from native order.
        let insertion = proposal
            .emits
            .iter()
            .map(|e| e.key.as_str())
            .collect::<Vec<_>>();
        let model = order::iteration_order(insertion)?;
        let mut copied = original.clone();
        copied.entries =
            model
                .iter()
                .map(|key| {
                    Ok(original.entries
                        [*entry_by_key.get(&uuid(key)?).context("model key missing")?]
                    .clone())
                })
                .collect::<Result<_>>()?;
        let buckets = order::bucket_count(keys.len());
        let associations = proposal
            .emits
            .iter()
            .zip(occurrences)
            .enumerate()
            .map(|(proposed_ordinal, (emit, occurrence))| {
                let uuid_hash = order::uuid_hash(&emit.key)?;
                Ok(ProposedAssociation {
                    key: emit.key.clone(),
                    event_id: emit.event_id.clone(),
                    proposed_ordinal,
                    uuid_hash,
                    bucket: uuid_hash & ((buckets - 1) as u32),
                    occurrence: (&occurrence).into(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Some(ProposalComparison {
            untrusted_diagnostic_only: true,
            associations,
            model_order: model.iter().map(|k| (*k).to_owned()).collect(),
            literal: literal(inputs.raw(RowRole::HelpProps), &copied.render()),
            payload: payload(&original, &copied)?,
        })
    } else {
        None
    };
    ensure!(coverage.help == original, "caller payload changed");
    inputs.verify_sources()?;
    Ok(CurrentComparison {
        trace_status: "NotIdentified".into(),
        graph_completeness: "Partial".into(),
        first_visit_authority: None,
        native_acceptance: false,
        current_roundtrip,
        proposal,
    })
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledSources {
    pub helper: String,
    pub comparison: String,
    pub tests: String,
    pub order: String,
    pub renderer: String,
}
pub fn compiled_sources() -> CompiledSources {
    CompiledSources {
        helper: hash(include_bytes!("cache_insertion_trace.rs")),
        comparison: hash(include_bytes!("cache_insertion_comparison.rs")),
        tests: hash(include_bytes!("../cache_insertion_trace.rs")),
        order: hash(include_bytes!("../../src/restructure/caches/order.rs")),
        renderer: hash(include_bytes!("../../src/restructure/caches/help_props.rs")),
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    CapturedHistoricalSnapshot,
    WeakPlainOnlyDiagnostic,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotDeclaration {
    pub case: String,
    pub stage: String,
    pub purpose: String,
    pub registry_root_uuid: String,
    pub capture_witness: FileBinding,
    pub input_manifest: FileBinding,
    pub facts_manifest: Option<FileBinding>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureWitnessV1 {
    pub schema: String,
    pub case: String,
    pub stage: String,
    pub purpose: String,
    pub source_head: String,
    pub registry_root_uuid: String,
    pub input_manifest_sha256: String,
    pub provenance: Provenance,
    pub original_proofs: Vec<FileBinding>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedCase {
    pub id: String,
    pub provenance: Provenance,
    pub current: SnapshotDeclaration,
    pub baseline: Option<SnapshotDeclaration>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedCorpusV1 {
    pub schema: String,
    pub input_exporter_source_head: String,
    pub compiled_source: CompiledSources,
    pub cases: Vec<RetainedCase>,
}
struct Snapshot {
    declaration: SnapshotDeclaration,
    witness: CaptureWitnessV1,
    inputs: ProjectionInputs,
    facts: Option<FactInputs>,
    coverage: Coverage,
    graph: Option<FactGraph>,
}
fn read_bound(binding: &FileBinding) -> Result<Vec<u8>> {
    binding.verify()?;
    let bytes = fs::read(&binding.path)?;
    ensure!(
        bytes.len() as u64 == binding.length && hash(&bytes) == binding.sha256,
        "manifest changed during read"
    );
    binding.verify()?;
    Ok(bytes)
}
fn read_json<T: serde::de::DeserializeOwned>(binding: &FileBinding) -> Result<T> {
    Ok(serde_json::from_slice(&read_bound(binding)?)?)
}
fn load_snapshot(d: &SnapshotDeclaration, case: &RetainedCase, source: &str) -> Result<Snapshot> {
    ensure!(d.case == case.id, "snapshot case declaration differs");
    let witness: CaptureWitnessV1 = read_json(&d.capture_witness)?;
    ensure!(
        witness.schema == "cache-trace-capture-witness-v1"
            && witness.case == d.case
            && witness.stage == d.stage
            && witness.purpose == d.purpose
            && witness.source_head == source
            && witness.registry_root_uuid == d.registry_root_uuid
            && witness.input_manifest_sha256 == d.input_manifest.sha256
            && witness.provenance == case.provenance,
        "capture witness binding differs"
    );
    ensure!(
        !witness.original_proofs.is_empty(),
        "missing original capture/source proof"
    );
    for proof in &witness.original_proofs {
        proof.verify()?;
    }
    let manifest: ManifestV1 = read_json(&d.input_manifest)?;
    ensure!(
        manifest.case == d.case
            && manifest.stage == d.stage
            && manifest.snapshot_purpose == d.purpose
            && manifest.registry_root_uuid == d.registry_root_uuid,
        "snapshot identity differs"
    );
    // Each stage independently declares its original proof; equal payload is not a stage witness.
    for proof in &witness.original_proofs {
        ensure!(
            manifest.predecessors.iter().any(|p| p.path == proof.path
                && p.length == proof.length
                && p.sha256 == proof.sha256),
            "capture proof absent from stage predecessors"
        );
    }
    let inputs = ProjectionInputs::load(manifest, source)?;
    let coverage = inputs.project()?;
    let facts = if let Some(binding) = &d.facts_manifest {
        Some(FactInputs::load(&inputs, read_json(binding)?)?)
    } else {
        None
    };
    let graph = facts.as_ref().map(|f| f.project(&inputs)).transpose()?;
    Ok(Snapshot {
        declaration: d.clone(),
        witness,
        inputs,
        facts,
        coverage,
        graph,
    })
}
impl Snapshot {
    fn verify(&self) -> Result<()> {
        self.declaration.capture_witness.verify()?;
        self.declaration.input_manifest.verify()?;
        for proof in &self.witness.original_proofs {
            proof.verify()?;
        }
        self.inputs.verify_sources()?;
        if let Some(binding) = &self.declaration.facts_manifest {
            binding.verify()?;
        }
        if let Some(facts) = &self.facts {
            facts.verify_sources(&self.inputs)?;
        }
        Ok(())
    }
}
/// Kernel file identity, not spelling/canonical path: hard links may otherwise bypass alias checks.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct PhysicalFileId {
    volume: u64,
    index: u64,
}
fn physical_file_id(path: &Path) -> Result<PhysicalFileId> {
    physical(path)?;
    let file = fs::File::open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = file.metadata()?;
        Ok(PhysicalFileId {
            volume: m.dev(),
            index: m.ino(),
        })
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
        };
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        // Original read-only File handle remains alive for this query. No process inspection.
        ensure!(
            unsafe { GetFileInformationByHandle(file.as_raw_handle().cast(), &mut info) } != 0,
            "physical file identity unavailable: {}",
            std::io::Error::last_os_error()
        );
        Ok(PhysicalFileId {
            volume: u64::from(info.dwVolumeSerialNumber),
            index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
        })
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = file;
        bail!("physical file identity unsupported on this host")
    }
}
#[derive(Clone)]
struct RangeUse {
    identity: PhysicalFileId,
    start: u64,
    end: u64,
    row: RowBinding,
    capture: PathBuf,
}
fn register_sources(
    snapshots: &[(RetainedCase, Snapshot, Option<Snapshot>)],
    outer: &FileBinding,
) -> Result<(Vec<PathBuf>, Vec<RangeUse>)> {
    let mut protected = BTreeSet::new();
    let mut source_files = BTreeSet::new();
    let mut uses = Vec::new();
    protected.insert(physical_file_id(&outer.path)?);
    for (_, current, baseline) in snapshots {
        for s in std::iter::once(current).chain(baseline.iter()) {
            for p in [
                &s.declaration.capture_witness,
                &s.declaration.input_manifest,
            ]
            .into_iter()
            .chain(s.declaration.facts_manifest.iter())
            {
                ensure!(
                    protected.insert(physical_file_id(&p.path)?),
                    "duplicate/aliased proof manifest"
                );
            }
            for p in s
                .witness
                .original_proofs
                .iter()
                .chain(&s.inputs.manifest.predecessors)
            {
                source_files.insert((physical_file_id(&p.path)?, fs::canonicalize(&p.path)?));
            }
            let capture = fs::canonicalize(&s.declaration.capture_witness.path)?;
            for row in &s.inputs.manifest.rows {
                let path = fs::canonicalize(row.locator.path())?;
                source_files.insert((physical_file_id(&path)?, path.clone()));
                let start = match row.locator {
                    Locator::GzipRawDeflateRange { offset, .. } => offset,
                    _ => 0,
                };
                let end = start
                    .checked_add(row.stored_length)
                    .context("selected member range overflow")?;
                let used = RangeUse {
                    identity: physical_file_id(&path)?,
                    start,
                    end,
                    row: row.clone(),
                    capture: capture.clone(),
                };
                for other in &uses {
                    let other: &RangeUse = other;
                    if other.identity != used.identity {
                        continue;
                    }
                    if other.start < used.end && used.start < other.end {
                        // Independently captured unchanged historical stages may reuse the SAME
                        // identical range; conflicting identities or overlapping members refuse.
                        let a = &other.row;
                        let b = &used.row;
                        let unchanged = other.start == used.start
                            && other.end == used.end
                            && other.capture != used.capture
                            && a.role == b.role
                            && a.origin.table == b.origin.table
                            && a.origin.filename == b.origin.filename
                            && a.origin.part == b.origin.part
                            && a.stored_sha256 == b.stored_sha256
                            && a.packed_sha256 == b.packed_sha256
                            && a.plain_sha256 == b.plain_sha256
                            && a.packed_length == b.packed_length
                            && a.plain_length == b.plain_length
                            && (a.origin.case != b.origin.case || a.origin.stage != b.origin.stage);
                        ensure!(
                            unchanged,
                            "duplicate/overlapping/conflicting selected payload member"
                        );
                    }
                }
                uses.push(used);
            }
            if let Some(facts) = &s.facts {
                let base_ids = s
                    .inputs
                    .manifest
                    .rows
                    .iter()
                    .map(|r| physical_file_id(r.locator.path()))
                    .collect::<Result<BTreeSet<_>>>()?;
                let mut fact_ids = BTreeSet::new();
                for row in &facts.manifest.rows {
                    let id = physical_file_id(&row.file.path)?;
                    ensure!(
                        !base_ids.contains(&id) && fact_ids.insert(id),
                        "supplementary row physical alias"
                    );
                    source_files.insert((
                        physical_file_id(&row.file.path)?,
                        fs::canonicalize(&row.file.path)?,
                    ));
                }
            }
        }
    }
    ensure!(
        source_files.iter().all(|(id, _)| !protected.contains(id)),
        "proof manifest impersonates a row/original proof payload"
    );
    // Canonical paths remain for output containment; identity is used only for alias/overlap.
    let mut files = source_files
        .into_iter()
        .map(|(_, p)| p)
        .collect::<BTreeSet<_>>();
    files.insert(fs::canonicalize(&outer.path)?);
    for (_, current, baseline) in snapshots {
        for s in std::iter::once(current).chain(baseline.iter()) {
            for p in [
                &s.declaration.capture_witness,
                &s.declaration.input_manifest,
            ]
            .into_iter()
            .chain(s.declaration.facts_manifest.iter())
            {
                files.insert(fs::canonicalize(&p.path)?);
            }
        }
    }
    let files = files.into_iter().collect();
    Ok((files, uses))
}
#[derive(Serialize)]
pub struct PartialObservation {
    pub reason: String,
    pub owner: Option<String>,
    pub occurrence: Option<OccurrenceView>,
}
#[derive(Serialize)]
#[serde(tag = "domain", rename_all = "snake_case")]
pub enum KeySource {
    MetadataRecord { record_ordinal: usize },
    TypeSet { set_ordinal: usize },
    NilAggregate,
}
#[derive(Serialize)]
pub struct CoveredKey {
    pub key: String,
    pub origin: KeySource,
    pub occurrence: OccurrenceView,
}
#[derive(Serialize)]
pub struct SnapshotReport {
    pub case: String,
    pub stage: String,
    pub purpose: String,
    pub provenance: Provenance,
    pub capture_witness: FileBinding,
    pub input_manifest: FileBinding,
    pub rows: Vec<RowBinding>,
    pub predecessors: Vec<FileBinding>,
    pub original_proofs: Vec<FileBinding>,
    pub facts_manifest: Option<FileBinding>,
    pub fact_rows: Vec<FactRowBinding>,
    pub metadata_keys: usize,
    pub set_keys: usize,
    pub nil_keys: usize,
    pub owner_records: usize,
    pub all_sets: usize,
    pub comparison: CurrentComparison,
    pub partial_reasons: Vec<String>,
    pub observed_edges: Vec<EdgeView>,
    pub partial_observations: Vec<PartialObservation>,
    pub covered_keys: Vec<CoveredKey>,
}
fn snapshot_report(s: &Snapshot) -> Result<SnapshotReport> {
    let mut counts = [0usize; 3];
    for origin in &s.coverage.origins {
        counts[match origin {
            KeyOrigin::MetadataRecord { .. } => 0,
            KeyOrigin::TypeSet { .. } => 1,
            KeyOrigin::NilAggregate => 2,
        }] += 1;
    }
    Ok(SnapshotReport {
        case: s.declaration.case.clone(),
        stage: s.declaration.stage.clone(),
        purpose: s.declaration.purpose.clone(),
        provenance: s.witness.provenance,
        capture_witness: s.declaration.capture_witness.clone(),
        input_manifest: s.declaration.input_manifest.clone(),
        rows: s.inputs.manifest.rows.clone(),
        predecessors: s.inputs.manifest.predecessors.clone(),
        original_proofs: s.witness.original_proofs.clone(),
        facts_manifest: s.declaration.facts_manifest.clone(),
        fact_rows: s
            .facts
            .as_ref()
            .map_or_else(Vec::new, |f| f.manifest.rows.clone()),
        metadata_keys: counts[0],
        set_keys: counts[1],
        nil_keys: counts[2],
        owner_records: s.coverage.registry.records.len(),
        all_sets: s.coverage.sets.sets.len(),
        comparison: compare_current(&s.inputs, None)?,
        partial_reasons: s.graph.as_ref().map_or_else(
            || vec!["missing facts manifest; no graph or insertion authority".into()],
            |g| g.partial.iter().map(|p| p.reason.into()).collect(),
        ),
        observed_edges: s.graph.as_ref().map_or_else(Vec::new, observed_edges),
        partial_observations: s.graph.as_ref().map_or_else(Vec::new, |g| {
            g.partial
                .iter()
                .map(|p| PartialObservation {
                    reason: p.reason.into(),
                    owner: p.owner.clone(),
                    occurrence: p.occurrence.as_ref().map(Into::into),
                })
                .collect()
        }),
        covered_keys: s
            .coverage
            .help
            .entries
            .iter()
            .zip(&s.coverage.origins)
            .map(|(e, o)| {
                let occurrence = key_occurrence_from(&s.inputs, &s.coverage, &e.key)?;
                Ok(CoveredKey {
                    key: e.key.clone(),
                    origin: match o {
                        KeyOrigin::MetadataRecord { record_ordinal } => KeySource::MetadataRecord {
                            record_ordinal: *record_ordinal,
                        },
                        KeyOrigin::TypeSet { set_ordinal } => KeySource::TypeSet {
                            set_ordinal: *set_ordinal,
                        },
                        KeyOrigin::NilAggregate => KeySource::NilAggregate,
                    },
                    occurrence: (&occurrence).into(),
                })
            })
            .collect::<Result<_>>()?,
    })
}
#[derive(Serialize)]
pub struct StageComparison {
    pub literal: LiteralComparison,
    pub payload: PayloadComparison,
    pub recognized_edges: EdgeComparison,
}
#[derive(Serialize)]
pub struct CaseReport {
    pub id: String,
    pub current: SnapshotReport,
    pub baseline: Option<SnapshotReport>,
    pub baseline_vs_current: Option<StageComparison>,
}
#[derive(Serialize)]
pub struct CorpusReport {
    pub schema: String,
    pub diagnosis_finished: bool,
    pub native_acceptance: bool,
    pub original_issue_open: bool,
    pub trace_status: String,
    pub graph_completeness: String,
    pub first_visit_authority: Option<()>,
    pub compiled_source: CompiledSources,
    pub outer_manifest: FileBinding,
    pub cases: Vec<CaseReport>,
}
fn output_directory(path: &Path, sources: &[PathBuf]) -> Result<()> {
    ensure!(
        path.is_absolute()
            && !path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir)),
        "output must be absolute without parent traversal"
    );
    ensure!(
        fs::symlink_metadata(path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
        "output already exists or cannot be inspected"
    );
    let parent = path.parent().context("output parent missing")?;
    physical_directory(parent)?;
    let parent = fs::canonicalize(parent)?;
    let resolved = parent.join(path.file_name().context("output filename missing")?);
    for input in sources {
        ensure!(
            resolved != *input
                && !resolved.starts_with(input.parent().context("input parent missing")?),
            "output aliases or is nested in immutable inputs"
        );
    }
    fs::create_dir(&resolved)?;
    Ok(())
}
fn physical_directory(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        let m = fs::symlink_metadata(ancestor)?;
        ensure!(
            m.is_dir() && !m.file_type().is_symlink(),
            "output parent is not a physical directory"
        );
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            ensure!(m.file_attributes() & 0x400 == 0, "reparse output parent");
        }
    }
    Ok(())
}
fn create_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("output file parent missing")?;
    physical_directory(parent)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    file.sync_all()?;
    physical_directory(parent)?;
    physical(path)?;
    FileBinding {
        path: path.to_owned(),
        length: bytes.len() as u64,
        sha256: hash(bytes),
    }
    .verify()?;
    Ok(())
}
/// Portable actual handler; the retained env entry additionally enforces the authorized F scope.
/// Files alone are not runtime/native acceptance; ROOT must retain successful original run receipts.
pub fn run_closed(outer: FileBinding, output: &Path) -> Result<CorpusReport> {
    let corpus: RetainedCorpusV1 = read_json(&outer)?;
    ensure!(
        corpus.schema == "cache-trace-retained-corpus-v1"
            && hex(&corpus.input_exporter_source_head, 40),
        "corpus schema/source identity"
    );
    ensure!(
        serde_json::to_vec(&corpus.compiled_source)? == serde_json::to_vec(&compiled_sources())?,
        "running compiled source binding differs"
    );
    ensure!(!corpus.cases.is_empty(), "closed corpus has no cases");
    let mut ids = BTreeSet::new();
    let mut snapshots = Vec::new();
    for case in &corpus.cases {
        ensure!(
            !case.id.is_empty()
                && !case.id.chars().any(char::is_control)
                && ids.insert(case.id.clone()),
            "invalid/duplicate case id"
        );
        let current = load_snapshot(&case.current, case, &corpus.input_exporter_source_head)?;
        let baseline = case
            .baseline
            .as_ref()
            .map(|d| load_snapshot(d, case, &corpus.input_exporter_source_head))
            .transpose()?;
        if let Some(b) = &baseline {
            ensure!(
                b.declaration.stage != current.declaration.stage
                    && uuid(&b.declaration.registry_root_uuid)?
                        == uuid(&current.declaration.registry_root_uuid)?,
                "baseline/current identity differs or stage reused"
            );
        }
        snapshots.push((case.clone(), current, baseline));
    }
    let (sources, _) = register_sources(&snapshots, &outer)?;
    let mut cases = Vec::new();
    for (case, current, baseline) in &snapshots {
        let comparison = baseline
            .as_ref()
            .map(|b| {
                Ok::<_, anyhow::Error>(StageComparison {
                    literal: literal(
                        b.inputs.raw(RowRole::HelpProps),
                        current.inputs.raw(RowRole::HelpProps),
                    ),
                    payload: payload(&b.coverage.help, &current.coverage.help)?,
                    recognized_edges: edges(b.graph.as_ref(), current.graph.as_ref()),
                })
            })
            .transpose()?;
        cases.push(CaseReport {
            id: case.id.clone(),
            current: snapshot_report(current)?,
            baseline: baseline.as_ref().map(snapshot_report).transpose()?,
            baseline_vs_current: comparison,
        });
    }
    outer.verify()?;
    for (_, s, b) in &snapshots {
        s.verify()?;
        if let Some(b) = b {
            b.verify()?;
        }
    }
    let report = CorpusReport {
        schema: "cache-trace-diagnostic-report-v1".into(),
        diagnosis_finished: false,
        native_acceptance: false,
        original_issue_open: true,
        trace_status: "NotIdentified".into(),
        graph_completeness: "Partial".into(),
        first_visit_authority: None,
        compiled_source: compiled_sources(),
        outer_manifest: outer.clone(),
        cases,
    };
    let bytes = serde_json::to_vec_pretty(&report)?;
    output_directory(output, &sources)?;
    create_file(&output.join("report.json"), &bytes)?;
    // No success receipt before report durability and the final complete input revalidation.
    outer.verify()?;
    for (_, s, b) in &snapshots {
        s.verify()?;
        if let Some(b) = b {
            b.verify()?;
        }
    }
    let completion = serde_json::json!({"schema":"cache-trace-diagnostic-completion-v1","diagnosis_finished":true,"report_sha256":hash(&bytes),"outer_sha256":outer.sha256,"case_count":report.cases.len(),"native_acceptance":false,"trace_status":"NotIdentified","first_visit_authority":null,"original_issue_open":true});
    create_file(
        &output.join("completion.json"),
        &serde_json::to_vec_pretty(&completion)?,
    )?;
    Ok(report)
}
#[derive(Clone, Debug)]
pub struct EnvRequest {
    pub manifest: PathBuf,
    pub sha256: String,
    pub output: PathBuf,
}
impl EnvRequest {
    pub fn parse(
        manifest: Option<std::ffi::OsString>,
        sha256: Option<std::ffi::OsString>,
        output: Option<std::ffi::OsString>,
    ) -> Result<Self> {
        let manifest = PathBuf::from(manifest.context("missing IBCMD_RS_CACHE_TRACE_MANIFEST")?);
        let sha256 = sha256
            .context("missing independently ROOT-pinned manifest SHA256")?
            .into_string()
            .map_err(|_| anyhow::anyhow!("manifest SHA256 is not UTF8"))?;
        let output = PathBuf::from(output.context("missing new IBCMD_RS_CACHE_TRACE_OUTPUT")?);
        ensure!(hex(&sha256, 64), "invalid independent manifest SHA256");
        for path in [&manifest, &output] {
            ensure!(
                path.is_absolute()
                    && path
                        .to_string_lossy()
                        .replace('\\', "/")
                        .get(..3)
                        .is_some_and(|p| p.eq_ignore_ascii_case("F:/")),
                "retained laboratory paths must be absolute F paths"
            );
        }
        Ok(Self {
            manifest,
            sha256,
            output,
        })
    }
}
pub fn run_from_env() -> Result<CorpusReport> {
    let request = EnvRequest::parse(
        std::env::var_os("IBCMD_RS_CACHE_TRACE_MANIFEST"),
        std::env::var_os("IBCMD_RS_CACHE_TRACE_MANIFEST_SHA256"),
        std::env::var_os("IBCMD_RS_CACHE_TRACE_OUTPUT"),
    )?;
    physical(&request.manifest)?;
    let outer = FileBinding {
        length: fs::metadata(&request.manifest)?.len(),
        path: request.manifest,
        sha256: request.sha256,
    };
    run_closed(outer, &request.output)
}
