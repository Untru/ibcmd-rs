//! The own `config apply`: move what `infobase config import` staged in
//! `ConfigSave` into `Config` the way the platform's exclusive
//! `config apply --dynamic=disable` does, without the platform.
//!
//! What the native apply does, measured on 8.3.27.2214 (see
//! `docs/apply/own-apply.md`): it copies every staged row under a `.new` name
//! in autocommit statements, records a `commit` marker, renames the rows over
//! their names, folds an earlier dynamic generation into the ordinary rows,
//! resets the change registrations of the staged objects, gives
//! `Files.MobileVersions.dat` a new head GUID, rebuilds derived caches and
//! empties `ConfigSave`. Its crash safety is that marker protocol.
//!
//! This module reaches the same end state for a configuration that needs no
//! restructuring in **one serializable transaction** whose data never leaves
//! the server: a crash or a failed postcondition leaves the database as it was
//! (nothing for `config repair` to finish). The parts:
//!
//! - [`plan`]: reads only row metadata and server-computed fingerprints,
//!   refuses what needs a restructuring ([`gate`]), and renders the script
//!   ([`sqlgen`]);
//! - the recovery artifact ([`recovery`]) keeps the rows the apply overwrites;
//! - [`apply_staged_configuration`] runs the script and checks the result.

pub mod check_gate;
pub mod dynamic;
mod dynamic_metadata;
mod dynamic_overlay;
pub mod dynamic_platform85;
pub mod errors;
pub mod gate;
pub mod model;
pub mod objects;
mod params_marker;
pub mod recovery;
pub mod registrations;
pub mod removals;
pub mod si;
pub mod sqlgen;
pub mod synonyms;
pub mod versions;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::mssql_platform_profile::{MssqlNativePlatformProfile, OwnRasProcess, session_exemption};
use crate::sql::{ScriptVariables, SqlClient, SqlExec, SqlParam, SqlValue};

use check_gate::ApplyCheckGate;
pub use errors::{
    BackupRequired, DynamicUnsupported, ExclusiveAccessRefused, ExclusiveAccessUnprovable,
    NativeCommand, NeedsNativeApply,
};
use gate::{ConservativeGate, GateInput, GateVerdict, StructuralGate, StructurePhase};
use model::{RowMeta, hex_lower, quote_ident, quote_string};
use sqlgen::{
    AppendedFile, FilesRewrite, Fingerprint, NewRegistration, NodeLiteral, ParamsRewrite,
    ScriptInputs, fingerprint_select, render_apply_script, replaced_source, special_config_source,
    special_params_source, staged_source,
};

/// How the apply learns that nobody else is connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Exclusivity {
    /// No other user session on the database, as SQL Server sees it (needs
    /// `VIEW SERVER STATE`). Refuses when a working process still holds a
    /// connection.
    SqlSessions,
    /// The caller has proved it another way (for instance with `rac session
    /// list`); the script does not look.
    Assumed,
    /// The dynamic apply ([`dynamic`]) publishes while sessions are connected and asks for no
    /// exclusive access.
    NotRequired,
}

/// How the stage was applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyMode {
    /// The rows replace the active ones in place, with the database to itself.
    Exclusive,
    /// The rows are published as a dynamic (online) generation beside the active ones
    /// (`--dynamic=force`, [`dynamic`]).
    Dynamic,
}

/// Which overwritten rows the recovery artifact keeps the bytes of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryBlobs {
    /// Rows the apply changes (bytes differ from the staged row).
    Changed,
    /// Only the manifest of hashes.
    None,
}

/// A class of restructuring the apply lets through instead of refusing it
/// (`--allow-restructure`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AllowRestructure {
    /// S1 of the restructure track (#391): attributes, tabular sections, string widening, the
    /// index flag and plain new catalogs and documents, rebuilt inside the apply's transaction.
    S1,
}

/// What the operator says about a copy to go back to. A restructuring drops the
/// old tables inside the transaction; the recovery artifact keeps the `Config`
/// rows and the cache rows only, so the way back is a SQL Server backup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupPolicy {
    /// Nothing said: a structural apply is refused.
    None,
    /// `--i-have-a-backup`: the operator has one.
    Acknowledged,
    /// `--recovery-backup <file>`: the apply takes `BACKUP DATABASE ... WITH COPY_ONLY`
    /// to this file (a path the SQL Server service can write) before the transaction.
    File(PathBuf),
}

/// Which structural gate judges the stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GateChoice {
    /// The restructure check of track rcheck (`apply_check::check_staged`):
    /// the default. It refuses on a restructuring and on anything it cannot
    /// place.
    ApplyCheck,
    /// The conservative rule of [`gate::ConservativeGate`]: modules, forms,
    /// templates, pictures and help pages, nothing else.
    Conservative,
}

#[derive(Debug, Clone)]
pub struct ConfigApplyOptions {
    pub database: String,
    pub platform_profile: MssqlNativePlatformProfile,
    /// Plan, check and render, but write nothing (not even the recovery
    /// artifact).
    pub dry_run: bool,
    /// Run the whole script and roll it back: proves the SQL and the
    /// postconditions on this very database, changes nothing.
    pub rehearse: bool,
    pub exclusivity: Exclusivity,
    pub recovery_dir: Option<PathBuf>,
    pub recovery_blobs: RecoveryBlobs,
    /// With no `recovery_dir`: how many artifacts of this database the default
    /// directory keeps after a successful run (0: all of them).
    pub recovery_keep: usize,
    pub script_output: Option<PathBuf>,
    /// The gate; [`GateChoice::ApplyCheck`] unless asked otherwise.
    pub gate: GateChoice,
    /// A class of restructuring to let through; it replaces the gate by the
    /// restructure track's, which starts from the same checks.
    pub allow_restructure: Option<AllowRestructure>,
    /// The backup a structural apply needs (see [`BackupPolicy`]).
    pub backup: BackupPolicy,
    /// See [`ConservativeGate::admit_unverified_roles`]; only with
    /// [`GateChoice::Conservative`].
    pub admit_unverified_roles: bool,
    /// The most rows and bytes of tables a restructuring may rebuild in the transaction (S1-J); the
    /// measured default unless a flag or the settings chain says otherwise.
    pub restructure_limit: crate::restructure::size_guard::LimitSetting,
    /// The worker processes whose idle `1CV83 Server` SQL sessions the exclusivity check leaves out: the sessions the
    /// tool's own RAS verification made the cluster open (#409 F-3; the old activation commands, #408). A session of
    /// that program that runs or holds a transaction, and every session of another program or process, still counts.
    /// Empty (the default): every session but this process's counts.
    pub own_ras_processes: Vec<OwnRasProcess>,
}

/// The XML dialect the restructure check decodes descriptors with.
fn xml_version_of(profile: MssqlNativePlatformProfile) -> Option<&'static str> {
    match profile {
        MssqlNativePlatformProfile::Platform8_3_27_1989
        | MssqlNativePlatformProfile::Platform8_3_27_2214 => Some("2.20"),
        MssqlNativePlatformProfile::Platform8_5_1_1150 => Some("2.21"),
    }
}

/// The ONE place that picks the structural gate. Callers that need another
/// (a test, a caller with its own check) use [`plan_with_gate`] and
/// [`apply_with_gate`].
pub fn structural_gate<'a>(
    sql: &'a SqlExec,
    options: &ConfigApplyOptions,
) -> Result<Box<dyn StructuralGate + 'a>> {
    if let Some(kind) = options.allow_restructure {
        return restructure_gate(kind, sql, options);
    }
    Ok(match options.gate {
        GateChoice::ApplyCheck => Box::new(ApplyCheckGate::new(
            sql,
            xml_version_of(options.platform_profile),
        )),
        GateChoice::Conservative => Box::new(options.conservative_gate()),
    })
}

/// The gate that lets a class of restructuring through. The restructure track's
/// S1 gate (`restructure::s1::S1Gate`, #391) starts from the same checks, prepares
/// the structure phase and hands it over through [`StructuralGate::take_structure`];
/// it is built here, in the one place that builds gates.
///
/// Behind the default gate: the restructure check judges the stage first and what it passes (a
/// harmless change of a descriptor, a body of any role it knows) goes as it always did. Only what the
/// check refuses reaches the S1 gate, whose conservative rule alone would refuse every changed
/// descriptor, so a stage that needs no restructuring is not made harder by asking for S1.
fn restructure_gate<'a>(
    kind: AllowRestructure,
    sql: &'a SqlExec,
    options: &ConfigApplyOptions,
) -> Result<Box<dyn StructuralGate + 'a>> {
    match kind {
        AllowRestructure::S1 => Ok(Box::new(gate::FirstThen::new(
            Box::new(ApplyCheckGate::new(
                sql,
                xml_version_of(options.platform_profile),
            )),
            Box::new(
                crate::restructure::s1::S1Gate::new(
                    sql,
                    options.conservative_gate(),
                    crate::restructure::plan::PlanOptions::default(),
                )
                .xml_version(xml_version_of(options.platform_profile))
                .size_limit(options.restructure_limit.clone()),
            ),
        ))),
    }
}

impl ConfigApplyOptions {
    pub fn conservative_gate(&self) -> ConservativeGate {
        ConservativeGate {
            admit_unverified_roles: self.admit_unverified_roles,
        }
    }

    pub fn new(database: impl Into<String>, platform_profile: MssqlNativePlatformProfile) -> Self {
        Self {
            database: database.into(),
            platform_profile,
            dry_run: false,
            rehearse: false,
            exclusivity: Exclusivity::SqlSessions,
            recovery_dir: None,
            recovery_blobs: RecoveryBlobs::Changed,
            recovery_keep: recovery::KEEP_DEFAULT,
            script_output: None,
            gate: GateChoice::ApplyCheck,
            allow_restructure: None,
            backup: BackupPolicy::None,
            admit_unverified_roles: false,
            restructure_limit: Default::default(),
            own_ras_processes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ApplyTimings {
    pub storage_check_ms: u128,
    pub inventory_ms: u128,
    pub gate_ms: u128,
    /// The changed synonyms: the descriptors read and compared, the registry rewritten.
    #[serde(skip_serializing_if = "is_zero_ms")]
    pub synonyms_ms: u128,
    pub fingerprints_ms: u128,
    pub recovery_ms: u128,
    /// The copy-only backup a structural apply takes before its transaction.
    pub backup_ms: u128,
    pub sql_ms: u128,
    pub total_ms: u128,
}

/// The way back the operator gave a structural apply, as the report and the
/// recovery artifact name it.
#[derive(Debug, Clone, Serialize)]
pub struct BackupRecord {
    /// `file`: the apply took the backup; `acknowledged`: the operator says they
    /// have one.
    pub kind: &'static str,
    pub path: Option<String>,
    /// How long the backup took (`file` only).
    pub seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StageSummary {
    pub rows: usize,
    pub bytes: i64,
    pub descriptors: usize,
    pub bodies: usize,
    pub service_rows: usize,
    /// Staged rows with no `Config` row of the same name.
    pub new_rows: usize,
    /// Staged rows whose bytes equal the active row's.
    pub identical_rows: usize,
    pub replaced_rows: usize,
    pub replaced_parts_dropped: usize,
    /// Staged rows consumed without being moved into `Config` (an empty or
    /// dynamic-only `deleted` list).
    pub consumed_rows: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DynamicSummary {
    pub generations: Vec<String>,
    pub alias_rows: usize,
}

/// What a dynamic apply published ([`dynamic`]).
#[derive(Debug, Clone, Serialize)]
pub struct DynamicPublication {
    /// The generation this apply made (the one the `versions` row it staged names).
    pub generation: String,
    /// The generation that was active before it.
    pub previous_generation: String,
    /// The history after it, oldest first: the `DynamicallyUpdated` markers list it. The next
    /// exclusive apply folds all of it into the ordinary rows.
    pub history: Vec<String>,
    /// The rows this apply wrote beside the active ones (`<name>_dynupdate_<generation>`).
    pub alias_rows: Vec<String>,
    /// The service rows replaced in place (`root`, `version`).
    pub replaced_in_place: Vec<String>,
    /// The generations past which the report warns that the overlay is growing ([`dynamic::WARN_GENERATIONS`]).
    pub warn_after_generations: usize,
}

/// The change registrations of the exchange-plan nodes (docs/apply/own-apply.md, "Exchange plans").
#[derive(Debug, Clone, Serialize)]
pub struct RegistrationSummary {
    /// Nodes of the plans that register changes, without the plans' own nodes; a node with an initial
    /// image has no rows until an apply registers a change for it.
    pub nodes: usize,
    /// Objects the stage changes that the register knows.
    pub changed_objects: usize,
    /// `_ConfigChngR` rows inserted for nodes that had none for a changed object, and the file rows
    /// listed for them.
    pub rows_added: i64,
    pub file_rows_added: i64,
    /// Objects a `deleted` list names rows of: their rows are reset as if their rows were staged.
    pub objects_of_dropped_rows: usize,
}

/// What the staged new rows (forms, templates, body rows) add to the apply.
#[derive(Debug, Clone, Serialize)]
pub struct NewObjectsSummary {
    pub objects: Vec<objects::NewObject>,
    pub appended_bodies: Vec<objects::NewBody>,
    /// Nodes each new object is registered for.
    pub registration_nodes: usize,
    pub search_info_records: usize,
}

/// The forms and templates a stage's `deleted` list removes (docs/apply/own-apply.md, "Removals").
#[derive(Debug, Clone, Serialize)]
pub struct RemovalsSummary {
    pub objects: Vec<removals::RemovedObject>,
    /// `Config` rows deleted.
    pub rows_deleted: usize,
    /// Records taken out of the main search information, and entries out of the properties row.
    pub search_info_records: usize,
    pub property_entries: usize,
}

fn is_zero(count: &usize) -> bool {
    *count == 0
}

fn is_zero_ms(millis: &u128) -> bool {
    *millis == 0
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigApplyReport {
    pub schema_version: u32,
    /// `exclusive` (the rows replaced in place) or `dynamic` (a generation published beside them).
    pub mode: ApplyMode,
    pub database: String,
    pub platform_profile: String,
    pub storage_schema_sha256: String,
    pub dry_run: bool,
    pub rehearsal: bool,
    /// The transaction committed.
    pub executed: bool,
    /// Nothing was staged: nothing to apply.
    pub nothing_to_apply: bool,
    pub exclusivity: Exclusivity,
    pub active_generation: Option<String>,
    pub new_generation: Option<String>,
    pub stage: Option<StageSummary>,
    /// The overlay an exclusive apply folded.
    pub dynamic: Option<DynamicSummary>,
    /// The generation a dynamic apply published.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published: Option<DynamicPublication>,
    pub gate: Option<GateVerdict>,
    /// The restructuring the gate let through and the script runs in its transaction.
    pub structure: Option<StructurePhase>,
    /// The backup taken, or the operator's word that they have one.
    pub backup: Option<BackupRecord>,
    /// The change registrations for the exchange-plan nodes.
    pub registrations: Option<RegistrationSummary>,
    pub new_objects: Option<NewObjectsSummary>,
    pub removals: Option<RemovalsSummary>,
    /// Records of the object registry that took a changed synonym.
    #[serde(skip_serializing_if = "is_zero")]
    pub synonym_records: usize,
    pub tables_touched: Vec<String>,
    /// Derived state the native apply also rewrites and this one does not
    /// (or only in part): the honest gaps.
    pub not_written: Vec<String>,
    pub warnings: Vec<String>,
    pub script_sha256: Option<String>,
    pub script_path: Option<PathBuf>,
    pub recovery_dir: Option<PathBuf>,
    pub recovery_token: Option<String>,
    pub timings: ApplyTimings,
}

/// Why the own apply stopped before writing, for callers that map it to an
/// exit code.
#[derive(Debug)]
pub struct StructuralRefusal {
    pub verdict: GateVerdict,
}

impl std::fmt::Display for StructuralRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // a gate with words of its own (the restructure check) speaks first
        if let Some(refusal) = &self.verdict.refusal {
            return write!(formatter, "{refusal}");
        }
        write!(
            formatter,
            "the staged configuration needs the platform's own config apply (a restructuring or a change this apply does not do): {} blocker(s)",
            self.verdict.blockers.len() + self.verdict.blockers_omitted
        )?;
        for blocker in self.verdict.blockers.iter().take(5) {
            write!(formatter, "\n  {}: {}", blocker.row, blocker.reason)?;
        }
        Ok(())
    }
}

impl std::error::Error for StructuralRefusal {}

fn ms(since: Instant) -> u128 {
    since.elapsed().as_millis()
}

fn require_client(sql: &SqlExec) -> Result<&dyn SqlClient> {
    sql.client().ok_or_else(|| {
        anyhow!("the own config apply runs on the built-in SQL client; drop --sqlcmd")
    })
}

const ROW_COLUMNS: &str = "FileName, PartNo, CONVERT(bigint, DataSize), DATALENGTH(BinaryData), CONVERT(int, Attributes), \
     CONVERT(varchar(27), Creation, 121), CONVERT(varchar(27), Modified, 121), CONVERT(varchar(64), HASHBYTES('SHA2_256', BinaryData), 2)";

fn read_row_metas(client: &dyn SqlClient, query: &str) -> Result<Vec<RowMeta>> {
    let mut rows = Vec::new();
    client.read_rows(query, &[], &mut |row| {
        rows.push(RowMeta {
            name: row.text(0)?.to_owned(),
            part: i32::try_from(row.i64(1)?).context("PartNo")?,
            data_size: row.i64(2)?,
            byte_len: row.i64(3)?,
            attributes: i16::try_from(row.i64(4)?).context("Attributes")?,
            creation: row.text(5)?.to_owned(),
            modified: row.text(6)?.to_owned(),
            sha256: row.text(7)?.to_ascii_lowercase(),
        });
        Ok(())
    })?;
    Ok(rows)
}

fn read_fingerprint(client: &dyn SqlClient, source: &str) -> Result<Fingerprint> {
    let rows = client.query_rows(&fingerprint_select(source), &[])?;
    let row = rows
        .first()
        .ok_or_else(|| anyhow!("a fingerprint query returned no row"))?;
    let values = (0..5)
        .map(|index| row.i64(index))
        .collect::<Result<Vec<_>>>()?;
    Fingerprint::from_values(&values)
}

fn read_blob(
    client: &dyn SqlClient,
    database: &str,
    table: &str,
    name: &str,
) -> Result<Option<Vec<u8>>> {
    let db = quote_ident(database)?;
    let value = client.query_scalar(
        &format!("SELECT BinaryData FROM {db}.dbo.{table} WHERE FileName = @P1 AND PartNo = 0"),
        &[SqlParam::Text(name)],
    )?;
    match value {
        None => Ok(None),
        Some(SqlValue::Binary(bytes)) => Ok(Some(bytes)),
        Some(other) => bail!("{table}.{name} is not binary: {other:?}"),
    }
}

/// No row of an unfinished operation (`commit`, `dynamicCommit`, `dbStruFinal`, `convertPhase`,
/// `erase_save`, `deleted`, `*.new`) in `Config`, and none but the stage's own list of removals in
/// `ConfigSave`: what is left over is `config repair`'s to finish. `db` is the quoted database name.
pub(crate) fn require_no_unfinished_operation(client: &dyn SqlClient, db: &str) -> Result<()> {
    let unfinished = sqlgen::UNFINISHED_NAMES
        .iter()
        .map(|name| format!("N'{}'", quote_string(name)))
        .collect::<Vec<_>>()
        .join(", ");
    // In `ConfigSave` a row `deleted` is not a marker of an unfinished
    // operation but the stage's own list of removals (the platform's import
    // writes one to every stage): it is judged below.
    let unfinished_in_save = sqlgen::UNFINISHED_NAMES
        .iter()
        .filter(|name| **name != "deleted")
        .map(|name| format!("N'{}'", quote_string(name)))
        .collect::<Vec<_>>()
        .join(", ");
    let left_over = scalar_i64(
        client,
        &format!(
            "SELECT (SELECT COUNT_BIG(*) FROM {db}.dbo.Config WHERE FileName IN ({unfinished}) OR FileName LIKE N'%.new') + (SELECT COUNT_BIG(*) FROM {db}.dbo.ConfigSave WHERE FileName IN ({unfinished_in_save}) OR FileName LIKE N'%.new')"
        ),
    )?;
    if left_over != 0 {
        return Err(NeedsNativeApply::repair(format!(
            "{left_over} row(s) of an unfinished operation (commit / dynamicCommit / dbStruFinal / convertPhase / erase_save / deleted / *.new) are recorded in Config or ConfigSave; run the native `ibcmd infobase config repair` first"
        ))
        .into());
    }
    Ok(())
}

/// No dynamic-update overlay in `Params` and a schema storage at rest. `db` is the quoted database name.
pub(crate) fn require_settled_storage(client: &dyn SqlClient, db: &str) -> Result<()> {
    // An overlay of a dynamic update in Params (a `.si` row under an alias name)
    // is folded by the native apply's `.si` promotion; this apply only folds
    // `Config`, so it leaves such a database to the native one.
    let params_overlays = scalar_i64(
        client,
        &format!(
            "SELECT COUNT_BIG(*) FROM {db}.dbo.Params WHERE FileName LIKE {}",
            sqlgen::ALIAS_PATTERN
        ),
    )?;
    if params_overlays != 0 {
        return Err(NeedsNativeApply::apply(format!(
            "Params holds {params_overlays} dynamic-update overlay row(s) (names with _dynupdate_): run the native `ibcmd infobase config apply`"
        ))
        .into());
    }
    require_schema_settled(client, db)
}

/// The schema must be settled even when the bounded dynamic path accepts measured Params aliases.
pub(crate) fn require_schema_settled(client: &dyn SqlClient, db: &str) -> Result<()> {
    // The schema storage of a settled infobase is at Status 100; the native apply
    // walks it through 200, 400 and 500 and back, so any other value is an
    // interrupted operation.
    let unsettled = scalar_i64(
        client,
        &format!("SELECT COUNT_BIG(*) FROM {db}.dbo.SchemaStorage WHERE Status <> 100"),
    )?;
    if unsettled != 0 {
        return Err(NeedsNativeApply::repair(format!(
            "SchemaStorage is not settled ({unsettled} row(s) with Status other than 100): an interrupted restructuring or apply; run the native `ibcmd infobase config repair` first"
        ))
        .into());
    }
    Ok(())
}

/// `Files.MobileVersions.dat`: every apply, exclusive or dynamic, puts a fresh GUID at the head of the
/// ring (the platform does, so mobile clients see a new version). The rewrite the script makes and the
/// bytes it replaces (for the recovery artifact). A database without a `_YearOffset` table gets
/// unshifted timestamps, and one without the file no rewrite; both are said in the report.
pub(crate) fn plan_mobile_versions(
    client: &dyn SqlClient,
    database: &str,
    report: &mut ConfigApplyReport,
) -> Result<(Vec<FilesRewrite>, Option<Vec<u8>>)> {
    let db = quote_ident(database)?;
    let has_year_offset = scalar_i64(
        client,
        &format!(
            "SELECT CASE WHEN OBJECT_ID(N'{db}.dbo._YearOffset', N'U') IS NULL THEN 0 ELSE 1 END"
        ),
    )? == 1;
    if !has_year_offset {
        report.warnings.push(
            "the database has no _YearOffset table; Files rows get unshifted timestamps".to_owned(),
        );
    }
    let mut files_rewrites = Vec::new();
    let mut mobile_before = None;
    let mobile_metas = read_row_metas(
        client,
        &format!(
            "SELECT {ROW_COLUMNS} FROM {db}.dbo.Files WHERE FileName = N'MobileVersions.dat' ORDER BY PartNo"
        ),
    )?;
    match mobile_metas.as_slice() {
        [] => report.warnings.push(
            "Files.MobileVersions.dat is absent; mobile clients get no new version".to_owned(),
        ),
        [meta] if meta.part == 0 => {
            let current = read_blob(client, database, "Files", "MobileVersions.dat")?
                .ok_or_else(|| anyhow!("Files.MobileVersions.dat vanished"))?;
            let next = versions::mobile_versions_prepend(&current, Uuid::new_v4())?;
            files_rewrites.push(FilesRewrite {
                file_name: "MobileVersions.dat".to_owned(),
                old_data_size: meta.data_size,
                old_sha256_hex: meta.sha256.to_ascii_uppercase(),
                new_bytes: next,
            });
            mobile_before = Some(current);
        }
        _ => bail!("Files.MobileVersions.dat has several parts"),
    }
    Ok((files_rewrites, mobile_before))
}

/// The exchange-plan nodes as the script's literals.
pub(crate) fn node_literals(nodes: &[objects::RegistrationNode]) -> Vec<NodeLiteral> {
    nodes
        .iter()
        .map(|node| NodeLiteral {
            plan: node.plan,
            type_hex: node.type_ref.clone(),
            reference_hex: node.reference.clone(),
        })
        .collect()
}

/// The list in a stage's `deleted` row: `<BOM><count>,"<row name>",<flag>,...`
/// (`0` when nothing is removed), as (name, flag) pairs. `None` when the text
/// is no such list.
fn parse_removals(plain: &[u8]) -> Option<Vec<(String, String)>> {
    let text = String::from_utf8_lossy(versions::strip_bom(plain)).into_owned();
    let mut tokens = text.trim().split(',').map(str::trim);
    let count: usize = tokens.next()?.parse().ok()?;
    let rest: Vec<&str> = tokens.collect();
    if rest.len() != count * 2 {
        return None;
    }
    let mut entries = Vec::with_capacity(count);
    for pair in rest.chunks(2) {
        let name = pair[0].strip_prefix('"')?.strip_suffix('"')?;
        entries.push((name.to_owned(), pair[1].to_owned()));
    }
    Some(entries)
}

/// A row of an online (dynamic) update: an alias, `versions_dynupdate_<g>`,
/// or the marker.
fn is_dynamic_update_row(name: &str) -> bool {
    name == "DynamicallyUpdated" || name.contains("_dynupdate_")
}

/// The list, split: the rows of a dynamic update that `Config` carries and the list names (flag 0), as
/// listed, and the names of other `Config` rows (flag 0). An entry with another flag names an element
/// that has no row of its own (an attribute); it is in neither.
fn split_removals(
    entries: &[(String, String)],
    overlay_rows: &std::collections::HashSet<String>,
) -> (Vec<String>, Vec<String>) {
    let mut overlay = Vec::new();
    let mut objects = Vec::new();
    for (name, flag) in entries {
        if flag != "0" {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        if is_dynamic_update_row(name) {
            if !lower.starts_with("deleted_dynupdate_") && overlay_rows.contains(&lower) {
                overlay.push(name.clone());
            }
        } else {
            objects.push(name.clone());
        }
    }
    (overlay, objects)
}

/// What is done with a `deleted` list.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ListAnswer {
    /// Every name is accounted for: the rows of the dynamic update (all of them, or none), the rows of
    /// the removed forms and templates. The list is consumed.
    Consumed,
    /// What is left (the attributes the stage removes) is for the gate to judge; consumed only if it
    /// hands over a phase that answers for it.
    Judged,
    /// The list is refused as a whole.
    Refused(String),
}

/// The apply's answer to a `deleted` list: `accounted` are the lower-cased names of the removed
/// objects' rows the analysis takes (`blockers` are its reasons against the rest), `gate_judges` says
/// whether the gate takes the names that remain. A list that names some but not all of the rows of a
/// dynamic update is refused (the native apply's answer is not measured), and so is any list with a
/// reason against it: the list is answered whole or not at all.
fn answer_removals(
    entries: &[(String, String)],
    overlay_rows: &std::collections::HashSet<String>,
    accounted: &std::collections::HashSet<String>,
    blockers: &[gate::GateBlocker],
    gate_judges: bool,
) -> ListAnswer {
    let (overlay_listed, _) = split_removals(entries, overlay_rows);
    let named: std::collections::HashSet<String> = overlay_listed
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect();
    if !named.is_empty() && named != *overlay_rows {
        return ListAnswer::Refused(format!(
            "it names {} of the {} rows of the dynamic update that Config carries, and the native apply's answer to a partial list is not measured",
            named.len(),
            overlay_rows.len()
        ));
    }
    if !blockers.is_empty() {
        return ListAnswer::Refused(format!(
            "{} reason(s) against removing the objects it names",
            blockers.len()
        ));
    }
    let remaining: Vec<&str> = entries
        .iter()
        .filter(|(name, flag)| {
            let lower = name.to_ascii_lowercase();
            !(flag == "0" && (accounted.contains(&lower) || named.contains(&lower)))
        })
        .map(|(name, _)| name.as_str())
        .collect();
    match remaining.first() {
        None => ListAnswer::Consumed,
        Some(_) if gate_judges => ListAnswer::Judged,
        Some(first) => ListAnswer::Refused(format!(
            "{} name(s) not accounted for, the first {first}",
            remaining.len()
        )),
    }
}

/// What a stage's `deleted` row lists, for the message that names it.
fn describe_removals(plain: &[u8]) -> String {
    match parse_removals(plain) {
        None => "a list this apply cannot read".to_owned(),
        Some(entries) if entries.is_empty() => {
            "it is empty (the platform's own import writes one to every stage)".to_owned()
        }
        Some(entries) => {
            let count = entries.len();
            let overlay = entries
                .iter()
                .filter(|(name, _)| is_dynamic_update_row(name))
                .count();
            let first = entries
                .iter()
                .find(|(name, _)| !is_dynamic_update_row(name))
                .map(|(name, _)| format!(", the first of them {name}"))
                .unwrap_or_default();
            format!(
                "{count} row name(s), {overlay} of them rows of a dynamic update and {} others{first}",
                count - overlay
            )
        }
    }
}

/// The cache rows a restructuring rewrites joined with the search information a
/// new form or template rewrites. Both edit the same stored bytes (the object
/// registry `1a621f0f`, `siVersions`); chaining the two edits is not built, so a
/// row both want is a refusal: the stage is applied in two steps.
fn merge_params_rewrites(
    mut own: Vec<ParamsRewrite>,
    phase: &[ParamsRewrite],
) -> Result<Vec<ParamsRewrite>> {
    if let Some(clash) = phase
        .iter()
        .find(|rewrite| own.iter().any(|other| other.file_name == rewrite.file_name))
    {
        bail!(
            "the restructuring and the new forms or templates of the stage both rewrite Params.{}: apply them in two steps",
            clash.file_name
        );
    }
    own.extend(phase.iter().cloned());
    Ok(own)
}

/// A structural apply that writes needs the operator's word about a way back:
/// the old tables are dropped in the transaction. A dry run and a rehearsal
/// write nothing that stays.
fn require_backup(policy: &BackupPolicy, structural: bool, writes: bool) -> Result<()> {
    if structural && writes && *policy == BackupPolicy::None {
        return Err(BackupRequired.into());
    }
    Ok(())
}

fn scalar_i64(client: &dyn SqlClient, query: &str) -> Result<i64> {
    match client.query_scalar(query, &[])? {
        Some(SqlValue::Int(value)) => Ok(value),
        other => bail!("expected an integer from {query}, got {other:?}"),
    }
}

/// A session other than ours on the database, for the message that refuses
/// the apply.
#[derive(Debug, Clone, Serialize)]
pub struct OtherSession {
    pub session_id: i64,
    pub login: String,
    pub host: String,
    pub program: String,
    pub status: String,
    pub last_request_end: String,
}

/// The query of [`other_sessions`]: `@P1` the database, `@P2` this process's id. The idle `1CV83 Server` sessions
/// of `own_ras_processes` are left out ([`session_exemption`]).
fn other_sessions_query(own_ras_processes: &[OwnRasProcess]) -> String {
    format!(
        "SELECT session_id, ISNULL(login_name, N''), ISNULL(host_name, N''), ISNULL(program_name, N''), status, ISNULL(CONVERT(varchar(27), last_request_end_time, 121), N'') \
         FROM sys.dm_exec_sessions WHERE is_user_process = 1 AND database_id = DB_ID(@P1) AND ISNULL(host_process_id, -1) <> @P2{} ORDER BY session_id",
        session_exemption(own_ras_processes)
    )
}

pub fn other_sessions(
    client: &dyn SqlClient,
    database: &str,
    own_ras_processes: &[OwnRasProcess],
) -> Result<Vec<OtherSession>> {
    let permitted = scalar_i64(
        client,
        "SELECT CONVERT(bigint, HAS_PERMS_BY_NAME(NULL, NULL, N'VIEW SERVER STATE'))",
    )?;
    if permitted != 1 {
        return Err(ExclusiveAccessUnprovable {
            reason: "exclusive access cannot be proven: the login lacks VIEW SERVER STATE, so other sessions are invisible".to_owned(),
        }
        .into());
    }
    let pid = i64::from(std::process::id());
    let mut sessions = Vec::new();
    client.read_rows(
        &other_sessions_query(own_ras_processes),
        &[SqlParam::Text(database), SqlParam::I64(pid)],
        &mut |row| {
            sessions.push(OtherSession {
                session_id: row.i64(0)?,
                login: row.text(1)?.to_owned(),
                host: row.text(2)?.to_owned(),
                program: row.text(3)?.to_owned(),
                status: row.text(4)?.to_owned(),
                last_request_end: row.text(5)?.to_owned(),
            });
            Ok(())
        },
    )?;
    Ok(sessions)
}

/// The report of a plan that has looked at nothing yet.
pub(crate) fn blank_report(
    options: &ConfigApplyOptions,
    storage: &crate::mssql_platform_profile::MssqlStorageVerification,
    mode: ApplyMode,
) -> ConfigApplyReport {
    ConfigApplyReport {
        schema_version: 1,
        mode,
        database: options.database.clone(),
        platform_profile: storage.claimed_platform_profile.clone(),
        storage_schema_sha256: storage.storage_schema_sha256.clone(),
        dry_run: options.dry_run,
        rehearsal: options.rehearse,
        executed: false,
        nothing_to_apply: false,
        exclusivity: if mode == ApplyMode::Dynamic {
            Exclusivity::NotRequired
        } else {
            options.exclusivity
        },
        active_generation: None,
        new_generation: None,
        stage: None,
        dynamic: None,
        published: None,
        gate: None,
        structure: None,
        backup: None,
        registrations: None,
        new_objects: None,
        removals: None,
        synonym_records: 0,
        tables_touched: Vec::new(),
        not_written: Vec::new(),
        warnings: Vec::new(),
        script_sha256: None,
        script_path: None,
        recovery_dir: None,
        recovery_token: None,
        timings: ApplyTimings::default(),
    }
}

/// The plan and the script, built from a read-only look at the database.
pub struct ConfigApplyPlan {
    pub report: ConfigApplyReport,
    pub script: Option<String>,
    inputs: Option<ScriptInputs>,
    staged: Vec<RowMeta>,
    replaced: Vec<RowMeta>,
    mobile_versions_before: Option<Vec<u8>>,
    new: objects::NewObjects,
    registration: registrations::RegistrationPlan,
    removals: removals::Removals,
}

pub fn plan(sql: &SqlExec, options: &ConfigApplyOptions) -> Result<ConfigApplyPlan> {
    let gate = structural_gate(sql, options)?;
    plan_with_gate(sql, options, gate.as_ref())
}

/// [`plan`] with another structural gate.
pub fn plan_with_gate(
    sql: &SqlExec,
    options: &ConfigApplyOptions,
    structural_gate: &dyn StructuralGate,
) -> Result<ConfigApplyPlan> {
    let total = Instant::now();
    let client = require_client(sql)?;
    let database = options.database.as_str();
    let db = quote_ident(database)?;
    let mut timings = ApplyTimings::default();

    let started = Instant::now();
    let storage = crate::mssql_platform_profile::verify_mssql_storage_profile(
        options.platform_profile,
        client,
        database,
    )?;
    timings.storage_check_ms = ms(started);

    let started = Instant::now();
    let staged = read_row_metas(
        client,
        &format!("SELECT {ROW_COLUMNS} FROM {db}.dbo.ConfigSave ORDER BY FileName, PartNo"),
    )?;
    let mut report = blank_report(options, &storage, ApplyMode::Exclusive);
    if staged.is_empty() {
        report.nothing_to_apply = true;
        timings.inventory_ms = ms(started);
        timings.total_ms = ms(total);
        report.timings = timings;
        return Ok(ConfigApplyPlan {
            report,
            script: None,
            inputs: None,
            staged,
            replaced: Vec::new(),
            mobile_versions_before: None,
            new: objects::NewObjects::default(),
            registration: registrations::RegistrationPlan::default(),
            removals: removals::Removals::default(),
        });
    }
    let replaced = read_row_metas(
        client,
        &format!(
            "SELECT {ROW_COLUMNS} FROM {db}.dbo.Config s WHERE EXISTS (SELECT 1 FROM {db}.dbo.ConfigSave x WHERE x.FileName = s.FileName) ORDER BY FileName, PartNo"
        ),
    )?;
    let special = read_row_metas(
        client,
        &format!(
            "SELECT {ROW_COLUMNS} FROM {db}.dbo.Config WHERE FileName = N'DynamicallyUpdated' OR FileName LIKE N'%\\_dynupdate\\_%' ESCAPE N'\\' ORDER BY FileName, PartNo"
        ),
    )?;
    require_no_unfinished_operation(client, &db)?;
    // The change registrations exist with their file lists or not at all.
    let has_change_registrations = scalar_i64(
        client,
        &format!(
            "SELECT CASE WHEN OBJECT_ID(N'{db}.dbo._ConfigChngR', N'U') IS NULL OR OBJECT_ID(N'{db}.dbo._ConfigChngR_ExtProps', N'U') IS NULL THEN 0 ELSE 1 END"
        ),
    )? == 1;

    // The stage's list of removals (`deleted`). The native apply never deletes a row that a staged
    // row does not replace; a removal travels in this list. The list is answered name by name:
    //
    // - an empty list, or the rows of a dynamic update that `Config` carries (all of them): consumed,
    //   the rows deleted without folding, as the native apply does;
    // - the rows of a removed form or template ([`removals`]): deleted with the search-information
    //   records, when the analysis accounts for every row of the object;
    // - anything else (an attribute id, a name with a table, a body of an object that stays, a name the
    //   analysis cannot place): the gate judges it when it says it can (the S1 gate, for the removed
    //   attributes), and the list is consumed only if a structure phase accounts for the rest;
    //   otherwise the list is refused as a whole (docs/apply/own-apply.md, "Removals").
    let mut consumed: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut consumed_note = None;
    // the dynamic-update rows the list names: dropped, not folded
    let mut dropped_rows: Vec<String> = Vec::new();
    // the list names removals and the gate said it judges them
    let mut judged_deleted = false;
    let mut removals = removals::Removals::default();
    if staged
        .iter()
        .any(|row| row.name.eq_ignore_ascii_case("deleted"))
    {
        let several_parts = staged
            .iter()
            .any(|row| row.name.eq_ignore_ascii_case("deleted") && row.part != 0);
        let plain = if several_parts {
            None
        } else {
            read_blob(client, database, "ConfigSave", "deleted")?
                .and_then(|bytes| versions::inflate_row(&bytes).ok())
        };
        let overlay_rows: std::collections::HashSet<String> = special
            .iter()
            .map(|row| row.name.to_ascii_lowercase())
            .collect();
        let description = plain
            .as_deref()
            .map(describe_removals)
            .unwrap_or_else(|| "a list this apply cannot read".to_owned());
        let Some(entries) = plain.as_deref().and_then(parse_removals) else {
            return Err(NeedsNativeApply::apply(format!(
                "the stage carries a `deleted` row, the list of removals: {description}"
            ))
            .into());
        };
        let (overlay_listed, object_names) = split_removals(&entries, &overlay_rows);
        let mut analysis = if object_names.is_empty() {
            removals::RemovalAnalysis::default()
        } else {
            removals::analyze(&removals::RemovalInput {
                client,
                database,
                names: &object_names,
                overlay_rows: &overlay_rows,
                staged: &staged,
            })?
        };
        // Removals are measured on 8.3.27 only: the search-information records and the change register
        // of a removed form were compared with the native apply there.
        if options.platform_profile == MssqlNativePlatformProfile::Platform8_5_1_1150
            && !analysis.removals.is_empty()
        {
            analysis.blockers.push(gate::GateBlocker {
                row: analysis.removals.rows.first().cloned().unwrap_or_default(),
                reason: "a removed form or template is measured on 8.3.27 only; on 8.5 the platform's own apply removes it".to_owned(),
            });
        }
        if !analysis.removals.is_empty() && analysis.blockers.is_empty() {
            analysis.blockers.extend(removals::extension_blockers(
                client,
                sql,
                database,
                &analysis.removals,
            )?);
        }
        let answer = answer_removals(
            &entries,
            &overlay_rows,
            &analysis.removals.accounted,
            &analysis.blockers,
            structural_gate.judges_deleted_row(),
        );
        match answer {
            ListAnswer::Consumed | ListAnswer::Judged => {
                dropped_rows = overlay_listed;
                let mut note = description;
                if !dropped_rows.is_empty() {
                    note.push_str("; the dynamic-update rows it names are deleted, not folded");
                }
                if !analysis.removals.is_empty() {
                    note.push_str(&format!(
                        "; {} removed form(s) or template(s) ({} rows) are deleted with their search-information records",
                        analysis.removals.objects.len(),
                        analysis.removals.rows.len()
                    ));
                }
                if answer == ListAnswer::Judged {
                    judged_deleted = true;
                    note.push_str("; the gate judges the rest");
                }
                consumed_note = Some(note);
                consumed.insert("deleted".to_owned());
                removals = analysis.removals;
            }
            ListAnswer::Refused(reason) => {
                let removal_blockers = analysis.blockers;
                let mut message = format!(
                    "the stage carries a `deleted` row, the list of removals: {description}. This apply consumes an empty list, the rows of a dynamic update and the rows of a removed form or template that it can account for name by name ({}); a list it cannot account for as a whole, and the stage of the platform's own `config import`, need the native `ibcmd infobase config apply`",
                    reason
                );
                for blocker in removal_blockers.iter().take(5) {
                    message.push_str(&format!("; {}: {}", blocker.row, blocker.reason));
                }
                if removal_blockers.len() > 5 {
                    message.push_str(&format!(
                        " (and {} more reasons)",
                        removal_blockers.len() - 5
                    ));
                }
                return Err(NeedsNativeApply::apply(message).into());
            }
        }
    }
    require_settled_storage(client, &db)?;
    timings.inventory_ms = ms(started);

    let active: HashMap<(String, i32), RowMeta> = replaced
        .iter()
        .cloned()
        .map(|row| (row.key(), row))
        .collect();
    let staged_bytes: i64 = staged.iter().map(|row| row.byte_len).sum();
    let mut stage = StageSummary {
        rows: staged.len(),
        bytes: staged_bytes,
        descriptors: 0,
        bodies: 0,
        service_rows: 0,
        new_rows: 0,
        identical_rows: 0,
        replaced_rows: replaced.len(),
        replaced_parts_dropped: 0,
        consumed_rows: 0,
    };
    for row in &staged {
        match model::classify_name(&row.name) {
            model::RowName::Service(_) => stage.service_rows += 1,
            model::RowName::Descriptor(_) => stage.descriptors += 1,
            model::RowName::Body { .. } => stage.bodies += 1,
            model::RowName::Other => {}
        }
        if consumed.contains(&row.name.to_ascii_lowercase()) {
            stage.consumed_rows += 1;
            continue;
        }
        match active.get(&row.key()) {
            None => stage.new_rows += 1,
            Some(active_row) if active_row.sha256 == row.sha256 => stage.identical_rows += 1,
            Some(_) => {}
        }
    }
    let staged_keys: std::collections::HashSet<_> = staged.iter().map(RowMeta::key).collect();
    stage.replaced_parts_dropped = replaced
        .iter()
        .filter(|row| !staged_keys.contains(&row.key()))
        .count();

    // versions: the staged row replaces the ordinary one and names a new
    // generation.
    let staged_versions = read_blob(client, database, "ConfigSave", "versions")?
        .ok_or_else(|| anyhow!("ConfigSave holds no versions row: not a complete stage"))?;
    let staged_versions = versions::parse_versions(&staged_versions)?;
    // A delta stage (the rows of a few objects and `versions`) is a stage too:
    // the native apply takes it and keeps the active `root` and `version`.
    for service in ["root", "version"] {
        if !staged.iter().any(|row| row.name == service) {
            report.warnings.push(format!(
                "the stage has no {service} row: a delta stage, the active {service} stays"
            ));
        }
    }
    let active_versions = read_blob(client, database, "Config", "versions")?
        .ok_or_else(|| anyhow!("Config holds no versions row"))?;
    let active_versions = versions::parse_versions(&active_versions)?;
    let config_marker = read_blob(client, database, "Config", "DynamicallyUpdated")?;
    let params_marker = read_blob(client, database, "Params", "DynamicallyUpdated")?;
    let history =
        versions::parse_dynamic_history(config_marker.as_deref(), params_marker.as_deref())?;
    let mut known = vec![active_versions.generation];
    known.extend(history.generations.iter().copied());
    if known.contains(&staged_versions.generation) {
        bail!(
            "the staged versions row reuses generation {}: the stage was made against another state",
            staged_versions.generation
        );
    }
    let listed: std::collections::HashSet<String> = staged_versions
        .entries
        .iter()
        .map(|(name, _)| name.to_lowercase())
        .collect();
    let unlisted = staged
        .iter()
        .filter(|row| {
            row.part == 0
                && !listed.contains(&row.name.to_lowercase())
                && !consumed.contains(&row.name.to_ascii_lowercase())
        })
        .count();
    if unlisted > 0 {
        report.warnings.push(format!(
            "{unlisted} staged row(s) are not listed in the staged versions row"
        ));
    }
    if special.iter().any(|row| {
        row.name
            .to_ascii_lowercase()
            .starts_with("deleted_dynupdate_")
    }) {
        return Err(NeedsNativeApply::apply(
            "a deleted_dynupdate_* row records deleted objects of a dynamic generation; the own apply cannot fold deletions: run the native `ibcmd infobase config apply`",
        )
        .into());
    }
    let alias_rows = special
        .iter()
        .filter(|row| {
            row.name.contains("_dynupdate_") && !row.name.starts_with("versions_dynupdate_")
        })
        .count();
    let handled = |name: &str| {
        history
            .generations
            .iter()
            .any(|generation| name.contains(&generation.hyphenated().to_string()))
    };
    let orphan_aliases = special
        .iter()
        .filter(|row| row.name.contains("_dynupdate_") && !handled(&row.name))
        .count();
    if orphan_aliases > 0 {
        report.warnings.push(format!(
            "{orphan_aliases} dynamic alias row(s) belong to no generation of the DynamicallyUpdated markers; left untouched"
        ));
    }
    report.active_generation = Some(
        history
            .generations
            .last()
            .copied()
            .unwrap_or(active_versions.generation)
            .hyphenated()
            .to_string(),
    );
    report.new_generation = Some(staged_versions.generation.hyphenated().to_string());
    report.dynamic = Some(DynamicSummary {
        generations: history
            .generations
            .iter()
            .map(|generation| generation.hyphenated().to_string())
            .collect(),
        alias_rows,
    });
    if let Some(description) = consumed_note {
        report.warnings.push(format!(
            "the stage's `deleted` list is answered by this apply ({description}); the row is consumed, not moved into Config, as the native apply does"
        ));
    }
    report.stage = Some(stage);

    // New rows: a form or template an existing object gains, or a body row.
    let started = Instant::now();
    let analysis = objects::analyze(&objects::AnalysisInput {
        client,
        database,
        staged: &staged,
        active: &active,
        has_change_registrations,
    })?;
    let new = analysis.new;
    let mut analysis_blockers = analysis.blockers;
    // New objects are measured on 8.3.27 only: their registrations, node rule and
    // search records were compared with the native apply there.
    if options.platform_profile == MssqlNativePlatformProfile::Platform8_5_1_1150 && !new.is_empty()
    {
        let first = new
            .objects
            .first()
            .map(|object| object.uuid.clone())
            .or_else(|| new.bodies.first().map(|body| body.file_name.clone()))
            .unwrap_or_default();
        analysis_blockers.push(gate::GateBlocker {
            row: first,
            reason: "a new form, template or body is measured on 8.3.27 only; on 8.5 the platform's own apply registers it".to_owned(),
        });
    }
    // A new object on a change register that has no rows at all (the ERP УХ
    // corpus): the native apply writes nothing there for existing objects and
    // what it does for a new one is not measured, so the apply does not guess.
    if !new.is_empty() && has_change_registrations {
        let register_has_rows = scalar_i64(
            client,
            &format!(
                "SELECT CASE WHEN EXISTS (SELECT 1 FROM {db}.dbo._ConfigChngR) THEN 1 ELSE 0 END"
            ),
        )? == 1;
        if !register_has_rows {
            let first = new
                .objects
                .first()
                .map(|object| object.uuid.clone())
                .or_else(|| new.bodies.first().map(|body| body.file_name.clone()))
                .unwrap_or_default();
            analysis_blockers.push(gate::GateBlocker {
                row: first,
                reason: "the change register has no rows: how the native apply registers a new form, template or body there is not measured (docs/apply/own-apply.md)".to_owned(),
            });
        }
    }

    // The structural gate.
    // The owners' descriptors that differ from the active ones by references only: to the new objects
    // and from the removed ones. The names the apply deletes itself (the removed objects' rows and the
    // rows of a dynamic update) are not for a gate that judges the list to look at.
    let accepted_owners: std::collections::HashSet<String> = new
        .owners
        .iter()
        .chain(removals.owners.iter())
        .cloned()
        .collect();
    let removed_names: std::collections::HashSet<String> = removals
        .accounted
        .iter()
        .cloned()
        .chain(dropped_rows.iter().map(|name| name.to_ascii_lowercase()))
        .collect();
    let mut verdict = structural_gate.check(&GateInput {
        client,
        database,
        staged: &staged,
        active: &active,
        accepted_new_rows: &new.rows,
        accepted_owner_descriptors: &accepted_owners,
        new_object_kinds: &new.kinds,
        consumed_rows: &consumed,
        removed_rows: &removed_names,
    })?;
    timings.gate_ms = ms(started);
    // A gate that lets a restructuring through hands over the structure work: T-SQL for this
    // transaction and the cache rows it makes stale (docs/apply/own-apply.md, "Restructuring").
    let structure = structural_gate.take_structure();
    // The catalogs and documents the structure phase creates (S1-F): the phase answers for the objects and
    // their rows (the analysis above knows a new form or template only), this apply moves the rows like any
    // staged row and registers the objects like a new form.
    let created: Vec<gate::CreatedObject> = structure
        .as_ref()
        .map(|phase| phase.created.clone())
        .unwrap_or_default();
    let answered_rows: Vec<String> = structure
        .as_ref()
        .map(|phase| phase.answered_rows.clone())
        .unwrap_or_default();
    let answered_for = |row: &str| {
        let row = row.to_ascii_lowercase();
        answered_rows.contains(&row)
            || created.iter().any(|object| {
                let uuid = object.uuid.to_ascii_lowercase();
                row == uuid
                    || object
                        .files
                        .iter()
                        .any(|file| file.to_ascii_lowercase() == row)
            })
    };
    for blocker in analysis_blockers {
        if !answered_for(&blocker.row) {
            verdict.block(&blocker.row, blocker.reason);
        }
    }
    if verdict.restructuring_required {
        report.gate = Some(verdict.clone());
        timings.total_ms = ms(total);
        report.timings = timings;
        return Err(anyhow::Error::new(StructuralRefusal { verdict }));
    }
    report.gate = Some(verdict);
    // A `deleted` list of removals is consumed only when a phase accounts for it.
    if judged_deleted
        && !structure
            .as_ref()
            .is_some_and(|phase| phase.consumed_staged_rows > 0)
    {
        return Err(NeedsNativeApply::apply(
            "the stage's `deleted` list asks for removals that no structure phase accounts for; run the native `ibcmd infobase config apply`",
        )
        .into());
    }

    // Fingerprints the script asserts.
    let started = Instant::now();
    let staged_fp = read_fingerprint(client, &staged_source(database)?)?;
    let replaced_fp = read_fingerprint(client, &replaced_source(database)?)?;
    let special_config_fp = read_fingerprint(client, &special_config_source(database)?)?;
    let special_params_fp = read_fingerprint(client, &special_params_source(database)?)?;
    timings.fingerprints_ms = ms(started);
    if staged_fp.rows != staged.len() as i64 {
        bail!("ConfigSave changed while the plan was being made");
    }

    // Files.MobileVersions.dat: a fresh GUID at the head.
    let (files_rewrites, mobile_before) = plan_mobile_versions(client, database, &mut report)?;

    // The `Params` rows to rewrite: the search information of new objects.
    // The `.ui` rows (the platform's configuration-licensing records) are never
    // written.
    let mut params_rewrites = new.search_info.clone();
    if let Some(phase) = &structure {
        params_rewrites = merge_params_rewrites(params_rewrites, &phase.params_rewrites)?;
    }
    // The removed forms and templates take their records out of the search information, on top of
    // what the new objects and a restructuring of the same stage rewrite.
    let mut search_info_removal = removals::SearchInfoRemoval::default();
    if !removals.is_empty() {
        let (rewrites, summary) =
            removals::plan_search_info(client, database, &removals, params_rewrites).map_err(
                |error| {
                    anyhow::Error::from(NeedsNativeApply::apply(format!(
                        "the search information cannot be edited for the removed forms and templates: {error:#}; run the native `ibcmd infobase config apply`"
                    )))
                },
            )?;
        params_rewrites = rewrites;
        search_info_removal = summary;
    }
    // Every staged descriptor's synonyms must agree with the registry, on top of the edits above. Never
    // filter by the physical Config digest: an effective dynamic alias can differ while the stage reverts to
    // the physical base's exact bytes, and that reversion still needs the registry's synonym changed.
    let staged_descriptors: Vec<String> = staged
        .iter()
        .filter(|row| {
            row.part == 0
                && matches!(
                    model::classify_name(&row.name),
                    model::RowName::Descriptor(_)
                )
                && !consumed.contains(&row.name.to_ascii_lowercase())
        })
        .map(|row| row.name.clone())
        .collect();
    let mut synonym_records = 0usize;
    let synonyms_started = Instant::now();
    if !staged_descriptors.is_empty() {
        let synonym_error = |error: anyhow::Error| {
            anyhow::Error::from(NeedsNativeApply::apply(format!(
                "the object registry cannot be edited for the changed synonyms: {error:#}; run the native `ibcmd infobase config apply`"
            )))
        };
        let changes =
            synonyms::read_changes(client, database, &staged_descriptors).map_err(synonym_error)?;
        if !changes.is_empty() {
            let (rewrites, records) =
                synonyms::plan_search_info(client, database, &changes, params_rewrites)
                    .map_err(synonym_error)?;
            params_rewrites = rewrites;
            synonym_records = records;
        }
        timings.synonyms_ms = ms(synonyms_started);
    }

    // The change registrations of the nodes of distributed infobases: the rows a node with no rows gets
    // for the objects this stage changes (docs/apply/own-apply.md, "Exchange plans").
    // A removed object is registered like an object that owns a staged row: its message numbers are reset and a
    // node with no row of it gets one, with its files -- as for the rows of a dynamic update a `deleted` list names
    // (twins of a removed form and template with message numbers and a missing row, docs/apply/own-apply.md).
    let registered_names: Vec<String> = dropped_rows
        .iter()
        .chain(removals.rows.iter())
        .cloned()
        .collect();
    let registration = if has_change_registrations {
        registrations::plan(client, database, &staged, &registered_names)?
    } else {
        registrations::RegistrationPlan::default()
    };
    // The nodes a created object is registered at: every node of the exchange plans but the plans' own, the ones
    // with no rows included (`objects::registration_nodes`, #412). Measured on 8.3.27 only, and on a register
    // that has rows.
    let created_nodes = if created.is_empty() || !has_change_registrations {
        Vec::new()
    } else {
        if options.platform_profile == MssqlNativePlatformProfile::Platform8_5_1_1150 {
            return Err(NeedsNativeApply::apply(
                "a created object is measured on 8.3.27 only; on 8.5 the platform's own apply registers it",
            )
            .into());
        }
        objects::registration_nodes(client, &db).map_err(NeedsNativeApply::apply)?
    };

    let mut touched = vec!["Config", "ConfigSave"];
    if config_marker.is_some() || params_marker.is_some() || !params_rewrites.is_empty() {
        touched.push("Params");
    }
    if has_change_registrations {
        touched.push("_ConfigChngR");
        if !new.is_empty()
            || !registration.dropped_files.is_empty()
            || registration.added_file_rows > 0
            || created.iter().any(|object| !object.files.is_empty())
        {
            touched.push("_ConfigChngR_ExtProps");
        }
    }
    if !files_rewrites.is_empty() {
        touched.push("Files");
    }
    report.tables_touched = touched.into_iter().map(str::to_owned).collect();
    if let Some(phase) = &structure {
        report
            .tables_touched
            .extend(["SchemaStorage", "DBSchema"].map(str::to_owned));
        report.tables_touched.extend(phase.tables.iter().cloned());
    }
    let nodes_seen = if (new.is_empty()
        && created.is_empty()
        && registration.dropped_files.is_empty())
        || !has_change_registrations
    {
        0
    } else {
        scalar_i64(
            client,
            &format!(
                "SELECT COUNT_BIG(*) FROM (SELECT DISTINCT _NodeTRef, _NodeRRef FROM {db}.dbo._ConfigChngR) d"
            ),
        )? as usize
    };
    report.registrations = has_change_registrations.then_some(RegistrationSummary {
        nodes: registration.nodes.len(),
        changed_objects: registration.changed_objects,
        rows_added: registration.added_rows,
        file_rows_added: registration.added_file_rows,
        objects_of_dropped_rows: registration.extra_objects.len(),
    });
    report.new_objects = (!new.is_empty()).then(|| NewObjectsSummary {
        objects: new.objects.clone(),
        appended_bodies: new.bodies.clone(),
        registration_nodes: new.nodes.len(),
        search_info_records: new.search_info_records,
    });
    let object_hex = |uuid: &str| -> Result<String> {
        Ok(model::hex_upper(
            &Uuid::parse_str(uuid)
                .with_context(|| format!("{uuid} is not a uuid"))?
                .to_bytes_le(),
        ))
    };
    let mut new_registrations = new
        .objects
        .iter()
        .map(|object| {
            Ok(NewRegistration {
                object_hex: object_hex(&object.uuid)?,
                files: object.bodies.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    for object in &created {
        new_registrations.push(NewRegistration {
            object_hex: object_hex(&object.uuid)?,
            files: object.files.clone(),
        });
    }
    let mut appended_files = new
        .bodies
        .iter()
        .map(|body| {
            Ok(AppendedFile {
                object_hex: object_hex(&body.object)?,
                file_name: body.file_name.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    // the bodies a `deleted` list names are listed for the objects, as the native apply does
    for file in &registration.dropped_files {
        if !appended_files
            .iter()
            .any(|known| known.object_hex == file.object_hex && known.file_name == file.file_name)
        {
            appended_files.push(file.clone());
        }
    }
    // the nodes registrations are made for: the plans' nodes, the ones with no rows included
    let node_source = if !registration.nodes.is_empty() {
        &registration.nodes
    } else if !new.nodes.is_empty() {
        &new.nodes
    } else {
        &created_nodes
    };
    let nodes = node_literals(node_source);
    report.not_written = vec![
        "Params .ui rows (the platform's configuration-licensing records, track ui #340): never written; the native apply re-encrypts two of them on every apply".to_owned(),
    ];
    if structure.is_some() && options.backup == BackupPolicy::None {
        report.warnings.push(
            "the stage restructures tables: a real run needs --recovery-backup <file> (recommended) or --i-have-a-backup".to_owned(),
        );
    }
    if structure.is_some() {
        report.not_written.push(
            "the pre-image of the rebuilt tables: the old tables are dropped inside the transaction, so a committed restructuring is taken back from a SQL Server backup (the recovery artifact keeps the Config rows and the cache rows only)".to_owned(),
        );
    }
    report.not_written.extend([
        "Params .si service-information rows and siVersions, except the main row, the properties row and their versions when a new or removed form or template changes the records, a restructuring changes a cache, or a synonym changes (the native apply re-encodes every .si row with a new version; the content is unchanged otherwise)".to_owned(),
        "the help/search index in Files (userDocs_ru*, userPostings_ru*, userVocabulary_ru*)".to_owned(),
        "the extension CAS garbage collection (ConfigCAS, Files CAS_GC_Info, extd_props_cached/gc.mrk)".to_owned(),
        "scratch rows of the extension restructure (_ExtensionsRestructNGS)".to_owned(),
    ]);

    let removed_fp = if removals.rows.is_empty() {
        Fingerprint::default()
    } else {
        let fingerprint =
            read_fingerprint(client, &sqlgen::removed_source(database, &removals.rows)?)?;
        if fingerprint.rows != removals.rows.len() as i64 {
            bail!(
                "Config changed while the plan was being made: {} of the {} rows of the removed objects are there",
                fingerprint.rows,
                removals.rows.len()
            );
        }
        fingerprint
    };
    report.synonym_records = synonym_records;
    report.removals = (!removals.is_empty()).then(|| RemovalsSummary {
        objects: removals.objects.clone(),
        rows_deleted: removals.rows.len(),
        search_info_records: search_info_removal.records_removed,
        property_entries: search_info_removal.property_entries_removed,
    });
    let consumed_row_count = staged
        .iter()
        .filter(|row| consumed.contains(&row.name.to_ascii_lowercase()))
        .count() as i64;
    let mut consumed_names = consumed.iter().cloned().collect::<Vec<_>>();
    consumed_names.sort();
    let marker_decision = params_marker::plan(params_marker::PlanInput {
        client,
        db: &db,
        profile: options.platform_profile,
        staged: &staged,
        replaced: &replaced,
        special: &special,
        active_versions: &active_versions,
        history: &history,
        config_marker: config_marker.as_deref(),
        params_marker: params_marker.as_deref(),
    })
    .map_err(|error| {
        NeedsNativeApply::apply(format!("Params marker descriptor decision: {error:#}"))
    })?;
    let inputs = ScriptInputs {
        database: options.database.clone(),
        client_pid: std::process::id(),
        rehearse: options.rehearse,
        require_exclusive: options.exclusivity == Exclusivity::SqlSessions,
        session_exemption: session_exemption(&options.own_ras_processes),
        staged: staged_fp,
        replaced: replaced_fp,
        special_config: special_config_fp,
        special_params: special_params_fp,
        clear_params_marker: marker_decision.clear,
        params_marker_guard_sql: marker_decision.guard_sql,
        generations: history.generations.clone(),
        reset_change_registrations: has_change_registrations,
        files_rewrites,
        params_rewrites,
        new_registrations,
        nodes,
        nodes_seen,
        registration_additions: registration.additions.clone(),
        registration_rows_expected: registration.added_rows,
        registration_file_rows_expected: registration.added_file_rows,
        plan_node_counts: registration.node_counts.clone(),
        extra_changed_objects: registration.extra_objects.clone(),
        appended_files,
        appended_registration_ids: None,
        consumed_names,
        consumed_row_count,
        dropped_rows,
        removed_rows: removals.rows.clone(),
        removed: removed_fp,
        structure_sql: structure.as_ref().map(|phase| phase.sql.clone()),
    };
    report.structure = structure;
    let script = render_apply_script(&inputs)?;
    let script_sha = hex_lower(&Sha256::digest(script.as_bytes()));
    report.script_sha256 = Some(script_sha.clone());
    report.recovery_token = Some(script_sha[..16].to_owned());
    timings.total_ms = ms(total);
    report.timings = timings;
    Ok(ConfigApplyPlan {
        report,
        script: Some(script),
        inputs: Some(inputs),
        staged,
        replaced,
        mobile_versions_before: mobile_before,
        new,
        registration,
        removals,
    })
}

/// Plans and, unless `dry_run`, applies the staged configuration.
pub fn apply_staged_configuration(
    sql: &SqlExec,
    options: &ConfigApplyOptions,
) -> Result<ConfigApplyReport> {
    let gate = structural_gate(sql, options)?;
    apply_with_gate(sql, options, gate.as_ref())
}

pub fn apply_with_gate(
    sql: &SqlExec,
    options: &ConfigApplyOptions,
    structural_gate: &dyn StructuralGate,
) -> Result<ConfigApplyReport> {
    let total = Instant::now();
    let client = require_client(sql)?;
    let mut plan = plan_with_gate(sql, options, structural_gate)?;
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
    if options.exclusivity == Exclusivity::SqlSessions {
        let sessions = other_sessions(client, &options.database, &options.own_ras_processes)?;
        if !sessions.is_empty() {
            return Err(ExclusiveAccessRefused {
                database: options.database.clone(),
                sessions,
                in_transaction: false,
            }
            .into());
        }
    }
    if options.dry_run {
        plan.report.timings.total_ms = ms(total);
        return Ok(plan.report);
    }

    // A restructuring needs the operator's word about a way back, and a backup asked
    // for is taken before anything is written (a rehearsal and a dry run write
    // nothing that stays).
    require_backup(
        &options.backup,
        plan.report.structure.is_some(),
        !options.rehearse,
    )?;
    let mut backup = None;
    let mut backup_ms = 0u128;
    match &options.backup {
        BackupPolicy::None => {}
        BackupPolicy::Acknowledged => {
            backup = Some(BackupRecord {
                kind: "acknowledged",
                path: None,
                seconds: None,
            });
        }
        BackupPolicy::File(file) if !options.rehearse => {
            if file.exists() {
                bail!(
                    "the backup file {} exists already: the apply does not overwrite a backup; name another file",
                    file.display()
                );
            }
            let started = Instant::now();
            let db = quote_ident(&options.database)?;
            client
                .execute(
                    &format!(
                        "BACKUP DATABASE {db} TO DISK = N'{}' WITH COPY_ONLY, COMPRESSION",
                        quote_string(&file.display().to_string())
                    ),
                    &[],
                )
                .with_context(|| {
                    format!(
                        "the backup to {} failed; nothing was changed",
                        file.display()
                    )
                })?;
            backup_ms = ms(started);
            backup = Some(BackupRecord {
                kind: "file",
                path: Some(file.display().to_string()),
                seconds: Some(backup_ms as f64 / 1000.0),
            });
        }
        BackupPolicy::File(_) => {}
    }

    // The recovery artifact: the pre-image of what the script overwrites.
    let started = Instant::now();
    let token = plan
        .report
        .recovery_token
        .clone()
        .ok_or_else(|| anyhow!("the plan has no recovery token"))?;
    let dir = options.recovery_dir.clone().unwrap_or_else(|| {
        recovery::artifact_dir(&recovery::default_root(), &options.database, &token)
    });
    plan.report.backup = backup;
    plan.report.timings.backup_ms = backup_ms;
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
            reset_change_registrations: plan
                .inputs
                .as_ref()
                .is_some_and(|inputs| inputs.reset_change_registrations),
            new_objects: &plan.new,
            removals: &plan.removals,
            registration: &plan.registration,
            params_rewrites: plan
                .inputs
                .as_ref()
                .map_or(&[][..], |inputs| inputs.params_rewrites.as_slice()),
            bound_params_preimages: None,
            backup: plan.report.backup.as_ref(),
            dynamic_generation: None,
            appended_existing_rows: &[],
        },
    )?;
    plan.report.recovery_dir = Some(dir);
    plan.report.timings.recovery_ms = ms(started);

    let started = Instant::now();
    if let Err(error) = client.run_script(&script, ScriptVariables::Refuse) {
        // the refusals of the transaction's own checks are typed; a drifted
        // fingerprint or a failed postcondition stays an ordinary failure
        if let Some((number, message)) = errors::server_error_of(&error)
            && let Some(typed) =
                errors::from_transaction_code(number, &message, &options.database, || {
                    other_sessions(client, &options.database, &options.own_ras_processes)
                        .unwrap_or_default()
                })
        {
            return Err(typed);
        }
        return Err(error.context("the config apply transaction failed; the database is unchanged"));
    }
    plan.report.timings.sql_ms = ms(started);
    plan.report.executed = !options.rehearse;

    // A last look, outside the transaction.
    if !options.rehearse {
        let db = quote_ident(&options.database)?;
        let left = scalar_i64(
            client,
            &format!("SELECT COUNT_BIG(*) FROM {db}.dbo.ConfigSave"),
        )?;
        if left != 0 {
            bail!("ConfigSave still holds {left} row(s) after the apply");
        }
    }

    // The default directory keeps the last few artifacts of a database: the run
    // succeeded, so the older ones go. A directory the caller named is theirs.
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

/// The disposable databases of the lab tracks: `ibcmd_rs_04_*` and `ibcmd_rs_05_*`.
pub fn is_lab_database(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    ["ibcmd_rs_04_", "ibcmd_rs_05_"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// Whether a run of `ibcmd-rs mssql-config-apply` needs `--allow-non-lab`: a write to a database that is
/// not one of the lab's. A dry run writes nothing. (The drop-in `ibcmd infobase config apply` goes through
/// [`apply_staged_configuration`] and asks for no acknowledgement at all: it is the production entry.)
pub fn write_needs_acknowledgement(database: &str, dry_run: bool) -> bool {
    !dry_run && !is_lab_database(database)
}

/// `ibcmd-rs mssql-config-apply`: build the SQL handle, run, print the report.
pub fn run_command(args: &crate::cli::MssqlConfigApplyArgs) -> Result<()> {
    use crate::cli::{MssqlConfigApplyExclusivityArg, MssqlConfigApplyRecoveryArg};
    use crate::sql::SqlOptions;

    if write_needs_acknowledgement(&args.database, args.dry_run) && !args.allow_non_lab {
        bail!(
            "--allow-non-lab acknowledgement is required for a database write outside the lab (a database named ibcmd_rs_04_* or ibcmd_rs_05_*) or use --dry-run"
        );
    }
    let password = args.sql_user.as_deref().and_then(|_| {
        args.sql_pwd
            .clone()
            .filter(|value| !value.is_empty())
            .or_else(|| std::env::var(&args.sql_pwd_env).ok())
    });
    let sql = SqlExec::from_options(SqlOptions {
        sqlcmd: None,
        bcp: None,
        server: &args.server,
        user: args.sql_user.as_deref(),
        password: password.as_deref(),
        password_env: &args.sql_pwd_env,
        trust_server_certificate: true,
    })?;
    let mut options = ConfigApplyOptions::new(args.database.clone(), args.platform_profile);
    options.dry_run = args.dry_run;
    options.rehearse = args.rehearse;
    options.exclusivity = match args.exclusivity {
        MssqlConfigApplyExclusivityArg::Sql => Exclusivity::SqlSessions,
        MssqlConfigApplyExclusivityArg::Assumed => Exclusivity::Assumed,
    };
    options.recovery_dir = args.recovery_dir.clone();
    options.recovery_keep = args.recovery_keep;
    options.recovery_blobs = match args.recovery_blobs {
        MssqlConfigApplyRecoveryArg::Changed => RecoveryBlobs::Changed,
        MssqlConfigApplyRecoveryArg::None => RecoveryBlobs::None,
    };
    options.script_output = args.script_output.clone();
    options.allow_restructure = args
        .allow_restructure
        .map(|crate::cli::MssqlConfigApplyRestructureArg::S1| AllowRestructure::S1);
    options.backup = match (&args.recovery_backup, args.i_have_a_backup) {
        (Some(file), _) => BackupPolicy::File(file.clone()),
        (None, true) => BackupPolicy::Acknowledged,
        (None, false) => BackupPolicy::None,
    };
    options.gate = match args.gate {
        crate::cli::MssqlConfigApplyGateArg::ApplyCheck => GateChoice::ApplyCheck,
        crate::cli::MssqlConfigApplyGateArg::Conservative => GateChoice::Conservative,
    };
    if args.admit_unverified_roles
        && (options.gate != GateChoice::Conservative || options.allow_restructure.is_some())
    {
        bail!(
            "--admit-unverified-roles is a switch of the conservative gate: add --gate conservative"
        );
    }
    options.admit_unverified_roles = args.admit_unverified_roles;
    options.restructure_limit = crate::restructure::size_guard::resolve_limit(
        &crate::settings::Settings::load(None)?,
        args.restructure_limit_rows,
        args.restructure_limit_bytes.as_deref(),
    )?;
    match apply_staged_configuration(&sql, &options) {
        Ok(report) => {
            let json = serde_json::to_string_pretty(&report)?;
            if let Some(path) = &args.report {
                std::fs::write(path, &json)
                    .with_context(|| format!("failed to write {}", path.display()))?;
            }
            println!("{json}");
            Ok(())
        }
        Err(error) => {
            if let Some(refusal) = refusal_report(&error, &args.database) {
                let json = serde_json::to_string_pretty(&refusal)?;
                if let Some(path) = &args.report {
                    std::fs::write(path, &json)
                        .with_context(|| format!("failed to write {}", path.display()))?;
                }
                println!("{json}");
            }
            Err(error)
        }
    }
}

/// What the command prints when the apply refuses by type: the same fields
/// whatever the refusal, `refused` naming the kind.
fn refusal_report(error: &anyhow::Error, database: &str) -> Option<serde_json::Value> {
    if let Some(refusal) = error.downcast_ref::<StructuralRefusal>() {
        return Some(serde_json::json!({
            "refused": "needs_native_apply",
            "database": database,
            "gate": refusal.verdict,
        }));
    }
    if let Some(refusal) = error.downcast_ref::<NeedsNativeApply>() {
        return Some(serde_json::json!({
            "refused": "needs_native_apply",
            "database": database,
            "native_command": match refusal.command {
                NativeCommand::Apply => "apply",
                NativeCommand::Repair => "repair",
            },
            "reason": refusal.reason,
        }));
    }
    if let Some(refusal) = error.downcast_ref::<ExclusiveAccessRefused>() {
        return Some(serde_json::json!({
            "refused": "exclusive_access",
            "database": database,
            "in_transaction": refusal.in_transaction,
            "sessions": refusal.sessions,
        }));
    }
    if error.downcast_ref::<BackupRequired>().is_some() {
        return Some(serde_json::json!({
            "refused": "backup_required",
            "database": database,
            "message": error.to_string(),
        }));
    }
    if let Some(refusal) = error.downcast_ref::<ExclusiveAccessUnprovable>() {
        return Some(serde_json::json!({
            "refused": "exclusive_access_unprovable",
            "database": database,
            "reason": refusal.reason,
        }));
    }
    None
}

fn safe_stem(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn write_artifact(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    match std::fs::read(path) {
        Ok(existing) if existing == bytes => Ok(()),
        Ok(_) => bail!("refusing to overwrite existing artifact {}", path.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => std::fs::write(path, bytes)
            .with_context(|| format!("failed to write {}", path.display())),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sessions_query_leaves_out_only_the_idle_sessions_of_the_own_ras_processes() {
        // no own process (the default of every caller but the old activation commands): every session but ours counts
        let plain = other_sessions_query(&[]);
        assert!(plain.contains("ISNULL(host_process_id, -1) <> @P2 ORDER BY session_id"));
        assert!(!plain.contains("1CV83 Server"));
        assert!(
            ConfigApplyOptions::new("db", MssqlNativePlatformProfile::Platform8_3_27_2214)
                .own_ras_processes
                .is_empty()
        );
        let own = [OwnRasProcess {
            host: "wks".to_owned(),
            pid: 4711,
        }];
        let query = other_sessions_query(&own);
        assert!(query.contains("<> @P2 AND NOT (ISNULL(program_name,N'')=N'1CV83 Server'"));
        assert!(query.contains("status=N'sleeping' AND open_transaction_count=0"));
        assert!(query.contains("N'wks' AND ISNULL(host_process_id,-1) IN (4711)"));
        assert!(query.ends_with(" ORDER BY session_id"));
    }

    #[test]
    fn the_8_5_profile_is_planned_like_any_other() {
        // it needs a server, and says so; it is not turned away by its version
        let sql = SqlExec::detached("no server in a unit test");
        let options = ConfigApplyOptions::new("db", MssqlNativePlatformProfile::Platform8_5_1_1150);
        let error = plan(&sql, &options).err().expect("a detached handle fails");
        assert!(
            !error.to_string().contains("measured on 8.3.27 only"),
            "{error}"
        );
        assert!(error.downcast_ref::<NeedsNativeApply>().is_none());
    }

    #[test]
    fn a_platform_that_is_measured_still_needs_a_server() {
        let sql = SqlExec::detached("no server in a unit test");
        let options =
            ConfigApplyOptions::new("db", MssqlNativePlatformProfile::Platform8_3_27_2214);
        let error = plan(&sql, &options).err().expect("a detached handle fails");
        assert!(!error.to_string().contains("8.5"), "{error}");
    }

    #[test]
    fn safe_stems_keep_only_file_name_characters() {
        assert_eq!(safe_stem("a b/c:d-e_f"), "a_b_c_d-e_f");
    }

    #[test]
    fn a_deleted_row_is_described_from_its_list() {
        // the shapes measured in the lab: the empty list, one row, the rows of a
        // dynamic update
        assert!(describe_removals("\u{feff}0".as_bytes()).starts_with("it is empty"));
        let one = "\u{feff}1,\"5ff28850-03db-4a0f-b95e-c2ea4d8c516d\",0";
        assert_eq!(
            describe_removals(one.as_bytes()),
            "1 row name(s), 0 of them rows of a dynamic update and 1 others, the first of them 5ff28850-03db-4a0f-b95e-c2ea4d8c516d"
        );
        let overlay = "\u{feff}3,\"ab132638-5188-470d-9432-de85f2b2c7d8_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b.0\",0,\"DynamicallyUpdated\",0,\"versions_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b\",0";
        assert_eq!(
            describe_removals(overlay.as_bytes()),
            "3 row name(s), 3 of them rows of a dynamic update and 0 others"
        );
        // a count that disagrees with the names, and text that is no list
        assert!(describe_removals(b"2,\"a\",0").contains("cannot read"));
        assert!(describe_removals(b"{1,2}").contains("cannot read"));
    }

    fn answer(
        text: &str,
        carried: &std::collections::HashSet<String>,
        accounted: &[&str],
        blockers: &[gate::GateBlocker],
        gate_judges: bool,
    ) -> Option<ListAnswer> {
        let accounted: std::collections::HashSet<String> = accounted
            .iter()
            .map(|name| name.to_ascii_lowercase())
            .collect();
        parse_removals(text.as_bytes())
            .map(|entries| answer_removals(&entries, carried, &accounted, blockers, gate_judges))
    }

    #[test]
    fn a_deleted_list_asks_for_nothing_when_it_is_empty_or_only_a_dynamic_update() {
        let alias =
            "ab132638-5188-470d-9432-de85f2b2c7d8_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b";
        let versions = "versions_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b";
        let carried: std::collections::HashSet<String> = [alias, versions, "DynamicallyUpdated"]
            .iter()
            .map(|name| name.to_ascii_lowercase())
            .collect();
        let asks_for_nothing = |text: &str| {
            answer(text, &carried, &[], &[], false).map(|answer| answer == ListAnswer::Consumed)
        };
        // empty
        assert_eq!(asks_for_nothing("\u{feff}0"), Some(true));
        // the rows of a dynamic update that Config carries
        let overlay = format!("\u{feff}3,\"{alias}\",0,\"DynamicallyUpdated\",0,\"{versions}\",0");
        assert_eq!(asks_for_nothing(&overlay), Some(true));
        // a dynamic-update row Config does not carry now
        let other = "\u{feff}1,\"a627e390-8fad-4a95-afe6-674f54813188_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b\",0";
        assert_eq!(asks_for_nothing(other), Some(false));
        // an ordinary row: a removal the analysis has not accounted for
        assert_eq!(
            asks_for_nothing("\u{feff}1,\"5ff28850-03db-4a0f-b95e-c2ea4d8c516d\",0"),
            Some(false)
        );
        // a name with flag 1 (a nested object, not a row)
        let flagged = format!("\u{feff}1,\"{alias}\",1");
        assert_eq!(asks_for_nothing(&flagged), Some(false));
        // the removal list of a dynamic update itself is never folded
        let mut with_removals = carried.clone();
        with_removals.insert("deleted_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b".to_owned());
        let entries =
            parse_removals(b"1,\"deleted_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b\",0")
                .unwrap();
        assert_ne!(
            answer_removals(
                &entries,
                &with_removals,
                &std::collections::HashSet::new(),
                &[],
                false
            ),
            ListAnswer::Consumed
        );
        // some but not all of the rows of the update: not measured
        let partial = format!("\u{feff}1,\"{alias}\",0");
        match answer(&partial, &carried, &[], &[], false) {
            Some(ListAnswer::Refused(reason)) => {
                assert!(reason.contains("1 of the 3"), "{reason}")
            }
            other => panic!("{other:?}"),
        }
        // text that is no list
        assert_eq!(asks_for_nothing("{1,2}"), None);
        assert_eq!(asks_for_nothing("2,\"a\",0"), None);
    }

    const FORM: &str = "8a7546f4-bfc9-4732-bf60-43a41e2c8753";
    const TEMPLATE: &str = "0384ec55-dc18-40ae-ae83-7defc28d8f3f";
    const ATTRIBUTE: &str = "c1a2b3d4-0000-4000-8000-000000000001";

    /// The list the platform's own import writes for a tree that loses a form and a template of a БСП
    /// clone with a pending dynamic update, and `extra` entries behind it.
    fn native_list(extra: &str, extra_count: usize) -> (String, std::collections::HashSet<String>) {
        let alias =
            "ab132638-5188-470d-9432-de85f2b2c7d8_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b";
        let versions = "versions_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b";
        let carried = [alias, versions, "DynamicallyUpdated"]
            .iter()
            .map(|name| name.to_ascii_lowercase())
            .collect();
        let text = format!(
            "\u{feff}{},\"{TEMPLATE}\",0,\"{TEMPLATE}.0\",0,\"{FORM}\",0,\"{FORM}.0\",0,\"{FORM}.1\",0,\"{alias}\",0,\"DynamicallyUpdated\",0,\"{versions}\",0{extra}",
            8 + extra_count
        );
        (text, carried)
    }

    fn form_and_template_rows() -> [String; 5] {
        [
            TEMPLATE.to_owned(),
            format!("{TEMPLATE}.0"),
            FORM.to_owned(),
            format!("{FORM}.0"),
            format!("{FORM}.1"),
        ]
    }

    #[test]
    fn the_rows_of_a_removed_form_and_template_are_answered_name_by_name() {
        let (text, carried) = native_list("", 0);
        let rows = form_and_template_rows();
        let accounted: Vec<&str> = rows.iter().map(String::as_str).collect();
        // every name is accounted for: the objects' rows and the rows of the update
        assert_eq!(
            answer(&text, &carried, &accounted, &[], false),
            Some(ListAnswer::Consumed)
        );
        assert_eq!(
            answer(&text, &carried, &accounted, &[], true),
            Some(ListAnswer::Consumed),
            "nothing is left for a gate that judges the list"
        );
        // the analysis accounts for nothing: the list is refused as a whole
        match answer(&text, &carried, &[], &[], false) {
            Some(ListAnswer::Refused(reason)) => {
                assert!(reason.contains("5 name(s) not accounted for"), "{reason}")
            }
            other => panic!("{other:?}"),
        }
        // one body of the form is left out: refused, not consumed in part
        match answer(&text, &carried, &accounted[..4], &[], false) {
            Some(ListAnswer::Refused(reason)) => {
                assert!(reason.contains(&format!("{FORM}.1")), "{reason}")
            }
            other => panic!("{other:?}"),
        }
        // a reason against the objects refuses the list whole, whatever the gate says
        let blocker = gate::GateBlocker {
            row: FORM.to_owned(),
            reason: "the owner's descriptor is not staged".to_owned(),
        };
        assert!(matches!(
            answer(
                &text,
                &carried,
                &accounted,
                std::slice::from_ref(&blocker),
                true
            ),
            Some(ListAnswer::Refused(_))
        ));
    }

    #[test]
    fn a_mixed_list_of_a_removed_form_and_removed_attributes_is_the_gates_for_the_attributes_only()
    {
        // the platform's own import: the rows of the form with the flag 0, the id of a removed attribute
        // with the flag 1
        let (text, carried) = native_list(&format!(",\"{ATTRIBUTE}\",1"), 1);
        let rows = form_and_template_rows();
        let accounted: Vec<&str> = rows.iter().map(String::as_str).collect();
        // a gate that judges the list takes what the analysis leaves: the attribute
        assert_eq!(
            answer(&text, &carried, &accounted, &[], true),
            Some(ListAnswer::Judged)
        );
        // no gate takes it: the whole list goes to the native apply, the form included
        match answer(&text, &carried, &accounted, &[], false) {
            Some(ListAnswer::Refused(reason)) => {
                assert!(reason.contains("1 name(s) not accounted for"), "{reason}");
                assert!(reason.contains(ATTRIBUTE), "{reason}");
            }
            other => panic!("{other:?}"),
        }
        // the rows the analysis does not take go to the gate too, which refuses a name that is no
        // attribute (the S1 plan: "the staged image deletes ..., which is not an attribute it removes")
        assert_eq!(
            answer(&text, &carried, &accounted[..2], &[], true),
            Some(ListAnswer::Judged)
        );
        // the analysis's reasons stop the list before any gate
        let blocker = gate::GateBlocker {
            row: FORM.to_owned(),
            reason: "the object has a table".to_owned(),
        };
        assert!(matches!(
            answer(
                &text,
                &carried,
                &accounted,
                std::slice::from_ref(&blocker),
                true
            ),
            Some(ListAnswer::Refused(_))
        ));
    }

    #[test]
    fn the_list_is_split_into_the_rows_of_the_update_and_the_rows_of_objects() {
        let (text, carried) = native_list(&format!(",\"{ATTRIBUTE}\",1"), 1);
        let entries = parse_removals(text.as_bytes()).unwrap();
        let (overlay, objects) = split_removals(&entries, &carried);
        assert_eq!(overlay.len(), 3);
        assert_eq!(objects, form_and_template_rows().to_vec());
    }

    #[test]
    fn a_lab_database_is_written_without_an_acknowledgement_and_any_other_needs_one() {
        for lab in [
            "ibcmd_rs_04_apply_dibn_20260930",
            "ibcmd_rs_05_x",
            "IBCMD_RS_04_Upper",
        ] {
            assert!(is_lab_database(lab), "{lab}");
            assert!(!write_needs_acknowledgement(lab, false), "{lab}");
        }
        for other in [
            "prod_trade",
            "ibcmd_rs_bsp_8327_native_20260919",
            "ibcmd_rs_03_old",
            "xibcmd_rs_04_a",
            "",
        ] {
            assert!(!is_lab_database(other), "{other}");
            assert!(write_needs_acknowledgement(other, false), "{other}");
            // a dry run writes nothing
            assert!(!write_needs_acknowledgement(other, true), "{other}");
        }
    }

    #[test]
    fn the_dropin_entry_never_asks_for_a_lab_acknowledgement() {
        // The drop-in reaches the apply through apply_staged_configuration, which knows nothing about the lab;
        // only the developer command (run_command) checks the name. The drop-in source must not mention the flag.
        let dropin = include_str!("../dropin/apply.rs");
        assert!(
            !dropin.contains(concat!("allow_", "non_lab")),
            "the drop-in apply asks for an acknowledgement"
        );
        assert!(
            !dropin.contains(concat!("allow-", "non-lab")),
            "the drop-in apply asks for an acknowledgement"
        );
        assert!(!dropin.contains("write_needs_acknowledgement"));
    }

    #[test]
    fn a_refusal_is_reported_by_its_type() {
        let structural = anyhow::Error::new(StructuralRefusal {
            verdict: GateVerdict::default(),
        });
        assert_eq!(
            refusal_report(&structural, "db").unwrap()["refused"],
            "needs_native_apply"
        );
        let native = anyhow::Error::new(NeedsNativeApply::repair("unfinished"));
        let report = refusal_report(&native, "db").unwrap();
        assert_eq!(report["native_command"], "repair");
        assert_eq!(report["reason"], "unfinished");
        let sessions = anyhow::Error::new(ExclusiveAccessRefused {
            database: "db".to_owned(),
            sessions: Vec::new(),
            in_transaction: true,
        });
        let report = refusal_report(&sessions, "db").unwrap();
        assert_eq!(report["refused"], "exclusive_access");
        assert_eq!(report["in_transaction"], true);
        let blind = anyhow::Error::new(ExclusiveAccessUnprovable {
            reason: "cannot prove".to_owned(),
        });
        assert_eq!(
            refusal_report(&blind, "db").unwrap()["refused"],
            "exclusive_access_unprovable"
        );
        assert!(refusal_report(&anyhow::anyhow!("plain"), "db").is_none());
    }

    fn rewrite(name: &str) -> ParamsRewrite {
        ParamsRewrite {
            file_name: name.to_owned(),
            old_data_size: 1,
            old_sha256_hex: String::new(),
            new_bytes: Vec::new(),
            set_creation: false,
        }
    }

    #[test]
    fn a_cache_row_both_the_restructuring_and_a_new_form_want_is_a_refusal() {
        // distinct rows join
        let joined = merge_params_rewrites(
            vec![rewrite("2203278d-ef4f-4f68-98f1-feb257d53ecc.si")],
            &[rewrite("ea13a2c9-0c2f-40fa-b855-710387e3271d.si")],
        )
        .unwrap();
        assert_eq!(joined.len(), 2);
        // the object registry row (1a621f0f) is rewritten by both: two steps
        let error = merge_params_rewrites(
            vec![
                rewrite("1a621f0f-5568-4183-bd9f-f6ef670e7090.si"),
                rewrite("siVersions"),
            ],
            &[
                rewrite("ea13a2c9-0c2f-40fa-b855-710387e3271d.si"),
                rewrite("1a621f0f-5568-4183-bd9f-f6ef670e7090.si"),
            ],
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Params.1a621f0f-5568-4183-bd9f-f6ef670e7090.si")
                && error.to_string().contains("in two steps"),
            "{error}"
        );
        // a restructuring alone brings its own rows
        assert_eq!(
            merge_params_rewrites(Vec::new(), &[rewrite("siVersions")])
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn a_structural_apply_that_writes_needs_a_word_about_a_backup() {
        // no restructuring: nothing is asked
        assert!(require_backup(&BackupPolicy::None, false, true).is_ok());
        // a restructuring that writes and no backup said: refused, by type, in Russian, naming both
        let error = require_backup(&BackupPolicy::None, true, true).unwrap_err();
        assert!(error.downcast_ref::<BackupRequired>().is_some(), "{error}");
        assert!(error.to_string().contains("--i-have-a-backup"), "{error}");
        assert!(error.to_string().contains("--recovery-backup"), "{error}");
        assert!(error.to_string().contains("резервная копия"), "{error}");
        let report = refusal_report(&error, "db").unwrap();
        assert_eq!(report["refused"], "backup_required");
        // said either way, or nothing kept (a rehearsal, a dry run)
        assert!(require_backup(&BackupPolicy::Acknowledged, true, true).is_ok());
        assert!(require_backup(&BackupPolicy::File(PathBuf::from("x.bak")), true, true).is_ok());
        assert!(require_backup(&BackupPolicy::None, true, false).is_ok());
    }

    #[test]
    fn the_s1_class_builds_the_s1_gate_of_the_restructure_track() {
        let sql = SqlExec::detached("no server in a unit test");
        let mut options =
            ConfigApplyOptions::new("db", MssqlNativePlatformProfile::Platform8_3_27_2214);
        options.allow_restructure = Some(AllowRestructure::S1);
        let gate = structural_gate(&sql, &options).expect("the S1 gate is built");
        assert_eq!(gate.name(), "s1");
        // The gate judges the stage's `deleted` list of removed attributes; the others do not.
        assert!(gate.judges_deleted_row());
        options.allow_restructure = None;
        assert!(
            !structural_gate(&sql, &options)
                .unwrap()
                .judges_deleted_row()
        );
    }

    #[test]
    fn the_backup_is_named_in_the_report() {
        let taken = BackupRecord {
            kind: "file",
            path: Some("F:/b.bak".to_owned()),
            seconds: Some(1.5),
        };
        let json = serde_json::to_value(&taken).unwrap();
        assert_eq!(json["kind"], "file");
        assert_eq!(json["path"], "F:/b.bak");
        assert_eq!(json["seconds"], 1.5);
        let said = serde_json::to_value(BackupRecord {
            kind: "acknowledged",
            path: None,
            seconds: None,
        })
        .unwrap();
        assert_eq!(said["kind"], "acknowledged");
        assert!(said["path"].is_null());
    }
}
