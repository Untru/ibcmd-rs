//! A full `mssql-dump-config` writes the descriptor rows of every kind the
//! metadata model decodes row -> model -> XML through `metadata_model::export`
//! instead of the legacy converters; `--legacy-export` (or
//! `IBCMD_RS_LEGACY_EXPORT=1`) keeps the legacy converters.
//!
//! Two steps. A [`ModelPlan`], from the rows alone and before any legacy
//! index: the root row's lists give every top-level object's kind, the
//! owners' lists every owned object's (forms, templates, recalculations,
//! nested subsystems). Then the [`ModelExport`]'s [`NameIndex`]: each object
//! row through its kind's `object_names`, owned objects under their owner's
//! name, the predefined items of the predefined-data bodies. Kinds whose rows
//! `object_names` cannot read yet borrow their names from the legacy indexes
//! the export builds anyway, so the export stays whole while the kinds move
//! over one by one.

use super::*;
use crate::metadata_model::brace::parse_row;
use crate::metadata_model::export::names::{
    IndexComparison, compare, has_decoder, has_names, has_owned_names, may_own_objects,
    owned_objects, predefined_items, predefined_suffix, root_kinds,
};
use crate::metadata_model::export::{
    ExportContext, NameIndex, decode_object, object_names, owned_object_names, write_document,
};
use crate::metadata_model::index::ConfigIndex;
use crate::metadata_model::objects::parts::Compat;

/// Whether the export keeps its research summaries (the model plan, the
/// name index) off stderr: the drop-in `infobase config export` prints only
/// the platform's own lines. Warnings about single rows still print.
static QUIET_SUMMARIES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_quiet_summaries(quiet: bool) {
    QUIET_SUMMARIES.store(quiet, std::sync::atomic::Ordering::Relaxed);
}

pub(super) fn quiet_summaries() -> bool {
    QUIET_SUMMARIES.load(std::sync::atomic::Ordering::Relaxed)
}

/// The environment switches, for callers without the command-line flags:
/// `IBCMD_RS_LEGACY_EXPORT=1` (or `IBCMD_RS_MODEL_EXPORT=0`) opts out.
pub(super) const MODEL_EXPORT_ENV: &str = "IBCMD_RS_MODEL_EXPORT";
pub(super) const LEGACY_EXPORT_ENV: &str = "IBCMD_RS_LEGACY_EXPORT";

/// Whether a full export writes its descriptors through the model: yes,
/// unless `--legacy-export`, `IBCMD_RS_LEGACY_EXPORT=1` or
/// `IBCMD_RS_MODEL_EXPORT=0` asks for the legacy converters.
/// (`--model-export` is the default spelled out.)
pub(super) fn requested(model_flag: bool, legacy_flag: bool) -> bool {
    let env_is = |name: &str, value: &str| std::env::var(name).is_ok_and(|set| set.trim() == value);
    if legacy_flag || env_is(LEGACY_EXPORT_ENV, "1") || env_is(MODEL_EXPORT_ENV, "0") {
        return false;
    }
    let _ = model_flag;
    true
}

/// `IBCMD_RS_MODEL_EXPORT_SHADOW=1`: the legacy converter also runs on the
/// modelled rows, to time it on the same rows and compare the bytes.
pub(super) fn shadow() -> bool {
    static SHADOW: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SHADOW.get_or_init(|| {
        std::env::var("IBCMD_RS_MODEL_EXPORT_SHADOW").is_ok_and(|value| value.trim() == "1")
    })
}

/// A text row's raw row: inflated as stored, with no rewriting.
fn raw_row_text(row: &ConfigRow) -> Result<Vec<u8>> {
    let bytes = row.binary_bytes()?;
    inflate_raw_deflate(&bytes)
}

/// The root kind of a full name (`Catalog.X.Attribute.Y` -> `Catalog`).
fn root_kind(full_name: &str) -> &str {
    full_name.split('.').next().unwrap_or_default()
}

/// The folder a kind's files go to (`Catalog` -> `Catalogs`, `Form` ->
/// `Forms`).
fn kind_folder(kind: &str) -> Option<String> {
    let folder = if kind == "FilterCriterion" {
        "FilterCriteria".to_string()
    } else if let Some(rest) = kind.strip_prefix("ChartOf") {
        format!("ChartsOf{rest}")
    } else if kind.ends_with("ss") {
        format!("{kind}es")
    } else {
        format!("{kind}s")
    };
    (crate::metadata_model::index::kind_of_collection(&folder) == Some(kind)).then_some(folder)
}

/// The root row among the text rows: the configuration's own row.
fn root_text_row(texts: &[MetadataTextRow]) -> Option<&MetadataTextRow> {
    texts.iter().find(|row| {
        !row.file_name.contains('.')
            && parse_configuration_reference_text_for_row(&row.text, &row.file_name).is_some()
    })
}

// ---------------------------------------------------------------------------
// The plan.

/// What each metadata row is, from the rows alone.
pub(super) struct ModelPlan {
    version: String,
    compatibility_mode: Option<String>,
    compat: Compat,
    /// The configuration's own row.
    root: Option<String>,
    /// Top-level objects, from the root row's lists.
    kinds: HashMap<String, &'static str>,
    /// Owned objects, owners before what they own: (owner, kind, uuid).
    owned: Vec<(String, &'static str, String)>,
    owned_kinds: HashMap<String, &'static str>,
}

impl ModelPlan {
    pub(super) fn new(
        metadata_rows: &[ConfigRow],
        texts: &[MetadataTextRow],
        source_version: InfobaseConfigSourceVersion,
    ) -> Self {
        let raw_by_name = metadata_rows
            .iter()
            .map(|row| (row.file_name.as_str(), row))
            .collect::<HashMap<_, _>>();
        let root = root_text_row(texts);
        let compatibility_mode = root.and_then(|row| {
            parse_configuration_properties_from_text(&row.text, &BTreeMap::new(), source_version)
                .and_then(|properties| properties.compatibility_mode)
        });
        // A row that names no compatibility runs in the platform's own: the
        // release the registry maps the XML format to.
        let compat = compatibility_mode
            .as_deref()
            .and_then(Compat::parse)
            .unwrap_or_else(|| {
                let [major, minor, patch] =
                    crate::platform::of_xml_dialect(source_version.as_str()).release();
                Compat(major, minor, patch)
            });
        let kinds = root
            .and_then(|row| raw_by_name.get(row.file_name.as_str()))
            .and_then(|raw| raw_row_text(raw).ok())
            .and_then(|text| parse_row(&text).ok())
            .map(|tree| root_kinds(&tree))
            .unwrap_or_default();

        // Owners' lists, top-level owners in parallel, nested subsystems as
        // they come.
        let owned_lists = |uuid: &str| -> Vec<(&'static str, String)> {
            raw_by_name
                .get(uuid)
                .and_then(|raw| raw_row_text(raw).ok())
                .filter(|text| std::str::from_utf8(text).is_ok_and(may_own_objects))
                .and_then(|text| parse_row(&text).ok())
                .map(|tree| owned_objects(&tree))
                .unwrap_or_default()
        };
        let mut pending = parallel::install_memory_bound_or_inline(|| {
            texts
                .par_iter()
                .filter(|row| kinds.contains_key(row.file_name.as_str()))
                .map(|row| (row.file_name.clone(), owned_lists(&row.file_name)))
                .filter(|(_, owned)| !owned.is_empty())
                .collect::<Vec<_>>()
        });
        pending.reverse();
        let mut owned = Vec::new();
        let mut owned_kinds = HashMap::new();
        while let Some((owner, list)) = pending.pop() {
            for (kind, uuid) in list {
                if !raw_by_name.contains_key(uuid.as_str()) || owned_kinds.contains_key(&uuid) {
                    continue;
                }
                owned_kinds.insert(uuid.clone(), kind);
                owned.push((owner.clone(), kind, uuid.clone()));
                if matches!(kind, "Subsystem" | "Cube") {
                    let nested = owned_lists(&uuid);
                    if !nested.is_empty() {
                        pending.push((uuid, nested));
                    }
                }
            }
        }
        Self {
            version: source_version.as_str().to_string(),
            compatibility_mode,
            compat,
            root: root.map(|row| row.file_name.clone()),
            kinds,
            owned,
            owned_kinds,
        }
    }

    /// The kind of a metadata row: the configuration's own row, a top-level
    /// object or an owned object.
    pub(super) fn kind_of(&self, file_name: &str) -> Option<&'static str> {
        if self.root.as_deref() == Some(file_name) {
            return Some("Configuration");
        }
        self.kinds
            .get(file_name)
            .or_else(|| self.owned_kinds.get(file_name))
            .copied()
    }

    /// A shared form has no owning metadata object. Use its root-list identity,
    /// not the descriptor layout shared with owned forms; conflicting ownership
    /// remains uncertain and must keep its diagnostic.
    pub(super) fn is_unowned_common_form(&self, file_name: &str) -> bool {
        self.kinds.get(file_name) == Some(&"CommonForm")
            && !self.owned_kinds.contains_key(file_name)
    }

    /// Whether every descriptor row goes through the model: then the legacy
    /// indexes only the descriptor converters read need not be built.
    pub(super) fn models_every_descriptor(&self, texts: &[MetadataTextRow]) -> bool {
        texts
            .iter()
            .filter(|row| !row.file_name.contains('.') && row.header.is_some())
            .all(|row| self.kind_of(&row.file_name).is_some_and(has_decoder))
    }

    /// Whether every row of these kinds goes through the model: each kind has
    /// a decoder and the plan knows the kind of every descriptor row (a row
    /// it cannot place could be one of them).
    pub(super) fn models_kinds(&self, kinds: &[&str], texts: &[MetadataTextRow]) -> bool {
        kinds.iter().all(|kind| has_decoder(kind))
            && texts
                .iter()
                .filter(|row| !row.file_name.contains('.') && row.header.is_some())
                .all(|row| self.kind_of(&row.file_name).is_some())
    }

    /// Whether the rows alone name everything: every top-level kind has
    /// `object_names`, every owned kind `owned_object_names`, and the plan
    /// places every descriptor row. Then the index takes nothing from the
    /// legacy indexes.
    pub(super) fn names_from_rows_alone(&self, texts: &[MetadataTextRow]) -> bool {
        self.kinds.values().all(|kind| has_names(kind))
            && self.owned.iter().all(|(_, kind, _)| has_owned_names(kind))
            && texts
                .iter()
                .filter(|row| !row.file_name.contains('.') && row.header.is_some())
                .all(|row| self.kind_of(&row.file_name).is_some())
    }

    /// The descriptor rows the legacy converters still write, by kind (`?`
    /// for a row neither the root's nor an owner's lists name).
    pub(super) fn legacy_descriptor_kinds(
        &self,
        texts: &[MetadataTextRow],
    ) -> BTreeMap<&'static str, usize> {
        let mut out = BTreeMap::new();
        for row in texts
            .iter()
            .filter(|row| !row.file_name.contains('.') && row.header.is_some())
        {
            match self.kind_of(&row.file_name) {
                Some(kind) if has_decoder(kind) => {}
                Some(kind) => *out.entry(kind).or_default() += 1,
                None => *out.entry("?").or_default() += 1,
            }
        }
        out
    }

    /// The predefined-data bodies the index reads: `<owner>.<suffix>` of every
    /// top-level object whose kind stores predefined items.
    pub(super) fn predefined_body_file_names(&self) -> BTreeSet<String> {
        self.kinds
            .iter()
            .filter_map(|(uuid, kind)| Some(format!("{uuid}.{}", predefined_suffix(kind)?)))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// The index and the export.

/// The legacy indexes the row-built index borrows names from, for the kinds
/// without `object_names`.
pub(super) struct LegacyNames<'a> {
    pub(super) object_refs: &'a BTreeMap<String, String>,
    pub(super) type_index: &'a BTreeMap<String, String>,
    pub(super) form_refs: &'a BTreeMap<String, FormSourceReference>,
    pub(super) template_refs: &'a BTreeMap<String, TemplateSourceReference>,
}

/// What the index was built from.
#[derive(Debug, Default, Clone, Serialize)]
pub(super) struct ModelIndexReport {
    /// Rows read through `object_names`, by kind.
    pub(super) rows_by_kind: BTreeMap<String, usize>,
    /// Top-level rows of kinds without `object_names`, named from their
    /// header and the legacy indexes.
    pub(super) legacy_rows_by_kind: BTreeMap<String, usize>,
    /// Rows `object_names` failed on (kind: first error line -> count).
    pub(super) failures: BTreeMap<String, usize>,
    pub(super) names: usize,
    pub(super) types: usize,
    pub(super) predefined: usize,
    /// Owned objects (forms, templates, recalculations, nested subsystems)
    /// named through their owner's row, and those only the legacy path
    /// indexes named.
    pub(super) owned_names: usize,
    pub(super) legacy_owned_names: usize,
    /// Of `names`/`types`, what the legacy indexes gave.
    pub(super) legacy_names: usize,
    pub(super) legacy_types: usize,
    pub(super) predefined_bodies: usize,
    pub(super) compatibility_mode: Option<String>,
    /// The legacy indexes were consulted (some kind's rows cannot name
    /// themselves yet).
    pub(super) used_legacy_indexes: bool,
}

pub(super) struct ModelExport {
    context: ExportContext,
    plan: ModelPlan,
    pub(super) report: ModelIndexReport,
}

impl ModelExport {
    /// The name index and the decoders' context for one export.
    pub(super) fn build(
        plan: ModelPlan,
        metadata_rows: &[ConfigRow],
        texts: &[MetadataTextRow],
        predefined_rows: &[ConfigRow],
        legacy: Option<&LegacyNames<'_>>,
    ) -> Result<Self> {
        let raw_by_name = metadata_rows
            .iter()
            .map(|row| (row.file_name.as_str(), row))
            .collect::<HashMap<_, _>>();
        let mut report = ModelIndexReport {
            compatibility_mode: plan.compatibility_mode.clone(),
            ..ModelIndexReport::default()
        };

        // Every top-level object (and the root) of a kind with
        // `object_names`, in parallel.
        let named = parallel::install_memory_bound_or_inline(|| {
            texts
                .par_iter()
                .filter_map(|row| {
                    let kind = plan
                        .kinds
                        .get(row.file_name.as_str())
                        .copied()
                        .or_else(|| {
                            (plan.root.as_deref() == Some(row.file_name.as_str()))
                                .then_some("Configuration")
                        })?;
                    if !has_names(kind) {
                        return None;
                    }
                    let result = raw_by_name
                        .get(row.file_name.as_str())
                        .ok_or_else(|| anyhow!("no stored row"))
                        .and_then(|raw| raw_row_text(raw))
                        .and_then(|text| parse_row(&text))
                        .and_then(|tree| object_names(kind, &tree));
                    Some((kind, result))
                })
                .collect::<Vec<_>>()
        });
        let mut index = NameIndex::default();
        for (kind, result) in named {
            match result {
                Ok(names) => {
                    *report.rows_by_kind.entry(kind.to_string()).or_default() += 1;
                    index.add(&names);
                }
                Err(error) => {
                    let line = format!("{error:#}");
                    let line = line.lines().next().unwrap_or_default();
                    *report
                        .failures
                        .entry(format!("{kind}: {line}"))
                        .or_default() += 1;
                }
            }
        }
        if let Some(root) = &plan.root {
            index.insert_name(root, "Configuration");
        }

        // Top-level objects of the other kinds: the root row's kind and the
        // row's own name.
        for row in texts {
            if let Some(kind) = plan.kinds.get(row.file_name.as_str())
                && !has_names(kind)
            {
                *report
                    .legacy_rows_by_kind
                    .entry(kind.to_string())
                    .or_default() += 1;
                if let Some(header) = row.header.as_ref()
                    && index.insert_name(&row.file_name, &format!("{kind}.{}", header.name))
                {
                    report.legacy_names += 1;
                }
            }
        }

        // Owned objects under their owner's name, owners first.
        let texts_by_name = texts
            .iter()
            .map(|row| (row.file_name.as_str(), row))
            .collect::<HashMap<_, _>>();
        for (owner, kind, uuid) in &plan.owned {
            let Some(owner_name) = index.name(owner).map(str::to_string) else {
                continue;
            };
            // A kind that reads its owned rows gives the object, its children
            // and its generated types; the others their own name.
            if has_owned_names(kind) {
                let names = raw_by_name
                    .get(uuid.as_str())
                    .ok_or_else(|| anyhow!("no stored row"))
                    .and_then(|raw| raw_row_text(raw))
                    .and_then(|text| parse_row(&text))
                    .and_then(|tree| owned_object_names(kind, &tree, &owner_name));
                match names {
                    Ok(names) => {
                        index.set_name(&names.uuid, &names.full_name);
                        index.add(&names);
                        *report.rows_by_kind.entry(kind.to_string()).or_default() += 1;
                        report.owned_names += 1;
                        continue;
                    }
                    Err(error) => {
                        let line = format!("{error:#}");
                        let line = line.lines().next().unwrap_or_default();
                        *report
                            .failures
                            .entry(format!("{kind}: {line}"))
                            .or_default() += 1;
                    }
                }
            }
            let Some(header) = texts_by_name
                .get(uuid.as_str())
                .and_then(|row| row.header.as_ref())
            else {
                continue;
            };
            index.set_name(uuid, &format!("{owner_name}.{kind}.{}", header.name));
            report.owned_names += 1;
        }
        // While some kind's rows cannot name themselves: what the owners'
        // lists did not name from the legacy path indexes, and the children
        // and generated types of those kinds from the legacy object
        // references and type index.
        if let Some(legacy) = legacy {
            report.used_legacy_indexes = true;
            for (uuid, form_ref) in legacy.form_refs {
                if let Some(name) = form_source_reference_name(form_ref)
                    && index.insert_name(uuid, &name)
                {
                    report.legacy_owned_names += 1;
                }
            }
            for (uuid, template_ref) in legacy.template_refs {
                if let Some(name) = template_source_reference_name(template_ref)
                    && index.insert_name(uuid, &name)
                {
                    report.legacy_owned_names += 1;
                }
            }
            for (uuid, name) in legacy.object_refs {
                if root_kind(name) == "Configuration" {
                    continue;
                }
                if index.insert_name(uuid, name) {
                    report.legacy_names += 1;
                }
            }
            for (type_id, name) in legacy.type_index {
                let name = name.strip_prefix("cfg:").unwrap_or(name);
                if index.insert_type(type_id, name) {
                    report.legacy_types += 1;
                }
            }
        }

        // Predefined items, from the bodies, under their owner's full name.
        let items = parallel::install_memory_bound_or_inline(|| {
            predefined_rows
                .par_iter()
                .filter_map(|row| {
                    let (owner, suffix) = row.file_name.split_once('.')?;
                    let kind = *plan.kinds.get(owner)?;
                    (predefined_suffix(kind) == Some(suffix)).then_some(())?;
                    let text = raw_row_text(row).ok()?;
                    if text.is_empty() {
                        return None;
                    }
                    let body = parse_row(&text).ok()?;
                    Some((owner, predefined_items(kind, &body).ok()?))
                })
                .collect::<Vec<_>>()
        });
        report.predefined_bodies = items.len();
        for (owner, owner_items) in items {
            let Some(owner) = index.name(owner).map(str::to_string) else {
                continue;
            };
            for (uuid, name) in owner_items {
                index.insert_predefined(&owner, &uuid, &name);
            }
        }

        let (names, types, predefined) = index.sizes();
        report.names = names;
        report.types = types;
        report.predefined = predefined;
        Ok(Self {
            context: ExportContext {
                names: index,
                version: plan.version.clone(),
                compat: plan.compat,
            },
            plan,
            report,
        })
    }

    pub(super) fn names(&self) -> &NameIndex {
        &self.context.names
    }

    /// Validate known scalar domains without requiring a model output path.
    /// A generic decoder coverage gap still belongs to the legacy writer;
    /// a typed invalid value must never become that writer's guessed default.
    pub(super) fn validate_descriptor_values(&self, file_name: &str, stored: &[u8]) -> Result<()> {
        let Some(kind) = self
            .plan
            .kind_of(file_name)
            .filter(|kind| has_decoder(kind))
        else {
            return Ok(());
        };
        let result = (|| {
            let text = String::from_utf8(inflate_raw_deflate(stored)?)?;
            // Legacy readers apply these native-evidenced upgrades before
            // conversion. Validate the same stored values in that admitted
            // layout too, without changing the raw model writer's coverage.
            let text = upgrade::upgrade_record_by_shape(&text, file_name).unwrap_or(text);
            let text = upgrade::upgrade_metadata_record(kind, &text).unwrap_or(text);
            let tree = parse_row(text.as_bytes())?;
            decode_object(kind, &tree, &self.context)
        })();
        if let Err(error) = result
            && (kind == ibcmd_schema::interface::InterfaceLayout::KIND
                || error.is::<crate::metadata_model::export::values::UnsupportedDescriptorValue>())
        {
            return Err(error).context("stored descriptor has an unsupported value");
        }
        Ok(())
    }

    /// The descriptor XML of a row through the model, or `None` when its kind
    /// has no decoder: the legacy converter writes it then.
    pub(super) fn export_row(
        &self,
        row: &MetadataTextRow,
        stored: &[u8],
    ) -> Option<Result<ExtractedMetadataSourceXml>> {
        let kind = self.plan.kind_of(&row.file_name)?;
        if !has_decoder(kind) {
            return None;
        }
        let relative_path = self.relative_path(&row.file_name, kind)?;
        Some((|| {
            let text = inflate_raw_deflate(stored)?;
            let tree = parse_row(&text)?;
            let object = decode_object(kind, &tree, &self.context)?;
            let xml = write_document(&object, &self.context.version);
            Ok(ExtractedMetadataSourceXml {
                relative_path,
                xml: xml.into_bytes(),
            })
        })())
    }

    /// `Configuration.xml`, `Catalogs/X.xml`, `Catalogs/X/Forms/F.xml`,
    /// `Subsystems/A/Subsystems/B.xml`: the folders follow the full name.
    fn relative_path(&self, file_name: &str, kind: &str) -> Option<PathBuf> {
        if kind == "Configuration" {
            return Some(PathBuf::from("Configuration.xml"));
        }
        let full_name = self.context.names.name(file_name)?;
        let parts = full_name.split('.').collect::<Vec<_>>();
        if parts.len() < 2 || parts.len() % 2 != 0 {
            return None;
        }
        let mut path = PathBuf::new();
        for pair in parts.chunks(2) {
            path.push(kind_folder(pair[0])?);
            path.push(sanitize_source_path_segment(pair[1]));
        }
        Some(path.with_extension("xml"))
    }

    /// One line on stderr: what the index holds and where it came from.
    pub(super) fn log_summary(&self, elapsed_ms: u64) {
        if quiet_summaries() {
            return;
        }
        let report = &self.report;
        let rows = report.rows_by_kind.values().sum::<usize>();
        let failed = report.failures.values().sum::<usize>();
        eprintln!(
            "model export: name index of {} names, {} types, {} predefined items in {elapsed_ms} ms; \
             object_names read {rows} rows ({failed} failed) of {} kinds, {} owned objects named \
             through their owner; the legacy indexes {} ({} names, {} types)",
            report.names,
            report.types,
            report.predefined,
            report.rows_by_kind.len(),
            report.owned_names,
            if report.used_legacy_indexes {
                "consulted"
            } else {
                "not consulted"
            },
            report.legacy_names + report.legacy_owned_names,
            report.legacy_types,
        );
        for (failure, count) in report.failures.iter().take(10) {
            eprintln!("model export: object_names failed {count}x: {failure}");
        }
    }
}

// ---------------------------------------------------------------------------
// The platform an export is for.

/// Whether the configuration of `mssql-dump-config`'s source -- the database,
/// or the `--rows-dir` folder -- is kept in compatibility 8.5 or later: its
/// Configuration row stores the `{76,...}` tuple. Such a configuration can
/// only be 8.5.1's (the settings' export rule); an older one says nothing
/// about the platform, since 8.5 runs 8.3.27-compatible configurations too.
/// Resolves the export's published storage view, including a completed stage
/// for the main configuration, then reads the root service row and its Configuration row.
pub fn configuration_compatibility_8_5_or_later(args: &MssqlDumpConfigArgs) -> Result<bool> {
    configuration_compatibility_with_snapshot(args).map(|(compatible, _)| compatible)
}

/// Return any PostgreSQL probe rows to the resolving operation, never a global cache.
pub(crate) fn configuration_compatibility_with_snapshot(
    args: &MssqlDumpConfigArgs,
) -> Result<(bool, Option<PostgresExportSnapshot>)> {
    if args.rows_dir.is_none()
        && args.file_infobase.is_none()
        && let Some(target) = &args.postgres
    {
        let (rows, snapshot) = super::postgres_infobase::load_for_probe(target)?;
        let read = |name: &str| -> Result<crate::metadata_model::brace::Brace> {
            let names = BTreeSet::from([name.to_owned()]);
            let row = rows
                .rows_named(
                    &args.database,
                    MssqlConfigurationTableRole::Current.sql_name(),
                    &names,
                )?
                .into_iter()
                .find(|row| row.file_name == name)
                .ok_or_else(|| anyhow!("the Config table has no `{name}` row"))?;
            let text = inflate_raw_deflate(&row.binary)
                .with_context(|| format!("failed to inflate the `{name}` row"))?;
            parse_row(&text).with_context(|| format!("failed to parse the `{name}` row"))
        };
        return Ok((configuration_compatibility_from_read(read)?, Some(snapshot)));
    }
    let _offline = match (&args.rows_dir, &args.file_infobase) {
        (Some(dir), _) => Some(offline_rows::activate(dir)?),
        (None, Some(path)) => Some(offline_rows::activate_shared(
            super::file_infobase::read_rows(path)?,
        )?),
        (None, None) => None,
    };
    let password = sql_password(
        args.sql_user.as_deref(),
        args.sql_pwd.as_deref(),
        &args.sql_pwd_env,
    );
    // `--rows-dir` answers every read from its folder; no login is needed.
    let sql = if args.rows_dir.is_some() || args.file_infobase.is_some() || args.postgres.is_some()
    {
        crate::sql::SqlExec::detached("the rows come from a folder, a file infobase or PostgreSQL")
    } else {
        super::sql_for_export(args, password.as_deref())?
    };
    Ok((
        configuration_compatibility_from_storage(&sql, &args.database, args.main_configuration)?,
        None,
    ))
}

fn configuration_compatibility_from_storage(
    sql: &crate::sql::SqlExec,
    database: &str,
    main_configuration: bool,
) -> Result<bool> {
    let _views = dynamic_generation::StorageViewScope::begin(database);
    let read = |name: &str| -> Result<crate::metadata_model::brace::Brace> {
        let names = BTreeSet::from([name.to_string()]);
        let rows = fetch_binary_rows(
            sql,
            database,
            MssqlConfigurationTableRole::Current.sql_name(),
            &names,
            false,
        )?;
        let row = rows
            .into_iter()
            .find(|row| row.file_name == name)
            .ok_or_else(|| anyhow!("the Config table has no `{name}` row"))?;
        let text = inflate_raw_deflate(&row.binary)
            .with_context(|| format!("failed to inflate the `{name}` row"))?;
        parse_row(&text).with_context(|| format!("failed to parse the `{name}` row"))
    };
    // No row at all: the default configuration of an empty infobase
    // (#469), which keeps no compatibility mode to tell.
    let table = MssqlConfigurationTableRole::Current.sql_name();
    let selected = BTreeSet::new();
    let headers = fetch_row_headers(sql, database, table, &selected)?;
    let headers =
        install_storage_overlay(sql, database, table, &selected, headers, main_configuration)?;
    if headers.is_empty() {
        return Ok(false);
    }
    configuration_compatibility_from_read(read)
}

fn configuration_compatibility_from_read(
    read: impl Fn(&str) -> Result<crate::metadata_model::brace::Brace>,
) -> Result<bool> {
    let uuid = crate::metadata_model::root::export::root_configuration_uuid(&read("root")?)?;
    let shape = crate::metadata_model::root::export::stored_shape(&read(&uuid)?)?;
    Ok(shape == crate::metadata_model::root::ConfigurationShape::V76)
}

// ---------------------------------------------------------------------------
// Diagnostics.

/// A row of a modelled kind the model could not write: said once per row on
/// stderr (the first twenty), and the legacy converter writes it.
pub(super) fn report_fallback(file_name: &str, error: &anyhow::Error) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static REPORTED: AtomicUsize = AtomicUsize::new(0);
    if REPORTED.fetch_add(1, Ordering::Relaxed) < 20 {
        let text = format!("{error:#}");
        eprintln!(
            "model export: {file_name}: {}; the legacy converter writes it",
            text.lines().next().unwrap_or_default()
        );
    }
}

/// The bytes a descriptor's file gets (`write_source_xml_file`).
fn as_written(xml: &[u8], source_version: InfobaseConfigSourceVersion) -> Vec<u8> {
    let adapter = MssqlLegacyAdapter::from_legacy_selector(source_version);
    let mut normalized =
        normalize_legacy_source_asset_xml_version_bytes(xml, adapter.xml_dialect());
    if source_version == InfobaseConfigSourceVersion::V2_21 {
        normalized = declare_palette_namespace_beside_style(normalized);
    }
    normalized
}

/// Whether the legacy converter would have written the same file.
pub(super) fn same_as_written(
    legacy: Option<&ExtractedMetadataSourceXml>,
    modelled: &ExtractedMetadataSourceXml,
    source_version: InfobaseConfigSourceVersion,
) -> bool {
    legacy.is_some_and(|legacy| {
        legacy.relative_path == modelled.relative_path
            && as_written(&legacy.xml, source_version) == as_written(&modelled.xml, source_version)
    })
}

/// A modelled row whose file differs from the legacy one (the first twenty).
pub(super) fn report_shadow_difference(file_name: &str, relative_path: &Path) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static REPORTED: AtomicUsize = AtomicUsize::new(0);
    if REPORTED.fetch_add(1, Ordering::Relaxed) < 20 {
        eprintln!(
            "model export shadow: {file_name} ({}) differs from the legacy converter's file",
            relative_path.display()
        );
    }
}

// ---------------------------------------------------------------------------
// The audit: the row-built index against the tree's.

#[derive(Debug, Serialize)]
pub struct NameIndexAudit {
    pub tree_compatibility_mode: Option<String>,
    pub rows_compatibility_mode: Option<String>,
    pub rows: serde_json::Value,
    pub comparison: IndexComparison,
    pub equal: bool,
    pub build_rows_ms: u64,
    pub build_tree_ms: u64,
    /// Of `build_rows_ms`: reading the rows, the legacy indexes, the model's
    /// own plan and index.
    pub fetch_ms: u64,
    pub legacy_ms: u64,
    pub model_ms: u64,
    /// The legacy object references and type index against the row-built
    /// index: whether the body writers could read the latter instead.
    pub object_refs: LegacyMapComparison,
    pub type_index: LegacyMapComparison,
}

/// Builds the index from a folder of stored rows the way the export does and
/// compares it with the one the source tree gives.
pub fn audit_name_index(
    tree: &Path,
    rows_dir: &Path,
    source_version: InfobaseConfigSourceVersion,
    max_samples: usize,
) -> Result<NameIndexAudit> {
    let _offline = offline_rows::activate(rows_dir)?;
    let started = Instant::now();
    // The offline rows answer every read; nothing may reach a database.
    let sql = crate::sql::SqlExec::detached("the name-index audit reads a rows folder only");
    let metadata_rows = fetch_metadata_rows(&sql, "", "Config")?;
    let texts = build_metadata_text_rows_audited(&metadata_rows).rows;
    let fetch_ms = elapsed_ms(started);
    let legacy_started = Instant::now();
    let object_refs = build_metadata_object_reference_indexes_from_texts(&texts).references;
    let type_index = build_metadata_type_indexes_from_texts(&texts).references;
    let form_refs = build_complete_form_source_reference_index(&texts);
    let template_refs = build_template_source_reference_index_from_texts(&metadata_rows, &texts);
    let legacy_ms = elapsed_ms(legacy_started);
    let model_started = Instant::now();
    let plan = ModelPlan::new(&metadata_rows, &texts, source_version);
    let predefined_rows =
        fetch_config_rows(&sql, "", "Config", &plan.predefined_body_file_names())?;
    let legacy = LegacyNames {
        object_refs: &object_refs,
        type_index: &type_index,
        form_refs: &form_refs,
        template_refs: &template_refs,
    };
    let complete = plan.names_from_rows_alone(&texts);
    let model = ModelExport::build(
        plan,
        &metadata_rows,
        &texts,
        &predefined_rows,
        (!complete).then_some(&legacy),
    )?;
    let model_ms = elapsed_ms(model_started);
    let build_rows_ms = elapsed_ms(started);
    let started = Instant::now();
    let config_index = ConfigIndex::build(tree)?;
    let expected = NameIndex::from_config_index(&config_index);
    let build_tree_ms = elapsed_ms(started);
    let comparison = compare(&expected, model.names(), max_samples);
    let object_refs_comparison = compare_legacy_map(
        "object refs",
        &object_refs,
        model.names().name_entries(),
        max_samples,
    );
    let type_index_comparison = compare_legacy_map(
        "type index",
        &type_index
            .iter()
            .map(|(id, name)| {
                (
                    id.clone(),
                    name.strip_prefix("cfg:").unwrap_or(name).to_string(),
                )
            })
            .collect(),
        model.names().type_entries(),
        max_samples,
    );
    Ok(NameIndexAudit {
        tree_compatibility_mode: config_index.compatibility_mode.clone(),
        rows_compatibility_mode: model.report.compatibility_mode.clone(),
        rows: serde_json::to_value(&model.report)?,
        equal: comparison.is_equal(),
        comparison,
        build_rows_ms,
        build_tree_ms,
        fetch_ms,
        legacy_ms,
        model_ms,
        object_refs: object_refs_comparison,
        type_index: type_index_comparison,
    })
}

/// How a legacy uuid -> name map relates to the row-built index: the keys
/// both hold with the same or another name, and what each holds alone, by
/// the kind the name starts with and its second-to-last segment
/// (`Catalog/Attribute`).
#[derive(Debug, Default, Serialize)]
pub struct LegacyMapComparison {
    pub legacy: usize,
    pub index: usize,
    pub equal: usize,
    pub different: BTreeMap<String, usize>,
    pub legacy_only: BTreeMap<String, usize>,
    pub index_only: BTreeMap<String, usize>,
    pub samples: Vec<String>,
}

fn name_shape(name: &str) -> String {
    let parts = name.split('.').collect::<Vec<_>>();
    match parts.len() {
        0 | 1 => name.to_string(),
        2 => parts[0].to_string(),
        n => format!("{}/{}", parts[0], parts[n - 2]),
    }
}

fn compare_legacy_map<'a>(
    what: &str,
    legacy: &BTreeMap<String, String>,
    index: impl Iterator<Item = (&'a str, &'a str)>,
    max_samples: usize,
) -> LegacyMapComparison {
    let index = index.collect::<HashMap<_, _>>();
    let mut out = LegacyMapComparison {
        legacy: legacy.len(),
        index: index.len(),
        ..LegacyMapComparison::default()
    };
    let sample = |text: String, samples: &mut Vec<String>| {
        if samples.len() < max_samples * 8 {
            samples.push(format!("{what}: {text}"));
        }
    };
    for (uuid, name) in legacy {
        match index.get(uuid.as_str()) {
            Some(found) if *found == name.as_str() => out.equal += 1,
            Some(found) => {
                *out.different.entry(name_shape(name)).or_default() += 1;
                sample(
                    format!("{uuid}: legacy {name}, index {found}"),
                    &mut out.samples,
                );
            }
            None => {
                *out.legacy_only.entry(name_shape(name)).or_default() += 1;
                sample(format!("{uuid}: legacy only {name}"), &mut out.samples);
            }
        }
    }
    for (uuid, name) in &index {
        if !legacy.contains_key(*uuid) {
            *out.index_only.entry(name_shape(name)).or_default() += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A registered text answers a header search from its marker table
    /// exactly as the plain search does: the first `{1,0,..}` marker, else
    /// the first `{0,0,..}` one, and nothing for a uuid it does not hold.
    #[test]
    fn a_registered_text_finds_the_same_headers() {
        let first = "11111111-1111-1111-1111-111111111111";
        let second = "22222222-2222-2222-2222-222222222222";
        let third = "33333333-3333-3333-3333-333333333333";
        let mut text = String::from("{1,{3,");
        // Padding past the registration threshold.
        text.push_str(&"{\"#\",0},".repeat(700));
        text.push_str(&format!(
            "{{0,0,{first}}},\"Старое\",{{0}},\"\",{{1,0,{first}}},\"Имя\",{{1,\"ru\",\"Синоним\"}},\"Комментарий\",\
             {{1,0,{second}}},\"Второе\",{{0}},\"\",{{0,0,{third}}},\"Третье\",{{0}},\"x\",{{1,0,{second}}},\"Дубль\",{{0}},\"\"}}}}"
        ));
        assert!(text.len() >= BRACE_JUMPS_MIN_TEXT);
        let plain = [first, second, third, "44444444-4444-4444-4444-444444444444"]
            .map(|uuid| parse_metadata_header_from_text(&text, uuid));
        let _registered = register_brace_jumps([text.as_str()]);
        let jumped = [first, second, third, "44444444-4444-4444-4444-444444444444"]
            .map(|uuid| parse_metadata_header_from_text(&text, uuid));
        assert_eq!(plain, jumped);
        assert_eq!(
            jumped[0].as_ref().map(|header| header.name.as_str()),
            Some("Имя")
        );
        assert_eq!(
            jumped[1].as_ref().map(|header| header.name.as_str()),
            Some("Второе")
        );
        assert_eq!(
            jumped[2].as_ref().map(|header| header.comment.as_str()),
            Some("x")
        );
        assert_eq!(jumped[3], None);
    }

    #[test]
    fn kinds_go_to_their_folders() {
        for (kind, folder) in [
            ("Catalog", "Catalogs"),
            ("ChartOfAccounts", "ChartsOfAccounts"),
            ("BusinessProcess", "BusinessProcesses"),
            ("FilterCriterion", "FilterCriteria"),
            ("Form", "Forms"),
            ("Recalculation", "Recalculations"),
            ("Subsystem", "Subsystems"),
        ] {
            assert_eq!(kind_folder(kind).as_deref(), Some(folder), "{kind}");
        }
        assert_eq!(kind_folder("Configuration"), None);
    }
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;
    use crate::sql::{
        Dbms, ScriptVariables, SqlClient, SqlLogin, SqlParam, SqlRow, SqlTarget, SqlValue,
    };
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    struct StageClient {
        replies: Mutex<VecDeque<Vec<SqlRow>>>,
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl SqlClient for StageClient {
        fn dbms(&self) -> Dbms {
            Dbms::SqlServer
        }
        fn max_connections(&self) -> usize {
            1
        }
        fn read_rows(
            &self,
            query: &str,
            _: &[SqlParam<'_>],
            each: &mut dyn FnMut(SqlRow) -> Result<()>,
        ) -> Result<()> {
            self.calls.lock().unwrap().push(query.to_owned());
            for row in self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected SQL read")
            {
                each(row)?;
            }
            Ok(())
        }
        fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
            panic!("read-only probe")
        }
        fn execute(&self, _: &str, _: &[SqlParam<'_>]) -> Result<u64> {
            panic!("read-only probe")
        }
        fn query_json(&self, _: &str) -> Result<Option<String>> {
            panic!("unexpected JSON query")
        }
        fn write_rows(&self, _: &str, _: &[&str], _: &[Vec<SqlParam<'_>>]) -> Result<u64> {
            panic!("read-only probe")
        }
    }

    #[test]
    fn compatibility_probe_reads_a_completed_stage_over_empty_config() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/file-infobase/tiny-source");
        let path = root.join("Configuration.xml");
        let xml = fs::read(&path).unwrap();
        let facts = crate::metadata_model::root::configuration_facts(&xml).unwrap();
        let context = crate::metadata_model::DescriptorContext::new(&root, "2.21").unwrap();
        let packed = |bytes: Vec<u8>| crate::module_blob::deflate_raw(&bytes).unwrap();
        let descriptor = packed(
            crate::metadata_model::compile_descriptor("Configuration", &path, &xml, &context)
                .unwrap(),
        );
        let root_row = packed(crate::metadata_model::root::root_row(&facts));
        let versions = packed(crate::metadata_model::root::versions_row(
            &[facts.uuid.clone()],
            || "10000000-0000-4000-8000-000000000001".to_owned(),
        ));
        let header = |name: &str, size: usize| SqlRow {
            result_set: 0,
            values: vec![
                SqlValue::Text(name.to_owned()),
                SqlValue::Int(0),
                SqlValue::Int(size as i64),
            ],
        };
        let binary = |name: &str, bytes: Vec<u8>| SqlRow {
            result_set: 0,
            values: vec![
                SqlValue::Text(name.to_owned()),
                SqlValue::Int(0),
                SqlValue::Int(bytes.len() as i64),
                SqlValue::Binary(bytes),
            ],
        };
        let calls = Arc::new(Mutex::new(Vec::new()));
        let client = StageClient {
            calls: Arc::clone(&calls),
            replies: Mutex::new(VecDeque::from([
                vec![],
                vec![
                    header("root", root_row.len()),
                    header(&facts.uuid, descriptor.len()),
                    header("versions", versions.len()),
                ],
                vec![binary("versions", versions)],
                vec![binary("root", root_row)],
                vec![binary(&facts.uuid, descriptor)],
            ])),
        };
        let sql = crate::sql::SqlExec::with_client(
            SqlTarget {
                server: "memory-only".to_owned(),
                database: None,
                login: SqlLogin::Integrated,
                trust_server_certificate: false,
            },
            Box::new(client),
        );
        assert!(
            configuration_compatibility_from_storage(&sql, "compatibility-stage-test", true)
                .unwrap()
        );
        let calls = calls.lock().unwrap();
        assert!(
            calls
                .iter()
                .skip(1)
                .all(|query| query.contains("[ConfigSave]")),
            "{calls:?}"
        );
    }
}
