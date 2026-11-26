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
//! The ordinary route admits nonstructural changes. The schema-planned route
//! can compile structural changes as a proposal, but its caller must prove the
//! physical transition before staging or publishing any row.

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
    ("DataProcessors", "DataProcessor"),
    ("HTTPServices", "HTTPService"),
    ("Subsystems", "Subsystem"),
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
///
/// Compiles a proposed schema update without authorising publication. The
/// caller must prove it nonstructural or validate its physical schema plan
/// before staging or activating any row.
pub(crate) fn compile_tree_edit_for_restructure(
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
    compile_tree_edit_from_export(
        input_dir,
        active,
        prefixes,
        &base_export.0,
        keys_by_output(&report),
        |image, expected| {
            verify_export(
                image,
                expected,
                source_version,
                Some(crate::mssql_dump::extension::base_index_provider(
                    sql,
                    database,
                    source_version,
                )),
            )
        },
    )
}

fn compile_tree_edit_from_export(
    input_dir: &Path,
    active: &StorageImage,
    prefixes: &[String],
    base_export: &Path,
    mut keys: BTreeMap<String, String>,
    verify: impl FnOnce(&StorageImage, &Expected<'_>) -> Result<()>,
) -> Result<Option<TreeEdit>> {
    let mut diff = diff_trees_normalized(base_export, input_dir)?;
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
    let root_edit = compilation_root_edit(base_export, input_dir, &diff, &prefixes)?;
    let version_edit = root_edit.version;

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
        // Keep the base only as a fallback. Added child descriptors cause
        // compile_edit to rewrite the root's collections; Version must be
        // applied to that result rather than replacing it with the old root.
        Some((key.clone(), packed.clone(), old.clone(), new.clone()))
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
    if let Some((key, base, old, new)) = version_row {
        let current = edit.rows.get(&key).unwrap_or(&base);
        let packed = patch_root_version(current, &key, &old, &new)?;
        edit.rows.insert(key, packed);
        edit.prefixes.insert("Configuration.xml".to_owned());
    }

    let image = proposed_image(active, &edit.rows, &edit.dropped)?;
    verify(
        &image,
        &Expected {
            tree: input_dir,
            active_export: base_export,
            prefixes: &prefixes,
        },
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

/// Judge a complete externally staged image as well: an activation must not
/// publish a native schema edit merely because its CAS graph hashes correctly.
pub(crate) fn require_nonstructural_stage(
    sql: &SqlExec,
    database: &str,
    version: InfobaseConfigSourceVersion,
    active: &StorageImage,
    stage: &crate::mssql_extension_stage::ExtensionStagePlan,
) -> Result<()> {
    let proposed = stage_image(active, stage)?;
    let base = Scratch::new("activation-base");
    let next = Scratch::new("activation-next");
    for (image, dir) in [(active, &base.0), (&proposed, &next.0)] {
        let report = crate::mssql_dump::extension::export_extension_image_to_source(
            image,
            dir,
            version,
            Some(crate::mssql_dump::extension::base_index_provider(
                sql, database, version,
            )),
        )?;
        if report.storage.failed > 0 || report.storage.opaque > 0 {
            bail!(
                "cannot prove extension stage schema safety: incomplete source export; nothing was published"
            );
        }
    }
    require_nonstructural_source_transition(&base.0, &next.0)
}

/// The same pure source proof used by activation after its complete exports.
pub(crate) fn require_nonstructural_source_transition(base: &Path, next: &Path) -> Result<()> {
    let diff = diff_trees_normalized(base, next)?;
    let old = std::fs::read_to_string(base.join("Configuration.xml"))?;
    let new = std::fs::read_to_string(next.join("Configuration.xml"))?;
    let root_edit = root_policy(&old, &new, false).map_err(|paths| anyhow!(
        "extension stage requires an unmodeled schema transition, nothing was published: Configuration.xml ({})", paths.join(", ")
    ))?;
    validate_root_membership(base, next, &diff, &root_edit)?;
    let refused = structural_refusals(&diff, true);
    if !refused.is_empty() {
        bail!(
            "extension stage requires an unmodeled schema transition, nothing was published: {}",
            list_paths(&refused)
        );
    }
    Ok(())
}

fn reference_path(reference: &ibcmd_xml::extension_root::RootChildReference) -> Option<String> {
    crate::mssql_dump::root_family_folder(&reference.kind)
        .map(|folder| format!("{folder}/{}.xml", reference.name))
}
fn descriptor_identity(
    root: &Path,
    path: &str,
    reference: &ibcmd_xml::extension_root::RootChildReference,
) -> Result<uuid::Uuid> {
    use ibcmd_xml::{AttributeKind, XmlNode, XmlReader};
    let xml = XmlReader::from_slice(&std::fs::read(root.join(path)).with_context(|| {
        format!(
            "root reference {}.{} lacks descriptor {path}",
            reference.kind, reference.name
        )
    })?)?;
    fn children(element: &ibcmd_xml::XmlElement) -> Vec<&ibcmd_xml::XmlElement> {
        element
            .children()
            .iter()
            .filter_map(|node| {
                if let XmlNode::Element(element) = node {
                    Some(element)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    }
    let objects = children(xml.root());
    let [object] = objects.as_slice() else {
        bail!("descriptor {path} has no unique metadata object");
    };
    if xml.root().name().raw() != "MetaDataObject" || object.name().raw() != reference.kind {
        bail!(
            "descriptor {path} does not have root reference kind {}",
            reference.kind
        );
    }
    let properties = children(object)
        .into_iter()
        .filter(|e| e.name().raw() == "Properties")
        .collect::<Vec<_>>();
    let [properties] = properties.as_slice() else {
        bail!("descriptor {path} lacks unique Properties");
    };
    let names = children(properties)
        .into_iter()
        .filter(|e| e.name().raw() == "Name")
        .collect::<Vec<_>>();
    let [name] = names.as_slice() else {
        bail!("descriptor {path} lacks unique Name");
    };
    let value = name
        .children()
        .iter()
        .filter_map(|node| {
            if let XmlNode::Text(text) = node {
                Some(text.value())
            } else {
                None
            }
        })
        .collect::<String>();
    if value != reference.name
        || !name.attributes().is_empty()
        || name
            .children()
            .iter()
            .any(|n| !matches!(n, XmlNode::Text(_)))
    {
        bail!(
            "descriptor {path} does not have root reference name {}",
            reference.name
        );
    }
    let id = object
        .attributes()
        .iter()
        .find_map(|attribute| match attribute.kind() {
            AttributeKind::Ordinary(name) if name.raw() == "uuid" => Some(attribute.value()),
            _ => None,
        })
        .with_context(|| format!("descriptor {path} lacks uuid"))?;
    uuid::Uuid::parse_str(id).with_context(|| format!("descriptor {path} has invalid uuid"))
}
fn validate_root_membership(
    base: &Path,
    next: &Path,
    diff: &TreeDiff,
    edit: &ibcmd_xml::extension_root::ExtensionRootEdit,
) -> Result<()> {
    let selected = |path: &String| {
        diff.changed.contains(path) || diff.added.contains(path) || diff.removed.contains(path)
    };
    let mut retained = BTreeMap::new();
    let mut removed = BTreeMap::new();
    for child in &edit.removed_children {
        let path = reference_path(child).context("removed root reference has unknown family")?;
        if !diff.removed.contains(&path) || next.join(&path).exists() {
            bail!(
                "root removal {}.{} requires the selected removal of descriptor {path}",
                child.kind,
                child.name
            );
        }
        removed.insert(descriptor_identity(base, &path, child)?, child);
    }
    for child in &edit.retained_children {
        let Some(path) = reference_path(child) else {
            continue;
        };
        if !edit.added_children.is_empty() || selected(&path) {
            let before = descriptor_identity(base, &path, child)?;
            if selected(&path) && descriptor_identity(next, &path, child)? != before {
                bail!(
                    "retained root reference {}.{} changes descriptor uuid",
                    child.kind,
                    child.name
                );
            }
            if retained.insert(before, child).is_some() {
                bail!("retained root references share a descriptor uuid");
            }
        }
    }
    let mut added = BTreeSet::new();
    for child in &edit.added_children {
        let path = reference_path(child).context("added root reference has unknown family")?;
        if !(diff.added.contains(&path) || diff.changed.contains(&path)) {
            bail!(
                "root addition {}.{} requires descriptor {path} selected for compilation",
                child.kind,
                child.name
            );
        }
        let id = descriptor_identity(next, &path, child)?;
        if retained.contains_key(&id) || !added.insert(id) {
            bail!(
                "root addition {}.{} aliases another descriptor uuid",
                child.kind,
                child.name
            );
        }
        if removed
            .get(&id)
            .is_some_and(|before| before.kind != child.kind)
        {
            bail!(
                "root addition {}.{} reuses the identity of a different metadata kind",
                child.kind,
                child.name
            );
        }
    }
    Ok(())
}
/// A structural schema plan replaces a source refusal only after proving the
/// same root/descriptor identities as the whole-tree compiler. DATA additions
/// are admitted here; their physical effects are classified by the planner.
pub(crate) fn require_structural_root_correspondence(base: &Path, next: &Path) -> Result<()> {
    let diff = diff_trees_normalized(base, next)?;
    compilation_root_edit(base, next, &diff, &[]).map(|_| ())
}
fn compilation_root_edit(
    base: &Path,
    input: &Path,
    diff: &TreeDiff,
    prefixes: &[String],
) -> Result<ibcmd_xml::extension_root::ExtensionRootEdit> {
    let root_selected = prefixes.is_empty() || under_prefixes("Configuration.xml", prefixes);
    if !root_selected && diff.added.iter().chain(&diff.removed).any(|path| {
        let parts = path.split('/').collect::<Vec<_>>();
        matches!(parts.as_slice(), [family,file] if file.ends_with(".xml") && crate::metadata_model::index::kind_of_collection(family).is_some())
    }) {
        bail!("top-level descriptor additions/removals require selecting Configuration.xml with the descriptor");
    }
    let old = std::fs::read_to_string(base.join("Configuration.xml"))?;
    let new = std::fs::read_to_string(
        if root_selected { input } else { base }.join("Configuration.xml"),
    )?;
    let edit = root_policy(&old, &new, true).map_err(|changed| {
        anyhow!(
            "unsupported extension root transition, nothing was staged: Configuration.xml ({})",
            changed.join(", ")
        )
    })?;
    validate_root_membership(base, input, diff, &edit)?;
    Ok(edit)
}

/// Materialises the stage's content rows as an extension image. The caller
/// must first verify the stage's manifest/CAS graph; this helper intentionally
/// provides no schema-safety or publication authorisation.
pub(crate) fn stage_image(
    active: &StorageImage,
    stage: &crate::mssql_extension_stage::ExtensionStagePlan,
) -> Result<StorageImage> {
    let rows: BTreeMap<_, _> = stage
        .rows()
        .iter()
        .filter(|r| r.logical_name != "configinfo")
        .map(|r| (r.logical_name.clone(), r.binary_data.clone()))
        .collect();
    let dropped = active
        .entries()
        .iter()
        .map(|e| e.logical_key().as_str().to_owned())
        .filter(|key| !rows.contains_key(key))
        .collect();
    proposed_image(active, &rows, &dropped)
}

/// Two files of a tree are the same file: byte for byte, and a module
/// (`.bsl`) also when only its byte order mark or line ends differ.
fn same_content(path: &str, left: &[u8], right: &[u8]) -> bool {
    left == right
        || (path.ends_with(".bsl") && module_text_normalized(left) == module_text_normalized(right))
        || (path == "Configuration.xml" && ibcmd_xml::equal_ignoring_source_formatting(left, right))
        || (path.ends_with("/Ext/Form.xml")
            && ibcmd_xml::source_defaults::form_source_input_equal(left, right))
        || (metadata_descriptor(path, left)
            && ibcmd_xml::source_defaults::metadata_source_input_equal(left, right))
}
fn metadata_descriptor(path: &str, bytes: &[u8]) -> bool {
    if !path.ends_with(".xml") || path.contains("/Ext/") || path.starts_with("Ext/") {
        return false;
    }
    let Ok(xml) = ibcmd_xml::XmlReader::from_slice(bytes) else {
        return false;
    };
    if xml.root().name().raw() != "MetaDataObject" {
        return false;
    }
    let objects = xml
        .root()
        .children()
        .iter()
        .filter_map(|n| {
            if let ibcmd_xml::XmlNode::Element(e) = n {
                Some(e)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let [object] = objects.as_slice() else {
        return false;
    };
    let parts = path.split('/').collect::<Vec<_>>();
    match parts.as_slice() {
        [family, _] => crate::mssql_dump::root_family_folder(object.name().raw()) == Some(*family),
        [family, _, "Forms", _] => {
            crate::metadata_model::index::kind_of_collection(family).is_some()
                && object.name().raw() == "Form"
        }
        [family, _, "Templates", _] => {
            crate::metadata_model::index::kind_of_collection(family).is_some()
                && object.name().raw() == "Template"
        }
        _ => false,
    }
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
    base_provider: Option<crate::mssql_dump::extension::BaseIndexProvider>,
) -> Result<()> {
    let scratch = Scratch::new("proposed");
    crate::mssql_dump::extension::export_extension_image_to_source(
        image,
        &scratch.0,
        source_version,
        base_provider,
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
/// active export: table-less child lists, its Version, or both together.
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

fn subsystem_metadata_path(mut parts: &[&str]) -> bool {
    loop {
        match parts {
            ["Subsystems", file] => return file.ends_with(".xml"),
            ["Subsystems", _, "Ext", ..] => return true,
            ["Subsystems", _, rest @ ..] if rest.first() == Some(&"Subsystems") => parts = rest,
            _ => return false,
        }
    }
}

fn refusal(path: &str, moved: Option<&str>, root_edit_supported: bool) -> Option<&'static str> {
    if path == CONFIG_DUMP_INFO {
        return None;
    }
    let segments = path.split('/').collect::<Vec<_>>();
    if subsystem_metadata_path(&segments) {
        return None;
    }
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
            [family, _, "Forms" | "Templates", file]
                if is_tableless_family(family) && file.ends_with(".xml") =>
            {
                return None;
            }
            [family, _, "Forms" | "Templates", _, "Ext", ..] if is_tableless_family(family) => {
                return None;
            }
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
#[cfg(test)]
pub(crate) fn root_children_only_change(active: &str, tree: &str) -> bool {
    root_children_change(active, tree, false)
}

fn root_policy(
    active: &str,
    tree: &str,
    all_families: bool,
) -> std::result::Result<ibcmd_xml::extension_root::ExtensionRootEdit, Vec<String>> {
    ibcmd_xml::extension_root::extension_root_edit(active.as_bytes(), tree.as_bytes(), |tag| {
        if all_families {
            crate::mssql_dump::root_family_folder(tag).is_some()
        } else {
            TABLELESS_FAMILIES.iter().any(|(_, child)| *child == tag)
        }
    })
}
#[cfg(test)]
fn root_children_change(active: &str, tree: &str, all_families: bool) -> bool {
    root_policy(active, tree, all_families).is_ok_and(|edit| edit.version.is_none())
}
#[cfg(test)]
fn root_version_edit(active: &str, tree: &str) -> Option<(String, String)> {
    root_version_edit_for_policy(active, tree, false)
}
#[cfg(test)]
fn root_version_edit_for_policy(
    active: &str,
    tree: &str,
    all_families: bool,
) -> Option<(String, String)> {
    root_policy(active, tree, all_families).ok()?.version
}

/// The native 8.3.27 extension properties tuple stores Version in member15.
/// Preserve the complete root envelope and adopted header verbatim.
/// Called only after a complete active-image export validates its class
/// sequence and adopted layout; the proposed image is exported again too.
fn patch_root_version(packed: &[u8], key: &str, old: &str, new: &str) -> Result<Vec<u8>> {
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
    let (mut text, version) = root_version_range(packed, key)?;
    if text[version.clone()] != format!("\"{old}\"") {
        bail!("extension root Version differs from the active source");
    }
    text.replace_range(version, &format!("\"{new}\""));
    crate::module_blob::deflate_raw(text.as_bytes())
}

fn root_version_range(packed: &[u8], key: &str) -> Result<(String, std::ops::Range<usize>)> {
    use crate::module_blob::{inflate_raw, scan_braced_fields};
    let text =
        String::from_utf8(inflate_raw(packed)?).context("the extension root is not UTF-8")?;
    let start = text
        .find('{')
        .context("the extension root has no envelope")?;
    if !text[..start].trim_matches('\u{feff}').trim().is_empty() {
        bail!("unsupported extension root prefix");
    }
    use ibcmd_schema::configuration_root::ExtensionRootVersionLayout as L;
    let root = scan_braced_fields(&text, start)?;
    if root.len() != L::ROOT_FIELDS
        || &text[root[L::ROOT_TAG].clone()] != "2"
        || &text[root[L::ROOT_SECTIONS].clone()] != "7"
    {
        bail!("unsupported extension root envelope");
    }
    let identity = scan_braced_fields(&text, root[L::ROOT_IDENTITY].start)?;
    if identity.len() != 1 || text[identity[0].clone()] != *key {
        bail!("extension root identity differs from its storage key");
    }
    let section = scan_braced_fields(&text, root[L::ROOT_PROPERTIES_SECTION].start)?;
    if section.len() != L::SECTION_FIELDS
        || uuid::Uuid::parse_str(&text[section[L::SECTION_CLASS].clone()]).is_err()
    {
        bail!("unsupported extension root properties section");
    }
    let payload = scan_braced_fields(&text, section[L::SECTION_PAYLOAD].start)?;
    if payload.len() <= L::PAYLOAD_MEMBERS || &text[payload[L::PAYLOAD_TAG].clone()] != "1" {
        bail!("unsupported extension root properties payload");
    }
    let properties = scan_braced_fields(&text, payload[L::PAYLOAD_PROPERTIES].start)?;
    if properties.is_empty()
        || !L::accepts(
            payload.len(),
            &text[payload[L::PAYLOAD_MEMBERS].clone()],
            properties.len(),
            &text[properties[L::PROPERTIES_TAG].clone()],
        )
    {
        bail!("unsupported extension root properties layout");
    }
    let version = properties[L::VERSION].clone();
    Ok((text, version))
}

pub(crate) fn stored_root_version(packed: &[u8], key: &str) -> Result<String> {
    let (text, span) = root_version_range(packed, key)?;
    crate::mssql_dump::parse_1c_quoted_string(&text[span])
        .context("extension Version is not a quoted string")
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
    #[test]
    fn version_and_tableless_child_edits_compose() {
        let before = "<MetaDataObject><Configuration uuid=\"root\"><Properties><ObjectBelonging>Adopted</ObjectBelonging><Name>Coverage</Name><Version>1.1.0.0</Version><Unknown>keep</Unknown></Properties><ChildObjects>\n<CommonModule>Existing</CommonModule>\n<Catalog>ExistingData</Catalog>\n</ChildObjects></Configuration></MetaDataObject>";
        let after = before.replace("1.1.0.0", "1.1.0.1").replace(
            "<CommonModule>Existing</CommonModule>",
            "<CommonModule>Existing</CommonModule>\n<CommonModule>Added</CommonModule>",
        );
        assert_eq!(
            root_version_edit(before, &after),
            Some(("1.1.0.0".to_owned(), "1.1.0.1".to_owned()))
        );
        assert!(
            root_version_edit(
                before,
                &after.replace("<Unknown>keep</Unknown>", "<Unknown>changed</Unknown>")
            )
            .is_none()
        );
        assert!(
            root_version_edit(
                before,
                &after.replace(
                    "<Catalog>ExistingData</Catalog>",
                    "<Catalog>OtherData</Catalog>"
                )
            )
            .is_none()
        );
    }
    #[test]
    fn restructure_root_policy_admits_collections_without_other_properties() {
        let before = "<MetaDataObject><Configuration uuid=\"root\"><Properties><Name>Coverage</Name><Version>1.1.0.1</Version><Unknown>keep</Unknown></Properties><ChildObjects>\n<Catalog>Existing</Catalog>\n</ChildObjects></Configuration></MetaDataObject>";
        let after = before.replace("1.1.0.1", "1.1.0.2").replace("<Catalog>Existing</Catalog>", "<Catalog>Existing</Catalog>\n<Catalog>Added</Catalog>\n<InformationRegister>AddedRegister</InformationRegister>\n<HTTPService>AddedService</HTTPService>");
        assert!(root_version_edit(before, &after).is_none());
        assert_eq!(
            root_version_edit_for_policy(before, &after, true),
            Some(("1.1.0.1".to_owned(), "1.1.0.2".to_owned()))
        );
        assert!(
            root_version_edit_for_policy(
                before,
                &after.replace("<Unknown>keep</Unknown>", "<Unknown>changed</Unknown>"),
                true
            )
            .is_none()
        );
        assert!(
            root_version_edit_for_policy(
                before,
                &after.replace("uuid=\"root\"", "uuid=\"other\""),
                true
            )
            .is_none()
        );
        assert!(
            root_version_edit_for_policy(
                before,
                &after.replace(
                    "<HTTPService>AddedService</HTTPService>",
                    "<Unmodeled>New</Unmodeled>"
                ),
                true
            )
            .is_none()
        );
        let children_only = after.replace("1.1.0.2", "1.1.0.1");
        assert!(!root_children_only_change(before, &children_only));
        assert!(root_children_change(before, &children_only, true));
    }
    #[test]
    fn formatted_root_version_and_new_common_form_are_semantic_edits() {
        let before = r#"<MetaDataObject xmlns="urn:metadata" version="2.21"><Configuration uuid="root"><Properties><Name>Coverage</Name><Version>1.2.0.0</Version><UsePurposes/></Properties><ChildObjects><Catalog>Existing</Catalog></ChildObjects></Configuration></MetaDataObject>"#;
        let after = "\u{feff}<?xml version=\"1.0\"?>\n<MetaDataObject version=\"2.21\" xmlns=\"urn:metadata\">\n  <Configuration uuid=\"root\">\n    <Properties><Name>Coverage</Name><Version>1.2.0.4</Version><UsePurposes></UsePurposes></Properties>\n    <ChildObjects><Catalog>Existing</Catalog><CommonForm>Dialog</CommonForm></ChildObjects>\n  </Configuration>\n</MetaDataObject>";
        assert_eq!(
            root_version_edit(before, after),
            Some(("1.2.0.0".into(), "1.2.0.4".into()))
        );
        assert!(root_children_only_change(
            before,
            &after.replace("1.2.0.4", "1.2.0.0")
        ));
        assert!(!root_children_only_change(
            before,
            &after.replace("<Name>Coverage</Name>", "<Name>Other</Name>")
        ));
    }
    #[test]
    fn root_roundtrip_allows_only_xml_formatting_not_property_values() {
        let before = b"<Configuration b=\"2\" a=\"1\"><Properties><Comment> keep </Comment><UsePurposes/></Properties></Configuration>";
        let formatted = b"<Configuration a=\"1\" b=\"2\">\n  <Properties><Comment> keep </Comment><UsePurposes></UsePurposes></Properties>\n</Configuration>";
        assert!(same_content("Configuration.xml", before, formatted));
        assert!(!same_content(
            "Configuration.xml",
            before,
            &String::from_utf8_lossy(formatted)
                .replace(" keep ", "keep")
                .into_bytes()
        ));
        assert!(!same_content("Other.xml", before, formatted));
    }

    #[test]
    fn data_processor_nested_dialog_has_no_persistent_schema_transition() {
        let refused = structural_refusals(
            &diff(
                &["DataProcessors/Tools.xml", "Configuration.xml"],
                &[
                    "DataProcessors/Tools/Forms/Import.xml",
                    "DataProcessors/Tools/Forms/Import/Ext/Form.xml",
                    "DataProcessors/Tools/Forms/Import/Ext/Form/Module.bsl",
                ],
                &[],
            ),
            true,
        );
        assert!(refused.is_empty(), "{refused:?}");
        assert!(!structural_refusals(&diff(&["Catalogs/Data.xml"], &[], &[]), true).is_empty());
    }
    #[test]
    fn native_dialog_tree_compiles_and_verifies_in_an_isolated_process() {
        const CHILD: &str = "IBCMD_RS_DIALOG_TEST_PLATFORM";
        let platform = match std::env::var(CHILD) {
            Ok(platform) => platform,
            Err(_) => {
                for platform in ["8.5.1.1150", "8.3.27.2214"] {
                    for case in ["Update", "Defaults"] {
                        let output = std::process::Command::new(std::env::current_exe().unwrap())
                        .args(["--exact", "mssql_extension_tree_load::tests::native_dialog_tree_compiles_and_verifies_in_an_isolated_process", "--nocapture"])
                        .env(CHILD, platform).env("IBCMD_RS_DIALOG_TEST_CASE", case).output().unwrap();
                        assert!(
                            output.status.success(),
                            "{platform}: {}\n{}",
                            String::from_utf8_lossy(&output.stdout),
                            String::from_utf8_lossy(&output.stderr)
                        );
                        assert!(
                            String::from_utf8_lossy(&output.stdout)
                                .lines()
                                .any(|line| line.starts_with(
                                    "test result: ok. 1 passed; 0 failed; 0 ignored;"
                                )),
                            "{platform}/{case}: child did not run the exact native fixture test"
                        );
                    }
                }
                return;
            }
        };
        let fixture = std::env::var_os("IBCMD_RS_EXTENSION_TEST_CORPUS")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/1c-fixtures")
            })
            .join("coverage")
            .join(&platform)
            .join("extensions/restructuring");
        let case = std::env::var("IBCMD_RS_DIALOG_TEST_CASE").unwrap_or_else(|_| "Update".into());
        let canonical = fixture
            .join("native")
            .join(format!("CovRestructureDialog{case}"));
        let input_scratch = Scratch::new("dialog-test-input");
        let input = input_scratch.0.clone();
        let from = if case == "Defaults" {
            fixture.join("input/dialog-defaults")
        } else {
            canonical.clone()
        };
        for path in relative_files(&from).unwrap() {
            let destination = input.join(&path);
            std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
            std::fs::copy(from.join(&path), destination).unwrap();
        }
        if case == "Update" {
            let root = std::fs::read_to_string(input.join("Configuration.xml")).unwrap();
            let formatted = root
                .trim_start_matches('\u{feff}')
                .replace("\r\n", "\n")
                .replace('\t', "  ")
                .replace("<Comment/>", "<Comment></Comment>")
                .replace("<Vendor/>", "<Vendor></Vendor>");
            std::fs::write(input.join("Configuration.xml"), formatted).unwrap();
        }
        // The unused palette declaration belongs to the native XML 2.21 envelope.
        if case == "Defaults" && platform.starts_with("8.5.") {
            let descriptor = input.join("DataProcessors/Cov_ImportTools/Forms/ImportDialog.xml");
            let document =
                ibcmd_xml::XmlReader::from_slice(&std::fs::read(&descriptor).unwrap()).unwrap();
            let attributes = document.root().attributes().iter().filter(|attribute| {
                !matches!(attribute.kind(), ibcmd_xml::AttributeKind::Namespace(Some(prefix)) if prefix == "pal")
            }).cloned().collect::<Vec<_>>();
            assert_eq!(attributes.len() + 1, document.root().attributes().len());
            let root = ibcmd_xml::XmlElement::with_parts(
                document.root().name().clone(),
                attributes,
                document.root().children().to_vec(),
            );
            let bytes = ibcmd_xml::XmlWriter::to_vec(
                &document.with_root(root),
                ibcmd_xml::LexicalPolicy::Preserve,
            )
            .unwrap();
            std::fs::write(descriptor, bytes).unwrap();
        }
        let packed = crate::module_blob::decode_base64_mime(
            &std::fs::read_to_string(fixture.join("variants/CovRestructureDialogBaseline.cfe.b64"))
                .unwrap(),
        )
        .unwrap();
        let archive = ibcmd_cf::archive::decode_archive_uniform(
            std::io::Cursor::new(&packed),
            ResourceLimits::for_input_bytes(packed.len() as u64),
            ibcmd_core::artifact::StorageProfileId::parse("mssql-extension-8.3.27").unwrap(),
            ibcmd_core::storage::StorageProvenance::new("generated dialog extension baseline")
                .unwrap(),
            ibcmd_cf::payload::PayloadEncoding::RawDeflate,
        )
        .unwrap();
        let version = if platform.starts_with("8.5.") {
            InfobaseConfigSourceVersion::V2_21
        } else {
            InfobaseConfigSourceVersion::V2_20
        };
        let base = Scratch::new("dialog-test-base");
        let report = crate::mssql_dump::extension::export_extension_entries_to_source(
            "mssql-extension-8.3.27",
            archive
                .image()
                .entries()
                .iter()
                .map(|e| {
                    (
                        e.logical_key().as_str().to_owned(),
                        e.packed_payload().to_vec(),
                    )
                })
                .collect(),
            &base.0,
            false,
            version,
        )
        .unwrap();
        // A CFE manifest is validated above, whereas SQL active images retain
        // only its content rows. Compile exactly that production representation.
        let active = StorageImage::with_retained_byte_limit(
            archive
                .image()
                .entries()
                .iter()
                .filter(|e| e.logical_key().as_str() != "configinfo")
                .cloned()
                .collect(),
            ResourceLimits::for_input_bytes(packed.len() as u64).max_retained_bytes_usize(),
        )
        .unwrap();
        assert_eq!((report.storage.failed, report.storage.opaque), (0, 0));
        let before = std::fs::read_to_string(base.0.join("Configuration.xml")).unwrap();
        let after = std::fs::read_to_string(input.join("Configuration.xml")).unwrap();
        let policy = root_policy(&before, &after, false).unwrap();
        assert_eq!(
            policy.version,
            Some((
                "1.2.0.4".into(),
                if case == "Defaults" {
                    "1.2.0.6".into()
                } else {
                    "1.2.0.5".into()
                }
            ))
        );
        let diff = diff_trees_normalized(&base.0, &input).unwrap();
        let refused = structural_refusals(&diff, true);
        assert!(refused.is_empty(), "{platform}: {refused:?}");
        let edit = compile_tree_edit_from_export(
            &input,
            &active,
            &[],
            &base.0,
            keys_by_output(&report),
            |image, expected| {
                verify_export(image, expected, version, None)?;
                let native_check = Scratch::new("dialog-native-compare");
                let report = crate::mssql_dump::extension::export_extension_image_to_source(
                    image,
                    &native_check.0,
                    version,
                    None,
                )?;
                assert_eq!((report.storage.failed, report.storage.opaque), (0, 0));
                let files = |root: &Path| {
                    relative_files(root)
                        .unwrap()
                        .into_iter()
                        .filter(|p| p != CONFIG_DUMP_INFO)
                        .map(|p| (p.clone(), std::fs::read(root.join(p)).unwrap()))
                        .collect::<BTreeMap<_, _>>()
                };
                let want = files(&canonical);
                let got = files(&native_check.0);
                let differ = want
                    .keys()
                    .chain(got.keys())
                    .filter(|p| want.get(*p) != got.get(*p))
                    .collect::<BTreeSet<_>>();
                assert!(
                    differ.is_empty(),
                    "{platform}/{case} native source differences: {differ:?}"
                );
                Ok(())
            },
        )
        .unwrap()
        .unwrap();
        assert!(
            edit.changed_files
                .iter()
                .any(|p| p.contains("/Forms/ImportDialog/Ext/Form.xml"))
        );
        assert!(
            edit.compiled_objects
                .iter()
                .any(|p| p.starts_with("DataProcessors/"))
        );
    }
    fn root_review_tree(children: &str, descriptors: &[(&str, &str, &str, &str)]) -> Scratch {
        let root = Scratch::new("root-review");
        std::fs::create_dir_all(&root.0).unwrap();
        std::fs::write(root.0.join("Configuration.xml"), format!("<Configuration><Properties><Version>1.2.0.0</Version></Properties><ChildObjects>{children}</ChildObjects></Configuration>")).unwrap();
        for (path, kind, name, uuid) in descriptors {
            let file = root.0.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file,format!("<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' version='2.20'><{kind} uuid='{uuid}'><Properties><Name>{name}</Name></Properties></{kind}></MetaDataObject>")).unwrap();
        }
        root
    }
    #[test]
    fn root_review_rejects_unbacked_membership_and_identity_deltas() {
        let a = (
            "CommonModules/A.xml",
            "CommonModule",
            "A",
            "aaaaaaaa-0000-4000-8000-000000000001",
        );
        let b = (
            "CommonModules/B.xml",
            "CommonModule",
            "B",
            "aaaaaaaa-0000-4000-8000-000000000002",
        );
        let old = root_review_tree("<CommonModule>A</CommonModule>", &[a]);
        let mut accepted = Vec::new();
        for (label, children, files) in [
            ("root-only removal", "", vec![a]),
            (
                "root-only rename",
                "<CommonModule>B</CommonModule>",
                vec![a],
            ),
            (
                "missing addition descriptor",
                "<CommonModule>A</CommonModule><CommonModule>B</CommonModule>",
                vec![a],
            ),
            (
                "duplicate membership",
                "<CommonModule>A</CommonModule><CommonModule>A</CommonModule>",
                vec![a],
            ),
            (
                "retained UUID replacement",
                "<CommonModule>A</CommonModule>",
                vec![(a.0, a.1, a.2, b.3)],
            ),
            (
                "addition name mismatch",
                "<CommonModule>A</CommonModule><CommonModule>B</CommonModule>",
                vec![a, (b.0, b.1, "Other", b.3)],
            ),
            (
                "addition aliases retained UUID",
                "<CommonModule>A</CommonModule><CommonModule>B</CommonModule>",
                vec![a, (b.0, b.1, b.2, a.3)],
            ),
        ] {
            let next = root_review_tree(children, &files);
            if require_nonstructural_source_transition(&old.0, &next.0).is_ok() {
                accepted.push(label);
            }
        }
        assert!(
            accepted.is_empty(),
            "unsafe source transitions accepted: {accepted:?}"
        );
    }
    #[test]
    fn root_review_keeps_coordinated_tableless_edits_and_requested_order() {
        let a = (
            "CommonModules/A.xml",
            "CommonModule",
            "A",
            "aaaaaaaa-0000-4000-8000-000000000001",
        );
        let b = (
            "CommonModules/B.xml",
            "CommonModule",
            "B",
            "aaaaaaaa-0000-4000-8000-000000000002",
        );
        let old = root_review_tree("<CommonModule>A</CommonModule>", &[a]);
        for (children, files) in [
            ("", vec![]),
            ("<CommonModule>B</CommonModule>", vec![(b.0, b.1, b.2, a.3)]),
            (
                "<CommonModule>A</CommonModule><CommonModule>B</CommonModule>",
                vec![a, b],
            ),
        ] {
            let next = root_review_tree(children, &files);
            require_nonstructural_source_transition(&old.0, &next.0).unwrap();
        }
        let old = root_review_tree(
            "<CommonModule>A</CommonModule><CommonModule>B</CommonModule>",
            &[a, b],
        );
        let next = root_review_tree(
            "<CommonModule>B</CommonModule><CommonModule>A</CommonModule>",
            &[a, b],
        );
        require_nonstructural_source_transition(&old.0, &next.0).unwrap();
    }
    #[test]
    fn root_review_keeps_http_and_nested_subsystems_tableless() {
        let old = root_review_tree("", &[]);
        let next = root_review_tree(
            "<HTTPService>H</HTTPService><Subsystem>S</Subsystem>",
            &[
                (
                    "HTTPServices/H.xml",
                    "HTTPService",
                    "H",
                    "aaaaaaaa-0000-4000-8000-000000000001",
                ),
                (
                    "Subsystems/S.xml",
                    "Subsystem",
                    "S",
                    "aaaaaaaa-0000-4000-8000-000000000002",
                ),
                (
                    "Subsystems/S/Subsystems/Child.xml",
                    "Subsystem",
                    "Child",
                    "aaaaaaaa-0000-4000-8000-000000000003",
                ),
            ],
        );
        let asset = next
            .0
            .join("Subsystems/S/Subsystems/Child/Ext/CommandInterface.xml");
        std::fs::create_dir_all(asset.parent().unwrap()).unwrap();
        std::fs::write(asset, b"<CommandInterface/>").unwrap();
        require_nonstructural_source_transition(&old.0, &next.0).unwrap();
        std::fs::create_dir_all(next.0.join("Subsystems/S/Other")).unwrap();
        std::fs::write(next.0.join("Subsystems/S/Other/Unknown.xml"), b"unknown").unwrap();
        assert!(require_nonstructural_source_transition(&old.0, &next.0).is_err());
    }
    #[test]
    fn root_review_selected_imports_require_selected_counterparts() {
        let a = (
            "CommonModules/A.xml",
            "CommonModule",
            "A",
            "aaaaaaaa-0000-4000-8000-000000000001",
        );
        let b = (
            "CommonModules/B.xml",
            "CommonModule",
            "B",
            "aaaaaaaa-0000-4000-8000-000000000002",
        );
        let old = root_review_tree("<CommonModule>A</CommonModule>", &[a]);
        let next = root_review_tree(
            "<CommonModule>A</CommonModule><CommonModule>B</CommonModule>",
            &[a, b],
        );
        for (prefixes, valid) in [
            (vec![], true),
            (vec!["Configuration.xml".to_owned()], false),
            (vec!["CommonModules/B".to_owned()], false),
            (
                vec!["Configuration.xml".to_owned(), "CommonModules/B".to_owned()],
                true,
            ),
        ] {
            let mut diff = diff_trees_normalized(&old.0, &next.0).unwrap();
            if !prefixes.is_empty() {
                for files in [&mut diff.changed, &mut diff.added, &mut diff.removed] {
                    files.retain(|p| under_prefixes(p, &prefixes));
                }
            }
            assert_eq!(
                compilation_root_edit(&old.0, &next.0, &diff, &prefixes).is_ok(),
                valid,
                "{prefixes:?}"
            );
        }
        let next = root_review_tree("<CommonModule>B</CommonModule>", &[a]);
        for (root, text) in [(&old.0, "before"), (&next.0, "after")] {
            let file = root.join("CommonModules/A/Ext/Module.bsl");
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        }
        let prefixes = vec!["CommonModules/A/Ext/Module.bsl".to_owned()];
        let mut diff = diff_trees_normalized(&old.0, &next.0).unwrap();
        for files in [&mut diff.changed, &mut diff.added, &mut diff.removed] {
            files.retain(|p| under_prefixes(p, &prefixes));
        }
        compilation_root_edit(&old.0, &next.0, &diff, &prefixes).unwrap();
    }
}
