//! The TDS connection: tiberius over tokio's TCP stream, driven by a
//! current-thread tokio runtime of its own, so that the synchronous code of
//! ibcmd-rs (and its rayon workers) calls it like a blocking client.

use std::borrow::Cow;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use futures_util::TryStreamExt;
use tiberius::{
    AuthMethod, Client, ColumnData, Config, EncryptionLevel, Query, QueryItem, Row, SqlBrowser,
};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use super::address::ServerAddress;
use super::script::{ScriptBatch, split_batches};
use crate::sql::{ScriptVariables, SqlLogin, SqlParam, SqlRow, SqlTarget, SqlValue};

type TdsClient = Client<Compat<TcpStream>>;

/// Attempts at opening a connection before a transient failure (a refused or
/// timed-out TCP connect, a stalled login handshake on a busy machine) is
/// reported. sqlcmd runs were retried the same way.
const CONNECT_ATTEMPTS: u32 = 6;
/// A TCP connect that neither succeeds nor fails is abandoned after this.
const TCP_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// The largest TDS packet a client may ask for; the server may grant less.
/// Large packets carry the staged rows (tens of MB) in fewer round trips.
const PACKET_SIZE: u32 = 32767;

fn bind<'q>(query: &mut Query<'q>, param: SqlParam<'q>) {
    match param {
        SqlParam::Text(value) => query.bind(value),
        SqlParam::U8(value) => query.bind(value),
        SqlParam::I32(value) => query.bind(value),
        SqlParam::I64(value) => query.bind(value),
        SqlParam::Binary(value) => query.bind(value),
    }
}

/// A result row as owned values.
pub fn sql_row(row: Row) -> SqlRow {
    SqlRow {
        result_set: row.result_index(),
        values: row.into_iter().map(sql_value).collect(),
    }
}

/// A column value as an owned value; binary and text move, not copy.
pub fn sql_value(value: ColumnData<'static>) -> SqlValue {
    match value {
        ColumnData::U8(value) => value.map_or(SqlValue::Null, |value| SqlValue::Int(value.into())),
        ColumnData::I16(value) => value.map_or(SqlValue::Null, |value| SqlValue::Int(value.into())),
        ColumnData::I32(value) => value.map_or(SqlValue::Null, |value| SqlValue::Int(value.into())),
        ColumnData::I64(value) => value.map_or(SqlValue::Null, SqlValue::Int),
        ColumnData::Bit(value) => value.map_or(SqlValue::Null, |value| SqlValue::Int(value.into())),
        ColumnData::F32(value) => {
            value.map_or(SqlValue::Null, |value| SqlValue::Float(value.into()))
        }
        ColumnData::F64(value) => value.map_or(SqlValue::Null, SqlValue::Float),
        ColumnData::String(value) => {
            value.map_or(SqlValue::Null, |value| SqlValue::Text(value.into_owned()))
        }
        ColumnData::Binary(value) => {
            value.map_or(SqlValue::Null, |value| SqlValue::Binary(value.into_owned()))
        }
        ColumnData::Xml(value) => value.map_or(SqlValue::Null, |value| {
            SqlValue::Text(value.into_owned().into_string())
        }),
        ColumnData::Numeric(value) => value.map_or(SqlValue::Null, |value| {
            if value.scale() == 0
                && let Ok(value) = i64::try_from(value.value())
            {
                return SqlValue::Int(value);
            }
            SqlValue::Other(value.to_string())
        }),
        ColumnData::Guid(value) => {
            value.map_or(SqlValue::Null, |value| SqlValue::Other(value.to_string()))
        }
        ColumnData::DateTime(None)
        | ColumnData::SmallDateTime(None)
        | ColumnData::Time(None)
        | ColumnData::Date(None)
        | ColumnData::DateTime2(None)
        | ColumnData::DateTimeOffset(None) => SqlValue::Null,
        other => SqlValue::Other(format!("{other:?}")),
    }
}

/// One open connection to SQL Server.
pub struct TdsConnection {
    runtime: tokio::runtime::Runtime,
    client: TdsClient,
    /// The database the session started in, where a query that switched
    /// databases (`USE`) brings it back.
    home_database: String,
}

impl TdsConnection {
    /// Opens a connection, retrying transient connect failures.
    pub fn open(target: &SqlTarget, address: &ServerAddress) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .context("failed to start the SQL client runtime")?;
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let result = runtime.block_on(connect(target, address));
            match result {
                Ok(client) => {
                    let mut connection = Self {
                        runtime,
                        client,
                        home_database: String::new(),
                    };
                    connection
                        .initialize_session()
                        .with_context(|| format!("SQL Server {}", target.server))?;
                    return Ok(connection);
                }
                Err(error) if attempt < CONNECT_ATTEMPTS && connect_error_is_transient(&error) => {
                    std::thread::sleep(Duration::from_secs(u64::from(attempt) * 2));
                }
                Err(error) => {
                    return Err(anyhow!(error)).with_context(|| {
                        format!(
                            "failed to connect to SQL Server {} as {}",
                            target.server,
                            target.login.describe()
                        )
                    });
                }
            }
        }
    }

    /// The session options a `sqlcmd` session starts with, so that scripts
    /// written for sqlcmd behave the same: it switches QUOTED_IDENTIFIER off
    /// (the only option in which the two sessions differ; checked with
    /// `@@OPTIONS`: 5688 under sqlcmd, 5944 under a bare TDS login).
    fn initialize_session(&mut self) -> Result<()> {
        self.run_batch("SET QUOTED_IDENTIFIER OFF;")?;
        let mut home = None;
        self.query_each("SELECT DB_NAME()", &[], |row| {
            home = row.try_get::<&str, _>(0)?.map(ToOwned::to_owned);
            Ok(())
        })?;
        self.home_database = home.context("the session reports no current database")?;
        Ok(())
    }

    /// `sp_executesql` scopes a query's `SET` options and temporary tables,
    /// but not its `USE`: a query that may have switched databases is
    /// followed by a switch back, so a pooled connection comes back as it
    /// went out.
    fn restore_database(&mut self, sql: &str) -> Result<()> {
        if !mentions_use(sql) || self.home_database.is_empty() {
            return Ok(());
        }
        let statement = format!("USE [{}];", self.home_database.replace(']', "]]"));
        self.run_batch(&statement)
    }

    /// Runs one batch as sqlcmd sends it (`simple_query`): statements that
    /// change the session (`USE`, `SET`, temporary tables, an open
    /// transaction) last until the connection closes. Results are drained and
    /// dropped; the first error of the batch is returned once the server has
    /// finished it, as `sqlcmd -b` would report it.
    pub fn run_batch(&mut self, sql: &str) -> Result<()> {
        let Self {
            runtime, client, ..
        } = self;
        runtime
            .block_on(async {
                let mut stream = client.simple_query(sql).await?;
                while stream.try_next().await?.is_some() {}
                Ok::<_, tiberius::error::Error>(())
            })
            .map_err(request_error)
    }

    /// Runs a query through `sp_executesql` and hands every row to `row`,
    /// in order, as it arrives. What the text changes in the session (`SET`
    /// options, `#temp` tables, the current database) ends with the call, so
    /// a pooled connection comes back as it went out.
    pub fn query_each<'q>(
        &mut self,
        sql: &'q str,
        params: &[SqlParam<'q>],
        mut row: impl FnMut(Row) -> Result<()>,
    ) -> Result<()> {
        let Self {
            runtime, client, ..
        } = self;
        runtime.block_on(async {
            let mut query = Query::new(Cow::Borrowed(sql));
            for param in params {
                bind(&mut query, *param);
            }
            let mut stream = query.query(client).await.map_err(request_error)?;
            while let Some(item) = stream.try_next().await.map_err(request_error)? {
                if let QueryItem::Row(value) = item {
                    row(value)?;
                }
            }
            Ok::<_, anyhow::Error>(())
        })?;
        self.restore_database(sql)
    }

    /// Runs a statement through `sp_executesql` and returns the rows it
    /// affected (all statements of the text summed); session changes end with
    /// it as with [`TdsConnection::query_each`].
    pub fn execute<'q>(&mut self, sql: &'q str, params: &[SqlParam<'q>]) -> Result<u64> {
        let Self {
            runtime, client, ..
        } = self;
        let result = runtime
            .block_on(async {
                let mut query = Query::new(Cow::Borrowed(sql));
                for param in params {
                    bind(&mut query, *param);
                }
                query.execute(client).await
            })
            .map_err(request_error)?;
        self.restore_database(sql)?;
        Ok(result.total())
    }
}

/// The prefix of every error the server or the connection reports for a
/// request (a query, a statement, a batch).
pub const REQUEST_FAILED: &str = "SQL Server request failed";

fn request_error(error: tiberius::error::Error) -> anyhow::Error {
    anyhow::Error::new(error).context(REQUEST_FAILED)
}

/// Whether a text may contain a `USE` statement: the word `use` (any case)
/// followed by a blank or `[`. A false positive costs one round trip.
fn mentions_use(sql: &str) -> bool {
    let bytes = sql.as_bytes();
    bytes.windows(4).enumerate().any(|(index, window)| {
        window[..3].eq_ignore_ascii_case(b"use")
            && (window[3].is_ascii_whitespace() || window[3] == b'[')
            && (index == 0 || !is_identifier_byte(bytes[index - 1]))
    })
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'@' | b'#' | b'$') || byte >= 0x80
}

async fn connect(target: &SqlTarget, address: &ServerAddress) -> tiberius::Result<TdsClient> {
    let mut config = tiberius_config(target, address)?;
    let mut named = address.browser_instance().is_some();
    let mut routed = false;
    loop {
        let tcp = tcp_connect(&config, named).await?;
        match Client::connect(config.clone(), tcp.compat_write()).await {
            // An Azure gateway or an availability group listener may send the
            // client on to the instance that serves the database.
            Err(tiberius::error::Error::Routing { host, port }) if !routed => {
                routed = true;
                named = false;
                config = tiberius_config(target, address)?;
                config.host(host);
                config.port(port);
            }
            result => return result,
        }
    }
}

/// Opens the TCP stream; `named` asks the SQL Browser for the instance's port
/// first.
async fn tcp_connect(config: &Config, named: bool) -> tiberius::Result<TcpStream> {
    let connect = async {
        if named {
            TcpStream::connect_named(config).await
        } else {
            let stream = TcpStream::connect(config.get_addr()).await?;
            Ok(stream)
        }
    };
    let stream = tokio::time::timeout(TCP_CONNECT_TIMEOUT, connect)
        .await
        .map_err(|_| {
            tiberius::error::Error::from(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!(
                    "no TCP connection to {} within {} s",
                    config.get_addr(),
                    TCP_CONNECT_TIMEOUT.as_secs()
                ),
            ))
        })??;
    stream.set_nodelay(true)?;
    Ok(stream)
}

fn tiberius_config(target: &SqlTarget, address: &ServerAddress) -> tiberius::Result<Config> {
    let mut config = Config::new();
    config.host(&address.host);
    match address.browser_instance() {
        Some(instance) => config.instance_name(instance),
        None => config.port(address.direct_port()),
    }
    if let Some(database) = &target.database {
        config.database(database);
    }
    config.application_name("ibcmd-rs");
    config.authentication(authentication(&target.login)?);
    // ODBC 18's sqlcmd encrypts every connection; so does the built-in client.
    config.encryption(EncryptionLevel::Required);
    if target.trust_server_certificate {
        config.trust_cert();
    }
    // A staged apply or a bulk read may keep one request busy for minutes;
    // sqlcmd waits without limit as well (`-t 0`).
    config.command_timeout(None);
    config.packet_size(PACKET_SIZE);
    Ok(config)
}

fn authentication(login: &SqlLogin) -> tiberius::Result<AuthMethod> {
    match login {
        SqlLogin::Sql {
            user,
            password: Some(password),
        } => Ok(AuthMethod::sql_server(user, password)),
        SqlLogin::Sql {
            user,
            password: None,
        } => Err(tiberius::error::Error::Conversion(
            format!("SQL login {user:?} has no password").into(),
        )),
        #[cfg(windows)]
        SqlLogin::Integrated => Ok(AuthMethod::Integrated),
        #[cfg(not(windows))]
        SqlLogin::Integrated => Err(tiberius::error::Error::Conversion(
            "Windows (integrated) authentication is available on Windows only; pass --sql-user"
                .into(),
        )),
    }
}

/// A failure worth another attempt: the network or the handshake, not a
/// refused login, a missing database or a protocol mismatch.
fn connect_error_is_transient(error: &tiberius::error::Error) -> bool {
    match error {
        tiberius::error::Error::Io { .. } | tiberius::error::Error::Tls(_) => true,
        tiberius::error::Error::Server(token) => !matches!(
            token.code(),
            // Login failed, password expired or must change, database
            // missing or not accessible.
            18456 | 18470 | 18486 | 18487 | 18488 | 4060 | 4064
        ),
        _ => false,
    }
}

/// A small set of open connections shared by the threads of one command.
///
/// A connection goes back to the pool after a call that succeeded; after an
/// error it is closed instead (its stream or its session may be half-way
/// through something). At most `capacity` connections are open at once; a
/// caller beyond that waits for one to come back.
pub struct TdsPool {
    target: SqlTarget,
    address: ServerAddress,
    capacity: usize,
    state: Mutex<PoolState>,
    returned: Condvar,
}

struct PoolState {
    idle: Vec<TdsConnection>,
    open: usize,
}

impl TdsPool {
    /// A pool for `target`; nothing connects until the first call.
    pub fn new(target: SqlTarget, capacity: usize) -> Result<Self> {
        let address = ServerAddress::parse(&target.server)?;
        Ok(Self {
            target,
            address,
            capacity: capacity.max(1),
            state: Mutex::new(PoolState {
                idle: Vec::new(),
                open: 0,
            }),
            returned: Condvar::new(),
        })
    }

    pub fn target(&self) -> &SqlTarget {
        &self.target
    }

    pub fn address(&self) -> &ServerAddress {
        &self.address
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Runs `work` on a pooled connection. `work` must not ask the same pool
    /// for a second connection: with every connection taken it would wait
    /// for itself.
    pub fn with<R>(&self, work: impl FnOnce(&mut TdsConnection) -> Result<R>) -> Result<R> {
        let mut lease = self.checkout()?;
        let connection = lease
            .connection
            .as_mut()
            .expect("a lease holds its connection until it ends");
        let result = work(connection);
        if result.is_ok() {
            lease.keep = true;
        }
        result
    }

    /// A connection of its own for work whose session state must not reach a
    /// pooled connection: a script's batches share `USE`, `SET` and temporary
    /// tables, exactly as they do in one sqlcmd run.
    pub fn dedicated(&self) -> Result<TdsConnection> {
        TdsConnection::open(&self.target, &self.address)
    }

    /// Runs every row the query returns through `row`.
    pub fn query_each<'q>(
        &self,
        sql: &'q str,
        params: &[SqlParam<'q>],
        row: impl FnMut(Row) -> Result<()>,
    ) -> Result<()> {
        self.with(|connection| connection.query_each(sql, params, row))
    }

    /// Every row of every result set the query returns.
    pub fn query_rows(&self, sql: &str) -> Result<Vec<SqlRow>> {
        let mut rows = Vec::new();
        self.query_each(sql, &[], |row| {
            rows.push(sql_row(row));
            Ok(())
        })?;
        Ok(rows)
    }

    /// Runs statements that return nothing the caller reads.
    pub fn execute(&self, sql: &str) -> Result<u64> {
        self.with(|connection| connection.execute(sql, &[]))
    }

    /// Runs a script written for `sqlcmd -i` on a connection of its own, one
    /// batch at a time, and stops at the first batch that fails.
    pub fn run_script(&self, script: &str, variables: ScriptVariables) -> Result<()> {
        let batches = split_batches(script, variables)?;
        if batches.is_empty() {
            return Ok(());
        }
        let mut connection = self.dedicated()?;
        let count = batches.len();
        for (index, ScriptBatch { first_line, text }) in batches.into_iter().enumerate() {
            connection.run_batch(&text).with_context(|| {
                if count == 1 {
                    "the script failed".to_owned()
                } else {
                    format!(
                        "batch {} of {count} (from script line {first_line}) failed",
                        index + 1
                    )
                }
            })?;
        }
        Ok(())
    }

    fn checkout(&self) -> Result<Lease<'_>> {
        let mut state = self.lock();
        loop {
            if let Some(connection) = state.idle.pop() {
                return Ok(Lease {
                    pool: self,
                    connection: Some(connection),
                    keep: false,
                });
            }
            if state.open < self.capacity {
                state.open += 1;
                break;
            }
            state = self
                .returned
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        drop(state);
        match TdsConnection::open(&self.target, &self.address) {
            Ok(connection) => Ok(Lease {
                pool: self,
                connection: Some(connection),
                keep: false,
            }),
            Err(error) => {
                self.lock().open -= 1;
                self.returned.notify_one();
                Err(error)
            }
        }
    }

    fn lock(&self) -> MutexGuard<'_, PoolState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A pooled connection on loan; it returns (or, after a failure or a panic,
/// closes) itself when dropped.
struct Lease<'a> {
    pool: &'a TdsPool,
    connection: Option<TdsConnection>,
    keep: bool,
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        let connection = self.connection.take();
        if self.keep
            && let Some(connection) = connection
        {
            self.pool.lock().idle.push(connection);
        } else {
            // Closed outside the lock: dropping a connection closes its socket.
            drop(connection);
            self.pool.lock().open -= 1;
        }
        self.pool.returned.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use tiberius::ColumnData;
    use tiberius::numeric::Numeric;

    use super::{mentions_use, sql_value};
    use crate::sql::SqlValue;

    #[test]
    fn use_statements_are_spotted_in_any_spelling() {
        for sql in [
            "USE [db]; SELECT 1",
            "SET NOCOUNT ON; use tempdb;
SELECT 1",
            "SET NOCOUNT ON;
Use	[x]",
        ] {
            assert!(mentions_use(sql), "{sql}");
        }
        for sql in [
            "SELECT FileName FROM [db].dbo.Config",
            "SELECT reused FROM t",
            "SELECT @use FROM t",
            "SELECT DATALENGTH(BinaryData) AS use_count FROM t",
        ] {
            assert!(!mentions_use(sql), "{sql}");
        }
    }

    #[test]
    fn integers_of_every_width_read_as_i64() {
        assert_eq!(sql_value(ColumnData::U8(Some(7))), SqlValue::Int(7));
        assert_eq!(sql_value(ColumnData::I16(Some(-2))), SqlValue::Int(-2));
        assert_eq!(
            sql_value(ColumnData::I32(Some(1 << 20))),
            SqlValue::Int(1 << 20)
        );
        assert_eq!(
            sql_value(ColumnData::I64(Some(1 << 40))),
            SqlValue::Int(1 << 40)
        );
        assert_eq!(sql_value(ColumnData::Bit(Some(true))), SqlValue::Int(1));
        assert_eq!(
            sql_value(ColumnData::Numeric(Some(Numeric::new_with_scale(42, 0)))),
            SqlValue::Int(42)
        );
        assert_eq!(
            sql_value(ColumnData::Numeric(Some(Numeric::new_with_scale(425, 1)))),
            SqlValue::Other("42.5".to_owned())
        );
    }

    #[test]
    fn nulls_of_every_type_read_as_null() {
        for value in [
            ColumnData::I32(None),
            ColumnData::String(None),
            ColumnData::Binary(None),
            ColumnData::DateTime2(None),
            ColumnData::Guid(None),
            ColumnData::Numeric(None),
        ] {
            assert!(sql_value(value).is_null());
        }
    }

    #[test]
    fn text_and_binary_move_into_owned_values() {
        assert_eq!(
            sql_value(ColumnData::String(Some(Cow::Owned(
                "Конфигурация".to_owned()
            )))),
            SqlValue::Text("Конфигурация".to_owned())
        );
        assert_eq!(
            sql_value(ColumnData::Binary(Some(Cow::Owned(vec![0x0a, 0xff])))),
            SqlValue::Binary(vec![0x0a, 0xff])
        );
    }
}
