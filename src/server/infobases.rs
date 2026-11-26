//! The infobases an editor can open: the `[[database]]` entries of the
//! settings files (`crate::settings`, highest layer first) and the ones the
//! editor names in `initialize`, each with the platform it runs on and where
//! its Config table is read from -- a SQL Server database, or a folder of
//! stored rows (`rows-dir`, the offline source of `mssql-dump-config
//! --rows-dir`).
//!
//! A login's password reaches the server only in memory (`infobases/connect`
//! from the editor's secret storage), else from `IBCMD_DB_PSW` as on the
//! command line: never from an argument or a file. It is kept in a [`Secret`],
//! which prints as `***` and is overwritten when dropped.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::platform::PlatformSpec;
use crate::settings::{
    DatabaseTarget, ENV_DB_PASSWORD, PlatformHint, SettingSource, Settings,
    resolve_platform_with_source,
};
use crate::sql::{SqlExec, SqlOptions};
use crate::stored_objects::{ConfigSource, StoredConfiguration};

/// A password held in memory only.
#[derive(Clone, Default)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(mask: bool) -> Entry {
        let name = if mask { "demo_*" } else { "demo" };
        let spec = crate::platform::parse_flag("8.3.27").unwrap();
        Entry {
            id: entry_id(Some("sql01"), name),
            name: name.to_string(),
            server: Some("sql01".to_string()),
            source: "mssql",
            rows_dir: None,
            db_path: None,
            platform: spec.to_string(),
            xml_version: spec.xml_version().as_str().to_string(),
            platform_source: "test".to_string(),
            mask,
            origin: "test".to_string(),
            spec,
        }
    }

    #[test]
    fn editor_sql_sessions_validate_server_certificates_for_both_auth_modes() {
        for login in [
            Login {
                auth: Some(AuthMode::Windows),
                ..Login::default()
            },
            Login {
                auth: Some(AuthMode::Sql),
                user: Some("fixture-user".into()),
                password: Some(Secret::new("fixture-password".into())),
            },
        ] {
            let session = Session::open(&entry(false), None, login, &Settings::default()).unwrap();
            let ConfigSource::Database { sql, .. } = session.configuration.source() else {
                panic!("SQL session expected");
            };
            assert!(!sql.trust_server_certificate());
        }
    }

    #[test]
    fn explicit_auth_does_not_inherit_a_global_sql_user() {
        let env =
            |key: &str| (key == crate::settings::ENV_DB_USER).then(|| "global-user".to_string());
        let settings = Settings::from_sources(&crate::settings::SettingsSources {
            env: &env,
            executable: None,
            current_dir: None,
            app_data: None,
            native_config: None,
        })
        .unwrap();
        assert!(settings.db_user().is_some());
        let legacy = Session::open(
            &entry(false),
            None,
            Login {
                password: Some(Secret::new("test-password".into())),
                ..Login::default()
            },
            &settings,
        )
        .unwrap();
        assert_eq!(legacy.login.user.as_deref(), Some("global-user"));
        let windows = Session::open(
            &entry(false),
            None,
            Login {
                auth: Some(AuthMode::Windows),
                ..Login::default()
            },
            &settings,
        )
        .unwrap();
        assert!(windows.login.user.is_none());
        assert!(windows.login.password.is_none());
        assert!(
            Session::open(
                &entry(false),
                None,
                Login {
                    auth: Some(AuthMode::Sql),
                    ..Login::default()
                },
                &settings
            )
            .is_err()
        );
        assert!(
            Session::open(
                &entry(false),
                None,
                Login {
                    auth: Some(AuthMode::Windows),
                    user: Some("unexpected".into()),
                    ..Login::default()
                },
                &settings
            )
            .is_err()
        );
    }

    #[test]
    fn a_fixed_infobase_cannot_be_redirected_to_another_database() {
        let result = Session::open(
            &entry(false),
            Some("other"),
            Login::default(),
            &Settings::default(),
        );
        assert!(
            result.is_err(),
            "opened a different database under the fixed id"
        );
    }

    #[test]
    fn a_fixed_infobase_can_repeat_its_database_name() {
        let session = Session::open(
            &entry(false),
            Some("demo"),
            Login::default(),
            &Settings::default(),
        )
        .unwrap();
        assert_eq!(session.entry.id, "sql01/demo");
        assert_eq!(session.database, "demo");
    }

    #[test]
    fn a_mask_opens_the_database_under_its_own_id() {
        let session = Session::open(
            &entry(true),
            Some("demo_test"),
            Login::default(),
            &Settings::default(),
        )
        .unwrap();
        assert_eq!(session.entry.id, "sql01/demo_test");
        assert_eq!(session.database, "demo_test");
        assert!(!session.entry.mask);
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("***")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        // Overwrites the bytes before the allocation is freed; `black_box`
        // keeps the compiler from dropping the writes as dead.
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        bytes.iter_mut().for_each(|byte| *byte = 0);
        std::hint::black_box(&bytes);
    }
}

/// One infobase the editor may open.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// What the other methods take: `server/name`, or `name` without a server.
    pub id: String,
    /// The database name (a mask with `*` for a settings entry that names
    /// many), or the label of a rows folder.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// `mssql` or `rows-dir`.
    pub source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows_dir: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_path: Option<PathBuf>,
    /// The platform (`8.3.27.2214`) and the XML format it reads and writes.
    pub platform: String,
    pub xml_version: String,
    /// Where the platform came from.
    pub platform_source: String,
    /// The entry names many databases (`bsp_*`): `infobases/connect` with
    /// `database` opens one of them.
    pub mask: bool,
    /// `<settings file>:<line>` or `initialize`.
    pub origin: String,
    #[serde(skip)]
    pub spec: PlatformSpec,
}

/// An infobase named in `initialize`.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntryParams {
    pub id: Option<String>,
    pub name: String,
    pub server: Option<String>,
    pub rows_dir: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_path: Option<PathBuf>,
    pub platform: Option<String>,
}

fn entry_id(server: Option<&str>, name: &str) -> String {
    match server {
        Some(server) => format!("{server}/{name}"),
        None => name.to_string(),
    }
}

fn source_text(source: &SettingSource) -> String {
    source.to_string()
}

/// The entries of the settings, then the editor's own (which replace a
/// settings entry of the same id).
pub fn entries(settings: &Settings, extra: &[EntryParams]) -> Result<Vec<Entry>> {
    let mut entries = BTreeMap::<String, Entry>::new();
    let mut order = Vec::<String>::new();
    for file in settings.files().iter().rev() {
        for binding in file.databases() {
            let id = entry_id(binding.server.as_deref(), &binding.name);
            if entries.contains_key(&id) {
                continue;
            }
            order.push(id.clone());
            entries.insert(
                id.clone(),
                Entry {
                    id,
                    name: binding.name.clone(),
                    server: binding.server.clone(),
                    source: if binding.db_path.is_some() {
                        "file-db"
                    } else if binding.rows_dir.is_some() {
                        "rows-dir"
                    } else {
                        "mssql"
                    },
                    rows_dir: binding.rows_dir.clone(),
                    db_path: binding.db_path.clone(),
                    platform: binding.platform.to_string(),
                    xml_version: binding.platform.xml_version().as_str().to_string(),
                    platform_source: format!("{}:{}", file.path.display(), binding.line),
                    mask: binding.name.contains('*'),
                    origin: format!("{}:{}", file.path.display(), binding.line),
                    spec: binding.platform,
                },
            );
        }
    }
    for params in extra {
        let name = params.name.trim();
        if name.is_empty() {
            return Err(anyhow!("an infobase of initialize has an empty name"));
        }
        if params.db_path.is_some()
            && (params.rows_dir.is_some() || params.server.is_some() || name.contains('*'))
        {
            return Err(anyhow!(
                "dbPath cannot be combined with rowsDir, server or a database mask"
            ));
        }
        if params
            .db_path
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err(anyhow!("dbPath is empty"));
        }
        let server = params
            .server
            .as_deref()
            .map(str::trim)
            .filter(|server| !server.is_empty());
        let id = params
            .id
            .clone()
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(|| entry_id(server, name));
        let platform = resolve_platform_with_source(
            params.platform.as_deref(),
            settings,
            DatabaseTarget::new(server, name),
            PlatformHint::None,
        )
        .with_context(|| format!("the platform of infobase {id}"))?;
        if !entries.contains_key(&id) {
            order.push(id.clone());
        }
        entries.insert(
            id.clone(),
            Entry {
                id,
                name: name.to_string(),
                server: server.map(str::to_string),
                source: if params.db_path.is_some() {
                    "file-db"
                } else if params.rows_dir.is_some() {
                    "rows-dir"
                } else {
                    "mssql"
                },
                rows_dir: params.rows_dir.clone(),
                db_path: params.db_path.clone(),
                platform: platform.value.to_string(),
                xml_version: platform.value.xml_version().as_str().to_string(),
                platform_source: if params.platform.is_some() {
                    "initialize".to_string()
                } else {
                    source_text(&platform.source)
                },
                mask: name.contains('*') && params.rows_dir.is_none(),
                origin: "initialize".to_string(),
                spec: platform.value,
            },
        );
    }
    Ok(order
        .into_iter()
        .filter_map(|id| entries.remove(&id))
        .collect())
}

/// Explicit editor authentication; absence preserves the legacy settings fallback.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    Windows,
    Sql,
}

/// A login for a SQL Server infobase.
#[derive(Clone, Debug, Default)]
pub struct Login {
    pub auth: Option<AuthMode>,
    pub user: Option<String>,
    pub password: Option<Secret>,
}

/// An open infobase: its entry, its login and what has been read of it.
pub struct Session {
    pub entry: Entry,
    /// The database a SQL Server session reads (the entry's name, or the
    /// one `infobases/connect` named for a mask).
    pub database: String,
    pub server: String,
    pub login: Login,
    pub configuration: StoredConfiguration,
}

impl Session {
    /// Opens `entry` (`database` names one database of a mask). Nothing is
    /// read yet; a SQL Server handle connects on its first query.
    pub fn open(
        entry: &Entry,
        database: Option<&str>,
        login: Login,
        settings: &Settings,
    ) -> Result<Self> {
        if entry.db_path.is_some() {
            if entry.source != "file-db"
                || entry.rows_dir.is_some()
                || entry.server.is_some()
                || entry.mask
            {
                return Err(anyhow!(
                    "dbPath requires source=file-db, no rowsDir/server and a fixed database"
                ));
            }
        } else if entry.source == "file-db" {
            return Err(anyhow!("file-db requires dbPath"));
        }
        let database = match (
            entry.mask,
            database.map(str::trim).filter(|name| !name.is_empty()),
        ) {
            (true, None) => {
                return Err(anyhow!(
                    "infobase {} is a mask ({}); name the database with `database`",
                    entry.id,
                    entry.name
                ));
            }
            (false, Some(database)) if database != entry.name => {
                return Err(anyhow!(
                    "infobase {} names database {}; database cannot redirect a fixed infobase to {database}",
                    entry.id,
                    entry.name
                ));
            }
            (_, Some(database)) => database.to_string(),
            (false, None) => entry.name.clone(),
        };
        if let Some(path) = &entry.db_path {
            if login.auth.is_some() || login.user.is_some() || login.password.is_some() {
                return Err(anyhow!("file-db does not accept SQL authentication"));
            }
            let path = std::fs::canonicalize(ibcmd_filedb::data_file(path))
                .with_context(|| format!("dbPath of infobase {} does not exist", entry.id))?;
            // Validate the actual database, without opening SQL or inheriting its credentials.
            ibcmd_filedb::FileDb::open(&path).map_err(|error| anyhow!("{error}"))?;
            let mut entry = entry.clone();
            entry.db_path = Some(path.clone());
            return Ok(Self {
                configuration: StoredConfiguration::new(ConfigSource::FileDb(path), entry.spec),
                entry,
                database,
                server: String::new(),
                login: Login::default(),
            });
        }
        let server = entry
            .server
            .clone()
            .or_else(|| settings.db_server().map(|server| server.value))
            .unwrap_or_else(|| "localhost".to_string());
        let mut entry = entry.clone();
        if entry.mask {
            entry.id = entry_id(entry.server.as_deref(), &database);
            entry.name = database.clone();
            entry.mask = false;
        }
        if login.auth == Some(AuthMode::Windows)
            && (login.user.is_some() || login.password.is_some())
        {
            return Err(anyhow!(
                "Windows authentication does not accept user or password"
            ));
        }
        if login.auth == Some(AuthMode::Sql)
            && login.user.as_deref().is_none_or(|v| v.trim().is_empty())
        {
            return Err(anyhow!("SQL authentication requires a user"));
        }
        let explicit = login.auth.is_some();
        let login = Login {
            auth: login.auth,
            user: login.user.or_else(|| {
                (!explicit)
                    .then(|| settings.db_user().map(|user| user.value))
                    .flatten()
            }),
            password: login.password.or_else(|| {
                if explicit {
                    return None;
                }
                std::env::var(ENV_DB_PASSWORD)
                    .ok()
                    .filter(|value| !value.is_empty())
                    .map(Secret::new)
            }),
        };
        let source = match &entry.rows_dir {
            Some(dir) => {
                if !dir.is_dir() {
                    return Err(anyhow!(
                        "the rows folder of infobase {} does not exist: {}",
                        entry.id,
                        dir.display()
                    ));
                }
                ConfigSource::RowsDir(dir.clone())
            }
            None => ConfigSource::Database {
                sql: SqlExec::from_options(SqlOptions {
                    sqlcmd: None,
                    bcp: None,
                    server: &server,
                    user: login.user.as_deref(),
                    password: login.password.as_ref().map(Secret::expose),
                    password_env: ENV_DB_PASSWORD,
                    trust_server_certificate: false,
                })?,
                database: database.clone(),
            },
        };
        Ok(Self {
            configuration: StoredConfiguration::new(source, entry.spec),
            entry,
            database,
            server,
            login,
        })
    }
}

#[cfg(test)]
mod file_source_contract_tests {
    use super::*;

    #[test]
    fn conflicting_file_sources_fail_before_opening_a_database() {
        for extra in [
            r#"{"name":"demo","dbPath":"missing/1Cv8.1CD","rowsDir":"missing/rows"}"#,
            r#"{"name":"demo","dbPath":"missing/1Cv8.1CD","server":"sql01"}"#,
            r#"{"name":"demo_*","dbPath":"missing/1Cv8.1CD"}"#,
        ] {
            let params: EntryParams = serde_json::from_str(extra).unwrap();
            let error = entries(&Settings::default(), &[params]).unwrap_err();
            assert!(error.to_string().contains("dbPath"), "{error:#}");
        }
    }
}
