use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use serde_json::Value;
use walkdir::WalkDir;

use crate::cli::{
    InfobaseConfigExportArgs, InfobaseConfigFormat, InfobaseConfigImportArgs,
    InfobaseConfigSourceVersion, MssqlDumpConfigArgs, MssqlStageSourceObjectsArgs,
};

#[derive(Debug, Serialize)]
pub struct InfobaseConfigExportReport {
    pub operation: &'static str,
    pub backend: &'static str,
    pub format: &'static str,
    pub source_version: &'static str,
    pub dbms: String,
    pub db_server: String,
    pub db_name: String,
    pub db_user: Option<String>,
    pub password_source: Option<String>,
    pub output_dir: PathBuf,
    pub temp_dump_dir: PathBuf,
    pub exported_files: usize,
    pub raw_rows: usize,
    pub metadata_xml_rows: usize,
    pub module_text_rows: usize,
    pub source_asset_rows: usize,
    pub dump_timings: crate::mssql_dump::MssqlDumpTimingReport,
}

#[derive(Debug, Serialize)]
pub struct InfobaseConfigImportReport {
    pub operation: &'static str,
    pub backend: &'static str,
    pub format: &'static str,
    pub source_version: &'static str,
    pub dbms: String,
    pub db_server: String,
    pub db_name: String,
    pub db_user: Option<String>,
    pub source_dir: PathBuf,
    pub staged_rows_before: i64,
    pub staged_rows_after: i64,
    pub scripts: Vec<PathBuf>,
}

#[derive(Debug)]
struct ConnectionConfig {
    dbms: String,
    db_server: String,
    db_name: String,
    db_user: Option<String>,
    db_pwd: Option<String>,
    password_source: Option<String>,
    format: InfobaseConfigFormat,
    source_version: InfobaseConfigSourceVersion,
}

pub fn export_config(args: &InfobaseConfigExportArgs) -> Result<InfobaseConfigExportReport> {
    let config = resolve_connection(
        args.settings.as_deref(),
        args.format,
        args.source_version,
        args.dbms.as_deref(),
        args.db_server.as_deref(),
        args.db_name.as_deref(),
        args.db_user.as_deref(),
        args.db_pwd.as_deref(),
        &args.db_pwd_env,
    )?;
    ensure_mssql(&config.dbms)?;

    let output_dir = absolute_path(&args.output_dir)?;
    prepare_output_dir(&output_dir, args.overwrite)?;
    let dump_args = MssqlDumpConfigArgs {
        sqlcmd: args.sqlcmd.clone(),
        server: config.db_server.clone(),
        sql_user: config.db_user.clone(),
        sql_pwd: config.db_pwd.clone(),
        sql_pwd_env: args.db_pwd_env.clone(),
        database: config.db_name.clone(),
        output_dir: output_dir.clone(),
        overwrite: false,
        include_config_save: false,
        file_names: Vec::new(),
        inflate: false,
        extract_module_text: true,
        extract_metadata_xml: true,
        write_binary_rows: false,
        write_manifest: false,
        source_version: config.source_version,
    };
    let dump = crate::mssql_dump::dump_config(&dump_args)?;

    let exported_files = count_files(&output_dir)?;

    Ok(InfobaseConfigExportReport {
        operation: "infobase config export",
        backend: "mssql-config-direct",
        format: format_name(config.format),
        source_version: config.source_version.as_str(),
        dbms: config.dbms,
        db_server: config.db_server,
        db_name: config.db_name,
        db_user: config.db_user,
        password_source: config.password_source,
        output_dir,
        temp_dump_dir: dump_args.output_dir,
        exported_files,
        raw_rows: dump.total_rows,
        metadata_xml_rows: dump.total_metadata_xml_rows,
        module_text_rows: dump.total_module_text_rows,
        source_asset_rows: dump.total_source_asset_rows,
        dump_timings: dump.timings,
    })
}

pub fn import_config(args: &InfobaseConfigImportArgs) -> Result<InfobaseConfigImportReport> {
    let config = resolve_connection(
        args.settings.as_deref(),
        args.format,
        args.source_version,
        args.dbms.as_deref(),
        args.db_server.as_deref(),
        args.db_name.as_deref(),
        args.db_user.as_deref(),
        args.db_pwd.as_deref(),
        &args.db_pwd_env,
    )?;
    ensure_mssql(&config.dbms)?;

    let stage_args = MssqlStageSourceObjectsArgs {
        server: config.db_server.clone(),
        database: config.db_name.clone(),
        source_root: absolute_path(&args.source_dir)?,
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        batch_size: args.batch_size,
        path_prefix: args.path_prefix.clone(),
        script_output: args.script_output.clone(),
    };
    let report = crate::mssql::stage_source_objects(&stage_args)?;

    Ok(InfobaseConfigImportReport {
        operation: "infobase config import",
        backend: "mssql-configsave-stage",
        format: format_name(config.format),
        source_version: config.source_version.as_str(),
        dbms: config.dbms,
        db_server: config.db_server,
        db_name: config.db_name,
        db_user: config.db_user,
        source_dir: stage_args.source_root,
        staged_rows_before: report.before.row_count,
        staged_rows_after: report.after.row_count,
        scripts: report.scripts,
    })
}

fn resolve_connection(
    settings_path: Option<&Path>,
    cli_format: Option<InfobaseConfigFormat>,
    cli_source_version: Option<InfobaseConfigSourceVersion>,
    cli_dbms: Option<&str>,
    cli_db_server: Option<&str>,
    cli_db_name: Option<&str>,
    cli_db_user: Option<&str>,
    cli_db_pwd: Option<&str>,
    db_pwd_env: &str,
) -> Result<ConnectionConfig> {
    let settings = match settings_path {
        Some(path) => Some(read_settings(path)?),
        None => None,
    };

    let format = cli_format
        .or_else(|| settings_format(&settings))
        .unwrap_or(InfobaseConfigFormat::Xml);
    let source_version = cli_source_version
        .or_else(|| settings_source_version(&settings))
        .unwrap_or(InfobaseConfigSourceVersion::V2_20);
    let dbms = first_value(cli_dbms, settings_value(&settings, "dbms-type"))
        .unwrap_or_else(|| "MSSQLServer".to_string());
    let db_server = first_value(cli_db_server, settings_value(&settings, "dbms-server"))
        .unwrap_or_else(|| "localhost".to_string());
    let db_name = first_value(cli_db_name, settings_value(&settings, "dbms-base"))
        .ok_or_else(|| anyhow!("database name is required: pass --db-name or --settings"))?;
    let db_user = first_value(cli_db_user, settings_value(&settings, "dbms-user"));
    let (db_pwd, password_source) = match db_user {
        Some(_) => resolve_password(cli_db_pwd, &settings, db_pwd_env)?,
        None => (None, None),
    };

    Ok(ConnectionConfig {
        dbms,
        db_server,
        db_name,
        db_user,
        db_pwd,
        password_source,
        format,
        source_version,
    })
}

fn read_settings(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read settings {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

fn settings_value(settings: &Option<Value>, name: &str) -> Option<String> {
    settings
        .as_ref()?
        .get("vrunner")?
        .get(name)?
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn settings_format(settings: &Option<Value>) -> Option<InfobaseConfigFormat> {
    let value = settings_string_at(settings, &["ibcmd-rs", "config-format"])
        .or_else(|| settings_string_at(settings, &["ibcmd-rs", "format"]))
        .or_else(|| settings_string_at(settings, &["format"]))
        .or_else(|| settings_value(settings, "format"))?;
    parse_format(&value)
}

fn settings_source_version(settings: &Option<Value>) -> Option<InfobaseConfigSourceVersion> {
    let value = settings_string_at(settings, &["ibcmd-rs", "source-version"])
        .or_else(|| settings_string_at(settings, &["ibcmd-rs", "xml-version"]))
        .or_else(|| settings_string_at(settings, &["ibcmd-rs", "xcf-version"]))
        .or_else(|| settings_string_at(settings, &["ibcmd-rs", "platform-version"]))
        .or_else(|| settings_string_at(settings, &["source-version"]))
        .or_else(|| settings_string_at(settings, &["xml-version"]))
        .or_else(|| settings_string_at(settings, &["xcf-version"]))
        .or_else(|| settings_string_at(settings, &["platform-version"]))
        .or_else(|| settings_value(settings, "source-version"))
        .or_else(|| settings_value(settings, "xml-version"))
        .or_else(|| settings_value(settings, "xcf-version"))
        .or_else(|| settings_value(settings, "platform-version"))?;
    parse_source_version(&value)
}

fn settings_string_at(settings: &Option<Value>, path: &[&str]) -> Option<String> {
    let mut current = settings.as_ref()?;
    for segment in path {
        current = current.get(*segment)?;
    }
    current
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn parse_format(value: &str) -> Option<InfobaseConfigFormat> {
    match value.trim().to_ascii_lowercase().as_str() {
        "xml" | "ibcmd-xml" | "source-tree" => Some(InfobaseConfigFormat::Xml),
        _ => None,
    }
}

fn parse_source_version(value: &str) -> Option<InfobaseConfigSourceVersion> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "2.20" | "20" | "8.3" | "8.3.27" => Some(InfobaseConfigSourceVersion::V2_20),
        "2.21" | "21" | "8.5" | "8.5.1" => Some(InfobaseConfigSourceVersion::V2_21),
        _ if normalized.starts_with("8.3.27.") => Some(InfobaseConfigSourceVersion::V2_20),
        _ if normalized.starts_with("8.5.1.") => Some(InfobaseConfigSourceVersion::V2_21),
        _ => None,
    }
}

fn first_value(cli: Option<&str>, settings: Option<String>) -> Option<String> {
    cli.filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .or(settings)
}

fn resolve_password(
    cli_db_pwd: Option<&str>,
    settings: &Option<Value>,
    db_pwd_env: &str,
) -> Result<(Option<String>, Option<String>)> {
    if let Some(value) = cli_db_pwd.filter(|value| !value.is_empty()) {
        return Ok((Some(value.to_string()), Some("--db-pwd".to_string())));
    }
    if let Ok(value) = env::var(db_pwd_env) {
        return Ok((Some(value), Some(format!("env:{db_pwd_env}"))));
    }
    if let Some(value) = settings_value(settings, "dbms-pwd") {
        return Ok((Some(value), Some("settings".to_string())));
    }
    bail!("database password is required when --db-user or settings dbms-user is set")
}

fn ensure_mssql(dbms: &str) -> Result<()> {
    if dbms.eq_ignore_ascii_case("MSSQLServer") || dbms.eq_ignore_ascii_case("MSSQL") {
        return Ok(());
    }
    bail!("unsupported dbms for direct infobase config operation: {dbms}")
}

fn format_name(format: InfobaseConfigFormat) -> &'static str {
    match format {
        InfobaseConfigFormat::Xml => "xml",
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return fs::canonicalize(path)
            .with_context(|| format!("failed to resolve {}", path.display()));
    }
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()?.join(path))
    }
}

fn prepare_output_dir(path: &Path, overwrite: bool) -> Result<()> {
    if path.exists() {
        if !path.is_dir() {
            bail!(
                "output path exists and is not a directory: {}",
                path.display()
            );
        }
        if fs::read_dir(path)?.next().is_some() && !overwrite {
            bail!(
                "output directory is not empty: {}. Pass --overwrite or --force",
                path.display()
            );
        }
        if overwrite {
            clear_directory(path)?;
        }
    } else {
        fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))?;
    }
    Ok(())
}

fn clear_directory(path: &Path) -> Result<()> {
    for entry in fs::read_dir(path).with_context(|| format!("failed to read {}", path.display()))? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

#[cfg(test)]
fn is_internal_dump_path(relative: &Path) -> bool {
    use std::ffi::OsStr;
    use std::path::Component;

    if relative == Path::new("manifest.json") {
        return true;
    }
    let Some(Component::Normal(first)) = relative.components().next() else {
        return false;
    };
    matches!(
        first,
        name if name == OsStr::new("Config")
            || name == OsStr::new("ConfigSave")
            || name == OsStr::new("Config_inflated")
            || name == OsStr::new("ConfigSave_inflated")
            || name == OsStr::new("Config_module_text")
            || name == OsStr::new("ConfigSave_module_text")
    )
}

fn count_files(root: &Path) -> Result<usize> {
    let mut count = 0;
    for entry in WalkDir::new(root) {
        let entry = entry?;
        if entry.file_type().is_file() {
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_format_from_top_level_settings() {
        let settings: Option<Value> = Some(serde_json::json!({
            "format": "ibcmd-xml",
            "ibcmd-rs": {
                "source-version": "8.5.1"
            },
            "vrunner": {
                "dbms-base": "servicedesk"
            }
        }));

        assert_eq!(settings_format(&settings), Some(InfobaseConfigFormat::Xml));
        assert_eq!(
            settings_source_version(&settings),
            Some(InfobaseConfigSourceVersion::V2_21)
        );
        assert_eq!(
            settings_value(&settings, "dbms-base"),
            Some("servicedesk".to_string())
        );
    }

    #[test]
    fn skips_only_internal_dump_roots() {
        assert!(is_internal_dump_path(Path::new("Config/versions.bin")));
        assert!(is_internal_dump_path(Path::new("Config_module_text/a.bsl")));
        assert!(is_internal_dump_path(Path::new("manifest.json")));
        assert!(!is_internal_dump_path(Path::new("Configuration.xml")));
        assert!(!is_internal_dump_path(Path::new(
            "Constants/UseFeature.xml"
        )));
        assert!(!is_internal_dump_path(Path::new("ConfigDumpInfo.xml")));
    }
}
