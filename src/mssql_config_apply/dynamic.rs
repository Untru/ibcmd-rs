//! The dynamic (online) `config apply`: `ibcmd infobase config apply --dynamic=force`.
//!
//! A small stage -- the rows of a few objects and the three service rows, the "delta" the platform's
//! own partial import stages -- is published as a generation **beside** the active rows while
//! sessions stay connected. The exclusive apply of this module's parent replaces rows in place with the
//! database to itself; this one writes `<name>_dynupdate_<generation>` rows and the
//! `DynamicallyUpdated` markers, replaces only `root` and `version`, and takes no table lock. Sessions
//! that are open keep the generation they loaded, sessions that open afterwards read the new one
//! (measured with a real client, `docs/apply/online-activation.md`); the next exclusive apply folds
//! every generation into the ordinary rows.
//!
//! Nothing here writes a row itself. The transition is the online engine's
//! (`mssql_main_activation`, mode `online`: aliases, markers, exact-stage assertions, an application
//! lock, row locks in a short serializable transaction), and the writes the platform's `force` makes
//! besides -- the change registrations of the changed objects (`_MessageNo` reset, the #412 rows of
//! the imaged nodes) and `Files.MobileVersions.dat` -- are the exclusive apply's own SQL
//! ([`sqlgen::render_parity_writes`]) run in the same transaction.
//!
//! What qualifies, and why it is narrower than what the engine takes (`docs/apply/dropin-dynamic.md`):
//!
//! - the stage is a delta: at most [`MAX_ROWS`] rows and [`MAX_PLAN_BYTES`] bytes, one part each;
//! - `root`, `version` and `versions` are staged, `root` and `version` unchanged;
//! - every other row is the descriptor or the `.0` body of a **common module or common form** that
//!   already exists, and a descriptor is unchanged in text: the kinds a session was measured with
//!   (other kinds are #345's), no new object, no removal, no change of an object's properties;
//! - the restructure check ([`ApplyCheckGate`]) finds no restructuring;
//! - the schema is settled; a pending overlay has the exact measured removal/service inventory
//!   (`dynamic_overlay`), otherwise no overlay in `Params`; the platform
//!   declares `mssql.config.apply.dynamic`.
//!
//! A stage that does not qualify is refused with the reasons ([`NeedsNativeApply`]); nothing is
//! written. A dynamic apply never falls back to an exclusive one: what a user asked to be dynamic is
//! not applied under a lock they did not ask for.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use sha2::{Digest, Sha256};

use crate::mssql_main_activation::{
    MAX_PLAN_BYTES, MAX_ROW_BYTES, MAX_ROWS, MainActivationError, MainActivationMode,
    MainActivationSnapshot, MainStorageRow, prepare_main_activation, render_main_activation_sql,
};
use crate::mssql_platform_profile::MssqlNativePlatformProfile;
use crate::sql::{ScriptVariables, SqlClient, SqlExec};

use super::check_gate::ApplyCheckGate;
use super::dynamic_metadata::{self, BodyRole, Owners};
use super::dynamic_overlay;
use super::errors::{self, DynamicUnsupported, NeedsNativeApply};
use super::gate::{GateInput, GateVerdict, StructuralGate};
use super::model::{RowMeta, RowName, classify_name, hex_lower, quote_ident};
use super::sqlgen::{self, ScriptInputs};
use super::{
    ApplyMode, ApplyTimings, ConfigApplyOptions, ConfigApplyReport, DynamicPublication,
    ROW_COLUMNS, RegistrationSummary, StageSummary, blank_report, ms, node_literals,
    other_sessions, plan_mobile_versions, read_blob, read_row_metas, recovery, registrations,
    require_client, require_no_unfinished_operation, require_schema_settled,
    require_settled_storage, scalar_i64, versions, write_artifact, xml_version_of,
};

/// The number of generations past which the report warns that the overlay grows.
pub const WARN_GENERATIONS: usize = 50;

/// Original reference graph and selected bodies read BEFORE a source export.
/// Private fields prevent callers from supplying SQL or manufacturing a newer
/// post-stage snapshot. The raw originals remain owned through publication.
#[derive(Debug, Clone)]
pub(crate) struct SourceOwnerPreimages {
    database: String,
    groups: Vec<SourcePreimageGroup>,
}

#[derive(Debug, Clone)]
struct SourcePreimageGroup {
    table: &'static str,
    filter: String,
    rows: Vec<RowMeta>,
    originals: Vec<Vec<u8>>,
}

impl SourcePreimageGroup {
    fn marker(&self) -> Option<&[u8]> {
        self.rows
            .iter()
            .position(|row| row.name.eq_ignore_ascii_case("DynamicallyUpdated"))
            .map(|index| self.originals[index].as_slice())
    }
}

impl SourceOwnerPreimages {
    #[cfg(test)]
    pub(crate) fn test_fixture(database: &str) -> Self {
        let client = tests::source_dependency_fixture();
        Self::capture_client(&client, database, &tests::source_dependency_names()).unwrap()
    }
    pub(crate) fn capture(sql: &SqlExec, database: &str, selected: &[String]) -> Result<Self> {
        Self::capture_client(require_client(sql)?, database, selected)
    }

    fn capture_client(client: &dyn SqlClient, database: &str, selected: &[String]) -> Result<Self> {
        let db = quote_ident(database)?;
        let mut requested = HashSet::new();
        let mut bodies = Vec::new();
        let mut unique = HashSet::new();
        for name in selected {
            if !unique.insert(name.to_ascii_lowercase()) {
                bail!("duplicate source dependency name");
            }
            match classify_name(name) {
                RowName::Descriptor(owner) => {
                    requested.insert(owner.to_ascii_lowercase());
                }
                RowName::Body { owner, suffix } => {
                    requested.insert(owner.to_ascii_lowercase());
                    bodies.push((owner.to_ascii_lowercase(), suffix.to_owned()));
                }
                _ => bail!("untyped source dependency {name}"),
            }
        }
        if bodies.is_empty() {
            bail!("source dependency closure has no body");
        }
        // The complete ordinary descriptor inventory is the reference graph
        // used by full-root export, including unchanged owners and children.
        let selected_filter = selected
            .iter()
            .map(|name| format!("N'{}'", super::model::quote_string(name)))
            .collect::<Vec<_>>()
            .join(", ");
        let filter = format!(
            "FileName = N'root' OR (LEN(FileName) = 36 AND TRY_CONVERT(uniqueidentifier, FileName) IS NOT NULL) OR FileName IN ({selected_filter})"
        );
        let mut budget = GraphBudget::default();
        let mut groups = Vec::new();
        for (table, filter) in [
            ("Config", filter),
            ("Config", dynamic_overlay::CONFIG_FILTER.to_owned()),
            ("Params", "FileName = N'DynamicallyUpdated'".to_owned()),
        ] {
            let rows = read_source_preimage_rows(client, &db, table, &filter)?;
            budget.reserve(&rows)?;
            let mut originals = Vec::new();
            for row in &rows {
                originals.push(dynamic_overlay::read_bound_blob(client, &db, table, row)?);
            }
            groups.push(SourcePreimageGroup {
                table,
                filter,
                rows,
                originals,
            });
        }
        let proof = Self {
            database: database.to_owned(),
            groups,
        };
        let ordinary = &proof.groups[0].rows;
        for name in selected {
            if ordinary
                .iter()
                .filter(|row| row.name.eq_ignore_ascii_case(name))
                .count()
                != 1
            {
                bail!("selected source dependency is missing or multipart: {name}");
            }
        }
        let history =
            versions::parse_dynamic_history(proof.groups[1].marker(), proof.groups[2].marker())?;
        let history_names = history
            .generations
            .iter()
            .map(|id| id.hyphenated().to_string())
            .collect::<Vec<_>>();
        let (owners, graph) = object_kinds(
            client,
            database,
            &requested,
            &proof.groups[1].rows,
            &history_names,
        )?;
        for row in &graph {
            if !ordinary.contains(row) {
                bail!("owner graph changed after initial source inventory");
            }
        }
        for (owner, suffix) in bodies {
            if !owners.admits(&owner) || owners.role(&owner, &suffix).is_none() {
                bail!("source body has no measured original owner/role: {owner}.{suffix}");
            }
        }
        proof.require_current_client(client, database)?;
        Ok(proof)
    }

    pub(crate) fn require_current(&self, sql: &SqlExec, database: &str) -> Result<()> {
        self.require_current_client(require_client(sql)?, database)
    }

    fn require_current_client(&self, client: &dyn SqlClient, database: &str) -> Result<()> {
        if self.database != database {
            bail!("source dependency proof belongs to a different database");
        }
        let db = quote_ident(database)?;
        for group in &self.groups {
            let current = read_source_preimage_rows(client, &db, group.table, &group.filter)?;
            require_same_source_inventory(&group.rows, &current)?;
            for (row, original) in group.rows.iter().zip(&group.originals) {
                dynamic_overlay::bound_blob(row, original)?;
            }
        }
        Ok(())
    }

    pub(crate) fn precondition_sql(&self, database: &str) -> Result<String> {
        if self.database != database {
            bail!("source dependency proof belongs to a different database");
        }
        let mut sql = "-- initial source dependency preimages\n".to_owned();
        for group in &self.groups {
            for (row, original) in group.rows.iter().zip(&group.originals) {
                dynamic_overlay::bound_blob(row, original)?;
            }
            sql.push_str(&dynamic_overlay::guard(
                group.table,
                &group.filter,
                &group.rows,
            ));
        }
        Ok(sql)
    }
}

fn require_same_source_inventory(original: &[RowMeta], current: &[RowMeta]) -> Result<()> {
    if original != current {
        bail!("initial source dependency headers/data/inventory changed");
    }
    Ok(())
}

fn read_source_preimage_rows(
    client: &dyn SqlClient,
    db: &str,
    table: &str,
    filter: &str,
) -> Result<Vec<RowMeta>> {
    let rows = read_row_metas(
        client,
        &format!(
            "SELECT TOP ({}) {ROW_COLUMNS} FROM {db}.dbo.{table} WHERE {filter} ORDER BY FileName, PartNo",
            dynamic_metadata::MAX_GRAPH_ROWS + 1
        ),
    )?;
    let mut budget = GraphBudget::default();
    budget.reserve(&rows)?;
    let mut seen = HashSet::new();
    for row in &rows {
        if row.part != 0
            || row.attributes != 0
            || row.data_size != row.byte_len
            || !seen.insert(row.key())
            || row.sha256.len() != 64
            || !row.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            bail!("unmeasured source dependency header");
        }
    }
    Ok(rows)
}

/// The reasons listed in a refusal before "и ещё N".
const REASONS_SHOWN: usize = 8;

/// The stage and the script of a dynamic apply, built from a read-only look at the database.
pub struct DynamicPlan {
    pub report: ConfigApplyReport,
    /// The transaction, when there is something to publish.
    pub script: Option<String>,
    staged: Vec<RowMeta>,
    replaced: Vec<RowMeta>,
    mobile_versions_before: Option<Vec<u8>>,
    registration: registrations::RegistrationPlan,
    reset_change_registrations: bool,
    params_rewrites: Vec<sqlgen::ParamsRewrite>,
    params_preimages: Vec<recovery::BoundParamsPreimage>,
    appended_existing_rows: Vec<recovery::AppendedExistingRow>,
}

/// What the size of `ConfigSave` alone says: rows, bytes, the largest row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StageSize {
    rows: i64,
    bytes: i64,
    largest: i64,
}

/// The reasons the size of a stage rules a dynamic update out (empty when it fits).
fn size_reasons(size: StageSize) -> Vec<String> {
    let mut reasons = Vec::new();
    if size.rows > MAX_ROWS as i64 {
        reasons.push(format!(
            "the stage holds {} rows and a dynamic update takes at most {MAX_ROWS} (the rows of a few objects and the service rows); a whole-tree stage of `config import` is not one (#395)",
            size.rows
        ));
    }
    if size.bytes > MAX_PLAN_BYTES as i64 {
        reasons.push(format!(
            "the stage holds {} bytes and a dynamic update takes at most {MAX_PLAN_BYTES}",
            size.bytes
        ));
    }
    if size.largest > MAX_ROW_BYTES as i64 {
        reasons.push(format!(
            "the largest row of the stage is {} bytes and a dynamic update takes rows of at most {MAX_ROW_BYTES}",
            size.largest
        ));
    }
    reasons
}

/// The words of a refusal: the first reasons, then how many more.
pub fn refusal_reason(reasons: &[String]) -> String {
    let mut parts: Vec<String> = reasons.iter().take(REASONS_SHOWN).cloned().collect();
    if reasons.len() > parts.len() {
        parts.push(format!("и ещё {}", reasons.len() - parts.len()));
    }
    parts.join("; ")
}

/// Judges the rows of a stage for a dynamic update, from what is known without their bytes: the
/// reasons it does not qualify (empty when it does). `same_text(name)` says whether the staged row
/// `name` has the text of the active one (asked for the descriptors and for `root` and `version`
/// when their bytes differ).
fn judge_rows(
    staged: &[RowMeta],
    active: &HashMap<(String, i32), RowMeta>,
    owners: &Owners,
    same_text: &mut dyn FnMut(&str) -> Result<bool>,
) -> Result<Vec<String>> {
    let mut reasons = Vec::new();
    let mut services: HashSet<&str> = HashSet::new();
    let mut refused_owners: HashSet<String> = HashSet::new();
    for row in staged {
        let name = row.name.as_str();
        // Native stages in the measured profile have no row flags. In particular, the
        // deleted alias must not inherit unmeasured flags from ConfigSave.
        if row.attributes != 0 {
            reasons.push(format!(
                "{name}: unmeasured staged Attributes {}; only Attributes=0 is supported",
                row.attributes
            ));
            continue;
        }
        if row.part != 0 {
            reasons.push(format!(
                "{name}: a row of several parts (part {})",
                row.part
            ));
            continue;
        }
        let known = active.get(&row.key());
        if row.data_size != row.byte_len || row.byte_len < 0 || row.byte_len > MAX_ROW_BYTES as i64
        {
            reasons.push(format!("{name}: unmeasured staged size metadata"));
            continue;
        }
        if known.is_some_and(|stored| {
            stored.attributes != 0
                || stored.data_size != stored.byte_len
                || stored.byte_len < 0
                || stored.byte_len > MAX_ROW_BYTES as i64
        }) {
            reasons.push(format!("{name}: unmeasured active row metadata"));
            continue;
        }
        match classify_name(name) {
            RowName::Service(service) => {
                services.insert(service);
                if service != "versions" {
                    let identical = known.is_some_and(|known| known.sha256 == row.sha256);
                    if known.is_none() {
                        reasons.push(format!("{name}: the active configuration has no such row"));
                    } else if !identical && !same_text(name)? {
                        reasons.push(format!(
                            "{name}: the service row differs from the active one"
                        ));
                    }
                } else if known.is_none() {
                    reasons.push(format!("{name}: the active configuration has no such row"));
                }
            }
            RowName::Descriptor(owner) | RowName::Body { owner, .. } => {
                let body_suffix = match classify_name(name) {
                    RowName::Body { suffix, .. } => Some(suffix),
                    _ => None,
                };
                if known.is_none() {
                    reasons.push(format!(
                        "{name}: no row of this name in Config; a new object, form or file is not published dynamically"
                    ));
                    continue;
                }
                // one reason for an object, however many of its rows are staged
                let owner = owner.to_ascii_lowercase();
                match owners.kinds.get(&owner) {
                    Some(_) if owners.admits(&owner) => {}
                    Some(kind) => {
                        if refused_owners.insert(owner.to_ascii_lowercase()) {
                            reasons.push(format!(
                            "{owner}: the object is a {kind} outside the measured dynamic owner/body cohort"
                            ));
                        }
                        continue;
                    }
                    None => {
                        if refused_owners.insert(owner.to_ascii_lowercase()) {
                            reasons.push(format!(
                                "{owner}: no proven existing owner in the active configuration"
                            ));
                        }
                        continue;
                    }
                }
                match body_suffix {
                    Some(suffix)
                        if matches!(
                            owners.role(&owner, suffix),
                            Some(
                                BodyRole::CommonModule
                                    | BodyRole::Module
                                    | BodyRole::Form
                                    | BodyRole::Template(_)
                            )
                        ) => {}
                    // Native repeated imports re-encode the existing common form help companion.
                    // Its inflated text must stay identical; changing help is not measured.
                    Some(suffix)
                        if owners.role(&owner, suffix) == Some(BodyRole::UnchangedHelp)
                            && same_text(name)? => {}
                    Some(suffix) => reasons.push(format!(
                        "{name}: unmeasured or structural body suffix .{suffix} for this owner"
                    )),
                    // a descriptor is published beside its body only when it is the same text
                    None => {
                        if !same_text(name)? {
                            reasons.push(format!(
                                "{name}: the descriptor's text differs from the active one; a change of an object's properties is not published dynamically"
                            ));
                        }
                    }
                }
            }
            // the list of removals is judged by [`judge_deleted_list`] and, when it passes, consumed
            RowName::Other if name.eq_ignore_ascii_case("deleted") => {}
            RowName::Other => reasons.push(format!(
                "{name}: not a service row, a descriptor or a body row of an object"
            )),
        }
    }
    for service in ["root", "version", "versions"] {
        if !services.contains(service) {
            reasons.push(format!(
                "{service}: the stage has no such row; a generation is made of root, version and versions"
            ));
        }
    }
    Ok(reasons)
}

/// Independently measured initial CommonModule/CommonForm five-row cohorts.
/// No previous generation or SI collection is admitted. CommonForm is limited
/// to its measured module-only container/layout, with opaque bytes bound below.
fn judge_initial_85_cohort(
    staged: &[RowMeta],
    active: &HashMap<(String, i32), RowMeta>,
    kinds: &HashMap<String, &'static str>,
    history: &[String],
    overlay: &[RowMeta],
    has_marker: bool,
) -> Vec<String> {
    let refusal = || {
        vec!["platform-8.5.1.1150: only an initial five-row delta of one existing CommonModule or module-only CommonForm .0 with its unchanged descriptor and root/version/versions is measured; history, overlays, removals and other owners/bodies require native apply".to_owned()]
    };
    if has_marker || !history.is_empty() || !overlay.is_empty() || staged.len() != 5 {
        return refusal();
    }
    let bodies: Vec<_> = staged
        .iter()
        .filter_map(|row| match classify_name(&row.name) {
            RowName::Body { owner, suffix: "0" } => Some((owner, row)),
            _ => None,
        })
        .collect();
    let [(owner, body)] = bodies.as_slice() else {
        return refusal();
    };
    if !matches!(
        kinds.get(&owner.to_ascii_lowercase()).copied(),
        Some("CommonModule" | "CommonForm")
    ) {
        return refusal();
    }
    let names: HashSet<_> = staged
        .iter()
        .map(|row| row.name.to_ascii_lowercase())
        .collect();
    let expected = HashSet::from([
        owner.to_ascii_lowercase(),
        format!("{}.0", owner.to_ascii_lowercase()),
        "root".to_owned(),
        "version".to_owned(),
        "versions".to_owned(),
    ]);
    if names != expected
        || staged.iter().any(|row| {
            row.part != 0
                || row.attributes != 0
                || row.byte_len < 0
                || row.data_size != row.byte_len
                || !active.contains_key(&row.key())
        })
        || active
            .get(&body.key())
            .is_none_or(|old| old.sha256 == body.sha256)
    {
        return refusal();
    }
    Vec::new()
}

/// The 8.5 native module oracle has two container elements and an unchanged,
/// measured info record. Bound inflation before the container parser allocates.
fn initial_85_module_text(blob: &[u8]) -> Result<Vec<u8>> {
    const MAX_PLAIN: usize = 8 * 1024 * 1024;
    let mut plain = Vec::new();
    let mut decoder = flate2::Decompress::new(false);
    let mut chunk = [0u8; 8192];
    loop {
        let before_in = decoder.total_in();
        let before_out = decoder.total_out();
        let capacity = chunk.len().min(MAX_PLAIN + 1 - plain.len());
        let status = decoder.decompress(
            &blob[before_in as usize..],
            &mut chunk[..capacity],
            flate2::FlushDecompress::None,
        )?;
        let produced = (decoder.total_out() - before_out) as usize;
        plain.extend_from_slice(&chunk[..produced]);
        if plain.len() > MAX_PLAIN {
            bail!("8.5 module decoded size exceeds {MAX_PLAIN}");
        }
        if status == flate2::Status::StreamEnd {
            if decoder.total_in() != blob.len() as u64 {
                bail!("8.5 module has trailing compressed data");
            }
            break;
        }
        if decoder.total_in() == before_in && produced == 0 {
            bail!("8.5 module has an incomplete DEFLATE stream");
        }
    }
    let elements = crate::v8_container::parse_v8_container(&plain)?;
    if elements.len() != 2
        || elements.iter().filter(|e| e.name == "text").count() != 1
        || elements.iter().filter(|e| e.name == "info").count() != 1
    {
        bail!("unmeasured 8.5 module container");
    }
    let info = &elements.iter().find(|e| e.name == "info").unwrap().data;
    if info != b"\xef\xbb\xbf{3,1,0,\"\",0}" {
        bail!("unmeasured 8.5 module info record");
    }
    let text = &elements.iter().find(|e| e.name == "text").unwrap().data;
    let body = text
        .strip_prefix(b"\xef\xbb\xbf")
        .ok_or_else(|| anyhow!("8.5 module text has no UTF-8 BOM"))?;
    std::str::from_utf8(body)?;
    Ok(text.clone())
}

/// Judges a stage's `deleted` row, the list of removals: `Ok(what it says)` when a dynamic apply may
/// consume it, `Err(reason)` when it may not.
///
/// It may when the list is empty (the platform's own import writes one to every stage; what the
/// platform's `force` writes for an empty list is NOT measured, this consumes it and writes nothing).
///
/// This legacy empty-list route does not admit nonempty lists. The measured pending-generation
/// route is separately judged by `dynamic_overlay::judge_deleted` and reproduces the extra writes.
/// Nonempty shapes reaching this fallback remain refused. `overlay_rows` is the current inventory.
///
/// Historically it refused lists naming rows of the online update the database carries (`overlay_rows`,
/// lower-cased), which is what the import stage of a target with a pending update lists, because the
/// exclusive apply that stage is made for promotes the update. Measured (evidence/dropin-dynamic/
/// acceptance.md, section 7): the platform's `force` on such a stage also writes a `deleted_dynupdate_<g>`
/// row, appends the alias bodies to the file lists of the change register and collects the service
/// information into `Params` (16 alias rows, 4 MB). The fallback does none of it, so it refuses the stage:
/// apply it exclusively. Any other name is a removal of an object, a form, an attribute: not published
/// dynamically either.
fn judge_deleted_list(
    plain: &[u8],
    overlay_rows: &HashSet<String>,
) -> std::result::Result<String, String> {
    let Some(entries) = super::parse_removals(plain) else {
        return Err("deleted: a list of removals this apply cannot read".to_owned());
    };
    if entries.is_empty() {
        return Ok("it is empty".to_owned());
    }
    let mut of_the_update = 0usize;
    for (name, flag) in &entries {
        let lower = name.to_ascii_lowercase();
        if flag == "0"
            && super::is_dynamic_update_row(name)
            && !lower.starts_with("deleted_dynupdate_")
            && overlay_rows.contains(&lower)
        {
            of_the_update += 1;
        } else {
            return Err(format!(
                "deleted: the stage lists the removal of {name}, and removals are not published dynamically"
            ));
        }
    }
    Err(format!(
        "deleted: the stage lists {of_the_update} row(s) of the online update the database carries for removal (the import of a target with a pending update does): the platform's dynamic update of it also writes a removal list, register file lists and service information that this apply does not reproduce; apply it exclusively (--dynamic=disable, nobody connected)"
    ))
}

/// The kind of every top-level object of the active configuration, by lower-cased uuid: the `root`
/// row names the configuration's row, which lists them.
fn object_kinds(
    client: &dyn SqlClient,
    database: &str,
    requested: &HashSet<String>,
    overlay: &[RowMeta],
    history: &[String],
) -> Result<(Owners, Vec<RowMeta>)> {
    let db = quote_ident(database)?;
    let mut budget = GraphBudget::default();
    let mut metas = read_row_metas(
        client,
        &format!("SELECT TOP (2) {ROW_COLUMNS} FROM {db}.dbo.Config WHERE FileName = N'root'"),
    )?;
    let root_meta = metas
        .first()
        .filter(|row| {
            metas.len() == 1
                && row.part == 0
                && row.attributes == 0
                && row.data_size == row.byte_len
                && row.byte_len >= 0
                && row.byte_len <= MAX_ROW_BYTES as i64
        })
        .ok_or_else(|| anyhow!("Config.root has unmeasured metadata"))?;
    budget.reserve(&metas)?;
    let root = dynamic_overlay::read_bound_blob(client, &db, "Config", root_meta)?;
    let plain = budget
        .inflate(&root)
        .context("the root row does not inflate")?;
    let text =
        std::str::from_utf8(versions::strip_bom(&plain)).context("Config.root is not UTF-8")?;
    let configuration = text
        .trim()
        .strip_prefix("{2,")
        .and_then(|rest| rest.split(',').next())
        .map(|value| value.trim().to_ascii_lowercase())
        .ok_or_else(|| anyhow!("the root row does not name the configuration"))?;
    uuid::Uuid::parse_str(&configuration)?;
    let descriptor = read_row_metas(
        client,
        &format!(
            "SELECT TOP (2) {ROW_COLUMNS} FROM {db}.dbo.Config WHERE FileName = N'{configuration}'"
        ),
    )?;
    let descriptor_meta = descriptor
        .first()
        .filter(|row| {
            descriptor.len() == 1
                && row.part == 0
                && row.attributes == 0
                && row.data_size == row.byte_len
                && row.byte_len >= 0
                && row.byte_len <= MAX_ROW_BYTES as i64
        })
        .ok_or_else(|| anyhow!("Config configuration descriptor has unmeasured metadata"))?;
    budget.reserve(&descriptor)?;
    let row = dynamic_overlay::read_bound_blob(client, &db, "Config", descriptor_meta)?;
    let plain = budget.inflate(&row)?;
    let tree = dynamic_metadata::descriptor_plain(&plain)
        .with_context(|| format!("the configuration row {configuration} does not parse"))?;
    metas.extend(descriptor);
    let mut owners = dynamic_metadata::root_owners(&tree)?;
    for id in owners.kinds.keys() {
        if uuid::Uuid::parse_str(id)?.hyphenated().to_string() != *id {
            bail!("noncanonical root owner UUID");
        }
    }
    let nested: HashSet<_> = requested
        .iter()
        .filter(|id| !owners.kinds.contains_key(*id))
        .cloned()
        .collect();
    if nested.is_empty() {
        return Ok((owners, metas));
    }
    let mut parents: Vec<_> = owners
        .kinds
        .iter()
        .filter(|(_, kind)| Owners::may_own_bodies(kind))
        .map(|(id, kind)| (id.clone(), *kind))
        .collect();
    parents.sort();
    ensure_graph_size(metas.len() + parents.len(), budget.compressed)?;
    let filter = parents
        .iter()
        .map(|(id, _)| format!("N'{id}'"))
        .collect::<Vec<_>>()
        .join(", ");
    if filter.is_empty() {
        return Ok((owners, metas));
    }
    let parent_metas = read_row_metas(
        client,
        &format!(
            "SELECT TOP ({}) {ROW_COLUMNS} FROM {db}.dbo.Config WHERE FileName IN ({filter}) ORDER BY FileName, PartNo",
            dynamic_metadata::MAX_GRAPH_ROWS + 1
        ),
    )?;
    let parent_kinds: HashMap<_, _> = parents.into_iter().collect();
    if parent_metas.len() != parent_kinds.len() {
        bail!("the bounded owner graph has missing or multipart descriptors");
    }
    let mut seen = HashSet::new();
    for meta in &parent_metas {
        let id = meta.name.to_ascii_lowercase();
        if meta.part != 0
            || meta.attributes != 0
            || meta.data_size != meta.byte_len
            || meta.byte_len > MAX_ROW_BYTES as i64
            || !seen.insert(id.clone())
        {
            bail!("unmeasured owner descriptor metadata");
        }
    }
    // Reserve the complete header batch before requesting even its first blob.
    budget.reserve(&parent_metas)?;
    for meta in &parent_metas {
        let id = meta.name.to_ascii_lowercase();
        let bytes = dynamic_overlay::read_bound_blob(client, &db, "Config", meta)?;
        let plain = budget.inflate(&bytes)?;
        let tree = dynamic_metadata::descriptor_plain(&plain)?;
        // This route changes bodies only. A prior native generation with changed ownership
        // must not make the ordinary descriptor graph classify an orphaned/new child.
        for alias in overlay.iter().filter(|row| {
            row.name
                .to_ascii_lowercase()
                .starts_with(&format!("{id}_dynupdate_"))
        }) {
            if dynamic_overlay::ordinary_alias_name(&alias.name, history)? != id {
                continue;
            }
            budget.reserve(std::slice::from_ref(alias))?;
            let alias_bytes = dynamic_overlay::read_bound_blob(client, &db, "Config", alias)?;
            if budget.inflate(&alias_bytes)? != plain {
                bail!("pending ownership/property change is outside the body-only cohort");
            }
        }
        owners.bind_children(&id, parent_kinds[&id], &tree)?;
    }
    metas.extend(parent_metas);
    let mut children = Vec::new();
    for id in nested {
        if !owners.kinds.contains_key(&id) {
            continue;
        }
        let child = read_row_metas(
            client,
            &format!("SELECT TOP (2) {ROW_COLUMNS} FROM {db}.dbo.Config WHERE FileName = N'{id}'"),
        )?;
        child
            .first()
            .filter(|row| {
                child.len() == 1
                    && row.part == 0
                    && row.attributes == 0
                    && row.data_size == row.byte_len
                    && row.byte_len <= MAX_ROW_BYTES as i64
            })
            .ok_or_else(|| anyhow!("unmeasured owned descriptor metadata"))?;
        children.extend(child);
    }
    budget.reserve(&children)?;
    for meta in &children {
        let id = meta.name.to_ascii_lowercase();
        let bytes = dynamic_overlay::read_bound_blob(client, &db, "Config", meta)?;
        let plain = budget.inflate(&bytes)?;
        let tree = dynamic_metadata::descriptor_plain(&plain)?;
        for alias in overlay.iter().filter(|row| {
            row.name
                .to_ascii_lowercase()
                .starts_with(&format!("{id}_dynupdate_"))
        }) {
            if dynamic_overlay::ordinary_alias_name(&alias.name, history)? != id {
                continue;
            }
            budget.reserve(std::slice::from_ref(alias))?;
            let alias_bytes = dynamic_overlay::read_bound_blob(client, &db, "Config", alias)?;
            if budget.inflate(&alias_bytes)? != plain {
                bail!("pending owned descriptor/property change is outside the body-only cohort");
            }
        }
        owners.bind_descriptor(&id, &tree)?;
    }
    metas.extend(children);
    Ok((owners, metas))
}

#[derive(Default)]
struct GraphBudget {
    rows: usize,
    compressed: usize,
    decoded: usize,
}

impl GraphBudget {
    fn reserve(&mut self, metas: &[RowMeta]) -> Result<()> {
        let rows = self
            .rows
            .checked_add(metas.len())
            .ok_or_else(|| anyhow!("graph row count overflow"))?;
        let mut compressed = self.compressed;
        for meta in metas {
            let size = usize::try_from(meta.byte_len).context("negative graph row length")?;
            compressed = compressed
                .checked_add(size)
                .ok_or_else(|| anyhow!("graph size overflow"))?;
        }
        ensure_graph_size(rows, compressed)?;
        self.rows = rows;
        self.compressed = compressed;
        Ok(())
    }

    fn inflate(&mut self, blob: &[u8]) -> Result<Vec<u8>> {
        let remaining = dynamic_metadata::MAX_GRAPH_BYTES
            .checked_sub(self.decoded)
            .ok_or_else(|| anyhow!("decoded graph size overflow"))?;
        let plain = dynamic_metadata::inflate_limit(blob, remaining)?;
        self.decoded = self
            .decoded
            .checked_add(plain.len())
            .ok_or_else(|| anyhow!("decoded graph size overflow"))?;
        ensure_graph_size(self.rows, self.decoded)?;
        Ok(plain)
    }
}

fn ensure_graph_size(rows: usize, bytes: usize) -> Result<()> {
    if rows > dynamic_metadata::MAX_GRAPH_ROWS || bytes > dynamic_metadata::MAX_GRAPH_BYTES {
        bail!("dynamic owner graph exceeds its independent row/byte budget");
    }
    Ok(())
}

fn same_semantic_text(
    name: &str,
    active_bytes: &[u8],
    staged_bytes: &[u8],
    profile: MssqlNativePlatformProfile,
) -> Result<bool> {
    let staged_plain = dynamic_metadata::inflate(staged_bytes)?;
    let active_plain = dynamic_metadata::inflate(active_bytes)?;
    Ok(staged_plain == active_plain
        || (name == "root" && profile.accepts_dynamic_root_restamp(&active_plain, &staged_plain)))
}

fn validate_pending_metadata(
    client: &dyn SqlClient,
    db: &str,
    overlay: &[RowMeta],
    history: &[String],
    owners: &Owners,
    metas: &mut Vec<RowMeta>,
) -> Result<()> {
    let mut ordinary_rows: HashMap<String, (RowMeta, Vec<u8>)> = HashMap::new();
    let mut compressed = 0usize;
    let mut decoded = 0usize;
    for alias in overlay {
        if alias.name.eq_ignore_ascii_case("DynamicallyUpdated") {
            continue;
        }
        let ordinary = dynamic_overlay::ordinary_alias_name(&alias.name, history)?;
        if ordinary == "versions" || ordinary == "deleted" {
            continue;
        }
        if !ordinary_rows.contains_key(&ordinary) {
            let rows = read_row_metas(
                client,
                &format!(
                    "SELECT TOP (2) {ROW_COLUMNS} FROM {db}.dbo.Config WHERE FileName = N'{}'",
                    super::model::quote_string(&ordinary)
                ),
            )?;
            let row = rows
                .first()
                .filter(|row| {
                    rows.len() == 1
                        && row.part == 0
                        && row.attributes == 0
                        && row.data_size == row.byte_len
                        && row.byte_len >= 0
                        && row.byte_len <= MAX_ROW_BYTES as i64
                })
                .ok_or_else(|| anyhow!("unmeasured retained ordinary row {ordinary}"))?;
            compressed = compressed
                .checked_add(usize::try_from(row.byte_len)?)
                .ok_or_else(|| anyhow!("retained metadata size overflow"))?;
            ensure_graph_size(ordinary_rows.len() + 1, compressed)?;
            let blob = dynamic_overlay::read_bound_blob(client, db, "Config", row)?;
            decoded = decoded
                .checked_add(dynamic_metadata::inflate(&blob)?.len())
                .ok_or_else(|| anyhow!("retained decoded size overflow"))?;
            ensure_graph_size(ordinary_rows.len() + 1, decoded)?;
            if let Some(previous) = metas.iter().find(|meta| meta.key() == row.key()) {
                if previous != row {
                    bail!("retained ordinary metadata drifted during graph judgment");
                }
            } else {
                metas.push(row.clone());
            }
            ordinary_rows.insert(ordinary.clone(), (row.clone(), blob));
        }
        compressed = compressed
            .checked_add(usize::try_from(alias.byte_len)?)
            .ok_or_else(|| anyhow!("retained alias size overflow"))?;
        ensure_graph_size(overlay.len(), compressed)?;
        let bytes = dynamic_overlay::read_bound_blob(client, db, "Config", alias)?;
        decoded = decoded
            .checked_add(dynamic_metadata::inflate(&bytes)?.len())
            .ok_or_else(|| anyhow!("retained alias decoded size overflow"))?;
        ensure_graph_size(overlay.len(), decoded)?;
        dynamic_metadata::validate_retained_alias(
            &ordinary,
            owners,
            &ordinary_rows[&ordinary].1,
            &bytes,
        )?;
    }
    Ok(())
}

/// The bytes of the row `name` the configuration is read from: its newest alias of a generation the
/// history lists, or the plain row.
fn effective_blob(
    client: &dyn SqlClient,
    database: &str,
    name: &str,
    history: &[String],
    judged: &[RowMeta],
) -> Result<Option<Vec<u8>>> {
    let stored = crate::mssql_dump::stored_row_name(
        history,
        name,
        judged.iter().map(|row| row.name.as_str()),
    );
    let meta = judged
        .iter()
        .find(|row| row.name.eq_ignore_ascii_case(&stored) && row.part == 0)
        .ok_or_else(|| anyhow!("Config.{stored} was not in the judged inventory"))?;
    let bytes = dynamic_overlay::read_bound_blob(client, &quote_ident(database)?, "Config", meta)?;
    Ok(Some(bytes))
}

fn read_stage_semantic(
    client: &dyn SqlClient,
    db: &str,
    staged: &[RowMeta],
    name: &str,
) -> Result<Vec<u8>> {
    let meta = staged
        .iter()
        .find(|row| row.name.eq_ignore_ascii_case(name) && row.part == 0)
        .ok_or_else(|| anyhow!("ConfigSave.{name} has no judged row"))?;
    dynamic_overlay::read_bound_blob(client, db, "ConfigSave", meta)
}

/// A no-op is safe only against the effective image, even when a manually staged request omits
/// `deleted`. The online engine's ordinary-row equality must not discard a pending-body revert.
fn judge_effective_noop(
    staged: &[RowMeta],
    history: &[String],
    overlay: &[RowMeta],
    read_effective: &mut dyn FnMut(&str) -> Result<Vec<u8>>,
) -> Result<()> {
    if history.is_empty() && overlay.is_empty() {
        return Ok(());
    }
    for row in staged {
        if matches!(
            classify_name(&row.name),
            RowName::Descriptor(_) | RowName::Body { .. }
        ) && !judged_stage_digest(staged, &row.name, &read_effective(&row.name)?)
        {
            return Err(NeedsNativeApply::apply("an ordinary-byte no-op would discard a pending-generation revert; this shape is not measured".to_owned()).into());
        }
    }
    Ok(())
}

fn unfinished_guard() -> String {
    let names = sqlgen::UNFINISHED_NAMES
        .iter()
        .map(|name| format!("N'{name}'"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "IF EXISTS (SELECT 1 FROM dbo.Config WITH (UPDLOCK, HOLDLOCK) WHERE FileName IN ({names}) OR FileName LIKE N'%.new') THROW 57307, 'Unfinished operation appeared', 1;\n"
    )
}

fn stage_size(client: &dyn SqlClient, db: &str) -> Result<StageSize> {
    let rows = client.query_rows(
        &format!(
            "SELECT COUNT_BIG(*), ISNULL(SUM(CONVERT(bigint, DATALENGTH(BinaryData))), 0), ISNULL(MAX(CONVERT(bigint, DATALENGTH(BinaryData))), 0) FROM {db}.dbo.ConfigSave"
        ),
        &[],
    )?;
    let row = rows
        .first()
        .ok_or_else(|| anyhow!("the size query of ConfigSave returned no row"))?;
    Ok(StageSize {
        rows: row.i64(0)?,
        bytes: row.i64(1)?,
        largest: row.i64(2)?,
    })
}

/// The semantic reader must use the same bytes the bounded stage inventory names.
fn judged_stage_digest(staged: &[RowMeta], name: &str, bytes: &[u8]) -> bool {
    staged
        .iter()
        .find(|row| row.name.eq_ignore_ascii_case(name) && row.part == 0)
        .is_some_and(|row| {
            usize::try_from(row.byte_len).ok() == Some(bytes.len())
                && row
                    .sha256
                    .eq_ignore_ascii_case(&hex_lower(&Sha256::digest(bytes)))
        })
}

/// Bind the rows judged above to the exact image the transaction will assert. A concurrent import
/// must not replace an empty `deleted` list with a nonempty one between judgment and publication.
fn require_judged_image(table: &str, judged: &[RowMeta], image: &[MainStorageRow]) -> Result<()> {
    let expected: HashMap<_, _> = judged.iter().map(|row| (row.key(), row)).collect();
    let actual: HashSet<_> = image
        .iter()
        .map(|row| (row.file_name.to_lowercase(), row.part_no))
        .collect();
    if expected.len() != judged.len() || actual.len() != image.len() || image.len() != judged.len()
    {
        bail!("{table} changed after the dynamic gates judged it; no publication was attempted");
    }
    for row in image {
        let key = (row.file_name.to_lowercase(), row.part_no);
        let same = expected.get(&key).is_some_and(|meta| {
            u64::try_from(meta.data_size).ok() == Some(row.data_size)
                && i32::from(meta.attributes) == row.attributes
                && meta.creation == row.creation
                && meta.modified == row.modified
                && usize::try_from(meta.byte_len).ok() == Some(row.binary_data.len())
                && meta.sha256.eq_ignore_ascii_case(&hex_lower(&row.sha256()))
        });
        if !same {
            bail!(
                "{table}.{} changed after the dynamic gates judged it; no publication was attempted",
                row.file_name
            );
        }
    }
    Ok(())
}

/// The transaction may have committed before a connection error reached the caller. Keep the
/// recovery location visible and never promise a rollback for an ambiguous transport failure.
fn uncertain_commit_context(database: &str, recovery_dir: &std::path::Path, token: &str) -> String {
    format!(
        "the dynamic apply transaction outcome for {database} is uncertain; it may have committed. Inspect ConfigSave and the dynamic markers before any retry; recovery artifact: {} (token {token})",
        recovery_dir.display()
    )
}

/// Maps what the engine refuses to what the user is told: a stage that is too big or names a
/// target the engine does not take is not a dynamic update ([`NeedsNativeApply`]); the rest is a
/// failure.
fn engine_error(error: MainActivationError) -> anyhow::Error {
    match error {
        MainActivationError::Limit(_) | MainActivationError::StructuralTarget(_) => {
            NeedsNativeApply::apply(format!("the online engine refuses the stage: {error}")).into()
        }
        other => anyhow::Error::new(other),
    }
}

fn platform_unsupported(options: &ConfigApplyOptions, error: &anyhow::Error) -> anyhow::Error {
    DynamicUnsupported {
        platform: options.platform_profile.id().to_owned(),
        reason: format!("{error:#}"),
    }
    .into()
}

/// The warning for an overlay of more than [`WARN_GENERATIONS`] generations: each one keeps its rows beside
/// the active ones until an exclusive apply folds them.
pub fn growth_warning(generations: usize) -> Option<String> {
    (generations > WARN_GENERATIONS).then(|| {
        format!(
            "the infobase now holds {generations} dynamic generations; each one keeps its rows beside the active ones until an exclusive apply (`config apply --dynamic=disable` with nobody connected) folds them"
        )
    })
}

/// Plans a dynamic apply of the staged configuration: reads the database and judges the stage.
///
/// `Err` of [`NeedsNativeApply`] means the stage does not qualify (the reasons are its words), `Err` of
/// [`DynamicUnsupported`] that the platform has no dynamic apply. An empty `ConfigSave` is a plan with
/// `nothing_to_apply`.
pub fn plan_dynamic(sql: &SqlExec, options: &ConfigApplyOptions) -> Result<DynamicPlan> {
    let total = Instant::now();
    let client = require_client(sql)?;
    let database = options.database.as_str();
    let db = quote_ident(database)?;
    let mut timings = ApplyTimings::default();
    if options.rehearse {
        bail!("a rehearsal is not built for the dynamic apply; use --dry-run");
    }
    options
        .platform_profile
        .require_config_apply_dynamic_supported()
        .map_err(|error| platform_unsupported(options, &error))?;

    let started = Instant::now();
    let storage = crate::mssql_platform_profile::verify_mssql_storage_profile(
        options.platform_profile,
        client,
        database,
    )?;
    timings.storage_check_ms = ms(started);
    let mut report = blank_report(options, &storage, ApplyMode::Dynamic);

    let started = Instant::now();
    let size = stage_size(client, &db)?;
    if size.rows == 0 {
        report.nothing_to_apply = true;
        timings.inventory_ms = ms(started);
        timings.total_ms = ms(total);
        report.timings = timings;
        return Ok(DynamicPlan {
            report,
            script: None,
            staged: Vec::new(),
            replaced: Vec::new(),
            mobile_versions_before: None,
            registration: registrations::RegistrationPlan::default(),
            reset_change_registrations: false,
            params_rewrites: Vec::new(),
            params_preimages: Vec::new(),
            appended_existing_rows: Vec::new(),
        });
    }
    let too_big = size_reasons(size);
    if !too_big.is_empty() {
        return Err(NeedsNativeApply::apply(refusal_reason(&too_big)).into());
    }
    require_no_unfinished_operation(client, &db)?;
    require_schema_settled(client, &db)?;

    let staged = read_row_metas(
        client,
        &format!("SELECT {ROW_COLUMNS} FROM {db}.dbo.ConfigSave ORDER BY FileName, PartNo"),
    )?;
    let replaced = read_row_metas(
        client,
        &format!(
            "SELECT {ROW_COLUMNS} FROM {db}.dbo.Config s WHERE EXISTS (SELECT 1 FROM {db}.dbo.ConfigSave x WHERE x.FileName = s.FileName) ORDER BY FileName, PartNo"
        ),
    )?;
    if staged.len() as i64 != size.rows {
        bail!("ConfigSave changed while the plan was being made");
    }
    let active: HashMap<(String, i32), RowMeta> = replaced
        .iter()
        .cloned()
        .map(|row| (row.key(), row))
        .collect();
    let config_marker = read_blob(client, database, "Config", "DynamicallyUpdated")?;
    let params_marker = read_blob(client, database, "Params", "DynamicallyUpdated")?;
    let history =
        versions::parse_dynamic_history(config_marker.as_deref(), params_marker.as_deref())?;
    let history_names: Vec<String> = history
        .generations
        .iter()
        .map(|generation| generation.hyphenated().to_string())
        .collect();
    let overlay = dynamic_overlay::inventory(client, &db)
        .map_err(|error| NeedsNativeApply::apply(format!("pending Config inventory: {error:#}")))?;
    let judged_effective: Vec<_> = replaced.iter().chain(&overlay).cloned().collect();
    timings.inventory_ms = ms(started);

    // The rows: kinds, and the text of what must not change.
    let started = Instant::now();
    let mut requested: HashSet<String> = staged
        .iter()
        .filter_map(|row| match classify_name(&row.name) {
            RowName::Descriptor(owner) | RowName::Body { owner, .. } => {
                Some(owner.to_ascii_lowercase())
            }
            _ => None,
        })
        .collect();
    for row in &overlay {
        if row.name.eq_ignore_ascii_case("DynamicallyUpdated") {
            continue;
        }
        let ordinary = dynamic_overlay::ordinary_alias_name(&row.name, &history_names)?;
        if let RowName::Descriptor(owner) | RowName::Body { owner, .. } = classify_name(&ordinary) {
            requested.insert(owner.to_ascii_lowercase());
        }
    }
    let (kinds, mut kind_metas) = if !requested.is_empty() {
        object_kinds(client, database, &requested, &overlay, &history_names)
            .map_err(|error| NeedsNativeApply::apply(format!("dynamic owner graph: {error:#}")))?
    } else {
        (Owners::default(), Vec::new())
    };
    let initial_85 = options.platform_profile == MssqlNativePlatformProfile::Platform8_5_1_1150;
    if !initial_85 {
        validate_pending_metadata(
            client,
            &db,
            &overlay,
            &history_names,
            &kinds,
            &mut kind_metas,
        )
        .map_err(|error| NeedsNativeApply::apply(format!("retained metadata: {error:#}")))?;
    }
    let mut same_text = |name: &str| -> Result<bool> {
        let staged_bytes = read_stage_semantic(client, &db, &staged, name)?;
        if !judged_stage_digest(&staged, name, &staged_bytes) {
            bail!(
                "ConfigSave.{name} changed while its descriptor was judged; no publication was attempted"
            );
        }
        let active_bytes = if matches!(classify_name(name), RowName::Service(_)) {
            let meta = replaced
                .iter()
                .find(|row| row.name.eq_ignore_ascii_case(name) && row.part == 0)
                .ok_or_else(|| anyhow!("Config.{name} has no judged row"))?;
            Some(dynamic_overlay::read_bound_blob(
                client, &db, "Config", meta,
            )?)
        } else {
            effective_blob(client, database, name, &history_names, &judged_effective)?
        }
        .ok_or_else(|| anyhow!("Config.{name} vanished"))?;
        if matches!(classify_name(name), RowName::Service(_))
            && !judged_stage_digest(&replaced, name, &active_bytes)
        {
            bail!(
                "Config.{name} changed while its service row was judged; no publication was attempted"
            );
        }
        same_semantic_text(name, &active_bytes, &staged_bytes, options.platform_profile)
    };
    let mut reasons = judge_rows(&staged, &active, &kinds, &mut same_text)?;
    if initial_85 {
        reasons.extend(judge_initial_85_cohort(
            &staged,
            &active,
            &kinds.kinds,
            &history_names,
            &overlay,
            config_marker.is_some() || params_marker.is_some(),
        ));
        if reasons.is_empty() {
            let body = staged
                .iter()
                .find(|row| matches!(classify_name(&row.name), RowName::Body { .. }))
                .unwrap();
            let staged_bytes = read_stage_semantic(client, &db, &staged, &body.name)?;
            let active_bytes = effective_blob(
                client,
                database,
                &body.name,
                &history_names,
                &judged_effective,
            )?
            .ok_or_else(|| anyhow!("8.5 active body vanished"))?;
            let kind = match classify_name(&body.name) {
                RowName::Body { owner, .. } => {
                    kinds.kinds.get(&owner.to_ascii_lowercase()).copied()
                }
                _ => None,
            };
            match kind {
                Some("CommonModule") => match (
                    initial_85_module_text(&active_bytes),
                    initial_85_module_text(&staged_bytes),
                ) {
                    (Ok(old), Ok(new)) if old != new => {}
                    (Ok(_), Ok(_)) => reasons.push(
                        "8.5 module text is unchanged; only a real initial body change is measured"
                            .to_owned(),
                    ),
                    _ => reasons.push("8.5 module has an unmeasured body/info layout".to_owned()),
                },
                Some("CommonForm") => {
                    if let Err(error) = super::dynamic_platform85::require_form_module_only_change(
                        options.platform_profile,
                        &active_bytes,
                        &staged_bytes,
                    ) {
                        reasons.push(format!("8.5 form module-only guard: {error:#}"));
                    }
                }
                _ => reasons.push("8.5 initial body kind is unmeasured".to_owned()),
            }
        }
    }
    if !initial_85 && reasons.is_empty() {
        for row in &staged {
            let RowName::Body { owner, suffix } = classify_name(&row.name) else {
                continue;
            };
            let Some(role) = kinds.role(&owner.to_ascii_lowercase(), suffix) else {
                continue;
            };
            if role == BodyRole::UnchangedHelp {
                continue;
            }
            let stage_bytes = read_stage_semantic(client, &db, &staged, &row.name)?;
            let active_bytes = effective_blob(
                client,
                database,
                &row.name,
                &history_names,
                &judged_effective,
            )?
            .ok_or_else(|| anyhow!("effective body disappeared"))?;
            for bytes in [&stage_bytes, &active_bytes] {
                if let Err(error) = dynamic_metadata::validate_body(role, bytes) {
                    reasons.push(format!("{}: unsupported body layout: {error:#}", row.name));
                }
            }
        }
    }
    // Bind the semantic inventory to the exact bytes the transaction will assert. The structural
    // checker reads independently, so its safe verdict must not permit an ABA replacement of
    // `versions` with an inventory that removes or silently changes unstaged metadata.
    let staged_versions = read_stage_semantic(client, &db, &staged, "versions")?;
    if !judged_stage_digest(&staged, "versions", &staged_versions) {
        bail!("ConfigSave.versions changed while its inventory was judged");
    }
    let effective_versions = effective_blob(
        client,
        database,
        "versions",
        &history_names,
        &judged_effective,
    )?
    .ok_or_else(|| anyhow!("effective Config lacks versions"))?;
    let old_versions = versions::parse_versions(&effective_versions)?;
    let new_versions = versions::parse_versions(&staged_versions)?;
    let old: HashMap<_, _> = old_versions
        .entries
        .iter()
        .map(|(name, version)| (name.to_ascii_lowercase(), version))
        .collect();
    let new: HashMap<_, _> = new_versions
        .entries
        .iter()
        .map(|(name, version)| (name.to_ascii_lowercase(), version))
        .collect();
    if old.len() != old_versions.entries.len()
        || new.len() != new_versions.entries.len()
        || old.keys().collect::<HashSet<_>>() != new.keys().collect::<HashSet<_>>()
        || new.iter().any(|(name, version)| {
            old.get(name) != Some(version)
                && !staged.iter().any(|row| row.name.eq_ignore_ascii_case(name))
        })
    {
        reasons.push("versions: removal, duplicate, new name or changed unstaged metadata is not published dynamically".to_owned());
    }
    // The list of removals, when the stage has one: consumed if it says nothing a dynamic apply acts on.
    let mut deleted_note = None;
    let mut pending_deleted = Vec::new();
    if staged
        .iter()
        .any(|row| row.name.eq_ignore_ascii_case("deleted"))
    {
        let bytes = Some(read_stage_semantic(client, &db, &staged, "deleted")?);
        if bytes
            .as_deref()
            .is_some_and(|bytes| !judged_stage_digest(&staged, "deleted", bytes))
        {
            bail!(
                "ConfigSave.deleted changed while its removal list was judged; no publication was attempted"
            );
        }
        let plain = bytes.and_then(|bytes| dynamic_metadata::inflate(&bytes).ok());
        match plain {
            None => reasons.push("deleted: a list of removals this apply cannot read".to_owned()),
            Some(plain) => {
                if super::parse_removals(&plain).is_some_and(|entries| !entries.is_empty()) {
                    match dynamic_overlay::judge_deleted(&plain, &overlay, &history_names, &kinds) {
                        Ok(names) => {
                            deleted_note = Some("the exact pending overlay inventory is retained beside the new generation".to_owned());
                            pending_deleted = names;
                        }
                        Err(reason) => reasons.push(format!("deleted: {reason:#}")),
                    }
                } else {
                    match judge_deleted_list(
                        &plain,
                        &overlay
                            .iter()
                            .map(|row| row.name.to_ascii_lowercase())
                            .collect(),
                    ) {
                        Ok(note) => deleted_note = Some(note),
                        Err(reason) => reasons.push(reason),
                    }
                }
            }
        }
    }
    // A service overlay is accepted only together with the exact nonempty removal inventory.
    let collection = if initial_85 {
        // The measured initial 8.5 generation creates no SI aliases and does not
        // update siVersions. Refuse existing aliases; never run the 8.3 collector.
        require_settled_storage(client, &db)?;
        None
    } else if pending_deleted.is_empty() {
        require_settled_storage(client, &db)?;
        None
    } else {
        Some(
            dynamic_overlay::collect(client, &db, &history_names).map_err(|error| {
                NeedsNativeApply::apply(format!("pending service collection: {error:#}"))
            })?,
        )
    };

    // The restructure check, only when nothing else rules the stage out.
    if reasons.is_empty() {
        let gate = ApplyCheckGate::new(sql, xml_version_of(options.platform_profile));
        let none_names: HashSet<String> = HashSet::new();
        let consumed_names: HashSet<String> =
            deleted_note.iter().map(|_| "deleted".to_owned()).collect();
        let none_kinds: HashMap<String, &'static str> = HashMap::new();
        let verdict: GateVerdict = gate.check(&GateInput {
            client,
            database,
            staged: &staged,
            active: &active,
            accepted_new_rows: &none_names,
            accepted_owner_descriptors: &none_names,
            new_object_kinds: &none_kinds,
            // a list of removals that passed is no row the check judges
            consumed_rows: &consumed_names,
            removed_rows: &none_names,
        })?;
        timings.gate_ms = ms(started);
        if verdict.restructuring_required {
            for blocker in &verdict.blockers {
                reasons.push(if blocker.row.is_empty() {
                    format!("structure: {}", blocker.reason)
                } else {
                    format!("{}: {}", blocker.row, blocker.reason)
                });
            }
            if reasons.is_empty() {
                reasons.push("structure: the restructure check refuses the stage".to_owned());
            }
        }
        report.gate = Some(verdict);
    }
    if !reasons.is_empty() {
        return Err(NeedsNativeApply::apply(refusal_reason(&reasons)).into());
    }

    // The online engine's plan: the exact stage, the rows it replaces, the markers.
    let inputs = crate::mssql::read_activation_inputs(sql, database)?;
    require_judged_image("ConfigSave", &staged, &inputs.staged)?;
    require_judged_image("Config", &replaced, &inputs.active)?;
    for (table, judged_marker, image_marker) in [
        ("Config", &config_marker, &inputs.config_marker),
        ("Params", &params_marker, &inputs.params_marker),
    ] {
        if judged_marker.as_deref() != image_marker.as_ref().map(|row| row.binary_data.as_slice()) {
            bail!(
                "{table}.DynamicallyUpdated changed after the dynamic gates judged it; no publication was attempted"
            );
        }
    }

    // the consumed list is asserted with the stage and emptied with it, and published nowhere
    let (consumed, published): (Vec<_>, Vec<_>) = inputs
        .staged
        .into_iter()
        .partition(|row| row.file_name.eq_ignore_ascii_case("deleted"));
    let allowed = crate::mssql::allowed_activation_targets(&published);
    let activation = prepare_main_activation(
        MainActivationMode::Online,
        published,
        MainActivationSnapshot {
            config_rows: inputs.active,
            config_dynamically_updated: inputs.config_marker,
            params_dynamically_updated: inputs.params_marker,
        },
        &allowed,
        true,
    )
    .map_err(engine_error)?
    .with_consumed_stage_rows(consumed);
    // The engine judges byte equality against ordinary rows. A request to revert a pending body
    // to those bytes must be a new generation, not silent no-op consumption; this unmeasured
    // rollback shape remains refused until the engine has a bound effective no-op image.
    if activation.is_no_op() {
        judge_effective_noop(&staged, &history_names, &overlay, &mut |name| {
            effective_blob(client, database, name, &history_names, &judged_effective)?
                .ok_or_else(|| anyhow!("effective Config.{name} vanished"))
        })?;
    }

    // The writes the platform's `force` makes besides the rows, planned by the exclusive apply's own
    // code: the mobile versions ring and the change registrations.
    let (files_rewrites, mobile_before) = plan_mobile_versions(client, database, &mut report)?;
    let has_change_registrations = scalar_i64(
        client,
        &format!(
            "SELECT CASE WHEN OBJECT_ID(N'{db}.dbo._ConfigChngR', N'U') IS NULL OR OBJECT_ID(N'{db}.dbo._ConfigChngR_ExtProps', N'U') IS NULL THEN 0 ELSE 1 END"
        ),
    )? == 1;
    let registration = if has_change_registrations {
        registrations::plan(client, database, &staged, &pending_deleted)?
    } else {
        registrations::RegistrationPlan::default()
    };
    let mut initial_85_registration_guard = String::new();
    let guarded_files = if initial_85 {
        if !has_change_registrations
            || !registration.is_empty()
            || !registration.missing.is_empty()
            || registration.added_rows != 0
            || registration.added_file_rows != 0
            || registration.changed_objects != 1
            || registration.nodes.len() != 3
        {
            return Err(NeedsNativeApply::apply(
                "8.5 registration state is outside the measured unchanged three-node cohort"
                    .to_owned(),
            )
            .into());
        }
        let body = staged
            .iter()
            .find(|row| matches!(classify_name(&row.name), RowName::Body { .. }))
            .unwrap();
        let object_hex = registrations::object_of(&body.name).unwrap();
        let touched = sqlgen::staged_objects_predicate(&format!("{db}.dbo."));
        let count = scalar_i64(
            client,
            &format!("SELECT COUNT_BIG(*) FROM {db}.dbo._ConfigChngR r WHERE {touched}"),
        )?;
        let non_null = scalar_i64(
            client,
            &format!(
                "SELECT COUNT_BIG(*) FROM {db}.dbo._ConfigChngR r WHERE r._MessageNo IS NOT NULL AND {touched}"
            ),
        )?;
        let unmeasured_lists = scalar_i64(
            client,
            &format!(
                "SELECT COUNT_BIG(*) FROM {db}.dbo._ConfigChngR_ExtProps e JOIN {db}.dbo._ConfigChngR r ON r._IDRRef=e._ConfigChngR_IDRRef WHERE r._MDObjID=0x{object_hex} AND (e._KeyField<>0x00000000 OR e._FileName<>N'{}')",
                super::model::quote_string(&body.name)
            ),
        )?;
        if count != 3 || non_null != 0 || unmeasured_lists != 0 {
            return Err(NeedsNativeApply::apply("8.5 requires exactly three unchanged NULL-message registrations with existing .0 file lists".to_owned()).into());
        }
        let locked_touched = sqlgen::staged_objects_predicate("dbo.");
        initial_85_registration_guard = format!(
            "IF (SELECT COUNT_BIG(*) FROM dbo._ConfigChngR r WITH (UPDLOCK,HOLDLOCK) WHERE {locked_touched}) <> 3 OR EXISTS (SELECT 1 FROM dbo._ConfigChngR r WITH (UPDLOCK,HOLDLOCK) WHERE r._MessageNo IS NOT NULL AND {locked_touched}) THROW 57318, '8.5 unchanged registration cohort drifted', 1;\n"
        );
        // Reuse the exact full-row/list preimage guard. These existing files must
        // produce no additions; the synthetic input is never sent to parity SQL.
        vec![sqlgen::AppendedFile {
            object_hex,
            file_name: body.name.clone(),
        }]
    } else {
        registration.dropped_files.clone()
    };
    let unchanged = activation.is_no_op();
    let appends = dynamic_overlay::plan_appends(client, &db, &guarded_files, &registration.nodes)
        .map_err(|error| {
        NeedsNativeApply::apply(format!("pending registration file lists: {error:#}"))
    })?;
    if initial_85 && (!appends.rows.is_empty() || appends.registration_ids.len() != 3) {
        return Err(NeedsNativeApply::apply(
            "8.5 existing registration file lists are outside the unchanged native cohort"
                .to_owned(),
        )
        .into());
    }
    // The reused parity helper also asserts all distinct registered node pairs before appending.
    // This count includes stale/own-node registrations; it is not the eligible-node count.
    let nodes_seen = if registration.dropped_files.is_empty() {
        0
    } else {
        usize::try_from(scalar_i64(client, &format!(
            "SELECT COUNT_BIG(*) FROM (SELECT DISTINCT _NodeTRef, _NodeRRef FROM {db}.dbo._ConfigChngR) d"
        ))?).map_err(|_| anyhow!("invalid registered-node count"))?
    };
    let parity = ScriptInputs {
        database: database.to_owned(),
        reset_change_registrations: has_change_registrations,
        files_rewrites,
        nodes: node_literals(&registration.nodes),
        nodes_seen,
        registration_additions: registration.additions.clone(),
        registration_rows_expected: registration.added_rows,
        registration_file_rows_expected: registration.added_file_rows,
        plan_node_counts: registration.node_counts.clone(),
        extra_changed_objects: registration.extra_objects.clone(),
        appended_files: registration.dropped_files.clone(),
        appended_registration_ids: Some(appends.registration_ids.clone()),
        ..ScriptInputs::default()
    };
    // The markers are stamped as the platform stamps them (local time, the year offset), like the rows
    // the parity writes touch.
    // Core activation binds stage names and bytes; dynamic publication also copies row
    // headers, so bind the entire judged stage before either publication or no-op cleanup.
    let mut precondition = dynamic_overlay::guard("ConfigSave", "1=1", &staged);
    precondition.push_str(&dynamic_overlay::guard(
        "Config",
        dynamic_overlay::CONFIG_FILTER,
        &overlay,
    ));
    if initial_85 {
        precondition.push_str("IF EXISTS (SELECT 1 FROM dbo.Params WITH (UPDLOCK, HOLDLOCK) WHERE FileName LIKE N'%[_]dynupdate[_]%') THROW 57316, 'Unmeasured 8.5 Params overlay appeared', 1;\n");
    }
    precondition.push_str(&appends.guard_sql);
    precondition.push_str(&initial_85_registration_guard);
    if initial_85 || !registration.dropped_files.is_empty() {
        precondition.push_str(
            &dynamic_overlay::node_guard(client, &db, &registration.nodes).map_err(|error| {
                NeedsNativeApply::apply(format!("pending registration node eligibility: {error:#}"))
            })?,
        );
    }
    if !kind_metas.is_empty() {
        let names = kind_metas
            .iter()
            .map(|row| format!("N'{}'", super::model::quote_string(&row.name)))
            .collect::<Vec<_>>()
            .join(", ");
        precondition.push_str(&dynamic_overlay::guard(
            "Config",
            &format!("FileName IN ({names})"),
            &kind_metas,
        ));
    }
    precondition.push_str("IF EXISTS (SELECT 1 FROM dbo.SchemaStorage WITH (UPDLOCK, HOLDLOCK) WHERE Status <> 100) THROW 57316, 'SchemaStorage no longer settled', 1;\n");
    precondition.push_str(&unfinished_guard());
    let mut parity_sql = sqlgen::render_parity_writes(&parity);
    let mut params_rewrites = Vec::new();
    let mut params_preimages = Vec::new();
    if let Some(collection) = &collection {
        precondition.push_str(&collection.guard_sql);
        parity_sql.push_str(&collection.render(activation.new_generation()));
        parity_sql.push_str(&dynamic_overlay::publish_deleted(
            activation.new_generation(),
        ));
        if !unchanged {
            params_rewrites = collection.rewrites.clone();
            params_preimages = collection.preimages.clone();
        }
    }
    let activation = activation
        .with_precondition_sql(precondition)
        .with_platform_timestamps(sqlgen::timestamp_declarations())
        .with_parity_sql(parity_sql);
    let rendered = render_main_activation_sql(database, &activation, None).map_err(engine_error)?;
    // Row locks may wait for a session that holds one; not for ever.
    let script = format!("SET LOCK_TIMEOUT 30000;\n{}", rendered.sql);
    let script_sha = hex_lower(&Sha256::digest(script.as_bytes()));

    // The report.
    let new_generation = activation.new_generation();
    let old_generation = activation.old_generation();
    let mut history_after: Vec<String> = activation
        .dynamic_history()
        .iter()
        .map(|generation| generation.hyphenated().to_string())
        .collect();
    let mut stage = StageSummary {
        rows: staged.len(),
        bytes: staged.iter().map(|row| row.byte_len).sum(),
        descriptors: 0,
        bodies: 0,
        service_rows: 0,
        new_rows: 0,
        identical_rows: 0,
        replaced_rows: replaced.len(),
        replaced_parts_dropped: 0,
        consumed_rows: usize::from(deleted_note.is_some()),
    };
    for row in &staged {
        match classify_name(&row.name) {
            RowName::Service(_) => stage.service_rows += 1,
            RowName::Descriptor(_) => stage.descriptors += 1,
            RowName::Body { .. } => stage.bodies += 1,
            RowName::Other => {}
        }
        if active
            .get(&row.key())
            .is_some_and(|known| known.sha256 == row.sha256)
        {
            stage.identical_rows += 1;
        }
    }
    report.stage = Some(stage);
    if let Some(note) = &deleted_note {
        report.warnings.push(if !pending_deleted.is_empty() && !unchanged {
            format!("the stage's `deleted` list is copied into deleted_dynupdate_{new_generation}: {note}")
        } else {
            "the stage's `deleted` list is consumed without publishing a removal alias".to_owned()
        });
    }
    if collection.is_some() && !unchanged {
        report.warnings.push(format!("native measured service collection: 16 unchanged raw SI aliases and siVersions tokens; {} exact additions to existing register file lists are saved for stopped-database recovery", appends.rows.len()));
    }
    report.active_generation = Some(old_generation.hyphenated().to_string());
    if unchanged {
        report.warnings.push(
            "the staged rows are the active ones byte for byte: no generation is published, ConfigSave is emptied".to_owned(),
        );
    } else {
        history_after.push(new_generation.hyphenated().to_string());
        report.new_generation = Some(new_generation.hyphenated().to_string());
        let mut aliases = activation.alias_rows();
        if !pending_deleted.is_empty() {
            aliases.push(format!("deleted_dynupdate_{new_generation}"));
        }
        report.published = Some(DynamicPublication {
            generation: new_generation.hyphenated().to_string(),
            previous_generation: old_generation.hyphenated().to_string(),
            history: history_after.clone(),
            alias_rows: aliases,
            replaced_in_place: vec!["root".to_owned(), "version".to_owned()],
            warn_after_generations: WARN_GENERATIONS,
        });
        if let Some(warning) = growth_warning(history_after.len()) {
            report.warnings.push(warning);
        }
    }
    // An unchanged stage is only consumed: nothing else is written, and nothing of the rest is said.
    let mut touched = if unchanged {
        vec!["ConfigSave"]
    } else {
        vec!["Config", "ConfigSave", "Params"]
    };
    if has_change_registrations && !unchanged {
        touched.push("_ConfigChngR");
        if registration.added_file_rows > 0 || !registration.dropped_files.is_empty() {
            touched.push("_ConfigChngR_ExtProps");
        }
    }
    if !parity.files_rewrites.is_empty() && !unchanged {
        touched.push("Files");
    }
    report.tables_touched = touched.into_iter().map(str::to_owned).collect();
    report.registrations =
        (has_change_registrations && !unchanged).then_some(RegistrationSummary {
            nodes: registration.nodes.len(),
            changed_objects: registration.changed_objects,
            rows_added: registration.added_rows,
            file_rows_added: registration.added_file_rows,
            objects_of_dropped_rows: registration.extra_objects.len(),
        });
    if options.backup != super::BackupPolicy::None {
        report.warnings.push(
            "a backup option is for a restructuring, which a dynamic apply never does: it is ignored".to_owned(),
        );
    }
    report.not_written = vec![
        "Params .ui rows (the platform's configuration-licensing records, track ui #340): never written".to_owned(),
        "the help/search index in Files (userDocs_ru*, userPostings_ru*, userVocabulary_ru*): not rebuilt; native-derived payloads can differ and help-search equivalence is not claimed".to_owned(),
        "the ordinary rows the aliases stand beside: they keep the previous text until an exclusive apply folds the generation".to_owned(),
    ];
    report.script_sha256 = Some(script_sha.clone());
    report.recovery_token = Some(script_sha[..16].to_owned());
    timings.total_ms = ms(total);
    report.timings = timings;
    Ok(DynamicPlan {
        report,
        script: Some(script),
        staged,
        replaced,
        mobile_versions_before: if unchanged { None } else { mobile_before },
        registration,
        reset_change_registrations: has_change_registrations && !unchanged,
        params_rewrites,
        params_preimages,
        appended_existing_rows: if unchanged { Vec::new() } else { appends.rows },
    })
}

/// Whether the staged configuration would be applied dynamically: a look, nothing is written. For
/// the hint the exclusive refusal gives (`можно применить динамически: --dynamic=force`).
pub fn would_qualify(sql: &SqlExec, options: &ConfigApplyOptions) -> bool {
    let mut options = options.clone();
    options.dry_run = true;
    matches!(plan_dynamic(sql, &options), Ok(plan) if !plan.report.nothing_to_apply)
}

/// Plans and, unless `dry_run`, applies the staged configuration as a dynamic generation.
pub fn apply_dynamic(sql: &SqlExec, options: &ConfigApplyOptions) -> Result<ConfigApplyReport> {
    let total = Instant::now();
    let client = require_client(sql)?;
    let mut plan = plan_dynamic(sql, options)?;
    if plan.report.nothing_to_apply {
        return Ok(plan.report);
    }
    let script = plan
        .script
        .clone()
        .ok_or_else(|| anyhow!("the plan has no script"))?;
    if let Some(path) = &options.script_output {
        write_artifact(path, script.as_bytes())?;
        plan.report.script_path = Some(path.clone());
    }
    if options.dry_run {
        plan.report.timings.total_ms = ms(total);
        return Ok(plan.report);
    }

    // The recovery artifact: what the transaction overwrites, and how to take the generation back.
    let started = Instant::now();
    let token = plan
        .report
        .recovery_token
        .clone()
        .ok_or_else(|| anyhow!("the plan has no recovery token"))?;
    let dir = options.recovery_dir.clone().unwrap_or_else(|| {
        recovery::artifact_dir(&recovery::default_root(), &options.database, &token)
    });
    let generation = plan
        .report
        .published
        .as_ref()
        .map(|published| published.generation.clone());
    recovery::write_recovery(
        client,
        &recovery::RecoveryRequest {
            database: &options.database,
            dir: &dir,
            token: &token,
            blobs: options.recovery_blobs,
            staged: &plan.staged,
            replaced: &plan.replaced,
            mobile_versions_before: plan.mobile_versions_before.as_deref(),
            reset_change_registrations: plan.reset_change_registrations,
            new_objects: &super::objects::NewObjects::default(),
            removals: &super::removals::Removals::default(),
            registration: &plan.registration,
            params_rewrites: &plan.params_rewrites,
            bound_params_preimages: Some(&plan.params_preimages),
            backup: None,
            dynamic_generation: generation.as_deref(),
            appended_existing_rows: &plan.appended_existing_rows,
        },
    )?;
    plan.report.recovery_dir = Some(dir.clone());
    plan.report.timings.recovery_ms = ms(started);

    let started = Instant::now();
    if let Err(error) = client.run_script(&script, ScriptVariables::Refuse) {
        if let Some((number, message)) = errors::server_error_of(&error)
            && let Some(typed) =
                errors::from_transaction_code(number, &message, &options.database, || {
                    other_sessions(client, &options.database, &options.own_ras_processes)
                        .unwrap_or_default()
                })
        {
            return Err(typed);
        }
        return Err(error.context(uncertain_commit_context(&options.database, &dir, &token)));
    }
    plan.report.timings.sql_ms = ms(started);
    plan.report.executed = true;

    // A last look, outside the transaction.
    let db = quote_ident(&options.database)?;
    let left = scalar_i64(
        client,
        &format!("SELECT COUNT_BIG(*) FROM {db}.dbo.ConfigSave"),
    )
    .with_context(|| format!(
        "the dynamic apply committed but its final ConfigSave verification failed; recovery artifact: {} (token {token}); inspect the database before any retry",
        dir.display()
    ))?;
    if left != 0 {
        bail!(
            "the dynamic apply committed, but ConfigSave now holds {left} row(s); recovery artifact: {} (token {token}); inspect the database before any retry",
            dir.display()
        );
    }
    if options.recovery_dir.is_none() && options.recovery_keep > 0 {
        match recovery::prune(
            &recovery::default_root(),
            &options.database,
            options.recovery_keep,
        ) {
            Ok(removed) if !removed.is_empty() => plan.report.warnings.push(format!(
                "{} older recovery artifact(s) of {} removed from {} (the newest {} are kept)",
                removed.len(),
                options.database,
                recovery::default_root().display(),
                options.recovery_keep
            )),
            Ok(_) => {}
            Err(error) => plan.report.warnings.push(format!(
                "could not prune older recovery artifacts: {error:#}"
            )),
        }
    }
    plan.report.timings.total_ms = ms(total);
    Ok(plan.report)
}

/// The rows of a stage by owner, for the reports of the tests.
#[cfg(test)]
fn owners_of(staged: &[RowMeta]) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut owners: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for row in staged {
        match classify_name(&row.name) {
            RowName::Descriptor(owner) | RowName::Body { owner, .. } => owners
                .entry(owner.to_ascii_lowercase())
                .or_default()
                .push(row.name.clone()),
            _ => {}
        }
    }
    owners
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) struct SourceDependencyProbe {
        rows: std::sync::Mutex<Vec<(RowMeta, Vec<u8>)>>,
        corrupt_blob: std::sync::atomic::AtomicBool,
        pending: std::sync::Mutex<Vec<(RowMeta, Vec<u8>)>>,
        params: std::sync::Mutex<Vec<(RowMeta, Vec<u8>)>>,
        blob_reads: std::sync::Mutex<Vec<(String, String)>>,
    }

    pub(super) fn source_dependency_names() -> Vec<String> {
        vec![
            "00000000-0000-4000-8000-000000000002".into(),
            "00000000-0000-4000-8000-000000000002.0".into(),
        ]
    }

    pub(super) fn source_dependency_fixture() -> SourceDependencyProbe {
        let configuration = "00000000-0000-4000-8000-000000000001";
        let owner = source_dependency_names()[0].clone();
        let class = crate::metadata_model::export::names::root_class_kinds()
            .iter()
            .find(|(_, kind)| *kind == "Catalog")
            .unwrap()
            .0;
        let blobs = vec![
            (
                "root".to_owned(),
                versions::deflate_row(format!("{{2,{configuration},0}}").as_bytes()).unwrap(),
            ),
            (
                configuration.to_owned(),
                versions::deflate_row(format!("{{{class},1,{owner}}}").as_bytes()).unwrap(),
            ),
            (
                owner.clone(),
                b"original descriptor retained by export".to_vec(),
            ),
            (format!("{owner}.0"), b"original module body".to_vec()),
        ];
        let rows = blobs
            .into_iter()
            .map(|(name, bytes)| {
                let mut row = meta(&name, &hex_lower(&Sha256::digest(&bytes)));
                row.data_size = bytes.len() as i64;
                row.byte_len = row.data_size;
                (row, bytes)
            })
            .collect();
        SourceDependencyProbe {
            rows: std::sync::Mutex::new(rows),
            corrupt_blob: false.into(),
            pending: std::sync::Mutex::new(Vec::new()),
            params: std::sync::Mutex::new(Vec::new()),
            blob_reads: std::sync::Mutex::new(Vec::new()),
        }
    }

    impl SqlClient for SourceDependencyProbe {
        fn dbms(&self) -> crate::sql::Dbms {
            crate::sql::Dbms::SqlServer
        }
        fn max_connections(&self) -> usize {
            1
        }
        fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
            panic!("readonly memory endpoint")
        }
        fn execute(&self, _: &str, _: &[crate::sql::SqlParam<'_>]) -> Result<u64> {
            panic!("readonly memory endpoint")
        }
        fn query_json(&self, _: &str) -> Result<Option<String>> {
            panic!("readonly memory endpoint")
        }
        fn write_rows(
            &self,
            _: &str,
            _: &[&str],
            _: &[Vec<crate::sql::SqlParam<'_>>],
        ) -> Result<u64> {
            panic!("readonly memory endpoint")
        }
        fn read_rows(
            &self,
            query: &str,
            _: &[crate::sql::SqlParam<'_>],
            each: &mut dyn FnMut(crate::sql::SqlRow) -> Result<()>,
        ) -> Result<()> {
            use crate::sql::{SqlRow, SqlValue};
            let blob = query.starts_with("SELECT TOP (2) BinaryData");
            let table = if query.contains("dbo.Params") {
                "Params"
            } else {
                assert!(query.contains("dbo.Config"), "unexpected table: {query}");
                "Config"
            };
            let mut candidates = if table == "Params" {
                self.params.lock().unwrap().clone()
            } else if query.contains("LIKE N'%!_dynupdate!_%'") {
                self.pending.lock().unwrap().clone()
            } else {
                let mut ordinary = self.rows.lock().unwrap().clone();
                if blob {
                    ordinary.extend(self.pending.lock().unwrap().clone());
                }
                ordinary
            };
            candidates.sort_by(|(a, _), (b, _)| (&a.name, a.part).cmp(&(&b.name, b.part)));
            for (row, bytes) in candidates {
                let whole_group = !blob
                    && (query.contains("LEN(FileName)")
                        || query.contains("LIKE N'%!_dynupdate!_%'"));
                let by_name = query.contains(&format!("N'{}'", row.name));
                if !whole_group && !by_name {
                    continue;
                }
                let values = if blob {
                    // Match the actual read_bound_blob causal predicates, then
                    // record every requested original before returning bytes.
                    assert!(query.contains(&format!("PartNo = {}", row.part)));
                    assert!(
                        query.contains(&format!("CONVERT(bigint, DataSize) = {}", row.data_size))
                    );
                    assert!(query.contains(&format!("DATALENGTH(BinaryData) = {}", row.byte_len)));
                    assert!(query.contains(&format!("0x{}", row.sha256)));
                    self.blob_reads
                        .lock()
                        .unwrap()
                        .push((table.to_owned(), row.name.clone()));
                    let raw = if self.corrupt_blob.load(std::sync::atomic::Ordering::Relaxed) {
                        b"wrong original".to_vec()
                    } else {
                        bytes
                    };
                    vec![SqlValue::Binary(raw)]
                } else {
                    vec![
                        SqlValue::Text(row.name),
                        SqlValue::Int(row.part.into()),
                        SqlValue::Int(row.data_size),
                        SqlValue::Int(row.byte_len),
                        SqlValue::Int(row.attributes.into()),
                        SqlValue::Text(row.creation),
                        SqlValue::Text(row.modified),
                        SqlValue::Text(row.sha256),
                    ]
                };
                each(SqlRow {
                    result_set: 0,
                    values,
                })?;
            }
            Ok(())
        }
    }

    #[test]
    fn source_dependency_capture_retains_original_graph_and_body_before_export() {
        let client = source_dependency_fixture();
        let proof =
            SourceOwnerPreimages::capture_client(&client, "lab", &source_dependency_names())
                .unwrap();
        proof.require_current_client(&client, "lab").unwrap();
        assert_eq!(proof.groups[0].originals.len(), 4);
        let sql = proof.precondition_sql("lab").unwrap();
        assert!(
            sql.contains("Attributes = 0")
                && sql.contains("Creation, 121")
                && sql.contains("Modified, 121")
        );
        for row in &proof.groups[0].rows {
            assert!(sql.contains(&format!("0x{}", row.sha256)));
        }
        assert!(sql.contains("LEN(FileName)") && sql.contains("TRY_CONVERT(uniqueidentifier"));
        assert!(sql.contains("dbo.Params WITH (UPDLOCK, HOLDLOCK)"));
        assert!(proof.precondition_sql("different").is_err());
    }

    #[test]
    fn source_dependency_proof_refuses_original_header_data_and_inventory_drift() {
        for fault in 0..8 {
            let client = source_dependency_fixture();
            let proof =
                SourceOwnerPreimages::capture_client(&client, "lab", &source_dependency_names())
                    .unwrap();
            let mut rows = client.rows.lock().unwrap();
            match fault {
                0 => rows[2].0.modified.push('1'),
                1 => rows[2].0.creation.push('1'),
                2 => rows[2].0.attributes = 1,
                3 => rows[2].0.sha256 = "f".repeat(64),
                4 => rows[3].0.sha256 = "e".repeat(64),
                5 => {
                    rows.remove(2);
                }
                6 => {
                    let duplicate = rows[2].clone();
                    rows.push(duplicate);
                }
                _ => rows[2].0.data_size += 1,
            }
            drop(rows);
            assert!(
                proof.require_current_client(&client, "lab").is_err(),
                "fault {fault}"
            );
        }
    }

    #[test]
    fn source_dependency_capture_refuses_missing_role_duplicate_and_unbound_bytes() {
        let client = source_dependency_fixture();
        client
            .corrupt_blob
            .store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(
            SourceOwnerPreimages::capture_client(&client, "lab", &source_dependency_names())
                .is_err()
        );
        for names in [
            vec![source_dependency_names()[0].clone()],
            vec![
                source_dependency_names()[1].clone(),
                source_dependency_names()[1].clone(),
            ],
            vec!["00000000-0000-4000-8000-000000000002.99".into()],
        ] {
            assert!(
                SourceOwnerPreimages::capture_client(&source_dependency_fixture(), "lab", &names)
                    .is_err()
            );
        }
        let wrong_role = source_dependency_fixture();
        wrong_role.rows.lock().unwrap()[3].0.name =
            "00000000-0000-4000-8000-000000000002.99".into();
        let names = vec![
            source_dependency_names()[0].clone(),
            "00000000-0000-4000-8000-000000000002.99".into(),
        ];
        let error = SourceOwnerPreimages::capture_client(&wrong_role, "lab", &names).unwrap_err();
        assert!(
            error.to_string().contains("measured original owner/role"),
            "{error:#}"
        );
    }

    fn source_dependency_row(name: &str, bytes: Vec<u8>) -> (RowMeta, Vec<u8>) {
        let mut row = meta(name, &hex_lower(&Sha256::digest(&bytes)));
        row.data_size = bytes.len() as i64;
        row.byte_len = row.data_size;
        (row, bytes)
    }

    const SOURCE_COVERAGE_GENERATION: &str = "00000000-0000-4000-8000-000000000090";
    const SOURCE_COVERAGE_ORDINARY: &str = "00000000-0000-4000-8000-000000000091";

    fn source_dependency_pending(client: &SourceDependencyProbe, originals: &[String]) {
        let ordinary = client.rows.lock().unwrap();
        let mut pending = client.pending.lock().unwrap();
        pending.push(source_dependency_row(
            "DynamicallyUpdated",
            format!("{{1,1,{SOURCE_COVERAGE_GENERATION}}}").into_bytes(),
        ));
        for name in originals {
            let bytes = ordinary
                .iter()
                .find(|(row, _)| row.name == *name)
                .unwrap()
                .1
                .clone();
            let alias = match name.rsplit_once('.') {
                Some((owner, suffix)) => {
                    format!("{owner}_dynupdate_{SOURCE_COVERAGE_GENERATION}.{suffix}")
                }
                None => format!("{name}_dynupdate_{SOURCE_COVERAGE_GENERATION}"),
            };
            pending.push(source_dependency_row(&alias, bytes));
        }
        client.params.lock().unwrap().push(source_dependency_row(
            "DynamicallyUpdated",
            format!("{{0,2,{SOURCE_COVERAGE_ORDINARY},{SOURCE_COVERAGE_GENERATION}}}").into_bytes(),
        ));
    }

    #[test]
    fn source_dependency_nonempty_pending_and_params_keep_original_guard_on_mutation() {
        for group_index in [1, 2] {
            for target in 0..if group_index == 1 { 3 } else { 1 } {
                for fault in 0..4 {
                    let client = source_dependency_fixture();
                    source_dependency_pending(&client, &source_dependency_names());
                    let proof = SourceOwnerPreimages::capture_client(
                        &client,
                        "lab",
                        &source_dependency_names(),
                    )
                    .unwrap();
                    assert_eq!(proof.groups[1].rows.len(), 3);
                    assert_eq!(proof.groups[2].rows.len(), 1);
                    proof.require_current_client(&client, "lab").unwrap();
                    // The actual Params predicate selects ONLY this exact marker.
                    // A new unrelated Params row is outside the bound inventory.
                    client.params.lock().unwrap().push(source_dependency_row(
                        "UnrelatedParamsRow",
                        b"outside marker inventory".to_vec(),
                    ));
                    proof.require_current_client(&client, "lab").unwrap();
                    let original_sql = proof.precondition_sql("lab").unwrap();
                    assert!(!original_sql.contains("UnrelatedParamsRow"));
                    for original_row in &proof.groups[group_index].rows {
                        assert!(original_sql.contains(&format!("0x{}", original_row.sha256)));
                    }
                    assert!(original_sql.contains("LIKE N'%!_dynupdate!_%'"));
                    let mut changed = if group_index == 1 {
                        client.pending.lock().unwrap()
                    } else {
                        client.params.lock().unwrap()
                    };
                    match fault {
                        0 => {
                            let name = changed[target].0.name.clone();
                            changed[target] =
                                source_dependency_row(&name, b"different original bytes".to_vec());
                        }
                        1 => changed[target].0.modified.push('1'),
                        2 => {
                            changed.remove(target);
                        }
                        _ => {
                            let mut extra = changed[target].clone();
                            if group_index == 1 {
                                extra.0.name.push_str("_dynupdate_phantom");
                            } else {
                                // SAME Params marker name really matches FileName;
                                // an extra part is an in-filter phantom, refused.
                                extra.0.part = 1;
                            }
                            changed.push(extra);
                        }
                    }
                    drop(changed);
                    assert!(
                        proof.require_current_client(&client, "lab").is_err(),
                        "group {group_index} fault {fault}"
                    );
                    assert_eq!(
                        proof.precondition_sql("lab").unwrap(),
                        original_sql,
                        "drift cannot replace the original proof by a fresh snapshot"
                    );
                }
            }
        }
    }

    fn source_dependency_native_template(
        case: &serde_json::Value,
    ) -> (SourceDependencyProbe, Vec<String>) {
        let client = source_dependency_fixture();
        let configuration = "00000000-0000-4000-8000-000000000001";
        let parent = case["parent"].as_str().unwrap();
        let owner = case["owner"].as_str().unwrap();
        let body = case["row"].as_str().unwrap();
        let kind = case["kind"].as_str().unwrap();
        let class = crate::metadata_model::export::names::root_class_kinds()
            .iter()
            .find(|(_, candidate)| *candidate == kind)
            .unwrap()
            .0;
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/dynamic-metadata-native");
        let mut rows = client.rows.lock().unwrap();
        rows.clear();
        rows.push(source_dependency_row(
            "root",
            versions::deflate_row(format!("{{2,{configuration},0}}").as_bytes()).unwrap(),
        ));
        rows.push(source_dependency_row(
            configuration,
            versions::deflate_row(format!("{{{class},1,{parent}}}").as_bytes()).unwrap(),
        ));
        for name in [parent, owner, body] {
            rows.push(source_dependency_row(
                name,
                std::fs::read(directory.join(format!("{name}.bin"))).unwrap(),
            ));
        }
        drop(rows);
        (
            client,
            vec![parent.to_owned(), owner.to_owned(), body.to_owned()],
        )
    }

    #[test]
    fn source_dependency_native_templates_capture_matching_aliases_and_refuse_conflicts() {
        let manifest: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/dynamic-metadata-native/manifest.json"
        ))
        .unwrap();
        let cases: Vec<_> = manifest["cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|case| case["family"] == "templates")
            .collect();
        assert_eq!(cases.len(), 3);
        for case in cases {
            let (client, selected) = source_dependency_native_template(case);
            source_dependency_pending(&client, &selected);
            let proof = SourceOwnerPreimages::capture_client(&client, "lab", &selected).unwrap();
            assert_eq!(proof.groups[0].rows.len(), 5);
            assert_eq!(proof.groups[1].rows.len(), 4);
            assert_eq!(proof.groups[2].rows.len(), 1);
            proof.require_current_client(&client, "lab").unwrap();
            let sql = proof.precondition_sql("lab").unwrap();
            for group in &proof.groups {
                for (row, bytes) in group.rows.iter().zip(&group.originals) {
                    assert_eq!(row.sha256, hex_lower(&Sha256::digest(bytes)));
                    assert!(sql.contains(&format!("0x{}", row.sha256)));
                }
            }
            for (position, reason) in [
                (1, "pending ownership/property change"),
                (2, "pending owned descriptor/property change"),
            ] {
                let (conflict, names) = source_dependency_native_template(case);
                source_dependency_pending(&conflict, &names);
                let mut pending = conflict.pending.lock().unwrap();
                let name = pending[position].0.name.clone();
                pending[position] = source_dependency_row(
                    &name,
                    versions::deflate_row(b"different pending descriptor/property bytes").unwrap(),
                );
                drop(pending);
                let error =
                    SourceOwnerPreimages::capture_client(&conflict, "lab", &names).unwrap_err();
                assert!(
                    error.to_string().contains(reason),
                    "{}: {error:#}",
                    case["role"]
                );
            }
            let (drift, names) = source_dependency_native_template(case);
            source_dependency_pending(&drift, &names);
            let original = SourceOwnerPreimages::capture_client(&drift, "lab", &names).unwrap();
            let before = original.precondition_sql("lab").unwrap();
            let mut rows = drift.rows.lock().unwrap();
            rows[2].0.creation.push('1');
            drop(rows);
            assert!(original.require_current_client(&drift, "lab").is_err());
            assert_eq!(original.precondition_sql("lab").unwrap(), before);
        }
    }

    fn source_dependency_blob_reads(client: &SourceDependencyProbe, table: &str) -> usize {
        client
            .blob_reads
            .lock()
            .unwrap()
            .iter()
            .filter(|(actual, _)| actual == table)
            .count()
    }

    #[test]
    fn source_dependency_full_capture_row_boundary_refuses_before_ordinary_blob() {
        for total in [
            dynamic_metadata::MAX_GRAPH_ROWS,
            dynamic_metadata::MAX_GRAPH_ROWS + 1,
        ] {
            let client = source_dependency_fixture();
            let mut rows = client.rows.lock().unwrap();
            for index in rows.len()..total {
                let name = format!("00000000-0000-4000-9000-{index:012x}");
                rows.push(source_dependency_row(&name, Vec::new()));
            }
            drop(rows);
            let result =
                SourceOwnerPreimages::capture_client(&client, "lab", &source_dependency_names());
            if total == dynamic_metadata::MAX_GRAPH_ROWS {
                let proof = result.unwrap();
                assert_eq!(proof.groups[0].rows.len(), total);
                assert_eq!(
                    source_dependency_blob_reads(&client, "Config"),
                    total + 2,
                    "all ordinary originals plus actual root/config ownership reads"
                );
                proof.require_current_client(&client, "lab").unwrap();
            } else {
                let error = result.unwrap_err();
                assert!(error.to_string().contains("row/byte budget"), "{error:#}");
                assert_eq!(source_dependency_blob_reads(&client, "Config"), 0);
            }
        }
    }

    fn source_dependency_pad_ordinary_bytes(client: &SourceDependencyProbe, bytes: usize) {
        let mut rows = client.rows.lock().unwrap();
        let mut remaining = bytes;
        for index in 0..2 {
            let size = remaining.min(MAX_ROW_BYTES);
            let name = format!("00000000-0000-4000-a000-{index:012x}");
            rows.push(source_dependency_row(&name, vec![b'x'; size]));
            remaining -= size;
        }
        assert_eq!(remaining, 0);
    }

    #[test]
    fn source_dependency_full_capture_byte_boundary_and_pending_cumulative_refusal() {
        for cumulative in [false, true] {
            for excess in [0usize, 1] {
                let client = source_dependency_fixture();
                if cumulative {
                    source_dependency_pending(&client, &source_dependency_names());
                }
                let ordinary_bytes: usize = client
                    .rows
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|(_, bytes)| bytes.len())
                    .sum();
                let pending_bytes: usize = client
                    .pending
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|(_, bytes)| bytes.len())
                    .sum();
                let params_bytes: usize = client
                    .params
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|(_, bytes)| bytes.len())
                    .sum();
                // For cumulative excess the pending group itself crosses the limit,
                // after a real bounded ordinary capture; Params is not reached.
                let target_padding = if cumulative && excess == 1 {
                    dynamic_metadata::MAX_GRAPH_BYTES + 1 - ordinary_bytes - pending_bytes
                } else {
                    dynamic_metadata::MAX_GRAPH_BYTES + excess
                        - ordinary_bytes
                        - pending_bytes
                        - params_bytes
                };
                source_dependency_pad_ordinary_bytes(&client, target_padding);
                let result = SourceOwnerPreimages::capture_client(
                    &client,
                    "lab",
                    &source_dependency_names(),
                );
                if excess == 0 {
                    let proof = result.unwrap();
                    let actual_bytes: usize = proof
                        .groups
                        .iter()
                        .flat_map(|g| &g.originals)
                        .map(Vec::len)
                        .sum();
                    assert_eq!(actual_bytes, dynamic_metadata::MAX_GRAPH_BYTES);
                    proof.require_current_client(&client, "lab").unwrap();
                } else {
                    let error = result.unwrap_err();
                    assert!(error.to_string().contains("row/byte budget"), "{error:#}");
                    let reads = client.blob_reads.lock().unwrap();
                    if cumulative {
                        assert_eq!(
                            reads.len(),
                            6,
                            "only six ordinary blobs acquired before pending reservation"
                        );
                        assert!(
                            reads.iter().all(|(table, name)| table == "Config"
                                && name != "DynamicallyUpdated"
                                && !name.contains("_dynupdate_")),
                            "no blob from the refused pending group, nor Params"
                        );
                    } else {
                        assert!(
                            reads.is_empty(),
                            "ordinary over-budget headers refuse before their first blob"
                        );
                    }
                }
            }
        }
    }

    struct PendingRows {
        ordinary: Vec<RowMeta>,
        ordinary_blob: Vec<u8>,
        aliases: HashMap<String, Vec<u8>>,
    }

    struct GraphHeaderProbe {
        blobs: HashMap<String, Vec<u8>>,
        parents: Vec<RowMeta>,
        large_reads: std::sync::atomic::AtomicUsize,
    }

    impl SqlClient for GraphHeaderProbe {
        fn dbms(&self) -> crate::sql::Dbms {
            crate::sql::Dbms::SqlServer
        }
        fn max_connections(&self) -> usize {
            1
        }
        fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
            panic!("read-only probe")
        }
        fn execute(&self, _: &str, _: &[crate::sql::SqlParam<'_>]) -> Result<u64> {
            panic!("read-only probe")
        }
        fn query_json(&self, _: &str) -> Result<Option<String>> {
            panic!("read-only probe")
        }
        fn write_rows(
            &self,
            _: &str,
            _: &[&str],
            _: &[Vec<crate::sql::SqlParam<'_>>],
        ) -> Result<u64> {
            panic!("read-only probe")
        }
        fn read_rows(
            &self,
            query: &str,
            _: &[crate::sql::SqlParam<'_>],
            each: &mut dyn FnMut(crate::sql::SqlRow) -> Result<()>,
        ) -> Result<()> {
            use crate::sql::{SqlRow, SqlValue};
            let found = self
                .blobs
                .iter()
                .find(|(name, _)| query.contains(&format!("FileName = N'{name}'")));
            if query.starts_with("SELECT TOP (2) BinaryData") {
                if let Some((_, bytes)) = found {
                    return each(SqlRow {
                        result_set: 0,
                        values: vec![SqlValue::Binary(bytes.clone())],
                    });
                }
                self.large_reads
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                bail!("large graph blob requested before header budget refusal");
            }
            let rows = if let Some((name, bytes)) = found {
                let mut row = meta(name, &hex_lower(&Sha256::digest(bytes)));
                row.byte_len = bytes.len() as i64;
                row.data_size = row.byte_len;
                vec![row]
            } else {
                assert!(query.contains("FileName IN ("));
                self.parents.clone()
            };
            for row in rows {
                each(SqlRow {
                    result_set: 0,
                    values: vec![
                        SqlValue::Text(row.name),
                        SqlValue::Int(row.part.into()),
                        SqlValue::Int(row.data_size),
                        SqlValue::Int(row.byte_len),
                        SqlValue::Int(row.attributes.into()),
                        SqlValue::Text(row.creation),
                        SqlValue::Text(row.modified),
                        SqlValue::Text(row.sha256),
                    ],
                })?;
            }
            Ok(())
        }
    }

    #[test]
    fn whole_graph_headers_refuse_before_any_large_parent_blob_read() {
        let config = "00000000-0000-4000-8000-000000000001";
        let ids = [
            "00000000-0000-4000-8000-000000000002",
            "00000000-0000-4000-8000-000000000003",
        ];
        let class = crate::metadata_model::export::names::root_class_kinds()
            .iter()
            .find(|(_, kind)| *kind == "Catalog")
            .unwrap()
            .0;
        let deflate = versions::deflate_row;
        let client = GraphHeaderProbe {
            blobs: HashMap::from([
                (
                    "root".into(),
                    deflate(format!("{{2,{config},0}}").as_bytes()).unwrap(),
                ),
                (
                    config.into(),
                    deflate(format!("{{{class},2,{},{}}}", ids[0], ids[1]).as_bytes()).unwrap(),
                ),
            ]),
            parents: ids
                .iter()
                .map(|id| {
                    let mut row = meta(id, "unused");
                    row.byte_len = MAX_ROW_BYTES as i64;
                    row.data_size = row.byte_len;
                    row
                })
                .collect(),
            large_reads: std::sync::atomic::AtomicUsize::new(0),
        };
        let requested = HashSet::from(["00000000-0000-4000-8000-000000000004".into()]);
        let err = object_kinds(&client, "fixture", &requested, &[], &[])
            .err()
            .expect("over-budget graph must refuse");
        assert!(err.to_string().contains("row/byte budget"), "{err:#}");
        assert_eq!(
            client
                .large_reads
                .load(std::sync::atomic::Ordering::Relaxed),
            0,
            "headers must refuse before any oversized graph body is read"
        );
    }

    #[test]
    fn semantic_comparison_refuses_full_plaintext_without_stream_end() {
        use std::io::Write;
        let profile = MssqlNativePlatformProfile::Platform8_3_27_2214;
        let plain = b"\xef\xbb\xbf{1,0}";
        let valid = versions::deflate_row(plain).unwrap();
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(plain).unwrap();
        encoder.flush().unwrap();
        let unfinished = encoder.get_ref().clone();
        assert_eq!(
            versions::inflate_row(&unfinished).unwrap(),
            plain,
            "meaningful legacy full-plaintext reproduction"
        );
        for name in ["descriptor", "common-form.1", "root"] {
            assert!(
                same_semantic_text(name, &valid, &unfinished, profile).is_err(),
                "{name}: full plaintext alone must not accept incomplete stream"
            );
        }
    }

    #[test]
    fn semantic_comparison_refuses_row_overflow() {
        use std::io::Write;
        let profile = MssqlNativePlatformProfile::Platform8_3_27_2214;
        let oversized = vec![b'x'; dynamic_metadata::MAX_PLAIN_ROW + 1];
        let valid_large = versions::deflate_row(&oversized).unwrap();
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(&oversized).unwrap();
        let other_large = encoder.finish().unwrap();
        assert_ne!(valid_large, other_large);
        assert!(same_semantic_text("common-form.1", &valid_large, &other_large, profile).is_err());
    }

    fn identical_invalid_semantic_rows() -> Vec<Vec<u8>> {
        use std::io::Write;
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(b"\xef\xbb\xbf{1,0}").unwrap();
        encoder.flush().unwrap();
        vec![
            encoder.get_ref().clone(),
            versions::deflate_row(&vec![b'x'; dynamic_metadata::MAX_PLAIN_ROW + 1]).unwrap(),
        ]
    }

    #[test]
    fn identical_invalid_semantic_bytes_are_not_an_equality_shortcut() {
        let profile = MssqlNativePlatformProfile::Platform8_3_27_2214;
        for invalid in identical_invalid_semantic_rows() {
            for name in ["descriptor", "common-form.1", "root"] {
                assert!(
                    same_semantic_text(name, &invalid, &invalid, profile).is_err(),
                    "{name}: identical bytes still require bounded complete decoding"
                );
            }
        }
        let valid = versions::deflate_row(b"\xef\xbb\xbf{1,0}").unwrap();
        for name in ["descriptor", "common-form.1", "root"] {
            assert!(same_semantic_text(name, &valid, &valid, profile).unwrap());
        }
    }

    #[test]
    fn identical_descriptor_and_help_judgment_require_strict_semantic_decoding() {
        let profile = MssqlNativePlatformProfile::Platform8_3_27_2214;
        for invalid in identical_invalid_semantic_rows() {
            for name in [MODULE.to_owned(), format!("{FORM}.1")] {
                let rows = vec![
                    meta("root", "same"),
                    meta("version", "same"),
                    meta("versions", "same"),
                    meta(&name, &hex_lower(&Sha256::digest(&invalid))),
                ];
                let active = rows.iter().cloned().map(|row| (row.key(), row)).collect();
                let mut compare = |row_name: &str| {
                    assert_eq!(row_name, name);
                    same_semantic_text(row_name, &invalid, &invalid, profile)
                };
                assert!(
                    judge_rows(&rows, &active, &kinds(), &mut compare).is_err(),
                    "{name}: production judge must propagate the strict semantic refusal even with identical SHA"
                );
            }
        }
    }

    #[test]
    fn graph_budget_counts_every_header_and_decoded_byte_at_exact_boundaries() {
        let mut budget = GraphBudget::default();
        // Root, configuration descriptor, parent, then child: no giant allocation.
        for (name, size) in [
            ("root", 1),
            ("configuration", 2),
            ("parent", MAX_ROW_BYTES),
            (
                "child",
                dynamic_metadata::MAX_GRAPH_BYTES - MAX_ROW_BYTES - 3,
            ),
        ] {
            let mut header = meta(name, "unused");
            header.byte_len = size as i64;
            budget.reserve(&[header]).unwrap();
        }
        assert_eq!(
            (budget.rows, budget.compressed),
            (4, dynamic_metadata::MAX_GRAPH_BYTES)
        );
        let mut extra = meta("child-over-limit", "unused");
        extra.byte_len = 1;
        assert!(budget.reserve(&[extra]).is_err());
        assert_eq!(
            (budget.rows, budget.compressed),
            (4, dynamic_metadata::MAX_GRAPH_BYTES),
            "failed reservation is atomic"
        );
        let mut row_budget = GraphBudget::default();
        let mut zero = meta("header", "unused");
        zero.byte_len = 0;
        row_budget
            .reserve(&vec![zero.clone(); dynamic_metadata::MAX_GRAPH_ROWS])
            .unwrap();
        assert!(row_budget.reserve(&[zero]).is_err());
        let mut negative = meta("negative", "unused");
        negative.byte_len = -1;
        assert!(GraphBudget::default().reserve(&[negative]).is_err());
        let payload = b"decoded-budget-boundary";
        budget.decoded = dynamic_metadata::MAX_GRAPH_BYTES - payload.len();
        let compressed = versions::deflate_row(payload).unwrap();
        assert_eq!(budget.inflate(&compressed).unwrap(), payload);
        assert_eq!(budget.decoded, dynamic_metadata::MAX_GRAPH_BYTES);
        assert!(
            budget
                .inflate(&versions::deflate_row(b"x").unwrap())
                .is_err()
        );
        assert_eq!(budget.decoded, dynamic_metadata::MAX_GRAPH_BYTES);
    }

    #[test]
    fn semantic_comparison_keeps_valid_compression_variants_and_exact_85_root_restamp() {
        use std::io::Write;
        let profile = MssqlNativePlatformProfile::Platform8_3_27_2214;
        let plain = b"\xef\xbb\xbf{1,0}";
        let valid = versions::deflate_row(plain).unwrap();
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::none());
        encoder.write_all(plain).unwrap();
        let different = encoder.finish().unwrap();
        assert_ne!(valid, different);
        for name in ["descriptor", "common-form.1", "root"] {
            assert!(same_semantic_text(name, &valid, &different, profile).unwrap());
            assert!(
                !same_semantic_text(
                    name,
                    &valid,
                    &versions::deflate_row(b"changed").unwrap(),
                    profile
                )
                .unwrap()
            );
            let mut trailing = valid.clone();
            trailing.push(0);
            assert!(same_semantic_text(name, &valid, &trailing, profile).is_err());
            for end in 0..valid.len() {
                assert!(same_semantic_text(name, &different, &valid[..end], profile).is_err());
            }
        }
        let active =
            include_bytes!("../../tests/fixtures/platform85-dynamic/root-active.native.bin");
        let staged =
            include_bytes!("../../tests/fixtures/platform85-dynamic/root-staged.native.bin");
        let active = versions::deflate_row(active).unwrap();
        let staged = versions::deflate_row(staged).unwrap();
        assert!(
            same_semantic_text(
                "root",
                &active,
                &staged,
                MssqlNativePlatformProfile::Platform8_5_1_1150
            )
            .unwrap()
        );
        assert!(!same_semantic_text("root", &active, &staged, profile).unwrap());
    }

    impl SqlClient for PendingRows {
        fn dbms(&self) -> crate::sql::Dbms {
            crate::sql::Dbms::SqlServer
        }
        fn max_connections(&self) -> usize {
            1
        }
        fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
            panic!("judgment is read-only")
        }
        fn execute(&self, _: &str, _: &[crate::sql::SqlParam<'_>]) -> Result<u64> {
            panic!("judgment is read-only")
        }
        fn query_json(&self, _: &str) -> Result<Option<String>> {
            panic!("judgment is read-only")
        }
        fn write_rows(
            &self,
            _: &str,
            _: &[&str],
            _: &[Vec<crate::sql::SqlParam<'_>>],
        ) -> Result<u64> {
            panic!("judgment is read-only")
        }
        fn read_rows(
            &self,
            query: &str,
            _: &[crate::sql::SqlParam<'_>],
            each: &mut dyn FnMut(crate::sql::SqlRow) -> Result<()>,
        ) -> Result<()> {
            use crate::sql::{SqlRow, SqlValue};
            assert!(query.starts_with("SELECT TOP (2)"));
            if query.starts_with("SELECT TOP (2) BinaryData") {
                let blob = self
                    .aliases
                    .iter()
                    .find_map(|(name, blob)| {
                        query
                            .contains(&format!("FileName = N'{name}'"))
                            .then_some(blob)
                    })
                    .unwrap_or(&self.ordinary_blob);
                return each(SqlRow {
                    result_set: 0,
                    values: vec![SqlValue::Binary(blob.clone())],
                });
            }
            for meta in &self.ordinary {
                each(SqlRow {
                    result_set: 0,
                    values: vec![
                        SqlValue::Text(meta.name.clone()),
                        SqlValue::Int(meta.part.into()),
                        SqlValue::Int(meta.data_size),
                        SqlValue::Int(meta.byte_len),
                        SqlValue::Int(meta.attributes.into()),
                        SqlValue::Text(meta.creation.clone()),
                        SqlValue::Text(meta.modified.clone()),
                        SqlValue::Text(meta.sha256.clone()),
                    ],
                })?;
            }
            Ok(())
        }
    }

    #[test]
    fn retained_metadata_reads_are_bounded_and_bind_full_ordinary_headers_before_sql() {
        use std::io::Write;
        let deflate = |text: &[u8]| {
            let mut encoder =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
            encoder.write_all(text).unwrap();
            encoder.finish().unwrap()
        };
        let id = "313d9858-3995-4a4c-b2b0-15d2350417b4";
        let ordinary = format!("{id}.0");
        let blob = deflate(b"\xef\xbb\xbfFunction Marker() Export\nReturn \"A\";\nEndFunction");
        let next = deflate(b"\xef\xbb\xbfFunction Marker() Export\nReturn \"B\";\nEndFunction");
        let history = vec![
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
        ];
        let mut baseline = meta(&ordinary, &hex_lower(&Sha256::digest(&blob)));
        baseline.byte_len = blob.len() as i64;
        baseline.data_size = baseline.byte_len;
        baseline.creation = "2026-10-01 12:00:00.000".to_owned();
        baseline.modified = "2026-10-01 13:00:00.000".to_owned();
        let aliases: Vec<_> = history
            .iter()
            .map(|generation| {
                let mut row = baseline.clone();
                row.name = format!("{id}_dynupdate_{generation}.0");
                row.byte_len = next.len() as i64;
                row.data_size = row.byte_len;
                row.sha256 = hex_lower(&Sha256::digest(&next));
                row
            })
            .collect();
        let mut client = PendingRows {
            ordinary: vec![baseline.clone()],
            ordinary_blob: blob,
            aliases: aliases
                .iter()
                .map(|row| (row.name.clone(), next.clone()))
                .collect(),
        };
        let owners: Owners = HashMap::from([(id.to_owned(), "CommonModule")]).into();
        let mut metas = Vec::new();
        validate_pending_metadata(
            &client,
            "[fixture]",
            &aliases,
            &history,
            &owners,
            &mut metas,
        )
        .unwrap();
        assert_eq!(
            metas,
            vec![baseline.clone()],
            "one ordinary preimage covers multiple generations"
        );
        let guard = dynamic_overlay::guard("Config", &format!("FileName = N'{ordinary}'"), &metas);
        assert!(
            guard.contains("Attributes = 0")
                && guard.contains(&baseline.creation)
                && guard.contains(&baseline.modified)
                && guard.contains(&baseline.sha256)
        );
        let mut changed_header = metas.clone();
        changed_header[0].modified = "drift".to_owned();
        assert!(
            validate_pending_metadata(
                &client,
                "[fixture]",
                &aliases,
                &history,
                &owners,
                &mut changed_header
            )
            .is_err()
        );
        for variant in 0..5 {
            client.ordinary = vec![baseline.clone()];
            match variant {
                0 => client.ordinary.clear(),
                1 => client.ordinary.push(baseline.clone()),
                2 => client.ordinary[0].attributes = 1,
                3 => client.ordinary[0].part = 1,
                4 => client.ordinary[0].data_size += 1,
                _ => unreachable!(),
            }
            assert!(
                validate_pending_metadata(
                    &client,
                    "[fixture]",
                    &aliases,
                    &history,
                    &owners,
                    &mut Vec::new()
                )
                .is_err(),
                "variant {variant}"
            );
        }
    }

    #[test]
    fn manually_staged_ordinary_noop_cannot_discard_pending_alias_revert_without_deleted() {
        let name = "313d9858-3995-4a4c-b2b0-15d2350417b4.0";
        let ordinary = b"ordinary A";
        let mut body = meta(name, &hex_lower(&Sha256::digest(ordinary)));
        body.byte_len = ordinary.len() as i64;
        body.data_size = body.byte_len;
        let history = vec![uuid::Uuid::new_v4().to_string()];
        // The engine ordinary image is a byte no-op, while the effective generation is B.
        // No deleted row is staged first; the second manually stages the empty list.
        for deleted in [false, true] {
            let mut staged = vec![body.clone()];
            if deleted {
                staged.push(meta("deleted", "unused"));
            }
            let refused =
                judge_effective_noop(&staged, &history, &[], &mut |_| Ok(b"pending B".to_vec()))
                    .unwrap_err();
            assert!(refused.downcast_ref::<NeedsNativeApply>().is_some());
            assert!(
                judge_effective_noop(&staged, &history, &[], &mut |_| Ok(ordinary.to_vec()))
                    .is_ok()
            );
        }
        assert!(
            judge_effective_noop(&[body], &[], &[], &mut |_| panic!(
                "clean noop must not read aliases"
            ))
            .is_ok()
        );
    }

    #[test]
    fn unfinished_phantom_guard_locks_new_suffix_as_well_as_named_markers() {
        let sql = unfinished_guard();
        assert!(sql.contains("WITH (UPDLOCK, HOLDLOCK)"));
        assert!(sql.contains("OR FileName LIKE N'%.new'"));
        for name in sqlgen::UNFINISHED_NAMES {
            assert!(sql.contains(&format!("N'{name}'")));
        }
    }

    const MODULE: &str = "313d9858-3995-4a4c-b2b0-15d2350417b4";
    const FORM: &str = "07d3ab2c-5a1c-4d50-9d7e-0c9f3a1c2b44";
    const CATALOG: &str = "5eab8a1b-1111-4222-8333-444455556666";

    fn meta(name: &str, sha: &str) -> RowMeta {
        RowMeta {
            name: name.to_owned(),
            part: 0,
            data_size: 10,
            byte_len: 10,
            attributes: 0,
            creation: "2026-01-01 00:00:00".to_owned(),
            modified: "2026-01-01 00:00:00".to_owned(),
            sha256: sha.to_owned(),
        }
    }

    fn kinds() -> Owners {
        HashMap::from([
            (MODULE.to_owned(), "CommonModule"),
            (FORM.to_owned(), "CommonForm"),
            (CATALOG.to_owned(), "Catalog"),
        ])
        .into()
    }

    /// The active rows of the names, with the digest "aa".
    fn active_of(names: &[&str]) -> HashMap<(String, i32), RowMeta> {
        names
            .iter()
            .map(|name| {
                let row = meta(name, "aa");
                (row.key(), row)
            })
            .collect()
    }

    fn delta() -> Vec<RowMeta> {
        vec![
            meta(MODULE, "aa"),
            meta(&format!("{MODULE}.0"), "bb"),
            meta("root", "aa"),
            meta("version", "aa"),
            meta("versions", "cc"),
        ]
    }

    fn all_active() -> HashMap<(String, i32), RowMeta> {
        active_of(&[
            MODULE,
            &format!("{MODULE}.0"),
            FORM,
            &format!("{FORM}.0"),
            CATALOG,
            &format!("{CATALOG}.0"),
            "root",
            "version",
            "versions",
        ])
    }

    fn judged(staged: &[RowMeta], same: bool) -> Vec<String> {
        judge_rows(staged, &all_active(), &kinds(), &mut |_| Ok(same)).unwrap()
    }

    #[test]
    fn a_module_body_with_its_descriptor_and_the_service_rows_qualifies() {
        assert_eq!(judged(&delta(), true), Vec::<String>::new());
        // the same for a common form
        let form = vec![
            meta(FORM, "aa"),
            meta(&format!("{FORM}.0"), "bb"),
            meta("root", "aa"),
            meta("version", "aa"),
            meta("versions", "cc"),
        ];
        assert_eq!(judged(&form, true), Vec::<String>::new());
        assert_eq!(owners_of(&form).len(), 1);
    }

    #[test]
    fn initial_85_cohort_accepts_only_one_existing_top_level_body() {
        let stage = delta();
        let active = all_active();
        let check = |rows: &[RowMeta], kinds: &HashMap<String, &'static str>| {
            judge_initial_85_cohort(rows, &active, kinds, &[], &[], false)
        };
        assert!(check(&stage, &kinds().kinds).is_empty());
        let mut form = kinds();
        form.kinds.insert(MODULE.to_owned(), "CommonForm");
        assert!(check(&stage, &form.kinds).is_empty());
        for kind in ["Form", "Catalog", "Template", "HTTPService"] {
            let mut owners = kinds();
            owners.kinds.insert(MODULE.to_owned(), kind);
            assert!(!check(&stage, &owners.kinds).is_empty(), "{kind}");
        }
        let mut unknown = kinds();
        unknown.kinds.remove(MODULE);
        assert!(!check(&stage, &unknown.kinds).is_empty());
        for replacement in [
            format!("{MODULE}.1"),
            format!("{FORM}.0"),
            "deleted".to_owned(),
        ] {
            let mut changed = stage.clone();
            changed[1].name = replacement;
            assert!(!check(&changed, &kinds().kinds).is_empty());
        }
        let mut no_change = stage.clone();
        no_change[1].sha256 = active.get(&no_change[1].key()).unwrap().sha256.clone();
        assert!(!check(&no_change, &kinds().kinds).is_empty());
        let mut duplicate = stage.clone();
        duplicate[0] = duplicate[2].clone();
        assert!(!check(&duplicate, &kinds().kinds).is_empty());
        let mut extra = stage.clone();
        extra.push(meta("deleted", "bb"));
        assert!(!check(&extra, &kinds().kinds).is_empty());
        let mut missing = active.clone();
        missing.remove(&(MODULE.to_owned(), 0));
        assert!(
            !judge_initial_85_cohort(&stage, &missing, &kinds().kinds, &[], &[], false).is_empty()
        );
    }

    #[test]
    fn initial_85_cohort_refuses_history_overlays_and_unmeasured_headers() {
        let stage = delta();
        let active = all_active();
        assert!(
            !judge_initial_85_cohort(&stage, &active, &kinds().kinds, &[], &[], true).is_empty()
        );
        assert!(
            !judge_initial_85_cohort(
                &stage,
                &active,
                &kinds().kinds,
                &["generation".to_owned()],
                &[],
                false
            )
            .is_empty()
        );
        assert!(
            !judge_initial_85_cohort(
                &stage,
                &active,
                &kinds().kinds,
                &[],
                &[meta("alias", "aa")],
                false
            )
            .is_empty()
        );
        for index in 0..stage.len() {
            for (part, flags, size, bytes) in [
                (1, 0, 10, 10),
                (0, 1, 10, 10),
                (0, 0, 11, 10),
                (0, 0, -1, -1),
            ] {
                let mut bad = stage.clone();
                bad[index].part = part;
                bad[index].attributes = flags;
                bad[index].data_size = size;
                bad[index].byte_len = bytes;
                assert!(
                    !judge_initial_85_cohort(&bad, &active, &kinds().kinds, &[], &[], false)
                        .is_empty()
                );
            }
        }
        // The profile cohort does not excuse a changed descriptor or version.
        assert!(
            judge_rows(&stage, &active, &kinds(), &mut |name| Ok(name != MODULE))
                .unwrap()
                .iter()
                .any(|reason| reason.contains("descriptor's text"))
        );
        let mut changed_version = stage.clone();
        changed_version[3].sha256 = "changed".to_owned();
        assert!(
            judge_rows(&changed_version, &active, &kinds(), &mut |_| Ok(false))
                .unwrap()
                .iter()
                .any(|reason| reason.starts_with("version:"))
        );
    }

    #[test]
    fn initial_85_module_layout_is_bounded_and_fail_closed() {
        use crate::v8_container::{V8Element, build_v8_container, make_v8_element_header};
        use std::io::Write;
        let deflate = |bytes: &[u8]| {
            let mut out =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
            out.write_all(bytes).unwrap();
            out.finish().unwrap()
        };
        let element = |name: &str, data: &[u8]| V8Element {
            name: name.to_owned(),
            header: make_v8_element_header(name),
            data: data.to_vec(),
        };
        let info = element("info", b"\xef\xbb\xbf{3,1,0,\"\",0}");
        let text = element("text", b"\xef\xbb\xbfProcedure Marker()\nEndProcedure");
        let packed = |elements: &[V8Element]| deflate(&build_v8_container(elements).unwrap());
        assert_eq!(
            initial_85_module_text(&packed(&[info.clone(), text.clone()])).unwrap(),
            text.data
        );
        // Container order is semantic; missing, unknown, duplicate and changed
        // records or invalid encoding do not approximate a valid module.
        assert!(initial_85_module_text(&packed(&[text.clone(), info.clone()])).is_ok());
        for bad in [
            vec![text.clone()],
            vec![info.clone(), text.clone(), text.clone()],
            vec![info.clone(), element("unknown", &text.data)],
            vec![element("info", b"\xef\xbb\xbf{3,2,0,\"\",0}"), text.clone()],
            vec![info.clone(), element("text", b"\xef\xbb\xbf\xff")],
            vec![info.clone(), element("text", b"missing BOM")],
        ] {
            assert!(initial_85_module_text(&packed(&bad)).is_err());
        }
        assert!(initial_85_module_text(&deflate(b"\xef\xbb\xbfplain source")).is_err());
        let mut trailing = packed(&[info, text]);
        trailing.extend_from_slice(b"unmeasured trailing data");
        assert!(initial_85_module_text(&trailing).is_err());
        assert!(initial_85_module_text(&deflate(&vec![0; 8 * 1024 * 1024 + 1])).is_err());
    }

    #[test]
    fn initial_85_module_requires_a_final_deflate_block() {
        use crate::v8_container::{V8Element, build_v8_container, make_v8_element_header};
        use std::io::{Read, Write};
        let element = |name: &str, data: &[u8]| V8Element {
            name: name.to_owned(),
            header: make_v8_element_header(name),
            data: data.to_vec(),
        };
        let container = build_v8_container(&[
            element("info", b"\xef\xbb\xbf{3,1,0,\"\",0}"),
            element("text", b"\xef\xbb\xbfProcedure Marker()\nEndProcedure"),
        ])
        .unwrap();
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(&container).unwrap();
        encoder.flush().unwrap();
        let incomplete = encoder.get_ref().clone();
        // A complete uncompressed v8 container is already present. A reader's
        // EOF success is insufficient: the DEFLATE stream has no final block.
        let mut legacy_plain = Vec::new();
        flate2::read::DeflateDecoder::new(incomplete.as_slice())
            .read_to_end(&mut legacy_plain)
            .unwrap();
        assert_eq!(legacy_plain, container);
        assert!(initial_85_module_text(&incomplete).is_err());
        let complete = encoder.finish().unwrap();
        assert!(initial_85_module_text(&complete).is_ok());
        for length in 0..complete.len() {
            assert!(
                initial_85_module_text(&complete[..length]).is_err(),
                "prefix {length}"
            );
        }
    }

    #[test]
    fn unmeasured_stage_flags_are_refused_before_semantic_reads() {
        for name in [MODULE.to_owned(), "root".to_owned(), "deleted".to_owned()] {
            let mut staged = delta();
            if name == "deleted" {
                staged.push(meta("deleted", "aa"));
            }
            let row = staged.iter_mut().find(|row| row.name == name).unwrap();
            row.attributes = 1;
            // Otherwise this descriptor/root would need a semantic byte read.
            row.sha256 = "different".to_owned();
            let reasons = judge_rows(&staged, &all_active(), &kinds(), &mut |read| {
                assert_ne!(read, name, "flagged payload must not be interpreted");
                Ok(true)
            })
            .unwrap();
            assert!(
                reasons.iter().any(|reason| {
                    reason.starts_with(&name) && reason.contains("only Attributes=0")
                }),
                "{reasons:?}"
            );
        }
    }

    #[test]
    fn full_stage_header_guard_precedes_even_noop_consumption() {
        let generation = "719baa18-69ed-439a-8962-1de53d98e05e";
        let rows: Vec<_> = [MODULE, "root", "version", "versions"]
            .into_iter()
            .map(|name| {
                let bytes = if name == "versions" {
                    super::super::versions::deflate_row(
                        format!("{{1,4,\"\",{generation},\"root\",x}}").as_bytes(),
                    )
                    .unwrap()
                } else {
                    b"unchanged".to_vec()
                };
                MainStorageRow {
                    file_name: name.to_owned(),
                    part_no: 0,
                    creation: "2026-01-01 00:00:00.000".to_owned(),
                    modified: "2026-01-01 00:00:00.000".to_owned(),
                    attributes: 0,
                    data_size: bytes.len() as u64,
                    binary_data: bytes,
                }
            })
            .collect();
        let consumed = MainStorageRow {
            file_name: "deleted".to_owned(),
            ..rows[0].clone()
        };
        let stage: Vec<_> = rows
            .iter()
            .chain(std::iter::once(&consumed))
            .map(|row| RowMeta {
                name: row.file_name.clone(),
                part: row.part_no,
                data_size: row.data_size as i64,
                byte_len: row.binary_data.len() as i64,
                attributes: row.attributes as i16,
                creation: row.creation.clone(),
                modified: row.modified.clone(),
                sha256: hex_lower(&Sha256::digest(&row.binary_data)),
            })
            .collect();
        let plan = prepare_main_activation(
            MainActivationMode::Online,
            rows.clone(),
            MainActivationSnapshot {
                config_rows: rows,
                config_dynamically_updated: None,
                params_dynamically_updated: None,
            },
            &[MODULE.to_owned()],
            true,
        )
        .unwrap()
        .with_consumed_stage_rows(vec![consumed])
        .with_precondition_sql(dynamic_overlay::guard("ConfigSave", "1=1", &stage));
        assert!(plan.is_no_op());
        let sql = render_main_activation_sql("lab", &plan, None).unwrap().sql;
        let guard = sql.find("Pending ConfigSave row drifted").unwrap();
        assert!(sql.find("BEGIN TRANSACTION").unwrap() < guard);
        assert!(sql.find("THROW 57204").unwrap() < guard);
        assert!(guard < sql.find("DELETE FROM dbo.ConfigSave;").unwrap());
        assert_eq!(sql.matches("Pending ConfigSave row drifted").count(), 5);
        assert!(sql.contains("FileName = N'deleted' AND PartNo = 0"));
        assert!(!sql.contains("INSERT dbo.Config"));
    }

    #[test]
    fn the_rows_are_judged_one_by_one_and_every_reason_is_named() {
        // an object of another kind
        let catalog = vec![
            meta(CATALOG, "aa"),
            meta(&format!("{CATALOG}.0"), "bb"),
            meta("root", "aa"),
            meta("version", "aa"),
            meta("versions", "cc"),
        ];
        assert!(judged(&catalog, true).is_empty());
        let mut unsupported = kinds();
        unsupported.kinds.insert(CATALOG.to_owned(), "HTTPService");
        let reasons = judge_rows(&catalog, &all_active(), &unsupported, &mut |_| Ok(true)).unwrap();
        // one reason for the object, though its descriptor and its body are staged
        assert_eq!(reasons.len(), 1, "{reasons:?}");
        assert!(
            reasons[0].starts_with(&format!("{CATALOG}: the object is a HTTPService")),
            "{reasons:?}"
        );
        // a new object: no row in Config
        let mut new = delta();
        new.push(meta("99999999-1111-2222-3333-444444444444.0", "dd"));
        let reasons = judged(&new, true);
        assert_eq!(reasons.len(), 1);
        assert!(
            reasons[0].contains("no row of this name in Config"),
            "{reasons:?}"
        );
        // an owner the configuration does not list at the top level
        let mut nested = all_active();
        let ghost = "aaaaaaaa-1111-2222-3333-444444444444";
        for name in [ghost.to_owned(), format!("{ghost}.0")] {
            let row = meta(&name, "aa");
            nested.insert(row.key(), row);
        }
        let stage = vec![
            meta(&format!("{ghost}.0"), "bb"),
            meta("root", "aa"),
            meta("version", "aa"),
            meta("versions", "cc"),
        ];
        let reasons = judge_rows(&stage, &nested, &kinds(), &mut |_| Ok(true)).unwrap();
        assert!(
            reasons[0].contains("no proven existing owner"),
            "{reasons:?}"
        );
        // a body that is not the .0
        let mut other_suffix = delta();
        other_suffix[1] = meta(&format!("{MODULE}.1"), "bb");
        let mut active = all_active();
        let row = meta(&format!("{MODULE}.1"), "aa");
        active.insert(row.key(), row);
        let reasons = judge_rows(&other_suffix, &active, &kinds(), &mut |_| Ok(true)).unwrap();
        assert!(
            reasons[0].contains("unmeasured or structural body suffix"),
            "{reasons:?}"
        );
        // a name that is nothing an apply knows; the list of removals is judged apart
        let mut odd = delta();
        odd.push(meta("something-else", "ee"));
        odd.push(meta("deleted", "ff"));
        let reasons = judged(&odd, true);
        assert_eq!(reasons.len(), 1, "{reasons:?}");
        assert!(
            reasons[0].starts_with("something-else: not a service row"),
            "{reasons:?}"
        );
    }

    fn overlay() -> HashSet<String> {
        [
            "a627e390-8fad-4a95-afe6-674f54813188_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b",
            "a627e390-8fad-4a95-afe6-674f54813188_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b.0",
            "dynamicallyupdated",
            "versions_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }

    #[test]
    fn an_empty_list_of_removals_is_consumed_and_one_that_names_the_online_update_is_refused() {
        // what the platform's import writes to every stage (unmeasured for the platform's force: consumed)
        assert_eq!(
            judge_deleted_list(b"0", &HashSet::new()).unwrap(),
            "it is empty"
        );
        assert_eq!(
            judge_deleted_list(b"\xef\xbb\xbf0", &overlay()).unwrap(),
            "it is empty"
        );
        // the import stage of a target that carries an online update lists its rows: the platform's force
        // does more for it than this apply does (section 7 of the acceptance), so it is the exclusive apply's
        let listed = br#"3,"a627e390-8fad-4a95-afe6-674f54813188_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b",0,"DynamicallyUpdated",0,"versions_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b",0"#;
        let reason = judge_deleted_list(listed, &overlay()).unwrap_err();
        assert!(
            reason.starts_with("deleted: the stage lists 3 row(s) of the online update"),
            "{reason}"
        );
        assert!(reason.contains("apply it exclusively"), "{reason}");
    }

    #[test]
    fn a_list_that_removes_something_or_cannot_be_read_is_refused() {
        // an object's row, an attribute (another flag), a row of no update the database carries, a
        // removal list of an earlier dynamic update, a list that is no list
        for (text, needle) in [
            (
                &br#"1,"313d9858-3995-4a4c-b2b0-15d2350417b4.0",0"#[..],
                "313d9858-3995-4a4c-b2b0-15d2350417b4.0",
            ),
            (
                &br#"1,"a627e390-8fad-4a95-afe6-674f54813188_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b",1"#[..],
                "a627e390",
            ),
            (
                &br#"1,"a627e390-8fad-4a95-afe6-674f54813188_dynupdate_ffffffff-0c47-4fad-986a-f08f28287c1b",0"#[..],
                "ffffffff",
            ),
            (
                &br#"1,"deleted_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b",0"#[..],
                "deleted_dynupdate_",
            ),
        ] {
            let reason = judge_deleted_list(text, &overlay()).unwrap_err();
            assert!(reason.starts_with("deleted: the stage lists the removal of "), "{reason}");
            assert!(reason.contains(needle), "{reason}");
        }
        let unreadable = judge_deleted_list(b"not a list", &overlay()).unwrap_err();
        assert!(unreadable.contains("cannot read"), "{unreadable}");
        // a list that says one name and holds none
        assert!(judge_deleted_list(b"2,\"x\",0", &overlay()).is_err());
    }

    #[test]
    fn a_descriptor_that_changed_and_a_service_row_that_changed_are_refused() {
        // the descriptor differs in text
        let mut stage = delta();
        stage[0] = meta(MODULE, "dd");
        let reasons = judge_rows(&stage, &all_active(), &kinds(), &mut |name| {
            Ok(name != MODULE)
        })
        .unwrap();
        assert_eq!(reasons.len(), 1);
        assert!(
            reasons[0].contains("descriptor's text differs from the active one"),
            "{reasons:?}"
        );
        // the same bytes are asked about all the same (an alias may hold another text), and
        // root/version are asked only when their bytes differ
        let mut asked = Vec::new();
        judge_rows(&delta(), &all_active(), &kinds(), &mut |name| {
            asked.push(name.to_owned());
            Ok(true)
        })
        .unwrap();
        assert_eq!(asked, vec![MODULE.to_owned()]);
        let mut changed = delta();
        changed[2] = meta("root", "zz");
        let reasons = judge_rows(&changed, &all_active(), &kinds(), &mut |name| {
            Ok(name != "root")
        })
        .unwrap();
        assert_eq!(reasons.len(), 1);
        assert!(
            reasons[0].starts_with("root: the service row differs"),
            "{reasons:?}"
        );
    }

    #[test]
    fn a_generation_is_made_of_root_version_and_versions() {
        let stage = vec![meta(MODULE, "aa"), meta(&format!("{MODULE}.0"), "bb")];
        let reasons = judged(&stage, true);
        assert_eq!(reasons.len(), 3, "{reasons:?}");
        for service in ["root", "version", "versions"] {
            assert!(
                reasons
                    .iter()
                    .any(|reason| reason
                        .starts_with(&format!("{service}: the stage has no such row"))),
                "{service}: {reasons:?}"
            );
        }
        // a row of several parts is not a delta row
        let mut parts = delta();
        parts.push(RowMeta {
            part: 1,
            ..meta(&format!("{MODULE}.0"), "bb")
        });
        let reasons = judged(&parts, true);
        assert!(
            reasons
                .iter()
                .any(|reason| reason.contains("a row of several parts"))
        );
    }

    #[test]
    fn the_size_of_the_stage_rules_out_a_whole_tree() {
        let fits = StageSize {
            rows: 5,
            bytes: 350_000,
            largest: 344_213,
        };
        assert!(size_reasons(fits).is_empty());
        let tree = StageSize {
            rows: 9_521,
            bytes: 130 << 20,
            largest: 300_000,
        };
        let reasons = size_reasons(tree);
        assert_eq!(reasons.len(), 2, "{reasons:?}");
        assert!(reasons[0].contains("9521 rows"), "{reasons:?}");
        assert!(reasons[0].contains("at most 128"), "{reasons:?}");
        assert!(reasons[0].contains("#395"), "{reasons:?}");
        let edge = StageSize {
            rows: 128,
            bytes: MAX_PLAN_BYTES as i64,
            largest: MAX_ROW_BYTES as i64,
        };
        assert!(size_reasons(edge).is_empty());
        let over = StageSize {
            rows: 129,
            bytes: MAX_PLAN_BYTES as i64 + 1,
            largest: MAX_ROW_BYTES as i64 + 1,
        };
        assert_eq!(size_reasons(over).len(), 3);
    }

    #[test]
    fn a_small_complete_cohort_over_128_is_not_admitted_by_byte_budget_alone() {
        let reasons = size_reasons(StageSize {
            rows: 129,
            bytes: 4096,
            largest: 512,
        });
        assert_eq!(reasons.len(), 1);
        assert!(reasons[0].contains("129 rows"));
        assert!(reasons[0].contains("at most 128"));
        assert!(
            size_reasons(StageSize {
                rows: 128,
                bytes: 4096,
                largest: 512
            })
            .is_empty()
        );
    }

    #[test]
    fn the_overlay_is_not_capped_but_warned_about_past_fifty_generations() {
        assert_eq!(WARN_GENERATIONS, 50);
        assert_eq!(growth_warning(1), None);
        assert_eq!(growth_warning(50), None);
        let warning = growth_warning(51).unwrap();
        assert!(warning.contains("51 dynamic generations"), "{warning}");
        assert!(growth_warning(4000).is_some());
    }

    #[test]
    fn a_refusal_lists_the_first_reasons_and_counts_the_rest() {
        let few = vec!["a: x".to_owned(), "b: y".to_owned()];
        assert_eq!(refusal_reason(&few), "a: x; b: y");
        let many: Vec<String> = (0..11).map(|index| format!("r{index}: z")).collect();
        let text = refusal_reason(&many);
        assert!(text.ends_with("; и ещё 3"), "{text}");
        assert_eq!(text.matches("; ").count(), 8);
    }

    #[test]
    fn judged_image_refuses_each_physical_header_drift_before_publication() {
        for table in ["ConfigSave", "Config"] {
            for name in ["deleted", "root", "313d9858-3995-4a4c-b2b0-15d2350417b4.0"] {
                let row = MainStorageRow {
                    file_name: name.to_owned(),
                    part_no: 0,
                    creation: "2026-01-01 00:00:00.000".to_owned(),
                    modified: "2026-01-01 00:00:00.003".to_owned(),
                    attributes: 0,
                    data_size: 1,
                    binary_data: b"0".to_vec(),
                };
                let judged = RowMeta {
                    name: name.to_owned(),
                    part: 0,
                    data_size: 1,
                    byte_len: 1,
                    attributes: 0,
                    creation: row.creation.clone(),
                    modified: row.modified.clone(),
                    sha256: hex_lower(&row.sha256()),
                };
                require_judged_image(
                    table,
                    std::slice::from_ref(&judged),
                    std::slice::from_ref(&row),
                )
                .unwrap();
                for header in 0..3 {
                    let mut changed = row.clone();
                    match header {
                        0 => changed.attributes = 1,
                        1 => changed.creation = "2026-01-01 00:00:00.006".to_owned(),
                        2 => changed.modified = "2026-01-01 00:00:00.009".to_owned(),
                        _ => unreachable!(),
                    }
                    let error =
                        require_judged_image(table, std::slice::from_ref(&judged), &[changed])
                            .expect_err("header-only drift must refuse");
                    assert!(
                        error.to_string().contains("no publication was attempted"),
                        "{table}/{name}/{header}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_changed_deleted_list_cannot_replace_the_image_already_judged() {
        let bytes = b"0".to_vec();
        let row = MainStorageRow {
            file_name: "deleted".to_owned(),
            part_no: 0,
            creation: meta("deleted", "").creation,
            modified: meta("deleted", "").modified,
            attributes: 0,
            data_size: 1,
            binary_data: bytes,
        };
        let judged = RowMeta {
            data_size: 1,
            byte_len: 1,
            sha256: hex_lower(&row.sha256()),
            ..meta("deleted", "")
        };
        require_judged_image("ConfigSave", &[judged.clone()], std::slice::from_ref(&row)).unwrap();
        let changed = MainStorageRow {
            binary_data: br#"1,"DynamicallyUpdated",0"#.to_vec(),
            ..row.clone()
        };
        let error = require_judged_image("ConfigSave", std::slice::from_ref(&judged), &[changed])
            .unwrap_err();
        assert!(error.to_string().contains("no publication was attempted"));
        assert!(require_judged_image("ConfigSave", std::slice::from_ref(&judged), &[]).is_err());
        assert!(
            require_judged_image(
                "ConfigSave",
                &[judged.clone(), judged.clone()],
                &[row.clone(), row]
            )
            .is_err()
        );
    }

    #[test]
    fn descriptor_comparison_refuses_bytes_outside_the_judged_inventory() {
        let original = b"descriptor with original properties";
        let row = RowMeta {
            byte_len: original.len() as i64,
            ..meta(MODULE, &hex_lower(&Sha256::digest(original)))
        };
        assert!(judged_stage_digest(
            std::slice::from_ref(&row),
            MODULE,
            original
        ));
        assert!(!judged_stage_digest(
            &[row],
            MODULE,
            b"descriptor with changed properties"
        ));
        assert!(!judged_stage_digest(&[], MODULE, original));
    }

    #[test]
    fn a_transient_empty_deleted_blob_cannot_bypass_a_nonempty_inventory() {
        let dangerous = br#"1,"DynamicallyUpdated",0"#;
        let row = RowMeta {
            byte_len: dangerous.len() as i64,
            ..meta("deleted", &hex_lower(&Sha256::digest(dangerous)))
        };
        assert!(judged_stage_digest(
            std::slice::from_ref(&row),
            "deleted",
            dangerous
        ));
        assert!(!judged_stage_digest(&[row], "deleted", b"0"));
    }

    #[test]
    fn uncertain_commit_failure_keeps_recovery_evidence_without_claiming_rollback() {
        let message =
            uncertain_commit_context("lab", std::path::Path::new("recovery/run1"), "abc123");
        assert!(message.contains("may have committed"));
        assert!(message.contains("recovery/run1") || message.contains("recovery\\run1"));
        assert!(message.contains("abc123"));
        assert!(message.contains("before any retry"));
        assert!(!message.contains("database is unchanged"));
    }

    #[test]
    fn the_engine_limits_are_the_refusal_of_a_stage_and_the_rest_a_failure() {
        let limit = engine_error(MainActivationError::Limit(
            "129 staged rows exceeds 128".into(),
        ));
        assert!(limit.downcast_ref::<NeedsNativeApply>().is_some());
        let target = engine_error(MainActivationError::StructuralTarget("x".into()));
        assert!(target.downcast_ref::<NeedsNativeApply>().is_some());
        let versions = engine_error(MainActivationError::Versions("reused".into()));
        assert!(versions.downcast_ref::<NeedsNativeApply>().is_none());
    }
}
