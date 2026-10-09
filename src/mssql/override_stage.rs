//! Step 2 of #388: a patch stage that carries what the target's own rows cannot.
//!
//! A patch stage takes an object's descriptor row from the target and rewrites
//! only its name, synonym and comment, so an attribute, a tabular section, an
//! enum value, a property, a subsystem's content, a new object or a removed one
//! never got through it (`docs/import/patch-mode.md`). The guard
//! (`stage_guard`) turned that silent loss into a refusal; this module makes the
//! stage carry the change:
//!
//! * **what differs** is asked of `apply_check::check_tree_against_db`, which
//!   compares every metadata file of the tree with the descriptor the target
//!   stores for the same uuid (as the target's export would write it);
//! * an object the target does not hold (a new catalog, form, template, a
//!   nested subsystem the patch stage skips) is built whole from the tree, the
//!   way a base-free stage builds it: its descriptor and every body row;
//! * an object the target holds whose descriptor differs gets its descriptor
//!   row compiled from the tree in place of the patched one -- unless the
//!   patched row is what the compiler writes (a change of the name, synonym or
//!   comment only), in which case the target's row stays, with the state the XML
//!   does not carry;
//! * an object the target holds and the tree lacks leaves the configuration: its
//!   rows are named in the `deleted` row, as the platform's own import names
//!   them, together with the rows of an online update that are still pending,
//!   and its names go from `versions`.
//!
//! The rows are staged over the target's exactly as before; the guard then
//! exports the state the apply would leave (the `deleted` names removed) and
//! compares it with the tree, so whatever this module gets wrong still refuses
//! the import.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

use super::empty_stage::{
    EmptyStageContext, EmptyStageObject, catch, descriptor_xmls_of, map_heaviest_first,
    prepare_empty_object,
};
use super::patch_refusal::ObjectFailure;
use super::stage_guard::{in_scope, normalize_prefix};
use super::{PreparedMetadataBodyStage, PreparedMetadataObjectStage, query_json, quote_ident};
use crate::apply_check::ObjectOp;
use crate::cli::MssqlStageSourceObjectsArgs;
use crate::metadata_model::brace::{Brace, parse_row, serialize_row};
use crate::metadata_model::compile_descriptor;
use crate::module_blob::{
    deflate_raw, hex_sha256, inflate_raw, parse_simple_metadata_xml_properties,
};
use crate::source_listing;
use crate::sql::SqlExec;

/// The row that lists what an import removes (`docs/import/patch-mode.md`,
/// section 7).
pub(super) const DELETED_ROW: &str = "deleted";

/// The text every error of a build starts with; `patch_refusal` recognizes it.
pub(super) const CANNOT_BUILD: &str = "cannot build from the tree: ";

/// What a patch stage says when the tree's predefined items are not the ones
/// the target's row holds: the row cannot be patched, the data is compiled.
pub(super) const PREDEFINED_ITEMS_DIFFER: &str = "predefined items differ from the stored row: ";

/// Whether the failure of a patch stage's object is one a build from the tree
/// fixes.
pub(super) fn is_buildable(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| cause.to_string().starts_with(PREDEFINED_ITEMS_DIFFER))
}

/// What the tree changes in the target, by how the stage takes it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct Plan {
    /// Tree files (relative, `/`) of objects the target does not hold.
    pub added: Vec<String>,
    /// Objects the target holds whose descriptor differs from the tree's:
    /// the object's id and its tree file.
    pub changed: Vec<(String, String)>,
    /// Ids of objects the target holds and the tree lacks.
    pub removed: Vec<String>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.changed.is_empty() && self.removed.is_empty()
    }

    /// Whether the patch loop must leave this tree file to the build: the
    /// target holds no row for it to patch.
    pub fn is_added(&self, relative: &str) -> bool {
        self.added
            .iter()
            .any(|added| added.eq_ignore_ascii_case(relative))
    }
}

/// Asks the comparison of the tree with the target which objects differ.

pub(super) fn plan_with_source(
    args: &MssqlStageSourceObjectsArgs,
    sql: &SqlExec,
    source: Option<&crate::module_blob::MetadataSourceContext>,
) -> Result<Plan> {
    let partial = !args.path_prefix.is_empty();
    let scope = args
        .path_prefix
        .iter()
        .map(|prefix| normalize_prefix(prefix))
        .filter(|prefix| !prefix.is_empty())
        .collect::<Vec<_>>();
    let version = args.source_version.map(|version| version.as_str());
    let verdict = crate::apply_check::check_tree_against_db_with_source(
        sql,
        &args.database,
        &args.source_root,
        version,
        partial,
        source,
    )
    .context("не удалось сравнить дерево с описаниями объектов базы")?;
    let mut plan = Plan::default();
    for object in &verdict.objects {
        match object.op {
            ObjectOp::Added if in_scope(&scope, &object.file_name) => {
                plan.added.push(object.file_name.clone());
            }
            ObjectOp::Changed if in_scope(&scope, &object.file_name) => {
                plan.changed
                    .push((object.id.clone(), object.file_name.clone()));
            }
            ObjectOp::Removed if !partial => plan.removed.push(object.id.clone()),
            _ => {}
        }
    }
    plan.added.sort();
    plan.changed.sort();
    plan.removed.sort();
    Ok(plan)
}

/// A descriptor row compiled from the tree in place of the patched one.
#[derive(Debug, Clone)]
pub(super) struct DescriptorRow {
    pub blob: Vec<u8>,
    pub sha256: String,
    pub plain_bytes: usize,
}

/// The tree a build compiles from, and the database it is for.
pub(super) struct Tree<'a> {
    pub root: &'a Path,
    /// The XML version asked for; the tree's own when absent.
    pub version: Option<&'a str>,
    pub database: &'a str,
}

/// What the build produced.
#[derive(Default)]
pub(super) struct Built {
    /// Whole objects from the tree.
    pub objects: Vec<PreparedMetadataObjectStage>,
    /// Ids among them the target holds no row of.
    pub new_ids: HashSet<String>,
    /// Descriptors compiled in place of patched ones, by object id.
    pub descriptors: HashMap<String, DescriptorRow>,
    pub failures: Vec<ObjectFailure>,
    /// Tree files of the whole objects, `new`, `changed` or `rebuilt` first.
    pub built_files: Vec<String>,
    /// Tree files whose descriptor was compiled in place of the patched one.
    pub compiled_files: Vec<String>,
}

/// Compiles what the plan asks for. `patched` gives the descriptor blob a
/// patch stage prepared for an object id (the objects it stages), `None` for
/// an object it does not stage; `rebuild` are objects of the target whose patch
/// failed in a way a build fixes.
///
/// For an object the target holds whose descriptor differs from the tree's
/// (the comparison is on exported XML, and several objects of a tree differ
/// only in how the model writes them), the descriptor is compiled and compared
/// with the row the stage would leave -- the patched one, or the target's own
/// for an object a patch stage does not stage. Only a compiled row that
/// differs is used.
#[cfg(test)]
pub(super) fn build(
    tree: &Tree<'_>,
    sql: &SqlExec,
    plan: &Plan,
    rebuild: &[PathBuf],
    patched: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Result<Built> {
    build_with_source(tree, sql, plan, rebuild, patched, None)
}

pub(super) fn build_with_source(
    tree: &Tree<'_>,
    sql: &SqlExec,
    plan: &Plan,
    rebuild: &[PathBuf],
    patched: &dyn Fn(&str) -> Option<Vec<u8>>,
    source: Option<&crate::module_blob::MetadataSourceContext>,
) -> Result<Built> {
    if let Some(source) = source {
        source.require_source_root(tree.root)?;
        source.require_original_unchanged()?;
    }
    let mut built = Built::default();
    // Objects the target holds whose patch failed in a way a build fixes.
    let rebuilt = rebuild
        .iter()
        .map(|xml| relative_of(tree.root, xml))
        .collect::<Vec<_>>();
    if plan.added.is_empty() && plan.changed.is_empty() && rebuilt.is_empty() {
        return Ok(built);
    }

    let root = tree.root;
    let walked = match source.and_then(|source| source.original_source()) {
        Some(original) => source_listing::TreeWalk {
            files: original
                .baseline()
                .files()
                .map(|member| root.join(member.path()))
                .collect(),
            listing: None,
        },
        None => source_listing::walk(root),
    };
    let descriptor_paths = descriptor_xmls_of(root, &walked.files);
    let files = super::empty_stage::read_descriptor_xmls_with_source(&descriptor_paths, source)?;
    let version = tree.version;
    let context =
        EmptyStageContext::for_objects_with_source(root, version, &files, walked.listing, source)?;
    drop(files);

    // Descriptors of objects the target holds: the compiler's row is used when
    // it is not the row the stage would leave.
    let mut whole = Vec::<(String, &'static str)>::new();
    let paths = plan
        .changed
        .iter()
        .map(|(_, relative)| at(root, relative))
        .collect::<Vec<_>>();
    let compiled = map_heaviest_first(root, &paths, |path| compile_descriptor_row(&context, path))?;
    for ((id, relative), compiled) in plan.changed.iter().zip(compiled) {
        // A descriptor the compiler cannot write stays as it is: the guard
        // compares the state with the tree either way.
        let Ok((compiled_id, kind, mut plain)) = compiled else {
            continue;
        };
        if compiled_id != *id {
            continue;
        }
        let staged = patched(id);
        let current = match &staged {
            Some(blob) => Some(blob.clone()),
            None => super::fetch_config_blob(sql, tree.database, id).ok(),
        };
        let Some(current_plain) = current.and_then(|blob| inflate_raw(&blob).ok()) else {
            continue;
        };
        if kind == "Constant" {
            // The always-used flag is stored, not exported: keep the target's.
            if let Ok(grafted) = graft_constant_flag(&plain, &current_plain) {
                plain = grafted;
            }
        }
        if plain == current_plain {
            continue;
        }
        if staged.is_some() {
            let blob = deflate_raw(&plain)?;
            built.descriptors.insert(
                id.clone(),
                DescriptorRow {
                    sha256: hex_sha256(&blob),
                    plain_bytes: plain.len(),
                    blob,
                },
            );
            built.compiled_files.push(relative.clone());
        } else {
            // A patch stage does not stage this object at all.
            whole.push((relative.clone(), "changed"));
        }
    }
    whole.extend(plan.added.iter().map(|relative| (relative.clone(), "new")));
    whole.extend(rebuilt.iter().map(|relative| (relative.clone(), "rebuilt")));

    // Objects the patch stage has no row for, or cannot patch: built whole.
    whole.sort();
    whole.dedup_by(|left, right| left.0 == right.0);
    let paths = whole
        .iter()
        .map(|(relative, _)| at(root, relative))
        .collect::<Vec<_>>();
    let objects = map_heaviest_first(root, &paths, |path| {
        prepare_empty_object(&context, path, true)
    })?;
    for ((relative, reason), object) in whole.iter().zip(objects) {
        match into_prepared(root, object) {
            Ok(prepared) => {
                if *reason == "new" {
                    built.new_ids.insert(prepared.object_id.clone());
                }
                built.built_files.push(format!("{reason} {relative}"));
                built.objects.push(prepared);
            }
            Err(failure) => built.failures.push(failure),
        }
    }
    built.built_files.sort();
    built.compiled_files.sort();
    if let Some(source) = source {
        source.require_original_unchanged()?;
    }
    Ok(built)
}

fn relative_of(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn at(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

/// `(uuid, kind, row text)` of an object's descriptor compiled from the tree.
fn compile_descriptor_row(
    context: &EmptyStageContext,
    path: &Path,
) -> Result<(String, String, Vec<u8>)> {
    let _listing = source_listing::install(context.listing.clone());
    let xml = context.descriptors.source.read_source(path)?;
    let properties = parse_simple_metadata_xml_properties(&xml)?;
    let plain = catch(|| compile_descriptor(&properties.kind, path, &xml, &context.descriptors))?;
    Ok((properties.uuid, properties.kind, plain))
}

/// The rows a base-free build made, as the stage's own prepared object; the
/// first failure of the object, in the words of the refusal, when it made none.
fn into_prepared(
    root: &Path,
    object: EmptyStageObject,
) -> std::result::Result<PreparedMetadataObjectStage, ObjectFailure> {
    if !object.failures.is_empty() {
        let reasons = object
            .failures
            .iter()
            .map(|failure| format!("{}: {}", failure.family, failure.error))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(ObjectFailure {
            xml: object.xml,
            error: anyhow!("{CANNOT_BUILD}{reasons}"),
        });
    }
    let fail = |xml: &Path, what: &str| ObjectFailure {
        xml: xml.to_path_buf(),
        error: anyhow!("{CANNOT_BUILD}{what}"),
    };
    let Some(properties) = object.properties.clone() else {
        return Err(fail(&object.xml, "the object's XML has no readable head"));
    };
    let mut descriptor = None;
    let mut body_rows = Vec::new();
    for row in &object.rows {
        if row.file_name == object.uuid {
            descriptor = Some(row);
        } else {
            body_rows.push(PreparedMetadataBodyStage {
                body_id: row.file_name.clone(),
                path: at(root, &row.source),
                blob_sha256: hex_sha256(&row.blob),
                blob: row.blob.clone(),
            });
        }
    }
    let Some(descriptor) = descriptor else {
        return Err(fail(&object.xml, "no descriptor row was produced"));
    };
    Ok(PreparedMetadataObjectStage {
        object_id: object.uuid.clone(),
        kind: object.kind.clone(),
        xml: object.xml.clone(),
        properties,
        metadata_plain_bytes: descriptor.plain.as_ref().map_or(0, Vec::len),
        metadata_blob_sha256: hex_sha256(&descriptor.blob),
        metadata_blob: descriptor.blob.clone(),
        body_rows,
    })
}

// ---------------------------------------------------------------------------
// The always-used flag of a constant.

/// The stored constant record carries, as its 12th slot, the flag that keeps
/// the constant loaded in every session; no exported property carries it
/// (`metadata_model/simple.rs::constant`). A record compiled from the tree
/// writes `0`; the target's own value is kept.
fn graft_constant_flag(compiled: &[u8], stored: &[u8]) -> Result<Vec<u8>> {
    const SLOT: usize = 11;
    let mut compiled_root = parse_row(compiled)?;
    let stored_root = parse_row(stored)?;
    let stored_flag = constant_record(&stored_root)
        .and_then(|record| record.get(SLOT))
        .cloned()
        .ok_or_else(|| anyhow!("the stored constant has no record with a flag"))?;
    let record = constant_record_mut(&mut compiled_root)
        .ok_or_else(|| anyhow!("the compiled constant has no record with a flag"))?;
    let slot = record
        .get_mut(SLOT)
        .ok_or_else(|| anyhow!("the compiled constant record is short"))?;
    *slot = stored_flag;
    Ok(serialize_row(&compiled_root))
}

/// The list that starts with the constant's record marker `16`, searched
/// breadth first from the row.
fn constant_record(root: &Brace) -> Option<&[Brace]> {
    let mut queue = vec![root];
    while let Some(node) = queue.pop() {
        if let Some(items) = node.as_list() {
            if items.first().and_then(Brace::as_atom) == Some("16") && items.len() > 12 {
                return Some(items);
            }
            queue.extend(items.iter().rev());
        }
    }
    None
}

fn constant_record_mut(root: &mut Brace) -> Option<&mut Vec<Brace>> {
    let path = constant_record_path(root, &mut Vec::new())?;
    let mut node = root;
    for index in path {
        node = node.as_list_mut()?.get_mut(index)?;
    }
    node.as_list_mut()
}

fn constant_record_path(node: &Brace, path: &mut Vec<usize>) -> Option<Vec<usize>> {
    let items = node.as_list()?;
    if items.first().and_then(Brace::as_atom) == Some("16") && items.len() > 12 {
        return Some(path.clone());
    }
    for (index, child) in items.iter().enumerate() {
        path.push(index);
        if let Some(found) = constant_record_path(child, path) {
            return Some(found);
        }
        path.pop();
    }
    None
}

// ---------------------------------------------------------------------------
// What leaves the configuration.

/// The names of the `versions` row, in its order (the header pair aside).
pub(super) fn versions_names(versions_blob: &[u8]) -> Result<Vec<String>> {
    let plain = inflate_raw(versions_blob).context("failed to inflate the versions row")?;
    let text = String::from_utf8_lossy(&plain);
    let mut names = Vec::new();
    let mut rest = text.as_ref();
    while let Some(open) = rest.find('"') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('"') else {
            break;
        };
        if close > 0 {
            names.push(after[..close].to_string());
        }
        rest = &after[close + 1..];
    }
    Ok(names)
}

/// The Config rows of the objects with these ids: every name of the target's
/// `versions` that is an id or `<id>.<suffix>`.
pub(super) fn rows_of_objects(versions: &[String], ids: &[String]) -> Vec<String> {
    let ids = ids.iter().map(String::as_str).collect::<HashSet<_>>();
    versions
        .iter()
        .filter(|name| {
            let owner = name
                .split_once('.')
                .map_or(name.as_str(), |(owner, _)| owner);
            ids.contains(owner)
        })
        .cloned()
        .collect()
}

/// The rows an online update of the target left pending: the platform's own
/// import lists them in `deleted` and its apply removes them.
pub(super) fn dynamic_update_rows(sql: &SqlExec, database: &str) -> Result<Vec<String>> {
    let query = format!(
        "SET NOCOUNT ON; USE {db}; SELECT FileName AS name FROM dbo.Config \
         WHERE PartNo = 0 AND (FileName = N'DynamicallyUpdated' OR FileName LIKE N'%[_]dynupdate[_]%') \
         ORDER BY FileName FOR JSON PATH;",
        db = quote_ident(database)
    );
    let Some(json) = query_json(sql, &query, "dynamic update rows")? else {
        return Ok(Vec::new());
    };
    #[derive(serde::Deserialize)]
    struct Name {
        name: String,
    }
    let names: Vec<Name> = serde_json::from_str(&json)
        .context("failed to parse the names of the dynamic update rows")?;
    let mut names = names.into_iter().map(|row| row.name).collect::<Vec<_>>();
    names.sort_by_key(|name| name.to_lowercase());
    Ok(names)
}

/// The `deleted` row of the platform's import: a BOM, the number of names,
/// then each name in quotes followed by `0` (the flag of a Config row), raw
/// deflate.
pub(super) fn deleted_row(names: &[String]) -> Result<Vec<u8>> {
    let mut text = format!("\u{feff}{}", names.len());
    for name in names {
        if name.contains('"') {
            bail!("a row name with a quote cannot be listed in the deleted row: {name}");
        }
        text.push_str(&format!(",\"{name}\",0"));
    }
    deflate_raw(text.as_bytes())
}

/// The `versions` row without the entries of `names`, its count lowered.
pub(super) fn drop_versions_entries(versions_blob: &[u8], names: &[String]) -> Result<Vec<u8>> {
    if names.is_empty() {
        return Ok(versions_blob.to_vec());
    }
    let plain = inflate_raw(versions_blob).context("failed to inflate the versions row")?;
    let mut text = String::from_utf8(plain).context("the versions row is not UTF-8")?;
    let mut removed = 0usize;
    for name in names.iter().collect::<BTreeSet<_>>() {
        let marker = format!(",\"{name}\",");
        let Some(start) = text.find(&marker) else {
            continue;
        };
        let generation = start + marker.len();
        let end = generation + 36;
        if text.get(generation..end).is_none() {
            bail!("the versions entry of {name} is cut short");
        }
        text.replace_range(start..end, "");
        removed += 1;
    }
    let header = text
        .find("{1,")
        .ok_or_else(|| anyhow!("the versions row has no header"))?;
    let count_start = header + "{1,".len();
    let count_end = text[count_start..]
        .find(',')
        .map(|offset| count_start + offset)
        .ok_or_else(|| anyhow!("the versions header has no count"))?;
    let count: usize = text[count_start..count_end]
        .parse()
        .context("the versions count is not a number")?;
    text.replace_range(
        count_start..count_end,
        &count.saturating_sub(removed).to_string(),
    );
    deflate_raw(text.as_bytes())
}

/// The rows of the plan that leave the configuration, and the `deleted` names.
pub(super) struct Removal {
    /// Rows of removed objects (their `versions` entries go too).
    pub object_rows: Vec<String>,
    /// Everything the `deleted` row lists: those rows and the pending online
    /// update's.
    pub deleted: Vec<String>,
}

pub(super) fn removal(versions: &[String], plan: &Plan, dynamic: Vec<String>) -> Removal {
    let object_rows = rows_of_objects(versions, &plan.removed);
    let mut deleted = object_rows.clone();
    for name in dynamic {
        if !deleted.contains(&name) {
            deleted.push(name);
        }
    }
    Removal {
        object_rows,
        deleted,
    }
}

/// A short report of what the stage built and removed, for the import's JSON.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct StageOverrides {
    /// Objects built whole from the tree (new to the target, or not staged by
    /// a patch stage).
    pub built_objects: usize,
    /// Of them, the ones the target holds no row of.
    pub new_objects: usize,
    /// Descriptors compiled from the tree in place of the patched rows.
    pub compiled_descriptors: usize,
    /// Rows of removed objects.
    pub removed_rows: usize,
    /// Names in the `deleted` row.
    pub deleted_names: usize,
    /// The tree files of the built objects (`new`, `changed`, `rebuilt` and
    /// the path), at most the first 60.
    pub built_files: Vec<String>,
    /// The tree files whose descriptor was compiled from the tree.
    pub compiled_files: Vec<String>,
    /// Files of the tree the target's own export does not reproduce (#395).
    pub differing_files: usize,
    /// Objects the stage did not prepare, because no file of theirs differs, and
    /// rows of the prepared ones that stay the target's.
    pub objects_left_out: usize,
    pub rows_left_out: usize,
    /// Seconds the comparison of the tree with the target's export took.
    pub compare_seconds: f64,
    /// Why every row is staged, when the comparison could not decide (#395).
    pub all_rows_because: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn versions_blob(entries: &[&str]) -> Vec<u8> {
        let mut text = format!(
            "\u{feff}{{1,{},\"\",aaaaaaaa-0000-4000-8000-000000000000",
            entries.len()
        );
        for (index, name) in entries.iter().enumerate() {
            text.push_str(&format!(",\"{name}\",bbbbbbbb-0000-4000-8000-{index:012}"));
        }
        text.push('}');
        deflate_raw(text.as_bytes()).unwrap()
    }

    fn text_of(blob: &[u8]) -> String {
        String::from_utf8(inflate_raw(blob).unwrap()).unwrap()
    }

    #[test]
    fn the_deleted_row_is_the_count_and_the_quoted_names() {
        let names = vec![
            "a".to_string(),
            "a.0".to_string(),
            "DynamicallyUpdated".to_string(),
        ];
        let blob = deleted_row(&names).unwrap();
        assert_eq!(
            text_of(&blob),
            "\u{feff}3,\"a\",0,\"a.0\",0,\"DynamicallyUpdated\",0"
        );
        assert!(deleted_row(&["a\"b".to_string()]).is_err());
    }

    #[test]
    fn the_rows_of_an_object_are_its_id_and_its_numbered_rows() {
        let versions = ["root", "obj", "obj.0", "obj.1", "object", "other.0"]
            .map(str::to_string)
            .to_vec();
        assert_eq!(
            rows_of_objects(&versions, &["obj".to_string()]),
            ["obj", "obj.0", "obj.1"]
        );
    }

    #[test]
    fn versions_entries_go_and_the_count_follows() {
        let blob = versions_blob(&["a", "b", "b.0", "c"]);
        let shorter = drop_versions_entries(&blob, &["b".to_string(), "b.0".to_string()]).unwrap();
        let text = text_of(&shorter);
        assert!(text.starts_with("\u{feff}{1,2,\"\","), "{text}");
        assert!(text.contains("\"a\","), "{text}");
        assert!(text.contains("\"c\","), "{text}");
        assert!(!text.contains("\"b\""), "{text}");
        assert!(!text.contains("\"b.0\""), "{text}");
        assert_eq!(versions_names(&shorter).unwrap(), ["a", "c"]);
        // Nothing to drop: the row is returned as it was.
        assert_eq!(drop_versions_entries(&blob, &[]).unwrap(), blob);
        // A name the row lacks changes nothing.
        let same = drop_versions_entries(&blob, &["zzz".to_string()]).unwrap();
        assert_eq!(text_of(&same), text_of(&blob));
    }

    #[test]
    fn removal_lists_the_objects_rows_then_the_pending_updates() {
        let versions = ["root", "obj", "obj.0", "keep"]
            .map(str::to_string)
            .to_vec();
        let plan = Plan {
            removed: vec!["obj".to_string()],
            ..Plan::default()
        };
        let removal = removal(
            &versions,
            &plan,
            vec![
                "DynamicallyUpdated".to_string(),
                "keep_dynupdate_x".to_string(),
            ],
        );
        assert_eq!(removal.object_rows, ["obj", "obj.0"]);
        assert_eq!(
            removal.deleted,
            ["obj", "obj.0", "DynamicallyUpdated", "keep_dynupdate_x"]
        );
    }

    #[test]
    fn a_compiled_constant_keeps_the_stored_always_used_flag() {
        let record =
            |flag: &str| format!("\u{feff}{{1,\r\n{{16,x,a,b,c,d,0,1,y,z,f,{flag},0,k,l,m,n}},0}}");
        let grafted = graft_constant_flag(record("0").as_bytes(), record("1").as_bytes()).unwrap();
        let text = String::from_utf8(grafted).unwrap();
        assert!(text.contains("f,1,0,k"), "{text}");
        // A row without a record is an error, not a silent pass.
        assert!(graft_constant_flag(b"{1,0}", record("1").as_bytes()).is_err());
    }

    const CATALOG_ID: &str = "a4d6ccbb-a666-4217-ad1a-174221faba2b";

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/native-evidence/8.3.27.2214/form-usual-group-options/seed-plain")
    }

    fn built(plan: &Plan, patched: &dyn Fn(&str) -> Option<Vec<u8>>) -> Built {
        let root = fixture();
        let sql = SqlExec::detached("a unit test reaches no database");
        build(
            &Tree {
                root: &root,
                version: Some("2.20"),
                database: "no_database",
            },
            &sql,
            plan,
            &[],
            patched,
        )
        .unwrap()
    }

    #[test]
    fn an_object_the_target_lacks_is_built_whole_with_its_bodies() {
        let plan = Plan {
            added: vec!["Catalogs/CorpusList.xml".to_string()],
            ..Plan::default()
        };
        let built = built(&plan, &|_| None);
        assert!(built.failures.is_empty(), "{:?}", built.failures.len());
        assert_eq!(built.objects.len(), 1);
        let object = &built.objects[0];
        assert_eq!(object.object_id, CATALOG_ID);
        assert_eq!(object.kind, "Catalog");
        assert!(!object.metadata_blob.is_empty());
        assert!(
            object
                .body_rows
                .iter()
                .any(|row| row.body_id == format!("{CATALOG_ID}.0")),
            "the object module row: {:?}",
            object
                .body_rows
                .iter()
                .map(|row| &row.body_id)
                .collect::<Vec<_>>()
        );
        assert!(built.new_ids.contains(CATALOG_ID));
        assert_eq!(built.built_files, ["new Catalogs/CorpusList.xml"]);
        assert!(built.descriptors.is_empty());
    }

    #[test]
    fn a_descriptor_is_compiled_only_when_it_is_not_the_row_the_stage_would_leave() {
        let plan = Plan {
            changed: vec![(
                CATALOG_ID.to_string(),
                "Catalogs/CorpusList.xml".to_string(),
            )],
            ..Plan::default()
        };
        // The stage's row is not what the tree says: the compiler's row replaces it.
        let stale = deflate_raw(b"\xef\xbb\xbf{1,0}").unwrap();
        let first = built(&plan, &|_| Some(stale.clone()));
        assert!(first.objects.is_empty());
        let row = first
            .descriptors
            .get(CATALOG_ID)
            .expect("the descriptor differs from the stale row");
        assert_eq!(first.compiled_files, ["Catalogs/CorpusList.xml"]);
        // The stage's row is what the compiler writes (a change of the header
        // only, carried by the patch): nothing is replaced.
        let same = built(&plan, &|_| Some(row.blob.clone()));
        assert!(same.descriptors.is_empty());
        assert!(same.compiled_files.is_empty());
        assert!(same.objects.is_empty());
    }

    #[test]
    fn an_object_no_writer_can_build_is_a_failure_in_the_words_of_the_refusal() {
        let plan = Plan {
            added: vec!["Catalogs/NoSuchObject.xml".to_string()],
            ..Plan::default()
        };
        let built = built(&plan, &|_| None);
        assert!(built.objects.is_empty());
        assert_eq!(built.failures.len(), 1);
        let text = format!("{:#}", built.failures[0].error);
        assert!(text.starts_with(CANNOT_BUILD), "{text}");
    }

    #[test]
    fn a_pending_online_update_publishes_its_versions_row_under_the_plain_name() {
        use crate::mssql_dump::dynamic_generation_aliases;
        let generation = "06cb0442-0c47-4fad-986a-f08f28287c1b";
        let marker = format!("\u{feff}{{1,1,{generation}}}");
        let alias = format!("versions_dynupdate_{generation}");
        let names = ["versions", alias.as_str(), "a", "a.0"];
        let aliases = dynamic_generation_aliases(Some(marker.as_bytes()), names).unwrap();
        assert_eq!(aliases.get("versions"), Some(&alias));
        assert_eq!(aliases.len(), 1);
        // No marker, no aliases; a marker nobody can read is an error.
        assert!(dynamic_generation_aliases(None, names).unwrap().is_empty());
        assert!(dynamic_generation_aliases(Some(b"{1,2,x}"), names).is_err());
    }

    #[test]
    fn a_predefined_difference_is_built_and_other_failures_are_not() {
        let differ = anyhow!("{PREDEFINED_ITEMS_DIFFER}added: x; removed: ")
            .context("failed to build the object");
        assert!(is_buildable(&differ));
        assert!(!is_buildable(&anyhow!("Config row not found: x")));
        assert!(!is_buildable(&anyhow!(
            "failed to resolve ExchangePlanContent Catalog.Old"
        )));
    }

    #[test]
    fn the_plan_says_which_tree_files_the_patch_loop_must_leave() {
        let plan = Plan {
            added: vec!["Catalogs/New.xml".to_string()],
            ..Plan::default()
        };
        assert!(plan.is_added("catalogs/new.xml"));
        assert!(!plan.is_added("Catalogs/Old.xml"));
        assert!(!plan.is_empty());
        assert!(Plan::default().is_empty());
    }
}
