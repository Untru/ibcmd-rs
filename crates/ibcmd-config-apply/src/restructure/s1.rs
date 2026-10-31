//! The structural gate of the own apply for the minimal set S1 (docs/apply/restructuring.md, section 12).
//!
//! The apply asks its gate whether the staged configuration is something it may move. The conservative
//! gate refuses every descriptor whose text differs. This gate lets through the changes of the set S1 --
//! built so far: new, removed, widened (variable strings) and re-indexed attributes of catalogs and documents -- and
//! hands the apply the structure work to run in its own transaction ([`StructurePhase`]); everything else
//! stays a refusal, with the reason.
//!
//! The decision is a chain, and every link fails closed:
//!
//! 1. the conservative gate's verdict: the descriptors that differ are its blockers;
//! 2. the restructuring check (`apply_check::check_staged`) names each change (its typed *reasons*), and
//!    `apply_check::s1::classify` maps every reason onto an operation of S1 or a refusal that names why
//!    (a `data` or `unknown` reason, another kind of object, another property ...); an operation that is
//!    designed but not built here is a refusal too;
//! 3. the plan ([`crate::restructure::plan::plan`]) reads the descriptors itself and must arrive at the same
//!    objects and attributes as the check ([`decide`]: two independent decoders agreeing is the evidence);
//! 4. the blockers of the planned objects' descriptors are withdrawn -- and if any other blocker is left,
//!    the stage holds more than the restructuring and the refusal stands.
//!
//! The `deleted` row of a stage that removes attributes lists their ids. The check does not read it, the
//! plan does (the ids must be attributes the stage removes, and nothing else), and the apply consumes it
//! when this gate says it judges it and hands over a phase that answers for it.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashSet};

use anyhow::Result;
use sha2::{Digest, Sha256};

use crate::apply_check::s1::{S1Operation, classify};
use crate::apply_check::{RuleId, Verdict, check_staged};
use crate::mssql_config_apply::AdoptionReader;
use crate::mssql_config_apply::gate::{
    ConservativeGate, CreatedObject, DESCRIPTOR_DIFFERS, GateInput, GateVerdict, StructuralGate,
    StructurePhase,
};
use crate::mssql_config_apply::sqlgen::ParamsRewrite;
use crate::restructure::plan::{Inputs, Plan, PlanOptions, plan};
use crate::restructure::reader::{ClientSource, read_inputs};
use crate::restructure::size_guard::{LimitSetting, guard_phase};
use crate::sql::SqlExec;

pub const GATE_NAME: &str = "s1";

fn sha256_upper(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect()
}

/// The stored `deleted` row without the given names (lower-cased); `None` stays `None`.
pub(super) fn without_names(
    stored: Option<Vec<u8>>,
    names: &HashSet<String>,
) -> Result<Option<Vec<u8>>> {
    let Some(stored) = stored else {
        return Ok(None);
    };
    let kept: Vec<(String, i64)> = crate::restructure::plan::parse_deleted(&stored)?
        .into_iter()
        .filter(|(id, _)| !names.contains(id))
        .collect();
    let mut text = format!("\u{feff}{}", kept.len());
    for (id, flag) in &kept {
        text.push_str(&format!(",\"{id}\",{flag}"));
    }
    Ok(Some(crate::restructure::names::deflate(text.as_bytes())?))
}

/// The plan as the apply takes it: T-SQL for its transaction and the cache rows as guarded rewrites. The
/// stage's `deleted` row, when there is one, is answered for by the phase (`consumed_staged_rows`): the
/// apply does not move it into `Config` and drops it with the rest of `ConfigSave`.
fn phase_of(plan: &Plan, inputs: &Inputs) -> Result<StructurePhase> {
    let mut params_rewrites = Vec::new();
    for cache in &plan.caches {
        let stored = inputs
            .cache_rows
            .iter()
            .find(|(name, _)| *name == cache.row_name)
            .map(|(_, bytes)| bytes)
            .ok_or_else(|| anyhow::anyhow!("Params.{} was not read", cache.row_name))?;
        params_rewrites.push(ParamsRewrite {
            file_name: cache.row_name.clone(),
            old_data_size: inputs
                .cache_sizes
                .get(&cache.row_name)
                .copied()
                .unwrap_or(stored.len() as i64),
            old_sha256_hex: sha256_upper(stored),
            new_bytes: cache.row.clone(),
            set_creation: cache.set_creation,
        });
    }
    let sql = plan.phase_sql("@now")?;
    Ok(StructurePhase {
        sql,
        consumed_staged_rows: usize::from(inputs.staged.deleted.is_some()),
        params_rewrites,
        tables: plan
            .tables()
            .map(|table| table.table.name.clone())
            .collect(),
        objects: plan
            .objects
            .iter()
            .map(|object| {
                format!(
                    "{} {} ({}): {}",
                    object.kind.label(),
                    object.object_name,
                    object.object,
                    object.changes()
                )
            })
            .collect(),
        caches: plan
            .caches
            .iter()
            .map(|cache| format!("Params.{}: {}", cache.row_name, cache.what))
            .collect(),
        created: plan
            .objects
            .iter()
            .filter(|object| object.created)
            .map(|object| {
                let prefix = format!("{}.", object.object_uuid.to_ascii_lowercase());
                CreatedObject {
                    uuid: object.object_uuid.clone(),
                    kind: match object.kind {
                        crate::restructure::object::ObjectKind::Catalog => "Catalog",
                        crate::restructure::object::ObjectKind::Document => "Document",
                    }
                    .to_owned(),
                    files: inputs
                        .staged
                        .new_files
                        .iter()
                        .filter(|name| name.to_ascii_lowercase().starts_with(&prefix))
                        .cloned()
                        .collect(),
                }
            })
            .collect(),
        answered_rows: if plan.objects.iter().any(|object| object.created) {
            crate::restructure::plan::configuration_uuid(&inputs.root_row)
                .into_iter()
                .collect()
        } else {
            Vec::new()
        },
        size_check: None,
    })
}

/// The decision, from what the database said: the conservative verdict, the check's verdict, the plan's
/// input. Pure, so that the refusals are testable without a database.
pub fn decide(
    mut verdict: GateVerdict,
    check: &Verdict,
    inputs: &Inputs,
    options: &PlanOptions,
) -> (GateVerdict, Option<StructurePhase>) {
    verdict.gate = format!("{GATE_NAME} (the conservative rule and the S1 restructurings)");
    if !verdict.restructuring_required {
        // Nothing the conservative gate refuses: a plain apply.
        return (verdict, None);
    }
    // The conservative rule's blockers are held apart. It re-lists every descriptor and body that differs,
    // harmless or not, and the restructuring check, which the apply asks first, names the rows it refuses.
    // A refusal of this gate therefore carries its own reasons (`S1: ...`) and nothing else; the conservative
    // lines come back only where they are what refuses the stage (the last step below). The decision is the
    // same as if they had been kept: every refusal below is a reason of this gate.
    let mut conservative = std::mem::take(&mut verdict.blockers);
    let conservative_omitted = std::mem::take(&mut verdict.blockers_omitted);
    let refuse = |mut verdict: GateVerdict, row: &str, reason: String| {
        verdict.block(row, format!("S1: {reason}"));
        (verdict, None)
    };

    let mut classification = classify(check);
    // The `deleted` row of a stage that removes attributes: the check does not know a list of ids and calls
    // it unknown; the plan reads it below and lets it through when it names removed attributes and nothing
    // else.
    classification
        .refusals
        .retain(|refusal| refusal.rule != RuleId::DeletedRowNotEmpty);
    if !classification.refusals.is_empty() {
        for refusal in &classification.refusals {
            verdict.block("", format!("S1: {}", refusal.message()));
        }
        return (verdict, None);
    }
    // Operations that are designed (12.3) but not built in this version.
    let mut unbuilt = false;
    for operation in &classification.operations {
        if !matches!(
            operation,
            S1Operation::AddAttribute { .. }
                | S1Operation::DeleteAttribute { .. }
                | S1Operation::WidenString { .. }
                | S1Operation::SwitchIndex { .. }
                | S1Operation::AddObject { .. }
                | S1Operation::AddTabularSection { .. }
                | S1Operation::AddSectionAttribute { .. }
        ) {
            unbuilt = true;
            verdict.block(
                &operation.object().row,
                format!(
                    "S1: {}: the S1 operation \"{}\" is designed (docs/apply/restructuring.md, 12.3) but not built in this version",
                    operation.object().full_name(),
                    operation.name()
                ),
            );
        }
    }
    if unbuilt {
        return (verdict, None);
    }
    if classification.operations.is_empty() {
        return refuse(
            verdict,
            "",
            "the conservative gate refuses descriptors, the restructuring check names no change in them"
                .to_owned(),
        );
    }

    let plan = match plan(inputs, options) {
        Ok(plan) => plan,
        Err(error) => return refuse(verdict, "", format!("{error:#}")),
    };

    // The two decoders agree: the same objects, the same new, removed, widened and re-indexed attributes, the
    // same new tabular sections and new attributes of the sections there were (`Section.Attribute`).
    type Names = [BTreeSet<String>; 6];
    let planned: BTreeMap<String, Names> = plan
        .objects
        .iter()
        .map(|object| {
            (
                object.object_uuid.to_ascii_lowercase(),
                [
                    object
                        .additions
                        .iter()
                        .map(|addition| addition.name.clone())
                        .collect(),
                    object
                        .removals
                        .iter()
                        .map(|removal| removal.name.clone())
                        .collect(),
                    object
                        .widenings
                        .iter()
                        .map(|widening| widening.name.clone())
                        .collect(),
                    object
                        .switches
                        .iter()
                        .map(|switch| switch.name.clone())
                        .collect(),
                    object
                        .sections
                        .iter()
                        .filter(|section| section.created.is_some())
                        .map(|section| section.name.clone())
                        .collect(),
                    object
                        .sections
                        .iter()
                        .filter(|section| section.created.is_none())
                        .flat_map(|section| {
                            section
                                .additions
                                .iter()
                                .map(|addition| format!("{}.{}", section.name, addition.name))
                        })
                        .collect(),
                ],
            )
        })
        .collect();
    let named_by_check: BTreeMap<String, Names> = classification
        .by_object()
        .into_iter()
        .map(|(object, operations)| {
            let names = |wanted: fn(&S1Operation) -> Option<String>| -> BTreeSet<String> {
                operations.iter().copied().filter_map(wanted).collect()
            };
            (
                object.row.to_ascii_lowercase(),
                [
                    names(|operation| match operation {
                        S1Operation::AddAttribute { attribute, .. } => Some(attribute.clone()),
                        _ => None,
                    }),
                    names(|operation| match operation {
                        S1Operation::DeleteAttribute { attribute, .. } => Some(attribute.clone()),
                        _ => None,
                    }),
                    names(|operation| match operation {
                        S1Operation::WidenString { attribute, .. } => Some(attribute.clone()),
                        _ => None,
                    }),
                    names(|operation| match operation {
                        S1Operation::SwitchIndex { attribute, .. } => Some(attribute.clone()),
                        _ => None,
                    }),
                    names(|operation| match operation {
                        S1Operation::AddTabularSection { section, .. } => Some(section.clone()),
                        _ => None,
                    }),
                    names(|operation| match operation {
                        S1Operation::AddSectionAttribute {
                            section, attribute, ..
                        } => Some(format!("{section}.{attribute}")),
                        _ => None,
                    }),
                ],
            )
        })
        .collect();
    if planned != named_by_check {
        return refuse(
            verdict,
            "",
            format!(
                "the plan changes {planned:?} and the restructuring check names {named_by_check:?}: the two decoders disagree"
            ),
        );
    }

    // The planned objects' descriptors are the blockers the plan answers for, and so is the stage's
    // `deleted` row (the plan checked that it names removed attributes only); another blocker is a
    // change this gate does not cover. The `root` row is answered for by the check: the conservative rule
    // compares its bytes, the check reads it (the 8.5 platform re-stamps the last block of its payload on
    // every write) and a root that really changed is a `service-row-changed` refusal above.
    let has_deleted = inputs.staged.deleted.is_some();
    // A created object is listed by the configuration's descriptor, which the stage changes for it (the check
    // paired the two reasons): the plan answers for that row when it creates something.
    let listing = if plan.objects.iter().any(|object| object.created) {
        crate::restructure::plan::configuration_uuid(&inputs.root_row).ok()
    } else {
        None
    };
    // The module, manager module and help rows of a created object have no active owner for the conservative
    // gate's role check; the plan admitted their suffixes (`create::plan_created`).
    let created_files: HashSet<String> = plan
        .objects
        .iter()
        .filter(|object| object.created)
        .flat_map(|object| {
            let prefix = format!("{}.", object.object_uuid.to_ascii_lowercase());
            inputs
                .staged
                .new_files
                .iter()
                .filter(move |name| name.to_ascii_lowercase().starts_with(&prefix))
                .map(|name| name.to_ascii_lowercase())
        })
        .collect();
    // A descriptor of an existing object whose text differs, and that no reason of the restructuring check names, is
    // one the check has read and finds harmless (a synonym, a comment, a presentation, a record format the staging
    // platform wrote): the conservative rule cannot tell it from a column and refuses it, the check, which the apply
    // asked first, accepted it, and applying it copies the row. Every other blocker is a limit of this apply
    // (a new object it cannot create, a row it does not know, a body of a role it does not pass), and stays.
    let refused_by_the_check: HashSet<String> = check
        .reasons
        .iter()
        .map(|reason| reason.file_name.to_ascii_lowercase())
        .collect();
    conservative.retain(|blocker| {
        let row = blocker.row.to_ascii_lowercase();
        !(blocker.reason == DESCRIPTOR_DIFFERS && !refused_by_the_check.contains(&row))
            && !planned.contains_key(&row)
            && !created_files.contains(&row)
            && !(has_deleted && row == "deleted")
            && row != "root"
            && listing.as_deref() != Some(row.as_str())
    });
    verdict.blockers.extend(conservative);
    verdict.blockers_omitted = conservative_omitted;
    verdict.restructuring_required = !verdict.blockers.is_empty() || verdict.blockers_omitted > 0;
    if verdict.restructuring_required {
        return (verdict, None);
    }
    match phase_of(&plan, inputs) {
        Ok(phase) => (verdict, Some(phase)),
        Err(error) => refuse(verdict, "", format!("{error:#}")),
    }
}

/// A refusal of this gate before it decides anything: its own reason, without the conservative rule's lines
/// (see [`decide`]).
pub(crate) fn own_refusal(mut verdict: GateVerdict, row: &str, reason: String) -> GateVerdict {
    verdict.blockers.clear();
    verdict.blockers_omitted = 0;
    verdict.block(row, reason);
    verdict
}

/// The reasons of the check that name a row the apply consumes without moving it (the stage's `deleted`
/// list, when the apply has read it) are dropped first, as the apply's own check gate does: the check does
/// not know that row.
fn without_consumed(mut check: Verdict, consumed: &HashSet<String>) -> Verdict {
    if !consumed.is_empty() {
        let before = check.reasons.len();
        check
            .reasons
            .retain(|reason| !consumed.contains(&reason.file_name.to_ascii_lowercase()));
        if check.reasons.len() != before {
            check.needs_restructuring = !check.reasons.is_empty();
        }
    }
    check
}

/// The gate: the conservative rule plus the S1 restructurings.
pub struct S1Gate<'a> {
    sql: &'a SqlExec,
    conservative: ConservativeGate,
    options: PlanOptions,
    /// The objects the extensions adopt (S1-I), read only when extensions are registered.
    read_adoptions: AdoptionReader,
    /// The XML dialect the check decodes descriptors with (`2.20` for 8.3); `None` lets it infer it.
    xml_version: Option<&'static str>,
    /// The limit on the rows and bytes the stage may rebuild (S1-J, `size_guard`).
    limit: LimitSetting,
    prepared: RefCell<Option<StructurePhase>>,
}

impl<'a> S1Gate<'a> {
    pub fn new(
        sql: &'a SqlExec,
        conservative: ConservativeGate,
        options: PlanOptions,
        read_adoptions: AdoptionReader,
    ) -> Self {
        Self {
            sql,
            conservative,
            options,
            read_adoptions,
            xml_version: None,
            limit: LimitSetting::default(),
            prepared: RefCell::new(None),
        }
    }

    /// The limit on the rebuilt tables; the measured default when not given.
    pub fn size_limit(mut self, limit: LimitSetting) -> Self {
        self.limit = limit;
        self
    }

    /// The XML dialect the restructure check decodes the descriptors with.
    pub fn xml_version(mut self, xml_version: Option<&'static str>) -> Self {
        self.xml_version = xml_version;
        self
    }
}

impl StructuralGate for S1Gate<'_> {
    fn name(&self) -> &'static str {
        GATE_NAME
    }

    fn check(&self, input: &GateInput<'_>) -> Result<GateVerdict> {
        self.prepared.borrow_mut().take();
        let verdict = self.conservative.check(input)?;
        if !verdict.restructuring_required {
            return Ok(GateVerdict {
                gate: GATE_NAME.to_owned(),
                ..verdict
            });
        }
        // The stage holds a change the conservative gate refuses: is it S1?
        let check = without_consumed(
            check_staged(self.sql, input.database, self.xml_version)?,
            input.consumed_rows,
        );
        let mut source = ClientSource {
            client: input.client,
            database: input.database,
        };
        let (mut inputs, storage) = read_inputs(&mut source)?;
        // The names of the `deleted` list that the apply has executed itself (the rows of removed forms
        // and templates) are not the plan's to judge: it reads the attributes that are left.
        if !input.removed_rows.is_empty() {
            inputs.staged.deleted =
                without_names(inputs.staged.deleted.take(), input.removed_rows)?;
        }
        // The objects the extensions adopt (S1-I): the plan refuses to change one. What cannot be read
        // is a refusal, not an error: the stage is then not one this gate can vouch for.
        if inputs.extensions.registered > 0 {
            match (self.read_adoptions)(self.sql, input.database) {
                Ok(adoptions) => {
                    inputs.extensions.adoptions = adoptions;
                    inputs.extensions.adoptions_read = true;
                }
                Err(error) => {
                    return Ok(own_refusal(
                        verdict,
                        "extensions",
                        format!(
                            "S1: the objects the extensions adopt could not be read: {error:#}"
                        ),
                    ));
                }
            }
        }
        let idle = storage
            .iter()
            .find(|row| row.schema_id == 0)
            .is_some_and(|row| row.is_idle());
        if !idle {
            return Ok(own_refusal(
                verdict,
                "SchemaStorage",
                "S1: the schema storage is not idle: an interrupted restructuring has to be finished first"
                    .to_owned(),
            ));
        }
        let (mut verdict, mut phase) = decide(verdict, &check, &inputs, &self.options);
        // S1-J: the tables the plan rebuilds are copied in one transaction; above the limit the stage is
        // refused and goes to the native apply.
        guard_phase(
            &mut source,
            self.options.method,
            &self.limit,
            &mut verdict,
            &mut phase,
        )?;
        *self.prepared.borrow_mut() = phase;
        Ok(verdict)
    }

    fn judges_deleted_row(&self) -> bool {
        true
    }

    fn take_structure(&self) -> Option<StructurePhase> {
        self.prepared.borrow_mut().take()
    }
}
