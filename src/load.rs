//! `cf load`: overlay the edits of an exported source tree onto the file it
//! was exported from. The base is exported again, the trees are compared
//! file by file, and each changed module maps through the export report
//! (storage key → output files) onto one overlay request against the original
//! container; everything else -- `Form.xml` included -- is refused, never
//! guessed.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use anyhow::{Context, Result};
use ibcmd_cf::{
    archive::{decode_archive_uniform, decode_packed_archive},
    overlay::{OverlayCodec, publish_overlay_new},
    payload::PayloadEncoding,
};
use ibcmd_core::{
    artifact::StorageProfileId,
    limits::ResourceLimits,
    storage::{MultipartIdentity, StorageEntry, StorageKey, StoragePatchTarget, StorageProvenance},
    version::XmlDialect,
};

pub(crate) mod compiled;
mod external;
pub(crate) mod index;
mod seed;
pub use seed::build_external;

pub use index::{archive_versions, write_tree_index};

use crate::{
    compiler::{
        CompileAxes, CompileRequest, SourcePayload, overlay::compile_overlay_with_retained_budget,
    },
    external::export::export_if_external,
    legacy_version::InfobaseConfigSourceVersion,
    module_blob::{
        encode_base64, inflate_raw, pack_form_body_blob_from_module_text,
        patch_versions_blob_bytes_allowing_additions,
    },
    mssql_dump::StorageImageSourceExportReport,
};

/// Files of two trees by `/`-separated relative path, sorted.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TreeDiff {
    pub changed: Vec<String>,
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

/// 1C never names a file or directory with a leading dot; such entries are
/// the editor's or the VCS's (`.git`, `.vscode`) and the export's own
/// `.complete` marker.
fn is_dot_entry(entry: &walkdir::DirEntry) -> bool {
    entry.depth() > 0 && entry.file_name().to_string_lossy().starts_with('.')
}

pub(crate) fn relative_files(root: &Path) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| !is_dot_entry(entry))
    {
        let entry = entry.with_context(|| format!("failed to walk {}", root.display()))?;
        if entry.file_type().is_file() {
            let rel = entry
                .path()
                .strip_prefix(root)
                .expect("walked path is under its root")
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(rel);
        }
    }
    Ok(out)
}

pub fn diff_trees(base: &Path, edited: &Path) -> Result<TreeDiff> {
    let (a, b) = (relative_files(base)?, relative_files(edited)?);
    let mut diff = TreeDiff::default();
    for rel in a.intersection(&b) {
        let (x, y) = (fs::read(base.join(rel))?, fs::read(edited.join(rel))?);
        if x != y {
            diff.changed.push(rel.clone());
        }
    }
    diff.added = b.difference(&a).cloned().collect();
    diff.removed = a.difference(&b).cloned().collect();
    Ok(diff)
}

/// One storage entry to rewrite. Paths are relative to the edited tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Edit {
    /// A module row (`<owner>.N` module container) from BSL text.
    Module { key: String, text_path: String },
    /// A managed form body row whose module is replaced; the rest of the body
    /// is kept from the base.
    FormBody { key: String, module: String },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Plan {
    pub edits: Vec<Edit>,
    pub unsupported: Vec<String>,
}

/// The storage profile the legacy source compiler (module text, form bodies)
/// is defined for; `cf overlay` uses the same default.
const OVERLAY_PROFILE: &str = "storage:mssql-config-configsave";

const FORM_MODULE_SUFFIX: &str = "/Ext/Form/Module.bsl";
const FORM_XML_SUFFIX: &str = "/Ext/Form.xml";
const CONFIG_DUMP_INFO: &str = "ConfigDumpInfo.xml";

/// A module's text lives under an object's `Ext/` directory, or under the
/// root `Ext/` for the configuration's own modules.
fn under_ext(path: &str) -> bool {
    path.starts_with("Ext/") || path.contains("/Ext/")
}

/// Maps tree edits onto storage entries; anything v1 cannot rewrite from the
/// base is listed in `unsupported`. `Form.xml` is among those: the form body
/// packer patches only some properties and would drop the rest of an edit
/// without an error.
pub fn classify(diff: &TreeDiff, keys: &BTreeMap<String, String>) -> Plan {
    let mut plan = Plan::default();
    for path in &diff.changed {
        // Derived from versions/configinfo, which a load refreshes: a file
        // loaded from this tree exports a newer one, and that is no edit.
        if path == CONFIG_DUMP_INFO {
            continue;
        }
        if let Some(stem) = path.strip_suffix(FORM_MODULE_SUFFIX) {
            let form_xml = format!("{stem}{FORM_XML_SUFFIX}");
            match keys.get(path).or_else(|| keys.get(&form_xml)) {
                Some(key) => plan.edits.push(Edit::FormBody {
                    key: key.clone(),
                    module: path.clone(),
                }),
                None => plan.unsupported.push(path.clone()),
            }
        } else if path.ends_with(".bsl") && under_ext(path) {
            match keys.get(path) {
                Some(key) => plan.edits.push(Edit::Module {
                    key: key.clone(),
                    text_path: path.clone(),
                }),
                None => plan.unsupported.push(path.clone()),
            }
        } else {
            plan.unsupported.push(path.clone());
        }
    }
    // A closed module (`X.bin`) replaced by its source (`X.bsl`) — the
    // decompiler's round trip — is a module edit of the entry that owned X.bin.
    let mut paired = BTreeSet::new();
    for path in &diff.added {
        // A module written for a form that had none: the form body holds the
        // module text, empty until now.
        if let Some(stem) = path.strip_suffix(FORM_MODULE_SUFFIX)
            && let Some(key) = keys.get(&format!("{stem}{FORM_XML_SUFFIX}"))
        {
            plan.edits.push(Edit::FormBody {
                key: key.clone(),
                module: path.clone(),
            });
            continue;
        }
        // Only a module's `.bin` (`…Module.bin`); a binary template is not one.
        let bin = path
            .strip_suffix(".bsl")
            .filter(|stem| under_ext(path) && stem.ends_with("Module"))
            .map(|stem| format!("{stem}.bin"));
        match bin
            .filter(|bin| diff.removed.contains(bin))
            .and_then(|bin| keys.get(&bin).map(|key| (bin.clone(), key.clone())))
        {
            Some((bin, key)) => {
                paired.insert(bin);
                plan.edits.push(Edit::Module {
                    key,
                    text_path: path.clone(),
                });
            }
            None => plan.unsupported.push(path.clone()),
        }
    }
    for path in &diff.removed {
        if !paired.contains(path) {
            plan.unsupported.push(path.clone());
        }
    }
    plan
}

/// Module text line ends as the platform stores them (CRLF); an editor may
/// have saved LF.
fn crlf(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + text.len() / 32);
    for (i, &byte) in text.iter().enumerate() {
        if byte == LF && (i == 0 || text[i - 1] != CR) {
            out.push(CR);
        }
        out.push(byte);
    }
    out
}

const CR: u8 = 0x0d;
const LF: u8 = 0x0a;

const UTF8_BOM: &[u8] = &[0xef, 0xbb, 0xbf];

/// The text of a module file of the edited tree, as the overlay stores it:
/// UTF-8 (an optional BOM kept) with CRLF line ends. Any other encoding is
/// refused -- the platform would read it as UTF-8 and garble it.
pub(crate) fn module_text(edited: &Path, rel: &str) -> std::result::Result<Vec<u8>, String> {
    let bytes =
        fs::read(edited.join(rel)).map_err(|error| format!("failed to read {rel}: {error}"))?;
    module_text_from_bytes(&bytes, rel)
}

/// Apply the same storage-text policy to bytes already held by a source guard.
/// The source path is only an error label; this function never reopens it.
pub(crate) fn module_text_from_bytes(
    bytes: &[u8],
    rel: &str,
) -> std::result::Result<Vec<u8>, String> {
    let body = bytes.strip_prefix(UTF8_BOM).unwrap_or(bytes);
    if std::str::from_utf8(body).is_err() {
        return Err(format!(
            "{rel}: module text must be UTF-8 (save the file as UTF-8)"
        ));
    }
    Ok(crlf(bytes))
}

/// How many refused paths a message names before it only counts the rest.
const LISTED_PATHS: usize = 20;

/// Refusals `cf load` reports with a stable code.
#[derive(Debug)]
pub enum LoadError {
    NoChanges,
    Unsupported(Vec<String>),
    Failed(anyhow::Error),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoChanges => write!(f, "the tree has no changes against the base"),
            Self::Unsupported(paths) => {
                write!(
                    f,
                    "these changes cannot be loaded (no object of the tree owns them, or \
                     the object cannot be compiled): {}",
                    paths
                        .iter()
                        .take(LISTED_PATHS)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                )?;
                if paths.len() > LISTED_PATHS {
                    write!(f, " and {} more", paths.len() - LISTED_PATHS)?;
                }
                Ok(())
            }
            Self::Failed(error) => write!(f, "{error:#}"),
        }
    }
}

impl LoadError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NoChanges => "no_changes",
            Self::Unsupported(_) => "unsupported_changes",
            Self::Failed(_) => "load_failed",
        }
    }
}

impl From<anyhow::Error> for LoadError {
    fn from(error: anyhow::Error) -> Self {
        Self::Failed(error)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LoadReport {
    /// How the base's tree was known: `index` (the tree's own index, no
    /// re-export) or `full` (the base exported again).
    pub base_export: &'static str,
    pub modules: usize,
    pub forms: usize,
    /// The tree's files whose text was loaded, sorted.
    pub applied: Vec<String>,
    /// The objects compiled from the tree (edits a module overlay cannot
    /// carry), as source prefixes: `Catalogs/Товары`, `Configuration.xml`.
    pub compiled_objects: Vec<String>,
    /// The objects the tree lost whole, removed from the file.
    pub removed_objects: Vec<String>,
    /// Loaded, but worth a look: an edited object the base changed since the
    /// tree was exported (its configVersion in the tree's ConfigDumpInfo.xml
    /// is not the base's) -- the tree's text replaced the base's.
    pub warnings: Vec<String>,
}

struct LoadCodec<'a> {
    edited: &'a Path,
    /// Ordinary module key -> its edited text source. Existing native info
    /// and element headers belong to the module and survive a text-only edit.
    modules: BTreeMap<String, String>,
    /// Form body key → the edited form module.
    forms: BTreeMap<String, String>,
    /// The base has a `versions` entry (configurations, external objects);
    /// an extension (.cfe) has none and keeps none.
    base_has_versions: bool,
}

impl OverlayCodec for LoadCodec<'_> {
    fn resolve_needs_base(
        &mut self,
        target: &StoragePatchTarget,
        _required: &StorageKey,
        base: &StorageEntry,
    ) -> Result<Vec<u8>, String> {
        let key = target.key().as_str();
        if let Some(path) = self.modules.get(key) {
            return crate::module_blob::pack_module_blob_bytes(
                &module_text(self.edited, path)?,
                Some(base.packed_payload()),
                None,
            )
            .map(|packed| packed.blob)
            .map_err(|error| format!("{error:#}"));
        }
        let module = self
            .forms
            .get(key)
            .ok_or_else(|| format!("no form edit was retained for `{key}`"))?;
        pack_form_body_blob_from_module_text(
            base.packed_payload(),
            &module_text(self.edited, module)?,
        )
        .map(|packed| packed.blob)
        .map_err(|error| format!("{error:#}"))
    }

    fn update_versions(
        &mut self,
        base: &StorageEntry,
        changed_keys: &[String],
    ) -> Result<Vec<u8>, String> {
        patch_versions_blob_bytes_allowing_additions(base.packed_payload(), changed_keys, true)
            .map(|patched| patched.blob)
            .map_err(|error| format!("{error:#}"))
    }

    fn requires_versions(&self) -> bool {
        self.base_has_versions
    }
}

/// Exports `base` the way `cf export` does (the external-object adapter
/// first), returning the report.
pub(crate) fn export_base(
    base: &Path,
    output_dir: &Path,
    source_version: InfobaseConfigSourceVersion,
) -> Result<StorageImageSourceExportReport> {
    let limits = ResourceLimits::for_input_bytes(fs::metadata(base)?.len());
    let profile = StorageProfileId::parse("storage:cf-cli").expect("static profile id is valid");
    let archive = decode_packed_archive(fs::File::open(base)?, limits, profile)
        .map_err(|error| anyhow::anyhow!("failed to read base `{}`: {error}", base.display()))?;
    if let Some(report) =
        export_if_external(&archive, output_dir, true, source_version).context("external base")?
    {
        return Ok(report);
    }
    crate::extension::export_packed_cf_archive_to_source(archive, output_dir, true, source_version)
}

/// Compare the exported representation against the edited source, without
/// changing source bytes or relaxing comparison of the produced file.
fn edited_source_matches_export(path: &str, edited: &[u8], exported: &[u8]) -> bool {
    edited == exported
        || path.ends_with(".bsl") && crate::mssql_dump::native_module_file_bytes(edited) == exported
        || path.ends_with(".xml")
            && ibcmd_xml::equal_ignoring_structural_line_endings(edited, exported)
}

/// The loaded file exported back: the files of every object the load
/// compiled (all of them for an external object) must be the tree's.
fn verify_loaded(
    edited: &Path,
    output: &Path,
    source_version: InfobaseConfigSourceVersion,
    diff: &TreeDiff,
    report: &LoadReport,
    external: bool,
) -> std::result::Result<BTreeMap<String, String>, LoadError> {
    let scratch = Scratch(std::env::temp_dir().join(format!(
        "ibcmd-load-verify-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    )));
    let exported = export_base(output, &scratch.0, source_version)
        .context("failed to export the loaded file")?;
    let in_scope = |path: &str| {
        path != CONFIG_DUMP_INFO
            && (external
                || diff
                    .changed
                    .iter()
                    .chain(&diff.added)
                    .any(|edited| edited == path)
                || report.compiled_objects.iter().any(|prefix| {
                    if prefix == "Configuration.xml" {
                        path == prefix || path.starts_with("Ext/")
                    } else {
                        path == format!("{prefix}.xml") || path.starts_with(&format!("{prefix}/"))
                    }
                }))
    };
    let (tree, back) = (relative_files(edited)?, relative_files(&scratch.0)?);
    let mut differ = Vec::new();
    for path in tree.union(&back).filter(|path| in_scope(path)) {
        let same = match (tree.contains(path), back.contains(path)) {
            (true, true) => {
                let (mine, theirs) = (
                    fs::read(edited.join(path)).context("tree file")?,
                    fs::read(scratch.0.join(path)).context("exported file")?,
                );
                edited_source_matches_export(path, &mine, &theirs)
            }
            _ => false,
        };
        if !same {
            differ.push(path.clone());
        }
    }
    if differ.is_empty() {
        // The new file's entries by output: the tree index's keys from now
        // on (an added file has one the old index cannot name).
        return Ok(keys_by_output(&exported));
    }
    Err(anyhow::anyhow!(
        "the loaded file does not export back to the edited tree -- the compiler does not write these faithfully yet, so nothing was written: {}",
        differ.iter().take(LISTED_PATHS).cloned().collect::<Vec<_>>().join(", ")
    )
    .into())
}

/// The base's scratch export, removed when dropped -- on a panic too (it
/// used to stay in %TEMP% whenever the load did not return).
struct Scratch(std::path::PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// `id` → `configVersion` of every `<Metadata>` line of a ConfigDumpInfo.xml
/// (empty when there is none: external objects keep no such file).
pub(crate) fn config_versions(path: &Path) -> BTreeMap<String, String> {
    let Ok(text) = fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    let attribute = |line: &str, name: &str| {
        let marker = format!(" {name}=\"");
        let start = line.find(&marker)? + marker.len();
        let end = start + line[start..].find('"')?;
        Some(line[start..end].to_owned())
    };
    text.lines()
        .filter(|line| line.trim_start().starts_with("<Metadata "))
        .filter_map(|line| Some((attribute(line, "id")?, attribute(line, "configVersion")?)))
        .collect()
}

/// The dialect a tree was exported in: the `version` of its root XML file
/// (Configuration.xml, or an external object's `<Name>.xml`).
fn tree_dialect(tree: &Path) -> Option<String> {
    let mut roots = fs::read_dir(tree)
        .ok()?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "xml"))
        .collect::<Vec<_>>();
    roots.sort();
    let root = roots
        .iter()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "Configuration.xml")
        })
        .or_else(|| {
            roots.iter().find(|path| {
                path.file_name()
                    .is_some_and(|name| name != "ConfigDumpInfo.xml")
            })
        })?;
    let head = fs::read(root).ok()?;
    let head = String::from_utf8_lossy(&head[..head.len().min(4096)]).into_owned();
    let at = head.find("<MetaDataObject ")?;
    let tag_end = at + head[at..].find('>')?;
    let tag = &head[at..tag_end];
    let value = tag.find(" version=\"")? + " version=\"".len();
    let end = value + tag[value..].find('"')?;
    Some(tag[value..end].to_owned())
}

/// Overlays the module and form edits of `edited` (an export of `base`,
/// changed) onto `base` and writes `output` (never overwritten).
pub fn load_onto_base(
    edited: &Path,
    base: &Path,
    output: &Path,
    source_version: InfobaseConfigSourceVersion,
) -> std::result::Result<LoadReport, LoadError> {
    if index::update_interrupted(edited) {
        return Err(LoadError::Failed(anyhow::anyhow!(
            "an update of `{}` was interrupted: the tree mixes two files; export it again with \
             --overwrite",
            edited.display()
        )));
    }
    if let Some(dialect) = tree_dialect(edited)
        && dialect != source_version.as_str()
    {
        // The platform whose XML the dialect is: 8.3.x writes 2.20, 8.5.x 2.21.
        let platform = |dialect: &str| if dialect == "2.21" { "8.5.1" } else { "8.3.27" };
        return Err(LoadError::Failed(anyhow::anyhow!(
            "`{}` holds XML {dialect} (platform {}); load it with --platform {} (this load reads \
             XML {})",
            edited.display(),
            platform(&dialect),
            platform(&dialect),
            source_version.as_str()
        )));
    }
    // A tree exported with `--index` from this very file needs no re-export:
    // its index holds the export's digests and storage keys.
    let base_sha256 = index::file_sha256(base)?;
    let tree_index = index::read_tree_index(edited)
        .filter(|found| found.base == base_sha256 && found.dialect == source_version.as_str());
    let (diff, keys, base_versions, base_export) = if let Some(found) = &tree_index {
        (
            index::diff_against_index(edited, found)?,
            found.keys(),
            BTreeMap::new(),
            "index",
        )
    } else {
        let (diff, keys, base_versions) = export_and_diff(base, edited, source_version)?;
        (diff, keys, base_versions, "full")
    };
    let plan = classify(&diff, &keys);
    let mut report = LoadReport {
        base_export,
        ..LoadReport::default()
    };
    // Module text alone goes through the overlay, byte for byte; anything
    // else compiles the objects it touches.
    // A compiled load's new file keys (None: not verified, so not known).
    let mut compiled_keys = None;
    let compiled_load = !plan.unsupported.is_empty();
    if !compiled_load {
        load_plan(
            edited,
            base,
            output,
            source_version,
            plan,
            &base_versions,
            &mut report,
        )?;
    } else {
        let external =
            external::load_external(edited, base, output, source_version, &diff, &mut report)?
                .is_some();
        if !external {
            compiled::load_compiled(edited, base, output, &diff, &keys, &mut report)?;
        }
        // A compile the file does not export back from is refused, the file
        // removed: the compiler does not write every property of every body
        // yet, and a load must never lose an edit or a neighbour quietly.
        // IBCMD_RS_LOAD_NO_VERIFY=1 keeps an unverified file, for diagnosing
        // what the compiler misses; never for real use.
        let verify = std::env::var_os("IBCMD_RS_LOAD_NO_VERIFY").is_none();
        if verify {
            match verify_loaded(edited, output, source_version, &diff, &report, external) {
                Ok(keys) => compiled_keys = Some(keys),
                Err(error) => {
                    let _ = fs::remove_file(output);
                    return Err(error);
                }
            }
        }
    }
    // The index follows the file just written: the next edit of this tree
    // loads onto it the same way. An unverified compiled load leaves no
    // index: its keys for added files are unknown, and a stale key would
    // send the next edit of such a file down the wrong path.
    if let Some(found) = &tree_index {
        if compiled_load && compiled_keys.is_none() {
            let _ = fs::remove_file(index::index_path(edited));
        } else {
            index::refresh_tree_index(
                edited,
                output,
                found,
                &report.applied,
                &diff.removed,
                compiled_keys.as_ref(),
            )?;
        }
    }
    Ok(report)
}

/// The base exported to scratch and the tree compared with it: the diff, the
/// export's file -> entry keys, and the base's configVersions.
fn export_and_diff(
    base: &Path,
    edited: &Path,
    source_version: InfobaseConfigSourceVersion,
) -> std::result::Result<(TreeDiff, BTreeMap<String, String>, BTreeMap<String, String>), LoadError>
{
    let scratch = Scratch(std::env::temp_dir().join(format!(
        "ibcmd-load-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    )));
    let result = (|| {
        let report = export_base(base, &scratch, source_version)?;
        let diff = diff_trees(&scratch, edited)?;
        let mut base_versions = config_versions(&scratch.join(CONFIG_DUMP_INFO));
        // Native CFE file export has no ConfigDumpInfo. The archive still
        // records the same entry versions in its configinfo manifest.
        if base_versions.is_empty() {
            base_versions = index::file_versions(base);
        }
        Ok::<_, anyhow::Error>((diff, keys_by_output(&report), base_versions))
    })();
    drop(scratch);
    Ok(result?)
}

fn load_plan(
    edited: &Path,
    base: &Path,
    output: &Path,
    source_version: InfobaseConfigSourceVersion,
    plan: Plan,
    base_versions: &BTreeMap<String, String>,
    report: &mut LoadReport,
) -> std::result::Result<(), LoadError> {
    let mut tree_versions = config_versions(&edited.join(CONFIG_DUMP_INFO));
    // For CFE, provenance is explicit (--index), rather than an extra XML
    // file absent from the native export contract. An older base's index
    // is useful here even though it cannot select the fast loading path.
    if tree_versions.is_empty() {
        if let Some(found) =
            index::read_tree_index(edited).filter(|found| found.dialect == source_version.as_str())
        {
            tree_versions = found.versions;
        }
    }
    if !plan.unsupported.is_empty() {
        return Err(LoadError::Unsupported(plan.unsupported));
    }
    if plan.edits.is_empty() {
        return Err(LoadError::NoChanges);
    }

    let mut texts = BTreeMap::new();
    let mut codec = LoadCodec {
        edited,
        modules: BTreeMap::new(),
        forms: BTreeMap::new(),
        base_has_versions: true,
    };
    for edit in &plan.edits {
        match edit {
            Edit::Module { key, text_path } => {
                codec.modules.insert(key.clone(), text_path.clone());
                texts.insert(
                    key.clone(),
                    module_text(edited, text_path).map_err(|error| anyhow::anyhow!(error))?,
                );
                report.modules += 1;
            }
            Edit::FormBody { key, module } => {
                // Refused here, before the overlay starts, not mid-write.
                module_text(edited, module).map_err(|error| anyhow::anyhow!(error))?;
                codec.forms.insert(key.clone(), module.clone());
                report.forms += 1;
            }
        }
        let (key, path) = match edit {
            Edit::Module { key, text_path } => (key, text_path),
            Edit::FormBody { key, module } => (key, module),
        };
        report.applied.push(path.clone());
        if let (Some(tree), Some(base)) = (tree_versions.get(key), base_versions.get(key))
            && tree != base
        {
            report.warnings.push(format!(
                "{path}: the base changed this object since the tree was exported                  (configVersion {tree} in the tree, {base} in the base); the tree's text replaced it"
            ));
        }
    }
    report.applied.sort();
    let provenance =
        |key: &str| StorageProvenance::new(&format!("cf-load:{key}")).context("load provenance");
    let mut requests = Vec::new();
    for key in texts.keys() {
        let target = StoragePatchTarget::new(
            StorageKey::new(key).context("module storage key")?,
            MultipartIdentity::single(),
            provenance(key)?,
        );
        requests.push(CompileRequest::new(
            target,
            SourcePayload::NeedsBase {
                required: StorageKey::new(key).context("module storage key")?,
                reason: "module text is replaced inside its native frame",
            },
        ));
    }
    for key in codec.forms.keys() {
        let storage_key = StorageKey::new(key).context("form storage key")?;
        let target = StoragePatchTarget::new(
            storage_key.clone(),
            MultipartIdentity::single(),
            provenance(key)?,
        );
        requests.push(CompileRequest::new(
            target,
            SourcePayload::NeedsBase {
                required: storage_key,
                reason: "managed form module is replaced inside its base body",
            },
        ));
    }
    let axes = CompileAxes::new(
        XmlDialect::parse(source_version.as_str()).context("XML dialect")?,
        None,
        None,
        StorageProfileId::parse(OVERLAY_PROFILE).expect("static profile id is valid"),
        None,
    );
    let retained = texts.values().map(Vec::len).sum::<usize>();
    let patch = compile_overlay_with_retained_budget(
        &axes,
        requests,
        ResourceLimits::for_input_bytes(retained as u64).max_retained_bytes_usize(),
    )
    .map_err(|error| anyhow::anyhow!("failed to compile load patch: {error}"))?;

    let limits = ResourceLimits::for_input_bytes(fs::metadata(base).context("base")?.len());
    let archive = decode_archive_uniform(
        fs::File::open(base).context("base")?,
        limits,
        StorageProfileId::parse(OVERLAY_PROFILE).expect("static profile id is valid"),
        StorageProvenance::new("cf load base").expect("static provenance is valid"),
        PayloadEncoding::RawDeflate,
    )
    .map_err(|error| anyhow::anyhow!("failed to decode base `{}`: {error}", base.display()))?;
    let has = |key: &str| {
        archive
            .image()
            .entries()
            .iter()
            .any(|entry| entry.logical_key().as_str() == key)
    };
    codec.base_has_versions = has("versions");
    if codec.base_has_versions || !has("configinfo") {
        publish(&archive, &patch, &mut codec, output, limits)?;
        return Ok(());
    }
    // An extension: configinfo keeps the SHA-1 of every entry's packed bytes,
    // and 8.3.27.2214 updating an installed extension keeps each entry whose
    // digest did not change -- an edit under the old digest never reaches the
    // infobase. The edited entries are packed first, then their digests.
    let changed = texts
        .keys()
        .chain(codec.forms.keys())
        .cloned()
        .collect::<Vec<_>>();
    let staged = staging_path(output);
    let result = publish(&archive, &patch, &mut codec, &staged, limits)
        .and_then(|()| refresh_configinfo(&staged, output, &changed, &axes, edited));
    let _ = fs::remove_file(&staged);
    result?;
    Ok(())
}

fn publish(
    archive: &ibcmd_cf::archive::CfArchive,
    patch: &ibcmd_core::storage::StoragePatch,
    codec: &mut LoadCodec<'_>,
    output: &Path,
    limits: ResourceLimits,
) -> Result<()> {
    publish_overlay_new(archive, patch, codec, output, limits).map_err(|error| {
        let blockers = error
            .preflight()
            .map(|preflight| {
                preflight
                    .blockers()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default();
        anyhow::anyhow!("failed to write `{}`: {error} {blockers}", output.display())
    })?;
    Ok(())
}

/// A sibling of `output` no run has used.
fn staging_path(output: &Path) -> std::path::PathBuf {
    let name = output
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    output.with_file_name(format!(
        ".{name}.{}-{}.configinfo",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ))
}

/// Writes `output`: `staged` with the configinfo digests of `changed`
/// recomputed from their packed bytes in `staged`.
fn refresh_configinfo(
    staged: &Path,
    output: &Path,
    changed: &[String],
    axes: &CompileAxes,
    edited: &Path,
) -> Result<()> {
    use sha1::{Digest, Sha1};
    let limits = ResourceLimits::for_input_bytes(fs::metadata(staged).context("staged")?.len());
    let archive = decode_archive_uniform(
        fs::File::open(staged).context("staged")?,
        limits,
        StorageProfileId::parse(OVERLAY_PROFILE).expect("static profile id is valid"),
        StorageProvenance::new("cf load staged").expect("static provenance is valid"),
        PayloadEncoding::RawDeflate,
    )
    .map_err(|error| anyhow::anyhow!("failed to decode `{}`: {error}", staged.display()))?;
    let packed = |key: &str| {
        archive
            .image()
            .entries()
            .iter()
            .find(|entry| entry.logical_key().as_str() == key)
            .map(StorageEntry::packed_payload)
            .with_context(|| format!("`{key}` is not in the written extension"))
    };
    let text = String::from_utf8(inflate_raw(packed("configinfo")?)?)
        .context("configinfo is not UTF-8")?;
    let mut digests = BTreeMap::new();
    for key in changed {
        digests.insert(key.clone(), encode_base64(&Sha1::digest(packed(key)?)));
    }
    let text = with_configinfo_digests(&text, &digests)?;
    let target = StoragePatchTarget::new(
        StorageKey::new("configinfo").context("configinfo storage key")?,
        MultipartIdentity::single(),
        StorageProvenance::new("cf-load:configinfo").context("load provenance")?,
    );
    let patch = compile_overlay_with_retained_budget(
        axes,
        [CompileRequest::new(
            target,
            SourcePayload::RawDeflated {
                bytes: text.as_bytes(),
            },
        )],
        ResourceLimits::for_input_bytes(text.len() as u64).max_retained_bytes_usize(),
    )
    .map_err(|error| anyhow::anyhow!("failed to compile configinfo: {error}"))?;
    let mut codec = LoadCodec {
        edited,
        modules: BTreeMap::new(),
        forms: BTreeMap::new(),
        base_has_versions: false,
    };
    publish(&archive, &patch, &mut codec, output, limits)
}

/// `text` (configinfo) with the digest after each `"<entry>",` replaced;
/// an entry it does not list exactly once is refused.
fn with_configinfo_digests(text: &str, digests: &BTreeMap<String, String>) -> Result<String> {
    let mut text = text.to_owned();
    for (entry, digest) in digests {
        let marker = format!("\"{entry}\",");
        if text.matches(&marker).count() != 1 {
            anyhow::bail!("configinfo does not list `{entry}` exactly once");
        }
        let start = text.find(&marker).expect("counted above") + marker.len();
        let token = &text[start..];
        let skip = token.len() - token.trim_start().len();
        let len = token[skip..]
            .find(|c: char| c == ',' || c == '}' || c.is_whitespace())
            .context("configinfo digest is not terminated")?;
        text.replace_range(start + skip..start + skip + len, digest);
    }
    Ok(text)
}

/// Output path → storage key, from an export report.
pub fn keys_by_output(report: &StorageImageSourceExportReport) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for entry in &report.storage.entries {
        for output in &entry.outputs {
            out.insert(output.replace('\\', "/"), entry.logical_key.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn filesystem_module_verification_projects_only_expected_source_bytes() {
        let raw = b"Procedure Smoke() Export\nEndProcedure\n";
        let native = b"\xef\xbb\xbfProcedure Smoke() Export\r\nEndProcedure\r\n";
        assert!(super::edited_source_matches_export(
            "Ext/Module.bsl",
            raw,
            raw
        ));
        assert!(super::edited_source_matches_export(
            "Ext/Module.bsl",
            raw,
            native
        ));
        assert!(!super::edited_source_matches_export(
            "Ext/Module.bin",
            raw,
            native
        ));
        assert!(!super::edited_source_matches_export(
            "Ext/Module.txt",
            raw,
            native
        ));
        // The projection branch compares exact native bytes, rather than
        // accepting a body obtained by normalizing both sides.
        assert!(!super::edited_source_matches_export(
            "Ext/Module.bsl",
            raw,
            b"\xef\xbb\xbfProcedure Smoke() Export\nEndProcedure\n",
        ));
        assert!(!super::edited_source_matches_export(
            "Ext/Module.bsl",
            raw,
            b"\xef\xbb\xbfProcedure Changed() Export\r\nEndProcedure\r\n",
        ));
    }

    use super::*;
    use std::fs;

    fn native_module_fixture() -> Vec<u8> {
        use crate::v8_container::{V8Element, build_v8_container, make_v8_element_header};
        let mut info_header = make_v8_element_header("info");
        info_header[..16].copy_from_slice(&[0x17; 16]);
        let mut text_header = make_v8_element_header("text");
        text_header[..16].copy_from_slice(&[0x29; 16]);
        crate::module_blob::deflate_raw(
            &build_v8_container(&[
                V8Element {
                    name: "info".into(),
                    header: info_header,
                    data: b"\xef\xbb\xbf{3,1,1,\"retained-context\",0}".to_vec(),
                },
                V8Element {
                    name: "text".into(),
                    header: text_header,
                    data: b"\xef\xbb\xbf// old\r\n".to_vec(),
                },
            ])
            .unwrap(),
        )
        .unwrap()
    }

    fn assert_module_context_preserved(base: &[u8], packed: &[u8], text: &[u8]) {
        let parse = |bytes: &[u8]| {
            crate::v8_container::parse_v8_container(&inflate_raw(bytes).unwrap()).unwrap()
        };
        let before = parse(base);
        let after = parse(packed);
        assert_eq!(after.len(), 2);
        for name in ["info", "text"] {
            let old = before.iter().find(|element| element.name == name).unwrap();
            let new = after.iter().find(|element| element.name == name).unwrap();
            assert_eq!(new.header, old.header, "{name} header");
            assert_eq!(
                new.data,
                if name == "info" {
                    old.data.as_slice()
                } else {
                    text
                }
            );
        }
    }

    #[test]
    fn compiled_module_edit_preserves_native_info_and_headers() {
        let root = tmp("compiled-module-context");
        let path = "CommonModules/Owned/Ext/Module.bsl";
        let key = "23eac911-1056-4e85-92d7-6467c511eb49.0";
        write(&root, path, "// edited\n");
        let base = native_module_fixture();
        let compiled = compiled::compile_edit(
            &root,
            [(key.to_owned(), base.clone())].into_iter().collect(),
            &diff(&[path], &[], &[]),
            &keys(&[(path, key)]),
        )
        .unwrap();
        assert_module_context_preserved(
            &base,
            &compiled.rows[key],
            &module_text(&root, path).unwrap(),
        );
    }

    #[test]
    fn overlay_module_edit_preserves_native_info_and_headers() {
        use ibcmd_core::storage::{
            CompressionKind, OpaqueStorageMetadata, StorageName, StorageOrigin, StoragePayloads,
        };
        let root = tmp("overlay-module-context");
        let path = "CommonModules/Owned/Ext/Module.bsl";
        let key = "23eac911-1056-4e85-92d7-6467c511eb49.0";
        write(&root, path, "// edited\n");
        let base = native_module_fixture();
        let entry = StorageEntry::new(
            StorageName::new(key).unwrap(),
            StorageKey::new(key).unwrap(),
            MultipartIdentity::single(),
            OpaqueStorageMetadata::empty(),
            StoragePayloads::new(base.clone(), inflate_raw(&base).unwrap()).unwrap(),
            CompressionKind::raw_deflate(),
            StorageOrigin::new(
                StorageProfileId::parse(OVERLAY_PROFILE).unwrap(),
                StorageProvenance::new("fixture:load-native-module").unwrap(),
            ),
        )
        .unwrap();
        let target = StoragePatchTarget::new(
            StorageKey::new(key).unwrap(),
            MultipartIdentity::single(),
            StorageProvenance::new("fixture:module-edit").unwrap(),
        );
        let mut codec = LoadCodec {
            edited: &root,
            modules: [(key.to_owned(), path.to_owned())].into_iter().collect(),
            forms: BTreeMap::new(),
            base_has_versions: false,
        };
        let packed = codec
            .resolve_needs_base(&target, entry.logical_key(), &entry)
            .unwrap();
        assert_module_context_preserved(&base, &packed, &module_text(&root, path).unwrap());
    }

    fn tmp(tag: &str) -> crate::test_scratch::TestScratch {
        let dir =
            std::env::temp_dir().join(format!("ibcmd-load-unit-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        crate::test_scratch::TestScratch::adopt(dir)
    }

    fn write(root: &std::path::Path, rel: &str, text: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn diff(changed: &[&str], added: &[&str], removed: &[&str]) -> TreeDiff {
        let own = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect();
        TreeDiff {
            changed: own(changed),
            added: own(added),
            removed: own(removed),
        }
    }

    fn keys(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(p, k)| ((*p).to_owned(), (*k).to_owned()))
            .collect()
    }

    #[test]
    fn changed_module_is_a_module_edit() {
        let plan = classify(
            &diff(&["О/Ext/ObjectModule.bsl"], &[], &[]),
            &keys(&[("О/Ext/ObjectModule.bsl", "o.0")]),
        );
        assert!(plan.unsupported.is_empty());
        assert_eq!(
            plan.edits,
            vec![Edit::Module {
                key: "o.0".into(),
                text_path: "О/Ext/ObjectModule.bsl".into()
            }]
        );
    }

    #[test]
    fn closed_module_bin_replaced_by_bsl_is_a_module_edit() {
        let plan = classify(
            &diff(
                &[],
                &["CommonModules/М/Ext/Module.bsl"],
                &["CommonModules/М/Ext/Module.bin"],
            ),
            &keys(&[("CommonModules/М/Ext/Module.bin", "u1.0")]),
        );
        assert!(plan.unsupported.is_empty());
        assert_eq!(
            plan.edits,
            vec![Edit::Module {
                key: "u1.0".into(),
                text_path: "CommonModules/М/Ext/Module.bsl".into()
            }]
        );
    }

    #[test]
    fn form_xml_change_is_unsupported_beside_a_form_module_edit() {
        let plan = classify(
            &diff(
                &["О/Forms/Ф/Ext/Form.xml", "О/Forms/Ф/Ext/Form/Module.bsl"],
                &[],
                &[],
            ),
            &keys(&[
                ("О/Forms/Ф/Ext/Form.xml", "f.0"),
                ("О/Forms/Ф/Ext/Form/Module.bsl", "f.0"),
            ]),
        );
        assert_eq!(plan.unsupported, vec!["О/Forms/Ф/Ext/Form.xml"]);
        assert_eq!(
            plan.edits,
            vec![Edit::FormBody {
                key: "f.0".into(),
                module: "О/Forms/Ф/Ext/Form/Module.bsl".into(),
            }]
        );
    }

    #[test]
    fn configuration_module_is_a_module_edit() {
        let plan = classify(
            &diff(&["Ext/SessionModule.bsl"], &[], &[]),
            &keys(&[("Ext/SessionModule.bsl", "c.7")]),
        );
        assert!(plan.unsupported.is_empty(), "{:?}", plan.unsupported);
        assert_eq!(
            plan.edits,
            vec![Edit::Module {
                key: "c.7".into(),
                text_path: "Ext/SessionModule.bsl".into()
            }]
        );
    }

    #[test]
    fn closed_configuration_module_replaced_by_source_is_a_module_edit() {
        let plan = classify(
            &diff(
                &[],
                &["Ext/ManagedApplicationModule.bsl"],
                &["Ext/ManagedApplicationModule.bin"],
            ),
            &keys(&[("Ext/ManagedApplicationModule.bin", "c.6")]),
        );
        assert!(plan.unsupported.is_empty(), "{:?}", plan.unsupported);
        assert_eq!(
            plan.edits,
            vec![Edit::Module {
                key: "c.6".into(),
                text_path: "Ext/ManagedApplicationModule.bsl".into()
            }]
        );
    }

    #[test]
    fn the_scratch_export_is_removed_even_when_the_load_panics() {
        let dir = std::env::temp_dir().join(format!("ibcmd-scratch-test-{}", std::process::id()));
        let _scratch = crate::test_scratch::TestScratch::adopt(&dir);
        fs::create_dir_all(dir.join("a")).unwrap();
        let guarded = dir.clone();
        let outcome = std::panic::catch_unwind(move || {
            let _scratch = Scratch(guarded);
            panic!("mid-load");
        });
        assert!(outcome.is_err());
        assert!(!dir.exists());
    }

    #[test]
    fn only_a_module_bin_pairs_with_source_text() {
        // A binary template (`Template.bin`) is no closed module: its
        // replacement by a `.bsl` is no module edit.
        let plan = classify(
            &diff(
                &[],
                &["Catalogs/С/Templates/Т/Ext/Template.bsl"],
                &["Catalogs/С/Templates/Т/Ext/Template.bin"],
            ),
            &keys(&[("Catalogs/С/Templates/Т/Ext/Template.bin", "t.0")]),
        );
        assert!(plan.edits.is_empty(), "{:?}", plan.edits);
        assert_eq!(plan.unsupported.len(), 2, "{:?}", plan.unsupported);
    }

    #[test]
    fn held_module_text_uses_existing_storage_policy() {
        for (input, expected) in [
            (b"// line\n".as_slice(), b"// line\r\n".as_slice()),
            (
                b"\xef\xbb\xbf// line\n".as_slice(),
                b"\xef\xbb\xbf// line\r\n".as_slice(),
            ),
            (b"// line\r\n".as_slice(), b"// line\r\n".as_slice()),
            ("А\n".as_bytes(), "А\r\n".as_bytes()),
            (b"tail\r".as_slice(), b"tail\r".as_slice()),
        ] {
            assert_eq!(
                super::module_text_from_bytes(input, "Ext/Module.bsl").unwrap(),
                expected
            );
        }
        assert_eq!(
            super::module_text_from_bytes(&[0xff], "Ext/Module.bsl").unwrap_err(),
            "Ext/Module.bsl: module text must be UTF-8 (save the file as UTF-8)"
        );
    }

    #[test]
    fn module_text_is_utf8_with_crlf_line_ends() {
        let root = tmp("utf8");
        write(&root, "М/Ext/Module.bsl", "\u{feff}А = 1;\nБ = 2;\n");
        assert_eq!(
            module_text(&root, "М/Ext/Module.bsl").unwrap(),
            "\u{feff}А = 1;\r\nБ = 2;\r\n".as_bytes()
        );
        fs::write(
            root.join("М/Ext/Module.bsl"),
            [0xc0u8, b' ', b'=', b' ', b'1'],
        )
        .unwrap();
        let error = module_text(&root, "М/Ext/Module.bsl").unwrap_err();
        assert!(error.contains("М/Ext/Module.bsl"), "{error}");
        assert!(error.contains("UTF-8"), "{error}");
    }

    #[test]
    fn diff_skips_dot_entries_of_either_tree() {
        let (a, b) = (tmp("dot-a"), tmp("dot-b"));
        write(&a, "М/Ext/Module.bsl", "same");
        write(&b, "М/Ext/Module.bsl", "same");
        write(&b, ".git/config", "[core]");
        write(&b, ".vscode/settings.json", "{}");
        write(&a, ".complete", "");
        assert_eq!(diff_trees(&a, &b).unwrap(), TreeDiff::default());
    }

    #[test]
    fn unsupported_message_lists_at_most_twenty_paths() {
        let paths = (0..25).map(|i| format!("О{i}.xml")).collect::<Vec<_>>();
        let message = LoadError::Unsupported(paths).to_string();
        assert!(message.contains("О19.xml"), "{message}");
        assert!(!message.contains("О20.xml"), "{message}");
        assert!(message.contains("5 more"), "{message}");
    }

    #[test]
    fn form_module_alone_edits_the_form_body() {
        let plan = classify(
            &diff(&["О/Forms/Ф/Ext/Form/Module.bsl"], &[], &[]),
            &keys(&[
                ("О/Forms/Ф/Ext/Form.xml", "f.0"),
                ("О/Forms/Ф/Ext/Form/Module.bsl", "f.0"),
            ]),
        );
        assert_eq!(
            plan.edits,
            vec![Edit::FormBody {
                key: "f.0".into(),
                module: "О/Forms/Ф/Ext/Form/Module.bsl".into(),
            }]
        );
    }

    #[test]
    fn unmapped_change_is_refused() {
        let plan = classify(
            &diff(&["О/Ext/ObjectModule.bsl"], &[], &[]),
            &BTreeMap::new(),
        );
        assert!(plan.edits.is_empty());
        assert_eq!(plan.unsupported, vec!["О/Ext/ObjectModule.bsl"]);
    }

    #[test]
    fn metadata_xml_and_structural_changes_are_unsupported() {
        let plan = classify(
            &diff(&["О.xml"], &["О/Templates/Н.xml"], &["О/Ext/Help.xml"]),
            &keys(&[("О.xml", "main"), ("О/Ext/Help.xml", "o.1")]),
        );
        assert!(plan.edits.is_empty());
        assert_eq!(
            plan.unsupported,
            vec!["О.xml", "О/Templates/Н.xml", "О/Ext/Help.xml"]
        );
    }

    #[test]
    fn module_text_line_ends_become_crlf() {
        assert_eq!(crlf(b"a\nb\r\nc\n"), b"a\r\nb\r\nc\r\n".to_vec());
        assert_eq!(crlf(b"\xef\xbb\xbfx"), b"\xef\xbb\xbfx".to_vec());
    }

    #[test]
    fn diff_reports_changed_added_and_removed_files() {
        let (a, b) = (tmp("a"), tmp("b"));
        write(&a, "Обработка/Ext/ObjectModule.bsl", "old");
        write(&b, "Обработка/Ext/ObjectModule.bsl", "new");
        write(&a, "Обработка/Ext/Module.bin", "x");
        write(&b, "Обработка/Ext/Module.bsl", "y");
        write(&a, "Обработка.xml", "same");
        write(&b, "Обработка.xml", "same");
        let d = diff_trees(&a, &b).unwrap();
        assert_eq!(d.changed, vec!["Обработка/Ext/ObjectModule.bsl"]);
        assert_eq!(d.added, vec!["Обработка/Ext/Module.bsl"]);
        assert_eq!(d.removed, vec!["Обработка/Ext/Module.bin"]);
    }
}
