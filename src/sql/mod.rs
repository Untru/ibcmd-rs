//! Database access for every command that reads or writes an infobase.
//!
//! A command reaches its database through [`SqlExec`]. By default that is a
//! client built into the process, behind the DBMS-neutral [`SqlClient`]
//! interface: for SQL Server, the TDS protocol the 1C platform's own driver
//! (MSOLEDBSQL, loaded by `sqlsrvr.dll`) speaks, through `tiberius`, with TLS
//! through rustls and Windows logins through SSPI -- neither sqlcmd.exe,
//! bcp.exe nor an ODBC/OLE DB driver has to be installed. Everything that is
//! SQL Server's own (TDS, instance names, `GO`, `sp_executesql`, `FOR JSON`
//! rows, `varbinary(max)` writes) lives in [`mssql`]; a PostgreSQL client can
//! implement the same interface beside it.
//!
//! A command given `--sqlcmd <path>` keeps the behaviour of ibcmd-rs 0.2
//! instead ([`SqlBackend::Tools`]): statements and scripts go through that
//! sqlcmd.exe and bulk reads and writes through bcp.exe (`--bcp-executable`,
//! else the bcp.exe beside sqlcmd). The lab bundle commands
//! (`mssql-storage-*`, `mssql-delta-*`) keep bcp.exe for their native-format
//! files either way.
//!
//! The statements the commands build are still T-SQL: the row reads and the
//! `FOR JSON` statistics (`mssql.rs`, `mssql_dump/fetch.rs`,
//! `mssql_dump/cas.rs`), the staging, extension and activation scripts
//! (`tempdb` tables, `THROW`, `SYSUTCDATETIME()`, `sys.*` views) and the
//! registry and profile probes. A second DBMS needs its own statements there
//! as well as its own client.

pub mod mssql;
mod owned;
mod value;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, bail};

pub use owned::{
    InputDomain, InputRelation, InputRow, OwnedSqlCommand, OwnedSqlState, SealedSqlInput,
    StorageTable,
};
pub use value::{SqlParam, SqlRow, SqlValue};

/// The environment variable that sets how many connections a command may
/// hold open at once for parallel reads and writes (default 4).
pub const CONNECTIONS_ENV: &str = "IBCMD_RS_SQL_CONNECTIONS";
const DEFAULT_CONNECTIONS: usize = 4;
const MAX_CONNECTIONS: usize = 64;

/// A database server product.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dbms {
    SqlServer,
}

/// How a script treats the client-side variables of its command-line tool
/// (sqlcmd's `$(name)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptVariables {
    /// The tool substituted them (sqlcmd without `-x`): a script that uses
    /// one is refused, since nothing substitutes it now.
    Refuse,
    /// The tool left them alone (`sqlcmd -x`): `$(` is plain text.
    Literal,
}

/// What a command needs from a database server, whatever the DBMS.
///
/// Every method may be called from several threads at once; a client keeps
/// up to [`SqlClient::max_connections`] connections open for that. Session
/// state a request sets up (the current database, `SET` options, temporary
/// tables) ends with the request, except within [`SqlClient::run_script`].
pub trait SqlClient: Send + Sync {
    fn dbms(&self) -> Dbms;

    /// An original sequential session. Unsupported clients must refuse here,
    /// rather than substituting independent pooled execute calls.
    fn open_owned_command(&self) -> Result<OwnedSqlCommand> {
        bail!("this SQL client does not support an original owned command session")
    }

    /// Why every request fails, for a handle that reaches no database
    /// (`SqlExec::detached`); `None` for a real client.
    fn detached_reason(&self) -> Option<&'static str> {
        None
    }

    /// How many requests may run at once (parallel reads should not split
    /// into more parts than this).
    fn max_connections(&self) -> usize;

    /// Runs a script written for the DBMS's command-line tool (`sqlcmd -i`)
    /// on a session of its own, batch after batch; stops at the first error.
    fn run_script(&self, script: &str, variables: ScriptVariables) -> Result<()>;

    /// Runs statements whose results nobody reads; returns the rows they
    /// affected.
    fn execute(&self, statement: &str, params: &[SqlParam<'_>]) -> Result<u64>;

    /// Streams every row of a query to `each`, in order, as rows arrive --
    /// the read of large binary rows. An error from `each` stops the read.
    fn read_rows(
        &self,
        query: &str,
        params: &[SqlParam<'_>],
        each: &mut dyn FnMut(SqlRow) -> Result<()>,
    ) -> Result<()>;

    /// Every row of a query.
    fn query_rows(&self, query: &str, params: &[SqlParam<'_>]) -> Result<Vec<SqlRow>> {
        let mut rows = Vec::new();
        self.read_rows(query, params, &mut |row| {
            rows.push(row);
            Ok(())
        })?;
        Ok(rows)
    }

    /// The first column of the first row, if there is a row.
    fn query_scalar(&self, query: &str, params: &[SqlParam<'_>]) -> Result<Option<SqlValue>> {
        let mut first = None;
        self.read_rows(query, params, &mut |mut row| {
            if first.is_none() && !row.values.is_empty() {
                first = Some(row.values.swap_remove(0));
            }
            Ok(())
        })?;
        Ok(first)
    }

    /// The JSON document a query builds (`FOR JSON` on SQL Server); `None`
    /// when the query returned no row.
    fn query_json(&self, query: &str) -> Result<Option<String>>;

    /// Writes rows into a table in bulk, each row one value per column, in
    /// the given column order. `table` and `columns` are spelled in the
    /// DBMS's own syntax.
    fn write_rows(&self, table: &str, columns: &[&str], rows: &[Vec<SqlParam<'_>>]) -> Result<u64>;
}

/// How a command logs in.
#[derive(Clone)]
pub enum SqlLogin {
    /// The Windows account the process runs as (SSPI; sqlcmd `-E`).
    Integrated,
    /// A database login (sqlcmd `-U`; the password from `--sql-pwd` or the
    /// environment).
    Sql {
        user: String,
        password: Option<String>,
    },
}

impl SqlLogin {
    pub fn from_user(user: Option<&str>, password: Option<&str>) -> Self {
        match user {
            Some(user) => Self::Sql {
                user: user.to_owned(),
                password: password.map(ToOwned::to_owned),
            },
            None => Self::Integrated,
        }
    }

    pub fn user(&self) -> Option<&str> {
        match self {
            Self::Integrated => None,
            Self::Sql { user, .. } => Some(user),
        }
    }

    pub fn password(&self) -> Option<&str> {
        match self {
            Self::Integrated => None,
            Self::Sql { password, .. } => password.as_deref(),
        }
    }

    /// For messages; never the password.
    pub fn describe(&self) -> String {
        match self {
            Self::Integrated => "the Windows login".to_owned(),
            Self::Sql { user, .. } => format!("SQL login {user:?}"),
        }
    }
}

impl std::fmt::Debug for SqlLogin {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Integrated => formatter.write_str("Integrated"),
            Self::Sql { user, password } => formatter
                .debug_struct("Sql")
                .field("user", user)
                .field("password", &password.as_ref().map(|_| "<redacted>"))
                .finish(),
        }
    }
}

/// The server a command works against.
#[derive(Clone, Debug)]
pub struct SqlTarget {
    /// The server as given (`host`, `host\instance`, `host,port`).
    pub server: String,
    /// The database a connection starts in; the login's default otherwise.
    /// Queries name their database themselves, so this is rarely set.
    pub database: Option<String>,
    pub login: SqlLogin,
    /// Accept the server's certificate without validating it (sqlcmd `-C`):
    /// what a default SQL Server install with its self-signed certificate
    /// needs.
    pub trust_server_certificate: bool,
}

/// The external tools of the `--sqlcmd` path.
#[derive(Clone, Debug)]
pub struct SqlTools {
    pub sqlcmd: PathBuf,
    pub bcp: PathBuf,
}

impl SqlTools {
    /// `bcp` as given, else the bcp.exe beside `sqlcmd`, else `bcp` on PATH.
    pub fn new(sqlcmd: &Path, bcp: Option<&Path>) -> Self {
        Self {
            sqlcmd: sqlcmd.to_path_buf(),
            bcp: bcp.map_or_else(|| bcp_beside(sqlcmd), Path::to_path_buf),
        }
    }
}

/// The bcp.exe installed with `sqlcmd` (the ODBC client tools ship both).
pub fn bcp_beside(sqlcmd: &Path) -> PathBuf {
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

/// Which way a command reaches its database.
#[derive(Clone, Copy)]
pub enum SqlBackend<'a> {
    /// The built-in client.
    Client(&'a dyn SqlClient),
    /// sqlcmd.exe and bcp.exe (`--sqlcmd`, SQL Server only).
    Tools(&'a SqlTools),
}

/// A command's handle on its database server: the target and the way to
/// reach it. Cheap to clone; clones share the connections.
#[derive(Clone)]
pub struct SqlExec {
    inner: Arc<SqlExecInner>,
}

struct SqlExecInner {
    target: SqlTarget,
    backend: Backend,
}

enum Backend {
    Client(Box<dyn SqlClient>),
    Tools(SqlTools),
}

/// A command's SQL arguments, as the CLI spells them.
#[derive(Clone, Copy, Debug)]
pub struct SqlOptions<'a> {
    /// `--sqlcmd`: run the external tools instead of the built-in client.
    pub sqlcmd: Option<&'a Path>,
    /// `--bcp-executable`, used only with `--sqlcmd`.
    pub bcp: Option<&'a Path>,
    pub server: &'a str,
    pub user: Option<&'a str>,
    /// The resolved password (`--sql-pwd` or the environment variable).
    pub password: Option<&'a str>,
    /// Where the password was expected, for the message when it is missing.
    pub password_env: &'a str,
    pub trust_server_certificate: bool,
}

impl<'a> SqlOptions<'a> {
    /// The Windows login with the server certificate trusted: the built-in
    /// client, or sqlcmd when `sqlcmd` is given.
    pub fn integrated(server: &'a str, sqlcmd: Option<&'a Path>) -> Self {
        Self {
            sqlcmd,
            bcp: None,
            server,
            user: None,
            password: None,
            password_env: "IBCMD_DB_PSW",
            trust_server_certificate: true,
        }
    }
}

impl SqlExec {
    /// The interactive original-session capability; external tools cannot
    /// simulate it with several sqlcmd/bcp processes or independent logins.
    pub fn open_owned_command(&self) -> Result<OwnedSqlCommand> {
        match &self.inner.backend {
            Backend::Client(client) => client.open_owned_command(),
            Backend::Tools(_) => {
                bail!("external SQL tools do not support an original owned command session")
            }
        }
    }

    /// The built-in SQL Server client, or the external tools when the
    /// options name sqlcmd. Nothing connects until the first request.
    pub fn from_options(options: SqlOptions<'_>) -> Result<Self> {
        let target = SqlTarget {
            server: options.server.to_owned(),
            database: None,
            login: SqlLogin::from_user(options.user, options.password),
            trust_server_certificate: options.trust_server_certificate,
        };
        match options.sqlcmd {
            Some(sqlcmd) => Ok(Self::with_tools(target, SqlTools::new(sqlcmd, options.bcp))),
            None => {
                if let SqlLogin::Sql {
                    user,
                    password: None,
                } = &target.login
                {
                    bail!(
                        "SQL login {user:?} needs a password: pass --sql-pwd or set {}",
                        options.password_env
                    );
                }
                Self::sql_server(target)
            }
        }
    }

    /// The built-in SQL Server client for `target`.
    pub fn sql_server(target: SqlTarget) -> Result<Self> {
        let client = mssql::MssqlClient::new(target.clone(), connections_from_env())?;
        Ok(Self::with_client(target, Box::new(client)))
    }

    /// A handle whose every request fails with `reason`.
    pub fn detached(reason: &'static str) -> Self {
        let target = SqlTarget {
            server: String::new(),
            database: None,
            login: SqlLogin::Integrated,
            trust_server_certificate: false,
        };
        Self::with_client(target, Box::new(Detached { reason }))
    }

    pub fn with_client(target: SqlTarget, client: Box<dyn SqlClient>) -> Self {
        Self {
            inner: Arc::new(SqlExecInner {
                target,
                backend: Backend::Client(client),
            }),
        }
    }

    pub fn with_tools(target: SqlTarget, tools: SqlTools) -> Self {
        Self {
            inner: Arc::new(SqlExecInner {
                target,
                backend: Backend::Tools(tools),
            }),
        }
    }

    pub fn backend(&self) -> SqlBackend<'_> {
        match &self.inner.backend {
            Backend::Client(client) => SqlBackend::Client(client.as_ref()),
            Backend::Tools(tools) => SqlBackend::Tools(tools),
        }
    }

    /// The built-in client, unless the command runs the external tools.
    pub fn client(&self) -> Option<&dyn SqlClient> {
        match &self.inner.backend {
            Backend::Client(client) => Some(client.as_ref()),
            Backend::Tools(_) => None,
        }
    }

    /// Why every request of this handle fails, when it is a detached one.
    pub fn detached_reason(&self) -> Option<&'static str> {
        self.client().and_then(SqlClient::detached_reason)
    }

    /// The external tools (`--sqlcmd`), if the command runs them.
    pub fn tools(&self) -> Option<&SqlTools> {
        match &self.inner.backend {
            Backend::Client(_) => None,
            Backend::Tools(tools) => Some(tools),
        }
    }

    pub fn target(&self) -> &SqlTarget {
        &self.inner.target
    }

    pub fn server(&self) -> &str {
        &self.inner.target.server
    }

    pub fn user(&self) -> Option<&str> {
        self.inner.target.login.user()
    }

    pub fn password(&self) -> Option<&str> {
        self.inner.target.login.password()
    }

    pub fn trust_server_certificate(&self) -> bool {
        self.inner.target.trust_server_certificate
    }
}

impl std::fmt::Debug for SqlExec {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let backend = match &self.inner.backend {
            Backend::Client(client) => format!(
                "{:?} client ({} connections)",
                client.dbms(),
                client.max_connections()
            ),
            Backend::Tools(tools) => {
                format!("{} + {}", tools.sqlcmd.display(), tools.bcp.display())
            }
        };
        formatter
            .debug_struct("SqlExec")
            .field("target", &self.inner.target)
            .field("backend", &backend)
            .finish()
    }
}

/// A client that refuses every request, for work that must not reach a
/// database: a base-free stage compiles against no infobase at all.
struct Detached {
    reason: &'static str,
}

impl SqlClient for Detached {
    fn dbms(&self) -> Dbms {
        Dbms::SqlServer
    }

    fn detached_reason(&self) -> Option<&'static str> {
        Some(self.reason)
    }

    fn max_connections(&self) -> usize {
        1
    }

    fn run_script(&self, _script: &str, _variables: ScriptVariables) -> Result<()> {
        bail!("{}", self.reason)
    }

    fn execute(&self, _statement: &str, _params: &[SqlParam<'_>]) -> Result<u64> {
        bail!("{}", self.reason)
    }

    fn read_rows(
        &self,
        _query: &str,
        _params: &[SqlParam<'_>],
        _each: &mut dyn FnMut(SqlRow) -> Result<()>,
    ) -> Result<()> {
        bail!("{}", self.reason)
    }

    fn query_json(&self, _query: &str) -> Result<Option<String>> {
        bail!("{}", self.reason)
    }

    fn write_rows(
        &self,
        _table: &str,
        _columns: &[&str],
        _rows: &[Vec<SqlParam<'_>>],
    ) -> Result<u64> {
        bail!("{}", self.reason)
    }
}

fn connections_from_env() -> usize {
    std::env::var(CONNECTIONS_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .map_or(DEFAULT_CONNECTIONS, |value| value.min(MAX_CONNECTIONS))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Dbms, SqlBackend, SqlExec, SqlLogin, SqlOptions};

    fn options<'a>(sqlcmd: Option<&'a Path>, user: Option<&'a str>) -> SqlOptions<'a> {
        SqlOptions {
            sqlcmd,
            bcp: None,
            server: "sql01\\ERP,1500",
            user,
            password: None,
            password_env: "TEST_SQL_PASSWORD",
            trust_server_certificate: true,
        }
    }

    #[test]
    fn the_built_in_client_is_the_default_and_opens_nothing_up_front() {
        let sql = SqlExec::from_options(options(None, None)).unwrap();
        let SqlBackend::Client(client) = sql.backend() else {
            panic!("the built-in client is the default");
        };
        assert_eq!(client.dbms(), Dbms::SqlServer);
        assert!(sql.tools().is_none());
        assert_eq!(sql.server(), "sql01\\ERP,1500");
        assert_eq!(sql.user(), None);
    }

    #[test]
    fn sqlcmd_selects_the_external_tools_with_bcp_beside_it() {
        let sql =
            SqlExec::from_options(options(Some(Path::new("no/such/dir/sqlcmd")), None)).unwrap();
        let tools = sql.tools().expect("--sqlcmd keeps the external tools");
        assert!(sql.client().is_none());
        assert_eq!(tools.sqlcmd, Path::new("no/such/dir/sqlcmd"));
        assert_eq!(tools.bcp, Path::new("bcp"));
        let explicit = SqlOptions {
            bcp: Some(Path::new("C:/tools/bcp.exe")),
            ..options(Some(Path::new("sqlcmd")), None)
        };
        let sql = SqlExec::from_options(explicit).unwrap();
        assert_eq!(sql.tools().unwrap().bcp, Path::new("C:/tools/bcp.exe"));
    }

    #[test]
    fn a_sql_login_without_password_is_refused_before_connecting() {
        let error = SqlExec::from_options(options(None, Some("ibcmd")))
            .unwrap_err()
            .to_string();
        assert!(error.contains("TEST_SQL_PASSWORD"), "{error}");
        // sqlcmd asked for the password itself, so --sqlcmd keeps accepting it.
        assert!(SqlExec::from_options(options(Some(Path::new("sqlcmd")), Some("ibcmd"))).is_ok());
    }

    #[test]
    fn an_unparsable_server_fails_only_the_built_in_client() {
        let bad = SqlOptions {
            server: "np:\\\\.\\pipe\\sql\\query",
            ..options(None, None)
        };
        assert!(SqlExec::from_options(bad).is_err());
        let with_sqlcmd = SqlOptions {
            sqlcmd: Some(Path::new("sqlcmd")),
            ..bad
        };
        assert!(SqlExec::from_options(with_sqlcmd).is_ok());
    }

    #[test]
    fn a_detached_handle_refuses_every_request() {
        let sql = SqlExec::detached("no database here");
        let client = sql.client().unwrap();
        let error = client
            .query_scalar("SELECT 1", &[])
            .unwrap_err()
            .to_string();
        assert_eq!(error, "no database here");
        assert!(client.execute("SELECT 1", &[]).is_err());
        assert!(client.write_rows("t", &["a"], &[]).is_err());
    }

    #[test]
    fn passwords_never_reach_debug_output() {
        let login = SqlLogin::from_user(Some("ibcmd"), Some("s3cret-value"));
        let rendered = format!("{login:?} {}", login.describe());
        assert!(!rendered.contains("s3cret-value"), "{rendered}");
        let sql = SqlExec::from_options(SqlOptions {
            password: Some("s3cret-value"),
            ..options(None, Some("ibcmd"))
        })
        .unwrap();
        assert!(!format!("{sql:?}").contains("s3cret-value"));
    }
}
