//! The gate of the `live` activation mode, before anything is written (#409 F-9 and F-10 of #344).
//!
//! `live` commits the promotion, then interrupts every connection of the database (`SET SINGLE_USER WITH ROLLBACK IMMEDIATE`),
//! takes a log backup of the tail (`BACKUP LOG ... WITH NORECOVERY`) and recovers the database again, so that the 1C working
//! processes reconnect and read the new generation. What can go wrong is known before the first write, and this module asks for
//! it there:
//!
//! * **F-9** the database has no log backup chain (`BACKUP LOG` fails with 4214 *after* the commit), the directory of the tail-log
//!   file does not exist on the SQL Server host, or the SQL Server account cannot write to it (a small backup of `model` is written
//!   there and removed again);
//! * **F-10** sessions of the database have an open transaction or a running request: the interruption would roll their work back.
//!   That needs the operator's word (`--interrupt-sessions`).
//!
//! The same checks are rendered into the activation script ([`render_live_gate`], before its transaction, so that the script is
//! safe on its own -- also on the `--sqlcmd` route), and, for the sessions, once more inside the promotion transaction just before
//! its `COMMIT` ([`render_sessions_check_in_transaction`]): a refusal there rolls the promotion back. The built-in SQL client also
//! runs the gate before the stage ([`preflight_live`]), so that a refusal leaves `ConfigSave` as it was.
//!
//! Nothing here connects into the database: the queries run from `master`, because a connection there could take the
//! single-user slot of a live switch.

use std::fmt::Write as _;

use anyhow::{Result, bail};
use serde::Serialize;

use crate::mssql_main_activation::{MainActivationError, quote_string, validate_tail_log_output};
use crate::sql::{ScriptVariables, SqlBackend, SqlExec};

/// The database has no log backup chain: no full backup since the recovery model was set.
pub const ERR_NO_LOG_CHAIN: u32 = 57235;
/// The directory of the tail-log output does not exist on the SQL Server host.
pub const ERR_NO_TAIL_DIRECTORY: u32 = 57236;
// 57237 is not used: a probe that fails raises the SQL Server error itself (3201, with the operating system's reason).
/// Sessions of the database have an open transaction or a running request and the interruption was not accepted.
pub const ERR_ACTIVE_SESSIONS: u32 = 57238;
/// The same, found inside the promotion transaction: the promotion is rolled back.
pub const ERR_SESSIONS_APPEARED: u32 = 57239;

/// The extension of the probe file: nothing but this tool's probes has it, and the removal is by this extension.
const PROBE_EXTENSION: &str = "ibcmdrsprobe";

/// The directory of a tail-log path on the SQL Server host: everything before the last separator (`\` or `/`); the root of a
/// drive keeps its separator (`C:\`). A path with no separator is not one this gate can check.
pub fn tail_directory(tail: &str) -> Result<String, MainActivationError> {
    let cut = tail.rfind(['\\', '/']).ok_or_else(|| {
        MainActivationError::SafetyGate(
            "the tail-log output has no directory: give an absolute path on the SQL Server host"
                .to_owned(),
        )
    })?;
    let directory = tail[..cut].trim_end_matches(['\\', '/']);
    if directory.is_empty() || directory.ends_with(':') {
        // `C:\tail.trn`: the root `C:\`. `\tail.trn` (no drive) is a path on the current drive of the service.
        return Ok(tail[..=cut].to_owned());
    }
    Ok(directory.to_owned())
}

/// The condition of "some session of the database is doing work": an open transaction or a request in progress. The session that
/// asks (`@@SPID`) is never counted.
pub(crate) fn active_work_condition(database_literal: &str) -> String {
    format!(
        "s.is_user_process=1 AND s.session_id<>@@SPID AND s.database_id=DB_ID(N'{database_literal}') AND (s.open_transaction_count>0 OR EXISTS (SELECT 1 FROM sys.dm_exec_requests q WHERE q.session_id=s.session_id))"
    )
}

pub struct LiveGateSettings<'a> {
    pub database: &'a str,
    pub tail_log_output: &'a str,
    /// The operator accepts that sessions with open work are interrupted (`--interrupt-sessions`).
    pub interrupt_sessions: bool,
    /// Write and remove a probe backup in the directory of the tail-log output.
    pub probe: bool,
}

/// The gate as SQL: from `master`, a `THROW` for the first condition that does not hold. The checks, in order: the database exists
/// (57230), FULL or BULK_LOGGED recovery (57231), ONLINE (57232), the tail-log file is not there (57233), a log backup chain
/// exists (57235), the directory exists (57236), the account can write there (when `probe`: the error of the probe backup, 3201),
/// and no session has open work unless it was accepted (57238).
pub fn render_live_gate(settings: &LiveGateSettings<'_>) -> Result<String, MainActivationError> {
    let tail = validate_tail_log_output(settings.tail_log_output)?;
    let directory = tail_directory(&tail)?;
    let database = quote_string(settings.database);
    let tail_literal = quote_string(&tail);
    let directory_literal = quote_string(&directory);
    let mut sql = String::new();
    writeln!(sql, "USE [master];").unwrap();
    writeln!(
        sql,
        "IF DB_ID(N'{database}') IS NULL THROW 57230, 'live activation database does not exist', 1;"
    )
    .unwrap();
    writeln!(sql, "IF (SELECT recovery_model FROM sys.databases WHERE name=N'{database}') NOT IN (1,2) THROW 57231, 'live activation requires FULL or BULK_LOGGED recovery', 1;").unwrap();
    writeln!(sql, "IF (SELECT state FROM sys.databases WHERE name=N'{database}') <> 0 THROW 57232, 'live activation requires an ONLINE database', 1;").unwrap();
    writeln!(sql, "IF EXISTS (SELECT 1 FROM sys.dm_os_file_exists(N'{tail_literal}') WHERE file_exists=1 OR file_is_a_directory=1) THROW 57233, 'tail-log output already exists or is a directory', 1;").unwrap();
    writeln!(sql, "IF NOT EXISTS (SELECT 1 FROM sys.database_recovery_status WHERE database_id=DB_ID(N'{database}') AND last_log_backup_lsn IS NOT NULL) THROW {ERR_NO_LOG_CHAIN}, 'live activation needs a log backup chain: take a full backup of the database (BACKUP DATABASE ... TO DISK=...) after the recovery model was set; the tail-log backup would fail with error 4214 after the promotion was committed', 1;").unwrap();
    writeln!(sql, "IF NOT EXISTS (SELECT 1 FROM sys.dm_os_file_exists(N'{directory_literal}') WHERE file_is_a_directory=1) THROW {ERR_NO_TAIL_DIRECTORY}, 'the directory of the tail-log output does not exist on the SQL Server host', 1;").unwrap();
    if settings.probe {
        // A backup of `model` (a few hundred KB compressed) written next to the future tail file and removed again: the only way to
        // know that the SQL Server service account can write there. `xp_delete_file` removes by the probe's own extension.
        let separator = if directory.ends_with(['\\', '/']) {
            ""
        } else if directory.contains('/') && !directory.contains('\\') {
            "/"
        } else {
            "\\"
        };
        // No TRY/CATCH around it on purpose: a failed BACKUP raises two errors, and the second ("BACKUP DATABASE is terminating
        // abnormally", 3013) is all a CATCH block gets; the first (3201, with the operating system's reason) is what tells the
        // operator what is wrong. The caller adds the meaning (`probe_failure_hint`).
        writeln!(sql, "DECLARE @LiveProbeFile nvarchar(2048)=N'{directory_literal}{separator}ibcmdrs-live-probe-'+CONVERT(nvarchar(36),NEWID())+N'.{PROBE_EXTENSION}';").unwrap();
        writeln!(sql, "BACKUP DATABASE [model] TO DISK=@LiveProbeFile WITH COPY_ONLY, COMPRESSION, INIT, CHECKSUM;").unwrap();
        writeln!(sql, "EXEC master.sys.xp_delete_file 0, N'{directory_literal}{separator}', N'{PROBE_EXTENSION}', '20991231';").unwrap();
    }
    if !settings.interrupt_sessions {
        let condition = active_work_condition(&database);
        writeln!(sql, "DECLARE @LiveActiveSessions int=(SELECT COUNT(*) FROM sys.dm_exec_sessions s WHERE {condition});").unwrap();
        writeln!(sql, "IF @LiveActiveSessions > 0 BEGIN DECLARE @LiveActiveMessage nvarchar(2048)=CONCAT(N'live activation refused: ',@LiveActiveSessions,N' session(s) of the database have an open transaction or a running request; the live switch would roll their work back (ROLLBACK IMMEDIATE). Wait until they are idle, or accept the interruption with --interrupt-sessions'); THROW {ERR_ACTIVE_SESSIONS},@LiveActiveMessage,1; END;").unwrap();
    }
    Ok(sql)
}

/// The sessions check that runs inside the promotion transaction, just before its `COMMIT`: work that started after the gate ran
/// rolls the promotion back instead of being rolled back by the interruption. Empty when the interruption was accepted.
pub fn render_sessions_check_in_transaction(database: &str, interrupt_sessions: bool) -> String {
    if interrupt_sessions {
        return String::new();
    }
    let condition = active_work_condition(&quote_string(database));
    format!(
        "IF EXISTS (SELECT 1 FROM sys.dm_exec_sessions s WHERE {condition}) THROW {ERR_SESSIONS_APPEARED}, 'live activation refused: sessions of the database started work while the promotion was prepared; the promotion is rolled back and nothing was changed. Retry when they are idle, or accept the interruption with --interrupt-sessions', 1;\n"
    )
}

/// One session that has open work, as the gate saw it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveSessionInfo {
    pub session_id: i64,
    pub login: String,
    pub host: String,
    pub program: String,
    pub status: String,
    pub open_transactions: i64,
    /// `<status> <command>` of the running request, empty when there is none.
    pub request: String,
}

/// What the gate found, for the report of the command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveGateReport {
    pub recovery_model: String,
    /// A log backup chain exists (`last_log_backup_lsn` is set).
    pub log_chain: bool,
    pub tail_directory: String,
    /// A probe backup was written to the tail directory and removed again.
    pub write_probe: bool,
    /// User sessions of the database (not the gate's own), the 1C working processes' among them.
    pub connections: i64,
    pub connections_1c: i64,
    pub with_open_transaction: i64,
    pub with_running_request: i64,
    /// `--interrupt-sessions` was given.
    pub interrupt_sessions: bool,
    /// Up to ten of the sessions whose work the live switch would roll back.
    pub active_sessions: Vec<LiveSessionInfo>,
}

fn facts_query(database: &str) -> String {
    let database = quote_string(database);
    format!(
        "SELECT CASE d.recovery_model WHEN 1 THEN N'FULL' WHEN 2 THEN N'BULK_LOGGED' WHEN 3 THEN N'SIMPLE' ELSE N'?' END, \
         CASE WHEN r.last_log_backup_lsn IS NULL THEN 0 ELSE 1 END, \
         (SELECT COUNT(*) FROM sys.dm_exec_sessions s WHERE s.is_user_process=1 AND s.session_id<>@@SPID AND s.database_id=d.database_id), \
         (SELECT COUNT(*) FROM sys.dm_exec_sessions s WHERE s.is_user_process=1 AND s.session_id<>@@SPID AND s.database_id=d.database_id AND s.program_name=N'1CV83 Server'), \
         (SELECT COUNT(*) FROM sys.dm_exec_sessions s WHERE s.is_user_process=1 AND s.session_id<>@@SPID AND s.database_id=d.database_id AND s.open_transaction_count>0), \
         (SELECT COUNT(*) FROM sys.dm_exec_sessions s WHERE s.is_user_process=1 AND s.session_id<>@@SPID AND s.database_id=d.database_id AND EXISTS (SELECT 1 FROM sys.dm_exec_requests q WHERE q.session_id=s.session_id)) \
         FROM sys.databases d LEFT JOIN sys.database_recovery_status r ON r.database_id=d.database_id WHERE d.name=N'{database}'"
    )
}

fn active_sessions_query(database: &str) -> String {
    let condition = active_work_condition(&quote_string(database));
    format!(
        "SELECT TOP (10) s.session_id, ISNULL(s.login_name,N''), ISNULL(s.host_name,N''), ISNULL(s.program_name,N''), s.status, s.open_transaction_count, \
         ISNULL((SELECT TOP 1 q.status+N' '+q.command FROM sys.dm_exec_requests q WHERE q.session_id=s.session_id),N'') \
         FROM sys.dm_exec_sessions s WHERE {condition} ORDER BY s.open_transaction_count DESC, s.session_id"
    )
}

/// What a failed probe means for the operator: the tail-log backup would fail the same way after the promotion was committed.
fn probe_failure_hint(error_text: &str) -> Option<&'static str> {
    if error_text.contains("code: 3201")
        || error_text.contains(PROBE_EXTENSION)
        || error_text.contains("xp_delete_file")
    {
        Some(
            "the SQL Server service account cannot write a backup to the directory of the tail-log output, or cannot remove the probe file from it; the tail-log backup would fail the same way after the promotion was committed",
        )
    } else {
        None
    }
}

/// The refusal text for sessions with open work: who they are, and what to do.
fn describe_sessions(sessions: &[LiveSessionInfo], total: i64) -> String {
    let listed = sessions
        .iter()
        .take(5)
        .map(|s| {
            format!(
                "session {} of {:?} on {:?} ({:?}, {}, {} open transaction(s){})",
                s.session_id,
                s.login,
                s.host,
                s.program,
                s.status,
                s.open_transactions,
                if s.request.is_empty() {
                    String::new()
                } else {
                    format!(", request: {}", s.request)
                }
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    format!("{total} session(s) with open work: {listed}")
}

/// Asks the database, before the stage, what the live gate asks in the script (the built-in SQL client only: a `--sqlcmd`
/// connection cannot read results, and the gate at the head of the script is the last word there). `probe` writes and removes the
/// probe backup; a dry run passes `false`. Returns what it found, or the reason the live switch would be refused.
pub fn preflight_live(
    sql: &SqlExec,
    database: &str,
    tail_log_output: &str,
    interrupt_sessions: bool,
    probe: bool,
) -> Result<Option<LiveGateReport>> {
    let SqlBackend::Client(client) = sql.backend() else {
        return Ok(None);
    };
    let gate = render_live_gate(&LiveGateSettings {
        database,
        tail_log_output,
        interrupt_sessions,
        probe,
    })
    .map_err(anyhow::Error::new)?;
    let facts = client.query_rows(&facts_query(database), &[])?;
    let Some(facts) = facts.first() else {
        bail!("live activation refused before any write: the database {database} does not exist");
    };
    let active = client
        .query_rows(&active_sessions_query(database), &[])?
        .into_iter()
        .map(|row| {
            Ok(LiveSessionInfo {
                session_id: row.i64(0)?,
                login: row.text(1)?.to_owned(),
                host: row.text(2)?.to_owned(),
                program: row.text(3)?.to_owned(),
                status: row.text(4)?.to_owned(),
                open_transactions: row.i64(5)?,
                request: row.text(6)?.to_owned(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let report = LiveGateReport {
        recovery_model: facts.text(0)?.to_owned(),
        log_chain: facts.i64(1)? == 1,
        tail_directory: tail_directory(tail_log_output).map_err(anyhow::Error::new)?,
        write_probe: probe,
        connections: facts.i64(2)?,
        connections_1c: facts.i64(3)?,
        with_open_transaction: facts.i64(4)?,
        with_running_request: facts.i64(5)?,
        interrupt_sessions,
        active_sessions: active,
    };
    if let Err(error) = client.run_script(&gate, ScriptVariables::Refuse) {
        let mut text = format!("{error:#}");
        if !interrupt_sessions && !report.active_sessions.is_empty() {
            let total = report
                .with_open_transaction
                .max(report.with_running_request)
                .max(report.active_sessions.len() as i64);
            text = format!(
                "{text} [{}]",
                describe_sessions(&report.active_sessions, total)
            );
        }
        if let Some(hint) = probe_failure_hint(&text) {
            text = format!("{text} [{hint}]");
        }
        bail!("live activation refused before any write: {text}");
    }
    Ok(Some(report))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings<'a>(interrupt: bool, probe: bool) -> LiveGateSettings<'a> {
        LiveGateSettings {
            database: "lab]db",
            tail_log_output: r"F:\lab\bak\tail'x.trn",
            interrupt_sessions: interrupt,
            probe,
        }
    }

    #[test]
    fn the_tail_directory_is_what_precedes_the_last_separator() {
        assert_eq!(
            tail_directory(r"F:\lab\bak\tail.trn").unwrap(),
            r"F:\lab\bak"
        );
        assert_eq!(
            tail_directory("/var/opt/mssql/tail.trn").unwrap(),
            "/var/opt/mssql"
        );
        assert_eq!(
            tail_directory(r"\\host\share\tail.trn").unwrap(),
            r"\\host\share"
        );
        assert_eq!(tail_directory(r"C:\tail.trn").unwrap(), r"C:\");
        assert!(
            tail_directory("tail.trn").is_err(),
            "no separator: no directory to check"
        );
    }

    #[test]
    fn the_gate_checks_in_the_order_the_failures_cost() {
        let sql = render_live_gate(&settings(false, true)).unwrap();
        let position = |needle: &str| {
            sql.find(needle)
                .unwrap_or_else(|| panic!("{needle} is not in the gate:\n{sql}"))
        };
        let order = [
            "USE [master];",
            "THROW 57230",
            "THROW 57231",
            "THROW 57232",
            "THROW 57233",
            "THROW 57235",
            "THROW 57236",
            "BACKUP DATABASE [model]",
            "xp_delete_file",
            "THROW 57238",
        ];
        let positions = order.map(position);
        assert!(positions.windows(2).all(|w| w[0] < w[1]), "{positions:?}");
        // the log chain is read from the recovery status, the directory from the host's file view
        assert!(sql.contains("last_log_backup_lsn IS NOT NULL"));
        assert!(sql.contains(r"sys.dm_os_file_exists(N'F:\lab\bak') WHERE file_is_a_directory=1"));
        // quoting: the database and the path are literals
        assert!(sql.contains("N'lab]db'"));
        assert!(sql.contains(r"N'F:\lab\bak\tail''x.trn'"));
    }

    #[test]
    fn the_probe_is_written_next_to_the_tail_file_and_removed_by_its_own_extension() {
        let sql = render_live_gate(&settings(false, true)).unwrap();
        assert!(
            sql.contains(
                r"N'F:\lab\bak\ibcmdrs-live-probe-'+CONVERT(nvarchar(36),NEWID())+N'.ibcmdrsprobe'"
            ),
            "{sql}"
        );
        assert!(
            sql.contains(
                r"EXEC master.sys.xp_delete_file 0, N'F:\lab\bak\', N'ibcmdrsprobe', '20991231';"
            ),
            "{sql}"
        );
        assert!(
            sql.contains("BACKUP DATABASE [model] TO DISK=@LiveProbeFile WITH COPY_ONLY"),
            "{sql}"
        );
        // no probe (a dry run): nothing is written
        let dry = render_live_gate(&settings(false, false)).unwrap();
        assert!(!dry.contains("BACKUP DATABASE [model]") && !dry.contains("xp_delete_file"));
        // a root directory and a UNC share keep their form
        let root = render_live_gate(&LiveGateSettings {
            tail_log_output: r"C:\tail.trn",
            ..settings(true, true)
        })
        .unwrap();
        assert!(root.contains(r"N'C:\ibcmdrs-live-probe-'"), "{root}");
    }

    #[test]
    fn the_sessions_check_is_off_only_when_the_interruption_is_accepted() {
        let refusing = render_live_gate(&settings(false, false)).unwrap();
        assert!(refusing.contains("s.open_transaction_count>0"));
        assert!(refusing.contains("sys.dm_exec_requests"));
        assert!(
            refusing.contains("s.session_id<>@@SPID"),
            "the gate's own session is never counted"
        );
        assert!(refusing.contains("--interrupt-sessions"));
        let accepted = render_live_gate(&settings(true, false)).unwrap();
        assert!(
            !accepted.contains("57238") && !accepted.contains("dm_exec_sessions"),
            "{accepted}"
        );
        // inside the transaction: a refusal rolls the promotion back; nothing to check when accepted
        let inside = render_sessions_check_in_transaction("lab]db", false);
        assert!(
            inside.contains("THROW 57239") && inside.contains("promotion is rolled back"),
            "{inside}"
        );
        assert!(render_sessions_check_in_transaction("lab]db", true).is_empty());
    }

    #[test]
    fn a_path_that_cannot_be_checked_is_refused_by_the_gate_itself() {
        let error = render_live_gate(&LiveGateSettings {
            tail_log_output: "tail.trn",
            ..settings(false, true)
        })
        .unwrap_err();
        assert!(error.to_string().contains("no directory"), "{error}");
        assert!(
            render_live_gate(&LiveGateSettings {
                tail_log_output: "",
                ..settings(false, true)
            })
            .is_err()
        );
    }

    /// A database that answers with canned rows: the facts, the sessions, and the outcome of the gate script.
    mod canned {
        use super::super::*;
        use crate::sql::{Dbms, SqlClient, SqlParam, SqlRow, SqlValue};

        pub struct Database {
            pub facts: Vec<SqlValue>,
            pub active: Vec<Vec<SqlValue>>,
            pub gate_error: Option<&'static str>,
        }

        impl SqlClient for Database {
            fn dbms(&self) -> Dbms {
                Dbms::SqlServer
            }
            fn max_connections(&self) -> usize {
                1
            }
            fn run_script(&self, _script: &str, _variables: ScriptVariables) -> Result<()> {
                match self.gate_error {
                    Some(text) => bail!("{text}"),
                    None => Ok(()),
                }
            }
            fn execute(&self, _statement: &str, _params: &[SqlParam<'_>]) -> Result<u64> {
                unreachable!()
            }
            fn read_rows(
                &self,
                query: &str,
                _params: &[SqlParam<'_>],
                each: &mut dyn FnMut(SqlRow) -> Result<()>,
            ) -> Result<()> {
                let rows: Vec<Vec<SqlValue>> = if query.contains("FROM sys.databases d") {
                    if self.facts.is_empty() {
                        vec![]
                    } else {
                        vec![self.facts.clone()]
                    }
                } else {
                    self.active.clone()
                };
                for values in rows {
                    each(SqlRow {
                        result_set: 0,
                        values,
                    })?;
                }
                Ok(())
            }
            fn query_json(&self, _query: &str) -> Result<Option<String>> {
                unreachable!()
            }
            fn write_rows(
                &self,
                _table: &str,
                _columns: &[&str],
                _rows: &[Vec<SqlParam<'_>>],
            ) -> Result<u64> {
                unreachable!()
            }
        }
    }

    fn facts(
        model: &str,
        chain: i64,
        total: i64,
        one_c: i64,
        tx: i64,
        running: i64,
    ) -> Vec<crate::sql::SqlValue> {
        use crate::sql::SqlValue::{Int, Text};
        vec![
            Text(model.to_owned()),
            Int(chain),
            Int(total),
            Int(one_c),
            Int(tx),
            Int(running),
        ]
    }

    fn session(id: i64, tx: i64, request: &str) -> Vec<crate::sql::SqlValue> {
        use crate::sql::SqlValue::{Int, Text};
        vec![
            Int(id),
            Text("sa".to_owned()),
            Text("WKS".to_owned()),
            Text("1CV83 Server".to_owned()),
            Text("sleeping".to_owned()),
            Int(tx),
            Text(request.to_owned()),
        ]
    }

    fn exec(database: canned::Database) -> SqlExec {
        let target = crate::sql::SqlTarget {
            server: String::new(),
            database: None,
            login: crate::sql::SqlLogin::Integrated,
            trust_server_certificate: false,
        };
        SqlExec::with_client(target, Box::new(database))
    }

    #[test]
    fn a_gate_that_passes_reports_what_it_found() {
        let sql = exec(canned::Database {
            facts: facts("FULL", 1, 4, 3, 0, 0),
            active: vec![],
            gate_error: None,
        });
        let report = preflight_live(&sql, "lab", r"F:\lab\bak\t.trn", false, true)
            .unwrap()
            .unwrap();
        assert_eq!(report.recovery_model, "FULL");
        assert!(report.log_chain && report.write_probe && !report.interrupt_sessions);
        assert_eq!((report.connections, report.connections_1c), (4, 3));
        assert_eq!(
            (report.with_open_transaction, report.with_running_request),
            (0, 0)
        );
        assert_eq!(report.tail_directory, r"F:\lab\bak");
        assert!(report.active_sessions.is_empty());
    }

    #[test]
    fn a_refusal_names_the_sessions_whose_work_would_be_rolled_back() {
        let sql = exec(canned::Database {
            facts: facts("FULL", 1, 3, 3, 1, 1),
            active: vec![session(57, 1, "running UPDATE")],
            gate_error: Some(
                "live activation refused: 1 session(s) of the database have an open transaction",
            ),
        });
        let error = preflight_live(&sql, "lab", r"F:\lab\bak\t.trn", false, true)
            .unwrap_err()
            .to_string();
        assert!(
            error.starts_with("live activation refused before any write"),
            "{error}"
        );
        assert!(error.contains("session 57 of \"sa\" on \"WKS\""), "{error}");
        assert!(
            error.contains("1 open transaction(s), request: running UPDATE"),
            "{error}"
        );
        // accepted: the sessions are reported, not a reason to refuse
        let accepted = exec(canned::Database {
            facts: facts("FULL", 1, 3, 3, 1, 1),
            active: vec![session(57, 1, "")],
            gate_error: None,
        });
        let report = preflight_live(&accepted, "lab", r"F:\lab\bak\t.trn", true, true)
            .unwrap()
            .unwrap();
        assert!(report.interrupt_sessions);
        assert_eq!(report.active_sessions.len(), 1);
    }

    #[test]
    fn a_probe_that_cannot_write_says_what_that_means() {
        let sql = exec(canned::Database {
            facts: facts("FULL", 1, 0, 0, 0, 0),
            active: vec![],
            gate_error: Some(
                r"Token error: 'cannot open backup device F:\x\probe. Operating system error 5 (Access denied)' (code: 3201, state: 1, class: 16)",
            ),
        });
        let error = preflight_live(&sql, "lab", r"F:\x\t.trn", false, true)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("code: 3201"),
            "the SQL Server error stays: {error}"
        );
        assert!(error.contains("Access denied"), "{error}");
        assert!(
            error.contains("would fail the same way after the promotion was committed"),
            "{error}"
        );
        // an error that has nothing to do with the probe gets no such hint
        let other = exec(canned::Database {
            facts: facts("FULL", 1, 0, 0, 0, 0),
            active: vec![],
            gate_error: Some(
                "live activation needs a log backup chain: take a full backup of the database (BACKUP DATABASE ... TO DISK=...) after the recovery model was set",
            ),
        });
        let error = preflight_live(&other, "lab", r"F:\x\t.trn", false, true)
            .unwrap_err()
            .to_string();
        assert!(!error.contains("service account"), "{error}");
    }

    #[test]
    fn a_broken_log_chain_is_refused_with_the_reason_the_database_gave() {
        let sql = exec(canned::Database {
            facts: facts("FULL", 0, 0, 0, 0, 0),
            active: vec![],
            gate_error: Some("live activation needs a log backup chain"),
        });
        let error = preflight_live(&sql, "lab", r"F:\lab\bak\t.trn", false, false)
            .unwrap_err()
            .to_string();
        assert!(error.contains("needs a log backup chain"), "{error}");
        // a database that is not there
        let none = exec(canned::Database {
            facts: vec![],
            active: vec![],
            gate_error: None,
        });
        assert!(
            preflight_live(&none, "lab", r"F:\lab\bak\t.trn", false, false)
                .unwrap_err()
                .to_string()
                .contains("does not exist")
        );
    }

    #[test]
    fn the_sqlcmd_route_cannot_ask_and_leaves_it_to_the_script() {
        let sql = SqlExec::detached("no server in a unit test");
        // a detached handle is the built-in client failing every request, not the sqlcmd route: the failure is an error, not a pass
        assert!(preflight_live(&sql, "lab", r"F:\lab\bak\t.trn", false, true).is_err());
    }
}
