//! Reading the settings files: `ibcmd-rs.toml`, and the `database:` section
//! of native ibcmd's YAML config.

use std::ops::Range;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use toml::Spanned;

use super::{DatabaseBinding, Located, NativeConfig, SettingsFile, SettingsLayer};

/// `ibcmd-rs.toml`: every key it may hold.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct FileDocument {
    platform: Option<Spanned<String>>,
    db_server: Option<Spanned<String>>,
    db_user: Option<Spanned<String>>,
    /// A whole number of rows, or a string of digits (S1-J).
    restructure_limit_rows: Option<Spanned<toml::Value>>,
    /// A number of bytes, or a string with a unit (`"4GB"`).
    restructure_limit_bytes: Option<Spanned<toml::Value>>,
    #[serde(default)]
    database: Vec<DatabaseDocument>,
}

/// One `[[database]]` entry.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct DatabaseDocument {
    server: Option<Spanned<String>>,
    name: Spanned<String>,
    platform: Spanned<String>,
    rows_dir: Option<Spanned<String>>,
}

fn read_text(path: &Path, what: &str) -> Result<String> {
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read {what} {}", path.display()))?;
    let text = String::from_utf8(bytes)
        .map_err(|_| anyhow!("{what} {} is not UTF-8 text", path.display()))?;
    Ok(match text.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_string(),
        None => text,
    })
}

/// The 1-based line a byte offset lies on.
fn line_of(text: &str, span: Range<usize>) -> usize {
    text[..span.start.min(text.len())].matches('\n').count() + 1
}

/// Reads and checks one settings file: an unknown key, a malformed value or
/// a platform the registry does not know fails, naming the file and line.
pub(super) fn read_settings_file(path: &Path, layer: SettingsLayer) -> Result<SettingsFile> {
    let text = read_text(path, "settings file")?;
    let document: FileDocument = toml::from_str(&text)
        .map_err(|error| anyhow!("{}: {}", path.display(), error.to_string().trim_end()))?;
    let at = |span: Range<usize>| format!("{}:{}", path.display(), line_of(&text, span));

    let platform = document
        .platform
        .map(|value| {
            let line = line_of(&text, value.span());
            let spec = crate::platform::parse(value.get_ref()).with_context(|| {
                format!("{}: platform = \"{}\"", at(value.span()), value.get_ref())
            })?;
            Ok::<_, anyhow::Error>(Located { value: spec, line })
        })
        .transpose()?;
    let text_value =
        |value: Option<Spanned<String>>, key: &str| -> Result<Option<Located<String>>> {
            let Some(value) = value else {
                return Ok(None);
            };
            let trimmed = value.get_ref().trim();
            if trimmed.is_empty() {
                bail!("{}: {key} is empty", at(value.span()));
            }
            Ok(Some(Located {
                value: trimmed.to_string(),
                line: line_of(&text, value.span()),
            }))
        };
    let db_server = text_value(document.db_server, "db-server")?;
    let db_user = text_value(document.db_user, "db-user")?;
    let limit = |value: Option<Spanned<toml::Value>>,
                 key: &str,
                 bytes: bool|
     -> Result<Option<Located<u64>>> {
        let Some(value) = value else {
            return Ok(None);
        };
        let parsed = match value.get_ref() {
            toml::Value::Integer(number) if *number > 0 => Ok(*number as u64),
            toml::Value::String(text) if bytes => {
                crate::restructure::size_guard::parse_byte_size(text)
            }
            toml::Value::String(text) => super::restructure::parse_count(text),
            other => Err(anyhow!(
                "expected a positive number, found a {}",
                other.type_str()
            )),
        }
        .with_context(|| format!("{}: {key}", at(value.span())))?;
        if parsed == 0 {
            bail!("{}: {key} is 0", at(value.span()));
        }
        Ok(Some(Located {
            value: parsed,
            line: line_of(&text, value.span()),
        }))
    };
    let restructure_limit_rows = limit(
        document.restructure_limit_rows,
        "restructure-limit-rows",
        false,
    )?;
    let restructure_limit_bytes = limit(
        document.restructure_limit_bytes,
        "restructure-limit-bytes",
        true,
    )?;

    let mut databases = Vec::with_capacity(document.database.len());
    for entry in document.database {
        let line = line_of(&text, entry.platform.span());
        let name = entry.name.get_ref().trim();
        if name.is_empty() {
            bail!("{}: [[database]] name is empty", at(entry.name.span()));
        }
        let server = match entry.server {
            Some(server) if server.get_ref().trim().is_empty() => {
                bail!("{}: [[database]] server is empty", at(server.span()))
            }
            Some(server) => Some(server.get_ref().trim().to_string()),
            None => None,
        };
        let platform = crate::platform::parse(entry.platform.get_ref()).with_context(|| {
            format!(
                "{}: [[database]] platform = \"{}\"",
                at(entry.platform.span()),
                entry.platform.get_ref()
            )
        })?;
        let rows_dir = match entry.rows_dir {
            Some(dir) if dir.get_ref().trim().is_empty() => {
                bail!("{}: [[database]] rows-dir is empty", at(dir.span()))
            }
            Some(dir) => {
                let dir = Path::new(dir.get_ref().trim());
                Some(match path.parent() {
                    Some(folder) if dir.is_relative() => folder.join(dir),
                    _ => dir.to_path_buf(),
                })
            }
            None => None,
        };
        databases.push(DatabaseBinding {
            server,
            name: name.to_string(),
            platform,
            line,
            rows_dir,
        });
    }
    Ok(SettingsFile {
        path: path.to_path_buf(),
        layer,
        platform,
        db_server,
        db_user,
        restructure_limit_rows,
        restructure_limit_bytes,
        databases,
    })
}

/// Reads the `database:` section of native ibcmd's config file (the YAML
/// `ibcmd server config init --out=<file>` writes); the other sections are
/// the server's and are not read.
pub(super) fn read_native_config(path: &Path) -> Result<NativeConfig> {
    let text = read_text(path, "native config")?;
    let document: yaml_serde::Value =
        yaml_serde::from_str(&text).map_err(|error| anyhow!("{}: {}", path.display(), error))?;
    let mut config = NativeConfig {
        path: path.to_path_buf(),
        ..NativeConfig::default()
    };
    let database = match document.get("database") {
        None | Some(yaml_serde::Value::Null) => return Ok(config),
        Some(database @ yaml_serde::Value::Mapping(_)) => database,
        Some(_) => bail!("{}: `database` is not a section", path.display()),
    };
    let scalar = |key: &str| -> Result<Option<String>> {
        Ok(match database.get(key) {
            None | Some(yaml_serde::Value::Null) => None,
            Some(yaml_serde::Value::String(text)) => Some(text.clone()),
            Some(yaml_serde::Value::Number(number)) => Some(number.to_string()),
            Some(yaml_serde::Value::Bool(flag)) => Some(flag.to_string()),
            Some(_) => bail!("{}: database.{key} is not a single value", path.display()),
        }
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty()))
    };
    config.dbms = scalar("dbms")?;
    config.server = scalar("server")?;
    config.name = scalar("name")?;
    config.user = scalar("user")?;
    config.password = scalar("password")?;
    Ok(config)
}
