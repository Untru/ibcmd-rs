//! The T-SQL of the own apply: the fingerprint queries the plan and the
//! script share, and the one-transaction script itself.
//!
//! Nothing here moves row bytes through the client: rows travel from
//! `ConfigSave` to `Config` inside the server, and the script proves it with
//! aggregate fingerprints computed there.

use std::fmt::Write as _;

use anyhow::{Result, bail};
use uuid::Uuid;

use super::model::{hex_upper, quote_ident, quote_string};

/// Error numbers of the script's `THROW`s (57300..57399).
pub mod code {
    pub const LOCK_BUSY: u32 = 57300;
    pub const NO_VIEW_SERVER_STATE: u32 = 57301;
    pub const OTHER_SESSIONS: u32 = 57302;
    pub const STAGE_DRIFTED: u32 = 57303;
    pub const ACTIVE_DRIFTED: u32 = 57304;
    pub const SPECIAL_DRIFTED: u32 = 57305;
    pub const FILES_DRIFTED: u32 = 57306;
    pub const UNFINISHED_OPERATION: u32 = 57307;
    pub const REPLACE_COUNT: u32 = 57308;
    pub const POSTCONDITION: u32 = 57309;
    pub const CLEANUP: u32 = 57310;
    pub const MARKER_CLEANUP: u32 = 57311;
    pub const ALIAS_LEFT: u32 = 57312;
    pub const CHANGE_REGISTRATION: u32 = 57313;
    pub const FILES_WRITE: u32 = 57314;
    pub const PARAMS_WRITE: u32 = 57315;
    pub const UNFINISHED_SCHEMA: u32 = 57316;
    pub const ALREADY_REGISTERED: u32 = 57317;
    pub const NODES_DRIFTED: u32 = 57318;
    pub const NEW_REGISTRATION: u32 = 57319;
    pub const REMOVED_DRIFTED: u32 = 57320;
    pub const REMOVE_COUNT: u32 = 57321;
}

/// Names whose presence means an earlier operation did not finish
/// (`ibcmd infobase config repair` territory).
pub const UNFINISHED_NAMES: [&str; 6] = [
    "commit",
    "dynamicCommit",
    "dbStruFinal",
    "convertPhase",
    "erase_save",
    "deleted",
];

/// An order-independent digest of a set of storage rows, computed by the
/// server: the row count, the byte total, and three sums of 32-bit slices of
/// a SHA-256 over each row's name, part, size and content hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub struct Fingerprint {
    pub rows: i64,
    pub bytes: i64,
    pub h1: i64,
    pub h2: i64,
    pub h3: i64,
}

impl Fingerprint {
    pub fn from_values(values: &[i64]) -> Result<Self> {
        match values {
            [rows, bytes, h1, h2, h3] => Ok(Self {
                rows: *rows,
                bytes: *bytes,
                h1: *h1,
                h2: *h2,
                h3: *h3,
            }),
            _ => bail!(
                "a fingerprint row needs five integers, got {}",
                values.len()
            ),
        }
    }

    pub fn hex(&self) -> String {
        format!(
            "{}:{}:{:x}:{:x}:{:x}",
            self.rows, self.bytes, self.h1, self.h2, self.h3
        )
    }
}

/// `SELECT n, bytes, h1, h2, h3` over the rows `source` yields. `source` is
/// the text after `FROM` and must alias the table as `s`.
pub fn fingerprint_select(source: &str) -> String {
    format!(
        "SELECT COUNT_BIG(*) AS n, ISNULL(SUM(CONVERT(bigint, t.DataSize)), 0) AS bytes, \
         ISNULL(SUM(CONVERT(bigint, CAST(SUBSTRING(t.d, 1, 4) AS int))), 0) AS h1, \
         ISNULL(SUM(CONVERT(bigint, CAST(SUBSTRING(t.d, 5, 4) AS int))), 0) AS h2, \
         ISNULL(SUM(CONVERT(bigint, CAST(SUBSTRING(t.d, 9, 4) AS int))), 0) AS h3 \
         FROM (SELECT s.DataSize AS DataSize, HASHBYTES('SHA2_256', CONCAT(s.FileName, N'|', s.PartNo, N'|', s.DataSize, N'|', \
         CONVERT(nvarchar(64), HASHBYTES('SHA2_256', s.BinaryData), 2))) AS d FROM {source}) t"
    )
}

/// The source of the staged rows.
pub fn staged_source(database: &str) -> Result<String> {
    Ok(format!("{}.dbo.ConfigSave s", quote_ident(database)?))
}

/// The `Config` rows a staged name replaces.
pub fn replaced_source(database: &str) -> Result<String> {
    let db = quote_ident(database)?;
    Ok(format!(
        "{db}.dbo.Config s WHERE EXISTS (SELECT 1 FROM {db}.dbo.ConfigSave x WHERE x.FileName = s.FileName)"
    ))
}

/// `LIKE` pattern of a dynamic alias row, with `!` as the escape character
/// (so that no backslash has to survive any quoting layer).
pub const ALIAS_PATTERN: &str = "N'%!_dynupdate!_%' ESCAPE N'!'";

/// The dynamic-update leftovers of `Config`: the marker and every alias.
pub fn special_config_source(database: &str) -> Result<String> {
    let db = quote_ident(database)?;
    Ok(format!(
        "{db}.dbo.Config s WHERE s.FileName = N'DynamicallyUpdated' OR s.FileName LIKE {ALIAS_PATTERN}"
    ))
}

/// The `Config` rows a stage's `deleted` list removes (the rows of removed forms and templates).
pub fn removed_source(database: &str, names: &[String]) -> Result<String> {
    let db = quote_ident(database)?;
    Ok(format!(
        "{db}.dbo.Config s WHERE s.FileName IN ({})",
        name_list(names)
    ))
}

/// `N'a', N'b'`: names as a SQL list. An empty list is `N''`, which matches no row name.
fn name_list(names: &[String]) -> String {
    if names.is_empty() {
        return "N''".to_owned();
    }
    names
        .iter()
        .map(|name| format!("N'{}'", quote_string(name)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The `Params` marker row, when present.
pub fn special_params_source(database: &str) -> Result<String> {
    let db = quote_ident(database)?;
    Ok(format!(
        "{db}.dbo.Params s WHERE s.FileName = N'DynamicallyUpdated'"
    ))
}

/// A `Files` row the script rewrites, with the digest the plan saw.
#[derive(Debug, Clone)]
pub struct FilesRewrite {
    pub file_name: String,
    pub old_data_size: i64,
    pub old_sha256_hex: String,
    pub new_bytes: Vec<u8>,
}

/// A `Params` row the script rewrites (the search information of new objects).
#[derive(Debug, Clone)]
pub struct ParamsRewrite {
    pub file_name: String,
    pub old_data_size: i64,
    pub old_sha256_hex: String,
    pub new_bytes: Vec<u8>,
    /// The platform writes the search-information rows anew (`Creation` moves
    /// too); `siVersions` only changes `Modified`.
    pub set_creation: bool,
}

/// A new object to register for every node, with the files it owns in the
/// order they are listed.
#[derive(Debug, Clone)]
pub struct NewRegistration {
    /// `_MDObjID`: the uuid in the platform's byte order, 32 hex digits.
    pub object_hex: String,
    pub files: Vec<String>,
}

/// A file an existing, registered object gains.
#[derive(Debug, Clone)]
pub struct AppendedFile {
    pub object_hex: String,
    pub file_name: String,
}

/// An object a change register row is inserted for at the nodes that have none, with the list of the changed
/// files the row gets (empty when only the object's descriptor is staged).
#[derive(Debug, Clone)]
pub struct ObjectRegistration {
    /// `_MDObjID`, 32 hex digits.
    pub object_hex: String,
    pub files: Vec<String>,
}

/// An exchange-plan node new objects are registered for.
#[derive(Debug, Clone)]
pub struct NodeLiteral {
    /// The exchange plan's number: `_Node<plan>` is its node table.
    pub plan: i64,
    /// `_NodeTRef`, 8 hex digits.
    pub type_hex: String,
    /// `_NodeRRef`, 32 hex digits.
    pub reference_hex: String,
}

/// Everything the script is rendered from.
#[derive(Debug, Clone, Default)]
pub struct ScriptInputs {
    pub(crate) source_preimages: Option<super::dynamic::SourceOwnerPreimages>,
    pub database: String,
    /// The client process: its own sessions do not count as "other".
    pub client_pid: u32,
    /// End with `ROLLBACK` instead of `COMMIT` (a rehearsal).
    pub rehearse: bool,
    /// Require that no other user session is connected to the database.
    pub require_exclusive: bool,
    /// A condition to append to the session count (`" AND NOT (...)"`, or empty): the idle `1CV83 Server` sessions of
    /// the worker processes that the tool's own RAS verification opened (#409 F-3, #408).
    pub session_exemption: String,
    pub staged: Fingerprint,
    pub replaced: Fingerprint,
    pub special_config: Fingerprint,
    pub special_params: Fingerprint,
    /// Delete the `Params` marker for an admitted genuine descriptor change.
    /// On 8.3.27.2214 a decoded-equal descriptor companion to changed bodies
    /// retains it (native S/R5); other profiles retain their prior policy.
    pub clear_params_marker: bool,
    /// Exact preimages used by the semantic marker decision. Rendered only by
    /// the bound descriptor planner, under the transaction's table locks.
    pub params_marker_guard_sql: String,
    /// Dynamic generations to fold into the ordinary rows, oldest first.
    pub generations: Vec<Uuid>,
    /// `_ConfigChngR` exists: reset the change registrations of the staged
    /// descriptors.
    pub reset_change_registrations: bool,
    pub files_rewrites: Vec<FilesRewrite>,
    pub params_rewrites: Vec<ParamsRewrite>,
    /// New objects to register (needs `reset_change_registrations`).
    pub new_registrations: Vec<NewRegistration>,
    /// The nodes new objects are registered for.
    pub nodes: Vec<NodeLiteral>,
    /// How many distinct nodes `_ConfigChngR` holds now (the plan's view of
    /// the nodes, asserted again under the lock).
    pub nodes_seen: usize,
    /// Changed objects that miss a row at some of `nodes`: the rows are inserted, `_MessageNo` NULL, with
    /// the changed files as their list (the native apply does this for a node with an initial image, which
    /// has no rows). `nodes` must hold every node of the plans that register changes.
    pub registration_additions: Vec<ObjectRegistration>,
    /// How many `_ConfigChngR` rows and how many `_ConfigChngR_ExtProps` rows that inserts.
    pub registration_rows_expected: i64,
    pub registration_file_rows_expected: i64,
    /// (plan number, nodes of it that are not the plan's own), asserted again under the lock.
    pub plan_node_counts: Vec<(i64, i64)>,
    /// Owners of the rows a `deleted` list names (`_MDObjID` hex): their rows are reset as well.
    pub extra_changed_objects: Vec<String>,
    pub appended_files: Vec<AppendedFile>,
    /// Dynamic-only exact eligible existing registrations. None preserves the exclusive path;
    /// Some(empty) appends nothing, even if registrations appear later.
    pub appended_registration_ids: Option<Vec<String>>,
    /// Staged rows that are consumed, not moved (an empty or dynamic-only
    /// `deleted` list): their names, and how many `ConfigSave` rows they are.
    pub consumed_names: Vec<String>,
    pub consumed_row_count: i64,
    /// Dynamic-update rows the stage's `deleted` list names: deleted outright,
    /// not folded (the native apply deletes what the list names). Empty: the
    /// generations, if any, are folded.
    pub dropped_rows: Vec<String>,
    /// The `Config` rows of the forms and templates the stage's `deleted` list removes, and the digest
    /// of them the plan saw. Deleted outright; the stage holds no row of these names.
    pub removed_rows: Vec<String>,
    pub removed: Fingerprint,
    /// The structure phase of a restructuring the gate let through (T-SQL, see
    /// `gate::StructurePhase`): run after the assertions and `@now`, before the fold and the move.
    pub structure_sql: Option<String>,
}

/// The consumed names as a SQL list (`N'a', N'b'`); `None` when nothing is
/// consumed.
fn consumed_list(input: &ScriptInputs) -> Option<String> {
    if input.consumed_names.is_empty() {
        return None;
    }
    Some(
        input
            .consumed_names
            .iter()
            .map(|name| format!("N'{}'", quote_string(name)))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

/// ` WHERE <column> NOT IN (<consumed>)`, or nothing.
fn where_not_consumed(input: &ScriptInputs, column: &str) -> String {
    consumed_list(input)
        .map(|names| format!(" WHERE {column} NOT IN ({names})"))
        .unwrap_or_default()
}

/// ` AND <column> NOT IN (<consumed>)`, or nothing.
fn and_not_consumed(input: &ScriptInputs, column: &str) -> String {
    consumed_list(input)
        .map(|names| format!(" AND {column} NOT IN ({names})"))
        .unwrap_or_default()
}

/// The `_ConfigChngR` rows (alias `r`) of every object that owns a staged
/// row: the object whose uuid a staged name starts with, and the object whose
/// file list names a staged row. `prefix` qualifies the tables (`dbo.` inside
/// the script, `[db].dbo.` outside it).
pub fn staged_objects_predicate(prefix: &str) -> String {
    staged_objects_predicate_with(prefix, &[])
}

/// [`staged_objects_predicate`] and the objects a `deleted` list names rows of (`extra`, `_MDObjID` hex).
pub fn staged_objects_predicate_with(prefix: &str, extra: &[String]) -> String {
    let extra_clause = if extra.is_empty() {
        String::new()
    } else {
        format!(
            " OR r._MDObjID IN ({})",
            extra
                .iter()
                .map(|object| format!("0x{object}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    format!(
        "(r._MDObjID IN (SELECT CAST(TRY_CAST(LEFT(s.FileName, 36) AS uniqueidentifier) AS binary(16)) FROM {prefix}ConfigSave s WHERE s.PartNo = 0 AND TRY_CAST(LEFT(s.FileName, 36) AS uniqueidentifier) IS NOT NULL AND (LEN(s.FileName) = 36 OR SUBSTRING(s.FileName, 37, 1) = N'.'))          OR EXISTS (SELECT 1 FROM {prefix}_ConfigChngR_ExtProps e JOIN {prefix}ConfigSave s ON s.FileName = e._FileName WHERE e._ConfigChngR_IDRRef = r._IDRRef){extra_clause})"
    )
}

fn throw(out: &mut String, condition: &str, code: u32, message: &str) {
    writeln!(
        out,
        "IF {condition} THROW {code}, N'{}', 1;",
        quote_string(message)
    )
    .unwrap();
}

fn assert_fingerprint(
    out: &mut String,
    label: &str,
    source: &str,
    want: &Fingerprint,
    code: u32,
    message: &str,
) {
    writeln!(
        out,
        "SELECT @n = q.n, @b = q.bytes, @h1 = q.h1, @h2 = q.h2, @h3 = q.h3 FROM ({}) q;",
        fingerprint_select(source)
    )
    .unwrap();
    throw(
        out,
        &format!(
            "@n <> {} OR @b <> {} OR @h1 <> {} OR @h2 <> {} OR @h3 <> {}",
            want.rows, want.bytes, want.h1, want.h2, want.h3
        ),
        code,
        &format!("{message} ({label})"),
    );
}

/// The whole apply as one T-SQL batch: one transaction that either leaves the
/// database exactly as it was or moves every staged row into `Config`.
pub fn render_apply_script(input: &ScriptInputs) -> Result<String> {
    let db = quote_ident(&input.database)?;
    let mut sql = String::new();
    writeln!(sql, "SET NOCOUNT ON;").unwrap();
    writeln!(sql, "SET XACT_ABORT ON;").unwrap();
    writeln!(sql, "SET LOCK_TIMEOUT 30000;").unwrap();
    writeln!(sql, "USE {db};").unwrap();
    writeln!(sql, "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;").unwrap();
    writeln!(sql, "BEGIN TRY").unwrap();
    writeln!(sql, "BEGIN TRANSACTION;").unwrap();
    writeln!(sql, "DECLARE @r int, @touch nvarchar(256), @n bigint, @b bigint, @h1 bigint, @h2 bigint, @h3 bigint;").unwrap();
    writeln!(
        sql,
        "EXEC @r = sys.sp_getapplock @Resource = N'ibcmd-rs:config-apply', @LockMode = 'Exclusive', @LockOwner = 'Transaction', @LockTimeout = 0;"
    )
    .unwrap();
    throw(
        &mut sql,
        "@r < 0",
        code::LOCK_BUSY,
        "the config apply application lock is busy",
    );

    // Exclusive table locks first, so the checks below see a frozen state.
    for table in ["Config", "ConfigSave", "Params", "Files"] {
        writeln!(
            sql,
            "SELECT TOP (1) @touch = FileName FROM dbo.{table} WITH (TABLOCKX, HOLDLOCK) ORDER BY FileName;"
        )
        .unwrap();
    }
    if let Some(original) = &input.source_preimages {
        sql.push_str(&original.precondition_sql(&input.database)?);
    }
    if input.reset_change_registrations {
        writeln!(
            sql,
            "SELECT TOP (1) @touch = NULL FROM dbo._ConfigChngR WITH (TABLOCKX, HOLDLOCK);"
        )
        .unwrap();
    }

    // Exclusive access: no other user session on the database.
    if input.require_exclusive {
        throw(
            &mut sql,
            "HAS_PERMS_BY_NAME(NULL, NULL, N'VIEW SERVER STATE') <> 1",
            code::NO_VIEW_SERVER_STATE,
            "exclusive access cannot be proven without VIEW SERVER STATE",
        );
        throw(
            &mut sql,
            &format!(
                "EXISTS (SELECT 1 FROM sys.dm_exec_sessions WHERE is_user_process = 1 AND session_id <> @@SPID AND database_id = DB_ID() AND ISNULL(host_process_id, -1) <> {}{})",
                input.client_pid, input.session_exemption
            ),
            code::OTHER_SESSIONS,
            "another session is connected to the database; the apply needs exclusive access",
        );
    }

    // The unfinished-operation markers must be absent. A staged row the plan
    // consumes (the `deleted` list) is not one: in `ConfigSave` it is input.
    let names_of = |skip: &[String]| {
        UNFINISHED_NAMES
            .iter()
            .filter(|name| {
                !skip
                    .iter()
                    .any(|skipped| skipped.eq_ignore_ascii_case(name))
            })
            .map(|name| format!("N'{}'", quote_string(name)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let unfinished = names_of(&[]);
    let unfinished_in_save = names_of(&input.consumed_names);
    throw(
        &mut sql,
        &format!(
            "EXISTS (SELECT 1 FROM dbo.Config WHERE FileName IN ({unfinished}) OR FileName LIKE N'%.new') OR EXISTS (SELECT 1 FROM dbo.ConfigSave WHERE FileName IN ({unfinished_in_save}) OR FileName LIKE N'%.new')"
        ),
        code::UNFINISHED_OPERATION,
        "an unfinished operation is recorded in Config or ConfigSave; run the native config repair first",
    );
    // A restructuring in flight leaves the schema storage in another state than 100.
    throw(
        &mut sql,
        "EXISTS (SELECT 1 FROM dbo.SchemaStorage WHERE Status <> 100)",
        code::UNFINISHED_SCHEMA,
        "SchemaStorage is not in the settled state (Status 100): an interrupted restructuring; run the native config repair first",
    );

    // What the plan saw must still be there.
    assert_fingerprint(
        &mut sql,
        "ConfigSave",
        &staged_source_local(),
        &input.staged,
        code::STAGE_DRIFTED,
        "ConfigSave changed since the plan was made",
    );
    assert_fingerprint(
        &mut sql,
        "Config",
        &replaced_source_local(),
        &input.replaced,
        code::ACTIVE_DRIFTED,
        "the Config rows to replace changed since the plan was made",
    );
    assert_fingerprint(
        &mut sql,
        "Config markers",
        &special_config_local(),
        &input.special_config,
        code::SPECIAL_DRIFTED,
        "the dynamic-update rows of Config changed since the plan was made",
    );
    assert_fingerprint(
        &mut sql,
        "Params marker",
        &special_params_local(),
        &input.special_params,
        code::SPECIAL_DRIFTED,
        "Params.DynamicallyUpdated changed since the plan was made",
    );
    if !input.removed_rows.is_empty() {
        assert_fingerprint(
            &mut sql,
            "Config removed rows",
            &removed_source_local(&input.removed_rows),
            &input.removed,
            code::REMOVED_DRIFTED,
            "the Config rows the stage's deleted list removes changed since the plan was made",
        );
    }

    // Deciding marker/descriptor/history headers are frozen before any fold,
    // publication, marker cleanup, or ConfigSave consumption (including no-op).
    sql.push_str(&input.params_marker_guard_sql);
    render_timestamps(&mut sql);

    // A restructuring the gate let through: the tables are rebuilt and the schema published inside
    // this transaction, so a failed assertion below rolls them back too.
    if let Some(structure) = &input.structure_sql {
        writeln!(sql, "-- structure phase").unwrap();
        sql.push_str(structure);
        if !structure.ends_with('\n') {
            sql.push('\n');
        }
    }

    // The dynamic rows a `deleted` list names are deleted, and the ordinary rows
    // stay as they are (measured: the native apply keeps the text from before
    // the online update when the list names the alias). Otherwise the dynamic
    // generations are folded into the ordinary rows, oldest first: the alias
    // row of an object replaces its ordinary row.
    if !input.dropped_rows.is_empty() {
        let names = input
            .dropped_rows
            .iter()
            .map(|name| format!("N'{}'", quote_string(name)))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(sql, "DELETE FROM dbo.Config WHERE FileName IN ({names});").unwrap();
        throw(
            &mut sql,
            &format!("EXISTS (SELECT 1 FROM dbo.Config WHERE FileName IN ({names}))"),
            code::ALIAS_LEFT,
            "a dynamic-update row that the stage's deleted list names is left",
        );
    } else if !input.generations.is_empty() {
        for generation in &input.generations {
            let g = generation.hyphenated().to_string();
            // `<x>_dynupdate_<g>[.<suffix>]` -> `<x>[.<suffix>]`: cut the marker
            // (11 characters) and the generation (36).
            let target = "LEFT(FileName, CHARINDEX(N'_dynupdate_', FileName) - 1) + SUBSTRING(FileName, CHARINDEX(N'_dynupdate_', FileName) + 47, 4000)";
            let filter = format!(
                "FileName LIKE N'%!_dynupdate!_{g}%' ESCAPE N'!' AND FileName NOT LIKE N'versions!_dynupdate!_%' ESCAPE N'!' AND FileName NOT LIKE N'deleted!_dynupdate!_%' ESCAPE N'!'"
            );
            writeln!(sql, "DELETE FROM dbo.Config WHERE FileName IN (SELECT {target} FROM dbo.Config WHERE {filter});").unwrap();
            writeln!(
                sql,
                "UPDATE dbo.Config SET FileName = {target} WHERE {filter};"
            )
            .unwrap();
            writeln!(
                sql,
                "DELETE FROM dbo.Config WHERE FileName = N'versions_dynupdate_{g}';"
            )
            .unwrap();
        }
        throw(
            &mut sql,
            &format!(
                "EXISTS (SELECT 1 FROM dbo.Config WHERE FileName LIKE {ALIAS_PATTERN} AND {})",
                generation_filter(&input.generations)
            ),
            code::ALIAS_LEFT,
            "a dynamic alias row is left after the fold",
        );
    }
    let clear_params = input.clear_params_marker && input.special_params.rows > 0;
    if input.special_config.rows > 0 || clear_params {
        writeln!(
            sql,
            "DELETE FROM dbo.Config WHERE FileName = N'DynamicallyUpdated';"
        )
        .unwrap();
        let mut left =
            "EXISTS (SELECT 1 FROM dbo.Config WHERE FileName = N'DynamicallyUpdated')".to_owned();
        if clear_params {
            writeln!(
                sql,
                "DELETE FROM dbo.Params WHERE FileName = N'DynamicallyUpdated';"
            )
            .unwrap();
            left.push_str(
                " OR EXISTS (SELECT 1 FROM dbo.Params WHERE FileName = N'DynamicallyUpdated')",
            );
        }
        throw(
            &mut sql,
            &left,
            code::MARKER_CLEANUP,
            "DynamicallyUpdated is left after the cleanup",
        );
    }

    // The rows of the removed forms and templates go (every part of each). The stage holds no row of
    // these names, and the fold above has not made one.
    if !input.removed_rows.is_empty() {
        let names = name_list(&input.removed_rows);
        writeln!(sql, "DELETE FROM dbo.Config WHERE FileName IN ({names});").unwrap();
        throw(
            &mut sql,
            &format!("@@ROWCOUNT <> {}", input.removed.rows),
            code::REMOVE_COUNT,
            "the number of Config rows removed differs from the plan's",
        );
        throw(
            &mut sql,
            &format!("EXISTS (SELECT 1 FROM dbo.Config WHERE FileName IN ({names}))"),
            code::REMOVE_COUNT,
            "a row of a removed form or template is left in Config",
        );
    }

    // The move: the staged rows replace every part of the rows they name.
    writeln!(
        sql,
        "DELETE FROM dbo.Config WHERE FileName IN (SELECT FileName FROM dbo.ConfigSave{});",
        where_not_consumed(input, "FileName")
    )
    .unwrap();
    writeln!(
        sql,
        "INSERT dbo.Config (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo) SELECT FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo FROM dbo.ConfigSave{};",
        where_not_consumed(input, "FileName")
    )
    .unwrap();
    throw(
        &mut sql,
        &format!(
            "@@ROWCOUNT <> {}",
            input.staged.rows - input.consumed_row_count
        ),
        code::REPLACE_COUNT,
        "the number of rows moved into Config differs from the staged count",
    );

    // Change registrations: every object that owns a staged row is changed for
    // every node again, and new objects are registered.
    render_change_registrations(&mut sql, input);
    render_files_rewrites(&mut sql, input);
    for rewrite in &input.params_rewrites {
        let name = quote_string(&rewrite.file_name);
        throw(
            &mut sql,
            &format!(
                "(SELECT COUNT_BIG(*) FROM dbo.Params WHERE FileName = N'{name}' AND PartNo = 0 AND CONVERT(bigint, DataSize) = {} AND HASHBYTES('SHA2_256', BinaryData) = 0x{}) <> 1",
                rewrite.old_data_size, rewrite.old_sha256_hex
            ),
            code::FILES_DRIFTED,
            &format!(
                "Params.{} changed since the plan was made",
                rewrite.file_name
            ),
        );
        writeln!(
            sql,
            "DELETE FROM dbo.Params WHERE FileName = N'{name}' AND PartNo <> 0;"
        )
        .unwrap();
        let creation = if rewrite.set_creation {
            "Creation = @now, "
        } else {
            ""
        };
        writeln!(
            sql,
            "UPDATE dbo.Params SET {creation}Modified = @now, DataSize = {}, BinaryData = 0x{} WHERE FileName = N'{name}' AND PartNo = 0;",
            rewrite.new_bytes.len(),
            hex_upper(&rewrite.new_bytes)
        )
        .unwrap();
        throw(
            &mut sql,
            "@@ROWCOUNT <> 1",
            code::PARAMS_WRITE,
            &format!("Params.{} was not rewritten", rewrite.file_name),
        );
    }

    // Postcondition: every staged row is in Config, byte for byte, and no
    // other part of it is.
    throw(
        &mut sql,
        &format!(
            "EXISTS (SELECT 1 FROM dbo.ConfigSave s WHERE NOT EXISTS (SELECT 1 FROM dbo.Config c WHERE c.FileName = s.FileName AND c.PartNo = s.PartNo AND c.DataSize = s.DataSize AND c.Creation = s.Creation AND c.Modified = s.Modified AND c.Attributes = s.Attributes AND c.BinaryData = s.BinaryData){})",
            and_not_consumed(input, "s.FileName")
        ),
        code::POSTCONDITION,
        "a staged row is missing or differs in Config after the move",
    );
    throw(
        &mut sql,
        &format!(
            "EXISTS (SELECT 1 FROM dbo.Config c WHERE EXISTS (SELECT 1 FROM dbo.ConfigSave s WHERE s.FileName = c.FileName{}) AND NOT EXISTS (SELECT 1 FROM dbo.ConfigSave s WHERE s.FileName = c.FileName AND s.PartNo = c.PartNo))",
            and_not_consumed(input, "s.FileName")
        ),
        code::POSTCONDITION,
        "Config keeps a part the staged row does not have",
    );
    // A consumed row must not have reached Config.
    if let Some(names) = consumed_list(input) {
        throw(
            &mut sql,
            &format!("EXISTS (SELECT 1 FROM dbo.Config WHERE FileName IN ({names}))"),
            code::POSTCONDITION,
            "a consumed staged row reached Config",
        );
    }

    // ConfigSave is consumed.
    writeln!(sql, "DELETE FROM dbo.ConfigSave;").unwrap();
    throw(
        &mut sql,
        &format!("@@ROWCOUNT <> {}", input.staged.rows),
        code::CLEANUP,
        "ConfigSave cleanup removed an unexpected number of rows",
    );
    throw(
        &mut sql,
        "EXISTS (SELECT 1 FROM dbo.ConfigSave)",
        code::CLEANUP,
        "ConfigSave is not empty after the cleanup",
    );

    if input.rehearse {
        writeln!(sql, "ROLLBACK TRANSACTION;").unwrap();
    } else {
        writeln!(sql, "COMMIT TRANSACTION;").unwrap();
    }
    writeln!(sql, "END TRY").unwrap();
    writeln!(sql, "BEGIN CATCH").unwrap();
    writeln!(sql, "IF XACT_STATE() <> 0 ROLLBACK TRANSACTION;").unwrap();
    writeln!(sql, "THROW;").unwrap();
    writeln!(sql, "END CATCH;").unwrap();
    Ok(sql)
}

/// The declarations of `@offset` and `@now`: timestamps as the platform writes them, local time
/// shifted by the infobase's year offset.
pub fn timestamp_declarations() -> String {
    let mut sql = String::new();
    render_timestamps(&mut sql);
    sql
}

/// Timestamps as the platform writes them: local time, shifted by the infobase's year offset
/// (`@offset`, `@now`).
fn render_timestamps(sql: &mut String) {
    writeln!(
        sql,
        "DECLARE @offset int = ISNULL((SELECT TOP (1) Offset FROM dbo._YearOffset), 0);"
    )
    .unwrap();
    writeln!(
        sql,
        "DECLARE @now datetime2(6) = DATEADD(year, @offset, CONVERT(datetime2(6), SYSDATETIME()));"
    )
    .unwrap();
}

/// The change registrations of the objects that own a staged row: every node is told again
/// (`_MessageNo` NULL), a node with no row of a changed object gets one (#412), new objects are
/// registered. Run while `ConfigSave` still holds the staged rows.
fn render_change_registrations(sql: &mut String, input: &ScriptInputs) {
    if !input.reset_change_registrations {
        return;
    }
    let predicate = staged_objects_predicate_with("dbo.", &input.extra_changed_objects);
    writeln!(
        sql,
        "UPDATE r SET _MessageNo = NULL FROM dbo._ConfigChngR r WHERE r._MessageNo IS NOT NULL AND {predicate};"
    )
    .unwrap();
    throw(
        sql,
        &format!(
            "EXISTS (SELECT 1 FROM dbo._ConfigChngR r WHERE r._MessageNo IS NOT NULL AND {predicate})"
        ),
        code::CHANGE_REGISTRATION,
        "a change registration of a staged object was not reset",
    );
    // New objects first: their check of the nodes counts the nodes the register holds, which the
    // additions below change.
    render_new_registrations(sql, input);
    render_registration_additions(sql, input);
}

/// The `Files` rows the apply rewrites (`MobileVersions.dat`: a fresh GUID at the head), guarded by
/// the digest the plan saw. Needs `@now`.
fn render_files_rewrites(sql: &mut String, input: &ScriptInputs) {
    for rewrite in &input.files_rewrites {
        let name = quote_string(&rewrite.file_name);
        throw(
            sql,
            &format!(
                "(SELECT COUNT_BIG(*) FROM dbo.Files WHERE FileName = N'{name}' AND PartNo = 0 AND CONVERT(bigint, DataSize) = {} AND HASHBYTES('SHA2_256', BinaryData) = 0x{}) <> 1",
                rewrite.old_data_size, rewrite.old_sha256_hex
            ),
            code::FILES_DRIFTED,
            &format!(
                "Files.{} changed since the plan was made",
                rewrite.file_name
            ),
        );
        writeln!(
            sql,
            "DELETE FROM dbo.Files WHERE FileName = N'{name}' AND PartNo <> 0;"
        )
        .unwrap();
        writeln!(
            sql,
            "UPDATE dbo.Files SET Creation = @now, Modified = @now, DataSize = {}, BinaryData = 0x{} WHERE FileName = N'{name}' AND PartNo = 0;",
            rewrite.new_bytes.len(),
            hex_upper(&rewrite.new_bytes)
        )
        .unwrap();
        throw(
            sql,
            "@@ROWCOUNT <> 1",
            code::FILES_WRITE,
            &format!("Files.{} was not rewritten", rewrite.file_name),
        );
    }
}

/// The per-apply writes every apply of a stage makes besides moving its rows -- the change
/// registrations of the changed objects and `Files.MobileVersions.dat` -- as a fragment of a
/// transaction that is open: the exclusive apply renders the same text into its script
/// ([`render_apply_script`]), and the dynamic apply hands it to the online transition
/// (`mssql_main_activation::MainActivationPlan::with_parity_sql`). It reads `ConfigSave`, so it
/// runs before the stage is consumed, and needs `@now` declared ([`timestamp_declarations`]).
///
/// Only what the inputs carry is rendered (`reset_change_registrations`, the registration
/// additions, `files_rewrites`); the rest of [`ScriptInputs`] is the exclusive script's.
pub fn render_parity_writes(input: &ScriptInputs) -> String {
    let mut sql = String::new();
    render_change_registrations(&mut sql, input);
    render_files_rewrites(&mut sql, input);
    sql
}

/// The rows a node with no rows gets for the objects the stage changes: inserted with `_MessageNo` NULL,
/// the changed files listed. The pairs are the plan's; the script asserts the nodes and that exactly the
/// planned rows are missing before it inserts them.
fn render_registration_additions(sql: &mut String, input: &ScriptInputs) {
    if input.registration_additions.is_empty() || input.nodes.is_empty() {
        return;
    }
    for (plan, count) in &input.plan_node_counts {
        throw(
            sql,
            &format!(
                "(SELECT COUNT_BIG(*) FROM dbo._Node{plan} WHERE _PredefinedID = 0x00000000000000000000000000000000) <> {count}"
            ),
            code::NODES_DRIFTED,
            "the nodes of an exchange plan changed since the plan was made",
        );
    }
    writeln!(
        sql,
        "CREATE TABLE #reg_nodes (t binary(4) NOT NULL, n binary(16) NOT NULL);"
    )
    .unwrap();
    let nodes = input
        .nodes
        .iter()
        .map(|node| format!("(0x{}, 0x{})", node.type_hex, node.reference_hex))
        .collect::<Vec<_>>();
    for chunk in nodes.chunks(500) {
        writeln!(sql, "INSERT #reg_nodes (t, n) VALUES {};", chunk.join(", ")).unwrap();
    }
    writeln!(
        sql,
        "CREATE TABLE #reg_objs (o binary(16) NOT NULL PRIMARY KEY);"
    )
    .unwrap();
    let objects = input
        .registration_additions
        .iter()
        .map(|object| format!("(0x{})", object.object_hex))
        .collect::<Vec<_>>();
    for chunk in objects.chunks(500) {
        writeln!(sql, "INSERT #reg_objs (o) VALUES {};", chunk.join(", ")).unwrap();
    }
    writeln!(
        sql,
        "CREATE TABLE #reg_files (o binary(16) NOT NULL, k int NOT NULL, f nvarchar(400) NOT NULL);"
    )
    .unwrap();
    let mut files = Vec::new();
    for object in &input.registration_additions {
        for (index, file) in object.files.iter().enumerate() {
            files.push(format!(
                "(0x{}, {index}, N'{}')",
                object.object_hex,
                quote_string(file)
            ));
        }
    }
    for chunk in files.chunks(500) {
        writeln!(
            sql,
            "INSERT #reg_files (o, k, f) VALUES {};",
            chunk.join(", ")
        )
        .unwrap();
    }
    writeln!(
        sql,
        "CREATE TABLE #reg_add (t binary(4) NOT NULL, n binary(16) NOT NULL, o binary(16) NOT NULL, id binary(16) NOT NULL);"
    )
    .unwrap();
    writeln!(
        sql,
        "DECLARE @reg_max binary(16) = (SELECT MAX(_IDRRef) FROM dbo._ConfigChngR);"
    )
    .unwrap();
    writeln!(
        sql,
        "DECLARE @reg_head binary(8) = SUBSTRING(@reg_max, 1, 8), @reg_tail bigint = CAST(SUBSTRING(@reg_max, 9, 8) AS bigint);"
    )
    .unwrap();
    throw(
        sql,
        "@reg_max IS NULL OR @reg_tail > 9000000000000000000",
        code::NEW_REGISTRATION,
        "no room for new change-registration ids",
    );
    writeln!(
        sql,
        "INSERT #reg_add (t, n, o, id) SELECT nd.t, nd.n, ob.o, CAST(@reg_head + CAST(@reg_tail + ROW_NUMBER() OVER (ORDER BY nd.n, ob.o) AS binary(8)) AS binary(16)) FROM #reg_nodes nd CROSS JOIN #reg_objs ob WHERE NOT EXISTS (SELECT 1 FROM dbo._ConfigChngR x WHERE x._NodeTRef = nd.t AND x._NodeRRef = nd.n AND x._MDObjID = ob.o);"
    )
    .unwrap();
    throw(
        sql,
        &format!("@@ROWCOUNT <> {}", input.registration_rows_expected),
        code::NODES_DRIFTED,
        "the change registrations that miss a row are not the ones the plan saw",
    );
    writeln!(
        sql,
        "INSERT dbo._ConfigChngR (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID, _IDRRef) SELECT t, n, NULL, o, id FROM #reg_add;"
    )
    .unwrap();
    throw(
        sql,
        &format!("@@ROWCOUNT <> {}", input.registration_rows_expected),
        code::NEW_REGISTRATION,
        "the changed objects were not registered at every node that had no row",
    );
    if input.registration_file_rows_expected > 0 {
        writeln!(
            sql,
            "INSERT dbo._ConfigChngR_ExtProps (_ConfigChngR_IDRRef, _KeyField, _FileName) SELECT a.id, CAST(f.k AS binary(4)), f.f FROM #reg_add a JOIN #reg_files f ON f.o = a.o;"
        )
        .unwrap();
        throw(
            sql,
            &format!("@@ROWCOUNT <> {}", input.registration_file_rows_expected),
            code::NEW_REGISTRATION,
            "the changed files were not listed for the new change registrations",
        );
    }
    writeln!(
        sql,
        "DROP TABLE #reg_add; DROP TABLE #reg_files; DROP TABLE #reg_objs; DROP TABLE #reg_nodes;"
    )
    .unwrap();
}

/// New objects registered for every node, their files listed, and files an
/// existing object gains appended to its list.
fn render_new_registrations(sql: &mut String, input: &ScriptInputs) {
    if input.new_registrations.is_empty() && input.appended_files.is_empty() {
        return;
    }
    throw(
        sql,
        &format!(
            "(SELECT COUNT_BIG(*) FROM (SELECT DISTINCT _NodeTRef, _NodeRRef FROM dbo._ConfigChngR) d) <> {}",
            input.nodes_seen
        ),
        code::NODES_DRIFTED,
        "the exchange-plan nodes changed since the plan was made",
    );
    if !input.new_registrations.is_empty() && !input.nodes.is_empty() {
        let objects = input
            .new_registrations
            .iter()
            .map(|object| format!("0x{}", object.object_hex))
            .collect::<Vec<_>>()
            .join(", ");
        throw(
            sql,
            &format!("EXISTS (SELECT 1 FROM dbo._ConfigChngR WHERE _MDObjID IN ({objects}))"),
            code::ALREADY_REGISTERED,
            "a new object has change registrations already",
        );
        // Ids continue the sequence of the table: the greatest one plus one.
        writeln!(
            sql,
            "DECLARE @max binary(16) = (SELECT MAX(_IDRRef) FROM dbo._ConfigChngR);"
        )
        .unwrap();
        writeln!(
            sql,
            "DECLARE @head binary(8) = SUBSTRING(@max, 1, 8), @tail bigint = CAST(SUBSTRING(@max, 9, 8) AS bigint);"
        )
        .unwrap();
        throw(
            sql,
            "@max IS NULL OR @tail > 9000000000000000000",
            code::NEW_REGISTRATION,
            "no room for new change-registration ids",
        );
        let mut values = Vec::new();
        let mut sequence = 0usize;
        for node in &input.nodes {
            for object in &input.new_registrations {
                sequence += 1;
                values.push(format!(
                    "(0x{}, 0x{}, 0x{}, {sequence})",
                    node.type_hex, node.reference_hex, object.object_hex
                ));
            }
        }
        writeln!(
            sql,
            "INSERT dbo._ConfigChngR (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID, _IDRRef) SELECT CAST(v.t AS binary(4)), CAST(v.n AS binary(16)), NULL, CAST(v.o AS binary(16)), CAST(@head + CAST(@tail + v.k AS binary(8)) AS binary(16)) FROM (VALUES {}) AS v(t, n, o, k);",
            values.join(", ")
        )
        .unwrap();
        throw(
            sql,
            &format!("@@ROWCOUNT <> {sequence}"),
            code::NEW_REGISTRATION,
            "the new objects were not registered for every node",
        );
        let mut files = Vec::new();
        for object in &input.new_registrations {
            for (index, file) in object.files.iter().enumerate() {
                files.push(format!(
                    "(0x{}, {index}, N'{}')",
                    object.object_hex,
                    quote_string(file)
                ));
            }
        }
        if !files.is_empty() {
            writeln!(
                sql,
                "INSERT dbo._ConfigChngR_ExtProps (_ConfigChngR_IDRRef, _KeyField, _FileName) SELECT r._IDRRef, CAST(v.k AS binary(4)), v.f FROM (VALUES {}) AS v(o, k, f) JOIN dbo._ConfigChngR r ON r._MDObjID = CAST(v.o AS binary(16));",
                files.join(", ")
            )
            .unwrap();
            throw(
                sql,
                &format!("@@ROWCOUNT <> {}", files.len() * input.nodes.len()),
                code::NEW_REGISTRATION,
                "the files of the new objects were not listed for every node",
            );
        }
    }
    if !input.appended_files.is_empty() {
        let append_scope = match &input.appended_registration_ids {
            None => String::new(),
            Some(ids) if ids.is_empty() => " AND 1 = 0".to_owned(),
            Some(ids) => format!(
                " AND r._IDRRef IN ({})",
                ids.iter()
                    .map(|id| format!("0x{id}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        let values = input
            .appended_files
            .iter()
            .map(|file| {
                format!(
                    "(0x{}, N'{}')",
                    file.object_hex,
                    quote_string(&file.file_name)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            sql,
            "IF EXISTS (SELECT 1 FROM (VALUES {values}) AS v(o, f) GROUP BY v.o, v.f HAVING COUNT_BIG(*) > 1) THROW {}, 'Duplicate appended file input', 1;\n\
             IF EXISTS (SELECT 1 FROM dbo._ConfigChngR_ExtProps e WITH (UPDLOCK, HOLDLOCK) JOIN dbo._ConfigChngR r WITH (UPDLOCK, HOLDLOCK) ON r._IDRRef = e._ConfigChngR_IDRRef JOIN (VALUES {values}) AS v(o, f) ON r._MDObjID = CAST(v.o AS binary(16)) WHERE CAST(e._KeyField AS int) < 0{append_scope}) THROW {}, 'Unmeasured appended file key range', 1;\n\
             DECLARE @PendingAppendedFiles TABLE (r binary(16) NOT NULL, f nvarchar(128) NOT NULL, k bigint NOT NULL);\n\
             INSERT @PendingAppendedFiles (r, f, k) SELECT pending.r, pending.f, pending.m + ROW_NUMBER() OVER (PARTITION BY pending.r ORDER BY pending.f) FROM (\n\
               SELECT r._IDRRef AS r, v.f, CONVERT(bigint, ISNULL((SELECT MAX(CAST(e._KeyField AS int)) FROM dbo._ConfigChngR_ExtProps e WITH (UPDLOCK, HOLDLOCK) WHERE e._ConfigChngR_IDRRef = r._IDRRef), -1)) AS m\n\
               FROM (VALUES {values}) AS v(o, f) JOIN dbo._ConfigChngR r WITH (UPDLOCK, HOLDLOCK) ON r._MDObjID = CAST(v.o AS binary(16))\n\
               WHERE NOT EXISTS (SELECT 1 FROM dbo._ConfigChngR_ExtProps x WITH (UPDLOCK, HOLDLOCK) WHERE x._ConfigChngR_IDRRef = r._IDRRef AND x._FileName = v.f){append_scope}\n\
             ) AS pending;\n\
             IF EXISTS (SELECT 1 FROM @PendingAppendedFiles WHERE k > 2147483647) THROW {}, 'Appended file key overflow', 1;\n\
             INSERT dbo._ConfigChngR_ExtProps (_ConfigChngR_IDRRef, _KeyField, _FileName) SELECT r, CAST(CAST(k AS int) AS binary(4)), f FROM @PendingAppendedFiles;\n\
             IF @@ROWCOUNT <> (SELECT COUNT_BIG(*) FROM @PendingAppendedFiles) THROW {}, 'Appended file count drifted', 1;",
            code::NEW_REGISTRATION, code::NEW_REGISTRATION, code::NEW_REGISTRATION, code::NEW_REGISTRATION,
        )
        .unwrap();
        for file in &input.appended_files {
            throw(
                sql,
                &format!(
                    "EXISTS (SELECT 1 FROM dbo._ConfigChngR r WHERE r._MDObjID = 0x{}{append_scope} AND NOT EXISTS (SELECT 1 FROM dbo._ConfigChngR_ExtProps e WHERE e._ConfigChngR_IDRRef = r._IDRRef AND e._FileName = N'{}'))",
                    file.object_hex,
                    quote_string(&file.file_name)
                ),
                code::NEW_REGISTRATION,
                &format!("{} is not listed for every node", file.file_name),
            );
        }
    }
}

fn staged_source_local() -> String {
    "dbo.ConfigSave s".to_owned()
}

fn replaced_source_local() -> String {
    "dbo.Config s WHERE EXISTS (SELECT 1 FROM dbo.ConfigSave x WHERE x.FileName = s.FileName)"
        .to_owned()
}

fn special_config_local() -> String {
    format!(
        "dbo.Config s WHERE s.FileName = N'DynamicallyUpdated' OR s.FileName LIKE {ALIAS_PATTERN}"
    )
}

fn special_params_local() -> String {
    "dbo.Params s WHERE s.FileName = N'DynamicallyUpdated'".to_owned()
}

fn removed_source_local(names: &[String]) -> String {
    format!("dbo.Config s WHERE s.FileName IN ({})", name_list(names))
}

fn generation_filter(generations: &[Uuid]) -> String {
    let parts = generations
        .iter()
        .map(|generation| {
            format!(
                "FileName LIKE N'%!_dynupdate!_{}%' ESCAPE N'!'",
                generation.hyphenated()
            )
        })
        .collect::<Vec<_>>();
    format!("({})", parts.join(" OR "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> ScriptInputs {
        ScriptInputs {
            source_preimages: None,
            database: "db]x".to_owned(),
            client_pid: 4242,
            rehearse: false,
            require_exclusive: true,
            session_exemption: String::new(),
            staged: Fingerprint {
                rows: 3,
                bytes: 300,
                h1: 1,
                h2: 2,
                h3: 3,
            },
            replaced: Fingerprint {
                rows: 2,
                bytes: 200,
                h1: 4,
                h2: 5,
                h3: 6,
            },
            special_config: Fingerprint::default(),
            special_params: Fingerprint::default(),
            clear_params_marker: true,
            params_marker_guard_sql: String::new(),
            generations: Vec::new(),
            reset_change_registrations: true,
            files_rewrites: Vec::new(),
            params_rewrites: Vec::new(),
            new_registrations: Vec::new(),
            nodes: Vec::new(),
            nodes_seen: 0,
            registration_additions: Vec::new(),
            registration_rows_expected: 0,
            registration_file_rows_expected: 0,
            plan_node_counts: Vec::new(),
            extra_changed_objects: Vec::new(),
            appended_files: Vec::new(),
            appended_registration_ids: None,
            consumed_names: Vec::new(),
            consumed_row_count: 0,
            dropped_rows: Vec::new(),
            removed_rows: Vec::new(),
            removed: Fingerprint::default(),
            structure_sql: None,
        }
    }

    #[test]
    fn exclusive_source_initial_preimages_are_checked_under_locks_before_publication() {
        let mut input = inputs();
        input.source_preimages = Some(super::super::dynamic::SourceOwnerPreimages::test_fixture(
            &input.database,
        ));
        let sql = render_apply_script(&input).unwrap();
        let locks = sql
            .find("FROM dbo.Files WITH (TABLOCKX, HOLDLOCK)")
            .unwrap();
        let initial = sql.find("-- initial source dependency preimages").unwrap();
        let publication = sql.find("INSERT dbo.Config").unwrap();
        assert!(locks < initial && initial < publication);
        assert!(sql[initial..publication].contains("HASHBYTES('SHA2_256', BinaryData)"));
    }

    #[test]
    fn a_structure_phase_runs_inside_the_transaction_between_the_assertions_and_the_move() {
        let mut with = inputs();
        with.structure_sql = Some("-- restructure marker\nSELECT 1;".to_owned());
        let sql = render_apply_script(&with).unwrap();
        let begin = sql.find("BEGIN TRANSACTION;").unwrap();
        let drift = sql.find("Params.DynamicallyUpdated changed since").unwrap();
        let now = sql.find("DECLARE @now datetime2(6)").unwrap();
        let phase = sql.find("-- restructure marker").unwrap();
        let fold_or_move = sql.find("INSERT dbo.Config").unwrap();
        let commit = sql.find("COMMIT TRANSACTION;").unwrap();
        assert!(
            begin < drift
                && drift < now
                && now < phase
                && phase < fold_or_move
                && fold_or_move < commit
        );
        // ahead of the fold of a dynamic generation, too
        let mut folding = with;
        folding.generations =
            vec![Uuid::parse_str("719baa18-69ed-439a-8962-1de53d98e05e").unwrap()];
        let folded = render_apply_script(&folding).unwrap();
        assert!(
            folded.find("-- restructure marker").unwrap()
                < folded
                    .find("UPDATE dbo.Config SET FileName = LEFT(FileName")
                    .unwrap()
        );
        // without a phase the script is what it was
        assert!(
            !render_apply_script(&inputs())
                .unwrap()
                .contains("-- structure phase")
        );
    }

    #[test]
    fn the_exclusivity_check_leaves_out_the_idle_sessions_of_the_own_ras_processes() {
        // #408 step 2: the old activation commands hand their exclusive mode to this apply, and the tool's own RAS
        // verification made the cluster open idle SQL sessions on the database (#409 F-3).
        let plain = render_apply_script(&inputs()).unwrap();
        assert!(
            !plain.contains("1CV83 Server"),
            "no own process, no exemption"
        );
        let mut own = inputs();
        own.session_exemption = crate::mssql_platform_profile::session_exemption(&[
            crate::mssql_platform_profile::OwnRasProcess {
                host: "wks".to_owned(),
                pid: 4711,
            },
        ]);
        let sql = render_apply_script(&own).unwrap();
        let throw = sql.find("THROW 57302").unwrap();
        let condition = &sql[..throw];
        let start = condition
            .rfind("EXISTS (SELECT 1 FROM sys.dm_exec_sessions")
            .unwrap();
        let condition = &condition[start..];
        assert!(condition.contains("ISNULL(host_process_id, -1) <> 4242"));
        assert!(condition.contains("program_name,N'')=N'1CV83 Server'"));
        assert!(condition.contains("status=N'sleeping'"));
        assert!(condition.contains("open_transaction_count=0"));
        assert!(condition.contains("N'wks' AND ISNULL(host_process_id,-1) IN (4711)"));
    }

    #[test]
    fn the_script_is_one_guarded_transaction() {
        let sql = render_apply_script(&inputs()).unwrap();
        assert!(sql.contains("USE [db]]x];"));
        let begin = sql.find("BEGIN TRANSACTION;").unwrap();
        let commit = sql.find("COMMIT TRANSACTION;").unwrap();
        let mv = sql.find("INSERT dbo.Config").unwrap();
        let cleanup = sql.find("DELETE FROM dbo.ConfigSave;").unwrap();
        assert!(begin < mv && mv < cleanup && cleanup < commit);
        assert!(sql.contains("THROW 57302"), "exclusive access is asserted");
        assert!(sql.contains("ISNULL(host_process_id, -1) <> 4242"));
        assert!(sql.contains("@n <> 3 OR @b <> 300 OR @h1 <> 1 OR @h2 <> 2 OR @h3 <> 3"));
        assert!(sql.contains("UPDATE r SET _MessageNo = NULL FROM dbo._ConfigChngR r WHERE r._MessageNo IS NOT NULL AND (r._MDObjID IN"));
        assert!(
            sql.contains(
                "_ConfigChngR_ExtProps e JOIN dbo.ConfigSave s ON s.FileName = e._FileName"
            ),
            "a staged file resets the object that lists it"
        );
        assert!(
            !sql.contains("DECLARE @alias"),
            "no dynamic history, no fold"
        );
        assert!(sql.contains("BEGIN CATCH"));
    }

    #[test]
    fn a_rehearsal_ends_with_a_rollback() {
        let mut input = inputs();
        input.rehearse = true;
        let sql = render_apply_script(&input).unwrap();
        assert!(sql.contains("ROLLBACK TRANSACTION;\nEND TRY"));
        assert!(!sql.contains("COMMIT TRANSACTION;"));
    }

    #[test]
    fn a_rehearsal_rolls_the_structure_phase_back_with_the_rest() {
        let mut input = inputs();
        input.rehearse = true;
        input.structure_sql = Some(
            "-- restructure marker
SELECT 1;"
                .to_owned(),
        );
        let sql = render_apply_script(&input).unwrap();
        let phase = sql.find("-- restructure marker").unwrap();
        let rollback = sql.find("ROLLBACK TRANSACTION;").unwrap();
        assert!(phase < rollback);
        assert!(!sql.contains("COMMIT TRANSACTION;"));
        // and the phase text stands once, as the gate wrote it
        assert_eq!(sql.matches("-- restructure marker").count(), 1);
    }

    #[test]
    fn the_cache_rows_of_a_structure_phase_are_written_guarded_after_it_in_the_same_transaction() {
        let mut input = inputs();
        input.structure_sql = Some(
            "-- restructure marker
SELECT 1;"
                .to_owned(),
        );
        // the rows the phase makes stale, merged with the apply's own (distinct rows)
        input.params_rewrites = vec![
            ParamsRewrite {
                file_name: "ea13a2c9-0c2f-40fa-b855-710387e3271d.si".to_owned(),
                old_data_size: 10,
                old_sha256_hex: "AA".to_owned(),
                new_bytes: vec![1],
                set_creation: true,
            },
            ParamsRewrite {
                file_name: "siVersions".to_owned(),
                old_data_size: 5,
                old_sha256_hex: "BB".to_owned(),
                new_bytes: vec![2],
                set_creation: false,
            },
        ];
        let sql = render_apply_script(&input).unwrap();
        let phase = sql.find("-- restructure marker").unwrap();
        let first = sql.find("UPDATE dbo.Params SET Creation = @now").unwrap();
        let second = sql
            .find("UPDATE dbo.Params SET Modified = @now, DataSize = 1")
            .unwrap();
        let commit = sql.find("COMMIT TRANSACTION;").unwrap();
        assert!(phase < first && first < second && second < commit);
        // each is guarded by the digest the plan saw
        assert!(sql.contains("FileName = N'ea13a2c9-0c2f-40fa-b855-710387e3271d.si' AND PartNo = 0 AND CONVERT(bigint, DataSize) = 10 AND HASHBYTES('SHA2_256', BinaryData) = 0xAA"));
        assert!(sql.contains("FileName = N'siVersions' AND PartNo = 0 AND CONVERT(bigint, DataSize) = 5 AND HASHBYTES('SHA2_256', BinaryData) = 0xBB"));
    }

    #[test]
    fn dynamic_generations_are_folded_oldest_first_before_the_move() {
        let mut input = inputs();
        input.generations = vec![
            Uuid::parse_str("719baa18-69ed-439a-8962-1de53d98e05e").unwrap(),
            Uuid::parse_str("8c2ac6ff-7309-4025-93f6-264cbb068d62").unwrap(),
        ];
        input.special_config = Fingerprint {
            rows: 5,
            bytes: 1,
            h1: 1,
            h2: 1,
            h3: 1,
        };
        input.special_params = Fingerprint {
            rows: 1,
            bytes: 1,
            h1: 1,
            h2: 1,
            h3: 1,
        };
        let sql = render_apply_script(&input).unwrap();
        let first = sql.find("719baa18-69ed-439a-8962-1de53d98e05e").unwrap();
        let second = sql.find("8c2ac6ff-7309-4025-93f6-264cbb068d62").unwrap();
        let mv = sql.find("INSERT dbo.Config").unwrap();
        assert!(first < second && second < mv);
        assert!(sql.contains("DELETE FROM dbo.Config WHERE FileName = N'versions_dynupdate_719baa18-69ed-439a-8962-1de53d98e05e';"));
        assert!(sql.contains("DELETE FROM dbo.Params WHERE FileName = N'DynamicallyUpdated';"));
    }

    #[test]
    fn a_stage_of_body_rows_alone_leaves_the_params_marker() {
        let mut input = inputs();
        input.special_config = Fingerprint {
            rows: 1,
            bytes: 1,
            h1: 1,
            h2: 1,
            h3: 1,
        };
        input.special_params = input.special_config;
        input.clear_params_marker = false;
        let sql = render_apply_script(&input).unwrap();
        assert!(sql.contains("DELETE FROM dbo.Config WHERE FileName = N'DynamicallyUpdated';"));
        assert!(!sql.contains("DELETE FROM dbo.Params WHERE FileName = N'DynamicallyUpdated';"));
        input.clear_params_marker = true;
        let sql = render_apply_script(&input).unwrap();
        assert!(sql.contains("DELETE FROM dbo.Params WHERE FileName = N'DynamicallyUpdated';"));
    }

    #[test]
    fn descriptor_decision_preimages_precede_publication_and_marker_cleanup() {
        let mut input = inputs();
        input.params_marker_guard_sql =
            "-- bound descriptor decision preimages\nTHROW 57209, 'decision guard fixture', 1;\n"
                .to_owned();
        let sql = render_apply_script(&input).unwrap();
        let guard = sql.find("decision guard fixture").unwrap();
        let lock = sql.find("WITH (TABLOCKX, HOLDLOCK)").unwrap();
        let publication = sql.find("INSERT dbo.Config").unwrap();
        let cleanup = sql.find("DELETE FROM dbo.ConfigSave").unwrap();
        assert!(lock < guard && guard < publication && guard < cleanup);
    }

    #[test]
    fn rewrites_are_guarded_by_the_digest_the_plan_saw() {
        let mut input = inputs();
        input.files_rewrites.push(FilesRewrite {
            file_name: "MobileVersions.dat".to_owned(),
            old_data_size: 10,
            old_sha256_hex: "AB".to_owned(),
            new_bytes: vec![1, 2, 3],
        });
        let sql = render_apply_script(&input).unwrap();
        assert!(sql.contains("FileName = N'MobileVersions.dat' AND PartNo = 0 AND CONVERT(bigint, DataSize) = 10 AND HASHBYTES('SHA2_256', BinaryData) = 0xAB"));
        assert!(sql.contains("DataSize = 3, BinaryData = 0x010203"));
    }

    #[test]
    fn a_params_rewrite_moves_creation_only_when_asked_to() {
        let mut input = inputs();
        input.params_rewrites.push(ParamsRewrite {
            file_name: "1a621f0f-5568-4183-bd9f-f6ef670e7090.si".to_owned(),
            old_data_size: 7,
            old_sha256_hex: "CD".to_owned(),
            new_bytes: vec![9, 9],
            set_creation: true,
        });
        input.params_rewrites.push(ParamsRewrite {
            file_name: "siVersions".to_owned(),
            old_data_size: 5,
            old_sha256_hex: "EF".to_owned(),
            new_bytes: vec![8],
            set_creation: false,
        });
        let sql = render_apply_script(&input).unwrap();
        assert!(sql.contains("UPDATE dbo.Params SET Creation = @now, Modified = @now, DataSize = 2, BinaryData = 0x0909 WHERE FileName = N'1a621f0f-5568-4183-bd9f-f6ef670e7090.si'"));
        assert!(sql.contains("UPDATE dbo.Params SET Modified = @now, DataSize = 1, BinaryData = 0x08 WHERE FileName = N'siVersions'"));
    }

    fn two_nodes() -> Vec<NodeLiteral> {
        vec![
            NodeLiteral {
                plan: 989,
                type_hex: "000003DD".to_owned(),
                reference_hex: "AA".repeat(16),
            },
            NodeLiteral {
                plan: 989,
                type_hex: "000003DD".to_owned(),
                reference_hex: "BB".repeat(16),
            },
        ]
    }

    #[test]
    fn a_node_with_no_rows_gets_a_row_for_each_changed_object_with_the_changed_files() {
        let mut input = inputs();
        input.nodes = two_nodes();
        input.plan_node_counts = vec![(989, 2)];
        input.registration_additions = vec![
            ObjectRegistration {
                object_hex: "11".repeat(16),
                files: vec!["one.0".to_owned()],
            },
            // only the descriptor of this one is staged: its list stays empty
            ObjectRegistration {
                object_hex: "22".repeat(16),
                files: Vec::new(),
            },
        ];
        input.registration_rows_expected = 4;
        input.registration_file_rows_expected = 2;
        let sql = render_apply_script(&input).unwrap();
        let reset = sql.find("UPDATE r SET _MessageNo = NULL").unwrap();
        let nodes_check = sql.find("dbo._Node989 WHERE _PredefinedID = 0x").unwrap();
        let temp = sql.find("CREATE TABLE #reg_add").unwrap();
        let insert = sql.find("INSERT dbo._ConfigChngR (_NodeTRef").unwrap();
        let listing = sql.find("INSERT dbo._ConfigChngR_ExtProps").unwrap();
        let commit = sql.find("COMMIT TRANSACTION;").unwrap();
        assert!(
            reset < nodes_check
                && nodes_check < temp
                && temp < insert
                && insert < listing
                && listing < commit
        );
        // exactly the rows of the plan are missing, checked before they are inserted
        assert!(sql.contains("WHERE NOT EXISTS (SELECT 1 FROM dbo._ConfigChngR x WHERE x._NodeTRef = nd.t AND x._NodeRRef = nd.n AND x._MDObjID = ob.o)"));
        assert!(sql.matches("@@ROWCOUNT <> 4").count() >= 2);
        assert!(sql.contains("@@ROWCOUNT <> 2"));
        // the rows come in with _MessageNo NULL and the files of the object
        assert!(sql.contains("SELECT t, n, NULL, o, id FROM #reg_add"));
        assert!(sql.contains(&format!("(0x{}, 0, N'one.0')", "11".repeat(16))));
        assert!(sql.contains("(0x000003DD, 0x"));
        // ids continue the table's sequence from its greatest id
        assert!(sql.contains("MAX(_IDRRef) FROM dbo._ConfigChngR"));
    }

    #[test]
    fn the_parity_writes_are_the_exclusive_scripts_own_text() {
        // the dynamic apply runs the writes the platform's `force` makes besides the rows -- the
        // change registrations and MobileVersions.dat -- and they are the exclusive apply's SQL, not a copy
        let mut input = inputs();
        input.nodes = two_nodes();
        input.plan_node_counts = vec![(989, 2)];
        input.registration_additions = vec![ObjectRegistration {
            object_hex: "11".repeat(16),
            files: vec!["one.0".to_owned()],
        }];
        input.registration_rows_expected = 2;
        input.registration_file_rows_expected = 2;
        input.files_rewrites = vec![FilesRewrite {
            file_name: "MobileVersions.dat".to_owned(),
            old_data_size: 10,
            old_sha256_hex: "AB".to_owned(),
            new_bytes: vec![1, 2, 3],
        }];
        let parity = render_parity_writes(&input);
        let script = render_apply_script(&input).unwrap();
        // every statement of the fragment is in the script the exclusive apply runs, in the same words
        for line in parity.lines() {
            assert!(script.contains(line), "{line}");
        }
        // the order: the reset, the additions, the file; the timestamps are declared by the caller with the
        // same text the exclusive script declares them with
        let reset = parity.find("UPDATE r SET _MessageNo = NULL").unwrap();
        let additions = parity.find("CREATE TABLE #reg_add").unwrap();
        let file = parity
            .find("UPDATE dbo.Files SET Creation = @now, Modified = @now, DataSize = 3")
            .unwrap();
        assert!(reset < additions && additions < file);
        assert!(!parity.contains("DECLARE @now"));
        for line in timestamp_declarations().lines() {
            assert!(script.contains(line), "{line}");
        }
        assert!(
            timestamp_declarations().contains("DECLARE @now datetime2(6) = DATEADD(year, @offset")
        );
        // it reads the stage, so the caller runs it before ConfigSave is emptied; it takes no table lock,
        // opens no transaction and looks at no session
        for forbidden in [
            "TABLOCKX",
            "BEGIN TRANSACTION",
            "COMMIT",
            "DELETE FROM dbo.ConfigSave",
            "dm_exec_sessions",
        ] {
            assert!(!parity.contains(forbidden), "{forbidden}");
        }
        // what the inputs do not carry is not rendered
        let mut bare = ScriptInputs::default();
        bare.reset_change_registrations = false;
        let text = render_parity_writes(&bare);
        assert!(!text.contains("_ConfigChngR") && !text.contains("dbo.Files"));
    }

    #[test]
    fn nothing_to_add_renders_nothing_and_a_new_object_is_registered_before_the_additions() {
        let mut input = inputs();
        input.nodes = two_nodes();
        assert!(!render_apply_script(&input).unwrap().contains("#reg_add"));
        input.nodes_seen = 5;
        input.plan_node_counts = vec![(989, 2)];
        input.new_registrations = vec![NewRegistration {
            object_hex: "33".repeat(16),
            files: vec!["three.0".to_owned()],
        }];
        input.registration_additions = vec![ObjectRegistration {
            object_hex: "11".repeat(16),
            files: vec!["one.0".to_owned()],
        }];
        input.registration_rows_expected = 2;
        input.registration_file_rows_expected = 2;
        let sql = render_apply_script(&input).unwrap();
        // the new object's check of the nodes in the register comes first: the additions change that count
        let drift = sql
            .find("the exchange-plan nodes changed since the plan was made")
            .unwrap();
        let new_insert = sql.find("INSERT dbo._ConfigChngR (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID, _IDRRef) SELECT CAST(v.t").unwrap();
        let additions = sql.find("CREATE TABLE #reg_add").unwrap();
        assert!(drift < new_insert && new_insert < additions);
    }

    #[test]
    fn the_rows_of_the_owners_of_dropped_rows_are_reset_too() {
        let mut input = inputs();
        input.extra_changed_objects = vec!["AB".repeat(16)];
        let sql = render_apply_script(&input).unwrap();
        assert!(sql.contains(&format!("OR r._MDObjID IN (0x{})", "AB".repeat(16))));
        // without them the predicate is what it was
        assert!(
            !render_apply_script(&inputs())
                .unwrap()
                .contains("OR r._MDObjID IN (0x")
        );
    }

    #[test]
    fn new_objects_are_registered_for_every_node_with_their_files() {
        let mut input = inputs();
        input.nodes_seen = 5;
        input.nodes = vec![
            NodeLiteral {
                plan: 989,
                type_hex: "000003DD".to_owned(),
                reference_hex: "AA".repeat(16),
            },
            NodeLiteral {
                plan: 989,
                type_hex: "000003DD".to_owned(),
                reference_hex: "BB".repeat(16),
            },
        ];
        input.new_registrations = vec![
            NewRegistration {
                object_hex: "11".repeat(16),
                files: vec!["one.0".to_owned(), "one.1".to_owned()],
            },
            NewRegistration {
                object_hex: "22".repeat(16),
                files: vec!["two.0".to_owned()],
            },
        ];
        let sql = render_apply_script(&input).unwrap();
        let reset = sql.find("UPDATE r SET _MessageNo = NULL").unwrap();
        let insert = sql.find("INSERT dbo._ConfigChngR (").unwrap();
        let listing = sql.find("INSERT dbo._ConfigChngR_ExtProps").unwrap();
        assert!(reset < insert && insert < listing);
        // the ids continue the table's sequence
        assert!(sql.contains("CAST(@head + CAST(@tail + v.k AS binary(8)) AS binary(16))"));
        // two nodes x two objects, numbered 1..4, nodes outermost
        assert!(sql.contains(&format!(
            "(0x000003DD, 0x{}, 0x{}, 1), (0x000003DD, 0x{}, 0x{}, 2), (0x000003DD, 0x{}, 0x{}, 3), (0x000003DD, 0x{}, 0x{}, 4)",
            "AA".repeat(16),
            "11".repeat(16),
            "AA".repeat(16),
            "22".repeat(16),
            "BB".repeat(16),
            "11".repeat(16),
            "BB".repeat(16),
            "22".repeat(16)
        )));
        assert!(sql.contains("@@ROWCOUNT <> 4"));
        // three files, each listed for both nodes
        assert!(sql.contains(&format!(
            "(0x{}, 0, N'one.0'), (0x{}, 1, N'one.1'), (0x{}, 0, N'two.0')",
            "11".repeat(16),
            "11".repeat(16),
            "22".repeat(16)
        )));
        assert!(sql.contains("@@ROWCOUNT <> 6"));
        assert!(sql.contains("<> 5"), "the node count is asserted");
        assert!(
            sql.contains("THROW 57317"),
            "an already registered object is refused"
        );
    }

    #[test]
    fn a_file_an_object_gains_is_appended_after_its_last_key() {
        let mut input = inputs();
        input.nodes_seen = 3;
        input.appended_files = ["three.0", "three.1"]
            .iter()
            .map(|name| AppendedFile {
                object_hex: "33".repeat(16),
                file_name: (*name).to_owned(),
            })
            .collect();
        let sql = render_apply_script(&input).unwrap();
        assert!(sql.contains("ROW_NUMBER() OVER (PARTITION BY pending.r ORDER BY pending.f)"));
        assert!(sql.find("WHERE NOT EXISTS (SELECT 1 FROM dbo._ConfigChngR_ExtProps x WITH (UPDLOCK, HOLDLOCK)").unwrap()
            < sql.find(") AS pending;").unwrap());
        assert!(sql.contains("MAX(CAST(e._KeyField AS int)) FROM dbo._ConfigChngR_ExtProps e WITH (UPDLOCK, HOLDLOCK)"));
        assert!(sql.contains("Duplicate appended file input"));
        assert!(sql.contains("Unmeasured appended file key range"));
        assert!(sql.contains("k > 2147483647"));
        assert!(sql.contains(&format!("(0x{}, N'three.1')", "33".repeat(16))));
        assert!(
            !sql.contains("INSERT dbo._ConfigChngR ("),
            "no new object, no new registration row"
        );

        // The exclusive/default path keeps its old all-registration scope. Dynamic callers
        // constrain every existing-row read and assertion to the same measured eligible IDs.
        assert!(!sql.contains("AND r._IDRRef IN ("));
        input.appended_registration_ids = Some(vec!["AA".repeat(16)]);
        let filtered = render_apply_script(&input).unwrap();
        assert_eq!(
            filtered
                .matches(&format!("AND r._IDRRef IN (0x{})", "AA".repeat(16)))
                .count(),
            4
        );
        input.appended_registration_ids = Some(Vec::new());
        let empty = render_apply_script(&input).unwrap();
        assert_eq!(empty.matches("AND 1 = 0").count(), 4);
    }

    #[test]
    fn fingerprints_round_trip_their_five_numbers() {
        let fp = Fingerprint::from_values(&[3, 300, -1, 2, 255]).unwrap();
        assert_eq!(
            fp.hex(),
            "3:300:-1:2:ff".replace("-1", &format!("{:x}", -1i64))
        );
        assert!(Fingerprint::from_values(&[1, 2]).is_err());
        assert!(fingerprint_select("dbo.Config s").contains("FROM dbo.Config s) t"));
    }

    #[test]
    fn a_consumed_row_is_left_out_of_the_move_and_must_not_reach_config() {
        let mut consumed = inputs();
        consumed.consumed_names = vec!["deleted".to_owned()];
        consumed.consumed_row_count = 1;
        let sql = render_apply_script(&consumed).unwrap();
        // the move, the count and the postconditions leave the row out
        assert!(sql.contains(
            "DELETE FROM dbo.Config WHERE FileName IN (SELECT FileName FROM dbo.ConfigSave WHERE FileName NOT IN (N'deleted'));"
        ));
        assert!(sql.contains("FROM dbo.ConfigSave WHERE FileName NOT IN (N'deleted');"));
        assert!(sql.contains("@@ROWCOUNT <> 2"));
        assert!(sql.contains("AND s.FileName NOT IN (N'deleted')"));
        assert!(sql.contains("a consumed staged row reached Config"));
        // in ConfigSave a consumed `deleted` is input, not an unfinished marker; in Config it still is one
        assert!(sql.contains(
            "FROM dbo.ConfigSave WHERE FileName IN (N'commit', N'dynamicCommit', N'dbStruFinal', N'convertPhase', N'erase_save') OR"
        ));
        assert!(sql.contains(
            "FROM dbo.Config WHERE FileName IN (N'commit', N'dynamicCommit', N'dbStruFinal', N'convertPhase', N'erase_save', N'deleted') OR"
        ));
        // ConfigSave is still emptied whole: the count there is every staged row
        let cleanup = sql.find("DELETE FROM dbo.ConfigSave;").unwrap();
        assert!(sql[cleanup..].contains("@@ROWCOUNT <> 3"));
        // without a consumed row the script is the ordinary one
        let plain = render_apply_script(&inputs()).unwrap();
        assert!(!plain.contains("NOT IN (N'deleted')"));
        assert!(plain.contains("@@ROWCOUNT <> 3"));
        assert!(!plain.contains("a consumed staged row reached Config"));
        assert!(plain.contains(
            "FROM dbo.ConfigSave WHERE FileName IN (N'commit', N'dynamicCommit', N'dbStruFinal', N'convertPhase', N'erase_save', N'deleted') OR"
        ));
    }

    #[test]
    fn rows_a_deleted_list_names_are_dropped_and_not_folded() {
        let generation = Uuid::parse_str("719baa18-69ed-439a-8962-1de53d98e05e").unwrap();
        let mut folded = inputs();
        folded.generations = vec![generation];
        let fold = render_apply_script(&folded).unwrap();
        assert!(fold.contains("UPDATE dbo.Config SET FileName = LEFT(FileName"));
        let mut dropped = folded;
        dropped.dropped_rows = vec![
            "ab132638-5188-470d-9432-de85f2b2c7d8_dynupdate_719baa18-69ed-439a-8962-1de53d98e05e"
                .to_owned(),
            "versions_dynupdate_719baa18-69ed-439a-8962-1de53d98e05e".to_owned(),
            "DynamicallyUpdated".to_owned(),
        ];
        let drop = render_apply_script(&dropped).unwrap();
        assert!(!drop.contains("UPDATE dbo.Config SET FileName = LEFT(FileName"));
        assert!(drop.contains("DELETE FROM dbo.Config WHERE FileName IN (N'ab132638-5188-470d-9432-de85f2b2c7d8_dynupdate_719baa18-69ed-439a-8962-1de53d98e05e', N'versions_dynupdate_719baa18-69ed-439a-8962-1de53d98e05e', N'DynamicallyUpdated');"));
        assert!(drop.contains("that the stage''s deleted list names is left"));
    }

    #[test]
    fn the_rows_of_removed_objects_are_fingerprinted_deleted_and_counted_before_the_move() {
        let mut with = inputs();
        with.removed_rows = vec![
            "8a7546f4-bfc9-4732-bf60-43a41e2c8753".to_owned(),
            "8a7546f4-bfc9-4732-bf60-43a41e2c8753.0".to_owned(),
        ];
        with.removed = Fingerprint {
            rows: 2,
            bytes: 100,
            h1: 7,
            h2: 8,
            h3: 9,
        };
        with.dropped_rows = vec!["ab132638-5188-470d-9432-de85f2b2c7d8_dynupdate_x".to_owned()];
        let sql = render_apply_script(&with).unwrap();
        let names =
            "N'8a7546f4-bfc9-4732-bf60-43a41e2c8753', N'8a7546f4-bfc9-4732-bf60-43a41e2c8753.0'";
        // the digest of the rows the plan saw is asserted under the locks, ahead of anything written
        let drift = sql.find("Config removed rows").unwrap();
        assert!(sql.contains(&format!("dbo.Config s WHERE s.FileName IN ({names})")));
        assert!(sql.contains("@n <> 2 OR @b <> 100 OR @h1 <> 7 OR @h2 <> 8 OR @h3 <> 9"));
        assert!(drift < sql.find("DECLARE @now datetime2(6)").unwrap());
        // deleted after the dynamic rows, before the move, counted and checked
        let dynamic = sql.find("_dynupdate_x").unwrap();
        let delete = sql
            .find(&format!(
                "DELETE FROM dbo.Config WHERE FileName IN ({names});"
            ))
            .unwrap();
        let moved = sql.find("INSERT dbo.Config").unwrap();
        assert!(dynamic < delete && delete < moved);
        assert!(sql[delete..moved].contains("IF @@ROWCOUNT <> 2 THROW 57321"));
        assert_eq!(sql.matches("THROW 57321").count(), 2);
        assert!(sql.contains(&format!(
            "IF EXISTS (SELECT 1 FROM dbo.Config WHERE FileName IN ({names})) THROW 57321"
        )));
        // a stage that removes nothing has none of it
        let plain = render_apply_script(&inputs()).unwrap();
        assert!(!plain.contains("Config removed rows") && !plain.contains("57321"));
    }
}
