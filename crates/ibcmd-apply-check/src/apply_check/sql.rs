//! Reading the Config and ConfigSave tables for the check: read-only.
//!
//! The configuration the infobase runs is not simply the plain rows of
//! Config. An online (dynamic) update leaves the plain rows as they were and
//! writes the rows it changed under `<base>_dynupdate_<generation>`; the
//! table's `DynamicallyUpdated` row lists the generations, oldest first, and
//! for each name the newest generation that carries it wins (see
//! `mssql_dump::dynamic_generation`). Comparing a staged row with a plain one
//! would call a change what an earlier update already made, or miss a change
//! that reverts one, so the check reads the active rows.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::{Context, Result, anyhow, bail};

use crate::metadata_model::brace::parse_row;
use crate::sql::{SqlBackend, SqlClient, SqlExec};
use ibcmd_module_blob::inflate_raw;

use super::check::{Inputs, RowProvider, check};
use super::model::Verdict;
use super::roles::{DYNAMIC_MARKER, DYNAMIC_UPDATE_INFIX, RowName, parse_row_name};

/// `[database].dbo.[table]`
fn qualified(database: &str, table: &str) -> String {
    format!(
        "[{}].dbo.[{}]",
        database.replace(']', "]]"),
        table.replace(']', "]]")
    )
}

fn literal(name: &str) -> String {
    format!("N'{}'", name.replace('\'', "''"))
}

/// The bytes a stored row holds: raw deflate when it inflates, else as
/// stored.
fn content(stored: &[u8]) -> Vec<u8> {
    inflate_raw(stored).unwrap_or_else(|_| stored.to_vec())
}

/// The generation history a `DynamicallyUpdated` payload records, oldest
/// first: `{1,<count>,<generation>...}`.
pub fn parse_history(payload: &[u8]) -> Result<Vec<String>> {
    let payload = payload.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(payload);
    let text = std::str::from_utf8(payload)
        .context("the DynamicallyUpdated row is not text")?
        .trim();
    let fields = text
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
        .ok_or_else(|| anyhow!("the DynamicallyUpdated row is not a list: {text}"))?
        .split(',')
        .map(str::trim)
        .collect::<Vec<_>>();
    let count = fields
        .get(1)
        .and_then(|count| count.parse::<usize>().ok())
        .filter(|_| fields.first() == Some(&"1"))
        .ok_or_else(|| anyhow!("the DynamicallyUpdated row has an unknown shape: {text}"))?;
    if count == 0 || fields.len() != count + 2 {
        bail!(
            "the DynamicallyUpdated row lists {} generations, not {count}: {text}",
            fields.len() - 2
        );
    }
    for generation in &fields[2..] {
        if !super::roles::is_uuid(generation) {
            bail!("the DynamicallyUpdated row lists a generation that is no uuid: {generation}");
        }
    }
    Ok(fields[2..]
        .iter()
        .map(|generation| (*generation).to_string())
        .collect())
}

/// published name -> the row that holds it now, for every name an online
/// update replaced.
pub fn overlay(
    history: &[String],
    names: impl IntoIterator<Item = impl AsRef<str>>,
) -> HashMap<String, String> {
    let rank = history
        .iter()
        .enumerate()
        .map(|(rank, generation)| (format!("{DYNAMIC_UPDATE_INFIX}{generation}"), rank))
        .collect::<Vec<_>>();
    let mut best = HashMap::<String, (usize, String)>::new();
    for name in names {
        let name = name.as_ref();
        for (infix, rank) in &rank {
            let Some(position) = name.find(infix.as_str()) else {
                continue;
            };
            let published = format!("{}{}", &name[..position], &name[position + infix.len()..]);
            match best.get(&published) {
                Some((current, _)) if current >= rank => {}
                _ => {
                    best.insert(published, (*rank, name.to_string()));
                }
            }
            break;
        }
    }
    best.into_iter()
        .map(|(published, (_, alias))| (published, alias))
        .collect()
}

/// The names the active Config publishes: its plain rows, and the names that
/// exist only as an alias of an online update (an object the update added).
pub fn published_names<'a>(
    names: impl IntoIterator<Item = &'a String>,
    overlay: &HashMap<String, String>,
) -> BTreeSet<String> {
    let mut published = names
        .into_iter()
        .filter(|name| !name.contains(DYNAMIC_UPDATE_INFIX))
        .cloned()
        .collect::<BTreeSet<_>>();
    published.extend(overlay.keys().cloned());
    published
}

/// The files a `versions` row lists with the version id it gives each:
/// `{1,<count>,"",<generation>,"<file>",<version>,...}`.
pub fn parse_versions(row: &[u8]) -> Result<BTreeMap<String, String>> {
    let tree = parse_row(row).context("failed to read the versions row")?;
    let items = tree
        .as_list()
        .ok_or_else(|| anyhow!("the versions row is not a list"))?;
    if items.len() < 4 || items.first().and_then(|item| item.as_atom()) != Some("1") {
        bail!("the versions row has an unknown shape");
    }
    let mut files = BTreeMap::new();
    let mut index = 4;
    while index + 1 < items.len() {
        let name = items[index].as_str().ok_or_else(|| {
            anyhow!("the versions row lists a file name that is no string, at {index}")
        })?;
        let version = items[index + 1].as_atom().unwrap_or_default();
        files.insert(name.to_string(), version.to_string());
        index += 2;
    }
    Ok(files)
}

pub(super) struct Db<'a> {
    client: &'a dyn SqlClient,
    database: &'a str,
}

impl Db<'_> {
    /// name -> (parts, bytes) of every row of a table.
    fn names(&self, table: &str) -> Result<BTreeMap<String, (i64, i64)>> {
        let query = format!(
            "SELECT FileName, COUNT_BIG(*), SUM(CAST(DataSize AS bigint)) FROM {} GROUP BY FileName",
            qualified(self.database, table)
        );
        let mut out = BTreeMap::new();
        self.client.read_rows(&query, &[], &mut |row| {
            let name = row.text(0)?.to_string();
            let parts = row.i64(1)?;
            let bytes = row.value(2)?.as_i64().unwrap_or(0);
            out.insert(name, (parts, bytes));
            Ok(())
        })?;
        Ok(out)
    }

    /// The stored rows of `names` (parts joined), by name.
    fn rows(&self, table: &str, names: &[String]) -> Result<BTreeMap<String, Vec<u8>>> {
        let mut out = BTreeMap::new();
        if names.is_empty() {
            return Ok(out);
        }
        let slices = self.client.max_connections().max(1);
        let per_slice = names.len().div_ceil(slices);
        let table = qualified(self.database, table);
        let client = self.client;
        let parts = std::thread::scope(|scope| {
            let readers = names
                .chunks(per_slice)
                .map(|slice| {
                    let table = &table;
                    scope.spawn(move || -> Result<Vec<(String, i64, Vec<u8>)>> {
                        let mut found = Vec::new();
                        for batch in slice.chunks(200) {
                            let list = batch.iter().map(|name| literal(name)).collect::<Vec<_>>().join(", ");
                            let query = format!(
                                "SELECT FileName, PartNo, BinaryData FROM {table} WHERE FileName IN ({list}) \
                                 ORDER BY FileName, PartNo"
                            );
                            client.read_rows(&query, &[], &mut |mut row| {
                                let name = row.take_text(0)?;
                                let part = row.i64(1)?;
                                let data = row.take_binary(2)?;
                                found.push((name, part, data));
                                Ok(())
                            })?;
                        }
                        Ok(found)
                    })
                })
                .collect::<Vec<_>>();
            readers
                .into_iter()
                .map(|reader| match reader.join() {
                    Ok(result) => result,
                    Err(panic) => std::panic::resume_unwind(panic),
                })
                .collect::<Result<Vec<_>>>()
        })?;
        let mut joined = BTreeMap::<String, Vec<(i64, Vec<u8>)>>::new();
        for (name, part, data) in parts.into_iter().flatten() {
            joined.entry(name).or_default().push((part, data));
        }
        for (name, mut pieces) in joined {
            pieces.sort_by_key(|(part, _)| *part);
            let mut bytes = Vec::new();
            for (_, piece) in pieces {
                bytes.extend_from_slice(&piece);
            }
            out.insert(name, bytes);
        }
        Ok(out)
    }
}

/// The two tables, as the check reads them.
pub(super) struct Rows<'a> {
    pub(super) db: Db<'a>,
    /// published name -> the row that holds it in Config.
    overlay: HashMap<String, String>,
}

impl RowProvider for Rows<'_> {
    fn old_rows(&self, names: &[String]) -> Result<BTreeMap<String, Vec<u8>>> {
        let stored = names
            .iter()
            .map(|name| {
                self.overlay
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| name.clone())
            })
            .collect::<Vec<_>>();
        let rows = self.db.rows("Config", &stored)?;
        let mut out = BTreeMap::new();
        for (name, stored_name) in names.iter().zip(&stored) {
            if let Some(bytes) = rows.get(stored_name) {
                out.insert(name.clone(), content(bytes));
            }
        }
        Ok(out)
    }

    fn staged_rows(&self, names: &[String]) -> Result<BTreeMap<String, Vec<u8>>> {
        Ok(self
            .db
            .rows("ConfigSave", names)?
            .into_iter()
            .map(|(name, bytes)| {
                let inflated = content(&bytes);
                (name, inflated)
            })
            .collect())
    }
}

/// The active Config of a database as the check reads it: the old side of
/// [`Inputs`] and the rows behind it.
pub(super) struct Active<'a> {
    pub(super) rows: Rows<'a>,
    pub(super) inputs: Inputs,
}

fn client_of(sql: &SqlExec) -> Result<&dyn SqlClient> {
    let SqlBackend::Client(client) = sql.backend() else {
        bail!("the restructuring check needs the built-in SQL client; drop --sqlcmd");
    };
    Ok(client)
}

/// Reads the active Config of `database`: every descriptor, the service
/// rows and the inventory. Read-only.
pub(super) fn read_active<'a>(
    client: &'a dyn SqlClient,
    database: &'a str,
    xml_version: Option<&str>,
) -> Result<Active<'a>> {
    let db = Db { client, database };
    let config_names = db
        .names("Config")
        .with_context(|| format!("failed to list the rows of {database}.Config"))?;

    let history = if config_names.contains_key(DYNAMIC_MARKER) {
        let marker = db.rows("Config", &[DYNAMIC_MARKER.to_string()])?;
        let payload = marker
            .get(DYNAMIC_MARKER)
            .ok_or_else(|| anyhow!("the DynamicallyUpdated row of Config could not be read"))?;
        // The marker is stored as it is, not deflated.
        parse_history(payload)?
    } else {
        Vec::new()
    };
    let overlay = overlay(&history, config_names.keys());
    let rows = Rows { db, overlay };

    // The active Config: the published names. A name an online update
    // added exists only as an alias row.
    let published = published_names(config_names.keys(), &rows.overlay);
    let stored_of = |name: &str| {
        rows.overlay
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_string())
    };

    let mut inputs = Inputs {
        xml_version: xml_version.map(str::to_string),
        ..Inputs::default()
    };

    let service = ["root", "version", "versions"]
        .iter()
        .filter(|name| published.contains(**name))
        .map(|name| stored_of(name))
        .collect::<Vec<_>>();
    let fetched = rows.db.rows("Config", &service)?;
    let service_row = |name: &str| fetched.get(&stored_of(name)).map(|bytes| content(bytes));
    inputs.old_root = service_row("root");
    inputs.old_version = service_row("version");
    inputs.old_versions = match service_row("versions") {
        Some(bytes) => parse_versions(&bytes)?,
        None => bail!("the active Config holds no versions row: it is not a configuration"),
    };
    inputs.old_inventory = inputs.old_versions.keys().cloned().collect();

    // The descriptors the inventory lists: not every plain row is one of
    // them (an object an online update dropped keeps its row), and not every
    // one is a plain row (an object an online update added has only an
    // alias).
    let descriptor_names = inputs
        .old_inventory
        .iter()
        .filter(|name| matches!(parse_row_name(name), RowName::Descriptor(_)))
        .filter(|name| published.contains(*name))
        .cloned()
        .collect::<Vec<_>>();
    let stored = descriptor_names
        .iter()
        .map(|name| stored_of(name))
        .collect::<Vec<_>>();
    let fetched = rows.db.rows("Config", &stored)?;
    for (name, stored_name) in descriptor_names.iter().zip(&stored) {
        if let Some(bytes) = fetched.get(stored_name) {
            inputs.old_descriptors.insert(name.clone(), content(bytes));
        }
    }
    Ok(Active { rows, inputs })
}

/// Reads what the check needs and runs it: is the ConfigSave of `database` a
/// change of the data structure? Read-only.
pub fn check_staged(sql: &SqlExec, database: &str, xml_version: Option<&str>) -> Result<Verdict> {
    let client = client_of(sql)?;
    let Active { rows, mut inputs } = read_active(client, database, xml_version)?;

    // The ConfigSave.
    let save_names = rows
        .db
        .names("ConfigSave")
        .with_context(|| format!("failed to list the rows of {database}.ConfigSave"))?;
    inputs.staged_names = save_names.keys().cloned().collect();
    let staged_descriptor_names = save_names
        .keys()
        .filter(|name| matches!(parse_row_name(name), RowName::Descriptor(_)))
        .cloned()
        .collect::<Vec<_>>();
    inputs.staged_descriptors = rows
        .db
        .rows("ConfigSave", &staged_descriptor_names)?
        .into_iter()
        .map(|(name, bytes)| (name, content(&bytes)))
        .collect();
    let staged_service = ["root", "version", "versions", "deleted"]
        .iter()
        .filter(|name| save_names.contains_key(**name))
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let fetched = rows.db.rows("ConfigSave", &staged_service)?;
    inputs.staged_root = fetched.get("root").map(|bytes| content(bytes));
    inputs.staged_version = fetched.get("version").map(|bytes| content(bytes));
    inputs.staged_deleted = fetched.get("deleted").map(|bytes| content(bytes));
    if let Some(bytes) = fetched.get("versions") {
        inputs.staged_has_versions = true;
        inputs.new_versions = parse_versions(&content(bytes))?;
        inputs.new_inventory = inputs.new_versions.keys().cloned().collect();
    }
    Ok(check(&inputs, &rows))
}

/// The active Config of `database` for a comparison with a source tree.
pub(super) fn active_of<'a>(
    sql: &'a SqlExec,
    database: &'a str,
    xml_version: Option<&str>,
) -> Result<Active<'a>> {
    read_active(client_of(sql)?, database, xml_version)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "15bcc426-54ca-410a-9543-768987b832ac";
    const NEW: &str = "17894f1a-0404-4132-9792-15816a396671";
    const OBJECT: &str = "a627e390-8fad-4a95-afe6-674f54813188";

    #[test]
    fn the_history_is_read_oldest_first() {
        let text = format!("\u{feff}{{1,2,{OLD},{NEW}}}");
        assert_eq!(
            parse_history(text.as_bytes()).unwrap(),
            vec![OLD.to_string(), NEW.to_string()]
        );
        assert!(parse_history(format!("{{1,3,{OLD},{NEW}}}").as_bytes()).is_err());
        assert!(parse_history(format!("{{0,1,{OLD}}}").as_bytes()).is_err());
        assert!(parse_history(b"{1,1,nope}").is_err());
    }

    #[test]
    fn the_newest_generation_that_carries_a_name_holds_it() {
        let names = vec![
            OBJECT.to_string(),
            format!("{OBJECT}_dynupdate_{OLD}"),
            format!("{OBJECT}_dynupdate_{NEW}"),
            format!("{OBJECT}_dynupdate_{NEW}.0"),
            "versions".to_string(),
            format!("versions_dynupdate_{OLD}"),
            "untouched".to_string(),
        ];
        let overlay = overlay(&[OLD.to_string(), NEW.to_string()], &names);
        assert_eq!(overlay[OBJECT], format!("{OBJECT}_dynupdate_{NEW}"));
        assert_eq!(
            overlay[&format!("{OBJECT}.0")],
            format!("{OBJECT}_dynupdate_{NEW}.0")
        );
        assert_eq!(overlay["versions"], format!("versions_dynupdate_{OLD}"));
        assert!(!overlay.contains_key("untouched"));
        // A generation that is not in the history publishes nothing.
        assert!(super::overlay(&[], &names).is_empty());
    }

    #[test]
    fn an_object_an_online_update_added_is_published_though_it_has_only_an_alias() {
        let added = "0f0f0f0f-0000-4000-8000-000000000001";
        let names = vec![
            OBJECT.to_string(),
            "versions".to_string(),
            format!("{added}_dynupdate_{NEW}"),
            format!("{added}_dynupdate_{NEW}.0"),
            format!("{OBJECT}_dynupdate_{NEW}"),
        ];
        let overlay = overlay(&[NEW.to_string()], &names);
        let published = published_names(&names, &overlay);
        assert!(published.contains(added), "{published:?}");
        assert!(published.contains(&format!("{added}.0")));
        assert!(published.contains(OBJECT) && published.contains("versions"));
        assert!(!published.iter().any(|name| name.contains("_dynupdate_")));
        assert_eq!(published.len(), 4);
    }

    #[test]
    fn a_versions_row_lists_the_files_of_the_configuration() {
        let text = format!(
            "\u{feff}{{1,3,\"\",{OLD},\"root\",{NEW},\"{OBJECT}\",{NEW},\"versions\",{OLD}}}"
        );
        let files = parse_versions(text.as_bytes()).unwrap();
        assert_eq!(files.len(), 3);
        assert!(
            files.contains_key("root")
                && files.contains_key(OBJECT)
                && files.contains_key("versions")
        );
        assert_eq!(files[OBJECT], NEW);
    }
}
