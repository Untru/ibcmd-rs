//! `ibcmd-rs settings show`: the effective settings for a database and where
//! each value came from.

use std::fmt::Write as _;
use std::path::PathBuf;

use anyhow::Result;
use serde::Serialize;

use super::{
    DatabaseTarget, PlatformHint, SettingSource, Settings, SettingsLayer, Sourced,
    resolve_platform_with_source,
};
use crate::platform::{FormLayout, PlatformSpec};

/// One settings layer.
#[derive(Clone, Debug, Serialize)]
pub struct FileReport {
    pub path: PathBuf,
    pub layer: SettingsLayer,
    pub read: bool,
    /// Its `[[database]]` entries.
    pub databases: usize,
}

/// The platform and what it implies.
#[derive(Clone, Debug, Serialize)]
pub struct PlatformReport {
    pub version: PlatformSpec,
    pub xml_version: &'static str,
    pub form_layout: FormLayout,
    pub source: SettingSource,
}

/// One `[[database]]` entry, in the order they are searched.
#[derive(Clone, Debug, Serialize)]
pub struct BindingReport {
    pub path: PathBuf,
    pub line: usize,
    pub server: Option<String>,
    pub name: String,
    pub platform: PlatformSpec,
    /// Whether it is the entry that names the database shown.
    pub matches: bool,
    /// `rows-dir`: a folder of stored rows read instead of SQL Server.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows_dir: Option<PathBuf>,
}

/// What `settings show` prints.
#[derive(Clone, Debug, Serialize)]
pub struct SettingsReport {
    pub files: Vec<FileReport>,
    pub db_server: Option<Sourced<String>>,
    pub db_name: Option<Sourced<String>>,
    pub platform: PlatformReport,
    pub db_user: Option<Sourced<String>>,
    /// Where the password comes from; the password itself is never shown.
    pub db_password: Option<SettingSource>,
    pub dbms: Option<Sourced<String>>,
    /// The most rows of tables a restructuring may rebuild (S1-J); `None`: the measured default.
    pub restructure_limit_rows: Option<Sourced<u64>>,
    /// The most bytes of those tables and their indexes; `None`: the measured default.
    pub restructure_limit_bytes: Option<Sourced<u64>>,
    pub databases: Vec<BindingReport>,
}

fn flag(value: Option<&str>, name: &str) -> Option<Sourced<String>> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| Sourced {
            value: value.to_string(),
            source: SettingSource::Flag {
                flag: name.to_string(),
            },
        })
}

/// The effective settings for the database `--db-server`/`--db-name` (or
/// the settings) name, as a command without other flags would take them.
/// Auto-detection sees only the executable's folder here: no source tree and
/// no database is read.
pub fn settings_report(
    settings: &Settings,
    platform: Option<&str>,
    db_server: Option<&str>,
    db_name: Option<&str>,
) -> Result<SettingsReport> {
    let mut files = settings
        .files()
        .iter()
        .map(|file| FileReport {
            path: file.path.clone(),
            layer: file.layer,
            read: true,
            databases: file.databases().len(),
        })
        .chain(
            settings
                .absent_files()
                .iter()
                .map(|(path, layer)| FileReport {
                    path: path.clone(),
                    layer: *layer,
                    read: false,
                    databases: 0,
                }),
        )
        .collect::<Vec<_>>();
    files.sort_by_key(|file| layer_order(file.layer));

    let db_server = flag(db_server, "--db-server").or_else(|| settings.db_server());
    let db_name = flag(db_name, "--db-name").or_else(|| settings.db_name());
    let target = db_name.as_ref().and_then(|name| {
        DatabaseTarget::new(
            db_server.as_ref().map(|server| server.value.as_str()),
            &name.value,
        )
    });
    let resolved = resolve_platform_with_source(platform, settings, target, PlatformHint::None)?;
    let chosen = target.and_then(|target| settings.database_binding(target));
    let databases = settings
        .files()
        .iter()
        .rev()
        .flat_map(|file| {
            file.databases().iter().map(move |binding| BindingReport {
                path: file.path.clone(),
                line: binding.line,
                server: binding.server.clone(),
                name: binding.name.clone(),
                platform: binding.platform,
                matches: false,
                rows_dir: binding.rows_dir.clone(),
            })
        })
        .map(|mut binding| {
            binding.matches = chosen.as_ref().is_some_and(|chosen| {
                chosen.source
                    == SettingSource::Database {
                        path: binding.path.clone(),
                        line: binding.line,
                        server: binding.server.clone(),
                        name: binding.name.clone(),
                    }
            });
            binding
        })
        .collect();
    Ok(SettingsReport {
        files,
        db_server,
        db_name,
        platform: PlatformReport {
            version: resolved.value,
            xml_version: resolved.value.xml_version().as_str(),
            form_layout: resolved.value.form_layout(),
            source: resolved.source,
        },
        db_user: settings.db_user(),
        db_password: settings.db_password().map(|password| password.source),
        dbms: settings.dbms(),
        restructure_limit_rows: settings.restructure_limit_rows()?,
        restructure_limit_bytes: settings.restructure_limit_bytes()?,
        databases,
    })
}

fn layer_order(layer: SettingsLayer) -> u8 {
    match layer {
        SettingsLayer::Explicit => 0,
        SettingsLayer::NextToExecutable => 1,
        SettingsLayer::AppData => 2,
        SettingsLayer::CurrentDirectory => 3,
    }
}

/// The report as text.
pub fn render(report: &SettingsReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "settings files (a later one wins):");
    if report.files.is_empty() {
        let _ = writeln!(out, "  none");
    }
    for file in &report.files {
        let state = if file.read {
            match file.databases {
                0 => "read".to_string(),
                1 => "read, 1 [[database]] entry".to_string(),
                count => format!("read, {count} [[database]] entries"),
            }
        } else {
            "absent".to_string()
        };
        let _ = writeln!(
            out,
            "  {:<24} {}  ({state})",
            file.layer.to_string(),
            file.path.display()
        );
    }
    let _ = writeln!(out);
    let line = |out: &mut String, key: &str, value: &str, source: Option<&SettingSource>| {
        let _ = match source {
            Some(source) => writeln!(out, "{key:<12} {value:<24} {source}"),
            None => writeln!(out, "{key:<12} {value}"),
        };
    };
    let sourced = |out: &mut String, key: &str, value: &Option<Sourced<String>>| match value {
        Some(value) => line(out, key, &value.value, Some(&value.source)),
        None => line(out, key, "(not set)", None),
    };
    let platform = &report.platform;
    line(
        &mut out,
        "platform",
        &format!(
            "{} (XML {}, form layout {})",
            platform.version, platform.xml_version, platform.form_layout
        ),
        Some(&platform.source),
    );
    sourced(&mut out, "db-server", &report.db_server);
    sourced(&mut out, "db-name", &report.db_name);
    sourced(&mut out, "db-user", &report.db_user);
    match &report.db_password {
        Some(source) => line(&mut out, "db-password", "(set)", Some(source)),
        None => line(&mut out, "db-password", "(not set)", None),
    }
    sourced(&mut out, "dbms", &report.dbms);
    for (key, value) in [
        ("limit-rows", &report.restructure_limit_rows),
        ("limit-bytes", &report.restructure_limit_bytes),
    ] {
        match value {
            Some(value) => line(&mut out, key, &value.value.to_string(), Some(&value.source)),
            None => line(&mut out, key, "(the default)", None),
        }
    }
    if !report.databases.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[database]] entries, in the order they are searched:");
        for binding in &report.databases {
            let server = binding
                .server
                .as_deref()
                .map(|server| format!(" on {server}"))
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "{} {}{server} -> {}  ({}:{})",
                if binding.matches { "*" } else { " " },
                binding.name,
                binding.platform,
                binding.path.display(),
                binding.line
            );
        }
    }
    out
}
