//! What the own restructuring (S1, issue #391) may take of a verdict.
//!
//! The check says *what changed* (reasons with a rule, a kind, a path and an
//! operation) and whether the platform's own apply is needed. S1 is the small
//! set of restructurings the own apply does itself; this module maps every
//! reason of a verdict onto one of its operations or onto a refusal that
//! names why, and it does so from the typed fields of [`Reason`], never from
//! the wording of `change`. Nothing that is not an operation of S1 passes:
//! a reason no case below recognises is refused, so is a verdict the check
//! could not finish.
//!
//! S1, on catalogs and documents only:
//!
//! | operation | the reason |
//! |---|---|
//! | add an attribute | `column-added-or-dropped`, `ChildObjects/Attribute[A]`, added |
//! | delete an attribute | the same, removed |
//! | widen a string | `attribute-property-not-covered`, `.../Properties/Type/StringQualifiers/Length`, `n -> m`, `m > n` |
//! | switch the index | `attribute-property-not-covered`, `.../Properties/Indexing`, `DontIndex <-> Index` or `DontIndex <-> IndexWithAdditionalOrder` |
//! | add a tabular section | `tabular-section-added-dropped-moved`, `ChildObjects/TabularSection[T]`, added |
//! | add an attribute to a tabular section | `tabular-section-column-added-dropped-moved`, `ChildObjects/TabularSection[T]/ChildObjects/Attribute[A]`, added |
//! | add a plain object | `object-with-storage-added-or-dropped` on the object, added, **and** the same rule on `Configuration`, `ChildObjects/<Kind>[Name]`, added |
//!
//! An operation says what the descriptors changed by. It does not vouch for
//! what the reason cannot see: that the string being widened is a
//! variable-length one (`AllowedLength` is not part of the reason), that a
//! new object has no predefined items or hierarchy, that an attribute added
//! has a primitive type. The plan of the restructuring reads the same
//! descriptors and refuses what it does not build.
//!
//! The classification is deliberately independent of the restructuring's own
//! code (`src/restructure`): the gate there consumes [`classify`] and
//! [`Classification`], and adds its own refusals on top.

use serde::Serialize;

use super::model::{Reason, ReasonClass, Verdict};
use super::rule_id::RuleId;
use super::tree_diff::ChangeOp;

/// The kinds S1 restructures.
const S1_KINDS: [&str; 2] = ["Catalog", "Document"];

/// A top-level object: `Catalog` and `_ДемоПартнеры`, and the Config row (or
/// the file of a tree) that holds its descriptor.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct ObjectId {
    pub kind: String,
    pub name: String,
    /// The `FileName` of the descriptor row (the object's uuid), as the
    /// reason gave it.
    pub row: String,
}

impl ObjectId {
    /// `Catalog._ДемоПартнеры`
    pub fn full_name(&self) -> String {
        format!("{}.{}", self.kind, self.name)
    }

    fn same_object(&self, other: &ObjectId) -> bool {
        self.kind == other.kind && self.name == other.name
    }
}

/// The values of an attribute's `Indexing` that S1 switches. `DontIndex` goes to either of the other two
/// and back; the other two do not go to one another (not traced on the platform).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum IndexMode {
    DontIndex,
    Index,
    IndexWithAdditionalOrder,
}

impl IndexMode {
    fn parse(text: &str) -> Option<Self> {
        match text {
            "DontIndex" => Some(Self::DontIndex),
            "Index" => Some(Self::Index),
            "IndexWithAdditionalOrder" => Some(Self::IndexWithAdditionalOrder),
            _ => None,
        }
    }
}

/// One operation of S1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "operation", rename_all = "kebab-case")]
pub enum S1Operation {
    AddAttribute {
        object: ObjectId,
        attribute: String,
    },
    DeleteAttribute {
        object: ObjectId,
        attribute: String,
    },
    /// The `Length` of a string attribute grew.
    WidenString {
        object: ObjectId,
        attribute: String,
        from: u32,
        to: u32,
    },
    SwitchIndex {
        object: ObjectId,
        attribute: String,
        from: IndexMode,
        to: IndexMode,
    },
    AddTabularSection {
        object: ObjectId,
        section: String,
    },
    /// A new attribute in a tabular section the object had.
    AddSectionAttribute {
        object: ObjectId,
        section: String,
        attribute: String,
    },
    /// A new catalog or document, and the configuration's listing of it.
    AddObject {
        object: ObjectId,
    },
}

impl S1Operation {
    /// The stable name of the operation.
    pub fn name(&self) -> &'static str {
        match self {
            Self::AddAttribute { .. } => "add-attribute",
            Self::DeleteAttribute { .. } => "delete-attribute",
            Self::WidenString { .. } => "widen-string",
            Self::SwitchIndex { .. } => "switch-index",
            Self::AddTabularSection { .. } => "add-tabular-section",
            Self::AddSectionAttribute { .. } => "add-section-attribute",
            Self::AddObject { .. } => "add-object",
        }
    }

    /// The object the operation is on.
    pub fn object(&self) -> &ObjectId {
        match self {
            Self::AddAttribute { object, .. }
            | Self::DeleteAttribute { object, .. }
            | Self::WidenString { object, .. }
            | Self::SwitchIndex { object, .. }
            | Self::AddTabularSection { object, .. }
            | Self::AddSectionAttribute { object, .. }
            | Self::AddObject { object } => object,
        }
    }
}

macro_rules! refusal_codes {
    ($($(#[$doc:meta])* $variant:ident => $id:literal, $text:literal;)*) => {
        /// Why a reason is not an operation of S1. The code is stable; the
        /// text is for people.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum RefusalCode {
            $($(#[$doc])* $variant,)*
        }

        impl RefusalCode {
            pub const ALL: &'static [RefusalCode] = &[$(RefusalCode::$variant),*];

            pub fn id(self) -> &'static str {
                match self {
                    $(RefusalCode::$variant => $id,)*
                }
            }

            pub fn text(self) -> &'static str {
                match self {
                    $(RefusalCode::$variant => $text,)*
                }
            }
        }
    };
}

refusal_codes! {
    RuleUnspecified => "rule-unspecified", "the reason carries no rule";
    DataChange => "data-change", "the change is to data the platform derives from the configuration";
    RowUnexplained => "row-unexplained", "a row differs but both sides decode to the same XML, and nothing proves the difference harmless";
    RowFormatUnproven => "row-format-unproven", "a row differs by more than the record format of the staging platform";
    UnknownStep => "unknown-step", "the check could not judge a row or a file";
    ServiceRowChanged => "service-row-changed", "the root or version row of the configuration changed";
    BodyDataOutsideS1 => "body-data-outside-s1", "a body row that carries structure or data changed";
    KindOutsideS1 => "kind-outside-s1", "the object is not a catalog or a document";
    ConfigurationOutsideS1 => "configuration-outside-s1", "the configuration row changed in a way that is not the listing of a new object";
    NestedObjectOutsideS1 => "nested-object-outside-s1", "the change is inside an object nested in a catalog or a document";
    PropertyOutsideS1 => "property-outside-s1", "a property of the object changed (S1 changes no property of an object)";
    ChildOutsideS1 => "child-outside-s1", "a child object other than an attribute or a tabular section changed";
    AttributePropertyOutsideS1 => "attribute-property-outside-s1", "a property of an attribute changed that S1 does not switch (its type, its use, its history)";
    AttributeMoved => "attribute-moved", "the attributes changed order";
    LengthNotWidened => "length-not-widened", "the length of a string is not a number that grew";
    IndexModeOutsideS1 => "index-mode-outside-s1", "the index of an attribute changed between Index and IndexWithAdditionalOrder, or to or from a value that is none of the three modes";
    TabularSectionOutsideS1 => "tabular-section-outside-s1", "a tabular section was dropped or moved, or changed inside but for a new attribute";
    ObjectRemoved => "object-removed", "an object with tables was dropped";
    ObjectListMismatch => "object-list-mismatch", "a new object and the configuration's listing of it do not match";
    IncompleteVerdict => "incomplete-verdict", "the check did not compare every file that carries data";
    StructureOutsideS1 => "structure-outside-s1", "a structural change no operation of S1 covers";
}

/// A reason S1 does not take, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Refusal {
    pub code: RefusalCode,
    /// The rule of the reason (`RuleId::Unspecified` for a refusal of the
    /// whole verdict).
    pub rule: RuleId,
    /// `Reason::summary` of the reason; empty for a refusal of the verdict.
    pub reason: String,
    /// What in the reason decided it (the property, the values).
    pub detail: String,
}

impl Refusal {
    /// `structure-outside-s1: Catalog.X: ChildObjects/Attribute[A]/...: ... (detail)`
    pub fn message(&self) -> String {
        let mut text = self.code.id().to_string();
        if !self.reason.is_empty() {
            text.push_str(": ");
            text.push_str(&self.reason);
        }
        if !self.detail.is_empty() {
            text.push_str(" [");
            text.push_str(&self.detail);
            text.push(']');
        }
        text
    }
}

impl Serialize for RefusalCode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.id())
    }
}

/// What S1 makes of a verdict.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Classification {
    /// The operations, in the order of the reasons.
    pub operations: Vec<S1Operation>,
    /// One per reason S1 does not take; any of them refuses the stage.
    pub refusals: Vec<Refusal>,
}

impl Classification {
    /// Whether S1 may take the whole stage: no reason is left over. A stage
    /// with no operations (the check found nothing that needs the platform)
    /// is accepted too; the caller has its own plain path for it.
    pub fn accepted(&self) -> bool {
        self.refusals.is_empty()
    }

    /// The operations grouped by the object they are on, in the order the
    /// objects first appear; an object is told apart by its descriptor row
    /// (case aside).
    pub fn by_object(&self) -> Vec<(&ObjectId, Vec<&S1Operation>)> {
        let mut groups: Vec<(&ObjectId, Vec<&S1Operation>)> = Vec::new();
        for operation in &self.operations {
            let object = operation.object();
            match groups
                .iter_mut()
                .find(|(known, _)| known.row.eq_ignore_ascii_case(&object.row))
            {
                Some((_, operations)) => operations.push(operation),
                None => groups.push((object, vec![operation])),
            }
        }
        groups
    }

    /// What `mssql-apply-check --s1` prints after the verdict.
    pub fn render_text(&self) -> String {
        let mut out = String::new();
        if self.accepted() {
            out.push_str(&format!(
                "S1 принимает всё: операций {}.\n",
                self.operations.len()
            ));
        } else {
            out.push_str(&format!(
                "S1 отказывает: причин {}, принято операций {}.\n",
                self.refusals.len(),
                self.operations.len()
            ));
        }
        for operation in &self.operations {
            out.push_str(&format!(
                "  {}: {}\n",
                operation.name(),
                operation.object().full_name()
            ));
        }
        for refusal in &self.refusals {
            out.push_str(&format!("  отказ {}\n", refusal.message()));
        }
        out
    }

    /// The refusals as one message, or `None` when the stage is accepted.
    pub fn refusal_message(&self) -> Option<String> {
        if self.refusals.is_empty() {
            return None;
        }
        const SHOWN: usize = 4;
        let mut parts = self
            .refusals
            .iter()
            .take(SHOWN)
            .map(Refusal::message)
            .collect::<Vec<_>>();
        if self.refusals.len() > SHOWN {
            parts.push(format!("and {} more", self.refusals.len() - SHOWN));
        }
        Some(parts.join("; "))
    }
}

/// Maps every reason of `verdict` onto an operation of S1 or a refusal.
pub fn classify(verdict: &Verdict) -> Classification {
    let mut out = Classification::default();
    if !verdict.is_conclusive() {
        out.refusals.push(Refusal {
            code: RefusalCode::IncompleteVerdict,
            rule: RuleId::Unspecified,
            reason: String::new(),
            detail: format!(
                "{} files with data were not compared",
                verdict.stats.body_files_not_compared
            ),
        });
    }

    // A new object is two reasons that must be one another's pair.
    let mut new_objects = Vec::<(ObjectId, &Reason)>::new();
    let mut listings = Vec::<(ObjectId, &Reason)>::new();

    for reason in &verdict.reasons {
        match one(reason) {
            Taken::Operation(operation) => out.operations.push(operation),
            Taken::NewObject(object) => new_objects.push((object, reason)),
            Taken::Listing(object) => listings.push((object, reason)),
            Taken::Refused(refusal) => out.refusals.push(refusal),
        }
    }

    for (object, reason) in &new_objects {
        match listings
            .iter()
            .position(|(listed, _)| listed.same_object(object))
        {
            Some(index) => {
                listings.remove(index);
                out.operations.push(S1Operation::AddObject {
                    object: object.clone(),
                });
            }
            None => out.refusals.push(refuse(
                RefusalCode::ObjectListMismatch,
                reason,
                "the configuration does not list the new object",
            )),
        }
    }
    for (object, reason) in listings {
        out.refusals.push(refuse(
            RefusalCode::ObjectListMismatch,
            reason,
            &format!("no descriptor adds {}", object.full_name()),
        ));
    }
    out
}

/// What one reason is.
enum Taken {
    Operation(S1Operation),
    /// The descriptor of a new catalog or document (waits for its listing).
    NewObject(ObjectId),
    /// The configuration's listing of a new catalog or document.
    Listing(ObjectId),
    Refused(Refusal),
}

fn refuse(code: RefusalCode, reason: &Reason, detail: &str) -> Refusal {
    Refusal {
        code,
        rule: reason.rule,
        reason: reason.summary(),
        detail: detail.to_string(),
    }
}

/// The object a reason is on, when it is a top-level catalog or document:
/// `Catalog._ДемоПартнеры` of kind `Catalog`.
fn top_level_object(reason: &Reason) -> Option<ObjectId> {
    if !S1_KINDS.contains(&reason.kind.as_str()) {
        return None;
    }
    let name = reason.object.strip_prefix(&format!("{}.", reason.kind))?;
    (!name.is_empty() && !name.contains('.')).then(|| ObjectId {
        kind: reason.kind.clone(),
        name: name.to_string(),
        row: reason.file_name.clone(),
    })
}

/// `Some(label)` when `reason.path` is exactly the steps `names`, and the
/// step at `labelled` has a label (the child's name).
fn labelled_path<'a>(reason: &'a Reason, names: &[&str], labelled: usize) -> Option<&'a str> {
    if reason.path.len() != names.len()
        || reason
            .path
            .iter()
            .zip(names)
            .any(|(seg, name)| seg.name != *name)
    {
        return None;
    }
    reason.path.get(labelled)?.label.as_deref()
}

fn one(reason: &Reason) -> Taken {
    match reason.class {
        ReasonClass::Unknown => return Taken::Refused(refuse_unknown(reason)),
        ReasonClass::Data => {
            return Taken::Refused(refuse(RefusalCode::DataChange, reason, reason.rule.text()));
        }
        ReasonClass::Structure => {}
    }
    if reason.rule == RuleId::Unspecified {
        return Taken::Refused(refuse(RefusalCode::RuleUnspecified, reason, ""));
    }
    match reason.rule {
        RuleId::RootRowChanged | RuleId::VersionRowChanged => {
            return Taken::Refused(refuse(
                RefusalCode::ServiceRowChanged,
                reason,
                reason.rule.text(),
            ));
        }
        RuleId::BodyContentChanged | RuleId::BodyRowAdded | RuleId::BodyRowRemoved => {
            return Taken::Refused(refuse(
                RefusalCode::BodyDataOutsideS1,
                reason,
                reason.rule.text(),
            ));
        }
        _ => {}
    }
    if reason.kind == "Configuration" {
        return configuration(reason);
    }
    if !S1_KINDS.contains(&reason.kind.as_str()) {
        return Taken::Refused(refuse(
            RefusalCode::KindOutsideS1,
            reason,
            &format!("kind {}", reason.kind),
        ));
    }
    let Some(object) = top_level_object(reason) else {
        return Taken::Refused(refuse(RefusalCode::NestedObjectOutsideS1, reason, ""));
    };
    match reason.rule {
        RuleId::ObjectWithStorageAddedOrDropped => match (&reason.op, reason.path.is_empty()) {
            (Some(ChangeOp::Added), true) => Taken::NewObject(object),
            _ => Taken::Refused(refuse(RefusalCode::ObjectRemoved, reason, "")),
        },
        RuleId::ColumnAddedOrDropped => {
            let Some(attribute) = labelled_path(reason, &["ChildObjects", "Attribute"], 1) else {
                return Taken::Refused(refuse(
                    RefusalCode::ChildOutsideS1,
                    reason,
                    &reason.property,
                ));
            };
            match reason.op {
                Some(ChangeOp::Added) => Taken::Operation(S1Operation::AddAttribute {
                    object,
                    attribute: attribute.to_string(),
                }),
                Some(ChangeOp::Removed) => Taken::Operation(S1Operation::DeleteAttribute {
                    object,
                    attribute: attribute.to_string(),
                }),
                _ => Taken::Refused(refuse(RefusalCode::AttributeMoved, reason, "")),
            }
        }
        RuleId::ColumnsReordered => Taken::Refused(refuse(RefusalCode::AttributeMoved, reason, "")),
        RuleId::AttributePropertyNotCovered => attribute_property(reason, object),
        RuleId::TabularSectionAddedDroppedMoved => {
            match (
                labelled_path(reason, &["ChildObjects", "TabularSection"], 1),
                &reason.op,
            ) {
                (Some(section), Some(ChangeOp::Added)) => {
                    Taken::Operation(S1Operation::AddTabularSection {
                        object,
                        section: section.to_string(),
                    })
                }
                _ => Taken::Refused(refuse(
                    RefusalCode::TabularSectionOutsideS1,
                    reason,
                    &reason.property,
                )),
            }
        }
        RuleId::TabularSectionColumnAddedDroppedMoved => {
            let names = [
                "ChildObjects",
                "TabularSection",
                "ChildObjects",
                "Attribute",
            ];
            match (
                labelled_path(reason, &names, 1),
                labelled_path(reason, &names, 3),
                &reason.op,
            ) {
                (Some(section), Some(attribute), Some(ChangeOp::Added)) => {
                    Taken::Operation(S1Operation::AddSectionAttribute {
                        object,
                        section: section.to_string(),
                        attribute: attribute.to_string(),
                    })
                }
                _ => Taken::Refused(refuse(
                    RefusalCode::TabularSectionOutsideS1,
                    reason,
                    &reason.property,
                )),
            }
        }
        RuleId::TabularSectionPropertyNotCovered
        | RuleId::TabularSectionStandardAttributeNotCovered
        | RuleId::TabularSectionAttributePropertyNotCovered
        | RuleId::TabularSectionPartNotCovered => Taken::Refused(refuse(
            RefusalCode::TabularSectionOutsideS1,
            reason,
            &reason.property,
        )),
        RuleId::PropertyNotCovered
        | RuleId::PropertiesChanged
        | RuleId::StandardAttributePropertyNotCovered => Taken::Refused(refuse(
            RefusalCode::PropertyOutsideS1,
            reason,
            &reason.property,
        )),
        RuleId::AttributePartNotCovered => Taken::Refused(refuse(
            RefusalCode::AttributePropertyOutsideS1,
            reason,
            &reason.property,
        )),
        RuleId::ChildObjectsChanged
        | RuleId::ChildKindNotCovered
        | RuleId::GeneratedTypesChanged
        | RuleId::DescriptorPartNotCovered
        | RuleId::ObjectChanged => Taken::Refused(refuse(
            RefusalCode::ChildOutsideS1,
            reason,
            &reason.property,
        )),
        _ => Taken::Refused(refuse(
            RefusalCode::StructureOutsideS1,
            reason,
            reason.rule.id(),
        )),
    }
}

/// A property of an attribute of a catalog or a document.
fn attribute_property(reason: &Reason, object: ObjectId) -> Taken {
    if let Some(attribute) = labelled_path(
        reason,
        &[
            "ChildObjects",
            "Attribute",
            "Properties",
            "Type",
            "StringQualifiers",
            "Length",
        ],
        1,
    ) {
        let lengths = match &reason.op {
            Some(ChangeOp::Modified { old, new }) => old
                .trim()
                .parse::<u32>()
                .ok()
                .zip(new.trim().parse::<u32>().ok()),
            _ => None,
        };
        return match lengths {
            Some((from, to)) if to > from => Taken::Operation(S1Operation::WidenString {
                object,
                attribute: attribute.to_string(),
                from,
                to,
            }),
            _ => Taken::Refused(refuse(
                RefusalCode::LengthNotWidened,
                reason,
                &reason.change,
            )),
        };
    }
    if let Some(attribute) = labelled_path(
        reason,
        &["ChildObjects", "Attribute", "Properties", "Indexing"],
        1,
    ) {
        let modes = match &reason.op {
            Some(ChangeOp::Modified { old, new }) => {
                IndexMode::parse(old.trim()).zip(IndexMode::parse(new.trim()))
            }
            _ => None,
        };
        return match modes {
            Some((from, to)) if (from == IndexMode::DontIndex) != (to == IndexMode::DontIndex) => {
                Taken::Operation(S1Operation::SwitchIndex {
                    object,
                    attribute: attribute.to_string(),
                    from,
                    to,
                })
            }
            _ => Taken::Refused(refuse(
                RefusalCode::IndexModeOutsideS1,
                reason,
                &reason.change,
            )),
        };
    }
    Taken::Refused(refuse(
        RefusalCode::AttributePropertyOutsideS1,
        reason,
        &reason.property,
    ))
}

/// The configuration row: only the listing of a new catalog or document.
fn configuration(reason: &Reason) -> Taken {
    if reason.rule == RuleId::ObjectWithStorageAddedOrDropped
        && reason.op == Some(ChangeOp::Added)
        && let [children, listed] = reason.path.as_slice()
        && children.name == "ChildObjects"
        && S1_KINDS.contains(&listed.name.as_str())
        && let Some(name) = listed.label.as_deref().filter(|name| !name.is_empty())
    {
        return Taken::Listing(ObjectId {
            kind: listed.name.clone(),
            name: name.to_string(),
            row: reason.file_name.clone(),
        });
    }
    Taken::Refused(refuse(
        RefusalCode::ConfigurationOutsideS1,
        reason,
        &reason.property,
    ))
}

/// A reason of class `unknown`: the check could not tell.
fn refuse_unknown(reason: &Reason) -> Refusal {
    match reason.rule {
        RuleId::Unspecified => refuse(RefusalCode::RuleUnspecified, reason, ""),
        RuleId::RowDecodesToSameXml => refuse(RefusalCode::RowUnexplained, reason, ""),
        RuleId::RowFormatUpgradeUnproven => refuse(RefusalCode::RowFormatUnproven, reason, ""),
        rule => refuse(RefusalCode::UnknownStep, reason, rule.id()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::super::tree_diff::Seg;
    use super::*;

    fn seg(name: &str, label: Option<&str>) -> Seg {
        Seg {
            name: name.to_string(),
            label: label.map(str::to_string),
        }
    }

    fn reason(
        rule: RuleId,
        kind: &str,
        object: &str,
        path: Vec<Seg>,
        op: Option<ChangeOp>,
    ) -> Reason {
        Reason {
            class: ReasonClass::Structure,
            object: object.to_string(),
            file_name: "row".to_string(),
            property: path
                .iter()
                .map(|seg| seg.name.clone())
                .collect::<Vec<_>>()
                .join("/"),
            change: "x".to_string(),
            rule,
            kind: kind.to_string(),
            path,
            op,
        }
    }

    fn modified(old: &str, new: &str) -> Option<ChangeOp> {
        Some(ChangeOp::Modified {
            old: old.to_string(),
            new: new.to_string(),
        })
    }

    fn verdict(reasons: Vec<Reason>) -> Verdict {
        let mut verdict = Verdict::new("rows");
        for reason in reasons {
            verdict.push_reason(reason);
        }
        verdict
    }

    fn attribute(op: ChangeOp) -> Reason {
        reason(
            RuleId::ColumnAddedOrDropped,
            "Catalog",
            "Catalog.X",
            vec![seg("ChildObjects", None), seg("Attribute", Some("A"))],
            Some(op),
        )
    }

    #[test]
    fn adding_and_deleting_an_attribute_are_operations() {
        let class = classify(&verdict(vec![
            attribute(ChangeOp::Added),
            reason(
                RuleId::ColumnAddedOrDropped,
                "Document",
                "Document.D",
                vec![seg("ChildObjects", None), seg("Attribute", Some("B"))],
                Some(ChangeOp::Removed),
            ),
        ]));
        assert!(class.accepted(), "{:?}", class.refusals);
        assert_eq!(
            class.operations,
            vec![
                S1Operation::AddAttribute {
                    object: ObjectId {
                        kind: "Catalog".into(),
                        name: "X".into(),
                        row: "row".into()
                    },
                    attribute: "A".into()
                },
                S1Operation::DeleteAttribute {
                    object: ObjectId {
                        kind: "Document".into(),
                        name: "D".into(),
                        row: "row".into()
                    },
                    attribute: "B".into()
                },
            ]
        );
    }

    #[test]
    fn a_string_that_grew_is_widened_and_one_that_shrank_is_refused() {
        let path = |leaf: &[&str]| {
            let mut path = vec![
                seg("ChildObjects", None),
                seg("Attribute", Some("A")),
                seg("Properties", None),
            ];
            path.extend(leaf.iter().map(|name| seg(name, None)));
            path
        };
        let length = path(&["Type", "StringQualifiers", "Length"]);
        let widened = classify(&verdict(vec![reason(
            RuleId::AttributePropertyNotCovered,
            "Catalog",
            "Catalog.X",
            length.clone(),
            modified("50", "100"),
        )]));
        assert!(matches!(
            widened.operations.as_slice(),
            [S1Operation::WidenString {
                from: 50,
                to: 100,
                ..
            }]
        ));
        for (old, new) in [("100", "50"), ("50", "50"), ("50", "many"), ("", "9")] {
            let refused = classify(&verdict(vec![reason(
                RuleId::AttributePropertyNotCovered,
                "Catalog",
                "Catalog.X",
                length.clone(),
                modified(old, new),
            )]));
            assert_eq!(refused.refusals.len(), 1, "{old} -> {new}");
            assert_eq!(refused.refusals[0].code, RefusalCode::LengthNotWidened);
        }
    }

    #[test]
    fn the_index_switches_between_dontindex_and_either_indexed_mode_only() {
        let indexing = vec![
            seg("ChildObjects", None),
            seg("Attribute", Some("A")),
            seg("Properties", None),
            seg("Indexing", None),
        ];
        for (old, new) in [
            ("DontIndex", "Index"),
            ("Index", "DontIndex"),
            ("DontIndex", "IndexWithAdditionalOrder"),
            ("IndexWithAdditionalOrder", "DontIndex"),
        ] {
            let class = classify(&verdict(vec![reason(
                RuleId::AttributePropertyNotCovered,
                "Document",
                "Document.D",
                indexing.clone(),
                modified(old, new),
            )]));
            assert!(class.accepted(), "{old} -> {new}");
            assert_eq!(class.operations[0].name(), "switch-index");
        }
        let refused = classify(&verdict(vec![reason(
            RuleId::AttributePropertyNotCovered,
            "Catalog",
            "Catalog.X",
            indexing,
            modified("Index", "IndexWithAdditionalOrder"),
        )]));
        assert_eq!(refused.refusals[0].code, RefusalCode::IndexModeOutsideS1);
    }

    #[test]
    fn a_new_tabular_section_is_an_operation_and_a_dropped_one_is_not() {
        let section = |op| {
            reason(
                RuleId::TabularSectionAddedDroppedMoved,
                "Catalog",
                "Catalog.X",
                vec![seg("ChildObjects", None), seg("TabularSection", Some("T"))],
                Some(op),
            )
        };
        let class = classify(&verdict(vec![section(ChangeOp::Added)]));
        assert_eq!(class.operations[0].name(), "add-tabular-section");
        for op in [ChangeOp::Removed, ChangeOp::Reordered] {
            let refused = classify(&verdict(vec![section(op)]));
            assert_eq!(
                refused.refusals[0].code,
                RefusalCode::TabularSectionOutsideS1
            );
        }
    }

    #[test]
    fn a_new_attribute_of_a_tabular_section_is_an_operation_and_a_dropped_or_moved_one_is_not() {
        let column = |op| {
            reason(
                RuleId::TabularSectionColumnAddedDroppedMoved,
                "Catalog",
                "Catalog.X",
                vec![
                    seg("ChildObjects", None),
                    seg("TabularSection", Some("T")),
                    seg("ChildObjects", None),
                    seg("Attribute", Some("C")),
                ],
                Some(op),
            )
        };
        let class = classify(&verdict(vec![column(ChangeOp::Added)]));
        match class.operations.as_slice() {
            [
                S1Operation::AddSectionAttribute {
                    section, attribute, ..
                },
            ] => assert_eq!((section.as_str(), attribute.as_str()), ("T", "C")),
            other => panic!("{other:?} {:?}", class.refusals),
        }
        assert_eq!(class.operations[0].name(), "add-section-attribute");
        for op in [ChangeOp::Removed, ChangeOp::Reordered] {
            let refused = classify(&verdict(vec![column(op)]));
            assert_eq!(
                refused.refusals[0].code,
                RefusalCode::TabularSectionOutsideS1
            );
        }
        // Without the labels of the path nothing is taken.
        let mut unlabelled = column(ChangeOp::Added);
        unlabelled.path = vec![
            seg("ChildObjects", None),
            seg("TabularSection", None),
            seg("ChildObjects", None),
            seg("Attribute", None),
        ];
        assert_eq!(
            classify(&verdict(vec![unlabelled])).refusals[0].code,
            RefusalCode::TabularSectionOutsideS1
        );
    }

    fn new_object(kind: &str, name: &str) -> Reason {
        reason(
            RuleId::ObjectWithStorageAddedOrDropped,
            kind,
            &format!("{kind}.{name}"),
            Vec::new(),
            Some(ChangeOp::Added),
        )
    }

    fn listing(kind: &str, name: &str) -> Reason {
        reason(
            RuleId::ObjectWithStorageAddedOrDropped,
            "Configuration",
            "Configuration",
            vec![seg("ChildObjects", None), seg(kind, Some(name))],
            Some(ChangeOp::Added),
        )
    }

    #[test]
    fn a_new_object_needs_its_listing_and_the_listing_its_object() {
        let both = classify(&verdict(vec![
            new_object("Catalog", "N"),
            listing("Catalog", "N"),
        ]));
        assert!(both.accepted(), "{:?}", both.refusals);
        assert_eq!(both.operations.len(), 1);
        assert_eq!(both.operations[0].name(), "add-object");

        let alone = classify(&verdict(vec![new_object("Catalog", "N")]));
        assert_eq!(alone.refusals[0].code, RefusalCode::ObjectListMismatch);
        let listed = classify(&verdict(vec![listing("Document", "N")]));
        assert_eq!(listed.refusals[0].code, RefusalCode::ObjectListMismatch);
        // The pair must be of one name and one kind.
        let apart = classify(&verdict(vec![
            new_object("Catalog", "N"),
            listing("Catalog", "M"),
        ]));
        assert_eq!(apart.refusals.len(), 2);
    }

    #[test]
    fn an_object_dropped_or_of_another_kind_is_refused() {
        let mut dropped = new_object("Catalog", "N");
        dropped.op = Some(ChangeOp::Removed);
        assert_eq!(
            classify(&verdict(vec![dropped])).refusals[0].code,
            RefusalCode::ObjectRemoved
        );
        let register = classify(&verdict(vec![
            new_object("InformationRegister", "R"),
            listing("InformationRegister", "R"),
        ]));
        assert_eq!(register.refusals.len(), 2);
        assert!(
            register
                .refusals
                .iter()
                .all(|r| r.code == RefusalCode::KindOutsideS1
                    || r.code == RefusalCode::ConfigurationOutsideS1)
        );
    }

    #[test]
    fn data_unknown_and_unspecified_reasons_are_refused_with_their_own_codes() {
        let mut data = attribute(ChangeOp::Added);
        data.class = ReasonClass::Data;
        assert_eq!(
            classify(&verdict(vec![data])).refusals[0].code,
            RefusalCode::DataChange
        );
        let mut unknown = Reason::step(
            ReasonClass::Unknown,
            RuleId::RowDecodesToSameXml,
            "Catalog.X",
            "row",
            "",
            "the row differs",
        );
        assert_eq!(
            classify(&verdict(vec![unknown.clone()])).refusals[0].code,
            RefusalCode::RowUnexplained
        );
        unknown.rule = RuleId::RowFormatUpgradeUnproven;
        assert_eq!(
            classify(&verdict(vec![unknown.clone()])).refusals[0].code,
            RefusalCode::RowFormatUnproven
        );
        unknown.rule = RuleId::DescriptorsUnreadable;
        assert_eq!(
            classify(&verdict(vec![unknown])).refusals[0].code,
            RefusalCode::UnknownStep
        );
        // A literal that predates ids is never taken, whatever it says.
        let mut plain = attribute(ChangeOp::Added);
        plain.rule = RuleId::Unspecified;
        assert_eq!(
            classify(&verdict(vec![plain])).refusals[0].code,
            RefusalCode::RuleUnspecified
        );
    }

    #[test]
    fn an_incomplete_verdict_is_refused_even_without_reasons() {
        let mut incomplete = Verdict::new("tree-db");
        incomplete.incomplete = true;
        incomplete.stats.body_files_not_compared = 3;
        let class = classify(&incomplete);
        assert_eq!(class.refusals[0].code, RefusalCode::IncompleteVerdict);
        assert!(!class.accepted());
        assert!(classify(&Verdict::new("rows")).accepted());
    }

    #[test]
    fn what_no_operation_covers_is_refused_by_default() {
        for (rule, path) in [
            (
                RuleId::PropertyNotCovered,
                vec![seg("Properties", None), seg("CodeLength", None)],
            ),
            (
                RuleId::GeneratedTypesChanged,
                vec![seg("InternalInfo", None)],
            ),
            (
                RuleId::AttributePropertyNotCovered,
                vec![
                    seg("ChildObjects", None),
                    seg("Attribute", Some("A")),
                    seg("Properties", None),
                    seg("Type", None),
                    seg("Type", None),
                ],
            ),
            (RuleId::KindOwnsNoStoredData, Vec::new()),
        ] {
            let refused = classify(&verdict(vec![reason(
                rule,
                "Catalog",
                "Catalog.X",
                path,
                modified("a", "b"),
            )]));
            assert_eq!(refused.refusals.len(), 1, "{rule:?}");
            assert!(refused.operations.is_empty(), "{rule:?}");
        }
    }

    #[test]
    fn a_nested_object_or_a_row_is_not_a_catalog_operation() {
        let nested = classify(&verdict(vec![reason(
            RuleId::ColumnAddedOrDropped,
            "Catalog",
            "Catalog.X.Form.F",
            vec![seg("ChildObjects", None), seg("Attribute", Some("A"))],
            Some(ChangeOp::Added),
        )]));
        assert_eq!(nested.refusals[0].code, RefusalCode::NestedObjectOutsideS1);
        let root = classify(&verdict(vec![Reason::step(
            ReasonClass::Structure,
            RuleId::RootRowChanged,
            "Configuration",
            "root",
            "",
            "the root row differs",
        )]));
        assert_eq!(root.refusals[0].code, RefusalCode::ServiceRowChanged);
    }

    /// The operations of S1, written down once more as a plain table: the
    /// exhaustive test below holds `classify` to it from the outside.
    fn spec_says_operation(
        class: ReasonClass,
        rule: RuleId,
        kind: &str,
        path: &[Seg],
        op: &Option<ChangeOp>,
    ) -> bool {
        let names = path.iter().map(|seg| seg.name.as_str()).collect::<Vec<_>>();
        let named = path.get(1).is_some_and(|seg| seg.label.is_some());
        class == ReasonClass::Structure
            && ["Catalog", "Document"].contains(&kind)
            && match (rule, names.as_slice(), op) {
                (
                    RuleId::ColumnAddedOrDropped,
                    ["ChildObjects", "Attribute"],
                    Some(ChangeOp::Added | ChangeOp::Removed),
                ) => named,
                (
                    RuleId::AttributePropertyNotCovered,
                    [
                        "ChildObjects",
                        "Attribute",
                        "Properties",
                        "Type",
                        "StringQualifiers",
                        "Length",
                    ],
                    Some(ChangeOp::Modified { old, new }),
                ) => {
                    named
                        && matches!(
                            (old.parse::<u32>(), new.parse::<u32>()),
                            (Ok(old), Ok(new)) if new > old
                        )
                }
                (
                    RuleId::AttributePropertyNotCovered,
                    ["ChildObjects", "Attribute", "Properties", "Indexing"],
                    Some(ChangeOp::Modified { old, new }),
                ) => {
                    named
                        && matches!(
                            (old.as_str(), new.as_str()),
                            ("DontIndex", "Index" | "IndexWithAdditionalOrder")
                                | ("Index" | "IndexWithAdditionalOrder", "DontIndex")
                        )
                }
                (
                    RuleId::TabularSectionAddedDroppedMoved,
                    ["ChildObjects", "TabularSection"],
                    Some(ChangeOp::Added),
                ) => named,
                (
                    RuleId::TabularSectionColumnAddedDroppedMoved,
                    [
                        "ChildObjects",
                        "TabularSection",
                        "ChildObjects",
                        "Attribute",
                    ],
                    Some(ChangeOp::Added),
                ) => named && path.get(3).is_some_and(|seg| seg.label.is_some()),
                _ => false,
            }
    }

    #[test]
    fn exactly_the_operations_of_s1_pass_and_nothing_else_does() {
        let paths: Vec<Vec<Seg>> = vec![
            vec![],
            vec![seg("Properties", None), seg("CodeLength", None)],
            vec![seg("ChildObjects", None), seg("Attribute", Some("A"))],
            vec![seg("ChildObjects", None), seg("Attribute", None)],
            vec![seg("ChildObjects", None), seg("TabularSection", Some("T"))],
            vec![seg("ChildObjects", None), seg("Dimension", Some("D"))],
            vec![
                seg("ChildObjects", None),
                seg("Attribute", Some("A")),
                seg("Properties", None),
                seg("Type", None),
                seg("StringQualifiers", None),
                seg("Length", None),
            ],
            vec![
                seg("ChildObjects", None),
                seg("Attribute", Some("A")),
                seg("Properties", None),
                seg("Indexing", None),
            ],
            vec![
                seg("ChildObjects", None),
                seg("Attribute", Some("A")),
                seg("Properties", None),
                seg("Use", None),
            ],
            vec![
                seg("ChildObjects", None),
                seg("TabularSection", Some("T")),
                seg("ChildObjects", None),
                seg("Attribute", Some("C")),
            ],
            vec![seg("ChildObjects", None), seg("Catalog", Some("N"))],
            vec![seg("InternalInfo", None)],
        ];
        let ops = [
            None,
            Some(ChangeOp::Added),
            Some(ChangeOp::Removed),
            Some(ChangeOp::Reordered),
            modified("50", "100"),
            modified("100", "50"),
            modified("DontIndex", "Index"),
            modified("Index", "DontIndex"),
            modified("Index", "IndexWithAdditionalOrder"),
            modified("ForItem", "ForFolder"),
        ];
        let kinds = [
            "Catalog",
            "Document",
            "Enum",
            "InformationRegister",
            "AccumulationRegister",
            "ChartOfAccounts",
            "Configuration",
            "Report",
            "Form",
            "",
        ];
        let (mut checked, mut passed) = (0, 0);
        for &rule in RuleId::ALL {
            for kind in kinds {
                for path in &paths {
                    for op in &ops {
                        for class in [
                            ReasonClass::Structure,
                            ReasonClass::Data,
                            ReasonClass::Unknown,
                        ] {
                            let mut one =
                                reason(rule, kind, &format!("{kind}.X"), path.clone(), op.clone());
                            one.class = class;
                            let taken = classify(&verdict(vec![one]));
                            let expected = spec_says_operation(class, rule, kind, path, op);
                            checked += 1;
                            if !taken.operations.is_empty() {
                                passed += 1;
                            }
                            assert_eq!(
                                !taken.operations.is_empty(),
                                expected,
                                "{rule:?} {kind} {path:?} {op:?} {class:?}"
                            );
                            // A single reason is an operation or a refusal,
                            // never both and never neither.
                            assert_eq!(
                                taken.operations.len() + taken.refusals.len(),
                                1,
                                "{rule:?} {kind} {path:?} {op:?} {class:?}"
                            );
                        }
                    }
                }
            }
        }
        assert!(checked > 10_000, "{checked} combinations");
        // Two kinds x (add and delete an attribute, widen, switch the index
        // both ways, add a section, add an attribute to a section): the table
        // above and nothing else.
        assert_eq!(passed, 14, "of {checked} combinations");
    }

    #[test]
    fn operations_are_grouped_by_the_descriptor_row_of_their_object() {
        let mut first = attribute(ChangeOp::Added);
        first.file_name = "5EAB8A1B-070F-4DCF-BDCC-A259C62C3693".to_string();
        let mut second = reason(
            RuleId::TabularSectionAddedDroppedMoved,
            "Catalog",
            "Catalog.X",
            vec![seg("ChildObjects", None), seg("TabularSection", Some("T"))],
            Some(ChangeOp::Added),
        );
        second.file_name = "5eab8a1b-070f-4dcf-bdcc-a259c62c3693".to_string();
        let mut other = reason(
            RuleId::ColumnAddedOrDropped,
            "Document",
            "Document.D",
            vec![seg("ChildObjects", None), seg("Attribute", Some("B"))],
            Some(ChangeOp::Added),
        );
        other.file_name = "11111111-1111-4111-8111-111111111111".to_string();
        let class = classify(&verdict(vec![first, other, second]));
        let groups = class.by_object();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0.full_name(), "Catalog.X");
        assert_eq!(
            groups[0].1.iter().map(|op| op.name()).collect::<Vec<_>>(),
            ["add-attribute", "add-tabular-section"]
        );
        assert_eq!(groups[1].0.full_name(), "Document.D");
    }

    #[test]
    fn the_new_object_keeps_the_row_of_its_descriptor_not_the_configuration_s() {
        let mut object = new_object("Catalog", "N");
        object.file_name = "22222222-2222-4222-8222-222222222222".to_string();
        let mut listed = listing("Catalog", "N");
        listed.file_name = "66193438-abc5-410b-a1f1-a204102d1a62".to_string();
        let class = classify(&verdict(vec![listed, object]));
        match class.operations.as_slice() {
            [S1Operation::AddObject { object }] => {
                assert_eq!(object.row, "22222222-2222-4222-8222-222222222222")
            }
            other => panic!("{other:?} {:?}", class.refusals),
        }
    }

    #[test]
    fn every_refusal_code_has_a_unique_id_and_a_text() {
        let mut ids = HashSet::new();
        for code in RefusalCode::ALL {
            assert!(ids.insert(code.id()), "{}", code.id());
            assert!(!code.text().is_empty());
        }
    }

    #[test]
    fn a_refusal_message_names_the_code_the_reason_and_the_detail() {
        let class = classify(&verdict(vec![reason(
            RuleId::PropertyNotCovered,
            "Catalog",
            "Catalog.X",
            vec![seg("Properties", None), seg("CodeLength", None)],
            modified("9", "12"),
        )]));
        let message = class.refusal_message().unwrap();
        assert!(message.starts_with("property-outside-s1: Catalog.X: Properties/CodeLength"));
        assert!(message.ends_with("[Properties/CodeLength]"), "{message}");
    }
}
