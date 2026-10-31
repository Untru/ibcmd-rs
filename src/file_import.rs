//! Safe editor partial CONFIGSAVE staging on real file infobases (#382).
//! Preflight builds and verifies a complete effective tree; publication uses
//! an opaque plan bound to the target identity and original source census.
#[cfg(test)]
use crate::{
    import_plan::{ImportScratch, references},
    metadata_model::{index::ConfigIndex, xml::MetadataXml},
};
use crate::{
    mssql_source_change::{HeldSourceRoot, SourceInventoryLimits},
    platform::PlatformSpec,
};
use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

pub use crate::import_plan::{MissingReference, Selection};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preflight {
    pub target: PathBuf,
    pub files: Vec<String>,
    pub missing_references: Vec<MissingReference>,
    pub ready: bool,
}
/// Optional untrusted-input limits, retained by the plan for publication.
/// The callback must remain valid for the whole prepare/confirm/publish lifetime.
#[derive(Clone)]
pub struct ReadControl {
    pub limits: ibcmd_filedb::ReadLimits,
    pub check: Arc<dyn Fn() -> ibcmd_filedb::Result<()> + Send + Sync>,
}
impl ReadControl {
    fn open(
        &self,
        target: &Path,
        guard: &mut dyn FnMut(&fs::Metadata) -> Result<()>,
    ) -> Result<ibcmd_filedb::WriteSession> {
        let check = self.check.clone();
        Ok(ibcmd_filedb::open_for_write_with_limits(
            target,
            self.limits,
            move || check(),
            &mut |metadata| {
                guard(metadata).map_err(|error| ibcmd_filedb::Error::Refused(format!("{error:#}")))
            },
        )?)
    }
}
fn open_session(
    target: &Path,
    read_control: Option<&ReadControl>,
    guard: &mut dyn FnMut(&fs::Metadata) -> Result<()>,
) -> Result<ibcmd_filedb::WriteSession> {
    if let Some(control) = read_control {
        return control.open(target, guard);
    }
    let session = ibcmd_filedb::open_for_write(target)?;
    guard(&session.metadata()?)?;
    Ok(session)
}
/// Contents are private: callers cannot substitute compiled rows or fingerprints.
pub struct Plan {
    summary: Preflight,
    snapshot: String,
    source: HeldSourceRoot,
    rows: Option<Vec<ibcmd_filedb::ConfigRow>>,
    read_control: Option<ReadControl>,
}
impl Plan {
    pub fn summary(&self) -> &Preflight {
        &self.summary
    }

    /// The caller supplies its CURRENT connection target after user confirmation.
    /// A changed target, source or CONFIG/CONFIGSAVE invalidates the entire plan.
    pub fn publish(self, current_target: &Path) -> Result<ibcmd_filedb::ReplaceReport> {
        self.publish_guarded(current_target, &mut |_| Ok(()))
    }

    /// Operator policy can bind the actual consumed descriptor to its pinned identity.
    pub fn publish_guarded(
        self,
        current_target: &Path,
        guard: &mut dyn FnMut(&fs::Metadata) -> Result<()>,
    ) -> Result<ibcmd_filedb::ReplaceReport> {
        crate::stored_objects::exclusive(|| {
            crate::cancel::check()?;
            let current = fs::canonicalize(ibcmd_filedb::data_file(current_target))?;
            if current != self.summary.target {
                bail!("connection changed since import preflight; nothing written");
            }
            let rows = self.rows.as_ref().ok_or_else(|| anyhow!("unresolved references; confirm dependencies and repeat preflight before importing"))?;
            self.source.require_unchanged()?;
            let session = open_session(&current, self.read_control.as_ref(), guard)?;
            if session.snapshot()? != self.snapshot {
                bail!("file database changed since import preflight; nothing written");
            }
            let result = session.replace_guarded("CONFIGSAVE", rows, |metadata| {
                self.source
                    .require_unchanged()
                    .map_err(|error| ibcmd_filedb::Error::Refused(error.to_string()))?;
                guard(metadata)
                    .map_err(|error| ibcmd_filedb::Error::Refused(format!("{error:#}")))?;
                // The source census/policy can take time. Cancellation must be
                // the final action before the format layer publishes its copy.
                if let Some(control) = &self.read_control {
                    (control.check)()?;
                }
                crate::cancel::check()
                    .map_err(|error| ibcmd_filedb::Error::Refused(error.to_string()))
            });
            // Preserve the typed cancellation for the RPC layer.
            finish_publication(result, || Ok(crate::cancel::check()?))
        })
    }
}

fn finish_publication(
    result: ibcmd_filedb::Result<ibcmd_filedb::ReplaceReport>,
    cancelled: impl FnOnce() -> Result<()>,
) -> Result<ibcmd_filedb::ReplaceReport> {
    if matches!(&result, Err(ibcmd_filedb::Error::Published { .. })) {
        return result.map_err(Into::into);
    }
    if result.is_err() {
        cancelled()?;
    }
    result.map_err(Into::into)
}

pub fn prepare(
    target: &Path,
    source: &Path,
    selection: &Selection,
    platform: PlatformSpec,
) -> Result<Plan> {
    prepare_guarded(target, source, selection, platform, &mut |_| Ok(()))
}

pub fn prepare_guarded(
    target: &Path,
    source: &Path,
    selection: &Selection,
    platform: PlatformSpec,
    guard: &mut dyn FnMut(&fs::Metadata) -> Result<()>,
) -> Result<Plan> {
    crate::stored_objects::exclusive(|| {
        prepare_locked(
            target,
            source,
            selection,
            platform,
            guard,
            None,
            SourceInventoryLimits::default(),
        )
    })
}
/// Bounded prepare for untrusted consumers. Identity/policy is checked on the
/// actual write descriptor before database metadata is parsed. Publication opens
/// a new bounded session and retains the same callback; final guard still runs
/// inside the verified-copy publication boundary.
pub fn prepare_guarded_with_limits(
    target: &Path,
    source: &Path,
    selection: &Selection,
    platform: PlatformSpec,
    guard: &mut dyn FnMut(&fs::Metadata) -> Result<()>,
    read_control: ReadControl,
) -> Result<Plan> {
    prepare_guarded_with_source_limits(
        target,
        source,
        selection,
        platform,
        guard,
        read_control,
        SourceInventoryLimits::default(),
    )
}

/// Also bound source materialization within the shared held-source reader.
/// A caller's earlier census alone cannot enforce this across filesystem races.
pub fn prepare_guarded_with_source_limits(
    target: &Path,
    source: &Path,
    selection: &Selection,
    platform: PlatformSpec,
    guard: &mut dyn FnMut(&fs::Metadata) -> Result<()>,
    read_control: ReadControl,
    source_limits: SourceInventoryLimits,
) -> Result<Plan> {
    crate::stored_objects::exclusive(|| {
        prepare_locked(
            target,
            source,
            selection,
            platform,
            guard,
            Some(read_control),
            source_limits,
        )
    })
}
fn prepare_locked(
    target: &Path,
    source: &Path,
    selection: &Selection,
    platform: PlatformSpec,
    guard: &mut dyn FnMut(&fs::Metadata) -> Result<()>,
    read_control: Option<ReadControl>,
    source_limits: SourceInventoryLimits,
) -> Result<Plan> {
    crate::cancel::check()?;
    crate::import_plan::validate_selection(selection)?;
    let source = if read_control.is_some() {
        HeldSourceRoot::open_with_traversal_limits(
            source,
            source_limits,
            crate::mssql_source_change::SourceTraversalLimits {
                max_entries: source_limits.max_files,
                max_depth: 64,
            },
        )?
    } else {
        HeldSourceRoot::open(source, source_limits)?
    };
    let target = fs::canonicalize(ibcmd_filedb::data_file(target)).context("file import target")?;
    let mut session = open_session(&target, read_control.as_ref(), guard)?;
    let snapshot = session.snapshot()?;
    let config = session.config_rows("CONFIG")?;
    let saved = session.config_rows("CONFIGSAVE")?;
    let effective = crate::mssql_dump::file_infobase_effective(config, saved)?;
    let planned = crate::import_plan::prepare(effective.published, source, selection, platform)?;
    let ready = planned.missing_references.is_empty();
    let rows = planned
        .rows
        .as_ref()
        .map(|entries| {
            let deleted = effective
                .stored_names
                .iter()
                .filter(|name| !entries.contains_key(*name))
                .cloned()
                .collect::<Vec<_>>();
            crate::file_infobase::stage_rows(entries, &deleted)
        })
        .transpose()?;
    let source = planned.source;
    source.require_unchanged()?;
    crate::cancel::check()?;
    guard(&session.metadata()?)?;
    if session.snapshot()? != snapshot {
        bail!("file database changed during preflight; nothing written");
    }
    crate::cancel::check()?;
    if let Some(control) = &read_control {
        (control.check)()?;
    }
    Ok(Plan {
        read_control,
        summary: Preflight {
            target,
            files: planned.files,
            missing_references: planned.missing_references,
            ready,
        },
        snapshot,
        source,
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn published_recovery_error_survives_late_cancellation() {
        let error = finish_publication(
            Err(ibcmd_filedb::Error::Published {
                recovery: PathBuf::from("retained.bak.new"),
                error: std::io::Error::other("injected backup rename failure"),
            }),
            || Err(crate::cancel::Cancelled.into()),
        )
        .unwrap_err();
        assert!(!crate::cancel::is_cancelled(&error));
        assert!(error.to_string().contains("database was published"));
        assert!(error.to_string().contains("retained.bak.new"));
    }
    #[test]
    fn owned_scratch_is_removed_on_success_error_and_unwind() {
        // This unit-test build permits explicit retention; non-test library
        // builds compile out that override in ImportScratch::drop.
        let retained = std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").as_deref()
            == Some(std::ffi::OsStr::new("1"));
        let success = ImportScratch::create().unwrap();
        let path = success.path.clone();
        fs::write(success.join("config.cf"), b"owned").unwrap();
        drop(success);
        assert_eq!(path.exists(), retained);
        let error_path = std::sync::Mutex::new(None);
        let fail = || -> Result<()> {
            let scratch = ImportScratch::create()?;
            *error_path.lock().unwrap() = Some(scratch.path.clone());
            fs::write(scratch.join("config.cf"), b"owned")?;
            bail!("preflight cancelled");
        };
        assert!(fail().is_err());
        assert_eq!(
            error_path.lock().unwrap().as_ref().unwrap().exists(),
            retained
        );
        let unwind_path = std::sync::Mutex::new(None);
        let _ = std::panic::catch_unwind(|| {
            let scratch = ImportScratch::create().unwrap();
            *unwind_path.lock().unwrap() = Some(scratch.path.clone());
            panic!("injected interruption");
        });
        assert_eq!(
            unwind_path.lock().unwrap().as_ref().unwrap().exists(),
            retained
        );
    }
    #[test]
    fn reference_looking_plain_text_is_not_a_dependency() {
        let doc=MetadataXml::parse(br#"<MetaDataObject><Catalog><Properties><Comment>Catalog.DoesNotExist</Comment><Synonym><item><content>Document.AlsoMissing</content></item></Synonym></Properties></Catalog></MetaDataObject>"#).unwrap();
        let mut missing = Vec::new();
        references(
            &doc.root,
            "Catalogs/Example.xml",
            &ConfigIndex::default(),
            &ConfigIndex::default(),
            &mut missing,
        );
        assert!(missing.is_empty());
    }
    #[test]
    fn missing_metadata_type_suggests_the_source_object() {
        let mut source = ConfigIndex::default();
        source.objects.insert(
            "Catalog.Dependency".into(),
            crate::metadata_model::index::ObjectEntry {
                kind: "Catalog".into(),
                name: "Dependency".into(),
                uuid: uuid::Uuid::new_v4().to_string(),
                full_name: "Catalog.Dependency".into(),
                path: "Catalogs/Dependency.xml".into(),
            },
        );
        let doc=MetadataXml::parse(br#"<MetaDataObject><Attribute><Type><Type>cfg:CatalogRef.Dependency</Type></Type></Attribute></MetaDataObject>"#).unwrap();
        let mut missing = Vec::new();
        references(
            &doc.root,
            "Documents/Example.xml",
            &ConfigIndex::default(),
            &source,
            &mut missing,
        );
        assert_eq!(missing.len(), 1);
        assert_eq!(
            missing[0].suggested_object.as_deref(),
            Some("Catalog.Dependency")
        );
    }
}
