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
            platform: spec.to_string(),
            xml_version: spec.xml_version().as_str().to_string(),
            platform_source: "test".to_string(),
            mask,
            origin: "test".to_string(),
            spec,
        }
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
                    source: if binding.rows_dir.is_some() {
                        "rows-dir"
                    } else {
                        "mssql"
                    },
                    rows_dir: binding.rows_dir.clone(),
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
                source: if params.rows_dir.is_some() {
                    "rows-dir"
                } else {
                    "mssql"
                },
                rows_dir: params.rows_dir.clone(),
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

/// A login for a SQL Server infobase.
#[derive(Clone, Debug, Default)]
pub struct Login {
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
        let login = Login {
            user: login
                .user
                .or_else(|| settings.db_user().map(|user| user.value)),
            password: login.password.or_else(|| {
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
                    trust_server_certificate: true,
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
