//! Base-free staging: the complete Config row set an EMPTY infobase needs,
//! built from the source tree alone.
//!
//! Every metadata XML of the tree (nested subsystems, recalculations, forms
//! and templates included) gives its descriptor row through
//! `metadata_model::compile_descriptor` and its bodies through the loader's
//! own body writers, run with `BASE_FREE_STAGE` set so that a writer which
//! would patch a base row fails naming that row instead of querying. The
//! configuration adds `root`, `version` and a fresh `versions`.
//!
//! Two consumers share it: `audit-empty-stage`, which compares the row set
//! with a database's stored Config rows without touching any database, and
//! `mssql-stage-source-objects --base-free`, which loads it into ConfigSave
//! with SQL that does not read Config at all.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result, anyhow, bail};
use rayon::prelude::*;
use serde::Serialize;

use super::stage_timing;
use super::{
    BASE_FREE_MISSING_ROW, BASE_FREE_STAGE, BulkStageRow, GeneratedBlobReport, MetadataBodyFamily,
    StageSourceObjectsReport, StagedMetadataBodyReport, StagedMetadataObjectReport,
    StorageTableManifest, build_bulk_stage_prepare_sql, bulk_stage_paths,
    bulk_stage_rows_file_needed, bulk_stage_table_name, command_interface_body_suffix,
    infer_common_module_text_path, mssql_compile_axes_from_metadata_xml,
    pack_module_body_source_with_source, prepare_metadata_body_family, quote_ident,
    require_non_lab_confirmation, resolve_sqlcmd_password, run_bulk_stage,
    source_module_body_path_with_source, source_xml_version_from_bytes, stage_sql,
    storage_table_stats, write_bulk_stage_rows,
};
use crate::cli::MssqlStageSourceObjectsArgs;
use crate::compiler::families::assets::SourceAssetRegistry;
use crate::metadata_model::audit::{brace_path_at, is_descriptor_xml};
use crate::metadata_model::bodies_rows::{StubPart, stub_row_text};
use crate::metadata_model::root::{
    ConfigurationFacts, MODULE_GROUP_CLASS_ID, configuration_facts, root_row, version_row,
    versions_row,
};
use crate::metadata_model::{DescriptorContext, compile_descriptor};
use crate::module_blob::{
    SimpleMetadataXmlProperties, deflate_raw, hex_sha256, inflate_raw,
    parse_simple_metadata_xml_properties,
};
use crate::parallel;
use crate::source_listing::{self, SourceListing};
use crate::sql::SqlExec;

/// One row of the stage: its Config file name and stored bytes.
#[derive(Debug, Clone)]
pub(crate) struct EmptyStageRow {
    pub file_name: String,
    /// `descriptor`, a body family (`kind body`, `help`, ...), `module` or
    /// `service`.
    pub family: String,
    /// The file it was compiled from, relative to the tree.
    pub source: String,
    /// The stored bytes: raw deflate of the row text.
    pub blob: Vec<u8>,
    /// The row text itself (BOM included), kept for the audit.
    pub plain: Option<Vec<u8>>,
}

/// A row, or a group of rows, the stage could not produce.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct EmptyStageFailure {
    /// The row it would have been, when known.
    pub file_name: Option<String>,
    pub kind: String,
    pub family: String,
    pub source: String,
    pub error: String,
}

/// One metadata XML with every row it gives.
#[derive(Debug)]
pub(crate) struct EmptyStageObject {
    pub kind: String,
    pub uuid: String,
    pub xml: PathBuf,
    pub relative: String,
    pub properties: Option<SimpleMetadataXmlProperties>,
    pub rows: Vec<EmptyStageRow>,
    pub failures: Vec<EmptyStageFailure>,
}

/// What every row of a tree needs besides its own XML.
pub(crate) struct EmptyStageContext {
    pub root: PathBuf,
    pub version: String,
    pub facts: ConfigurationFacts,
    pub descriptors: DescriptorContext,
    /// The managed-application module group the configuration's own rows
    /// are stored under.
    pub module_group: Option<String>,
    /// The tree's files and folders, listed once (`source_listing::walk`):
    /// every object's writers answer their existence probes from it.
    pub listing: Option<std::sync::Arc<SourceListing>>,
}

impl EmptyStageContext {
    /// `files` is every descriptor XML of the tree, read (`read_descriptor_xmls`);
    /// `listing` the tree's list of files, when it has one.
    pub fn new(
        root: &Path,
        version: Option<&str>,
        files: &[(PathBuf, std::sync::Arc<Vec<u8>>)],
        listing: Option<std::sync::Arc<SourceListing>>,
    ) -> Result<Self> {
        // No base rows exist: every base-row read fails naming its row.
        BASE_FREE_STAGE.store(true, Ordering::Relaxed);
        // Nor any always-used constant: track A compiles each constant with
        // the flag clear, and a constants set is written against that.
        if crate::module_blob::clear_always_used_constants() {
            eprintln!(
                "IBCMD_RS_ALWAYS_USED_CONSTANTS is ignored: it names a target database's flags, and an empty infobase's constants carry none"
            );
        }
        Self::for_objects(root, version, files, listing)
    }

    /// The context for compiling single objects inside a patch stage
    /// (`override_stage`): the same facts about the tree, but none of the
    /// process-wide switches of a whole base-free stage -- the rest of the
    /// stage still has base rows and the target's always-used constants. Each
    /// object is compiled with `SqlExec::detached(BASE_FREE_MISSING_ROW)`, which
    /// its writers take for "no base row".
    pub fn for_objects(
        root: &Path,
        version: Option<&str>,
        files: &[(PathBuf, std::sync::Arc<Vec<u8>>)],
        listing: Option<std::sync::Arc<SourceListing>>,
    ) -> Result<Self> {
        Self::for_objects_with_source(root, version, files, listing, None)
    }

    pub(super) fn for_objects_with_source(
        root: &Path,
        version: Option<&str>,
        files: &[(PathBuf, std::sync::Arc<Vec<u8>>)],
        listing: Option<std::sync::Arc<SourceListing>>,
        source: Option<&crate::module_blob::MetadataSourceContext>,
    ) -> Result<Self> {
        if let Some(source) = source {
            source.require_source_root(root)?;
            source.require_original_unchanged()?;
        }
        let configuration_path = root.join("Configuration.xml");
        let configuration = super::read_stage_source(source, &configuration_path)
            .with_context(|| format!("failed to read {}", configuration_path.display()))?;
        let version = match version {
            Some(version) => version.to_string(),
            None => source_xml_version_from_bytes(&configuration)?
                .ok_or_else(|| anyhow!("Configuration.xml declares no source version"))?,
        };
        let mut facts = configuration_facts(&configuration)?;
        // Body layouts follow the configuration, not the XML dialect: a 2.21
        // tree of an 8.3-compatible configuration stores 8.3 bodies (the
        // layout the registry's `FormLayout::stored` gives it) unless its
        // forms show it is stored the 8.5 way
        // (`metadata_model::common::tree_stores_layout_8_5_1`); the
        // Configuration tuple takes the same layout.
        let platform = crate::platform::of_xml_dialect(&version);
        let xml_2_21 = platform.xml_version() == crate::cli::InfobaseConfigSourceVersion::V2_21;
        let layout_8_5_1 = xml_2_21
            && platform.form_layout() >= crate::platform::FormLayout::V8_5_1
            && match source {
                Some(source) => source.stores_layout_8_5_1()?,
                None => crate::metadata_model::common::tree_stores_layout_8_5_1(root),
            };
        crate::module_blob::XML_2_21_TREE_IN_LAYOUT_8_3
            .store(xml_2_21 && !layout_8_5_1, Ordering::Relaxed);
        if layout_8_5_1 {
            facts.shape = crate::metadata_model::root::ConfigurationShape::V76;
        }
        let mut descriptors = DescriptorContext::with_files(root, &version, files)?;
        if let Some(source) = source {
            descriptors.source = source.clone();
        }
        let module_group = module_group_of(&configuration);
        Ok(Self {
            root: root.to_path_buf(),
            version,
            facts,
            descriptors,
            module_group,
            listing,
        })
    }
}

/// The descriptor XMLs among the files of the tree's walk
/// (`source_listing::walk`): `audit::descriptor_xmls(root)`, the same files in
/// the same (sorted) order. Walking ERP УХ's 140 709 files on one thread took
/// 25-75 s, and a stage used to walk the tree twice (once for the index, once
/// for the objects); the walk lists every folder on a task of its own, once,
/// and its list also answers the existence probes of every object's writers.
pub(super) fn descriptor_xmls_of(root: &Path, files: &[PathBuf]) -> Vec<PathBuf> {
    let mut paths = files
        .iter()
        .filter(|path| {
            path.strip_prefix(root)
                .map(|relative| is_descriptor_xml(&relative.to_string_lossy()))
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

/// Where an object goes in the stage's dispatch order: 0 first. Templates,
/// roles, exchange plans and the configuration hold the stage's longest single
/// rows (ERP УХ: spreadsheets of 15-27 s, roles of 10-15 s, an exchange plan's
/// content), and in tree order the heaviest spreadsheets
/// (`Reports/РегламентированныйОтчетСтатистика…`) came last and ran alone at
/// the end of the stage.
fn dispatch_rank(root: &Path, path: &Path) -> u8 {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let components = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect::<Vec<_>>();
    let heavy = match components.as_slice() {
        [file] => file == "configuration.xml",
        [folder, _] => matches!(
            folder.as_str(),
            "roles" | "commontemplates" | "exchangeplans"
        ),
        [.., folder, _] => folder == "templates",
        [] => false,
    };
    if heavy { 0 } else { 1 }
}

/// `work` over every path on the file-bound pool, the results in `paths`
/// order. The paths are dispatched one at a time in `dispatch_rank` order
/// (tree order within a rank) rather than split into ranges, so a heavy object
/// starts when its turn comes, not when its range does.
pub(super) fn map_heaviest_first<T: Send>(
    root: &Path,
    paths: &[PathBuf],
    work: impl Fn(&Path) -> T + Sync,
) -> Result<Vec<T>> {
    let mut order = (0..paths.len()).collect::<Vec<_>>();
    order.sort_by_key(|&index| dispatch_rank(root, &paths[index]));
    let next = AtomicUsize::new(0);
    let results = paths
        .iter()
        .map(|_| Mutex::new(None))
        .collect::<Vec<Mutex<Option<T>>>>();
    parallel::install_io_bound(|| {
        rayon::scope(|scope| {
            for _ in 0..rayon::current_num_threads() {
                scope.spawn(|_| {
                    loop {
                        let position = next.fetch_add(1, Ordering::Relaxed);
                        let Some(&index) = order.get(position) else {
                            break;
                        };
                        let result = work(&paths[index]);
                        if let Ok(mut slot) = results[index].lock() {
                            *slot = Some(result);
                        }
                    }
                });
            }
        });
    })?;
    results
        .into_iter()
        .enumerate()
        .map(|(index, slot)| {
            slot.into_inner()
                .ok()
                .flatten()
                .ok_or_else(|| anyhow!("no result for {}", paths[index].display()))
        })
        .collect()
}

/// Every descriptor XML of the list, read on the file-bound pool: the index,
/// the descriptors and the name resolvers of every body writer then read them
/// from memory. ERP УХ: 56 758 files, 366 MB.
pub(super) fn read_descriptor_xmls(
    paths: &[PathBuf],
) -> Result<Vec<(PathBuf, std::sync::Arc<Vec<u8>>)>> {
    read_descriptor_xmls_with_source(paths, None)
}

pub(super) fn read_descriptor_xmls_with_source(
    paths: &[PathBuf],
    source: Option<&crate::module_blob::MetadataSourceContext>,
) -> Result<Vec<(PathBuf, std::sync::Arc<Vec<u8>>)>> {
    parallel::install_io_bound(|| {
        paths
            .par_iter()
            .map(|path| {
                super::read_stage_source(source, path)
                    .with_context(|| format!("failed to read {}", path.display()))
                    .map(|bytes| {
                        (
                            path.clone(),
                            match bytes {
                                crate::module_blob::SourceBytes::Shared(bytes) => bytes,
                                crate::module_blob::SourceBytes::Owned(bytes) => {
                                    std::sync::Arc::new(bytes)
                                }
                            },
                        )
                    })
            })
            .collect::<Result<Vec<_>>>()
    })?
}

/// `<xr:ContainedObject>` of the module-group class in `Configuration.xml`.
fn module_group_of(xml: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(xml);
    let class = text.find(&format!("<xr:ClassId>{MODULE_GROUP_CLASS_ID}</xr:ClassId>"))?;
    let rest = &text[class..];
    let open = rest.find("<xr:ObjectId>")? + "<xr:ObjectId>".len();
    let close = rest[open..].find("</xr:ObjectId>")? + open;
    let uuid = rest[open..close].trim();
    (uuid.len() == 36).then(|| uuid.to_ascii_lowercase())
}

fn relative_of(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// The kind a timing line is filed under: a template also names its type.
fn timing_kind(kind: &str, xml: &[u8]) -> String {
    if !matches!(kind, "Template" | "CommonTemplate") {
        return kind.to_string();
    }
    let text = String::from_utf8_lossy(xml);
    let template_type = text
        .find("<TemplateType>")
        .map(|at| &text[at + "<TemplateType>".len()..])
        .and_then(|rest| rest.find("</TemplateType>").map(|end| &rest[..end]))
        .unwrap_or("?");
    format!("{kind}/{template_type}")
}

fn error_text(error: &anyhow::Error) -> String {
    format!("{error:#}")
}

/// The row a failed writer would have written, when the failure names it or
/// the family has one row.
fn failed_row_name(
    error: &str,
    family: MetadataBodyFamily,
    kind: &str,
    uuid: &str,
    module_group: Option<&str>,
) -> Option<String> {
    if let Some(at) = error.find(BASE_FREE_MISSING_ROW) {
        let name = error[at + BASE_FREE_MISSING_ROW.len()..]
            .trim_start()
            .split(|ch: char| ch.is_whitespace() || ch == ':' || ch == ',')
            .next()
            .unwrap_or_default();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    let owner = if kind == "Configuration" {
        module_group.unwrap_or(uuid)
    } else {
        uuid
    };
    let suffix = match family {
        MetadataBodyFamily::KindBody => match kind {
            "Form" | "CommonForm" | "Role" | "Template" | "CommonTemplate" | "CommonPicture"
            | "Style" | "ScheduledJob" | "XDTOPackage" | "WSReference" => "0",
            "ExchangePlan" => "1",
            "Catalog" => "1c",
            "ChartOfCharacteristicTypes" | "BusinessProcess" => "7",
            _ => return None,
        },
        MetadataBodyFamily::Help => SourceAssetRegistry
            .help_suffix(kind)?
            .trim_start_matches('.'),
        MetadataBodyFamily::CommandInterface => command_interface_body_suffix(kind)?,
        MetadataBodyFamily::AdditionalIndexes => match kind {
            "Document" => "3",
            "AccumulationRegister" => "4",
            "Catalog" => "1d",
            _ => return None,
        },
        MetadataBodyFamily::ObjectModules | MetadataBodyFamily::NestedCommandModules => {
            return None;
        }
    };
    Some(format!("{owner}.{suffix}"))
}

pub(super) fn catch<T>(run: impl FnOnce() -> Result<T>) -> Result<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
        Ok(result) => result,
        Err(panic) => {
            let message = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|text| text.to_string()))
                .unwrap_or_default();
            Err(anyhow!("writer panicked: {message}"))
        }
    }
}

/// Every row one metadata XML gives, base-free.
pub(crate) fn prepare_empty_object(
    context: &EmptyStageContext,
    path: &Path,
    keep_plain: bool,
) -> EmptyStageObject {
    let relative = relative_of(&context.root, path);
    // The writers' existence probes answer from the tree's list.
    let _listing = source_listing::install(context.listing.clone());
    let mut object = EmptyStageObject {
        kind: String::new(),
        uuid: String::new(),
        xml: path.to_path_buf(),
        relative: relative.clone(),
        properties: None,
        rows: Vec::new(),
        failures: Vec::new(),
    };
    let fail = |object: &mut EmptyStageObject, family: &str, error: String| {
        object.failures.push(EmptyStageFailure {
            file_name: None,
            kind: object.kind.clone(),
            family: family.to_string(),
            source: relative.clone(),
            error,
        });
    };
    let read_started = stage_timing::start();
    // Read once, up front (`read_descriptor_xmls`); from disk only when not.
    let xml = match context.descriptors.source.read_source(path) {
        Ok(xml) => xml,
        Err(error) => {
            fail(&mut object, "read", error.to_string());
            return object;
        }
    };
    let properties = match parse_simple_metadata_xml_properties(&xml) {
        Ok(properties) => properties,
        Err(error) => {
            fail(&mut object, "parse", error_text(&error));
            return object;
        }
    };
    object.kind = properties.kind.clone();
    object.uuid = properties.uuid.clone();
    let axes = match mssql_compile_axes_from_metadata_xml(&xml) {
        Ok(axes) => axes,
        Err(error) => {
            fail(&mut object, "parse", error_text(&error));
            return object;
        }
    };
    let kind = if stage_timing::enabled() {
        timing_kind(&properties.kind, &xml)
    } else {
        String::new()
    };
    stage_timing::record(read_started, "xml read and parse", &kind, &relative);

    // The descriptor row.
    let started = stage_timing::start();
    let descriptor =
        catch(|| compile_descriptor(&properties.kind, path, &xml, &context.descriptors))
            .and_then(|plain| Ok((deflate_raw(&plain)?, plain)));
    stage_timing::record(started, "descriptor", &kind, &relative);
    match descriptor {
        Ok((blob, plain)) => object.rows.push(EmptyStageRow {
            file_name: properties.uuid.clone(),
            family: "descriptor".to_string(),
            source: relative.clone(),
            blob,
            plain: keep_plain.then_some(plain),
        }),
        Err(error) => object.failures.push(EmptyStageFailure {
            file_name: Some(properties.uuid.clone()),
            kind: properties.kind.clone(),
            family: "descriptor".to_string(),
            source: relative.clone(),
            error: error_text(&error),
        }),
    }

    // The bodies.
    let source = Some(&context.descriptors.source);
    if properties.kind == "CommonModule" {
        let module_path =
            source_module_body_path_with_source(infer_common_module_text_path(path), source);
        let text_path = match module_path {
            Ok(path) => path,
            Err(error) => {
                fail(&mut object, "module source", error_text(&error));
                return object;
            }
        };
        if let Some(text_path) = text_path {
            let body_id = format!("{}.0", properties.uuid);
            let started = stage_timing::start();
            let packed =
                catch(|| pack_module_body_source_with_source(&text_path, &body_id, &axes, source));
            stage_timing::record(started, "module", &kind, &relative);
            match packed {
                Ok(packed) => object.rows.push(EmptyStageRow {
                    file_name: body_id,
                    family: "module".to_string(),
                    source: relative_of(&context.root, &text_path),
                    plain: None,
                    blob: packed.blob,
                }),
                Err(error) => object.failures.push(EmptyStageFailure {
                    file_name: Some(body_id),
                    kind: properties.kind.clone(),
                    family: "module".to_string(),
                    source: relative_of(&context.root, &text_path),
                    error: error_text(&error),
                }),
            }
        }
    } else {
        // Track D: predefined data, flowcharts and aggregates, base-free.
        let started = stage_timing::start();
        track_d_body_rows(context, path, &xml, &properties, &relative, &mut object);
        stage_timing::record(started, "model bodies", &kind, &relative);
        for family in MetadataBodyFamily::ALL {
            if family == MetadataBodyFamily::KindBody
                && crate::metadata_model::bodies_rows::owns_kind_body(&properties.kind, &xml)
            {
                continue;
            }
            let started = stage_timing::start();
            let result = catch(|| {
                prepare_metadata_body_family(
                    family,
                    &SqlExec::detached(BASE_FREE_MISSING_ROW),
                    "",
                    path,
                    &xml,
                    &properties,
                    source,
                    &axes,
                )
            });
            stage_timing::record(started, family.label(), &kind, &relative);
            match result {
                Ok(rows) => object
                    .rows
                    .extend(rows.into_iter().map(|row| EmptyStageRow {
                        file_name: row.body_id,
                        family: family.label().to_string(),
                        source: relative_of(&context.root, &row.path),
                        blob: row.blob,
                        plain: None,
                    })),
                Err(error) => {
                    let error = error_text(&error);
                    object.failures.push(EmptyStageFailure {
                        file_name: failed_row_name(
                            &error,
                            family,
                            &properties.kind,
                            &properties.uuid,
                            context.module_group.as_deref(),
                        ),
                        kind: properties.kind.clone(),
                        family: family.label().to_string(),
                        source: relative.clone(),
                        error,
                    });
                }
            }
        }
    }
    if let Err(error) = context.descriptors.source.require_original_reads() {
        fail(&mut object, "original source", error_text(&error));
    }
    object.properties = Some(properties);
    object
}

/// Track D's body rows of one object (`metadata_model::bodies_rows`).
fn track_d_body_rows(
    context: &EmptyStageContext,
    path: &Path,
    xml: &[u8],
    properties: &SimpleMetadataXmlProperties,
    relative: &str,
    object: &mut EmptyStageObject,
) {
    use crate::metadata_model::bodies_rows::{body_row_suffix, compile_body_rows};
    let result = catch(|| {
        compile_body_rows(&properties.kind, path, xml, &context.descriptors)?
            .into_iter()
            .map(|row| {
                Ok(EmptyStageRow {
                    blob: deflate_raw(&row.text)?,
                    file_name: row.file_name,
                    family: "kind body".to_string(),
                    source: relative_of(&context.root, &row.source),
                    plain: None,
                })
            })
            .collect::<Result<Vec<_>>>()
    });
    match result {
        Ok(rows) => object.rows.extend(rows),
        Err(error) => object.failures.push(EmptyStageFailure {
            file_name: body_row_suffix(&properties.kind)
                .map(|suffix| format!("{}.{suffix}", properties.uuid)),
            kind: properties.kind.clone(),
            family: "kind body".to_string(),
            source: relative.to_string(),
            error: error_text(&error),
        }),
    }
}

/// The service rows: `root`, `version`, and a `versions` naming `names`.
pub(crate) fn service_rows(
    context: &EmptyStageContext,
    names: &[String],
) -> Result<Vec<EmptyStageRow>> {
    let fresh = || uuid::Uuid::new_v4().hyphenated().to_string();
    let mut rows = Vec::with_capacity(3);
    for (file_name, plain) in [
        ("root", root_row(&context.facts)),
        ("version", version_row(&context.facts)?),
        ("versions", versions_row(names, fresh)),
    ] {
        rows.push(EmptyStageRow {
            file_name: file_name.to_string(),
            family: "service".to_string(),
            source: "Configuration.xml".to_string(),
            blob: deflate_raw(&plain)?,
            plain: Some(plain),
        });
    }
    Ok(rows)
}

/// The whole stage, in tree order, then the content-free rows
/// `ConfigDumpInfo.xml` lists, service rows last.
pub(crate) struct EmptyStage {
    pub context: EmptyStageContext,
    pub objects: Vec<EmptyStageObject>,
    pub stubs: StubRows,
    pub service: Vec<EmptyStageRow>,
    /// Every file of the tree, as the stage walked it: what the guard compares
    /// with the export without walking the tree again.
    pub tree_files: Vec<PathBuf>,
}

impl EmptyStage {
    pub fn failures(&self) -> impl Iterator<Item = &EmptyStageFailure> {
        self.objects
            .iter()
            .flat_map(|object| object.failures.iter())
            .chain(self.stubs.failures.iter())
    }

    pub fn rows(&self) -> impl Iterator<Item = &EmptyStageRow> {
        self.objects
            .iter()
            .flat_map(|object| object.rows.iter())
            .chain(self.stubs.rows.iter())
            .chain(self.service.iter())
    }
}

/// The rows of the objects `selected` (descriptor XMLs of the tree) alone,
/// base-free, against the whole tree's context: objects a container does not
/// have yet (`offline_compile`, `cf load` of an added object). Sets
/// `BASE_FREE_STAGE`: no base row is read after it.
pub(crate) fn prepare_empty_objects(
    root: &Path,
    selected: &[PathBuf],
) -> Result<Vec<EmptyStageObject>> {
    let walked = source_listing::walk(root);
    let paths = descriptor_xmls_of(root, &walked.files);
    let files = read_descriptor_xmls(&paths)?;
    let context = EmptyStageContext::new(root, None, &files, walked.listing)?;
    drop(files);
    Ok(selected
        .iter()
        .map(|path| prepare_empty_object(&context, path, false))
        .collect())
}

pub(crate) fn prepare_empty_stage(root: &Path, version: Option<&str>) -> Result<EmptyStage> {
    stage_timing::reset_from_env();
    let setup = stage_timing::start();
    let walked = source_listing::walk(root);
    let paths = descriptor_xmls_of(root, &walked.files);
    stage_timing::record(setup, "setup: tree walk", "", "");
    let setup = stage_timing::start();
    let files = read_descriptor_xmls(&paths)?;
    stage_timing::record(setup, "setup: descriptor reads", "", "");
    let setup = stage_timing::start();
    let context = EmptyStageContext::new(root, version, &files, walked.listing)?;
    drop(files);
    stage_timing::record(setup, "setup: context", "", "");
    let started = std::time::Instant::now();
    // One task per source XML, each mostly waiting for its files: the
    // file-bound pool, heaviest first.
    let objects = map_heaviest_first(root, &paths, |path| {
        prepare_empty_object(&context, path, false)
    })?;
    if stage_timing::enabled() {
        eprintln!(
            "{}",
            stage_timing::report(started.elapsed(), parallel::io_bound_worker_count())
        );
    }
    let stubs = stub_rows(
        &context,
        &objects
            .iter()
            .flat_map(|object| object.rows.iter().map(|row| row.file_name.as_str()))
            .collect(),
    )?;
    let names = objects
        .iter()
        .flat_map(|object| object.rows.iter().map(|row| row.file_name.clone()))
        .chain(stubs.rows.iter().map(|row| row.file_name.clone()))
        .collect::<Vec<_>>();
    let service = service_rows(&context, &names)?;
    Ok(EmptyStage {
        context,
        objects,
        stubs,
        service,
        tree_files: walked.files,
    })
}

// ---------------------------------------------------------------------------
// The audit.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EmptyStageAuditOptions {
    /// Samples kept per (pattern, kind, outcome).
    pub max_samples: usize,
    /// Where the stored and produced text of differing samples is written.
    pub diff_dir: Option<PathBuf>,
    /// A TSV of every produced row (file name, family, bytes, sha256 of the
    /// stored bytes, sha256 of the inflated text), to check a
    /// `--base-free --script-only` bcp file against.
    pub manifest: Option<PathBuf>,
    /// Write every produced row here as `<FileName>__part0.bin`, the stored
    /// bytes (raw deflate) -- the layout of a rows cache, for an offline
    /// export of the empty-infobase row set.
    pub rows_out: Option<PathBuf>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct EmptyStageOutcomes {
    pub identical: usize,
    pub different: usize,
    /// Of `different`: equal once line breaks are ignored.
    pub different_layout: usize,
    /// Of `different`: v8 containers whose elements are equal (their
    /// headers carry the time a platform wrote them).
    pub different_headers: usize,
    /// Of `different`: rows stored deflated twice whose content is equal
    /// once inflated again (a parent configuration `.cf`); only the
    /// compressor differs.
    pub different_recompressed: usize,
    /// In Config, not produced.
    pub missing: usize,
    /// Of `missing`: rows the platform keeps for itself, which no XML
    /// defines (`DynamicallyUpdated`, `*_dynupdate_*`, the 8.5 configuration
    /// cache `<configuration>.<uuid>`).
    pub missing_platform: usize,
    /// Of `missing`: empty stubs a Designer history leaves behind (an emptied
    /// help `{5,0,0}`, an emptied command interface `{7,0,0,0,0,0,0}`); the
    /// XML has no file for them.
    pub missing_stub: usize,
    /// Produced, not in Config.
    pub extra: usize,
    /// Would have been produced; the writer failed.
    pub failed: usize,
}

impl EmptyStageOutcomes {
    fn add(&mut self, outcome: Outcome, benign: Option<Benign>) {
        match outcome {
            Outcome::Identical => self.identical += 1,
            Outcome::Different => {
                self.different += 1;
                match benign {
                    Some(Benign::Layout) => self.different_layout += 1,
                    Some(Benign::Headers) => self.different_headers += 1,
                    Some(Benign::Recompressed) => self.different_recompressed += 1,
                    Some(Benign::Platform | Benign::Stub) | None => {}
                }
            }
            Outcome::Missing => {
                self.missing += 1;
                match benign {
                    Some(Benign::Platform) => self.missing_platform += 1,
                    Some(Benign::Stub) => self.missing_stub += 1,
                    _ => {}
                }
            }
            Outcome::Extra => self.extra += 1,
            Outcome::Failed => self.failed += 1,
        }
    }
    fn total(&self) -> usize {
        self.identical + self.different + self.missing + self.extra + self.failed
    }
}

/// A difference that leaves the content equal, or a missing row no XML
/// defines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Benign {
    Layout,
    Headers,
    Recompressed,
    Platform,
    Stub,
}

impl Benign {
    fn label(self) -> &'static str {
        match self {
            Self::Layout => "layout",
            Self::Headers => "container headers",
            Self::Recompressed => "recompressed",
            Self::Platform => "platform's own",
            Self::Stub => "empty stub",
        }
    }
}

/// Why a stored row nothing produced is not a gap, when it is not. (A
/// `<configuration>.<uuid>` row is a parent configuration, which a tree
/// holds as `Ext/ParentConfigurations/<name>.cf`: a gap when missing.)
fn missing_reason(name: &str, stored: Option<&[u8]>) -> Option<Benign> {
    if name == "DynamicallyUpdated" || name.contains("_dynupdate_") {
        return Some(Benign::Platform);
    }
    let text = stored?;
    let text = text.strip_prefix(b"\xef\xbb\xbf").unwrap_or(text);
    matches!(text, b"{5,0,0}" | b"{7,0,0,0,0,0,0}").then_some(Benign::Stub)
}

fn benign_difference(stored: &[u8], produced: &[u8]) -> Option<Benign> {
    let strip = |bytes: &[u8]| {
        bytes
            .iter()
            .copied()
            .filter(|byte| !matches!(byte, b'\r' | b'\n'))
            .collect::<Vec<_>>()
    };
    if strip(stored) == strip(produced) {
        return Some(Benign::Layout);
    }
    if containers_equal(stored, produced, 0) {
        return Some(Benign::Headers);
    }
    // A row deflated twice (a parent configuration) compares as its inner
    // stream, which is the compressor's output, not content.
    match (inflate_raw(stored), inflate_raw(produced)) {
        (Ok(stored), Ok(produced)) if !stored.is_empty() && stored == produced => {
            Some(Benign::Recompressed)
        }
        _ => None,
    }
}

/// Both v8 containers, with the same element names and equal data (nested
/// containers compared the same way).
fn containers_equal(left: &[u8], right: &[u8], depth: usize) -> bool {
    if depth > 3 {
        return false;
    }
    let (Ok(left), Ok(right)) = (
        crate::v8_container::parse_v8_container(left),
        crate::v8_container::parse_v8_container(right),
    ) else {
        return false;
    };
    left.len() == right.len()
        && left.iter().zip(right.iter()).all(|(left, right)| {
            left.name == right.name
                && (left.data == right.data || containers_equal(&left.data, &right.data, depth + 1))
        })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Outcome {
    Identical,
    Different,
    Missing,
    Extra,
    Failed,
}

impl Outcome {
    fn label(self) -> &'static str {
        match self {
            Self::Identical => "identical",
            Self::Different => "different",
            Self::Missing => "missing",
            Self::Extra => "extra",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EmptyStageSample {
    pub file_name: String,
    pub kind: String,
    pub family: String,
    pub source: String,
    /// First differing byte of the inflated text, and its brace path.
    pub offset: Option<usize>,
    pub brace_path: Option<String>,
    pub detail: String,
}

#[derive(Debug, Default, Serialize)]
pub struct EmptyStageVersionsReport {
    pub stored_names: usize,
    pub produced_names: usize,
    /// Names only the stored `versions` lists (samples).
    pub only_stored: Vec<String>,
    pub only_stored_count: usize,
    /// Names only the produced one lists (samples).
    pub only_produced: Vec<String>,
    pub only_produced_count: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct EmptyStageAuditReport {
    pub root: String,
    pub rows: String,
    pub source_version: String,
    pub configuration_shape: String,
    pub compatibility: u32,
    pub descriptor_xmls: usize,
    pub stored_rows: usize,
    /// Stored rows kept in more than one part (only part 0 is compared).
    pub multipart_rows: Vec<String>,
    pub produced_rows: usize,
    pub failures: usize,
    pub totals: EmptyStageOutcomes,
    /// By FileName pattern: `<uuid>`, `<uuid>.0`, `root`, ...
    pub by_pattern: BTreeMap<String, EmptyStageOutcomes>,
    /// By owner kind (the metadata class whose uuid the file name starts with).
    pub by_kind: BTreeMap<String, EmptyStageOutcomes>,
    /// By what produced (or would have produced) the row.
    pub by_family: BTreeMap<String, EmptyStageOutcomes>,
    /// `pattern | kind` for the rows that are not identical.
    pub by_pattern_kind: BTreeMap<String, EmptyStageOutcomes>,
    /// Failure reasons (uuids masked) with counts, by family.
    pub failure_reasons: BTreeMap<String, usize>,
    /// `versions` compared by names (its uuids are fresh by design).
    pub versions: EmptyStageVersionsReport,
    pub samples: BTreeMap<String, Vec<EmptyStageSample>>,
}

/// A stored Config row set: `<name>__part<N>.bin` (raw deflate) or
/// `<name>__part<N>.txt` (inflated).
struct StoredRows {
    dir: PathBuf,
    /// name -> is `.txt`
    names: BTreeMap<String, bool>,
    multipart: Vec<String>,
}

impl StoredRows {
    fn scan(dir: &Path) -> Result<Self> {
        let mut names = BTreeMap::new();
        let mut multipart = BTreeSet::new();
        for entry in
            fs::read_dir(dir).with_context(|| format!("failed to list {}", dir.display()))?
        {
            let entry = entry?;
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let (stem, text) = if let Some(stem) = file_name.strip_suffix(".bin") {
                (stem, false)
            } else if let Some(stem) = file_name.strip_suffix(".txt") {
                (stem, true)
            } else {
                continue;
            };
            let Some((name, part)) = stem.rsplit_once("__part") else {
                continue;
            };
            if part == "0" {
                names.entry(name.to_string()).or_insert(text);
            } else {
                multipart.insert(name.to_string());
            }
        }
        Ok(Self {
            dir: dir.to_path_buf(),
            names,
            multipart: multipart.into_iter().collect(),
        })
    }

    /// The inflated text of a stored row (a row that is not deflate is
    /// returned as stored).
    fn plain(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let Some(text) = self.names.get(name) else {
            return Ok(None);
        };
        let path = self.dir.join(format!(
            "{name}__part0.{}",
            if *text { "txt" } else { "bin" }
        ));
        let bytes =
            fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        if *text {
            return Ok(Some(bytes));
        }
        Ok(Some(inflate_raw(&bytes).unwrap_or(bytes)))
    }
}

fn is_uuid(text: &str) -> bool {
    text.len() == 36
        && text.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

/// `<uuid>`, `<uuid>.<suffix>`, `<uuid>_dynupdate_<uuid>[.<suffix>]`, or the
/// name itself.
fn pattern_of(name: &str) -> String {
    let (base, suffix) = match name.split_once('.') {
        Some((base, suffix)) => (base, Some(suffix)),
        None => (name, None),
    };
    let base = if is_uuid(base) {
        "<uuid>".to_string()
    } else if let Some((left, right)) = base.split_once("_dynupdate_")
        && is_uuid(left)
        && is_uuid(right)
    {
        "<uuid>_dynupdate_<uuid>".to_string()
    } else {
        return name.to_string();
    };
    match suffix {
        Some(suffix) => format!("{base}.{suffix}"),
        None => base,
    }
}

/// Masks uuids and cuts a failure to one short line.
fn reason_of(error: &str) -> String {
    let line = error.lines().next().unwrap_or_default();
    let mut out = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if index + 36 <= bytes.len()
            && line.is_char_boundary(index + 36)
            && is_uuid(&line[index..index + 36])
        {
            out.push_str("<uuid>");
            index += 36;
            continue;
        }
        let ch = line[index..].chars().next().unwrap_or(' ');
        out.push(ch);
        index += ch.len_utf8();
    }
    // Paths differ per object; keep the text before the first one.
    let cut = out
        .find(":\\")
        .map(|at| at.saturating_sub(1))
        .into_iter()
        .chain(out.find(" F:/"))
        .chain(out.find(" E:/"))
        .min();
    if let Some(cut) = cut {
        out.truncate(cut);
        out.push_str(" <path>");
    }
    out.chars().take(220).collect()
}

fn first_difference(left: &[u8], right: &[u8]) -> usize {
    left.iter()
        .zip(right.iter())
        .position(|(l, r)| l != r)
        .unwrap_or_else(|| left.len().min(right.len()))
}

fn excerpt(text: &[u8], offset: usize) -> String {
    let start = offset.saturating_sub(40);
    let end = (offset + 60).min(text.len());
    String::from_utf8_lossy(&text[start.min(end)..end]).replace("\r\n", "⏎")
}

/// The names a `versions` row lists (the leading `""` aside).
fn versions_names(plain: &[u8]) -> BTreeSet<String> {
    let text = String::from_utf8_lossy(plain);
    let mut names = BTreeSet::new();
    let mut rest = text.as_ref();
    while let Some(open) = rest.find('"') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('"') else {
            break;
        };
        let name = &after[..close];
        if !name.is_empty() {
            names.insert(name.to_string());
        }
        rest = &after[close + 1..];
    }
    names
}

struct Measured {
    file_name: String,
    kind: String,
    family: String,
    source: String,
    outcome: Outcome,
    benign: Option<Benign>,
    offset: Option<usize>,
    brace_path: Option<String>,
    detail: String,
    expected: Option<Vec<u8>>,
    actual: Option<Vec<u8>>,
}

pub fn audit_empty_stage(
    root: &Path,
    rows: &Path,
    version: Option<&str>,
    options: &EmptyStageAuditOptions,
) -> Result<EmptyStageAuditReport> {
    stage_timing::reset_from_env();
    let setup = stage_timing::start();
    let stored = StoredRows::scan(rows)?;
    stage_timing::record(setup, "setup: stored row list", "", "");
    let setup = stage_timing::start();
    let walked = source_listing::walk(root);
    let paths = descriptor_xmls_of(root, &walked.files);
    stage_timing::record(setup, "setup: tree walk", "", "");
    let setup = stage_timing::start();
    let files = read_descriptor_xmls(&paths)?;
    stage_timing::record(setup, "setup: descriptor reads", "", "");
    let setup = stage_timing::start();
    let context = EmptyStageContext::new(root, version, &files, walked.listing)?;
    drop(files);
    stage_timing::record(setup, "setup: context", "", "");
    if let Some(dir) = &options.rows_out {
        fs::create_dir_all(dir).with_context(|| format!("failed to create {}", dir.display()))?;
    }
    let write_row = |row: &EmptyStageRow| -> Result<()> {
        if let Some(dir) = &options.rows_out {
            let path = dir.join(format!("{}__part0.bin", row.file_name));
            fs::write(&path, &row.blob)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        Ok(())
    };

    // Owner kind of every uuid the tree names.
    let mut kinds: HashMap<String, String> = HashMap::new();
    for entry in context.descriptors.index.objects.values() {
        kinds.insert(entry.uuid.clone(), entry.kind.clone());
    }
    for (full, uuid) in &context.descriptors.index.children {
        let parts = full.rsplitn(3, '.').collect::<Vec<_>>();
        let kind = parts.get(1).copied().unwrap_or("Child");
        kinds
            .entry(uuid.clone())
            .or_insert_with(|| kind.to_string());
    }
    if let Some(group) = &context.module_group {
        kinds.insert(group.clone(), "Configuration".to_string());
    }
    let kind_of = |name: &str| -> String {
        let base = name.split(['.', '_']).next().unwrap_or(name);
        kinds.get(base).cloned().unwrap_or_else(|| {
            if matches!(name, "root" | "version" | "versions") {
                "Configuration".to_string()
            } else {
                "<unknown>".to_string()
            }
        })
    };
    let keep_samples = options.diff_dir.is_some();

    // Produce and compare object by object, in parallel; keep outcomes only.
    let parallel_started = std::time::Instant::now();
    let per_object = map_heaviest_first(
        root,
        &paths,
        |path| -> Result<(
            Vec<Measured>,
            Vec<String>,
            Vec<(String, String, usize, String, String)>,
        )> {
            let object = prepare_empty_object(&context, path, true);
            let audit_started = stage_timing::start();
            let mut measured = Vec::new();
            let mut names = Vec::new();
            let mut manifest = Vec::new();
            for row in &object.rows {
                write_row(row)?;
                names.push(row.file_name.clone());
                let plain = match &row.plain {
                    Some(plain) => plain.clone(),
                    None => inflate_raw(&row.blob).unwrap_or_else(|_| row.blob.clone()),
                };
                if options.manifest.is_some() {
                    manifest.push((
                        row.file_name.clone(),
                        row.family.clone(),
                        row.blob.len(),
                        hex_sha256(&row.blob),
                        hex_sha256(&plain),
                    ));
                }
                let (outcome, benign, offset, brace_path, detail, expected) =
                    match stored.plain(&row.file_name)? {
                        None => (Outcome::Extra, None, None, None, String::new(), None),
                        Some(expected) if expected == plain => {
                            (Outcome::Identical, None, None, None, String::new(), None)
                        }
                        Some(expected) => {
                            let offset = first_difference(&expected, &plain);
                            let detail = format!(
                                "stored {} | produced {}",
                                excerpt(&expected, offset),
                                excerpt(&plain, offset)
                            );
                            (
                                Outcome::Different,
                                benign_difference(&expected, &plain),
                                Some(offset),
                                Some(brace_path_at(&expected, offset)),
                                detail,
                                Some(expected),
                            )
                        }
                    };
                measured.push(Measured {
                    file_name: row.file_name.clone(),
                    kind: object.kind.clone(),
                    family: row.family.clone(),
                    source: row.source.clone(),
                    outcome,
                    benign,
                    offset,
                    brace_path,
                    detail,
                    expected: if keep_samples { expected } else { None },
                    actual: if keep_samples && outcome == Outcome::Different {
                        Some(plain)
                    } else {
                        None
                    },
                });
            }
            stage_timing::record(
                audit_started,
                "audit: compare and write",
                &object.kind,
                &object.relative,
            );
            for failure in &object.failures {
                measured.push(Measured {
                    file_name: failure.file_name.clone().unwrap_or_default(),
                    kind: if failure.kind.is_empty() {
                        "<unparsed>".to_string()
                    } else {
                        failure.kind.clone()
                    },
                    family: failure.family.clone(),
                    source: failure.source.clone(),
                    outcome: Outcome::Failed,
                    benign: None,
                    offset: None,
                    brace_path: None,
                    detail: failure.error.clone(),
                    expected: None,
                    actual: None,
                });
            }
            Ok((measured, names, manifest))
        },
    )?
    .into_iter()
    .collect::<Result<Vec<_>>>()?;
    if stage_timing::enabled() {
        eprintln!(
            "{}",
            stage_timing::report(
                parallel_started.elapsed(),
                parallel::io_bound_worker_count()
            )
        );
    }

    let tail_started = std::time::Instant::now();
    let mut measured = Vec::new();
    let mut names = Vec::new();
    let mut manifest = Vec::new();
    for (object_measured, object_names, object_manifest) in per_object {
        measured.extend(object_measured);
        names.extend(object_names);
        manifest.extend(object_manifest);
    }

    // The content-free rows ConfigDumpInfo.xml lists, once every object's
    // own rows are known.
    let stubs = stub_rows(&context, &names.iter().map(String::as_str).collect())?;
    for row in &stubs.rows {
        write_row(row)?;
        names.push(row.file_name.clone());
        let (item, digest) = measure_stub(&stored, row, keep_samples)?;
        if options.manifest.is_some() {
            manifest.push(digest);
        }
        measured.push(item);
    }
    for failure in &stubs.failures {
        measured.push(Measured {
            file_name: failure.file_name.clone().unwrap_or_default(),
            kind: failure.kind.clone(),
            family: failure.family.clone(),
            source: failure.source.clone(),
            outcome: Outcome::Failed,
            benign: None,
            offset: None,
            brace_path: None,
            detail: failure.error.clone(),
            expected: None,
            actual: None,
        });
    }

    // Service rows: root and version compared byte for byte (masking their
    // uuids once hid a generated uuid in 8.5's version row, which apply
    // refused), versions by its names.
    let service = service_rows(&context, &names)?;
    let mut versions_report = EmptyStageVersionsReport::default();
    for row in &service {
        write_row(row)?;
        let plain = row.plain.clone().unwrap_or_default();
        if options.manifest.is_some() {
            manifest.push((
                row.file_name.clone(),
                row.family.clone(),
                row.blob.len(),
                hex_sha256(&row.blob),
                hex_sha256(&plain),
            ));
        }
        names.push(row.file_name.clone());
        let expected = stored.plain(&row.file_name)?;
        let (outcome, detail) = match &expected {
            None => (Outcome::Extra, String::new()),
            Some(expected) if row.file_name == "versions" => {
                let stored_names = versions_names(expected);
                let produced_names = versions_names(&plain);
                versions_report.stored_names = stored_names.len();
                versions_report.produced_names = produced_names.len();
                let only_stored = stored_names.difference(&produced_names).collect::<Vec<_>>();
                let only_produced = produced_names.difference(&stored_names).collect::<Vec<_>>();
                versions_report.only_stored_count = only_stored.len();
                versions_report.only_produced_count = only_produced.len();
                versions_report.only_stored = only_stored
                    .iter()
                    .take(50)
                    .map(|name| name.to_string())
                    .collect();
                versions_report.only_produced = only_produced
                    .iter()
                    .take(50)
                    .map(|name| name.to_string())
                    .collect();
                if only_stored.is_empty() && only_produced.is_empty() {
                    (
                        Outcome::Identical,
                        "names identical, uuids fresh".to_string(),
                    )
                } else {
                    (
                        Outcome::Different,
                        format!(
                            "{} names only stored, {} only produced",
                            only_stored.len(),
                            only_produced.len()
                        ),
                    )
                }
            }
            Some(expected) => {
                let (left, right) = (expected.as_slice(), plain.as_slice());
                if left == right {
                    (Outcome::Identical, String::new())
                } else {
                    let offset = first_difference(left, right);
                    (
                        Outcome::Different,
                        format!(
                            "stored {} | produced {}",
                            excerpt(left, offset),
                            excerpt(right, offset)
                        ),
                    )
                }
            }
        };
        measured.push(Measured {
            file_name: row.file_name.clone(),
            kind: "Configuration".to_string(),
            family: row.family.clone(),
            source: row.source.clone(),
            outcome,
            benign: None,
            offset: None,
            brace_path: None,
            detail,
            expected: None,
            actual: None,
        });
    }

    // Stored rows nobody produced or failed to produce.
    let accounted = measured
        .iter()
        .filter(|item| !item.file_name.is_empty())
        .map(|item| item.file_name.clone())
        .collect::<BTreeSet<_>>();
    for name in stored.names.keys() {
        if !accounted.contains(name) {
            let plain = stored.plain(name)?;
            measured.push(Measured {
                file_name: name.clone(),
                kind: kind_of(name),
                family: "<none>".to_string(),
                source: String::new(),
                outcome: Outcome::Missing,
                benign: missing_reason(name, plain.as_deref()),
                offset: None,
                brace_path: None,
                detail: String::new(),
                expected: None,
                actual: None,
            });
        }
    }

    let mut report = EmptyStageAuditReport {
        root: root.display().to_string(),
        rows: rows.display().to_string(),
        source_version: context.version.clone(),
        configuration_shape: format!("{:?}", context.facts.shape),
        compatibility: context.facts.compatibility,
        descriptor_xmls: paths.len(),
        stored_rows: stored.names.len(),
        multipart_rows: stored.multipart.clone(),
        produced_rows: names.len(),
        failures: measured
            .iter()
            .filter(|item| item.outcome == Outcome::Failed)
            .count(),
        versions: versions_report,
        ..Default::default()
    };
    measured.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    for item in &measured {
        let pattern = if item.file_name.is_empty() {
            "<unattributed>".to_string()
        } else {
            pattern_of(&item.file_name)
        };
        report.totals.add(item.outcome, item.benign);
        report
            .by_pattern
            .entry(pattern.clone())
            .or_default()
            .add(item.outcome, item.benign);
        report
            .by_kind
            .entry(item.kind.clone())
            .or_default()
            .add(item.outcome, item.benign);
        report
            .by_family
            .entry(item.family.clone())
            .or_default()
            .add(item.outcome, item.benign);
        if item.outcome != Outcome::Identical {
            report
                .by_pattern_kind
                .entry(format!("{pattern} | {}", item.kind))
                .or_default()
                .add(item.outcome, item.benign);
        }
        if item.outcome == Outcome::Failed {
            *report
                .failure_reasons
                .entry(format!(
                    "{} | {} | {}",
                    item.kind,
                    item.family,
                    reason_of(&item.detail)
                ))
                .or_default() += 1;
        }
        if item.outcome != Outcome::Identical {
            let outcome = match item.benign {
                Some(benign) => format!("{} ({})", item.outcome.label(), benign.label()),
                None => item.outcome.label().to_string(),
            };
            let key = format!("{outcome} | {pattern} | {}", item.kind);
            let samples = report.samples.entry(key).or_default();
            if samples.len() < options.max_samples {
                if let (Some(dir), Some(expected), Some(actual)) =
                    (&options.diff_dir, &item.expected, &item.actual)
                {
                    let folder = dir.join(&item.kind);
                    fs::create_dir_all(&folder)?;
                    fs::write(
                        folder.join(format!("{}.stored.txt", item.file_name)),
                        expected,
                    )?;
                    fs::write(
                        folder.join(format!("{}.produced.txt", item.file_name)),
                        actual,
                    )?;
                }
                samples.push(EmptyStageSample {
                    file_name: item.file_name.clone(),
                    kind: item.kind.clone(),
                    family: item.family.clone(),
                    source: item.source.clone(),
                    offset: item.offset,
                    brace_path: item.brace_path.clone(),
                    detail: item.detail.chars().take(400).collect(),
                });
            }
        }
    }
    if let Some(path) = &options.manifest {
        manifest.sort();
        let mut text = String::from("file_name\tfamily\tbytes\tblob_sha256\tplain_sha256\n");
        for (name, family, bytes, blob, plain) in manifest {
            text.push_str(&format!("{name}\t{family}\t{bytes}\t{blob}\t{plain}\n"));
        }
        fs::write(path, text).with_context(|| format!("failed to write {}", path.display()))?;
    }
    if stage_timing::enabled() {
        eprintln!(
            "stage timing: serial tail (service rows, missing rows, report) {:.1} s",
            tail_started.elapsed().as_secs_f64()
        );
    }
    Ok(report)
}

/// Tables for the terminal.
pub fn empty_stage_summary(report: &EmptyStageAuditReport) -> String {
    fn table(title: &str, rows: &BTreeMap<String, EmptyStageOutcomes>, limit: usize) -> String {
        let mut rows = rows.iter().collect::<Vec<_>>();
        rows.sort_by(|left, right| {
            right
                .1
                .total()
                .cmp(&left.1.total())
                .then(left.0.cmp(right.0))
        });
        let mut lines = vec![format!(
            "{:<44} {:>7} {:>9} {:>9} {:>7} {:>7} {:>7} {:>6} {:>6}",
            title,
            "total",
            "identical",
            "different",
            "layout",
            "headers",
            "missing",
            "extra",
            "failed"
        )];
        for (key, counts) in rows.into_iter().take(limit) {
            lines.push(format!(
                "{:<44} {:>7} {:>9} {:>9} {:>7} {:>7} {:>7} {:>6} {:>6}",
                key.chars().take(44).collect::<String>(),
                counts.total(),
                counts.identical,
                counts.different,
                counts.different_layout,
                counts.different_headers,
                counts.missing,
                counts.extra,
                counts.failed
            ));
        }
        lines.join("\n")
    }
    let mut out = vec![
        format!(
            "{} | {} | shape {} compat {} | {} XMLs | stored {} rows, produced {}, failures {}",
            report.root,
            report.source_version,
            report.configuration_shape,
            report.compatibility,
            report.descriptor_xmls,
            report.stored_rows,
            report.produced_rows,
            report.failures
        ),
        format!(
            "TOTAL identical {} different {} (layout only {}, container headers only {}, recompressed only {}) missing {} (platform's own {}, empty stubs {}) extra {} failed {}",
            report.totals.identical,
            report.totals.different,
            report.totals.different_layout,
            report.totals.different_headers,
            report.totals.different_recompressed,
            report.totals.missing,
            report.totals.missing_platform,
            report.totals.missing_stub,
            report.totals.extra,
            report.totals.failed
        ),
        table("pattern", &report.by_pattern, 40),
        table("family", &report.by_family, 20),
        table("kind", &report.by_kind, 60),
    ];
    if !report.multipart_rows.is_empty() {
        out.push(format!(
            "{} stored rows have more than one part (only part 0 compared)",
            report.multipart_rows.len()
        ));
    }
    let mut reasons = report.failure_reasons.iter().collect::<Vec<_>>();
    reasons.sort_by(|left, right| right.1.cmp(left.1));
    out.push("failure reasons:".to_string());
    for (reason, count) in reasons.into_iter().take(40) {
        out.push(format!("{count:>7}  {reason}"));
    }
    out.join("\n")
}

// ---------------------------------------------------------------------------
// The loader mode.
// ---------------------------------------------------------------------------

/// `mssql-stage-source-objects --base-free`: every row of the tree into
/// ConfigSave through the bulk path, with no read of the target's Config.
pub(super) fn stage_source_objects_base_free(
    args: &MssqlStageSourceObjectsArgs,
) -> Result<StageSourceObjectsReport> {
    require_non_lab_confirmation(args.allow_non_lab, "source tree staging")?;
    if !args.replace_config_save {
        bail!("staging deletes existing ConfigSave rows; pass --replace-config-save");
    }
    if args.per_row {
        bail!("--base-free stages through the bulk path; drop --per-row");
    }
    if !args.path_prefix.is_empty() {
        bail!(
            "--base-free stages the whole tree (an empty infobase needs every row); drop --path-prefix"
        );
    }
    if args.script_only {
        super::OFFLINE_STAGE.store(true, Ordering::Relaxed);
    }
    let version = args.source_version.map(|version| version.as_str());
    let stage = prepare_empty_stage(&args.source_root, version)?;
    let tail_started = std::time::Instant::now();
    let failures = stage.failures().collect::<Vec<_>>();
    // A partial row set is only ever written, never loaded: it lets the
    // bcp file be checked against the audit before every writer is done.
    let partial = args.script_only
        && std::env::var_os("IBCMD_RS_BASE_FREE_ALLOW_FAILURES").is_some_and(|value| value == "1");
    if !failures.is_empty() && partial {
        eprintln!(
            "IBCMD_RS_BASE_FREE_ALLOW_FAILURES=1: writing the {} rows produced, {} failed (script only)",
            stage.rows().count(),
            failures.len()
        );
    }
    if !failures.is_empty() && !partial {
        let mut reasons = BTreeMap::<String, usize>::new();
        for failure in &failures {
            *reasons
                .entry(format!(
                    "{} | {} | {}",
                    failure.kind,
                    failure.family,
                    reason_of(&failure.error)
                ))
                .or_default() += 1;
        }
        let mut reasons = reasons.into_iter().collect::<Vec<_>>();
        reasons.sort_by(|left, right| right.1.cmp(&left.1));
        let listed = reasons
            .iter()
            .take(25)
            .map(|(reason, count)| format!("  {count:>6}  {reason}"))
            .collect::<Vec<_>>()
            .join("\n");
        bail!(
            "a base-free stage needs every row, and {} could not be produced (audit-empty-stage lists them all):\n{listed}",
            failures.len()
        );
    }

    let rows = stage.rows().collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    for row in &rows {
        if !seen.insert(row.file_name.as_str()) {
            bail!("two rows of the tree share the file name {}", row.file_name);
        }
    }
    let bulk = rows
        .iter()
        .map(|row| BulkStageRow {
            file_name: &row.file_name,
            requires_config_row: false,
            blob: &row.blob,
        })
        .collect::<Vec<_>>();
    let (rows_path, prepare_path, apply_path) =
        bulk_stage_paths(args.script_output.as_ref(), &args.database);
    if let Some(parent) = rows_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let table = bulk_stage_table_name(&args.database);
    // The SQL handle is built before the first write, so a wrong server name or
    // a missing password stops the stage before any file is written.
    let sql = if args.script_only {
        None
    } else {
        let sql_password = resolve_sqlcmd_password(
            args.sql_user.as_deref(),
            args.sql_pwd.as_deref(),
            &args.sql_pwd_env,
        );
        Some(stage_sql(
            args.sqlcmd.as_deref(),
            args.bcp_executable.as_deref(),
            &args.server,
            args.sql_user.as_deref(),
            sql_password.as_deref(),
            &args.sql_pwd_env,
        )?)
    };
    // The guard: the state this stage would leave, exported with the model and
    // compared with the tree, before anything is written.
    let verification = if super::stage_guard::wanted(args.verify) {
        Some(super::timed_stage_step("verify the staged state", || {
            super::stage_guard::verify_base_free_stage(args, sql.as_ref(), &bulk, &stage.tree_files)
        })?)
    } else {
        None
    };
    if sql
        .as_ref()
        .is_none_or(|sql| bulk_stage_rows_file_needed(sql, args.script_only))
    {
        let write_started = std::time::Instant::now();
        write_bulk_stage_rows(&rows_path, &bulk)?;
        if stage_timing::enabled() {
            eprintln!(
                "stage timing: bcp file written in {:.1} s",
                write_started.elapsed().as_secs_f64()
            );
        }
    }
    fs::write(&prepare_path, build_bulk_stage_prepare_sql(&table))
        .with_context(|| format!("failed to write {}", prepare_path.display()))?;
    fs::write(
        &apply_path,
        build_base_free_bulk_stage_apply_sql(
            &args.database,
            &table,
            super::bulk_stage_part_count(&bulk),
        ),
    )
    .with_context(|| format!("failed to write {}", apply_path.display()))?;

    let not_queried = |table: &str| StorageTableManifest {
        table_name: table.to_string(),
        file_name: String::new(),
        row_count: -1,
        binary_bytes: -1,
        row_checksum: None,
    };
    let mut before = not_queried("ConfigSave");
    let mut after = not_queried("ConfigSave");
    if let Some(sql) = &sql {
        before = storage_table_stats(sql, &args.database, "ConfigSave")?;
        run_bulk_stage(sql, &table, &bulk, &rows_path, &prepare_path, &apply_path)?;
        after = storage_table_stats(sql, &args.database, "ConfigSave")?;
    }

    let report_started = std::time::Instant::now();
    let versions = stage
        .service
        .iter()
        .find(|row| row.file_name == "versions")
        .map(|row| GeneratedBlobReport {
            bytes: row.blob.len(),
            sha256: hex_sha256(&row.blob),
        })
        .unwrap_or(GeneratedBlobReport {
            bytes: 0,
            sha256: String::new(),
        });
    let metadata_objects = stage
        .objects
        .iter()
        .filter_map(|object| {
            let properties = object.properties.clone()?;
            let descriptor = object.rows.iter().find(|row| row.file_name == object.uuid);
            Some(StagedMetadataObjectReport {
                object_id: object.uuid.clone(),
                kind: object.kind.clone(),
                xml: object.xml.clone(),
                properties,
                metadata_plain_bytes: 0,
                metadata_blob: GeneratedBlobReport {
                    bytes: descriptor.map_or(0, |row| row.blob.len()),
                    sha256: descriptor
                        .map(|row| hex_sha256(&row.blob))
                        .unwrap_or_default(),
                },
                body_rows: object
                    .rows
                    .iter()
                    .filter(|row| row.file_name != object.uuid)
                    .map(|row| StagedMetadataBodyReport {
                        body_id: row.file_name.clone(),
                        path: stage.context.root.join(&row.source),
                        blob: GeneratedBlobReport {
                            bytes: row.blob.len(),
                            sha256: hex_sha256(&row.blob),
                        },
                    })
                    .collect(),
            })
        })
        .collect();
    if stage_timing::enabled() {
        eprintln!(
            "stage timing: report built in {:.1} s; after the stage {:.1} s",
            report_started.elapsed().as_secs_f64(),
            tail_started.elapsed().as_secs_f64()
        );
    }
    let source_version = Some(stage.context.version.clone());
    // Freeing the stage -- every row's bytes and the tree's metadata XMLs, some
    // million allocations -- took 26 s of ERP УХ's stage on the lab
    // workstation after all the work was done. A thread of its own frees it
    // while the caller writes its report, and the process's exit does not
    // wait for it.
    std::thread::spawn(move || drop(stage));
    Ok(StageSourceObjectsReport {
        database: args.database.clone(),
        source_version,
        metadata_objects,
        common_modules: Vec::new(),
        scripts: vec![prepare_path, apply_path.clone()],
        script: apply_path,
        before,
        after,
        versions_blob: versions,
        version_replacements: Vec::new(),
        verification,
        overrides: None,
    })
}

/// The bulk apply of a base-free stage: the staged rows are the whole
/// configuration, `root`, `version` and `versions` among them, so nothing is
/// copied from or checked against Config. Attributes are 0 (what every
/// staged body row the target lacks already gets on the default path); a row
/// over the platform's part size is stored in parts, as its own import does.
pub(super) fn build_base_free_bulk_stage_apply_sql(
    database: &str,
    table: &str,
    staged_rows: usize,
) -> String {
    let stage = format!("tempdb.dbo.{}", quote_ident(table));
    format!(
        "SET NOCOUNT ON;\n\
         SET XACT_ABORT ON;\n\
         USE {db};\n\
         IF (SELECT COUNT_BIG(*) FROM {stage}) <> {staged_rows}\n\
             THROW 55002, 'bcp loaded an unexpected number of staged rows', 1;\n\
         IF EXISTS (SELECT 1 FROM {stage} GROUP BY FileName, DataSize HAVING SUM(DATALENGTH(BinaryData)) <> DataSize)\n\
             THROW 55003, 'A staged row lost bytes on its way in', 1;\n\
         IF EXISTS (SELECT FileName FROM {stage} GROUP BY FileName, PartNo HAVING COUNT_BIG(*) > 1)\n\
             THROW 55004, 'Two staged rows share a file name', 1;\n\
         IF (SELECT COUNT_BIG(*) FROM {stage} WHERE FileName IN (N'root', N'version', N'versions')) <> 3\n\
             THROW 55005, 'A base-free stage must carry root, version and versions', 1;\n\
         BEGIN TRAN;\n\
         DELETE FROM dbo.ConfigSave;\n\
         INSERT INTO dbo.ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
         SELECT s.FileName, DATEADD(year, 2000, SYSUTCDATETIME()), DATEADD(year, 2000, SYSUTCDATETIME()), 0, s.DataSize, s.BinaryData, s.PartNo\n\
         FROM {stage} s;\n\
         IF (SELECT COUNT_BIG(*) FROM dbo.ConfigSave) <> {staged_rows}\n\
             THROW 56999, 'Unexpected ConfigSave row count after base-free staging', 1;\n\
         COMMIT;\n\
         DROP TABLE {stage};\n",
        db = quote_ident(database),
    )
}

// ---------------------------------------------------------------------------
// Content-free rows the tree's ConfigDumpInfo.xml lists.
// ---------------------------------------------------------------------------

/// Rows a stored configuration keeps for parts whose content was deleted in
/// the Designer -- an emptied help, command interface, predefined data or
/// aggregates. The export writes no file for them, so the tree holds nothing
/// to compile them from, but its `ConfigDumpInfo.xml` still lists them, and
/// the platform's export of a database without them lists them no more (ERP
/// УХ: 145 such rows, 132 helps, 11 predefined, 1 aggregates, 1 command
/// interface). They are written from that list, their bytes modelled on the
/// stored rows (`bodies_rows::stub_row_text`).
#[derive(Debug, Default)]
pub(crate) struct StubRows {
    pub rows: Vec<EmptyStageRow>,
    pub failures: Vec<EmptyStageFailure>,
}

/// The entries of `ConfigDumpInfo.xml` that may name a content-free row:
/// top-level entries (they carry a configVersion; the nested ones are
/// children inside their parent's descriptor) of a part `StubPart` knows.
/// A tree without the file has none.
fn dump_info_stub_candidates(root: &Path) -> Result<Vec<(String, String, StubPart)>> {
    use quick_xml::events::Event;
    let path = root.join("ConfigDumpInfo.xml");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };
    let mut reader = quick_xml::Reader::from_reader(bytes.as_slice());
    let mut buffer = Vec::new();
    let mut out = Vec::new();
    loop {
        let event = reader
            .read_event_into(&mut buffer)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        match event {
            Event::Start(element) | Event::Empty(element)
                if element.local_name().as_ref() == b"Metadata" =>
            {
                let (mut name, mut id, mut versioned) = (None, None, false);
                for attribute in element.attributes().flatten() {
                    let value = || String::from_utf8_lossy(&attribute.value).into_owned();
                    match attribute.key.local_name().as_ref() {
                        b"name" => name = Some(value()),
                        b"id" => id = Some(value().to_ascii_lowercase()),
                        b"configVersion" => versioned = true,
                        _ => {}
                    }
                }
                if let (true, Some(name), Some(id)) = (versioned, name, id)
                    && let Some(part) = StubPart::of_dump_info_name(&name)
                {
                    out.push((name, id, part));
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Ok(out)
}

/// The content-free rows `ConfigDumpInfo.xml` lists and nothing produced
/// (`produced`: every object's own rows). An entry is written only for an
/// object the tree holds, in the row its part takes for the object's kind:
/// a list older than the tree names nothing else.
pub(crate) fn stub_rows(context: &EmptyStageContext, produced: &HashSet<&str>) -> Result<StubRows> {
    let mut stubs = StubRows::default();
    let index = &context.descriptors.index;
    for (name, id, part) in dump_info_stub_candidates(&context.root)? {
        if produced.contains(id.as_str()) {
            continue;
        }
        let Some((owner, suffix)) = id.split_once('.') else {
            continue;
        };
        let Some(entry) = index
            .objects_by_uuid
            .get(owner)
            .and_then(|full_name| index.objects.get(full_name))
        else {
            continue;
        };
        let source = relative_of(&context.root, &entry.path);
        let text = context
            .descriptors
            .source
            .read_source(&entry.path)
            .map_err(anyhow::Error::from)
            .and_then(|xml| stub_row_text(part, &entry.kind, suffix, &xml, &context.descriptors))
            .and_then(|text| text.map(|text| Ok((deflate_raw(&text)?, text))).transpose());
        match text {
            Ok(Some((blob, plain))) => stubs.rows.push(EmptyStageRow {
                file_name: id,
                family: "stub".to_string(),
                source,
                blob,
                plain: Some(plain),
            }),
            Ok(None) => {}
            Err(error) => stubs.failures.push(EmptyStageFailure {
                file_name: Some(id),
                kind: entry.kind.clone(),
                family: "stub".to_string(),
                source,
                error: format!("{name}: {}", error_text(&error)),
            }),
        }
    }
    Ok(stubs)
}

/// A stub row against the stored one, as the audit measures every row;
/// with its manifest line.
fn measure_stub(
    stored: &StoredRows,
    row: &EmptyStageRow,
    keep_samples: bool,
) -> Result<(Measured, (String, String, usize, String, String))> {
    let plain = row
        .plain
        .clone()
        .unwrap_or_else(|| inflate_raw(&row.blob).unwrap_or_else(|_| row.blob.clone()));
    let digest = (
        row.file_name.clone(),
        row.family.clone(),
        row.blob.len(),
        hex_sha256(&row.blob),
        hex_sha256(&plain),
    );
    let mut item = Measured {
        file_name: row.file_name.clone(),
        kind: "stub".to_string(),
        family: row.family.clone(),
        source: row.source.clone(),
        outcome: Outcome::Extra,
        benign: None,
        offset: None,
        brace_path: None,
        detail: String::new(),
        expected: None,
        actual: None,
    };
    match stored.plain(&row.file_name)? {
        None => {}
        Some(expected) if expected == plain => item.outcome = Outcome::Identical,
        Some(expected) => {
            let offset = first_difference(&expected, &plain);
            item.outcome = Outcome::Different;
            item.benign = benign_difference(&expected, &plain);
            item.offset = Some(offset);
            item.brace_path = Some(brace_path_at(&expected, offset));
            item.detail = format!(
                "stored {} | produced {}",
                excerpt(&expected, offset),
                excerpt(&plain, offset)
            );
            if keep_samples {
                item.expected = Some(expected);
                item.actual = Some(plain);
            }
        }
    }
    Ok((item, digest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_and_reasons() {
        assert_eq!(pattern_of("66193438-abc5-410b-a1f1-a204102d1a62"), "<uuid>");
        assert_eq!(
            pattern_of("66193438-abc5-410b-a1f1-a204102d1a62.1c"),
            "<uuid>.1c"
        );
        assert_eq!(pattern_of("versions"), "versions");
        assert_eq!(
            pattern_of(
                "66193438-abc5-410b-a1f1-a204102d1a62_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b.0"
            ),
            "<uuid>_dynupdate_<uuid>.0"
        );
        assert_eq!(
            reason_of(
                "base-free stage has no base Config row 66193438-abc5-410b-a1f1-a204102d1a62.0"
            ),
            "base-free stage has no base Config row <uuid>.0"
        );
        let names = versions_names(b"{1,3,\"\",u,\"a\",u,\"root\",u}");
        assert_eq!(names.into_iter().collect::<Vec<_>>(), vec!["a", "root"]);
    }

    #[test]
    fn parallel_descriptor_walk_matches_the_serial_one() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-descriptor-walk-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        for file in [
            "Configuration.xml",
            "ConfigDumpInfo.xml",
            "Catalogs/A.xml",
            "Catalogs/A/Ext/ObjectModule.bsl",
            "Catalogs/A/Forms/F.xml",
            "Catalogs/A/Forms/F/Ext/Form.xml",
            "Catalogs/A/Forms/F/Ext/Form/Items/X.xml",
            "Catalogs/B.xml",
            "Subsystems/S.xml",
            "Subsystems/S/Subsystems/T.xml",
            "Subsystems/S/Ext/CommandInterface.xml",
            "Subsystems/S/EXT/Other.xml",
            "Ext/Top.xml",
            "Languages/Русский.xml",
        ] {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, b"x").unwrap();
        }
        let serial = crate::metadata_model::audit::descriptor_xmls(&root);
        let parallel = descriptor_xmls_of(&root, &source_listing::walk(&root).files);
        let _ = fs::remove_dir_all(&root);
        assert_eq!(serial.len(), 7, "{serial:?}");
        assert_eq!(parallel, serial);
    }

    #[test]
    fn stub_candidates_are_the_listed_rows_of_the_four_parts() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-dump-info-stubs-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(&root).unwrap();
        assert!(dump_info_stub_candidates(&root).unwrap().is_empty());
        fs::write(
            root.join("ConfigDumpInfo.xml"),
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <ConfigDumpInfo xmlns=\"http://v8.1c.ru/8.3/xcf/dumpinfo\" format=\"Hierarchical\" version=\"2.20\">\n\
             \t<ConfigVersions>\n\
             \t\t<Metadata name=\"Catalog.A\" id=\"aaaaaaaa-0000-4000-8000-000000000001\" configVersion=\"01\">\n\
             \t\t\t<Metadata name=\"Catalog.A.Attribute.Help\" id=\"bbbbbbbb-0000-4000-8000-000000000001\"/>\n\
             \t\t</Metadata>\n\
             \t\t<Metadata name=\"Catalog.A.Help\" id=\"AAAAAAAA-0000-4000-8000-000000000001.0\" configVersion=\"02\"/>\n\
             \t\t<Metadata name=\"Catalog.A.Predefined\" id=\"aaaaaaaa-0000-4000-8000-000000000001.1c\" configVersion=\"03\"/>\n\
             \t\t<Metadata name=\"Catalog.A.Form.F\" id=\"cccccccc-0000-4000-8000-000000000001\" configVersion=\"04\"/>\n\
             \t\t<Metadata name=\"Catalog.A.Form.F.Form\" id=\"cccccccc-0000-4000-8000-000000000001.0\" configVersion=\"05\"/>\n\
             \t\t<Metadata name=\"AccumulationRegister.R.Aggregates\" id=\"dddddddd-0000-4000-8000-000000000001.3\" configVersion=\"06\"/>\n\
             \t\t<Metadata name=\"Subsystem.S.CommandInterface\" id=\"eeeeeeee-0000-4000-8000-000000000001.1\" configVersion=\"07\"/>\n\
             \t</ConfigVersions>\n\
             </ConfigDumpInfo>\n",
        )
        .unwrap();
        let found = dump_info_stub_candidates(&root).unwrap();
        let _ = fs::remove_dir_all(&root);
        let found = found
            .iter()
            .map(|(_, id, part)| (id.as_str(), *part))
            .collect::<Vec<_>>();
        assert_eq!(
            found,
            vec![
                ("aaaaaaaa-0000-4000-8000-000000000001.0", StubPart::Help),
                (
                    "aaaaaaaa-0000-4000-8000-000000000001.1c",
                    StubPart::Predefined
                ),
                (
                    "dddddddd-0000-4000-8000-000000000001.3",
                    StubPart::Aggregates
                ),
                (
                    "eeeeeeee-0000-4000-8000-000000000001.1",
                    StubPart::CommandInterface
                ),
            ]
        );
    }

    #[test]
    fn base_free_apply_reads_no_config() {
        let sql = build_base_free_bulk_stage_apply_sql("db", "stage", 3);
        assert!(!sql.contains("dbo.Config "));
        assert!(!sql.contains("dbo.Config\n"));
        assert!(sql.contains("DELETE FROM dbo.ConfigSave"));
    }
}
