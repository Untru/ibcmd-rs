//! `--base` and `--sync` of a configuration export (Untru/ibcmd-rs#362): the
//! options of the platform's `config export` that write only what changed
//! since an earlier export and bring a folder in line with the configuration.
//! The semantics are the issue's; no run of the platform's own export with
//! these options is captured in the repository yet, so nothing here is
//! measured against it.
//!
//! What changed is read off the versions of the stored rows, as the issue
//! asks. The `versions` row of the Config table holds one version per row,
//! and the `ConfigDumpInfo.xml` an export writes lists them by row id
//! (`<Metadata name="…" id="…" configVersion="…">`, one top-level element per
//! row; a nested `<Metadata>` is a part of its owner's row, an attribute or a
//! command, and has no version of its own).
//!
//! - `--base=<ConfigDumpInfo.xml>`: a row whose entry the base lists with the
//!   same version and the same name is not converted at all, so none of its
//!   files is written. Every other row (changed, new, or not listed in a
//!   `ConfigDumpInfo.xml` at all) is exported exactly as a full export writes
//!   it, and `ConfigDumpInfo.xml` is written for the whole configuration.
//!   A rename converts every row (`renamed_since`): rows refer to an object
//!   by its id, so the rows whose files name a renamed object or attribute
//!   need not change their version while their files change
//!   (`Configuration.xml` lists every object by name).
//! - `--sync`: what a fresh full export would not leave in the folder is
//!   removed -- files of objects that are gone, files a changed object no
//!   longer produces, files left under an old name. Without `--base` every
//!   row is converted and every file the export did not write is removed.
//!
//! Each row writes the files of its own entry only: the converter of a row
//! (`dump_table_row_bytes`) writes its module text, its metadata XML and its
//! source asset from its own bytes and the indexes every row shares, and
//! `tests/export_base_sync.rs` checks on the fixtures that the files written
//! by the rows of the entries of a `ConfigDumpInfo.xml`, one entry at a time,
//! make up a full export, each file written by exactly one entry.
//!
//! A rename is told from the names `ConfigDumpInfo.xml` gives the rows and
//! their parts. A name it does not list (a predefined item named in another
//! object's file) can move without any of them: such a file keeps the old
//! name until a full export, the versions being the rule, as the issue asks.
//!
//! `--sync` touches only the export's own part of the folder:
//! `Configuration.xml`, `ConfigDumpInfo.xml`, `Ext/` and the metadata
//! collection folders (`Catalogs/`, ...), and never a dot directory, so a
//! `.git`, a README or a `manifest.json` beside the tree stays.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde::{Deserialize, Serialize};

use crate::cli::InfobaseConfigSourceVersion;
use crate::metadata_model::index::{NESTED_COLLECTIONS, ROOT_COLLECTIONS};

/// The file the export writes for the whole configuration.
pub(crate) const CONFIG_DUMP_INFO: &str = "ConfigDumpInfo.xml";

/// The one layout this export writes (`format="Hierarchical"`).
const HIERARCHICAL: &str = "Hierarchical";

/// One top-level `<Metadata>` of a ConfigDumpInfo.xml: one stored row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DumpInfoEntry {
    pub(crate) name: String,
    pub(crate) id: String,
    pub(crate) config_version: String,
}

/// A ConfigDumpInfo.xml as read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct DumpInfo {
    /// `format` of the root element (`Hierarchical`).
    pub(crate) layout: Option<String>,
    /// `version` of the root element: the XML format (`2.20`, `2.21`).
    pub(crate) format_version: Option<String>,
    /// The rows: the top-level `<Metadata>` of `<ConfigVersions>`.
    pub(crate) entries: Vec<DumpInfoEntry>,
    /// The parts of the rows: the `<Metadata>` nested in them (attributes,
    /// tabular sections, commands, ...), id -> name. They have no version.
    pub(crate) parts: BTreeMap<String, String>,
}

/// The attributes of an element by local name, unescaped.
fn attributes(start: &BytesStart<'_>) -> Result<BTreeMap<String, String>> {
    let mut attributes = BTreeMap::new();
    for attribute in start.attributes() {
        let attribute = attribute.context("bad XML attribute")?;
        let value = attribute
            .unescape_value()
            .context("bad XML attribute value")?
            .into_owned();
        attributes.insert(
            String::from_utf8_lossy(attribute.key.local_name().as_ref()).into_owned(),
            value,
        );
    }
    Ok(attributes)
}

impl DumpInfo {
    /// Parses a ConfigDumpInfo.xml. The top-level `<Metadata>` of
    /// `<ConfigVersions>` are rows, each with its version; the ones nested in
    /// them are parts of their row. Read as a stream rather than into a
    /// DOM: the file lists every row and part of the configuration (the tree
    /// of ERP УХ holds 140 709 files, `InfobaseConfigExportReport`).
    pub(crate) fn parse(xml: &[u8]) -> Result<Self> {
        let xml = xml.strip_prefix(b"\xef\xbb\xbf").unwrap_or(xml);
        let mut reader = Reader::from_reader(xml);
        let mut buffer = Vec::new();
        let mut info = Self::default();
        let mut ids = BTreeSet::new();
        // The local names of the open elements, the root first.
        let mut open = Vec::<String>::new();
        let mut root_seen = false;
        loop {
            let event = reader.read_event_into(&mut buffer).map_err(|error| {
                anyhow!("XML parse error at {}: {error}", reader.error_position())
            })?;
            let (start, empty) = match &event {
                Event::Start(start) => (start, false),
                Event::Empty(start) => (start, true),
                Event::End(_) => {
                    open.pop();
                    buffer.clear();
                    continue;
                }
                Event::Eof => {
                    if !open.is_empty() {
                        bail!("the file ended inside <{}>", open.last().unwrap());
                    }
                    break;
                }
                _ => {
                    buffer.clear();
                    continue;
                }
            };
            let name = String::from_utf8_lossy(start.local_name().as_ref()).into_owned();
            match open.len() {
                0 => {
                    if root_seen || name != "ConfigDumpInfo" {
                        bail!("the root element is <{name}>, not <ConfigDumpInfo>");
                    }
                    root_seen = true;
                    let attributes = attributes(start)?;
                    info.layout = attributes.get("format").cloned();
                    info.format_version = attributes.get("version").cloned();
                }
                2 if name == "Metadata" && open[1] == "ConfigVersions" => {
                    let mut attributes = attributes(start)?;
                    let mut take = |key: &str| {
                        attributes
                            .remove(key)
                            .ok_or_else(|| anyhow!("a <Metadata> of <ConfigVersions> has no {key}"))
                    };
                    let entry = DumpInfoEntry {
                        name: take("name")?,
                        id: take("id")?,
                        config_version: take("configVersion")?,
                    };
                    if !ids.insert(entry.id.clone()) {
                        bail!("the entry {} is listed twice", entry.id);
                    }
                    info.entries.push(entry);
                }
                depth if depth > 2 && name == "Metadata" && open[1] == "ConfigVersions" => {
                    let mut attributes = attributes(start)?;
                    if let (Some(id), Some(name)) =
                        (attributes.remove("id"), attributes.remove("name"))
                    {
                        info.parts.entry(id).or_insert(name);
                    }
                }
                _ => {}
            }
            if !empty {
                open.push(name);
            }
            buffer.clear();
        }
        if !root_seen {
            bail!("the file holds no <ConfigDumpInfo>");
        }
        Ok(info)
    }

    pub(crate) fn read(path: &Path) -> Result<Self> {
        let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
        Self::parse(&bytes).with_context(|| format!("failed to read {}", path.display()))
    }

    fn by_id(&self) -> HashMap<&str, &DumpInfoEntry> {
        self.entries
            .iter()
            .map(|entry| (entry.id.as_str(), entry))
            .collect()
    }
}

/// What `--base` and `--sync` asked of an export, read before the export
/// writes anything (the base may be the folder's own ConfigDumpInfo.xml).
#[derive(Clone, Debug)]
pub(crate) struct IncrementalExport {
    base: Option<(PathBuf, DumpInfo)>,
    sync: bool,
}

/// How an export with `--base` or `--sync` went, for the report.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IncrementalExportSummary {
    /// `incremental`: the rows of unchanged entries were not converted;
    /// `full`: every row was.
    pub mode: String,
    /// Why every row was converted, when it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<PathBuf>,
    pub sync: bool,
    /// Entries converted: new, or with a version or a name other than the
    /// base's.
    pub changed_entries: usize,
    /// Entries left as they are: neither converted nor written.
    pub unchanged_entries: usize,
    /// Entries of the base the configuration no longer has.
    pub removed_entries: usize,
    /// Files written, `ConfigDumpInfo.xml` included.
    pub written_files: usize,
    /// Files `--sync` removed, relative to the output directory.
    #[serde(default)]
    pub removed_files: Vec<String>,
}

/// The names ConfigDumpInfo.xml gives the configuration, read before any row
/// is converted.
#[derive(Clone, Debug, Default)]
pub(crate) struct ConfigurationNames {
    /// Rows named whole by the configuration's references, id -> name
    /// (`canonical_reference_names`); a row it lacks is its owner's name and
    /// a role.
    pub(crate) rows: BTreeMap<String, String>,
    /// The parts of the rows, id -> name (`child_reference_names`).
    pub(crate) parts: BTreeMap<String, String>,
}

/// Which rows an export converts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RowSelection {
    /// Rows whose entry the base lists with the same version and name: not
    /// converted, so nothing of theirs is written.
    pub(crate) unchanged: BTreeSet<String>,
    /// The role of each unchanged row named by its owner and a role
    /// (`Catalog.X.ObjectModule`): the last segment of its name in the base.
    /// The new ConfigDumpInfo.xml names such a row from the file its
    /// conversion writes (`config_dump_top_name`); the row is the base's, so
    /// its role is the base's.
    pub(crate) role_hints: BTreeMap<String, String>,
    /// Versioned entries converted: new, or with another version or name.
    pub(crate) changed: BTreeSet<String>,
    /// Entries of the base the configuration does not list.
    pub(crate) removed: BTreeSet<String>,
    /// Why every row is converted, when it is.
    pub(crate) full_reason: Option<String>,
}

impl IncrementalExport {
    /// `None` when neither option is given.
    pub(crate) fn load(base: Option<&Path>, sync: bool) -> Result<Option<Self>> {
        if base.is_none() && !sync {
            return Ok(None);
        }
        let base = match base {
            Some(path) => {
                let info = DumpInfo::read(path).with_context(|| {
                    format!("--base {} is not a ConfigDumpInfo.xml", path.display())
                })?;
                Some((path.to_path_buf(), info))
            }
            None => None,
        };
        Ok(Some(Self { base, sync }))
    }

    pub(crate) fn base_path(&self) -> Option<&Path> {
        self.base.as_ref().map(|(path, _)| path.as_path())
    }

    fn base(&self) -> Option<&DumpInfo> {
        self.base.as_ref().map(|(_, info)| info)
    }

    /// The rows to convert. `current` is the configuration's `versions` (row
    /// id -> configVersion), `names` the names its ConfigDumpInfo.xml gives,
    /// `None` when they could not be read.
    pub(crate) fn select(
        &self,
        current: &BTreeMap<String, String>,
        names: Option<&ConfigurationNames>,
        source_version: InfobaseConfigSourceVersion,
    ) -> RowSelection {
        let full = |reason: String| RowSelection {
            changed: current.keys().cloned().collect(),
            removed: self
                .base()
                .map(|base| {
                    base.entries
                        .iter()
                        .filter(|entry| !current.contains_key(&entry.id))
                        .map(|entry| entry.id.clone())
                        .collect()
                })
                .unwrap_or_default(),
            full_reason: Some(reason),
            ..RowSelection::default()
        };
        let Some(base) = self.base() else {
            return full("no --base: every row is exported".to_owned());
        };
        // Another XML format rewrites every file.
        if base.format_version.as_deref() != Some(source_version.as_str()) {
            return full(format!(
                "the base is of XML version {}, the export writes {}",
                base.format_version.as_deref().unwrap_or("(none)"),
                source_version.as_str()
            ));
        }
        if base
            .layout
            .as_deref()
            .is_some_and(|layout| layout != HIERARCHICAL)
        {
            return full(format!(
                "the base is of format {}, the export writes {HIERARCHICAL}",
                base.layout.as_deref().unwrap_or_default()
            ));
        }
        let Some(names) = names else {
            return full("the configuration's names could not be read".to_owned());
        };
        // A rename moves names in the files of other objects:
        // `Configuration.xml` lists every object by name, a subsystem its
        // content, a form the attributes it shows. Their rows refer to the
        // object by its id (the export names every reference through
        // `object_refs`), so their versions need not move with the name.
        // Only a full export renders those files as they are now.
        let renamed = renamed_since(base, current, names);
        if !renamed.is_empty() {
            let shown = renamed.iter().take(5).cloned().collect::<Vec<_>>();
            let more = renamed.len().saturating_sub(shown.len());
            return full(format!(
                "renamed since the base, which other objects' files name: {}{}",
                shown.join(", "),
                if more > 0 {
                    format!(" and {more} more")
                } else {
                    String::new()
                }
            ));
        }
        let listed = base.by_id();
        let mut selection = RowSelection::default();
        for (id, version) in current {
            let unchanged = listed
                .get(id.as_str())
                .filter(|entry| &entry.config_version == version)
                .and_then(|entry| same_name(id, &entry.name, &names.rows));
            match unchanged {
                Some(role) => {
                    selection.unchanged.insert(id.clone());
                    if let Some(role) = role {
                        selection.role_hints.insert(id.clone(), role);
                    }
                }
                None => {
                    selection.changed.insert(id.clone());
                }
            }
        }
        selection.removed = base
            .entries
            .iter()
            .filter(|entry| !current.contains_key(&entry.id))
            .map(|entry| entry.id.clone())
            .collect();
        selection
    }

    /// After the export: removes what `--sync` removes and sums up. `written`
    /// holds the files the export wrote, `ConfigDumpInfo.xml` among them.
    pub(crate) fn finish(
        &self,
        output_dir: &Path,
        written: &[PathBuf],
        selection: &RowSelection,
    ) -> Result<IncrementalExportSummary> {
        let removed_files = if self.sync {
            let current = DumpInfo::read(&output_dir.join(CONFIG_DUMP_INFO)).context(
                "the export wrote no readable ConfigDumpInfo.xml, which --sync needs to tell \
                 what belongs to the configuration",
            )?;
            sync_output_dir(
                output_dir,
                written,
                &current,
                self.base(),
                &selection.unchanged,
            )?
        } else {
            Vec::new()
        };
        Ok(IncrementalExportSummary {
            mode: if selection.full_reason.is_some() {
                "full"
            } else {
                "incremental"
            }
            .to_owned(),
            reason: selection.full_reason.clone(),
            base: self.base_path().map(Path::to_path_buf),
            sync: self.sync,
            changed_entries: selection.changed.len(),
            unchanged_entries: selection.unchanged.len(),
            removed_entries: selection.removed.len(),
            written_files: written.len(),
            removed_files,
        })
    }
}

/// `old -> new` for every row and part the base names otherwise than the
/// configuration does.
fn renamed_since(
    base: &DumpInfo,
    current: &BTreeMap<String, String>,
    names: &ConfigurationNames,
) -> Vec<String> {
    let mut renamed = Vec::new();
    for entry in &base.entries {
        if !current.contains_key(&entry.id) {
            continue;
        }
        let now = match names.rows.get(&entry.id) {
            Some(name) => Some(name.clone()),
            // `<owner>.<role>`: the owner's name now, the role the base's
            None => entry.id.rsplit_once('.').and_then(|(owner, _)| {
                let (_, role) = entry.name.rsplit_once('.')?;
                Some(format!("{}.{role}", names.rows.get(owner)?))
            }),
        };
        if let Some(now) = now
            && now != entry.name
        {
            renamed.push(format!("{} -> {now}", entry.name));
        }
    }
    for (id, name) in &base.parts {
        if let Some(now) = names.parts.get(id)
            && now != name
        {
            renamed.push(format!("{name} -> {now}"));
        }
    }
    renamed
}

/// `Some(role)` when the configuration names the row `id` as the base did
/// (`base_name`): `Some(None)` for a row named whole by the configuration's
/// references, `Some(Some(role))` for one named by its owner and a role.
/// `None` when the name moved or cannot be told.
fn same_name(
    id: &str,
    base_name: &str,
    names: &BTreeMap<String, String>,
) -> Option<Option<String>> {
    if let Some(current) = names.get(id) {
        return (current == base_name).then_some(None);
    }
    // `config_dump_top_name`: a row the references do not name is its
    // owner's reference (`<owner id>.<n>`) and a role.
    let (owner_id, _) = id.rsplit_once('.')?;
    let owner = names.get(owner_id)?;
    let (base_owner, role) = base_name.rsplit_once('.')?;
    (base_owner == owner && !role.is_empty()).then(|| Some(role.to_owned()))
}

/// The folder of a metadata kind in the export (`Catalog` -> `Catalogs`,
/// `Form` -> `Forms`, `Command` -> `Commands`). A command has no file of its
/// own but its module lies in `<owner>/Commands/<name>/Ext/CommandModule.bsl`
/// (`mssql_dump` joins `Commands` for it).
fn collection_folder(kind: &str) -> Option<&'static str> {
    ROOT_COLLECTIONS
        .iter()
        .chain(NESTED_COLLECTIONS)
        .chain(&[("Commands", "Command")])
        .find_map(|(folder, of)| (*of == kind).then_some(*folder))
}

/// The folder of the object `Kind.Name[.Kind.Name]...` relative to the
/// export root: `Catalogs/X`, `Catalogs/X/Forms/F`, `Subsystems/A/Subsystems/B`;
/// empty for the configuration itself. `None` when a kind has no known folder.
fn object_dir(segments: &[&str]) -> Option<String> {
    if let ["Configuration", _] = segments {
        return Some(String::new());
    }
    if segments.is_empty() || segments.len() % 2 != 0 {
        return None;
    }
    let mut parts = Vec::with_capacity(segments.len());
    for pair in segments.chunks(2) {
        let [kind, name] = pair else {
            return None;
        };
        if name.is_empty() {
            return None;
        }
        parts.push(collection_folder(kind)?);
        parts.push(*name);
    }
    Some(parts.join("/"))
}

/// Which entry of a ConfigDumpInfo.xml a file of the export belongs to,
/// by the entry's name:
///
/// - an object `Kind.Name` writes `<folder>.xml` (`Catalogs/X.xml`); the
///   configuration writes `Configuration.xml`;
/// - a role of an object, `<object>.<Role>`, writes `<folder>/Ext/<Role>.<ext>`
///   and anything under `<folder>/Ext/<Role>/` (`Ext/Form.xml` and
///   `Ext/Form/Module.bsl` of a form, `Ext/Help.xml` and `Ext/Help/ru.html`);
///   the configuration's roles lie in the root's `Ext/`.
///
/// The name of a role is the file name `ConfigDumpInfo.xml` takes it from
/// (`config_dump_top_name` names it by the stem of the file the row writes),
/// so the two directions agree by construction.
#[derive(Debug, Default)]
pub(crate) struct Ownership<'a> {
    files: HashMap<String, &'a str>,
    role_stems: HashMap<String, &'a str>,
    folders: HashMap<String, &'a str>,
}

impl<'a> Ownership<'a> {
    pub(crate) fn new(entries: &'a [DumpInfoEntry]) -> Self {
        let mut ownership = Self::default();
        for entry in entries {
            let segments = entry.name.split('.').collect::<Vec<_>>();
            if segments.len() < 2 {
                continue;
            }
            let id = entry.id.as_str();
            if segments.len() % 2 == 0 {
                let Some(dir) = object_dir(&segments) else {
                    continue;
                };
                if dir.is_empty() {
                    ownership.files.insert("Configuration.xml".to_owned(), id);
                    ownership.folders.insert("Ext".to_owned(), id);
                } else {
                    ownership.files.insert(format!("{dir}.xml"), id);
                    ownership.folders.insert(dir, id);
                }
            } else {
                let (role, owner) = segments.split_last().expect("two segments at least");
                let Some(dir) = object_dir(owner) else {
                    continue;
                };
                let stem = if dir.is_empty() {
                    format!("Ext/{role}")
                } else {
                    format!("{dir}/Ext/{role}")
                };
                ownership.role_stems.insert(stem, id);
            }
        }
        ownership
    }

    /// The entry whose own file `relative` (`/`-separated) is: the object's
    /// XML, or a file of a role.
    pub(crate) fn exact(&self, relative: &str) -> Option<&'a str> {
        if let Some(id) = self.files.get(relative).copied() {
            return Some(id);
        }
        let (folder, file) = relative.rsplit_once('/').unwrap_or(("", relative));
        if let Some((stem, _)) = file.rsplit_once('.') {
            let stem = if folder.is_empty() {
                stem.to_owned()
            } else {
                format!("{folder}/{stem}")
            };
            if let Some(id) = self.role_stems.get(&stem).copied() {
                return Some(id);
            }
        }
        ancestors(relative).find_map(|ancestor| self.role_stems.get(ancestor).copied())
    }

    /// The innermost object whose folder holds `relative`.
    pub(crate) fn folder(&self, relative: &str) -> Option<&'a str> {
        ancestors(relative).find_map(|ancestor| self.folders.get(ancestor).copied())
    }
}

/// The folders above a relative path, the innermost first.
fn ancestors(relative: &str) -> impl Iterator<Item = &str> {
    let mut rest = relative;
    std::iter::from_fn(move || {
        let (parent, _) = rest.rsplit_once('/')?;
        rest = parent;
        Some(parent)
    })
}

/// Whether `relative` lies in the export's own part of a folder:
/// `Configuration.xml`, `ConfigDumpInfo.xml`, `Ext/` and the collection
/// folders of the configuration's root kinds; never under a dot entry.
pub(crate) fn in_export_territory(relative: &str) -> bool {
    if relative.split('/').any(|part| part.starts_with('.')) {
        return false;
    }
    match relative.split_once('/') {
        None => relative == "Configuration.xml" || relative == CONFIG_DUMP_INFO,
        Some((first, _)) => {
            first == "Ext" || ROOT_COLLECTIONS.iter().any(|(folder, _)| *folder == first)
        }
    }
}

/// Whether `--sync` removes the file `relative` that the export did not
/// write. `unchanged` are the rows the export did not convert.
fn stale(
    relative: &str,
    current: &Ownership<'_>,
    base: Option<&Ownership<'_>>,
    unchanged: &BTreeSet<String>,
) -> bool {
    // Every row was converted: what the export did not write, a fresh export
    // does not hold.
    if unchanged.is_empty() {
        return true;
    }
    // A file of an entry of the configuration: kept when the entry was left
    // as it is, stale when the entry was exported again and did not write it.
    if let Some(id) = current.exact(relative) {
        return !unchanged.contains(id);
    }
    // A file the base gives an entry at a path the configuration gives no
    // entry: the entry is gone or named otherwise now.
    if base.is_some_and(|base| base.exact(relative).is_some()) {
        return true;
    }
    // Anything else in an existing object's folder is kept (it cannot be
    // told from the versions); outside every object's folder it is stale.
    current.folder(relative).is_none()
}

/// `--sync`: removes the files of the export's part of `output_dir` that a
/// fresh export would not hold, and the folders that leaves empty. Returns
/// the files removed, `/`-separated and relative to `output_dir`.
pub(crate) fn sync_output_dir(
    output_dir: &Path,
    written: &[PathBuf],
    current: &DumpInfo,
    base: Option<&DumpInfo>,
    unchanged: &BTreeSet<String>,
) -> Result<Vec<String>> {
    let written = written
        .iter()
        .filter_map(|path| path.strip_prefix(output_dir).ok())
        .map(|path| written_key(&relative_string(path)))
        .collect::<HashSet<_>>();
    let current_owners = Ownership::new(&current.entries);
    let base_owners = base.map(|base| Ownership::new(&base.entries));
    let mut removed = Vec::new();
    let walk = walkdir::WalkDir::new(output_dir)
        .min_depth(1)
        .into_iter()
        .filter_entry(|entry| !entry.file_name().to_string_lossy().starts_with('.'));
    for entry in walk {
        let entry = entry.with_context(|| format!("failed to walk {}", output_dir.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = relative_string(
            entry
                .path()
                .strip_prefix(output_dir)
                .context("a walked path left the output directory")?,
        );
        if relative == CONFIG_DUMP_INFO
            || !in_export_territory(&relative)
            || written.contains(&written_key(&relative))
            || !stale(&relative, &current_owners, base_owners.as_ref(), unchanged)
        {
            continue;
        }
        fs::remove_file(entry.path())
            .with_context(|| format!("failed to remove {}", entry.path().display()))?;
        removed.push(relative);
    }
    // The folders the removed files leave empty, the innermost first; a
    // folder that still holds anything stays.
    let mut folders = removed
        .iter()
        .flat_map(|relative| ancestors(relative))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    folders.sort_by_key(|folder| std::cmp::Reverse(folder.matches('/').count()));
    for folder in folders {
        let _ = fs::remove_dir(output_dir.join(&folder));
    }
    removed.sort();
    Ok(removed)
}

fn relative_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// How a written file is told from the folder's: on Windows a file written
/// over an existing one keeps the existing name's case (NTFS does not rename
/// it), so after an object renamed in case only the folder lists the old
/// spelling of the file just written.
fn written_key(relative: &str) -> String {
    if cfg!(windows) {
        relative.to_lowercase()
    } else {
        relative.to_owned()
    }
}

/// The output directory of an export with `--base` or `--sync`: it may hold
/// an earlier export (the point of both), so it is created when missing and
/// otherwise left as it is.
pub(crate) fn prepare_output_dir(path: &Path) -> Result<()> {
    if path.exists() {
        if !path.is_dir() {
            bail!(
                "output path exists and is not a directory: {}",
                path.display()
            );
        }
        return Ok(());
    }
    fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const V1: &str = "11111111111111111111111111111111111111";
    const V2: &str = "22222222222222222222222222222222222222";

    #[test]
    fn a_truncated_base_manifest_is_refused() {
        for xml in [
            "<ConfigDumpInfo format=\"Hierarchical\" version=\"2.20\"><ConfigVersions>",
            "<ConfigDumpInfo format=\"Hierarchical\" version=\"2.20\"><ConfigVersions><Metadata name=\"Catalog.X\" id=\"a\" configVersion=\"v1\"/>",
        ] {
            assert!(DumpInfo::parse(xml.as_bytes()).is_err(), "accepted {xml}");
        }
    }

    fn entry(name: &str, id: &str, version: &str) -> DumpInfoEntry {
        DumpInfoEntry {
            name: name.to_owned(),
            id: id.to_owned(),
            config_version: version.to_owned(),
        }
    }

    fn dump_info(version: &str, entries: Vec<DumpInfoEntry>) -> DumpInfo {
        DumpInfo {
            layout: Some(HIERARCHICAL.to_owned()),
            format_version: Some(version.to_owned()),
            entries,
            parts: BTreeMap::new(),
        }
    }

    fn with_base(base: DumpInfo, sync: bool) -> IncrementalExport {
        IncrementalExport {
            base: Some((PathBuf::from("ConfigDumpInfo.xml"), base)),
            sync,
        }
    }

    #[test]
    fn only_the_top_level_entries_are_rows() {
        let xml = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<ConfigDumpInfo xmlns=\"http://v8.1c.ru/8.3/xcf/dumpinfo\" format=\"Hierarchical\" version=\"2.20\">\r\n\
\t<ConfigVersions>\r\n\
\t\t<Metadata name=\"Catalog.Товары\" id=\"a\" configVersion=\"v1\">\r\n\
\t\t\t<Metadata name=\"Catalog.Товары.Attribute.Цена\" id=\"b\"/>\r\n\
\t\t</Metadata>\r\n\
\t\t<Metadata name=\"Catalog.Товары.ObjectModule\" id=\"a.0\" configVersion=\"v2\"/>\r\n\
\t\t<Metadata name=\"Language.Р&amp;Д\" id=\"c\" configVersion=\"v3\"/>\r\n\
\t</ConfigVersions>\r\n\
</ConfigDumpInfo>";
        let info = DumpInfo::parse(xml.as_bytes()).unwrap();
        assert_eq!(info.layout.as_deref(), Some("Hierarchical"));
        assert_eq!(info.format_version.as_deref(), Some("2.20"));
        assert_eq!(
            info.entries,
            vec![
                entry("Catalog.Товары", "a", "v1"),
                entry("Catalog.Товары.ObjectModule", "a.0", "v2"),
                entry("Language.Р&Д", "c", "v3"),
            ]
        );
        assert_eq!(
            info.parts,
            BTreeMap::from([("b".to_owned(), "Catalog.Товары.Attribute.Цена".to_owned())])
        );
        // an empty configuration lists nothing
        let empty =
            DumpInfo::parse(b"<ConfigDumpInfo version=\"2.20\"><ConfigVersions/></ConfigDumpInfo>")
                .unwrap();
        assert!(empty.entries.is_empty());
    }

    #[test]
    fn a_file_that_is_no_config_dump_info_is_refused() {
        assert!(DumpInfo::parse(b"<MetaDataObject/>").is_err());
        assert!(DumpInfo::parse(b"not xml <").is_err());
        let twice = "<ConfigDumpInfo version=\"2.20\"><ConfigVersions>\
<Metadata name=\"Language.Р\" id=\"c\" configVersion=\"v\"/>\
<Metadata name=\"Language.Р\" id=\"c\" configVersion=\"v\"/>\
</ConfigVersions></ConfigDumpInfo>";
        assert!(DumpInfo::parse(twice.as_bytes()).is_err());
        let unversioned = "<ConfigDumpInfo version=\"2.20\"><ConfigVersions>\
<Metadata name=\"Language.Р\" id=\"c\"/></ConfigVersions></ConfigDumpInfo>";
        assert!(DumpInfo::parse(unversioned.as_bytes()).is_err());
    }

    fn names(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(id, name)| ((*id).to_owned(), (*name).to_owned()))
            .collect()
    }

    fn named(rows: &[(&str, &str)]) -> ConfigurationNames {
        ConfigurationNames {
            rows: names(rows),
            parts: BTreeMap::new(),
        }
    }

    fn versions(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        names(pairs)
    }

    #[test]
    fn a_row_is_left_out_when_its_version_and_name_are_the_bases() {
        let base = dump_info(
            "2.20",
            vec![
                entry("Catalog.Товары", "a", V1),
                entry("Catalog.Товары.ObjectModule", "a.0", V1),
                entry("Catalog.Товары.Form.Форма", "f", V1),
                entry("Catalog.Товары.Form.Форма.Form", "f.0", V1),
                entry("Catalog.Удалённый", "d", V1),
            ],
        );
        let current = versions(&[
            ("a", V1),
            ("a.0", V2),
            ("f", V1),
            ("f.0", V1),
            ("n", V1),
            ("n.0", V1),
        ]);
        let refs = named(&[
            ("a", "Catalog.Товары"),
            ("f", "Catalog.Товары.Form.Форма"),
            ("n", "Catalog.Новый"),
        ]);
        let selection = with_base(base, false).select(
            &current,
            Some(&refs),
            InfobaseConfigSourceVersion::V2_20,
        );
        assert_eq!(selection.full_reason, None);
        assert_eq!(
            selection.unchanged,
            BTreeSet::from(["a", "f", "f.0"].map(str::to_owned))
        );
        // the module changed; the new object and its module are not listed
        assert_eq!(
            selection.changed,
            BTreeSet::from(["a.0", "n", "n.0"].map(str::to_owned))
        );
        assert_eq!(selection.removed, BTreeSet::from(["d".to_owned()]));
        // the form body is named by its owner and the base's role
        assert_eq!(
            selection.role_hints,
            BTreeMap::from([("f.0".to_owned(), "Form".to_owned())])
        );
    }

    #[test]
    fn a_rename_exports_every_row() {
        let base = dump_info(
            "2.20",
            vec![
                entry("Catalog.Старый", "a", V1),
                entry("Catalog.Старый.ObjectModule", "a.0", V1),
                entry("Catalog.Старый.Form.Форма", "f", V1),
                entry("Catalog.Старый.Form.Форма.Form", "f.0", V1),
                entry("Configuration.К", "c", V1),
            ],
        );
        // The catalog row is rewritten by the rename (its version moves); its
        // form and module rows keep theirs, and so does the configuration,
        // whose Configuration.xml names the catalog.
        let current = versions(&[("a", V2), ("a.0", V1), ("f", V1), ("f.0", V1), ("c", V1)]);
        let refs = named(&[
            ("a", "Catalog.Новый"),
            ("f", "Catalog.Новый.Form.Форма"),
            ("c", "Configuration.К"),
        ]);
        let selection =
            with_base(base, true).select(&current, Some(&refs), InfobaseConfigSourceVersion::V2_20);
        assert!(selection.unchanged.is_empty(), "{selection:?}");
        assert_eq!(selection.changed.len(), 5);
        let reason = selection.full_reason.unwrap();
        assert!(
            reason.contains("Catalog.Старый -> Catalog.Новый"),
            "{reason}"
        );
        assert!(
            reason.contains("Catalog.Старый.ObjectModule -> Catalog.Новый.ObjectModule"),
            "{reason}"
        );
    }

    #[test]
    fn a_part_renamed_exports_every_row() {
        let mut base = dump_info(
            "2.20",
            vec![
                entry("Catalog.Товары", "a", V1),
                entry("Configuration.К", "c", V1),
            ],
        );
        base.parts
            .insert("p".to_owned(), "Catalog.Товары.Attribute.Цена".to_owned());
        base.parts.insert(
            "q".to_owned(),
            "Catalog.Товары.Attribute.Удалённый".to_owned(),
        );
        let current = versions(&[("a", V2), ("c", V1)]);
        let mut refs = named(&[("a", "Catalog.Товары"), ("c", "Configuration.К")]);
        // a part removed is no rename
        refs.parts
            .insert("p".to_owned(), "Catalog.Товары.Attribute.Цена".to_owned());
        let export = with_base(base, false);
        let kept = export.select(&current, Some(&refs), InfobaseConfigSourceVersion::V2_20);
        assert_eq!(kept.full_reason, None);
        assert_eq!(kept.unchanged, BTreeSet::from(["c".to_owned()]));
        refs.parts.insert(
            "p".to_owned(),
            "Catalog.Товары.Attribute.Стоимость".to_owned(),
        );
        let renamed = export.select(&current, Some(&refs), InfobaseConfigSourceVersion::V2_20);
        assert!(renamed.unchanged.is_empty());
        assert!(
            renamed
                .full_reason
                .unwrap()
                .contains("Catalog.Товары.Attribute.Цена -> Catalog.Товары.Attribute.Стоимость")
        );
    }

    #[test]
    fn another_format_or_no_names_exports_every_row() {
        let base = dump_info("2.20", vec![entry("Catalog.Товары", "a", V1)]);
        let current = versions(&[("a", V1)]);
        let refs = named(&[("a", "Catalog.Товары")]);
        let export = with_base(base.clone(), false);
        let other = export.select(&current, Some(&refs), InfobaseConfigSourceVersion::V2_21);
        assert!(other.unchanged.is_empty());
        assert!(other.full_reason.unwrap().contains("2.21"));
        let unnamed = export.select(&current, None, InfobaseConfigSourceVersion::V2_20);
        assert!(unnamed.unchanged.is_empty() && unnamed.full_reason.is_some());
        let plain = with_base(
            DumpInfo {
                layout: Some("Plain".to_owned()),
                ..base
            },
            false,
        )
        .select(&current, Some(&refs), InfobaseConfigSourceVersion::V2_20);
        assert!(plain.unchanged.is_empty() && plain.full_reason.is_some());
        let sync_only = IncrementalExport {
            base: None,
            sync: true,
        }
        .select(&current, Some(&refs), InfobaseConfigSourceVersion::V2_20);
        assert_eq!(sync_only.changed.len(), 1);
        assert!(sync_only.unchanged.is_empty() && sync_only.full_reason.is_some());
    }

    #[test]
    fn the_files_of_an_entry_follow_its_name() {
        let entries = vec![
            entry("Configuration.Конф", "c", V1),
            entry("Configuration.Конф.ManagedApplicationModule", "c.6", V1),
            entry("Configuration.Конф.ParentConfigurations", "c.4", V1),
            entry("Catalog.Товары", "a", V1),
            entry("Catalog.Товары.ObjectModule", "a.0", V1),
            entry("Catalog.Товары.Help", "a.1", V1),
            entry("Catalog.Товары.Form.Форма", "f", V1),
            entry("Catalog.Товары.Form.Форма.Form", "f.0", V1),
            entry("Catalog.Товары.Command.Печать.CommandModule", "k.2", V1),
            entry("Catalog.Товары.Template.Макет", "t", V1),
            entry("Catalog.Товары.Template.Макет.Template", "t.0", V1),
            entry("CommonForm.Общая", "g", V1),
            entry("CommonForm.Общая.Form", "g.0", V1),
            entry("Subsystem.А", "s", V1),
            entry("Subsystem.А.Subsystem.Б", "s2", V1),
            entry("Subsystem.А.Subsystem.Б.CommandInterface", "s2.1", V1),
            entry("CalculationRegister.Р.Recalculation.П", "r", V1),
            entry("Language.Русский", "l", V1),
            // a kind without a known folder owns nothing
            entry("Unknown.X", "u", V1),
        ];
        let owners = Ownership::new(&entries);
        for (path, id) in [
            ("Configuration.xml", "c"),
            ("Ext/ManagedApplicationModule.bsl", "c.6"),
            ("Ext/ParentConfigurations.bin", "c.4"),
            ("Ext/ParentConfigurations/Поставщик.cf", "c.4"),
            ("Catalogs/Товары.xml", "a"),
            ("Catalogs/Товары/Ext/ObjectModule.bsl", "a.0"),
            ("Catalogs/Товары/Ext/Help.xml", "a.1"),
            ("Catalogs/Товары/Ext/Help/ru.html", "a.1"),
            ("Catalogs/Товары/Forms/Форма.xml", "f"),
            ("Catalogs/Товары/Forms/Форма/Ext/Form.xml", "f.0"),
            ("Catalogs/Товары/Forms/Форма/Ext/Form/Module.bsl", "f.0"),
            (
                "Catalogs/Товары/Commands/Печать/Ext/CommandModule.bsl",
                "k.2",
            ),
            ("Catalogs/Товары/Templates/Макет.xml", "t"),
            ("Catalogs/Товары/Templates/Макет/Ext/Template.xml", "t.0"),
            ("CommonForms/Общая.xml", "g"),
            ("CommonForms/Общая/Ext/Form.xml", "g.0"),
            ("CommonForms/Общая/Ext/Form/Module.bsl", "g.0"),
            ("Subsystems/А.xml", "s"),
            ("Subsystems/А/Subsystems/Б.xml", "s2"),
            ("Subsystems/А/Subsystems/Б/Ext/CommandInterface.xml", "s2.1"),
            ("CalculationRegisters/Р/Recalculations/П.xml", "r"),
            ("Languages/Русский.xml", "l"),
        ] {
            assert_eq!(owners.exact(path), Some(id), "{path}");
        }
        for path in [
            "Catalogs/Товары/Ext/Predefined.xml",
            "Catalogs/Другой.xml",
            "Ext/Splash.xml",
            "Unknowns/X.xml",
        ] {
            assert_eq!(owners.exact(path), None, "{path}");
        }
        // the folders a file lies in, the innermost object first
        assert_eq!(
            owners.folder("Catalogs/Товары/Ext/Predefined.xml"),
            Some("a")
        );
        assert_eq!(
            owners.folder("Catalogs/Товары/Forms/Форма/Ext/Other.xml"),
            Some("f")
        );
        assert_eq!(owners.folder("Ext/Splash.xml"), Some("c"));
        assert_eq!(owners.folder("Catalogs/Другой/Ext/ObjectModule.bsl"), None);
        assert_eq!(owners.folder("Configuration.xml"), None);
    }

    #[test]
    fn sync_keeps_to_the_exports_part_of_the_folder() {
        for path in [
            "Configuration.xml",
            "ConfigDumpInfo.xml",
            "Ext/ManagedApplicationModule.bsl",
            "Catalogs/Товары.xml",
            "Languages/Русский.xml",
            "CommonModules/М/Ext/Module.bsl",
        ] {
            assert!(in_export_territory(path), "{path}");
        }
        for path in [
            "README.md",
            "manifest.json",
            ".git/config",
            "Catalogs/.keep",
            "Catalogs/Товары/.idea/x.xml",
            "Docs/Catalogs/Товары.xml",
            "Config/row.bin",
        ] {
            assert!(!in_export_territory(path), "{path}");
        }
    }

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "ibcmd-rs-incremental-{tag}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn put(&self, relative: &str) -> PathBuf {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, relative).unwrap();
            path
        }

        fn files(&self) -> BTreeSet<String> {
            walkdir::WalkDir::new(&self.0)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_file())
                .map(|entry| relative_string(entry.path().strip_prefix(&self.0).unwrap()))
                .collect()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn sync_removes_what_a_fresh_export_would_not_hold() {
        let dir = Scratch::new("sync");
        let base = dump_info(
            "2.20",
            vec![
                entry("Configuration.К", "c", V1),
                entry("Catalog.Товары", "a", V1),
                entry("Catalog.Товары.ObjectModule", "a.0", V1),
                entry("Catalog.Товары.Form.Форма", "f", V1),
                entry("Catalog.Товары.Form.Форма.Form", "f.0", V1),
                entry("Catalog.Старый", "o", V1),
                entry("Catalog.Старый.ManagerModule", "o.3", V1),
                entry("Document.Д", "d", V1),
                entry("Document.Д.ObjectModule", "d.0", V1),
            ],
        );
        let current = dump_info(
            "2.20",
            vec![
                entry("Configuration.К", "c", V1),
                entry("Catalog.Товары", "a", V1),
                entry("Catalog.Товары.ObjectModule", "a.0", V1),
                entry("Catalog.Товары.Form.Форма", "f", V1),
                entry("Catalog.Товары.Form.Форма.Form", "f.0", V2),
                // the document is renamed
                entry("Document.Новый", "d", V2),
                entry("Document.Новый.ObjectModule", "d.0", V1),
            ],
        );
        for path in [
            "ConfigDumpInfo.xml",
            "Configuration.xml",
            "Catalogs/Товары.xml",
            "Catalogs/Товары/Ext/ObjectModule.bsl",
            // a file in an unchanged object's folder no entry names: kept
            "Catalogs/Товары/Ext/Predefined.xml",
            "Catalogs/Товары/Forms/Форма.xml",
            // the changed form body no longer has a module
            "Catalogs/Товары/Forms/Форма/Ext/Form.xml",
            "Catalogs/Товары/Forms/Форма/Ext/Form/Module.bsl",
            // a removed catalog
            "Catalogs/Старый.xml",
            "Catalogs/Старый/Ext/ManagerModule.bsl",
            // the renamed document's files under the old name
            "Documents/Д.xml",
            "Documents/Д/Ext/ObjectModule.bsl",
            // a stray file in a collection folder
            "Catalogs/Лишний.xml",
            // outside the export's part
            "README.md",
            ".git/HEAD",
            "Catalogs/.gitkeep",
        ] {
            dir.put(path);
        }
        let written = [
            "ConfigDumpInfo.xml",
            "Catalogs/Товары/Forms/Форма/Ext/Form.xml",
            "Documents/Новый.xml",
            "Documents/Новый/Ext/ObjectModule.bsl",
        ]
        .map(|path| dir.put(path));
        let unchanged = BTreeSet::from(["c", "a", "a.0", "f"].map(str::to_owned));
        let removed = sync_output_dir(&dir.0, &written, &current, Some(&base), &unchanged).unwrap();
        assert_eq!(
            removed,
            vec![
                "Catalogs/Лишний.xml",
                "Catalogs/Старый.xml",
                "Catalogs/Старый/Ext/ManagerModule.bsl",
                "Catalogs/Товары/Forms/Форма/Ext/Form/Module.bsl",
                "Documents/Д.xml",
                "Documents/Д/Ext/ObjectModule.bsl",
            ]
        );
        assert_eq!(
            dir.files(),
            BTreeSet::from(
                [
                    ".git/HEAD",
                    "Catalogs/.gitkeep",
                    "Catalogs/Товары.xml",
                    "Catalogs/Товары/Ext/ObjectModule.bsl",
                    "Catalogs/Товары/Ext/Predefined.xml",
                    "Catalogs/Товары/Forms/Форма.xml",
                    "Catalogs/Товары/Forms/Форма/Ext/Form.xml",
                    "ConfigDumpInfo.xml",
                    "Configuration.xml",
                    "Documents/Новый.xml",
                    "Documents/Новый/Ext/ObjectModule.bsl",
                    "README.md",
                ]
                .map(str::to_owned)
            )
        );
        // the folders the removed files left empty are gone
        assert!(!dir.0.join("Catalogs/Старый").exists());
        assert!(!dir.0.join("Documents/Д").exists());
        assert!(!dir.0.join("Catalogs/Товары/Forms/Форма/Ext/Form").exists());
        assert!(dir.0.join("Catalogs").is_dir());
    }

    #[test]
    fn sync_without_a_base_keeps_only_what_the_export_wrote() {
        let dir = Scratch::new("sync-full");
        let current = dump_info(
            "2.20",
            vec![
                entry("Configuration.К", "c", V1),
                entry("Catalog.Товары", "a", V1),
            ],
        );
        for path in [
            "Catalogs/Товары/Ext/Predefined.xml",
            "Catalogs/Лишний.xml",
            "Ext/Splash.xml",
            "README.md",
        ] {
            dir.put(path);
        }
        let written = [
            "ConfigDumpInfo.xml",
            "Configuration.xml",
            "Catalogs/Товары.xml",
        ]
        .map(|path| dir.put(path));
        let removed = sync_output_dir(&dir.0, &written, &current, None, &BTreeSet::new()).unwrap();
        assert_eq!(
            removed,
            vec![
                "Catalogs/Лишний.xml",
                "Catalogs/Товары/Ext/Predefined.xml",
                "Ext/Splash.xml",
            ]
        );
        assert!(dir.0.join("README.md").exists());
        assert!(!dir.0.join("Ext").exists());
    }

    #[test]
    fn the_output_directory_may_hold_an_earlier_export() {
        let dir = Scratch::new("prepare");
        dir.put("Catalogs/Товары.xml");
        prepare_output_dir(&dir.0).unwrap();
        assert!(dir.0.join("Catalogs/Товары.xml").exists());
        let missing = dir.0.join("new/out");
        prepare_output_dir(&missing).unwrap();
        assert!(missing.is_dir());
        assert!(prepare_output_dir(&dir.0.join("Catalogs/Товары.xml")).is_err());
    }
}
