//! Settings: what a command takes when a flag does not say it (#332), and
//! which platform each database runs on (#366).
//!
//! # Where a value comes from
//!
//! Highest first:
//!
//! 1. the command-line flag;
//! 2. the platform only: the `[[database]]` entry of the settings files that
//!    matches the command's database (the first match in file order, the
//!    highest layer's file searched first) -- deliberately above the
//!    environment, so that one variable cannot silently override every
//!    binding;
//! 3. the environment: `IBCMD_RS_PLATFORM`, `IBCMD_RS_DB_SERVER`,
//!    `IBCMD_RS_DB_USER` and `IBCMD_DB_PSW` (the password);
//! 4. the settings files: the top-level `platform`, `db-server` and
//!    `db-user` of `ibcmd-rs.toml`;
//! 5. the `database:` section of native ibcmd's own config file, the YAML
//!    `ibcmd server config init` writes, passed as `--config`: `server`,
//!    `name`, `user`, `password`, `dbms`;
//! 6. the platform only, auto-detection: an executable in
//!    `...\1cv8\<version>\bin` runs as that version; a load reads the XML
//!    format of the tree's `Configuration.xml` (2.20 -> 8.3.27, 2.21 ->
//!    8.5.1); an export of a configuration kept in compatibility 8.5 or later
//!    is 8.5.1's;
//! 7. the platform only: 8.3.27, with a warning on stderr.
//!
//! The database itself cannot tell 8.3.27 from 8.5 -- `IBVersion` and
//! `Params` are identical, and ERP УХ runs on 8.5 in 8.3.27 compatibility --
//! so nothing but the compatibility rule is inferred from it.
//!
//! # Settings files
//!
//! `IBCMD_RS_CONFIG`, when set, names the one file read, which must exist.
//! Otherwise up to three layers are read, a later one winning: `ibcmd-rs.toml`
//! next to the executable, `%APPDATA%\ibcmd-rs\ibcmd-rs.toml`, and
//! `ibcmd-rs.toml` in the current directory (a project can keep one in git).
//! A malformed file, an unknown key or a platform the registry does not know
//! fails when the settings are read, naming the file and the line. A
//! password is never read from these files: `IBCMD_DB_PSW` or native
//! `--config` carry it.
//!
//! ```toml
//! platform = "8.3.27.2214"       # for the databases no entry names
//! db-server = "sql01"
//! db-user = "sa"
//!
//! [[database]]
//! name = "erp_prod"              # a database name, or a mask with `*`
//! platform = "8.3.27.2214"
//!
//! [[database]]
//! server = "sql02"               # optional: any server when absent
//! name = "bsp_*"
//! platform = "8.5.1.1150"
//! ```
//!
//! # For commands
//!
//! [`Settings::load`] gathers the settings once, [`resolve_platform`] picks
//! the platform of one database: the explicit `--platform` value, the
//! database the command targets ([`DatabaseTarget`], the server and name
//! the command resolved from its flags and [`Settings::db_server`] /
//! [`Settings::db_name`]), and what auto-detection may look at
//! ([`PlatformHint`]). [`resolve_platform_with_source`] also tells where the
//! answer came from and prints nothing.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::legacy_version::InfobaseConfigSourceVersion;
use crate::platform::PlatformSpec;

pub mod commands;
mod files;
mod restructure;
pub mod show;

/// The platform of the databases nothing else names.
pub const ENV_PLATFORM: &str = "IBCMD_RS_PLATFORM";
/// The SQL Server a command connects to when no flag names one.
pub const ENV_DB_SERVER: &str = "IBCMD_RS_DB_SERVER";
/// The SQL Server login when no flag names one.
pub const ENV_DB_USER: &str = "IBCMD_RS_DB_USER";
/// The SQL Server password (the variable the commands already read).
pub const ENV_DB_PASSWORD: &str = "IBCMD_DB_PSW";
/// The one settings file to read instead of the layers.
pub const ENV_CONFIG: &str = "IBCMD_RS_CONFIG";
/// The most rows of rebuilt tables a restructuring may hold (S1-J, settings/restructure.rs).
pub const ENV_RESTRUCTURE_LIMIT_ROWS: &str = "IBCMD_RS_RESTRUCTURE_LIMIT_ROWS";
/// The most bytes a restructuring may write (the data of the rebuilt tables twice, their other indexes once).
pub const ENV_RESTRUCTURE_LIMIT_BYTES: &str = "IBCMD_RS_RESTRUCTURE_LIMIT_BYTES";
/// The settings file name of every layer.
pub const SETTINGS_FILE_NAME: &str = "ibcmd-rs.toml";
/// The platform assumed, with a warning, when nothing names one.
pub const DEFAULT_PLATFORM: &str = "8.3.27";

/// Where one value came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SettingSource {
    /// A command-line flag.
    Flag { flag: String },
    /// A `[[database]]` entry of a settings file.
    Database {
        path: PathBuf,
        line: usize,
        server: Option<String>,
        name: String,
    },
    /// An environment variable.
    Environment { variable: String },
    /// A top-level key of a settings file.
    File { path: PathBuf, line: usize },
    /// The `database:` section of native ibcmd's config file.
    NativeConfig { path: PathBuf, key: String },
    /// The executable lies in a platform's `bin` folder.
    ExecutableDirectory { path: PathBuf },
    /// The XML format the source tree's `Configuration.xml` declares.
    SourceTree { path: PathBuf },
    /// The configuration is kept in compatibility 8.5 or later.
    Compatibility,
    /// Nothing named it.
    Default,
}

impl fmt::Display for SettingSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flag { flag } => write!(formatter, "{flag}"),
            Self::Database {
                path,
                line,
                server,
                name,
            } => {
                write!(formatter, "[[database]] name = \"{name}\"")?;
                if let Some(server) = server {
                    write!(formatter, ", server = \"{server}\"")?;
                }
                write!(formatter, " ({}:{line})", path.display())
            }
            Self::Environment { variable } => write!(formatter, "{variable}"),
            Self::File { path, line } => write!(formatter, "{}:{line}", path.display()),
            Self::NativeConfig { path, key } => {
                write!(formatter, "database.{key} in {}", path.display())
            }
            Self::ExecutableDirectory { path } => {
                write!(formatter, "the executable's folder {}", path.display())
            }
            Self::SourceTree { path } => {
                write!(formatter, "the XML format of {}", path.display())
            }
            Self::Compatibility => {
                formatter.write_str("the configuration's compatibility mode (8.5 or later)")
            }
            Self::Default => formatter.write_str("the default"),
        }
    }
}

/// A value and where it came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Sourced<T> {
    pub value: T,
    pub source: SettingSource,
}

impl<T> Sourced<T> {
    fn new(value: T, source: SettingSource) -> Self {
        Self { value, source }
    }
}

/// The database a command works with, for the `[[database]]` entries.
///
/// `server` is compared with an entry's `server` case-insensitively and as
/// written (`localhost` and `.` are different names); a target without a
/// server matches only the entries without one. `name` is matched against
/// the entry's `name`, where `*` stands for any run of characters, also
/// case-insensitively (as SQL Server compares database names).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DatabaseTarget<'a> {
    pub server: Option<&'a str>,
    pub name: &'a str,
}

impl<'a> DatabaseTarget<'a> {
    /// A target, or `None` when the command names no database.
    pub fn new(server: Option<&'a str>, name: &'a str) -> Option<Self> {
        let name = name.trim();
        (!name.is_empty()).then(|| Self {
            server: server.map(str::trim).filter(|server| !server.is_empty()),
            name,
        })
    }
}

/// What auto-detection may look at when nothing names the platform.
#[derive(Clone, Copy)]
pub enum PlatformHint<'a> {
    /// Nothing but the executable's folder.
    None,
    /// An export: a configuration kept in compatibility 8.5 or later can only
    /// be 8.5.1's. The probe runs only when nothing above auto-detection
    /// decides, so an export that names its platform reads nothing for it.
    Export {
        compatibility_8_5_or_later: &'a dyn Fn() -> Result<bool>,
    },
    /// A load: the tree's `Configuration.xml` declares its XML format.
    Import { source_root: &'a Path },
}

impl fmt::Debug for PlatformHint<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => formatter.write_str("None"),
            Self::Export { .. } => formatter.write_str("Export"),
            Self::Import { source_root } => formatter
                .debug_struct("Import")
                .field("source_root", source_root)
                .finish(),
        }
    }
}

/// Where a settings file sits among the layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SettingsLayer {
    /// The one file `IBCMD_RS_CONFIG` names.
    Explicit,
    /// Next to the executable.
    NextToExecutable,
    /// `%APPDATA%\ibcmd-rs\`.
    AppData,
    /// The current directory.
    CurrentDirectory,
}

impl fmt::Display for SettingsLayer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Explicit => ENV_CONFIG,
            Self::NextToExecutable => "next to the executable",
            Self::AppData => "%APPDATA%\\ibcmd-rs",
            Self::CurrentDirectory => "the current directory",
        })
    }
}

/// A value of a settings file with its line.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Located<T> {
    value: T,
    line: usize,
}

/// One `[[database]]` entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatabaseBinding {
    pub server: Option<String>,
    pub name: String,
    pub platform: PlatformSpec,
    /// The line of the entry's `platform`.
    pub line: usize,
    /// `rows-dir`: the Config table of this infobase is read from a folder of
    /// stored rows (`mssql-dump-config --rows-dir`) instead of SQL Server; a
    /// relative path is the settings file's folder's. Only the editor server
    /// (`serve --stdio`) reads it.
    pub rows_dir: Option<PathBuf>,
}

impl DatabaseBinding {
    /// Whether the entry names this database.
    pub fn matches(&self, target: DatabaseTarget<'_>) -> bool {
        let server_matches = match (&self.server, target.server) {
            (None, _) => true,
            (Some(expected), Some(actual)) => expected.eq_ignore_ascii_case(actual),
            (Some(_), None) => false,
        };
        server_matches && mask_matches(&self.name, target.name)
    }
}

/// `*` stands for any run of characters; everything else must be equal,
/// ignoring case.
fn mask_matches(mask: &str, name: &str) -> bool {
    let mask = mask.to_lowercase().chars().collect::<Vec<_>>();
    let name = name.to_lowercase().chars().collect::<Vec<_>>();
    // Greedy match with one backtrack point per star.
    let (mut m, mut n) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while n < name.len() {
        if m < mask.len() && mask[m] == '*' {
            star = Some((m, n));
            m += 1;
        } else if m < mask.len() && mask[m] == name[n] {
            m += 1;
            n += 1;
        } else if let Some((star_m, star_n)) = star {
            m = star_m + 1;
            n = star_n + 1;
            star = Some((star_m, star_n + 1));
        } else {
            return false;
        }
    }
    mask[m..].iter().all(|&character| character == '*')
}

/// One settings file that was read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsFile {
    pub path: PathBuf,
    pub layer: SettingsLayer,
    platform: Option<Located<PlatformSpec>>,
    db_server: Option<Located<String>>,
    db_user: Option<Located<String>>,
    restructure_limit_rows: Option<Located<u64>>,
    restructure_limit_bytes: Option<Located<u64>>,
    databases: Vec<DatabaseBinding>,
}

impl SettingsFile {
    /// The `[[database]]` entries in file order.
    pub fn databases(&self) -> &[DatabaseBinding] {
        &self.databases
    }
}

/// The `database:` section of native ibcmd's config file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeConfig {
    pub path: PathBuf,
    pub dbms: Option<String>,
    pub server: Option<String>,
    pub name: Option<String>,
    pub user: Option<String>,
    pub password: Option<String>,
}

/// What [`Settings::from_sources`] reads, so that tests can supply it.
pub struct SettingsSources<'a> {
    /// Reads an environment variable.
    pub env: &'a dyn Fn(&str) -> Option<String>,
    /// The running executable.
    pub executable: Option<PathBuf>,
    /// The current directory.
    pub current_dir: Option<PathBuf>,
    /// `%APPDATA%`.
    pub app_data: Option<PathBuf>,
    /// Native ibcmd's config file (`--config`).
    pub native_config: Option<&'a Path>,
}

/// The settings a command reads besides its flags.
#[derive(Clone, Debug, Default)]
pub struct Settings {
    /// Files read, the lowest layer first.
    files: Vec<SettingsFile>,
    /// Layers looked for and absent.
    absent: Vec<(PathBuf, SettingsLayer)>,
    /// The variables above that are set to a non-empty value.
    env: BTreeMap<&'static str, String>,
    env_platform: Option<PlatformSpec>,
    native: Option<NativeConfig>,
    executable: Option<PathBuf>,
}

impl Settings {
    /// Reads the environment, the settings files and native `--config`.
    pub fn load(native_config: Option<&Path>) -> Result<Self> {
        let env = |name: &str| std::env::var(name).ok();
        Self::from_sources(&SettingsSources {
            env: &env,
            executable: std::env::current_exe().ok(),
            current_dir: std::env::current_dir().ok(),
            app_data: std::env::var_os("APPDATA").map(PathBuf::from),
            native_config,
        })
    }

    /// As [`Settings::load`], from explicit sources.
    pub fn from_sources(sources: &SettingsSources<'_>) -> Result<Self> {
        let mut env = BTreeMap::new();
        for variable in [
            ENV_PLATFORM,
            ENV_DB_SERVER,
            ENV_DB_USER,
            ENV_DB_PASSWORD,
            ENV_CONFIG,
            ENV_RESTRUCTURE_LIMIT_ROWS,
            ENV_RESTRUCTURE_LIMIT_BYTES,
        ] {
            if let Some(value) = (sources.env)(variable).filter(|value| !value.trim().is_empty()) {
                env.insert(variable, value);
            }
        }
        let env_platform = env
            .get(ENV_PLATFORM)
            .map(|value| {
                crate::platform::parse(value)
                    .with_context(|| format!("{ENV_PLATFORM}=`{}`", value.trim()))
            })
            .transpose()?;

        let mut files = Vec::new();
        let mut absent = Vec::new();
        if let Some(path) = env.get(ENV_CONFIG) {
            let path = PathBuf::from(path.trim());
            if !path.is_file() {
                bail!("{ENV_CONFIG} names {}, which is not a file", path.display());
            }
            files.push(files::read_settings_file(&path, SettingsLayer::Explicit)?);
        } else {
            let layers = [
                (
                    sources
                        .executable
                        .as_deref()
                        .and_then(Path::parent)
                        .map(|dir| dir.join(SETTINGS_FILE_NAME)),
                    SettingsLayer::NextToExecutable,
                ),
                (
                    sources
                        .app_data
                        .as_deref()
                        .map(|dir| dir.join("ibcmd-rs").join(SETTINGS_FILE_NAME)),
                    SettingsLayer::AppData,
                ),
                (
                    sources
                        .current_dir
                        .as_deref()
                        .map(|dir| dir.join(SETTINGS_FILE_NAME)),
                    SettingsLayer::CurrentDirectory,
                ),
            ];
            let mut seen = Vec::<PathBuf>::new();
            for (path, layer) in layers {
                let Some(path) = path else {
                    continue;
                };
                // The current directory may be the executable's own.
                let key = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
                if seen.contains(&key) {
                    continue;
                }
                seen.push(key);
                if path.is_file() {
                    files.push(files::read_settings_file(&path, layer)?);
                } else {
                    absent.push((path, layer));
                }
            }
        }
        let native = sources
            .native_config
            .map(files::read_native_config)
            .transpose()?;
        Ok(Self {
            files,
            absent,
            env,
            env_platform,
            native,
            executable: sources.executable.clone(),
        })
    }

    /// The settings files read, the lowest layer first.
    pub fn files(&self) -> &[SettingsFile] {
        &self.files
    }

    /// The layers looked for that do not exist.
    pub fn absent_files(&self) -> &[(PathBuf, SettingsLayer)] {
        &self.absent
    }

    /// Native ibcmd's config, when one was given.
    pub fn native_config(&self) -> Option<&NativeConfig> {
        self.native.as_ref()
    }

    fn env_value(&self, variable: &'static str) -> Option<Sourced<String>> {
        self.env.get(variable).map(|value| {
            Sourced::new(
                value.clone(),
                SettingSource::Environment {
                    variable: variable.to_string(),
                },
            )
        })
    }

    /// A top-level value of the highest layer that sets it.
    fn file_value(
        &self,
        pick: impl Fn(&SettingsFile) -> Option<&Located<String>>,
    ) -> Option<Sourced<String>> {
        self.files.iter().rev().find_map(|file| {
            pick(file).map(|located| {
                Sourced::new(
                    located.value.clone(),
                    SettingSource::File {
                        path: file.path.clone(),
                        line: located.line,
                    },
                )
            })
        })
    }

    fn native_value(
        &self,
        key: &str,
        pick: impl Fn(&NativeConfig) -> Option<&String>,
    ) -> Option<Sourced<String>> {
        let native = self.native.as_ref()?;
        pick(native).map(|value| {
            Sourced::new(
                value.clone(),
                SettingSource::NativeConfig {
                    path: native.path.clone(),
                    key: key.to_string(),
                },
            )
        })
    }

    /// The SQL Server: `IBCMD_RS_DB_SERVER`, `db-server` of the settings
    /// files, `database.server` of native `--config`.
    pub fn db_server(&self) -> Option<Sourced<String>> {
        self.env_value(ENV_DB_SERVER)
            .or_else(|| self.file_value(|file| file.db_server.as_ref()))
            .or_else(|| self.native_value("server", |native| native.server.as_ref()))
    }

    /// The SQL Server login: `IBCMD_RS_DB_USER`, `db-user` of the settings
    /// files, `database.user` of native `--config`.
    pub fn db_user(&self) -> Option<Sourced<String>> {
        self.env_value(ENV_DB_USER)
            .or_else(|| self.file_value(|file| file.db_user.as_ref()))
            .or_else(|| self.native_value("user", |native| native.user.as_ref()))
    }

    /// The SQL Server password: `IBCMD_DB_PSW`, `database.password` of native
    /// `--config` (never a settings file).
    pub fn db_password(&self) -> Option<Sourced<String>> {
        self.env_value(ENV_DB_PASSWORD)
            .or_else(|| self.native_value("password", |native| native.password.as_ref()))
    }

    /// The database name: `database.name` of native `--config`.
    pub fn db_name(&self) -> Option<Sourced<String>> {
        self.native_value("name", |native| native.name.as_ref())
    }

    /// The DBMS: `database.dbms` of native `--config`.
    pub fn dbms(&self) -> Option<Sourced<String>> {
        self.native_value("dbms", |native| native.dbms.as_ref())
    }

    /// The `[[database]]` entry that names the target: the first match in
    /// file order, the highest layer's file first.
    pub fn database_binding(&self, target: DatabaseTarget<'_>) -> Option<Sourced<PlatformSpec>> {
        self.files.iter().rev().find_map(|file| {
            file.databases
                .iter()
                .find(|binding| binding.matches(target))
                .map(|binding| {
                    Sourced::new(
                        binding.platform,
                        SettingSource::Database {
                            path: file.path.clone(),
                            line: binding.line,
                            server: binding.server.clone(),
                            name: binding.name.clone(),
                        },
                    )
                })
        })
    }

    /// `IBCMD_RS_PLATFORM`.
    pub fn environment_platform(&self) -> Option<Sourced<PlatformSpec>> {
        self.env_platform.map(|platform| {
            Sourced::new(
                platform,
                SettingSource::Environment {
                    variable: ENV_PLATFORM.to_string(),
                },
            )
        })
    }

    /// The top-level `platform` of the highest layer that sets it.
    pub fn file_platform(&self) -> Option<Sourced<PlatformSpec>> {
        self.files.iter().rev().find_map(|file| {
            file.platform.as_ref().map(|located| {
                Sourced::new(
                    located.value,
                    SettingSource::File {
                        path: file.path.clone(),
                        line: located.line,
                    },
                )
            })
        })
    }

    /// The platform whose `bin` folder holds the executable
    /// (`...\1cv8\8.3.27.2214\bin\ibcmd-rs.exe`), when it lies in one. A
    /// folder named like a version the registry does not know is refused.
    pub fn executable_platform(&self) -> Result<Option<Sourced<PlatformSpec>>> {
        let Some(executable) = &self.executable else {
            return Ok(None);
        };
        let Some(version) = platform_folder(executable) else {
            return Ok(None);
        };
        let spec = crate::platform::parse(&version).with_context(|| {
            format!(
                "the executable lies in the folder of platform {version} ({})",
                executable.display()
            )
        })?;
        Ok(Some(Sourced::new(
            spec,
            SettingSource::ExecutableDirectory {
                path: executable
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_default(),
            },
        )))
    }
}

/// `<version>` of an executable at `...\1cv8\<version>\bin\<file>`, when the
/// folder is named like a version (three or four numbers).
fn platform_folder(executable: &Path) -> Option<String> {
    let bin = executable.parent()?;
    let version = bin.parent()?;
    let family = version.parent()?;
    let name = |path: &Path| path.file_name()?.to_str().map(str::to_string);
    if !name(bin)?.eq_ignore_ascii_case("bin") || !name(family)?.eq_ignore_ascii_case("1cv8") {
        return None;
    }
    let version = name(version)?;
    let parts = version.split('.').collect::<Vec<_>>();
    (matches!(parts.len(), 3 | 4)
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit())))
    .then_some(version)
}

/// The platform of the XML format a source tree's `Configuration.xml`
/// declares: 2.20 -> 8.3.27, 2.21 -> 8.5.1.
fn source_tree_platform(source_root: &Path) -> Result<Option<Sourced<PlatformSpec>>> {
    let path = source_root.join("Configuration.xml");
    let Some(version) = crate::metadata_model::export::tree_version(source_root) else {
        return Ok(None);
    };
    let xml = match version.as_str() {
        "2.20" => InfobaseConfigSourceVersion::V2_20,
        "2.21" => InfobaseConfigSourceVersion::V2_21,
        other => bail!(
            "{} declares XML {other}, which no platform ibcmd-rs knows reads or writes",
            path.display()
        ),
    };
    let spec = crate::platform::for_xml_version(xml)
        .with_context(|| format!("the XML format of {}", path.display()))?;
    Ok(Some(Sourced::new(spec, SettingSource::SourceTree { path })))
}

/// The platform of one database, and where it came from, without printing
/// anything. See the module docs for the order.
pub fn resolve_platform_with_source(
    explicit: Option<&str>,
    settings: &Settings,
    target: Option<DatabaseTarget<'_>>,
    hint: PlatformHint<'_>,
) -> Result<Sourced<PlatformSpec>> {
    if let Some(text) = explicit {
        let spec = crate::platform::parse(text).context("--platform")?;
        return Ok(Sourced::new(
            spec,
            SettingSource::Flag {
                flag: "--platform".to_string(),
            },
        ));
    }
    if let Some(bound) = target.and_then(|target| settings.database_binding(target)) {
        return Ok(bound);
    }
    if let Some(platform) = settings.environment_platform() {
        return Ok(platform);
    }
    if let Some(platform) = settings.file_platform() {
        return Ok(platform);
    }
    if let Some(platform) = settings.executable_platform()? {
        return Ok(platform);
    }
    match hint {
        PlatformHint::None => {}
        PlatformHint::Import { source_root } => {
            if let Some(platform) = source_tree_platform(source_root)? {
                return Ok(platform);
            }
        }
        PlatformHint::Export {
            compatibility_8_5_or_later,
        } => {
            if compatibility_8_5_or_later()
                .context("failed to read the configuration's compatibility mode")?
            {
                let spec = crate::platform::for_xml_version(InfobaseConfigSourceVersion::V2_21)
                    .context("a configuration kept in compatibility 8.5 or later")?;
                return Ok(Sourced::new(spec, SettingSource::Compatibility));
            }
        }
    }
    Ok(Sourced::new(
        crate::platform::parse(DEFAULT_PLATFORM)?,
        SettingSource::Default,
    ))
}

/// The platform of one database (see the module docs for the order). When
/// nothing names it, 8.3.27 is assumed with a warning on stderr.
pub fn resolve_platform(
    explicit: Option<&str>,
    settings: &Settings,
    target: Option<DatabaseTarget<'_>>,
    hint: PlatformHint<'_>,
) -> Result<PlatformSpec> {
    let resolved = resolve_platform_with_source(explicit, settings, target, hint)?;
    if resolved.source == SettingSource::Default {
        warn_default_platform(resolved.value, target);
    }
    Ok(resolved.value)
}

fn warn_default_platform(platform: PlatformSpec, target: Option<DatabaseTarget<'_>>) {
    let database = target
        .map(|target| match target.server {
            Some(server) => format!(" for {} on {server}", target.name),
            None => format!(" for {}", target.name),
        })
        .unwrap_or_default();
    eprintln!(
        "warning: no platform is configured{database}; assuming {platform} (XML {}). Name it with --platform, a [[database]] entry or `platform` in {SETTINGS_FILE_NAME}, or {ENV_PLATFORM}.",
        platform.xml_version().as_str()
    );
}

#[cfg(test)]
mod tests;
