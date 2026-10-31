//! The S1 gate: what the gate lets through of the classification of the restructuring check
//! (`apply_check::s1::classify`), and what it refuses (the fixtures of case a2 stand in for the database).

use super::tests_plan::{
    CATALOG, NEW_ROW, OLD_ROW, client_as_string, client_indexing, inputs, options,
    string_client_inputs, widen_client,
};
use crate::apply_check::{ChangeOp, Reason, ReasonClass, RuleId, Seg, Verdict};
use crate::mssql_config_apply::gate::{GateBlocker, GateVerdict};
use crate::restructure::s1::{decide, own_refusal, without_names};
use std::collections::HashSet;

fn seg(name: &str, label: Option<&str>) -> Seg {
    Seg {
        name: name.to_owned(),
        label: label.map(str::to_owned),
    }
}

/// A typed reason of class `structure` on the descriptor of the catalog of case a2.
fn structure(
    rule: RuleId,
    kind: &str,
    object: &str,
    path: Vec<Seg>,
    op: Option<ChangeOp>,
) -> Reason {
    Reason {
        class: ReasonClass::Structure,
        object: object.to_owned(),
        file_name: CATALOG.to_owned(),
        property: path
            .iter()
            .map(|seg| seg.name.clone())
            .collect::<Vec<_>>()
            .join("/"),
        change: "x".to_owned(),
        rule,
        kind: kind.to_owned(),
        path,
        op,
    }
}

fn check_of(reasons: Vec<Reason>) -> Verdict {
    let mut verdict = Verdict::new("rows");
    for reason in reasons {
        verdict.push_reason(reason);
    }
    verdict
}

const OBJECT: &str = "Catalog._ДемоПартнеры";

fn attribute_path(name: &str) -> Vec<Seg> {
    vec![seg("ChildObjects", None), seg("Attribute", Some(name))]
}

fn attribute(name: &str, op: ChangeOp) -> Reason {
    structure(
        RuleId::ColumnAddedOrDropped,
        "Catalog",
        OBJECT,
        attribute_path(name),
        Some(op),
    )
}

fn length(name: &str, from: &str, to: &str) -> Reason {
    let mut path = attribute_path(name);
    path.extend([
        seg("Properties", None),
        seg("Type", None),
        seg("StringQualifiers", None),
        seg("Length", None),
    ]);
    structure(
        RuleId::AttributePropertyNotCovered,
        "Catalog",
        OBJECT,
        path,
        Some(ChangeOp::Modified {
            old: from.to_owned(),
            new: to.to_owned(),
        }),
    )
}

fn indexing(name: &str, from: &str, to: &str) -> Reason {
    let mut path = attribute_path(name);
    path.extend([seg("Properties", None), seg("Indexing", None)]);
    structure(
        RuleId::AttributePropertyNotCovered,
        "Catalog",
        OBJECT,
        path,
        Some(ChangeOp::Modified {
            old: from.to_owned(),
            new: to.to_owned(),
        }),
    )
}

/// The check's verdict for case a2: one new attribute.
fn check_of_a2() -> Verdict {
    check_of(vec![attribute("ДемоНовыйРеквизит", ChangeOp::Added)])
}

/// The conservative gate's verdict: the descriptors that differ are blockers.
fn conservative(rows: &[&str]) -> GateVerdict {
    GateVerdict {
        restructuring_required: !rows.is_empty(),
        blockers: rows
            .iter()
            .map(|row| GateBlocker {
                row: (*row).to_owned(),
                reason: "the descriptor's text differs from the active one: a metadata change, possibly structural".to_owned(),
            })
            .collect(),
        gate: "conservative".to_owned(),
        ..GateVerdict::default()
    }
}

fn blocked_with(verdict: &GateVerdict, text: &str) -> bool {
    verdict
        .blockers
        .iter()
        .any(|blocker| blocker.reason.starts_with("S1: ") && blocker.reason.contains(text))
}

#[test]
fn a_planned_attribute_is_let_through_and_its_blocker_withdrawn() {
    let (verdict, phase) = decide(
        conservative(&[CATALOG]),
        &check_of_a2(),
        &inputs(OLD_ROW, NEW_ROW),
        &options(),
    );
    assert!(!verdict.restructuring_required, "{:?}", verdict.blockers);
    assert!(verdict.blockers.is_empty());
    assert!(verdict.gate.starts_with("s1"));
    let phase = phase.expect("a structure phase");
    assert_eq!(
        phase.tables,
        ["_Reference20", "_Reference20_VT155", "_Reference20_VT159"]
    );
    assert_eq!(phase.objects.len(), 1);
    assert!(phase.objects[0].contains("Fld11034 = ДемоНовыйРеквизит"));
    assert!(phase.sql.contains("create table dbo._Reference20NG"));
    assert!(phase.sql.contains("Modified = @now"));
    // The fixtures hold no cache rows and the options leave the caches alone.
    assert!(phase.params_rewrites.is_empty());
    // The fixture stage has the `deleted` row of a native import (an empty list): the phase answers for it.
    assert_eq!(phase.consumed_staged_rows, 1);
}

#[test]
fn a_root_blocker_is_the_checks_to_answer() {
    // The conservative rule compares the bytes of `root`; on 8.5 the platform re-stamps its payload on every
    // write, and the check (which reads it) found the row the same: the blocker is withdrawn.
    let (verdict, phase) = decide(
        conservative(&[CATALOG, "root"]),
        &check_of_a2(),
        &inputs(OLD_ROW, NEW_ROW),
        &options(),
    );
    assert!(!verdict.restructuring_required, "{:?}", verdict.blockers);
    assert!(phase.is_some());

    // A root the check found changed is a typed refusal, and the plan is not made.
    let mut changed = check_of_a2();
    changed.push_reason(Reason {
        class: ReasonClass::Structure,
        object: "Configuration".to_owned(),
        file_name: "root".to_owned(),
        rule: RuleId::RootRowChanged,
        ..Reason::default()
    });
    let (verdict, phase) = decide(
        conservative(&[CATALOG, "root"]),
        &changed,
        &inputs(OLD_ROW, NEW_ROW),
        &options(),
    );
    assert!(verdict.restructuring_required && phase.is_none());
    assert!(
        blocked_with(&verdict, "service-row-changed"),
        "{:?}",
        verdict.blockers
    );
}

#[test]
fn a_refusal_carries_the_gates_own_reasons_and_not_the_conservative_lines_of_harmless_rows() {
    // A mixed stage: a predefined-data body (a data change, which S1 does not do) next to descriptors and bodies that the
    // restructuring check finds harmless. The conservative rule lists every one of them; the check, asked first by the
    // apply, names the row it refuses, so the gate's refusal is its own reason and the rest is noise.
    let harmless = [
        "4a7aa9ca-5e44-4ffa-bebc-26e2685e48f0",
        "66193438-abc5-410b-a1f1-a204102d1a62",
        "794f5fb5-3b8f-4f45-9250-7a3b9d1db383.1c",
        "b68ec95b-64f3-4a2b-9683-231d50f01eeb.0",
    ];
    let mut rows = vec![CATALOG];
    rows.extend(harmless);
    let mut check = check_of_a2();
    check.push_reason(Reason {
        class: ReasonClass::Data,
        object: "Catalog.СтраныМира".to_owned(),
        file_name: "794f5fb5-3b8f-4f45-9250-7a3b9d1db383.1c".to_owned(),
        kind: "Catalog".to_owned(),
        ..Reason::default()
    });
    let (verdict, phase) = decide(
        conservative(&rows),
        &check,
        &inputs(OLD_ROW, NEW_ROW),
        &options(),
    );
    assert!(verdict.restructuring_required && phase.is_none());
    assert!(
        blocked_with(&verdict, "data-change"),
        "{:?}",
        verdict.blockers
    );
    assert!(
        verdict
            .blockers
            .iter()
            .all(|blocker| blocker.reason.starts_with("S1: ")),
        "{:?}",
        verdict.blockers
    );
    assert_eq!(verdict.blockers_omitted, 0);

    // Many conservative lines do not push the gate's own reason out of the list.
    let mut crowded = conservative(&rows);
    crowded.blockers_omitted = 500;
    let (verdict, _) = decide(crowded, &check, &inputs(OLD_ROW, NEW_ROW), &options());
    assert!(
        blocked_with(&verdict, "data-change"),
        "{:?}",
        verdict.blockers
    );
    assert_eq!(verdict.blockers_omitted, 0);
}

#[test]
fn an_early_refusal_of_the_gate_drops_the_conservative_lines_too() {
    let mut crowded = conservative(&[CATALOG, "ffffffff-0000-4000-8000-000000000000"]);
    crowded.blockers_omitted = 4;
    let verdict = own_refusal(
        crowded,
        "SchemaStorage",
        "S1: the schema storage is not idle".to_owned(),
    );
    assert!(verdict.restructuring_required);
    assert_eq!(verdict.blockers.len(), 1);
    assert_eq!(verdict.blockers[0].row, "SchemaStorage");
    assert_eq!(verdict.blockers_omitted, 0);
}

#[test]
fn an_s1_change_and_a_descriptor_the_check_finds_harmless_elsewhere_are_let_through() {
    // The conservative rule refuses both descriptors (it cannot tell a synonym from a column); the restructuring check names
    // the attribute of case a2 and nothing about the other descriptor, which it has read and accepts (a synonym).
    let elsewhere = "ffffffff-0000-4000-8000-000000000000";
    let (verdict, phase) = decide(
        conservative(&[CATALOG, elsewhere]),
        &check_of_a2(),
        &inputs(OLD_ROW, NEW_ROW),
        &options(),
    );
    assert!(!verdict.restructuring_required, "{:?}", verdict.blockers);
    assert!(verdict.blockers.is_empty());
    assert!(phase.is_some());
    assert_eq!(verdict.blockers_omitted, 0);

    // Many descriptors of that kind: the omitted ones are the conservative rule's too, and go with them.
    let mut crowded = conservative(&[CATALOG, elsewhere]);
    crowded.blockers_omitted = 300;
    let (verdict, phase) = decide(
        crowded,
        &check_of_a2(),
        &inputs(OLD_ROW, NEW_ROW),
        &options(),
    );
    assert!(
        verdict.restructuring_required && phase.is_none(),
        "an omitted blocker cannot be judged"
    );
}

#[test]
fn an_s1_change_and_a_harmful_change_elsewhere_are_still_refused() {
    let elsewhere = "ffffffff-0000-4000-8000-000000000000";
    // The check names a structural change of another descriptor, of a kind S1 does not do: the classification refuses it
    // (a reason of this gate), whatever the conservative rule says of the row.
    let mut check = check_of_a2();
    check.push_reason(Reason {
        class: ReasonClass::Structure,
        object: "AccumulationRegister.X".to_owned(),
        file_name: elsewhere.to_owned(),
        rule: RuleId::ColumnAddedOrDropped,
        kind: "AccumulationRegister".to_owned(),
        path: vec![seg("ChildObjects", None), seg("Resource", Some("Р"))],
        op: Some(ChangeOp::Added),
        ..Reason::default()
    });
    let (verdict, phase) = decide(
        conservative(&[CATALOG, elsewhere]),
        &check,
        &inputs(OLD_ROW, NEW_ROW),
        &options(),
    );
    assert!(verdict.restructuring_required && phase.is_none());
    assert!(!verdict.blockers.is_empty());
    assert!(
        verdict
            .blockers
            .iter()
            .all(|blocker| blocker.reason.starts_with("S1: ")),
        "{:?}",
        verdict.blockers
    );

    // A reason that the classification passes (an operation of S1 on the other descriptor's object) that the plan does not
    // make: the two decoders disagree, and the row is not withdrawn on the strength of the conservative rule alone.
    let mut other = check_of_a2();
    other.push_reason(Reason {
        file_name: elsewhere.to_owned(),
        ..attribute("Другой", ChangeOp::Added)
    });
    let (verdict, phase) = decide(
        conservative(&[CATALOG, elsewhere]),
        &other,
        &inputs(OLD_ROW, NEW_ROW),
        &options(),
    );
    assert!(verdict.restructuring_required && phase.is_none());
}

#[test]
fn a_widened_string_is_let_through_and_narrowing_is_not() {
    let widened = widen_client(
        &client_as_string(OLD_ROW, 50),
        "{\"S\",50,1}",
        "{\"S\",200,1}",
    );
    let staged = string_client_inputs(&widened);
    let check = check_of(vec![length("Клиент", "50", "200")]);
    let (verdict, phase) = decide(conservative(&[CATALOG]), &check, &staged, &options());
    assert!(!verdict.restructuring_required, "{:?}", verdict.blockers);
    let phase = phase.expect("a structure phase");
    assert!(phase.objects[0].contains("widened attributes"));
    assert!(phase.objects[0].contains("(50 -> 200)"));
    assert!(phase.params_rewrites.is_empty() && phase.caches.is_empty());
    assert!(phase.sql.contains("create table dbo._Reference20NG"));

    // The check names another attribute than the plan widens: the decoders disagree.
    let other = check_of(vec![length("Другой", "50", "200")]);
    let (verdict, phase) = decide(conservative(&[CATALOG]), &other, &staged, &options());
    assert!(verdict.restructuring_required && phase.is_none());
    assert!(blocked_with(&verdict, "disagree"), "{:?}", verdict.blockers);

    // The check itself refuses a limit that shrank (a typed refusal of the classification) ...
    let shrank = check_of(vec![length("Клиент", "50", "20")]);
    let (verdict, phase) = decide(conservative(&[CATALOG]), &shrank, &staged, &options());
    assert!(verdict.restructuring_required && phase.is_none());
    assert!(
        blocked_with(&verdict, "length-not-widened"),
        "{:?}",
        verdict.blockers
    );
    // ... and when it names it as a widening the plan refuses it.
    let narrowed = widen_client(
        &client_as_string(OLD_ROW, 50),
        "{\"S\",50,1}",
        "{\"S\",20,1}",
    );
    let (verdict, phase) = decide(
        conservative(&[CATALOG]),
        &check,
        &string_client_inputs(&narrowed),
        &options(),
    );
    assert!(verdict.restructuring_required && phase.is_none());
    assert!(
        blocked_with(&verdict, "the limit is shorter"),
        "{:?}",
        verdict.blockers
    );
}

#[test]
fn a_deleted_attribute_is_let_through_with_the_deleted_row_the_phase_answers_for() {
    // The reverse of case a2: the images swapped is not a removal the fixtures can hold (DBNames has no
    // number for the attribute), so the removal is exercised on the state after the addition.
    let added = crate::restructure::plan::plan(&inputs(OLD_ROW, NEW_ROW), &options()).unwrap();
    let mut back = inputs(NEW_ROW, OLD_ROW);
    back.schema = added.new_schema.clone();
    back.main_names = added.new_names_row.clone();
    back.staged.deleted = Some(
        crate::restructure::names::deflate(
            "\u{feff}1,\"c60cdc87-198a-4f6e-8f17-76bcb1b1914b\",1".as_bytes(),
        )
        .unwrap(),
    );
    let check = check_of(vec![attribute("ДемоНовыйРеквизит", ChangeOp::Removed)]);
    let (verdict, phase) = decide(conservative(&[CATALOG]), &check, &back, &options());
    assert!(!verdict.restructuring_required, "{:?}", verdict.blockers);
    let phase = phase.expect("a structure phase");
    assert!(phase.objects[0].contains("removed attributes Fld11034"));
    assert_eq!(phase.consumed_staged_rows, 1);

    // The check calls the `deleted` list unknown: the gate lets that one reason through, no other.
    let mut with_deleted = check.clone();
    with_deleted.push_reason(Reason {
        class: ReasonClass::Unknown,
        object: "deleted".to_owned(),
        file_name: "deleted".to_owned(),
        rule: RuleId::DeletedRowNotEmpty,
        ..Reason::default()
    });
    let (verdict, phase) = decide(conservative(&[CATALOG]), &with_deleted, &back, &options());
    assert!(!verdict.restructuring_required, "{:?}", verdict.blockers);
    assert!(phase.is_some());

    // A list that names anything but the attributes the stage removes is the plan's refusal.
    let mut wrong = back.clone();
    wrong.staged.deleted = Some(
        crate::restructure::names::deflate(
            "\u{feff}1,\"aaaaaaaa-0000-4000-8000-000000000000\",1".as_bytes(),
        )
        .unwrap(),
    );
    let (verdict, phase) = decide(conservative(&[CATALOG]), &with_deleted, &wrong, &options());
    assert!(verdict.restructuring_required && phase.is_none());
    assert!(
        blocked_with(&verdict, "deletes aaaaaaaa"),
        "{:?}",
        verdict.blockers
    );
}

#[test]
fn nothing_is_let_through_that_the_gate_does_not_cover() {
    let base = inputs(OLD_ROW, NEW_ROW);

    // Nothing to restructure: a plain apply.
    let (verdict, phase) = decide(conservative(&[]), &check_of_a2(), &base, &options());
    assert!(!verdict.restructuring_required && phase.is_none());

    // A row the conservative rule refuses for a limit of this apply (not for what the restructuring check judges) and
    // that this gate does not explain: the refusal stands, for that row.
    let mut limit = conservative(&[CATALOG]);
    limit.block(
        "ffffffff-0000-4000-8000-000000000000",
        "a row name the own apply does not know (not a service row, descriptor or body row)",
    );
    let (verdict, phase) = decide(limit, &check_of_a2(), &base, &options());
    assert!(verdict.restructuring_required && phase.is_none());
    assert_eq!(verdict.blockers.len(), 1);
    assert_eq!(
        verdict.blockers[0].row,
        "ffffffff-0000-4000-8000-000000000000"
    );

    // A new object that the stage does not hold (S1-F builds them; the plan finds none in this image): the two
    // decoders disagree and the refusal stands.
    let object = check_of(vec![
        structure(
            RuleId::ObjectWithStorageAddedOrDropped,
            "Catalog",
            "Catalog.Новый",
            vec![],
            Some(ChangeOp::Added),
        ),
        structure(
            RuleId::ObjectWithStorageAddedOrDropped,
            "Configuration",
            "Configuration",
            vec![seg("ChildObjects", None), seg("Catalog", Some("Новый"))],
            Some(ChangeOp::Added),
        ),
    ]);
    let (verdict, phase) = decide(conservative(&[CATALOG]), &object, &base, &options());
    assert!(verdict.restructuring_required && phase.is_none());
    assert!(
        blocked_with(&verdict, "the two decoders disagree"),
        "{:?}",
        verdict.blockers
    );

    // A reason the classification refuses: a data change, an unknown row, another kind of object.
    for (class, rule, kind, object) in [
        (ReasonClass::Data, RuleId::Unspecified, "Catalog", OBJECT),
        (ReasonClass::Unknown, RuleId::RowUnknown, "", "Enum.X"),
        (
            ReasonClass::Structure,
            RuleId::ColumnAddedOrDropped,
            "AccumulationRegister",
            "AccumulationRegister.X",
        ),
    ] {
        let mut check = check_of_a2();
        check.push_reason(Reason {
            class,
            object: object.to_owned(),
            file_name: CATALOG.to_owned(),
            rule,
            kind: kind.to_owned(),
            path: vec![seg("ChildObjects", None), seg("Resource", Some("Р"))],
            op: Some(ChangeOp::Added),
            ..Reason::default()
        });
        let (verdict, phase) = decide(conservative(&[CATALOG]), &check, &base, &options());
        assert!(
            verdict.restructuring_required && phase.is_none(),
            "{rule:?}"
        );
    }

    // The check and the plan disagree about the attribute.
    let check = check_of(vec![attribute("ДругойРеквизит", ChangeOp::Added)]);
    let (verdict, phase) = decide(conservative(&[CATALOG]), &check, &base, &options());
    assert!(verdict.restructuring_required && phase.is_none());
    assert!(blocked_with(&verdict, "disagree"), "{:?}", verdict.blockers);

    // A conservative verdict with more blockers than it lists.
    let mut crowded = conservative(&[CATALOG]);
    crowded.blockers_omitted = 3;
    let (verdict, phase) = decide(crowded, &check_of_a2(), &base, &options());
    assert!(verdict.restructuring_required && phase.is_none());

    // The plan refuses (the images are the same: nothing is new, removed or retyped).
    let (verdict, phase) = decide(
        conservative(&[CATALOG]),
        &check_of_a2(),
        &inputs(OLD_ROW, OLD_ROW),
        &options(),
    );
    assert!(verdict.restructuring_required && phase.is_none());
    assert!(
        blocked_with(&verdict, "adds no attribute"),
        "{:?}",
        verdict.blockers
    );

    // A check that needs a restructuring and names no reason is refused: nothing is an operation.
    let silent = Verdict {
        needs_restructuring: true,
        ..Verdict::default()
    };
    let (verdict, phase) = decide(conservative(&[CATALOG]), &silent, &base, &options());
    assert!(verdict.restructuring_required && phase.is_none());
}

#[test]
fn an_index_switch_is_let_through_and_the_untraced_ones_are_not() {
    // DontIndex -> Index, and -> IndexWithAdditionalOrder, of the boolean "Клиент" of case a2.
    for (mode, word) in [(1u8, "Index"), (2u8, "IndexWithAdditionalOrder")] {
        let indexed = client_indexing(OLD_ROW, 0, mode);
        let staged = inputs(OLD_ROW, &indexed);
        let check = check_of(vec![indexing("Клиент", "DontIndex", word)]);
        let (verdict, phase) = decide(conservative(&[CATALOG]), &check, &staged, &options());
        assert!(
            !verdict.restructuring_required,
            "{word}: {:?}",
            verdict.blockers
        );
        let phase = phase.expect("a structure phase");
        assert!(
            phase.objects[0].contains("switched indexes Fld151 = Клиент"),
            "{:?}",
            phase.objects
        );
        assert!(phase.params_rewrites.is_empty() && phase.caches.is_empty());
        assert_eq!(phase.tables.len(), 3);

        // The check names another attribute than the plan switches: the decoders disagree.
        let other = check_of(vec![indexing("Другой", "DontIndex", word)]);
        let (verdict, phase) = decide(conservative(&[CATALOG]), &other, &staged, &options());
        assert!(verdict.restructuring_required && phase.is_none());
        assert!(blocked_with(&verdict, "disagree"), "{:?}", verdict.blockers);
    }

    // The check refuses the switch between the two indexed modes (not traced) ...
    let between = check_of(vec![indexing(
        "Клиент",
        "Index",
        "IndexWithAdditionalOrder",
    )]);
    let staged = inputs(OLD_ROW, &client_indexing(OLD_ROW, 0, 1));
    let (verdict, phase) = decide(conservative(&[CATALOG]), &between, &staged, &options());
    assert!(verdict.restructuring_required && phase.is_none());
    assert!(
        blocked_with(&verdict, "index-mode-outside-s1"),
        "{:?}",
        verdict.blockers
    );
    // ... and so does the plan when the check names it as a switch from DontIndex (the images say 1 -> 2).
    let mut both = inputs(
        &client_indexing(OLD_ROW, 0, 1),
        &client_indexing(OLD_ROW, 0, 2),
    );
    both.schema = crate::restructure::plan::plan(&staged, &options())
        .unwrap()
        .new_schema;
    let check = check_of(vec![indexing(
        "Клиент",
        "DontIndex",
        "IndexWithAdditionalOrder",
    )]);
    let (verdict, phase) = decide(conservative(&[CATALOG]), &check, &both, &options());
    assert!(verdict.restructuring_required && phase.is_none());
    assert!(
        blocked_with(&verdict, "only DontIndex <-> Index"),
        "{:?}",
        verdict.blockers
    );
}

/// The extensions of the infobase (S1-I): the objects they adopt come with the plan's input.
fn with_extensions(adopted: &[(&str, &str)]) -> crate::restructure::plan::Inputs {
    use crate::mssql_dump::extension::AdoptedObject;
    use crate::restructure::extensions::{Adoption, ExtensionInputs};
    let mut staged = inputs(OLD_ROW, NEW_ROW);
    staged.extensions = ExtensionInputs {
        registered: 2,
        adoptions_read: true,
        adoptions: adopted
            .iter()
            .map(|(extension, name)| Adoption {
                extension: (*extension).to_owned(),
                image: "active",
                object: AdoptedObject {
                    row: "3014d9c1-cb00-49fb-81b3-e8ced354975f".to_owned(),
                    uuid: "3014d9c1-cb00-49fb-81b3-e8ced354975f".to_owned(),
                    name: (*name).to_owned(),
                    extends: None,
                },
            })
            .collect(),
        ..ExtensionInputs::default()
    };
    staged
}

#[test]
fn an_attribute_of_a_catalog_an_extension_adopts_is_refused_and_another_object_is_not() {
    // The extension has an object of its own uuid and the name of the catalog: adopted by identity.
    let (verdict, phase) = decide(
        conservative(&[CATALOG]),
        &check_of_a2(),
        &with_extensions(&[("_ДемоРасширение", "_ДемоПартнеры")]),
        &options(),
    );
    assert!(verdict.restructuring_required && phase.is_none());
    assert!(
        blocked_with(&verdict, "is adopted by the extension _ДемоРасширение"),
        "{:?}",
        verdict.blockers
    );
    // An extension whose adopted objects are other ones lets the stage through.
    let (verdict, phase) = decide(
        conservative(&[CATALOG]),
        &check_of_a2(),
        &with_extensions(&[("_ДемоРасширение", "_ДемоНоменклатура")]),
        &options(),
    );
    assert!(!verdict.restructuring_required, "{:?}", verdict.blockers);
    assert!(phase.is_some());
    // Extensions whose objects were not read are a refusal, not a guess.
    let mut unread = with_extensions(&[]);
    unread.extensions.adoptions_read = false;
    let (verdict, phase) = decide(
        conservative(&[CATALOG]),
        &check_of_a2(),
        &unread,
        &options(),
    );
    assert!(verdict.restructuring_required && phase.is_none());
    assert!(
        blocked_with(&verdict, "were not read"),
        "{:?}",
        verdict.blockers
    );
}

#[test]
fn a_deleted_list_of_a_removed_form_and_a_removed_attribute_is_judged_for_the_attribute_only() {
    let added = crate::restructure::plan::plan(&inputs(OLD_ROW, NEW_ROW), &options()).unwrap();
    let mut back = inputs(NEW_ROW, OLD_ROW);
    back.schema = added.new_schema.clone();
    back.main_names = added.new_names_row.clone();
    let form = "8a7546f4-bfc9-4732-bf60-43a41e2c8753";
    let attribute_id = "c60cdc87-198a-4f6e-8f17-76bcb1b1914b";
    // the platform's own import: the rows of the form with the flag 0, the attribute with the flag 1
    let list = format!("\u{feff}4,\"{form}\",0,\"{form}.0\",0,\"{form}.1\",0,\"{attribute_id}\",1");
    let stored = crate::restructure::names::deflate(list.as_bytes()).unwrap();
    let check = check_of(vec![attribute("ДемоНовыйРеквизит", ChangeOp::Removed)]);

    // the plan alone: the rows of a form are no attributes, the whole list is refused
    back.staged.deleted = Some(stored.clone());
    let (verdict, phase) = decide(conservative(&[CATALOG]), &check, &back, &options());
    assert!(verdict.restructuring_required && phase.is_none());
    assert!(
        blocked_with(&verdict, &format!("deletes {form}")),
        "{:?}",
        verdict.blockers
    );

    // the apply has executed the form's rows itself: the gate judges the attribute that is left
    let executed: HashSet<String> = [form.to_owned(), format!("{form}.0"), format!("{form}.1")]
        .into_iter()
        .collect();
    back.staged.deleted = without_names(Some(stored.clone()), &executed).unwrap();
    let left =
        crate::restructure::plan::parse_deleted(back.staged.deleted.as_deref().unwrap()).unwrap();
    assert_eq!(left, vec![(attribute_id.to_owned(), 1)]);
    let (verdict, phase) = decide(conservative(&[CATALOG]), &check, &back, &options());
    assert!(!verdict.restructuring_required, "{:?}", verdict.blockers);
    let phase = phase.expect("a structure phase");
    assert!(phase.objects[0].contains("removed attributes Fld11034"));
    assert_eq!(phase.consumed_staged_rows, 1);

    // a row the apply did not execute stays in the list, and the plan refuses it
    let partly: HashSet<String> = [form.to_owned(), format!("{form}.0")].into_iter().collect();
    back.staged.deleted = without_names(Some(stored), &partly).unwrap();
    let (verdict, phase) = decide(conservative(&[CATALOG]), &check, &back, &options());
    assert!(verdict.restructuring_required && phase.is_none());
    assert!(
        blocked_with(&verdict, &format!("deletes {form}.1")),
        "{:?}",
        verdict.blockers
    );

    // a stage with no list has nothing to strip
    assert_eq!(without_names(None, &executed).unwrap(), None);
}

fn section_reason(name: &str) -> Reason {
    structure(
        RuleId::TabularSectionAddedDroppedMoved,
        "Catalog",
        OBJECT,
        vec![seg("ChildObjects", None), seg("TabularSection", Some(name))],
        Some(ChangeOp::Added),
    )
}

fn section_attribute(section: &str, name: &str) -> Reason {
    structure(
        RuleId::TabularSectionColumnAddedDroppedMoved,
        "Catalog",
        OBJECT,
        vec![
            seg("ChildObjects", None),
            seg("TabularSection", Some(section)),
            seg("ChildObjects", None),
            seg("Attribute", Some(name)),
        ],
        Some(ChangeOp::Added),
    )
}

#[test]
fn a_new_section_and_a_new_attribute_of_an_old_one_are_let_through_and_the_decoders_must_agree() {
    use super::tests_sections::{
        SECTIONS, attributes_of, collection, fresh, recount, rename, staged, with_new_section,
    };
    use crate::restructure::object::ObjectFacts;

    // The new section НоваяТЧ of the helper, and a new attribute in the second stored section.
    let mut root = with_new_section();
    let stored = ObjectFacts::parse(&super::tests_sections::tree()).unwrap();
    let old_section = stored.sections()[1].name.clone();
    let template = stored.sections()[1].attributes[0].name.clone();
    let sections = collection(&mut root, SECTIONS);
    let attributes = attributes_of(&mut sections[3]);
    let mut added = fresh(&attributes[2], 7, true);
    rename(&mut added, &template, "ДемоРеквизитТЧ");
    attributes.push(added);
    recount(attributes);
    let inputs = staged(&root);

    let check = check_of(vec![
        section_reason("НоваяТЧ"),
        section_attribute(&old_section, "ДемоРеквизитТЧ"),
    ]);
    let (verdict, phase) = decide(conservative(&[CATALOG]), &check, &inputs, &options());
    assert!(!verdict.restructuring_required, "{:?}", verdict.blockers);
    let phase = phase.expect("a structure phase");
    assert_eq!(
        phase.tables,
        [
            "_Reference20",
            "_Reference20_VT155",
            "_Reference20_VT159",
            "_Reference20_VT11035"
        ]
    );
    assert!(phase.objects[0].contains("new tabular sections VT11035 = НоваяТЧ"));
    assert!(phase.objects[0].contains("new attributes of tabular sections"));
    assert!(
        phase
            .sql
            .contains("create table dbo._Reference20_VT11035NG")
    );
    assert!(
        phase
            .sql
            .contains("the created table _Reference20_VT11035 is not empty")
    );
    assert!(!phase.sql.contains("drop table dbo._Reference20_VT11035;"));
    assert!(phase.params_rewrites.is_empty());

    // The check names the section but not the attribute, or another section, or another attribute.
    for (what, reasons) in [
        ("without the attribute", vec![section_reason("НоваяТЧ")]),
        (
            "another section",
            vec![
                section_reason("Другая"),
                section_attribute(&old_section, "ДемоРеквизитТЧ"),
            ],
        ),
        (
            "another attribute",
            vec![
                section_reason("НоваяТЧ"),
                section_attribute(&old_section, "Другой"),
            ],
        ),
        (
            "another section of the attribute",
            vec![
                section_reason("НоваяТЧ"),
                section_attribute("Другая", "ДемоРеквизитТЧ"),
            ],
        ),
    ] {
        let (verdict, phase) = decide(
            conservative(&[CATALOG]),
            &check_of(reasons),
            &inputs,
            &options(),
        );
        assert!(verdict.restructuring_required && phase.is_none(), "{what}");
        assert!(
            blocked_with(&verdict, "disagree"),
            "{what}: {:?}",
            verdict.blockers
        );
    }

    // A section attribute dropped or moved is the classification's refusal, however the plan reads it.
    let mut dropped = section_attribute(&old_section, "ДемоРеквизитТЧ");
    dropped.op = Some(ChangeOp::Removed);
    let (verdict, phase) = decide(
        conservative(&[CATALOG]),
        &check_of(vec![section_reason("НоваяТЧ"), dropped]),
        &inputs,
        &options(),
    );
    assert!(verdict.restructuring_required && phase.is_none());
    assert!(
        blocked_with(&verdict, "tabular-section-outside-s1"),
        "{:?}",
        verdict.blockers
    );
}
