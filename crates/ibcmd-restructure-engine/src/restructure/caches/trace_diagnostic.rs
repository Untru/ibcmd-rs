//! Read-only A1 observations of typed current cache inputs. No native insertion authority.
//!
//! Callers own raw-row admission/custody. This API revalidates the public typed projections,
//! records their coordinates, and never edits/render caches or admits a complete graph.
//! Descriptor compatibility and semantic payload remain unproved by `RecordMap::inspect`.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::metadata_model::brace::Brace;
use crate::mssql_config_apply::si::SiMain;

use super::{
    facts,
    help_props::HelpProps,
    root, slots,
    type_index::TypeIndex,
    type_sets::{self, TypeSets},
};

const NIL: &str = "00000000-0000-0000-0000-000000000000";
const SET_LINK_LIST: &str = "9cd510d6-abfc-11d4-9434-004095e12fc7";

/// Typed projection coordinates, not invented raw byte offsets or a source receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Coordinate {
    Registry {
        record: usize,
        span: (usize, usize),
    },
    TypeSet {
        set: usize,
        member: Option<usize>,
    },
    Help {
        entry: usize,
        property: Option<usize>,
        member: Option<usize>,
    },
    TypeIndex {
        section: usize,
        entry: usize,
        type_ordinal: usize,
    },
    Descriptor {
        row: usize,
        path: Vec<usize>,
        schema_slot: Option<usize>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyOrigin {
    Metadata { record: usize },
    TypeSet { set: usize },
    NilAggregate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyObservation {
    pub key: String,
    /// Original help ordinal; comparison only, never a traversal seed.
    pub help_ordinal: usize,
    pub origin: KeyOrigin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    Owner,
    MetadataReference,
    GeneratedTypeMember,
    NilSetMember,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    pub from: String,
    pub target: String,
    /// Resolved metadata owner for a generated TypeId, otherwise the actual target.
    pub referent: Option<String>,
    pub kind: EdgeKind,
    pub coordinate: Coordinate,
    /// The original declaration coordinate for a resolved generated TypeId.
    pub declaration: Option<Coordinate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Residue {
    pub coordinate: Coordinate,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedObservation {
    pub owner: String,
    pub type_id: String,
    pub value_id: String,
    pub coordinate: Coordinate,
    pub declaration: Option<Coordinate>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyReferenceKind {
    BareNil,
    NilListMember,
    EmptyList,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmptyReference {
    pub owner: String,
    pub coordinate: Coordinate,
    pub kind: EmptyReferenceKind,
}

/// Descriptor rows must be the same admitted current inputs; no disk fallback or adoption.
pub struct Descriptor<'a> {
    /// Ordinal in the caller's admitted descriptor/type-index row roster.
    pub row_ordinal: usize,
    pub owner: &'a str,
    pub kind: &'a str,
    pub row: &'a Brace,
}

pub struct Inputs<'a> {
    pub root: &'a str,
    pub registry: &'a SiMain,
    pub sets: &'a TypeSets,
    pub help: &'a HelpProps,
    pub type_index: Option<&'a TypeIndex>,
    pub descriptors: &'a [Descriptor<'a>],
}

/// Observations remain Partial even when all presently observed references resolve.
#[derive(Debug)]
pub struct Diagnostic {
    keys: Vec<KeyObservation>,
    edges: Vec<Edge>,
    residues: Vec<Residue>,
    generated: Vec<GeneratedObservation>,
    empty_references: Vec<EmptyReference>,
}

fn uuid(value: &str) -> Result<String> {
    ensure!(root::is_uuid(value), "invalid UUID {value}");
    Ok(value.to_ascii_lowercase())
}

fn nonnil(value: &str) -> Result<String> {
    let value = uuid(value)?;
    ensure!(value != NIL, "nil declaration");
    Ok(value)
}

fn count(node: &Brace) -> Result<usize> {
    let value = node.as_atom().context("count is not an atom")?;
    ensure!(
        !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()),
        "invalid count"
    );
    value.parse().context("count is not addressable")
}

/// Existing canonical tagged metadata-reference shape; keeps nil as an empty value.
fn metadata_target(node: &Brace) -> Result<String> {
    let [tag, class, target] = node.as_list().context("reference is not a list")? else {
        anyhow::bail!("reference wrapper arity");
    };
    ensure!(
        tag.as_str() == Some("#")
            && uuid(class.as_atom().context("reference class")?)? == facts::METADATA_REF,
        "reference class/tag"
    );
    let [version, target] = target.as_list().context("reference target is not a list")? else {
        anyhow::bail!("reference target arity");
    };
    ensure!(version.as_atom() == Some("1"), "reference target version");
    uuid(target.as_atom().context("reference target atom")?)
}

fn set_links(value: &Brace) -> Result<Vec<String>> {
    let [tag, class, body] = value.as_list().context("set aggregate list")? else {
        anyhow::bail!("set aggregate arity");
    };
    ensure!(
        tag.as_str() == Some("#")
            && uuid(class.as_atom().context("set aggregate class")?)? == SET_LINK_LIST,
        "set aggregate class/tag"
    );
    let [version, n, members @ ..] = body.as_list().context("set aggregate body")? else {
        anyhow::bail!("set aggregate body arity");
    };
    ensure!(
        version.as_atom() == Some("0") && count(n)? == members.len(),
        "set aggregate version/count"
    );
    let mut seen = BTreeSet::new();
    members
        .iter()
        .map(|member| {
            let id = metadata_target(member)?;
            ensure!(
                id != NIL && seen.insert(id.clone()),
                "nil/duplicate set aggregate member"
            );
            Ok(id)
        })
        .collect()
}

fn residue(out: &mut Diagnostic, coordinate: Coordinate, reason: impl Into<String>) {
    out.residues.push(Residue {
        coordinate,
        reason: reason.into(),
    });
}

/// Complete help-key classification/bijection, with actual row order preserved.
/// Registry records and sets which the help row does not emit are NOT invented as help keys.
pub fn inspect(input: &Inputs<'_>) -> Result<Diagnostic> {
    let root_id = nonnil(input.root)?;
    let mut metadata = BTreeMap::new();
    let mut classes = BTreeSet::new();
    for class in &input.registry.classes {
        ensure!(classes.insert(nonnil(class)?), "duplicate registry class");
    }
    let mut ancestors = Vec::new();
    for (ordinal, record) in input.registry.records.iter().enumerate() {
        let id = nonnil(&record.uuid)?;
        let parent = uuid(&record.parent)?;
        ensure!(
            record.kind < input.registry.classes.len(),
            "registry kind out of range"
        );
        ensure!(
            record.start <= record.synonyms.0
                && record.synonyms.0 <= record.synonyms.1
                && record.synonyms.1 <= record.end,
            "registry spans inconsistent"
        );
        ensure!(
            metadata.insert(id.clone(), ordinal).is_none(),
            "duplicate metadata identity"
        );
        if ordinal == 0 {
            ensure!(
                id == root_id && parent == NIL,
                "registry root identity/parent"
            );
        } else {
            while ancestors.last() != Some(&parent) && !ancestors.is_empty() {
                ancestors.pop();
            }
            ensure!(
                !ancestors.is_empty(),
                "registry owner/preorder unresolved at {ordinal}"
            );
        }
        ancestors.push(id);
    }
    ensure!(!metadata.is_empty(), "missing registry root");
    let mut sets = BTreeMap::new();
    for (ordinal, (key, _)) in input.sets.sets.iter().enumerate() {
        let id = nonnil(key)?;
        ensure!(
            !metadata.contains_key(&id),
            "ambiguous metadata/set identity"
        );
        ensure!(
            sets.insert(id, ordinal).is_none(),
            "duplicate type-set identity"
        );
    }
    let mut out = Diagnostic {
        keys: Vec::new(),
        edges: Vec::new(),
        residues: Vec::new(),
        generated: Vec::new(),
        empty_references: Vec::new(),
    };
    let mut emitted = BTreeSet::new();
    let mut emitted_sets = BTreeSet::new();
    let mut nil_links = None;
    for (ordinal, entry) in input.help.entries.iter().enumerate() {
        let key = uuid(&entry.key)?;
        ensure!(emitted.insert(key.clone()), "duplicate help identity");
        let mut properties = BTreeSet::new();
        let mut property23 = None;
        for (property, (name, value)) in entry.props.iter().enumerate() {
            ensure!(
                !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit()),
                "invalid help property id"
            );
            let id: usize = name.parse().context("help property id not addressable")?;
            ensure!(properties.insert(id), "duplicate help property id");
            if id == 23 {
                property23 = Some((property, value));
            }
        }
        let origin = if key == NIL {
            let (property, value) = property23.context("nil lacks property23")?;
            let links = set_links(value).map_err(|error| {
                anyhow::anyhow!("help entry {ordinal}, property {property}: {error}")
            })?;
            for (member, target) in links.iter().enumerate() {
                ensure!(sets.contains_key(target), "nil aggregate names foreign set");
                out.edges.push(Edge {
                    from: key.clone(),
                    target: target.clone(),
                    referent: Some(target.clone()),
                    kind: EdgeKind::NilSetMember,
                    coordinate: Coordinate::Help {
                        entry: ordinal,
                        property: Some(property),
                        member: Some(member),
                    },
                    declaration: None,
                });
            }
            nil_links = Some(links.into_iter().collect::<BTreeSet<_>>());
            KeyOrigin::NilAggregate
        } else if let Some(&record) = metadata.get(&key) {
            KeyOrigin::Metadata { record }
        } else if let Some(&set) = sets.get(&key) {
            ensure!(property23.is_some(), "emitted set lacks property23");
            // Preserve the actual property; its complete semantic grammar is not guessed.
            residue(
                &mut out,
                Coordinate::Help {
                    entry: ordinal,
                    property: property23.map(|p| p.0),
                    member: None,
                },
                "type-set property23 payload semantics remain unproved",
            );
            emitted_sets.insert(key.clone());
            KeyOrigin::TypeSet { set }
        } else {
            anyhow::bail!("unresolved help key {key} at {ordinal}");
        };
        out.keys.push(KeyObservation {
            key,
            help_ordinal: ordinal,
            origin,
        });
    }
    ensure!(
        nil_links.context("missing nil aggregate")? == emitted_sets,
        "nil/emitted-set bijection differs"
    );

    for (ordinal, record) in input.registry.records.iter().enumerate().skip(1) {
        out.edges.push(Edge {
            from: uuid(&record.parent)?,
            target: uuid(&record.uuid)?,
            referent: Some(uuid(&record.uuid)?),
            kind: EdgeKind::Owner,
            coordinate: Coordinate::Registry {
                record: ordinal,
                span: (record.start, record.end),
            },
            declaration: None,
        });
    }
    let mut types = BTreeMap::new();
    let mut owners = BTreeSet::new();
    let mut values = BTreeSet::new();
    if let Some(index) = input.type_index {
        let mut sections = BTreeSet::new();
        for (section, body) in index.sections.iter().enumerate() {
            let class = nonnil(&body.class)?;
            ensure!(
                sections.insert(class.clone()),
                "duplicate type-index section"
            );
            for (entry, declaration) in body.entries.iter().enumerate() {
                let owner = nonnil(&declaration.object)?;
                ensure!(owners.insert(owner.clone()), "duplicate type-index owner");
                let ordinal = *metadata
                    .get(&owner)
                    .context("type-index owner unresolved")?;
                ensure!(
                    uuid(&input.registry.classes[input.registry.records[ordinal].kind])? == class,
                    "type-index owner class mismatch"
                );
                let mut categories = BTreeSet::new();
                for (category, slot) in declaration.types.iter().enumerate() {
                    let id = nonnil(&slot.type_id)?;
                    ensure!(categories.insert(slot.index), "duplicate type category");
                    ensure!(
                        values.insert(nonnil(&slot.value_id)?),
                        "duplicate generated ValueId"
                    );
                    ensure!(
                        types
                            .insert(
                                id,
                                (
                                    owner.clone(),
                                    nonnil(&slot.value_id)?,
                                    Coordinate::TypeIndex {
                                        section,
                                        entry,
                                        type_ordinal: category
                                    }
                                )
                            )
                            .is_none(),
                        "ambiguous generated TypeId"
                    );
                }
            }
        }
    }
    for (set, (key, members)) in input.sets.sets.iter().enumerate() {
        let mut seen = BTreeSet::new();
        for (member, value) in members.iter().enumerate() {
            let coordinate = Coordinate::TypeSet {
                set,
                member: Some(member),
            };
            if let Some(id) = type_sets::member_id(value) {
                let id = nonnil(id)?;
                ensure!(
                    seen.insert(id.clone()),
                    "duplicate type-set reference member"
                );
                let resolved = types.get(&id);
                if resolved.is_none() {
                    residue(
                        &mut out,
                        coordinate.clone(),
                        format!("generated TypeId {id} has no supplied declaration"),
                    );
                }
                out.edges.push(Edge {
                    from: uuid(key)?,
                    target: id,
                    referent: resolved.map(|(owner, _, _)| owner.clone()),
                    kind: EdgeKind::GeneratedTypeMember,
                    coordinate,
                    declaration: resolved.map(|(_, _, at)| at.clone()),
                });
            } else {
                residue(
                    &mut out,
                    coordinate,
                    "non-reference Pattern member: primitive/payload semantics not admitted",
                );
            }
        }
    }
    let mut descriptor_owners = BTreeSet::new();
    let mut descriptor_rows = BTreeSet::new();
    let mut descriptor_types = BTreeSet::new();
    let mut descriptor_values = BTreeSet::new();
    for descriptor in input.descriptors {
        let row = descriptor.row_ordinal;
        ensure!(
            descriptor_rows.insert(row),
            "duplicate descriptor row coordinate"
        );
        let owner = nonnil(descriptor.owner)?;
        ensure!(
            descriptor_owners.insert(owner.clone()),
            "duplicate descriptor owner"
        );
        let ordinal = *metadata
            .get(&owner)
            .context("descriptor owner unresolved")?;
        let class = uuid(&input.registry.classes[input.registry.records[ordinal].kind])?;
        ensure!(
            root::kind_of_class(&class) == Some(descriptor.kind),
            "descriptor family not registry authority"
        );
        let [version, _, n, collections @ ..] = descriptor
            .row
            .as_list()
            .context("descriptor list missing")?
        else {
            anyhow::bail!("descriptor outer arity")
        };
        ensure!(
            version.as_atom() == Some("1") && count(n)? == collections.len(),
            "descriptor version/collection count"
        );
        let mut collection_classes = BTreeSet::new();
        for collection in collections {
            let [class, n, children @ ..] =
                collection.as_list().context("descriptor collection list")?
            else {
                anyhow::bail!("descriptor collection arity")
            };
            ensure!(
                collection_classes.insert(nonnil(
                    class.as_atom().context("descriptor collection class")?
                )?),
                "duplicate descriptor collection"
            );
            ensure!(count(n)? == children.len(), "descriptor child count");
        }
        let (record, _) = slots::owner_record(descriptor.row)?;
        let inspected = slots::RecordMap::inspect(descriptor.kind, record)?;
        let at = Coordinate::Descriptor {
            row,
            path: vec![1],
            schema_slot: None,
        };
        residue(
            &mut out,
            at.clone(),
            format!(
                "position {:?}; semantic payload unchecked; compatibility unbound",
                inspected.position_confidence()
            ),
        );
        let Some(header_slot) = inspected.shared_present_slots().find(|slot| {
            matches!(
                slot.role(),
                slots::OwnerSlotRole::Header | slots::OwnerSlotRole::WrappedHeader
            )
        }) else {
            residue(
                &mut out,
                at,
                "own Header coordinate is not shared by every matching recipe",
            );
            continue;
        };
        let mut header = header_slot
            .actual()
            .first()
            .context("descriptor Header missing")?;
        if header_slot.role() == slots::OwnerSlotRole::WrappedHeader {
            let [zero, inner] = header.as_list().context("descriptor Header wrapper")? else {
                anyhow::bail!("descriptor Header wrapper arity")
            };
            ensure!(
                zero.as_atom() == Some("0"),
                "descriptor Header wrapper version"
            );
            header = inner;
        }
        let [tag, identity, name, ..] = header.as_list().context("descriptor own Header list")?
        else {
            anyhow::bail!("descriptor own Header arity")
        };
        let [one, zero, id] = identity.as_list().context("descriptor identity list")? else {
            anyhow::bail!("descriptor own identity arity")
        };
        ensure!(
            tag.as_atom() == Some("3")
                && one.as_atom() == Some("1")
                && zero.as_atom() == Some("0")
                && uuid(id.as_atom().context("descriptor identity UUID")?)? == owner
                && name.as_str() == Some(input.registry.records[ordinal].name.as_str()),
            "descriptor own identity/name differs"
        );
        let mut actual_generated = BTreeSet::new();
        for slot in inspected.shared_present_slots() {
            let Some(range) = slot.actual_range() else {
                continue;
            };
            let at = Coordinate::Descriptor {
                row,
                path: vec![1, range.start],
                schema_slot: Some(slot.schema_ordinal()),
            };
            let source_coordinate = at.clone();
            (|| -> Result<()> {
            match slot.role() {
                slots::OwnerSlotRole::GeneratedPair(_) => {
                    let [type_id, value_id] = slot.actual() else {
                        anyhow::bail!("generated pair width")
                    };
                    let type_id = nonnil(type_id.as_atom().context("generated TypeId atom")?)?;
                    let value_id = nonnil(value_id.as_atom().context("generated ValueId atom")?)?;
                    ensure!(
                        actual_generated.insert(type_id.clone()),
                        "duplicate descriptor generated TypeId"
                    );
                    ensure!(
                        descriptor_types.insert(type_id.clone())
                            && descriptor_values.insert(value_id.clone()),
                        "duplicate generated declaration across descriptors"
                    );
                    let declaration = types.get(&type_id);
                    if let Some((declared_owner, declared_value, _)) = declaration {
                        ensure!(
                            declared_owner == &owner && declared_value == &value_id,
                            "descriptor/type-index generated ownership differs"
                        );
                    } else {
                        residue(
                            &mut out,
                            at.clone(),
                            format!(
                                "descriptor TypeId {type_id} lacks supplied type-index declaration"
                            ),
                        );
                    }
                    out.generated.push(GeneratedObservation {
                        owner: owner.clone(),
                        type_id,
                        value_id,
                        coordinate: at,
                        declaration: declaration.map(|(_, _, at)| at.clone()),
                    });
                }
                slots::OwnerSlotRole::MetadataReference(_) => {
                    let id = slot
                        .actual()
                        .first()
                        .and_then(Brace::as_atom)
                        .context("bare reference atom missing")?;
                    descriptor_edge(
                        &mut out,
                        &metadata,
                        &owner,
                        uuid(id)?,
                        at,
                        EmptyReferenceKind::BareNil,
                    );
                }
                slots::OwnerSlotRole::MetadataReferenceList(_) => {
                    let node = slot.actual().first().context("reference list missing")?;
                    let [version, n, members @ ..] =
                        node.as_list().context("reference list shape")?
                    else {
                        anyhow::bail!("reference list arity")
                    };
                    ensure!(
                        version.as_atom() == Some("0") && count(n)? == members.len(),
                        "reference list version/count"
                    );
                    if members.is_empty() {
                        out.empty_references.push(EmptyReference {
                            owner: owner.clone(),
                            coordinate: at.clone(),
                            kind: EmptyReferenceKind::EmptyList,
                        });
                    }
                    for (member, value) in members.iter().enumerate() {
                        let mut path = vec![1, range.start];
                        path.push(
                            member
                                .checked_add(2)
                                .context("reference ordinal overflow")?,
                        );
                        descriptor_edge(
                            &mut out,
                            &metadata,
                            &owner,
                            metadata_target(value)?,
                            Coordinate::Descriptor {
                                row,
                                path,
                                schema_slot: Some(slot.schema_ordinal()),
                            },
                            EmptyReferenceKind::NilListMember,
                        );
                    }
                }
                slots::OwnerSlotRole::FieldReferenceList(_)
                | slots::OwnerSlotRole::TypePattern(_)
                | slots::OwnerSlotRole::Characteristics
                | slots::OwnerSlotRole::StandardAttributes
                | slots::OwnerSlotRole::StandardTabularSections => residue(
                    &mut out,
                    at,
                    format!("{:?} reference semantics not established", slot.role()),
                ),
                _ => {}
            }
            Ok(())
            })().map_err(|error| anyhow::anyhow!("descriptor coordinate {source_coordinate:?}: {error}"))?;
        }
        for (type_id, (declared_owner, _, coordinate)) in &types {
            if declared_owner == &owner && !actual_generated.contains(type_id) {
                residue(
                    &mut out,
                    coordinate.clone(),
                    format!(
                        "TypeId {type_id} has no shared present descriptor pair; layout position unproved"
                    ),
                );
            }
        }
    }
    for (owner, &ordinal) in &metadata {
        if !descriptor_owners.contains(owner) {
            let record = &input.registry.records[ordinal];
            residue(
                &mut out,
                Coordinate::Registry {
                    record: ordinal,
                    span: (record.start, record.end),
                },
                format!(
                    "admitted descriptor facts absent for {owner}; outgoing references unproved"
                ),
            );
        }
    }
    Ok(out)
}

fn descriptor_edge(
    out: &mut Diagnostic,
    metadata: &BTreeMap<String, usize>,
    owner: &str,
    target: String,
    coordinate: Coordinate,
    empty_kind: EmptyReferenceKind,
) {
    // Nil in a reference slot is an empty value, NOT the synthetic nil-aggregate action.
    if target == NIL {
        out.empty_references.push(EmptyReference {
            owner: owner.to_owned(),
            coordinate,
            kind: empty_kind,
        });
        return;
    }
    let referent = metadata.contains_key(&target).then(|| target.clone());
    if referent.is_none() {
        residue(
            out,
            coordinate.clone(),
            format!("unresolved metadata referent {target}"),
        );
    }
    out.edges.push(Edge {
        from: owner.to_owned(),
        target,
        referent,
        kind: EdgeKind::MetadataReference,
        coordinate,
        declaration: None,
    });
}

impl Diagnostic {
    pub fn keys(&self) -> &[KeyObservation] {
        &self.keys
    }
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }
    pub fn residues(&self) -> &[Residue] {
        &self.residues
    }
    pub fn generated(&self) -> &[GeneratedObservation] {
        &self.generated
    }
    pub fn empty_references(&self) -> &[EmptyReference] {
        &self.empty_references
    }
    pub fn first_visit_authority(&self) -> Option<()> {
        None
    }
    /// This observation API does not manufacture a traversal from incomplete descriptor facts.
    /// Explicit untrusted experiments can use the existing source-bound comparison protocol.
    pub fn insertion_candidate(&self) -> Option<&[String]> {
        None
    }
}
