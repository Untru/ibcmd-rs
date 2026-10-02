//! The guard of `infobase config import`.
//!
//! A stage builds the rows it would write into ConfigSave from the tree. In
//! patch mode most of them are the database's own rows with the tree's name,
//! synonym and comment patched in, so a change the patch does not carry (an
//! added attribute, a tabular section, a subsystem's content, a property) used
//! to end in a successful import that had dropped it. The guard closes that
//! hole with the one check that does not depend on what the stage knows:
//! before anything is written, the configuration the stage would leave -- the
//! stored rows with the staged ones in place of theirs -- is exported with the
//! model, in memory, and every file of the export is compared with the tree.
//! A file that differs is a change the stage does not carry; a file the tree
//! lacks is something the tree removed that the stage leaves in. Any
//! difference refuses the import ([`StageRefused`], exit -1) with the files
//! listed, and ConfigSave is exactly what it was.
//!
//! The comparison is by content, not by formatting: identical bytes pass at
//! once; an XML file that differs is compared leaf by leaf (element path,
//! attributes and trimmed text, as `source-diff-explain` does), and a module
//! or another text file after its line ends and byte order mark are
//! normalized. `ConfigDumpInfo.xml` is not compared: it holds generation ids
//! no stage keeps.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::Instant;

use anyhow::{Context, Result, anyhow};
use rayon::prelude::*;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::cli::{InfobaseConfigSourceVersion, MssqlStageSourceObjectsArgs};
use crate::mssql_dump::{FileSink, StagedRow, StateBase, export_staged_state};
use crate::plan::{
    SourceDiffLeafDifference, SourceDiffLeafDifferenceKind, diff_indexed_xml_values,
    parse_indexed_xml_values,
};
use crate::source::{SourceManifest, is_metadata_collection};
use crate::source_listing;
use crate::sql::SqlExec;

/// Environment switch for scripts: `0`/`off` skips the guard, `1`/`on` runs
/// it whatever the command line and the stage mode say.
pub(super) const VERIFY_ENV: &str = "IBCMD_RS_STAGE_VERIFY";

/// Files listed in a refusal, at most; the rest are counted.
const LISTED_FILES: usize = 25;
/// Leaves listed for one XML file, at most.
const LISTED_LEAVES: usize = 3;
/// Characters of a value shown in a message.
const SHOWN_VALUE_CHARS: usize = 70;

/// Which stage a guard checks, for the wording of its refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StageKind {
    /// The target holds the configuration: its rows patched from the tree.
    Patch,
    /// Every row compiled from the tree.
    BaseFree,
}

/// How a file of the staged state differs from the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Difference {
    /// Both have the file; `leaves` say where the content differs.
    Changed { leaves: Vec<String> },
    /// Only the staged state has the file: something the tree lacks would
    /// stay in the configuration.
    OnlyInState,
    /// Only the tree has the file: something the stage does not carry.
    OnlyInTree,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileDifference {
    pub path: String,
    pub difference: Difference,
}

/// What the guard did, for the import's report.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StageVerification {
    /// Files of the staged state compared with the tree.
    pub checked_files: usize,
    /// Of them, identical to the byte.
    pub identical_files: usize,
    /// Rows of the staged state (stored and staged).
    pub state_rows: usize,
    pub read_seconds: f64,
    pub export_seconds: f64,
    pub total_seconds: f64,
}

/// The import was refused because the staged state differs from the tree.
/// The text is the message, in Russian, one line per `[ERROR]`.
#[derive(Debug)]
pub struct StageRefused {
    message: String,
    /// The files of the staged state that differ from the tree, when the guard found them.
    differences: Vec<FileDifference>,
}

impl StageRefused {
    /// A refusal with this message: one `[ERROR]` line per line of it.
    pub(super) fn new(message: String) -> Self {
        Self {
            message,
            differences: Vec::new(),
        }
    }

    fn with_differences(mut self, differences: Vec<FileDifference>) -> Self {
        self.differences = differences;
        self
    }

    /// The files that differ (empty for a refusal that is not the guard's comparison).
    pub(super) fn differences(&self) -> &[FileDifference] {
        &self.differences
    }
}

impl fmt::Display for StageRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for StageRefused {}

/// What the stage already knows of the tree's files.
pub(crate) enum TreeFiles<'a> {
    /// The stage scanned the tree: every file with its sha256, so a file the
    /// export reproduces to the byte is not read again.
    Scanned(&'a SourceManifest),
    /// The stage only listed the tree (a base-free stage reads what its rows
    /// need): the files are hashed beside the export, which needs the model
    /// and not the disk for most of its time.
    Listed(&'a [PathBuf]),
    /// The stage knows nothing: the guard lists the tree itself.
    Unlisted,
}

/// The guard's input beyond the rows.
pub(crate) struct GuardRequest<'a> {
    pub kind: StageKind,
    pub source_root: &'a Path,
    pub tree: TreeFiles<'a>,
    /// The rows of the target the stage deletes (its `deleted` row): not in
    /// the state the apply leaves.
    pub removed: &'a [String],
    /// The path prefixes the stage was asked to import (empty: all).
    pub path_prefix: &'a [String],
    /// The XML version asked for; the tree's own when absent.
    pub source_version: Option<InfobaseConfigSourceVersion>,
}

/// Whether the guard runs: as `verify` says, unless `IBCMD_RS_STAGE_VERIFY`
/// forces it off (`0`, `off`, `no`) or on (`1`, `on`, `yes`) for a script.
pub(super) fn wanted(verify: bool) -> bool {
    wanted_with(std::env::var(VERIFY_ENV).ok().as_deref(), verify)
}

fn wanted_with(environment: Option<&str>, verify: bool) -> bool {
    match environment.map(str::trim) {
        Some("0" | "off" | "no") => false,
        Some("1" | "on" | "yes") => true,
        _ => verify,
    }
}

/// The rows a stage would leave over: the lab's row folder when the stage
/// reads its base rows from one (`IBCMD_RS_BASE_ROWS_DIR`), else the
/// database's Config table when there is one to read -- from the rows the
/// stage has already read, when it has -- else nothing.
fn state_base<'a>(
    base_dir: Option<&'a Path>,
    sql: Option<&'a SqlExec>,
    database: &'a str,
    prefetched: Option<&'a HashMap<String, Arc<Vec<u8>>>>,
) -> StateBase<'a> {
    match (base_dir, sql, prefetched) {
        (Some(dir), _, _) => StateBase::Folder(dir),
        (None, Some(sql), Some(part0)) => StateBase::Prefetched {
            part0,
            sql,
            database,
        },
        (None, Some(sql), None) => StateBase::Database { sql, database },
        (None, None, _) => StateBase::Nothing,
    }
}

/// The guard for a patch stage: its rows over the target's Config.
pub(super) fn verify_patch_stage(
    args: &MssqlStageSourceObjectsArgs,
    sql: &SqlExec,
    manifest: &SourceManifest,
    rows: &[super::BulkStageRow<'_>],
    removed: &[String],
) -> Result<StageVerification> {
    let base_dir = std::env::var_os("IBCMD_RS_BASE_ROWS_DIR").map(PathBuf::from);
    verify_staged_state(
        &GuardRequest {
            kind: StageKind::Patch,
            source_root: &args.source_root,
            tree: TreeFiles::Scanned(manifest),
            removed,
            // A partial import of files (#363) is checked on those files:
            // the directory need not hold the rest of the objects' files.
            path_prefix: if args.files.is_empty() {
                &args.path_prefix
            } else {
                &args.files
            },
            source_version: args.source_version,
        },
        state_base(
            base_dir.as_deref(),
            Some(sql),
            &args.database,
            super::prefetched_base_rows(&args.database),
        ),
        &staged_rows(rows),
    )
}

/// The guard for a base-free stage: its rows over what the target holds (an
/// empty infobase's few rows), or over nothing when the stage reaches no
/// database. `tree_files` is the list of files the stage walked.
pub(super) fn verify_base_free_stage(
    args: &MssqlStageSourceObjectsArgs,
    sql: Option<&SqlExec>,
    rows: &[super::BulkStageRow<'_>],
    tree_files: &[PathBuf],
) -> Result<StageVerification> {
    let base_dir = std::env::var_os("IBCMD_RS_BASE_ROWS_DIR").map(PathBuf::from);
    verify_staged_state(
        &GuardRequest {
            kind: StageKind::BaseFree,
            source_root: &args.source_root,
            tree: TreeFiles::Listed(tree_files),
            removed: &[],
            path_prefix: &args.path_prefix,
            source_version: args.source_version,
        },
        state_base(base_dir.as_deref(), sql, &args.database, None),
        &staged_rows(rows),
    )
}

fn staged_rows<'a>(rows: &'a [super::BulkStageRow<'a>]) -> Vec<StagedRow<'a>> {
    rows.iter()
        .map(|row| StagedRow {
            file_name: row.file_name,
            bytes: row.blob,
        })
        .collect()
}

/// Checks the state `staged` would leave over `base` against the tree.
///
/// `Ok` carries what was checked; a state that differs from the tree is
/// `Err(`[`StageRefused`]`)`; any other error is a check that could not run
/// (the refusal says so and how to skip it).
pub(crate) fn verify_staged_state(
    request: &GuardRequest<'_>,
    base: StateBase<'_>,
    staged: &[StagedRow<'_>],
) -> Result<StageVerification> {
    let started = Instant::now();
    let (outcome, export) = compare_state(request, base, staged).map_err(|error| {
        anyhow!(
            "Не удалось проверить результат загрузки: {error:#}\n\
                 Загрузка не выполнена, в ConfigSave ничего не записано. Проверку можно отключить \
                 ключом --no-verify (тогда расхождения с деревом не обнаруживаются)."
        )
    })?;
    let total_seconds = started.elapsed().as_secs_f64();
    if !outcome.differences.is_empty() {
        let message = refusal_text(
            request.kind,
            request.source_root,
            &outcome.differences,
            outcome.compared,
        );
        return Err(anyhow::Error::new(
            StageRefused::new(message).with_differences(outcome.differences),
        ));
    }
    Ok(StageVerification {
        checked_files: outcome.compared,
        identical_files: outcome.identical,
        state_rows: export.state_rows,
        read_seconds: export.read_ms as f64 / 1000.0,
        export_seconds: export.export_ms as f64 / 1000.0,
        total_seconds,
    })
}

/// How the tree stands against the target's own state: the stored rows as the
/// storage publishes them, nothing staged, exported with the model.
pub(super) struct TargetComparison {
    /// Files of the tree the target's export does not reproduce (by content),
    /// files the export has and the tree lacks, and files the tree has and the
    /// export lacks.
    pub differences: Vec<FileDifference>,
    pub compared: usize,
    pub identical: usize,
    pub seconds: f64,
}

/// Compares the tree with the export of the target's own state (#395): what
/// the target already holds as the tree has it needs no row in a stage.
pub(super) fn compare_tree_with_target(
    args: &MssqlStageSourceObjectsArgs,
    sql: &SqlExec,
    manifest: &SourceManifest,
) -> Result<TargetComparison> {
    let started = Instant::now();
    let base_dir = std::env::var_os("IBCMD_RS_BASE_ROWS_DIR").map(PathBuf::from);
    let request = GuardRequest {
        kind: StageKind::Patch,
        source_root: &args.source_root,
        tree: TreeFiles::Scanned(manifest),
        removed: &[],
        path_prefix: &args.path_prefix,
        source_version: args.source_version,
    };
    let (outcome, _) = compare_state(
        &request,
        state_base(
            base_dir.as_deref(),
            Some(sql),
            &args.database,
            super::prefetched_base_rows(&args.database),
        ),
        &[],
    )?;
    Ok(TargetComparison {
        differences: outcome.differences,
        compared: outcome.compared,
        identical: outcome.identical,
        seconds: started.elapsed().as_secs_f64(),
    })
}

/// Exports the state `staged` would leave over `base` and compares each file
/// with the tree.
fn compare_state(
    request: &GuardRequest<'_>,
    base: StateBase<'_>,
    staged: &[StagedRow<'_>],
) -> Result<(Compared, crate::mssql_dump::StateExportReport)> {
    let output_root = std::env::temp_dir().join("ibcmd-rs-verified-state");
    let comparer = Arc::new(match &request.tree {
        TreeFiles::Scanned(manifest) => TreeComparer::new(
            &output_root,
            request.source_root,
            manifest
                .files
                .iter()
                .map(|file| (file.path.clone(), Some(file.sha256.clone())))
                .collect(),
            request.path_prefix,
        ),
        TreeFiles::Listed(paths) => TreeComparer::hashing(
            &output_root,
            request.source_root,
            paths.to_vec(),
            request.path_prefix,
        ),
        TreeFiles::Unlisted => TreeComparer::hashing(
            &output_root,
            request.source_root,
            source_listing::walk(request.source_root).files,
            request.path_prefix,
        ),
    });
    let version = request.source_version.unwrap_or_else(|| {
        crate::metadata_model::export::tree_version(request.source_root)
            .and_then(|text| {
                <InfobaseConfigSourceVersion as clap::ValueEnum>::from_str(text.trim(), true).ok()
            })
            .unwrap_or(InfobaseConfigSourceVersion::V2_20)
    });
    let export = export_staged_state(
        base,
        staged,
        request.removed,
        version,
        &output_root,
        comparer.clone(),
    )?;
    let outcome = comparer.finish()?;
    Ok((outcome, export))
}

// ---------------------------------------------------------------------------
// The comparison.

/// What the tree holds of one file.
struct TreeFile {
    /// Its sha256, when the stage scanned it; else the comparer reads the file.
    sha256: Option<String>,
}

/// The configuration files of a walk, in scope, keyed as the comparer keys
/// them (relative to the root, `/`-separated), each with its sha256 -- read on
/// the file-bound pool, which is where a tree of 140 thousand files spends
/// its time. A file that cannot be read has no hash, and is read again (with
/// the error) when the export produces it.
fn hash_tree(
    root: &Path,
    paths: Vec<PathBuf>,
    scope: &[String],
) -> std::result::Result<HashMap<String, TreeFile>, String> {
    let entries = paths
        .into_iter()
        .filter_map(|path| {
            let relative = path
                .strip_prefix(root)
                .ok()?
                .components()
                .map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            (is_configuration_file(&relative) && in_scope(scope, &relative))
                .then_some((relative, path))
        })
        .collect::<Vec<_>>();
    let hashed = crate::parallel::install_io_bound(|| {
        entries
            .par_iter()
            .map(|(relative, path)| {
                (
                    relative.clone(),
                    fs::read(path).ok().map(|bytes| sha256_hex(&bytes)),
                )
            })
            .collect::<Vec<_>>()
    })
    .map_err(|error| format!("{error:#}"))?;
    Ok(hashed
        .into_iter()
        .map(|(relative, sha256)| (relative, TreeFile { sha256 }))
        .collect())
}

#[derive(Default)]
struct Outcome {
    /// Relative paths the export produced.
    seen: HashSet<String>,
    compared: usize,
    identical: usize,
    differences: Vec<FileDifference>,
}

/// The sink of the verification export: each file the export would write is
/// compared with the tree's file of the same relative path.
struct TreeComparer {
    output_root: PathBuf,
    tree_root: PathBuf,
    scope: Vec<String>,
    /// The tree's files: known at once, or read by a thread that was started
    /// with the export (`pending`) and is waited for by the first file that
    /// needs it.
    tree: OnceLock<std::result::Result<HashMap<String, TreeFile>, String>>,
    pending: Mutex<Option<JoinHandle<std::result::Result<HashMap<String, TreeFile>, String>>>>,
    outcome: Mutex<Outcome>,
}

/// The result of a finished comparison.
struct Compared {
    compared: usize,
    identical: usize,
    differences: Vec<FileDifference>,
}

impl TreeComparer {
    fn new(
        output_root: &Path,
        tree_root: &Path,
        files: Vec<(String, Option<String>)>,
        path_prefix: &[String],
    ) -> Self {
        let scope = path_prefix
            .iter()
            .map(|prefix| normalize_prefix(prefix))
            .filter(|prefix| !prefix.is_empty())
            .collect::<Vec<_>>();
        let tree = files
            .into_iter()
            .filter(|(path, _)| in_scope(&scope, path))
            .map(|(path, sha256)| (path, TreeFile { sha256 }))
            .collect();
        Self {
            output_root: output_root.to_path_buf(),
            tree_root: tree_root.to_path_buf(),
            scope,
            tree: OnceLock::from(Ok(tree)),
            pending: Mutex::new(None),
            outcome: Mutex::new(Outcome::default()),
        }
    }

    /// As [`TreeComparer::new`], for the files of a walk: their hashes are
    /// read on a thread of their own, beside whatever the caller does next.
    fn hashing(
        output_root: &Path,
        tree_root: &Path,
        paths: Vec<PathBuf>,
        path_prefix: &[String],
    ) -> Self {
        let scope = path_prefix
            .iter()
            .map(|prefix| normalize_prefix(prefix))
            .filter(|prefix| !prefix.is_empty())
            .collect::<Vec<_>>();
        let handle = {
            let root = tree_root.to_path_buf();
            let scope = scope.clone();
            std::thread::spawn(move || hash_tree(&root, paths, &scope))
        };
        Self {
            output_root: output_root.to_path_buf(),
            tree_root: tree_root.to_path_buf(),
            scope,
            tree: OnceLock::new(),
            pending: Mutex::new(Some(handle)),
            outcome: Mutex::new(Outcome::default()),
        }
    }

    /// The tree's files, once they are known.
    fn tree(&self) -> Result<&HashMap<String, TreeFile>> {
        self.tree
            .get_or_init(|| {
                let handle = self
                    .pending
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .take();
                match handle {
                    Some(handle) => handle.join().unwrap_or_else(|_| {
                        Err("the thread that reads the tree panicked".to_string())
                    }),
                    None => Err("the tree's files were not read".to_string()),
                }
            })
            .as_ref()
            .map_err(|error| anyhow!("не удалось прочитать дерево файлов для проверки: {error}"))
    }

    /// The path of a produced file relative to the output root, `/`-separated.
    fn relative(&self, path: &Path) -> Option<String> {
        let relative = path.strip_prefix(&self.output_root).ok()?;
        Some(
            relative
                .components()
                .map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/"),
        )
    }

    /// The differences, sorted by path, once the export is over: what the
    /// export produced and compared, and what the tree holds that it did not
    /// produce.
    fn finish(&self) -> Result<Compared> {
        let tree = self.tree()?;
        let mut outcome = self
            .outcome
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut only_in_tree = tree
            .keys()
            .filter(|path| !outcome.seen.contains(*path) && is_configuration_file(path))
            .cloned()
            .collect::<Vec<_>>();
        only_in_tree.sort();
        for path in only_in_tree {
            outcome.differences.push(FileDifference {
                path,
                difference: Difference::OnlyInTree,
            });
        }
        let mut differences = std::mem::take(&mut outcome.differences);
        differences.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(Compared {
            compared: outcome.compared,
            identical: outcome.identical,
            differences,
        })
    }
}

impl FileSink for TreeComparer {
    fn accept(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        let Some(relative) = self.relative(path) else {
            return Ok(());
        };
        if is_generation_file(&relative) || !in_scope(&self.scope, &relative) {
            return Ok(());
        }
        let difference = match self.tree()?.get(&relative) {
            None => Some(Difference::OnlyInState),
            Some(TreeFile {
                sha256: Some(known),
            }) if *known == sha256_hex(bytes) => None,
            Some(_) => {
                let tree_path = relative
                    .split('/')
                    .fold(self.tree_root.clone(), |path, part| path.join(part));
                let tree_bytes = fs::read(&tree_path)
                    .with_context(|| format!("failed to read {}", tree_path.display()))?;
                compare_content(&relative, bytes, &tree_bytes)
                    .map(|leaves| Difference::Changed { leaves })
            }
        };
        let mut outcome = self
            .outcome
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        outcome.seen.insert(relative.clone());
        outcome.compared += 1;
        match difference {
            Some(difference) => outcome.differences.push(FileDifference {
                path: relative,
                difference,
            }),
            None => outcome.identical += 1,
        }
        Ok(())
    }
}

/// A path as a person writes it: without the `\\?\` of a canonical Windows path.
pub(super) fn display_path(path: &Path) -> String {
    let text = path.display().to_string();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// `ConfigDumpInfo.xml` holds the generation ids of a configuration: no stage
/// keeps them, so it is never compared.
fn is_generation_file(relative: &str) -> bool {
    relative.eq_ignore_ascii_case("ConfigDumpInfo.xml")
}

/// Whether a file of the tree is part of the configuration -- the export
/// would write it -- rather than something else that lives beside it (a
/// readme, a version control folder's leftovers): `Configuration.xml`, a
/// metadata collection's folder, the configuration's own `Ext`.
fn is_configuration_file(relative: &str) -> bool {
    if is_generation_file(relative) {
        return false;
    }
    if relative.eq_ignore_ascii_case("Configuration.xml") {
        return true;
    }
    let Some((first, _)) = relative.split_once('/') else {
        return false;
    };
    first.eq_ignore_ascii_case("Ext") || is_metadata_collection(first)
}

pub(super) fn normalize_prefix(prefix: &str) -> String {
    prefix.replace('\\', "/").trim_matches('/').to_string()
}

/// Whether a relative path is inside the paths the stage was asked for
/// (`scan_sources_with_prefixes` semantics: the folder, or the object's XML
/// beside it). No prefix: everything.
pub(super) fn in_scope(scope: &[String], relative: &str) -> bool {
    if scope.is_empty() {
        return true;
    }
    let lower = relative.to_ascii_lowercase();
    scope.iter().any(|prefix| {
        let prefix = prefix.to_ascii_lowercase();
        lower == prefix
            || lower == format!("{prefix}.xml")
            || lower.starts_with(&format!("{prefix}/"))
    })
}

/// How the content of a file is compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContentKind {
    Xml,
    Text,
    Binary,
}

fn content_kind(relative: &str) -> ContentKind {
    let extension = relative
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "xml" | "xsd" | "wsdl" => ContentKind::Xml,
        "bsl" | "txt" | "html" | "htm" | "css" | "js" => ContentKind::Text,
        _ => ContentKind::Binary,
    }
}

/// `None` when the two contents are the same file; otherwise where they
/// differ (a few lines of text for a message).
fn compare_content(relative: &str, state: &[u8], tree: &[u8]) -> Option<Vec<String>> {
    if state == tree {
        return None;
    }
    match content_kind(relative) {
        ContentKind::Xml => compare_xml(state, tree),
        ContentKind::Text => compare_text(state, tree),
        ContentKind::Binary => Some(vec![format!(
            "содержимое отличается (в дереве {} байт, в собранной конфигурации {})",
            tree.len(),
            state.len()
        )]),
    }
}

fn compare_xml(state: &[u8], tree: &[u8]) -> Option<Vec<String>> {
    let tree_values = match parse_indexed_xml_values(tree) {
        Ok(values) => values,
        Err(error) => return Some(vec![format!("XML дерева не разбирается: {error}")]),
    };
    let state_values = match parse_indexed_xml_values(state) {
        Ok(values) => values,
        Err(error) => {
            return Some(vec![format!(
                "XML собранной конфигурации не разбирается: {error}"
            )]);
        }
    };
    let differences = diff_indexed_xml_values(&tree_values, &state_values, &[]);
    if differences.is_empty() {
        return None;
    }
    let described = describe_differences(&tree_values, &state_values, &differences);
    let mut leaves = described
        .iter()
        .take(LISTED_LEAVES)
        .cloned()
        .collect::<Vec<_>>();
    if described.len() > LISTED_LEAVES {
        leaves.push(format!(
            "и ещё различий: {}",
            described.len() - LISTED_LEAVES
        ));
    }
    Some(leaves)
}

/// The differences as a reader counts them:
///
/// - an element only one side has is one difference, named by its `Name` when
///   it has one, not one per value inside it;
/// - a list of names (`ChildObjects/Form`) that differs is one difference per
///   name only one side has, not one per position the others shifted to.
fn describe_differences(
    tree: &BTreeMap<String, String>,
    state: &BTreeMap<String, String>,
    differences: &[SourceDiffLeafDifference],
) -> Vec<String> {
    let mut described = Vec::new();
    let mut elements = BTreeSet::new();
    let mut lists = BTreeSet::new();
    for difference in differences {
        let (only_here, other, here_is_tree) = match difference.kind {
            SourceDiffLeafDifferenceKind::LeftOnly => (tree, state, true),
            SourceDiffLeafDifferenceKind::RightOnly => (state, tree, false),
            SourceDiffLeafDifferenceKind::ValueOrAttrDiff => {
                describe_leaf_or_list(tree, state, difference, &mut lists, &mut described);
                continue;
            }
        };
        match missing_element(&difference.path, other) {
            Some(element) if element != difference.path => {
                // An entry of a list of names (`.../Item[5]`, and the
                // attributes on it) is said with its list, by name.
                if only_here.contains_key(&element) {
                    if let Some(list) = list_of_entry(tree, state, &element) {
                        describe_list(tree, state, list, &mut lists, &mut described);
                        continue;
                    }
                }
                if elements.insert((here_is_tree, element.clone())) {
                    let (has, hasnt) = if here_is_tree {
                        ("в дереве", "в собранной конфигурации")
                    } else {
                        ("в собранной конфигурации", "в дереве")
                    };
                    described.push(format!(
                        "{has} есть элемент {}{}, {hasnt} его нет",
                        readable_path(&element),
                        name_hint(only_here, &element)
                    ));
                }
            }
            _ => describe_leaf_or_list(tree, state, difference, &mut lists, &mut described),
        }
    }
    described
}

/// A leaf that is one entry of a list of names is described with its list,
/// once; any other leaf on its own.
fn describe_leaf_or_list(
    tree: &BTreeMap<String, String>,
    state: &BTreeMap<String, String>,
    difference: &SourceDiffLeafDifference,
    lists: &mut BTreeSet<String>,
    described: &mut Vec<String>,
) {
    match list_of_entry(tree, state, &difference.path) {
        Some(list) => describe_list(tree, state, list, lists, described),
        None => described.push(describe_leaf(difference)),
    }
}

/// The list of names `entry` is an entry of (`.../Item` for `.../Item[5]`),
/// when one of the two sides has at least two such entries: a lone element is
/// not a list.
fn list_of_entry<'a>(
    tree: &BTreeMap<String, String>,
    state: &BTreeMap<String, String>,
    entry: &'a str,
) -> Option<&'a str> {
    let list = list_of(entry)?;
    (list_entries(tree, list)
        .len()
        .max(list_entries(state, list).len())
        >= 2)
        .then_some(list)
}

/// The names a list has on one side only, said once per list.
fn describe_list(
    tree: &BTreeMap<String, String>,
    state: &BTreeMap<String, String>,
    list: &str,
    lists: &mut BTreeSet<String>,
    described: &mut Vec<String>,
) {
    if !lists.insert(list.to_string()) {
        return;
    }
    let (in_tree, in_state) = (list_entries(tree, list), list_entries(state, list));
    let (only_tree, only_state) = (
        multiset_minus(&in_tree, &in_state),
        multiset_minus(&in_state, &in_tree),
    );
    let name = readable_path(list);
    if only_tree.is_empty() && only_state.is_empty() {
        described.push(format!("порядок {name} другой"));
        return;
    }
    for entry in only_tree {
        described.push(format!(
            "в дереве есть {name} «{}», в собранной конфигурации его нет",
            shorten(&entry)
        ));
    }
    for entry in only_state {
        described.push(format!(
            "в собранной конфигурации есть {name} «{}», в дереве его нет",
            shorten(&entry)
        ));
    }
}

/// `A[1]/ChildObjects[1]/Form[2]` -> `A[1]/ChildObjects[1]/Form`: the list a
/// text entry belongs to (not an attribute, not the only entry of its kind
/// with children below it).
fn list_of(path: &str) -> Option<&str> {
    let (list, index) = path.rsplit_once('[')?;
    let index = index.strip_suffix(']')?;
    (!index.is_empty()
        && index.bytes().all(|byte| byte.is_ascii_digit())
        && !list
            .rsplit('/')
            .next()
            .is_some_and(|last| last.starts_with('@')))
    .then_some(list)
}

/// The texts of `list[1]`, `list[2]`, ... in order.
fn list_entries(values: &BTreeMap<String, String>, list: &str) -> Vec<String> {
    let prefix = format!("{list}[");
    let mut entries = values
        .range(prefix.clone()..)
        .take_while(|(key, _)| key.starts_with(&prefix))
        .filter_map(|(key, value)| {
            let index = key[prefix.len()..].strip_suffix(']')?;
            Some((index.parse::<usize>().ok()?, value.clone()))
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries.into_iter().map(|(_, value)| value).collect()
}

/// The entries of `left` that `right` does not account for, each name as many
/// times as `left` has it beyond `right`.
fn multiset_minus(left: &[String], right: &[String]) -> Vec<String> {
    let mut remaining = right.to_vec();
    let mut only = Vec::new();
    for entry in left {
        match remaining.iter().position(|other| other == entry) {
            Some(at) => {
                remaining.swap_remove(at);
            }
            None => only.push(entry.clone()),
        }
    }
    only
}

/// The shortest ancestor of `path` (segment by segment, `path` itself last)
/// that `other` has nothing at or below: the root of the missing subtree.
fn missing_element(path: &str, other: &BTreeMap<String, String>) -> Option<String> {
    let mut end = 0;
    for segment in path.split('/') {
        end += segment.len();
        let prefix = &path[..end];
        let below = format!("{prefix}/");
        let present = other.contains_key(prefix)
            || other
                .range(below.clone()..)
                .next()
                .is_some_and(|(key, _)| key.starts_with(&below));
        if !present {
            return Some(prefix.to_string());
        }
        end += 1;
    }
    None
}

/// ` («Name»)` for the element that has a name of its own.
fn name_hint(values: &BTreeMap<String, String>, element: &str) -> String {
    ["/Properties[1]/Name[1]", "/Name[1]"]
        .iter()
        .find_map(|suffix| values.get(&format!("{element}{suffix}")))
        .map(|name| format!(" («{}»)", shorten(name)))
        .unwrap_or_default()
}

fn describe_leaf(difference: &SourceDiffLeafDifference) -> String {
    let path = readable_path(&difference.path);
    let shown = |value: &Option<String>| {
        value
            .as_deref()
            .map(shorten)
            .map(|value| format!("«{value}»"))
            .unwrap_or_default()
    };
    match difference.kind {
        SourceDiffLeafDifferenceKind::LeftOnly => format!(
            "в дереве есть {path} = {}, в собранной конфигурации его нет",
            shown(&difference.left)
        ),
        SourceDiffLeafDifferenceKind::RightOnly => format!(
            "в собранной конфигурации есть {path} = {}, в дереве его нет",
            shown(&difference.right)
        ),
        SourceDiffLeafDifferenceKind::ValueOrAttrDiff => format!(
            "{path}: в дереве {}, в собранной конфигурации {}",
            shown(&difference.left),
            shown(&difference.right)
        ),
    }
}

/// `MetaDataObject[1]/Catalog[1]/ChildObjects[1]/Attribute[3]/@uuid` ->
/// `MetaDataObject/Catalog/ChildObjects/Attribute[3]/@uuid`: the first
/// sibling needs no number.
fn readable_path(indexed: &str) -> String {
    indexed.replace("[1]", "")
}

fn shorten(value: &str) -> String {
    let single_line = value.replace(['\r', '\n'], " ");
    if single_line.chars().count() <= SHOWN_VALUE_CHARS {
        return single_line;
    }
    let mut cut = single_line
        .chars()
        .take(SHOWN_VALUE_CHARS - 1)
        .collect::<String>();
    cut.push('…');
    cut
}

/// A module or another text file with its byte order mark dropped and its
/// line ends unified.
fn normalize_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

fn compare_text(state: &[u8], tree: &[u8]) -> Option<Vec<String>> {
    let state_text = normalize_text(state);
    let tree_text = normalize_text(tree);
    if state_text == tree_text {
        return None;
    }
    let mut state_lines = state_text.lines();
    let mut tree_lines = tree_text.lines();
    let mut number = 0usize;
    loop {
        number += 1;
        match (tree_lines.next(), state_lines.next()) {
            (Some(tree_line), Some(state_line)) if tree_line == state_line => {}
            (Some(tree_line), Some(state_line)) => {
                return Some(vec![format!(
                    "строка {number}: в дереве «{}», в собранной конфигурации «{}»",
                    shorten(tree_line),
                    shorten(state_line)
                )]);
            }
            (Some(tree_line), None) => {
                return Some(vec![format!(
                    "строка {number}: в дереве «{}», в собранной конфигурации текст кончился",
                    shorten(tree_line)
                )]);
            }
            (None, Some(state_line)) => {
                return Some(vec![format!(
                    "строка {number}: в дереве текст кончился, в собранной конфигурации «{}»",
                    shorten(state_line)
                )]);
            }
            (None, None) => {
                // Same lines, other trailing line ends.
                return Some(vec!["различаются пробелы или концы строк".to_string()]);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The refusal.

/// The message of a refused import: a headline, the files (at most
/// [`LISTED_FILES`]), what the stage can carry and what to do.
fn refusal_text(
    kind: StageKind,
    tree: &Path,
    differences: &[FileDifference],
    compared: usize,
) -> String {
    let mut lines = vec![format!(
        "Загрузка отменена: конфигурация базы после неё не совпала бы с деревом {} (файлов с расхождениями: {} из {}). В ConfigSave ничего не записано.",
        display_path(tree),
        differences.len(),
        compared.max(differences.len())
    )];
    for difference in differences.iter().take(LISTED_FILES) {
        match &difference.difference {
            Difference::Changed { leaves } => {
                if leaves.len() == 1 {
                    lines.push(format!("  {}: {}", difference.path, leaves[0]));
                } else {
                    lines.push(format!("  {}:", difference.path));
                    for leaf in leaves {
                        lines.push(format!("    - {leaf}"));
                    }
                }
            }
            Difference::OnlyInState => lines.push(format!(
                "  {}: файла нет в дереве, а в базе он остался бы (удаление отдельных файлов объекта пока не переносится)",
                difference.path
            )),
            Difference::OnlyInTree => lines.push(format!(
                "  {}: файл есть в дереве, но в собранной конфигурации его нет",
                difference.path
            )),
        }
    }
    if differences.len() > LISTED_FILES {
        lines.push(format!(
            "  ... и ещё файлов: {}",
            differences.len() - LISTED_FILES
        ));
    }
    match kind {
        StageKind::Patch => {
            lines.push(
                "Загрузка в базу, где конфигурация уже есть, собирает из дерева то, что в нём отличается от базы (описания объектов, новые и удалённые объекты, модули, формы, макеты, права ролей, командные интерфейсы), а остальные строки оставляет как в базе; перечисленное выше так не переносится."
                    .to_string(),
            );
            lines.push(
                "Что делать: загрузите эту конфигурацию штатным ibcmd (или Конфигуратором), либо в пустую базу. Ключ --base-free в базу с конфигурацией собирает все строки из дерева заново и теряет то, чего в XML нет (счётчики схем бизнес-процессов, признак «использовать всегда» у констант): применяйте его только к базе, где это допустимо."
                    .to_string(),
            );
        }
        StageKind::BaseFree => lines.push(
            "Собранные из дерева строки не выгружаются обратно в то же дерево: сообщите об этом с перечисленными файлами."
                .to_string(),
        ),
    }
    lines.push(
        "Чтобы всё равно загрузить остальное, добавьте --no-verify (перечисленное выше в базу не попадёт)."
            .to_string(),
    );
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xml(body: &str) -> Vec<u8> {
        format!("\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n{body}").into_bytes()
    }

    #[test]
    fn identical_content_is_the_same_file() {
        assert_eq!(compare_content("Catalogs/A.xml", b"<a/>", b"<a/>"), None);
    }

    #[test]
    fn xml_that_differs_only_in_formatting_is_the_same_file() {
        let state = xml("<Root>\r\n\t<Name>Имя</Name>\r\n\t<Item id=\"1\" kind=\"a\"/>\r\n</Root>");
        let tree = xml("<Root><Name>Имя</Name><Item kind=\"a\" id=\"1\"/></Root>");
        assert_eq!(compare_content("Catalogs/A.xml", &state, &tree), None);
    }

    #[test]
    fn a_lost_element_is_named_by_its_path() {
        let state = xml(
            "<Root><ChildObjects><Attribute><Name>Один</Name></Attribute></ChildObjects></Root>",
        );
        let tree = xml(
            "<Root><ChildObjects><Attribute><Name>Один</Name></Attribute>\
             <Attribute><Name>Два</Name></Attribute></ChildObjects></Root>",
        );
        let leaves = compare_content("Catalogs/A.xml", &state, &tree).unwrap();
        assert_eq!(
            leaves,
            [
                "в дереве есть элемент Root/ChildObjects/Attribute[2] («Два»), в собранной конфигурации его нет"
            ]
        );
    }

    #[test]
    fn an_element_only_one_side_has_is_one_difference_not_one_per_value() {
        let state = xml("<Root><Item id=\"1\"><Name>Один</Name></Item></Root>");
        let tree = xml("<Root><Item id=\"1\"><Name>Один</Name></Item>\
             <Item id=\"2\"><Properties><Name>Два</Name><Kind>k</Kind><Flag>true</Flag></Properties></Item></Root>");
        let leaves = compare_content("x.xml", &state, &tree).unwrap();
        assert_eq!(
            leaves,
            ["в дереве есть элемент Root/Item[2] («Два»), в собранной конфигурации его нет"]
        );
        // The other way round it is the state that has more.
        let leaves = compare_content("x.xml", &tree, &state).unwrap();
        assert_eq!(
            leaves,
            ["в собранной конфигурации есть элемент Root/Item[2] («Два»), в дереве его нет"]
        );
    }

    #[test]
    fn a_removed_list_entry_is_named_not_every_position_that_shifted() {
        let state = xml(
            "<Root><ChildObjects><Form>Первая</Form><Form>Вторая</Form><Form>Третья</Form></ChildObjects></Root>",
        );
        let tree =
            xml("<Root><ChildObjects><Form>Вторая</Form><Form>Третья</Form></ChildObjects></Root>");
        let leaves = compare_content("Catalogs/A.xml", &state, &tree).unwrap();
        assert_eq!(
            leaves,
            ["в собранной конфигурации есть Root/ChildObjects/Form «Первая», в дереве его нет"]
        );
        // The other way, and a replaced name.
        let leaves = compare_content("Catalogs/A.xml", &tree, &state).unwrap();
        assert_eq!(
            leaves,
            ["в дереве есть Root/ChildObjects/Form «Первая», в собранной конфигурации его нет"]
        );
        let renamed =
            xml("<Root><ChildObjects><Form>Вторая</Form><Form>Новая</Form></ChildObjects></Root>");
        let leaves = compare_content("Catalogs/A.xml", &tree, &renamed).unwrap();
        assert_eq!(
            leaves,
            [
                "в дереве есть Root/ChildObjects/Form «Новая», в собранной конфигурации его нет",
                "в собранной конфигурации есть Root/ChildObjects/Form «Третья», в дереве его нет",
            ]
        );
    }

    #[test]
    fn a_list_entry_with_an_attribute_is_named_once() {
        let state = xml(
            "<Root><Content><Item kind=\"ref\">Первая</Item><Item kind=\"ref\">Вторая</Item></Content></Root>",
        );
        let tree = xml(
            "<Root><Content><Item kind=\"ref\">Первая</Item><Item kind=\"ref\">Вторая</Item>\
             <Item kind=\"ref\">Третья</Item></Content></Root>",
        );
        let leaves = compare_content("Subsystems/A.xml", &state, &tree).unwrap();
        assert_eq!(
            leaves,
            ["в дереве есть Root/Content/Item «Третья», в собранной конфигурации его нет"]
        );
        let leaves = compare_content("Subsystems/A.xml", &tree, &state).unwrap();
        assert_eq!(
            leaves,
            ["в собранной конфигурации есть Root/Content/Item «Третья», в дереве его нет"]
        );
    }

    #[test]
    fn a_reordered_list_says_so() {
        let state = xml("<Root><ChildObjects><Form>А</Form><Form>Б</Form></ChildObjects></Root>");
        let tree = xml("<Root><ChildObjects><Form>Б</Form><Form>А</Form></ChildObjects></Root>");
        let leaves = compare_content("Catalogs/A.xml", &state, &tree).unwrap();
        assert_eq!(leaves, ["порядок Root/ChildObjects/Form другой"]);
    }

    #[test]
    fn a_single_missing_value_is_named_with_its_value() {
        let state = xml("<Root><Properties><Name>A</Name></Properties></Root>");
        let tree =
            xml("<Root><Properties><Name>A</Name><Comment>note</Comment></Properties></Root>");
        let leaves = compare_content("x.xml", &state, &tree).unwrap();
        assert_eq!(
            leaves,
            ["в дереве есть Root/Properties/Comment = «note», в собранной конфигурации его нет"]
        );
    }

    #[test]
    fn the_verbatim_prefix_of_a_windows_path_is_not_shown() {
        assert_eq!(display_path(Path::new(r"\\?\F:\tree")), r"F:\tree");
        assert_eq!(display_path(Path::new(r"F:\tree")), r"F:\tree");
    }

    #[test]
    fn a_changed_value_shows_both_values() {
        let state = xml("<Root><Version>3.1.11.466</Version></Root>");
        let tree = xml("<Root><Version>3.1.11.467</Version></Root>");
        let leaves = compare_content("Configuration.xml", &state, &tree).unwrap();
        assert_eq!(
            leaves,
            ["Root/Version: в дереве «3.1.11.467», в собранной конфигурации «3.1.11.466»"]
        );
    }

    #[test]
    fn many_differences_are_counted_beyond_the_listed_ones() {
        let state = xml("<Root><A>1</A><B>1</B><C>1</C><D>1</D><E>1</E></Root>");
        let tree = xml("<Root><A>2</A><B>2</B><C>2</C><D>2</D><E>2</E></Root>");
        let leaves = compare_content("x.xml", &state, &tree).unwrap();
        assert_eq!(leaves.len(), LISTED_LEAVES + 1);
        assert_eq!(leaves[LISTED_LEAVES], "и ещё различий: 2");
    }

    #[test]
    fn an_xml_that_does_not_parse_is_a_difference() {
        let leaves = compare_content("x.xml", b"<a><b></a>", b"<a/>").unwrap();
        assert!(leaves[0].contains("не разбирается"), "{leaves:?}");
    }

    #[test]
    fn a_module_is_compared_without_its_line_ends_and_byte_order_mark() {
        let state = "\u{feff}Процедура А()\r\nКонецПроцедуры\r\n".as_bytes();
        let tree = "Процедура А()\nКонецПроцедуры\n".as_bytes();
        assert_eq!(compare_content("Ext/Module.bsl", state, tree), None);
    }

    #[test]
    fn a_module_that_differs_names_the_first_line() {
        let state = "Процедура А()\r\nКонецПроцедуры\r\n".as_bytes();
        let tree = "Процедура А()\r\n// новая строка\r\nКонецПроцедуры\r\n".as_bytes();
        let leaves = compare_content("Ext/Module.bsl", state, tree).unwrap();
        assert_eq!(
            leaves,
            ["строка 2: в дереве «// новая строка», в собранной конфигурации «КонецПроцедуры»"]
        );
    }

    #[test]
    fn a_binary_file_is_compared_to_the_byte() {
        let leaves = compare_content("Ext/Picture/Picture.png", b"\x89PNG1", b"\x89PNG22").unwrap();
        assert!(leaves[0].contains("в дереве 6 байт"), "{leaves:?}");
    }

    #[test]
    fn only_a_configurations_own_files_count_when_the_tree_has_more() {
        for (path, configuration) in [
            ("Configuration.xml", true),
            ("Catalogs/A.xml", true),
            ("Catalogs/A/Ext/ObjectModule.bsl", true),
            ("Ext/HomePageWorkArea.xml", true),
            ("Languages/Русский.xml", true),
            ("ConfigDumpInfo.xml", false),
            ("README.md", false),
            ("notes/todo.txt", false),
        ] {
            assert_eq!(is_configuration_file(path), configuration, "{path}");
        }
    }

    #[test]
    fn the_scope_is_the_prefix_folder_and_the_objects_xml_beside_it() {
        let scope = vec![normalize_prefix("Catalogs\\Products/")];
        assert!(in_scope(&scope, "Catalogs/Products.xml"));
        assert!(in_scope(&scope, "Catalogs/Products/Ext/ObjectModule.bsl"));
        assert!(in_scope(&scope, "catalogs/products/Forms/F.xml"));
        assert!(!in_scope(&scope, "Catalogs/ProductsOld.xml"));
        assert!(!in_scope(&scope, "Documents/Sale.xml"));
        assert!(in_scope(&[], "Documents/Sale.xml"));
    }

    fn comparer(tree_files: &[(&str, &[u8])], scope: &[&str]) -> (TreeComparer, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-stage-guard-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        let mut files = Vec::new();
        for (relative, bytes) in tree_files {
            let path = relative
                .split('/')
                .fold(root.clone(), |path, part| path.join(part));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, bytes).unwrap();
            files.push(crate::source::SourceFile {
                path: relative.to_string(),
                size_bytes: bytes.len() as u64,
                sha256: sha256_hex(bytes),
                kind: crate::source::SourceKind::Other,
                xml_root: None,
                object_hint: None,
            });
        }
        let manifest = SourceManifest {
            root: root.clone(),
            generated_at_unix: 0,
            files,
        };
        let scope = scope
            .iter()
            .map(|prefix| prefix.to_string())
            .collect::<Vec<_>>();
        let listed = manifest
            .files
            .iter()
            .map(|file| (file.path.clone(), Some(file.sha256.clone())))
            .collect();
        let comparer = TreeComparer::new(&root.join("virtual"), &root, listed, &scope);
        (comparer, root)
    }

    fn produce(comparer: &TreeComparer, relative: &str, bytes: &[u8]) {
        let path = relative
            .split('/')
            .fold(comparer.output_root.clone(), |path, part| path.join(part));
        comparer.accept(&path, bytes).unwrap();
    }

    #[test]
    fn the_comparer_sorts_out_identical_changed_missing_and_extra_files() {
        let (comparer, root) = comparer(
            &[
                ("Configuration.xml", b"<a/>"),
                ("Catalogs/A.xml", b"<a><x>1</x></a>"),
                ("Catalogs/B.xml", b"<b/>"),
                ("Catalogs/Gone.xml", b"<gone/>"),
                ("ConfigDumpInfo.xml", b"<ids old/>"),
                ("README.md", b"not part of the configuration"),
            ],
            &[],
        );
        produce(&comparer, "Configuration.xml", b"<a/>");
        produce(&comparer, "Catalogs/A.xml", b"<a><x>2</x></a>");
        produce(&comparer, "Catalogs/B.xml", b"<b/>");
        produce(&comparer, "Catalogs/New.xml", b"<new/>");
        produce(&comparer, "ConfigDumpInfo.xml", b"<ids new/>");
        let result = comparer.finish().unwrap();
        assert_eq!(result.compared, 4);
        assert_eq!(result.identical, 2);
        let listed = result
            .differences
            .iter()
            .map(|difference| {
                let what = match &difference.difference {
                    Difference::Changed { .. } => "changed",
                    Difference::OnlyInState => "only in state",
                    Difference::OnlyInTree => "only in tree",
                };
                format!("{} {}", what, difference.path)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            listed,
            [
                "changed Catalogs/A.xml",
                "only in tree Catalogs/Gone.xml",
                "only in state Catalogs/New.xml",
            ]
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_file_without_a_known_hash_is_read_and_compared() {
        let (_, root) = comparer(
            &[
                ("Catalogs/A.xml", b"<a><x>1</x></a>"),
                ("Catalogs/B.xml", b"<b/>"),
            ],
            &[],
        );
        // The stage only listed the tree: no hashes.
        let listed = ["Catalogs/A.xml", "Catalogs/B.xml"]
            .iter()
            .map(|relative| (relative.to_string(), None))
            .collect();
        let comparer = TreeComparer::new(&root.join("virtual"), &root, listed, &[]);
        produce(&comparer, "Catalogs/A.xml", b"<a><x>2</x></a>");
        produce(&comparer, "Catalogs/B.xml", b"<b/>");
        let result = comparer.finish().unwrap();
        assert_eq!(result.compared, 2);
        assert_eq!(result.identical, 1);
        assert_eq!(result.differences.len(), 1);
        assert_eq!(result.differences[0].path, "Catalogs/A.xml");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_walked_tree_is_hashed_beside_the_export_and_compared() {
        let (_, root) = comparer(
            &[
                ("Configuration.xml", b"<a/>"),
                ("Catalogs/A.xml", b"<a><x>1</x></a>"),
                ("Catalogs/Gone.xml", b"<gone/>"),
                ("README.md", b"not part of the configuration"),
            ],
            &[],
        );
        let walked = source_listing::walk(&root);
        let comparer = TreeComparer::hashing(&root.join("virtual"), &root, walked.files, &[]);
        produce(&comparer, "Configuration.xml", b"<a/>");
        produce(&comparer, "Catalogs/A.xml", b"<a><x>2</x></a>");
        let result = comparer.finish().unwrap();
        assert_eq!(result.compared, 2);
        assert_eq!(result.identical, 1);
        let listed = result
            .differences
            .iter()
            .map(|difference| difference.path.as_str())
            .collect::<Vec<_>>();
        // The readme is not a file of the configuration, the missing one is.
        assert_eq!(listed, ["Catalogs/A.xml", "Catalogs/Gone.xml"]);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn the_folders_of_the_8_5_kinds_are_configuration_files_too() {
        // PaletteColors is a top-level folder of a 2.21 tree; a base-free stage lists the walk of the
        // tree, and a folder the listing does not know left the export's files "in the database".
        let files: &[(&str, &[u8])] = &[
            ("Configuration.xml", b"<a/>"),
            ("PaletteColors/ВниманиеБИПЦветФона.xml", b"<p1/>"),
            ("PaletteColors/НеверноеЗначениеЦветФона.xml", b"<p2/>"),
            ("ExternalDataSources/Source.xml", b"<s/>"),
            ("Bots/Bot.xml", b"<b/>"),
            ("IntegrationServices/Service.xml", b"<i/>"),
        ];
        let (_, root) = comparer(files, &[]);
        let walked = source_listing::walk(&root);
        let comparer = TreeComparer::hashing(&root.join("virtual"), &root, walked.files, &[]);
        for (relative, bytes) in files {
            produce(&comparer, relative, bytes);
        }
        let result = comparer.finish().unwrap();
        assert_eq!(result.compared, 6);
        assert_eq!(result.identical, 6);
        assert!(result.differences.is_empty(), "{:?}", result.differences);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn new_root_kinds_cannot_bypass_the_guard_when_missing_or_changed() {
        for folder in [
            "PaletteColors",
            "Bots",
            "IntegrationServices",
            "ExternalDataSources",
        ] {
            let changed = format!("{folder}/Changed.xml");
            let missing = format!("{folder}/Missing.xml");
            let extra = format!("{folder}/Extra.xml");
            let (_, root) = comparer(&[(&changed, b"<old/>"), (&missing, b"<missing/>")], &[]);
            let walked = source_listing::walk(&root);
            let comparer = TreeComparer::hashing(&root.join("virtual"), &root, walked.files, &[]);
            produce(&comparer, &changed, b"<new/>");
            produce(&comparer, &extra, b"<extra/>");
            let result = comparer.finish().unwrap();
            assert_eq!(result.compared, 2, "{folder}");
            assert_eq!(result.identical, 0, "{folder}");
            assert_eq!(result.differences.len(), 3, "{folder}");
            assert!(
                result.differences.iter().any(
                    |d| d.path == changed && matches!(d.difference, Difference::Changed { .. })
                ),
                "{folder}"
            );
            assert!(
                result
                    .differences
                    .iter()
                    .any(|d| d.path == missing && matches!(d.difference, Difference::OnlyInTree)),
                "{folder}"
            );
            assert!(
                result
                    .differences
                    .iter()
                    .any(|d| d.path == extra && matches!(d.difference, Difference::OnlyInState)),
                "{folder}"
            );
            fs::remove_dir_all(root).ok();
        }
    }

    #[test]
    fn a_path_prefix_limits_the_comparison() {
        let (comparer, root) = comparer(
            &[
                ("Catalogs/A.xml", b"<a/>"),
                ("Catalogs/A/Ext/ObjectModule.bsl", b"old"),
                ("Documents/D.xml", b"<d/>"),
            ],
            &["Catalogs/A"],
        );
        produce(&comparer, "Catalogs/A.xml", b"<a/>");
        produce(&comparer, "Catalogs/A/Ext/ObjectModule.bsl", b"new");
        // Outside the prefix: neither compared nor missed.
        produce(&comparer, "Documents/D.xml", b"<other/>");
        let result = comparer.finish().unwrap();
        assert_eq!(result.compared, 2);
        assert_eq!(result.differences.len(), 1);
        assert_eq!(
            result.differences[0].path,
            "Catalogs/A/Ext/ObjectModule.bsl"
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn the_refusal_lists_the_files_and_says_what_to_do() {
        let differences = vec![
            FileDifference {
                path: "Catalogs/A.xml".to_string(),
                difference: Difference::Changed {
                    leaves: vec![
                        "Root/Version: в дереве «2», в собранной конфигурации «1»".to_string(),
                    ],
                },
            },
            FileDifference {
                path: "Catalogs/Gone.xml".to_string(),
                difference: Difference::OnlyInState,
            },
        ];
        let text = refusal_text(StageKind::Patch, Path::new("C:\\tree"), &differences, 100);
        let lines = text.lines().collect::<Vec<_>>();
        assert!(lines[0].starts_with("Загрузка отменена:"), "{text}");
        assert!(
            lines[0].contains("файлов с расхождениями: 2 из 100"),
            "{text}"
        );
        assert!(
            lines[0].ends_with("В ConfigSave ничего не записано."),
            "{text}"
        );
        assert!(
            lines[1].starts_with("  Catalogs/A.xml: Root/Version"),
            "{text}"
        );
        assert!(
            lines[2].starts_with("  Catalogs/Gone.xml: файла нет в дереве"),
            "{text}"
        );
        assert!(text.contains("штатным ibcmd"), "{text}");
        assert!(text.contains("--no-verify"), "{text}");
    }

    #[test]
    fn a_long_list_of_files_is_cut() {
        let differences = (0..LISTED_FILES + 7)
            .map(|index| FileDifference {
                path: format!("Catalogs/C{index:03}.xml"),
                difference: Difference::OnlyInTree,
            })
            .collect::<Vec<_>>();
        let text = refusal_text(StageKind::Patch, Path::new("t"), &differences, 1000);
        assert!(text.contains("  Catalogs/C024.xml"), "{text}");
        assert!(!text.contains("Catalogs/C025.xml"), "{text}");
        assert!(text.contains("... и ещё файлов: 7"), "{text}");
    }

    // -----------------------------------------------------------------
    // The guard end to end on a small native configuration of the repository's
    // evidence (two catalogs with forms and modules, a report with a schema):
    // the rows its base-free stage builds are exported by the model into the
    // comparer, with and without changes the stage does not carry.

    use std::collections::BTreeMap;

    use super::super::empty_stage::prepare_empty_stage;

    fn scratch(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "ibcmd-rs-stage-guard-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/native-evidence/8.3.27.2214/form-usual-group-options/seed-plain")
    }

    /// The object module of a catalog of the fixture.
    const MODULE: &str = "Catalogs/CorpusList/Ext/ObjectModule.bsl";

    fn at(root: &Path, relative: &str) -> PathBuf {
        relative
            .split('/')
            .fold(root.to_path_buf(), |path, part| path.join(part))
    }

    fn copy_tree(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    /// The rows the base-free stage of a tree builds: file name -> stored bytes.
    fn stage_rows(tree: &Path) -> BTreeMap<String, Vec<u8>> {
        let stage = prepare_empty_stage(tree, Some("2.20")).unwrap();
        let failures = stage.failures().collect::<Vec<_>>();
        assert!(failures.is_empty(), "{failures:#?}");
        stage
            .rows()
            .map(|row| (row.file_name.clone(), row.blob.clone()))
            .collect()
    }

    fn staged(rows: &BTreeMap<String, Vec<u8>>) -> Vec<StagedRow<'_>> {
        rows.iter()
            .map(|(file_name, bytes)| StagedRow { file_name, bytes })
            .collect()
    }

    /// Writes the files the export hands it into a folder: the platform's
    /// export of the staged state, as a tree to compare with.
    struct WriteTo {
        output_root: PathBuf,
        folder: PathBuf,
    }

    impl FileSink for WriteTo {
        fn accept(&self, path: &Path, bytes: &[u8]) -> Result<()> {
            let target = self.folder.join(path.strip_prefix(&self.output_root)?);
            fs::create_dir_all(target.parent().unwrap())?;
            fs::write(target, bytes)?;
            Ok(())
        }
    }

    /// The tree the model exports for `rows` alone.
    fn exported_tree(rows: &BTreeMap<String, Vec<u8>>, folder: &Path) {
        let output_root = std::env::temp_dir().join("ibcmd-rs-verified-state");
        let sink = Arc::new(WriteTo {
            output_root: output_root.clone(),
            folder: folder.to_path_buf(),
        });
        export_staged_state(
            StateBase::Nothing,
            &staged(rows),
            &[],
            InfobaseConfigSourceVersion::V2_20,
            &output_root,
            sink,
        )
        .unwrap();
    }

    fn request(tree: &Path, kind: StageKind) -> GuardRequest<'_> {
        GuardRequest {
            kind,
            source_root: tree,
            tree: TreeFiles::Unlisted,
            removed: &[],
            path_prefix: &[],
            source_version: Some(InfobaseConfigSourceVersion::V2_20),
        }
    }

    fn refusal(result: Result<StageVerification>) -> String {
        let error = result.expect_err("the stage should be refused");
        error
            .downcast_ref::<StageRefused>()
            .unwrap_or_else(|| panic!("not a refusal: {error:#}"))
            .to_string()
    }

    fn a_stage_that_exports_back_to_its_tree_passes() {
        let work = scratch("passes");
        let rows = stage_rows(&fixture());
        let tree = work.join("tree");
        exported_tree(&rows, &tree);
        assert!(tree.join("Configuration.xml").is_file());
        assert!(at(&tree, MODULE).is_file());
        let checked = verify_staged_state(
            &request(&tree, StageKind::BaseFree),
            StateBase::Nothing,
            &staged(&rows),
        )
        .unwrap();
        assert!(checked.checked_files >= 15, "{checked:?}");
        assert_eq!(checked.checked_files, checked.identical_files);
        assert_eq!(checked.state_rows, rows.len());
        fs::remove_dir_all(work).ok();
    }

    fn a_change_the_stage_does_not_carry_is_refused_and_named() {
        let work = scratch("lost");
        let rows = stage_rows(&fixture());
        let tree = work.join("tree");
        exported_tree(&rows, &tree);
        // The tree changes a property of the configuration and a module; the
        // stage still holds the old rows (a patch stage that left both out).
        let configuration = tree.join("Configuration.xml");
        let text = fs::read_to_string(&configuration).unwrap();
        assert!(text.contains("<Comment/>"));
        fs::write(
            &configuration,
            text.replacen("<Comment/>", "<Comment>A changed comment</Comment>", 1),
        )
        .unwrap();
        let module = at(&tree, MODULE);
        let mut source = fs::read_to_string(&module).unwrap();
        source.push_str("\r\n// added\r\n");
        fs::write(&module, source).unwrap();

        let message = refusal(verify_staged_state(
            &request(&tree, StageKind::Patch),
            StateBase::Nothing,
            &staged(&rows),
        ));
        assert!(message.starts_with("Загрузка отменена:"), "{message}");
        assert!(message.contains("файлов с расхождениями: 2"), "{message}");
        assert!(message.contains("  Configuration.xml:"), "{message}");
        assert!(message.contains("A changed comment"), "{message}");
        assert!(
            message.contains("  Catalogs/CorpusList/Ext/ObjectModule.bsl: строка"),
            "{message}"
        );
        assert!(
            message.contains("В ConfigSave ничего не записано."),
            "{message}"
        );
        fs::remove_dir_all(work).ok();
    }

    fn a_change_the_stage_carries_passes() {
        let work = scratch("carried");
        // The database holds the fixture; the tree changes a module and the
        // stage stages the rows rebuilt from the changed tree.
        let stored = stage_rows(&fixture());
        let stored_folder = work.join("stored");
        fs::create_dir_all(&stored_folder).unwrap();
        for (file_name, bytes) in &stored {
            fs::write(stored_folder.join(format!("{file_name}__part0.bin")), bytes).unwrap();
        }
        let changed = work.join("changed");
        copy_tree(&fixture(), &changed);
        fs::write(
            at(&changed, MODULE),
            "Procedure Changed() Export\r\n\tReturn;\r\nEndProcedure",
        )
        .unwrap();
        let rebuilt = stage_rows(&changed);
        let only_changed = rebuilt
            .iter()
            .filter(|(name, bytes)| stored.get(*name) != Some(bytes))
            .map(|(name, bytes)| (name.clone(), bytes.clone()))
            .collect::<BTreeMap<_, _>>();
        assert!(!only_changed.is_empty(), "the module edit changes a row");
        assert!(only_changed.len() < rebuilt.len() / 2, "and only a few");

        let tree = work.join("tree");
        exported_tree(&rebuilt, &tree);
        // With the changed rows staged over the stored ones the state is the
        // changed tree.
        verify_staged_state(
            &request(&tree, StageKind::Patch),
            StateBase::Folder(&stored_folder),
            &staged(&only_changed),
        )
        .unwrap();
        // Without them the stored module shows.
        let message = refusal(verify_staged_state(
            &request(&tree, StageKind::Patch),
            StateBase::Folder(&stored_folder),
            &[],
        ));
        assert!(message.contains("ObjectModule.bsl"), "{message}");
        fs::remove_dir_all(work).ok();
    }

    fn files_only_one_side_has_are_refused() {
        let work = scratch("sides");
        let rows = stage_rows(&fixture());
        let tree = work.join("tree");
        exported_tree(&rows, &tree);
        // A file the state lacks (a new object the stage does not carry) and a
        // file only the state has (an object the tree dropped).
        fs::write(at(&tree, "Catalogs/Extra.xml"), "<MetaDataObject/>").unwrap();
        fs::remove_file(at(&tree, "Languages/Русский.xml")).unwrap();
        let message = refusal(verify_staged_state(
            &request(&tree, StageKind::Patch),
            StateBase::Nothing,
            &staged(&rows),
        ));
        assert!(
            message.contains(
                "  Catalogs/Extra.xml: файл есть в дереве, но в собранной конфигурации его нет"
            ),
            "{message}"
        );
        assert!(
            message.contains("  Languages/Русский.xml: файла нет в дереве, а в базе он остался бы"),
            "{message}"
        );
        fs::remove_dir_all(work).ok();
    }

    fn a_tree_that_only_formats_differently_passes() {
        let work = scratch("format");
        let rows = stage_rows(&fixture());
        let tree = work.join("tree");
        exported_tree(&rows, &tree);
        // Line ends, indentation and the byte order mark are the tree's own.
        for relative in ["Configuration.xml", "Catalogs/CorpusList.xml"] {
            let path = at(&tree, relative);
            let text = fs::read_to_string(&path).unwrap();
            let text = text
                .trim_start_matches('\u{feff}')
                .replace("\r\n", "\n")
                .replace('\t', "  ");
            fs::write(&path, text).unwrap();
        }
        let module = at(&tree, MODULE);
        let text = fs::read_to_string(&module).unwrap();
        fs::write(
            &module,
            text.trim_start_matches('\u{feff}').replace("\r\n", "\n"),
        )
        .unwrap();
        verify_staged_state(
            &request(&tree, StageKind::Patch),
            StateBase::Nothing,
            &staged(&rows),
        )
        .unwrap();
        fs::remove_dir_all(work).ok();
    }

    /// The base-free stage switches process-wide writers into base-free mode
    /// and never back, which other tests of this binary would notice: the
    /// scenarios run in a process of their own.
    #[test]
    fn end_to_end_on_a_small_native_configuration() {
        const INNER: &str = "IBCMD_RS_STAGE_GUARD_E2E";
        if std::env::var_os(INNER).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "mssql::stage_guard::tests::end_to_end_on_a_small_native_configuration",
                    "--test-threads=1",
                    "--nocapture",
                ])
                .env(INNER, "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        a_stage_that_exports_back_to_its_tree_passes();
        a_change_the_stage_does_not_carry_is_refused_and_named();
        a_change_the_stage_carries_passes();
        files_only_one_side_has_are_refused();
        a_tree_that_only_formats_differently_passes();
    }

    #[test]
    fn the_environment_overrides_the_command_line() {
        assert!(wanted_with(None, true));
        assert!(!wanted_with(None, false));
        assert!(!wanted_with(Some("0"), true));
        assert!(!wanted_with(Some(" off "), true));
        assert!(wanted_with(Some("1"), false));
        assert!(wanted_with(Some("on"), false));
        assert!(wanted_with(Some("perhaps"), true));
        assert!(!wanted_with(Some("perhaps"), false));
    }
}
