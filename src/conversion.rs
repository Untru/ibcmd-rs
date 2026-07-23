//! Platform-independent configuration conversion orchestration.
//!
//! Every supported route uses explicit artifact formats and exact profile IDs.
//! The module contains no executable discovery and never starts 1C, EDT, Java,
//! or any other subprocess.

use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    fs::{self, File},
    io::Cursor,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use ibcmd_cf::{
    archive::{CfArchive, decode_archive_uniform},
    bootstrap::{
        BootstrapCfProfile, assemble_bootstrap_artifact, publish_bootstrap_patch_new,
        validate_bootstrap_artifact, write_bootstrap_artifact,
    },
    export::StorageExportDisposition,
    payload::PayloadEncoding,
    writer::{publish_repacked_new, validate_repacked_archive, write_archive},
};
use ibcmd_core::{
    artifact::ProfileId,
    diagnostic::{LossPolicy, ObjectPath, PathSegment},
    family::FamilyId,
    limits::ResourceLimits,
    migration::{
        executor::{MigrationExecutionRequest, MigrationExecutor},
        graph::{MigrationGraph, MigrationPlan},
        report::MigrationReport,
        v2_20_to_v2_21::V2_20ToV2_21,
        v2_21_to_v2_20::V2_21ToV2_20,
    },
    profile::{EffectiveProfile, ProfileRegistry},
    storage::{StorageImage, StorageProvenance},
    validate::validate_configuration,
};
use ibcmd_v8::format::Revision;
use ibcmd_xml::{
    DialectDetection, DialectRegistry, MetadataEnvelope, MetadataRegistry, XmlDocument, XmlNode,
    XmlReader, bundled_metadata_registry,
    metadata::decode_configuration_envelope,
    source_tree::{SourceEntry, SourceKind, SourcePath, SourceTree, publish_new, read_source_tree},
};
use serde::Serialize;

use crate::{
    cli::{
        CfCompression, CfRevision, ConversionFormat, ConversionLossPolicy, ConvertArgs,
        InfobaseConfigSourceVersion,
    },
    compiler::bootstrap::compile_bootstrap_source_tree,
    mssql_dump::export_storage_image_to_source,
    profile_registry::{BUNDLED_PROFILES, ProfileRegistryLimits, load_profile_registry},
};

const REPORT_SCHEMA_VERSION: u32 = 1;
const PHASE_DECODE: &str = "decode";
const PHASE_VALIDATE: &str = "validate";
const PHASE_PLAN: &str = "migration_plan";
const PHASE_MIGRATE: &str = "migrate";
const PHASE_PREFLIGHT: &str = "encode_preflight";
const PHASE_ENCODE: &str = "atomic_encode";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversionPhaseStatus {
    Pending,
    Completed,
    Failed,
    SkippedDryRun,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConversionPhaseReport {
    pub phase: &'static str,
    pub status: ConversionPhaseStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConversionEndpointReport {
    pub format: &'static str,
    pub profile: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConversionPlanReport {
    pub kind: &'static str,
    pub route_profiles: Vec<String>,
    pub steps: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FileMigrationReport {
    pub path: String,
    pub report: MigrationReport,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConversionPreflightReport {
    pub source_entries: usize,
    pub target_entries: usize,
    pub target_bytes: u64,
    pub opaque_entries: usize,
    pub structural_entries: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConversionPublicationReport {
    pub artifact: &'static str,
    pub entries: usize,
    pub bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cf_revision: Option<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConversionDiagnostic {
    pub code: String,
    pub phase: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConversionReport {
    pub schema_version: u32,
    pub command: &'static str,
    pub ok: bool,
    pub dry_run: bool,
    pub input: String,
    pub output: String,
    pub source: ConversionEndpointReport,
    pub target: ConversionEndpointReport,
    pub loss_policy: LossPolicy,
    pub phases: Vec<ConversionPhaseReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<ConversionPlanReport>,
    pub migrations: Vec<FileMigrationReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preflight: Option<ConversionPreflightReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publication: Option<ConversionPublicationReport>,
    pub output_published: bool,
    pub errors: Vec<ConversionDiagnostic>,
}

impl ConversionReport {
    fn new(args: &ConvertArgs) -> Self {
        Self {
            schema_version: REPORT_SCHEMA_VERSION,
            command: "convert",
            ok: false,
            dry_run: args.dry_run,
            input: display_path(&args.input),
            output: display_path(&args.output),
            source: ConversionEndpointReport {
                format: args.source_format.as_str(),
                profile: args.source_profile.clone(),
            },
            target: ConversionEndpointReport {
                format: args.target_format.as_str(),
                profile: args.target_profile.clone(),
            },
            loss_policy: loss_policy(args.loss),
            phases: [
                PHASE_DECODE,
                PHASE_VALIDATE,
                PHASE_PLAN,
                PHASE_MIGRATE,
                PHASE_PREFLIGHT,
                PHASE_ENCODE,
            ]
            .into_iter()
            .map(|phase| ConversionPhaseReport {
                phase,
                status: ConversionPhaseStatus::Pending,
            })
            .collect(),
            plan: None,
            migrations: Vec::new(),
            preflight: None,
            publication: None,
            output_published: false,
            errors: Vec::new(),
        }
    }

    fn mark(&mut self, phase: &'static str, status: ConversionPhaseStatus) {
        self.phases
            .iter_mut()
            .find(|entry| entry.phase == phase)
            .expect("conversion phase list is complete")
            .status = status;
    }
}

#[derive(Debug)]
pub struct ConversionError {
    report: Box<ConversionReport>,
}

impl ConversionError {
    #[must_use]
    pub const fn report(&self) -> &ConversionReport {
        &self.report
    }
}

impl Display for ConversionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self.report.errors.first() {
            Some(error) => formatter.write_str(&error.message),
            None => formatter.write_str("offline conversion failed"),
        }
    }
}

impl Error for ConversionError {}

/// Runs one completely offline conversion request.
pub fn convert(args: &ConvertArgs) -> std::result::Result<ConversionReport, ConversionError> {
    let mut report = ConversionReport::new(args);
    if artifact_path_conflict(args) {
        return Err(failure(
            &mut report,
            PHASE_DECODE,
            "conversion.artifact-path-conflict",
            "destination must not replace or be created inside the source artifact".to_owned(),
            Some(display_path(&args.output)),
        ));
    }
    if let Some(report_path) = &args.report
        && report_path_conflict(
            report_path,
            &args.input,
            &args.output,
            args.source_format.as_str(),
            args.target_format.as_str(),
        )
    {
        return Err(failure(
            &mut report,
            PHASE_DECODE,
            "conversion.report-path-conflict",
            "report path must not replace or modify the source or converted artifact".to_owned(),
            Some(display_path(report_path)),
        ));
    }
    let profiles = load_profile_registry(
        BUNDLED_PROFILES,
        args.profile_dir.as_deref(),
        ProfileRegistryLimits::default(),
    )
    .map_err(|error| {
        failure(
            &mut report,
            PHASE_DECODE,
            "conversion.profile-registry-failed",
            format!("failed to load conversion profiles: {error:#}"),
            None,
        )
    })?;
    let source_id = parse_profile_id(&mut report, PHASE_DECODE, "source", &args.source_profile)?;
    let target_id = parse_profile_id(&mut report, PHASE_DECODE, "target", &args.target_profile)?;
    let source_profile = find_profile(&mut report, &profiles, "source", &source_id)?;
    let target_profile = find_profile(&mut report, &profiles, "target", &target_id)?;
    require_profile_coordinate(&mut report, args.source_format, "source", source_profile)?;
    require_profile_coordinate(&mut report, args.target_format, "target", target_profile)?;

    match (args.source_format, args.target_format) {
        (ConversionFormat::Xml, ConversionFormat::Xml) => {
            convert_xml_to_xml(args, &profiles, source_profile, target_profile, report)
        }
        (ConversionFormat::Xml, ConversionFormat::Cf) => {
            convert_xml_to_cf(args, &profiles, source_profile, target_profile, report)
        }
        (ConversionFormat::Cf, ConversionFormat::Xml) => {
            convert_cf_to_xml(args, &profiles, source_profile, target_profile, report)
        }
        (ConversionFormat::Cf, ConversionFormat::Cf) => {
            convert_cf_to_cf(args, source_profile, target_profile, report)
        }
    }
}

/// Writes a requested diagnostic report. The conversion artifact itself still
/// follows create-new/no-clobber publication; this explicit report path is a
/// conventional replaceable command output.
pub fn write_report(report: &ConversionReport, path: &Path) -> Result<()> {
    if report_path_conflict(
        path,
        Path::new(&report.input),
        Path::new(&report.output),
        report.source.format,
        report.target.format,
    ) {
        bail!("report path conflicts with the source or converted artifact");
    }
    let mut bytes = serde_json::to_vec_pretty(report).context("failed to serialize report")?;
    bytes.push(b'\n');
    fs::write(path, bytes)
        .with_context(|| format!("failed to write conversion report `{}`", path.display()))
}

fn parse_profile_id(
    report: &mut ConversionReport,
    phase: &'static str,
    endpoint: &'static str,
    value: &str,
) -> std::result::Result<ProfileId, ConversionError> {
    ProfileId::parse(value).map_err(|error| {
        failure(
            report,
            phase,
            "conversion.invalid-profile-id",
            format!("invalid {endpoint} profile `{value}`: {error}"),
            None,
        )
    })
}

fn find_profile<'a>(
    report: &mut ConversionReport,
    profiles: &'a ProfileRegistry,
    endpoint: &'static str,
    id: &ProfileId,
) -> std::result::Result<&'a EffectiveProfile, ConversionError> {
    profiles.get(id).ok_or_else(|| {
        failure(
            report,
            PHASE_DECODE,
            "conversion.profile-not-found",
            format!("{endpoint} profile `{id}` was not found"),
            None,
        )
    })
}

fn require_profile_coordinate(
    report: &mut ConversionReport,
    format: ConversionFormat,
    endpoint: &'static str,
    profile: &EffectiveProfile,
) -> std::result::Result<(), ConversionError> {
    let valid = match format {
        ConversionFormat::Xml => {
            profile.xml_dialect.is_some()
                && profile.platform_build.is_none()
                && profile.storage_profile.is_none()
        }
        ConversionFormat::Cf => {
            profile.platform_build.is_some() && profile.storage_profile.is_some()
        }
    };
    if valid {
        Ok(())
    } else {
        Err(failure(
            report,
            PHASE_DECODE,
            "conversion.profile-format-mismatch",
            format!(
                "{endpoint} profile `{}` does not provide the independent coordinates required by {}",
                profile.id,
                format.as_str()
            ),
            None,
        ))
    }
}

enum DecodedXmlEntry {
    Metadata {
        path: SourcePath,
        envelope: Box<MetadataEnvelope>,
    },
    Passthrough {
        path: SourcePath,
        kind: SourceKind,
        bytes: Vec<u8>,
    },
}

enum MigratedXmlEntry {
    Metadata {
        path: SourcePath,
        envelope: Box<MetadataEnvelope>,
    },
    Passthrough {
        path: SourcePath,
        bytes: Vec<u8>,
    },
}

fn decode_xml_tree(
    tree: &SourceTree,
    profile: &EffectiveProfile,
    dialects: &DialectRegistry,
    codecs: &MetadataRegistry,
) -> std::result::Result<Vec<DecodedXmlEntry>, (String, String)> {
    let mut decoded = Vec::with_capacity(tree.entries().len());
    for (index, source) in tree.entries().iter().enumerate() {
        if !matches!(
            source.kind(),
            SourceKind::ConfigurationRoot | SourceKind::MetadataXml
        ) {
            decoded.push(DecodedXmlEntry::Passthrough {
                path: source.path().clone(),
                kind: source.kind(),
                bytes: source.bytes().to_vec(),
            });
            continue;
        }
        let document = XmlReader::from_slice(source.bytes())
            .map_err(|error| (source.path().to_string(), error.to_string()))?;
        validate_dialect(&document, dialects, &profile.id)
            .map_err(|message| (source.path().to_string(), message))?;
        let family =
            metadata_family(&document).map_err(|message| (source.path().to_string(), message))?;
        let index = u32::try_from(index).map_err(|_| {
            (
                source.path().to_string(),
                "source index exceeds canonical path range".to_owned(),
            )
        })?;
        let object_path = ObjectPath::new(vec![
            PathSegment::name("source").expect("static object path is valid"),
            PathSegment::index(index),
        ])
        .expect("bounded source tree produces a bounded object path");
        let envelope = if family.as_str() == "Configuration" {
            decode_configuration_envelope(&document, profile.id.clone(), object_path)
        } else {
            codecs.decode(&family, &document, profile.id.clone(), object_path)
        }
        .map_err(|error| (source.path().to_string(), error.to_string()))?;
        decoded.push(DecodedXmlEntry::Metadata {
            path: source.path().clone(),
            envelope: Box::new(envelope),
        });
    }
    Ok(decoded)
}

fn validate_decoded_xml(
    decoded: &[DecodedXmlEntry],
    cross_profile: bool,
) -> std::result::Result<(), (Option<String>, String)> {
    for entry in decoded {
        match entry {
            DecodedXmlEntry::Metadata { path, envelope } => {
                let configuration = envelope.configuration().map_err(|error| {
                    (
                        Some(path.to_string()),
                        format!("invalid canonical model: {error}"),
                    )
                })?;
                validate_configuration(&configuration).map_err(|diagnostics| {
                    (
                        Some(path.to_string()),
                        format!("canonical validation failed: {diagnostics:?}"),
                    )
                })?;
            }
            DecodedXmlEntry::Passthrough { path, kind, .. }
                if cross_profile && !matches!(kind, SourceKind::Module | SourceKind::Binary) =>
            {
                return Err((
                    Some(path.to_string()),
                    "cross-profile conversion has no verified adapter for this source asset"
                        .to_owned(),
                ));
            }
            DecodedXmlEntry::Passthrough { .. } => {}
        }
    }
    Ok(())
}

fn convert_xml_to_xml(
    args: &ConvertArgs,
    profiles: &ProfileRegistry,
    source_profile: &EffectiveProfile,
    target_profile: &EffectiveProfile,
    mut report: ConversionReport,
) -> std::result::Result<ConversionReport, ConversionError> {
    let tree = read_source_tree(&args.input).map_err(|error| {
        failure(
            &mut report,
            PHASE_DECODE,
            "conversion.xml-decode-failed",
            format!("failed to read XML source tree: {error}"),
            Some(display_path(&args.input)),
        )
    })?;
    let dialects = DialectRegistry::from_profiles(profiles).map_err(|error| {
        failure(
            &mut report,
            PHASE_DECODE,
            "conversion.xml-dialect-registry-failed",
            error.to_string(),
            None,
        )
    })?;
    let codecs = bundled_metadata_registry();
    let decoded =
        decode_xml_tree(&tree, source_profile, &dialects, &codecs).map_err(|(path, message)| {
            failure(
                &mut report,
                PHASE_DECODE,
                "conversion.xml-decode-failed",
                message,
                Some(path),
            )
        })?;
    report.mark(PHASE_DECODE, ConversionPhaseStatus::Completed);

    validate_decoded_xml(&decoded, source_profile.id != target_profile.id).map_err(
        |(path, message)| {
            failure(
                &mut report,
                PHASE_VALIDATE,
                "conversion.xml-validation-failed",
                message,
                path,
            )
        },
    )?;
    report.mark(PHASE_VALIDATE, ConversionPhaseStatus::Completed);

    let graph = migration_graph(profiles).map_err(|message| {
        failure(
            &mut report,
            PHASE_PLAN,
            "conversion.migration-graph-failed",
            message,
            None,
        )
    })?;
    let plan = graph
        .plan(source_profile, target_profile)
        .map_err(|error| {
            failure(
                &mut report,
                PHASE_PLAN,
                error.code(),
                error.to_string(),
                None,
            )
        })?;
    report.plan = Some(graph_plan_report(&plan));
    report.mark(PHASE_PLAN, ConversionPhaseStatus::Completed);

    let mut migrated = Vec::with_capacity(decoded.len());
    for entry in decoded {
        match entry {
            DecodedXmlEntry::Metadata { path, envelope } => {
                let source_configuration = envelope.configuration().map_err(|error| {
                    failure(
                        &mut report,
                        PHASE_MIGRATE,
                        "conversion.canonical-model-failed",
                        error.to_string(),
                        Some(path.to_string()),
                    )
                })?;
                let execution =
                    MigrationExecutor::new(&graph).execute(MigrationExecutionRequest::new(
                        &plan,
                        &source_configuration,
                        loss_policy(args.loss),
                    ));
                let execution = match execution {
                    Ok(execution) => execution,
                    Err(error) => {
                        if let Some(migration) = error.report() {
                            report.migrations.push(FileMigrationReport {
                                path: path.to_string(),
                                report: migration.clone(),
                            });
                        }
                        return Err(failure(
                            &mut report,
                            PHASE_MIGRATE,
                            error.code(),
                            error.to_string(),
                            Some(path.to_string()),
                        ));
                    }
                };
                let (configuration, migration_report) = execution.into_parts();
                report.migrations.push(FileMigrationReport {
                    path: path.to_string(),
                    report: migration_report,
                });
                let envelope = if plan.is_empty() {
                    envelope
                } else {
                    let mut objects = configuration.into_objects();
                    let root = objects.remove(0);
                    Box::new((*envelope).with_model(root, objects).map_err(|error| {
                        failure(
                            &mut report,
                            PHASE_MIGRATE,
                            "conversion.xml-envelope-rebuild-failed",
                            error.to_string(),
                            Some(path.to_string()),
                        )
                    })?)
                };
                migrated.push(MigratedXmlEntry::Metadata { path, envelope });
            }
            DecodedXmlEntry::Passthrough { path, bytes, .. } => {
                migrated.push(MigratedXmlEntry::Passthrough { path, bytes });
            }
        }
    }
    report.mark(PHASE_MIGRATE, ConversionPhaseStatus::Completed);

    let mut encoded_entries = Vec::with_capacity(migrated.len());
    for entry in migrated {
        match entry {
            MigratedXmlEntry::Metadata { path, envelope } => {
                let bytes = codecs
                    .encode(&envelope, &target_profile.id)
                    .map_err(|error| {
                        failure(
                            &mut report,
                            PHASE_PREFLIGHT,
                            "conversion.xml-encode-failed",
                            error.to_string(),
                            Some(path.to_string()),
                        )
                    })?;
                let document = XmlReader::from_slice(&bytes).map_err(|error| {
                    failure(
                        &mut report,
                        PHASE_PREFLIGHT,
                        "conversion.xml-encode-invalid",
                        error.to_string(),
                        Some(path.to_string()),
                    )
                })?;
                validate_dialect(&document, &dialects, &target_profile.id).map_err(|message| {
                    failure(
                        &mut report,
                        PHASE_PREFLIGHT,
                        "conversion.xml-target-profile-mismatch",
                        message,
                        Some(path.to_string()),
                    )
                })?;
                encoded_entries.push(SourceEntry::from_bytes(path.clone(), bytes).map_err(
                    |error| {
                        failure(
                            &mut report,
                            PHASE_PREFLIGHT,
                            "conversion.xml-target-tree-invalid",
                            error.to_string(),
                            Some(path.to_string()),
                        )
                    },
                )?);
            }
            MigratedXmlEntry::Passthrough { path, bytes } => {
                encoded_entries.push(SourceEntry::from_bytes(path.clone(), bytes).map_err(
                    |error| {
                        failure(
                            &mut report,
                            PHASE_PREFLIGHT,
                            "conversion.xml-target-tree-invalid",
                            error.to_string(),
                            Some(path.to_string()),
                        )
                    },
                )?);
            }
        }
    }
    let target_tree = SourceTree::new(encoded_entries).map_err(|error| {
        failure(
            &mut report,
            PHASE_PREFLIGHT,
            "conversion.xml-target-tree-invalid",
            error.to_string(),
            None,
        )
    })?;
    ensure_destination_absent(&args.output).map_err(|message| {
        failure(
            &mut report,
            PHASE_PREFLIGHT,
            "conversion.destination-exists",
            message,
            Some(display_path(&args.output)),
        )
    })?;
    let bytes = source_tree_bytes(&target_tree);
    report.preflight = Some(ConversionPreflightReport {
        source_entries: tree.entries().len(),
        target_entries: target_tree.entries().len(),
        target_bytes: bytes,
        opaque_entries: 0,
        structural_entries: 0,
    });
    report.mark(PHASE_PREFLIGHT, ConversionPhaseStatus::Completed);

    if args.dry_run {
        report.mark(PHASE_ENCODE, ConversionPhaseStatus::SkippedDryRun);
    } else {
        publish_new(&target_tree, &args.output).map_err(|error| {
            failure(
                &mut report,
                PHASE_ENCODE,
                "conversion.xml-publication-failed",
                error.to_string(),
                Some(display_path(&args.output)),
            )
        })?;
        report.publication = Some(ConversionPublicationReport {
            artifact: "xml",
            entries: target_tree.entries().len(),
            bytes,
            cf_revision: None,
        });
        report.output_published = true;
        report.mark(PHASE_ENCODE, ConversionPhaseStatus::Completed);
    }
    report.ok = true;
    Ok(report)
}

fn convert_xml_to_cf(
    args: &ConvertArgs,
    profiles: &ProfileRegistry,
    source_profile: &EffectiveProfile,
    target_profile: &EffectiveProfile,
    mut report: ConversionReport,
) -> std::result::Result<ConversionReport, ConversionError> {
    let tree = read_source_tree(&args.input).map_err(|error| {
        failure(
            &mut report,
            PHASE_DECODE,
            "conversion.xml-decode-failed",
            error.to_string(),
            Some(display_path(&args.input)),
        )
    })?;
    let dialects = DialectRegistry::from_profiles(profiles).map_err(|error| {
        failure(
            &mut report,
            PHASE_DECODE,
            "conversion.xml-dialect-registry-failed",
            error.to_string(),
            None,
        )
    })?;
    let codecs = bundled_metadata_registry();
    let decoded =
        decode_xml_tree(&tree, source_profile, &dialects, &codecs).map_err(|(path, message)| {
            failure(
                &mut report,
                PHASE_DECODE,
                "conversion.xml-decode-failed",
                message,
                Some(path),
            )
        })?;
    report.mark(PHASE_DECODE, ConversionPhaseStatus::Completed);
    validate_decoded_xml(&decoded, false).map_err(|(path, message)| {
        failure(
            &mut report,
            PHASE_VALIDATE,
            "conversion.xml-validation-failed",
            message,
            path,
        )
    })?;
    report.mark(PHASE_VALIDATE, ConversionPhaseStatus::Completed);
    report.plan = Some(direct_plan(
        source_profile,
        target_profile,
        "adapter:xml-to-cf",
    ));
    report.mark(PHASE_PLAN, ConversionPhaseStatus::Completed);
    report.mark(PHASE_MIGRATE, ConversionPhaseStatus::Completed);

    let dialect = source_profile
        .xml_dialect
        .as_ref()
        .expect("XML endpoint was validated")
        .value
        .clone();
    let compilation =
        compile_bootstrap_source_tree(&tree, dialect, target_profile).map_err(|error| {
            failure(
                &mut report,
                PHASE_PREFLIGHT,
                "conversion.cf-bootstrap-compile-failed",
                error.to_string(),
                None,
            )
        })?;
    let source_entries = compilation.source_files();
    let target_entries = compilation.patch().len();
    let revision = cli_revision(args.target_revision);
    let mut cf_profile = BootstrapCfProfile::new(
        revision,
        args.target_storage_version,
        compilation.storage_profile().clone(),
    )
    .with_reserved(args.target_reserved);
    if let Some(page_size) = args.target_page_size {
        cf_profile = cf_profile.with_page_size(page_size);
    }
    let patch = compilation.into_patch();
    let artifact =
        assemble_bootstrap_artifact(patch.clone(), cf_profile.clone(), ResourceLimits::default())
            .map_err(|error| {
            failure(
                &mut report,
                PHASE_PREFLIGHT,
                "conversion.cf-preflight-failed",
                error.to_string(),
                None,
            )
        })?;
    let mut bytes = Vec::new();
    let write = write_bootstrap_artifact(&mut bytes, &artifact).map_err(|error| {
        failure(
            &mut report,
            PHASE_PREFLIGHT,
            "conversion.cf-preflight-write-failed",
            error.to_string(),
            None,
        )
    })?;
    let validation =
        validate_bootstrap_artifact(Cursor::new(&bytes), &artifact, ResourceLimits::default())
            .map_err(|error| {
                failure(
                    &mut report,
                    PHASE_PREFLIGHT,
                    "conversion.cf-preflight-validation-failed",
                    error.to_string(),
                    None,
                )
            })?;
    if write.entries_written != validation.entries_validated {
        return Err(failure(
            &mut report,
            PHASE_PREFLIGHT,
            "conversion.cf-preflight-count-mismatch",
            "CF preflight write and validation entry counts differ".to_owned(),
            None,
        ));
    }
    ensure_destination_absent(&args.output).map_err(|message| {
        failure(
            &mut report,
            PHASE_PREFLIGHT,
            "conversion.destination-exists",
            message,
            Some(display_path(&args.output)),
        )
    })?;
    report.preflight = Some(ConversionPreflightReport {
        source_entries,
        target_entries,
        target_bytes: write.bytes_written,
        opaque_entries: 0,
        structural_entries: 0,
    });
    report.mark(PHASE_PREFLIGHT, ConversionPhaseStatus::Completed);

    if args.dry_run {
        report.mark(PHASE_ENCODE, ConversionPhaseStatus::SkippedDryRun);
    } else {
        let publication =
            publish_bootstrap_patch_new(patch, cf_profile, &args.output, ResourceLimits::default())
                .map_err(|error| {
                    failure(
                        &mut report,
                        PHASE_ENCODE,
                        "conversion.cf-publication-failed",
                        error.to_string(),
                        Some(display_path(&args.output)),
                    )
                })?;
        report.publication = Some(ConversionPublicationReport {
            artifact: "cf",
            entries: publication.validation.entries_validated,
            bytes: publication.published_bytes,
            cf_revision: Some(revision_name(revision)),
        });
        report.output_published = true;
        report.mark(PHASE_ENCODE, ConversionPhaseStatus::Completed);
    }
    report.ok = true;
    Ok(report)
}

fn convert_cf_to_xml(
    args: &ConvertArgs,
    profiles: &ProfileRegistry,
    source_profile: &EffectiveProfile,
    target_profile: &EffectiveProfile,
    mut report: ConversionReport,
) -> std::result::Result<ConversionReport, ConversionError> {
    let archive = decode_cf(args, source_profile).map_err(|message| {
        failure(
            &mut report,
            PHASE_DECODE,
            "conversion.cf-decode-failed",
            message,
            Some(display_path(&args.input)),
        )
    })?;
    report.mark(PHASE_DECODE, ConversionPhaseStatus::Completed);
    StorageImage::new(archive.image().entries().to_vec()).map_err(|error| {
        failure(
            &mut report,
            PHASE_VALIDATE,
            "conversion.cf-validation-failed",
            error.to_string(),
            None,
        )
    })?;
    report.mark(PHASE_VALIDATE, ConversionPhaseStatus::Completed);
    report.plan = Some(direct_plan(
        source_profile,
        target_profile,
        "adapter:cf-to-xml",
    ));
    report.mark(PHASE_PLAN, ConversionPhaseStatus::Completed);
    report.mark(PHASE_MIGRATE, ConversionPhaseStatus::Completed);

    let selector = legacy_xml_selector(target_profile).ok_or_else(|| {
        failure(
            &mut report,
            PHASE_PREFLIGHT,
            "conversion.xml-target-adapter-missing",
            format!(
                "CF export has no native target adapter for profile `{}`",
                target_profile.id
            ),
            None,
        )
    })?;
    let staging = TemporaryDirectory::new("cf-to-xml").map_err(|error| {
        failure(
            &mut report,
            PHASE_PREFLIGHT,
            "conversion.staging-failed",
            error.to_string(),
            None,
        )
    })?;
    let export = export_storage_image_to_source(archive.image(), staging.path(), false, selector)
        .map_err(|error| {
        failure(
            &mut report,
            PHASE_PREFLIGHT,
            "conversion.cf-export-failed",
            format!("{error:#}"),
            None,
        )
    })?;
    if export.storage.failed != 0 {
        return Err(failure(
            &mut report,
            PHASE_PREFLIGHT,
            "conversion.cf-export-entry-failed",
            format!(
                "{} CF storage entries failed XML export",
                export.storage.failed
            ),
            None,
        ));
    }
    let structural_entries = export
        .storage
        .entries
        .iter()
        .filter(|entry| {
            entry.disposition == StorageExportDisposition::Opaque
                && is_regenerated_structural_entry(&entry.logical_key)
        })
        .count();
    let unsafe_opaque = export.storage.opaque.saturating_sub(structural_entries);
    if unsafe_opaque != 0 {
        return Err(failure(
            &mut report,
            PHASE_PREFLIGHT,
            "conversion.cf-opaque-entry",
            format!(
                "{} CF storage entries have no lossless XML adapter",
                unsafe_opaque
            ),
            None,
        ));
    }
    let target_tree = read_source_tree(staging.path()).map_err(|error| {
        failure(
            &mut report,
            PHASE_PREFLIGHT,
            "conversion.xml-target-tree-invalid",
            error.to_string(),
            None,
        )
    })?;
    let dialects = DialectRegistry::from_profiles(profiles).map_err(|error| {
        failure(
            &mut report,
            PHASE_PREFLIGHT,
            "conversion.xml-dialect-registry-failed",
            error.to_string(),
            None,
        )
    })?;
    let codecs = bundled_metadata_registry();
    let decoded = decode_xml_tree(&target_tree, target_profile, &dialects, &codecs).map_err(
        |(path, message)| {
            failure(
                &mut report,
                PHASE_PREFLIGHT,
                "conversion.xml-target-validation-failed",
                message,
                Some(path),
            )
        },
    )?;
    validate_decoded_xml(&decoded, false).map_err(|(path, message)| {
        failure(
            &mut report,
            PHASE_PREFLIGHT,
            "conversion.xml-target-validation-failed",
            message,
            path,
        )
    })?;
    ensure_destination_absent(&args.output).map_err(|message| {
        failure(
            &mut report,
            PHASE_PREFLIGHT,
            "conversion.destination-exists",
            message,
            Some(display_path(&args.output)),
        )
    })?;
    let target_bytes = source_tree_bytes(&target_tree);
    report.preflight = Some(ConversionPreflightReport {
        source_entries: archive.image().len(),
        target_entries: target_tree.entries().len(),
        target_bytes,
        opaque_entries: 0,
        structural_entries,
    });
    report.mark(PHASE_PREFLIGHT, ConversionPhaseStatus::Completed);

    if args.dry_run {
        report.mark(PHASE_ENCODE, ConversionPhaseStatus::SkippedDryRun);
    } else {
        publish_new(&target_tree, &args.output).map_err(|error| {
            failure(
                &mut report,
                PHASE_ENCODE,
                "conversion.xml-publication-failed",
                error.to_string(),
                Some(display_path(&args.output)),
            )
        })?;
        report.publication = Some(ConversionPublicationReport {
            artifact: "xml",
            entries: target_tree.entries().len(),
            bytes: target_bytes,
            cf_revision: None,
        });
        report.output_published = true;
        report.mark(PHASE_ENCODE, ConversionPhaseStatus::Completed);
    }
    report.ok = true;
    Ok(report)
}

fn convert_cf_to_cf(
    args: &ConvertArgs,
    source_profile: &EffectiveProfile,
    target_profile: &EffectiveProfile,
    mut report: ConversionReport,
) -> std::result::Result<ConversionReport, ConversionError> {
    let archive = decode_cf(args, source_profile).map_err(|message| {
        failure(
            &mut report,
            PHASE_DECODE,
            "conversion.cf-decode-failed",
            message,
            Some(display_path(&args.input)),
        )
    })?;
    report.mark(PHASE_DECODE, ConversionPhaseStatus::Completed);
    StorageImage::new(archive.image().entries().to_vec()).map_err(|error| {
        failure(
            &mut report,
            PHASE_VALIDATE,
            "conversion.cf-validation-failed",
            error.to_string(),
            None,
        )
    })?;
    report.mark(PHASE_VALIDATE, ConversionPhaseStatus::Completed);
    if source_profile.id != target_profile.id {
        return Err(failure(
            &mut report,
            PHASE_PLAN,
            "conversion.cf-profile-migration-missing",
            format!(
                "no verified CF migration path exists from `{}` to `{}`",
                source_profile.id, target_profile.id
            ),
            None,
        ));
    }
    report.plan = Some(ConversionPlanReport {
        kind: "lossless_repack",
        route_profiles: vec![source_profile.id.to_string()],
        steps: vec!["adapter:cf-lossless-repack".to_owned()],
    });
    report.mark(PHASE_PLAN, ConversionPhaseStatus::Completed);
    report.mark(PHASE_MIGRATE, ConversionPhaseStatus::Completed);

    let mut bytes = Vec::new();
    let write =
        write_archive(&mut bytes, &archive, ResourceLimits::default()).map_err(|error| {
            failure(
                &mut report,
                PHASE_PREFLIGHT,
                "conversion.cf-preflight-write-failed",
                error.to_string(),
                None,
            )
        })?;
    let validation = validate_repacked_archive(
        Cursor::new(&bytes),
        archive.metadata(),
        archive.image(),
        ResourceLimits::default(),
    )
    .map_err(|error| {
        failure(
            &mut report,
            PHASE_PREFLIGHT,
            "conversion.cf-preflight-validation-failed",
            error.to_string(),
            None,
        )
    })?;
    ensure_destination_absent(&args.output).map_err(|message| {
        failure(
            &mut report,
            PHASE_PREFLIGHT,
            "conversion.destination-exists",
            message,
            Some(display_path(&args.output)),
        )
    })?;
    report.preflight = Some(ConversionPreflightReport {
        source_entries: archive.image().len(),
        target_entries: validation.entries_validated,
        target_bytes: write.bytes_written,
        opaque_entries: 0,
        structural_entries: 0,
    });
    report.mark(PHASE_PREFLIGHT, ConversionPhaseStatus::Completed);

    if args.dry_run {
        report.mark(PHASE_ENCODE, ConversionPhaseStatus::SkippedDryRun);
    } else {
        let publication = publish_repacked_new(&archive, &args.output, ResourceLimits::default())
            .map_err(|error| {
            failure(
                &mut report,
                PHASE_ENCODE,
                "conversion.cf-publication-failed",
                error.to_string(),
                Some(display_path(&args.output)),
            )
        })?;
        report.publication = Some(ConversionPublicationReport {
            artifact: "cf",
            entries: publication.validation.entries_validated,
            bytes: publication.published_bytes,
            cf_revision: Some(revision_name(archive.metadata().revision())),
        });
        report.output_published = true;
        report.mark(PHASE_ENCODE, ConversionPhaseStatus::Completed);
    }
    report.ok = true;
    Ok(report)
}

fn migration_graph(profiles: &ProfileRegistry) -> std::result::Result<MigrationGraph, String> {
    MigrationGraph::new(
        profiles,
        vec![V2_20ToV2_21::verified_edge(), V2_21ToV2_20::verified_edge()],
    )
    .map_err(|error| error.to_string())
}

fn graph_plan_report(plan: &MigrationPlan) -> ConversionPlanReport {
    ConversionPlanReport {
        kind: "migration_graph",
        route_profiles: plan
            .route_profiles()
            .iter()
            .map(ToString::to_string)
            .collect(),
        steps: plan.step_ids().iter().map(ToString::to_string).collect(),
    }
}

fn direct_plan(
    source: &EffectiveProfile,
    target: &EffectiveProfile,
    step: &'static str,
) -> ConversionPlanReport {
    ConversionPlanReport {
        kind: "direct_adapter",
        route_profiles: vec![source.id.to_string(), target.id.to_string()],
        steps: vec![step.to_owned()],
    }
}

fn decode_cf(
    args: &ConvertArgs,
    source_profile: &EffectiveProfile,
) -> std::result::Result<CfArchive, String> {
    let storage_profile = source_profile
        .storage_profile
        .as_ref()
        .expect("CF endpoint was validated")
        .value
        .clone();
    let source = File::open(&args.input)
        .map_err(|error| format!("failed to open `{}`: {error}", args.input.display()))?;
    let provenance = StorageProvenance::new("offline conversion service")
        .expect("static conversion provenance is valid");
    decode_archive_uniform(
        source,
        ResourceLimits::default(),
        storage_profile,
        provenance,
        payload_encoding(args.source_compression),
    )
    .map_err(|error| error.to_string())
}

fn validate_dialect(
    document: &XmlDocument,
    dialects: &DialectRegistry,
    profile: &ProfileId,
) -> std::result::Result<(), String> {
    let detection = dialects
        .detect(document)
        .map_err(|error| format!("XML dialect detection failed: {error}"))?;
    let matches = match detection {
        DialectDetection::Exact { candidate, .. } => candidate.profile_id() == profile,
        DialectDetection::Ambiguous { candidates, .. } => candidates
            .iter()
            .any(|candidate| candidate.profile_id() == profile),
        DialectDetection::Unknown { .. } => false,
    };
    matches.then_some(()).ok_or_else(|| {
        format!("XML dialect evidence is incompatible with selected profile `{profile}`")
    })
}

fn metadata_family(document: &XmlDocument) -> std::result::Result<FamilyId, String> {
    if document.root().name().local() != "MetaDataObject" {
        return Err("metadata source root is not MetaDataObject".to_owned());
    }
    let mut elements = document.root().children().iter().filter_map(|node| {
        if let XmlNode::Element(element) = node {
            Some(element)
        } else {
            None
        }
    });
    let family = elements
        .next()
        .ok_or_else(|| "MetaDataObject has no metadata element".to_owned())?;
    if elements.next().is_some() {
        return Err("MetaDataObject contains more than one metadata element".to_owned());
    }
    FamilyId::parse(family.name().local()).map_err(|error| error.to_string())
}

fn legacy_xml_selector(profile: &EffectiveProfile) -> Option<InfobaseConfigSourceVersion> {
    match profile.xml_dialect.as_ref()?.value.to_string().as_str() {
        "2.20" => Some(InfobaseConfigSourceVersion::V2_20),
        "2.21" => Some(InfobaseConfigSourceVersion::V2_21),
        _ => None,
    }
}

fn payload_encoding(compression: CfCompression) -> PayloadEncoding {
    match compression {
        CfCompression::RawDeflate => PayloadEncoding::RawDeflate,
        CfCompression::Stored => PayloadEncoding::Stored,
    }
}

fn cli_revision(revision: CfRevision) -> Revision {
    match revision {
        CfRevision::Format15 => Revision::Format15,
        CfRevision::Format16 => Revision::Format16,
    }
}

fn revision_name(revision: Revision) -> &'static str {
    match revision {
        Revision::Format15 => "format15",
        Revision::Format16 => "format16",
    }
}

fn loss_policy(policy: ConversionLossPolicy) -> LossPolicy {
    match policy {
        ConversionLossPolicy::Error => LossPolicy::Error,
        ConversionLossPolicy::Warn => LossPolicy::Warn,
        ConversionLossPolicy::Drop => LossPolicy::DropExplicitly,
    }
}

fn source_tree_bytes(tree: &SourceTree) -> u64 {
    tree.entries()
        .iter()
        .map(|entry| u64::try_from(entry.bytes().len()).unwrap_or(u64::MAX))
        .fold(0_u64, u64::saturating_add)
}

fn is_regenerated_structural_entry(logical_key: &str) -> bool {
    matches!(logical_key, "root" | "version" | "versions")
}

fn ensure_destination_absent(path: &Path) -> std::result::Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(format!("destination already exists: {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to inspect destination `{}`: {error}",
            path.display()
        )),
    }
}

fn report_path_conflict(
    report: &Path,
    input: &Path,
    output: &Path,
    source_format: &str,
    target_format: &str,
) -> bool {
    let report = normalized_absolute(report);
    let input = normalized_absolute(input);
    let output = normalized_absolute(output);
    report == input
        || report == output
        || (source_format == "xml" && report.starts_with(&input))
        || (target_format == "xml" && report.starts_with(&output))
}

fn artifact_path_conflict(args: &ConvertArgs) -> bool {
    let input = normalized_absolute(&args.input);
    let output = normalized_absolute(&args.output);
    input == output || (args.source_format == ConversionFormat::Xml && output.starts_with(input))
}

fn normalized_absolute(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let absolute = lexical_normalize(&absolute);
    let mut existing = absolute.clone();
    let mut missing = Vec::new();
    while !existing.exists() {
        let Some(name) = existing.file_name().map(ToOwned::to_owned) else {
            break;
        };
        missing.push(name);
        if !existing.pop() {
            break;
        }
    }
    if let Ok(mut resolved) = existing.canonicalize() {
        for component in missing.into_iter().rev() {
            resolved.push(component);
        }
        lexical_normalize(&resolved)
    } else {
        absolute
    }
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn failure(
    report: &mut ConversionReport,
    phase: &'static str,
    code: impl Into<String>,
    message: String,
    path: Option<String>,
) -> ConversionError {
    report.ok = false;
    report.mark(phase, ConversionPhaseStatus::Failed);
    report.errors.push(ConversionDiagnostic {
        code: code.into(),
        phase,
        message,
        path,
    });
    ConversionError {
        report: Box::new(report.clone()),
    }
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

struct TemporaryDirectory {
    path: PathBuf,
}

impl TemporaryDirectory {
    fn new(label: &str) -> std::io::Result<Self> {
        let root = std::env::temp_dir();
        for attempt in 0_u32..1_024 {
            let path = root.join(format!(
                "ibcmd-rs-convert-{label}-{}-{attempt}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "no temporary conversion directory name available",
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
