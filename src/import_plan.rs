//! Backend-neutral compilation of selected changes over immutable effective rows.
//!
//! Call under the existing export-engine lock. This planner owns no database
//! session, snapshot, target identity or publication authority. Adapters retain
//! those guards and revalidate the returned source after packing their rows.
pub(crate) mod witness;

use crate::{
    metadata_model::{
        index::{ConfigIndex, ROOT_COLLECTIONS},
        xml::{Element, MetadataXml},
    },
    mssql_source_change::HeldSourceRoot,
    platform::PlatformSpec,
};
use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

// Owns only a private directory created by this request. No Plan needs the
// compiled scratch once its verified rows have been materialized.
pub(crate) struct ImportScratch {
    pub(crate) path: PathBuf,
    identity: fs::Metadata,
}
impl ImportScratch {
    pub(crate) fn create() -> Result<Self> {
        let path =
            std::env::temp_dir().join(format!("ibcmd-editor-import-{}", uuid::Uuid::new_v4()));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&path)?;
        Ok(Self {
            identity: fs::symlink_metadata(&path)?,
            path,
        })
    }
    pub(crate) fn join(&self, path: &str) -> PathBuf {
        self.path.join(path)
    }
}
impl Drop for ImportScratch {
    fn drop(&mut self) {
        // Retention is confined to the unit-test build. The environment cannot
        // bypass owned-scratch cleanup in production or integration builds.
        #[cfg(test)]
        if std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").as_deref()
            == Some(std::ffi::OsStr::new("1"))
        {
            return;
        }
        let _ = &self.identity;
        let Ok(now) = fs::symlink_metadata(&self.path) else {
            return;
        };
        if !now.is_dir() || now.file_type().is_symlink() {
            return;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if (now.dev(), now.ino()) != (self.identity.dev(), self.identity.ino()) {
                return;
            }
        }
        // Runtime-owned temporary data only. remove_dir_all does not follow
        // child symlinks. Lab oracle evidence has a separate, retained location.
        let _ = fs::remove_dir_all(&self.path);
    }
}
#[derive(Default, Clone, Debug)]
pub struct Selection {
    pub files: Vec<String>,
    pub objects: Vec<String>,
    pub changed: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingReference {
    pub path: String,
    pub reference: String,
    pub suggested_object: Option<String>,
}
/// Fully verified packed entries, or reference diagnostics requiring a new plan.
/// Row materialization and target publication remain adapter responsibilities.
pub(crate) struct Prepared {
    pub(crate) source: HeldSourceRoot,
    pub(crate) files: Vec<String>,
    pub(crate) missing_references: Vec<MissingReference>,
    pub(crate) rows: Option<BTreeMap<String, Vec<u8>>>,
}

pub(crate) fn validate_selection(selection: &Selection) -> Result<()> {
    let modes = usize::from(!selection.files.is_empty())
        + usize::from(!selection.objects.is_empty())
        + usize::from(selection.changed);
    if modes != 1 {
        bail!("choose exactly one import selection: files, objects or changed");
    }
    Ok(())
}

pub(crate) fn prepare(
    effective_rows: BTreeMap<String, Vec<u8>>,
    source: HeldSourceRoot,
    selection: &Selection,
    platform: PlatformSpec,
) -> Result<Prepared> {
    Ok(prepare_inner(effective_rows, source, selection, platform, None)?.0)
}

/// Opt-in raw evidence only. It cannot create a SQL publication capability.
pub(crate) struct PreparedWithRawEvidence {
    pub(crate) prepared: Prepared,
    pub(crate) evidence: witness::RawCompilerEvidence,
}
pub(crate) fn prepare_with_export_evidence(
    effective_rows: BTreeMap<String, Vec<u8>>,
    source: HeldSourceRoot,
    selection: &Selection,
    platform: PlatformSpec,
    request: witness::Request,
) -> Result<PreparedWithRawEvidence> {
    let mut capture = witness::Capture::new(request)?;
    let (prepared, evidence) = prepare_inner(
        effective_rows,
        source,
        selection,
        platform,
        Some(&mut capture),
    )?;
    Ok(PreparedWithRawEvidence {
        prepared,
        evidence: evidence.ok_or_else(|| anyhow!("compiler raw evidence is missing"))?,
    })
}
fn prepare_inner(
    effective_rows: BTreeMap<String, Vec<u8>>,
    source: HeldSourceRoot,
    selection: &Selection,
    platform: PlatformSpec,
    mut capture: Option<&mut witness::Capture>,
) -> Result<(Prepared, Option<witness::RawCompilerEvidence>)> {
    validate_selection(selection)?;
    if effective_rows.is_empty() {
        bail!("partial import requires an existing configuration");
    }
    let old_packed = if let Some(capture) = capture.as_deref_mut() {
        Some(capture.packed(&effective_rows, platform)?)
    } else {
        None
    };
    let scratch = ImportScratch::create()?;
    let base = scratch.join("base.cf");
    let tree = scratch.join("expected");
    crate::mssql_dump::config_save::write_rows_cf(effective_rows, &base, false)?;
    let version = platform.xml_version();
    crate::platform::note_export_platform(platform);
    let old_artifact = if let Some(capture) = capture.as_deref_mut() {
        Some(capture.artifact(&base)?)
    } else {
        None
    };
    let mut binding = if let (Some(capture), Some((digest, _)), Some(artifact)) =
        (capture.as_deref_mut(), old_packed.as_ref(), old_artifact)
    {
        Some(capture.binding(&source, selection, platform, *digest, artifact)?)
    } else {
        None
    };
    let old_report = crate::load::export_base(&base, &tree, version)?;
    // OLD raw descriptor candidates are retained BEFORE selected fs::write.
    let old_evidence = match (capture.as_deref_mut(), old_packed) {
        (Some(capture), Some(packed)) => {
            anyhow::ensure!(
                Some(capture.artifact(&base)?) == old_artifact,
                "baseline compiler artifact changed during export"
            );
            Some(capture.side(&tree, old_report, packed, platform)?)
        }
        _ => {
            drop(old_report);
            None
        }
    };
    let baseline = crate::load::relative_files(&tree)?;
    let source_paths = source
        .baseline()
        .files()
        .map(|file| file.path().to_string())
        .collect::<BTreeSet<_>>();
    // A native export's inventory is the synchronization baseline. A
    // difference against a newer target is a conflict, not a local edit.
    let sync = if selection.changed {
        let local = source.source_bytes("ConfigDumpInfo.xml").context("changed import requires a synchronized ConfigDumpInfo.xml; refresh the export baseline or select files explicitly")?;
        Some((
            crate::mssql_dump::incremental::DumpInfo::parse(&local)?,
            crate::mssql_dump::incremental::DumpInfo::read(&tree.join("ConfigDumpInfo.xml"))?,
        ))
    } else {
        None
    };
    let sync_indices = sync.as_ref().map(|(local, current)| {
        (
            crate::mssql_dump::incremental::Ownership::new(&local.entries),
            crate::mssql_dump::incremental::Ownership::new(&current.entries),
            local
                .entries
                .iter()
                .map(|entry| (entry.id.as_str(), entry))
                .collect::<BTreeMap<_, _>>(),
            current
                .entries
                .iter()
                .map(|entry| (entry.id.as_str(), entry))
                .collect::<BTreeMap<_, _>>(),
        )
    });
    let selected = if !selection.files.is_empty() {
        crate::mssql::files_stage::select(source.canonical_root(), &selection.files)?
            .files()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
    } else {
        let mut prefixes = Vec::new();
        for object in &selection.objects {
            if object == "Configuration" {
                bail!("partial import of Configuration is not supported");
            }
            let owned = crate::stored_objects::source_prefixes(object)
                .ok_or_else(|| anyhow!("invalid object name {object}"))?;
            for prefix in &owned {
                if !baseline.contains(&format!("{prefix}.xml")) {
                    bail!("new object {object} is not supported by partial import");
                }
            }
            prefixes.extend(owned);
        }
        source_paths
            .union(&baseline)
            .filter(|path| {
                if selection.changed {
                    path.as_str() != "ConfigDumpInfo.xml"
                        && crate::stored_objects::object_of_path(path).is_some()
                } else {
                    prefixes.iter().any(|prefix| in_prefix(path, prefix))
                }
            })
            .cloned()
            .collect()
    };
    let mut changed = Vec::new();
    let mut selected_bytes = Vec::new();
    for path in selected {
        crate::cancel::check()?;
        if !baseline.contains(&path) {
            bail!("new file {path} is not supported by partial import; nothing written");
        }
        let bytes = source.source_bytes(&path).with_context(|| {
            format!("missing or changed selected file {path}; deletions are not supported")
        })?;
        let expected = tree.join(&path);
        if *bytes != fs::read(&expected)? {
            if let Some((old_owner, now_owner, old_entries, now_entries)) = &sync_indices {
                let old_id = old_owner.exact(&path).or_else(|| old_owner.folder(&path));
                let now_id = now_owner.exact(&path).or_else(|| now_owner.folder(&path));
                let unchanged = old_id.zip(now_id).is_some_and(|(a, b)| {
                    a == b
                        && old_entries
                            .get(a)
                            .zip(now_entries.get(b))
                            .is_some_and(|(old, now)| {
                                old.name == now.name && old.config_version == now.config_version
                            })
                });
                if !unchanged {
                    bail!(
                        "{path}: target changed since the synchronized export; refresh the baseline or select files explicitly; nothing written"
                    );
                }
            }
            if path == "Configuration.xml" {
                bail!("changed Configuration.xml requires whole-tree import");
            }
            fs::write(&expected, bytes.as_slice())?;
            changed.push(path.clone());
            selected_bytes.push((path, bytes));
        }
    }
    if changed.is_empty() {
        bail!("selected files have no changes");
    }
    if let (Some(capture), Some(binding)) = (capture.as_deref_mut(), binding.as_mut()) {
        binding.resolved_selection = capture.resolved_selection(&changed)?;
    }
    let expected_index = ConfigIndex::build(&tree)?;
    let source_xml = source_paths
        .iter()
        .filter(|path| path.ends_with(".xml"))
        .map(|path| {
            Ok((
                source.canonical_root().join(path),
                source.source_bytes(path)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let source_index = ConfigIndex::build_from_files(source.canonical_root(), &source_xml)?;
    let mut missing_references = Vec::new();
    for (path, bytes) in &selected_bytes {
        if path.ends_with(".xml") {
            let doc = MetadataXml::parse(bytes)?;
            references(
                &doc.root,
                path,
                &expected_index,
                &source_index,
                &mut missing_references,
            );
        }
    }
    let ready = missing_references.is_empty();
    let mut new_evidence = None;
    let rows = if ready {
        let output = scratch.join("staged.cf");
        crate::load::load_onto_base(&tree, &base, &output, version)
            .map_err(|error| anyhow!("{error}"))?;
        let new_artifact = if let Some(capture) = capture.as_deref_mut() {
            Some(capture.artifact(&output)?)
        } else {
            None
        };
        let mut entries = crate::file_infobase::read_cf_entries(&output)?;
        // Bind the complete raw artifact, including any CF service entry
        // excluded from the unchanged FileDb prepared map below.
        let new_packed = if let Some(capture) = capture.as_deref_mut() {
            Some(capture.packed(&entries, platform)?)
        } else {
            None
        };
        entries.remove("deleted");
        let staged = entries
            .iter()
            .map(|(file_name, bytes)| crate::mssql_dump::StagedRow { file_name, bytes })
            .collect::<Vec<_>>();
        // Always verify; diagnostic environment switches cannot disable editor safety.
        crate::mssql::verify_staged_state(
            &crate::mssql::GuardRequest {
                kind: crate::mssql::StageKind::Patch,
                source_root: &tree,
                tree: crate::mssql::TreeFiles::Unlisted,
                removed: &[],
                path_prefix: &[],
                source_version: Some(version),
            },
            crate::mssql_dump::StateBase::Nothing,
            &staged,
        )?;
        if let Some(capture) = capture.as_deref_mut() {
            // Export the exact verified compiled artifact, not the edited tree
            // or load's reduced keys_by_output projection.
            let packed = new_packed.ok_or_else(|| anyhow!("compiled packed witness is missing"))?;
            if let Some(binding) = binding.as_mut() {
                binding.new_packed = Some(packed.0);
                binding.new_artifact = new_artifact;
            }
            let compiled_root = scratch.join("compiled-evidence");
            let new_report = crate::load::export_base(&output, &compiled_root, version)?;
            anyhow::ensure!(
                Some(capture.artifact(&output)?) == new_artifact,
                "compiled artifact changed during verification or export"
            );
            new_evidence = Some(capture.side(&compiled_root, new_report, packed, platform)?);
        }
        Some(entries)
    } else {
        None
    };
    let evidence = match (binding, old_evidence) {
        (Some(binding), Some(old)) => Some(witness::RawCompilerEvidence {
            // No observation escapes until mandatory verification and artifact
            // hash checks above succeeded (or no compile due to missing refs).
            engine_observations: capture
                .as_deref_mut()
                .ok_or_else(|| anyhow!("compiler observer capture is missing"))?
                .take_observations(),
            binding,
            old,
            new: new_evidence,
            resolved_selected: changed.iter().cloned().collect(),
        }),
        _ => None,
    };
    Ok((
        Prepared {
            source,
            files: changed,
            missing_references,
            rows,
        },
        evidence,
    ))
}

fn in_prefix(path: &str, prefix: &str) -> bool {
    path == prefix
        || path == format!("{prefix}.xml")
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}
pub(crate) fn references(
    node: &Element,
    path: &str,
    target: &ConfigIndex,
    source: &ConfigIndex,
    missing: &mut Vec<MissingReference>,
) {
    if matches!(
        node.name.as_str(),
        "Comment"
            | "Synonym"
            | "Title"
            | "ToolTip"
            | "Description"
            | "Name"
            | "Presentation"
            | "Format"
    ) {
        return;
    }
    let value = node.text.trim();
    let name = value.rsplit(':').next().unwrap_or(value);
    let head = name.split('.').next().unwrap_or_default();
    let is_object = ROOT_COLLECTIONS.iter().any(|(_, kind)| *kind == head);
    let is_type = node.name == "Type"
        && (value.starts_with("cfg:") || (name.contains('.') && head.ends_with("Ref")));
    if (is_object || is_type) && name.contains('.') {
        let standard = name
            .split_once(".StandardAttribute.")
            .is_some_and(|(owner, attribute)| {
                target.objects.contains_key(owner)
                    && crate::metadata_model::objects::standard_markers(head)
                        .is_some_and(|markers| markers.iter().any(|(field, _)| *field == attribute))
            });
        let known = target.objects.contains_key(name)
            || target.children.contains_key(name)
            || target.generated_types.contains_key(name)
            || standard;
        if !known {
            let suggested = if source.objects.contains_key(name) {
                Some(name.to_string())
            } else {
                let short = name.split_once('.').map(|(_, name)| name).unwrap_or(name);
                let candidates = source
                    .objects
                    .values()
                    .filter(|object| object.name == short && head.starts_with(&object.kind))
                    .collect::<Vec<_>>();
                (candidates.len() == 1).then(|| candidates[0].full_name.clone())
            };
            if !missing
                .iter()
                .any(|item| item.path == path && item.reference == value)
            {
                missing.push(MissingReference {
                    path: path.to_string(),
                    reference: value.to_string(),
                    suggested_object: suggested,
                });
            }
        }
    }
    for child in &node.children {
        references(child, path, target, source, missing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Read, path::Path};

    // The offline metadata compiler intentionally installs one process-wide
    // base table. Keep each metadata fixture in a fresh test process; never
    // reset the production guard or let another fixture consume its base.
    fn in_fresh_compiler_process(test: &str) -> bool {
        const CHILD: &str = "IBCMD_IMPORT_PLAN_COMPILER_TEST";
        if std::env::var(CHILD).ok().as_deref() == Some(test) {
            return false;
        }
        static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test, "--nocapture"])
            .env(CHILD, test)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("test result: ok. 1 passed; 0 failed;"),
            "{stdout}"
        );
        true
    }

    fn baseline() -> (
        ImportScratch,
        BTreeMap<String, Vec<u8>>,
        PathBuf,
        PlatformSpec,
    ) {
        let scratch = ImportScratch::create().unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/file-infobase");
        let database = scratch.join("baseline.1CD");
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(fs::File::open(fixture.join("tiny.1CD.gz")).unwrap())
            .read_to_end(&mut bytes)
            .unwrap();
        fs::write(&database, bytes).unwrap();
        let rows = {
            let mut reader = ibcmd_filedb::FileDb::open(&database).unwrap();
            crate::mssql_dump::file_infobase_effective(
                reader.config_rows("CONFIG").unwrap(),
                Vec::new(),
            )
            .unwrap()
            .published
        };
        let cf = scratch.join("baseline.cf");
        crate::mssql_dump::config_save::write_rows_cf(rows.clone(), &cf, false).unwrap();
        let root = scratch.join("source");
        let platform = crate::platform::parse_flag("8.5.1").unwrap();
        crate::load::export_base(&cf, &root, platform.xml_version()).unwrap();
        (scratch, rows, root, platform)
    }
    fn selected(path: &str) -> Selection {
        Selection {
            files: vec![path.into()],
            ..Selection::default()
        }
    }
    fn held(root: &Path) -> HeldSourceRoot {
        HeldSourceRoot::open(
            root,
            crate::mssql_source_change::SourceInventoryLimits::default(),
        )
        .unwrap()
    }

    #[test]
    fn verified_existing_unselected_payload_survives_next_selected_plan() {
        if in_fresh_compiler_process(
            "import_plan::tests::verified_existing_unselected_payload_survives_next_selected_plan",
        ) {
            return;
        }
        crate::stored_objects::exclusive(|| -> Result<()> {
            let (_scratch, initial, root, platform) = baseline();
            let language = "Languages/English.xml";
            let index = ConfigIndex::build(&root).unwrap();
            let key = index.objects.get("Language.English").unwrap().uuid.clone();
            let text = fs::read_to_string(root.join(language)).unwrap();
            assert!(text.contains("<Comment/>"));
            fs::write(
                root.join(language),
                text.replace("<Comment/>", "<Comment>previous staged edit</Comment>"),
            )
            .unwrap();
            let first = prepare(initial, held(&root), &selected(language), platform).unwrap();
            assert!(first.missing_references.is_empty());
            let previous = first.rows.unwrap();
            let previous_language = previous.get(&key).unwrap().clone();
            let module = "CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl";
            let mut body = fs::read(root.join(module)).unwrap();
            body.extend_from_slice(b"\r\n// next selected edit\r\n");
            fs::write(root.join(module), body).unwrap();
            let second =
                prepare(previous.clone(), held(&root), &selected(module), platform).unwrap();
            assert_eq!(second.files, vec![module.to_string()]);
            assert!(second.missing_references.is_empty());
            assert_eq!(
                second.rows.as_ref().unwrap().get(&key).unwrap(),
                &previous_language
            );
            assert_eq!(previous.get(&key).unwrap(), &previous_language);
            second.source.require_unchanged().unwrap();
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn missing_existing_and_new_files_are_refused() {
        crate::stored_objects::exclusive(|| -> Result<()> {
            let (scratch, rows, root, platform) = baseline();
            let existing = "CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl";
            assert!(root.join(existing).is_file());
            fs::rename(
                root.join(existing),
                scratch.join("retained-original-module.bsl"),
            )
            .unwrap();
            assert!(!root.join(existing).exists());
            assert!(prepare(rows.clone(), held(&root), &selected(existing), platform).is_err());
            let new = "CommonModules/ФайловаяБазаПроверка/Ext/Additional.bsl";
            fs::write(root.join(new), b"// unsupported new body").unwrap();
            let error = prepare(rows, held(&root), &selected(new), platform)
                .err()
                .unwrap();
            assert!(error.to_string().contains("new file"), "{error:#}");
            Ok(())
        })
        .unwrap();
    }
    #[test]
    fn opt_in_retains_old_descriptor_before_edit_and_verified_new_raw_export() {
        if in_fresh_compiler_process(
            "import_plan::tests::opt_in_retains_old_descriptor_before_edit_and_verified_new_raw_export",
        ) {
            return;
        }
        crate::stored_objects::exclusive(|| -> Result<()> {
            let (scratch, rows, root, platform) = baseline();
            let profile_root = scratch.join("profile-evidence");
            let profile = crate::load::export_base(
                &scratch.join("baseline.cf"),
                &profile_root,
                platform.xml_version(),
            )?;
            let language = "Languages/English.xml";
            let old = fs::read(root.join(language))?;
            let changed = String::from_utf8(old.clone())?
                .replace("<Comment/>", "<Comment>synthetic selected change</Comment>");
            assert_ne!(old, changed.as_bytes());
            fs::write(root.join(language), changed.as_bytes())?;
            let request = witness::Request {
                target: witness::TargetBinding {
                    snapshot: [1; 32],
                    schema: [2; 32],
                    profile: [3; 32],
                    epoch: 4,
                },
                report_profile: profile
                    .storage
                    .source_profile
                    .clone()
                    .ok_or_else(|| anyhow!("fixture profile missing"))?,
                report_schema: profile.storage.schema_version,
                limits: witness::Limits::default(),
                check: std::sync::Arc::new(|| Ok(crate::cancel::check()?)),
            };
            let result = prepare_with_export_evidence(
                rows,
                held(&root),
                &selected(language),
                platform,
                request,
            )?;
            assert!(result.prepared.rows.is_some());
            assert_eq!(result.prepared.files, vec![language.to_string()]);
            let old_candidate = result
                .evidence
                .old_descriptors()
                .iter()
                .find(|d| d.path == language)
                .unwrap();
            let new_candidate = result
                .evidence
                .new_descriptors()
                .unwrap()
                .iter()
                .find(|d| d.path == language)
                .unwrap();
            assert_eq!(old_candidate.bytes.as_slice(), old.as_slice());
            assert_eq!(new_candidate.bytes.as_slice(), changed.as_bytes());
            assert!(!result.evidence.old_report().entries.is_empty());
            assert!(!result.evidence.new_report().unwrap().entries.is_empty());
            assert!(result.evidence.binding().new_packed.is_some());
            assert!(result.evidence.binding().new_artifact.is_some());
            result.prepared.source.require_unchanged()?;
            Ok(())
        })
        .unwrap();
    }
    #[test]
    fn provided_view_diagnostic_compiles_but_never_grants_sql_authority() {
        if in_fresh_compiler_process(
            "import_plan::tests::provided_view_diagnostic_compiles_but_never_grants_sql_authority",
        ) {
            return;
        }
        use crate::server::sql_import::compiler_diagnostic::{self, Limits, ProvidedView, Refusal};
        // Release fixture setup's engine lock before the consumer takes its own.
        let (scratch, rows, root, platform, export_profile, export_schema) =
            crate::stored_objects::exclusive(|| -> Result<_> {
                let (scratch, rows, root, platform) = baseline();
                let profile = crate::load::export_base(
                    &scratch.join("baseline.cf"),
                    &scratch.join("diagnostic-profile"),
                    platform.xml_version(),
                )?;
                Ok((
                    scratch,
                    rows,
                    root,
                    platform,
                    profile.storage.source_profile.unwrap(),
                    profile.storage.schema_version,
                ))
            })
            .unwrap();
        let language = "Languages/English.xml";
        let text = fs::read_to_string(root.join(language)).unwrap();
        fs::write(
            root.join(language),
            text.replace("<Comment/>", "<Comment>synthetic diagnostic</Comment>"),
        )
        .unwrap();
        let report = compiler_diagnostic::inspect_provided_view(
            ProvidedView {
                rows,
                snapshot_label: [1; 32],
                schema_label: [2; 32],
                profile_label: [3; 32],
                epoch_label: 4,
                export_profile,
                export_schema,
            },
            &root,
            &selected(language),
            platform,
            Limits::default(),
            std::sync::Arc::new(|| Ok(())),
        )
        .unwrap();
        assert!(!report.database_accessed);
        assert!(!report.publication_allowed);
        assert_eq!(report.engine_observations.len(), 2);
        for observation in &report.engine_observations {
            assert!(observation.candidates > 0);
            assert_ne!(
                observation.canonical_map_digest,
                observation.ordered_view_digest
            );
            assert_eq!(observation.unsupported_dimensions.len(), 6);
            assert!(
                observation
                    .unsupported_dimensions
                    .contains(&"owner_completeness")
            );
        }
        assert_eq!(report.changed_files, 1);
        assert_eq!(report.missing_references, 0);
        assert!(report.compiled_entries.is_some());
        assert!(report.old_report_entries > 0);
        assert!(report.new_report_entries.is_some_and(|n| n > 0));
        assert!(report.old_descriptors > 0);
        assert!(report.new_descriptors.is_some_and(|n| n > 0));
        assert_ne!(
            Some(report.old_descriptor_digest),
            report.new_descriptor_digest
        );
        assert_eq!(
            report.refusal,
            Refusal::IndependentOwnerCompletenessUnavailable
        );
        let ledger = report
            .outputs
            .as_ref()
            .expect("this fixture emits one stable selected output pair");
        assert_eq!(ledger.pairs, 1);
        assert!(ledger.old_claiming_keys > 0 && ledger.new_claiming_keys > 0);
        // Retain a bounded synthetic receipt so a later reviewed oracle can pin
        // this fixture's actual refusal branch, without logging source data.
        let refusal = match report.refusal {
            Refusal::IndependentOwnerCompletenessUnavailable => "owner_completeness_unavailable",
            Refusal::OutputMappingUnavailable => "output_mapping_unavailable",
            Refusal::MissingReferences => "missing_references",
        };
        let receipt = serde_json::json!({"test_only":true,"compiler_diagnostic_executed":true,
            "sql_executed":false,"native_executed":false,"sql_acceptance":false,
            "publication_allowed":report.publication_allowed,"platform":platform.display(),
            "refusal":refusal,"changed_files":report.changed_files,
            "output_pairs":report.outputs.as_ref().map(|v|v.pairs),
            "old_descriptors":report.old_descriptors,"new_descriptors":report.new_descriptors});
        fs::write(
            scratch.join("compiler-diagnostic-receipt.json"),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .unwrap();
        // This tests admission/compilation/diagnostics, not real codec ownership.
        assert!(scratch.path.exists());
    }
}
