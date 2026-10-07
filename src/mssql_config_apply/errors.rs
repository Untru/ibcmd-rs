//! The refusals of the own apply that a caller sorts by type.
//!
//! The drop-in `ibcmd infobase config apply` tells the user in the platform's
//! words what happened, and for that it has to know *why* the apply stopped.
//! Every error below is an ordinary [`anyhow::Error`] that can be
//! `downcast_ref`ed; the messages are the ones the apply always printed, so a
//! caller that still reads the text keeps working.
//!
//! - [`StructuralRefusal`](super::StructuralRefusal): the stage needs a
//!   restructuring (or something the gate does not admit);
//! - [`NeedsNativeApply`]: the stage or the database is left to the platform's
//!   own `config apply` or `config repair` for another reason (a removal, an
//!   unfinished operation, an overlay this apply does not fold, an 8.5 base);
//! - [`ExclusiveAccessRefused`]: other sessions are connected;
//! - [`ExclusiveAccessUnprovable`]: the login cannot see the sessions.
//!
//! Anything else (a stage made against another state, a query that failed, a
//! transaction that rolled back on a drifted fingerprint) stays a plain error.

use std::fmt;

use anyhow::Error;

use super::OtherSession;
use super::sqlgen::code;

/// The platform command a refused stage or database is left to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCommand {
    /// `ibcmd infobase config apply`.
    Apply,
    /// `ibcmd infobase config repair` (an interrupted operation).
    Repair,
}

/// The stage or the database needs the platform's own `config apply` (or
/// `config repair`); nothing was written.
#[derive(Debug, Clone)]
pub struct NeedsNativeApply {
    pub command: NativeCommand,
    /// Why, in the words the apply has always used.
    pub reason: String,
}

impl NeedsNativeApply {
    pub fn apply(reason: impl Into<String>) -> Self {
        Self {
            command: NativeCommand::Apply,
            reason: reason.into(),
        }
    }

    pub fn repair(reason: impl Into<String>) -> Self {
        Self {
            command: NativeCommand::Repair,
            reason: reason.into(),
        }
    }
}

impl fmt::Display for NeedsNativeApply {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.reason)
    }
}

impl std::error::Error for NeedsNativeApply {}

/// Other user sessions are connected to the database: the apply needs the
/// database to itself. Nothing was written.
#[derive(Debug, Clone)]
pub struct ExclusiveAccessRefused {
    pub database: String,
    /// The sessions found (all of them; a message lists the first few).
    pub sessions: Vec<OtherSession>,
    /// The check inside the transaction refused (a session connected after
    /// the plan was made) rather than the check before it.
    pub in_transaction: bool,
}

impl fmt::Display for ExclusiveAccessRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.in_transaction {
            return write!(
                formatter,
                "the config apply transaction failed; the database is unchanged: another session is connected to the database; the apply needs exclusive access"
            );
        }
        let listing = self
            .sessions
            .iter()
            .take(5)
            .map(|session| {
                format!(
                    "  session {} ({}, {}, {}, {})",
                    session.session_id,
                    session.login,
                    session.host,
                    session.program,
                    session.status
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        write!(
            formatter,
            "exclusive access is not established: {} other session(s) are connected to {}:\n{listing}\nclose them (or, when a working process merely keeps a pooled connection, stop it) and retry",
            self.sessions.len(),
            self.database
        )
    }
}

impl std::error::Error for ExclusiveAccessRefused {}

/// The login cannot see other sessions (no `VIEW SERVER STATE`), so exclusive
/// access cannot be proven. `--exclusivity assumed` is the way out for an
/// operator who has proved it another way.
#[derive(Debug, Clone)]
pub struct ExclusiveAccessUnprovable {
    pub reason: String,
}

impl fmt::Display for ExclusiveAccessUnprovable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.reason)
    }
}

impl std::error::Error for ExclusiveAccessUnprovable {}

/// The platform of the database has no dynamic apply (`mssql.config.apply.dynamic` is not declared
/// supported for its profile): `--dynamic=force` is named as not served, as an option is. Nothing was
/// written.
#[derive(Debug, Clone)]
pub struct DynamicUnsupported {
    /// The profile that was asked, `platform-8.5.1.1150`.
    pub platform: String,
    /// Why, from the profile registry.
    pub reason: String,
}

impl fmt::Display for DynamicUnsupported {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the dynamic apply is not available for {}: {}",
            self.platform, self.reason
        )
    }
}

impl std::error::Error for DynamicUnsupported {}

/// A restructuring is about to write and the operator has not said how to go
/// back: the old tables are dropped inside the transaction, and the recovery
/// artifact keeps the `Config` rows and the cache rows only. Nothing was
/// written. The words are Russian, as the platform's own refusals are.
#[derive(Debug, Clone, Default)]
pub struct BackupRequired;

impl fmt::Display for BackupRequired {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "Применение меняет структуру таблиц базы данных: старые таблицы удаляются внутри транзакции, а артефакт восстановления возвращает только строки Config и кэши. \
Нужна резервная копия SQL Server. Укажите `--recovery-backup <путь>` (рекомендуется: перед транзакцией применение само снимет копию \
`BACKUP DATABASE ... WITH COPY_ONLY, COMPRESSION` в этот файл и назовёт её в отчёте и в артефакте восстановления) или `--i-have-a-backup` \
(вы подтверждаете, что копия у вас есть; это будет записано в отчёте).",
        )
    }
}

impl std::error::Error for BackupRequired {}

/// The SQL Server error (code, message) behind a failed request, if the
/// failure was the server's answer to it.
pub(super) fn server_error_of(error: &Error) -> Option<(u32, String)> {
    error.chain().find_map(
        |cause| match cause.downcast_ref::<tiberius::error::Error>()? {
            tiberius::error::Error::Server(token) => {
                Some((token.code(), token.message().to_owned()))
            }
            _ => None,
        },
    )
}

/// The typed error for a `THROW` of the apply transaction, by its code; `None`
/// for the codes that are ordinary failures (a drifted fingerprint, a failed
/// postcondition). `sessions` lists the sessions again when the transaction
/// refused for them (the server's message carries none).
pub(super) fn from_transaction_code(
    code_number: u32,
    message: &str,
    database: &str,
    sessions: impl FnOnce() -> Vec<OtherSession>,
) -> Option<Error> {
    match code_number {
        code::OTHER_SESSIONS => Some(Error::new(ExclusiveAccessRefused {
            database: database.to_owned(),
            sessions: sessions(),
            in_transaction: true,
        })),
        code::NO_VIEW_SERVER_STATE => Some(Error::new(ExclusiveAccessUnprovable {
            reason: message.to_owned(),
        })),
        code::UNFINISHED_OPERATION | code::UNFINISHED_SCHEMA => {
            Some(Error::new(NeedsNativeApply::repair(format!(
                "{message}: run the native `ibcmd infobase config repair` first"
            ))))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(id: i64) -> OtherSession {
        OtherSession {
            session_id: id,
            login: "sa".to_owned(),
            host: "HOST".to_owned(),
            program: "1CV8".to_owned(),
            status: "sleeping".to_owned(),
            last_request_end: String::new(),
        }
    }

    #[test]
    fn needs_native_apply_downcasts_and_keeps_its_words() {
        let error = Error::new(NeedsNativeApply::apply(
            "Params holds 3 dynamic-update overlay row(s): run the native `ibcmd infobase config apply`",
        ));
        let typed = error
            .downcast_ref::<NeedsNativeApply>()
            .expect("a typed error");
        assert_eq!(typed.command, NativeCommand::Apply);
        // the message is the reason, as before (also through a context)
        assert!(format!("{error:#}").contains("run the native `ibcmd infobase config apply`"));
        let wrapped = error.context("while planning");
        assert!(wrapped.downcast_ref::<NeedsNativeApply>().is_some());
        let repair = NeedsNativeApply::repair("an unfinished operation");
        assert_eq!(repair.command, NativeCommand::Repair);
    }

    #[test]
    fn exclusive_access_refused_lists_the_sessions_in_the_old_words() {
        let error = Error::new(ExclusiveAccessRefused {
            database: "lab".to_owned(),
            sessions: vec![session(51), session(52)],
            in_transaction: false,
        });
        let typed = error
            .downcast_ref::<ExclusiveAccessRefused>()
            .expect("a typed error");
        assert_eq!(typed.sessions.len(), 2);
        let text = error.to_string();
        assert!(
            text.starts_with(
                "exclusive access is not established: 2 other session(s) are connected to lab:"
            ),
            "{text}"
        );
        assert!(
            text.contains("  session 51 (sa, HOST, 1CV8, sleeping)"),
            "{text}"
        );
        assert!(text.ends_with("and retry"), "{text}");
        // the transaction's own refusal keeps the marker a text reader looks for
        let late = ExclusiveAccessRefused {
            database: "lab".to_owned(),
            sessions: vec![session(53)],
            in_transaction: true,
        };
        assert!(
            late.to_string()
                .contains("another session is connected to the database")
        );
    }

    #[test]
    fn exclusive_access_unprovable_keeps_its_reason() {
        let error = Error::new(ExclusiveAccessUnprovable {
            reason: "exclusive access cannot be proven: the login lacks VIEW SERVER STATE, so other sessions are invisible".to_owned(),
        });
        assert!(error.downcast_ref::<ExclusiveAccessUnprovable>().is_some());
        assert!(
            error
                .to_string()
                .contains("exclusive access cannot be proven")
        );
    }

    #[test]
    fn the_transaction_codes_map_to_the_types() {
        let sessions = || vec![session(7)];
        let refused = from_transaction_code(code::OTHER_SESSIONS, "x", "lab", sessions).unwrap();
        let typed = refused.downcast_ref::<ExclusiveAccessRefused>().unwrap();
        assert!(typed.in_transaction);
        assert_eq!(typed.sessions.len(), 1);
        let blind =
            from_transaction_code(code::NO_VIEW_SERVER_STATE, "cannot prove", "lab", Vec::new)
                .unwrap();
        assert!(blind.downcast_ref::<ExclusiveAccessUnprovable>().is_some());
        for number in [code::UNFINISHED_OPERATION, code::UNFINISHED_SCHEMA] {
            let repair = from_transaction_code(number, "unfinished", "lab", Vec::new).unwrap();
            let typed = repair.downcast_ref::<NeedsNativeApply>().unwrap();
            assert_eq!(typed.command, NativeCommand::Repair);
            assert!(
                typed
                    .reason
                    .contains("run the native `ibcmd infobase config repair`")
            );
        }
        // a drifted fingerprint is an ordinary failure
        for number in [
            code::STAGE_DRIFTED,
            code::ACTIVE_DRIFTED,
            code::POSTCONDITION,
        ] {
            assert!(from_transaction_code(number, "x", "lab", Vec::new).is_none());
        }
    }

    #[test]
    fn backup_required_is_typed_russian_and_names_both_options() {
        let error = Error::new(BackupRequired);
        assert!(error.downcast_ref::<BackupRequired>().is_some());
        let text = error.to_string();
        assert!(text.contains("--recovery-backup <путь>"), "{text}");
        assert!(text.contains("--i-have-a-backup"), "{text}");
        assert!(text.contains("рекомендуется"), "{text}");
        assert!(text.contains("резервная копия"), "{text}");
    }

    #[test]
    fn an_error_that_is_not_the_servers_has_no_server_code() {
        assert!(server_error_of(&anyhow::anyhow!("no server")).is_none());
    }
}
