//! EDT is another format adapter. Profiles, reports and publication stay here.
use super::*;
use ibcmd_edt::{
    ConversionOptions, Disposition, edt_to_xml, read_project, read_xml_source, xml_to_edt,
};
use ibcmd_xml::source_tree::{
    MAX_SOURCE_DIRECTORIES, MAX_SOURCE_FILE_BYTES, MAX_SOURCE_FILES, MAX_SOURCE_RETAINED_BYTES,
    ReaderLimits, publish_new_with_limits,
};

fn limits() -> ReaderLimits {
    ReaderLimits {
        files: MAX_SOURCE_FILES,
        directories: MAX_SOURCE_DIRECTORIES,
        depth: ibcmd_xml::source_tree::MAX_SOURCE_DEPTH,
        asset_bytes: MAX_SOURCE_FILE_BYTES,
        total_bytes: MAX_SOURCE_RETAINED_BYTES,
    }
}

pub(super) fn convert(
    args: &ConvertArgs,
    profiles: &ProfileRegistry,
    source: &EffectiveProfile,
    target: &EffectiveProfile,
    mut report: ConversionReport,
) -> std::result::Result<ConversionReport, ConversionError> {
    let edt_profile = if args.source_format == ConversionFormat::Edt {
        source
    } else {
        target
    };
    let constant = |key: &str| {
        edt_profile
            .constants
            .get(key)
            .map(|value| value.value.clone())
    };
    let edt_version = constant("edt.tool-version").ok_or_else(|| {
        failure(
            &mut report,
            PHASE_DECODE,
            "conversion.edt-profile-incomplete",
            "EDT profile must declare edt.tool-version".to_owned(),
            None,
        )
    })?;
    let runtime_version = constant("edt.runtime-version").ok_or_else(|| {
        failure(
            &mut report,
            PHASE_DECODE,
            "conversion.edt-profile-incomplete",
            "EDT profile must declare an explicit edt.runtime-version".to_owned(),
            None,
        )
    })?;
    let source_dialect = source
        .xml_dialect
        .as_ref()
        .expect("coordinate checked")
        .value
        .to_string();
    let target_dialect = target
        .xml_dialect
        .as_ref()
        .expect("coordinate checked")
        .value
        .to_string();
    if source_dialect != target_dialect {
        return Err(failure(
            &mut report, PHASE_PLAN, "conversion.edt-migration-unsupported",
            "EDT conversion requires equal associated XML dialects; cross-version assets require a verified migration".to_owned(), None,
        ));
    }
    let options = ConversionOptions {
        edt_version: edt_version.clone(),
        xml_dialect: target_dialect.clone(),
        runtime_version: Some(runtime_version.clone()),
    };
    let (conversion, source_entries) = if args.source_format == ConversionFormat::Edt {
        let project = read_project(&args.input, limits()).map_err(|error| {
            failure(
                &mut report,
                PHASE_DECODE,
                "conversion.edt-decode-failed",
                error.to_string(),
                Some(display_path(&args.input)),
            )
        })?;
        report.mark(PHASE_DECODE, ConversionPhaseStatus::Completed);
        if args.target_format == ConversionFormat::Edt {
            return Err(failure(
                &mut report,
                PHASE_PLAN,
                "conversion.route-unsupported",
                "EDT to EDT is not a verified route; use an explicit XML endpoint".to_owned(),
                None,
            ));
        }
        let conversion = edt_to_xml(&project, &options).map_err(|error| {
            failure(
                &mut report,
                PHASE_PREFLIGHT,
                "conversion.edt-encode-preflight-failed",
                error.to_string(),
                Some(display_path(&args.input)),
            )
        })?;
        (conversion, project.entries().len())
    } else {
        let tree = read_xml_source(&args.input, limits()).map_err(|error| {
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
        // The EDT adapter owns complete semantic decoding. The physical CF
        // compiler's narrower family readers are not EDT capability gates.
        for entry in tree.entries().iter().filter(|entry| {
            matches!(
                entry.kind(),
                SourceKind::ConfigurationRoot | SourceKind::MetadataXml
            )
        }) {
            let document = XmlReader::from_slice(entry.bytes()).map_err(|error| {
                failure(
                    &mut report,
                    PHASE_DECODE,
                    "conversion.xml-decode-failed",
                    error.to_string(),
                    Some(entry.path().to_string()),
                )
            })?;
            validate_dialect(&document, &dialects, &source.id).map_err(|message| {
                failure(
                    &mut report,
                    PHASE_DECODE,
                    "conversion.xml-source-profile-mismatch",
                    message,
                    Some(entry.path().to_string()),
                )
            })?;
        }
        report.mark(PHASE_DECODE, ConversionPhaseStatus::Completed);
        let conversion = xml_to_edt(&tree, &options).map_err(|error| {
            failure(
                &mut report,
                PHASE_PREFLIGHT,
                "conversion.edt-encode-preflight-failed",
                error.to_string(),
                Some(display_path(&args.input)),
            )
        })?;
        (conversion, tree.entries().len())
    };
    validate_configuration(&conversion.canonical).map_err(|diagnostics| {
        failure(
            &mut report,
            PHASE_VALIDATE,
            "conversion.edt-canonical-invalid",
            format!("canonical metadata validation failed: {diagnostics:?}"),
            None,
        )
    })?;
    report.mark(PHASE_VALIDATE, ConversionPhaseStatus::Completed);
    report.plan = Some(ConversionPlanReport {
        kind: "format_adapter",
        route_profiles: vec![source.id.to_string(), target.id.to_string()],
        steps: vec!["codec:edt-project".to_owned()],
    });
    report.mark(PHASE_PLAN, ConversionPhaseStatus::Completed);
    report.mark(PHASE_MIGRATE, ConversionPhaseStatus::Completed);
    let retained = conversion
        .accounting
        .iter()
        .filter(|entry| matches!(entry.disposition, Disposition::Retained))
        .count();
    report.edt = Some(EdtConversionReport {
        edt_version,
        xml_dialect: target_dialect,
        runtime_version,
        canonical_objects: conversion.canonical.len(),
        canonical_retained_bytes: conversion
            .canonical
            .objects()
            .iter()
            .map(|object| object.retained_byte_len())
            .sum(),
        asset_references: conversion
            .canonical
            .objects()
            .iter()
            .map(|object| object.assets().len())
            .sum(),
        referenced_asset_bytes: conversion
            .canonical
            .objects()
            .iter()
            .flat_map(|object| object.assets())
            .map(|asset| asset.byte_len())
            .sum(),
        files: conversion
            .accounting
            .iter()
            .map(|entry| EdtFileAccounting {
                path: entry.path.to_string(),
                disposition: match entry.disposition {
                    Disposition::Converted => "converted",
                    Disposition::Retained => "retained",
                },
            })
            .collect(),
    });
    conversion.tree.validate().map_err(|error| {
        failure(
            &mut report,
            PHASE_PREFLIGHT,
            "conversion.edt-target-tree-invalid",
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
    let bytes = source_tree_bytes(&conversion.tree);
    report.preflight = Some(ConversionPreflightReport {
        source_entries,
        target_entries: conversion.tree.entries().len(),
        target_bytes: bytes,
        opaque_entries: retained,
        structural_entries: 0,
    });
    report.mark(PHASE_PREFLIGHT, ConversionPhaseStatus::Completed);
    if args.dry_run {
        report.mark(PHASE_ENCODE, ConversionPhaseStatus::SkippedDryRun);
    } else {
        publish_new_with_limits(&conversion.tree, &args.output, limits()).map_err(|error| {
            failure(
                &mut report,
                PHASE_ENCODE,
                "conversion.edt-publication-failed",
                error.to_string(),
                Some(display_path(&args.output)),
            )
        })?;
        report.publication = Some(ConversionPublicationReport {
            artifact: args.target_format.as_str(),
            entries: conversion.tree.entries().len(),
            bytes,
            cf_revision: None,
        });
        report.output_published = true;
        report.mark(PHASE_ENCODE, ConversionPhaseStatus::Completed);
    }
    report.ok = true;
    Ok(report)
}
