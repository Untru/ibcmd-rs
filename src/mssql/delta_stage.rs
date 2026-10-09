//! #395: a patch stage against a database stages the rows that change, and no others.
//!
//! Until now a patch stage compiled every object of the tree and staged all its rows
//! (9 521 for the БСП, 116 717 for ERP УХ), most of them the very rows the target already
//! holds, written again by another deflate stream and, for forms and modules, by another
//! writer. The platform's own partial import stages what changed and nothing else; the
//! rows that stay are the target's own, to the byte.
//!
//! What changed is decided by the target's own export. The rows the stage has read are
//! exported with the model exactly as the guard exports a staged state (nothing staged),
//! and every file is compared with the tree (`stage_guard::compare_tree_with_target`). A
//! file the export reproduces is a file the tree has not changed; the rows it comes from
//! stay the target's and are not staged. An object is left out entirely when no file of
//! its own -- its metadata file and the files of its folder, but not the folders of the
//! objects it owns -- differs, so its descriptor is not compiled either; of an object that
//! is prepared, a row is staged when its own source differs: the descriptor when the
//! metadata file does, a body when a file of the `Ext` folder it comes from does.
//!
//! What this leaves to the guard: the export of the state the stage leaves (the target's
//! rows with the few staged ones) is compared with the tree as before, so a row left out
//! that should have gone in refuses the import instead of being lost.
//!
//! Left in whatever the comparison says:
//! * the rows an online update the target has pending publishes under another name
//!   (`BASE_ROW_ALIASES`): the apply replaces them with the plain rows, so they are staged
//!   from the published ones as before;
//! * the descriptors the override compiles (`override_stage`) and the objects it builds.
//!
//! When a file that differs belongs to no object the stage prepares, nothing is left out
//! (the reason is in the report) and the stage stages everything, as it did.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::Path;

use anyhow::Result;
use rayon::prelude::*;

use super::override_stage::Plan;
use super::stage_guard::{Difference, FileDifference};
use super::{PreparedCommonModuleObjectStage, PreparedMetadataObjectStage};
use crate::cli::MssqlStageSourceObjectsArgs;
use crate::module_blob::{hex_sha256, inflate_raw};
use crate::source::SourceManifest;
use crate::sql::SqlExec;

/// Environment switch: `1` stages every row of the tree, as a patch stage did before #395.
pub(super) const ALL_ROWS_ENV: &str = "IBCMD_RS_STAGE_ALL_ROWS";

/// What a row of the target's pending online update stands for.
pub(super) enum Pending {
    /// Not a row that update replaces (or it cannot be told).
    No,
    /// The update publishes the very bytes the plain row holds.
    Same,
    /// The update publishes these bytes in place of the plain row's.
    Replaces(Vec<u8>),
}

/// What a prepared row becomes.
enum Fate {
    /// The target's row stays; nothing is staged.
    Stays,
    /// The prepared row is staged.
    Staged,
    /// These bytes are staged, as they are.
    Verbatim(Vec<u8>),
}

/// How much of an object's metadata file the search for its uuid reads.
const HEAD_BYTES: usize = 8192;

/// What the comparison of the tree with the target's export decided.
pub(super) enum Outcome {
    Active(Box<Delta>),
    /// Every row is staged: why.
    Off(String),
}

/// The files of the tree the target's export does not reproduce, and what follows from it.
pub(super) struct Delta {
    /// The tree's paths (relative, `/`, lower case) that differ, sorted.
    differing: Vec<String>,
    differing_set: HashSet<String>,
    /// Metadata files (same spelling) whose own files differ.
    dirty_units: HashSet<String>,
    /// Metadata files of the objects the override compiles.
    forced_units: HashSet<String>,
    /// Ids (lower case) of the descriptors the override compiles.
    forced_ids: HashSet<String>,
    /// Every metadata file the stage prepares, and the ones the guard said to stage whole.
    units: HashSet<String>,
    widened_units: HashSet<String>,
    /// Row names (lower case) an online update of the target replaces.
    aliased_rows: HashSet<String>,
    /// Metadata files of the objects with a row in `aliased_rows`.
    aliased_units: HashSet<String>,
    pub stats: DeltaStats,
}

/// What the report says of it.
#[derive(Debug, Clone, Default)]
pub(super) struct DeltaStats {
    /// Files of the tree the target's export compared with, and of them the ones it reproduces.
    pub compared: usize,
    pub identical: usize,
    /// Files of the tree that differ.
    pub differing: usize,
    pub seconds: f64,
    /// Objects the stage does not prepare, and rows it leaves out of the ones it prepares.
    pub objects_left_out: usize,
    pub rows_left_out: usize,
}

fn lower(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

fn relative(root: &Path, path: &Path) -> String {
    lower(&path.strip_prefix(root).unwrap_or(path).to_string_lossy())
}

/// The row names' leading id: `<uuid>` of `<uuid>.0`.
fn row_id(name: &str) -> &str {
    name.split('.').next().unwrap_or(name)
}

/// The uuid of the object a metadata file holds, read from its head:
/// `<MetaDataObject ...><Catalog uuid="...">`.
fn object_uuid(path: &Path) -> Option<String> {
    let mut head = vec![0u8; HEAD_BYTES];
    let mut file = fs::File::open(path).ok()?;
    let mut filled = 0;
    while filled < head.len() {
        let read = file.read(&mut head[filled..]).ok()?;
        if read == 0 {
            break;
        }
        filled += read;
    }
    head.truncate(filled);
    let text = String::from_utf8_lossy(&head);
    let start = text.find("<MetaDataObject")?;
    let after = &text[start..];
    let tag_end = after.find('>')?;
    let object = &after[tag_end + 1..];
    let open = object.find('<')?;
    let tag = &object[open + 1..];
    let close = tag.find('>')?;
    let tag = &tag[..close];
    let at = tag.find("uuid=\"")? + "uuid=\"".len();
    let value = &tag[at..];
    let end = value.find('"')?;
    Some(value[..end].to_lowercase())
}

/// Bound metadata identities use the existing complete metadata parser, not
/// the legacy 8 KiB text probe. Missing captured members and I/O are errors.
fn original_object_uuid(
    path: &Path,
    source: Option<&crate::module_blob::MetadataSourceContext>,
) -> Result<Option<String>> {
    let Some(source) = source.filter(|source| source.original_source().is_some()) else {
        return Ok(object_uuid(path));
    };
    let bytes = source.read_source(path)?;
    let properties = crate::module_blob::parse_simple_metadata_xml_properties(&bytes)?;
    Ok(Some(
        uuid::Uuid::parse_str(&properties.uuid)?
            .hyphenated()
            .to_string(),
    ))
}

fn source_aliased_units(
    root: &Path,
    xmls: &[&Path],
    aliased_ids: &HashSet<String>,
    source: Option<&crate::module_blob::MetadataSourceContext>,
) -> Result<HashSet<String>> {
    if let Some(source) = source {
        source.require_source_root(root)?;
    }
    Ok(if aliased_ids.is_empty() {
        HashSet::new()
    } else {
        let owned = xmls
            .iter()
            .map(|xml| (relative(root, xml), xml.to_path_buf()))
            .collect::<Vec<_>>();
        crate::parallel::install_io_bound(|| {
            owned
                .par_iter()
                .map(|(unit, xml)| {
                    let uuid = original_object_uuid(xml, source)?;
                    Ok(uuid
                        .is_some_and(|uuid| aliased_ids.contains(&uuid))
                        .then(|| unit.clone()))
                })
                .collect::<Result<Vec<_>>>()
                .map(|units| units.into_iter().flatten().collect::<HashSet<_>>())
        })??
    })
}

/// Compares the tree with the target's export and decides what the stage leaves out.
///
/// `xmls` are the metadata files the stage would prepare (the patch loop's, after the
/// override took the objects it builds); `aliases` are the rows an online update of the
/// target publishes under another name.

pub(super) fn plan_with_source(
    args: &MssqlStageSourceObjectsArgs,
    sql: &SqlExec,
    manifest: &SourceManifest,
    xmls: &[&Path],
    overrides: &Plan,
    aliases: &BTreeMap<String, String>,
    source: Option<&crate::module_blob::MetadataSourceContext>,
) -> Result<Outcome> {
    if let Some(source) = source {
        source.require_original_unchanged()?;
    }
    if std::env::var(ALL_ROWS_ENV).is_ok_and(|value| value.trim() == "1") {
        return Ok(Outcome::Off(format!("{ALL_ROWS_ENV}=1")));
    }
    let comparison =
        super::stage_guard::compare_tree_with_target_with_source(args, sql, manifest, source)?;
    plan_from_comparison(args, xmls, overrides, aliases, source, comparison)
}

/// The actual delta derivation after the database/export boundary. Kept
/// separate so original-source controls can exercise the planner without SQL.
fn plan_from_comparison(
    args: &MssqlStageSourceObjectsArgs,
    xmls: &[&Path],
    overrides: &Plan,
    aliases: &BTreeMap<String, String>,
    source: Option<&crate::module_blob::MetadataSourceContext>,
    comparison: super::stage_guard::TargetComparison,
) -> Result<Outcome> {
    if let Some(source) = source {
        source.require_source_root(&args.source_root)?;
        source.require_original_unchanged()?;
    }
    let root = args.source_root.as_path();
    let mut units = xmls
        .iter()
        .map(|xml| relative(root, xml))
        .collect::<HashSet<_>>();
    let forced_units = overrides
        .changed
        .iter()
        .map(|(_, file)| lower(file))
        .collect::<HashSet<_>>();
    units.extend(overrides.added.iter().map(|file| lower(file)));
    units.extend(forced_units.iter().cloned());

    let mut differing = comparison
        .differences
        .iter()
        // A file the tree lacks belongs to what the plan removes or the guard refuses.
        .filter(|difference| difference.difference != Difference::OnlyInState)
        .map(|difference| lower(&difference.path))
        .collect::<Vec<_>>();
    differing.sort();
    differing.dedup();

    let mut dirty_units = HashSet::new();
    for path in &differing {
        // The nearest object that owns the file: itself, else the metadata file named as the
        // deepest folder above it; the configuration's own files are those of `Ext`.
        let mut owner = units.contains(path).then(|| path.clone());
        let mut folder = path.as_str();
        while owner.is_none() {
            let Some(cut) = folder.rfind('/') else { break };
            folder = &folder[..cut];
            let candidate = format!("{folder}.xml");
            if units.contains(&candidate) {
                owner = Some(candidate);
            }
        }
        if owner.is_none() && path.starts_with("ext/") && units.contains("configuration.xml") {
            owner = Some("configuration.xml".to_string());
        }
        match owner {
            Some(owner) => {
                dirty_units.insert(owner);
            }
            None => {
                return Ok(Outcome::Off(format!(
                    "no object of the stage owns the file {path}"
                )));
            }
        }
    }

    let aliased_rows = aliases
        .keys()
        .map(|name| name.to_lowercase())
        .collect::<HashSet<_>>();
    let aliased_ids = aliased_rows
        .iter()
        .map(|name| row_id(name).to_string())
        .collect::<HashSet<_>>();
    let aliased_units = source_aliased_units(root, xmls, &aliased_ids, source)?;
    if let Some(source) = source {
        source.require_original_unchanged()?;
    }

    let forced_ids = overrides
        .changed
        .iter()
        .map(|(id, _)| id.to_lowercase())
        .collect();
    let differing_set = differing.iter().cloned().collect();
    Ok(Outcome::Active(Box::new(Delta {
        stats: DeltaStats {
            compared: comparison.compared,
            identical: comparison.identical,
            differing: differing.len(),
            seconds: comparison.seconds,
            ..DeltaStats::default()
        },
        differing,
        differing_set,
        dirty_units,
        forced_units,
        forced_ids,
        units,
        widened_units: HashSet::new(),
        aliased_rows,
        aliased_units,
    })))
}

/// The nearest folder named `Ext` above a tree path (`a/b/ext/c/d.xml` -> `a/b/ext`).
fn ext_folder(path: &str) -> Option<&str> {
    let mut end = path.len();
    while let Some(cut) = path[..end].rfind('/') {
        end = cut;
        if path[..end].rsplit('/').next() == Some("ext") {
            return Some(&path[..end]);
        }
    }
    None
}

impl Delta {
    /// Whether the stage has to prepare the object whose metadata file this is.
    pub(super) fn prepares(&self, root: &Path, xml: &Path) -> bool {
        let unit = relative(root, xml);
        self.dirty_units.contains(&unit)
            || self.forced_units.contains(&unit)
            || self.aliased_units.contains(&unit)
            || self.widened_units.contains(&unit)
    }

    /// The metadata file of the object that owns a file of the tree: the file itself, else the
    /// one named as the deepest folder above it; the configuration owns the files of the root
    /// `Ext`.
    fn owner_of(&self, path: &str) -> Option<String> {
        if self.units.contains(path) {
            return Some(path.to_string());
        }
        let mut folder = path;
        while let Some(cut) = folder.rfind('/') {
            folder = &folder[..cut];
            let candidate = format!("{folder}.xml");
            if self.units.contains(&candidate) {
                return Some(candidate);
            }
        }
        (path.starts_with("ext/") && self.units.contains("configuration.xml"))
            .then(|| "configuration.xml".to_string())
    }

    /// The guard found files of the staged state that differ from the tree: the objects that
    /// own them are staged whole from the tree from now on. `false` when that adds nothing.
    pub(super) fn widen(&mut self, differences: &[FileDifference]) -> bool {
        let before = self.widened_units.len();
        for difference in differences {
            if let Some(owner) = self.owner_of(&lower(&difference.path)) {
                self.widened_units.insert(owner);
            }
        }
        self.widened_units.len() > before
    }

    /// Whether a file of the tree under `folder` differs.
    fn differs_under(&self, folder: &str) -> bool {
        let prefix = format!("{folder}/");
        let at = self
            .differing
            .partition_point(|path| path.as_str() < prefix.as_str());
        self.differing
            .get(at)
            .is_some_and(|path| path.starts_with(&prefix))
    }

    /// What becomes of the descriptor row of the object with this metadata file and id.
    fn descriptor_fate(
        &self,
        root: &Path,
        xml: &Path,
        id: &str,
        built: &HashSet<String>,
        pending: &dyn Fn(&str) -> Pending,
    ) -> Fate {
        let unit = relative(root, xml);
        let lower = id.to_lowercase();
        if self.differing_set.contains(&unit)
            || self.forced_units.contains(&unit)
            || self.forced_ids.contains(&lower)
            || built.contains(&lower)
        {
            return Fate::Staged;
        }
        self.pending_fate(&lower, id, pending)
    }

    /// What becomes of a body row read from this file: it stays the target's while nothing in
    /// the `Ext` folder it comes from differs. A row of no `Ext` folder, or of a file outside
    /// the tree, is staged.
    fn body_fate(
        &self,
        root: &Path,
        body_id: &str,
        path: &Path,
        pending: &dyn Fn(&str) -> Pending,
    ) -> Fate {
        self.body_fate_in_scope(root, body_id, path, None, pending)
    }

    /// A module-only object edit does not rebuild unchanged sibling bodies. All other
    /// routes retain the existing Ext grouping, including combined form/help assets.
    fn body_fate_in_scope(
        &self,
        root: &Path,
        body_id: &str,
        path: &Path,
        module_only_ext: Option<&str>,
        pending: &dyn Fn(&str) -> Pending,
    ) -> Fate {
        if !path.starts_with(root) {
            return Fate::Staged;
        }
        let file = relative(root, path);
        let Some(ext) = ext_folder(&file) else {
            return Fate::Staged;
        };
        if self.differing_set.contains(&file)
            || (module_only_ext != Some(ext) && self.differs_under(ext))
        {
            return Fate::Staged;
        }
        self.pending_fate(&body_id.to_lowercase(), body_id, pending)
    }

    /// Only two independent module files may differ, with an unchanged descriptor.
    /// An override, built/widened owner or any other asset change keeps the old route.
    fn module_only_ext(
        &self,
        root: &Path,
        object: &PreparedMetadataObjectStage,
        built: &HashSet<String>,
    ) -> Option<String> {
        if !object.xml.starts_with(root) {
            return None;
        }
        let unit = relative(root, &object.xml);
        let id = object.object_id.to_lowercase();
        if self.differing_set.contains(&unit)
            || self.forced_units.contains(&unit)
            || self.forced_ids.contains(&id)
            || built.contains(&id)
            || self.widened_units.contains(&unit)
            || self.aliased_rows.contains(&id)
        {
            return None;
        }
        let folder = unit.strip_suffix(".xml")?;
        let prefix = format!("{folder}/");
        let ext = format!("{folder}/ext");
        let object_module = format!("{ext}/objectmodule.bsl");
        let manager_module = format!("{ext}/managermodule.bsl");
        let mut found = false;
        for file in self
            .differing
            .iter()
            .filter(|file| file.starts_with(&prefix))
        {
            if file != &object_module && file != &manager_module {
                return None;
            }
            found = true;
        }
        found.then_some(ext)
    }

    /// A row the tree does not change: the target's own, unless an online update of the target
    /// is pending on it -- the apply then leaves the plain row in place of the published one, so
    /// the published bytes are staged (the platform's own apply publishes them the same way).
    fn pending_fate(&self, lower: &str, id: &str, pending: &dyn Fn(&str) -> Pending) -> Fate {
        if !self.aliased_rows.contains(lower) {
            return Fate::Stays;
        }
        match pending(id) {
            Pending::Same => Fate::Stays,
            Pending::Replaces(bytes) => Fate::Verbatim(bytes),
            Pending::No => Fate::Staged,
        }
    }

    /// Takes out of a prepared object the rows the target keeps; `false` when none is left.
    /// `built` are the ids (lower case) whose descriptors the override compiled.
    pub(super) fn trim_object(
        &mut self,
        root: &Path,
        object: &mut PreparedMetadataObjectStage,
        built: &HashSet<String>,
        pending: &dyn Fn(&str) -> Pending,
    ) -> bool {
        if self.widened_units.contains(&relative(root, &object.xml)) {
            return true;
        }
        let before = usize::from(!object.metadata_blob.is_empty()) + object.body_rows.len();
        let module_only_ext = self.module_only_ext(root, object, built);
        match self.descriptor_fate(root, &object.xml, &object.object_id, built, pending) {
            Fate::Stays => {
                object.metadata_blob = Vec::new();
                object.metadata_blob_sha256.clear();
                object.metadata_plain_bytes = 0;
            }
            Fate::Staged => {}
            Fate::Verbatim(bytes) => {
                object.metadata_plain_bytes = inflate_raw(&bytes).map_or(0, |plain| plain.len());
                object.metadata_blob_sha256 = hex_sha256(&bytes);
                object.metadata_blob = bytes;
            }
        }
        let bodies = std::mem::take(&mut object.body_rows);
        for mut body in bodies {
            match self.body_fate_in_scope(
                root,
                &body.body_id,
                &body.path,
                module_only_ext.as_deref(),
                pending,
            ) {
                Fate::Stays => {}
                Fate::Staged => object.body_rows.push(body),
                Fate::Verbatim(bytes) => {
                    body.blob_sha256 = hex_sha256(&bytes);
                    body.blob = bytes;
                    object.body_rows.push(body);
                }
            }
        }
        let left = usize::from(!object.metadata_blob.is_empty()) + object.body_rows.len();
        self.stats.rows_left_out += before - left;
        left > 0
    }

    /// As [`Delta::trim_object`], for a common module.
    pub(super) fn trim_module(
        &mut self,
        root: &Path,
        module: &mut PreparedCommonModuleObjectStage,
        built: &HashSet<String>,
        pending: &dyn Fn(&str) -> Pending,
    ) -> bool {
        if self.widened_units.contains(&relative(root, &module.xml)) {
            return true;
        }
        let before = module.row_count();
        match self.descriptor_fate(root, &module.xml, &module.module_id, built, pending) {
            Fate::Stays => {
                module.metadata_blob = Vec::new();
                module.metadata_blob_sha256.clear();
                module.metadata_plain_bytes = 0;
            }
            Fate::Staged => {}
            Fate::Verbatim(bytes) => {
                module.metadata_plain_bytes = inflate_raw(&bytes).map_or(0, |plain| plain.len());
                module.metadata_blob_sha256 = hex_sha256(&bytes);
                module.metadata_blob = bytes;
            }
        }
        if module.has_module_body {
            match self.body_fate(root, &module.module_body_id, &module.text, pending) {
                Fate::Stays => module.has_module_body = false,
                Fate::Staged => {}
                Fate::Verbatim(bytes) => {
                    module.module_blob_sha256 = hex_sha256(&bytes);
                    module.module_blob = bytes;
                }
            }
        }
        let left = module.row_count();
        self.stats.rows_left_out += before - left;
        left > 0
    }

    /// Counts the objects the stage did not prepare.
    pub(super) fn count_left_out(&mut self, objects: usize) {
        self.stats.objects_left_out += objects;
    }

    /// Starts the counts of an attempt over.
    pub(super) fn reset_left_out(&mut self) {
        self.stats.objects_left_out = 0;
        self.stats.rows_left_out = 0;
    }
}

#[cfg(test)]
impl Delta {
    /// A comparison that found these files (lower case, `/`) different, these metadata
    /// files carried by the override and these rows replaced by a pending update.
    pub(super) fn for_test(differing: &[&str], forced: &[&str], aliased: &[&str]) -> Self {
        let mut differing = differing.iter().map(|p| p.to_string()).collect::<Vec<_>>();
        differing.sort();
        Self {
            differing_set: differing.iter().cloned().collect(),
            differing,
            dirty_units: HashSet::new(),
            forced_units: forced.iter().map(|p| p.to_string()).collect(),
            forced_ids: HashSet::new(),
            units: HashSet::new(),
            widened_units: HashSet::new(),
            aliased_rows: aliased.iter().map(|p| p.to_string()).collect(),
            aliased_units: HashSet::new(),
            stats: DeltaStats::default(),
        }
    }

    /// The metadata files (lower case, `/`) the stage prepares.
    pub(super) fn with_units(mut self, units: &[&str]) -> Self {
        self.units = units.iter().map(|unit| unit.to_string()).collect();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delta(differing: &[&str], forced: &[&str], aliased: &[&str]) -> Delta {
        Delta::for_test(differing, forced, aliased)
    }

    #[test]
    fn the_ext_folder_of_a_file_is_the_nearest_one() {
        assert_eq!(
            ext_folder("catalogs/x/ext/objectmodule.bsl"),
            Some("catalogs/x/ext")
        );
        assert_eq!(
            ext_folder("catalogs/x/commands/c/ext/commandmodule.bsl"),
            Some("catalogs/x/commands/c/ext")
        );
        assert_eq!(
            ext_folder("catalogs/x/forms/f/ext/form/module.bsl"),
            Some("catalogs/x/forms/f/ext")
        );
        assert_eq!(ext_folder("ext/homepageworkarea.xml"), Some("ext"));
        assert_eq!(ext_folder("catalogs/x.xml"), None);
        assert_eq!(ext_folder("catalogs/extra/x.xml"), None);
    }

    fn no_update(_: &str) -> Pending {
        Pending::No
    }

    fn stays(fate: Fate) -> bool {
        matches!(fate, Fate::Stays)
    }

    fn module_object() -> PreparedMetadataObjectStage {
        let root = Path::new("R");
        PreparedMetadataObjectStage {
            object_id: "owner".into(),
            kind: "Catalog".into(),
            xml: root.join("Catalogs/X.xml"),
            properties: crate::module_blob::SimpleMetadataXmlProperties {
                kind: "Catalog".into(),
                uuid: "owner".into(),
                name: "X".into(),
                synonyms: Vec::new(),
                comment: String::new(),
            },
            metadata_plain_bytes: 1,
            metadata_blob: vec![99],
            metadata_blob_sha256: "descriptor".into(),
            body_rows: [
                "ObjectModule.bsl",
                "ManagerModule.bsl",
                "Help/ru.html",
                "RecordSetModule.bsl",
            ]
            .iter()
            .enumerate()
            .map(|(i, file)| super::super::PreparedMetadataBodyStage {
                body_id: format!("owner.{i}"),
                path: root.join(format!("Catalogs/X/Ext/{file}")),
                blob: vec![i as u8],
                blob_sha256: format!("body-{i}"),
            })
            .collect(),
        }
    }

    #[test]
    fn module_only_edit_keeps_unchanged_siblings_byte_exact() {
        for file in ["objectmodule.bsl", "managermodule.bsl"] {
            let mut delta = delta(&[&format!("catalogs/x/ext/{file}")], &[], &[]);
            let mut object = module_object();
            assert!(delta.trim_object(Path::new("R"), &mut object, &HashSet::new(), &no_update));
            assert!(object.metadata_blob.is_empty());
            assert_eq!(object.body_rows.len(), 1);
            assert_eq!(
                object.body_rows[0]
                    .path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_lowercase(),
                file
            );
            assert_eq!(delta.stats.rows_left_out, 4);
        }
    }

    #[test]
    fn module_only_edit_preserves_pending_sibling_preimages() {
        let mut delta = delta(
            &["catalogs/x/ext/objectmodule.bsl"],
            &[],
            &["owner.1", "owner.2", "owner.3"],
        );
        let mut object = module_object();
        let pending = |id: &str| match id {
            "owner.1" => Pending::Replaces(vec![44, 55]),
            "owner.2" => Pending::Same,
            _ => Pending::No,
        };
        assert!(delta.trim_object(Path::new("R"), &mut object, &HashSet::new(), &pending));
        assert_eq!(
            object
                .body_rows
                .iter()
                .map(|body| body.body_id.as_str())
                .collect::<Vec<_>>(),
            ["owner.0", "owner.1", "owner.3"]
        );
        assert_eq!(object.body_rows[1].blob, [44, 55]);
        assert_eq!(object.body_rows[1].blob_sha256, hex_sha256(&[44, 55]));
    }

    #[test]
    fn mixed_or_forced_module_edits_keep_ext_grouping() {
        for extra in [
            "catalogs/x.xml",
            "catalogs/x/ext/help/ru.html",
            "catalogs/x/ext/predefined.xml",
            "catalogs/x/forms/f/ext/form/module.bsl",
            "catalogs/x/templates/t/ext/template.txt",
        ] {
            let mut delta = delta(&["catalogs/x/ext/objectmodule.bsl", extra], &[], &[]);
            let mut object = module_object();
            assert!(delta.trim_object(Path::new("R"), &mut object, &HashSet::new(), &no_update));
            assert_eq!(object.body_rows.len(), 4, "{extra}");
        }
        for route in [
            "forced-unit",
            "forced-id",
            "built",
            "widened",
            "pending-descriptor",
        ] {
            let mut delta = delta(&["catalogs/x/ext/objectmodule.bsl"], &[], &[]);
            let mut built = HashSet::new();
            match route {
                "forced-unit" => {
                    delta.forced_units.insert("catalogs/x.xml".into());
                }
                "forced-id" => {
                    delta.forced_ids.insert("owner".into());
                }
                "built" => {
                    built.insert("owner".into());
                }
                "widened" => {
                    delta.widened_units.insert("catalogs/x.xml".into());
                }
                _ => {
                    delta.aliased_rows.insert("owner".into());
                }
            }
            let mut object = module_object();
            assert!(delta.trim_object(Path::new("R"), &mut object, &built, &no_update));
            assert_eq!(object.body_rows.len(), 4, "{route}");
        }
    }

    #[test]
    fn a_body_stays_the_targets_while_nothing_in_its_ext_folder_differs() {
        let root = Path::new("R");
        let delta = delta(
            &["catalogs/x.xml", "catalogs/y/ext/managermodule.bsl"],
            &[],
            &[],
        );
        // The descriptor of x differs; its module does not.
        assert!(stays(delta.body_fate(
            root,
            "id.0",
            &root.join("Catalogs/X/Ext/ObjectModule.bsl"),
            &no_update
        )));
        // y's manager module differs, and so does every body of its Ext folder.
        assert!(!stays(delta.body_fate(
            root,
            "id.1",
            &root.join("Catalogs/Y/Ext/ObjectModule.bsl"),
            &no_update
        )));
        assert!(!stays(delta.body_fate(
            root,
            "id.2",
            &root.join("Catalogs/Y/Ext/ManagerModule.bsl"),
            &no_update
        )));
        // A file with no Ext folder, or outside the tree, is staged.
        assert!(!stays(delta.body_fate(
            root,
            "id.3",
            &root.join("Catalogs/X.xml"),
            &no_update
        )));
        assert!(!stays(delta.body_fate(
            root,
            "id.4",
            Path::new("Elsewhere/Ext/A.bsl"),
            &no_update
        )));
    }

    #[test]
    fn a_descriptor_stays_the_targets_unless_its_file_differs_or_the_override_carries_it() {
        let root = Path::new("R");
        let delta = delta(&["catalogs/x.xml"], &["catalogs/z.xml"], &[]);
        let none = HashSet::new();
        let fate = |xml: &str, id: &str, built: &HashSet<String>| {
            delta.descriptor_fate(root, &root.join(xml), id, built, &no_update)
        };
        assert!(!stays(fate("Catalogs/X.xml", "1", &none)));
        assert!(stays(fate("Catalogs/Y.xml", "2", &none)));
        assert!(!stays(fate("Catalogs/Z.xml", "3", &none)));
        let built = ["4".to_string()].into_iter().collect();
        assert!(!stays(fate("Catalogs/Y.xml", "4", &built)));
    }

    #[test]
    fn a_row_a_pending_update_replaces_is_staged_as_the_update_publishes_it() {
        let root = Path::new("R");
        let delta = delta(&[], &[], &["aaaa.0", "bbbb"]);
        let update = |id: &str| match id.to_lowercase().as_str() {
            "aaaa.0" => Pending::Replaces(vec![1, 2, 3]),
            "bbbb" => Pending::Same,
            _ => Pending::No,
        };
        let path = root.join("Catalogs/X/Ext/ObjectModule.bsl");
        // Unchanged in the tree, and the update replaces it: the published bytes.
        assert!(matches!(
            delta.body_fate(root, "AAAA.0", &path, &update),
            Fate::Verbatim(bytes) if bytes == [1, 2, 3]
        ));
        // The update publishes what the plain row holds: nothing to stage.
        assert!(stays(delta.descriptor_fate(
            root,
            &root.join("Catalogs/X.xml"),
            "BBBB",
            &HashSet::new(),
            &update
        )));
        // An update row nobody can read is staged as prepared.
        let unreadable = |_: &str| Pending::No;
        assert!(matches!(
            delta.body_fate(root, "aaaa.0", &path, &unreadable),
            Fate::Staged
        ));
    }

    #[test]
    fn a_file_of_the_tree_belongs_to_the_nearest_object_and_the_guard_widens_it_once() {
        let mut delta = delta(&[], &[], &[]);
        delta.units = [
            "catalogs/x.xml",
            "catalogs/x/forms/f.xml",
            "configuration.xml",
        ]
        .iter()
        .map(|unit| unit.to_string())
        .collect();
        assert_eq!(
            delta
                .owner_of("catalogs/x/forms/f/ext/help/ru.html")
                .as_deref(),
            Some("catalogs/x/forms/f.xml")
        );
        assert_eq!(
            delta.owner_of("catalogs/x/ext/help/ru.html").as_deref(),
            Some("catalogs/x.xml")
        );
        assert_eq!(
            delta.owner_of("catalogs/x.xml").as_deref(),
            Some("catalogs/x.xml")
        );
        assert_eq!(
            delta.owner_of("ext/homepageworkarea.xml").as_deref(),
            Some("configuration.xml")
        );
        assert_eq!(delta.owner_of("catalogs/y/ext/a.bsl"), None);

        let difference = |path: &str| FileDifference {
            path: path.to_string(),
            difference: Difference::OnlyInTree,
        };
        // A file nobody owns widens nothing; one that an object owns widens that object, once.
        assert!(!delta.widen(&[difference("catalogs/y/ext/a.bsl")]));
        assert!(delta.widen(&[difference("Catalogs/X/Ext/Help/ru.html")]));
        assert!(!delta.widen(&[difference("Catalogs/X/Ext/Help/ru.html")]));
        assert!(delta.widened_units.contains("catalogs/x.xml"));
        assert!(delta.prepares(Path::new("R"), &Path::new("R").join("Catalogs/X.xml")));
    }

    #[test]
    fn the_uuid_of_an_object_is_read_from_the_head_of_its_file() {
        let dir = std::env::temp_dir().join(format!("ibcmd-delta-head-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("X.xml");
        fs::write(
            &file,
            "\u{feff}<?xml version=\"1.0\"?>\r\n<MetaDataObject xmlns=\"a\" version=\"2.20\">\r\n\t<Catalog uuid=\"ABCDEF01-0000-4000-8000-000000000000\">\r\n</Catalog></MetaDataObject>",
        )
        .unwrap();
        assert_eq!(
            object_uuid(&file).as_deref(),
            Some("abcdef01-0000-4000-8000-000000000000")
        );
        fs::write(&file, "<Other/>").unwrap();
        assert_eq!(object_uuid(&file), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_row_name_leads_with_its_id() {
        assert_eq!(row_id("abc.0"), "abc");
        assert_eq!(row_id("abc"), "abc");
        assert_eq!(row_id("abc_dynupdate_def.0"), "abc_dynupdate_def");
    }
}

#[cfg(test)]
#[path = "delta_stage_original_tests.rs"]
mod original_alias_tests;
