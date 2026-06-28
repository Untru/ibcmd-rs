use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use rayon::prelude::*;
use serde::Serialize;
use walkdir::WalkDir;

use crate::module_blob::{
    MetadataSourceContext, pack_moxel_spreadsheet_blob_from_xml_with_source,
    parse_simple_metadata_xml_properties, parse_template_type_from_xml,
};
use crate::mssql_dump::extract_moxel_spreadsheet_xml;
use crate::parallel;
use crate::source::{SourceKind, SourceManifest, scan_sources};

#[derive(Debug, Serialize)]
pub struct SpreadsheetTemplateAuditReport {
    pub root: PathBuf,
    pub template_xml_files: usize,
    pub spreadsheet_templates: usize,
    pub packed: usize,
    pub failed: usize,
    pub errors: Vec<SpreadsheetTemplateAuditError>,
}

#[derive(Debug, Serialize)]
pub struct SpreadsheetTemplateAuditError {
    pub metadata_xml: String,
    pub template_xml: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct SpreadsheetTemplateRoundTripAuditReport {
    pub root: PathBuf,
    pub template_xml_files: usize,
    pub spreadsheet_templates: usize,
    pub packed: usize,
    pub extracted: usize,
    pub repacked: usize,
    pub matched: usize,
    pub different: usize,
    pub failed: usize,
    pub errors: Vec<SpreadsheetTemplateRoundTripAuditError>,
}

#[derive(Debug, Serialize)]
pub struct SpreadsheetTemplateRoundTripAuditError {
    pub metadata_xml: String,
    pub template_xml: String,
    pub phase: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct FormSourceAuditReport {
    pub root: PathBuf,
    pub form_xml_files: usize,
    pub parsed: usize,
    pub failed: usize,
    pub total_xml_bytes: u64,
    pub max_xml_bytes: u64,
    pub max_xml_path: Option<String>,
    pub forms_with_module: usize,
    pub total_module_bytes: u64,
    pub forms_with_ext_form_files: usize,
    pub ext_form_files: usize,
    pub ext_form_bytes: u64,
    pub forms_stageable_by_current_loader: usize,
    pub forms_without_stageable_body: usize,
    pub unsupported_form_xml_files: usize,
    pub unsupported_form_xml_bytes: u64,
    pub forms_with_ignored_ext_form_files: usize,
    pub ignored_ext_form_files: usize,
    pub ignored_ext_form_bytes: u64,
    pub top_level_elements: Vec<FormElementCount>,
    pub elements: Vec<FormElementCount>,
    pub errors: Vec<FormSourceAuditError>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct FormElementCount {
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct FormSourceAuditError {
    pub form_xml: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct SourceLoadCoverageAuditReport {
    pub root: PathBuf,
    pub total_files: usize,
    pub total_bytes: u64,
    pub files_by_kind: Vec<SourceKindCount>,
    pub stage_entry_files: usize,
    pub stage_metadata_xml_files: usize,
    pub stage_common_module_xml_files: usize,
    pub potentially_stageable_body_files: usize,
    pub module_files: usize,
    pub supported_module_files: usize,
    pub supported_ext_body_files: usize,
    pub unsupported_form_xml_files: usize,
    pub unsupported_form_xml_bytes: u64,
    pub form_xml_stageable_by_module: usize,
    pub form_xml_without_stageable_module: usize,
    pub ignored_form_ext_files: usize,
    pub ignored_form_ext_bytes: u64,
    pub known_uncovered_files: usize,
    pub known_uncovered_bytes: u64,
    pub top_known_uncovered: Vec<SourceLoadCoverageItem>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SourceKindCount {
    pub kind: String,
    pub count: usize,
    pub bytes: u64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SourceLoadCoverageItem {
    pub path: String,
    pub bytes: u64,
    pub reason: String,
}

#[derive(Debug, Default)]
struct FormXmlShape {
    top_level_elements: BTreeMap<String, usize>,
    elements: BTreeMap<String, usize>,
}

#[derive(Debug, Default)]
struct SpreadsheetTemplateRoundTripItemAudit {
    packed: usize,
    extracted: usize,
    repacked: usize,
    matched: usize,
    different: usize,
    errors: Vec<SpreadsheetTemplateRoundTripAuditError>,
}

pub fn audit_source_load_coverage(root: &Path) -> Result<SourceLoadCoverageAuditReport> {
    let manifest = scan_sources(root)?;
    audit_source_load_coverage_from_manifest(&manifest)
}

pub fn audit_source_load_coverage_from_manifest(
    manifest: &SourceManifest,
) -> Result<SourceLoadCoverageAuditReport> {
    let mut files_by_kind = BTreeMap::<String, (usize, u64)>::new();
    let mut stage_metadata_xml_files = 0usize;
    let mut stage_common_module_xml_files = 0usize;
    let mut module_files = 0usize;
    let mut supported_module_files = 0usize;
    let mut supported_ext_body_files = 0usize;
    let mut unsupported_form_xml_files = 0usize;
    let mut unsupported_form_xml_bytes = 0u64;
    let mut form_xml_stageable_by_module = 0usize;
    let mut ignored_form_ext_files = 0usize;
    let mut ignored_form_ext_bytes = 0u64;
    let mut known_uncovered = Vec::new();

    for file in &manifest.files {
        let kind = source_kind_name(&file.kind).to_string();
        let entry = files_by_kind.entry(kind).or_default();
        entry.0 += 1;
        entry.1 += file.size_bytes;

        if is_stage_metadata_xml_path(&file.path) {
            stage_metadata_xml_files += 1;
        }
        if is_root_common_module_xml_path(&file.path) {
            stage_common_module_xml_files += 1;
        }

        if file.kind == SourceKind::Module {
            module_files += 1;
            if is_supported_module_file(&file.path) {
                supported_module_files += 1;
            }
        }
        if is_supported_ext_body_file(&file.path) {
            supported_ext_body_files += 1;
        }

        if is_form_ext_xml_path(&file.path) {
            unsupported_form_xml_files += 1;
            unsupported_form_xml_bytes += file.size_bytes;
            if form_module_path_exists(&manifest.files, &file.path) {
                form_xml_stageable_by_module += 1;
            }
            known_uncovered.push(SourceLoadCoverageItem {
                path: file.path.clone(),
                bytes: file.size_bytes,
                reason: "full Form.xml body is not compiled by current loader".to_string(),
            });
        } else if is_form_ext_non_module_file(&file.path) {
            ignored_form_ext_files += 1;
            ignored_form_ext_bytes += file.size_bytes;
            known_uncovered.push(SourceLoadCoverageItem {
                path: file.path.clone(),
                bytes: file.size_bytes,
                reason: "non-module file under Ext/Form is not loaded by current form body packer"
                    .to_string(),
            });
        } else if is_known_uncovered_configuration_asset(&file.path) {
            known_uncovered.push(SourceLoadCoverageItem {
                path: file.path.clone(),
                bytes: file.size_bytes,
                reason: "configuration asset is not routed by current loader".to_string(),
            });
        }
    }

    known_uncovered.sort_by(|left, right| {
        right
            .bytes
            .cmp(&left.bytes)
            .then_with(|| left.path.cmp(&right.path))
    });
    let known_uncovered_files = known_uncovered.len();
    let known_uncovered_bytes = known_uncovered.iter().map(|item| item.bytes).sum();
    known_uncovered.truncate(50);

    let stage_entry_files = stage_metadata_xml_files + stage_common_module_xml_files;
    let potentially_stageable_body_files = supported_module_files + supported_ext_body_files;
    let form_xml_without_stageable_module =
        unsupported_form_xml_files.saturating_sub(form_xml_stageable_by_module);

    Ok(SourceLoadCoverageAuditReport {
        root: manifest.root.clone(),
        total_files: manifest.files.len(),
        total_bytes: manifest.files.iter().map(|file| file.size_bytes).sum(),
        files_by_kind: sorted_source_kind_counts(files_by_kind),
        stage_entry_files,
        stage_metadata_xml_files,
        stage_common_module_xml_files,
        potentially_stageable_body_files,
        module_files,
        supported_module_files,
        supported_ext_body_files,
        unsupported_form_xml_files,
        unsupported_form_xml_bytes,
        form_xml_stageable_by_module,
        form_xml_without_stageable_module,
        ignored_form_ext_files,
        ignored_form_ext_bytes,
        known_uncovered_files,
        known_uncovered_bytes,
        top_known_uncovered: known_uncovered,
    })
}

pub fn audit_spreadsheet_templates(root: &Path) -> Result<SpreadsheetTemplateAuditReport> {
    if !root.is_dir() {
        return Err(anyhow!(
            "source root is not a directory: {}",
            root.display()
        ));
    }

    let root = fs::canonicalize(root)
        .with_context(|| format!("failed to canonicalize {}", root.display()))?;
    let source = MetadataSourceContext::new(root.clone());
    let mut template_xml_files = 0usize;
    let mut spreadsheet_templates = 0usize;
    let mut packed = 0usize;
    let mut errors = Vec::new();

    for entry in WalkDir::new(&root)
        .into_iter()
        .filter_entry(|entry| !is_ignored(entry.path()))
    {
        let entry = entry?;
        if !entry.file_type().is_file() || entry.file_name() != "Template.xml" {
            continue;
        }
        let template_xml = entry.path();
        let Some(metadata_xml) = template_metadata_xml_path(template_xml) else {
            continue;
        };
        template_xml_files += 1;
        let metadata = fs::read(&metadata_xml)
            .with_context(|| format!("failed to read {}", metadata_xml.display()))?;
        if parse_template_type_from_xml(&metadata)?.as_deref() != Some("SpreadsheetDocument") {
            continue;
        }
        spreadsheet_templates += 1;

        let xml = fs::read(template_xml)
            .with_context(|| format!("failed to read {}", template_xml.display()))?;
        match pack_moxel_spreadsheet_blob_from_xml_with_source(&xml, Some(&source)) {
            Ok(_) => packed += 1,
            Err(error) => errors.push(SpreadsheetTemplateAuditError {
                metadata_xml: relative_path_string(&root, &metadata_xml),
                template_xml: relative_path_string(&root, template_xml),
                message: error.to_string(),
            }),
        }
    }

    errors.sort_by(|left, right| left.template_xml.cmp(&right.template_xml));
    Ok(SpreadsheetTemplateAuditReport {
        root,
        template_xml_files,
        spreadsheet_templates,
        packed,
        failed: errors.len(),
        errors,
    })
}

fn sorted_source_kind_counts(counts: BTreeMap<String, (usize, u64)>) -> Vec<SourceKindCount> {
    let mut counts = counts
        .into_iter()
        .map(|(kind, (count, bytes))| SourceKindCount { kind, count, bytes })
        .collect::<Vec<_>>();
    counts.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.kind.cmp(&right.kind))
    });
    counts
}

fn source_kind_name(kind: &SourceKind) -> &'static str {
    match kind {
        SourceKind::ConfigurationRoot => "configuration_root",
        SourceKind::MetadataXml => "metadata_xml",
        SourceKind::Module => "module",
        SourceKind::Form => "form",
        SourceKind::Template => "template",
        SourceKind::Binary => "binary",
        SourceKind::OtherXml => "other_xml",
        SourceKind::Other => "other",
    }
}

fn is_stage_metadata_xml_path(path: &str) -> bool {
    is_configuration_metadata_xml_path(path)
        || is_root_metadata_xml_path(path)
        || is_template_metadata_xml_path(path)
        || is_form_metadata_xml_path(path)
}

fn is_configuration_metadata_xml_path(path: &str) -> bool {
    normalize_source_path(path).eq_ignore_ascii_case("Configuration.xml")
}

fn is_root_metadata_xml_path(path: &str) -> bool {
    let lower = normalize_source_path(path).to_ascii_lowercase();
    if !lower.ends_with(".xml") {
        return false;
    }
    let parts = lower.split('/').collect::<Vec<_>>();
    parts.len() == 2 && is_stage_root_metadata_collection(parts[0])
}

fn is_root_common_module_xml_path(path: &str) -> bool {
    let lower = normalize_source_path(path).to_ascii_lowercase();
    if !lower.ends_with(".xml") {
        return false;
    }
    let parts = lower.split('/').collect::<Vec<_>>();
    parts.len() == 2 && parts[0] == "commonmodules"
}

fn is_template_metadata_xml_path(path: &str) -> bool {
    let lower = normalize_source_path(path).to_ascii_lowercase();
    if !lower.ends_with(".xml") || lower.contains("/ext/") {
        return false;
    }
    let parts = lower.split('/').collect::<Vec<_>>();
    parts.len() >= 4 && parts[parts.len() - 2] == "templates"
}

fn is_form_metadata_xml_path(path: &str) -> bool {
    let lower = normalize_source_path(path).to_ascii_lowercase();
    if !lower.ends_with(".xml") || lower.contains("/ext/") {
        return false;
    }
    let parts = lower.split('/').collect::<Vec<_>>();
    parts.len() >= 4 && parts[parts.len() - 2] == "forms"
}

fn is_stage_root_metadata_collection(value: &str) -> bool {
    matches!(
        value,
        "catalogs"
            | "documents"
            | "informationregisters"
            | "accumulationregisters"
            | "accountingregisters"
            | "calculationregisters"
            | "chartsofcharacteristictypes"
            | "chartsofaccounts"
            | "chartsofcalculationtypes"
            | "chartsofcalculationregisters"
            | "commonforms"
            | "commonpictures"
            | "commontemplates"
            | "commonattributes"
            | "commandgroups"
            | "documentjournals"
            | "reports"
            | "dataprocessors"
            | "enums"
            | "exchangeplans"
            | "eventsubscriptions"
            | "filtercriteria"
            | "functionaloptions"
            | "functionaloptionsparameters"
            | "httpservices"
            | "languages"
            | "scheduledjobs"
            | "sessionparameters"
            | "settingsstorages"
            | "styleitems"
            | "styles"
            | "subsystems"
            | "roles"
            | "commoncommands"
            | "businessprocesses"
            | "bots"
            | "definedtypes"
            | "tasks"
            | "constants"
            | "documentnumerators"
            | "integrationservices"
            | "sequences"
            | "webservices"
            | "wsreferences"
            | "xdtopackages"
    )
}

fn is_supported_module_file(path: &str) -> bool {
    let lower = normalize_source_path(path).to_ascii_lowercase();
    matches!(
        lower.rsplit('/').next(),
        Some("module.bsl")
            | Some("managermodule.bsl")
            | Some("objectmodule.bsl")
            | Some("recordsetmodule.bsl")
            | Some("valuemanagermodule.bsl")
            | Some("commandmodule.bsl")
    )
}

fn is_supported_ext_body_file(path: &str) -> bool {
    let lower = normalize_source_path(path).to_ascii_lowercase();
    lower.ends_with("/ext/rights.xml")
        || lower.ends_with("/ext/schedule.xml")
        || lower.ends_with("/ext/template.xml")
        || lower.ends_with("/ext/template.txt")
        || lower.ends_with("/ext/template.bin")
        || lower.ends_with("/ext/picture.xml")
        || lower.ends_with("/ext/predefined.xml")
        || lower.ends_with("/ext/content.xml")
        || lower.ends_with("/ext/flowchart.xml")
        || lower.ends_with("/ext/help.xml")
        || lower.ends_with("/ext/commandinterface.xml")
        || lower.ends_with("/ext/style.xml")
        || lower == "ext/homepageworkarea.xml"
        || lower == "ext/mobileclientsignature.bin"
        || lower == "ext/mainsectioncommandinterface.xml"
        || lower == "ext/clientapplicationinterface.xml"
        || lower == "ext/standaloneconfigurationcontent.bin"
}

fn is_form_ext_xml_path(path: &str) -> bool {
    normalize_source_path(path)
        .to_ascii_lowercase()
        .ends_with("/ext/form.xml")
}

fn is_form_ext_non_module_file(path: &str) -> bool {
    let normalized = normalize_source_path(path);
    let lower = normalized.to_ascii_lowercase();
    lower.contains("/ext/form/")
        && !lower.ends_with("/ext/form/module.bsl")
        && !lower.ends_with("/ext/form.xml")
}

fn form_module_path_exists(files: &[crate::source::SourceFile], form_xml_path: &str) -> bool {
    let module_path = normalize_source_path(form_xml_path)
        .trim_end_matches("Form.xml")
        .to_string()
        + "Form/Module.bsl";
    files
        .iter()
        .any(|file| normalize_source_path(&file.path).eq_ignore_ascii_case(&module_path))
}

fn is_known_uncovered_configuration_asset(path: &str) -> bool {
    let lower = normalize_source_path(path).to_ascii_lowercase();
    matches!(lower.as_str(), "ext/additionalindexes.xml")
}

fn normalize_source_path(path: &str) -> String {
    path.replace('\\', "/")
}

pub fn audit_spreadsheet_template_roundtrip(
    root: &Path,
) -> Result<SpreadsheetTemplateRoundTripAuditReport> {
    if !root.is_dir() {
        return Err(anyhow!(
            "source root is not a directory: {}",
            root.display()
        ));
    }

    let root = fs::canonicalize(root)
        .with_context(|| format!("failed to canonicalize {}", root.display()))?;
    let source = MetadataSourceContext::new(root.clone());
    let object_refs = common_picture_object_refs(&root)?;
    let mut template_xml_files = 0usize;
    let mut templates = Vec::new();

    for entry in WalkDir::new(&root)
        .into_iter()
        .filter_entry(|entry| !is_ignored(entry.path()))
    {
        let entry = entry?;
        if !entry.file_type().is_file() || entry.file_name() != "Template.xml" {
            continue;
        }
        let template_xml = entry.path();
        let Some(metadata_xml) = template_metadata_xml_path(template_xml) else {
            continue;
        };
        template_xml_files += 1;
        let metadata = fs::read(&metadata_xml)
            .with_context(|| format!("failed to read {}", metadata_xml.display()))?;
        if parse_template_type_from_xml(&metadata)?.as_deref() != Some("SpreadsheetDocument") {
            continue;
        }
        templates.push((metadata_xml, template_xml.to_path_buf()));
    }

    let item_reports = parallel::install(|| {
        templates
            .par_iter()
            .map(|(metadata_xml, template_xml)| {
                audit_one_spreadsheet_template_roundtrip(
                    &root,
                    &source,
                    &object_refs,
                    metadata_xml,
                    template_xml,
                )
            })
            .collect::<Vec<_>>()
    })?;
    let spreadsheet_templates = templates.len();
    let mut packed = 0usize;
    let mut extracted = 0usize;
    let mut repacked = 0usize;
    let mut matched = 0usize;
    let mut different = 0usize;
    let mut errors = Vec::new();
    for report in item_reports {
        packed += report.packed;
        extracted += report.extracted;
        repacked += report.repacked;
        matched += report.matched;
        different += report.different;
        errors.extend(report.errors);
    }

    errors.sort_by(|left, right| {
        left.template_xml
            .cmp(&right.template_xml)
            .then(left.phase.cmp(&right.phase))
    });
    Ok(SpreadsheetTemplateRoundTripAuditReport {
        root,
        template_xml_files,
        spreadsheet_templates,
        packed,
        extracted,
        repacked,
        matched,
        different,
        failed: errors.len(),
        errors,
    })
}

pub fn audit_form_sources(root: &Path) -> Result<FormSourceAuditReport> {
    if !root.is_dir() {
        return Err(anyhow!(
            "source root is not a directory: {}",
            root.display()
        ));
    }

    let root = fs::canonicalize(root)
        .with_context(|| format!("failed to canonicalize {}", root.display()))?;
    let mut top_level_elements = BTreeMap::new();
    let mut elements = BTreeMap::new();
    let mut report = FormSourceAuditReport {
        root: root.clone(),
        form_xml_files: 0,
        parsed: 0,
        failed: 0,
        total_xml_bytes: 0,
        max_xml_bytes: 0,
        max_xml_path: None,
        forms_with_module: 0,
        total_module_bytes: 0,
        forms_with_ext_form_files: 0,
        ext_form_files: 0,
        ext_form_bytes: 0,
        forms_stageable_by_current_loader: 0,
        forms_without_stageable_body: 0,
        unsupported_form_xml_files: 0,
        unsupported_form_xml_bytes: 0,
        forms_with_ignored_ext_form_files: 0,
        ignored_ext_form_files: 0,
        ignored_ext_form_bytes: 0,
        top_level_elements: Vec::new(),
        elements: Vec::new(),
        errors: Vec::new(),
    };

    for entry in WalkDir::new(&root)
        .into_iter()
        .filter_entry(|entry| !is_ignored(entry.path()))
    {
        let entry = entry?;
        if !entry.file_type().is_file() || entry.file_name() != "Form.xml" {
            continue;
        }
        let form_xml = entry.path();
        if !is_form_ext_xml(form_xml) {
            continue;
        }
        report.form_xml_files += 1;
        let metadata = entry.metadata()?;
        let xml_len = metadata.len();
        report.total_xml_bytes += xml_len;
        if xml_len > report.max_xml_bytes {
            report.max_xml_bytes = xml_len;
            report.max_xml_path = Some(relative_path_string(&root, form_xml));
        }

        let form_ext_dir = form_xml.with_extension("");
        let module_path = form_ext_dir.join("Module.bsl");
        if let Ok(module_metadata) = fs::metadata(&module_path)
            && module_metadata.is_file()
        {
            report.forms_with_module += 1;
            report.total_module_bytes += module_metadata.len();
        }
        let (ext_files, ext_bytes, ignored_ext_files, ignored_ext_bytes) =
            count_form_ext_files(&form_ext_dir)?;
        if ext_files > 0 {
            report.forms_with_ext_form_files += 1;
            report.ext_form_files += ext_files;
            report.ext_form_bytes += ext_bytes;
        }
        if ignored_ext_files > 0 {
            report.forms_with_ignored_ext_form_files += 1;
            report.ignored_ext_form_files += ignored_ext_files;
            report.ignored_ext_form_bytes += ignored_ext_bytes;
        }

        let xml =
            fs::read(form_xml).with_context(|| format!("failed to read {}", form_xml.display()))?;
        match parse_form_xml_shape(&xml) {
            Ok(shape) => {
                report.parsed += 1;
                merge_counts(&mut top_level_elements, shape.top_level_elements);
                merge_counts(&mut elements, shape.elements);
            }
            Err(error) => {
                report.failed += 1;
                report.errors.push(FormSourceAuditError {
                    form_xml: relative_path_string(&root, form_xml),
                    message: error.to_string(),
                });
            }
        }
    }

    report
        .errors
        .sort_by(|left, right| left.form_xml.cmp(&right.form_xml));
    report.forms_stageable_by_current_loader = report.forms_with_module;
    report.forms_without_stageable_body = report
        .form_xml_files
        .saturating_sub(report.forms_stageable_by_current_loader);
    report.unsupported_form_xml_files = report.form_xml_files;
    report.unsupported_form_xml_bytes = report.total_xml_bytes;
    report.top_level_elements = sorted_element_counts(top_level_elements);
    report.elements = sorted_element_counts(elements);
    Ok(report)
}

fn is_form_ext_xml(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some("Form.xml")
        && path.parent().and_then(|parent| parent.file_name()) == Some(std::ffi::OsStr::new("Ext"))
}

fn count_form_ext_files(form_ext_dir: &Path) -> Result<(usize, u64, usize, u64)> {
    if !form_ext_dir.is_dir() {
        return Ok((0, 0, 0, 0));
    }
    let mut files = 0usize;
    let mut bytes = 0u64;
    let mut ignored_files = 0usize;
    let mut ignored_bytes = 0u64;
    for entry in WalkDir::new(form_ext_dir)
        .into_iter()
        .filter_entry(|entry| !is_ignored(entry.path()))
    {
        let entry = entry?;
        if entry.file_type().is_file() {
            let len = entry.metadata()?.len();
            files += 1;
            bytes += len;
            if !is_form_module_file(form_ext_dir, entry.path()) {
                ignored_files += 1;
                ignored_bytes += len;
            }
        }
    }
    Ok((files, bytes, ignored_files, ignored_bytes))
}

fn is_form_module_file(form_ext_dir: &Path, path: &Path) -> bool {
    path.parent() == Some(form_ext_dir)
        && path.file_name().and_then(|name| name.to_str()) == Some("Module.bsl")
}

fn parse_form_xml_shape(xml: &[u8]) -> Result<FormXmlShape> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut depth = 0usize;
    let mut shape = FormXmlShape::default();
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                push_form_element(&mut shape, &event, depth)?;
                depth += 1;
            }
            Ok(Event::Empty(event)) => {
                push_form_element(&mut shape, &event, depth)?;
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(anyhow!("invalid Form.xml: {error}")),
            _ => {}
        }
    }
    Ok(shape)
}

fn push_form_element(shape: &mut FormXmlShape, event: &BytesStart<'_>, depth: usize) -> Result<()> {
    let name = String::from_utf8_lossy(event.local_name().as_ref()).to_string();
    *shape.elements.entry(name.clone()).or_insert(0) += 1;
    if depth == 1 {
        *shape.top_level_elements.entry(name).or_insert(0) += 1;
    }
    Ok(())
}

fn merge_counts(target: &mut BTreeMap<String, usize>, source: BTreeMap<String, usize>) {
    for (key, count) in source {
        *target.entry(key).or_insert(0) += count;
    }
}

fn sorted_element_counts(counts: BTreeMap<String, usize>) -> Vec<FormElementCount> {
    let mut counts = counts
        .into_iter()
        .map(|(name, count)| FormElementCount { name, count })
        .collect::<Vec<_>>();
    counts.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.name.cmp(&right.name))
    });
    counts
}

fn audit_one_spreadsheet_template_roundtrip(
    root: &Path,
    source: &MetadataSourceContext,
    object_refs: &BTreeMap<String, String>,
    metadata_xml: &Path,
    template_xml: &Path,
) -> SpreadsheetTemplateRoundTripItemAudit {
    let mut report = SpreadsheetTemplateRoundTripItemAudit::default();
    let xml = match fs::read(template_xml) {
        Ok(xml) => xml,
        Err(error) => {
            push_roundtrip_error(
                root,
                metadata_xml,
                template_xml,
                "read",
                error,
                &mut report.errors,
            );
            return report;
        }
    };
    let first = match pack_moxel_spreadsheet_blob_from_xml_with_source(&xml, Some(source)) {
        Ok(blob) => {
            report.packed += 1;
            blob
        }
        Err(error) => {
            push_roundtrip_error(
                root,
                metadata_xml,
                template_xml,
                "pack",
                error,
                &mut report.errors,
            );
            return report;
        }
    };
    let extracted_xml = match extract_moxel_spreadsheet_xml(&first.blob, object_refs) {
        Some(xml) => {
            report.extracted += 1;
            xml
        }
        None => {
            report.errors.push(SpreadsheetTemplateRoundTripAuditError {
                metadata_xml: relative_path_string(root, metadata_xml),
                template_xml: relative_path_string(root, template_xml),
                phase: "extract".to_string(),
                message: "failed to extract SpreadsheetDocument XML from packed MOXCEL blob"
                    .to_string(),
            });
            return report;
        }
    };
    let second = match pack_moxel_spreadsheet_blob_from_xml_with_source(
        extracted_xml.as_bytes(),
        Some(source),
    ) {
        Ok(blob) => {
            report.repacked += 1;
            blob
        }
        Err(error) => {
            push_roundtrip_error(
                root,
                metadata_xml,
                template_xml,
                "repack",
                error,
                &mut report.errors,
            );
            return report;
        }
    };
    let second_extracted_xml = match extract_moxel_spreadsheet_xml(&second.blob, object_refs) {
        Some(xml) => xml,
        None => {
            report.errors.push(SpreadsheetTemplateRoundTripAuditError {
                metadata_xml: relative_path_string(root, metadata_xml),
                template_xml: relative_path_string(root, template_xml),
                phase: "extract-repacked".to_string(),
                message: "failed to extract SpreadsheetDocument XML from repacked MOXCEL blob"
                    .to_string(),
            });
            return report;
        }
    };
    if extracted_xml == second_extracted_xml {
        report.matched += 1;
    } else {
        report.different += 1;
        report.errors.push(SpreadsheetTemplateRoundTripAuditError {
            metadata_xml: relative_path_string(root, metadata_xml),
            template_xml: relative_path_string(root, template_xml),
            phase: "compare".to_string(),
            message: roundtrip_difference_message(&extracted_xml, &second_extracted_xml),
        });
    }
    report
}

fn template_metadata_xml_path(template_xml: &Path) -> Option<PathBuf> {
    if template_xml.file_name()? != "Template.xml" {
        return None;
    }
    let ext_dir = template_xml.parent()?;
    if ext_dir.file_name()? != "Ext" {
        return None;
    }
    let template_dir = ext_dir.parent()?;
    Some(template_dir.with_extension("xml"))
}

fn relative_path_string(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn is_ignored(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == ".git" || name == ".vscode" || name == "target")
}

fn common_picture_object_refs(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut refs = BTreeMap::new();
    let common_pictures = root.join("CommonPictures");
    if !common_pictures.is_dir() {
        return Ok(refs);
    }
    for entry in fs::read_dir(&common_pictures)
        .with_context(|| format!("failed to read {}", common_pictures.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|value| value.to_str()) != Some("xml") {
            continue;
        }
        let xml = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let properties = parse_simple_metadata_xml_properties(&xml)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        if properties.kind == "CommonPicture" {
            refs.insert(
                properties.uuid,
                format!("CommonPicture.{}", properties.name),
            );
        }
    }
    Ok(refs)
}

fn roundtrip_difference_message(first: &str, second: &str) -> String {
    let first_diff = first
        .bytes()
        .zip(second.bytes())
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| first.len().min(second.len()));
    format!(
        "round-trip SpreadsheetDocument XML differs: first_bytes={}, second_bytes={}, first_diff_offset={first_diff}, first_context=\"{}\", second_context=\"{}\"",
        first.len(),
        second.len(),
        diff_context(first, first_diff),
        diff_context(second, first_diff)
    )
}

fn diff_context(text: &str, byte_offset: usize) -> String {
    let center = floor_char_boundary(text, byte_offset.min(text.len()));
    let start = text[..center]
        .char_indices()
        .rev()
        .nth(80)
        .map(|(index, _)| index)
        .unwrap_or(0);
    let end = text[center..]
        .char_indices()
        .nth(80)
        .map(|(index, _)| center + index)
        .unwrap_or(text.len());
    text[start..end]
        .chars()
        .flat_map(|ch| ch.escape_default())
        .collect()
}

fn floor_char_boundary(text: &str, mut offset: usize) -> usize {
    offset = offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn push_roundtrip_error(
    root: &Path,
    metadata_xml: &Path,
    template_xml: &Path,
    phase: &str,
    error: impl std::fmt::Display,
    errors: &mut Vec<SpreadsheetTemplateRoundTripAuditError>,
) {
    errors.push(SpreadsheetTemplateRoundTripAuditError {
        metadata_xml: relative_path_string(root, metadata_xml),
        template_xml: relative_path_string(root, template_xml),
        phase: phase.to_string(),
        message: error.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audits_spreadsheet_templates_and_reports_errors() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-audit-spreadsheet-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(root.join("CommonTemplates/Good/Ext"))?;
        fs::create_dir_all(root.join("CommonTemplates/Broken/Ext"))?;
        fs::write(
            root.join("CommonTemplates/Good.xml"),
            br#"
<MetaDataObject>
  <CommonTemplate uuid="aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa">
    <Properties>
      <Name>Good</Name>
      <Synonym/>
      <Comment/>
      <TemplateType>SpreadsheetDocument</TemplateType>
    </Properties>
  </CommonTemplate>
</MetaDataObject>
"#,
        )?;
        fs::write(
            root.join("CommonTemplates/Good/Ext/Template.xml"),
            br#"
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:v8="http://v8.1c.ru/8.1/data/core">
  <columns><size>1</size></columns>
  <rowsItem>
    <index>0</index>
    <row><empty>true</empty></row>
  </rowsItem>
</document>
"#,
        )?;
        fs::write(
            root.join("CommonTemplates/Broken.xml"),
            br#"
<MetaDataObject>
  <CommonTemplate uuid="bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb">
    <Properties>
      <Name>Broken</Name>
      <Synonym/>
      <Comment/>
      <TemplateType>SpreadsheetDocument</TemplateType>
    </Properties>
  </CommonTemplate>
</MetaDataObject>
"#,
        )?;
        fs::write(
            root.join("CommonTemplates/Broken/Ext/Template.xml"),
            br#"<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet"/>"#,
        )?;

        let report = audit_spreadsheet_templates(&root)?;

        assert_eq!(report.template_xml_files, 2);
        assert_eq!(report.spreadsheet_templates, 2);
        assert_eq!(report.packed, 1);
        assert_eq!(report.failed, 1);
        assert_eq!(
            report.errors[0].template_xml,
            "CommonTemplates/Broken/Ext/Template.xml"
        );
        assert!(
            report.errors[0]
                .message
                .contains("SpreadsheetDocument XML has no rowsItem entries")
        );

        Ok(())
    }

    #[test]
    fn audits_spreadsheet_template_roundtrip_matches_stable_moxel() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-audit-spreadsheet-roundtrip-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(root.join("CommonTemplates/Good/Ext"))?;
        fs::write(
            root.join("CommonTemplates/Good.xml"),
            br#"
<MetaDataObject>
  <CommonTemplate uuid="aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa">
    <Properties>
      <Name>Good</Name>
      <Synonym/>
      <Comment/>
      <TemplateType>SpreadsheetDocument</TemplateType>
    </Properties>
  </CommonTemplate>
</MetaDataObject>
"#,
        )?;
        fs::write(
            root.join("CommonTemplates/Good/Ext/Template.xml"),
            br#"
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet">
  <columns><size>1</size></columns>
	<rowsItem>
		<index>0</index>
		<row>
			<c>
				<c>
					<f>0</f>
					<tl>
						<v8:item>
							<v8:lang>ru</v8:lang>
							<v8:content>Hello</v8:content>
						</v8:item>
					</tl>
				</c>
			</c>
		</row>
	</rowsItem>
</document>
"#,
        )?;

        let report = audit_spreadsheet_template_roundtrip(&root)?;

        assert_eq!(report.template_xml_files, 1);
        assert_eq!(report.spreadsheet_templates, 1);
        assert_eq!(report.packed, 1);
        assert_eq!(report.extracted, 1);
        assert_eq!(report.repacked, 1);
        assert_eq!(report.matched, 1);
        assert_eq!(report.different, 0);
        assert_eq!(report.failed, 0);
        assert!(report.errors.is_empty());

        Ok(())
    }

    #[test]
    fn audits_form_sources_counts_modules_assets_and_xml_shape() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-audit-forms-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(root.join("Catalogs/Products/Forms/ListForm/Ext/Form/Items/Icon"))?;
        fs::create_dir_all(root.join("CommonForms/Broken/Ext"))?;
        fs::write(
            root.join("Catalogs/Products/Forms/ListForm/Ext/Form.xml"),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
  <Attributes>
    <Attribute name="Item"/>
  </Attributes>
  <Items>
    <Item name="List"/>
  </Items>
  <Commands/>
</Form>
"#,
        )?;
        fs::write(
            root.join("Catalogs/Products/Forms/ListForm/Ext/Form/Module.bsl"),
            b"\xef\xbb\xbf&AtClient\nProcedure Open(Command)\nEndProcedure\n",
        )?;
        fs::write(
            root.join("Catalogs/Products/Forms/ListForm/Ext/Form/Items/Icon/Picture.png"),
            b"\x89PNG\r\n\x1a\n",
        )?;
        fs::write(
            root.join("CommonForms/Broken/Ext/Form.xml"),
            br#"<Form><Items></Form>"#,
        )?;

        let report = audit_form_sources(&root)?;

        assert_eq!(report.form_xml_files, 2);
        assert_eq!(report.parsed, 1);
        assert_eq!(report.failed, 1);
        assert_eq!(report.forms_with_module, 1);
        assert_eq!(report.forms_with_ext_form_files, 1);
        assert_eq!(report.ext_form_files, 2);
        assert_eq!(report.forms_stageable_by_current_loader, 1);
        assert_eq!(report.forms_without_stageable_body, 1);
        assert_eq!(report.unsupported_form_xml_files, 2);
        assert!(report.unsupported_form_xml_bytes > 0);
        assert_eq!(report.forms_with_ignored_ext_form_files, 1);
        assert_eq!(report.ignored_ext_form_files, 1);
        assert_eq!(report.ignored_ext_form_bytes, 8);
        assert!(report.top_level_elements.contains(&FormElementCount {
            name: "Attributes".to_string(),
            count: 1
        }));
        assert!(report.top_level_elements.contains(&FormElementCount {
            name: "Items".to_string(),
            count: 1
        }));
        assert!(report.top_level_elements.contains(&FormElementCount {
            name: "Commands".to_string(),
            count: 1
        }));
        assert!(report.elements.contains(&FormElementCount {
            name: "Item".to_string(),
            count: 1
        }));
        assert_eq!(report.errors[0].form_xml, "CommonForms/Broken/Ext/Form.xml");

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn audits_source_load_coverage_marks_stage_entries_and_uncovered_forms() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-audit-load-coverage-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        fs::create_dir_all(root.join("Catalogs/Products/Forms/ListForm/Ext/Form/Items/Icon"))?;
        fs::create_dir_all(root.join("Catalogs/Products/Ext"))?;
        fs::create_dir_all(root.join("CommonModules/Foo/Ext"))?;
        fs::create_dir_all(root.join("Ext"))?;
        fs::write(
            root.join("Catalogs/Products.xml"),
            br#"<MetaDataObject><Catalog uuid="11111111-1111-4111-8111-111111111111"/></MetaDataObject>"#,
        )?;
        fs::write(
            root.join("Catalogs/Products/Forms/ListForm.xml"),
            br#"<MetaDataObject><Form uuid="22222222-2222-4222-8222-222222222222"/></MetaDataObject>"#,
        )?;
        fs::write(
            root.join("Catalogs/Products/Forms/ListForm/Ext/Form.xml"),
            br#"<Form><Attributes/></Form>"#,
        )?;
        fs::write(
            root.join("Catalogs/Products/Forms/ListForm/Ext/Form/Module.bsl"),
            b"Procedure Open(Command)\nEndProcedure\n",
        )?;
        fs::write(
            root.join("Catalogs/Products/Forms/ListForm/Ext/Form/Items/Icon/Picture.png"),
            b"\x89PNG\r\n\x1a\n",
        )?;
        fs::write(
            root.join("Catalogs/Products/Ext/Predefined.xml"),
            b"<Data/>",
        )?;
        fs::write(
            root.join("CommonModules/Foo.xml"),
            br#"<MetaDataObject><CommonModule uuid="33333333-3333-4333-8333-333333333333"/></MetaDataObject>"#,
        )?;
        fs::write(
            root.join("CommonModules/Foo/Ext/Module.bsl"),
            b"Procedure Run()\nEndProcedure\n",
        )?;
        fs::write(
            root.join("Ext/MobileClientSignature.bin"),
            b"{2,\"\",\"\",{0},0}",
        )?;
        fs::write(
            root.join("Ext/MainSectionCommandInterface.xml"),
            b"<CommandInterface/>",
        )?;
        fs::write(
            root.join("Ext/HomePageWorkArea.xml"),
            b"<HomePageWorkArea/>",
        )?;
        fs::write(
            root.join("Ext/ClientApplicationInterface.xml"),
            b"<ClientApplicationInterface/>",
        )?;
        fs::write(
            root.join("Ext/StandaloneConfigurationContent.bin"),
            b"<StandaloneContent/>",
        )?;

        let report = audit_source_load_coverage(&root)?;

        assert_eq!(report.total_files, 13);
        assert_eq!(report.stage_metadata_xml_files, 2);
        assert_eq!(report.stage_common_module_xml_files, 1);
        assert_eq!(report.stage_entry_files, 3);
        assert_eq!(report.module_files, 2);
        assert_eq!(report.supported_module_files, 2);
        assert_eq!(report.supported_ext_body_files, 6);
        assert_eq!(report.potentially_stageable_body_files, 8);
        assert_eq!(report.unsupported_form_xml_files, 1);
        assert_eq!(report.form_xml_stageable_by_module, 1);
        assert_eq!(report.form_xml_without_stageable_module, 0);
        assert_eq!(report.ignored_form_ext_files, 1);
        assert_eq!(report.known_uncovered_files, 2);
        assert!(
            report
                .top_known_uncovered
                .iter()
                .any(|item| item.path == "Catalogs/Products/Forms/ListForm/Ext/Form.xml")
        );

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn roundtrip_difference_message_handles_utf8_offsets() {
        let message =
            roundtrip_difference_message("prefix Привет suffix", "prefix Проверка suffix");

        assert!(message.contains("first_diff_offset="));
        assert!(message.contains("first_context="));
        assert!(message.contains("second_context="));
    }
}
