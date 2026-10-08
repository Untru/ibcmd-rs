//! Where the named slots and the generated types are in the owner record of a reference object.
//!
//! The layouts of `metadata_model::objects` (the base-free compiler) already say what every slot of
//! every kind holds; this walks one the way the compiler lays it out and reports the position of each
//! slot in the record (`{<tag>,...}`, position 0 is the tag).

use std::collections::HashMap;
use std::ops::Range;

use anyhow::{Context, Result, bail};

use crate::metadata_model::brace::Brace;
use crate::metadata_model::objects::{layout, parts::Slot};

/// The uuid pair of a generated type: `TypeId` and `ValueId`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedType {
    /// `Object`, `Ref`, `Selection`, `List`, `Manager`, ...
    pub category: &'static str,
    pub type_id: String,
    pub value_id: String,
}

/// The positions of one kind's record for one record version.
#[derive(Clone, Debug)]
pub struct RecordMap {
    pub names: HashMap<&'static str, usize>,
    /// `(category, position of the TypeId)` in layout order; the ValueId follows.
    pub generated: Vec<(&'static str, usize)>,
    /// The position of the header (`{0,<md base>}` or `<md base>`).
    pub header: usize,
}

/// A read-only projection of the existing private owner Slot declaration.
/// These are positional roles, not admission of the actual payload or a reference graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerSlotRole {
    RecordTag,
    GeneratedPair(&'static str),
    Header,
    WrappedHeader,
    Flag(&'static str),
    Number(&'static str),
    Code(&'static str),
    Text(&'static str),
    Localized(&'static str),
    MetadataReference(&'static str),
    MetadataReferenceList(&'static str),
    FieldReferenceList(&'static str),
    TypePattern(&'static str),
    InputModes,
    StandardAttributes,
    StandardTabularSections,
    Characteristics,
    ThisNode,
    Constant(i64),
    NilSentinel,
}

impl OwnerSlotRole {
    pub fn name(self) -> Option<&'static str> {
        match self {
            Self::Flag(name)
            | Self::Number(name)
            | Self::Code(name)
            | Self::Text(name)
            | Self::Localized(name)
            | Self::MetadataReference(name)
            | Self::MetadataReferenceList(name)
            | Self::FieldReferenceList(name)
            | Self::TypePattern(name) => Some(name),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotGate {
    Modern,
    Since8_5_1,
}

/// Recipes declared by the existing table, not a bound configuration compatibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerBranch {
    Before8_3_27,
    From8_3_27,
    From8_5_1,
}

impl OwnerBranch {
    fn flags(self) -> (bool, bool) {
        match self {
            Self::Before8_3_27 => (false, false),
            Self::From8_3_27 => (true, false),
            Self::From8_5_1 => (true, true),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotPresence {
    Present,
    NotStoredByBranch,
    MissingDeclaredValue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionConfidence {
    OwnerPositionsExact,
    AmbiguousOwnerPositions,
    IncompleteOwnerShape,
    UnknownLayout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnknownLayoutReason {
    UnknownKind,
    UnknownTag,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticPayloadStatus {
    SemanticPayloadUnchecked,
}

/// Physical positions alone never establish the configuration's compatibility context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompatibilityStatus {
    Unbound,
}

#[derive(Debug)]
pub struct OwnerSlotFact<'a> {
    schema_ordinal: usize,
    role: OwnerSlotRole,
    gates: Vec<SlotGate>,
    expected_width: usize,
    expected_range: Option<Range<usize>>,
    actual_range: Option<Range<usize>>,
    actual: &'a [Brace],
    presence: SlotPresence,
}

impl<'a> OwnerSlotFact<'a> {
    pub fn schema_ordinal(&self) -> usize {
        self.schema_ordinal
    }
    pub fn role(&self) -> OwnerSlotRole {
        self.role
    }
    pub fn gates(&self) -> &[SlotGate] {
        &self.gates
    }
    pub fn expected_width(&self) -> usize {
        self.expected_width
    }
    /// Parsed owner ordinals, not source byte offsets; may extend past a truncated record.
    pub fn expected_range(&self) -> Option<Range<usize>> {
        self.expected_range.clone()
    }
    pub fn actual_range(&self) -> Option<Range<usize>> {
        self.actual_range.clone()
    }
    pub fn actual(&self) -> &'a [Brace] {
        self.actual
    }
    pub fn presence(&self) -> SlotPresence {
        self.presence
    }

    fn positions_agree(&self, other: &Self) -> bool {
        self.schema_ordinal == other.schema_ordinal
            && self.role == other.role
            && self.gates == other.gates
            && self.expected_width == other.expected_width
            && self.expected_range == other.expected_range
            && self.actual_range == other.actual_range
            && self.presence == other.presence
    }
}

#[derive(Debug)]
pub struct OwnerBranchObservation<'a> {
    branch: OwnerBranch,
    declared_tag: i64,
    expected_width: usize,
    record: &'a [Brace],
    slots: Vec<OwnerSlotFact<'a>>,
    tail: Option<Range<usize>>,
}

impl<'a> OwnerBranchObservation<'a> {
    pub fn branch(&self) -> OwnerBranch {
        self.branch
    }
    pub fn declared_tag(&self) -> i64 {
        self.declared_tag
    }
    pub fn expected_width(&self) -> usize {
        self.expected_width
    }
    pub fn actual_width(&self) -> usize {
        self.record.len()
    }
    pub fn slots(&self) -> &[OwnerSlotFact<'a>] {
        &self.slots
    }
    pub fn extra_tail_range(&self) -> Option<Range<usize>> {
        self.tail.clone()
    }
    pub fn extra_tail(&self) -> &'a [Brace] {
        self.tail
            .as_ref()
            .map_or(&self.record[0..0], |range| &self.record[range.clone()])
    }
}

#[derive(Debug)]
pub struct RecordInspection<'a> {
    record: &'a [Brace],
    actual_tag: i64,
    observations: Vec<OwnerBranchObservation<'a>>,
    confidence: PositionConfidence,
    unknown: Option<UnknownLayoutReason>,
}

impl<'a> RecordInspection<'a> {
    pub fn record(&self) -> &'a [Brace] {
        self.record
    }
    pub fn actual_tag(&self) -> i64 {
        self.actual_tag
    }
    pub fn observations(&self) -> &[OwnerBranchObservation<'a>] {
        &self.observations
    }
    pub fn position_confidence(&self) -> PositionConfidence {
        self.confidence
    }
    pub fn unknown_reason(&self) -> Option<UnknownLayoutReason> {
        self.unknown
    }
    pub fn semantic_payload_status(&self) -> SemanticPayloadStatus {
        SemanticPayloadStatus::SemanticPayloadUnchecked
    }
    pub fn compatibility_status(&self) -> CompatibilityStatus {
        CompatibilityStatus::Unbound
    }

    /// Only fully present positions which agree across EVERY tag-matching recipe.
    /// Incomplete/tail observations are not filtered out to make a preferred recipe win.
    pub fn shared_present_slots(&self) -> impl Iterator<Item = &OwnerSlotFact<'a>> + '_ {
        self.observations
            .first()
            .into_iter()
            .flat_map(|first| first.slots.iter())
            .filter(move |fact| {
                fact.presence == SlotPresence::Present
                    && self.observations.iter().all(|branch| {
                        branch
                            .slots
                            .get(fact.schema_ordinal)
                            .is_some_and(|other| fact.positions_agree(other))
                    })
            })
    }
}

struct DeclaredSlot {
    schema_ordinal: usize,
    role: OwnerSlotRole,
    gates: Vec<SlotGate>,
    width: usize,
    range: Option<Range<usize>>,
}

/// The one declaration walk consumed by both the legacy map and the read-only inspector.
fn declared_slots(
    slots: &'static [Slot],
    modern: bool,
    since: bool,
) -> Result<(Vec<DeclaredSlot>, usize)> {
    let mut facts = Vec::new();
    let mut position = 0usize;
    for (schema_ordinal, slot) in slots.iter().enumerate() {
        let mut gates = Vec::new();
        let mut inner = slot;
        loop {
            let (gate, nested) = match inner {
                Slot::Modern(nested) => (SlotGate::Modern, *nested),
                Slot::Since8_5_1(nested) => (SlotGate::Since8_5_1, *nested),
                _ => break,
            };
            gates.try_reserve(1)?;
            gates.push(gate);
            inner = nested;
        }
        let role = match *inner {
            Slot::Tag(..) => OwnerSlotRole::RecordTag,
            Slot::Generated(category) => OwnerSlotRole::GeneratedPair(category),
            Slot::Header => OwnerSlotRole::Header,
            Slot::WrappedHeader => OwnerSlotRole::WrappedHeader,
            Slot::Flag(name) => OwnerSlotRole::Flag(name),
            Slot::Number(name) => OwnerSlotRole::Number(name),
            Slot::Code(name, _) => OwnerSlotRole::Code(name),
            Slot::Text(name) => OwnerSlotRole::Text(name),
            Slot::Localized(name) => OwnerSlotRole::Localized(name),
            Slot::Reference(name) => OwnerSlotRole::MetadataReference(name),
            Slot::References(name) => OwnerSlotRole::MetadataReferenceList(name),
            Slot::Fields(name) => OwnerSlotRole::FieldReferenceList(name),
            Slot::TypePattern(name) => OwnerSlotRole::TypePattern(name),
            Slot::InputModes => OwnerSlotRole::InputModes,
            Slot::StandardAttributes(_) => OwnerSlotRole::StandardAttributes,
            Slot::StandardTabularSections(_) => OwnerSlotRole::StandardTabularSections,
            Slot::Characteristics => OwnerSlotRole::Characteristics,
            Slot::ThisNode => OwnerSlotRole::ThisNode,
            Slot::Const(value) => OwnerSlotRole::Constant(value),
            Slot::Nil => OwnerSlotRole::NilSentinel,
            Slot::Modern(_) | Slot::Since8_5_1(_) => unreachable!("gates already unwrapped"),
        };
        let width = if matches!(role, OwnerSlotRole::GeneratedPair(_)) {
            2
        } else {
            1
        };
        let stored = gates.iter().all(|gate| match gate {
            SlotGate::Modern => modern,
            SlotGate::Since8_5_1 => since,
        });
        let range = if stored {
            let end = position
                .checked_add(width)
                .context("owner slot position overflow")?;
            let range = position..end;
            position = end;
            Some(range)
        } else {
            None
        };
        facts.try_reserve(1)?;
        facts.push(DeclaredSlot {
            schema_ordinal,
            role,
            gates,
            width,
            range,
        });
    }
    Ok((facts, position))
}

impl RecordMap {
    /// The map of `kind` for a record starting with `tag`.
    pub fn new(kind: &str, tag: i64) -> Result<Self> {
        let layout = layout(kind).with_context(|| format!("no layout for {kind}"))?;
        let Some(Slot::Tag(old, new, latest)) = layout.slots.first().copied() else {
            bail!("the {kind} layout does not start with its tag");
        };
        let modern = tag != old;
        let since_8_5_1 = tag == latest && latest != new;
        let mut map = Self {
            names: HashMap::new(),
            generated: Vec::new(),
            header: usize::MAX,
        };
        for fact in declared_slots(layout.slots, modern, since_8_5_1)?.0 {
            let Some(range) = fact.range else {
                continue;
            };
            match fact.role {
                OwnerSlotRole::GeneratedPair(category) => {
                    map.generated.push((category, range.start))
                }
                OwnerSlotRole::Header | OwnerSlotRole::WrappedHeader => {
                    if map.header == usize::MAX {
                        map.header = range.start;
                    }
                }
                role => {
                    if let Some(name) = role.name() {
                        map.names.insert(name, range.start);
                    }
                }
            }
        }
        if map.header == usize::MAX {
            bail!("the {kind} layout has no header slot");
        }
        Ok(map)
    }

    /// Inspect all canonical tag-matching owner recipes without choosing compatibility.
    /// Exact positions are not payload, Header, collection or reference-graph admission.
    pub fn inspect<'a>(kind: &str, record: &'a [Brace]) -> Result<RecordInspection<'a>> {
        inspect_layout(kind, layout(kind).map(|layout| layout.slots), record)
    }

    /// The slot named `name` of a record.
    pub fn slot<'a>(&self, record: &'a [Brace], name: &str) -> Result<&'a Brace> {
        let position = *self
            .names
            .get(name)
            .with_context(|| format!("the layout has no slot {name}"))?;
        record
            .get(position)
            .with_context(|| format!("the record is too short for {name}"))
    }

    /// The generated types of a record, in layout order.
    pub fn generated_types(&self, record: &[Brace]) -> Result<Vec<GeneratedType>> {
        self.generated
            .iter()
            .map(|(category, position)| {
                let atom = |offset: usize| {
                    record
                        .get(position + offset)
                        .and_then(Brace::as_atom)
                        .map(str::to_ascii_lowercase)
                        .with_context(|| format!("no uuid for the {category} type"))
                };
                Ok(GeneratedType {
                    category,
                    type_id: atom(0)?,
                    value_id: atom(1)?,
                })
            })
            .collect()
    }
}

fn inspect_layout<'a>(
    kind: &str,
    slots: Option<&'static [Slot]>,
    record: &'a [Brace],
) -> Result<RecordInspection<'a>> {
    let actual_tag = record
        .first()
        .and_then(Brace::as_atom)
        .and_then(|tag| tag.parse::<i64>().ok())
        .context("owner record has no integer tag")?;
    let mut inspection = RecordInspection {
        record,
        actual_tag,
        observations: Vec::new(),
        confidence: PositionConfidence::UnknownLayout,
        unknown: None,
    };
    let Some(slots) = slots else {
        inspection.unknown = Some(UnknownLayoutReason::UnknownKind);
        return Ok(inspection);
    };
    let Some(Slot::Tag(old, modern, latest)) = slots.first().copied() else {
        bail!("the {kind} layout does not start with its tag");
    };
    for (branch, declared_tag) in [
        (OwnerBranch::Before8_3_27, old),
        (OwnerBranch::From8_3_27, modern),
        (OwnerBranch::From8_5_1, latest),
    ] {
        if declared_tag != actual_tag {
            continue;
        }
        let (modern, since) = branch.flags();
        let (declared, expected_width) = declared_slots(slots, modern, since)?;
        let mut facts = Vec::new();
        facts.try_reserve(declared.len())?;
        for fact in declared {
            let (presence, actual_range, actual) = match &fact.range {
                None => (SlotPresence::NotStoredByBranch, None, &record[0..0]),
                Some(range) => {
                    let actual_range = range.start.min(record.len())..range.end.min(record.len());
                    let presence = if range.end <= record.len() {
                        SlotPresence::Present
                    } else {
                        SlotPresence::MissingDeclaredValue
                    };
                    (presence, Some(actual_range.clone()), &record[actual_range])
                }
            };
            facts.push(OwnerSlotFact {
                schema_ordinal: fact.schema_ordinal,
                role: fact.role,
                gates: fact.gates,
                expected_width: fact.width,
                expected_range: fact.range,
                actual_range,
                actual,
                presence,
            });
        }
        inspection.observations.try_reserve(1)?;
        inspection.observations.push(OwnerBranchObservation {
            branch,
            declared_tag,
            expected_width,
            record,
            slots: facts,
            tail: (expected_width < record.len()).then_some(expected_width..record.len()),
        });
    }
    let Some(first) = inspection.observations.first() else {
        inspection.unknown = Some(UnknownLayoutReason::UnknownTag);
        return Ok(inspection);
    };
    let agreement = inspection.observations.iter().all(|branch| {
        first.expected_width == branch.expected_width
            && first.slots.len() == branch.slots.len()
            && first
                .slots
                .iter()
                .zip(&branch.slots)
                .all(|(a, b)| a.positions_agree(b))
    });
    inspection.confidence = if !agreement {
        PositionConfidence::AmbiguousOwnerPositions
    } else if inspection
        .observations
        .iter()
        .any(|branch| branch.expected_width != record.len())
    {
        PositionConfidence::IncompleteOwnerShape
    } else {
        PositionConfidence::OwnerPositionsExact
    };
    Ok(inspection)
}

/// The owner record of a descriptor row `{1,<record>,<count>,<collection>...}`, and its tag.
pub fn owner_record(row: &Brace) -> Result<(&[Brace], i64)> {
    let root = row.as_list().context("a descriptor row is not a list")?;
    if root.first().and_then(Brace::as_atom) != Some("1") {
        bail!("not a descriptor row");
    }
    let record = root
        .get(1)
        .and_then(Brace::as_list)
        .context("a descriptor row has no owner record")?;
    let tag = record
        .first()
        .and_then(Brace::as_atom)
        .and_then(|tag| tag.parse().ok())
        .context("a descriptor row has no tag")?;
    Ok((record, tag))
}

/// `{3,{1,0,<uuid>},"Name",...}` -> (uuid, name).
pub fn md_base(node: &Brace) -> Result<(String, String)> {
    let items = node.as_list().context("an md header is not a list")?;
    let uuid = items
        .get(1)
        .and_then(Brace::as_list)
        .and_then(|inner| inner.get(2))
        .and_then(Brace::as_atom)
        .context("an md header has no uuid")?;
    let name = items
        .get(2)
        .and_then(Brace::as_str)
        .context("an md header has no name")?;
    Ok((uuid.to_ascii_lowercase(), name.to_owned()))
}

/// The uuid and the name of a descriptor row's object.
pub fn object_identity(row: &Brace, kind: &str) -> Result<(String, String)> {
    let (record, tag) = owner_record(row)?;
    let map = RecordMap::new(kind, tag)?;
    let header = record
        .get(map.header)
        .context("the record is too short for its header")?;
    let base = match header.as_list() {
        // `{0,<md base>}`
        Some([first, second]) if first.as_atom() == Some("0") => second,
        _ => header,
    };
    md_base(base)
}

#[cfg(test)]
mod inspection_tests {
    use super::*;

    // Synthetic private Slot recipe: exercises ambiguity which current measured family
    // owner tables do not need. It adds no family, tag or production declaration.
    static SAME_TAG_DIFFERENT_WIDTH: &[Slot] = &[
        Slot::Tag(40, 40, 40),
        Slot::Header,
        Slot::Modern(&Slot::Generated("Pair")),
        Slot::Flag("AfterPair"),
    ];

    #[test]
    fn exact_width_recipe_never_hides_other_compatible_truncated_recipe() {
        let record = [
            Brace::num(40),
            Brace::str("unvalidated header"),
            Brace::num(0),
        ];
        let before = record.clone();
        let inspection =
            inspect_layout("SyntheticTestOnly", Some(SAME_TAG_DIFFERENT_WIDTH), &record).unwrap();
        assert_eq!(
            inspection.position_confidence(),
            PositionConfidence::AmbiguousOwnerPositions
        );
        assert_eq!(inspection.observations().len(), 3);
        let old = &inspection.observations()[0];
        assert_eq!(old.expected_width(), record.len());
        assert_eq!(old.slots()[2].presence(), SlotPresence::NotStoredByBranch);
        for modern in &inspection.observations()[1..] {
            assert_eq!(modern.expected_width(), 5);
            let pair = &modern.slots()[2];
            assert_eq!(pair.presence(), SlotPresence::MissingDeclaredValue);
            assert_eq!(pair.expected_range(), Some(2..4));
            assert_eq!(pair.actual_range(), Some(2..3));
            assert_eq!(pair.actual().len(), 1);
            assert_eq!(
                modern.slots()[3].presence(),
                SlotPresence::MissingDeclaredValue
            );
        }
        assert_eq!(
            inspection
                .shared_present_slots()
                .map(|s| s.role())
                .collect::<Vec<_>>(),
            vec![OwnerSlotRole::RecordTag, OwnerSlotRole::Header]
        );
        assert_eq!(record, before);
    }

    #[test]
    fn longer_exact_recipe_never_hides_other_compatible_extra_tail() {
        let record = [
            Brace::num(40),
            Brace::str("unchecked"),
            Brace::num(1),
            Brace::num(2),
            Brace::num(0),
        ];
        let inspection =
            inspect_layout("SyntheticTestOnly", Some(SAME_TAG_DIFFERENT_WIDTH), &record).unwrap();
        assert_eq!(
            inspection.position_confidence(),
            PositionConfidence::AmbiguousOwnerPositions
        );
        let old = &inspection.observations()[0];
        assert_eq!(old.extra_tail_range(), Some(3..5));
        assert_eq!(old.extra_tail(), &record[3..]);
        for modern in &inspection.observations()[1..] {
            assert_eq!(modern.expected_width(), record.len());
            assert_eq!(modern.extra_tail_range(), None);
            assert_eq!(modern.slots()[2].presence(), SlotPresence::Present);
        }
        assert_eq!(inspection.shared_present_slots().count(), 2);
        assert_eq!(
            inspection.semantic_payload_status(),
            SemanticPayloadStatus::SemanticPayloadUnchecked
        );
        assert_eq!(
            inspection.compatibility_status(),
            CompatibilityStatus::Unbound
        );
    }
}
