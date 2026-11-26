//! The load of edits a module overlay cannot carry: every object the tree
//! changed, added or lost is compiled from the tree against the base's own
//! rows (`crate::mssql::compile_source_rows_offline`, the staging compiler
//! run offline), and the file is written as the base's entries with those
//! rows put in: replaced, added, or removed.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::Path,
};

use anyhow::{Context, Result};
use ibcmd_cf::{
    archive::{CfArchive, decode_archive_uniform},
    payload::PayloadEncoding,
    writer::publish_storage_image_new,
};
use ibcmd_core::{
    artifact::StorageProfileId,
    limits::ResourceLimits,
    storage::{
        CompressionKind, MultipartIdentity, OpaqueStorageMetadata, StorageEntry, StorageImage,
        StorageKey, StorageName, StorageOrigin, StoragePayloads, StorageProvenance,
    },
};

use super::{CONFIG_DUMP_INFO, LoadError, LoadReport, OVERLAY_PROFILE, TreeDiff};
use crate::module_blob::inflate_raw;

/// The object a tree path belongs to, as a source prefix the compiler
/// selects by: `Catalogs/Товары` for `Catalogs/Товары.xml` and everything
/// under `Catalogs/Товары/`; `Configuration.xml` for the root's own files.
pub(crate) fn object_prefix(path: &str) -> Option<String> {
    if path == CONFIG_DUMP_INFO {
        return None;
    }
    let mut segments = path.split('/');
    let first = segments.next()?;
    if first == "Configuration.xml" || first == "Ext" {
        return Some("Configuration.xml".to_owned());
    }
    let second = segments.next()?;
    let name = second.strip_suffix(".xml").unwrap_or(second);
    Some(format!("{first}/{name}"))
}

/// A metadata (descriptor) XML: an `.xml` outside every `Ext/` directory,
/// other than the root's own files.
fn is_descriptor_xml(path: &str) -> bool {
    path.ends_with(".xml")
        && !path.starts_with("Ext/")
        && !path.contains("/Ext/")
        && path != CONFIG_DUMP_INFO
        && path != "Configuration.xml"
}

/// The descriptor XML whose child list names `path` (a descriptor XML):
/// `Catalogs/X.xml` for `Catalogs/X/Forms/F.xml`, `Configuration.xml` for a
/// top-level object.
fn parent_descriptor(path: &str) -> String {
    let segments = path.split('/').collect::<Vec<_>>();
    if segments.len() <= 2 {
        return "Configuration.xml".to_owned();
    }
    format!("{}.xml", segments[..segments.len() - 2].join("/"))
}

/// The parents (still in the tree) of the descriptors the tree added or
/// removed: their child lists changed, which a patch of the base row keeps.
fn parents_of_added_or_removed(diff: &TreeDiff, edited: &Path) -> Vec<String> {
    diff.added
        .iter()
        .chain(&diff.removed)
        .filter(|path| is_descriptor_xml(path))
        .map(|path| parent_descriptor(path))
        .filter(|parent| edited.join(parent).is_file())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The objects the tree lost whole: their metadata file is gone.
fn removed_objects(diff: &TreeDiff) -> BTreeSet<String> {
    diff.removed
        .iter()
        .filter(|path| path.matches('/').count() == 1 && path.ends_with(".xml"))
        .filter_map(|path| object_prefix(path))
        .collect()
}

/// The compiled rows of an edit and the entries it removes.
pub(crate) struct CompiledEdit {
    /// Entry -> packed payload: the entries of the changed or added files,
    /// and new entries (the root row's families already following the tree).
    pub rows: BTreeMap<String, Vec<u8>>,
    /// Entries removed with their bodies (`<key>.<n>`).
    pub dropped: BTreeSet<String>,
    pub prefixes: BTreeSet<String>,
    pub removed: BTreeSet<String>,
}

/// Compiles the objects `diff` touches in `edited` (a tree in configuration
/// layout) against `base_rows` (the base's entries), `keys` mapping the base
/// export's files to their entries.
pub(crate) fn compile_edit(
    edited: &Path,
    base_rows: HashMap<String, Vec<u8>>,
    whole_diff: &TreeDiff,
    keys: &BTreeMap<String, String>,
) -> std::result::Result<CompiledEdit, LoadError> {
    // Module text is overlaid on the base's body as `cf load` always did
    // (a form's module into its base body); only the other edits compile --
    // a form the compiler cannot write stays loadable for its module.
    let modules = super::classify(whole_diff, keys).edits;
    let mut module_paths = BTreeSet::new();
    for edit in &modules {
        let path = match edit {
            super::Edit::Module { text_path, .. } => text_path,
            super::Edit::FormBody { module, .. } => module,
        };
        module_paths.insert(path.clone());
        // A closed module replaced by its text: the `.bin` it replaces.
        if let Some(stem) = path.strip_suffix(".bsl") {
            module_paths.insert(format!("{stem}.bin"));
        }
    }
    let keep = |paths: &Vec<String>| {
        paths
            .iter()
            .filter(|path| !module_paths.contains(*path))
            .cloned()
            .collect::<Vec<_>>()
    };
    let compile_diff = TreeDiff {
        changed: keep(&whole_diff.changed),
        added: keep(&whole_diff.added),
        removed: keep(&whole_diff.removed),
    };
    let diff = &compile_diff;
    let module_bases = modules
        .iter()
        .filter_map(|edit| match edit {
            super::Edit::FormBody { key, .. } | super::Edit::Module { key, .. } => base_rows
                .get(key)
                .map(|packed| (key.clone(), packed.clone())),
        })
        .collect::<BTreeMap<_, _>>();
    // A path no object owns (an external object's layout, a stray root file)
    // would be dropped without a word; it is refused instead.
    let unowned = diff
        .changed
        .iter()
        .chain(&diff.added)
        .chain(&diff.removed)
        .filter(|path| path.as_str() != CONFIG_DUMP_INFO)
        .filter(|path| {
            object_prefix(path).is_none_or(|prefix| {
                let metadata = if prefix == "Configuration.xml" {
                    prefix
                } else {
                    format!("{prefix}.xml")
                };
                // Either tree has the object's metadata file: the edited one,
                // or (a removed object) the base export's keys.
                !edited.join(&metadata).is_file() && !keys.contains_key(&metadata)
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    if !unowned.is_empty() {
        return Err(LoadError::Unsupported(unowned));
    }
    let removed = removed_objects(diff);
    let prefixes = diff
        .changed
        .iter()
        .chain(&diff.added)
        .chain(&diff.removed)
        .filter_map(|path| object_prefix(path))
        .filter(|prefix| !removed.contains(prefix))
        .collect::<BTreeSet<_>>();

    // Every entry of a removed object goes: its metadata row and its bodies.
    let mut dropped = BTreeSet::new();
    for object in &removed {
        let Some(uuid) = keys.get(&format!("{object}.xml")) else {
            return Err(anyhow::anyhow!(
                "`{object}.xml` was removed, but the base export does not name its entry"
            )
            .into());
        };
        dropped.insert(uuid.clone());
        // Its forms, templates and commands are entries of their own (their
        // own uuids): they go with it.
        for path in &diff.removed {
            if object_prefix(path).as_deref() == Some(object.as_str())
                && let Some(key) = keys.get(path)
            {
                dropped.insert(key.clone());
            }
        }
    }
    let base_keys = base_rows.keys().cloned().collect::<BTreeSet<_>>();
    let root_children_changed = parents_of_added_or_removed(diff, edited)
        .iter()
        .any(|parent| parent == "Configuration.xml");
    let root_key = keys.get("Configuration.xml").cloned();
    let base_root = root_key
        .as_ref()
        .and_then(|key| base_rows.get(key).cloned());
    let compiled = if prefixes.is_empty() {
        crate::mssql::CompiledContainerRows::default()
    } else {
        let prefixes = prefixes.iter().cloned().collect::<Vec<_>>();
        let fresh = parents_of_added_or_removed(diff, edited)
            .into_iter()
            .filter(|parent| parent != "Configuration.xml")
            .collect::<Vec<_>>();
        crate::mssql::compile_source_rows_offline(edited, base_rows, &prefixes, &fresh)
            .context("failed to compile the edited objects")?
    };
    // Only the entries of the files the tree changed or added are taken, and
    // new entries: an object's other bodies stay the base's byte for byte (a
    // recompile may lose what the compiler cannot read, as a type it does
    // not know).
    let edited_keys = diff
        .changed
        .iter()
        .chain(&diff.added)
        .filter_map(|path| keys.get(path).map(|key| (key.clone(), path.clone())))
        .collect::<BTreeMap<_, _>>();
    let mut rows = compiled
        .rows
        .into_iter()
        .filter(|(key, _)| edited_keys.contains_key(key) || !base_keys.contains(key))
        .collect::<BTreeMap<_, _>>();
    let lost = edited_keys
        .iter()
        .filter(|(key, _)| !rows.contains_key(*key))
        .map(|(_, path)| path.clone())
        .collect::<Vec<_>>();
    if !lost.is_empty() {
        let skipped = crate::mssql::skipped_bodies();
        return Err(anyhow::anyhow!(
            "the compiler wrote no entry for the edited files {}{}",
            lost.join(", "),
            if skipped.is_empty() {
                String::new()
            } else {
                format!(" (bodies it could not compile: {})", skipped.join("; "))
            }
        )
        .into());
    }
    // A body file the tree lost, of an object it kept: its entry goes when
    // the compile did not write it again.
    for path in &diff.removed {
        if let Some(key) = keys.get(path)
            && !rows.contains_key(key)
            && !prefixes.is_empty()
            && object_prefix(path).is_some_and(|prefix| prefixes.contains(&prefix))
        {
            dropped.insert(key.clone());
        }
    }
    // Objects came or went at the top: the root row's family lists follow
    // the tree's <ChildObjects> (a patch of the row keeps the base's lists).
    if root_children_changed {
        let key = root_key.context("the base export does not name the configuration's entry")?;
        let current = rows
            .get(&key)
            .cloned()
            .or(base_root)
            .context("the base has no configuration entry")?;
        let plain = String::from_utf8(inflate_raw(&current)?)
            .context("the configuration row is not UTF-8")?;
        let patched = with_tree_families(&plain, &tree_root_children(edited)?)?;
        rows.insert(key, crate::module_blob::deflate_raw(patched.as_bytes())?);
    }
    // The module edits, unless their entry was compiled (a form whose
    // Form.xml changed too compiles with its module).
    for edit in &modules {
        match edit {
            super::Edit::Module { key, text_path } if !rows.contains_key(key) => {
                let text = super::module_text(edited, text_path)
                    .map_err(|error| anyhow::anyhow!(error))?;
                let packed = match module_bases.get(key) {
                    Some(base) => {
                        crate::module_blob::pack_module_blob_bytes(&text, Some(base), None)?.blob
                    }
                    None => crate::mssql::pack_module_text(key, &text)?,
                };
                rows.insert(key.clone(), packed);
            }
            super::Edit::FormBody { key, module } if !rows.contains_key(key) => {
                let text =
                    super::module_text(edited, module).map_err(|error| anyhow::anyhow!(error))?;
                let base = module_bases
                    .get(key)
                    .with_context(|| format!("the base has no form body {key}"))?;
                rows.insert(
                    key.clone(),
                    crate::module_blob::pack_form_body_blob_from_module_text(base, &text)?.blob,
                );
            }
            _ => {}
        }
    }
    Ok(CompiledEdit {
        rows,
        dropped,
        prefixes,
        removed,
    })
}

/// Writes `output`: `base` with the objects `diff` touches compiled from
/// `edited`. `keys` maps the base export's files to their entries.
pub(crate) fn load_compiled(
    edited: &Path,
    base: &Path,
    output: &Path,
    diff: &TreeDiff,
    keys: &BTreeMap<String, String>,
    report: &mut LoadReport,
) -> std::result::Result<(), LoadError> {
    let (archive, limits) = decode_base(base)?;
    let base_rows = archive
        .image()
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.logical_key().as_str().to_owned(),
                entry.packed_payload().to_vec(),
            )
        })
        .collect::<HashMap<_, _>>();
    let edit = compile_edit(edited, base_rows, diff, keys)?;
    write_edit(&archive, edit.rows, &edit.dropped, output, limits)?;
    report.compiled_objects = edit.prefixes.into_iter().collect();
    report.removed_objects = edit.removed.into_iter().collect();
    report.applied = applied_paths(diff);
    Ok(())
}

/// The files of `diff` a load took, sorted.
pub(crate) fn applied_paths(diff: &TreeDiff) -> Vec<String> {
    let mut applied = diff
        .changed
        .iter()
        .chain(&diff.added)
        .filter(|path| path.as_str() != CONFIG_DUMP_INFO)
        .cloned()
        .collect::<Vec<_>>();
    applied.sort();
    applied
}

pub(crate) fn decode_base(base: &Path) -> Result<(CfArchive, ResourceLimits)> {
    let limits = ResourceLimits::for_input_bytes(fs::metadata(base).context("base")?.len());
    let archive = decode_archive_uniform(
        fs::File::open(base).context("base")?,
        limits,
        StorageProfileId::parse(OVERLAY_PROFILE).expect("static profile id is valid"),
        StorageProvenance::new("cf load base").expect("static provenance is valid"),
        PayloadEncoding::RawDeflate,
    )
    .map_err(|error| anyhow::anyhow!("failed to decode base `{}`: {error}", base.display()))?;
    Ok((archive, limits))
}

/// Writes `output`: `archive`'s entries with `rows` put in and `dropped`
/// left out, its `versions` (or an extension's `configinfo`) following.
pub(crate) fn write_edit(
    archive: &CfArchive,
    mut rows: BTreeMap<String, Vec<u8>>,
    dropped: &BTreeSet<String>,
    output: &Path,
    limits: ResourceLimits,
) -> Result<()> {
    let base_payload = |key: &str| {
        archive
            .image()
            .entries()
            .iter()
            .find(|entry| entry.logical_key().as_str() == key)
            .map(|entry| entry.packed_payload().to_vec())
    };
    if let Some(base_versions) = base_payload("versions") {
        let changes = rows.keys().cloned().collect::<Vec<_>>();
        let mut versions = if changes.is_empty() {
            base_versions
        } else {
            crate::module_blob::patch_versions_blob_bytes_allowing_additions(
                &base_versions,
                &changes,
                true,
            )?
            .blob
        };
        // The versions row lists every entry: a removed one leaves it too.
        if !dropped.is_empty() {
            versions = without_versions_entries(&versions, |name| is_dropped(dropped, name))?;
        }
        rows.insert("versions".to_owned(), versions);
    } else if let Some(configinfo) = base_payload("configinfo") {
        // An extension: configinfo keeps the SHA-1 of every entry's packed
        // bytes, and a platform updating an installed extension keeps each
        // entry whose digest did not change.
        use sha1::{Digest, Sha1};
        let digests = rows
            .iter()
            .map(|(key, packed)| {
                (
                    key.clone(),
                    crate::module_blob::encode_base64(&Sha1::digest(packed)),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let text =
            String::from_utf8(inflate_raw(&configinfo)?).context("configinfo is not UTF-8")?;
        let rewritten = with_configinfo(&text, &digests, |name| is_dropped(dropped, name))?;
        rows.insert(
            "configinfo".to_owned(),
            crate::module_blob::deflate_raw(rewritten.as_bytes())?,
        );
    }
    let image = rebuilt_image(archive, &rows, dropped, limits)?;
    publish_storage_image_new(archive.metadata(), &image, output, limits)
        .map_err(|error| anyhow::anyhow!("failed to write `{}`: {error}", output.display()))?;
    Ok(())
}

/// The base's entries in their order with `rows` put in (replaced where the
/// base has the key, appended where it has not) and `dropped` keys and their
/// bodies (`<key>.<n>`) left out.
fn rebuilt_image(
    archive: &CfArchive,
    rows: &BTreeMap<String, Vec<u8>>,
    dropped: &BTreeSet<String>,
    limits: ResourceLimits,
) -> Result<StorageImage> {
    let gone = |key: &str| is_dropped(dropped, key);
    let base_entries = archive.image().entries();
    let mut entries = Vec::with_capacity(base_entries.len() + rows.len());
    let mut placed = BTreeSet::new();
    for entry in base_entries {
        let key = entry.logical_key().as_str();
        if gone(key) {
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
    let template = base_entries
        .first()
        .context("the base has no entries to take an element header from")?;
    for (key, packed) in rows {
        if !placed.contains(key) && !gone(key) {
            entries.push(new_entry(template, key, packed)?);
        }
    }
    StorageImage::with_retained_byte_limit(entries, limits.max_retained_bytes_usize())
        .context("the rebuilt image is not valid")
}

/// The root's child objects in the tree: family kind -> the objects' uuids
/// in `<ChildObjects>` order, read from their own XML files.
fn tree_root_children(edited: &Path) -> Result<BTreeMap<String, Vec<String>>> {
    let xml = fs::read_to_string(edited.join("Configuration.xml"))
        .context("failed to read Configuration.xml")?;
    let section = xml
        .split_once("<ChildObjects>")
        .and_then(|(_, rest)| rest.split_once("</ChildObjects>"))
        .map_or("", |(inner, _)| inner);
    let mut children = BTreeMap::<String, Vec<String>>::new();
    for line in section.lines() {
        let line = line.trim();
        let Some((kind, rest)) = line.strip_prefix('<').and_then(|rest| rest.split_once('>'))
        else {
            continue;
        };
        let Some(name) = rest.strip_suffix(&format!("</{kind}>")) else {
            continue;
        };
        let Some(folder) = crate::mssql_dump::root_family_folder(kind) else {
            anyhow::bail!("Configuration.xml names a child kind `{kind}` no family holds");
        };
        let path = edited.join(folder).join(format!("{name}.xml"));
        let bytes =
            fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let uuid = crate::module_blob::parse_simple_metadata_xml_properties(&bytes)?.uuid;
        children.entry(kind.to_owned()).or_default().push(uuid);
    }
    Ok(children)
}

/// The root row (`plain`, its text) with every family list
/// (`{<class>,<count>,<uuid>...}`) the tree names rewritten to the tree's
/// children, in the tree's order; a family the tree names none of is emptied.
fn with_tree_families(plain: &str, children: &BTreeMap<String, Vec<String>>) -> Result<String> {
    let bytes = plain.as_bytes();
    let mut out = String::with_capacity(plain.len());
    let mut copied = 0;
    let mut at = 0;
    let mut placed = BTreeSet::new();
    while let Some(offset) = plain[at..].find('{') {
        let start = at + offset;
        at = start + 1;
        // No class here: too near the end, or 36 bytes on is inside a
        // letter (a `{` before Cyrillic text). The families further on are
        // still looked for.
        let Some(class) = plain.get(start + 1..start + 37) else {
            continue;
        };
        let Some(kind) = crate::compiler::root::configuration_family_kind(class) else {
            continue;
        };
        // `{<class>,<count>` then `,<uuid>` * count then `}`.
        let rest = &plain[start + 37..];
        let Some(rest) = rest.strip_prefix(',') else {
            continue;
        };
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        let Ok(count) = rest[..digits].parse::<usize>() else {
            continue;
        };
        let mut end = start + 38 + digits;
        let mut valid = true;
        for _ in 0..count {
            let item = plain.get(end..end + 37).unwrap_or_default();
            if !item.starts_with(',') || uuid::Uuid::parse_str(item[1..].trim()).is_err() {
                valid = false;
                break;
            }
            end += 37;
        }
        if !valid || bytes.get(end) != Some(&b'}') {
            continue;
        }
        let uuids = children.get(kind).cloned().unwrap_or_default();
        out.push_str(&plain[copied..start]);
        out.push_str(&format!("{{{class},{}", uuids.len()));
        for uuid in &uuids {
            out.push(',');
            out.push_str(uuid);
        }
        out.push('}');
        copied = end + 1;
        at = end + 1;
        placed.insert(kind.to_owned());
    }
    out.push_str(&plain[copied..]);
    if let Some(kind) = children.keys().find(|kind| !placed.contains(*kind)) {
        anyhow::bail!("the root row has no family for the tree's {kind} objects");
    }
    Ok(out)
}

/// `configinfo` (an extension's digest list, `{<count>,"<entry>",<sha1>,...}`
/// as its last block) with `digests` set -- replaced or added -- and the
/// entries `gone` names left out.
fn with_configinfo(
    text: &str,
    digests: &BTreeMap<String, String>,
    gone: impl Fn(&str) -> bool,
) -> Result<String> {
    let start = text
        .rfind("\n{")
        .context("configinfo has no digest block")?
        + 1;
    let block = text[start..].trim_end();
    let inner = block
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
        .context("configinfo digest block is not braced")?;
    let (_, pairs) = inner
        .split_once(',')
        .context("configinfo digest block has no count")?;
    let fields = pairs.split(',').collect::<Vec<_>>();
    if fields.len() % 2 != 0 {
        anyhow::bail!("configinfo digests are not entry/digest pairs");
    }
    let mut kept = Vec::new();
    let mut seen = BTreeSet::new();
    for pair in fields.chunks(2) {
        let name = pair[0].trim().trim_matches('"');
        if gone(name) {
            continue;
        }
        seen.insert(name.to_owned());
        let digest = digests.get(name).map_or(pair[1].trim(), String::as_str);
        kept.push(format!("\"{name}\",{digest}"));
    }
    for (name, digest) in digests {
        if !seen.contains(name) {
            kept.push(format!("\"{name}\",{digest}"));
        }
    }
    // Added keys can precede the original keys. The manifest contract
    // requires the complete mapping to remain unique and sorted.
    kept.sort_unstable();
    Ok(format!(
        "{}{{{},{}}}{}",
        &text[..start],
        kept.len(),
        kept.join(","),
        &text[start + block.len()..]
    ))
}

/// `key` is a dropped entry or one of its bodies (`<key>.<n>`).
pub(crate) fn is_dropped(dropped: &BTreeSet<String>, key: &str) -> bool {
    dropped.contains(key)
        || key
            .split_once('.')
            .is_some_and(|(owner, _)| dropped.contains(owner))
}

/// The versions row (`{1,<count>,"<entry>",<generation>,...}`) without the
/// entries `gone` names; the count follows.
fn without_versions_entries(packed: &[u8], gone: impl Fn(&str) -> bool) -> Result<Vec<u8>> {
    let plain = String::from_utf8(inflate_raw(packed)?).context("versions is not UTF-8")?;
    let bom = plain.starts_with('\u{feff}');
    let text = plain.trim_start_matches('\u{feff}');
    let body = text
        .trim_end()
        .strip_prefix("{1,")
        .and_then(|rest| rest.strip_suffix('}'))
        .context("versions is not {1,<count>,...}")?;
    let (_, pairs) = body.split_once(',').context("versions has no count")?;
    let fields = pairs.split(',').collect::<Vec<_>>();
    if fields.len() % 2 != 0 {
        anyhow::bail!("versions pairs are not name/generation pairs");
    }
    let mut kept = Vec::new();
    for pair in fields.chunks(2) {
        if !gone(pair[0].trim().trim_matches('"')) {
            kept.push(format!("{},{}", pair[0], pair[1]));
        }
    }
    let rewritten = format!(
        "{}{{1,{},{}}}",
        if bom { "\u{feff}" } else { "" },
        kept.len(),
        kept.join(",")
    );
    crate::module_blob::deflate_raw(rewritten.as_bytes())
}

fn payloads(packed: &[u8]) -> Result<StoragePayloads> {
    let unpacked = inflate_raw(packed).context("a compiled row is not raw deflate")?;
    StoragePayloads::new(packed.to_vec(), unpacked).context("compiled row payloads")
}

pub(crate) fn with_payload(entry: &StorageEntry, packed: &[u8]) -> Result<StorageEntry> {
    StorageEntry::new(
        entry.logical_name().clone(),
        entry.logical_key().clone(),
        entry.multipart(),
        entry.opaque_metadata().clone(),
        payloads(packed)?,
        CompressionKind::raw_deflate(),
        entry.origin().clone(),
    )
    .context("replaced entry")
}

/// A new entry named `key`: the template's attributes and element-header
/// prefix (its timestamps) with the new name.
pub(crate) fn new_entry(template: &StorageEntry, key: &str, packed: &[u8]) -> Result<StorageEntry> {
    let header = template.raw_header();
    let prefix = ibcmd_v8::format15::ELEMENT_HEADER_PREFIX_SIZE.min(header.len());
    let mut raw_header = header[..prefix].to_vec();
    raw_header.extend(key.encode_utf16().flat_map(u16::to_le_bytes));
    raw_header.extend_from_slice(&[0; 4]);
    StorageEntry::new(
        StorageName::new(key).context("new entry name")?,
        StorageKey::new(key).context("new entry key")?,
        MultipartIdentity::single(),
        OpaqueStorageMetadata::new(template.attributes().to_vec(), raw_header)
            .context("new entry metadata")?,
        payloads(packed)?,
        CompressionKind::raw_deflate(),
        StorageOrigin::new(
            template.source_profile().clone(),
            StorageProvenance::new(&format!("cf-load:{key}")).context("new entry provenance")?,
        ),
    )
    .context("new entry")
}
