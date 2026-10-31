//! Refusing what needs a restructuring.
//!
//! The own apply moves rows; it does not change the database's structure.
//! Whether a staged configuration needs that is decided here, behind
//! [`StructuralGate`], so that the full check of the restructure-check track
//! (`check_staged`) can replace [`ConservativeGate`] at the one call site in
//! `plan`.
//!
//! The conservative rule admits exactly what the acceptance of #337 names and
//! nothing else:
//!
//! - the service rows `root` and `version` unchanged, `versions` replaced;
//! - a descriptor row that exists in `Config` and inflates to the same text
//!   (only the compression differs);
//! - a body row whose owner kind and suffix the source-asset registry names as
//!   a module, a form, a template, a picture or a help page.
//!
//! Every other change -- a descriptor whose text differs, a new object, a
//! predefined-data, rights, interface or package body, an unknown row name --
//! is a blocker.

use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::Result;
use serde::Serialize;

use crate::metadata_model::brace::parse_row;
use crate::metadata_model::export::names::{
    may_own_objects, own_header, owned_objects, root_kinds,
};
use crate::sql::{SqlClient, SqlParam};
use ibcmd_compiler_families::compiler::families::assets::{SourceAssetRegistry, SourceAssetRole};

use super::model::{RowMeta, RowName, classify_name, quote_ident};
use super::sqlgen::ParamsRewrite;
use super::versions::{inflate_row, strip_bom};

/// The conservative rule's reason for a descriptor of an existing object whose text differs from the active one: it
/// cannot tell a synonym from a column, so it refuses them all. The S1 gate withdraws this blocker for a row that the
/// restructuring check has read and finds harmless.
pub const DESCRIPTOR_DIFFERS: &str =
    "the descriptor's text differs from the active one: a metadata change, possibly structural";

/// One reason the staged configuration is not for the own apply.
#[derive(Debug, Clone, Serialize)]
pub struct GateBlocker {
    pub row: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct GateStats {
    pub rows_identical: usize,
    pub descriptors_layout_only: usize,
    pub service_rows: usize,
    /// Parts beyond the first of a multi-part body row.
    pub extra_parts: usize,
    /// New forms and templates the caller analysed and accepted.
    pub new_objects: usize,
    /// Changed body rows by the role the registry gives them.
    pub bodies_by_role: BTreeMap<String, usize>,
    /// What the restructure check looked at (the `apply-check` gate only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restructure_check: Option<RestructureCheckStats>,
}

/// The figures of the restructure check's verdict that a report keeps.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RestructureCheckStats {
    pub staged_rows: usize,
    pub descriptors_compared: usize,
    pub body_rows_compared: usize,
    /// Objects whose descriptor differs, harmless or not.
    pub objects_changed: usize,
    pub notes: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct GateVerdict {
    /// The staged configuration needs a restructuring (or something the own
    /// apply does not know how to do): refuse.
    pub restructuring_required: bool,
    pub blockers: Vec<GateBlocker>,
    /// Blockers beyond the ones listed.
    pub blockers_omitted: usize,
    pub stats: GateStats,
    pub gate: String,
    /// The refusal in the words the gate has for it (the restructure check
    /// speaks Russian: «требуется штатный config apply: ...»); the
    /// conservative gate leaves it to the caller.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
}

pub(super) const MAX_LISTED_BLOCKERS: usize = 200;

impl GateVerdict {
    /// Adds a blocker. Public: a gate outside this module blocks what it does not
    /// cover, too.
    pub fn block(&mut self, row: &str, reason: impl Into<String>) {
        self.restructuring_required = true;
        if self.blockers.len() < MAX_LISTED_BLOCKERS {
            self.blockers.push(GateBlocker {
                row: row.to_owned(),
                reason: reason.into(),
            });
        } else {
            self.blockers_omitted += 1;
        }
    }
}

/// What a gate may look at.
pub struct GateInput<'a> {
    pub client: &'a dyn SqlClient,
    pub database: &'a str,
    /// Every `ConfigSave` row.
    pub staged: &'a [RowMeta],
    /// The `Config` rows that share a name with a staged row, by lower-cased
    /// (name, part).
    pub active: &'a HashMap<(String, i32), RowMeta>,
    /// New rows the caller has analysed and accepted (lower-cased names): a
    /// form or template of an existing object and its bodies, a body row an
    /// existing object gains.
    pub accepted_new_rows: &'a HashSet<String>,
    /// Descriptors that differ from the active row only by the references to
    /// accepted new objects (lower-cased names).
    pub accepted_owner_descriptors: &'a HashSet<String>,
    /// The kind of every accepted new object, by uuid, for the role check of
    /// its bodies.
    pub new_object_kinds: &'a HashMap<String, &'static str>,
    /// Staged rows the caller consumes without moving them into `Config`
    /// (lower-cased names): the `deleted` list, when it is empty or names
    /// only the rows of a dynamic update. A gate does not judge them.
    pub consumed_rows: &'a HashSet<String>,
    /// Names of the stage's `deleted` list (lower-cased) that the caller has executed itself: the rows of
    /// the removed forms and templates. They are no staged rows. A gate that judges the list (the S1
    /// gate, for the attributes it names) leaves them out.
    pub removed_rows: &'a HashSet<String>,
}

/// What a gate that lets a restructuring through hands to the apply: the structure work as T-SQL for
/// the apply's own transaction, and the `Params` cache rows it makes stale. The apply runs the text
/// after its fingerprint assertions and before it folds the dynamic generations and moves the rows
/// (one transaction: a failed assertion rolls the rebuilt tables back with everything else), and
/// writes the cache rows through its guarded rewrite ([`ParamsRewrite`]), together with its own.
///
/// The text assumes what the script gives it: a transaction with `XACT_ABORT ON`, the exclusive locks
/// on `Config`, `ConfigSave`, `Params` and `Files`, exclusive access to the database, the variable
/// `@now` (the timestamp the platform writes). It declares variables with the prefix `@ddl_` only and
/// does not read `Config`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StructurePhase {
    #[serde(skip)]
    pub sql: String,
    /// Staged rows the phase answers for (the stage's `deleted` row, which lists the attributes the
    /// stage removes): the apply consumes them like an empty `deleted` list -- they are not moved into
    /// `Config` and go with the rest of `ConfigSave` -- but only when a phase says it has judged them.
    pub consumed_staged_rows: usize,
    #[serde(skip)]
    pub params_rewrites: Vec<ParamsRewrite>,
    /// The tables the phase rebuilds or creates.
    pub tables: Vec<String>,
    /// One line per changed object.
    pub objects: Vec<String>,
    /// One line per cache row the phase rewrites (`Params.<row>: what`).
    pub caches: Vec<String>,
    /// The catalogs and documents the phase creates. The apply moves their staged rows like any staged row and
    /// registers them at the exchange-plan nodes (the phase answers for the objects, not for their rows).
    pub created: Vec<CreatedObject>,
    /// Staged rows besides the created objects' and their files whose analysis blockers the phase answers for
    /// (lower-cased): the configuration's descriptor, which lists the objects the phase creates.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub answered_rows: Vec<String>,
    /// The size guard's verdict on the tables the phase rebuilds: the limit and where it came from, the
    /// totals, the largest table (`restructure::size_guard`, S1-J). A gate without one leaves it out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_check: Option<serde_json::Value>,
}

/// A catalog or a document a structure phase creates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreatedObject {
    /// The uuid of its descriptor row.
    pub uuid: String,
    /// `Catalog` or `Document`.
    pub kind: String,
    /// The staged rows of the object besides its descriptor (`<uuid>.0`, `<uuid>.1`, ...), in the order
    /// their files are registered.
    pub files: Vec<String>,
}

pub trait StructuralGate {
    fn name(&self) -> &'static str;
    fn check(&self, input: &GateInput<'_>) -> Result<GateVerdict>;

    /// The structure phase the last [`StructuralGate::check`] prepared for the stage, when the gate
    /// lets a restructuring through instead of refusing it. Taken once; a gate that refuses
    /// restructurings (the conservative one, the restructure check) has none.
    fn take_structure(&self) -> Option<StructurePhase> {
        None
    }

    /// Whether the gate judges a stage's `deleted` row itself. The apply consumes an empty list and a list
    /// of the rows of a dynamic update and refuses every other; a gate that lets attribute deletions
    /// through says `true` and must then hand over a phase whose `consumed_staged_rows` is not 0, or the
    /// apply refuses the list after all.
    fn judges_deleted_row(&self) -> bool {
        false
    }
}

/// Two gates in a row: what the first passes goes through as it is; what it refuses is handed to the
/// second, which may refuse it again or let it through with a structure phase. The second gate is asked
/// only when the first refuses, so a gate that can do more (the S1 gate) never makes a stage that the
/// first passes harder to pass: the conservative rule under the S1 gate refuses every descriptor whose
/// text differs, harmless or not, and the restructure check does not.
///
/// What both refuse is refused for the reasons of both, the first gate's first: its refusals name every
/// row it will not pass, the second gate's own reasons say what of that it cannot do either. The decision
/// is the second gate's, as before; only the list of reasons is the two lists.
pub struct FirstThen<'a> {
    first: Box<dyn StructuralGate + 'a>,
    then: Box<dyn StructuralGate + 'a>,
}

impl<'a> FirstThen<'a> {
    pub fn new(first: Box<dyn StructuralGate + 'a>, then: Box<dyn StructuralGate + 'a>) -> Self {
        Self { first, then }
    }
}

impl StructuralGate for FirstThen<'_> {
    /// The gate that answers for a restructuring.
    fn name(&self) -> &'static str {
        self.then.name()
    }

    fn check(&self, input: &GateInput<'_>) -> Result<GateVerdict> {
        let first = self.first.check(input)?;
        if !first.restructuring_required {
            return Ok(first);
        }
        let mut then = self.then.check(input)?;
        if then.restructuring_required {
            let own = std::mem::take(&mut then.blockers);
            then.blockers_omitted += first.blockers_omitted;
            then.blockers = first.blockers;
            for blocker in own {
                if then.blockers.len() < MAX_LISTED_BLOCKERS {
                    then.blockers.push(blocker);
                } else {
                    then.blockers_omitted += 1;
                }
            }
            if then.stats.restructure_check.is_none() {
                then.stats.restructure_check = first.stats.restructure_check;
            }
        }
        Ok(then)
    }

    fn take_structure(&self) -> Option<StructurePhase> {
        self.then.take_structure()
    }

    fn judges_deleted_row(&self) -> bool {
        self.then.judges_deleted_row()
    }
}

/// The rule above.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConservativeGate {
    /// Also pass changed body rows of the roles that are neither structural
    /// nor a module/form/template/picture/help page -- command interface,
    /// rights, package, splash and the like -- without proving that their text
    /// is unchanged. For a stage that `infobase config import` made from a tree
    /// exported out of this very database: the importer rewrites every row of
    /// the tree, and some kinds come back in another (equivalent) layout.
    pub admit_unverified_roles: bool,
}

impl StructuralGate for ConservativeGate {
    fn name(&self) -> &'static str {
        "conservative"
    }

    fn check(&self, input: &GateInput<'_>) -> Result<GateVerdict> {
        let mut verdict = GateVerdict {
            gate: self.name().to_owned(),
            ..GateVerdict::default()
        };
        let mut changed_bodies: Vec<(&RowMeta, String, String)> = Vec::new();
        let mut descriptors_to_compare = 0usize;
        let mut root_to_compare = false;
        // A row of several parts (the platform cuts a value at 10 MB) is one change:
        // any part that is new or differs makes its name changed, and its first
        // part stands for it in the role check.
        let mut changed_names: HashSet<String> = HashSet::new();
        let mut first_parts: HashSet<String> = HashSet::new();
        let mut multi_part: HashSet<String> = HashSet::new();
        for row in input.staged {
            let name = row.name.to_ascii_lowercase();
            if input.consumed_rows.contains(&name) {
                continue;
            }
            if row.part == 0 {
                first_parts.insert(name.clone());
            } else {
                multi_part.insert(name.clone());
            }
            if input
                .active
                .get(&row.key())
                .is_none_or(|active| active.sha256 != row.sha256)
            {
                changed_names.insert(name);
            }
        }
        for (name, part) in input.active.keys() {
            if *part != 0 {
                multi_part.insert(name.clone());
            }
        }
        for row in input.staged {
            let active = input.active.get(&row.key());
            let identical = active.is_some_and(|active| active.sha256 == row.sha256);
            let name_key = row.name.to_ascii_lowercase();
            if input.consumed_rows.contains(&name_key) {
                continue;
            }
            match classify_name(&row.name) {
                RowName::Service(name) => {
                    verdict.stats.service_rows += 1;
                    if name == "root" && !identical && row.part == 0 {
                        // the row that names the configuration's descriptor: the platform stores it as a
                        // stored block, the importer deflates it; the text is what counts
                        root_to_compare = true;
                    } else if name != "versions" && !identical {
                        verdict.block(&row.name, format!("the service row {name} changes"));
                    }
                }
                // an unchanged first part of a body row another part of which changes
                RowName::Body { owner, suffix }
                    if identical && row.part == 0 && changed_names.contains(&name_key) =>
                {
                    changed_bodies.push((row, owner.to_owned(), suffix.to_owned()));
                }
                _ if identical => verdict.stats.rows_identical += 1,
                RowName::Descriptor(_) if row.part != 0 => verdict.block(
                    &row.name,
                    "a descriptor with a part number other than 0",
                ),
                RowName::Descriptor(_) => match active {
                    Some(_) => descriptors_to_compare += 1,
                    None if input.accepted_new_rows.contains(&name_key) => {
                        verdict.stats.new_objects += 1;
                    }
                    None => verdict.block(
                        &row.name,
                        "a new object the own apply cannot create (only a new form or template of an existing object)",
                    ),
                },
                RowName::Body { owner, suffix } => {
                    if row.part != 0 {
                        if first_parts.contains(&name_key) {
                            verdict.stats.extra_parts += 1;
                        } else {
                            verdict.block(
                                &row.name,
                                "a body part whose first part is not staged",
                            );
                        }
                    } else {
                        changed_bodies.push((row, owner.to_owned(), suffix.to_owned()));
                    }
                }
                RowName::Other => verdict.block(
                    &row.name,
                    "a row name the own apply does not know (not a service row, descriptor or body row)",
                ),
            }
        }
        if descriptors_to_compare > 0 {
            compare_descriptors(input, &mut verdict)?;
        }
        if root_to_compare {
            compare_root(input, &mut verdict)?;
        }
        if !changed_bodies.is_empty() {
            classify_bodies(
                input,
                &changed_bodies,
                &multi_part,
                self.admit_unverified_roles,
                &mut verdict,
            )?;
        }
        Ok(verdict)
    }
}

const DESCRIPTOR_PATTERN: &str = "________-____-____-____-____________";

/// Descriptor rows whose bytes differ from the active row's must still inflate
/// to the same text.
fn compare_descriptors(input: &GateInput<'_>, verdict: &mut GateVerdict) -> Result<()> {
    let db = quote_ident(input.database)?;
    let query = format!(
        "SELECT s.FileName, s.BinaryData, c.BinaryData FROM {db}.dbo.ConfigSave s JOIN {db}.dbo.Config c ON c.FileName = s.FileName AND c.PartNo = s.PartNo \
         WHERE s.PartNo = 0 AND s.FileName LIKE N'{DESCRIPTOR_PATTERN}' AND HASHBYTES('SHA2_256', s.BinaryData) <> HASHBYTES('SHA2_256', c.BinaryData) ORDER BY s.FileName"
    );
    input.client.read_rows(&query, &[], &mut |mut row| {
        let name = row.take_text(0)?;
        let staged = row.take_binary(1)?;
        let active = row.take_binary(2)?;
        // The caller has proved that this one differs by references to new
        // objects and nothing else.
        if input
            .accepted_owner_descriptors
            .contains(&name.to_ascii_lowercase())
        {
            return Ok(());
        }
        match (inflate_row(&staged), inflate_row(&active)) {
            (Ok(staged), Ok(active)) if staged == active => {
                verdict.stats.descriptors_layout_only += 1;
            }
            (Ok(_), Ok(_)) => verdict.block(&name, DESCRIPTOR_DIFFERS),
            _ => verdict.block(&name, "a descriptor row that does not inflate"),
        }
        Ok(())
    })
}

/// The `root` row that differs from the active one in its bytes must still inflate to the same text (it names
/// the configuration's descriptor; a database whose root the platform once rewrote keeps it as a stored block).
fn compare_root(input: &GateInput<'_>, verdict: &mut GateVerdict) -> Result<()> {
    let db = quote_ident(input.database)?;
    let query = format!(
        "SELECT s.BinaryData, c.BinaryData FROM {db}.dbo.ConfigSave s JOIN {db}.dbo.Config c ON c.FileName = s.FileName AND c.PartNo = s.PartNo WHERE s.PartNo = 0 AND s.FileName IN (N'root')"
    );
    input.client.read_rows(&query, &[], &mut |mut row| {
        let staged = row.take_binary(0)?;
        let active = row.take_binary(1)?;
        match (inflate_row(&staged), inflate_row(&active)) {
            (Ok(staged), Ok(active)) if staged == active => {
                verdict.stats.descriptors_layout_only += 1;
            }
            _ => verdict.block("root", "the service row root changes"),
        }
        Ok(())
    })
}

/// The kind of every object a body row can belong to.
#[derive(Default)]
struct KindMap {
    by_uuid: HashMap<String, &'static str>,
    /// Uuids named inside an owner's descriptor that have no descriptor row:
    /// nested objects such as commands.
    nested: HashSet<String>,
}

fn read_kinds(input: &GateInput<'_>) -> Result<KindMap> {
    let db = quote_ident(input.database)?;
    // The configuration's own descriptor is named by the `root` row.
    let root = input
        .client
        .query_scalar(
            &format!(
                "SELECT BinaryData FROM {db}.dbo.Config WHERE FileName = N'root' AND PartNo = 0"
            ),
            &[],
        )?
        .and_then(|value| match value {
            crate::sql::SqlValue::Binary(bytes) => Some(bytes),
            _ => None,
        });
    let mut config_uuid = None;
    if let Some(bytes) = root {
        let plain = inflate_row(&bytes)?;
        let text = std::str::from_utf8(strip_bom(&plain))
            .unwrap_or_default()
            .to_owned();
        config_uuid = text
            .trim()
            .strip_prefix("{2,")
            .and_then(|rest| rest.split(',').next())
            .map(|value| value.trim().to_ascii_lowercase());
    }
    let Some(config_uuid) = config_uuid else {
        anyhow::bail!("the root row does not name the configuration");
    };
    let mut kinds = KindMap::default();
    let mut descriptors: Vec<(String, String)> = Vec::new();
    let query = format!(
        "SELECT FileName, BinaryData FROM {db}.dbo.Config WHERE PartNo = 0 AND FileName LIKE N'{DESCRIPTOR_PATTERN}'"
    );
    let mut top_level: Option<HashMap<String, &'static str>> = None;
    let mut configuration_object: Option<String> = None;
    input.client.read_rows(&query, &[], &mut |mut row| {
        let name = row.take_text(0)?.to_ascii_lowercase();
        let bytes = row.take_binary(1)?;
        let Ok(plain) = inflate_row(&bytes) else {
            return Ok(());
        };
        let Ok(text) = String::from_utf8(strip_bom(&plain).to_vec()) else {
            return Ok(());
        };
        if name == config_uuid
            && let Ok(tree) = parse_row(&plain)
        {
            top_level = Some(root_kinds(&tree));
            // The configuration object has an id of its own; its modules
            // and pages are the body rows of that id.
            configuration_object = own_header(&tree).map(|(uuid, _)| uuid);
        }
        descriptors.push((name, text));
        Ok(())
    })?;
    if let Some(top_level) = top_level {
        for (uuid, kind) in top_level {
            kinds.by_uuid.insert(uuid, kind);
        }
    }
    kinds.by_uuid.insert(config_uuid, "Configuration");
    if let Some(uuid) = configuration_object {
        kinds.by_uuid.insert(uuid, "Configuration");
    }
    let mut owned = Vec::new();
    for (name, text) in &descriptors {
        if kinds.by_uuid.contains_key(name)
            && may_own_objects(text)
            && let Ok(tree) = parse_row(text.as_bytes())
        {
            owned.extend(owned_objects(&tree));
        }
    }
    for (kind, uuid) in owned {
        kinds.by_uuid.entry(uuid).or_insert(kind);
    }
    // Nested objects (commands): a uuid in a top-level descriptor's text that
    // is no row of its own.
    kinds.nested = descriptors
        .iter()
        .filter(|(name, _)| kinds.by_uuid.contains_key(name))
        .flat_map(|(_, text)| uuids_in(text))
        .filter(|uuid| !kinds.by_uuid.contains_key(uuid))
        .collect();
    Ok(kinds)
}

fn uuids_in(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index + 36 <= bytes.len() {
        if bytes[index + 8] == b'-'
            && bytes[index + 13] == b'-'
            && bytes[index + 18] == b'-'
            && bytes[index + 23] == b'-'
            && super::model::is_uuid_text(&text[index..index + 36])
        {
            out.push(text[index..index + 36].to_ascii_lowercase());
            index += 36;
        } else {
            index += 1;
        }
    }
    out
}

/// The roles a body row may have and still leave the database's structure
/// alone.
fn role_is_admitted(role: SourceAssetRole) -> bool {
    matches!(
        role,
        SourceAssetRole::Module
            | SourceAssetRole::OrdinaryApplicationModule
            | SourceAssetRole::ExternalConnectionModule
            | SourceAssetRole::ManagedApplicationModule
            | SourceAssetRole::SessionModule
            | SourceAssetRole::CommandModule
            | SourceAssetRole::FormModule
            | SourceAssetRole::ObjectModule
            | SourceAssetRole::ManagerModule
            | SourceAssetRole::ValueManagerModule
            | SourceAssetRole::RecordSetModule
            | SourceAssetRole::Picture
            | SourceAssetRole::Help
    )
}

/// The family the registry knows an owner kind by.
fn registry_family(kind: &str) -> &str {
    kind
}

/// Roles that change neither the database's structure nor code, and that
/// `admit_unverified_roles` lets through.
fn role_is_unverified_ok(role: SourceAssetRole) -> bool {
    matches!(
        role,
        SourceAssetRole::CommandInterface
            | SourceAssetRole::Splash
            | SourceAssetRole::ParentConfigurations
            | SourceAssetRole::HomePageWorkArea
            | SourceAssetRole::MainSectionCommandInterface
            | SourceAssetRole::MobileClientSignature
            | SourceAssetRole::ClientApplicationInterface
            | SourceAssetRole::MainSectionPicture
            | SourceAssetRole::StandaloneContent
            | SourceAssetRole::Rights
            | SourceAssetRole::Package
    )
}

fn classify_bodies(
    input: &GateInput<'_>,
    changed: &[(&RowMeta, String, String)],
    multi_part: &HashSet<String>,
    admit_unverified: bool,
    verdict: &mut GateVerdict,
) -> Result<()> {
    let kinds = read_kinds(input)?;
    // Rows whose role is not admitted may still be a no-op: same text, other
    // compression.
    let mut to_compare: Vec<(&RowMeta, String, Comparator)> = Vec::new();
    for (row, owner, suffix) in changed {
        let owner_key = owner.to_ascii_lowercase();
        let kind = match kinds
            .by_uuid
            .get(&owner_key)
            .or_else(|| input.new_object_kinds.get(&owner_key))
        {
            Some(kind) => *kind,
            None if kinds.nested.contains(&owner_key) => "Command",
            None => {
                verdict.block(
                    &row.name,
                    "the body's owner is not an object of the active configuration",
                );
                continue;
            }
        };
        // Templates carry no registry route: their `.0` body is the template.
        if matches!(kind, "Template" | "CommonTemplate") && suffix == "0" {
            *verdict
                .stats
                .bodies_by_role
                .entry("Template".to_owned())
                .or_default() += 1;
            continue;
        }
        // An exchange plan's content body lists the objects it exchanges; the
        // importer writes the list in another order. The same set is no change.
        if kind == "ExchangePlan" && suffix == "1" {
            to_compare.push((
                row,
                "the exchange plan's content (the set of exchanged objects) differs".to_owned(),
                Comparator::UnorderedContent,
            ));
            continue;
        }
        match SourceAssetRegistry.route_by_suffix(registry_family(kind), suffix) {
            Some(route) if role_is_admitted(route.role()) => {
                *verdict
                    .stats
                    .bodies_by_role
                    .entry(format!("{:?}", route.role()))
                    .or_default() += 1;
            }
            Some(route) if admit_unverified && role_is_unverified_ok(route.role()) => {
                *verdict
                    .stats
                    .bodies_by_role
                    .entry(format!("{:?} (unverified)", route.role()))
                    .or_default() += 1;
            }
            Some(route) => to_compare.push((
                row,
                format!("a {kind} body of role {:?}: not a module, form, template, picture or help page", route.role()),
                // The standalone content lists ids; the importer sorts them.
                if route.role() == SourceAssetRole::StandaloneContent {
                    Comparator::UnorderedUuids
                } else {
                    Comparator::Inflated
                },
            )),
            None => to_compare.push((
                row,
                format!("a {kind} body with suffix .{suffix} that the source-asset registry does not name"),
                Comparator::Inflated,
            )),
        }
    }
    // A text comparison reads the first part only: a row of several parts whose
    // role would need one cannot be proved unchanged.
    let (multi, single): (Vec<_>, Vec<_>) = to_compare
        .into_iter()
        .partition(|(row, _, _)| multi_part.contains(&row.name.to_ascii_lowercase()));
    for (row, reason, _) in &multi {
        verdict.block(
            &row.name,
            format!("{reason}; it has several parts, and only a single-part text can be compared"),
        );
    }
    compare_bodies(input, &single, verdict)
}

/// How a body row that no role admits is compared with the active row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Comparator {
    /// The inflated texts are equal.
    Inflated,
    /// `{2,N,<id>,<flag>,...,0}`: the same pairs, in any order.
    UnorderedContent,
    /// The same text with its uuids blanked out, and the same uuids in any
    /// order.
    UnorderedUuids,
}

/// Two texts equal but for the order of their uuids.
fn uuid_sets_equal(staged: &[u8], active: &[u8]) -> bool {
    let (Ok(staged), Ok(active)) = (
        std::str::from_utf8(strip_bom(staged)),
        std::str::from_utf8(strip_bom(active)),
    ) else {
        return false;
    };
    let blank = |text: &str| {
        let mut out = String::with_capacity(text.len());
        let bytes = text.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if index + 36 <= bytes.len()
                && text.is_char_boundary(index)
                && text.is_char_boundary(index + 36)
                && super::model::is_uuid_text(&text[index..index + 36])
            {
                out.push('#');
                index += 36;
            } else {
                let ch = text[index..]
                    .chars()
                    .next()
                    .expect("index is a char boundary");
                out.push(ch);
                index += ch.len_utf8();
            }
        }
        out
    };
    let mut a = uuids_in(staged);
    let mut b = uuids_in(active);
    a.sort();
    b.sort();
    a == b && blank(staged) == blank(active)
}

/// Content bodies (`{2,N,<uuid>,<flag>,...,0}`) equal as sets of pairs.
fn content_sets_equal(staged: &[u8], active: &[u8]) -> bool {
    fn pairs(bytes: &[u8]) -> Option<Vec<(String, String)>> {
        let text = std::str::from_utf8(strip_bom(bytes)).ok()?;
        let inner = text.trim().strip_prefix('{')?.strip_suffix('}')?;
        let tokens = inner.split(',').map(str::trim).collect::<Vec<_>>();
        if tokens.first() != Some(&"2") {
            return None;
        }
        let count = tokens.get(1)?.parse::<usize>().ok()?;
        if tokens.len() != 2 + 2 * count + 1 || tokens.last() != Some(&"0") {
            return None;
        }
        let mut out = tokens[2..2 + 2 * count]
            .chunks(2)
            .map(|pair| (pair[0].to_ascii_lowercase(), pair[1].to_owned()))
            .collect::<Vec<_>>();
        out.sort();
        Some(out)
    }
    match (pairs(staged), pairs(active)) {
        (Some(staged), Some(active)) => staged == active,
        _ => false,
    }
}

/// The rows that are not admitted by role must be no change once compared as
/// their kind requires.
fn compare_bodies(
    input: &GateInput<'_>,
    rows: &[(&RowMeta, String, Comparator)],
    verdict: &mut GateVerdict,
) -> Result<()> {
    let db = quote_ident(input.database)?;
    let comparators: HashMap<String, Comparator> = rows
        .iter()
        .map(|(row, _, comparator)| (row.name.to_lowercase(), *comparator))
        .collect();
    let mut equal: HashSet<String> = HashSet::new();
    for chunk in rows.chunks(400) {
        let placeholders = (1..=chunk.len())
            .map(|index| format!("@P{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!(
            "SELECT s.FileName, s.BinaryData, c.BinaryData FROM {db}.dbo.ConfigSave s JOIN {db}.dbo.Config c ON c.FileName = s.FileName AND c.PartNo = s.PartNo              WHERE s.PartNo = 0 AND s.FileName IN ({placeholders})"
        );
        let params = chunk
            .iter()
            .map(|(row, _, _)| SqlParam::Text(row.name.as_str()))
            .collect::<Vec<_>>();
        input.client.read_rows(&query, &params, &mut |mut row| {
            let name = row.take_text(0)?;
            let staged = row.take_binary(1)?;
            let active = row.take_binary(2)?;
            let key = name.to_lowercase();
            let is_equal = match comparators.get(&key) {
                Some(Comparator::UnorderedContent) => match (inflate_row(&staged), inflate_row(&active)) {
                    (Ok(staged), Ok(active)) => content_sets_equal(&staged, &active),
                    _ => false,
                },
                Some(Comparator::UnorderedUuids) => match (inflate_row(&staged), inflate_row(&active)) {
                    (Ok(staged), Ok(active)) => uuid_sets_equal(&staged, &active),
                    _ => false,
                },
                _ => matches!((inflate_row(&staged), inflate_row(&active)), (Ok(staged), Ok(active)) if staged == active),
            };
            if is_equal {
                equal.insert(key);
            }
            Ok(())
        })?;
    }
    for (row, reason, _) in rows {
        if equal.contains(&row.name.to_lowercase()) {
            *verdict
                .stats
                .bodies_by_role
                .entry("layout-only".to_owned())
                .or_default() += 1;
        } else {
            verdict.block(&row.name, reason.as_str());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuids_are_found_in_descriptor_text() {
        let text = "{3,{1,0,b4e1435c-482c-4e14-afee-045b1489fcd0},\"X\",7f89218f-417a-4e94-9be0-0085d83c9c2d,B866D5A8-2333-4865-961B-A5C8E9BFC05E}";
        let found = uuids_in(text);
        assert_eq!(found.len(), 3);
        assert!(found.contains(&"b866d5a8-2333-4865-961b-a5c8e9bfc05e".to_owned()));
    }

    #[test]
    fn admitted_roles_are_modules_forms_pictures_and_help() {
        assert!(role_is_admitted(SourceAssetRole::ObjectModule));
        assert!(role_is_admitted(SourceAssetRole::FormModule));
        assert!(role_is_admitted(SourceAssetRole::Help));
        assert!(role_is_admitted(SourceAssetRole::Picture));
        assert!(!role_is_admitted(SourceAssetRole::Rights));
        assert!(!role_is_admitted(SourceAssetRole::Predefined));
        assert!(!role_is_admitted(SourceAssetRole::CommandInterface));
        assert!(!role_is_admitted(SourceAssetRole::Package));
    }

    #[test]
    fn the_registry_names_the_bodies_the_gate_admits() {
        let route = SourceAssetRegistry.route_by_suffix("Catalog", "3").unwrap();
        assert_eq!(route.role(), SourceAssetRole::ManagerModule);
        let route = SourceAssetRegistry.route_by_suffix("Form", "0").unwrap();
        assert_eq!(route.role(), SourceAssetRole::FormModule);
        let route = SourceAssetRegistry.route_by_suffix("Command", "2").unwrap();
        assert_eq!(route.role(), SourceAssetRole::CommandModule);
        // predefined data has a route, with a role the gate does not admit
        let route = SourceAssetRegistry
            .route_by_suffix("Catalog", "1c")
            .unwrap();
        assert_eq!(route.role(), SourceAssetRole::Predefined);
        assert!(!role_is_admitted(route.role()));
    }

    #[test]
    fn exchange_plan_content_compares_as_a_set() {
        let a = b"{2,3,190bc52e-0d51-4dfc-9f16-99ed71ebfa75,0,f81b51c5-57e4-41f8-9ad5-3d2717351b95,0,1685c406-d0cb-4e0a-a3b0-036c8c22942a,1,0}";
        let b = b"{2,3,1685c406-d0cb-4e0a-a3b0-036c8c22942a,1,190bc52e-0d51-4dfc-9f16-99ed71ebfa75,0,f81b51c5-57e4-41f8-9ad5-3d2717351b95,0,0}";
        assert!(content_sets_equal(a, b));
        // another flag, another set
        let c = b"{2,3,1685c406-d0cb-4e0a-a3b0-036c8c22942a,0,190bc52e-0d51-4dfc-9f16-99ed71ebfa75,0,f81b51c5-57e4-41f8-9ad5-3d2717351b95,0,0}";
        assert!(!content_sets_equal(a, c));
        assert!(!content_sets_equal(a, b"{2,3,x}"));
    }

    #[test]
    fn uuid_lists_compare_in_any_order_but_not_across_structure() {
        let a = b"{2,2,190bc52e-0d51-4dfc-9f16-99ed71ebfa75,f81b51c5-57e4-41f8-9ad5-3d2717351b95}";
        let b = b"{2,2,f81b51c5-57e4-41f8-9ad5-3d2717351b95,190bc52e-0d51-4dfc-9f16-99ed71ebfa75}";
        assert!(uuid_sets_equal(a, b));
        let c = b"{2,3,f81b51c5-57e4-41f8-9ad5-3d2717351b95,190bc52e-0d51-4dfc-9f16-99ed71ebfa75}";
        assert!(!uuid_sets_equal(a, c));
        let d = b"{2,2,f81b51c5-57e4-41f8-9ad5-3d2717351b95,00000000-0d51-4dfc-9f16-99ed71ebfa75}";
        assert!(!uuid_sets_equal(a, d));
    }

    #[test]
    fn a_blocker_list_is_capped_but_counted() {
        let mut verdict = GateVerdict::default();
        for index in 0..(MAX_LISTED_BLOCKERS + 5) {
            verdict.block(&format!("row{index}"), "x");
        }
        assert!(verdict.restructuring_required);
        assert_eq!(verdict.blockers.len(), MAX_LISTED_BLOCKERS);
        assert_eq!(verdict.blockers_omitted, 5);
    }
}

#[cfg(test)]
#[path = "gate_parts_tests.rs"]
mod parts_tests;
