//! The restructure check of track rcheck (#338) as the structural gate.
//!
//! [`crate::apply_check::check_staged`] reads the `ConfigSave` of a database
//! and its active `Config` (with the online-update overlay folded in) and says
//! whether applying the stage changes tables, columns, indexes or data the
//! platform derives from the configuration. It fails closed: a change no rule
//! lists as harmless, a row it cannot read and a row it does not know are
//! reasons of class `unknown`, and any reason makes the stage a restructuring.
//! This gate refuses on exactly that, with the check's own message
//! («требуется штатный config apply: <причины>»).
//!
//! It judges what the platform's own apply would do to the database. What is
//! specific to this apply -- which new rows it can register, how it places
//! their records -- stays in [`super::objects`] and is merged into the verdict
//! by the plan, whichever gate runs.

use std::collections::HashSet;

use anyhow::Result;

use crate::apply_check::{ReasonClass, Verdict, check_staged};
use crate::sql::SqlExec;

use super::gate::{
    GateBlocker, GateInput, GateVerdict, MAX_LISTED_BLOCKERS, RestructureCheckStats, StructuralGate,
};

/// The default gate of the own apply.
pub struct ApplyCheckGate<'a> {
    sql: &'a SqlExec,
    xml_version: Option<&'static str>,
}

impl<'a> ApplyCheckGate<'a> {
    /// `xml_version` is the XML dialect the descriptors are decoded with
    /// (`2.20` for 8.3, `2.21` for 8.5); `None` lets the check infer it from
    /// the shape of the Configuration row.
    pub fn new(sql: &'a SqlExec, xml_version: Option<&'static str>) -> Self {
        Self { sql, xml_version }
    }
}

impl StructuralGate for ApplyCheckGate<'_> {
    fn name(&self) -> &'static str {
        "apply-check"
    }

    fn check(&self, input: &GateInput<'_>) -> Result<GateVerdict> {
        let verdict = check_staged(self.sql, input.database, self.xml_version)?;
        Ok(gate_verdict(verdict, input.consumed_rows))
    }
}

/// The gate's answer for a verdict of the restructure check. The reasons that
/// name a row the caller consumes without moving it (the `deleted` list, when
/// the caller has read it and it asks for nothing) are dropped first: the
/// check does not know that row and calls it `unknown`.
pub(super) fn gate_verdict(mut verdict: Verdict, consumed: &HashSet<String>) -> GateVerdict {
    if !consumed.is_empty() {
        let before = verdict.reasons.len();
        verdict
            .reasons
            .retain(|reason| !consumed.contains(&reason.file_name.to_ascii_lowercase()));
        if verdict.reasons.len() != before {
            verdict.needs_restructuring = !verdict.reasons.is_empty();
        }
    }
    // Refuse on a restructuring, on any reason the check could not place
    // (`unknown`), and on a verdict that left files with data out.
    let unknown = verdict
        .reasons
        .iter()
        .any(|reason| reason.class == ReasonClass::Unknown);
    let required = verdict.needs_restructuring || unknown || !verdict.is_conclusive();
    if required {
        verdict.needs_restructuring = true;
    }
    let refusal = verdict.refusal().or_else(|| {
        required.then(|| {
            "требуется штатный config apply: проверка не смогла исключить реструктуризацию"
                .to_owned()
        })
    });

    let mut gate = GateVerdict {
        restructuring_required: required,
        gate: "apply-check".to_owned(),
        refusal,
        ..GateVerdict::default()
    };
    for reason in verdict.reasons.iter().take(MAX_LISTED_BLOCKERS) {
        gate.blockers.push(GateBlocker {
            row: reason.file_name.clone(),
            reason: format!("[{}] {}", reason.class.label(), reason.summary()),
        });
    }
    gate.blockers_omitted = verdict.reasons.len().saturating_sub(MAX_LISTED_BLOCKERS);
    gate.stats.bodies_by_role = verdict.stats.body_rows_by_role.clone();
    gate.stats.restructure_check = Some(RestructureCheckStats {
        staged_rows: verdict.stats.staged_rows,
        descriptors_compared: verdict.stats.descriptors_compared,
        body_rows_compared: verdict.stats.body_rows_compared,
        objects_changed: verdict.objects.len(),
        notes: verdict.notes.len() + verdict.stats.notes_dropped,
    });
    gate
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apply_check::Reason;

    fn reason(class: ReasonClass, file_name: &str, change: &str) -> Reason {
        Reason {
            class,
            object: format!("Object {file_name}"),
            file_name: file_name.to_owned(),
            property: "Properties/X".to_owned(),
            change: change.to_owned(),
            ..Reason::default()
        }
    }

    fn none() -> HashSet<String> {
        HashSet::new()
    }

    #[test]
    fn a_stage_with_no_reason_passes_and_keeps_the_figures() {
        let mut verdict = Verdict::new("rows");
        verdict.stats.staged_rows = 30;
        verdict.stats.descriptors_compared = 2;
        verdict
            .stats
            .body_rows_by_role
            .insert("Module".to_owned(), 7);
        let gate = gate_verdict(verdict, &none());
        assert!(!gate.restructuring_required);
        assert!(gate.refusal.is_none());
        assert!(gate.blockers.is_empty());
        assert_eq!(gate.gate, "apply-check");
        assert_eq!(gate.stats.bodies_by_role.get("Module"), Some(&7));
        let check = gate.stats.restructure_check.expect("the check's figures");
        assert_eq!((check.staged_rows, check.descriptors_compared), (30, 2));
    }

    #[test]
    fn a_structural_reason_refuses_in_the_checks_own_words() {
        let mut verdict = Verdict::new("rows");
        verdict.push_reason(reason(ReasonClass::Structure, "aaaa", "9 -> 12"));
        let gate = gate_verdict(verdict, &none());
        assert!(gate.restructuring_required);
        let refusal = gate.refusal.expect("a refusal");
        assert!(
            refusal.starts_with("требуется штатный config apply: "),
            "{refusal}"
        );
        assert!(refusal.contains("9 -> 12"), "{refusal}");
        assert_eq!(gate.blockers.len(), 1);
        assert!(gate.blockers[0].reason.starts_with("[structure] "));
        assert_eq!(gate.blockers[0].row, "aaaa");
    }

    #[test]
    fn an_unknown_reason_refuses_even_when_the_flag_was_not_raised() {
        let mut verdict = Verdict::new("rows");
        // a verdict built by hand: a reason of class unknown without the flag
        verdict.reasons.push(reason(
            ReasonClass::Unknown,
            "bbbb",
            "a row the check does not know",
        ));
        assert!(!verdict.needs_restructuring);
        let gate = gate_verdict(verdict, &none());
        assert!(gate.restructuring_required);
        assert!(gate.refusal.is_some());
    }

    #[test]
    fn a_verdict_that_left_files_out_is_not_a_pass() {
        let mut verdict = Verdict::new("tree-db");
        verdict.incomplete = true;
        let gate = gate_verdict(verdict, &none());
        assert!(gate.restructuring_required);
        assert!(
            gate.refusal
                .expect("a refusal")
                .starts_with("требуется штатный config apply")
        );
    }

    #[test]
    fn the_reason_of_a_consumed_row_is_dropped_and_nothing_else() {
        let mut verdict = Verdict::new("rows");
        verdict.push_reason(reason(
            ReasonClass::Unknown,
            "deleted",
            "ConfigSave holds a row the check does not know",
        ));
        let mut consumed = HashSet::new();
        consumed.insert("deleted".to_owned());
        // alone, the row was the only reason: the stage passes
        let gate = gate_verdict(verdict.clone(), &consumed);
        assert!(!gate.restructuring_required);
        assert!(gate.refusal.is_none());
        // not consumed, it refuses
        assert!(gate_verdict(verdict.clone(), &none()).restructuring_required);
        // next to a real reason, the real one stays
        verdict.push_reason(reason(ReasonClass::Data, "cccc", "content changed"));
        let gate = gate_verdict(verdict, &consumed);
        assert!(gate.restructuring_required);
        assert_eq!(gate.blockers.len(), 1);
        assert_eq!(gate.blockers[0].row, "cccc");
    }

    #[test]
    fn the_blocker_list_is_capped_and_the_rest_counted() {
        let mut verdict = Verdict::new("rows");
        for index in 0..(MAX_LISTED_BLOCKERS + 7) {
            verdict.push_reason(reason(
                ReasonClass::Structure,
                &format!("row{index}"),
                "added",
            ));
        }
        let gate = gate_verdict(verdict, &none());
        assert_eq!(gate.blockers.len(), MAX_LISTED_BLOCKERS);
        assert_eq!(gate.blockers_omitted, 7);
    }
}
