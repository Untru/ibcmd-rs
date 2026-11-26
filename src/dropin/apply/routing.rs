//! SQL-guard interactive fallback, not native standalone-server administration.
//! A prompt is never authority to terminate a session or retry an uncertain transaction.

use std::io::{BufRead, IsTerminal, Read, Write};

use anyhow::{Result, bail};

use super::{Outcome, classify_with, points_at_force, sessions_text};
use crate::dropin::parse::{ApplyRequest, DynamicMode, ExclusivityMode, SessionTerminate};
use crate::mssql_config_apply::{ConfigApplyReport, ExclusiveAccessRefused};

pub(super) trait Dispatch {
    fn exclusive(&mut self) -> Result<ConfigApplyReport>;
    fn dynamic(&mut self) -> Result<ConfigApplyReport>;
    /// Read-only judgment; dispatch must still perform its complete fresh admission and CAS.
    fn qualifies(&mut self) -> bool;
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Choice {
    Cancel,
    RetryExclusive,
    Dynamic,
}

pub(super) trait Interaction {
    fn available(&self) -> bool;
    fn choose(&mut self, listing: &str) -> Result<Option<Choice>>;
}

pub(super) struct TerminalInteraction;
impl Interaction for TerminalInteraction {
    fn available(&self) -> bool {
        std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
    }
    fn choose(&mut self, listing: &str) -> Result<Option<Choice>> {
        read_choice(
            &mut std::io::stdin().lock(),
            &mut std::io::stderr().lock(),
            listing,
        )
    }
}

fn read_choice(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    listing: &str,
) -> Result<Option<Choice>> {
    writeln!(
        writer,
        "{listing}\nSQL-guard: 1 Отмена; 2 Повторить монопольно; 3 Обновить динамически"
    )?;
    write!(writer, "Выберите 1, 2 или 3: ")?;
    writer.flush()?;
    let mut line = Vec::new();
    if reader.take(65).read_until(b'\n', &mut line)? == 0 {
        return Ok(None);
    }
    if line.len() > 64 {
        bail!("ответ на запрос SQL-guard превышает 64 байта; ничего не применено");
    }
    if line.last() != Some(&b'\n') {
        bail!("ответ на запрос SQL-guard не завершён переводом строки; ничего не применено");
    }
    match line.as_slice().trim_ascii() {
        b"1" => Ok(Some(Choice::Cancel)),
        b"2" => Ok(Some(Choice::RetryExclusive)),
        b"3" => Ok(Some(Choice::Dynamic)),
        _ => bail!(
            "некорректный ответ на запрос SQL-guard: ожидается 1, 2 или 3; ничего не применено"
        ),
    }
}

fn finish(result: Result<ConfigApplyReport>, terminate: SessionTerminate, hint: bool) -> Outcome {
    match result {
        Ok(report) if report.nothing_to_apply => Outcome::NothingToApply(Box::new(report)),
        Ok(report) => Outcome::Applied(Box::new(report)),
        Err(error) => classify_with(&error, terminate, hint),
    }
}

pub(super) fn apply(
    request: &ApplyRequest,
    database: &str,
    dispatch: &mut impl Dispatch,
    io: &mut impl Interaction,
) -> Outcome {
    if request.dynamic == DynamicMode::Force {
        return finish(dispatch.dynamic(), request.session_terminate, false);
    }
    let error = match dispatch.exclusive() {
        Ok(report) => return finish(Ok(report), request.session_terminate, false),
        Err(error) => error,
    };
    let refusal = error.downcast_ref::<ExclusiveAccessRefused>();
    let hint = points_at_force(request.dynamic) && refusal.is_some() && dispatch.qualifies();
    let prompt = request.dynamic == DynamicMode::Prompt
        && request.exclusivity == ExclusivityMode::Sql
        && request.session_terminate == SessionTerminate::Disable
        && hint
        && refusal
            .is_some_and(|r| !r.in_transaction && r.database == database && !r.sessions.is_empty())
        && io.available();
    if !prompt {
        return classify_with(&error, request.session_terminate, hint);
    }
    // The error above is the typed pre-write refusal. A transaction refusal, transport failure or
    // blind census can never get here. A second refusal is terminal: there is no retry loop.
    let listing = sessions_text(&refusal.unwrap().sessions, &error.to_string());
    match io.choose(&listing) {
        Ok(Some(Choice::Cancel)) => Outcome::Cancelled,
        Ok(Some(Choice::RetryExclusive)) => {
            finish(dispatch.exclusive(), request.session_terminate, false)
        }
        Ok(Some(Choice::Dynamic)) => finish(dispatch.dynamic(), request.session_terminate, false),
        Ok(None) => {
            Outcome::Refused("Ответ на запрос SQL-guard не получен; ничего не применено".into())
        }
        Err(error) => Outcome::Refused(format!(
            "Запрос SQL-guard завершился с ошибкой; ничего не применено: {error:#}"
        )),
    }
}

#[cfg(test)]
mod tests;
