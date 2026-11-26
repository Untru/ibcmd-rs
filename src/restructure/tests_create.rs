pub(super) use super::test_create_support::*;
use crate::restructure::caches::tests_corpus::{ROOT_ROW, lab_snap};

mod gate {
    use super::*;
    use crate::apply_check::{ChangeOp, Reason, ReasonClass, RuleId, Seg, Verdict};
    use crate::mssql_config_apply::gate::{GateBlocker, GateVerdict};
    use crate::restructure::s1::decide;

    const NEW_DOCUMENT: &str = "32c4ee6e-b84a-4d6c-a195-033eafd1577f";

    fn reason(object: &str, kind: &str, file: &str, path: Vec<Seg>) -> Reason {
        Reason {
            class: ReasonClass::Structure,
            object: object.to_owned(),
            file_name: file.to_owned(),
            property: String::new(),
            change: "x".to_owned(),
            rule: RuleId::ObjectWithStorageAddedOrDropped,
            kind: kind.to_owned(),
            path,
            op: Some(ChangeOp::Added),
        }
    }

    /// The check's verdict for the new document: the object and its listing in the configuration.
    fn check(name: &str) -> Verdict {
        let mut verdict = Verdict::new("rows");
        verdict.push_reason(reason(
            &format!("Document.{name}"),
            "Document",
            NEW_DOCUMENT,
            Vec::new(),
        ));
        verdict.push_reason(reason(
            "Configuration",
            "Configuration",
            ROOT_ROW,
            vec![
                Seg {
                    name: "ChildObjects".to_owned(),
                    label: None,
                },
                Seg {
                    name: "Document".to_owned(),
                    label: Some(name.to_owned()),
                },
            ],
        ));
        verdict
    }

    /// What the conservative gate says of the staged rows of case d: a changed configuration descriptor and a
    /// new one.
    fn conservative() -> GateVerdict {
        GateVerdict {
            restructuring_required: true,
            blockers: [ROOT_ROW, NEW_DOCUMENT]
                .into_iter()
                .map(|row| GateBlocker {
                    row: row.to_owned(),
                    reason: "a metadata change".to_owned(),
                })
                .collect(),
            gate: "conservative".to_owned(),
            ..GateVerdict::default()
        }
    }

    #[test]
    fn the_gate_lets_a_new_document_through_with_a_phase_that_creates_its_tables() {
        let before = lab_snap!("d_staged");
        let inputs = inputs_of(&before);
        let (verdict, phase) = decide(
            conservative(),
            &check("ДемоНовыйДокумент"),
            &inputs,
            &options(),
        );
        assert!(!verdict.restructuring_required, "{:?}", verdict.blockers);
        let phase = phase.expect("a phase");
        assert_eq!(phase.tables, ["_Document11034", "_Document11034_VT11036"]);
        assert!(phase.objects[0].contains("Document"), "{:?}", phase.objects);
        assert!(phase.sql.contains("_Document11034NG"));
        assert_eq!(phase.created.len(), 1);
        assert_eq!(phase.created[0].uuid, NEW_DOCUMENT);
        assert_eq!(phase.created[0].kind, "Document");
        assert_eq!(phase.answered_rows, [ROOT_ROW]);
        eprintln!("created: {:?}", phase.created);
    }

    #[test]
    fn the_gate_withdraws_the_blockers_of_the_module_rows_of_a_created_object() {
        let before = lab_snap!("d_staged");
        let mut inputs = inputs_of(&before);
        let module = format!("{NEW_DOCUMENT}.0");
        inputs.staged.new_files.insert(module.clone());
        let mut verdict = conservative();
        verdict.blockers.push(GateBlocker {
            row: module.clone(),
            reason: "the body's owner is not an object of the active configuration".to_owned(),
        });
        let (verdict, phase) = decide(verdict, &check("ДемоНовыйДокумент"), &inputs, &options());
        assert!(!verdict.restructuring_required, "{:?}", verdict.blockers);
        let phase = phase.expect("a phase");
        assert_eq!(phase.created[0].files, [module]);

        // a row of another object stays a blocker
        let mut verdict = conservative();
        verdict.blockers.push(GateBlocker {
            row: "11111111-1111-4111-8111-111111111111.0".to_owned(),
            reason: "the body's owner is not an object of the active configuration".to_owned(),
        });
        let (verdict, phase) = decide(verdict, &check("ДемоНовыйДокумент"), &inputs, &options());
        assert!(phase.is_none());
        assert!(verdict.restructuring_required);
    }

    #[test]
    fn a_created_object_with_a_file_that_is_no_module_or_help_page_is_not_built() {
        let before = lab_snap!("d_staged");
        let mut inputs = inputs_of(&before);
        // a document has no such body
        inputs.staged.new_files.insert(format!("{NEW_DOCUMENT}.9"));
        let (verdict, phase) = decide(
            conservative(),
            &check("ДемоНовыйДокумент"),
            &inputs,
            &options(),
        );
        assert!(phase.is_none());
        assert!(verdict.restructuring_required);
        let text = format!("{:?}", verdict.blockers);
        assert!(text.contains("not a module or a help page"), "{text}");
    }

    #[test]
    fn the_gate_refuses_a_new_object_the_plan_does_not_create() {
        let before = lab_snap!("d_staged");
        let inputs = inputs_of(&before);
        // the check names another document than the one the stage creates
        let mut wrong = check("ДругойДокумент");
        for reason in &mut wrong.reasons {
            if reason.kind == "Document" {
                reason.file_name = "11111111-1111-4111-8111-111111111111".to_owned();
            }
        }
        let (verdict, phase) = decide(conservative(), &wrong, &inputs, &options());
        assert!(phase.is_none());
        assert!(verdict.restructuring_required);
    }
}
