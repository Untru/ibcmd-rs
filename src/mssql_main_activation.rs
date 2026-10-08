//! Fail-closed main-configuration publication plans for MSSQL 8.3.27.
//!
//! This module only models and renders the row transition evidenced for the
//! `Config`/`ConfigSave`/`Params` storage boundary.  It deliberately does not
//! claim that an online publication invalidates already-running server caches.

use flate2::read::DeflateDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Read;
use uuid::Uuid;

use crate::mssql_platform_profile::{OwnRasProcess, exclusive_session_gate};

pub(crate) const MAX_ROW_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_PLAN_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const MAX_ROWS: usize = 128;
const MAX_INFLATED_VERSIONS_BYTES: usize = 16 * 1024 * 1024;
const SERVICE_NAMES: [&str; 3] = ["root", "version", "versions"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MainActivationMode {
    Exclusive,
    /// Publish while sessions remain connected. Existing sessions retain their
    /// loaded generation; sessions opened afterwards use the new generation.
    Online,
    /// Default: legacy two-cycle SQL reconnection readiness. The explicit live
    /// checkpoint renderer splits cycle 1/continue and accepts idle RAS sessions only.
    /// Warm client-side code still needs restart; neither route promises zero errors.
    Live,
    /// Publish ordinary rows while sessions are connected. The caller must
    /// hand the dedicated 1C worker process off after the SQL commit.
    Worker,
}

/// Who carries out a promotion (#408 step 2, `docs/apply/online-activation.md` 6.6).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MainActivationExecutor {
    /// The transaction this module renders ([`render_main_activation_sql`]): `online`, `live`, `worker`, and
    /// `exclusive` where the built-in SQL client is not there (`--sqlcmd`).
    #[default]
    Script,
    /// `mssql_config_apply`, which folds the rows of earlier online generations as the native apply does:
    /// the `exclusive` mode.
    ConfigApply,
}

impl MainActivationExecutor {
    /// The executor of a mode: the config apply for `exclusive` where it can run (`config_apply_available`: the
    /// built-in SQL client), the script for the rest.
    pub fn for_mode(mode: MainActivationMode, config_apply_available: bool) -> Self {
        if mode == MainActivationMode::Exclusive && config_apply_available {
            Self::ConfigApply
        } else {
            Self::Script
        }
    }

    /// Whether the executor folds the rows of online generations into the ordinary ones for this mode (only the
    /// config apply does, and only for `exclusive`), so that a promotion does not lose them.
    pub fn folds_online_generations(self, mode: MainActivationMode) -> bool {
        self == Self::ConfigApply && mode == MainActivationMode::Exclusive
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MainStorageRow {
    pub file_name: String,
    pub part_no: i32,
    pub creation: String,
    pub modified: String,
    pub attributes: i32,
    pub data_size: u64,
    pub binary_data: Vec<u8>,
}

impl MainStorageRow {
    pub fn sha256(&self) -> [u8; 32] {
        Sha256::digest(&self.binary_data).into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MainActivationSnapshot {
    /// Exact ordinary Config rows corresponding to every staged FileName.
    pub config_rows: Vec<MainStorageRow>,
    /// Existing Config.DynamicallyUpdated row, if present.
    pub config_dynamically_updated: Option<MainStorageRow>,
    /// Existing Params.DynamicallyUpdated row, if present.
    pub params_dynamically_updated: Option<MainStorageRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MainActivationRecoverySnapshot {
    pub old_generation: String,
    pub new_generation: String,
    pub overwritten_config_rows: Vec<MainStorageRow>,
    /// Ordinary preimages asserted by an online publication but left in place.
    /// Omitted when empty so historical non-no-op LIVE token bytes stay exact.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retained_config_rows: Vec<MainStorageRow>,
    pub prior_config_dynamically_updated: Option<MainStorageRow>,
    pub prior_params_dynamically_updated: Option<MainStorageRow>,
    pub staged_rows: Vec<MainStorageRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MainActivationDryRunReport {
    pub mode: MainActivationMode,
    pub old_generation: String,
    pub new_generation: String,
    pub changed_targets: Vec<String>,
    pub touched_tables: Vec<String>,
    pub staged_rows: usize,
    pub staged_bytes: usize,
    pub no_op: bool,
    pub online_protocol_verified: bool,
    pub existing_sessions_retain_generation: bool,
    pub live_session_switch_expected: bool,
    pub requires_tail_log_artifact: bool,
    pub recovery_token: String,
    /// New Config row names, excluding the root/version replacements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dynamic_aliases: Vec<String>,
    /// Fully qualified marker names inserted or updated by this publication.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dynamic_markers: Vec<String>,
    /// `exclusive` only: the worker processes whose idle `1CV83 Server` sessions
    /// the session gate leaves out because the tool's own RAS verification
    /// opened them (#409 F-3).
    #[serde(default)]
    pub own_ras_processes: Vec<OwnRasProcess>,
    /// Who carries the promotion out (`config_apply`: the report of that run is `config_apply` of the command's report).
    #[serde(default)]
    pub executor: MainActivationExecutor,
    /// The `Config` rows the promotion's script writes, under the names it
    /// writes them: an online run's `_dynupdate_` aliases, the ordinary names
    /// otherwise (#409 F-13). Empty when the own apply carries it out: that
    /// run names its own rows.
    #[serde(default)]
    pub published_config_rows: Vec<String>,
    /// What the promotion's script does to the two `DynamicallyUpdated`
    /// markers: `Config.DynamicallyUpdated: written|deleted` (#409 F-13);
    /// empty as `published_config_rows` is.
    #[serde(default)]
    pub dynamic_marker_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainActivationPlan {
    mode: MainActivationMode,
    ordinary_generation: Uuid,
    old_generation: Uuid,
    new_generation: Uuid,
    dynamic_history: Vec<Uuid>,
    staged_rows: Vec<MainStorageRow>,
    active_rows: Vec<MainStorageRow>,
    changed_targets: Vec<String>,
    config_marker: Option<MainStorageRow>,
    params_marker: Option<MainStorageRow>,
    no_op: bool,
    recovery: MainActivationRecoverySnapshot,
    own_ras_processes: Vec<OwnRasProcess>,
    /// Writes the caller's apply makes in the same transaction (see [`Self::with_parity_sql`]).
    parity_sql: Option<String>,
    /// Internal caller's exact semantic inventory assertions, also required before no-op cleanup.
    precondition_sql: Option<String>,
    /// T-SQL that declares `@now`, the timestamp as the platform writes it (see
    /// [`Self::with_platform_timestamps`]).
    platform_timestamps: Option<String>,
    /// Staged rows the publication consumes without publishing them (see [`Self::with_consumed_stage_rows`]).
    consumed_stage_rows: Vec<MainStorageRow>,
    executor: MainActivationExecutor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainActivationScript {
    pub sql: String,
    pub report: MainActivationDryRunReport,
    pub recovery: MainActivationRecoverySnapshot,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MainActivationError {
    SafetyGate(String),
    InvalidRow(String),
    StructuralTarget(String),
    Versions(String),
    Limit(String),
}

impl std::fmt::Display for MainActivationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (prefix, detail) = match self {
            Self::SafetyGate(detail) => ("write safety gate rejected activation", detail),
            Self::InvalidRow(detail) => ("invalid activation row", detail),
            Self::StructuralTarget(detail) => ("unsupported or structural staged target", detail),
            Self::Versions(detail) => ("invalid versions blob", detail),
            Self::Limit(detail) => ("activation input exceeds a safety limit", detail),
        };
        write!(formatter, "{prefix}: {detail}")
    }
}

impl std::error::Error for MainActivationError {}

impl MainActivationPlan {
    pub fn mode(&self) -> MainActivationMode {
        self.mode
    }

    pub fn old_generation(&self) -> Uuid {
        self.old_generation
    }

    pub fn new_generation(&self) -> Uuid {
        self.new_generation
    }

    pub fn is_no_op(&self) -> bool {
        self.no_op
    }

    /// Who carries the promotion out: `mssql_config_apply` for `exclusive` (unless the plan was made for the
    /// legacy runner), the rendered script for the rest. A no-op is always the script's: it only empties
    /// `ConfigSave`.
    pub fn executor(&self) -> MainActivationExecutor {
        self.executor
    }

    /// Whether the promotion is handed to `mssql_config_apply`: it changes something and the plan's executor is
    /// the apply.
    pub fn is_carried_out_by_config_apply(&self) -> bool {
        self.executor == MainActivationExecutor::ConfigApply && !self.no_op
    }

    pub fn recovery(&self) -> &MainActivationRecoverySnapshot {
        &self.recovery
    }

    /// The dynamic generations the database holds before this publication, oldest first.
    pub fn dynamic_history(&self) -> &[Uuid] {
        &self.dynamic_history
    }

    /// The rows an `online` publication writes beside the active ones, by the name they get
    /// (`<name>_dynupdate_<generation>`, the storage suffix last). `root` and `version` are
    /// replaced in place and are not among them.
    pub fn alias_rows(&self) -> Vec<String> {
        let generation = self.new_generation.hyphenated().to_string();
        self.staged_rows
            .iter()
            .filter(|row| !matches!(row.file_name.as_str(), "root" | "version"))
            .map(|row| match row.file_name.as_str() {
                "versions" => format!("versions_dynupdate_{generation}"),
                name => dynamic_alias(name, &generation),
            })
            .collect()
    }

    /// The worker processes the exclusive session gate leaves out (see [`Self::with_own_ras_processes`]).
    pub fn own_ras_processes(&self) -> &[OwnRasProcess] {
        &self.own_ras_processes
    }

    /// The plan with the worker processes whose idle SQL sessions the
    /// `exclusive` session gate leaves out: the sessions the tool's own RAS
    /// verification made the cluster open (#409 F-3). Without any, the gate
    /// counts every session.
    pub fn with_own_ras_processes(mut self, own: Vec<OwnRasProcess>) -> Self {
        self.own_ras_processes = own;
        self
    }

    /// The plan with T-SQL the transaction runs after the rows are published and before the stage
    /// is consumed (`ConfigSave` still holds the staged rows): the writes an apply makes besides
    /// moving rows -- the change registrations and `Files.MobileVersions.dat` -- which the
    /// exclusive apply renders and the dynamic apply of the drop-in shares
    /// (`mssql_config_apply::sqlgen::render_parity_writes`). Only an `online` publication takes
    /// it, and an unchanged stage (`no_op`) writes nothing, so nothing of it either.
    pub fn with_parity_sql(mut self, sql: String) -> Self {
        self.parity_sql = Some(sql).filter(|sql| !sql.trim().is_empty());
        self
    }

    /// Internal, validated assertions executed under the same serializable transaction and row
    /// locks as publication, after the exact stage/markers and before even no-op consumption.
    /// This hook is for CAS assertions, not writes or user-supplied SQL.
    pub(crate) fn with_precondition_sql(mut self, sql: String) -> Self {
        self.precondition_sql = Some(sql).filter(|sql| !sql.trim().is_empty());
        self
    }

    /// The plan whose `online` publication stamps the `DynamicallyUpdated` markers the way the
    /// platform does -- local time shifted by the infobase's year offset, the `@now` the
    /// `declarations` declare -- instead of UTC (the default, which the source-driven online apply
    /// has always written). The declarations are run at the start of the transition, so the same
    /// `@now` serves the parity writes ([`Self::with_parity_sql`]).
    pub fn with_platform_timestamps(mut self, declarations: String) -> Self {
        self.platform_timestamps = Some(declarations).filter(|sql| !sql.trim().is_empty());
        self
    }

    /// The plan for a stage that holds rows besides the ones published: the caller has judged them (the
    /// empty `deleted` list of an import stage, or a measured pending list the caller explicitly
    /// copies through its parity hook). The core transition neither publishes nor acts on them.
    /// They are part of the exact stage
    /// the transaction asserts and go with `ConfigSave` when it is emptied; the plan validated and
    /// published only the rows it was prepared with.
    pub fn with_consumed_stage_rows(mut self, rows: Vec<MainStorageRow>) -> Self {
        self.consumed_stage_rows = rows;
        self
    }

    /// How many rows the stage holds in all: the published and the consumed.
    fn stage_row_count(&self) -> usize {
        self.staged_rows.len() + self.consumed_stage_rows.len()
    }

    pub fn dry_run_report(&self) -> MainActivationDryRunReport {
        let recovery_json = serde_json::to_vec(&self.recovery)
            .expect("serializing a bounded recovery snapshot cannot fail");
        MainActivationDryRunReport {
            mode: self.mode,
            old_generation: self.old_generation.hyphenated().to_string(),
            new_generation: self.new_generation.hyphenated().to_string(),
            changed_targets: self.changed_targets.clone(),
            // Every mode replaces Config rows from ConfigSave and writes or
            // deletes the Params marker; a no-op only empties ConfigSave.
            touched_tables: if self.no_op {
                vec!["ConfigSave"]
            } else {
                vec!["Config", "ConfigSave", "Params"]
            }
            .into_iter()
            .map(str::to_owned)
            .collect(),
            staged_rows: self.staged_rows.len(),
            staged_bytes: self
                .staged_rows
                .iter()
                .map(|row| row.binary_data.len())
                .sum(),
            no_op: self.no_op,
            online_protocol_verified: self.mode == MainActivationMode::Online,
            existing_sessions_retain_generation: self.mode == MainActivationMode::Online,
            live_session_switch_expected: matches!(
                self.mode,
                MainActivationMode::Live | MainActivationMode::Worker
            ),
            requires_tail_log_artifact: self.mode == MainActivationMode::Live && !self.no_op,
            recovery_token: hex(&Sha256::digest(recovery_json)),
            dynamic_aliases: if self.mode == MainActivationMode::Online && !self.no_op {
                self.alias_rows()
            } else {
                Vec::new()
            },
            dynamic_markers: if self.mode == MainActivationMode::Online && !self.no_op {
                vec![
                    "Config.DynamicallyUpdated".to_owned(),
                    "Params.DynamicallyUpdated".to_owned(),
                ]
            } else {
                Vec::new()
            },
            own_ras_processes: self.own_ras_processes.clone(),
            executor: self.executor,
            published_config_rows: if self.no_op
                || self.executor == MainActivationExecutor::ConfigApply
            {
                Vec::new()
            } else {
                let generation = self.new_generation.hyphenated().to_string();
                self.staged_rows
                    .iter()
                    .map(|row| published_file_name(self.mode, &row.file_name, &generation))
                    .collect()
            },
            dynamic_marker_actions: if self.no_op
                || self.executor == MainActivationExecutor::ConfigApply
            {
                Vec::new()
            } else if self.mode == MainActivationMode::Online {
                vec![
                    "Config.DynamicallyUpdated: written".to_owned(),
                    "Params.DynamicallyUpdated: written".to_owned(),
                ]
            } else {
                [
                    ("Config", self.config_marker.is_some()),
                    ("Params", self.params_marker.is_some()),
                ]
                .into_iter()
                .filter(|(_, present)| *present)
                .map(|(table, _)| format!("{table}.DynamicallyUpdated: deleted"))
                .collect()
            },
        }
    }
}

/// Builds an immutable plan only from an exact ConfigSave image and its
/// matching ordinary Config/Params snapshot, for the transaction this module renders
/// ([`MainActivationExecutor::Script`]).
pub fn prepare_main_activation(
    mode: MainActivationMode,
    staged_rows: Vec<MainStorageRow>,
    snapshot: MainActivationSnapshot,
    allowed_non_structural_targets: &[String],
    allow_non_lab: bool,
) -> Result<MainActivationPlan, MainActivationError> {
    prepare_main_activation_for(
        MainActivationExecutor::Script,
        mode,
        staged_rows,
        snapshot,
        allowed_non_structural_targets,
        allow_non_lab,
    )
}

/// [`prepare_main_activation`] for a given executor. The config apply carries out `exclusive` only; the script
/// carries out every mode, `exclusive` included where the built-in SQL client is not there.
pub fn prepare_main_activation_for(
    executor: MainActivationExecutor,
    mode: MainActivationMode,
    staged_rows: Vec<MainStorageRow>,
    snapshot: MainActivationSnapshot,
    allowed_non_structural_targets: &[String],
    allow_non_lab: bool,
) -> Result<MainActivationPlan, MainActivationError> {
    if executor == MainActivationExecutor::ConfigApply && mode != MainActivationMode::Exclusive {
        return Err(MainActivationError::SafetyGate(format!(
            "the {} mode is not carried out by the config apply",
            mode_name(mode)
        )));
    }
    if !allow_non_lab {
        return Err(MainActivationError::SafetyGate(
            "--allow-non-lab acknowledgement is required".to_owned(),
        ));
    }
    validate_rows("ConfigSave", &staged_rows, false)?;
    validate_rows("Config", &snapshot.config_rows, false)?;
    if staged_rows.len() > MAX_ROWS {
        return Err(MainActivationError::Limit(format!(
            "{} staged rows exceeds {MAX_ROWS}",
            staged_rows.len()
        )));
    }
    let staged_bytes = staged_rows.iter().try_fold(0usize, |total, row| {
        total
            .checked_add(row.binary_data.len())
            .ok_or_else(|| MainActivationError::Limit("staged byte count overflow".to_owned()))
    })?;
    if staged_bytes > MAX_PLAN_BYTES {
        return Err(MainActivationError::Limit(format!(
            "{staged_bytes} staged bytes exceeds {MAX_PLAN_BYTES}"
        )));
    }

    validate_optional_marker("Config", snapshot.config_dynamically_updated.as_ref())?;
    validate_optional_marker("Params", snapshot.params_dynamically_updated.as_ref())?;

    let staged = index_rows(&staged_rows)?;
    let active = index_rows(&snapshot.config_rows)?;
    for service in SERVICE_NAMES {
        require_single_part(&staged, service, "ConfigSave")?;
        require_single_part(&active, service, "Config")?;
    }
    if staged.len() != active.len() || staged.keys().ne(active.keys()) {
        return Err(MainActivationError::InvalidRow(
            "Config snapshot must contain exactly the ordinary rows named by ConfigSave".to_owned(),
        ));
    }

    let allowed = allowed_non_structural_targets
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut changed_targets = Vec::new();
    for row in &staged_rows {
        if SERVICE_NAMES.contains(&row.file_name.as_str()) {
            continue;
        }
        if !is_existing_body_target(&row.file_name) || !allowed.contains(row.file_name.as_str()) {
            return Err(MainActivationError::StructuralTarget(row.file_name.clone()));
        }
        changed_targets.push(row.file_name.clone());
    }
    changed_targets.sort();
    changed_targets.dedup();
    if changed_targets.is_empty() {
        return Err(MainActivationError::StructuralTarget(
            "the stage contains no admitted existing module/form body rows".to_owned(),
        ));
    }

    let equal_to_active = staged.iter().all(|(key, row)| {
        active
            .get(key)
            .is_some_and(|current| current.binary_data == row.binary_data)
    });
    // The ordinary rows are not the configuration of a database that holds online generations: a stage that equals
    // them (an online change taken back to the original text) still changes the configuration, so a promotion is a
    // no-op only on a database without markers. The config apply folds the generations; the script's promotion
    // refuses such a database (#408 step 1). The online mode compares with the generation it extends and is never
    // refused for a stage that changes nothing.
    let no_op = equal_to_active
        && !(mode != MainActivationMode::Online
            && (snapshot.config_dynamically_updated.is_some()
                || snapshot.params_dynamically_updated.is_some()));
    let PublicationState {
        ordinary_generation,
        old_generation,
        dynamic_history,
    } = check_publication_state(
        mode,
        executor,
        !no_op,
        &active[&("versions".to_owned(), 0)].binary_data,
        snapshot.config_dynamically_updated.as_ref(),
        snapshot.params_dynamically_updated.as_ref(),
    )?;
    let new_generation =
        generation_from_versions(&staged[&("versions".to_owned(), 0)].binary_data)?;
    if !no_op && old_generation == new_generation {
        return Err(MainActivationError::Versions(
            "changed payload reuses the active generation UUID".to_owned(),
        ));
    }

    let (overwritten, retained) = snapshot.config_rows.iter().cloned().partition(|row| {
        !no_op
            && (mode != MainActivationMode::Online
                || matches!(row.file_name.as_str(), "root" | "version"))
    });
    let recovery = MainActivationRecoverySnapshot {
        old_generation: old_generation.hyphenated().to_string(),
        new_generation: new_generation.hyphenated().to_string(),
        overwritten_config_rows: overwritten,
        retained_config_rows: retained,
        prior_config_dynamically_updated: snapshot.config_dynamically_updated.clone(),
        prior_params_dynamically_updated: snapshot.params_dynamically_updated.clone(),
        staged_rows: staged_rows.clone(),
    };
    Ok(MainActivationPlan {
        mode,
        ordinary_generation,
        old_generation,
        new_generation,
        dynamic_history,
        staged_rows,
        active_rows: snapshot.config_rows,
        changed_targets,
        config_marker: snapshot.config_dynamically_updated,
        params_marker: snapshot.params_dynamically_updated,
        no_op,
        recovery,
        own_ras_processes: Vec::new(),
        parity_sql: None,
        precondition_sql: None,
        platform_timestamps: None,
        consumed_stage_rows: Vec::new(),
        executor,
    })
}

/// What the ordinary `versions` row and the `DynamicallyUpdated` markers of a
/// database say about how it is published.
struct PublicationState {
    ordinary_generation: Uuid,
    old_generation: Uuid,
    dynamic_history: Vec<Uuid>,
}

/// The checks of the publication state that need no staged row: the plan
/// makes them ([`prepare_main_activation`]) and so does the preflight
/// ([`preflight_publication`]), so the two cannot disagree.
///
/// `changes` is whether the promotion writes anything.
fn check_publication_state(
    mode: MainActivationMode,
    executor: MainActivationExecutor,
    changes: bool,
    ordinary_versions: &[u8],
    config_marker: Option<&MainStorageRow>,
    params_marker: Option<&MainStorageRow>,
) -> Result<PublicationState, MainActivationError> {
    // #408 (finding F-4 of #344): a promotion by the script replaces only the staged rows and
    // deletes both markers, so what earlier online generations published (their
    // `_dynupdate_` rows) silently stops being the configuration. Refuse it before anything
    // is written. The config apply folds the aliases as the native apply does (step 2), so the
    // promotion it carries out (`exclusive`) is not refused.
    if changes
        && mode != MainActivationMode::Online
        && !executor.folds_online_generations(mode)
        && (config_marker.is_some() || params_marker.is_some())
    {
        return Err(MainActivationError::SafetyGate(online_history_refusal(
            mode,
            "Config/Params `DynamicallyUpdated` markers are present",
        )));
    }

    let ordinary_generation = generation_from_versions(ordinary_versions)?;
    let (old_generation, dynamic_history) =
        active_generation_from_markers(ordinary_generation, config_marker, params_marker)?;
    Ok(PublicationState {
        ordinary_generation,
        old_generation,
        dynamic_history,
    })
}

/// Refuses, before anything is staged, a promotion that changes something and
/// that the state of the database alone rules out: an ordinary mode on a
/// database that holds online generations (#408), markers that do not agree
/// with each other or with the ordinary `versions` row.
///
/// `ordinary_versions` and the markers are the rows **as stored**, not the
/// configuration an online update published (#409 F-2: an apply was refused
/// only after the stage, on markers it had read through the export's view, and
/// left `ConfigSave` filled). The plan the activation builds from the staged
/// rows makes the same checks ([`prepare_main_activation`]).
pub fn preflight_publication(
    executor: MainActivationExecutor,
    mode: MainActivationMode,
    ordinary_versions: &MainStorageRow,
    config_marker: Option<&MainStorageRow>,
    params_marker: Option<&MainStorageRow>,
) -> Result<(), MainActivationError> {
    validate_rows("Config", std::slice::from_ref(ordinary_versions), false)?;
    validate_optional_marker("Config", config_marker)?;
    validate_optional_marker("Params", params_marker)?;
    check_publication_state(
        mode,
        executor,
        true,
        &ordinary_versions.binary_data,
        config_marker,
        params_marker,
    )
    .map(|_| ())
}

/// Refuses, before anything is staged, a tail-log argument the activation
/// would refuse after it: `live` needs one (`will_write`: a real run) and no
/// other mode takes one.
pub fn preflight_tail_log(
    mode: MainActivationMode,
    tail_log_output: Option<&str>,
    will_write: bool,
) -> Result<(), MainActivationError> {
    match (mode, tail_log_output) {
        (MainActivationMode::Live, Some(path)) => validate_tail_log_output(path).map(|_| ()),
        (MainActivationMode::Live, None) if will_write => Err(MainActivationError::SafetyGate(
            "--tail-log-output is required for live activation".to_owned(),
        )),
        (_, None) => Ok(()),
        (_, Some(_)) => Err(MainActivationError::SafetyGate(
            "--tail-log-output is only valid for live activation".to_owned(),
        )),
    }
}

/// Refuses, before anything is staged, `--interrupt-sessions` for a mode that does not interrupt sessions: it is the operator's
/// acceptance of what `live` does (#409 F-10) and means nothing elsewhere.
pub fn preflight_interrupt_sessions(
    mode: MainActivationMode,
    interrupt_sessions: bool,
) -> Result<(), MainActivationError> {
    if interrupt_sessions && mode != MainActivationMode::Live {
        return Err(MainActivationError::SafetyGate(
            "--interrupt-sessions is only valid for live activation".to_owned(),
        ));
    }
    Ok(())
}

/// The script of a mode, with the live switch's sessions gate as it stands when nobody accepted an interruption
/// ([`render_main_activation_sql_with`]).
pub fn render_main_activation_sql(
    database: &str,
    plan: &MainActivationPlan,
    tail_log_output: Option<&str>,
) -> Result<MainActivationScript, MainActivationError> {
    render_main_activation_sql_with(database, plan, tail_log_output, false)
}

/// A no-op still clears ConfigSave: it shares the pending-cycle guard with default LIVE.
/// The non-noop checkpoint instead receives the identity-pinned phase-1 wrapper.
pub fn protect_live_script(database: &str, checkpoint: bool, script: &mut MainActivationScript) {
    if script.report.mode == MainActivationMode::Live && (!checkpoint || script.report.no_op) {
        script.sql = crate::mssql_live_continue::guard_pending_live(database, &script.sql);
    }
}

/// The script of a mode. `interrupt_sessions` is the operator's acceptance that the live switch rolls back the open work of the
/// sessions of the database (`--interrupt-sessions`, live only; #409 F-10): without it the script refuses to start, and rolls the
/// promotion back if such work appears before its `COMMIT`.
pub fn render_main_activation_sql_with(
    database: &str,
    plan: &MainActivationPlan,
    tail_log_output: Option<&str>,
    interrupt_sessions: bool,
) -> Result<MainActivationScript, MainActivationError> {
    render_main_activation_sql_internal(database, plan, tail_log_output, interrupt_sessions, false)
}

pub fn render_main_activation_checkpoint(
    database: &str,
    plan: &MainActivationPlan,
    tail_log_output: Option<&str>,
    interrupt_sessions: bool,
) -> Result<MainActivationScript, MainActivationError> {
    if plan.mode != MainActivationMode::Live {
        return Err(MainActivationError::SafetyGate(
            "live checkpoint requires live mode".to_owned(),
        ));
    }
    let mut script = render_main_activation_sql_internal(
        database,
        plan,
        tail_log_output,
        interrupt_sessions,
        true,
    )?;
    script.report.live_session_switch_expected = false;
    Ok(script)
}

fn render_main_activation_sql_internal(
    database: &str,
    plan: &MainActivationPlan,
    tail_log_output: Option<&str>,
    interrupt_sessions: bool,
    checkpoint: bool,
) -> Result<MainActivationScript, MainActivationError> {
    if plan.is_carried_out_by_config_apply() {
        return Err(MainActivationError::SafetyGate(
            "an exclusive promotion is carried out by mssql_config_apply, not by a script of this module".to_owned(),
        ));
    }
    let database_name = database;
    let database_literal = quote_string(database_name);
    let database = quote_ident(database_name)?;
    let live_tail = match (plan.mode, plan.no_op, tail_log_output) {
        (MainActivationMode::Live, false, Some(path)) => Some(validate_tail_log_output(path)?),
        (MainActivationMode::Live, false, None) => {
            return Err(MainActivationError::SafetyGate(
                "--tail-log-output is required for live activation".to_owned(),
            ));
        }
        (MainActivationMode::Live, true, _) => None,
        (_, _, Some(_)) => {
            return Err(MainActivationError::SafetyGate(
                "--tail-log-output is only valid for live activation".to_owned(),
            ));
        }
        (_, _, None) => None,
    };
    let mut sql = String::new();
    writeln!(sql, "SET NOCOUNT ON;").unwrap();
    writeln!(sql, "SET XACT_ABORT ON;").unwrap();
    if let Some(tail) = live_tail.as_deref() {
        // The gate of the live switch, before the transaction: the database, its recovery model and state, the tail file, the log
        // backup chain, the directory and the account's right to write there, the sessions with open work (#409 F-9, F-10).
        sql.push_str(&crate::mssql_live_gate::render_live_gate(
            &crate::mssql_live_gate::LiveGateSettings {
                database: database_name,
                tail_log_output: tail,
                interrupt_sessions,
                probe: true,
            },
        )?);
    }
    writeln!(sql, "USE {database};").unwrap();
    writeln!(sql, "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;").unwrap();
    writeln!(sql, "BEGIN TRY").unwrap();
    writeln!(sql, "BEGIN TRANSACTION;").unwrap();
    writeln!(sql, "DECLARE @LockResult int;").unwrap();
    writeln!(sql, "EXEC @LockResult = sys.sp_getapplock @Resource=N'ibcmd-rs:main-activation', @LockMode='Exclusive', @LockOwner='Transaction', @LockTimeout=0;").unwrap();
    writeln!(
        sql,
        "IF @LockResult < 0 THROW 57200, 'main activation application lock is busy', 1;"
    )
    .unwrap();

    let exact_stage: Vec<MainStorageRow> = plan
        .staged_rows
        .iter()
        .chain(&plan.consumed_stage_rows)
        .cloned()
        .collect();
    render_expected_table(&mut sql, "ExpectedStage", &exact_stage);
    render_expected_table(&mut sql, "ExpectedActive", &plan.active_rows);
    render_exact_set_assertion(&mut sql, "ConfigSave", "ExpectedStage", 57201);
    render_selected_assertion(&mut sql, "Config", "ExpectedActive", 57202);
    render_marker_assertion(&mut sql, "Config", plan.config_marker.as_ref(), 57203);
    render_marker_assertion(&mut sql, "Params", plan.params_marker.as_ref(), 57204);
    if let Some(precondition) = &plan.precondition_sql {
        sql.push_str(precondition);
        if !precondition.ends_with('\n') {
            sql.push('\n');
        }
    }
    if !plan.no_op && plan.mode != MainActivationMode::Online {
        render_online_history_assertion(&mut sql, plan.mode, 57208);
    }

    if plan.no_op {
        writeln!(sql, "DELETE FROM dbo.ConfigSave;").unwrap();
        writeln!(
            sql,
            "IF @@ROWCOUNT <> {} THROW 57205, 'ConfigSave cleanup drifted', 1;",
            plan.stage_row_count()
        )
        .unwrap();
        writeln!(sql, "IF EXISTS (SELECT 1 FROM dbo.ConfigSave) THROW 57206, 'ConfigSave postcondition failed', 1;").unwrap();
        writeln!(sql, "COMMIT TRANSACTION;").unwrap();
        render_catch(&mut sql, None);
        return Ok(MainActivationScript {
            sql,
            report: plan.dry_run_report(),
            recovery: plan.recovery.clone(),
        });
    }

    if plan.mode == MainActivationMode::Online
        && let Some(declarations) = &plan.platform_timestamps
    {
        sql.push_str(declarations);
        if !declarations.ends_with('\n') {
            sql.push('\n');
        }
    }
    match plan.mode {
        MainActivationMode::Exclusive => render_ordinary_transition(&mut sql, plan, true),
        MainActivationMode::Online => render_online_transition(&mut sql, plan),
        MainActivationMode::Live | MainActivationMode::Worker => {
            render_ordinary_transition(&mut sql, plan, false)
        }
    }
    if plan.mode == MainActivationMode::Online
        && let Some(parity) = &plan.parity_sql
    {
        writeln!(sql, "-- the writes of the apply besides the rows").unwrap();
        sql.push_str(parity);
        if !parity.ends_with('\n') {
            sql.push('\n');
        }
    }
    writeln!(sql, "DELETE FROM dbo.ConfigSave;").unwrap();
    writeln!(
        sql,
        "IF @@ROWCOUNT <> {} THROW 57220, 'ConfigSave cleanup drifted', 1;",
        plan.stage_row_count()
    )
    .unwrap();
    render_postconditions(&mut sql, plan);
    writeln!(sql, "IF EXISTS (SELECT 1 FROM dbo.ConfigSave) THROW 57221, 'ConfigSave postcondition failed', 1;").unwrap();
    if live_tail.is_some() {
        // work that started after the gate ran rolls the promotion back, instead of being rolled back by the interruption
        sql.push_str(
            &crate::mssql_live_gate::render_sessions_check_in_transaction(
                database_name,
                interrupt_sessions,
            ),
        );
    }
    writeln!(sql, "COMMIT TRANSACTION;").unwrap();
    render_catch(&mut sql, None);
    if let Some(tail) = live_tail.as_deref() {
        if checkpoint {
            render_live_recovery(
                &mut sql,
                &database,
                &database_literal,
                tail,
                &plan.dry_run_report().recovery_token,
            );
        } else {
            render_legacy_live_recovery(&mut sql, &database, &database_literal, tail);
        }
    }

    if sql.len() > MAX_PLAN_BYTES {
        return Err(MainActivationError::Limit(format!(
            "rendered SQL uses {} bytes; maximum is {MAX_PLAN_BYTES}",
            sql.len()
        )));
    }
    Ok(MainActivationScript {
        sql,
        report: plan.dry_run_report(),
        recovery: plan.recovery.clone(),
    })
}

fn render_ordinary_transition(
    sql: &mut String,
    plan: &MainActivationPlan,
    require_no_sessions: bool,
) {
    if require_no_sessions {
        writeln!(
            sql,
            "{}",
            exclusive_session_gate(
                57209,
                "exclusive activation requires no other database sessions",
                &plan.own_ras_processes
            )
        )
        .unwrap();
    }
    for row in &plan.staged_rows {
        let name = quote_string(&row.file_name);
        writeln!(
            sql,
            "DELETE FROM dbo.Config WHERE FileName=N'{name}' AND PartNo={};",
            row.part_no
        )
        .unwrap();
        writeln!(sql, "INSERT dbo.Config (FileName,Creation,Modified,Attributes,DataSize,BinaryData,PartNo) SELECT FileName,Creation,Modified,Attributes,DataSize,BinaryData,PartNo FROM dbo.ConfigSave WHERE FileName=N'{name}' AND PartNo={};", row.part_no).unwrap();
        writeln!(
            sql,
            "IF @@ROWCOUNT <> 1 THROW 57210, 'exclusive promotion source drifted', 1;"
        )
        .unwrap();
    }
    render_marker_delete(sql, "Config", plan.config_marker.is_some(), 57215);
    render_marker_delete(sql, "Params", plan.params_marker.is_some(), 57216);
}

fn render_online_transition(sql: &mut String, plan: &MainActivationPlan) {
    let generation = plan.new_generation.hyphenated().to_string();
    for row in &plan.staged_rows {
        let source = quote_string(&row.file_name);
        let destination = quote_string(&published_file_name(
            MainActivationMode::Online,
            &row.file_name,
            &generation,
        ));
        if matches!(row.file_name.as_str(), "root" | "version") {
            writeln!(
                sql,
                "DELETE FROM dbo.Config WHERE FileName=N'{destination}' AND PartNo={};",
                row.part_no
            )
            .unwrap();
        } else {
            writeln!(sql, "IF EXISTS (SELECT 1 FROM dbo.Config WITH (UPDLOCK,HOLDLOCK) WHERE FileName=N'{destination}' AND PartNo={}) THROW 57211, 'dynamic alias already exists', 1;", row.part_no).unwrap();
        }
        writeln!(sql, "INSERT dbo.Config (FileName,Creation,Modified,Attributes,DataSize,BinaryData,PartNo) SELECT N'{destination}',Creation,Modified,Attributes,DataSize,BinaryData,PartNo FROM dbo.ConfigSave WHERE FileName=N'{source}' AND PartNo={};", row.part_no).unwrap();
        writeln!(
            sql,
            "IF @@ROWCOUNT <> 1 THROW 57212, 'online promotion source drifted', 1;"
        )
        .unwrap();
    }
    let (config_payload, params_payload) = next_dynamic_marker_payloads(plan);
    let stamp = if plan.platform_timestamps.is_some() {
        "@now"
    } else {
        "DATEADD(year,2000,SYSUTCDATETIME())"
    };
    render_marker_upsert(sql, "Config", &config_payload, 57213, stamp);
    render_marker_upsert(sql, "Params", &params_payload, 57214, stamp);
}

fn render_expected_table(sql: &mut String, variable: &str, rows: &[MainStorageRow]) {
    writeln!(sql, "DECLARE @{variable} TABLE (FileName nvarchar(4000) NOT NULL, PartNo int NOT NULL, Creation varchar(27) NOT NULL, Modified varchar(27) NOT NULL, Attributes int NOT NULL, DataSize bigint NOT NULL, ByteLength bigint NOT NULL, Digest varbinary(32) NOT NULL, PRIMARY KEY(FileName,PartNo));").unwrap();
    for row in rows {
        writeln!(
            sql,
            "INSERT @{variable} VALUES (N'{}',{},'{}','{}',{},{},{},0x{});",
            quote_string(&row.file_name),
            row.part_no,
            quote_string(&row.creation),
            quote_string(&row.modified),
            row.attributes,
            row.data_size,
            row.binary_data.len(),
            hex(&row.sha256())
        )
        .unwrap();
    }
}

fn render_exact_set_assertion(sql: &mut String, table: &str, expected: &str, code: u32) {
    writeln!(sql, "IF EXISTS (SELECT FileName,PartNo,CONVERT(varchar(27),Creation,121),CONVERT(varchar(27),Modified,121),CONVERT(int,Attributes),CONVERT(bigint,DataSize),CONVERT(bigint,DATALENGTH(BinaryData)),HASHBYTES('SHA2_256',BinaryData) FROM dbo.{table} WITH (UPDLOCK,HOLDLOCK) EXCEPT SELECT FileName,PartNo,Creation,Modified,Attributes,DataSize,ByteLength,Digest FROM @{expected}) OR EXISTS (SELECT FileName,PartNo,Creation,Modified,Attributes,DataSize,ByteLength,Digest FROM @{expected} EXCEPT SELECT FileName,PartNo,CONVERT(varchar(27),Creation,121),CONVERT(varchar(27),Modified,121),CONVERT(int,Attributes),CONVERT(bigint,DataSize),CONVERT(bigint,DATALENGTH(BinaryData)),HASHBYTES('SHA2_256',BinaryData) FROM dbo.{table} WITH (UPDLOCK,HOLDLOCK)) THROW {code}, '{table} exact snapshot drifted', 1;").unwrap();
}

fn render_selected_assertion(sql: &mut String, table: &str, expected: &str, code: u32) {
    writeln!(sql, "IF EXISTS (SELECT E.FileName,E.PartNo FROM @{expected} E LEFT JOIN dbo.{table} T WITH (UPDLOCK,HOLDLOCK) ON T.FileName=E.FileName AND T.PartNo=E.PartNo AND CONVERT(varchar(27),T.Creation,121)=E.Creation AND CONVERT(varchar(27),T.Modified,121)=E.Modified AND CONVERT(int,T.Attributes)=E.Attributes AND CONVERT(bigint,T.DataSize)=E.DataSize AND CONVERT(bigint,DATALENGTH(T.BinaryData))=E.ByteLength AND HASHBYTES('SHA2_256',T.BinaryData)=E.Digest WHERE T.FileName IS NULL) THROW {code}, '{table} selected snapshot drifted', 1;").unwrap();
}

fn render_marker_assertion(
    sql: &mut String,
    table: &str,
    marker: Option<&MainStorageRow>,
    code: u32,
) {
    match marker {
        Some(row) => writeln!(sql, "IF (SELECT COUNT_BIG(*) FROM dbo.{table} WITH (UPDLOCK,HOLDLOCK) WHERE FileName=N'DynamicallyUpdated') <> 1 OR (SELECT COUNT_BIG(*) FROM dbo.{table} WITH (UPDLOCK,HOLDLOCK) WHERE FileName=N'DynamicallyUpdated' AND PartNo=0 AND CONVERT(varchar(27),Creation,121)='{}' AND CONVERT(varchar(27),Modified,121)='{}' AND CONVERT(int,Attributes)={} AND CONVERT(bigint,DataSize)={} AND DATALENGTH(BinaryData)={} AND HASHBYTES('SHA2_256',BinaryData)=0x{}) <> 1 THROW {code}, '{table}.DynamicallyUpdated drifted', 1;", quote_string(&row.creation), quote_string(&row.modified), row.attributes, row.data_size, row.binary_data.len(), hex(&row.sha256())).unwrap(),
        None => writeln!(sql, "IF EXISTS (SELECT 1 FROM dbo.{table} WITH (UPDLOCK,HOLDLOCK) WHERE FileName=N'DynamicallyUpdated') THROW {code}, 'unexpected {table}.DynamicallyUpdated row', 1;").unwrap(),
    }
}

/// #408: no `_dynupdate_` row may exist in `Config` when an ordinary promotion (which
/// deletes the markers and leaves such rows orphaned) is about to write. Checked inside
/// the serializable transaction, before the first write, so it also covers alias rows
/// that no marker names.
fn render_online_history_assertion(sql: &mut String, mode: MainActivationMode, code: u32) {
    let message = quote_string(&online_history_refusal(
        mode,
        "`_dynupdate_` rows are present in Config",
    ));
    writeln!(
        sql,
        "IF EXISTS (SELECT 1 FROM dbo.Config WITH (HOLDLOCK) WHERE FileName LIKE N'%[_]dynupdate[_]%') THROW {code}, '{message}', 1;"
    )
    .unwrap();
}

fn mode_name(mode: MainActivationMode) -> &'static str {
    match mode {
        MainActivationMode::Exclusive => "exclusive",
        MainActivationMode::Online => "online",
        MainActivationMode::Live => "live",
        MainActivationMode::Worker => "worker",
    }
}

fn online_history_refusal(mode: MainActivationMode, evidence: &str) -> String {
    // The exclusive mode is carried out by that apply where the built-in SQL client is (#408 step 2); it reaches
    // this refusal only through `--sqlcmd`.
    let hint = if mode == MainActivationMode::Exclusive {
        " (the exclusive mode of this command does the same on the built-in SQL client: leave out --sqlcmd)"
    } else {
        ""
    };
    format!(
        "{} promotion refused before any write: this database holds online (dynamic) generations ({evidence}).          A {} promotion replaces only the staged rows and deletes the markers, so the earlier online changes          would be lost. Use `mssql-config-apply`, which folds them as the native apply does, or the native          apply{hint}; the staged ConfigSave is left as it is",
        mode_name(mode),
        mode_name(mode)
    )
}

fn render_marker_upsert(sql: &mut String, table: &str, payload: &[u8], code: u32, stamp: &str) {
    let data = hex(payload);
    writeln!(sql, "IF EXISTS (SELECT 1 FROM dbo.{table} WHERE FileName=N'DynamicallyUpdated' AND PartNo=0) UPDATE dbo.{table} SET Modified={stamp},DataSize={},BinaryData=0x{} WHERE FileName=N'DynamicallyUpdated' AND PartNo=0 ELSE INSERT dbo.{table} (FileName,Creation,Modified,Attributes,DataSize,BinaryData,PartNo) VALUES (N'DynamicallyUpdated',{stamp},{stamp},0,{},0x{},0);", payload.len(), data, payload.len(), data).unwrap();
    writeln!(
        sql,
        "IF @@ROWCOUNT <> 1 THROW {code}, '{table}.DynamicallyUpdated upsert failed', 1;"
    )
    .unwrap();
}

fn render_marker_delete(sql: &mut String, table: &str, expected: bool, code: u32) {
    writeln!(
        sql,
        "DELETE FROM dbo.{table} WHERE FileName=N'DynamicallyUpdated';"
    )
    .unwrap();
    writeln!(
        sql,
        "IF @@ROWCOUNT <> {} THROW {code}, '{table}.DynamicallyUpdated cleanup drifted', 1;",
        if expected { 1 } else { 0 }
    )
    .unwrap();
}

fn render_postconditions(sql: &mut String, plan: &MainActivationPlan) {
    let generation = plan.new_generation.hyphenated().to_string();
    let published = plan
        .staged_rows
        .iter()
        .cloned()
        .map(|mut row| {
            row.file_name = published_file_name(plan.mode, &row.file_name, &generation);
            row
        })
        .collect::<Vec<_>>();
    render_expected_table(sql, "ExpectedPublished", &published);
    render_selected_assertion(sql, "Config", "ExpectedPublished", 57222);
    if plan.mode == MainActivationMode::Online {
        let (config_payload, params_payload) = next_dynamic_marker_payloads(plan);
        writeln!(sql, "IF (SELECT COUNT_BIG(*) FROM dbo.Config WHERE FileName=N'DynamicallyUpdated' AND PartNo=0 AND CONVERT(bigint,DataSize)={} AND HASHBYTES('SHA2_256',BinaryData)=0x{}) <> 1 THROW 57223, 'Config.DynamicallyUpdated postcondition failed', 1;", config_payload.len(), hex(&Sha256::digest(&config_payload))).unwrap();
        writeln!(sql, "IF (SELECT COUNT_BIG(*) FROM dbo.Params WHERE FileName=N'DynamicallyUpdated' AND PartNo=0 AND CONVERT(bigint,DataSize)={} AND HASHBYTES('SHA2_256',BinaryData)=0x{}) <> 1 THROW 57224, 'Params.DynamicallyUpdated postcondition failed', 1;", params_payload.len(), hex(&Sha256::digest(&params_payload))).unwrap();
    } else {
        writeln!(sql, "IF EXISTS (SELECT 1 FROM dbo.Config WHERE FileName=N'DynamicallyUpdated') THROW 57225, 'Config.DynamicallyUpdated cleanup postcondition failed', 1;").unwrap();
        writeln!(sql, "IF EXISTS (SELECT 1 FROM dbo.Params WHERE FileName=N'DynamicallyUpdated') THROW 57226, 'Params.DynamicallyUpdated cleanup postcondition failed', 1;").unwrap();
    }
}

fn render_catch(sql: &mut String, live_database: Option<(&str, &str)>) {
    writeln!(sql, "END TRY").unwrap();
    writeln!(sql, "BEGIN CATCH").unwrap();
    writeln!(sql, "IF XACT_STATE() <> 0 ROLLBACK TRANSACTION;").unwrap();
    if let Some((database, database_name)) = live_database {
        writeln!(sql, "USE [master];").unwrap();
        writeln!(sql, "IF DB_ID(N'{}') IS NOT NULL AND DATABASEPROPERTYEX(N'{}','Status') <> N'RESTORING' ALTER DATABASE {database} SET MULTI_USER;", quote_string(database_name), quote_string(database_name)).unwrap();
    }
    writeln!(sql, "THROW;").unwrap();
    writeln!(sql, "END CATCH;").unwrap();
}

fn render_live_recovery(
    sql: &mut String,
    database: &str,
    database_literal: &str,
    tail_log_output: &str,
    recovery_token: &str,
) {
    let tail = quote_string(tail_log_output);
    let backup_name = format!("ibcmd-rs:live:{recovery_token}:1");
    writeln!(sql, "CHECKPOINT;").unwrap();
    writeln!(sql, "USE [master];").unwrap();
    writeln!(sql, "BEGIN TRY").unwrap();
    writeln!(sql, "DECLARE @LiveFirstAccessChanged bit=0;").unwrap();
    // Promotion and cycle 1 commit independently. The guarded continuation owns cycle 2.
    writeln!(sql, "ALTER DATABASE {database} SET SINGLE_USER WITH ROLLBACK IMMEDIATE; SET @LiveFirstAccessChanged=1;").unwrap();
    writeln!(sql, "BACKUP LOG {database} TO DISK=N'{tail}' WITH NORECOVERY, INIT, COMPRESSION, CHECKSUM, NAME=N'{backup_name}';").unwrap();
    writeln!(sql, "RESTORE DATABASE {database} WITH RECOVERY;").unwrap();
    writeln!(
        sql,
        "ALTER DATABASE {database} SET MULTI_USER; SET @LiveFirstAccessChanged=0;"
    )
    .unwrap();
    writeln!(sql, "END TRY").unwrap();
    writeln!(sql, "BEGIN CATCH").unwrap();
    writeln!(sql, "IF @LiveFirstAccessChanged=1 AND DB_ID(N'{database_literal}') IS NOT NULL AND DATABASEPROPERTYEX(N'{database_literal}','Status') <> N'RESTORING' ALTER DATABASE {database} SET MULTI_USER;").unwrap();
    writeln!(sql, "IF DB_ID(N'{database_literal}') IS NOT NULL AND DATABASEPROPERTYEX(N'{database_literal}','Status') = N'RESTORING' BEGIN DECLARE @LiveRecoveryMessage nvarchar(2048)=N'live activation left database restoring; run RESTORE DATABASE {database} WITH RECOVERY; original error: '+ERROR_MESSAGE(); THROW 57250,@LiveRecoveryMessage,1; END;").unwrap();
    writeln!(sql, "THROW;").unwrap();
    writeln!(sql, "END CATCH;").unwrap();
}

fn render_legacy_live_recovery(
    sql: &mut String,
    database: &str,
    database_literal: &str,
    tail_log_output: &str,
) {
    let tail = quote_string(tail_log_output);
    writeln!(sql, "DECLARE @LiveExpected1cConnections int=(SELECT COUNT(*) FROM sys.dm_exec_sessions WHERE is_user_process=1 AND database_id=DB_ID(N'{database_literal}') AND program_name=N'1CV83 Server');").unwrap();
    writeln!(sql, "CHECKPOINT;").unwrap();
    writeln!(sql, "USE [master];").unwrap();
    writeln!(sql, "BEGIN TRY").unwrap();
    // The first recovery makes rphost observe the newly committed ordinary
    // generation. The second recovery advances already open 1C sessions to
    // that prepared generation. Both log backup sets are retained in one
    // operator-owned artifact.
    writeln!(
        sql,
        "ALTER DATABASE {database} SET SINGLE_USER WITH ROLLBACK IMMEDIATE;"
    )
    .unwrap();
    writeln!(
        sql,
        "BACKUP LOG {database} TO DISK=N'{tail}' WITH NORECOVERY, INIT, COMPRESSION, CHECKSUM;"
    )
    .unwrap();
    writeln!(sql, "RESTORE DATABASE {database} WITH RECOVERY;").unwrap();
    writeln!(sql, "ALTER DATABASE {database} SET MULTI_USER;").unwrap();
    writeln!(sql, "DECLARE @LiveReconnectDeadline datetime2(3)=DATEADD(millisecond,4000,SYSUTCDATETIME()), @LiveStableSince datetime2(3)=NULL, @LiveObserved1cConnections int=0;").unwrap();
    writeln!(sql, "WHILE @LiveExpected1cConnections > 0 AND SYSUTCDATETIME() < @LiveReconnectDeadline BEGIN SELECT @LiveObserved1cConnections=COUNT(*) FROM sys.dm_exec_sessions WHERE is_user_process=1 AND database_id=DB_ID(N'{database_literal}') AND program_name=N'1CV83 Server'; IF @LiveObserved1cConnections >= @LiveExpected1cConnections BEGIN IF @LiveStableSince IS NULL SET @LiveStableSince=SYSUTCDATETIME(); IF DATEDIFF(millisecond,@LiveStableSince,SYSUTCDATETIME()) >= 500 BREAK; END ELSE SET @LiveStableSince=NULL; WAITFOR DELAY '00:00:00.100'; END;").unwrap();
    writeln!(sql, "IF @LiveExpected1cConnections > 0 AND (@LiveObserved1cConnections < @LiveExpected1cConnections OR @LiveStableSince IS NULL OR DATEDIFF(millisecond,@LiveStableSince,SYSUTCDATETIME()) < 500) THROW 57234, '1C SQL connections did not recover before the live activation deadline; database is online and the second recovery was not started', 1;").unwrap();
    writeln!(
        sql,
        "ALTER DATABASE {database} SET SINGLE_USER WITH ROLLBACK IMMEDIATE;"
    )
    .unwrap();
    writeln!(
        sql,
        "BACKUP LOG {database} TO DISK=N'{tail}' WITH NORECOVERY, NOINIT, COMPRESSION, CHECKSUM;"
    )
    .unwrap();
    writeln!(sql, "RESTORE DATABASE {database} WITH RECOVERY;").unwrap();
    writeln!(sql, "ALTER DATABASE {database} SET MULTI_USER;").unwrap();
    writeln!(sql, "END TRY").unwrap();
    writeln!(sql, "BEGIN CATCH").unwrap();
    writeln!(sql, "IF DB_ID(N'{database_literal}') IS NOT NULL AND DATABASEPROPERTYEX(N'{database_literal}','Status') <> N'RESTORING' ALTER DATABASE {database} SET MULTI_USER;").unwrap();
    writeln!(sql, "IF DB_ID(N'{database_literal}') IS NOT NULL AND DATABASEPROPERTYEX(N'{database_literal}','Status') = N'RESTORING' BEGIN DECLARE @LiveRecoveryMessage nvarchar(2048)=N'live activation left database restoring; run RESTORE DATABASE {database} WITH RECOVERY; original error: '+ERROR_MESSAGE(); THROW 57250,@LiveRecoveryMessage,1; END;").unwrap();
    writeln!(sql, "THROW;").unwrap();
    writeln!(sql, "END CATCH;").unwrap();
}

pub(crate) fn validate_tail_log_output(path: &str) -> Result<String, MainActivationError> {
    if path.is_empty()
        || path.encode_utf16().count() > 2048
        || path.chars().any(|ch| ch == '\0' || ch.is_control())
    {
        return Err(MainActivationError::SafetyGate(
            "tail-log output path is empty, contains control characters, or exceeds 2048 UTF-16 code units"
                .to_owned(),
        ));
    }
    Ok(path.to_owned())
}

fn validate_rows(
    table: &str,
    rows: &[MainStorageRow],
    allow_empty: bool,
) -> Result<(), MainActivationError> {
    if !allow_empty && rows.is_empty() {
        return Err(MainActivationError::InvalidRow(format!(
            "{table} snapshot is empty"
        )));
    }
    for row in rows {
        if row.file_name.is_empty()
            || row.file_name.encode_utf16().count() > 128
            || row
                .file_name
                .chars()
                .any(|ch| ch == '\0' || ch.is_control())
        {
            return Err(MainActivationError::InvalidRow(format!(
                "{table} has an invalid FileName"
            )));
        }
        if row.part_no != 0 {
            return Err(MainActivationError::InvalidRow(format!(
                "{table}.{} uses unsupported PartNo {}",
                row.file_name, row.part_no
            )));
        }
        if row.binary_data.len() > MAX_ROW_BYTES || row.data_size != row.binary_data.len() as u64 {
            return Err(MainActivationError::InvalidRow(format!(
                "{table}.{} DataSize/binary length is invalid or exceeds {MAX_ROW_BYTES}",
                row.file_name
            )));
        }
    }
    index_rows(rows)?;
    Ok(())
}

fn validate_optional_marker(
    table: &str,
    marker: Option<&MainStorageRow>,
) -> Result<(), MainActivationError> {
    if let Some(row) = marker {
        validate_rows(table, std::slice::from_ref(row), false)?;
        if row.file_name != "DynamicallyUpdated" || row.part_no != 0 {
            return Err(MainActivationError::InvalidRow(format!(
                "{table} marker is not DynamicallyUpdated part 0"
            )));
        }
    }
    Ok(())
}

fn index_rows(
    rows: &[MainStorageRow],
) -> Result<BTreeMap<(String, i32), &MainStorageRow>, MainActivationError> {
    let mut indexed = BTreeMap::new();
    for row in rows {
        if indexed
            .insert((row.file_name.clone(), row.part_no), row)
            .is_some()
        {
            return Err(MainActivationError::InvalidRow(format!(
                "duplicate row {} part {}",
                row.file_name, row.part_no
            )));
        }
    }
    Ok(indexed)
}

fn require_single_part(
    rows: &BTreeMap<(String, i32), &MainStorageRow>,
    name: &str,
    table: &str,
) -> Result<(), MainActivationError> {
    if !rows.contains_key(&(name.to_owned(), 0)) {
        return Err(MainActivationError::InvalidRow(format!(
            "{table} is missing {name} part 0"
        )));
    }
    Ok(())
}

fn is_existing_body_target(name: &str) -> bool {
    let (uuid, suffix) = if name.len() == 36 {
        (name, "")
    } else if name.len() > 37 && name.as_bytes().get(36) == Some(&b'.') {
        (&name[..36], &name[37..])
    } else {
        return false;
    };
    Uuid::parse_str(uuid).is_ok()
        && (suffix.is_empty() || suffix.bytes().all(|byte| byte.is_ascii_digit()))
}

pub(crate) fn generation_from_versions(blob: &[u8]) -> Result<Uuid, MainActivationError> {
    let mut decoder = DeflateDecoder::new(blob).take((MAX_INFLATED_VERSIONS_BYTES + 1) as u64);
    let mut plain = Vec::new();
    decoder
        .read_to_end(&mut plain)
        .map_err(|error| MainActivationError::Versions(format!("raw deflate failed: {error}")))?;
    if plain.len() > MAX_INFLATED_VERSIONS_BYTES {
        return Err(MainActivationError::Limit(format!(
            "inflated versions exceeds {MAX_INFLATED_VERSIONS_BYTES} bytes"
        )));
    }
    let text = std::str::from_utf8(plain.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&plain))
        .map_err(|_| MainActivationError::Versions("header is not UTF-8".to_owned()))?;
    let mut fields = text
        .strip_prefix('{')
        .ok_or_else(|| MainActivationError::Versions("header does not start with '{'".to_owned()))?
        .splitn(5, ',');
    if fields.next().map(str::trim) != Some("1") {
        return Err(MainActivationError::Versions(
            "unsupported versions header tag".to_owned(),
        ));
    }
    let count = fields
        .next()
        .ok_or_else(|| MainActivationError::Versions("missing row count".to_owned()))?
        .trim();
    if count.is_empty() || !count.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(MainActivationError::Versions(
            "invalid versions row count".to_owned(),
        ));
    }
    if fields.next().map(str::trim) != Some("\"\"") {
        return Err(MainActivationError::Versions(
            "unsupported versions header shape".to_owned(),
        ));
    }
    let generation = Uuid::parse_str(
        fields
            .next()
            .ok_or_else(|| MainActivationError::Versions("missing generation UUID".to_owned()))?
            .trim(),
    )
    .map_err(|_| MainActivationError::Versions("invalid generation UUID".to_owned()))?;
    if fields.next().is_none() {
        return Err(MainActivationError::Versions(
            "versions header has no mapping payload".to_owned(),
        ));
    }
    Ok(generation)
}

fn active_generation_from_markers(
    ordinary: Uuid,
    config_marker: Option<&MainStorageRow>,
    params_marker: Option<&MainStorageRow>,
) -> Result<(Uuid, Vec<Uuid>), MainActivationError> {
    match (config_marker, params_marker) {
        (None, None) => Ok((ordinary, Vec::new())),
        (Some(config), Some(params)) => {
            let config = marker_fields(&config.binary_data)?;
            let params = marker_fields(&params.binary_data)?;
            let config_count = marker_count(&config, "Config")?;
            let params_count = marker_count(&params, "Params")?;
            if config[0] != "1" || config_count == 0 || config.len() != config_count + 2 {
                return Err(MainActivationError::Versions(
                    "unsupported Config.DynamicallyUpdated marker".to_owned(),
                ));
            }
            if params[0] != "0"
                || params_count != config_count + 1
                || params.len() != params_count + 2
            {
                return Err(MainActivationError::Versions(
                    "unsupported Params.DynamicallyUpdated marker".to_owned(),
                ));
            }
            if config_count > 4096 {
                return Err(MainActivationError::Limit(
                    "dynamic generation history exceeds 4096 entries".to_owned(),
                ));
            }
            let params_ordinary = Uuid::parse_str(params[2]).map_err(|_| {
                MainActivationError::Versions(
                    "Params.DynamicallyUpdated has an invalid ordinary generation".to_owned(),
                )
            })?;
            if params_ordinary != ordinary {
                return Err(MainActivationError::Versions(
                    "Params dynamic ordinary generation disagrees with versions".to_owned(),
                ));
            }
            let history = config[2..]
                .iter()
                .map(|value| {
                    Uuid::parse_str(value).map_err(|_| {
                        MainActivationError::Versions(
                            "Config.DynamicallyUpdated has an invalid generation".to_owned(),
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let params_history = params[3..]
                .iter()
                .map(|value| {
                    Uuid::parse_str(value).map_err(|_| {
                        MainActivationError::Versions(
                            "Params.DynamicallyUpdated has an invalid generation".to_owned(),
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if history != params_history {
                return Err(MainActivationError::Versions(
                    "Config/Params dynamic generation histories disagree".to_owned(),
                ));
            }
            Ok((*history.last().expect("non-empty history"), history))
        }
        _ => Err(MainActivationError::Versions(
            "Config and Params dynamic markers must both be present or both absent".to_owned(),
        )),
    }
}

fn marker_count(fields: &[&str], table: &str) -> Result<usize, MainActivationError> {
    fields
        .get(1)
        .ok_or_else(|| MainActivationError::Versions(format!("{table} marker has no count")))?
        .parse::<usize>()
        .map_err(|_| MainActivationError::Versions(format!("{table} marker count is invalid")))
}

fn next_dynamic_marker_payloads(plan: &MainActivationPlan) -> (Vec<u8>, Vec<u8>) {
    let mut history = plan.dynamic_history.clone();
    history.push(plan.new_generation);
    let generations = history
        .iter()
        .map(|value| value.hyphenated().to_string())
        .collect::<Vec<_>>()
        .join(",");
    let config = utf8_bom(&format!("{{1,{},{generations}}}", history.len()));
    let params = utf8_bom(&format!(
        "{{0,{},{},{generations}}}",
        history.len() + 1,
        plan.ordinary_generation.hyphenated()
    ));
    (config, params)
}

fn marker_fields(blob: &[u8]) -> Result<Vec<&str>, MainActivationError> {
    let text = std::str::from_utf8(blob.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(blob))
        .map_err(|_| MainActivationError::Versions("dynamic marker is not UTF-8".to_owned()))?;
    let inner = text
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .ok_or_else(|| MainActivationError::Versions("dynamic marker is not braced".to_owned()))?;
    Ok(inner.split(',').map(str::trim).collect())
}

/// The `Config` name a staged row is published under: an online run keeps
/// `root` and `version`, writes `versions` and every body under its
/// `_dynupdate_<generation>` alias; the other modes write the ordinary name.
fn published_file_name(mode: MainActivationMode, file_name: &str, generation: &str) -> String {
    if mode != MainActivationMode::Online {
        return file_name.to_owned();
    }
    match file_name {
        "root" | "version" => file_name.to_owned(),
        "versions" => format!("versions_dynupdate_{generation}"),
        _ => dynamic_alias(file_name, generation),
    }
}

fn dynamic_alias(name: &str, generation: &str) -> String {
    match name.split_once('.') {
        Some((base, suffix)) => format!("{base}_dynupdate_{generation}.{suffix}"),
        None => format!("{name}_dynupdate_{generation}"),
    }
}

fn utf8_bom(value: &str) -> Vec<u8> {
    let mut bytes = vec![0xef, 0xbb, 0xbf];
    bytes.extend_from_slice(value.as_bytes());
    bytes
}

fn quote_ident(value: &str) -> Result<String, MainActivationError> {
    if value.is_empty()
        || value.encode_utf16().count() > 128
        || value.chars().any(|ch| ch == '\0' || ch.is_control())
    {
        return Err(MainActivationError::InvalidRow(
            "invalid database identifier".to_owned(),
        ));
    }
    Ok(format!("[{}]", value.replace(']', "]]")))
}

pub(crate) fn quote_string(value: &str) -> String {
    value.replace('\'', "''")
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02X}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::DeflateEncoder};
    use std::io::Write;

    const OLD: &str = "719baa18-69ed-439a-8962-1de53d98e05e";
    const NEW: &str = "d66867e4-febb-4c6e-9ef1-230dac3e25fa";
    const BODY: &str = "b27aebc8-f190-4658-a81d-fd1406905f39";

    fn deflate(text: &str) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(text.as_bytes()).unwrap();
        encoder.finish().unwrap()
    }

    fn row(name: &str, data: Vec<u8>) -> MainStorageRow {
        MainStorageRow {
            file_name: name.to_owned(),
            part_no: 0,
            creation: "2026-08-29T00:00:00Z".to_owned(),
            modified: "2026-08-29T00:00:00Z".to_owned(),
            attributes: 0,
            data_size: data.len() as u64,
            binary_data: data,
        }
    }

    fn versions(generation: &str) -> Vec<u8> {
        deflate(&format!("{{1,4,\"\",{generation},\"root\",x}}"))
    }

    fn fixture(mode: MainActivationMode) -> MainActivationPlan {
        let staged = vec![
            row(BODY, b"new descriptor".to_vec()),
            row(&format!("{BODY}.0"), b"new body".to_vec()),
            row("root", b"new root".to_vec()),
            row("version", b"new version".to_vec()),
            row("versions", versions(NEW)),
        ];
        let active = vec![
            row(BODY, b"old descriptor".to_vec()),
            row(&format!("{BODY}.0"), b"old body".to_vec()),
            row("root", b"old root".to_vec()),
            row("version", b"old version".to_vec()),
            row("versions", versions(OLD)),
        ];
        prepare_main_activation(
            mode,
            staged,
            MainActivationSnapshot {
                config_rows: active,
                config_dynamically_updated: None,
                params_dynamically_updated: None,
            },
            &[BODY.to_owned(), format!("{BODY}.0")],
            true,
        )
        .unwrap()
    }

    #[test]
    fn derives_generations_from_raw_deflated_versions_header() {
        let plan = fixture(MainActivationMode::Exclusive);
        assert_eq!(plan.old_generation().to_string(), OLD);
        assert_eq!(plan.new_generation().to_string(), NEW);
    }

    #[test]
    fn complete_129_row_body_cohort_remains_refused_until_native_measurement() {
        // A structurally valid body-only cohort, with every original preimage
        // and all three service rows. Its small byte size must not silently
        // promote an unmeasured >128 publication to runtime support (#345).
        for count in [128, 129] {
            for mode in [
                MainActivationMode::Online,
                MainActivationMode::Exclusive,
                MainActivationMode::Live,
                MainActivationMode::Worker,
            ] {
                let base = fixture(mode);
                let mut staged = base
                    .staged_rows
                    .iter()
                    .filter(|row| SERVICE_NAMES.contains(&row.file_name.as_str()))
                    .cloned()
                    .collect::<Vec<_>>();
                let mut active = base
                    .active_rows
                    .iter()
                    .filter(|row| SERVICE_NAMES.contains(&row.file_name.as_str()))
                    .cloned()
                    .collect::<Vec<_>>();
                let mut allowed = Vec::new();
                for index in 0..count - 3 {
                    let name = format!("00000000-0000-4000-8000-{:012x}.0", index + 1);
                    staged.push(row(&name, b"new module".to_vec()));
                    active.push(row(&name, b"old module".to_vec()));
                    allowed.push(name);
                }
                let result = prepare_main_activation(
                    mode,
                    staged,
                    MainActivationSnapshot {
                        config_rows: active,
                        config_dynamically_updated: None,
                        params_dynamically_updated: None,
                    },
                    &allowed,
                    true,
                );
                if count == 128 {
                    assert_eq!(result.unwrap().dry_run_report().staged_rows, count);
                } else {
                    assert!(matches!(result, Err(MainActivationError::Limit(ref detail))
                        if detail.contains("129 staged rows exceeds 128")));
                }
            }
        }
    }

    #[test]
    fn exclusive_replaces_only_exact_staged_ordinary_rows() {
        let script =
            render_main_activation_sql("lab]db", &fixture(MainActivationMode::Exclusive), None)
                .unwrap();
        assert!(script.sql.contains("USE [lab]]db]"));
        assert!(
            script
                .sql
                .contains("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        );
        assert!(script.sql.contains("sp_getapplock"));
        assert!(
            script
                .sql
                .contains("exclusive activation requires no other database sessions")
        );
        assert!(
            script
                .sql
                .contains("DELETE FROM dbo.Config WHERE FileName=N'root'")
        );
        // No alias row is written (the #408 assertion only reads for them).
        assert!(!script.sql.contains(&format!("{BODY}_dynupdate_")));
        assert!(!script.sql.contains(&format!("_dynupdate_{NEW}")));
        assert!(!script.sql.contains("Files.MobileVersions"));
        assert!(!script.sql.contains("_ConfigChngR"));
        assert!(!script.sql.contains(".ui"));
    }

    #[test]
    fn exclusive_leaves_out_only_the_idle_sessions_of_the_own_ras_processes() {
        let own = vec![OwnRasProcess {
            host: "LAB-HOST".to_owned(),
            pid: 22608,
        }];
        let plan = fixture(MainActivationMode::Exclusive).with_own_ras_processes(own.clone());
        let script = render_main_activation_sql("lab", &plan, None).unwrap();
        assert!(
            script
                .sql
                .contains("exclusive activation requires no other database sessions")
        );
        assert!(script.sql.contains(
            "AND NOT (ISNULL(program_name,N'')=N'1CV83 Server' AND status=N'sleeping' AND open_transaction_count=0 AND ((ISNULL(host_name,N'')=N'LAB-HOST' AND ISNULL(host_process_id,-1) IN (22608))))"
        ));
        assert_eq!(script.report.own_ras_processes, own);
        // Without them, every session is counted, as before.
        let plain =
            render_main_activation_sql("lab", &fixture(MainActivationMode::Exclusive), None)
                .unwrap();
        assert!(plain.sql.contains(
            "database_id=DB_ID()) THROW 57209, 'exclusive activation requires no other database sessions', 1;"
        ));
        assert!(!plain.sql.contains("1CV83 Server' AND status"));
        // The other modes have no such gate, whatever they are told.
        let online = fixture(MainActivationMode::Online).with_own_ras_processes(own);
        assert!(
            !render_main_activation_sql("lab", &online, None)
                .unwrap()
                .sql
                .contains("no other database sessions")
        );
    }

    #[test]
    fn online_preserves_ordinary_body_and_creates_evidenced_aliases() {
        let script =
            render_main_activation_sql("lab", &fixture(MainActivationMode::Online), None).unwrap();
        assert!(script.sql.contains(&format!("{BODY}_dynupdate_{NEW}")));
        assert!(script.sql.contains(&format!("{BODY}_dynupdate_{NEW}.0")));
        assert!(script.sql.contains(&format!("versions_dynupdate_{NEW}")));
        assert!(
            !script
                .sql
                .contains(&format!("DELETE FROM dbo.Config WHERE FileName=N'{BODY}'"))
        );
        assert!(script.sql.contains("EFBBBF7B312C312C"));
        assert!(script.sql.contains("EFBBBF7B302C322C"));
        assert!(script.report.online_protocol_verified);
        assert!(script.report.existing_sessions_retain_generation);
    }

    #[test]
    fn source_initial_dependency_preimages_precede_all_script_publications() {
        let original =
            crate::mssql_config_apply::dynamic::SourceOwnerPreimages::test_fixture("lab");
        for mode in [
            MainActivationMode::Online,
            MainActivationMode::Live,
            MainActivationMode::Worker,
        ] {
            let plan =
                fixture(mode).with_precondition_sql(original.precondition_sql("lab").unwrap());
            let tail = (mode == MainActivationMode::Live).then_some(r"C:\tail.trn");
            let sql = render_main_activation_sql("lab", &plan, tail).unwrap().sql;
            let begin = sql.find("BEGIN TRANSACTION;").unwrap();
            let initial = sql.find("-- initial source dependency preimages").unwrap();
            let first_write = sql.find("DELETE FROM dbo.Config WHERE").unwrap();
            assert!(begin < initial && initial < first_write, "{mode:?}");
            assert!(sql[initial..first_write].contains("UPDLOCK, HOLDLOCK"));
            assert!(sql[initial..first_write].contains("HASHBYTES('SHA2_256', BinaryData)"));
        }
    }

    #[test]
    fn the_parity_sql_runs_in_the_online_transaction_before_the_stage_is_consumed() {
        let parity = "UPDATE dbo._ConfigChngR SET _MessageNo = NULL; -- parity\n";
        let plan = fixture(MainActivationMode::Online).with_parity_sql(parity.to_owned());
        let sql = render_main_activation_sql("lab", &plan, None).unwrap().sql;
        let upsert = sql
            .find("Params.DynamicallyUpdated upsert failed")
            .expect("the markers are written");
        let at = sql.find(parity).expect("the parity sql is in the script");
        let consumed = sql.find("DELETE FROM dbo.ConfigSave;").unwrap();
        // after the rows and the markers, while ConfigSave still holds the stage, inside the
        // transaction the postconditions and the commit close
        assert!(upsert < at && at < consumed, "{upsert} {at} {consumed}");
        assert!(at < sql.find("COMMIT TRANSACTION;").unwrap());
        // only the online publication takes it: the ordinary ones fold the overlay away and
        // replace rows in place, which is the exclusive apply's script
        for mode in [
            MainActivationMode::Exclusive,
            MainActivationMode::Live,
            MainActivationMode::Worker,
        ] {
            let plan = fixture(mode).with_parity_sql(parity.to_owned());
            let sql = render_main_activation_sql("lab", &plan, Some(r"C:\tail.trn"))
                .or_else(|_| render_main_activation_sql("lab", &plan, None))
                .unwrap()
                .sql;
            assert!(!sql.contains(parity), "{mode:?}");
        }
        // an empty fragment is no fragment, and an unchanged stage writes nothing at all
        let plain = render_main_activation_sql("lab", &fixture(MainActivationMode::Online), None)
            .unwrap()
            .sql;
        let empty = fixture(MainActivationMode::Online).with_parity_sql("  \n".to_owned());
        assert_eq!(
            render_main_activation_sql("lab", &empty, None).unwrap().sql,
            plain
        );
        assert!(!plain.contains("parity"));
    }

    #[test]
    fn the_markers_are_stamped_in_utc_unless_the_plan_brings_the_platforms_timestamps() {
        let plain = render_main_activation_sql("lab", &fixture(MainActivationMode::Online), None)
            .unwrap()
            .sql;
        assert!(plain.contains("SYSUTCDATETIME()"));
        assert!(!plain.contains("@now"));
        let declarations = "DECLARE @now datetime2(6) = SYSDATETIME(); -- platform\n";
        let stamped = render_main_activation_sql(
            "lab",
            &fixture(MainActivationMode::Online).with_platform_timestamps(declarations.to_owned()),
            None,
        )
        .unwrap()
        .sql;
        // declared before the first row is written, used for both markers, and no UTC left
        let declared = stamped.find(declarations).unwrap();
        let first_write = stamped
            .find("DELETE FROM dbo.Config WHERE FileName=N'root'")
            .unwrap();
        assert!(declared < first_write);
        assert!(!stamped.contains("SYSUTCDATETIME()"));
        assert_eq!(stamped.matches("Modified=@now").count(), 2);
        assert!(stamped.contains("VALUES (N'DynamicallyUpdated',@now,@now,0,"));
        // the ordinary modes take no such thing
        let exclusive = render_main_activation_sql(
            "lab",
            &fixture(MainActivationMode::Exclusive)
                .with_platform_timestamps(declarations.to_owned()),
            None,
        )
        .unwrap()
        .sql;
        assert!(!exclusive.contains("@now"));
    }

    #[test]
    fn a_consumed_stage_row_is_part_of_the_exact_stage_and_published_nowhere() {
        let deleted = row("deleted", b"0".to_vec());
        let plan =
            fixture(MainActivationMode::Online).with_consumed_stage_rows(vec![deleted.clone()]);
        let sql = render_main_activation_sql("lab", &plan, None).unwrap().sql;
        // asserted with the stage (the ConfigSave the transaction expects holds it), emptied with it (6 rows: 5 + 1)
        assert!(sql.contains(&format!(
            "INSERT @ExpectedStage VALUES (N'deleted',0,'{}','{}',{},{},{},0x{})",
            quote_string(&deleted.creation),
            quote_string(&deleted.modified),
            deleted.attributes,
            deleted.data_size,
            deleted.binary_data.len(),
            hex(&deleted.sha256())
        )));
        assert!(sql.contains("IF @@ROWCOUNT <> 6 THROW 57220, 'ConfigSave cleanup drifted', 1;"));
        // and neither moved, aliased nor expected in Config afterwards
        assert!(!sql.contains("deleted_dynupdate_"));
        assert!(!sql.contains("FileName=N'deleted' AND PartNo=0"));
        assert!(
            !plan
                .alias_rows()
                .iter()
                .any(|alias| alias.contains("deleted"))
        );
        assert_eq!(plan.dry_run_report().staged_rows, 5);
        let plain = render_main_activation_sql("lab", &fixture(MainActivationMode::Online), None)
            .unwrap()
            .sql;
        assert!(plain.contains("IF @@ROWCOUNT <> 5 THROW 57220"));
    }

    #[test]
    fn online_noop_consumes_the_exact_stage_without_parity_or_marker_writes() {
        let active = fixture(MainActivationMode::Online).active_rows;
        let plan = prepare_main_activation(
            MainActivationMode::Online,
            active.clone(),
            MainActivationSnapshot {
                config_rows: active,
                config_dynamically_updated: None,
                params_dynamically_updated: None,
            },
            &[BODY.to_owned(), format!("{BODY}.0")],
            true,
        )
        .unwrap()
        .with_consumed_stage_rows(vec![row("deleted", b"0".to_vec())])
        .with_precondition_sql("IF EXISTS (SELECT 1 FROM dbo.Params WITH (UPDLOCK, HOLDLOCK) WHERE FileName=N'stale-si') THROW 57305, 'pending service drifted', 1;".to_owned())
        .with_parity_sql("UPDATE dbo.Files SET BinaryData=0x01; -- must not run".to_owned())
        .with_platform_timestamps("DECLARE @now datetime2(6) = SYSDATETIME();".to_owned());
        assert!(plan.is_no_op());
        let sql = render_main_activation_sql("lab", &plan, None).unwrap().sql;
        assert!(sql.contains("INSERT @ExpectedStage VALUES (N'deleted'"));
        assert!(sql.contains("DELETE FROM dbo.ConfigSave;"));
        assert!(sql.contains("IF @@ROWCOUNT <> 6 THROW 57205"));
        let guard = sql.find("pending service drifted").unwrap();
        assert!(sql.find("BEGIN TRANSACTION").unwrap() < guard);
        assert!(sql.find("THROW 57204").unwrap() < guard);
        assert!(guard < sql.find("DELETE FROM dbo.ConfigSave;").unwrap());
        assert!(!sql.contains("must not run"));
        assert!(!sql.contains("DECLARE @now"));
        assert!(!sql.contains("INSERT dbo.Config"));
        assert!(!sql.contains("UPDATE dbo.Params"));
        assert!(!sql.contains("DELETE FROM dbo.Config WHERE"));
    }

    #[test]
    fn the_alias_rows_of_a_publication_are_named_as_the_transition_writes_them() {
        let plan = fixture(MainActivationMode::Online);
        assert_eq!(
            plan.alias_rows(),
            vec![
                format!("{BODY}_dynupdate_{NEW}"),
                format!("{BODY}_dynupdate_{NEW}.0"),
                format!("versions_dynupdate_{NEW}"),
            ]
        );
        assert!(plan.dynamic_history().is_empty());
        // and each of them is a row of the script
        let sql = render_main_activation_sql("lab", &plan, None).unwrap().sql;
        for alias in plan.alias_rows() {
            assert!(sql.contains(&format!("N'{alias}'")), "{alias}");
        }
    }

    #[test]
    fn live_noop_stage_cleanup_is_inside_the_master_pending_guard() {
        let base = fixture(MainActivationMode::Live);
        let noop = prepare_main_activation(
            MainActivationMode::Live,
            base.active_rows.clone(),
            MainActivationSnapshot {
                config_rows: base.active_rows.clone(),
                config_dynamically_updated: None,
                params_dynamically_updated: None,
            },
            &[BODY.to_owned(), format!("{BODY}.0")],
            true,
        )
        .unwrap();
        assert!(noop.is_no_op());
        for checkpoint in [false, true] {
            let mut script = render_main_activation_sql("lab", &noop, None).unwrap();
            protect_live_script("lab", checkpoint, &mut script);
            let delete = script.sql.find("DELETE FROM dbo.ConfigSave").unwrap();
            assert!(script.sql.find("sp_getapplock").unwrap() < delete);
            assert!(script.sql.find("THROW 57269").unwrap() < delete);
            assert!(delete < script.sql.find("sp_releaseapplock").unwrap());
            assert!(!script.sql.contains("BACKUP LOG"));
        }
    }

    #[test]
    fn default_live_keeps_the_legacy_two_cycle_sql_readiness_contract() {
        let sql = render_main_activation_sql(
            "lab",
            &fixture(MainActivationMode::Live),
            Some(r"F:\tail.trn"),
        )
        .unwrap()
        .sql;
        assert_eq!(sql.matches("BACKUP LOG [lab]").count(), 2);
        assert!(sql.contains("THROW 57234"));
        assert!(sql.contains("@LiveExpected1cConnections"));
        assert!(sql.contains("NORECOVERY, NOINIT"));
        assert!(!sql.contains("ibcmd-rs:live:"));
    }

    #[test]
    fn live_promotes_ordinary_rows_then_runs_guarded_tail_recovery() {
        let script = render_main_activation_checkpoint(
            "lab]db",
            &fixture(MainActivationMode::Live),
            Some(r"C:\tail's\generation.trn"),
            false,
        )
        .unwrap();
        let promotion = script
            .sql
            .find("DELETE FROM dbo.Config WHERE FileName=N'root'")
            .unwrap();
        let single_users = script
            .sql
            .match_indices("SET SINGLE_USER")
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let backups = script
            .sql
            .match_indices("BACKUP LOG [lab]]db]")
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let recoveries = script
            .sql
            .match_indices("RESTORE DATABASE [lab]]db] WITH RECOVERY")
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(single_users.len(), 1);
        assert_eq!(backups.len(), 1);
        assert_eq!(recoveries.len(), 2);
        assert!(promotion < single_users[0]);
        assert!(single_users[0] < backups[0] && backups[0] < recoveries[0]);
        assert!(!script.sql.contains("@LiveExpected1cConnections"));
        assert!(!script.sql.contains("57234"));
        assert!(!script.sql.contains("NOINIT"));
        assert!(script.sql.contains("sys.dm_os_file_exists"));
        assert!(script.sql.contains("file_is_a_directory=1"));
        assert!(script.sql.contains("C:\\tail''s\\generation.trn"));
        assert!(
            script
                .sql
                .contains("WITH NORECOVERY, INIT, COMPRESSION, CHECKSUM, NAME=")
        );
        for (index, _) in script.sql.match_indices("BACKUP LOG") {
            let statement = &script.sql[index..index + script.sql[index..].find(';').unwrap()];
            assert!(
                !statement.contains("COPY_ONLY"),
                "a copy-only tail backup would not advance the chain: {statement}"
            );
        }
        assert!(script.sql.contains("SET MULTI_USER"));
        assert!(
            script
                .sql
                .contains("live activation left database restoring")
        );
        assert!(!script.report.live_session_switch_expected);
        assert!(script.report.requires_tail_log_artifact);
        assert!(!script.report.existing_sessions_retain_generation);
        assert_eq!(
            script.report.touched_tables,
            ["Config", "ConfigSave", "Params"]
        );
    }

    #[test]
    fn the_live_gate_is_at_the_head_of_the_script_and_the_sessions_are_checked_again_before_the_commit()
     {
        // #409 F-9 (log chain, directory, the account's right to write) and F-10 (sessions with open work)
        let plan = fixture(MainActivationMode::Live);
        let tail = Some(r"C:\tail\generation.trn");
        let script = render_main_activation_sql("lab", &plan, tail).unwrap();
        let at = |needle: &str| {
            script
                .sql
                .find(needle)
                .unwrap_or_else(|| panic!("{needle} is not in the script"))
        };
        let (chain, directory, probe, sessions) = (
            at("THROW 57235"),
            at("THROW 57236"),
            at("BACKUP DATABASE [model]"),
            at("THROW 57238"),
        );
        let begin = at("BEGIN TRANSACTION");
        let again = at("THROW 57239");
        let commit = at("COMMIT TRANSACTION");
        assert!(
            chain < directory && directory < probe && probe < sessions,
            "the gate checks what costs least first"
        );
        assert!(
            sessions < begin,
            "nothing is written before the gate has passed"
        );
        assert!(
            begin < again && again < commit,
            "work that appears meanwhile rolls the promotion back"
        );
        assert!(
            script.sql.contains("BACKUP DATABASE [model]") && script.sql.contains("xp_delete_file")
        );
        // the tail-log backups come after the gate and the commit, as before
        assert!(commit < at("BACKUP LOG"));

        // the operator accepts the interruption: no sessions check, in the gate or before the commit; the rest stays
        let accepted = render_main_activation_sql_with("lab", &plan, tail, true).unwrap();
        assert!(!accepted.sql.contains("THROW 57238") && !accepted.sql.contains("THROW 57239"));
        assert!(
            accepted.sql.contains("THROW 57235")
                && accepted.sql.contains("BACKUP DATABASE [model]")
        );
        assert_eq!(
            accepted.report, script.report,
            "the acceptance is not part of the plan"
        );

        // the other modes have no live gate
        for mode in [
            MainActivationMode::Exclusive,
            MainActivationMode::Online,
            MainActivationMode::Worker,
        ] {
            let other = render_main_activation_sql("lab", &fixture(mode), None).unwrap();
            for code in ["57235", "57236", "57238", "57239"] {
                assert!(!other.sql.contains(code), "{mode:?}: {code}");
            }
        }
    }

    #[test]
    fn interrupting_sessions_is_accepted_for_the_live_mode_only() {
        assert!(preflight_interrupt_sessions(MainActivationMode::Live, true).is_ok());
        for mode in [
            MainActivationMode::Exclusive,
            MainActivationMode::Online,
            MainActivationMode::Worker,
        ] {
            assert!(
                preflight_interrupt_sessions(mode, false).is_ok(),
                "{mode:?}"
            );
            let error = preflight_interrupt_sessions(mode, true).unwrap_err();
            assert!(
                error.to_string().contains("only valid for live"),
                "{mode:?}: {error}"
            );
        }
    }

    fn markers(ordinary: &str, history: &[&str]) -> (MainStorageRow, MainStorageRow) {
        let generations = history.join(",");
        (
            row(
                "DynamicallyUpdated",
                utf8_bom(&format!("{{1,{},{generations}}}", history.len())),
            ),
            row(
                "DynamicallyUpdated",
                utf8_bom(&format!(
                    "{{0,{},{ordinary},{generations}}}",
                    history.len() + 1
                )),
            ),
        )
    }

    #[test]
    fn preflight_refuses_an_ordinary_mode_on_a_database_with_online_generations() {
        let ordinary = row("versions", versions(OLD));
        let (config, params) = markers(OLD, &[NEW]);
        for mode in [
            MainActivationMode::Exclusive,
            MainActivationMode::Live,
            MainActivationMode::Worker,
        ] {
            let error = preflight_publication(
                MainActivationExecutor::Script,
                mode,
                &ordinary,
                Some(&config),
                Some(&params),
            )
            .unwrap_err();
            assert!(
                matches!(error, MainActivationError::SafetyGate(_)),
                "{error}"
            );
            assert!(error.to_string().contains("refused before any write"));
            preflight_publication(MainActivationExecutor::Script, mode, &ordinary, None, None)
                .unwrap();
        }
        preflight_publication(
            MainActivationExecutor::Script,
            MainActivationMode::Online,
            &ordinary,
            Some(&config),
            Some(&params),
        )
        .unwrap();
        // The config apply folds the generations: it is asked for the exclusive mode, and only for that one
        // (#408 step 2). The live and worker modes keep refusing whatever executor is named.
        preflight_publication(
            MainActivationExecutor::ConfigApply,
            MainActivationMode::Exclusive,
            &ordinary,
            Some(&config),
            Some(&params),
        )
        .unwrap();
        for mode in [MainActivationMode::Live, MainActivationMode::Worker] {
            assert!(
                preflight_publication(
                    MainActivationExecutor::ConfigApply,
                    mode,
                    &ordinary,
                    Some(&config),
                    Some(&params),
                )
                .is_err(),
                "{mode:?}"
            );
        }
    }

    #[test]
    fn preflight_reads_the_markers_against_the_ordinary_versions() {
        let ordinary = row("versions", versions(OLD));
        // Markers of another ordinary generation: the state a stage made on the
        // leaked view of an export used to be refused with.
        let (config, params) = markers(NEW, &[NEW]);
        let error = preflight_publication(
            MainActivationExecutor::Script,
            MainActivationMode::Online,
            &ordinary,
            Some(&config),
            Some(&params),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("ordinary generation disagrees with versions"),
            "{error}"
        );
        // One marker alone.
        let (config, _) = markers(OLD, &[NEW]);
        let error = preflight_publication(
            MainActivationExecutor::Script,
            MainActivationMode::Online,
            &ordinary,
            Some(&config),
            None,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("must both be present or both absent"),
            "{error}"
        );
        // A versions row that is not one.
        let broken = row("versions", b"not deflated".to_vec());
        assert!(
            preflight_publication(
                MainActivationExecutor::Script,
                MainActivationMode::Online,
                &broken,
                None,
                None
            )
            .is_err()
        );
    }

    #[test]
    fn preflight_and_the_plan_make_the_same_checks() {
        // The plan of an online promotion on a database with a generation.
        let (config, params) = markers(OLD, &[BODY]);
        let staged = vec![
            row(BODY, b"new descriptor".to_vec()),
            row(&format!("{BODY}.0"), b"new body".to_vec()),
            row("root", b"new root".to_vec()),
            row("version", b"new version".to_vec()),
            row("versions", versions(NEW)),
        ];
        let active = vec![
            row(BODY, b"old descriptor".to_vec()),
            row(&format!("{BODY}.0"), b"old body".to_vec()),
            row("root", b"old root".to_vec()),
            row("version", b"old version".to_vec()),
            row("versions", versions(OLD)),
        ];
        let snapshot = |config: &MainStorageRow, params: &MainStorageRow| MainActivationSnapshot {
            config_rows: active.clone(),
            config_dynamically_updated: Some(config.clone()),
            params_dynamically_updated: Some(params.clone()),
        };
        let targets = [BODY.to_owned(), format!("{BODY}.0")];
        for (executor, mode) in [
            (MainActivationExecutor::Script, MainActivationMode::Online),
            (
                MainActivationExecutor::Script,
                MainActivationMode::Exclusive,
            ),
            (MainActivationExecutor::Script, MainActivationMode::Live),
            (MainActivationExecutor::Script, MainActivationMode::Worker),
            (
                MainActivationExecutor::ConfigApply,
                MainActivationMode::Exclusive,
            ),
        ] {
            let plan = prepare_main_activation_for(
                executor,
                mode,
                staged.clone(),
                snapshot(&config, &params),
                &targets,
                true,
            );
            let preflight =
                preflight_publication(executor, mode, &active[4], Some(&config), Some(&params));
            assert_eq!(plan.is_ok(), preflight.is_ok(), "{executor:?} {mode:?}");
            if let (Err(plan), Err(preflight)) = (&plan, &preflight) {
                assert_eq!(plan, preflight, "{executor:?} {mode:?}");
            }
        }
    }

    #[test]
    fn preflight_takes_a_tail_for_live_only() {
        let tail = Some(r"C:\tail.trn");
        assert!(preflight_tail_log(MainActivationMode::Live, tail, true).is_ok());
        assert!(preflight_tail_log(MainActivationMode::Live, None, false).is_ok());
        assert!(matches!(
            preflight_tail_log(MainActivationMode::Live, None, true),
            Err(MainActivationError::SafetyGate(_))
        ));
        assert!(matches!(
            preflight_tail_log(MainActivationMode::Live, Some("bad\npath"), true),
            Err(MainActivationError::SafetyGate(_))
        ));
        for mode in [
            MainActivationMode::Exclusive,
            MainActivationMode::Online,
            MainActivationMode::Worker,
        ] {
            assert!(preflight_tail_log(mode, None, true).is_ok());
            assert!(matches!(
                preflight_tail_log(mode, tail, false),
                Err(MainActivationError::SafetyGate(_))
            ));
        }
    }

    #[test]
    fn live_requires_a_bounded_tail_path_and_other_modes_reject_it() {
        assert!(matches!(
            render_main_activation_sql("lab", &fixture(MainActivationMode::Live), None),
            Err(MainActivationError::SafetyGate(_))
        ));
        assert!(matches!(
            render_main_activation_sql(
                "lab",
                &fixture(MainActivationMode::Exclusive),
                Some(r"C:\tail.trn")
            ),
            Err(MainActivationError::SafetyGate(_))
        ));
        assert!(matches!(
            render_main_activation_sql(
                "lab",
                &fixture(MainActivationMode::Live),
                Some("bad\npath")
            ),
            Err(MainActivationError::SafetyGate(_))
        ));
    }

    #[test]
    fn worker_promotes_ordinary_rows_without_database_recovery() {
        let script =
            render_main_activation_sql("lab", &fixture(MainActivationMode::Worker), None).unwrap();
        assert!(
            script
                .sql
                .contains("DELETE FROM dbo.Config WHERE FileName=N'root'")
        );
        assert!(
            !script
                .sql
                .contains("exclusive activation requires no other database sessions")
        );
        assert!(!script.sql.contains("SET SINGLE_USER"));
        assert!(!script.sql.contains("BACKUP LOG"));
        assert!(!script.sql.contains("RESTORE DATABASE"));
        assert!(script.report.live_session_switch_expected);
        assert!(!script.report.requires_tail_log_artifact);
    }

    #[test]
    fn exact_sha_and_row_set_guards_precede_mutations() {
        let script =
            render_main_activation_sql("lab", &fixture(MainActivationMode::Exclusive), None)
                .unwrap();
        let guard = script.sql.find("SHA2_256").unwrap();
        let mutation = script.sql.find("DELETE FROM dbo.Config WHERE").unwrap();
        assert!(guard < mutation);
        assert!(script.sql.contains(
            "EXCEPT SELECT FileName,PartNo,Creation,Modified,Attributes,DataSize,ByteLength,Digest"
        ));
        assert!(script.sql.contains("ConfigSave exact snapshot drifted"));
    }

    #[test]
    fn fails_closed_without_acknowledgement_or_for_unknown_target() {
        let plan = fixture(MainActivationMode::Exclusive);
        let snapshot = MainActivationSnapshot {
            config_rows: plan.active_rows.clone(),
            config_dynamically_updated: None,
            params_dynamically_updated: None,
        };
        assert!(matches!(
            prepare_main_activation(
                MainActivationMode::Exclusive,
                plan.staged_rows.clone(),
                snapshot.clone(),
                &[BODY.to_owned(), format!("{BODY}.0")],
                false
            ),
            Err(MainActivationError::SafetyGate(_))
        ));
        assert!(matches!(
            prepare_main_activation(
                MainActivationMode::Exclusive,
                plan.staged_rows,
                snapshot,
                &[],
                true
            ),
            Err(MainActivationError::StructuralTarget(_))
        ));
    }

    #[test]
    fn rejects_missing_service_rows_duplicates_parts_and_reused_generation() {
        let plan = fixture(MainActivationMode::Exclusive);
        let mut missing = plan.staged_rows.clone();
        missing.retain(|row| row.file_name != "root");
        assert!(
            prepare_main_activation(
                MainActivationMode::Exclusive,
                missing,
                MainActivationSnapshot {
                    config_rows: plan.active_rows.clone(),
                    config_dynamically_updated: None,
                    params_dynamically_updated: None
                },
                &[BODY.to_owned(), format!("{BODY}.0")],
                true
            )
            .is_err()
        );

        let mut multipart = plan.staged_rows.clone();
        multipart[0].part_no = 1;
        assert!(
            prepare_main_activation(
                MainActivationMode::Exclusive,
                multipart,
                MainActivationSnapshot {
                    config_rows: plan.active_rows.clone(),
                    config_dynamically_updated: None,
                    params_dynamically_updated: None
                },
                &[BODY.to_owned(), format!("{BODY}.0")],
                true
            )
            .is_err()
        );

        let mut reused = plan.staged_rows;
        let versions_row = reused
            .iter_mut()
            .find(|row| row.file_name == "versions")
            .unwrap();
        versions_row.binary_data = versions(OLD);
        versions_row.data_size = versions_row.binary_data.len() as u64;
        assert!(matches!(
            prepare_main_activation(
                MainActivationMode::Exclusive,
                reused,
                MainActivationSnapshot {
                    config_rows: plan.active_rows,
                    config_dynamically_updated: None,
                    params_dynamically_updated: None
                },
                &[BODY.to_owned(), format!("{BODY}.0")],
                true
            ),
            Err(MainActivationError::Versions(_))
        ));
    }

    #[test]
    fn unchanged_payload_is_noop_that_only_validates_and_clears_stage() {
        let active = fixture(MainActivationMode::Exclusive).active_rows;
        let plan = prepare_main_activation(
            MainActivationMode::Exclusive,
            active.clone(),
            MainActivationSnapshot {
                config_rows: active,
                config_dynamically_updated: None,
                params_dynamically_updated: None,
            },
            &[BODY.to_owned(), format!("{BODY}.0")],
            true,
        )
        .unwrap();
        assert!(plan.is_no_op());
        let script = render_main_activation_sql("lab", &plan, None).unwrap();
        assert!(!script.sql.contains("exclusive promotion source drifted"));
        assert!(script.sql.contains("DELETE FROM dbo.ConfigSave"));
    }

    #[test]
    fn recovery_token_covers_prior_rows_and_markers() {
        let plan = fixture(MainActivationMode::Online);
        let report = plan.dry_run_report();
        assert_eq!(report.recovery_token.len(), 64);
        assert_eq!(plan.recovery().overwritten_config_rows.len(), 2);
        assert_eq!(plan.recovery().retained_config_rows.len(), 3);
        assert_eq!(report.touched_tables, ["Config", "ConfigSave", "Params"]);
    }

    #[test]
    fn online_recovery_distinguishes_replaced_rows_from_retained_preimages() {
        let plan = fixture(MainActivationMode::Online);
        let recovery = plan.recovery();
        assert_eq!(
            recovery
                .overwritten_config_rows
                .iter()
                .map(|row| row.file_name.as_str())
                .collect::<Vec<_>>(),
            ["root", "version"]
        );
        assert_eq!(
            recovery.retained_config_rows,
            plan.active_rows
                .iter()
                .filter(|row| !matches!(row.file_name.as_str(), "root" | "version"))
                .cloned()
                .collect::<Vec<_>>()
        );
        let report = plan.dry_run_report();
        assert_eq!(
            report.dynamic_aliases,
            [
                format!("{BODY}_dynupdate_{NEW}"),
                format!("{BODY}_dynupdate_{NEW}.0"),
                format!("versions_dynupdate_{NEW}")
            ]
        );
        assert_eq!(
            report.dynamic_markers,
            ["Config.DynamicallyUpdated", "Params.DynamicallyUpdated"]
        );
        let decoded: MainActivationRecoverySnapshot =
            serde_json::from_slice(&serde_json::to_vec(recovery).unwrap()).unwrap();
        assert_eq!(&decoded, recovery);
    }

    #[test]
    fn no_op_recovery_does_not_claim_config_replacements_or_new_aliases() {
        let base = fixture(MainActivationMode::Online);
        let plan = prepare_main_activation(
            MainActivationMode::Online,
            base.active_rows.clone(),
            MainActivationSnapshot {
                config_rows: base.active_rows.clone(),
                config_dynamically_updated: None,
                params_dynamically_updated: None,
            },
            &[BODY.to_owned(), format!("{BODY}.0")],
            true,
        )
        .unwrap();
        assert!(plan.is_no_op());
        assert!(plan.recovery().overwritten_config_rows.is_empty());
        assert_eq!(plan.recovery().retained_config_rows, base.active_rows);
        assert!(plan.dry_run_report().dynamic_aliases.is_empty());
        assert!(plan.dry_run_report().dynamic_markers.is_empty());
    }

    #[test]
    fn live_recovery_serializes_exactly_as_the_historical_token_input() {
        #[derive(Serialize)]
        struct LegacySnapshot<'a> {
            old_generation: &'a str,
            new_generation: &'a str,
            overwritten_config_rows: &'a [MainStorageRow],
            prior_config_dynamically_updated: &'a Option<MainStorageRow>,
            prior_params_dynamically_updated: &'a Option<MainStorageRow>,
            staged_rows: &'a [MainStorageRow],
        }
        let plan = fixture(MainActivationMode::Live);
        let current = plan.recovery();
        let legacy = LegacySnapshot {
            old_generation: &current.old_generation,
            new_generation: &current.new_generation,
            overwritten_config_rows: &plan.active_rows,
            prior_config_dynamically_updated: &current.prior_config_dynamically_updated,
            prior_params_dynamically_updated: &current.prior_params_dynamically_updated,
            staged_rows: &current.staged_rows,
        };
        let bytes = serde_json::to_vec(&legacy).unwrap();
        assert_eq!(serde_json::to_vec(current).unwrap(), bytes);
        assert_eq!(
            plan.dry_run_report().recovery_token,
            hex(&Sha256::digest(&bytes))
        );
        let historical: MainActivationRecoverySnapshot = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(&historical, current);
        assert!(current.retained_config_rows.is_empty());
    }

    #[test]
    fn header_only_drift_is_bound_to_recovery_and_sql_before_any_write() {
        let base = fixture(MainActivationMode::Online);
        for field in ["creation", "modified", "attributes"] {
            let mut active = base.active_rows.clone();
            match field {
                "creation" => active[0].creation = "4026-10-01 01:02:03.004".to_owned(),
                "modified" => active[0].modified = "4026-10-01 01:02:04.007".to_owned(),
                _ => active[0].attributes = 17,
            }
            let plan = prepare_main_activation(
                MainActivationMode::Online,
                base.staged_rows.clone(),
                MainActivationSnapshot {
                    config_rows: active,
                    config_dynamically_updated: None,
                    params_dynamically_updated: None,
                },
                &base.changed_targets,
                true,
            )
            .unwrap();
            assert_ne!(
                plan.dry_run_report().recovery_token,
                base.dry_run_report().recovery_token
            );
            let sql = render_main_activation_sql("lab", &plan, None).unwrap().sql;
            let cas = sql.find("Config selected snapshot drifted").unwrap();
            let write = sql.find("DELETE FROM dbo.Config WHERE").unwrap();
            assert!(cas < write);
            assert!(sql[..cas].contains("CONVERT(varchar(27),T.Creation,121)=E.Creation"));
            assert!(sql[..cas].contains("CONVERT(varchar(27),T.Modified,121)=E.Modified"));
            assert!(sql[..cas].contains("CONVERT(int,T.Attributes)=E.Attributes"));
            assert!(sql[..cas].contains("DATALENGTH(T.BinaryData)"));
        }
    }

    #[test]
    fn markers_carry_the_platform_year_offset() {
        // Native rows store their times 2000 years ahead (`4026-…`), as every
        // other row this crate writes does (#409 F-7).
        let script =
            render_main_activation_sql("lab", &fixture(MainActivationMode::Online), None).unwrap();
        let marker_lines = script
            .sql
            .lines()
            .filter(|line| line.contains("N'DynamicallyUpdated',"))
            .collect::<Vec<_>>();
        assert_eq!(marker_lines.len(), 2);
        for line in marker_lines {
            assert_eq!(
                line.matches("SYSUTCDATETIME()").count(),
                line.matches("DATEADD(year,2000,SYSUTCDATETIME())").count(),
                "{line}"
            );
        }
    }

    #[test]
    fn the_report_names_the_rows_and_markers_each_mode_writes() {
        let online = fixture(MainActivationMode::Online).dry_run_report();
        let generation = &online.new_generation;
        assert!(online.published_config_rows.contains(&"root".to_owned()));
        assert!(
            online
                .published_config_rows
                .contains(&format!("versions_dynupdate_{generation}"))
        );
        assert!(
            online
                .published_config_rows
                .iter()
                .filter(|name| !matches!(name.as_str(), "root" | "version"))
                .all(|name| name.contains(&format!("_dynupdate_{generation}")))
        );
        assert_eq!(
            online.dynamic_marker_actions,
            [
                "Config.DynamicallyUpdated: written",
                "Params.DynamicallyUpdated: written"
            ]
        );

        let exclusive = fixture(MainActivationMode::Exclusive).dry_run_report();
        assert!(
            exclusive
                .published_config_rows
                .contains(&"versions".to_owned())
        );
        assert!(
            exclusive
                .published_config_rows
                .iter()
                .all(|name| !name.contains("_dynupdate_"))
        );
    }

    #[test]
    fn prior_dynamic_marker_overrides_stale_ordinary_versions_generation() {
        let base = fixture(MainActivationMode::Online);
        let current = "a05f2e61-a8a0-4d85-999b-663afc575ced";
        let config_marker = row(
            "DynamicallyUpdated",
            utf8_bom(&format!("{{1,1,{current}}}")),
        );
        let params_marker = row(
            "DynamicallyUpdated",
            utf8_bom(&format!("{{0,2,{OLD},{current}}}")),
        );
        let plan = prepare_main_activation(
            MainActivationMode::Online,
            base.staged_rows,
            MainActivationSnapshot {
                config_rows: base.active_rows,
                config_dynamically_updated: Some(config_marker),
                params_dynamically_updated: Some(params_marker),
            },
            &[BODY.to_owned(), format!("{BODY}.0")],
            true,
        )
        .unwrap();
        assert_eq!(plan.old_generation().to_string(), current);
        let script = render_main_activation_sql("lab", &plan, None).unwrap();
        let expected_config = hex(&utf8_bom(&format!("{{1,2,{current},{NEW}}}")));
        let expected_params = hex(&utf8_bom(&format!("{{0,3,{OLD},{current},{NEW}}}")));
        assert!(script.sql.contains(&expected_config));
        assert!(script.sql.contains(&expected_params));
    }

    // The database of the #344 evidence (clone a1): ordinary generation `848a0a59-...`, two
    // online generations `4832dd20-...` and `96eee589-...`; the promotion stages an unrelated
    // module. The markers are exactly the ones the tool wrote there.
    const EVIDENCE_ORDINARY: &str = "848a0a59-8803-445f-b89d-3cfae4f98bbd";
    const EVIDENCE_G1: &str = "4832dd20-0b9e-45e3-a3e7-095525bf9b3f";
    const EVIDENCE_G2: &str = "96eee589-ca6c-4269-8b14-1ca1ee9b69ed";

    /// The promotion the script of this module would carry out (`--sqlcmd`, live, worker).
    fn database_with_online_generations(
        mode: MainActivationMode,
        staged_is_noop: bool,
    ) -> Result<MainActivationPlan, MainActivationError> {
        database_with_online_generations_for(MainActivationExecutor::Script, mode, staged_is_noop)
    }

    fn database_with_online_generations_for(
        executor: MainActivationExecutor,
        mode: MainActivationMode,
        staged_is_noop: bool,
    ) -> Result<MainActivationPlan, MainActivationError> {
        let base = fixture(MainActivationMode::Exclusive);
        let mut active = base.active_rows.clone();
        // Ordinary `versions` of that database carries the ordinary generation.
        let versions_row = active
            .iter_mut()
            .find(|r| r.file_name == "versions")
            .unwrap();
        versions_row.binary_data = versions(EVIDENCE_ORDINARY);
        versions_row.data_size = versions_row.binary_data.len() as u64;
        let staged = if staged_is_noop {
            active.clone()
        } else {
            base.staged_rows.clone()
        };
        prepare_main_activation_for(
            executor,
            mode,
            staged,
            MainActivationSnapshot {
                config_rows: active,
                config_dynamically_updated: Some(row(
                    "DynamicallyUpdated",
                    utf8_bom(&format!("{{1,2,{EVIDENCE_G1},{EVIDENCE_G2}}}")),
                )),
                params_dynamically_updated: Some(row(
                    "DynamicallyUpdated",
                    utf8_bom(&format!(
                        "{{0,3,{EVIDENCE_ORDINARY},{EVIDENCE_G1},{EVIDENCE_G2}}}"
                    )),
                )),
            },
            &[BODY.to_owned(), format!("{BODY}.0")],
            true,
        )
    }

    #[test]
    fn ordinary_promotion_refuses_a_database_with_online_generations() {
        // The script of this module (the `--sqlcmd` exclusive route, live, worker) refuses it, as step 1 of #408
        // made it. #408 / F-4: exclusive, live and worker deleted both markers and left the
        // `_dynupdate_` rows behind, so the earlier online changes vanished (measured on a
        // session: the original module text and no function of the second generation).
        for mode in [
            MainActivationMode::Exclusive,
            MainActivationMode::Live,
            MainActivationMode::Worker,
        ] {
            let error = database_with_online_generations(mode, false).unwrap_err();
            let MainActivationError::SafetyGate(message) = &error else {
                panic!("{mode:?}: expected a safety-gate refusal, got {error}");
            };
            assert!(
                message.contains("refused before any write"),
                "{mode:?}: {message}"
            );
            assert!(
                message.contains("earlier online changes"),
                "{mode:?}: {message}"
            );
            assert!(message.contains("would be lost"), "{mode:?}: {message}");
            assert!(
                message.contains("mssql-config-apply"),
                "{mode:?}: {message}"
            );
            assert!(message.contains("native"), "{mode:?}: {message}");
            assert!(message.contains(mode_name(mode)), "{mode:?}: {message}");
            // only the exclusive mode has another way: the config apply on the built-in SQL client
            assert_eq!(
                message.contains("leave out --sqlcmd"),
                mode == MainActivationMode::Exclusive,
                "{mode:?}: {message}"
            );
        }
    }

    #[test]
    fn online_still_extends_the_history_and_an_online_noop_is_never_refused() {
        let online = database_with_online_generations(MainActivationMode::Online, false).unwrap();
        assert_eq!(online.old_generation().to_string(), EVIDENCE_G2);
        let script = render_main_activation_sql("lab", &online, None).unwrap();
        assert!(!script.sql.contains("57208"));
        assert!(script.sql.contains(&hex(&utf8_bom(&format!(
            "{{1,3,{EVIDENCE_G1},{EVIDENCE_G2},{NEW}}}"
        )))));
        // An online stage that equals the rows it extends changes nothing and is not refused.
        let noop = database_with_online_generations(MainActivationMode::Online, true).unwrap();
        assert!(noop.is_no_op());
        // Step 1 called a promotion of a stage equal to the ordinary rows a no-op on a database with markers, too.
        // The ordinary rows are not the configuration of such a database, though: the aliases are, so the stage
        // (an online change taken back to the original text) does change it, and nothing reports "no change".
        // The script refuses (it would delete the markers and leave the aliases), the config apply folds.
        for mode in [
            MainActivationMode::Exclusive,
            MainActivationMode::Live,
            MainActivationMode::Worker,
        ] {
            assert!(
                database_with_online_generations(mode, true).is_err(),
                "{mode:?}"
            );
        }
    }

    // #408 step 2 (docs/apply/online-activation.md, 6.6): the `exclusive` mode is handed to `mssql_config_apply`.
    //
    // The F-4 database of the #344 evidence (an ordinary generation and two online generations) and the
    // promotion of an unrelated module. What the user needs is that the earlier online changes are still
    // the configuration afterwards: the apply that carries the `exclusive` mode out folds the `_dynupdate_`
    // rows into the ordinary rows as the native apply does, so this module must accept the promotion, name
    // that apply as its executor and never render it as a script. `live` and `worker` stay with the script
    // and keep refusing until the worker lab cluster exists.
    #[test]
    fn f4_an_exclusive_promotion_on_a_database_with_online_generations_is_carried_out_by_the_config_apply()
     {
        let plan = database_with_online_generations_for(
            MainActivationExecutor::ConfigApply,
            MainActivationMode::Exclusive,
            false,
        )
        .expect(
            "an exclusive promotion is accepted on a database with online generations: the config apply folds them",
        );
        assert_eq!(plan.executor(), MainActivationExecutor::ConfigApply);
        assert!(plan.is_carried_out_by_config_apply());
        assert_eq!(
            plan.old_generation().to_string(),
            EVIDENCE_G2,
            "the generation the promotion starts from is the last online one"
        );
        assert!(
            render_main_activation_sql("lab", &plan, None).is_err(),
            "no script of this module carries an exclusive promotion out"
        );
        for mode in [MainActivationMode::Live, MainActivationMode::Worker] {
            assert!(
                database_with_online_generations(mode, false).is_err(),
                "{mode:?} keeps refusing on a database with online generations"
            );
        }
        assert_eq!(
            database_with_online_generations(MainActivationMode::Online, false)
                .unwrap()
                .executor(),
            MainActivationExecutor::Script
        );
    }

    #[test]
    fn f4_a_stage_equal_to_the_ordinary_rows_is_no_noop_when_an_exclusive_promotion_has_to_fold() {
        // The ordinary rows are not the configuration of a database with online generations: the aliases are. A stage
        // that equals the ordinary rows (an online change taken back to the original text) still changes the
        // configuration, and the fold has to run.
        let plan = database_with_online_generations_for(
            MainActivationExecutor::ConfigApply,
            MainActivationMode::Exclusive,
            true,
        )
        .unwrap();
        assert!(!plan.is_no_op());
        assert_eq!(plan.executor(), MainActivationExecutor::ConfigApply);
        assert!(plan.is_carried_out_by_config_apply());
    }

    #[test]
    fn the_config_apply_carries_out_the_exclusive_mode_only_where_the_built_in_client_is_there() {
        for mode in [
            MainActivationMode::Exclusive,
            MainActivationMode::Online,
            MainActivationMode::Live,
            MainActivationMode::Worker,
        ] {
            assert_eq!(
                MainActivationExecutor::for_mode(mode, true) == MainActivationExecutor::ConfigApply,
                mode == MainActivationMode::Exclusive,
                "{mode:?}"
            );
            // `--sqlcmd`: the script, whatever the mode
            assert_eq!(
                MainActivationExecutor::for_mode(mode, false),
                MainActivationExecutor::Script,
                "{mode:?}"
            );
            assert_eq!(
                MainActivationExecutor::ConfigApply.folds_online_generations(mode),
                mode == MainActivationMode::Exclusive
            );
            assert!(!MainActivationExecutor::Script.folds_online_generations(mode));
        }
        // a plan made for the legacy runner is a script plan
        assert_eq!(
            fixture(MainActivationMode::Exclusive).executor(),
            MainActivationExecutor::Script
        );
    }

    #[test]
    fn a_config_apply_plan_is_made_for_the_exclusive_mode_only() {
        for mode in [
            MainActivationMode::Online,
            MainActivationMode::Live,
            MainActivationMode::Worker,
        ] {
            let error = database_with_online_generations_for(
                MainActivationExecutor::ConfigApply,
                mode,
                false,
            )
            .unwrap_err();
            let MainActivationError::SafetyGate(message) = &error else {
                panic!("{mode:?}: {error}");
            };
            assert!(
                message.contains("not carried out by the config apply"),
                "{message}"
            );
            assert!(message.contains(mode_name(mode)), "{message}");
        }
    }

    #[test]
    fn a_marker_free_exclusive_promotion_is_carried_out_by_the_config_apply_too() {
        // Native does more than the script replaced (`_MessageNo`, the registrations, `MobileVersions.dat`, its own
        // recovery artifact); the apply does it on a database without online generations as well.
        let base = fixture(MainActivationMode::Exclusive);
        let plan = prepare_main_activation_for(
            MainActivationExecutor::ConfigApply,
            MainActivationMode::Exclusive,
            base.staged_rows.clone(),
            MainActivationSnapshot {
                config_rows: base.active_rows.clone(),
                config_dynamically_updated: None,
                params_dynamically_updated: None,
            },
            &[BODY.to_owned(), format!("{BODY}.0")],
            true,
        )
        .unwrap();
        assert!(plan.is_carried_out_by_config_apply());
        assert_eq!(
            plan.dry_run_report().executor,
            MainActivationExecutor::ConfigApply
        );
        assert!(
            serde_json::to_string(&plan.dry_run_report())
                .unwrap()
                .contains("\"executor\":\"config_apply\"")
        );
        // no script of this module for it
        let error = render_main_activation_sql("lab", &plan, None).unwrap_err();
        assert!(error.to_string().contains("mssql_config_apply"), "{error}");
        // a stage equal to the rows of a database without markers is a no-op: the script that clears the stage is
        // all there is to do, and the apply is not asked
        let noop = prepare_main_activation_for(
            MainActivationExecutor::ConfigApply,
            MainActivationMode::Exclusive,
            base.active_rows.clone(),
            MainActivationSnapshot {
                config_rows: base.active_rows.clone(),
                config_dynamically_updated: None,
                params_dynamically_updated: None,
            },
            &[BODY.to_owned(), format!("{BODY}.0")],
            true,
        )
        .unwrap();
        assert!(noop.is_no_op());
        assert!(!noop.is_carried_out_by_config_apply());
        assert!(render_main_activation_sql("lab", &noop, None).is_ok());
    }

    #[test]
    fn ordinary_promotion_checks_for_alias_rows_in_the_transaction_before_any_write() {
        // Alias rows that no marker names (orphans of an earlier promotion, a native leftover)
        // are found by the script itself, inside the serializable transaction.
        for mode in [
            MainActivationMode::Exclusive,
            MainActivationMode::Live,
            MainActivationMode::Worker,
        ] {
            let tail = (mode == MainActivationMode::Live).then_some(r"C:\tail.trn");
            let script = render_main_activation_sql("lab", &fixture(mode), tail).unwrap();
            let check = script
                .sql
                .find("FileName LIKE N'%[_]dynupdate[_]%') THROW 57208")
                .unwrap_or_else(|| panic!("{mode:?}: no alias-row assertion"));
            let first_write = script.sql.find("DELETE FROM dbo.Config WHERE").unwrap();
            let markers = script.sql.find("THROW 57204").unwrap();
            let begin = script.sql.find("BEGIN TRANSACTION").unwrap();
            assert!(
                begin < markers && markers < check && check < first_write,
                "{mode:?}"
            );
            assert!(script.sql.contains("earlier online changes"), "{mode:?}");
            assert!(script.sql.contains("mssql-config-apply"), "{mode:?}");
        }
        let online =
            render_main_activation_sql("lab", &fixture(MainActivationMode::Online), None).unwrap();
        assert!(!online.sql.contains("57208"));
        assert!(!online.sql.contains("LIKE N'%[_]dynupdate[_]%'"));
    }

    #[test]
    fn no_mode_touches_the_platform_licensing_rows_or_the_mobile_ring() {
        // The `.ui` rows of `Params` are the platform's licensing records and `Files.MobileVersions.dat` is
        // its mobile-client ring: no publication mode writes either (docs/apply/params-ui.md).
        for mode in [
            MainActivationMode::Exclusive,
            MainActivationMode::Online,
            MainActivationMode::Live,
            MainActivationMode::Worker,
        ] {
            let tail = (mode == MainActivationMode::Live).then_some(r"C:\tail.trn");
            let script = render_main_activation_sql("lab", &fixture(mode), tail).unwrap();
            assert!(!script.sql.contains(".ui"), "{mode:?}");
            assert!(!script.sql.contains("MobileVersions"), "{mode:?}");
        }
    }
}
