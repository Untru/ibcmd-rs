//! Diagnostic input admission and key coverage only. Never an insertion planner.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use flate2::{Decompress, FlushDecompress, Status};
use ibcmd_rs::metadata_model::brace::{Brace, NIL_UUID, parse_row};
use ibcmd_rs::mssql_config_apply::si::{self, SiMain};
use ibcmd_rs::restructure::caches::{facts, help_props::HelpProps, registry, type_sets::TypeSets};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const SET_LINK_LIST_CLASS: &str = "9cd510d6-abfc-11d4-9434-004095e12fc7";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RowRole {
    Registry,
    TypeSets,
    HelpProps,
}

// A1.3/A1.4: source observations only. This component cannot produce an insertion candidate.
const TYPE_INDEX_ROW: &str = "2203278d-ef4f-4f68-98f1-feb257d53ecc.si";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "role", rename_all = "snake_case", deny_unknown_fields)]
pub enum FactRole {
    Descriptor { owner: String },
    TypeIndex,
}

/// Explicit, already inflated additional rows. No implicit descriptor discovery or decompression.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FactRowBinding {
    pub role: FactRole,
    pub origin: RowOrigin,
    pub file: FileBinding,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FactsManifestV1 {
    pub schema: String,
    pub source_head: String,
    pub case: String,
    pub stage: String,
    pub rows: Vec<FactRowBinding>,
}

pub struct FactInputs {
    base_witness: String,
    manifest: FactsManifestV1,
    plain: Vec<Vec<u8>>,
}

impl FactInputs {
    pub fn load(base: &ProjectionInputs, manifest: FactsManifestV1) -> Result<Self> {
        base.verify_sources()?;
        ensure!(
            manifest.schema == "cache-insertion-trace-facts-v1"
                && manifest.source_head == base.manifest.source_head
                && manifest.case == base.manifest.case
                && manifest.stage == base.manifest.stage,
            "facts source/case/stage binding differs"
        );
        let mut owners = BTreeSet::new();
        let mut paths: BTreeSet<_> = base
            .manifest
            .rows
            .iter()
            .map(|row| fs::canonicalize(row.locator.path()))
            .collect::<std::io::Result<_>>()?;
        for predecessor in &base.manifest.predecessors {
            paths.insert(fs::canonicalize(&predecessor.path)?);
        }
        let mut index = false;
        let mut plain = Vec::new();
        for row in &manifest.rows {
            ensure!(
                row.origin.case == manifest.case
                    && row.origin.stage == manifest.stage
                    && row.origin.part == 0
                    && !row.origin.version.is_empty()
                    && !row.origin.version.chars().any(char::is_control),
                "facts row identity"
            );
            match &row.role {
                FactRole::Descriptor { owner } => {
                    let owner = uuid(owner)?;
                    ensure!(
                        owner != NIL_UUID && owners.insert(owner.clone()),
                        "nil/duplicate descriptor owner"
                    );
                    ensure!(
                        row.origin.table == "Config" && row.origin.filename == owner,
                        "descriptor filename/table binding"
                    );
                }
                FactRole::TypeIndex => {
                    ensure!(!index, "duplicate type index row");
                    index = true;
                    ensure!(
                        row.origin.table == "Params" && row.origin.filename == TYPE_INDEX_ROW,
                        "type index filename/table binding"
                    );
                }
            }
            physical(&row.file.path)?;
            let canonical = fs::canonicalize(&row.file.path)?;
            ensure!(
                paths.insert(canonical),
                "aliased fact/base/predecessor input"
            );
            row.file.verify()?;
            let bytes = fs::read(&row.file.path)?;
            ensure!(
                bytes.len() as u64 == row.file.length && hash(&bytes) == row.file.sha256,
                "fact input changed while reading"
            );
            plain.push(bytes);
        }
        let result = Self {
            base_witness: hash(&serde_json::to_vec(&base.manifest)?),
            manifest,
            plain,
        };
        result.verify_sources(base)?;
        Ok(result)
    }

    pub fn verify_sources(&self, base: &ProjectionInputs) -> Result<()> {
        ensure!(
            hash(&serde_json::to_vec(&base.manifest)?) == self.base_witness,
            "different base ledger supplied to retained facts"
        );
        base.verify_sources()?;
        for row in &self.manifest.rows {
            row.file.verify()?;
        }
        Ok(())
    }

    pub fn raw(&self, ordinal: usize) -> Option<&[u8]> {
        self.plain.get(ordinal).map(Vec::as_slice)
    }

    pub fn project(&self, base: &ProjectionInputs) -> Result<FactGraph> {
        self.verify_sources(base)?;
        let coverage = base.project()?;
        let graph = observed_graph(base, &coverage, self)?;
        self.verify_sources(base)?;
        Ok(graph)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Occurrence {
    pub source: RowOrigin,
    pub plain_sha256: String,
    /// Indexes in the actual Brace tree; not re-rendered or guessed byte spans.
    pub path: Vec<usize>,
    /// Only SiMain supplies actual original byte spans.
    pub registry_span: Option<(usize, usize)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartialFact {
    pub reason: &'static str,
    pub owner: Option<String>,
    pub occurrence: Option<Occurrence>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedSlotFact {
    pub name: &'static str,
    pub position: usize,
    pub value: Brace,
}

#[derive(Clone, Debug)]
pub struct DescriptorFact {
    pub owner: String,
    pub kind: &'static str,
    pub tag: i64,
    /// RecordMap's physical indexes, never its HashMap iteration order.
    pub slots: Vec<NamedSlotFact>,
    pub facts: facts::ObjectFacts,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedDeclaration {
    pub owner: String,
    pub class: String,
    pub type_id: String,
    pub value_id: String,
    pub category_index: u32,
    pub occurrence: Occurrence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Metadata {
        uuid: String,
        registry_ordinal: Option<usize>,
    },
    TypeId {
        uuid: String,
        declaration: Option<usize>,
    },
    /// Instance value identity is NOT the generated ValueId namespace in TypeIndex.
    DesignTimeValue {
        type_id: String,
        value_id: String,
        type_declaration: Option<usize>,
    },
    Unknown {
        raw: Brace,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeMemberFact {
    Generated(Target),
    Primitive { tag: String, raw: Brace },
    Unknown { raw: Brace },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NilActionKind {
    ObservedAggregate,
    ObservedSetLink,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmptyValueKind {
    MetadataReference,
    DesignTimeReference,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObservationEvent {
    OwnerVisit {
        owner: String,
        parent: String,
        class: String,
        ordinal: usize,
        occurrence: Occurrence,
    },
    ReferenceVisit {
        owner: String,
        owner_identity_proven: bool,
        slot: Option<&'static str>,
        target: Target,
        first_in_domain: bool,
        occurrence: Occurrence,
    },
    /// A canonical empty value is not a reference to a nil declaration.
    EmptyValue {
        owner: String,
        kind: EmptyValueKind,
        raw: Brace,
        occurrence: Occurrence,
    },
    TypeSetVisit {
        set: String,
        member_ordinal: usize,
        member: TypeMemberFact,
        first_in_domain: Option<bool>,
        occurrence: Occurrence,
    },
    NilAction {
        action: NilActionKind,
        set: Option<String>,
        occurrence: Occurrence,
    },
    /// Reserved contract only: no A1 function creates this action.
    EmitKey { key: String },
}

pub struct FactGraph {
    pub type_index: Option<ibcmd_rs::restructure::caches::type_index::TypeIndex>,
    pub descriptors: Vec<DescriptorFact>,
    pub generated: Vec<GeneratedDeclaration>,
    pub events: Vec<ObservationEvent>,
    pub partial: Vec<PartialFact>,
    pub pending_atoms: [&'static str; 3],
    pub graph_completeness: GraphCompleteness,
    pub trace_status: TraceStatus,
    pub candidate: Option<Vec<String>>,
    pub first_visit_authority: Option<()>,
}

fn row_occurrence(origin: &RowOrigin, plain_sha256: &str, path: Vec<usize>) -> Occurrence {
    Occurrence {
        source: origin.clone(),
        plain_sha256: plain_sha256.to_owned(),
        path,
        registry_span: None,
    }
}

fn partial(
    graph: &mut FactGraph,
    reason: &'static str,
    owner: Option<&str>,
    occurrence: Option<Occurrence>,
) {
    graph.partial.push(PartialFact {
        reason,
        owner: owner.map(str::to_owned),
        occurrence,
    });
}

/// Close all variable counts and all identity domains BEFORE the existing TypeIndex parser.
fn type_index_preflight(tree: &Brace, coverage: &Coverage) -> Result<()> {
    let [version, body] = list(tree)? else {
        bail!("type index outer arity");
    };
    ensure!(atom(version)? == "1", "type index version");
    let (n, mut rest) = list(body)?.split_first().context("empty type index body")?;
    let sections = count(n)?;
    ensure!(
        sections <= rest.len() / 2,
        "type index section count exceeds input"
    );
    let mut classes = BTreeSet::new();
    let mut owners = BTreeSet::new();
    let mut type_ids = BTreeSet::new();
    let mut value_ids = BTreeSet::new();
    for _ in 0..sections {
        let [class, n, tail @ ..] = rest else {
            bail!("type index section truncated");
        };
        let class = uuid(atom(class)?)?;
        ensure!(
            class != NIL_UUID && classes.insert(class.clone()),
            "nil/duplicate type index class"
        );
        let entries = count(n)?;
        ensure!(
            entries <= tail.len() / 2,
            "type index entry count exceeds input"
        );
        rest = tail;
        for _ in 0..entries {
            let [owner, n, tail @ ..] = rest else {
                bail!("type index entry truncated");
            };
            let owner = uuid(atom(owner)?)?;
            ensure!(
                owner != NIL_UUID && owners.insert(owner.clone()),
                "nil/duplicate type index owner"
            );
            let ordinal = coverage
                .registry
                .index_of(&owner)
                .context("type index owner absent from registry")?;
            ensure!(
                coverage.registry.classes[coverage.registry.records[ordinal].kind] == class,
                "type index owner class differs"
            );
            let length = count(n)?
                .checked_mul(3)
                .context("type index cardinality overflow")?;
            ensure!(length <= tail.len(), "type index type count exceeds input");
            let mut categories = BTreeSet::new();
            for triple in tail[..length].chunks_exact(3) {
                let type_id = uuid(atom(&triple[0])?)?;
                let value_id = uuid(atom(&triple[1])?)?;
                ensure!(
                    type_id != NIL_UUID && type_ids.insert(type_id),
                    "nil/duplicate generated TypeId"
                );
                ensure!(
                    value_id != NIL_UUID && value_ids.insert(value_id),
                    "nil/duplicate generated ValueId"
                );
                let index =
                    u32::try_from(count(&triple[2])?).context("type category index not u32")?;
                ensure!(
                    categories.insert(index),
                    "duplicate type category index within owner"
                );
            }
            rest = &tail[length..];
        }
    }
    ensure!(rest.is_empty(), "type index trailing tokens");
    Ok(())
}

fn descriptor_preflight(tree: &Brace) -> Result<()> {
    let [version, record, n, collections @ ..] = list(tree)? else {
        bail!("descriptor outer arity");
    };
    ensure!(
        atom(version)? == "1" && !list(record)?.is_empty(),
        "descriptor version/record"
    );
    ensure!(
        count(n)? == collections.len(),
        "descriptor collection count"
    );
    let mut classes = BTreeSet::new();
    for collection in collections {
        let [class, n, children @ ..] = list(collection)? else {
            bail!("descriptor collection header");
        };
        let class = uuid(atom(class)?)?;
        ensure!(
            class != NIL_UUID && classes.insert(class),
            "nil/duplicate descriptor collection"
        );
        ensure!(count(n)? == children.len(), "descriptor child count");
    }
    Ok(())
}

fn own_header_preflight(
    record: &[Brace],
    map: &ibcmd_rs::restructure::caches::slots::RecordMap,
    owner: &str,
    name: &str,
) -> Result<()> {
    let header = record
        .get(map.header)
        .context("descriptor header missing")?;
    let header = match header.as_list() {
        Some([zero, header]) if zero.as_atom() == Some("0") => header,
        _ => header,
    };
    let [
        tag,
        identity,
        actual_name,
        synonyms,
        comment,
        zero1,
        zero2,
        nil,
        zero3,
    ] = list(header)?
    else {
        bail!("descriptor own header arity");
    };
    let [one, zero, id] = list(identity)? else {
        bail!("descriptor own identity arity");
    };
    ensure!(
        atom(tag)? == "3"
            && atom(one)? == "1"
            && atom(zero)? == "0"
            && uuid(atom(id)?)? == owner
            && actual_name.as_str() == Some(name),
        "descriptor own identity/name differs"
    );
    ensure!(
        comment.as_str().is_some()
            && atom(zero1)? == "0"
            && atom(zero2)? == "0"
            && uuid(atom(nil)?)? == NIL_UUID
            && atom(zero3)? == "0",
        "unknown descriptor header variant"
    );
    let (n, values) = list(synonyms)?
        .split_first()
        .context("empty descriptor synonym")?;
    ensure!(
        count(n)?.checked_mul(2) == Some(values.len())
            && values.iter().all(|v| v.as_str().is_some()),
        "descriptor synonym count/value"
    );
    Ok(())
}

/// Recognized primitive forms are the exact forms emitted by metadata_model::types.
/// Everything else remains whole Unknown; member_id(None) alone proves nothing.
fn primitive_member(node: &Brace) -> Option<String> {
    let fields = node.as_list()?;
    let tag = fields.first()?.as_str()?;
    let nonnegative = |v: &Brace| v.as_atom().and_then(|v| v.parse::<u64>().ok()).is_some();
    let flag = |v: &Brace| matches!(v.as_atom(), Some("0" | "1"));
    let valid = match (tag, fields) {
        ("B" | "L" | "S" | "N" | "D" | "R", [_]) => true,
        ("S" | "R", [_, length, allowed]) => nonnegative(length) && flag(allowed),
        ("N", [_, digits, fraction, sign]) => {
            nonnegative(digits) && nonnegative(fraction) && flag(sign)
        }
        ("D", [_, fraction]) => matches!(fraction.as_str(), Some("D" | "T")),
        _ => false,
    };
    valid.then(|| tag.to_owned())
}

#[derive(Clone, Copy)]
struct ReferenceContext<'a> {
    owner: &'a str,
    owner_identity_proven: bool,
    slots: &'a [NamedSlotFact],
    origin: &'a RowOrigin,
    plain_sha256: &'a str,
    coverage: &'a Coverage,
    generated_by_type: &'a BTreeMap<String, usize>,
}

fn observe_references(
    tree: &Brace,
    context: ReferenceContext<'_>,
    graph: &mut FactGraph,
    seen: &mut BTreeSet<(u8, String)>,
) -> Result<()> {
    let ReferenceContext {
        owner,
        owner_identity_proven,
        slots,
        origin,
        plain_sha256,
        coverage,
        generated_by_type,
    } = context;
    use ibcmd_rs::metadata_model::types::DESIGN_TIME_REF_TYPE;
    let mut pending = vec![(tree, Vec::new())];
    while let Some((node, path)) = pending.pop() {
        let Some(items) = node.as_list() else {
            continue;
        };
        let class = items.get(1).and_then(Brace::as_atom);
        let occurrence = row_occurrence(origin, plain_sha256, path.clone());
        if items.first().and_then(Brace::as_str) == Some("Pattern") {
            for (i, member) in items.iter().enumerate().skip(1) {
                if member
                    .as_list()
                    .and_then(|items| items.first())
                    .and_then(Brace::as_str)
                    != Some("#")
                    && primitive_member(member).is_none()
                {
                    let mut member_path = path.clone();
                    member_path.push(i);
                    partial(
                        graph,
                        "unknown-descriptor-pattern-member",
                        Some(owner),
                        Some(row_occurrence(origin, plain_sha256, member_path)),
                    );
                }
            }
        }
        let in_pattern = path
            .split_last()
            .and_then(|(_, parent)| tree.at(parent))
            .and_then(Brace::as_list)
            .and_then(|items| items.first())
            .and_then(Brace::as_str)
            == Some("Pattern");
        let target = if in_pattern && items.first().and_then(Brace::as_str) == Some("#") {
            let [_, id] = items else {
                bail!("descriptor TypeId member arity");
            };
            let id = uuid(atom(id)?)?;
            ensure!(id != NIL_UUID, "nil descriptor TypeId member");
            let declaration = generated_by_type.get(&id).copied();
            if declaration.is_none() {
                partial(
                    graph,
                    "unresolved-descriptor-type-id",
                    Some(owner),
                    Some(occurrence.clone()),
                );
            }
            Some((
                Target::TypeId {
                    uuid: id.clone(),
                    declaration,
                },
                3,
                id,
            ))
        } else if class.is_some_and(|id| id.eq_ignore_ascii_case(facts::METADATA_REF)) {
            let [tag, _, value] = items else {
                bail!("metadata reference arity");
            };
            let [one, id] = list(value)? else {
                bail!("metadata reference target arity");
            };
            ensure!(
                tag.as_str() == Some("#") && atom(one)? == "1",
                "metadata reference tag/version"
            );
            let id = uuid(atom(id)?)?;
            if id == NIL_UUID {
                partial(
                    graph,
                    "empty-metadata-reference-value-semantic-slot-unproved",
                    Some(owner),
                    Some(occurrence.clone()),
                );
                graph.events.push(ObservationEvent::EmptyValue {
                    owner: owner.to_owned(),
                    kind: EmptyValueKind::MetadataReference,
                    raw: node.clone(),
                    occurrence: occurrence.clone(),
                });
                None
            } else {
                let ordinal = coverage.registry.index_of(&id);
                if ordinal.is_none() {
                    partial(
                        graph,
                        "unresolved-metadata-reference",
                        Some(owner),
                        Some(occurrence.clone()),
                    );
                }
                Some((
                    Target::Metadata {
                        uuid: id.clone(),
                        registry_ordinal: ordinal,
                    },
                    0,
                    id,
                ))
            }
        } else if class.is_some_and(|id| id.eq_ignore_ascii_case(DESIGN_TIME_REF_TYPE)) {
            let [tag, _, value] = items else {
                bail!("design-time reference arity");
            };
            let [zero, type_id, value_id] = list(value)? else {
                bail!("design-time reference target arity");
            };
            ensure!(
                tag.as_str() == Some("#") && atom(zero)? == "0",
                "design-time reference tag/version"
            );
            let type_id = uuid(atom(type_id)?)?;
            let value_id = uuid(atom(value_id)?)?;
            if type_id == NIL_UUID && value_id == NIL_UUID {
                partial(
                    graph,
                    "empty-design-time-reference-value-semantic-slot-unproved",
                    Some(owner),
                    Some(occurrence.clone()),
                );
                graph.events.push(ObservationEvent::EmptyValue {
                    owner: owner.to_owned(),
                    kind: EmptyValueKind::DesignTimeReference,
                    raw: node.clone(),
                    occurrence: occurrence.clone(),
                });
                None
            } else {
                ensure!(
                    type_id != NIL_UUID,
                    "nil design-time TypeId with nonempty value"
                );
                let declaration = generated_by_type.get(&type_id).copied();
                partial(
                    graph,
                    "design-time-instance-value-not-resolved-by-generated-value-index",
                    Some(owner),
                    Some(occurrence.clone()),
                );
                Some((
                    Target::DesignTimeValue {
                        type_id: type_id.clone(),
                        value_id: value_id.clone(),
                        type_declaration: declaration,
                    },
                    1,
                    format!("{type_id}/{value_id}"),
                ))
            }
        } else if items.first().and_then(Brace::as_str) == Some("#") {
            partial(
                graph,
                "unknown-tagged-reference-or-value",
                Some(owner),
                Some(occurrence.clone()),
            );
            Some((
                Target::Unknown { raw: node.clone() },
                2,
                format!("{owner}:{path:?}"),
            ))
        } else {
            None
        };
        if let Some((target, domain, identity)) = target {
            let slot = if path.first() == Some(&1) {
                path.get(1)
                    .and_then(|i| slots.iter().find(|slot| slot.position == *i))
                    .map(|slot| slot.name)
            } else {
                None
            };
            graph.events.push(ObservationEvent::ReferenceVisit {
                owner: owner.to_owned(),
                owner_identity_proven,
                slot,
                target,
                first_in_domain: seen.insert((domain, identity)),
                occurrence,
            });
        }
        // Descend even unknown envelopes: a recognized nested edge must not disappear.
        for (i, child) in items.iter().enumerate().rev() {
            let mut child_path = path.clone();
            child_path.push(i);
            pending.push((child, child_path));
        }
    }
    Ok(())
}

fn observed_graph(
    base: &ProjectionInputs,
    coverage: &Coverage,
    inputs: &FactInputs,
) -> Result<FactGraph> {
    use ibcmd_rs::restructure::caches::{
        root,
        slots::{RecordMap, owner_record},
        type_index::TypeIndex,
    };
    let mut graph = FactGraph {
        type_index: None,
        descriptors: Vec::new(),
        generated: Vec::new(),
        events: Vec::new(),
        partial: Vec::new(),
        pending_atoms: ["A1.5", "A1.6", "A1.7"],
        graph_completeness: GraphCompleteness::Partial,
        trace_status: TraceStatus::NotIdentified,
        candidate: None,
        first_visit_authority: None,
    };
    let source = |role| {
        base.manifest
            .rows
            .iter()
            .find(|row| row.role == role)
            .expect("closed base roles")
    };
    for (ordinal, record) in coverage.registry.records.iter().enumerate() {
        let mut occurrence = row_occurrence(
            &source(RowRole::Registry).origin,
            &source(RowRole::Registry).plain_sha256,
            vec![2, 1 + 7 * ordinal],
        );
        occurrence.registry_span = Some((record.start, record.end));
        graph.events.push(ObservationEvent::OwnerVisit {
            owner: record.uuid.clone(),
            parent: record.parent.clone(),
            class: coverage.registry.classes[record.kind].clone(),
            ordinal,
            occurrence,
        });
    }
    let index_row = inputs
        .manifest
        .rows
        .iter()
        .position(|row| matches!(row.role, FactRole::TypeIndex));
    if let Some(i) = index_row {
        let tree = parse_row(&inputs.plain[i])?;
        type_index_preflight(&tree, coverage)?;
        let index = TypeIndex::parse(&inputs.plain[i])?;
        let mut offset = 1;
        for section in &index.sections {
            offset += 2;
            for entry in &section.entries {
                offset += 2;
                for slot in &entry.types {
                    graph.generated.push(GeneratedDeclaration {
                        owner: uuid(&entry.object)?,
                        class: uuid(&section.class)?,
                        type_id: uuid(&slot.type_id)?,
                        value_id: uuid(&slot.value_id)?,
                        category_index: slot.index,
                        occurrence: row_occurrence(
                            &inputs.manifest.rows[i].origin,
                            &inputs.manifest.rows[i].file.sha256,
                            vec![1, offset],
                        ),
                    });
                    offset += 3;
                }
            }
        }
        graph.type_index = Some(index);
    } else {
        partial(&mut graph, "missing-type-index", None, None);
    }
    let generated_by_type: BTreeMap<_, _> = graph
        .generated
        .iter()
        .enumerate()
        .map(|(i, d)| (d.type_id.clone(), i))
        .collect();
    let mut provided = BTreeSet::new();
    let mut seen_references = BTreeSet::new();
    let mut descriptor_type_ids = BTreeSet::new();
    let mut descriptor_value_ids = BTreeSet::new();
    for (row, raw) in inputs.manifest.rows.iter().zip(&inputs.plain) {
        let FactRole::Descriptor { owner } = &row.role else {
            continue;
        };
        let owner = uuid(owner)?;
        provided.insert(owner.clone());
        let ordinal = coverage
            .registry
            .index_of(&owner)
            .context("descriptor owner absent from registry")?;
        let record = &coverage.registry.records[ordinal];
        let tree = parse_row(raw)?;
        descriptor_preflight(&tree)?;
        let class = &coverage.registry.classes[record.kind];
        let kind = root::kind_of_class(class);
        let (body, tag) = owner_record(&tree)?;
        let map = kind.and_then(|kind| RecordMap::new(kind, tag).ok());
        let mut owner_identity_proven = false;
        let slots = if let (Some(kind), Some(map)) = (kind, map) {
            own_header_preflight(body, &map, &owner, &record.name)?;
            owner_identity_proven = true;
            let generated = map.generated_types(body)?;
            let mut types = BTreeSet::new();
            let mut values = BTreeSet::new();
            for declaration in &generated {
                let type_id = uuid(&declaration.type_id)?;
                let value_id = uuid(&declaration.value_id)?;
                ensure!(
                    type_id != NIL_UUID
                        && types.insert(type_id.clone())
                        && value_id != NIL_UUID
                        && values.insert(value_id.clone()),
                    "nil/duplicate descriptor generated identities"
                );
                ensure!(
                    descriptor_type_ids.insert(type_id.clone())
                        && descriptor_value_ids.insert(value_id.clone()),
                    "duplicate generated declaration across descriptors"
                );
                if index_row.is_some() {
                    ensure!(
                        generated_by_type
                            .get(&type_id)
                            .is_some_and(|&i| graph.generated[i].owner == owner
                                && graph.generated[i].value_id == value_id),
                        "descriptor generated pair differs from current type index"
                    );
                }
            }
            let mut named: Vec<_> = map
                .names
                .iter()
                .map(|(&name, &position)| (position, name))
                .collect();
            named.sort_unstable();
            ensure!(
                named.windows(2).all(|pair| pair[0].0 != pair[1].0),
                "named slot position collision"
            );
            let slots = named
                .into_iter()
                .map(|(position, name)| {
                    Ok(NamedSlotFact {
                        name,
                        position,
                        value: map.slot(body, name)?.clone(),
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let facts = facts::ObjectFacts::parse(kind, &tree)?;
            ensure!(
                facts.uuid == owner && facts.name == record.name,
                "ObjectFacts own identity differs"
            );
            // RecordMap selects old/modern positions but does not close version/constant/collection semantics.
            // Keep those observations explicitly partial instead of inventing a second family decoder.
            partial(
                &mut graph,
                "owner-layout-version-and-unmapped-body-not-completely-admitted",
                Some(&owner),
                Some(row_occurrence(&row.origin, &row.file.sha256, vec![1])),
            );
            graph.descriptors.push(DescriptorFact {
                owner: owner.clone(),
                kind,
                tag,
                slots: slots.clone(),
                facts,
            });
            slots
        } else {
            partial(
                &mut graph,
                "unsupported-descriptor-class-or-record-map",
                Some(&owner),
                Some(row_occurrence(&row.origin, &row.file.sha256, vec![1])),
            );
            Vec::new()
        };
        observe_references(
            &tree,
            ReferenceContext {
                owner: &owner,
                owner_identity_proven,
                slots: &slots,
                origin: &row.origin,
                plain_sha256: &row.file.sha256,
                coverage,
                generated_by_type: &generated_by_type,
            },
            &mut graph,
            &mut seen_references,
        )?;
    }
    for record in &coverage.registry.records {
        if !provided.contains(&record.uuid) {
            partial(&mut graph, "missing-descriptor", Some(&record.uuid), None);
        }
    }
    let mut seen_types = BTreeSet::new();
    for (set_ordinal, (set, members)) in coverage.sets.sets.iter().enumerate() {
        for (member_ordinal, member) in members.iter().enumerate() {
            let occurrence = row_occurrence(
                &source(RowRole::TypeSets).origin,
                &source(RowRole::TypeSets).plain_sha256,
                vec![1, 2 + set_ordinal * 2, member_ordinal + 1],
            );
            let (member, first_in_domain) =
                if let Some(id) = ibcmd_rs::restructure::caches::type_sets::member_id(member) {
                    let id = uuid(id)?;
                    let declaration = generated_by_type.get(&id).copied();
                    if declaration.is_none() {
                        partial(
                            &mut graph,
                            "unresolved-type-id-member",
                            None,
                            Some(occurrence.clone()),
                        );
                    }
                    (
                        TypeMemberFact::Generated(Target::TypeId {
                            uuid: id.clone(),
                            declaration,
                        }),
                        Some(seen_types.insert(id)),
                    )
                } else if let Some(tag) = primitive_member(member) {
                    (
                        TypeMemberFact::Primitive {
                            tag,
                            raw: member.clone(),
                        },
                        None,
                    )
                } else {
                    partial(
                        &mut graph,
                        "unknown-primitive-or-member-shape",
                        None,
                        Some(occurrence.clone()),
                    );
                    (
                        TypeMemberFact::Unknown {
                            raw: member.clone(),
                        },
                        None,
                    )
                };
            graph.events.push(ObservationEvent::TypeSetVisit {
                set: uuid(set)?,
                member_ordinal,
                member,
                first_in_domain,
                occurrence,
            });
        }
    }
    let tree = parse_row(base.raw(RowRole::HelpProps))?;
    let body = list(&list(&tree)?[1])?;
    let mut offset = 1;
    for entry in &coverage.help.entries {
        if uuid(&entry.key)? == NIL_UUID {
            let property = entry
                .props
                .iter()
                .position(|(id, _)| id.parse::<usize>() == Ok(23))
                .context("nil property 23 missing")?;
            let path = vec![1, offset + 3 + property * 2];
            let occurrence = row_occurrence(
                &source(RowRole::HelpProps).origin,
                &source(RowRole::HelpProps).plain_sha256,
                path.clone(),
            );
            graph.events.push(ObservationEvent::NilAction {
                action: NilActionKind::ObservedAggregate,
                set: None,
                occurrence,
            });
            let wrapper = list(&body[offset + 3 + property * 2])?;
            let members = &list(&wrapper[2])?[2..];
            for (i, member) in members.iter().enumerate() {
                let target = uuid(atom(&list(&list(member)?[2])?[1])?)?;
                let mut occurrence_path = path.clone();
                occurrence_path.extend([2, 2 + i]);
                graph.events.push(ObservationEvent::NilAction {
                    action: NilActionKind::ObservedSetLink,
                    set: Some(target),
                    occurrence: row_occurrence(
                        &source(RowRole::HelpProps).origin,
                        &source(RowRole::HelpProps).plain_sha256,
                        occurrence_path,
                    ),
                });
            }
        }
        offset += 2 + entry.props.len() * 2;
    }
    partial(
        &mut graph,
        "source-dispatch-and-nil-insertion-time-not-identified",
        None,
        None,
    );
    Ok(graph)
}

impl RowRole {
    pub fn filename(self) -> &'static str {
        match self {
            Self::Registry => registry::REGISTRY_ROW,
            Self::TypeSets => "fe8acd6a-22c9-4b5a-aeae-232a1c8324cb.si",
            Self::HelpProps => "c4629235-4823-4320-b8b5-1d08f4c6d612.si",
        }
    }
}

/// These are exact supplied bindings, not inferred from directory names.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RowOrigin {
    pub case: String,
    pub stage: String,
    pub table: String,
    pub filename: String,
    pub part: u32,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "encoding", rename_all = "snake_case", deny_unknown_fields)]
pub enum Locator {
    Plain {
        path: PathBuf,
    },
    RawDeflate {
        path: PathBuf,
    },
    /// One closed gzip member from an append-only pack, containing raw DEFLATE.
    GzipRawDeflateRange {
        path: PathBuf,
        offset: u64,
    },
}

impl Locator {
    fn path(&self) -> &Path {
        match self {
            Self::Plain { path }
            | Self::RawDeflate { path }
            | Self::GzipRawDeflateRange { path, .. } => path,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RowBinding {
    pub role: RowRole,
    pub origin: RowOrigin,
    pub locator: Locator,
    /// Hash/length of precisely the selected file or pack member, not the whole pack.
    pub stored_length: u64,
    pub stored_sha256: String,
    /// For gzip packs, the inner raw-DEFLATE bytes also have independent custody.
    pub packed_length: u64,
    pub packed_sha256: String,
    pub plain_length: u64,
    pub plain_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestV1 {
    pub schema: String,
    pub source_head: String,
    pub case: String,
    pub stage: String,
    pub snapshot_purpose: String,
    pub registry_root_uuid: String,
    pub predecessors: Vec<FileBinding>,
    pub rows: Vec<RowBinding>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileBinding {
    pub path: PathBuf,
    pub length: u64,
    pub sha256: String,
}

impl FileBinding {
    fn verify(&self) -> Result<()> {
        physical(&self.path)?;
        ensure!(hex(&self.sha256, 64), "invalid predecessor hash");
        ensure!(
            fs::metadata(&self.path)?.len() == self.length,
            "predecessor length changed"
        );
        let mut file = fs::File::open(&self.path)?;
        let mut digest = Sha256::new();
        let mut buffer = [0u8; 8192];
        let mut length = 0u64;
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            length = length
                .checked_add(count as u64)
                .context("predecessor length overflow")?;
            digest.update(&buffer[..count]);
        }
        ensure!(
            length == self.length && format!("{:x}", digest.finalize()) == self.sha256,
            "predecessor bytes changed"
        );
        physical(&self.path)?;
        Ok(())
    }
}

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn uuid(value: &str) -> Result<String> {
    let parsed = uuid::Uuid::parse_str(value).context("invalid UUID")?;
    let canonical = parsed.hyphenated().to_string();
    ensure!(
        value.eq_ignore_ascii_case(&canonical),
        "UUID must use full hyphenated grammar"
    );
    Ok(canonical)
}

fn atom(value: &Brace) -> Result<&str> {
    value.as_atom().context("expected atom")
}

fn list(value: &Brace) -> Result<&[Brace]> {
    value.as_list().context("expected list")
}

fn count(value: &Brace) -> Result<usize> {
    let text = atom(value)?;
    ensure!(
        !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()),
        "invalid count"
    );
    text.parse().context("count is not addressable")
}

fn counted(items: &[Brace], stride: usize) -> Result<&[Brace]> {
    let (declared, rest) = items.split_first().context("missing count")?;
    let expected = count(declared)?
        .checked_mul(stride)
        .context("count multiplication overflow")?;
    ensure!(
        rest.len() == expected,
        "declared count differs from actual items"
    );
    Ok(rest)
}

/// No symlink/reparse ancestry, including a symlink to a correctly hashed file.
fn physical(path: &Path) -> Result<()> {
    ensure!(path.is_absolute(), "input path must be absolute");
    for ancestor in path.ancestors() {
        let metadata = fs::symlink_metadata(ancestor).context("input ancestor unavailable")?;
        ensure!(!metadata.file_type().is_symlink(), "linked input ancestor");
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            ensure!(
                metadata.file_attributes() & 0x400 == 0,
                "reparse input ancestor"
            );
        }
    }
    ensure!(fs::metadata(path)?.is_file(), "input is not a regular file");
    Ok(())
}

fn read_stored(binding: &RowBinding) -> Result<Vec<u8>> {
    use std::io::{Seek, SeekFrom};
    let path = binding.locator.path();
    physical(path)?;
    let mut file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    let offset = match binding.locator {
        Locator::GzipRawDeflateRange { offset, .. } => offset,
        _ => {
            ensure!(
                metadata.len() == binding.stored_length,
                "file length changed"
            );
            0
        }
    };
    let end = offset
        .checked_add(binding.stored_length)
        .context("range overflow")?;
    ensure!(end <= metadata.len(), "selected range lies outside file");
    file.seek(SeekFrom::Start(offset))?;
    let length = usize::try_from(binding.stored_length).context("range is not addressable")?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .context("cannot allocate actual selected input")?;
    file.take(binding.stored_length).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() == length && hash(&bytes) == binding.stored_sha256,
        "stored bytes changed"
    );
    physical(path)?;
    Ok(bytes)
}

/// Complete raw-DEFLATE consumption, with output growth derived from actual decoded chunks.
fn inflate(input: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = Decompress::new(false);
    let mut output = Vec::new();
    loop {
        let consumed = usize::try_from(decoder.total_in())?;
        let before_out = decoder.total_out();
        let mut chunk = [0u8; 8192];
        let flush = if consumed == input.len() {
            FlushDecompress::Finish
        } else {
            FlushDecompress::None
        };
        let status = decoder.decompress(&input[consumed..], &mut chunk, flush)?;
        let produced = usize::try_from(decoder.total_out() - before_out)?;
        output
            .try_reserve(produced)
            .context("cannot retain actual decoded bytes")?;
        output.extend_from_slice(&chunk[..produced]);
        if status == Status::StreamEnd {
            ensure!(
                decoder.total_in() == u64::try_from(input.len())?,
                "trailing DEFLATE bytes"
            );
            return Ok(output);
        }
        ensure!(
            decoder.total_in() > consumed as u64 || produced > 0,
            "incomplete DEFLATE stream"
        );
    }
}

fn decode(binding: &RowBinding, stored: &[u8]) -> Result<Vec<u8>> {
    let packed = match &binding.locator {
        Locator::GzipRawDeflateRange { .. } => {
            let mut decoder = flate2::bufread::GzDecoder::new(Cursor::new(stored));
            let mut packed = Vec::new();
            decoder.read_to_end(&mut packed)?;
            ensure!(
                decoder.into_inner().position() == stored.len() as u64,
                "trailing gzip member bytes"
            );
            packed
        }
        _ => stored.to_vec(),
    };
    ensure!(
        packed.len() as u64 == binding.packed_length && hash(&packed) == binding.packed_sha256,
        "packed binding differs"
    );
    let plain = match binding.locator {
        Locator::Plain { .. } => packed,
        _ => inflate(&packed)?,
    };
    ensure!(
        plain.len() as u64 == binding.plain_length && hash(&plain) == binding.plain_sha256,
        "plain binding differs"
    );
    Ok(plain)
}

/// Private retained bytes: subsequent filesystem drift cannot silently replace parsed input.
pub struct ProjectionInputs {
    manifest: ManifestV1,
    plain: BTreeMap<RowRole, Vec<u8>>,
}

impl ProjectionInputs {
    pub fn load(manifest: ManifestV1, expected_source_head: &str) -> Result<Self> {
        ensure!(
            manifest.schema == "cache-insertion-trace-input-v1",
            "unknown input schema"
        );
        ensure!(
            hex(expected_source_head, 40) && manifest.source_head == expected_source_head,
            "source binding differs"
        );
        for text in [&manifest.case, &manifest.stage, &manifest.snapshot_purpose] {
            ensure!(
                !text.is_empty() && !text.chars().any(char::is_control),
                "invalid snapshot identity"
            );
        }
        ensure!(
            !manifest.predecessors.is_empty(),
            "missing predecessor binding"
        );
        for predecessor in &manifest.predecessors {
            predecessor.verify()?;
        }
        ensure!(
            uuid(&manifest.registry_root_uuid)? != NIL_UUID,
            "nil registry root"
        );
        let mut plain = BTreeMap::new();
        let mut paths = BTreeSet::new();
        for binding in &manifest.rows {
            ensure!(
                binding.origin.case == manifest.case && binding.origin.stage == manifest.stage,
                "mixed case/stage rows"
            );
            ensure!(
                binding.origin.table == "Params" && binding.origin.part == 0,
                "wrong row table/part"
            );
            ensure!(
                binding.origin.filename == binding.role.filename(),
                "wrong row family filename"
            );
            ensure!(
                !binding.origin.version.is_empty()
                    && !binding.origin.version.chars().any(char::is_control),
                "missing row version"
            );
            ensure!(
                hex(&binding.stored_sha256, 64)
                    && hex(&binding.packed_sha256, 64)
                    && hex(&binding.plain_sha256, 64),
                "invalid byte hash"
            );
            let offset = match binding.locator {
                Locator::GzipRawDeflateRange { offset, .. } => offset,
                _ => 0,
            };
            physical(binding.locator.path())?;
            let physical_path = fs::canonicalize(binding.locator.path())?;
            // One physical range cannot stand in for two independently named rows.
            ensure!(
                paths.insert((physical_path, offset, binding.stored_length)),
                "aliased row range"
            );
            ensure!(!plain.contains_key(&binding.role), "duplicate row role");
            let stored = read_stored(binding)?;
            plain.insert(binding.role, decode(binding, &stored)?);
        }
        ensure!(
            plain.len() == 3
                && [RowRole::Registry, RowRole::TypeSets, RowRole::HelpProps]
                    .iter()
                    .all(|r| plain.contains_key(r)),
            "missing required row"
        );
        let inputs = Self { manifest, plain };
        inputs.verify_sources()?;
        Ok(inputs)
    }

    pub fn verify_sources(&self) -> Result<()> {
        for predecessor in &self.manifest.predecessors {
            predecessor.verify()?;
        }
        for binding in &self.manifest.rows {
            let _ = read_stored(binding)?;
        }
        Ok(())
    }

    pub fn raw(&self, role: RowRole) -> &[u8] {
        &self.plain[&role]
    }

    pub fn project(&self) -> Result<Coverage> {
        self.verify_sources()?;
        let result = coverage(
            self.raw(RowRole::Registry),
            self.raw(RowRole::TypeSets),
            self.raw(RowRole::HelpProps),
            &self.manifest.registry_root_uuid,
        )?;
        self.verify_sources()?;
        Ok(result)
    }
}

/// Structural validation precedes existing parsers' count arithmetic and UUID maps.
fn registry_preflight(tree: &Brace, declared_root: &str) -> Result<()> {
    let [version, classes, records] = list(tree)? else {
        bail!("registry outer arity");
    };
    ensure!(atom(version)? == "4", "registry version");
    let classes = counted(list(classes)?, 1)?;
    let mut class_ids = BTreeSet::new();
    for class in classes {
        let class = uuid(atom(class)?)?;
        ensure!(class != NIL_UUID, "nil registry class");
        ensure!(class_ids.insert(class), "duplicate registry class");
    }
    let records = counted(list(records)?, 7)?;
    ensure!(!records.is_empty(), "missing registry root");
    let root = uuid(declared_root)?;
    ensure!(root != NIL_UUID, "nil declared root");
    let mut ids = BTreeSet::new();
    let mut open = Vec::<String>::new();
    for (ordinal, record) in records.chunks_exact(7).enumerate() {
        let key = uuid(atom(&record[0])?)?;
        let parent = uuid(atom(&record[1])?)?;
        ensure!(
            key != NIL_UUID && ids.insert(key.clone()),
            "nil/duplicate registry record"
        );
        ensure!(
            count(&record[2])? < classes.len(),
            "registry kind outside class roster"
        );
        ensure!(
            record[3].as_str().is_some() && record[4].as_list().is_some(),
            "registry name/synonym shape"
        );
        let _ = atom(&record[5])?;
        let _ = atom(&record[6])?;
        if ordinal == 0 {
            ensure!(
                key == root && parent == NIL_UUID,
                "registry root binding differs"
            );
        } else {
            ensure!(parent != NIL_UUID && parent != key, "extra root/self owner");
            // Only an open ancestor is a valid owner in the admitted depth-first row.
            // This rejects missing/forward/cyclic owners and a reopened closed subtree.
            while open.last() != Some(&parent) && !open.is_empty() {
                open.pop();
            }
            ensure!(!open.is_empty(), "orphan/forward/nonpreorder owner");
        }
        open.push(key);
    }
    Ok(())
}

fn sets_preflight(tree: &Brace) -> Result<()> {
    let [version, body] = list(tree)? else {
        bail!("type sets outer arity");
    };
    ensure!(atom(version)? == "0", "type sets version");
    let mut keys = BTreeSet::new();
    for pair in counted(list(body)?, 2)?.chunks_exact(2) {
        let key = uuid(atom(&pair[0])?)?;
        ensure!(
            key != NIL_UUID && keys.insert(key),
            "nil/duplicate set declaration"
        );
        let (tag, members) = list(&pair[1])?.split_first().context("empty Pattern")?;
        ensure!(tag.as_str() == Some("Pattern"), "unknown set envelope");
        let mut types = BTreeSet::new();
        for member in members {
            let fields = list(member)?;
            ensure!(!fields.is_empty(), "empty type member");
            if fields[0].as_str() == Some("#") {
                ensure!(fields.len() == 2, "type reference arity");
                let id = uuid(atom(&fields[1])?)?;
                ensure!(
                    id != NIL_UUID && types.insert(id),
                    "nil/duplicate type binding"
                );
            }
            // Primitive/unknown semantic members remain whole Brace values.
            // A1.3 must establish their meaning; coverage is not a type-graph proof.
        }
    }
    Ok(())
}

fn help_preflight(tree: &Brace) -> Result<()> {
    let [version, body] = list(tree)? else {
        bail!("help outer arity");
    };
    ensure!(atom(version)? == "0", "help version");
    let (n, mut rest) = list(body)?.split_first().context("missing help count")?;
    let n = count(n)?;
    ensure!(
        n <= rest.len() / 2,
        "help count exceeds actual entry headers"
    );
    let mut keys = BTreeSet::new();
    for _ in 0..n {
        let [key, properties, tail @ ..] = rest else {
            bail!("truncated help entry");
        };
        ensure!(keys.insert(uuid(atom(key)?)?), "duplicate help key");
        let length = count(properties)?
            .checked_mul(2)
            .context("property count overflow")?;
        ensure!(length <= tail.len(), "property count exceeds actual tail");
        let mut ids = BTreeSet::new();
        for pair in tail[..length].chunks_exact(2) {
            ensure!(ids.insert(count(&pair[0])?), "duplicate property id");
        }
        rest = &tail[length..];
    }
    ensure!(rest.is_empty(), "trailing help entry items");
    Ok(())
}

fn aggregate_set_links(value: &Brace) -> Result<BTreeSet<String>> {
    let [tag, class, body] = list(value)? else {
        bail!("aggregate wrapper arity");
    };
    ensure!(
        tag.as_str() == Some("#") && uuid(atom(class)?)? == SET_LINK_LIST_CLASS,
        "unknown aggregate wrapper"
    );
    let [zero, count_token, members @ ..] = list(body)? else {
        bail!("aggregate list header");
    };
    ensure!(
        atom(zero)? == "0" && count(count_token)? == members.len(),
        "aggregate count/version"
    );
    let mut links = BTreeSet::new();
    for member in members {
        let [tag, class, target] = list(member)? else {
            bail!("set link wrapper arity");
        };
        ensure!(
            tag.as_str() == Some("#") && uuid(atom(class)?)? == facts::METADATA_REF,
            "unknown set link class"
        );
        let [one, id] = list(target)? else {
            bail!("set link target arity");
        };
        ensure!(atom(one)? == "1", "unknown set link target version");
        let id = uuid(atom(id)?)?;
        ensure!(
            id != NIL_UUID && links.insert(id),
            "nil/duplicate aggregate link"
        );
    }
    Ok(links)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyOrigin {
    MetadataRecord { record_ordinal: usize },
    TypeSet { set_ordinal: usize },
    NilAggregate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphCompleteness {
    Partial,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceStatus {
    NotIdentified,
}

pub struct Coverage {
    pub registry: SiMain,
    pub sets: TypeSets,
    pub help: HelpProps,
    /// Native cache ordinal is retained for comparison, never a traversal seed.
    pub origins: Vec<KeyOrigin>,
    pub pending_atoms: [&'static str; 5],
    pub first_visit_authority: Option<()>,
    pub graph_completeness: GraphCompleteness,
    pub trace_status: TraceStatus,
}

/// Only an independent literal byte comparison: never an expected-order generator.
/// `None` means identical whole byte slices, including BOM, CRLF and EOF.
pub fn first_raw_difference(left: &[u8], right: &[u8]) -> Option<usize> {
    left.iter()
        .zip(right)
        .position(|(a, b)| a != b)
        .or_else(|| (left.len() != right.len()).then_some(left.len().min(right.len())))
}

/// Also usable by cleanroom controls; no filesystem, planner, sorting, or rendering.
pub fn coverage(
    registry_raw: &[u8],
    sets_raw: &[u8],
    help_raw: &[u8],
    root: &str,
) -> Result<Coverage> {
    registry_preflight(&parse_row(registry_raw)?, root)?;
    sets_preflight(&parse_row(sets_raw)?)?;
    help_preflight(&parse_row(help_raw)?)?;
    let registry = si::parse(registry_raw)?;
    let sets = TypeSets::parse(sets_raw)?;
    let help = HelpProps::parse(help_raw)?;
    let metadata: BTreeMap<_, _> = registry
        .records
        .iter()
        .enumerate()
        .map(|(i, r)| (r.uuid.clone(), i))
        .collect();
    let set_keys: BTreeMap<_, _> = sets
        .sets
        .iter()
        .enumerate()
        .map(|(i, (key, _))| Ok((uuid(key)?, i)))
        .collect::<Result<_>>()?;
    ensure!(
        set_keys.keys().all(|key| !metadata.contains_key(key)),
        "metadata/set declaration collision"
    );
    let mut origins = Vec::new();
    let mut emitted_sets = BTreeSet::new();
    let mut aggregate = None;
    for entry in &help.entries {
        let key = uuid(&entry.key)?;
        let origin = if key == NIL_UUID {
            let value = entry
                .props
                .iter()
                .find(|(id, _)| id.parse::<usize>() == Ok(23))
                .context("nil aggregate lacks property 23")?;
            aggregate = Some(aggregate_set_links(&value.1)?);
            KeyOrigin::NilAggregate
        } else if let Some(&record_ordinal) = metadata.get(&key) {
            KeyOrigin::MetadataRecord { record_ordinal }
        } else if let Some(&set_ordinal) = set_keys.get(&key) {
            ensure!(
                entry
                    .props
                    .iter()
                    .any(|(id, _)| id.parse::<usize>() == Ok(23)),
                "set help entry lacks property 23"
            );
            emitted_sets.insert(key);
            KeyOrigin::TypeSet { set_ordinal }
        } else {
            bail!("unresolved help key {key}");
        };
        origins.push(origin);
    }
    ensure!(
        aggregate.context("missing nil aggregate")? == emitted_sets,
        "nil aggregate is not an exact emitted-set bijection"
    );
    Ok(Coverage {
        registry,
        sets,
        help,
        origins,
        pending_atoms: ["A1.3", "A1.4", "A1.5", "A1.6", "A1.7"],
        first_visit_authority: None,
        graph_completeness: GraphCompleteness::Partial,
        trace_status: TraceStatus::NotIdentified,
    })
}
