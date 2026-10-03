//! `ibcmd infobase config apply` of the drop-in command line: the platform's
//! syntax (`--force`, `--dynamic`, `--session-terminate`, ...) served by the
//! own exclusive apply (`crate::mssql_config_apply`), and the platform's words
//! and exit codes for what comes back.
//!
//! What is served, and why (measured on 8.3.27.2214; the evidence and the
//! whole table are in `docs/apply/dropin-apply.md`):
//!
//! - the platform's default, `--dynamic=auto`, applies exactly like
//!   `--dynamic=disable` when the exclusive lock can be taken: the staged rows
//!   replace the active ones in place, no `_dynupdate_` rows are written. So
//!   `auto`, `disable` and noninteractive `prompt` retain that route. Interactive
//!   `prompt` may offer a SQL-guard fallback after a typed pre-write session refusal:
//!   cancel, one exclusive retry, or an explicitly chosen dynamic apply. It does not
//!   implement native standalone-server mode or session administration;
//! - `--dynamic=force` (dynamic update only) is served by the dynamic apply
//!   (`crate::mssql_config_apply::dynamic`, `docs/apply/dropin-dynamic.md`,
//!   the second seam [`call_apply_dynamic`]): a small stage of common-module
//!   and common-form bodies is published as a generation while sessions stay
//!   connected, with the writes the platform's own `force` makes; any other
//!   stage is refused with `требуется штатный config apply: <reasons>` (exit
//!   1), and a platform without the capability is named as not served. The
//!   refusal for the sessions of `auto` and `prompt` ends with the line
//!   [`DYNAMIC_HINT`] when the stage would qualify;
//! - the platform run against a database (`--dbms=...`) does not look for other
//!   sessions at all. This apply does: with sessions connected it refuses, in
//!   the platform's words for a lock it cannot take. `--session-terminate=force`
//!   and `prompt` name what this version cannot do (end sessions), so they are
//!   accepted while nobody is connected and refused when somebody is;
//! - the structural gate is always the restructuring track's S1 gate
//!   (`AllowRestructure::S1`; the platform has no option for it, and the backup
//!   option is the operator's consent): what changes no table goes as a plain
//!   apply; a restructuring of the S1 set (attributes of catalogs and
//!   documents) is done inside the apply's transaction when the operator names
//!   a way back (`--recovery-backup`, `--i-have-a-backup`) and refused as
//!   `BackupRequired` (exit 1) when not; a stage that needs any other
//!   restructuring, or anything else the own apply does not do, is refused
//!   with `требуется штатный config apply: ...` (exit 1);
//! - what the own apply refuses is sorted by the type of its error
//!   (`mssql_config_apply::errors`), never by the words of its message; an
//!   error of no type is a failure (exit -1).
//!
//! THE SEAM is [`call_apply`]: the one place the own apply is called.

use std::path::Path;

use anyhow::{Result, bail};
use serde::Serialize;
use uuid::Uuid;

use crate::infobase::{ConnectionRequest, PlatformNeed, ensure_mssql, resolve_connection};
use crate::mssql_config_apply::gate::GateVerdict;
use crate::mssql_config_apply::{
    AllowRestructure, BackupRequired, ConfigApplyOptions, ConfigApplyReport, DynamicPublication,
    DynamicUnsupported, ExclusiveAccessRefused, ExclusiveAccessUnprovable, Exclusivity,
    NativeCommand, NeedsNativeApply, OtherSession, StructuralRefusal,
};
use crate::mssql_platform_profile::MssqlNativePlatformProfile;
use crate::platform::PlatformSpec;
use crate::settings::{DatabaseTarget, PlatformHint, Settings};
use crate::sql::{SqlExec, SqlOptions};

use super::parse::{ApplyRequest, Common, DynamicMode, ExclusivityMode, SessionTerminate};
use super::{APPLY, read_requested_password, sql_server_name, write_json};

mod routing;

/// What one `config apply` came to, before it is told to the user.
#[derive(Debug)]
pub enum Outcome {
    /// The operator explicitly cancelled the SQL-guard interactive fallback.
    Cancelled,
    /// The staged configuration was applied.
    Applied(Box<ConfigApplyReport>),
    /// `ConfigSave` held nothing.
    NothingToApply(Box<ConfigApplyReport>),
    /// Not carried out because this version does not serve what the stage
    /// needs (exit 1): the reason, in words.
    Refused(String),
    /// The operation failed (exit -1): the message.
    Failed(String),
}

/// The `--report` file of an apply.
#[derive(Serialize)]
struct ApplyReportFile<'a> {
    operation: &'static str,
    ok: bool,
    nothing_to_apply: bool,
    apply: &'a ConfigApplyReport,
}

/// The sessions the messages list before "and N more".
const SESSIONS_SHOWN: usize = 10;

/// The last line of the refusal for the sessions when the staged configuration could be applied
/// dynamically (`--dynamic=force`, `crate::mssql_config_apply::dynamic`).
pub const DYNAMIC_HINT: &str = "можно применить динамически: --dynamic=force";

/// `ibcmd infobase config apply ...`: the exit code.
pub fn run(mut request: ApplyRequest) -> i32 {
    let report = request.common.report.clone();
    APPLY.start();
    if let Err(message) = read_requested_password(&mut request.common) {
        return APPLY.fail_with(&message, report.as_deref());
    }
    tell(execute(&request), report.as_deref())
}

/// Says what happened, in the platform's words, and gives the exit code.
fn tell(outcome: Outcome, report: Option<&Path>) -> i32 {
    match outcome {
        Outcome::Cancelled => {
            if let Some(path) = report
                && let Err(error) = write_json(
                    path,
                    &serde_json::json!({
                        "operation": APPLY.command, "ok": true, "cancelled": true,
                        "applied": false,
                    }),
                )
            {
                return APPLY.fail_with(&format!("{error:#}"), None);
            }
            eprintln!("[WARN] Обновление конфигурации базы данных отменено");
            0
        }
        Outcome::Applied(applied) => {
            if let Some(generation) = applied
                .new_generation
                .as_deref()
                .and_then(native_generation)
            {
                println!("[INFO] Создано поколение конфигурации: {generation}");
            }
            if let Some(warning) = applied.published.as_ref().and_then(overlay_warning) {
                eprintln!("{warning}");
            }
            let file = report_file(&applied, true, false);
            APPLY.succeed(report, &file)
        }
        Outcome::NothingToApply(applied) => {
            let file = report_file(&applied, true, true);
            if let Some(path) = report
                && let Err(error) = write_json(path, &file)
            {
                return APPLY.fail_with(&format!("{error:#}"), None);
            }
            println!("[INFO] Обновление конфигурации базы данных не требуется");
            0
        }
        Outcome::Refused(message) => APPLY.refuse_with(&message, report),
        Outcome::Failed(message) => APPLY.fail_with(&message, report),
    }
}

/// The overlay of dynamic updates grows with every generation until an exclusive apply folds it: past
/// the number the apply names (50) the platform's `[WARN]` line says so.
fn overlay_warning(published: &DynamicPublication) -> Option<String> {
    (published.history.len() > published.warn_after_generations).then(|| {
        format!(
            "[WARN] В информационной базе накоплено динамических поколений конфигурации: {}; для их свёртки используйте штатное ibcmd infobase config apply --dynamic=disable, когда к базе никто не подключён",
            published.history.len()
        )
    })
}

fn report_file(
    report: &ConfigApplyReport,
    ok: bool,
    nothing_to_apply: bool,
) -> ApplyReportFile<'_> {
    ApplyReportFile {
        operation: APPLY.command,
        ok,
        nothing_to_apply,
        apply: report,
    }
}

/// Connects, applies and classifies what came back.
///
/// Force is always dynamic. Other modes first try the exclusive apply. Only prompt, a terminal,
/// and a known pre-write session refusal can offer an explicit dynamic choice.
pub fn execute(request: &ApplyRequest) -> Outcome {
    let (sql, options) = match connect(request) {
        Ok(connected) => connected,
        Err(error) => return Outcome::Failed(format!("{error:#}")),
    };
    routing::apply(
        request,
        &options.database,
        &mut SqlDispatch {
            sql: &sql,
            options: &options,
        },
        &mut routing::TerminalInteraction,
    )
}

struct SqlDispatch<'a> {
    sql: &'a SqlExec,
    options: &'a ConfigApplyOptions,
}
impl routing::Dispatch for SqlDispatch<'_> {
    fn exclusive(&mut self) -> Result<ConfigApplyReport> {
        call_apply(self.sql, self.options)
    }
    fn dynamic(&mut self) -> Result<ConfigApplyReport> {
        call_apply_dynamic(self.sql, self.options)
    }
    fn qualifies(&mut self) -> bool {
        call_would_qualify(self.sql, self.options)
    }
}

/// Whether the refusal for the sessions may point at `--dynamic=force`: `auto` and `prompt` leave the
/// choice to the program, so it says what is possible; `disable` said no to a dynamic update, and
/// `force` is the dynamic apply itself.
fn points_at_force(mode: DynamicMode) -> bool {
    matches!(mode, DynamicMode::Auto | DynamicMode::Prompt)
}

/// THE SEAM: the one call into the own apply (`crate::mssql_config_apply`,
/// track apply #337). It applies the staged configuration exclusively and
/// asks its structural gate whether the stage needs a restructuring; the gate
/// is the apply's own default (`ApplyCheckGate` once that lands, the
/// conservative one until then). Everything above this function is the
/// platform's command line, everything below the platform's words for the
/// result.
fn call_apply(sql: &SqlExec, options: &ConfigApplyOptions) -> Result<ConfigApplyReport> {
    crate::mssql_config_apply::apply_staged_configuration(sql, options)
}

/// THE SECOND SEAM: `--dynamic=force`. The staged configuration is published as a dynamic
/// generation while sessions stay connected (`crate::mssql_config_apply::dynamic`), or refused with
/// the reasons the stage is not one.
fn call_apply_dynamic(sql: &SqlExec, options: &ConfigApplyOptions) -> Result<ConfigApplyReport> {
    crate::mssql_config_apply::dynamic::apply_dynamic(sql, options)
}

/// Whether the staged configuration would be applied dynamically: a look that writes nothing.
fn call_would_qualify(sql: &SqlExec, options: &ConfigApplyOptions) -> bool {
    crate::mssql_config_apply::dynamic::would_qualify(sql, options)
}

/// The SQL handle and the apply options of a request.
fn connect(request: &ApplyRequest) -> Result<(SqlExec, ConfigApplyOptions)> {
    let common = &request.common;
    let db_pwd_env = common
        .db_pwd_env
        .clone()
        .unwrap_or_else(|| "IBCMD_DB_PSW".to_string());
    let server = common.db_server.as_deref().map(sql_server_name);
    let config = resolve_connection(ConnectionRequest {
        settings: common.settings.as_deref(),
        native_config: common.native_config.as_deref(),
        format: None,
        platform: common.platform,
        source_version: common.source_version,
        dbms: common.dbms.as_deref(),
        db_server: server.as_deref(),
        db_name: common.db_name.as_deref(),
        db_user: common.db_user.as_deref(),
        db_pwd: common.db_pwd.as_deref(),
        db_pwd_env: &db_pwd_env,
        need: PlatformNeed::Given,
    })?;
    ensure_mssql(&config.dbms)?;
    let profile = native_profile(common, &config.db_server, &config.db_name)?;
    let sql = SqlExec::from_options(SqlOptions {
        sqlcmd: None,
        bcp: None,
        server: &config.db_server,
        user: config.db_user.as_deref(),
        password: config.db_pwd.as_deref(),
        password_env: &db_pwd_env,
        trust_server_certificate: true,
    })?;
    let mut options = apply_options(request, &config.db_name, profile);
    // No native-looking flag: the limit on the rebuilt tables comes from the settings chain
    // (IBCMD_RS_RESTRUCTURE_LIMIT_ROWS / _BYTES, `restructure-limit-rows` / `-bytes` of ibcmd-rs.toml).
    options.restructure_limit = crate::restructure::size_guard::resolve_limit(
        &Settings::load(common.native_config.as_deref())?,
        None,
        None,
    )?;
    Ok((sql, options))
}

/// The options of the own apply for a request. The platform's `--force`
/// (confirm warnings) and `--session-terminate-message` need nothing here:
/// the apply raises no warning that asks for a confirmation and ends no
/// session. The gate is always S1's: the drop-in has no option to choose one,
/// and whether a restructuring of the S1 set may be done is the backup option.
pub fn apply_options(
    request: &ApplyRequest,
    database: &str,
    profile: MssqlNativePlatformProfile,
) -> ConfigApplyOptions {
    let mut options = ConfigApplyOptions::new(database, profile);
    options.exclusivity = match request.exclusivity {
        ExclusivityMode::Sql => Exclusivity::SqlSessions,
        ExclusivityMode::Assumed => Exclusivity::Assumed,
    };
    options.backup = request.backup.clone();
    options.allow_restructure = Some(AllowRestructure::S1);
    options
}

/// The storage layout the database is claimed to have: the platform named by
/// `--platform`, the settings or, when nothing names one, 8.3.27 (the only
/// one this apply is measured on; the apply verifies the claim against the
/// database).
fn native_profile(
    common: &Common,
    server: &str,
    database: &str,
) -> Result<MssqlNativePlatformProfile> {
    let settings = Settings::load(common.native_config.as_deref())?;
    let resolved = crate::settings::resolve_platform_with_source(
        common.platform.map(|platform| platform.display()),
        &settings,
        DatabaseTarget::new(Some(server), database),
        PlatformHint::None,
    )?;
    profile_of(resolved.value)
}

/// The native storage profile of a platform: its own for an exact build; for
/// a release the build this apply was measured on.
pub fn profile_of(spec: PlatformSpec) -> Result<MssqlNativePlatformProfile> {
    if let Some(profile) = spec.native_profile() {
        return Ok(profile);
    }
    if spec.is_exact_build() {
        bail!("для точной платформы {spec} не описана раскладка хранилища конфигурации");
    }
    match spec.release() {
        [8, 3, 27] => Ok(MssqlNativePlatformProfile::Platform8_3_27_2214),
        [8, 5, 1] => Ok(MssqlNativePlatformProfile::Platform8_5_1_1150),
        _ => bail!("для платформы {spec} не описана раскладка хранилища конфигурации"),
    }
}

/// Sorts an error of the own apply into what the platform would have said,
/// by the type the apply gives it (`mssql_config_apply::errors`; every one is
/// reached through any context the error was wrapped in). Nothing is read
/// from the words of a message: an error of no type is a failure.
///
/// | error | outcome | exit |
/// |---|---|---|
/// | `StructuralRefusal` | `требуется штатный config apply: <the gate's reasons>` | 1 |
/// | `NeedsNativeApply` | `требуется штатный config apply` (or `config repair`): its reason | 1 |
/// | `BackupRequired` | its words: name `--recovery-backup` or `--i-have-a-backup` | 1 |
/// | `ExclusiveAccessRefused` | the platform's lock words and the sessions | -1 (1 when `--session-terminate` asked to end them) |
/// | `ExclusiveAccessUnprovable` | the reason and `--exclusivity=assumed` | -1 |
/// | `DynamicUnsupported` | `--dynamic=force` named as not served for the platform | 1 |
/// | anything else | the error and its context | -1 |
pub fn classify(error: &anyhow::Error, terminate: SessionTerminate) -> Outcome {
    classify_with(error, terminate, false)
}

/// [`classify`], with the hint a refusal for the sessions gives when the stage could have been
/// applied dynamically (`dynamic_hint`).
pub fn classify_with(
    error: &anyhow::Error,
    terminate: SessionTerminate,
    dynamic_hint: bool,
) -> Outcome {
    if let Some(unsupported) = error.downcast_ref::<DynamicUnsupported>() {
        return Outcome::Refused(format!(
            "Параметр `--dynamic=force` команды `{}` не поддерживается для платформы {}: {}",
            APPLY.command, unsupported.platform, unsupported.reason
        ));
    }
    if let Some(refusal) = error.downcast_ref::<StructuralRefusal>() {
        return Outcome::Refused(structural_text(&refusal.verdict));
    }
    if let Some(native) = error.downcast_ref::<NeedsNativeApply>() {
        return Outcome::Refused(native_text(native));
    }
    if error.downcast_ref::<BackupRequired>().is_some() {
        return Outcome::Refused(BackupRequired.to_string());
    }
    if let Some(refusal) = error.downcast_ref::<ExclusiveAccessRefused>() {
        return sessions_outcome(
            &refusal.sessions,
            terminate,
            &format!("{error:#}"),
            dynamic_hint,
        );
    }
    if let Some(unprovable) = error.downcast_ref::<ExclusiveAccessUnprovable>() {
        return Outcome::Failed(format!(
            "{unprovable}\nЕсли с базой никто не работает, укажите --exclusivity=assumed"
        ));
    }
    Outcome::Failed(format!("{error:#}"))
}

/// `требуется штатный config apply: <reason>`; `config repair` for a base
/// with an operation that was never finished.
pub fn native_text(native: &NeedsNativeApply) -> String {
    let command = match native.command {
        NativeCommand::Apply => "config apply",
        NativeCommand::Repair => "config repair",
    };
    format!("требуется штатный {command}: {}", native.reason)
}

/// `требуется штатный config apply: <row>: <reason>; ...`, the way the check
/// of the restructure-check track words its refusal
/// (`apply_check::Verdict::refusal`).
pub fn structural_text(verdict: &GateVerdict) -> String {
    const SHOWN: usize = 8;
    let total = verdict.blockers.len() + verdict.blockers_omitted;
    let mut parts = verdict
        .blockers
        .iter()
        .take(SHOWN)
        .map(|blocker| {
            // a reason of the gate about the stage as a whole names no row
            if blocker.row.is_empty() {
                blocker.reason.clone()
            } else {
                format!("{}: {}", blocker.row, blocker.reason)
            }
        })
        .collect::<Vec<_>>();
    if total > parts.len() {
        parts.push(format!("и ещё {}", total - parts.len()));
    }
    if parts.is_empty() {
        parts.push("причина не названа".to_string());
    }
    format!("требуется штатный config apply: {}", parts.join("; "))
}

/// Other sessions keep the exclusive lock from being taken.
fn sessions_outcome(
    sessions: &[OtherSession],
    terminate: SessionTerminate,
    detail: &str,
    dynamic_hint: bool,
) -> Outcome {
    let mut listing = sessions_text(sessions, detail);
    if dynamic_hint {
        listing.push('\n');
        listing.push_str(DYNAMIC_HINT);
    }
    match terminate {
        SessionTerminate::Disable => Outcome::Failed(listing),
        SessionTerminate::Prompt | SessionTerminate::Force => Outcome::Refused(format!(
            "Параметр `--session-terminate={}` команды `infobase config apply` не поддерживается \
в этой версии ibcmd-rs (планируется в следующих): к базе подключены сеансы, а ibcmd-rs не завершает \
сеансы, закройте их сами\n{listing}",
            terminate.as_str()
        )),
    }
}

/// The platform's `Ошибка исключительной блокировки информационной базы.`
/// and its list of active sessions, from what SQL Server shows.
pub fn sessions_text(sessions: &[OtherSession], detail: &str) -> String {
    let mut out = String::from("Ошибка исключительной блокировки информационной базы.");
    if sessions.is_empty() {
        out.push_str(&format!("\nК базе подключены другие сеансы ({detail})"));
        return out;
    }
    out.push_str("\nАктивные сеансы и соединения:");
    let shown = sessions.len().min(SESSIONS_SHOWN);
    for (index, session) in sessions.iter().take(shown).enumerate() {
        let end = if index + 1 == sessions.len() { "" } else { ";" };
        out.push_str(&format!(
            "\nкомпьютер: {}, приложение: {}, соединение с СУБД: {} ({}, {}){end}",
            or_unknown(&session.host),
            or_unknown(&session.program),
            session.session_id,
            or_unknown(&session.login),
            session.status,
        ));
    }
    if sessions.len() > shown {
        out.push_str(&format!("\nи ещё {}", sessions.len() - shown));
    }
    out.push_str(
        "\nЗакройте их (если рабочий процесс лишь держит соединение из пула, остановите его) и повторите",
    );
    out
}

fn or_unknown(value: &str) -> &str {
    if value.is_empty() { "?" } else { value }
}

/// The generation as the platform prints it after `Создано поколение
/// конфигурации:`: the sixteen bytes of the GUID as stored (little-endian
/// fields), in hex, and eight zeros (measured: the generation
/// `40cfe0ac-6f3b-4851-85ba-295e568663f6` of a `versions` row is printed
/// `ace0cf403b6f514885ba295e568663f600000000`).
pub fn native_generation(generation: &str) -> Option<String> {
    let uuid = Uuid::parse_str(generation).ok()?;
    let mut text = uuid
        .to_bytes_le()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    text.push_str("00000000");
    Some(text)
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;

    use super::*;
    use crate::dropin::parse::{Invocation, parse_infobase};
    use crate::mssql_config_apply::gate::{GateBlocker, GateVerdict};
    use crate::mssql_config_apply::{ApplyMode, BackupPolicy};

    pub(super) fn request(list: &[&str]) -> ApplyRequest {
        let args = list
            .iter()
            .map(std::ffi::OsString::from)
            .collect::<Vec<_>>();
        match parse_infobase(&args) {
            Ok(Invocation::Apply(request)) => request,
            other => panic!("{list:?}: {other:?}"),
        }
    }

    pub(super) fn session(id: i64, host: &str, program: &str) -> OtherSession {
        OtherSession {
            session_id: id,
            login: "sa".to_string(),
            host: host.to_string(),
            program: program.to_string(),
            status: "sleeping".to_string(),
            last_request_end: String::new(),
        }
    }

    #[test]
    fn the_generation_is_printed_as_the_platform_prints_it() {
        // measured on 8.3.27.2214: the line of the apply and the head of the
        // `versions` row it left
        assert_eq!(
            native_generation("40cfe0ac-6f3b-4851-85ba-295e568663f6").as_deref(),
            Some("ace0cf403b6f514885ba295e568663f600000000")
        );
        assert_eq!(
            native_generation("a3d987b5-e2d0-6e47-b11e-2b29ebd47040").as_deref(),
            Some("b587d9a3d0e2476eb11e2b29ebd4704000000000")
        );
        assert_eq!(native_generation("not a uuid"), None);
    }

    #[test]
    fn the_structural_refusal_is_the_checks_words() {
        let mut verdict = GateVerdict::default();
        assert_eq!(
            structural_text(&verdict),
            "требуется штатный config apply: причина не названа"
        );
        verdict.restructuring_required = true;
        verdict.blockers = vec![
            GateBlocker {
                row: "aaa.0".to_string(),
                reason: "descriptor differs".to_string(),
            },
            GateBlocker {
                row: "bbb".to_string(),
                reason: "new object".to_string(),
            },
        ];
        assert_eq!(
            structural_text(&verdict),
            "требуется штатный config apply: aaa.0: descriptor differs; bbb: new object"
        );
        verdict.blockers_omitted = 3;
        for index in 0..7 {
            verdict.blockers.push(GateBlocker {
                row: format!("r{index}"),
                reason: "x".to_string(),
            });
        }
        let text = structural_text(&verdict);
        assert!(text.ends_with("; и ещё 4"), "{text}");
        assert_eq!(text.matches("; ").count(), 8);
    }

    #[test]
    fn a_structural_refusal_is_exit_one_material() {
        let verdict = GateVerdict {
            restructuring_required: true,
            blockers: vec![GateBlocker {
                row: "row".to_string(),
                reason: "restructuring".to_string(),
            }],
            ..GateVerdict::default()
        };
        let error = anyhow::Error::new(StructuralRefusal { verdict });
        match classify(&error, SessionTerminate::Disable) {
            Outcome::Refused(text) => {
                assert_eq!(text, "требуется штатный config apply: row: restructuring")
            }
            other => panic!("{other:?}"),
        }
    }

    fn refused_by_sessions(sessions: Vec<OtherSession>, in_transaction: bool) -> anyhow::Error {
        anyhow::Error::new(ExclusiveAccessRefused {
            database: "db".to_string(),
            sessions,
            in_transaction,
        })
    }

    #[test]
    fn connected_sessions_are_refused_in_the_platforms_words() {
        let sessions = vec![
            session(57, "DESKTOP-1", "1CV8"),
            session(61, "DESKTOP-1", "1CV8C"),
        ];
        let error = refused_by_sessions(sessions, false);
        match classify(&error, SessionTerminate::Disable) {
            Outcome::Failed(text) => {
                assert!(text.starts_with(
                    "Ошибка исключительной блокировки информационной базы.\nАктивные сеансы и соединения:\n"
                ));
                assert!(text.contains(
                    "компьютер: DESKTOP-1, приложение: 1CV8, соединение с СУБД: 57 (sa, sleeping);\n"
                ));
                // the last one ends without a semicolon
                assert!(text.contains("1CV8C, соединение с СУБД: 61 (sa, sleeping)\n"));
                assert!(text.ends_with("остановите его) и повторите"));
            }
            other => panic!("{other:?}"),
        }
        // a wish to terminate them is a wish this version cannot grant
        for terminate in [SessionTerminate::Prompt, SessionTerminate::Force] {
            match classify(&error, terminate) {
                Outcome::Refused(text) => {
                    assert!(
                        text.starts_with(&format!(
                            "Параметр `--session-terminate={}` команды `infobase config apply` не поддерживается в этой версии ibcmd-rs",
                            terminate.as_str()
                        )),
                        "{text}"
                    );
                    assert!(text.contains("компьютер: DESKTOP-1"), "{text}");
                }
                other => panic!("{other:?}"),
            }
        }
        // the sessions could not be listed: the apply's own words follow
        match classify(
            &refused_by_sessions(Vec::new(), false),
            SessionTerminate::Disable,
        ) {
            Outcome::Failed(text) => {
                assert!(text.contains("К базе подключены другие сеансы"), "{text}");
                assert!(
                    text.contains("exclusive access is not established"),
                    "{text}"
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_refusal_for_the_sessions_points_at_force_when_the_stage_would_qualify() {
        let error = refused_by_sessions(vec![session(57, "DESKTOP-1", "1CV8")], false);
        // without the hint it is what it always was
        let plain = match classify(&error, SessionTerminate::Disable) {
            Outcome::Failed(text) => text,
            other => panic!("{other:?}"),
        };
        assert!(!plain.contains("--dynamic=force"), "{plain}");
        // with it, one more line, the last, and the exit code is the same (-1)
        match classify_with(&error, SessionTerminate::Disable, true) {
            Outcome::Failed(text) => {
                assert_eq!(
                    text,
                    format!("{plain}\nможно применить динамически: --dynamic=force")
                );
                assert_eq!(text.lines().last(), Some(DYNAMIC_HINT));
            }
            other => panic!("{other:?}"),
        }
        // the sessions and a wish to end them: still «не поддерживается», still exit 1, and the hint is there
        match classify_with(&error, SessionTerminate::Force, true) {
            Outcome::Refused(text) => {
                assert!(text.contains("не поддерживается"), "{text}");
                assert!(text.ends_with(DYNAMIC_HINT), "{text}");
            }
            other => panic!("{other:?}"),
        }
        // only a refusal for the sessions carries it
        match classify_with(
            &anyhow::Error::new(NeedsNativeApply::apply("a deleted list")),
            SessionTerminate::Disable,
            true,
        ) {
            Outcome::Refused(text) => assert!(!text.contains("--dynamic=force"), "{text}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_long_overlay_is_warned_about_on_stderr_words_past_the_named_number() {
        let published = |count: usize| DynamicPublication {
            generation: "40cfe0ac-6f3b-4851-85ba-295e568663f6".to_string(),
            previous_generation: "a3d987b5-e2d0-6e47-b11e-2b29ebd47040".to_string(),
            history: (0..count).map(|index| format!("g{index}")).collect(),
            alias_rows: Vec::new(),
            replaced_in_place: Vec::new(),
            warn_after_generations: 50,
        };
        assert_eq!(overlay_warning(&published(50)), None);
        let warning = overlay_warning(&published(51)).unwrap();
        assert!(warning.starts_with("[WARN] "), "{warning}");
        assert!(warning.contains(": 51;"), "{warning}");
        assert!(
            warning.contains("штатное ibcmd infobase config apply --dynamic=disable"),
            "{warning}"
        );
        assert!(
            !warning.contains("обычное обновление (config apply"),
            "{warning}"
        );
    }

    #[test]
    fn only_auto_and_prompt_are_pointed_at_force() {
        assert!(points_at_force(DynamicMode::Auto));
        assert!(points_at_force(DynamicMode::Prompt));
        // `disable` said no to a dynamic update, and `force` is one
        assert!(!points_at_force(DynamicMode::Disable));
        assert!(!points_at_force(DynamicMode::Force));
        // and the request carries the word to the two seams: nothing but `force` is dynamic
        for (word, dynamic) in [
            ("auto", false),
            ("disable", false),
            ("prompt", false),
            ("force", true),
        ] {
            let request = request(&["config", "apply", &format!("--dynamic={word}")]);
            assert_eq!(request.dynamic == DynamicMode::Force, dynamic, "{word}");
        }
    }

    #[test]
    fn a_platform_without_the_dynamic_apply_is_named_and_is_exit_one_material() {
        for supported in [
            MssqlNativePlatformProfile::Platform8_3_27_2214,
            MssqlNativePlatformProfile::Platform8_5_1_1150,
        ] {
            supported.require_config_apply_dynamic_supported().unwrap();
        }
        let unsupported = MssqlNativePlatformProfile::Platform8_3_27_1989;
        let reason = unsupported
            .require_config_apply_dynamic_supported()
            .unwrap_err()
            .to_string();
        let error = anyhow::Error::new(DynamicUnsupported {
            platform: unsupported.id().to_string(),
            reason: reason.clone(),
        })
        .context("planning");
        match classify(&error, SessionTerminate::Disable) {
            Outcome::Refused(text) => {
                assert!(
                    text.starts_with("Параметр `--dynamic=force` команды `infobase config apply` не поддерживается для платформы platform-8.3.27.1989"),
                    "{text}"
                );
                assert!(text.contains("mssql.config.apply.dynamic"), "{text}");
                assert!(text.ends_with(&reason), "{text}");
                assert!(!text.contains("измерено только на 8.3"), "{text}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_stage_that_is_no_dynamic_update_is_the_native_applys_to_do() {
        // the words of the dynamic apply's refusal are the ones every stage the own apply leaves to
        // the platform has: `требуется штатный config apply: <reasons>`, exit 1
        let reasons = vec![
            "9521 rows".to_string(),
            "5eab8a1b-1111-4222-8333-444455556666: the object is a Catalog".to_string(),
        ];
        let error = anyhow::Error::new(NeedsNativeApply::apply(
            crate::mssql_config_apply::dynamic::refusal_reason(&reasons),
        ));
        match classify(&error, SessionTerminate::Disable) {
            Outcome::Refused(text) => assert_eq!(
                text,
                "требуется штатный config apply: 9521 rows; 5eab8a1b-1111-4222-8333-444455556666: the object is a Catalog"
            ),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_session_that_connects_during_the_transaction_is_the_same_refusal() {
        // the script's own assertion (THROW 57302), reached when one connects
        // between the look at the sessions and the transaction; the apply
        // lists the sessions itself, and the context around it changes nothing
        let error = refused_by_sessions(vec![session(90, "PC", "1CV8C")], true)
            .context("the config apply transaction failed");
        match classify(&error, SessionTerminate::Disable) {
            Outcome::Failed(text) => {
                assert!(text.contains("компьютер: PC, приложение: 1CV8C"), "{text}")
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_long_list_of_sessions_is_cut() {
        let sessions = (0..14)
            .map(|index| session(50 + index, "PC", "1CV8"))
            .collect::<Vec<_>>();
        let text = sessions_text(&sessions, "");
        assert_eq!(text.matches("компьютер: PC").count(), SESSIONS_SHOWN);
        assert!(text.contains("\nи ещё 4\n"), "{text}");
    }

    #[test]
    fn a_login_that_cannot_look_for_sessions_is_told_how_to_go_on() {
        let error = anyhow::Error::new(ExclusiveAccessUnprovable {
            reason: "exclusive access cannot be proven: the login lacks VIEW SERVER STATE, so other sessions are invisible".to_string(),
        });
        match classify(&error, SessionTerminate::Disable) {
            Outcome::Failed(text) => {
                assert!(
                    text.starts_with("exclusive access cannot be proven"),
                    "{text}"
                );
                assert!(text.ends_with("укажите --exclusivity=assumed"), "{text}")
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn what_the_own_apply_leaves_to_the_platform_is_exit_one_material() {
        let apply = anyhow::Error::new(NeedsNativeApply::apply(
            "Params holds 3 dynamic-update overlay row(s) (names with _dynupdate_): run the native `ibcmd infobase config apply`",
        ));
        match classify(&apply, SessionTerminate::Disable) {
            Outcome::Refused(text) => assert_eq!(
                text,
                "требуется штатный config apply: Params holds 3 dynamic-update overlay row(s) (names with _dynupdate_): run the native `ibcmd infobase config apply`"
            ),
            other => panic!("{other:?}"),
        }
        // an interrupted operation is left to `config repair`, and says so
        let repair = anyhow::Error::new(NeedsNativeApply::repair(
            "SchemaStorage is not settled: run the native `ibcmd infobase config repair` first",
        ));
        match classify(&repair, SessionTerminate::Disable) {
            Outcome::Refused(text) => assert!(
                text.starts_with("требуется штатный config repair: SchemaStorage is not settled"),
                "{text}"
            ),
            other => panic!("{other:?}"),
        }
        // through any context, still the type
        let wrapped =
            anyhow::Error::new(NeedsNativeApply::apply("a deleted list")).context("planning");
        assert!(matches!(
            classify(&wrapped, SessionTerminate::Disable),
            Outcome::Refused(_)
        ));
    }

    #[test]
    fn a_restructuring_without_a_backup_is_exit_one_and_names_both_options() {
        match classify(
            &anyhow::Error::new(BackupRequired),
            SessionTerminate::Disable,
        ) {
            Outcome::Refused(text) => {
                assert!(text.contains("--recovery-backup <путь>"), "{text}");
                assert!(text.contains("--i-have-a-backup"), "{text}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn errors_are_sorted_by_type_not_by_their_words() {
        // the words the apply used to be recognised by, on an error of no
        // type: a failure, whatever it says
        for text in [
            "exclusive access is not established: 2 other session(s) are connected to db",
            "exclusive access cannot be proven: the login lacks VIEW SERVER STATE",
            "Params holds 3 overlay row(s): run the native `ibcmd infobase config apply`",
            "an unfinished operation: run the native `ibcmd infobase config repair` first",
        ] {
            match classify(&anyhow!("{text}"), SessionTerminate::Disable) {
                Outcome::Failed(said) => assert_eq!(said, text),
                other => panic!("{text}: {other:?}"),
            }
        }
        // anything else is a failure, with its context
        let error = anyhow!("connection refused").context("the config apply transaction failed");
        match classify(&error, SessionTerminate::Disable) {
            Outcome::Failed(text) => {
                assert_eq!(
                    text,
                    "the config apply transaction failed: connection refused"
                )
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_options_follow_the_request() {
        let profile = MssqlNativePlatformProfile::Platform8_3_27_2214;
        let options = apply_options(&request(&["config", "apply", "--db-name=b"]), "b", profile);
        assert_eq!(options.database, "b");
        assert_eq!(options.exclusivity, Exclusivity::SqlSessions);
        assert!(!options.dry_run && !options.rehearse);
        let options = apply_options(
            &request(&["config", "apply", "--exclusivity=assumed"]),
            "b",
            profile,
        );
        assert_eq!(options.exclusivity, Exclusivity::Assumed);
        // the backup the operator names goes to the apply as it is
        assert_eq!(options.backup, BackupPolicy::None);
        let options = apply_options(
            &request(&["config", "apply", "--i-have-a-backup"]),
            "b",
            profile,
        );
        assert_eq!(options.backup, BackupPolicy::Acknowledged);
        let options = apply_options(
            &request(&["config", "apply", "--recovery-backup=F:/b/x.bak"]),
            "b",
            profile,
        );
        assert_eq!(
            options.backup,
            BackupPolicy::File(std::path::PathBuf::from("F:/b/x.bak"))
        );
    }

    #[test]
    fn the_gate_is_always_s1_and_the_backup_option_is_its_consent() {
        let profile = MssqlNativePlatformProfile::Platform8_3_27_2214;
        for line in [
            vec!["config", "apply"],
            vec!["config", "apply", "--i-have-a-backup"],
            vec!["config", "apply", "--recovery-backup=x.bak"],
        ] {
            let options = apply_options(&request(&line), "b", profile);
            assert_eq!(
                options.allow_restructure,
                Some(AllowRestructure::S1),
                "{line:?}"
            );
        }
    }

    #[test]
    fn the_three_branches_of_a_stage_that_changes_a_descriptor_are_told_apart() {
        // 1. an S1 change and no backup option: BackupRequired, exit 1, both options named
        match classify(
            &anyhow::Error::new(BackupRequired),
            SessionTerminate::Disable,
        ) {
            Outcome::Refused(text) => {
                assert!(text.contains("--recovery-backup") && text.contains("--i-have-a-backup"))
            }
            other => panic!("{other:?}"),
        }
        // 2. a change outside S1, or an S1 operation not built: the gate's reasons, exit 1, and a
        // reason about the stage as a whole starts the sentence instead of an empty row
        let verdict = GateVerdict {
            restructuring_required: true,
            blockers: vec![
                GateBlocker {
                    row: String::new(),
                    reason: "S1: property-outside-s1: Catalog.X: Properties/CodeLength: 9 -> 12"
                        .to_string(),
                },
                GateBlocker {
                    row: "5eab8a1b".to_string(),
                    reason: "S1: Catalog.X: the S1 operation \"add-tabular-section\" is designed but not built in this version"
                        .to_string(),
                },
            ],
            ..GateVerdict::default()
        };
        let text = structural_text(&verdict);
        assert!(
            text.starts_with("требуется штатный config apply: S1: property-outside-s1: Catalog.X"),
            "{text}"
        );
        assert!(
            text.contains("; 5eab8a1b: S1: Catalog.X: the S1 operation"),
            "{text}"
        );
        let error = anyhow::Error::new(StructuralRefusal { verdict });
        assert!(matches!(
            classify(&error, SessionTerminate::Disable),
            Outcome::Refused(_)
        ));
        // 3. a stage that needs no restructuring never gets here: the apply's own errors are the
        // only ones classified, and a failure of the database stays a failure (exit -1)
        assert!(matches!(
            classify(&anyhow!("connection lost"), SessionTerminate::Disable),
            Outcome::Failed(_)
        ));
    }

    #[test]
    fn a_platform_names_a_storage_profile() {
        let profile = |text: &str| profile_of(crate::platform::parse(text).unwrap()).unwrap();
        assert_eq!(
            profile("8.3.27.2214"),
            MssqlNativePlatformProfile::Platform8_3_27_2214
        );
        assert_eq!(
            profile("8.3.27.1989"),
            MssqlNativePlatformProfile::Platform8_3_27_1989
        );
        // a release stands for the build this apply was measured on
        assert_eq!(
            profile("8.3.27"),
            MssqlNativePlatformProfile::Platform8_3_27_2214
        );
        assert_eq!(
            profile("8.5.1"),
            MssqlNativePlatformProfile::Platform8_5_1_1150
        );
        assert_eq!(
            profile("8.5.1.1150"),
            MssqlNativePlatformProfile::Platform8_5_1_1150
        );
        // Known XML builds are not evidence for another build's native write
        // protocol. Only a declared release alias may choose its measured build.
        let unmeasured = crate::platform::parse("8.5.1.1529").unwrap();
        assert!(unmeasured.is_exact_build());
        assert!(unmeasured.native_profile().is_none());
        assert!(
            profile_of(unmeasured)
                .unwrap_err()
                .to_string()
                .contains("8.5.1.1529")
        );
        assert!(crate::platform::parse("8.5.1.1151").is_err());
    }

    #[test]
    fn the_report_file_says_what_was_done() {
        // the shape of the file is the caller's contract
        let value = serde_json::to_value(ApplyReportFile {
            operation: "infobase config apply",
            ok: true,
            nothing_to_apply: false,
            apply: &sample_report(),
        })
        .unwrap();
        assert_eq!(value["operation"], "infobase config apply");
        assert_eq!(value["ok"], true);
        assert_eq!(value["nothing_to_apply"], false);
        assert_eq!(value["apply"]["database"], "db");
        // an exclusive apply publishes no generation: the key is not there
        assert_eq!(value["apply"]["mode"], "exclusive");
        assert!(value["apply"].get("published").is_none());
    }

    #[test]
    fn the_report_of_a_dynamic_apply_names_the_mode_the_alias_rows_and_the_history() {
        let mut report = sample_report();
        report.mode = ApplyMode::Dynamic;
        report.exclusivity = Exclusivity::NotRequired;
        report.published = Some(crate::mssql_config_apply::DynamicPublication {
            generation: "40cfe0ac-6f3b-4851-85ba-295e568663f6".to_string(),
            previous_generation: "a3d987b5-e2d0-6e47-b11e-2b29ebd47040".to_string(),
            history: vec![
                "a3d987b5-e2d0-6e47-b11e-2b29ebd47040".to_string(),
                "40cfe0ac-6f3b-4851-85ba-295e568663f6".to_string(),
            ],
            alias_rows: vec!["x_dynupdate_40cfe0ac-6f3b-4851-85ba-295e568663f6.0".to_string()],
            replaced_in_place: vec!["root".to_string(), "version".to_string()],
            warn_after_generations: 50,
        });
        let value = serde_json::to_value(ApplyReportFile {
            operation: "infobase config apply",
            ok: true,
            nothing_to_apply: false,
            apply: &report,
        })
        .unwrap();
        assert_eq!(value["apply"]["mode"], "dynamic");
        assert_eq!(value["apply"]["exclusivity"], "not_required");
        assert_eq!(
            value["apply"]["published"]["alias_rows"][0],
            "x_dynupdate_40cfe0ac-6f3b-4851-85ba-295e568663f6.0"
        );
        assert_eq!(
            value["apply"]["published"]["history"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            value["apply"]["published"]["replaced_in_place"][1],
            "version"
        );
    }

    pub(super) fn sample_report() -> ConfigApplyReport {
        ConfigApplyReport {
            schema_version: 1,
            mode: ApplyMode::Exclusive,
            database: "db".to_string(),
            platform_profile: "platform-8.3.27.2214".to_string(),
            storage_schema_sha256: String::new(),
            dry_run: false,
            rehearsal: false,
            executed: true,
            nothing_to_apply: false,
            exclusivity: Exclusivity::SqlSessions,
            active_generation: None,
            new_generation: Some("40cfe0ac-6f3b-4851-85ba-295e568663f6".to_string()),
            stage: None,
            dynamic: None,
            published: None,
            gate: None,
            new_objects: None,
            removals: None,
            synonym_records: 0,
            structure: None,
            backup: None,
            registrations: None,
            tables_touched: Vec::new(),
            not_written: Vec::new(),
            warnings: Vec::new(),
            script_sha256: None,
            script_path: None,
            recovery_dir: None,
            recovery_token: None,
            timings: Default::default(),
        }
    }
}
