//! Original-session transport, not a configuration/source-preimage authority.
//! The first seam only owns closed session-local relations and their terminal
//! transaction. Main publication, locks and canonical admission remain callers'
//! separately reviewed responsibilities.

use std::collections::BTreeSet;

use anyhow::{Result, anyhow, bail, ensure};
use sha2::{Digest, Sha256};

use super::mssql::SealedSqlScript;
use super::{SqlParam, SqlRow};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum InputRelation {
    Proposed,
    Expected,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StorageTable {
    Config,
    ConfigSave,
    Params,
    Files,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct InputDomain {
    pub relation: InputRelation,
    pub table: StorageTable,
}

impl InputDomain {
    fn codes(self) -> (u8, u8) {
        (
            match self.relation {
                InputRelation::Proposed => 0,
                InputRelation::Expected => 1,
            },
            match self.table {
                StorageTable::Config => 0,
                StorageTable::ConfigSave => 1,
                StorageTable::Params => 2,
                StorageTable::Files => 3,
            },
        )
    }
}

/// Borrowed transport columns, not a second canonical metadata model. A later
/// source owner must supply its actual captured immutable bytes and headers.
/// These columns alone cannot establish that a database preimage was captured.
#[derive(Clone, Copy)]
pub struct InputRow<'a> {
    pub domain: InputDomain,
    pub file_name: &'a str,
    pub part_no: i32,
    pub creation: &'a str,
    pub modified: &'a str,
    pub attributes: i32,
    pub data_size: i64,
    pub binary: &'a [u8],
    pub sha256: [u8; 32],
}

struct CheckedRow<'a> {
    input: InputRow<'a>,
    ordinal: i64,
    offset: i64,
    name_bytes: Vec<u8>,
}

/// Complete actual host input validated before CREATE/INSERT/publication.
/// Empty declared domains are retained; they are not silently dropped.
pub struct SealedSqlInput<'a> {
    domains: BTreeSet<InputDomain>,
    rows: Vec<CheckedRow<'a>>,
    bytes: i64,
}

impl<'a> SealedSqlInput<'a> {
    pub fn new(
        domains: &[InputDomain],
        rows: impl IntoIterator<Item = InputRow<'a>>,
    ) -> Result<Self> {
        let mut declared = BTreeSet::new();
        for domain in domains {
            ensure!(declared.insert(*domain), "duplicate input domain");
        }
        let mut checked = Vec::new();
        let mut keys = BTreeSet::new();
        let mut offset = 0i64;
        for input in rows {
            ensure!(
                declared.contains(&input.domain),
                "row belongs to an undeclared input domain"
            );
            ensure!(
                !input.file_name.is_empty() && !input.file_name.chars().any(char::is_control),
                "invalid input row key"
            );
            // Intrinsic SQL nvarchar(max)/varbinary(max), not a source quota.
            i32::try_from(
                input
                    .file_name
                    .encode_utf16()
                    .count()
                    .checked_mul(2)
                    .ok_or_else(|| anyhow!("nvarchar byte extent overflow"))?,
            )?;
            i32::try_from(input.binary.len())?;
            ensure!(
                input.part_no >= 0 && input.data_size >= 0,
                "negative physical row header"
            );
            for stamp in [input.creation, input.modified] {
                ensure!(
                    stamp.is_ascii() && stamp.len() <= 27 && !stamp.chars().any(char::is_control),
                    "timestamp cannot be represented by the existing varchar(27) header"
                );
            }
            ensure!(
                <[u8; 32]>::from(Sha256::digest(input.binary)) == input.sha256,
                "input row digest differs from actual payload"
            );
            // Same case-fold domain as the existing RowMeta::key. Exact binary
            // spelling is independently checked on the server result below.
            ensure!(
                keys.insert((input.domain, input.file_name.to_lowercase(), input.part_no)),
                "duplicate or case-aliased input row key"
            );
            let ordinal = i64::try_from(checked.len())?;
            let length = i64::try_from(input.binary.len())?;
            checked.try_reserve(1)?;
            checked.push(CheckedRow {
                input,
                ordinal,
                offset,
                name_bytes: utf16_bytes(input.file_name)?,
            });
            offset = offset
                .checked_add(length)
                .ok_or_else(|| anyhow!("input relation byte offset overflow"))?;
        }
        Ok(Self {
            domains: declared,
            rows: checked,
            bytes: offset,
        })
    }
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
    pub fn byte_length(&self) -> i64 {
        self.bytes
    }
    pub fn domains(&self) -> impl ExactSizeIterator<Item = &InputDomain> {
        self.domains.iter()
    }
}

/// Only closed input/transaction operations below may establish terminal
/// authority. Arbitrary sealed SQL has no proved transaction semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnedSqlState {
    Created,
    TransferringInput,
    InputSealed,
    TransactionOpen,
    CommitPending,
    Committed,
    RolledBackKnown,
    Unknown,
}

/// Private boundary: the live implementer contains the actual original
/// TdsConnection, with no pool/reset/relogin/retry operation in this interface.
pub(crate) trait OriginalSqlDriver {
    fn run_batch(&mut self, sql: &str) -> Result<()>;
    fn simple_rows(&mut self, sql: &str, each: &mut dyn FnMut(SqlRow) -> Result<()>) -> Result<()>;
    fn execute(&mut self, sql: &str, params: &[SqlParam<'_>]) -> Result<u64>;
    fn read_rows(
        &mut self,
        sql: &str,
        params: &[SqlParam<'_>],
        each: &mut dyn FnMut(SqlRow) -> Result<()>,
    ) -> Result<()>;
}

/// One retained original driver and terminal state. On Unknown the driver,
/// runtime, TCP client and obtained error stay owned until the host decides
/// this object's lifetime. Dropping it closes resources; it proves no outcome.
pub struct OwnedSqlCommand {
    driver: Box<dyn OriginalSqlDriver>,
    session_id: i64,
    state: OwnedSqlState,
    last_error: Option<anyhow::Error>,
    last_observed_row: Option<SqlRow>,
}

impl std::fmt::Debug for OwnedSqlCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedSqlCommand")
            .field("session_id", &self.session_id)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl OwnedSqlCommand {
    pub(crate) fn from_original(driver: Box<dyn OriginalSqlDriver>) -> Self {
        let mut owner = Self {
            driver,
            session_id: 0,
            state: OwnedSqlState::Created,
            last_error: None,
            last_observed_row: None,
        };
        let mut session_id = None;
        let observed = &mut owner.last_observed_row;
        let result = owner
            .driver
            .simple_rows("SELECT @@SPID;", &mut |row| {
                *observed = Some(row);
                let row = observed.as_ref().expect("obtained original row");
                ensure!(
                    session_id.is_none() && row.result_set == 0 && row.values.len() == 1,
                    "original session identity result differs"
                );
                let id = row.i64(0)?;
                ensure!(id > 0, "original session returned an invalid SPID");
                session_id = Some(id);
                Ok(())
            })
            .and_then(|()| {
                session_id.ok_or_else(|| anyhow!("original session returned no identity"))
            });
        match result {
            Ok(id) => owner.session_id = id,
            Err(error) => {
                owner.state = OwnedSqlState::Unknown;
                owner.last_error = Some(error);
            }
        }
        owner
    }
    pub fn state(&self) -> OwnedSqlState {
        self.state
    }
    pub fn session_id(&self) -> i64 {
        self.session_id
    }
    pub fn retained_error(&self) -> Option<&anyhow::Error> {
        self.last_error.as_ref()
    }
    pub fn retained_observation(&self) -> Option<&SqlRow> {
        self.last_observed_row.as_ref()
    }

    fn require(&self, state: OwnedSqlState) -> Result<()> {
        ensure!(
            self.state == state,
            "original command is {:?}, expected {state:?}",
            self.state
        );
        Ok(())
    }
    fn unknown<T>(&mut self, error: anyhow::Error) -> Result<T> {
        self.state = OwnedSqlState::Unknown;
        let message = format!("original SQL command outcome is unknown: {error:#}");
        self.last_error = Some(error);
        bail!("{message}")
    }

    /// Create/insert/check only private #temp relations. Complete host sealing
    /// precedes any dispatch; transfer has no aggregate row or byte ceiling.
    pub fn transfer_input(&mut self, input: &SealedSqlInput<'_>) -> Result<()> {
        self.require(OwnedSqlState::Created)?;
        self.state = OwnedSqlState::TransferringInput;
        let result = self.transfer(input);
        match result {
            Ok(()) => {
                self.state = OwnedSqlState::InputSealed;
                Ok(())
            }
            Err(error) => self.unknown(error),
        }
    }

    fn transfer(&mut self, input: &SealedSqlInput<'_>) -> Result<()> {
        self.driver.run_batch(CREATE_RELATIONS)?;
        for domain in &input.domains {
            let (role, table) = domain.codes();
            ensure!(
                self.driver
                    .execute(INSERT_DOMAIN, &[SqlParam::U8(role), SqlParam::U8(table)])?
                    == 1,
                "input domain insert was not acknowledged exactly once"
            );
        }
        // Same existing INSERT shape and natural parameter bound. The tuning
        // split closes a request, never refuses a larger total input or row.
        let mut start = 0usize;
        while start < input.rows.len() {
            let mut end = start;
            let mut wire_bytes = 0usize;
            while end < input.rows.len()
                && end - start < super::mssql::INSERT_ROWS
                && (end - start + 1) * COLUMNS.len() <= super::mssql::MAX_PARAMETERS
            {
                let bytes = checked_wire_bytes(&input.rows[end])?;
                let total = wire_bytes
                    .checked_add(bytes)
                    .ok_or_else(|| anyhow!("parameter byte extent overflow"))?;
                if end > start && total > super::mssql::INSERT_BYTES {
                    break;
                }
                wire_bytes = total;
                end += 1;
            }
            ensure!(end > start, "a physical row cannot fit the parameter shape");
            let count = (end - start)
                .checked_mul(COLUMNS.len())
                .ok_or_else(|| anyhow!("parameter count overflow"))?;
            let mut params = Vec::new();
            params.try_reserve_exact(count)?;
            for row in &input.rows[start..end] {
                params.extend(row_params(row));
            }
            let sql = super::mssql::insert_statement(ROW_TABLE, &COLUMNS, end - start);
            ensure!(
                self.driver.execute(&sql, &params)? == u64::try_from(end - start)?,
                "input row insert acknowledgement count differs"
            );
            start = end;
        }
        self.verify_domains(input)?;
        let mut next = 0usize;
        let observed = &mut self.last_observed_row;
        self.driver.read_rows(READ_ROWS, &[], &mut |actual| {
            *observed = Some(actual);
            let actual = observed.as_ref().expect("obtained input row");
            let expected = input
                .rows
                .get(next)
                .ok_or_else(|| anyhow!("unexpected extra input relation row"))?;
            verify_row(actual, expected)?;
            next = next
                .checked_add(1)
                .ok_or_else(|| anyhow!("input row count overflow"))?;
            Ok(())
        })?;
        ensure!(
            next == input.rows.len(),
            "input relation is missing expected rows"
        );
        Ok(())
    }

    fn verify_domains(&mut self, input: &SealedSqlInput<'_>) -> Result<()> {
        let mut expected = input.domains.iter();
        let observed = &mut self.last_observed_row;
        self.driver.read_rows(READ_DOMAINS, &[], &mut |row| {
            *observed = Some(row);
            let row = observed.as_ref().expect("obtained input domain row");
            let domain = expected
                .next()
                .ok_or_else(|| anyhow!("unexpected input domain"))?;
            let (role, table) = domain.codes();
            ensure!(
                row.result_set == 0
                    && row.values.len() == 2
                    && row.i64(0)? == i64::from(role)
                    && row.i64(1)? == i64::from(table),
                "input domain identity differs"
            );
            Ok(())
        })?;
        ensure!(
            expected.next().is_none(),
            "input domain census is incomplete"
        );
        Ok(())
    }

    /// Only this closed transaction owns known terminal authority. It does not
    /// publish Config/ConfigSave; application lock/CAS migration is still G4.3.
    pub fn begin(&mut self) -> Result<()> {
        self.require(OwnedSqlState::InputSealed)?;
        match self.terminal(BEGIN, 1) {
            Ok(()) => {
                self.state = OwnedSqlState::TransactionOpen;
                Ok(())
            }
            Err(error) => self.unknown(error),
        }
    }
    pub fn commit(&mut self) -> Result<()> {
        self.require(OwnedSqlState::TransactionOpen)?;
        self.state = OwnedSqlState::CommitPending;
        match self.terminal(COMMIT, 0) {
            Ok(()) => {
                self.state = OwnedSqlState::Committed;
                Ok(())
            }
            Err(error) => self.unknown(error),
        }
    }
    pub fn rollback(&mut self) -> Result<()> {
        self.require(OwnedSqlState::TransactionOpen)?;
        match self.terminal(ROLLBACK, 0) {
            Ok(()) => {
                self.state = OwnedSqlState::RolledBackKnown;
                Ok(())
            }
            Err(error) => self.unknown(error),
        }
    }
    fn terminal(&mut self, sql: &str, count: i64) -> Result<()> {
        let session_id = self.session_id;
        let mut rows = 0usize;
        self.last_observed_row = None;
        let observed = &mut self.last_observed_row;
        self.driver.simple_rows(sql, &mut |row| {
            *observed = Some(row);
            let row = observed.as_ref().expect("obtained original terminal row");
            ensure!(
                rows == 0
                    && row.result_set == 0
                    && row.values.len() == 2
                    && row.i64(0)? == session_id
                    && row.i64(1)? == count,
                "foreign or malformed original transaction acknowledgement"
            );
            rows += 1;
            Ok(())
        })?;
        ensure!(rows == 1, "original transaction acknowledgement missing");
        Ok(())
    }

    /// Arbitrary SQL may contain its own COMMIT/BEGIN, so even drained success
    /// cannot establish our closed transaction authority. Before the first
    /// batch it becomes permanently Unknown/unproven. No subsequent owned
    /// commit/rollback/retry is admitted. Returns only acknowledged batch count.
    pub fn dispatch_unproven_script(&mut self, script: &SealedSqlScript) -> Result<usize> {
        self.require(OwnedSqlState::Created)?;
        if script.batches().is_empty() {
            return Ok(0);
        }
        self.state = OwnedSqlState::Unknown;
        let mut acknowledged = 0usize;
        for batch in script.batches() {
            if let Err(error) = self.driver.run_batch(&batch.text) {
                return self.unknown(error);
            }
            acknowledged = acknowledged
                .checked_add(1)
                .ok_or_else(|| anyhow!("script acknowledgement count overflow"))?;
        }
        Ok(acknowledged)
    }

    /// Full EOF/UTF-8/GO/sqlcmd validation completes before any batch dispatch.
    pub fn dispatch_unproven_reader(
        &mut self,
        input: impl std::io::Read,
        variables: super::ScriptVariables,
    ) -> Result<usize> {
        self.require(OwnedSqlState::Created)?;
        let script = SealedSqlScript::read(input, variables)?;
        self.dispatch_unproven_script(&script)
    }
}

const COLUMNS: [&str; 14] = [
    "Role",
    "StorageTable",
    "Ordinal",
    "ByteOffset",
    "FileName",
    "NameBytes",
    "PartNo",
    "Creation",
    "Modified",
    "Attributes",
    "DataSize",
    "ByteLength",
    "Digest",
    "BinaryData",
];
const ROW_TABLE: &str = "#ibcmd_owned_rows";
const CREATE_RELATIONS: &str = "SET NOCOUNT OFF; IF OBJECT_ID('tempdb..#ibcmd_owned_rows') IS NOT NULL OR OBJECT_ID('tempdb..#ibcmd_owned_domains') IS NOT NULL THROW 57800, 'owned input relation already exists', 1;
CREATE TABLE #ibcmd_owned_domains (Role tinyint NOT NULL,StorageTable tinyint NOT NULL,PRIMARY KEY(Role,StorageTable));
CREATE TABLE #ibcmd_owned_rows (Role tinyint NOT NULL,StorageTable tinyint NOT NULL,Ordinal bigint NOT NULL PRIMARY KEY,ByteOffset bigint NOT NULL,FileName nvarchar(max) COLLATE Latin1_General_100_BIN2 NOT NULL,NameBytes varbinary(max) NOT NULL,PartNo int NOT NULL,Creation varchar(27) COLLATE Latin1_General_100_BIN2 NOT NULL,Modified varchar(27) COLLATE Latin1_General_100_BIN2 NOT NULL,Attributes int NOT NULL,DataSize bigint NOT NULL,ByteLength bigint NOT NULL,Digest binary(32) NOT NULL,BinaryData varbinary(max) NOT NULL);";
const INSERT_DOMAIN: &str = "INSERT #ibcmd_owned_domains (Role,StorageTable) VALUES (@P1,@P2);";
const READ_DOMAINS: &str =
    "SELECT Role,StorageTable FROM #ibcmd_owned_domains ORDER BY Role,StorageTable;";
// BIN2 '=' still pads spaces in SQL Server. Full original UTF16LE bytes and
// byte length are independently compared, in addition to exact host text.
const READ_ROWS: &str = "SELECT Role,StorageTable,Ordinal,ByteOffset,FileName,NameBytes,CONVERT(varbinary(max),FileName),CONVERT(bigint,DATALENGTH(FileName)),PartNo,Creation,Modified,Attributes,DataSize,ByteLength,Digest,HASHBYTES('SHA2_256',BinaryData),CONVERT(bigint,DATALENGTH(BinaryData)) FROM #ibcmd_owned_rows ORDER BY Ordinal;";
const BEGIN: &str = "IF @@TRANCOUNT <> 0 THROW 57801, 'foreign transaction before owned BEGIN', 1; BEGIN TRANSACTION; SELECT @@SPID,@@TRANCOUNT;";
const COMMIT: &str = "IF @@TRANCOUNT <> 1 THROW 57802, 'owned COMMIT transaction mismatch', 1; COMMIT TRANSACTION; SELECT @@SPID,@@TRANCOUNT;";
const ROLLBACK: &str = "IF @@TRANCOUNT <> 1 THROW 57803, 'owned ROLLBACK transaction mismatch', 1; ROLLBACK TRANSACTION; SELECT @@SPID,@@TRANCOUNT;";

fn utf16_bytes(text: &str) -> Result<Vec<u8>> {
    let length = text
        .encode_utf16()
        .count()
        .checked_mul(2)
        .ok_or_else(|| anyhow!("UTF16 key byte extent overflow"))?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length)?;
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    Ok(bytes)
}
fn row_params<'a>(row: &'a CheckedRow<'a>) -> [SqlParam<'a>; 14] {
    let x = row.input;
    let (role, table) = x.domain.codes();
    [
        SqlParam::U8(role),
        SqlParam::U8(table),
        SqlParam::I64(row.ordinal),
        SqlParam::I64(row.offset),
        SqlParam::Text(x.file_name),
        SqlParam::Binary(&row.name_bytes),
        SqlParam::I32(x.part_no),
        SqlParam::Text(x.creation),
        SqlParam::Text(x.modified),
        SqlParam::I32(x.attributes),
        SqlParam::I64(x.data_size),
        SqlParam::I64(i64::try_from(x.binary.len()).expect("sealed physical length")),
        SqlParam::Binary(&row.input.sha256),
        SqlParam::Binary(x.binary),
    ]
}
fn checked_wire_bytes(row: &CheckedRow<'_>) -> Result<usize> {
    row_params(row).iter().try_fold(0usize, |total, param| {
        total
            .checked_add(param.checked_wire_bytes()?)
            .ok_or_else(|| anyhow!("parameter byte count overflow"))
    })
}

fn verify_row(row: &SqlRow, expected: &CheckedRow<'_>) -> Result<()> {
    let x = expected.input;
    let (role, table) = x.domain.codes();
    ensure!(
        row.result_set == 0 && row.values.len() == 17,
        "unexpected input result shape"
    );
    ensure!(
        row.i64(0)? == i64::from(role)
            && row.i64(1)? == i64::from(table)
            && row.i64(2)? == expected.ordinal
            && row.i64(3)? == expected.offset
            && row.text(4)? == x.file_name
            && row.binary(5)? == expected.name_bytes.as_slice()
            && row.binary(6)? == expected.name_bytes.as_slice()
            && row.i64(7)? == i64::try_from(expected.name_bytes.len())?
            && row.i64(8)? == i64::from(x.part_no)
            && row.text(9)? == x.creation
            && row.text(10)? == x.modified
            && row.i64(11)? == i64::from(x.attributes)
            && row.i64(12)? == x.data_size
            && row.i64(13)? == i64::try_from(x.binary.len())?
            && row.binary(14)? == x.sha256
            && row.binary(15)? == x.sha256
            && row.i64(16)? == i64::try_from(x.binary.len())?,
        "input full raw key/header/payload relation differs"
    );
    Ok(())
}

#[cfg(test)]
mod tests;
