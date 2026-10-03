use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;

use super::dynamic_generation::Selection;
#[cfg(test)]
use super::encode_hex_lower;
use super::offline_rows;
use super::{
    BinaryConfigRow, ConfigChunkRow, ConfigRow, ConfigRowHeader, qualified_storage_table,
    qualified_storage_table_for, quote_string,
};
use crate::runtime_evidence_schema::{SanitizedRuntimeArgumentKind, SubprocessJournalSchema};
use crate::sql::{SqlBackend, SqlClient, SqlExec, SqlTools};

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
/// What the journal names as the executable of a request the built-in client
/// made (there is no process to name).
const CLIENT_JOURNAL_EXECUTABLE: &str = "ibcmd-rs (built-in SQL client)";

/// Journals one request of the built-in client the way a sqlcmd or bcp run
/// is journaled: server, login kind and the query's digest, never its text,
/// and on failure no server message (it may quote data).
fn journal_client_request<T>(
    sql: &SqlExec,
    query: &str,
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let entry = start_client_request(sql, query)?;
    let result = work();
    finish_client_request(entry, result.is_ok())?;
    result
}

/// Enters one request of the built-in client in this thread's journal.
fn start_client_request(sql: &SqlExec, query: &str) -> Result<usize> {
    let mut arguments = vec!["-S".to_owned(), sql.server().to_owned()];
    match sql.user() {
        Some(user) => arguments.extend(["-U".to_owned(), user.to_owned()]),
        None => arguments.push("-E".to_owned()),
    }
    arguments.extend(["-Q".to_owned(), query_marker(query)]);
    start_subprocess_call(SanitizedSubprocessCall {
        executable: CLIENT_JOURNAL_EXECUTABLE.to_owned(),
        arguments,
        started_unix_ms: unix_time_ms(),
        ended_unix_ms: None,
        status: "running".to_owned(),
        exit_code: None,
        timed_out: false,
        exception: None,
    })
}

fn finish_client_request(entry: usize, passed: bool) -> Result<()> {
    complete_subprocess_call(
        entry,
        if passed { "passed" } else { "failed" },
        None,
        (!passed).then(|| "the built-in SQL client request failed".to_owned()),
    )
}

/// `FileName, PartNo, DataSize, BinaryData` rows of a query, as stored.
fn read_row_parts(client: &dyn SqlClient, query: &str) -> Result<Vec<BinaryConfigRow>> {
    let mut parts = Vec::new();
    client.read_rows(query, &[], &mut |mut row| {
        let part_no = row.i64(1)?;
        parts.push(BinaryConfigRow {
            file_name: row.take_text(0)?,
            part_no: i32::try_from(part_no)
                .with_context(|| format!("PartNo {part_no} is out of range"))?,
            data_size: row.i64(2)?,
            binary: row.take_binary(3)?,
        });
        Ok(())
    })?;
    Ok(parts)
}

/// Every Config row a selection names (or every one), assembled from its
/// parts.
pub(super) fn fetch_rows(
    sql: &SqlExec,
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
) -> Result<Vec<ConfigRow>> {
    if let Some(offline) = offline_rows::active() {
        return Ok(config_rows_from_binary(offline.rows_named(
            database,
            table,
            selected_file_names,
        )?));
    }
    let SqlBackend::Tools(tools) = sql.backend() else {
        return fetch_config_rows(sql, database, table, selected_file_names);
    };
    let query = build_fetch_rows_sql(database, table, selected_file_names);
    let stdout = run_sql_capture_tsv_with_policy(
        &tools.sqlcmd,
        sql.server(),
        sql.user(),
        sql.password(),
        sql.trust_server_certificate(),
        &query,
    )?;
    let chunks = parse_config_chunk_rows(&stdout)
        .with_context(|| format!("failed to parse {table} row chunks for {database}"))?;
    assemble_config_rows(chunks)
        .with_context(|| format!("failed to assemble {table} row chunks for {database}"))
}

#[allow(dead_code)]
pub(super) fn fetch_rows_direct_hex(
    sql: &SqlExec,
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
) -> Result<Vec<ConfigRow>> {
    if let Some(offline) = offline_rows::active() {
        return Ok(config_rows_from_binary(offline.rows_named(
            database,
            table,
            selected_file_names,
        )?));
    }
    let SqlBackend::Tools(tools) = sql.backend() else {
        return fetch_config_rows(sql, database, table, selected_file_names);
    };
    let query = build_fetch_rows_direct_hex_sql(database, table, selected_file_names);
    let stdout = run_sql_capture_tsv_with_policy(
        &tools.sqlcmd,
        sql.server(),
        sql.user(),
        sql.password(),
        sql.trust_server_certificate(),
        &query,
    )?;
    parse_config_direct_rows(&stdout)
        .with_context(|| format!("failed to parse direct {table} rows for {database}"))
}

pub(super) fn fetch_binary_rows(
    sql: &SqlExec,
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
    use_range_filter: bool,
) -> Result<Vec<BinaryConfigRow>> {
    if let Some(offline) = offline_rows::active() {
        // A range filter only stands in for the exact batch it was built
        // from, so the folder answers the batch itself.
        return Ok(apply_row_overrides(
            table,
            offline.rows_named(database, table, selected_file_names)?,
        ));
    }
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
                let mut batch_rows = fetch_binary_rows_query(sql, database, table, &query)
                    .with_context(|| {
                        format!("failed to fetch exact batch for {table} rows {first}..{last}")
                    })?;
                rows.append(&mut batch_rows);
            }
            return Ok(rows);
        }
    }

    if use_range_filter
        && let SqlBackend::Client(client) = sql.backend()
        && client.max_connections() > 1
        && selected_file_names.len() >= 2 * client.max_connections()
    {
        return fetch_binary_rows_sliced(sql, client, database, table, selected_file_names);
    }
    let query =
        build_fetch_binary_rows_query(database, table, selected_file_names, use_range_filter);
    fetch_binary_rows_query(sql, database, table, &query)
}

/// A range-filtered batch, read in slices on several connections at once:
/// the batch's file names cut into contiguous runs, each slice from its
/// first name up to (not including) the next slice's first, the last one up
/// to the batch's last name. The slices partition the batch's range in the
/// column's own collation, so they return exactly the parts the single range
/// query returns; the parts of one file name all fall in one slice, in
/// order, and are assembled together.
fn fetch_binary_rows_sliced(
    sql: &SqlExec,
    client: &dyn SqlClient,
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
) -> Result<Vec<BinaryConfigRow>> {
    let queries = sliced_range_queries(
        &qualified_storage_table(database, table),
        selected_file_names,
        client.max_connections(),
    );
    // The journal lives on this thread: each slice is entered before the
    // reads start and completed after they end.
    let entries = queries
        .iter()
        .map(|query| start_client_request(sql, query))
        .collect::<Result<Vec<_>>>()?;
    let results = std::thread::scope(|scope| {
        let readers = queries
            .iter()
            .map(|query| scope.spawn(move || read_row_parts(client, query)))
            .collect::<Vec<_>>();
        readers
            .into_iter()
            .map(|reader| {
                reader
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect::<Vec<_>>()
    });
    for (entry, result) in entries.into_iter().zip(&results) {
        finish_client_request(entry, result.is_ok())?;
    }
    let mut parts = Vec::new();
    for result in results {
        parts.append(
            &mut result
                .with_context(|| format!("failed to read the rows of {database}.{table}"))?,
        );
    }
    assemble_binary_config_rows(parts)
        .map(|rows| apply_row_overrides(table, rows))
        .with_context(|| format!("failed to assemble the rows of {database}.{table}"))
}

/// The range queries of a sliced batch read: at most `slices` contiguous runs
/// of the (sorted) file names, each from its first name up to, not
/// including, the next run's first, the last up to the last name.
fn sliced_range_queries(
    storage: &str,
    file_names: &BTreeSet<String>,
    slices: usize,
) -> Vec<String> {
    let names = file_names.iter().collect::<Vec<_>>();
    let Some(last) = names.last() else {
        return Vec::new();
    };
    let per_slice = names.len().div_ceil(slices.max(1));
    let lows = names
        .chunks(per_slice)
        .map(|chunk| chunk[0].as_str())
        .collect::<Vec<_>>();
    lows.iter()
        .enumerate()
        .map(|(index, low)| {
            let high = match lows.get(index + 1) {
                Some(next) => format!("FileName < N'{}'", quote_string(next)),
                None => format!("FileName <= N'{}'", quote_string(last)),
            };
            format!(
                "SELECT FileName, PartNo, DataSize, BinaryData\n\
                 FROM {storage}\n\
                 WHERE FileName >= N'{}' AND {high}\n\
                 ORDER BY FileName, PartNo",
                quote_string(low)
            )
        })
        .collect()
}

pub(super) fn fetch_binary_rows_query(
    sql: &SqlExec,
    database: &str,
    table: &str,
    query: &str,
) -> Result<Vec<BinaryConfigRow>> {
    let parts = fetch_binary_row_parts(sql, database, table, query)?;
    assemble_binary_config_rows(parts)
        .map(|rows| apply_row_overrides(table, rows))
        .with_context(|| format!("failed to assemble the rows of {database}.{table}"))
}

/// Part 0 of every Config row of a database, keyed by file name -- what
/// staging otherwise fetches one query per object (and what
/// `IBCMD_RS_BASE_ROWS_DIR`'s `<name>__part0.bin` files hold). The built-in
/// client reads it in slices of file names on several connections at once;
/// the `--sqlcmd` path with one `bcp queryout`.
pub(crate) fn fetch_config_part0_rows(
    sql: &SqlExec,
    database: &str,
) -> Result<std::collections::HashMap<String, Vec<u8>>> {
    if let Some(offline) = offline_rows::active() {
        return offline.part0_rows();
    }
    let table = format!("{}.dbo.Config", super::quote_ident(database));
    let query =
        format!("SELECT FileName, PartNo, DataSize, BinaryData FROM {table} WHERE PartNo = 0");
    let SqlBackend::Client(client) = sql.backend() else {
        return Ok(fetch_binary_row_parts(sql, database, "Config", &query)?
            .into_iter()
            .map(|part| (part.file_name, part.binary))
            .collect());
    };
    let slices = client.max_connections();
    let boundaries = if slices > 1 {
        journal_client_request(sql, &query, || {
            part0_slice_boundaries(client, &table, slices)
        })?
    } else {
        Vec::new()
    };
    let queries = slice_queries(&query, &boundaries);
    let parts = std::thread::scope(|scope| {
        let readers = queries
            .iter()
            .map(|slice| scope.spawn(move || read_row_parts(client, slice)))
            .collect::<Vec<_>>();
        let mut parts = Vec::new();
        for reader in readers {
            match reader.join() {
                Ok(slice) => parts.push(slice?),
                Err(panic) => std::panic::resume_unwind(panic),
            }
        }
        Ok::<_, anyhow::Error>(parts)
    })
    .with_context(|| format!("failed to read the Config rows of {database}"))?;
    let mut rows = std::collections::HashMap::new();
    for part in parts.into_iter().flatten() {
        if rows.insert(part.file_name.clone(), part.binary).is_some() {
            bail!("Config row {} was read twice", part.file_name);
        }
    }
    Ok(rows)
}

/// The first file name of each slice after the first, when the rows of
/// `table` with PartNo 0 are cut into `slices` slices of equal row count.
fn part0_slice_boundaries(
    client: &dyn SqlClient,
    table: &str,
    slices: usize,
) -> Result<Vec<String>> {
    let query = format!(
        "SELECT MIN(FileName) FROM (SELECT FileName, NTILE({slices}) OVER (ORDER BY FileName) \
         AS slice FROM {table} WHERE PartNo = 0) AS sliced \
         GROUP BY slice HAVING slice > 1 ORDER BY MIN(FileName)"
    );
    client
        .query_rows(&query, &[])?
        .into_iter()
        .map(|mut row| row.take_text(0))
        .collect()
}

/// `query` (which ends in a WHERE clause) once per slice: the file names
/// from one boundary up to the next. The column's own collation orders both
/// the boundaries and the comparisons, so every row falls in one slice.
fn slice_queries(query: &str, boundaries: &[String]) -> Vec<String> {
    let mut queries = Vec::with_capacity(boundaries.len() + 1);
    for index in 0..=boundaries.len() {
        let mut slice = query.to_owned();
        if let Some(low) = index.checked_sub(1).map(|low| &boundaries[low]) {
            slice.push_str(&format!(" AND FileName >= N'{}'", quote_string(low)));
        }
        if let Some(high) = boundaries.get(index) {
            slice.push_str(&format!(" AND FileName < N'{}'", quote_string(high)));
        }
        queries.push(slice);
    }
    queries
}

/// Runs one query of `FileName, PartNo, DataSize, BinaryData` rows and
/// returns the parts as stored, unassembled: the built-in client streams
/// them; the `--sqlcmd` path runs `bcp queryout` into a native-format file.
fn fetch_binary_row_parts(
    sql: &SqlExec,
    database: &str,
    table: &str,
    query: &str,
) -> Result<Vec<BinaryConfigRow>> {
    if offline_rows::active().is_some() {
        return Err(offline_rows::refuse(&format!(
            "the query {}",
            query_marker(query)
        )));
    }
    match sql.backend() {
        SqlBackend::Client(client) => journal_client_request(sql, query, || {
            read_row_parts(client, query)
                .with_context(|| format!("failed to read the rows of {database}.{table}"))
        }),
        SqlBackend::Tools(tools) => bcp_queryout_row_parts(sql, tools, database, table, query),
    }
}

fn bcp_queryout_row_parts(
    sql: &SqlExec,
    tools: &SqlTools,
    database: &str,
    table: &str,
    query: &str,
) -> Result<Vec<BinaryConfigRow>> {
    let (bcp, server, user, password) = (&tools.bcp, sql.server(), sql.user(), sql.password());
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
        "-a".to_owned(),
        "65535".to_owned(),
    ];
    if sql.trust_server_certificate() {
        arguments.push("-u".to_owned());
    }
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
    parse_bcp_native_config_rows(&bytes)
        .with_context(|| format!("failed to parse native bcp rows for {database}.{table}"))
}

/// The virtual load cycle, second half: when `IBCMD_RS_ROW_OVERRIDE_DIR` names
/// a directory holding `<file name>.bin`, a `Config` row is read with those
/// stored bytes instead of the database's. `mssql-audit-source-parity` writes
/// the rows a load would stage there (`IBCMD_RS_WRITE_STAGED_ROWS_DIR`), so an
/// export run this way reads exactly what a load followed by an export would.
fn apply_row_overrides(table: &str, mut rows: Vec<BinaryConfigRow>) -> Vec<BinaryConfigRow> {
    static DIR: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    let Some(dir) = DIR
        .get_or_init(|| std::env::var_os("IBCMD_RS_ROW_OVERRIDE_DIR").map(PathBuf::from))
        .as_ref()
    else {
        return rows;
    };
    if !table
        .trim_matches(['[', ']'])
        .eq_ignore_ascii_case("Config")
    {
        return rows;
    }
    for row in &mut rows {
        if row.part_no != 0 {
            continue;
        }
        if let Ok(bytes) = fs::read(dir.join(format!("{}.bin", row.file_name))) {
            row.data_size = bytes.len() as i64;
            row.binary = bytes;
        }
    }
    rows
}

#[allow(dead_code)]
pub(super) fn fetch_metadata_rows_hex(
    sql: &SqlExec,
    database: &str,
    table: &str,
) -> Result<Vec<ConfigRow>> {
    if let Some(offline) = offline_rows::active() {
        return Ok(config_rows_from_binary(offline.rows(
            database,
            table,
            |file_name| !file_name.contains('.'),
        )?));
    }
    let SqlBackend::Tools(tools) = sql.backend() else {
        return fetch_metadata_rows(sql, database, table);
    };
    let query = build_fetch_metadata_rows_sql(database, table);
    let stdout = run_sql_capture_tsv_with_policy(
        &tools.sqlcmd,
        sql.server(),
        sql.user(),
        sql.password(),
        sql.trust_server_certificate(),
        &query,
    )?;
    let chunks = parse_config_chunk_rows(&stdout)
        .with_context(|| format!("failed to parse {table} metadata row chunks for {database}"))?;
    assemble_config_rows(chunks)
        .with_context(|| format!("failed to assemble {table} metadata rows for {database}"))
}

/// Every metadata row (a file name without a dot) of the table.
pub(super) fn fetch_metadata_rows(
    sql: &SqlExec,
    database: &str,
    table: &str,
) -> Result<Vec<ConfigRow>> {
    if let Some(offline) = offline_rows::active() {
        let rows = offline.rows(database, table, |file_name| !file_name.contains('.'))?;
        return Ok(config_rows_from_binary(apply_row_overrides(table, rows)));
    }
    let query = build_fetch_metadata_rows_bcp_query(database, table);
    let rows = fetch_binary_rows_query(sql, database, table, &query)?;
    Ok(config_rows_from_binary(rows))
}

pub(super) fn fetch_metadata_owner_rows(
    sql: &SqlExec,
    database: &str,
    table: &str,
    metadata_file_names: &BTreeSet<String>,
) -> Result<Vec<ConfigRow>> {
    if metadata_file_names.is_empty() {
        return Ok(Vec::new());
    }
    if let Some(offline) = offline_rows::active() {
        // `FileName = N'<name>' OR FileName LIKE N'<name>.%'`; the names
        // carry no dot, so the owner of a dotted row is what precedes its
        // first dot.
        let rows = offline.rows(database, table, |file_name| {
            metadata_file_names.contains(file_name)
                || file_name
                    .split_once('.')
                    .is_some_and(|(owner, _)| metadata_file_names.contains(owner))
        })?;
        return Ok(config_rows_from_binary(apply_row_overrides(table, rows)));
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
            fetch_binary_rows_query(sql, database, table, &query).with_context(|| {
                format!("failed to fetch metadata owner rows batch {first}..{last}")
            })?;
        rows.append(&mut batch_rows);
    }
    Ok(config_rows_from_binary(rows))
}

pub(super) fn fetch_config_rows(
    sql: &SqlExec,
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
) -> Result<Vec<ConfigRow>> {
    let rows = fetch_binary_rows(sql, database, table, selected_file_names, false)?;
    Ok(config_rows_from_binary(rows))
}

pub(super) fn config_rows_from_binary(rows: Vec<BinaryConfigRow>) -> Vec<ConfigRow> {
    rows.into_iter().map(config_row_from_binary).collect()
}

pub(super) fn config_row_from_binary(row: BinaryConfigRow) -> ConfigRow {
    #[cfg(test)]
    let binary_hex = encode_hex_lower(&row.binary);
    #[cfg(not(test))]
    let binary_hex = String::new();
    ConfigRow {
        file_name: row.file_name,
        part_no: row.part_no,
        data_size: row.data_size,
        binary_hex,
        #[cfg(not(test))]
        binary: Some(row.binary),
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

    let selection = if use_range_filter {
        Selection::All
    } else {
        Selection::Names(selected_file_names)
    };
    format!(
        "SELECT FileName, PartNo, DataSize, BinaryData\n\
         FROM {qualified_table}\n\
         {filter}\
         ORDER BY FileName, PartNo",
        qualified_table = qualified_storage_table_for(database, table, selection),
        filter = filter,
    )
}

pub(super) fn split_selected_file_names_for_bcp_query(
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
        qualified_table =
            qualified_storage_table_for(database, table, Selection::Owners(metadata_file_names)),
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
    sql: &SqlExec,
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
) -> Result<Vec<ConfigRowHeader>> {
    if let Some(offline) = offline_rows::active() {
        return offline.headers(database, table, selected_file_names);
    }
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
                let query = build_fetch_row_headers_sql(database, table, &batch);
                let mut batch_rows = fetch_row_headers_query(sql, &query).with_context(|| {
                    format!(
                        "failed to fetch {table} row headers of {database} rows {first}..{last}"
                    )
                })?;
                rows.append(&mut batch_rows);
            }
            return Ok(rows);
        }
    }

    let query = build_fetch_row_headers_sql(database, table, selected_file_names);
    fetch_row_headers_query(sql, &query)
        .with_context(|| format!("failed to fetch {table} row headers of {database}"))
}

/// `file_name, part_no, data_size` rows of one header query.
fn fetch_row_headers_query(sql: &SqlExec, query: &str) -> Result<Vec<ConfigRowHeader>> {
    match sql.backend() {
        SqlBackend::Client(client) => journal_client_request(sql, query, || {
            client
                .query_rows(query, &[])?
                .into_iter()
                .map(|mut row| {
                    let part_no = row.i64(1)?;
                    Ok(ConfigRowHeader {
                        file_name: row.take_text(0)?,
                        part_no: i32::try_from(part_no)
                            .with_context(|| format!("PartNo {part_no} is out of range"))?,
                        data_size: row.i64(2)?,
                    })
                })
                .collect()
        }),
        SqlBackend::Tools(tools) => {
            let stdout = run_sql_capture_tsv_with_policy(
                &tools.sqlcmd,
                sql.server(),
                sql.user(),
                sql.password(),
                sql.trust_server_certificate(),
                query,
            )?;
            parse_config_row_headers(&stdout)
        }
    }
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
        qualified_table =
            qualified_storage_table_for(database, table, Selection::Names(selected_file_names)),
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
        qualified_table =
            qualified_storage_table_for(database, table, Selection::Names(selected_file_names)),
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
        qualified_table =
            qualified_storage_table_for(database, table, Selection::Names(selected_file_names)),
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
            #[cfg(not(test))]
            binary: None,
        });
    }
    Ok(rows)
}

pub(super) fn parse_config_chunk_rows(stdout: &str) -> Result<Vec<ConfigChunkRow>> {
    let mut rows = Vec::new();
    for (line_index, line) in stdout.lines().enumerate() {
        // Preserve a trailing tab: it represents an empty BinaryData chunk.
        let line = line.trim_end_matches(['\r', ' ']);
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
                #[cfg(not(test))]
                binary: None,
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
                #[cfg(not(test))]
                binary: None,
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
    run_sql_capture_tsv_with_policy(sqlcmd, server, user, password, true, sql)
}

#[derive(Debug)]
pub(super) struct ExactStorageRow {
    pub file_name: String,
    pub part_no: i32,
    pub creation: String,
    pub modified: String,
    pub attributes: i32,
    pub data_size: i64,
    pub binary: Vec<u8>,
}

#[cfg(test)]
mod activation_capture_tests {
    use super::*;
    use crate::sql::{Dbms, ScriptVariables, SqlLogin, SqlParam, SqlRow, SqlTarget, SqlValue};
    use sha2::{Digest, Sha256};
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    struct CaptureClient {
        replies: Mutex<VecDeque<Vec<SqlRow>>>,
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl SqlClient for CaptureClient {
        fn dbms(&self) -> Dbms {
            Dbms::SqlServer
        }
        fn max_connections(&self) -> usize {
            1
        }
        fn read_rows(
            &self,
            query: &str,
            _: &[SqlParam<'_>],
            each: &mut dyn FnMut(SqlRow) -> Result<()>,
        ) -> Result<()> {
            self.calls.lock().unwrap().push(query.to_owned());
            for row in self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected SQL request")
            {
                each(row)?;
            }
            Ok(())
        }
        fn execute(&self, _: &str, _: &[SqlParam<'_>]) -> Result<u64> {
            panic!("capture must never write")
        }
        fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
            panic!("capture must never write")
        }
        fn query_json(&self, _: &str) -> Result<Option<String>> {
            panic!("unexpected query")
        }
        fn write_rows(&self, _: &str, _: &[&str], _: &[Vec<SqlParam<'_>>]) -> Result<u64> {
            panic!("capture must never write")
        }
    }

    fn header(bytes: &[u8], attributes: i64, modified: &str) -> SqlRow {
        SqlRow {
            result_set: 0,
            values: vec![
                SqlValue::Text("root".to_owned()),
                SqlValue::Int(0),
                SqlValue::Int(attributes),
                SqlValue::Text("4026-10-01 01:02:03.004".to_owned()),
                SqlValue::Text(modified.to_owned()),
                SqlValue::Int(bytes.len() as i64),
                SqlValue::Int(bytes.len() as i64),
                SqlValue::Text(crate::mssql_config_apply::model::hex_upper(
                    &Sha256::digest(bytes),
                )),
            ],
        }
    }

    fn binary(bytes: &[u8]) -> SqlRow {
        SqlRow {
            result_set: 0,
            values: vec![
                SqlValue::Text("root".to_owned()),
                SqlValue::Int(0),
                SqlValue::Int(bytes.len() as i64),
                SqlValue::Binary(bytes.to_vec()),
            ],
        }
    }

    fn capture(replies: Vec<Vec<SqlRow>>) -> (SqlExec, Arc<Mutex<Vec<String>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let sql = SqlExec::with_client(
            SqlTarget {
                server: "fake".to_owned(),
                database: None,
                login: SqlLogin::Integrated,
                trust_server_certificate: false,
            },
            Box::new(CaptureClient {
                replies: Mutex::new(replies.into()),
                calls: calls.clone(),
            }),
        );
        (sql, calls)
    }

    #[test]
    fn activation_capture_keeps_nonzero_flags_and_exact_platform_dates() {
        let h = header(b"ABC", 17, "4026-10-01 01:02:04.007");
        let (sql, calls) = capture(vec![vec![h.clone()], vec![binary(b"ABC")], vec![h]]);
        let rows =
            fetch_activation_rows(&sql, "db", "Config", &BTreeSet::from(["root".to_owned()]))
                .unwrap();
        assert_eq!(rows[0].attributes, 17);
        assert_eq!(rows[0].creation, "4026-10-01 01:02:03.004");
        assert_eq!(rows[0].modified, "4026-10-01 01:02:04.007");
        assert_eq!(rows[0].binary_data, b"ABC");
        assert_eq!(calls.lock().unwrap().len(), 3);
    }

    #[test]
    fn activation_capture_refuses_binary_aba_and_full_header_drift() {
        let h = header(b"ABC", 17, "4026-10-01 01:02:04.007");
        // Even if a second header pass would have returned A, bytes B may not bind to A.
        let (sql, _) = capture(vec![vec![h.clone()], vec![binary(b"XYZ")], vec![h.clone()]]);
        assert!(
            fetch_activation_rows(&sql, "db", "Config", &BTreeSet::new())
                .unwrap_err()
                .to_string()
                .contains("changed during capture")
        );
        for after in [
            header(b"ABC", 18, "4026-10-01 01:02:04.007"),
            header(b"ABC", 17, "4026-10-01 01:02:04.008"),
        ] {
            let (sql, _) = capture(vec![vec![h.clone()], vec![binary(b"ABC")], vec![after]]);
            assert!(
                fetch_activation_rows(&sql, "db", "Config", &BTreeSet::new())
                    .unwrap_err()
                    .to_string()
                    .contains("physical headers changed")
            );
        }
    }

    #[test]
    fn activation_capture_checks_budget_before_binary_materialization() {
        let (sql, calls) = capture(vec![vec![header(b"ABC", 0, "4026-10-01 01:02:04.007")]]);
        assert!(
            fetch_bound_activation_rows(
                &sql,
                "db",
                "Config",
                "",
                ActivationCaptureBudget {
                    rows: 1,
                    row_bytes: 16,
                    total_bytes: 2
                }
            )
            .unwrap_err()
            .to_string()
            .contains("exceeds 2 bytes")
        );
        assert_eq!(calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn larger_extension_capture_keeps_existing_budget_and_bounds_sql_batches() {
        let headers: Vec<_> = (0..70)
            .map(|n| {
                let mut row = header(b"ABC", 1, "4026-10-01 01:02:04.007");
                row.values[0] = SqlValue::Text(format!("cas_{n:03}"));
                row
            })
            .collect();
        let binaries: Vec<_> = (0..70)
            .map(|n| {
                let mut row = binary(b"ABC");
                row.values[0] = SqlValue::Text(format!("cas_{n:03}"));
                row
            })
            .collect();
        let (sql, calls) = capture(vec![
            headers.clone(),
            binaries[..64].to_vec(),
            binaries[64..].to_vec(),
            headers,
        ]);
        let rows = fetch_exact_prefix_rows(&sql, "db", "ConfigCAS", "cas_", 100, 512 * 1024 * 1024)
            .unwrap();
        assert_eq!(rows.len(), 70);
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 4);
        for query in &calls[1..3] {
            assert!(query.matches("HASHBYTES('SHA2_256',BinaryData)=0x").count() <= 64);
            assert!(!query.contains("_dynupdate_"));
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ActivationHeader {
    name: String,
    part: i32,
    attributes: i32,
    creation: String,
    modified: String,
    size: i64,
    digest: Vec<u8>,
}

#[derive(Clone, Copy)]
struct ActivationCaptureBudget {
    rows: usize,
    row_bytes: u64,
    total_bytes: u64,
}

fn read_activation_headers(
    sql: &SqlExec,
    database: &str,
    table: &str,
    filter: &str,
    budget: ActivationCaptureBudget,
) -> Result<Vec<ActivationHeader>> {
    let query = format!(
        "SET NOCOUNT ON; SELECT TOP ({}) FileName AS file_name,PartNo,CONVERT(int,Attributes),CONVERT(varchar(27),Creation,121),CONVERT(varchar(27),Modified,121),CONVERT(bigint,DataSize),CONVERT(bigint,DATALENGTH(BinaryData)),CONVERT(varchar(64),HASHBYTES('SHA2_256',BinaryData),2) FROM {} {filter} ORDER BY FileName,PartNo;",
        budget
            .rows
            .checked_add(1)
            .ok_or_else(|| anyhow!("activation row budget overflow"))?,
        qualified_storage_table(database, table),
    );
    if offline_rows::active().is_some() {
        return Err(offline_rows::refuse(&format!(
            "the activation query {}",
            query_marker(&query)
        )));
    }
    let fields: Vec<Vec<String>> = match sql.backend() {
        SqlBackend::Client(client) => journal_client_request(sql, &query, || {
            client
                .query_rows(&query, &[])?
                .into_iter()
                .map(|mut row| {
                    Ok(vec![
                        row.take_text(0)?,
                        row.i64(1)?.to_string(),
                        row.i64(2)?.to_string(),
                        row.take_text(3)?,
                        row.take_text(4)?,
                        row.i64(5)?.to_string(),
                        row.i64(6)?.to_string(),
                        row.take_text(7)?,
                    ])
                })
                .collect::<Result<Vec<_>>>()
        })?,
        SqlBackend::Tools(tools) => {
            let text = run_sql_capture_tsv_with_policy(
                &tools.sqlcmd,
                sql.server(),
                sql.user(),
                sql.password(),
                sql.trust_server_certificate(),
                &query,
            )?;
            text.lines()
                .filter(|line| !line.trim().is_empty() && !is_sqlcmd_header_or_separator(line))
                .map(|line| {
                    line.split('\t')
                        .map(|field| field.trim_end().to_owned())
                        .collect()
                })
                .collect()
        }
    };
    parse_activation_headers(fields, budget)
}

fn parse_activation_headers(
    fields: Vec<Vec<String>>,
    budget: ActivationCaptureBudget,
) -> Result<Vec<ActivationHeader>> {
    if fields.len() > budget.rows {
        bail!("activation row count exceeds {}", budget.rows);
    }
    let mut seen = BTreeSet::new();
    let mut total = 0_u64;
    let mut headers = Vec::with_capacity(fields.len());
    for fields in fields {
        if fields.len() != 8 {
            bail!("unexpected activation header shape");
        }
        let name = fields[0].clone();
        let part = fields[1].trim().parse::<i32>()?;
        let attributes = fields[2].trim().parse::<i32>()?;
        let size = fields[5].trim().parse::<i64>()?;
        let length = fields[6].trim().parse::<i64>()?;
        if name.is_empty() || name.chars().any(char::is_control) || part != 0 {
            bail!("activation header has an unsupported name/PartNo");
        }
        if !seen.insert(name.to_lowercase()) {
            bail!("duplicate activation row {name}");
        }
        if size < 0 || size != length || size as u64 > budget.row_bytes {
            bail!("activation row {name} has an invalid or oversized binary length");
        }
        total = total
            .checked_add(size as u64)
            .ok_or_else(|| anyhow!("activation byte budget overflow"))?;
        if total > budget.total_bytes {
            bail!("activation data exceeds {} bytes", budget.total_bytes);
        }
        if fields[3].is_empty() || fields[4].is_empty() {
            bail!("activation row timestamps are missing");
        }
        let digest = super::decode_hex(fields[7].trim())?;
        if digest.len() != 32 {
            bail!("activation header SHA256 is malformed");
        }
        headers.push(ActivationHeader {
            name,
            part,
            attributes,
            creation: fields[3].clone(),
            modified: fields[4].clone(),
            size,
            digest,
        });
    }
    Ok(headers)
}

fn activation_binary_filter(headers: &[ActivationHeader]) -> String {
    let predicates = headers.iter().map(|row| format!(
        "(FileName=N'{}' AND PartNo={} AND Attributes={} AND CONVERT(varchar(27),Creation,121)='{}' AND CONVERT(varchar(27),Modified,121)='{}' AND CONVERT(bigint,DataSize)={} AND DATALENGTH(BinaryData)={} AND HASHBYTES('SHA2_256',BinaryData)=0x{})",
        quote_string(&row.name), row.part, row.attributes, quote_string(&row.creation),
        quote_string(&row.modified), row.size, row.size,
        crate::mssql_config_apply::model::hex_upper(&row.digest)))
        .collect::<Vec<_>>().join(" OR ");
    format!("WHERE {predicates}\n")
}

fn bind_activation_binary(
    headers: &[ActivationHeader],
    rows: Vec<BinaryConfigRow>,
) -> Result<Vec<ExactStorageRow>> {
    use sha2::{Digest, Sha256};
    let mut expected = headers
        .iter()
        .map(|row| (row.name.clone(), row))
        .collect::<BTreeMap<_, _>>();
    let mut output = Vec::with_capacity(headers.len());
    for row in rows {
        let header = expected
            .remove(&row.file_name)
            .ok_or_else(|| anyhow!("unexpected activation binary row"))?;
        if row.part_no != header.part
            || row.data_size != header.size
            || row.binary.len() as i64 != header.size
            || Sha256::digest(&row.binary).as_slice() != header.digest
        {
            bail!("activation row {} changed during capture", row.file_name);
        }
        output.push(ExactStorageRow {
            file_name: row.file_name,
            part_no: header.part,
            creation: header.creation.clone(),
            modified: header.modified.clone(),
            attributes: header.attributes,
            data_size: header.size,
            binary: row.binary,
        });
    }
    if !expected.is_empty() {
        bail!("activation BinaryData row set changed during capture");
    }
    Ok(output)
}

fn fetch_bound_activation_rows(
    sql: &SqlExec,
    database: &str,
    table: &str,
    filter: &str,
    budget: ActivationCaptureBudget,
) -> Result<Vec<ExactStorageRow>> {
    let headers = read_activation_headers(sql, database, table, filter, budget)?;
    if headers.is_empty() {
        return Ok(Vec::new());
    }
    let mut all_rows = Vec::with_capacity(headers.len());
    // Bound SQL expression size even for larger extension namespace budgets.
    for chunk in headers.chunks(64) {
        let selected = chunk
            .iter()
            .map(|row| row.name.clone())
            .collect::<BTreeSet<_>>();
        let binary_filter = activation_binary_filter(chunk);
        let rows = match sql.backend() {
            SqlBackend::Client(_) => {
                // The server only returns bytes matching the bounded immutable header pass.
                let query = format!(
                    "SELECT FileName,PartNo,DataSize,BinaryData FROM {} {binary_filter} ORDER BY FileName,PartNo",
                    qualified_storage_table(database, table)
                );
                assemble_binary_config_rows(fetch_binary_row_parts(sql, database, table, &query)?)?
            }
            SqlBackend::Tools(tools) => {
                let query = build_fetch_rows_sql(database, table, &selected);
                let original = selected
                    .iter()
                    .map(|name| format!("N'{}'", quote_string(name)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let original = format!("WHERE FileName IN ({original})\n");
                if query.matches(&original).count() != 1 {
                    bail!("activation chunk query has no unique bounded filter");
                }
                let query = query.replace(&original, &binary_filter);
                let text = run_sql_capture_tsv_with_policy(
                    &tools.sqlcmd,
                    sql.server(),
                    sql.user(),
                    sql.password(),
                    sql.trust_server_certificate(),
                    &query,
                )?;
                assemble_config_rows(parse_config_chunk_rows(&text)?)?
                    .into_iter()
                    .map(|row| {
                        Ok(BinaryConfigRow {
                            file_name: row.file_name.clone(),
                            part_no: row.part_no,
                            data_size: row.data_size,
                            binary: row.binary_bytes()?.into_owned(),
                        })
                    })
                    .collect::<Result<Vec<_>>>()?
            }
        };
        all_rows.extend(rows);
    }
    let output = bind_activation_binary(&headers, all_rows)?;
    // A->B->A during the binary read cannot substitute B: its digest is bound to A above.
    if read_activation_headers(sql, database, table, filter, budget)? != headers {
        bail!("activation full physical headers changed during capture");
    }
    Ok(output)
}

pub(super) fn fetch_activation_rows(
    sql: &SqlExec,
    database: &str,
    table: &str,
    selected: &BTreeSet<String>,
) -> Result<Vec<crate::mssql_main_activation::MainStorageRow>> {
    let filter = if selected.is_empty() {
        String::new()
    } else {
        format!(
            "WHERE FileName IN ({})",
            selected
                .iter()
                .map(|name| format!("N'{}'", quote_string(name)))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    fetch_bound_activation_rows(
        sql,
        database,
        table,
        &filter,
        ActivationCaptureBudget {
            rows: crate::mssql_main_activation::MAX_ROWS,
            row_bytes: crate::mssql_main_activation::MAX_ROW_BYTES as u64,
            total_bytes: crate::mssql_main_activation::MAX_PLAN_BYTES as u64,
        },
    )?
    .into_iter()
    .map(|row| {
        Ok(crate::mssql_main_activation::MainStorageRow {
            file_name: row.file_name,
            part_no: row.part_no,
            creation: row.creation,
            modified: row.modified,
            attributes: row.attributes,
            data_size: u64::try_from(row.data_size)?,
            binary_data: row.binary,
        })
    })
    .collect()
}

/// Reads a previously bound extension namespace with the same full-header/digest capture.
pub(super) fn fetch_exact_prefix_rows(
    sql: &SqlExec,
    database: &str,
    table: &str,
    prefix: &str,
    max_rows: usize,
    max_total_bytes: u64,
) -> Result<Vec<ExactStorageRow>> {
    let escaped = prefix
        .replace('~', "~~")
        .replace('%', "~%")
        .replace('_', "~_");
    let filter = format!(
        "WHERE FileName LIKE N'{}%' ESCAPE N'~'",
        quote_string(&escaped)
    );
    let rows = fetch_bound_activation_rows(
        sql,
        database,
        table,
        &filter,
        ActivationCaptureBudget {
            rows: max_rows,
            row_bytes: max_total_bytes,
            total_bytes: max_total_bytes,
        },
    )?;
    if rows.iter().any(|row| !row.file_name.starts_with(prefix)) {
        bail!("exact-prefix query returned an unrelated row");
    }
    Ok(rows)
}

pub(super) fn run_sql_capture_tsv_with_policy(
    sqlcmd: &Path,
    server: &str,
    user: Option<&str>,
    password: Option<&str>,
    trust_server_certificate: bool,
    sql: &str,
) -> Result<String> {
    if offline_rows::active().is_some() {
        return Err(offline_rows::refuse(&format!(
            "the sqlcmd query {}",
            query_marker(sql)
        )));
    }
    let mut arguments = vec!["-S".to_owned(), server.to_owned()];
    if trust_server_certificate {
        arguments.insert(0, "-C".to_owned());
    }
    let mut sanitized_arguments = arguments.clone();
    if let Some(user) = user {
        arguments.extend(["-U".to_owned(), user.to_owned()]);
        sanitized_arguments.extend(["-U".to_owned(), user.to_owned()]);
    } else {
        arguments.push("-E".to_owned());
        sanitized_arguments.push("-E".to_owned());
    }
    let common_arguments = [
        "-b", "-r", "1", "-x", "-f", "65001", "-s", "\t", "-w", "65535", "-y", "0", "-Y", "0",
    ]
    .map(ToOwned::to_owned);
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
    let mut command = Command::new(sqlcmd);
    command.args(&arguments);
    if let Some(password) = password.filter(|_| user.is_some()) {
        command.env("SQLCMDPASSWORD", password);
    }
    let output = command.output();
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

    use std::path::Path;

    use super::{
        BCP_INLINE_QUERY_MAX_CHARS, FAIL_NEXT_JOURNAL_PERSIST, SanitizedSubprocessCall,
        begin_subprocess_journal, build_fetch_binary_rows_query,
        build_fetch_metadata_owner_rows_bcp_query, build_fetch_row_headers_sql,
        complete_subprocess_call, fetch_binary_rows_query, journal_client_request,
        password_source_marker, query_marker, redact_password_value, run_sql_capture_tsv,
        slice_queries, sliced_range_queries, split_selected_file_names_for_bcp_query,
        split_selected_file_names_for_owner_rows_query,
        split_selected_file_names_for_row_headers_query, start_subprocess_call,
    };
    use crate::sql::{SqlExec, SqlOptions};

    #[test]
    fn tools_export_reads_obey_certificate_policy_in_actual_arguments() {
        use crate::sql::{SqlLogin, SqlTarget, SqlTools};
        for trust in [false, true] {
            let missing = std::env::temp_dir().join(format!(
                "ibcmd-certificate-tool-missing-{}",
                uuid::Uuid::new_v4()
            ));
            let sql = SqlExec::with_tools(
                SqlTarget {
                    server: "must-not-connect".into(),
                    database: None,
                    login: SqlLogin::Integrated,
                    trust_server_certificate: trust,
                },
                SqlTools {
                    sqlcmd: missing.clone(),
                    bcp: missing,
                },
            );
            let guard =
                begin_subprocess_journal("<password-source:none>", "fake", "db", None).unwrap();
            let names = BTreeSet::from(["root".to_owned()]);
            assert!(super::fetch_rows(&sql, "db", "Config", &names).is_err());
            assert!(super::fetch_rows_direct_hex(&sql, "db", "Config", &names).is_err());
            assert!(super::fetch_metadata_rows_hex(&sql, "db", "Config").is_err());
            assert!(super::fetch_row_headers_query(&sql, "SELECT 1").is_err());
            assert!(super::fetch_binary_row_parts(&sql, "db", "Config", "SELECT 1").is_err());
            let calls = super::current_subprocess_calls();
            assert_eq!(calls.len(), 5);
            for call in &calls[..4] {
                assert_eq!(
                    call.arguments.iter().any(|x| x == "-C"),
                    trust,
                    "actual sqlcmd arguments: {:?}",
                    call.arguments
                );
            }
            assert_eq!(
                calls[4].arguments.iter().any(|x| x == "-u"),
                trust,
                "actual bcp arguments: {:?}",
                calls[4].arguments
            );
            assert!(
                calls
                    .iter()
                    .all(|call| call.status == "failed" && call.exit_code.is_none())
            );
            guard
                .finish_failed(&anyhow::anyhow!("intentional missing tools"))
                .unwrap();
        }
    }

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
        let sql = SqlExec::from_options(SqlOptions {
            bcp: Some(&configured_bcp),
            ..SqlOptions::integrated("localhost", Some(Path::new("sqlcmd")))
        })
        .unwrap();
        let error = fetch_binary_rows_query(&sql, "test_db", "Config", "SELECT 1").unwrap_err();
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
    fn built_in_client_requests_are_journaled_without_query_text() {
        let journal_path = std::env::temp_dir().join(format!(
            "ibcmd-rs-client-runtime-{}.json",
            uuid::Uuid::new_v4().simple()
        ));
        let guard = begin_subprocess_journal(
            "<password-source:none>",
            "127.0.0.1,1",
            "test_db",
            Some(&journal_path),
        )
        .unwrap();
        let sql = SqlExec::from_options(SqlOptions::integrated("127.0.0.1,1", None)).unwrap();
        let journaled = journal_client_request(&sql, "SELECT top-secret-query", || {
            Err::<(), _>(anyhow::anyhow!("server said: top-secret-value"))
        })
        .unwrap_err();
        guard.finish_failed(&journaled).unwrap();
        let journal: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&journal_path).unwrap()).unwrap();
        assert_eq!(journal["calls"][0]["status"], "failed");
        assert_eq!(
            journal["calls"][0]["executable"],
            super::CLIENT_JOURNAL_EXECUTABLE
        );
        let serialized = journal.to_string();
        assert!(!serialized.contains("top-secret-query"));
        assert!(!serialized.contains("top-secret-value"));
        assert!(serialized.contains("<query-sha256:"));
        let _ = fs::remove_file(journal_path);
    }

    #[test]
    fn batch_slices_run_from_one_first_name_to_the_next() {
        let names = ["a", "b", "c", "d", "e"]
            .map(str::to_owned)
            .into_iter()
            .collect::<BTreeSet<_>>();
        let queries = sliced_range_queries("[db].dbo.[Config]", &names, 2);
        assert_eq!(queries.len(), 2);
        assert!(queries[0].contains("WHERE FileName >= N'a' AND FileName < N'd'"));
        assert!(queries[1].contains("WHERE FileName >= N'd' AND FileName <= N'e'"));
        assert!(
            queries
                .iter()
                .all(|query| query.ends_with("ORDER BY FileName, PartNo"))
        );
        // More slices than names: one slice a name, still gap-free.
        let queries = sliced_range_queries("t", &names, 8);
        assert_eq!(queries.len(), 5);
        assert!(queries[4].contains("FileName >= N'e' AND FileName <= N'e'"));
        let quoted = BTreeSet::from(["it's".to_owned()]);
        assert!(sliced_range_queries("t", &quoted, 4)[0].contains("N'it''s'"));
        assert!(sliced_range_queries("t", &BTreeSet::new(), 4).is_empty());
    }

    #[test]
    fn part0_slices_cover_every_file_name_once() {
        let query = "SELECT FileName FROM [db].dbo.Config WHERE PartNo = 0";
        assert_eq!(slice_queries(query, &[]), vec![query.to_owned()]);
        let slices = slice_queries(query, &["b".to_owned(), "it's".to_owned()]);
        assert_eq!(
            slices,
            vec![
                format!("{query} AND FileName < N'b'"),
                format!("{query} AND FileName >= N'b' AND FileName < N'it''s'"),
                format!("{query} AND FileName >= N'it''s'"),
            ]
        );
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
