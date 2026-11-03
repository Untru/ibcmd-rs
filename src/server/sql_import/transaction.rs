//! Guarded selected staging on one held-lock session.
//! This local implementation is NOT an admitted native writer profile. Selected
//! headers are inherited unchanged from captured effective part zero; actual
//! multipart/header/native apply behavior remains a disposable-SQL gate.
use super::{
    readset::{Check, Input, Key, Part, Readset, Table},
    selected::ModuleEdit,
    snapshot::{Profile, capture_held},
};
use crate::sql::{Dbms, SqlClient, SqlParam};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Unchanged,
    NotPublished,
    RolledBack,
    Committed,
    Unknown,
}
#[derive(Debug)]
pub struct Report {
    pub state: State,
    pub error: Option<anyhow::Error>,
    pub cleanup_uncertain: bool,
}
// Preserve the terminal transaction outcome through an anyhow/RPC boundary.
// The finite display never includes SQL bodies, target paths or credentials.
impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SQL import transaction outcome: {:?}", self.state)
    }
}
impl std::error::Error for Report {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.error.as_ref().map(|error| error.as_ref() as _)
    }
}
/// Explicit local policy. No default or accepted native-profile assertion.
/// `wire_bytes` caps parameter values using `SqlParam::wire_bytes`; SQL text
/// and protocol framing are excluded, so this is not a full TDS request cap.
pub struct WritePolicy {
    part_bytes: usize,
    wire_bytes: usize,
    existing_single_part: bool,
}
impl WritePolicy {
    pub fn new(part_bytes: usize, wire_bytes: usize) -> Result<Self> {
        ensure!(
            part_bytes > 0 && wire_bytes > 0,
            "invalid SQL write budgets"
        );
        Ok(Self {
            part_bytes,
            wire_bytes,
            existing_single_part: false,
        })
    }

    /// Narrow physical restriction for an existing single-part replacement.
    /// This is not schema/codec admission. Both effective selected input and
    /// replacement must remain one part; unselected multipart rows stay exact.
    pub fn existing_single_part(part_bytes: usize, wire_bytes: usize) -> Result<Self> {
        let mut policy = Self::new(part_bytes, wire_bytes)?;
        policy.existing_single_part = true;
        Ok(policy)
    }
}
struct Plan {
    expected: Readset,
    parts: Vec<Input>,
    names: Vec<String>,
}
/// Pure preview of the exact same physical restrictions used before staging.
/// This does not open a SQL session or grant publication authority.
pub(super) fn preflight_selected_parts(
    original: &Readset,
    edit: &ModuleEdit,
    policy: &WritePolicy,
    check: &Check,
) -> Result<usize> {
    check()?;
    ensure!(
        *edit.readset_digest() == original.digest()?,
        "SQL edit belongs to another snapshot"
    );
    if edit.replacements().is_empty() {
        check()?;
        return Ok(0);
    }
    Ok(physical_plan(original, edit, policy, check)?.parts.len())
}

fn physical_plan(
    original: &Readset,
    edit: &ModuleEdit,
    policy: &WritePolicy,
    check: &Check,
) -> Result<Plan> {
    check()?;
    ensure!(
        *edit.readset_digest() == original.digest()?,
        "SQL edit belongs to another snapshot"
    );
    let mut names = edit.replacements().keys().cloned().collect::<Vec<_>>();
    // The inventory is published after all selected body names, not by accident
    // of lexical ordering or by clearing the whole staging table.
    names.sort_by_key(|name| name == "versions");
    let mut parts = Vec::new();
    let limits = original.limits();
    let mut payload_total = 0u64;
    let mut metadata_total = 0u64;
    for name in &names {
        check()?;
        let header = original
            .parts(Table::ConfigSave)
            .chain(original.parts(Table::Config))
            .find(|(key, _)| key.file_name == *name && key.part_no == 0)
            .map(|(_, part)| part.header.clone())
            .context("selected SQL metadata provenance is missing")?;
        let bytes = &edit.replacements()[name];
        if policy.existing_single_part {
            let table = if original
                .parts(Table::ConfigSave)
                .any(|(key, _)| key.file_name == *name)
            {
                Table::ConfigSave
            } else {
                Table::Config
            };
            let mut effective = original
                .parts(table)
                .filter(|(key, _)| key.file_name == *name);
            let (key, part) = effective
                .next()
                .context("selected single-part provenance is missing")?;
            ensure!(
                key.part_no == 0
                    && effective.next().is_none()
                    && part.data_size == u64::try_from(part.binary.len())?,
                "selected SQL input is outside the single-part write policy"
            );
            ensure!(
                bytes.len() <= policy.part_bytes,
                "selected SQL replacement requires unadmitted partitioning"
            );
        }
        let total = u64::try_from(bytes.len())?;
        let count = bytes.len().max(1).div_ceil(policy.part_bytes);
        i64::try_from(total)?;
        ensure!(
            count <= limits.max_parts.saturating_sub(parts.len()),
            "SQL selected part count exceeded"
        );
        ensure!(
            bytes.len().min(policy.part_bytes) as u64 <= limits.max_part_bytes,
            "SQL selected part size exceeded"
        );
        payload_total = payload_total
            .checked_add(total)
            .context("SQL selected payload overflow")?;
        metadata_total = metadata_total
            .checked_add(
                u64::try_from(count)?
                    .checked_mul(u64::try_from(name.len() + header.len())?)
                    .context("SQL selected metadata overflow")?,
            )
            .context("SQL selected metadata overflow")?;
        ensure!(
            payload_total <= limits.max_payload_bytes
                && metadata_total <= limits.max_metadata_bytes,
            "SQL selected aggregate budget exceeded"
        );
        for number in 0..count {
            check()?;
            let begin = number
                .checked_mul(policy.part_bytes)
                .context("SQL part offset overflow")?
                .min(bytes.len());
            let end = begin.saturating_add(policy.part_bytes).min(bytes.len());
            let payload = &bytes[begin..end];
            let wire = [
                SqlParam::Text(name),
                SqlParam::I64(i64::try_from(total)?),
                SqlParam::Binary(payload),
                SqlParam::I64(i64::try_from(number)?),
            ]
            .iter()
            .try_fold(0usize, |sum, param| sum.checked_add(param.wire_bytes()))
            .context("SQL parameter size overflow")?;
            ensure!(
                wire <= policy.wire_bytes,
                "SQL staging parameter-value budget exceeded"
            );
            parts.push(Input {
                key: Key {
                    file_name: name.clone(),
                    part_no: u32::try_from(number)?,
                },
                part: Part {
                    data_size: total,
                    header: header.clone(),
                    binary: Arc::new(payload.to_vec()),
                },
            });
        }
    }
    let expected = original.with_selected_stage(parts.iter().map(|input| Input {
        key: input.key.clone(),
        part: input.part.clone(),
    }))?;
    Ok(Plan {
        expected,
        parts,
        names,
    })
}
fn exact(alias: &str) -> String {
    format!(
        "CONVERT(varbinary(max),CONVERT(nvarchar(128),{alias}.FileName))=CONVERT(varbinary(max),@P1)"
    )
}
fn begin(profile: &Profile) -> String {
    let admission = crate::mssql_live_continue::acquire_pending_live_for_dedicated_session(
        &profile.identity.database,
    );
    let db = profile.identity.database.replace(']', "]]");
    format!(
        "-- ibcmd-editor:begin\n{admission}USE [{db}]; SET NOCOUNT ON; SET XACT_ABORT ON; SET LOCK_TIMEOUT 5000; IF @@TRANCOUNT<>0 THROW 57630,'guarded stage requires an idle session',1; BEGIN TRANSACTION;"
    )
}
const TEMP: &str = "-- ibcmd-editor:temp\nSELECT TOP (0) FileName,Creation,Modified,Attributes,DataSize,BinaryData,PartNo INTO #ibcmd_editor_delta FROM dbo.ConfigSave;";
const COMMIT: &str = "-- ibcmd-editor:commit\nIF @@TRANCOUNT<>1 OR XACT_STATE()<>1 THROW 57634,'guarded commit transaction state changed',1; COMMIT TRANSACTION;";
fn populate_sql() -> String {
    let s = exact("s");
    let c = exact("c");
    format!(
        "-- ibcmd-editor:populate\nINSERT INTO #ibcmd_editor_delta (FileName,Creation,Modified,Attributes,DataSize,BinaryData,PartNo) SELECT @P1,s.Creation,s.Modified,s.Attributes,@P2,@P3,@P4 FROM dbo.ConfigSave s WHERE {s} AND s.PartNo=0 UNION ALL SELECT @P1,c.Creation,c.Modified,c.Attributes,@P2,@P3,@P4 FROM dbo.Config c WHERE {c} AND c.PartNo=0 AND NOT EXISTS(SELECT 1 FROM dbo.ConfigSave s WHERE {s} AND s.PartNo=0); IF @@ROWCOUNT<>1 THROW 57631,'selected metadata provenance is not unique',1;"
    )
}
fn delete_sql() -> String {
    format!(
        "-- ibcmd-editor:delete\nDELETE s FROM dbo.ConfigSave s WHERE {}; IF @@ROWCOUNT<>@P2 THROW 57632,'selected delete count changed',1;",
        exact("s")
    )
}
fn insert_sql() -> String {
    format!(
        "-- ibcmd-editor:insert\nINSERT INTO dbo.ConfigSave (FileName,Creation,Modified,Attributes,DataSize,BinaryData,PartNo) SELECT s.FileName,s.Creation,s.Modified,s.Attributes,s.DataSize,s.BinaryData,s.PartNo FROM #ibcmd_editor_delta s WHERE {}; IF @@ROWCOUNT<>@P2 THROW 57633,'selected insert count changed',1;",
        exact("s")
    )
}
#[derive(Default)]
struct Attempt {
    entered: bool,
    begin_sent: bool,
    commit_sent: bool,
    commit_ack: bool,
    rollback_ack: bool,
    cleanup_uncertain: bool,
    error: Option<anyhow::Error>,
}

/// Guarded operation used only after source, schema and one-shot plan admission.
/// Final guards are synchronous operator/source/cancellation checks, not consent
/// callbacks. Preparation/consent must finish BEFORE this transaction begins.
pub fn stage(
    client: &dyn SqlClient,
    original: &Readset,
    edit: &ModuleEdit,
    profile: &Profile,
    policy: &WritePolicy,
    check: Check,
    guard: &dyn Fn() -> Result<()>,
) -> Report {
    let prepared = (|| -> Result<Option<Plan>> {
        check()?;
        guard()?;
        ensure!(
            client.dbms() == Dbms::SqlServer && client.detached_reason().is_none(),
            "guarded SQL staging requires the connected SQL backend"
        );
        ensure!(
            original.identity() == &profile.identity && original.schema() == profile.schema,
            "SQL stage profile differs from its snapshot"
        );
        ensure!(
            !profile.identity.database.is_empty()
                && !profile.identity.database.contains('\0')
                && profile.identity.database.encode_utf16().count() <= 128,
            "invalid SQL stage database"
        );
        ensure!(
            *edit.readset_digest() == original.digest()?,
            "SQL edit belongs to another snapshot"
        );
        if edit.replacements().is_empty() {
            return Ok(None);
        }
        Ok(Some(physical_plan(original, edit, policy, &check)?))
    })();
    let plan = match prepared {
        Ok(Some(plan)) => plan,
        Ok(None) => {
            return Report {
                state: State::Unchanged,
                error: None,
                cleanup_uncertain: false,
            };
        }
        Err(error) => {
            return Report {
                state: State::NotPublished,
                error: Some(error),
                cleanup_uncertain: false,
            };
        }
    };
    let mut attempt = Attempt::default();
    let outer = client.with_dedicated_session(&mut |session| {
        if attempt.entered {
            attempt
                .error
                .get_or_insert_with(|| anyhow::anyhow!("dedicated callback was repeated"));
            anyhow::bail!("dedicated callback was repeated");
        }
        attempt.entered = true;
        let work = (|| -> Result<()> {
            check()?;
            guard()?;
            attempt.begin_sent = true;
            session.run_batch(&begin(profile))?;
            let before = capture_held(session, profile, original.limits(), check.clone())?;
            original.require_equal(&before.readset)?;
            check()?;
            session.run_batch(TEMP)?;
            let populate = populate_sql();
            for input in &plan.parts {
                check()?;
                // Four parameters per request (not an unbounded multi-row batch).
                session.execute(
                    &populate,
                    &[
                        SqlParam::Text(&input.key.file_name),
                        SqlParam::I64(i64::try_from(input.part.data_size)?),
                        SqlParam::Binary(input.part.binary.as_slice()),
                        SqlParam::I64(i64::from(input.key.part_no)),
                    ],
                )?;
            }
            check()?;
            guard()?; // Immediately before the first application DELETE.
            let delete = delete_sql();
            let insert = insert_sql();
            for name in &plan.names {
                check()?;
                let old_count = original
                    .parts(Table::ConfigSave)
                    .filter(|(key, _)| key.file_name == *name)
                    .count();
                session.execute(
                    &delete,
                    &[
                        SqlParam::Text(name),
                        SqlParam::I64(i64::try_from(old_count)?),
                    ],
                )?;
                check()?;
                let count = plan
                    .parts
                    .iter()
                    .filter(|input| input.key.file_name == *name)
                    .count();
                session.execute(
                    &insert,
                    &[SqlParam::Text(name), SqlParam::I64(i64::try_from(count)?)],
                )?;
            }
            let after = capture_held(session, profile, original.limits(), check.clone())?;
            ensure!(
                after.session_id == before.session_id,
                "SQL transaction changed sessions"
            );
            plan.expected.require_equal(&after.readset)?;
            check()?;
            guard()?;
            check()?; // Guard work may itself observe or trigger cancellation.
            attempt.commit_sent = true;
            session.run_batch(COMMIT)?;
            attempt.commit_ack = true;
            Ok(())
        })();
        if let Err(error) = work {
            attempt.error = Some(error);
            if attempt.begin_sent && !attempt.commit_sent {
                match session.rollback_only() {
                    Ok(()) => attempt.rollback_ack = true,
                    Err(_) => attempt.cleanup_uncertain = true,
                }
            }
            anyhow::bail!("guarded SQL staging failed");
        }
        Ok(())
    });
    if let Err(error) = outer {
        if attempt.error.is_none() {
            attempt.error = Some(error);
        }
    }
    if !attempt.entered && attempt.error.is_none() {
        attempt.error = Some(anyhow::anyhow!(
            "dedicated session did not invoke guarded work"
        ));
    }
    // Never check cancellation after acknowledged commit or reinterpret an
    // outer cleanup/cancellation error as a no-write outcome.
    let state = if attempt.commit_ack {
        State::Committed
    } else if attempt.commit_sent || attempt.cleanup_uncertain {
        State::Unknown
    } else if attempt.rollback_ack {
        State::RolledBack
    } else {
        State::NotPublished
    };
    Report {
        state,
        error: attempt.error,
        cleanup_uncertain: attempt.cleanup_uncertain,
    }
}

#[cfg(test)]
#[path = "transaction_tests.rs"]
mod tests;
