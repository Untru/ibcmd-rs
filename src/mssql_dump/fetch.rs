use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;

use super::{
    BinaryConfigRow, ConfigChunkRow, ConfigRow, ConfigRowHeader, encode_hex_lower,
    qualified_storage_table, quote_string,
};
use crate::runtime_evidence_schema::{SanitizedRuntimeArgumentKind, SubprocessJournalSchema};

#[derive(Clone, Debug, Serialize)]
pub(super) struct SanitizedSubprocessCall {
    pub executable: String,
    pub arguments: Vec<String>,
    pub started_unix_ms: u128,
    pub ended_unix_ms: Option<u128>,
    pub status: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub exception: Option<String>,
}

struct SubprocessJournalState {
    password_source_marker: String,
    path: Option<PathBuf>,
    server: String,
    database: String,
    started_unix_ms: u128,
    calls: Vec<SanitizedSubprocessCall>,
}

#[derive(Serialize)]
struct SubprocessJournalSnapshot<'a> {
    protocol_version: u32,
    status: &'a str,
    server: &'a str,
    database: &'a str,
    started_unix_ms: u128,
    ended_unix_ms: Option<u128>,
    exception: Option<&'a str>,
    calls: &'a [SanitizedSubprocessCall],
}

thread_local! {
    static SUBPROCESS_JOURNAL: RefCell<Option<SubprocessJournalState>> = const { RefCell::new(None) };
    #[cfg(test)]
    static FAIL_NEXT_JOURNAL_PERSIST: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(super) struct SubprocessJournalGuard {
    finished: bool,
}

impl SubprocessJournalGuard {
    pub(super) fn finish_passed(mut self) -> Result<Vec<SanitizedSubprocessCall>> {
        let calls = SUBPROCESS_JOURNAL.with(|journal| -> Result<Vec<SanitizedSubprocessCall>> {
            let mut state = journal.borrow_mut();
            let Some(current) = state.as_ref() else {
                return Ok(Vec::new());
            };
            persist_subprocess_journal(current, "passed", Some(unix_time_ms()), None)?;
            Ok(state.take().expect("journal state checked above").calls)
        })?;
        self.finished = true;
        Ok(calls)
    }

    pub(super) fn finish_failed(mut self, _error: &anyhow::Error) -> Result<()> {
        SUBPROCESS_JOURNAL.with(|journal| -> Result<()> {
            let mut state = journal.borrow_mut();
            if let Some(current) = state.as_ref() {
                persist_subprocess_journal(
                    current,
                    "failed",
                    Some(unix_time_ms()),
                    Some("MSSQL dump failed; nested subprocess details are recorded per call"),
                )?;
                let _ = state.take();
            }
            Ok(())
        })?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for SubprocessJournalGuard {
    fn drop(&mut self) {
        if !self.finished {
            SUBPROCESS_JOURNAL.with(|journal| {
                if let Some(state) = journal.borrow_mut().take() {
                    let _ = persist_subprocess_journal(
                        &state,
                        "failed",
                        Some(unix_time_ms()),
                        Some("subprocess journal guard dropped before explicit completion"),
                    );
                }
            });
        }
    }
}

pub(super) fn begin_subprocess_journal(
    password_source_marker: &str,
    server: &str,
    database: &str,
    path: Option<&Path>,
) -> Result<SubprocessJournalGuard> {
    SUBPROCESS_JOURNAL.with(|journal| {
        let state = SubprocessJournalState {
            password_source_marker: password_source_marker.to_owned(),
            path: path.map(Path::to_path_buf),
            server: server.to_owned(),
            database: database.to_owned(),
            started_unix_ms: unix_time_ms(),
            calls: Vec::new(),
        };
        persist_subprocess_journal(&state, "running", None, None)?;
        *journal.borrow_mut() = Some(state);
        Ok::<_, anyhow::Error>(())
    })?;
    Ok(SubprocessJournalGuard { finished: false })
}

fn password_source_marker() -> String {
    SUBPROCESS_JOURNAL.with(|journal| {
        journal
            .borrow()
            .as_ref()
            .map(|state| state.password_source_marker.clone())
            .unwrap_or_else(|| {
                SubprocessJournalSchema::redacted_password_source_marker().to_owned()
            })
    })
}

fn start_subprocess_call(call: SanitizedSubprocessCall) -> Result<usize> {
    SUBPROCESS_JOURNAL.with(|journal| {
        if let Some(state) = journal.borrow_mut().as_mut() {
            state.calls.push(call);
            let index = state.calls.len() - 1;
            persist_subprocess_journal(state, "running", None, None)?;
            Ok(index)
        } else {
            Ok(usize::MAX)
        }
    })
}

fn complete_subprocess_call(
    index: usize,
    status: &str,
    exit_code: Option<i32>,
    exception: Option<String>,
) -> Result<()> {
    SUBPROCESS_JOURNAL.with(|journal| {
        if let Some(state) = journal.borrow_mut().as_mut() {
            let call = state
                .calls
                .get_mut(index)
                .ok_or_else(|| anyhow!("subprocess journal call index is invalid"))?;
            call.ended_unix_ms = Some(unix_time_ms());
            call.status = status.to_owned();
            call.exit_code = exit_code;
            call.exception = exception;
            persist_subprocess_journal(state, "running", None, None)?;
        }
        Ok(())
    })
}

pub(super) fn current_subprocess_calls() -> Vec<SanitizedSubprocessCall> {
    SUBPROCESS_JOURNAL.with(|journal| {
        journal
            .borrow()
            .as_ref()
            .map(|state| state.calls.clone())
            .unwrap_or_default()
    })
}

fn persist_subprocess_journal(
    state: &SubprocessJournalState,
    status: &str,
    ended_unix_ms: Option<u128>,
    exception: Option<&str>,
) -> Result<()> {
    #[cfg(test)]
    FAIL_NEXT_JOURNAL_PERSIST.with(|fail_next| {
        if fail_next.replace(false) {
            bail!("injected subprocess journal persistence failure");
        }
        Ok::<_, anyhow::Error>(())
    })?;
    let Some(path) = state.path.as_deref() else {
        return Ok(());
    };
    write_json_atomic(
        path,
        &SubprocessJournalSnapshot {
            protocol_version: 1,
            status,
            server: &state.server,
            database: &state.database,
            started_unix_ms: state.started_unix_ms,
            ended_unix_ms,
            exception,
            calls: &state.calls,
        },
    )
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = SubprocessJournalSchema::journal_parent(path)
        .ok_or_else(|| anyhow!(SubprocessJournalSchema::missing_parent_error(path)))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(SubprocessJournalSchema::temporary_file_name(
        path,
        uuid::Uuid::new_v4().simple(),
    ));
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    if let Err(error) = replace_file_atomic(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_file_atomic(source: &Path, destination: &Path) -> Result<()> {
    fs::rename(source, destination)?;
    let parent = SubprocessJournalSchema::journal_parent(destination)
        .ok_or_else(|| anyhow!(SubprocessJournalSchema::missing_parent_error(destination)))?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(windows)]
fn replace_file_atomic(source: &Path, destination: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        return Err(std::io::Error::last_os_error()).context("MoveFileExW failed");
    }
    Ok(())
}

fn unix_time_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn query_marker(query: &str) -> String {
    SubprocessJournalSchema::query_digest_marker(query)
}

fn redact_password_value(arguments: &mut [String], password: Option<&str>) {
    let Some(password) = password.filter(|value| !value.is_empty()) else {
        return;
    };
    let marker = password_source_marker();
    for argument in arguments {
        if SubprocessJournalSchema::argument_kind(argument)
            == SanitizedRuntimeArgumentKind::Ordinary
        {
            *argument = argument.replace(password, &marker);
        }
    }
}
pub(super) fn fetch_rows(
    sqlcmd: &Path,
    server: &str,
    user: Option<&str>,
    password: Option<&str>,
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
) -> Result<Vec<ConfigRow>> {
    let sql = build_fetch_rows_sql(database, table, selected_file_names);
    let stdout = run_sql_capture_tsv(sqlcmd, server, user, password, &sql)?;
    let chunks = parse_config_chunk_rows(&stdout)
        .with_context(|| format!("failed to parse {table} row chunks for {database}"))?;
    assemble_config_rows(chunks)
        .with_context(|| format!("failed to assemble {table} row chunks for {database}"))
}

#[allow(dead_code)]
pub(super) fn fetch_rows_direct_hex(
    sqlcmd: &Path,
    server: &str,
    user: Option<&str>,
    password: Option<&str>,
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
) -> Result<Vec<ConfigRow>> {
    let sql = build_fetch_rows_direct_hex_sql(database, table, selected_file_names);
    let stdout = run_sql_capture_tsv(sqlcmd, server, user, password, &sql)?;
    parse_config_direct_rows(&stdout)
        .with_context(|| format!("failed to parse direct {table} rows for {database}"))
}

pub(super) fn fetch_binary_rows_bcp(
    _sqlcmd: &Path,
    bcp: &Path,
    server: &str,
    user: Option<&str>,
    password: Option<&str>,
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
    use_range_filter: bool,
) -> Result<Vec<BinaryConfigRow>> {
    if !selected_file_names.is_empty() && !use_range_filter {
        let batches = split_selected_file_names_for_bcp_query(
            database,
            table,
            selected_file_names,
            BCP_INLINE_QUERY_MAX_CHARS,
        );
        if batches.len() > 1 {
            let mut rows = Vec::new();
            for batch in batches {
                let first = batch.first().map(String::as_str).unwrap_or("<empty>");
                let last = batch.last().map(String::as_str).unwrap_or("<empty>");
                let query = build_fetch_binary_rows_query(database, table, &batch, false);
                let mut batch_rows = fetch_binary_rows_bcp_query(
                    bcp, server, user, password, database, table, &query,
                )
                .with_context(|| {
                    format!("failed to fetch exact bcp batch for {table} rows {first}..{last}")
                })?;
                rows.append(&mut batch_rows);
            }
            return Ok(rows);
        }
    }

    let query =
        build_fetch_binary_rows_query(database, table, selected_file_names, use_range_filter);
    fetch_binary_rows_bcp_query(bcp, server, user, password, database, table, &query)
}

pub(super) fn fetch_binary_rows_bcp_query(
    bcp: &Path,
    server: &str,
    user: Option<&str>,
    password: Option<&str>,
    database: &str,
    table: &str,
    query: &str,
) -> Result<Vec<BinaryConfigRow>> {
    let output_path = std::env::temp_dir().join(format!(
        "ibcmd-rs-bcp-{}-{}.bcp",
        std::process::id(),
        uuid::Uuid::new_v4().hyphenated()
    ));
    let mut arguments = vec![
        query.to_owned(),
        "queryout".to_owned(),
        output_path.to_string_lossy().into_owned(),
        "-S".to_owned(),
        server.to_owned(),
        "-n".to_owned(),
        "-u".to_owned(),
        "-a".to_owned(),
        "65535".to_owned(),
    ];
    let mut sanitized_arguments = arguments.clone();
    sanitized_arguments[0] = query_marker(query);
    match user {
        Some(user) => {
            arguments.extend(["-U".to_owned(), user.to_owned()]);
            sanitized_arguments.extend(["-U".to_owned(), user.to_owned()]);
            if let Some(password) = password {
                arguments.extend(["-P".to_owned(), password.to_owned()]);
                sanitized_arguments.extend(["-P".to_owned(), password_source_marker()]);
            }
        }
        None => {
            arguments.push("-T".to_owned());
            sanitized_arguments.push("-T".to_owned());
        }
    }
    redact_password_value(&mut sanitized_arguments, password);

    let started_unix_ms = unix_time_ms();
    let journal_index = start_subprocess_call(SanitizedSubprocessCall {
        executable: bcp.to_string_lossy().into_owned(),
        arguments: sanitized_arguments,
        started_unix_ms,
        ended_unix_ms: None,
        status: "running".to_owned(),
        exit_code: None,
        timed_out: false,
        exception: None,
    })?;
    let output = Command::new(bcp).args(&arguments).output();
    let output = match output {
        Ok(output) => {
            complete_subprocess_call(
                journal_index,
                if output.status.success() {
                    "passed"
                } else {
                    "failed"
                },
                output.status.code(),
                (!output.status.success())
                    .then(|| format!("bcp exited with code {:?}", output.status.code())),
            )?;
            output
        }
        Err(error) => {
            complete_subprocess_call(
                journal_index,
                "failed",
                None,
                Some(format!("failed to spawn bcp: {error}")),
            )?;
            return Err(error).with_context(|| format!("failed to run {}", bcp.display()));
        }
    };
    if !output.status.success() {
        let _ = fs::remove_file(&output_path);
        bail!(
            "bcp failed with status {}: {}{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let bytes = fs::read(&output_path)
        .with_context(|| format!("failed to read {}", output_path.display()))?;
    let _ = fs::remove_file(&output_path);
    let parts = parse_bcp_native_config_rows(&bytes)
        .with_context(|| format!("failed to parse native bcp rows for {database}.{table}"))?;
    assemble_binary_config_rows(parts)
        .with_context(|| format!("failed to assemble native bcp rows for {database}.{table}"))
}

#[cfg_attr(not(feature = "platform-oracle"), allow(dead_code))]
pub(crate) fn bcp_executable_for_sqlcmd(sqlcmd: &Path) -> PathBuf {
    if let Some(parent) = sqlcmd.parent() {
        for name in ["bcp.exe", "bcp"] {
            let candidate = parent.join(name);
            if candidate.exists() {
                return candidate;
            }
        }
    }
    PathBuf::from("bcp")
}

#[allow(dead_code)]
pub(super) fn fetch_metadata_rows(
    sqlcmd: &Path,
    server: &str,
    user: Option<&str>,
    password: Option<&str>,
    database: &str,
    table: &str,
) -> Result<Vec<ConfigRow>> {
    let sql = build_fetch_metadata_rows_sql(database, table);
    let stdout = run_sql_capture_tsv(sqlcmd, server, user, password, &sql)?;
    let chunks = parse_config_chunk_rows(&stdout)
        .with_context(|| format!("failed to parse {table} metadata row chunks for {database}"))?;
    assemble_config_rows(chunks)
        .with_context(|| format!("failed to assemble {table} metadata rows for {database}"))
}

pub(super) fn fetch_metadata_rows_bcp(
    _sqlcmd: &Path,
    bcp: &Path,
    server: &str,
    user: Option<&str>,
    password: Option<&str>,
    database: &str,
    table: &str,
) -> Result<Vec<ConfigRow>> {
    let query = build_fetch_metadata_rows_bcp_query(database, table);
    let rows = fetch_binary_rows_bcp_query(bcp, server, user, password, database, table, &query)?;
    Ok(config_rows_from_binary(rows))
}

pub(super) fn fetch_metadata_owner_rows_bcp(
    _sqlcmd: &Path,
    bcp: &Path,
    server: &str,
    user: Option<&str>,
    password: Option<&str>,
    database: &str,
    table: &str,
    metadata_file_names: &BTreeSet<String>,
) -> Result<Vec<ConfigRow>> {
    if metadata_file_names.is_empty() {
        return Ok(Vec::new());
    }

    let batches = split_selected_file_names_for_owner_rows_query(
        database,
        table,
        metadata_file_names,
        BCP_INLINE_QUERY_MAX_CHARS,
    );
    let mut rows = Vec::new();
    for batch in batches {
        let first = batch.first().map(String::as_str).unwrap_or("<empty>");
        let last = batch.last().map(String::as_str).unwrap_or("<empty>");
        let query = build_fetch_metadata_owner_rows_bcp_query(database, table, &batch);
        let mut batch_rows =
            fetch_binary_rows_bcp_query(bcp, server, user, password, database, table, &query)
                .with_context(|| {
                    format!("failed to fetch metadata owner rows batch {first}..{last}")
                })?;
        rows.append(&mut batch_rows);
    }
    Ok(config_rows_from_binary(rows))
}

pub(super) fn fetch_config_rows_bcp(
    sqlcmd: &Path,
    bcp: &Path,
    server: &str,
    user: Option<&str>,
    password: Option<&str>,
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
) -> Result<Vec<ConfigRow>> {
    let rows = fetch_binary_rows_bcp(
        sqlcmd,
        bcp,
        server,
        user,
        password,
        database,
        table,
        selected_file_names,
        false,
    )?;
    Ok(config_rows_from_binary(rows))
}

pub(super) fn config_rows_from_binary(rows: Vec<BinaryConfigRow>) -> Vec<ConfigRow> {
    rows.into_iter().map(config_row_from_binary).collect()
}

pub(super) fn config_row_from_binary(row: BinaryConfigRow) -> ConfigRow {
    ConfigRow {
        file_name: row.file_name,
        part_no: row.part_no,
        data_size: row.data_size,
        binary_hex: encode_hex_lower(&row.binary),
    }
}

pub(super) fn build_fetch_binary_rows_query(
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
    use_range_filter: bool,
) -> String {
    let filter = if selected_file_names.is_empty() {
        String::new()
    } else if use_range_filter {
        let first = selected_file_names
            .first()
            .expect("non-empty selected_file_names has first value");
        let last = selected_file_names
            .last()
            .expect("non-empty selected_file_names has last value");
        format!(
            "WHERE FileName >= N'{}' AND FileName <= N'{}'\n",
            quote_string(first),
            quote_string(last)
        )
    } else {
        let values = selected_file_names
            .iter()
            .map(|value| format!("N'{}'", quote_string(value)))
            .collect::<Vec<_>>()
            .join(", ");
        format!("WHERE FileName IN ({values})\n")
    };

    format!(
        "SELECT FileName, PartNo, DataSize, BinaryData\n\
         FROM {qualified_table}\n\
         {filter}\
         ORDER BY FileName, PartNo",
        qualified_table = qualified_storage_table(database, table),
        filter = filter,
    )
}

fn split_selected_file_names_for_bcp_query(
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
    max_query_chars: usize,
) -> Vec<BTreeSet<String>> {
    if selected_file_names.is_empty() {
        return Vec::new();
    }

    let base_query_chars = build_fetch_binary_rows_query(database, table, &BTreeSet::new(), false)
        .chars()
        .count();
    let filter_wrapper_chars = "WHERE FileName IN ()\n".chars().count();

    let mut batches = Vec::new();
    let mut current = BTreeSet::new();
    let mut current_value_chars = 0usize;

    for file_name in selected_file_names {
        let escaped = quote_string(file_name);
        let value_chars = 3 + escaped.chars().count();
        let separator_chars = usize::from(!current.is_empty()) * 2;
        let next_value_chars = current_value_chars + separator_chars + value_chars;
        let next_query_chars = base_query_chars + filter_wrapper_chars + next_value_chars;

        if !current.is_empty() && next_query_chars > max_query_chars {
            batches.push(std::mem::take(&mut current));
            current_value_chars = 0;
        }

        if !current.is_empty() {
            current_value_chars += 2;
        }
        current.insert(file_name.clone());
        current_value_chars += value_chars;
    }

    if !current.is_empty() {
        batches.push(current);
    }

    batches
}

pub(super) fn build_fetch_metadata_rows_bcp_query(database: &str, table: &str) -> String {
    format!(
        "SELECT FileName, PartNo, DataSize, BinaryData\n\
         FROM {qualified_table}\n\
         WHERE CHARINDEX(N'.', FileName) = 0\n\
         ORDER BY FileName, PartNo",
        qualified_table = qualified_storage_table(database, table),
    )
}

pub(super) fn build_fetch_metadata_owner_rows_bcp_query(
    database: &str,
    table: &str,
    metadata_file_names: &BTreeSet<String>,
) -> String {
    let filter = metadata_file_names
        .iter()
        .map(|value| {
            let value = quote_string(value);
            format!("FileName = N'{value}' OR FileName LIKE N'{value}.%'")
        })
        .collect::<Vec<_>>()
        .join(" OR ");

    format!(
        "SELECT FileName, PartNo, DataSize, BinaryData\n\
         FROM {qualified_table}\n\
         WHERE {filter}\n\
         ORDER BY FileName, PartNo",
        qualified_table = qualified_storage_table(database, table),
        filter = filter,
    )
}

fn split_selected_file_names_for_owner_rows_query(
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
    max_query_chars: usize,
) -> Vec<BTreeSet<String>> {
    if selected_file_names.is_empty() {
        return Vec::new();
    }

    let mut batches = Vec::new();
    let mut current = BTreeSet::new();
    for file_name in selected_file_names {
        let mut candidate = current.clone();
        candidate.insert(file_name.clone());
        if !current.is_empty()
            && build_fetch_metadata_owner_rows_bcp_query(database, table, &candidate)
                .chars()
                .count()
                > max_query_chars
        {
            batches.push(current);
            current = BTreeSet::from([file_name.clone()]);
        } else {
            current = candidate;
        }
    }

    if !current.is_empty() {
        batches.push(current);
    }

    batches
}

#[allow(dead_code)]
pub(super) fn build_fetch_metadata_rows_sql(database: &str, table: &str) -> String {
    format!(
        "SET NOCOUNT ON;\n\
         DECLARE @chunk_size int = {chunk_size};\n\
         WITH SourceRows AS (\n\
             SELECT FileName, PartNo, DataSize, BinaryData\n\
             FROM {qualified_table}\n\
             WHERE CHARINDEX(N'.', FileName) = 0\n\
         )\n\
         SELECT rows.FileName AS file_name,\n\
                rows.PartNo AS part_no,\n\
                rows.DataSize AS data_size,\n\
                chunks.chunk_index,\n\
                CONVERT(varchar(max), SUBSTRING(rows.BinaryData, chunks.chunk_index * @chunk_size + 1, @chunk_size), 2) AS binary_hex\n\
         FROM SourceRows rows\n\
         CROSS APPLY (\n\
             SELECT chunk_count = CASE\n\
                 WHEN DATALENGTH(rows.BinaryData) = 0 THEN 1\n\
                 ELSE (DATALENGTH(rows.BinaryData) + @chunk_size - 1) / @chunk_size\n\
             END\n\
         ) counts\n\
         CROSS APPLY (\n\
             SELECT TOP (counts.chunk_count)\n\
                    ROW_NUMBER() OVER (ORDER BY (SELECT NULL)) - 1 AS chunk_index\n\
             FROM sys.all_objects a CROSS JOIN sys.all_objects b\n\
         ) chunks\n\
         ORDER BY rows.FileName, rows.PartNo, chunks.chunk_index\n\
         ;",
        chunk_size = SQLCMD_BINARY_CHUNK_SIZE,
        qualified_table = qualified_storage_table(database, table),
    )
}

pub(super) fn fetch_row_headers(
    sqlcmd: &Path,
    server: &str,
    user: Option<&str>,
    password: Option<&str>,
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
) -> Result<Vec<ConfigRowHeader>> {
    if !selected_file_names.is_empty() {
        let batches = split_selected_file_names_for_row_headers_query(
            database,
            table,
            selected_file_names,
            SQLCMD_INLINE_QUERY_MAX_CHARS,
        );
        if batches.len() > 1 {
            let mut rows = Vec::new();
            for batch in batches {
                let first = batch.first().map(String::as_str).unwrap_or("<empty>");
                let last = batch.last().map(String::as_str).unwrap_or("<empty>");
                let sql = build_fetch_row_headers_sql(database, table, &batch);
                let stdout = run_sql_capture_tsv(sqlcmd, server, user, password, &sql)
                    .with_context(|| {
                        format!("failed to fetch row header batch for {table} rows {first}..{last}")
                    })?;
                let mut batch_rows = parse_config_row_headers(&stdout).with_context(|| {
                    format!("failed to parse {table} row header batch for {database} rows {first}..{last}")
                })?;
                rows.append(&mut batch_rows);
            }
            return Ok(rows);
        }
    }

    let sql = build_fetch_row_headers_sql(database, table, selected_file_names);
    let stdout = run_sql_capture_tsv(sqlcmd, server, user, password, &sql)?;
    parse_config_row_headers(&stdout)
        .with_context(|| format!("failed to parse {table} row headers for {database}"))
}

pub(super) fn build_fetch_row_headers_sql(
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
) -> String {
    let filter = if selected_file_names.is_empty() {
        String::new()
    } else {
        let values = selected_file_names
            .iter()
            .map(|value| format!("N'{}'", quote_string(value)))
            .collect::<Vec<_>>()
            .join(", ");
        format!("WHERE FileName IN ({values})\n")
    };

    format!(
        "SET NOCOUNT ON;\n\
         SELECT FileName AS file_name,\n\
                PartNo AS part_no,\n\
                DataSize AS data_size\n\
         FROM {qualified_table}\n\
         {filter}\
         ORDER BY FileName, PartNo\n\
         ;",
        qualified_table = qualified_storage_table(database, table),
        filter = filter,
    )
}

fn split_selected_file_names_for_row_headers_query(
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
    max_query_chars: usize,
) -> Vec<BTreeSet<String>> {
    if selected_file_names.is_empty() {
        return Vec::new();
    }

    let mut batches = Vec::new();
    let mut current = BTreeSet::new();
    for file_name in selected_file_names {
        let mut candidate = current.clone();
        candidate.insert(file_name.clone());
        if !current.is_empty()
            && build_fetch_row_headers_sql(database, table, &candidate)
                .chars()
                .count()
                > max_query_chars
        {
            batches.push(current);
            current = BTreeSet::from([file_name.clone()]);
        } else {
            current = candidate;
        }
    }

    if !current.is_empty() {
        batches.push(current);
    }

    batches
}

pub(super) fn parse_config_row_headers(stdout: &str) -> Result<Vec<ConfigRowHeader>> {
    let mut rows = Vec::new();
    for (line_index, line) in stdout.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        if is_sqlcmd_header_or_separator(line) {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 3 {
            bail!(
                "unexpected sqlcmd row header line {}: expected 3 tab-separated fields, got {}",
                line_index + 1,
                fields.len()
            );
        }
        rows.push(ConfigRowHeader {
            file_name: fields[0].trim_end().to_string(),
            part_no: fields[1]
                .trim()
                .parse()
                .with_context(|| format!("invalid part_no on header line {}", line_index + 1))?,
            data_size: fields[2]
                .trim()
                .parse()
                .with_context(|| format!("invalid data_size on header line {}", line_index + 1))?,
        });
    }
    Ok(rows)
}

pub(super) fn build_fetch_rows_sql(
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
) -> String {
    let filter = if selected_file_names.is_empty() {
        String::new()
    } else {
        let values = selected_file_names
            .iter()
            .map(|value| format!("N'{}'", quote_string(value)))
            .collect::<Vec<_>>()
            .join(", ");
        format!("WHERE FileName IN ({values})\n")
    };

    format!(
        "SET NOCOUNT ON;\n\
         DECLARE @chunk_size int = {chunk_size};\n\
         WITH SourceRows AS (\n\
             SELECT FileName, PartNo, DataSize, BinaryData\n\
             FROM {qualified_table}\n\
             {filter}\
         )\n\
         SELECT rows.FileName AS file_name,\n\
                rows.PartNo AS part_no,\n\
                rows.DataSize AS data_size,\n\
                chunks.chunk_index,\n\
                CONVERT(varchar(max), SUBSTRING(rows.BinaryData, chunks.chunk_index * @chunk_size + 1, @chunk_size), 2) AS binary_hex\n\
         FROM SourceRows rows\n\
         CROSS APPLY (\n\
             SELECT chunk_count = CASE\n\
                 WHEN DATALENGTH(rows.BinaryData) = 0 THEN 1\n\
                 ELSE (DATALENGTH(rows.BinaryData) + @chunk_size - 1) / @chunk_size\n\
             END\n\
         ) counts\n\
         CROSS APPLY (\n\
             SELECT TOP (counts.chunk_count)\n\
                    ROW_NUMBER() OVER (ORDER BY (SELECT NULL)) - 1 AS chunk_index\n\
             FROM sys.all_objects a CROSS JOIN sys.all_objects b\n\
         ) chunks\n\
         ORDER BY rows.FileName, rows.PartNo, chunks.chunk_index\n\
         ;",
        chunk_size = SQLCMD_BINARY_CHUNK_SIZE,
        qualified_table = qualified_storage_table(database, table),
        filter = filter,
    )
}

pub(super) fn build_fetch_rows_direct_hex_sql(
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
) -> String {
    let filter = if selected_file_names.is_empty() {
        String::new()
    } else {
        let values = selected_file_names
            .iter()
            .map(|value| format!("N'{}'", quote_string(value)))
            .collect::<Vec<_>>()
            .join(", ");
        format!("WHERE FileName IN ({values})\n")
    };

    format!(
        "SET NOCOUNT ON;\n\
         SELECT FileName AS file_name,\n\
                PartNo AS part_no,\n\
                DataSize AS data_size,\n\
                CONVERT(varchar(max), BinaryData, 2) AS binary_hex\n\
         FROM {qualified_table}\n\
         {filter}\
         ORDER BY FileName, PartNo\n\
         ;",
        qualified_table = qualified_storage_table(database, table),
        filter = filter,
    )
}

pub(super) fn parse_config_direct_rows(stdout: &str) -> Result<Vec<ConfigRow>> {
    let mut rows = Vec::new();
    for (line_index, line) in stdout.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        if is_sqlcmd_header_or_separator(line) {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 4 {
            bail!(
                "unexpected sqlcmd direct row line {}: expected 4 tab-separated fields, got {}",
                line_index + 1,
                fields.len()
            );
        }
        rows.push(ConfigRow {
            file_name: fields[0].trim_end().to_string(),
            part_no: fields[1].trim().parse().with_context(|| {
                format!("invalid part_no on direct row line {}", line_index + 1)
            })?,
            data_size: fields[2].trim().parse().with_context(|| {
                format!("invalid data_size on direct row line {}", line_index + 1)
            })?,
            binary_hex: fields[3].trim().to_ascii_lowercase(),
        });
    }
    Ok(rows)
}

pub(super) fn parse_config_chunk_rows(stdout: &str) -> Result<Vec<ConfigChunkRow>> {
    let mut rows = Vec::new();
    for (line_index, line) in stdout.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        if is_sqlcmd_header_or_separator(line) {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 5 {
            bail!(
                "unexpected sqlcmd row chunk line {}: expected 5 tab-separated fields, got {}",
                line_index + 1,
                fields.len()
            );
        }
        rows.push(ConfigChunkRow {
            file_name: fields[0].trim_end().to_string(),
            part_no: fields[1]
                .trim()
                .parse()
                .with_context(|| format!("invalid part_no on chunk line {}", line_index + 1))?,
            data_size: fields[2]
                .trim()
                .parse()
                .with_context(|| format!("invalid data_size on chunk line {}", line_index + 1))?,
            chunk_index: fields[3]
                .trim()
                .parse()
                .with_context(|| format!("invalid chunk_index on chunk line {}", line_index + 1))?,
            binary_hex: fields[4].trim_end().to_string(),
        });
    }
    Ok(rows)
}

pub(super) fn is_sqlcmd_header_or_separator(line: &str) -> bool {
    if line
        .split('\t')
        .next()
        .is_some_and(|field| field.trim() == "file_name")
    {
        return true;
    }
    line.chars().all(|ch| ch == '-' || ch == '\t' || ch == ' ')
}

pub(super) fn assemble_config_rows(chunks: Vec<ConfigChunkRow>) -> Result<Vec<ConfigRow>> {
    let mut parts = BTreeMap::<(String, i32), ConfigRow>::new();
    let mut expected_chunk = BTreeMap::<(String, i32), i32>::new();

    for chunk in chunks {
        let key = (chunk.file_name.clone(), chunk.part_no);
        let expected = expected_chunk.entry(key.clone()).or_insert(0);
        if chunk.chunk_index != *expected {
            bail!(
                "Config row {} part {} chunk order gap: expected {}, got {}",
                chunk.file_name,
                chunk.part_no,
                expected,
                chunk.chunk_index
            );
        }
        *expected += 1;

        parts
            .entry(key)
            .and_modify(|row| {
                row.binary_hex.push_str(&chunk.binary_hex);
            })
            .or_insert_with(|| ConfigRow {
                file_name: chunk.file_name,
                part_no: chunk.part_no,
                data_size: chunk.data_size,
                binary_hex: chunk.binary_hex,
            });
    }

    let mut rows = BTreeMap::<String, ConfigRow>::new();
    let mut expected_part = BTreeMap::<String, i32>::new();
    for part in parts.into_values() {
        let expected = expected_part.entry(part.file_name.clone()).or_insert(0);
        if part.part_no != *expected {
            bail!(
                "Config row {} part order gap: expected {}, got {}",
                part.file_name,
                expected,
                part.part_no
            );
        }
        *expected += 1;

        rows.entry(part.file_name.clone())
            .and_modify(|row| {
                if row.data_size != part.data_size {
                    row.data_size = part.data_size;
                }
                row.binary_hex.push_str(&part.binary_hex);
            })
            .or_insert_with(|| ConfigRow {
                file_name: part.file_name,
                part_no: 0,
                data_size: part.data_size,
                binary_hex: part.binary_hex,
            });
    }

    for row in rows.values() {
        let binary_bytes = row.binary_hex.len() / 2;
        if binary_bytes != row.data_size as usize {
            bail!(
                "Config row {} DataSize {} does not match assembled BinaryData length {}",
                row.file_name,
                row.data_size,
                binary_bytes
            );
        }
    }

    Ok(rows.into_values().collect())
}

pub(super) fn parse_bcp_native_config_rows(bytes: &[u8]) -> Result<Vec<BinaryConfigRow>> {
    let mut rows = Vec::new();
    let mut offset = 0usize;
    while offset < bytes.len() {
        let name_len = read_u16_le(bytes, &mut offset)? as usize;
        let name_bytes = read_bytes(bytes, &mut offset, name_len)?;
        if name_bytes.len() % 2 != 0 {
            bail!(
                "invalid native bcp nvarchar byte length: {}",
                name_bytes.len()
            );
        }
        let units = name_bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        let file_name = String::from_utf16(&units).context("invalid UTF-16 FileName in bcp row")?;
        let part_no = read_i32_le(bytes, &mut offset)?;
        let data_size = read_i64_le(bytes, &mut offset)?;
        let binary_len = read_i64_le(bytes, &mut offset)?;
        if binary_len < 0 {
            bail!("unexpected NULL BinaryData in bcp row {file_name}");
        }
        let binary = read_bytes(bytes, &mut offset, binary_len as usize)?.to_vec();
        rows.push(BinaryConfigRow {
            file_name,
            part_no,
            data_size,
            binary,
        });
    }
    Ok(rows)
}

pub(super) fn assemble_binary_config_rows(
    parts: Vec<BinaryConfigRow>,
) -> Result<Vec<BinaryConfigRow>> {
    let mut rows = BTreeMap::<String, BinaryConfigRow>::new();
    let mut expected_part = BTreeMap::<String, i32>::new();
    for part in parts {
        let expected = expected_part.entry(part.file_name.clone()).or_insert(0);
        if part.part_no != *expected {
            bail!(
                "Config row {} part order gap: expected {}, got {}",
                part.file_name,
                expected,
                part.part_no
            );
        }
        *expected += 1;

        rows.entry(part.file_name.clone())
            .and_modify(|row| {
                if row.data_size != part.data_size {
                    row.data_size = part.data_size;
                }
                row.binary.extend_from_slice(&part.binary);
            })
            .or_insert_with(|| BinaryConfigRow {
                file_name: part.file_name,
                part_no: 0,
                data_size: part.data_size,
                binary: part.binary,
            });
    }

    for row in rows.values() {
        if row.binary.len() != row.data_size as usize {
            bail!(
                "Config row {} DataSize {} does not match assembled BinaryData length {}",
                row.file_name,
                row.data_size,
                row.binary.len()
            );
        }
    }

    Ok(rows.into_values().collect())
}

pub(super) fn read_bytes<'a>(bytes: &'a [u8], offset: &mut usize, len: usize) -> Result<&'a [u8]> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| anyhow!("native bcp offset overflow"))?;
    let slice = bytes
        .get(*offset..end)
        .ok_or_else(|| anyhow!("unexpected end of native bcp data"))?;
    *offset = end;
    Ok(slice)
}

pub(super) fn read_u16_le(bytes: &[u8], offset: &mut usize) -> Result<u16> {
    let slice = read_bytes(bytes, offset, 2)?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

pub(super) fn read_i32_le(bytes: &[u8], offset: &mut usize) -> Result<i32> {
    let slice = read_bytes(bytes, offset, 4)?;
    Ok(i32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

pub(super) fn read_i64_le(bytes: &[u8], offset: &mut usize) -> Result<i64> {
    let slice = read_bytes(bytes, offset, 8)?;
    Ok(i64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

pub(super) const SQLCMD_BINARY_CHUNK_SIZE: usize = 16 * 1024;
pub(super) const SQLCMD_DUMP_FILE_BATCH_SIZE: usize = 4096;
pub(super) const SQLCMD_DUMP_BATCH_MAX_DATA_BYTES: u64 = 256 * 1024 * 1024;
pub(super) const SQLCMD_INLINE_QUERY_MAX_CHARS: usize = 24 * 1024;
pub(super) const BCP_INLINE_QUERY_MAX_CHARS: usize = 24 * 1024;

pub(super) fn run_sql_capture_tsv(
    sqlcmd: &Path,
    server: &str,
    user: Option<&str>,
    password: Option<&str>,
    sql: &str,
) -> Result<String> {
    let mut arguments = vec!["-C".to_owned(), "-S".to_owned(), server.to_owned()];
    let mut sanitized_arguments = arguments.clone();
    if let Some(user) = user {
        arguments.extend(["-U".to_owned(), user.to_owned()]);
        sanitized_arguments.extend(["-U".to_owned(), user.to_owned()]);
        if let Some(password) = password {
            arguments.extend(["-P".to_owned(), password.to_owned()]);
            sanitized_arguments.extend(["-P".to_owned(), password_source_marker()]);
        }
    }
    let common_arguments = ["-s", "\t", "-w", "65535", "-y", "0", "-Y", "0"].map(ToOwned::to_owned);
    arguments.extend(common_arguments.clone());
    sanitized_arguments.extend(common_arguments);
    let sql_file = if sql.chars().count() > SQLCMD_INLINE_QUERY_MAX_CHARS {
        let path = std::env::temp_dir().join(format!(
            "ibcmd-rs-sqlcmd-{}-{}.sql",
            std::process::id(),
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::write(&path, sql).with_context(|| format!("failed to write {}", path.display()))?;
        arguments.extend(["-i".to_owned(), path.to_string_lossy().into_owned()]);
        sanitized_arguments.extend(["-i".to_owned(), path.to_string_lossy().into_owned()]);
        Some(path)
    } else {
        arguments.extend(["-Q".to_owned(), sql.to_owned()]);
        sanitized_arguments.extend(["-Q".to_owned(), query_marker(sql)]);
        None
    };
    redact_password_value(&mut sanitized_arguments, password);
    let started_unix_ms = unix_time_ms();
    let journal_index = start_subprocess_call(SanitizedSubprocessCall {
        executable: sqlcmd.to_string_lossy().into_owned(),
        arguments: sanitized_arguments,
        started_unix_ms,
        ended_unix_ms: None,
        status: "running".to_owned(),
        exit_code: None,
        timed_out: false,
        exception: None,
    })?;
    let output = Command::new(sqlcmd).args(&arguments).output();
    if let Some(path) = &sql_file {
        let _ = fs::remove_file(path);
    }
    let output = match output {
        Ok(output) => {
            complete_subprocess_call(
                journal_index,
                if output.status.success() {
                    "passed"
                } else {
                    "failed"
                },
                output.status.code(),
                (!output.status.success())
                    .then(|| format!("sqlcmd exited with code {:?}", output.status.code())),
            )?;
            output
        }
        Err(error) => {
            complete_subprocess_call(
                journal_index,
                "failed",
                None,
                Some(format!("failed to spawn sqlcmd: {error}")),
            )?;
            return Err(error).with_context(|| format!("failed to run {}", sqlcmd.display()));
        }
    };
    if !output.status.success() {
        bail!(
            "sqlcmd failed with exit code {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub(super) fn sql_password(
    user: Option<&str>,
    password: Option<&str>,
    password_env: &str,
) -> Option<String> {
    user?;
    password
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| std::env::var(password_env).ok())
}

#[cfg(test)]
pub(super) fn normalize_sqlcmd_json(value: &str) -> String {
    value.replace(['\r', '\n'], "")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;

    use super::{
        BCP_INLINE_QUERY_MAX_CHARS, FAIL_NEXT_JOURNAL_PERSIST, SanitizedSubprocessCall,
        begin_subprocess_journal, build_fetch_binary_rows_query,
        build_fetch_metadata_owner_rows_bcp_query, build_fetch_row_headers_sql,
        complete_subprocess_call, fetch_binary_rows_bcp_query, password_source_marker,
        query_marker, redact_password_value, run_sql_capture_tsv,
        split_selected_file_names_for_bcp_query, split_selected_file_names_for_owner_rows_query,
        split_selected_file_names_for_row_headers_query, start_subprocess_call,
    };

    #[test]
    fn subprocess_journal_uses_query_hashes_and_password_source_markers() {
        let raw_query = "SELECT top-secret-query";
        let raw_password = "top-secret-password";
        let guard = begin_subprocess_journal(
            "<password-source:environment>",
            "localhost",
            "test_db",
            None,
        )
        .unwrap();
        let mut arguments = vec![
            "-U".to_owned(),
            raw_password.to_owned(),
            "-P".to_owned(),
            password_source_marker(),
            "-Q".to_owned(),
            query_marker(raw_query),
        ];
        redact_password_value(&mut arguments, Some(raw_password));
        let index = start_subprocess_call(SanitizedSubprocessCall {
            executable: "sqlcmd".to_owned(),
            arguments,
            started_unix_ms: 1,
            ended_unix_ms: None,
            status: "running".to_owned(),
            exit_code: None,
            timed_out: false,
            exception: None,
        })
        .unwrap();
        complete_subprocess_call(index, "passed", Some(0), None).unwrap();
        let calls = guard.finish_passed().unwrap();
        let serialized = serde_json::to_string(&calls).unwrap();

        assert_eq!(calls.len(), 1);
        assert!(calls[0].arguments[5].starts_with("<query-sha256:"));
        assert!(!serialized.contains(raw_query));
        assert!(!serialized.contains(raw_password));
        assert!(serialized.contains("<password-source:environment>"));
    }

    #[test]
    fn durable_subprocess_journal_redacts_hostile_marker_like_arguments() {
        let raw_password = "top-secret-password";
        let journal_path = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-runtime-hostile-marker-{}.json",
            uuid::Uuid::new_v4().simple()
        ));
        let guard = begin_subprocess_journal(
            "<password-source:environment>",
            "localhost",
            "test_db",
            Some(&journal_path),
        )
        .unwrap();
        let exact_password_marker = password_source_marker();
        let exact_query_marker = query_marker("SELECT 1");
        let mut arguments = vec![
            exact_password_marker.clone(),
            exact_query_marker.clone(),
            format!("<password-source:environment>{raw_password}"),
            format!("<password-source:<query-sha256:{raw_password}"),
            format!("<query-sha256:{raw_password}"),
            format!("<query-sha256:{}>{raw_password}", "a".repeat(63)),
            format!("<query-sha256:{}>{raw_password}", "A".repeat(64)),
            format!("<query-sha256:{}>suffix-{raw_password}", "a".repeat(64)),
        ];
        redact_password_value(&mut arguments, Some(raw_password));

        assert_eq!(arguments[0], exact_password_marker);
        assert_eq!(arguments[1], exact_query_marker);
        assert!(
            arguments
                .iter()
                .skip(2)
                .all(|argument| !argument.contains(raw_password))
        );

        let index = start_subprocess_call(SanitizedSubprocessCall {
            executable: "sqlcmd".to_owned(),
            arguments,
            started_unix_ms: 1,
            ended_unix_ms: None,
            status: "running".to_owned(),
            exit_code: None,
            timed_out: false,
            exception: None,
        })
        .unwrap();
        complete_subprocess_call(index, "passed", Some(0), None).unwrap();
        guard.finish_passed().unwrap();

        let serialized = fs::read_to_string(&journal_path).unwrap();
        assert!(!serialized.contains(raw_password));
        assert!(serialized.contains(&exact_password_marker));
        assert!(serialized.contains(&exact_query_marker));
        let _ = fs::remove_file(journal_path);
    }

    #[test]
    fn durable_subprocess_journal_survives_actual_spawn_failure() {
        let journal_path = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-runtime-failure-{}.json",
            uuid::Uuid::new_v4().simple()
        ));
        let guard = begin_subprocess_journal(
            "<password-source:environment>",
            "localhost",
            "test_db",
            Some(&journal_path),
        )
        .unwrap();
        let error = run_sql_capture_tsv(
            std::path::Path::new("ibcmd-rs-sqlcmd-that-does-not-exist"),
            "localhost",
            Some("user"),
            Some("top-secret-password"),
            "SELECT top-secret-query",
        )
        .unwrap_err();
        guard.finish_failed(&error).unwrap();

        let journal: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&journal_path).unwrap()).unwrap();
        assert_eq!(journal["status"], "failed");
        assert_eq!(journal["server"], "localhost");
        assert_eq!(journal["database"], "test_db");
        assert_eq!(journal["calls"][0]["status"], "failed");
        assert!(journal["calls"][0]["ended_unix_ms"].as_u64().is_some());
        let serialized = journal.to_string();
        assert!(!serialized.contains("top-secret-password"));
        assert!(!serialized.contains("SELECT top-secret-query"));
        let _ = fs::remove_file(journal_path);
    }

    #[test]
    fn failed_terminal_persist_is_retried_by_guard_drop() {
        let journal_path = std::env::temp_dir().join(format!(
            "ibcmd-rs-mssql-runtime-retry-{}.json",
            uuid::Uuid::new_v4().simple()
        ));
        let guard = begin_subprocess_journal(
            "<password-source:none>",
            "localhost",
            "test_db",
            Some(&journal_path),
        )
        .unwrap();
        FAIL_NEXT_JOURNAL_PERSIST.with(|fail_next| fail_next.set(true));

        assert!(guard.finish_passed().is_err());

        let journal: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&journal_path).unwrap()).unwrap();
        assert_eq!(journal["status"], "failed");
        assert_eq!(
            journal["exception"],
            "subprocess journal guard dropped before explicit completion"
        );
        let _ = fs::remove_file(journal_path);
    }

    #[test]
    fn configured_bcp_executable_is_recorded_exactly() {
        let journal_path = std::env::temp_dir().join(format!(
            "ibcmd-rs-bcp-runtime-path-{}.json",
            uuid::Uuid::new_v4().simple()
        ));
        let configured_bcp = std::env::temp_dir().join(format!(
            "ibcmd-rs-configured-bcp-that-does-not-exist-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let guard = begin_subprocess_journal(
            "<password-source:none>",
            "localhost",
            "test_db",
            Some(&journal_path),
        )
        .unwrap();
        let error = fetch_binary_rows_bcp_query(
            &configured_bcp,
            "localhost",
            None,
            None,
            "test_db",
            "Config",
            "SELECT 1",
        )
        .unwrap_err();
        guard.finish_failed(&error).unwrap();

        let journal: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&journal_path).unwrap()).unwrap();
        assert_eq!(
            journal["calls"][0]["executable"],
            configured_bcp.to_string_lossy().as_ref()
        );
        assert_eq!(journal["calls"][0]["status"], "failed");
        let _ = fs::remove_file(journal_path);
    }

    #[test]
    fn split_selected_file_names_for_bcp_query_keeps_small_selection_in_one_batch() {
        let selected = BTreeSet::from([
            "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
            "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb".to_string(),
        ]);

        let batches = split_selected_file_names_for_bcp_query("TestDb", "Config", &selected, 1_024);

        assert_eq!(batches, vec![selected]);
    }

    #[test]
    fn split_selected_file_names_for_bcp_query_caps_each_exact_query_length() {
        let selected = (0..900)
            .map(|index| format!("aaaaaaaa-aaaa-4aaa-aaaa-{index:012x}"))
            .collect::<BTreeSet<_>>();

        let batches = split_selected_file_names_for_bcp_query(
            "TestDb",
            "Config",
            &selected,
            BCP_INLINE_QUERY_MAX_CHARS,
        );

        assert!(batches.len() > 1);
        let rebuilt = batches
            .iter()
            .flat_map(|batch| batch.iter().cloned())
            .collect::<BTreeSet<_>>();
        assert_eq!(rebuilt, selected);
        for batch in &batches {
            assert!(!batch.is_empty());
            let query = build_fetch_binary_rows_query("TestDb", "Config", batch, false);
            assert!(query.chars().count() <= BCP_INLINE_QUERY_MAX_CHARS);
        }
    }

    #[test]
    fn split_selected_file_names_for_owner_rows_query_caps_each_query_length() {
        let selected = (0..500)
            .map(|index| format!("bbbbbbbb-bbbb-4bbb-bbbb-{index:012x}"))
            .collect::<BTreeSet<_>>();

        let batches = split_selected_file_names_for_owner_rows_query(
            "TestDb",
            "Config",
            &selected,
            BCP_INLINE_QUERY_MAX_CHARS,
        );

        assert!(batches.len() > 1);
        let rebuilt = batches
            .iter()
            .flat_map(|batch| batch.iter().cloned())
            .collect::<BTreeSet<_>>();
        assert_eq!(rebuilt, selected);
        for batch in &batches {
            assert!(!batch.is_empty());
            let query = build_fetch_metadata_owner_rows_bcp_query("TestDb", "Config", batch);
            assert!(query.chars().count() <= BCP_INLINE_QUERY_MAX_CHARS);
        }
    }

    #[test]
    fn split_selected_file_names_for_row_headers_query_caps_each_query_length() {
        let selected = (0..900)
            .map(|index| format!("cccccccc-cccc-4ccc-cccc-{index:012x}"))
            .collect::<BTreeSet<_>>();

        let batches = split_selected_file_names_for_row_headers_query(
            "TestDb",
            "Config",
            &selected,
            BCP_INLINE_QUERY_MAX_CHARS,
        );

        assert!(batches.len() > 1);
        let rebuilt = batches
            .iter()
            .flat_map(|batch| batch.iter().cloned())
            .collect::<BTreeSet<_>>();
        assert_eq!(rebuilt, selected);
        for batch in &batches {
            assert!(!batch.is_empty());
            let query = build_fetch_row_headers_sql("TestDb", "Config", batch);
            assert!(query.chars().count() <= BCP_INLINE_QUERY_MAX_CHARS);
        }
    }
}
