//! Whole-tree load of a configuration extension.
//!
//! The extension's active image is exported (the export equals the platform's
//! `config export --extension`, measured on the four БСП extensions), the tree
//! the user hands in is compared with that export file by file, and only the
//! objects a changed file belongs to are compiled -- against the active rows,
//! by the same offline staging compiler `cf load` uses for `.cfe` containers
//! (`crate::load::compiled::compile_edit`). The proposed image is exported
//! again and must equal the tree, `ConfigDumpInfo.xml` aside: an edit the
//! compiler does not write faithfully is refused by name and nothing is staged.
//!
//! Activation publishes rows and restructures nothing, so an edit that changes
//! what a database table holds is refused here too: only the bodies of an
//! object (module, form, template, picture, rights, help) and the descriptors
//! of the families that own no table are taken.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use ibcmd_core::limits::ResourceLimits;
use ibcmd_core::storage::{StorageEntry, StorageImage};

use crate::legacy_version::InfobaseConfigSourceVersion;
use crate::load::compiled::{compile_edit, is_dropped, new_entry, with_payload};
use crate::load::{TreeDiff, keys_by_output, relative_files};
use crate::mssql_extension_stage::ExtensionStageRow;
use crate::sql::SqlExec;

const CONFIG_DUMP_INFO: &str = "ConfigDumpInfo.xml";

/// Families whose objects own no database table -- (source directory, the tag
/// of the root's `<ChildObjects>` line) -- so a change of the descriptor
/// itself, or an object of the family coming or going, restructures nothing
/// (`Forms/` and `Templates/` of an object are nested and always allowed to
/// change in place).
const TABLELESS_FAMILIES: &[(&str, &str)] = &[
    ("CommonForms", "CommonForm"),
    ("CommonModules", "CommonModule"),
    ("CommonPictures", "CommonPicture"),
    ("CommonTemplates", "CommonTemplate"),
    ("Roles", "Role"),
];

/// What a whole-tree load proposes.
#[derive(Debug)]
pub(crate) struct TreeEdit {
    /// The complete content rows of the proposed image (no `configinfo`).
    pub rows: Vec<ExtensionStageRow>,
    /// The files of the tree that differ from the active image's export.
    pub changed_files: Vec<String>,
    /// The objects compiled, as source prefixes.
    pub compiled_objects: Vec<String>,
    /// The rows that replace or add an active row.
    pub replaced_rows: usize,
}

/// A directory removed when dropped, on a panic too.
struct Scratch(PathBuf, std::cell::Cell<bool>);

impl Scratch {
    fn new(tag: &str) -> Self {
        Self(
            std::env::temp_dir().join(format!(
                "ibcmd-extload-{tag}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or_default()
            )),
            std::cell::Cell::new(false),
        )
    }

    /// Lab aid: `IBCMD_RS_EXTLOAD_KEEP=1` leaves the directory for a look.
    fn keep_if_asked(&self) -> bool {
        let keep = std::env::var_os("IBCMD_RS_EXTLOAD_KEEP").is_some();
        self.1.set(keep);
        keep
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if !self.1.get() {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

/// Compiles the difference between `input_dir` and the export of `active`;
/// `None` when the tree is the export. With `prefixes` (source paths: an object
/// as `Catalogs/К`, a file) only the files under them are taken, and every
/// other file of the proposal stays the active image's.
///
/// One extension per process: the offline staging compiler reads its base rows
/// from a process-wide table.
pub(crate) fn compile_tree_edit(
    sql: &SqlExec,
    database: &str,
    source_version: InfobaseConfigSourceVersion,
    input_dir: &Path,
    active: &StorageImage,
    prefixes: &[String],
) -> Result<Option<TreeEdit>> {
    let base_export = Scratch::new("base");
    let report = crate::mssql_dump::extension::export_extension_image_to_source(
        active,
        &base_export.0,
        source_version,
        Some(crate::mssql_dump::extension::base_index_provider(
            sql,
            database,
            source_version,
        )),
    )
    .context("failed to export the active extension image for comparison")?;
    if report.storage.failed > 0 || report.storage.opaque > 0 {
        bail!(
            "the export of the active image has {} failed and {} opaque rows, so the tree cannot \
             be compared with it; nothing can be loaded whole",
            report.storage.failed,
            report.storage.opaque
        );
    }
    let mut keys = keys_by_output(&report);
    let mut diff = diff_trees_normalized(&base_export.0, input_dir)?;
    let prefixes = prefixes
        .iter()
        .map(|prefix| prefix.trim_matches('/').replace('\\', "/"))
        .collect::<Vec<_>>();
    if !prefixes.is_empty() {
        for list in [&mut diff.changed, &mut diff.added, &mut diff.removed] {
            list.retain(|path| under_prefixes(path, &prefixes));
        }
    }
    add_side_file_keys(
        &mut keys,
        diff.changed.iter().chain(&diff.added).chain(&diff.removed),
    );
    if diff.changed.is_empty() && diff.added.is_empty() && diff.removed.is_empty() {
        return Ok(None);
    }
    let mut version_edit = None;
    let root_edit_supported = diff.changed.iter().any(|path| path == "Configuration.xml")
        && match (
            std::fs::read_to_string(base_export.0.join("Configuration.xml")),
            std::fs::read_to_string(input_dir.join("Configuration.xml")),
        ) {
            (Ok(active_root), Ok(tree_root)) => {
                version_edit = root_version_edit(&active_root, &tree_root);
                root_children_only_change(&active_root, &tree_root) || version_edit.is_some()
            }
            _ => false,
        };
    let refused = structural_refusals(&diff, root_edit_supported);
    if !refused.is_empty() {
        bail!(
            "these changes are not loaded (activation restructures nothing, so an edit of what a \
             table holds is the platform's own load): {}",
            list_paths(&refused)
        );
    }

    let base_rows = active
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.logical_key().as_str().to_owned(),
                entry.packed_payload().to_vec(),
            )
        })
        .collect::<HashMap<_, _>>();
    // The generic configuration patcher edits its header, not Version. Keep
    // the root out of that compile and replace only its evidenced scalar.
    let compile_diff = TreeDiff {
        changed: diff
            .changed
            .iter()
            .filter(|path| version_edit.is_none() || path.as_str() != "Configuration.xml")
            .cloned()
            .collect(),
        added: diff.added.clone(),
        removed: diff.removed.clone(),
    };
    let version_row = if let Some((old, new)) = &version_edit {
        let key = keys
            .get("Configuration.xml")
            .context("the active export does not name the root row")?;
        let packed = base_rows
            .get(key)
            .context("the active image has no root row")?;
        Some((key.clone(), patch_root_version(packed, key, old, new)?))
    } else {
        None
    };
    let mut edit = compile_edit(input_dir, base_rows, &compile_diff, &keys)
        .map_err(|error| {
            let text = error.to_string();
            if text.contains("has already run in this process") {
                anyhow!(
                    "the staging compiler reads its base rows from a process-wide table, so one \
                     process compiles one extension's tree; load each changed extension with its \
                     own --extension"
                )
            } else {
                anyhow!("{text}")
            }
        })
        .context("failed to compile the changed objects")?;
    if let Some((key, packed)) = version_row {
        edit.rows.insert(key, packed);
        edit.prefixes.insert("Configuration.xml".to_owned());
    }

    let image = proposed_image(active, &edit.rows, &edit.dropped)?;
    verify_export(
        &image,
        &Expected {
            tree: input_dir,
            active_export: &base_export.0,
            prefixes: &prefixes,
        },
        source_version,
        sql,
        database,
    )?;

    let rows = image
        .entries()
        .iter()
        .map(|entry| {
            ExtensionStageRow::new(
                entry.logical_key().as_str(),
                entry.packed_payload().to_vec(),
            )
        })
        .collect::<Vec<_>>();
    let mut changed_files = diff
        .changed
        .iter()
        .chain(&diff.added)
        .chain(&diff.removed)
        .cloned()
        .collect::<Vec<_>>();
    changed_files.sort();
    Ok(Some(TreeEdit {
        rows,
        changed_files,
        compiled_objects: edit.prefixes.into_iter().collect(),
        replaced_rows: edit.rows.len(),
    }))
}

/// A module's text as the platform stores it: no byte order mark of its own
/// (the export writes one) and CRLF line ends. An editor may save either
/// differently, and the load stores what the platform stores.
fn module_text_normalized(bytes: &[u8]) -> Vec<u8> {
    let body = bytes.strip_prefix(&[0xef, 0xbb, 0xbf][..]).unwrap_or(bytes);
    let mut out = Vec::with_capacity(body.len() + body.len() / 32);
    for (index, &byte) in body.iter().enumerate() {
        if byte == b'\n' && (index == 0 || body[index - 1] != b'\r') {
            out.push(b'\r');
        }
        out.push(byte);
    }
    out
}

/// Two files of a tree are the same file: byte for byte, and a module
/// (`.bsl`) also when only its byte order mark or line ends differ.
fn same_content(path: &str, left: &[u8], right: &[u8]) -> bool {
    left == right
        || (path.ends_with(".bsl") && module_text_normalized(left) == module_text_normalized(right))
}

/// `diff_trees` for a tree an editor may have saved: `ConfigDumpInfo.xml` is
/// derived and left out, and a module counts as changed only by its text.
fn diff_trees_normalized(base: &Path, edited: &Path) -> Result<TreeDiff> {
    let (a, b) = (relative_files(base)?, relative_files(edited)?);
    let mut diff = TreeDiff::default();
    for path in a.intersection(&b) {
        if path == CONFIG_DUMP_INFO {
            continue;
        }
        let (x, y) = (
            std::fs::read(base.join(path))?,
            std::fs::read(edited.join(path))?,
        );
        if !same_content(path, &x, &y) {
            diff.changed.push(path.clone());
        }
    }
    diff.added = b
        .difference(&a)
        .filter(|path| path.as_str() != CONFIG_DUMP_INFO)
        .cloned()
        .collect();
    diff.removed = a
        .difference(&b)
        .filter(|path| path.as_str() != CONFIG_DUMP_INFO)
        .cloned()
        .collect();
    Ok(diff)
}

/// The export report names the file an entry is written as, not the files it
/// is unpacked into: the pages of a help (`Ext/Help/ru.html`) and the picture
/// data (`Ext/Picture/Picture.png`) are written beside the `Ext/Help.xml` and
/// `Ext/Picture.xml` of the entry that holds them. A file `<dir>/Ext/<Name>/...`
/// with no entry of its own belongs to the entry of `<dir>/Ext/<Name>.xml`.
fn add_side_file_keys<'a>(
    keys: &mut BTreeMap<String, String>,
    paths: impl Iterator<Item = &'a String>,
) {
    for path in paths {
        if keys.contains_key(path) {
            continue;
        }
        let segments = path.split('/').collect::<Vec<_>>();
        // `<...>/Ext/<Name>/<file>`, at any depth of nested directories.
        let Some(ext_at) = segments.iter().rposition(|segment| *segment == "Ext") else {
            continue;
        };
        if segments.len() < ext_at + 3 {
            continue;
        }
        let owner = format!(
            "{}/{}.xml",
            segments[..=ext_at].join("/"),
            segments[ext_at + 1]
        );
        if let Some(key) = keys.get(&owner).cloned() {
            keys.insert(path.clone(), key);
        }
    }
}

/// The active entries in their order with `rows` put in (replaced where the
/// image has the key, appended where it has not) and the `dropped` keys and
/// their bodies left out.
fn proposed_image(
    active: &StorageImage,
    rows: &BTreeMap<String, Vec<u8>>,
    dropped: &BTreeSet<String>,
) -> Result<StorageImage> {
    let base = active.entries();
    let mut entries: Vec<StorageEntry> = Vec::with_capacity(base.len() + rows.len());
    let mut placed = BTreeSet::new();
    for entry in base {
        let key = entry.logical_key().as_str();
        if is_dropped(dropped, key) {
            continue;
        }
        match rows.get(key) {
            Some(packed) => {
                entries.push(with_payload(entry, packed)?);
                placed.insert(key.to_owned());
            }
            None => entries.push(entry.clone()),
        }
    }
    let template = base
        .first()
        .context("the active image has no entries to take an element header from")?;
    for (key, packed) in rows {
        if !placed.contains(key) && !is_dropped(dropped, key) {
            entries.push(new_entry(template, key, packed)?);
        }
    }
    let total = entries
        .iter()
        .map(|entry| entry.packed_payload().len() as u64)
        .sum::<u64>();
    StorageImage::with_retained_byte_limit(
        entries,
        ResourceLimits::for_input_bytes(total).max_retained_bytes_usize(),
    )
    .context("the proposed extension image is not valid")
}

/// What the proposed image must export to: the tree, and -- for a load
/// restricted to `prefixes` -- the active image's own export outside them.
struct Expected<'a> {
    tree: &'a Path,
    active_export: &'a Path,
    prefixes: &'a [String],
}

impl Expected<'_> {
    /// The directory holding the expected file of `path`.
    fn root_of(&self, path: &str) -> &Path {
        if self.prefixes.is_empty() || under_prefixes(path, self.prefixes) {
            self.tree
        } else {
            self.active_export
        }
    }
}

/// `path` is one of the `prefixes`, or a file under it (`Catalogs/К` takes
/// `Catalogs/К.xml` and `Catalogs/К/...`).
fn under_prefixes(path: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|prefix| {
        path == prefix
            || path == format!("{prefix}.xml")
            || path
                .strip_prefix(prefix.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

/// The proposed image exported back must be the tree, file for file.
fn verify_export(
    image: &StorageImage,
    expected: &Expected<'_>,
    source_version: InfobaseConfigSourceVersion,
    sql: &SqlExec,
    database: &str,
) -> Result<()> {
    let scratch = Scratch::new("proposed");
    crate::mssql_dump::extension::export_extension_image_to_source(
        image,
        &scratch.0,
        source_version,
        Some(crate::mssql_dump::extension::base_index_provider(
            sql,
            database,
            source_version,
        )),
    )
    .context("failed to export the proposed extension image")?;
    let back = relative_files(&scratch.0)?;
    let mut paths = back.clone();
    paths.extend(relative_files(expected.tree)?);
    if !expected.prefixes.is_empty() {
        paths.extend(relative_files(expected.active_export)?);
    }
    let mut differ = Vec::new();
    for path in &paths {
        if path == CONFIG_DUMP_INFO {
            continue;
        }
        let want = expected.root_of(path).join(path);
        let same = match (want.is_file(), back.contains(path)) {
            (true, true) => same_content(
                path,
                &std::fs::read(&want).context("expected file")?,
                &std::fs::read(scratch.0.join(path)).context("exported file")?,
            ),
            (false, false) => true,
            _ => false,
        };
        if !same {
            differ.push(path.clone());
        }
    }
    if differ.is_empty() {
        return Ok(());
    }
    let kept = if scratch.keep_if_asked() {
        format!(" (the export is kept in {})", scratch.0.display())
    } else {
        String::new()
    };
    bail!(
        "the proposed extension does not export back to the tree -- the compiler does not write \
         these faithfully yet, so nothing was staged: {}{kept}",
        list_paths(&differ)
    );
}

fn list_paths(paths: &[String]) -> String {
    const LISTED: usize = 20;
    let mut text = paths
        .iter()
        .take(LISTED)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if paths.len() > LISTED {
        text.push_str(&format!(" and {} more", paths.len() - LISTED));
    }
    text
}

/// The changed, added or removed files a stage cannot carry: activation
/// publishes rows and restructures nothing, so an edit that changes what an
/// object's table holds (a descriptor of a family that owns one, an object or
/// a body added or removed) belongs to the platform's own load. Each is named
/// with the reason. The root is accepted only after comparing it with the
/// active export: table-less child lists alone, or its Version text alone.
pub(crate) fn structural_refusals(diff: &TreeDiff, root_edit_supported: bool) -> Vec<String> {
    let mut refused = Vec::new();
    let mut check = |path: &str, moved: Option<&str>| {
        if let Some(reason) = refusal(path, moved, root_edit_supported) {
            refused.push(format!("{path} ({reason})"));
        }
    };
    for path in &diff.changed {
        check(path, None);
    }
    for path in &diff.added {
        check(path, Some("added"));
    }
    for path in &diff.removed {
        check(path, Some("removed"));
    }
    refused
}

fn is_tableless_family(family: &str) -> bool {
    TABLELESS_FAMILIES.iter().any(|(dir, _)| *dir == family)
}

fn refusal(path: &str, moved: Option<&str>, root_edit_supported: bool) -> Option<&'static str> {
    if path == CONFIG_DUMP_INFO {
        return None;
    }
    let segments = path.split('/').collect::<Vec<_>>();
    if let Some(moved) = moved {
        // An object of a table-less family comes or goes with its descriptor
        // and its bodies (the root's child list follows, see
        // `root_children_only_change`); the module of a form is a part of the form's
        // body row.
        match segments.as_slice() {
            [family, name] if is_tableless_family(family) && name.ends_with(".xml") => {
                return None;
            }
            [family, _, "Ext", ..] if is_tableless_family(family) => return None,
            [_, _, "Forms", _, "Ext", "Form", "Module.bsl"] => return None,
            _ => {}
        }
        return Some(if moved == "added" {
            "a file added: the object's structure changes"
        } else {
            "a file removed: the object's structure changes"
        });
    }
    if path == "Configuration.xml" {
        return if root_edit_supported {
            None
        } else {
            Some("the root descriptor")
        };
    }
    if path.starts_with("Ext/") {
        return if path.ends_with(".bsl") {
            None
        } else {
            Some("an asset of the root")
        };
    }
    if path.contains("/Ext/") {
        return None;
    }
    if !path.ends_with(".xml") {
        return Some("a file outside every Ext directory");
    }
    match segments.as_slice() {
        [family, _] if is_tableless_family(family) => None,
        [_, _] => Some("the descriptor of an object that may own a table"),
        [_, _, "Forms" | "Templates", _] => None,
        _ => Some("a descriptor outside the known layout"),
    }
}

/// The root descriptors differ in the `<ChildObjects>` lines of table-less
/// families and nowhere else.
pub(crate) fn root_children_only_change(active: &str, tree: &str) -> bool {
    fn without_tableless_children(text: &str) -> Option<String> {
        let open = "<ChildObjects>";
        let close = "</ChildObjects>";
        let start = text.find(open)? + open.len();
        let end = start + text[start..].find(close)?;
        let kept = text[start..end]
            .lines()
            .filter(|line| {
                let line = line.trim();
                !TABLELESS_FAMILIES.iter().any(|(_, tag)| {
                    line.strip_prefix(&format!("<{tag}>"))
                        .is_some_and(|rest| rest.ends_with(&format!("</{tag}>")))
                })
            })
            .collect::<Vec<_>>()
            .join("\n");
        Some(format!("{}{}{}", &text[..start], kept, &text[end..]))
    }
    let (active, tree) = (active.replace("\r\n", "\n"), tree.replace("\r\n", "\n"));
    match (
        without_tableless_children(&active),
        without_tableless_children(&tree),
    ) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// A canonical root descriptor changes only its dotted numeric Version,
/// preserving the active version's component count. Every other byte,
/// including adopted state, identity and child lists, must remain unchanged.
fn root_version_edit(active: &str, tree: &str) -> Option<(String, String)> {
    fn version_and_mask(text: &str) -> Option<(String, String)> {
        let xml = ibcmd_xml::XmlReader::from_slice(text.as_bytes()).ok()?;
        fn elements(node: &ibcmd_xml::XmlElement) -> Vec<&ibcmd_xml::XmlElement> {
            node.children()
                .iter()
                .filter_map(|child| match child {
                    ibcmd_xml::XmlNode::Element(element) => Some(element),
                    _ => None,
                })
                .collect::<Vec<_>>()
        }
        if xml.root().name().raw() != "MetaDataObject" {
            return None;
        }
        let configurations = elements(xml.root())
            .into_iter()
            .filter(|node| node.name().raw() == "Configuration")
            .collect::<Vec<_>>();
        let [configuration] = configurations.as_slice() else {
            return None;
        };
        let properties = elements(configuration)
            .into_iter()
            .filter(|node| node.name().raw() == "Properties")
            .collect::<Vec<_>>();
        let [properties] = properties.as_slice() else {
            return None;
        };
        let versions = elements(properties)
            .into_iter()
            .filter(|node| node.name().raw() == "Version")
            .collect::<Vec<_>>();
        let [version] = versions.as_slice() else {
            return None;
        };
        if !version.attributes().is_empty() || !elements(version).is_empty() {
            return None;
        }
        // Noncanonical tags or a second Version elsewhere cannot provide a
        // different masking range from the semantic property above.
        let open = "<Version>";
        let close = "</Version>";
        if text.matches(open).count() != 1 || text.matches(close).count() != 1 {
            return None;
        }
        let start = text.find(open)? + open.len();
        let end = start + text[start..].find(close)?;
        let value = &text[start..end];
        let semantic = version
            .children()
            .iter()
            .filter_map(|child| match child {
                ibcmd_xml::XmlNode::Text(text) => Some(text.value()),
                _ => None,
            })
            .collect::<String>();
        if semantic != value {
            return None;
        }
        let components = value.split('.').collect::<Vec<_>>();
        if components.len() < 2
            || components
                .iter()
                .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
        {
            return None;
        }
        Some((
            value.to_owned(),
            format!("{}{}", &text[..start], &text[end..]),
        ))
    }
    match (version_and_mask(active), version_and_mask(tree)) {
        (Some((old, a)), Some((new, b))) => {
            (old != new && old.split('.').count() == new.split('.').count() && a == b)
                .then_some((old, new))
        }
        _ => None,
    }
}

/// The native 8.3.27 extension properties tuple stores Version in member15.
/// Preserve the complete root envelope and adopted header verbatim.
/// Called only after a complete active-image export validates its class
/// sequence and adopted layout; the proposed image is exported again too.
fn patch_root_version(packed: &[u8], key: &str, old: &str, new: &str) -> Result<Vec<u8>> {
    use crate::module_blob::{inflate_raw, scan_braced_fields};
    if old == new
        || old.split('.').count() != new.split('.').count()
        || [old, new].iter().any(|value| {
            value.split('.').count() < 2
                || value
                    .split('.')
                    .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
        })
    {
        bail!("unsupported extension Version format change");
    }
    let mut text =
        String::from_utf8(inflate_raw(packed)?).context("the extension root is not UTF-8")?;
    let start = text
        .find('{')
        .context("the extension root has no envelope")?;
    if !text[..start].trim_matches('\u{feff}').trim().is_empty() {
        bail!("unsupported extension root prefix");
    }
    let root = scan_braced_fields(&text, start)?;
    if root.len() != 11 || &text[root[0].clone()] != "2" || &text[root[2].clone()] != "7" {
        bail!("unsupported extension root envelope");
    }
    let identity = scan_braced_fields(&text, root[1].start)?;
    if identity.len() != 1 || text[identity[0].clone()] != *key {
        bail!("extension root identity differs from its storage key");
    }
    let section = scan_braced_fields(&text, root[3].start)?;
    if section.len() != 2 || uuid::Uuid::parse_str(&text[section[0].clone()]).is_err() {
        bail!("unsupported extension root properties section");
    }
    let payload = scan_braced_fields(&text, section[1].start)?;
    if payload.len() != 28 || &text[payload[0].clone()] != "1" || &text[payload[2].clone()] != "25"
    {
        bail!("unsupported extension root properties payload");
    }
    let properties = scan_braced_fields(&text, payload[1].start)?;
    // 68 is the layout discriminator, not its field count. This native
    // sample contains 60 members after that discriminator.
    if properties.len() != 61 || &text[properties[0].clone()] != "68" {
        bail!("unsupported extension root properties layout (expected layout 68, 60 members)");
    }
    let version = properties[15].clone();
    if text[version.clone()] != format!("\"{old}\"") {
        bail!("extension root Version differs from the active source");
    }
    text.replace_range(version, &format!("\"{new}\""));
    crate::module_blob::deflate_raw(text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_root_version_patch_preserves_adoption_and_all_other_bytes() {
        use crate::module_blob::{deflate_raw, inflate_raw};
        const BASE: &str =
            include_str!("../tests/fixtures/native-evidence/extension-root-version/base.txt");
        const NATIVE: &str = include_str!(
            "../tests/fixtures/native-evidence/extension-root-version/native-version1.txt"
        );
        let key = "771688e2-4830-4129-94b9-8b18fb96ae8f";
        let packed = deflate_raw(BASE.as_bytes()).unwrap();
        let result = patch_root_version(&packed, key, "1.7.0.0", "1.7.0.1").unwrap();
        assert_eq!(
            inflate_raw(&result).unwrap(),
            BASE.replace("\"1.7.0.0\"", "\"1.7.0.1\"").as_bytes()
        );
        // Native's own import additionally reorders adoption pairs and
        // refreshes its footer; its Version uses this same member.
        let native = deflate_raw(NATIVE.as_bytes()).unwrap();
        assert!(patch_root_version(&native, key, "1.7.0.1", "1.7.0.2").is_ok());
        assert!(patch_root_version(&packed, key, "0.0.0.0", "1.7.0.1").is_err());
        assert!(patch_root_version(&packed, "wrong-key", "1.7.0.0", "1.7.0.1").is_err());
        assert!(patch_root_version(&packed, key, "1.7.0.0", "1.7.1").is_err());
        assert!(patch_root_version(&packed, key, "1.7.0.0", "1.7.0.\"1").is_err());
        for malformed in [
            BASE.replace("{68,", "{67,"),
            BASE[..BASE.len() / 2].to_owned(),
            format!("unknown{BASE}"),
        ] {
            assert!(
                patch_root_version(
                    &deflate_raw(malformed.as_bytes()).unwrap(),
                    key,
                    "1.7.0.0",
                    "1.7.0.1"
                )
                .is_err()
            );
        }
    }

    fn diff(changed: &[&str], added: &[&str], removed: &[&str]) -> TreeDiff {
        let own = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect();
        TreeDiff {
            changed: own(changed),
            added: own(added),
            removed: own(removed),
        }
    }

    #[test]
    fn bodies_and_tableless_descriptors_change_in_place() {
        let refused = structural_refusals(
            &diff(
                &[
                    "Catalogs/К/Ext/ObjectModule.bsl",
                    "Catalogs/К/Forms/Ф/Ext/Form.xml",
                    "Catalogs/К/Forms/Ф.xml",
                    "Reports/О/Templates/М/Ext/Template.xml",
                    "CommonPictures/К.xml",
                    "CommonPictures/К/Ext/Picture/Picture.png",
                    "Roles/Р/Ext/Rights.xml",
                    "Ext/ManagedApplicationModule.bsl",
                    "ConfigDumpInfo.xml",
                ],
                &[],
                &[],
            ),
            false,
        );
        assert!(refused.is_empty(), "{refused:?}");
    }

    #[test]
    fn a_side_file_belongs_to_the_entry_of_its_manifest() {
        let mut keys = BTreeMap::new();
        keys.insert("К/Ext/Help.xml".to_owned(), "u.1".to_owned());
        keys.insert("К/Ext/Picture.xml".to_owned(), "p.0".to_owned());
        keys.insert("К/Ext/Form.xml".to_owned(), "f.0".to_owned());
        let paths = [
            "К/Ext/Help/ru.html".to_owned(),
            "К/Ext/Picture/Picture.png".to_owned(),
            "К/Ext/Other/x.bin".to_owned(),
            "К/Ext/Form.xml".to_owned(),
            "К/Ext/Help.xml".to_owned(),
        ];
        add_side_file_keys(&mut keys, paths.iter());
        assert_eq!(keys["К/Ext/Help/ru.html"], "u.1");
        assert_eq!(keys["К/Ext/Picture/Picture.png"], "p.0");
        assert!(!keys.contains_key("К/Ext/Other/x.bin"));
        assert_eq!(keys["К/Ext/Form.xml"], "f.0");
    }

    #[test]
    fn a_module_saved_without_a_bom_or_with_lf_is_the_same_module() {
        let stored = "\u{feff}А = 1;\r\nБ = 2;\r\n".as_bytes();
        let lf = "А = 1;\nБ = 2;\n".as_bytes();
        assert!(same_content("М/Ext/Module.bsl", stored, lf));
        assert!(same_content("М/Ext/Module.bsl", stored, stored));
        let other = "А = 1;\r\nБ = 3;\r\n".as_bytes();
        assert!(!same_content("М/Ext/Module.bsl", stored, other));
        // Only a module is compared by its text.
        assert!(!same_content("М/Ext/Form.xml", stored, lf));
    }

    #[test]
    fn descriptors_that_may_own_a_table_are_refused() {
        let refused = structural_refusals(
            &diff(
                &["Catalogs/К.xml", "Configuration.xml", "Ext/Logo.png"],
                &[],
                &[],
            ),
            false,
        );
        assert_eq!(refused.len(), 3, "{refused:?}");
        assert!(refused[0].starts_with("Catalogs/К.xml"), "{refused:?}");
    }

    #[test]
    fn a_body_added_or_removed_changes_the_structure_of_its_object() {
        let refused = structural_refusals(
            &diff(
                &[],
                &["Catalogs/К/Ext/Help.xml", "Catalogs/К/Ext/ObjectModule.bsl"],
                &[
                    "Catalogs/К/Ext/ObjectModule.bin",
                    "Catalogs/К/Ext/Help/ru.html",
                ],
            ),
            false,
        );
        assert_eq!(refused.len(), 4, "{refused:?}");
    }

    #[test]
    fn the_module_of_a_form_and_an_object_of_a_tableless_family_come_and_go() {
        let refused = structural_refusals(
            &diff(
                &["Configuration.xml"],
                &[
                    "Catalogs/К/Forms/Ф/Ext/Form/Module.bsl",
                    "CommonPictures/Н.xml",
                    "CommonPictures/Н/Ext/Picture.xml",
                    "CommonPictures/Н/Ext/Picture/Picture.png",
                    "CommonModules/М.xml",
                    "CommonModules/М/Ext/Module.bsl",
                ],
                &["Roles/Р.xml", "Roles/Р/Ext/Rights.xml"],
            ),
            true,
        );
        assert!(refused.is_empty(), "{refused:?}");
        // The same without the root's child list explained: only it is refused.
        let refused = structural_refusals(
            &diff(&["Configuration.xml"], &["CommonPictures/Н.xml"], &[]),
            false,
        );
        assert_eq!(refused.len(), 1, "{refused:?}");
        // A new object of a family that may own a table is refused, and so is
        // a new form of an object (its owner's descriptor changes too).
        let refused = structural_refusals(
            &diff(&[], &["Catalogs/К.xml", "Catalogs/К/Forms/Ф.xml"], &[]),
            true,
        );
        assert_eq!(refused.len(), 2, "{refused:?}");
    }

    #[test]
    fn a_root_differing_in_tableless_children_only_is_recognised() {
        let root = |children: &[&str], comment: &str| {
            format!(
                "<Configuration>\r\n\t<Properties><Comment>{comment}</Comment></Properties>\r\n\t<ChildObjects>\r\n{}\t</ChildObjects>\r\n</Configuration>",
                children
                    .iter()
                    .map(|line| format!("\t\t{line}\r\n"))
                    .collect::<String>()
            )
        };
        let base = root(
            &[
                "<Language>Русский</Language>",
                "<CommonPicture>А</CommonPicture>",
            ],
            "c",
        );
        let with_picture = root(
            &[
                "<Language>Русский</Language>",
                "<CommonPicture>А</CommonPicture>",
                "<CommonPicture>Б</CommonPicture>",
            ],
            "c",
        );
        assert!(root_children_only_change(&base, &with_picture));
        // A catalog in the list, or another property, is more than that.
        let with_catalog = root(
            &[
                "<Language>Русский</Language>",
                "<CommonPicture>А</CommonPicture>",
                "<Catalog>К</Catalog>",
            ],
            "c",
        );
        assert!(!root_children_only_change(&base, &with_catalog));
        let with_comment = root(
            &[
                "<Language>Русский</Language>",
                "<CommonPicture>А</CommonPicture>",
            ],
            "другой",
        );
        assert!(!root_children_only_change(&base, &with_comment));
    }
    #[test]
    fn version_edits_preserve_every_other_root_property_and_the_existing_format() {
        let root = "<MetaDataObject><Configuration uuid=\"root\"><Properties><ObjectBelonging>Adopted</ObjectBelonging><Name>ServiceDesk</Name><Version>1.7.0.0</Version><Unknown>keep</Unknown></Properties><ChildObjects><Catalog>C</Catalog></ChildObjects></Configuration></MetaDataObject>";
        let changed = root.replace("1.7.0.0", "1.7.0.1");
        assert!(root_version_edit(root, &changed).is_some());
        assert_eq!(
            structural_refusals(
                &diff(&["Configuration.xml", "Catalogs/C.xml"], &[], &[]),
                true
            )
            .len(),
            1
        );
        assert!(structural_refusals(&diff(&["Configuration.xml"], &[], &[]), true).is_empty());
        for edit in [
            changed.replace("<Name>ServiceDesk</Name>", "<Name>Other</Name>"),
            changed.replace("<Unknown>keep</Unknown>", "<Unknown>change</Unknown>"),
            changed.replace("<Catalog>C</Catalog>", "<Catalog>D</Catalog>"),
            changed.replace("Adopted", "Own"),
            changed.replace("uuid=\"root\"", "uuid=\"other\""),
            changed.replace(
                "<Version>1.7.0.1</Version>",
                "<Version>1.7.0.1</Version><Version>2.0.0.0</Version>",
            ),
            changed.replace("<Version>1.7.0.1</Version>", "<Version/>"),
            changed.replace("1.7.0.1", "1.7.1"),
            changed.replace("1.7.0.1", "1.7.0.x"),
            changed.replace("1.7.0.1", "1.7..1"),
            changed.replace("1.7.0.1", " 1.7.0.1"),
            changed.replace("<Version>", "<Version flag=\"x\">"),
            changed.replace("</Configuration>", ""),
        ] {
            assert!(root_version_edit(root, &edit).is_none(), "{edit}");
            assert_eq!(
                structural_refusals(
                    &diff(&["Configuration.xml"], &[], &[]),
                    root_version_edit(root, &edit).is_some()
                )
                .len(),
                1
            );
        }
        assert!(root_version_edit(root, root).is_none());
    }
}
