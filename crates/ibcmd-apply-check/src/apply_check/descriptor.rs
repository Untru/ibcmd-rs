//! Comparing one object's descriptor before and after, whatever it came
//! from (a Config row decoded by the metadata model, or a metadata XML of a
//! source tree): the changes go through the rules, the harmless ones become
//! notes and the others reasons. Each object that differs is also listed in
//! the verdict's `objects`, with the most serious class among its changes.

use crate::metadata_model::xml::Element;

use super::model::{Note, ObjectChange, ObjectOp, Reason, ReasonClass, Verdict};
use super::rule_id::RuleId;
use super::rules::{self, Class};
use super::tree_diff::{self, Change, ChangeOp};

/// What the object is called in messages and where it lives.
pub struct ObjectRef<'a> {
    pub kind: &'a str,
    /// `Catalog._ДемоКассы`, `Catalog.X.Form.F`.
    pub name: &'a str,
    /// The Config row, or the file of a tree.
    pub file_name: &'a str,
    /// The object's uuid (the Config row's name).
    pub id: &'a str,
}

fn class_of(class: Class) -> Option<ReasonClass> {
    match class {
        Class::Safe => None,
        Class::Structure => Some(ReasonClass::Structure),
        Class::Data => Some(ReasonClass::Data),
    }
}

/// The more serious of two classes (`None` is harmless).
fn worse(left: Option<ReasonClass>, right: Option<ReasonClass>) -> Option<ReasonClass> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (left, right) => left.or(right),
    }
}

/// Puts one change through the rules; the class it got, `None` when it is
/// harmless.
fn record(object: &ObjectRef<'_>, change: &Change, verdict: &mut Verdict) -> Option<ReasonClass> {
    let decision = rules::decide(object.kind, change);
    let class = class_of(decision.class);
    match class {
        None => verdict.push_note(Note {
            object: object.name.to_string(),
            file_name: object.file_name.to_string(),
            property: change.display_path(),
            change: change.describe(),
        }),
        Some(class) => verdict.push_reason(Reason {
            class,
            object: object.name.to_string(),
            file_name: object.file_name.to_string(),
            property: change.display_path(),
            change: format!("{} ({})", change.describe(), decision.rule),
            rule: decision.rule,
            kind: object.kind.to_string(),
            path: change.path.clone(),
            op: Some(change.op.clone()),
        }),
    }
    class
}

fn push_object(
    object: &ObjectRef<'_>,
    op: ObjectOp,
    class: Option<ReasonClass>,
    changes: usize,
    verdict: &mut Verdict,
) {
    verdict.objects.push(ObjectChange {
        object: object.name.to_string(),
        id: object.id.to_string(),
        file_name: object.file_name.to_string(),
        kind: object.kind.to_string(),
        op,
        class,
        changes,
    });
}

/// The differences of two decoded descriptors. Returns how many changes
/// there were; none means the trees are the same, which a caller that saw
/// the stored bytes differ must treat as suspicious.
pub fn compare(
    object: &ObjectRef<'_>,
    old: &Element,
    new: &Element,
    verdict: &mut Verdict,
) -> usize {
    let changes = tree_diff::diff(old, new);
    let mut worst = None;
    for change in &changes {
        worst = worse(worst, record(object, change, verdict));
    }
    if !changes.is_empty() {
        push_object(object, ObjectOp::Changed, worst, changes.len(), verdict);
    }
    changes.len()
}

/// An object that exists on one side only.
pub fn lifecycle(object: &ObjectRef<'_>, added: bool, verdict: &mut Verdict) {
    let what = if added { "added" } else { "removed" };
    let op = if added {
        ObjectOp::Added
    } else {
        ObjectOp::Removed
    };
    let decision = rules::decide_lifecycle(object.kind);
    let class = class_of(decision.class);
    match class {
        None => verdict.push_note(Note {
            object: object.name.to_string(),
            file_name: object.file_name.to_string(),
            property: String::new(),
            change: format!("{what} ({})", object.kind),
        }),
        Some(class) => verdict.push_reason(Reason {
            class,
            object: object.name.to_string(),
            file_name: object.file_name.to_string(),
            property: String::new(),
            change: format!("{what} ({}; {})", object.kind, decision.rule),
            rule: decision.rule,
            kind: object.kind.to_string(),
            path: Vec::new(),
            op: Some(if added {
                ChangeOp::Added
            } else {
                ChangeOp::Removed
            }),
        }),
    }
    push_object(object, op, class, 1, verdict);
}

/// The same object is another kind on the two sides: nothing to compare,
/// and a restructuring.
pub fn kind_changed(object: &ObjectRef<'_>, old_kind: &str, verdict: &mut Verdict) {
    verdict.push_reason(Reason {
        class: ReasonClass::Structure,
        object: object.name.to_string(),
        file_name: object.file_name.to_string(),
        property: String::new(),
        change: format!("the kind changed: {old_kind} -> {}", object.kind),
        rule: RuleId::KindChanged,
        kind: object.kind.to_string(),
        path: Vec::new(),
        op: None,
    });
    push_object(
        object,
        ObjectOp::Changed,
        Some(ReasonClass::Structure),
        1,
        verdict,
    );
}

/// An object the check could not judge (a row it cannot read, a kind it
/// cannot place): listed with the class `unknown`. The reason is the
/// caller's.
pub fn unresolved(object: &ObjectRef<'_>, op: ObjectOp, verdict: &mut Verdict) {
    push_object(object, op, Some(ReasonClass::Unknown), 1, verdict);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata_model::xml::parse_element_tree;

    fn object<'a>(kind: &'a str, name: &'a str) -> ObjectRef<'a> {
        ObjectRef {
            kind,
            name,
            file_name: "row",
            id: "u",
        }
    }

    fn catalog(code_length: &str, synonym: &str) -> Element {
        parse_element_tree(
            format!(
                "<Catalog uuid=\"u\"><Properties><Name>K</Name>\
                 <Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>{synonym}</v8:content></v8:item></Synonym>\
                 <CodeLength>{code_length}</CodeLength></Properties><ChildObjects/></Catalog>"
            )
            .as_bytes(),
        )
        .unwrap()
    }

    #[test]
    fn an_object_with_only_harmless_changes_is_listed_without_a_class() {
        let mut verdict = Verdict::new("trees");
        let count = compare(
            &object("Catalog", "Catalog.K"),
            &catalog("9", "a"),
            &catalog("9", "b"),
            &mut verdict,
        );
        assert_eq!(count, 1);
        assert!(!verdict.needs_restructuring);
        assert_eq!(verdict.objects.len(), 1);
        assert_eq!(verdict.objects[0].op, ObjectOp::Changed);
        assert_eq!(verdict.objects[0].class, None);
        assert_eq!(verdict.objects[0].changes, 1);
    }

    #[test]
    fn an_object_takes_the_class_of_its_worst_change() {
        let mut verdict = Verdict::new("trees");
        compare(
            &object("Catalog", "Catalog.K"),
            &catalog("9", "a"),
            &catalog("12", "b"),
            &mut verdict,
        );
        assert!(verdict.needs_restructuring);
        assert_eq!(verdict.objects.len(), 1);
        assert_eq!(verdict.objects[0].class, Some(ReasonClass::Structure));
        assert_eq!(verdict.objects[0].changes, 2);
        assert_eq!(verdict.notes.len(), 1);
        assert_eq!(verdict.reasons.len(), 1);
    }

    #[test]
    fn an_object_without_changes_is_not_listed() {
        let mut verdict = Verdict::new("trees");
        let count = compare(
            &object("Catalog", "Catalog.K"),
            &catalog("9", "a"),
            &catalog("9", "a"),
            &mut verdict,
        );
        assert_eq!(count, 0);
        assert!(verdict.objects.is_empty());
    }

    #[test]
    fn added_and_removed_objects_are_listed_by_their_kind() {
        let mut verdict = Verdict::new("trees");
        lifecycle(
            &object("CommonModule", "CommonModule.M"),
            true,
            &mut verdict,
        );
        lifecycle(&object("Catalog", "Catalog.K"), false, &mut verdict);
        unresolved(&object("", "row"), ObjectOp::Changed, &mut verdict);
        assert_eq!(verdict.objects.len(), 3);
        assert_eq!(verdict.objects[0].op, ObjectOp::Added);
        assert_eq!(verdict.objects[0].class, None);
        assert_eq!(verdict.objects[1].op, ObjectOp::Removed);
        assert_eq!(verdict.objects[1].class, Some(ReasonClass::Structure));
        assert_eq!(verdict.objects[2].class, Some(ReasonClass::Unknown));
    }

    #[test]
    fn the_most_serious_class_wins() {
        assert_eq!(worse(None, None), None);
        assert_eq!(
            worse(None, Some(ReasonClass::Data)),
            Some(ReasonClass::Data)
        );
        assert_eq!(
            worse(Some(ReasonClass::Structure), Some(ReasonClass::Data)),
            Some(ReasonClass::Data)
        );
        assert_eq!(
            worse(Some(ReasonClass::Unknown), Some(ReasonClass::Data)),
            Some(ReasonClass::Unknown)
        );
    }
}
