//! EDT is another format adapter. Profiles, reports and publication stay here.
use super::*;
use ibcmd_edt::{ConversionOptions, Disposition, read_directory_project, read_directory_source};

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
        let project = read_directory_project(&args.input).map_err(|error| {
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
        let conversion = project.edt_to_xml(&options).map_err(|error| {
            failure(
                &mut report,
                PHASE_PREFLIGHT,
                "conversion.edt-encode-preflight-failed",
                error.to_string(),
                Some(display_path(&args.input)),
            )
        })?;
        (conversion, project.file_count())
    } else {
        let tree = read_directory_source(&args.input).map_err(|error| {
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
        // The complete immutable snapshot was already validated. Load only
        // metadata envelopes for the explicit CLI profile, one file at a time.
        let mut descriptor_failure_path = None;
        tree.visit_xml_descriptors(|path, bytes| -> Result<(), String> {
            let validation = XmlReader::from_slice(bytes)
                .map_err(|error| format!("{path}: {error}"))
                .and_then(|document| {
                    validate_source_dialect(&document, &dialects, &source.id)
                        .map_err(|message| format!("{path}: {message}"))
                });
            if validation.is_err() {
                descriptor_failure_path = Some(path.to_owned());
            }
            validation
        })
        .map_err(|error| {
            failure(
                &mut report,
                PHASE_DECODE,
                "conversion.xml-source-profile-mismatch",
                error.to_string(),
                descriptor_failure_path
                    .take()
                    .or_else(|| Some(display_path(&args.input))),
            )
        })?;
        report.mark(PHASE_DECODE, ConversionPhaseStatus::Completed);
        let conversion = tree.xml_to_edt(&options).map_err(|error| {
            failure(
                &mut report,
                PHASE_PREFLIGHT,
                "conversion.edt-encode-preflight-failed",
                error.to_string(),
                Some(display_path(&args.input)),
            )
        })?;
        (conversion, tree.file_count())
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
        extensions: conversion
            .extensions
            .iter()
            .map(|extension| EdtSourceExtensionReport {
                id: extension.id,
                resources: extension.resources,
                references: extension.references,
            })
            .collect(),
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
    conversion.verify().map_err(|error| {
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
    let bytes = conversion.byte_len();
    report.preflight = Some(ConversionPreflightReport {
        source_entries,
        target_entries: conversion.file_count(),
        target_bytes: bytes,
        opaque_entries: retained,
        structural_entries: 0,
    });
    report.mark(PHASE_PREFLIGHT, ConversionPhaseStatus::Completed);
    if args.dry_run {
        report.mark(PHASE_ENCODE, ConversionPhaseStatus::SkippedDryRun);
    } else {
        conversion.publish_new(&args.output).map_err(|error| {
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
            entries: conversion.file_count(),
            bytes,
            cf_revision: None,
        });
        report.output_published = true;
        report.mark(PHASE_ENCODE, ConversionPhaseStatus::Completed);
    }
    report.ok = true;
    Ok(report)
}
