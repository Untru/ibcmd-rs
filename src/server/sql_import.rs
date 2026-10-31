//! Bounded source admission for SQL editor preparation.
//!
//! CommonModule preparation binds original source custody, observed storage
//! semantics and a full physical snapshot to one consumed server plan. Publication
//! stages text and versions only; native activation remains a separate operation.
pub mod compiler_diagnostic;
pub mod coverage;
pub mod readset;
pub mod selected;
pub mod snapshot;
pub mod transaction;

use std::{collections::BTreeMap, path::Path, sync::Arc};

use anyhow::{Context, Result, bail};

use crate::{
    file_import::Selection,
    mssql_source_change::{HeldSourceRoot, SourceInventoryLimits, SourceTraversalLimits},
};

/// Operator-side bounds, never supplied by an editor request.
#[derive(Clone, Copy)]
pub struct SourceLimits {
    pub inventory: SourceInventoryLimits,
    pub traversal: SourceTraversalLimits,
}
impl Default for SourceLimits {
    fn default() -> Self {
        Self {
            inventory: SourceInventoryLimits {
                max_files: 10_000,
                max_file_bytes: 16 * 1024 * 1024,
                max_total_bytes: 128 * 1024 * 1024,
            },
            traversal: SourceTraversalLimits {
                max_entries: 20_000,
                max_depth: 64,
            },
        }
    }
}

/// Original-census bytes only. This is source admission, not SQL write authority.
pub struct SourceAdmission {
    source: HeldSourceRoot,
    selected: BTreeMap<String, Arc<Vec<u8>>>,
}
impl SourceAdmission {
    pub fn prepare(
        root: &Path,
        selection: &Selection,
        verify: Option<bool>,
        limits: SourceLimits,
    ) -> Result<Self> {
        let modes = usize::from(!selection.files.is_empty())
            + usize::from(!selection.objects.is_empty())
            + usize::from(selection.changed);
        if modes != 1 {
            bail!("choose exactly one SQL import selection: files, objects or changed");
        }
        if verify == Some(false) {
            bail!("SQL editor import requires mandatory verification");
        }
        for object in &selection.objects {
            let prefixes = crate::stored_objects::source_prefixes(object)
                .with_context(|| format!("invalid SQL import object {object}"))?;
            if prefixes
                .iter()
                .any(|prefix| prefix == "Configuration" || prefix == "Ext")
            {
                bail!("partial SQL import of Configuration is unsupported");
            }
        }
        let source =
            HeldSourceRoot::open_with_traversal_limits(root, limits.inventory, limits.traversal)?;
        let mut selected = BTreeMap::new();
        for file in &selection.files {
            if file == "Configuration.xml" {
                bail!("Configuration.xml requires a whole-tree import");
            }
            // The held census checks exact spelling, aliases and original file
            // identity before returning bytes. A filesystem selector cannot
            // authorize a replacement that appeared after the census.
            selected.insert(file.clone(), source.source_bytes(file)?);
        }
        if !selection.files.is_empty() {
            crate::mssql::files_stage::select(source.canonical_root(), &selection.files)?;
        }
        if selection.changed {
            let inventory = source
                .source_bytes("ConfigDumpInfo.xml")
                .context("SQL changed import requires synchronized ConfigDumpInfo.xml")?;
            crate::mssql_dump::incremental::DumpInfo::parse(&inventory)?;
            selected.insert("ConfigDumpInfo.xml".into(), inventory);
        }
        source.require_unchanged()?;
        Ok(Self { source, selected })
    }

    /// Compile exactly one admitted CommonModule file from retained bytes.
    /// The result is not publication authority: target/profile, one-shot plan
    /// and full transactional CAS checks are still required by the adapter.
    pub fn prepare_common_module(
        &self,
        snapshot: &readset::Readset,
        selected_path: &str,
        profile: &crate::compiler::families::modules::CommonModuleProfile,
        limits: selected::Limits,
        check: &readset::Check,
    ) -> Result<selected::ModuleEdit> {
        self.prepare_selected_module(selected_path, check, |bytes| {
            selected::prepare_module_file(snapshot, selected_path, bytes, profile, limits, check)
        })
    }

    /// Compile existing CommonModule layout from held bytes, without a claimed
    /// platform coordinate. This still supplies no schema or write admission.
    pub fn prepare_common_module_layout(
        &self,
        snapshot: &readset::Readset,
        selected_path: &str,
        limits: selected::Limits,
        check: &readset::Check,
    ) -> Result<selected::ModuleEdit> {
        self.prepare_selected_module(selected_path, check, |bytes| {
            selected::prepare_module_file_layout(snapshot, selected_path, bytes, limits, check)
        })
    }

    /// Compile one existing Document-owned Form module from retained bytes.
    /// This does not admit Form.xml, other owner families or SQL publication.
    pub fn prepare_document_form_module(
        &self,
        snapshot: &readset::Readset,
        selected_path: &str,
        profile: &selected::form::DocumentFormProfile,
        limits: selected::Limits,
        check: &readset::Check,
    ) -> Result<selected::ModuleEdit> {
        self.prepare_selected_module(selected_path, check, |bytes| {
            selected::form::prepare_form_module_file(
                snapshot,
                selected_path,
                bytes,
                profile,
                limits,
                check,
            )
        })
    }

    /// Compile one held existing Document Form module by its original grammar.
    /// Target/schema admission and native acceptance are independent boundaries.
    pub fn prepare_document_form_module_layout(
        &self,
        snapshot: &readset::Readset,
        selected_path: &str,
        limits: selected::Limits,
        check: &readset::Check,
    ) -> Result<selected::ModuleEdit> {
        self.prepare_selected_module(selected_path, check, |bytes| {
            selected::form::prepare_form_module_file_layout(
                snapshot,
                selected_path,
                bytes,
                limits,
                check,
            )
        })
    }

    fn prepare_selected_module(
        &self,
        selected_path: &str,
        check: &readset::Check,
        compile: impl FnOnce(&[u8]) -> Result<selected::ModuleEdit>,
    ) -> Result<selected::ModuleEdit> {
        check()?;
        if self.selected.len() != 1 {
            bail!("SQL module preparation requires one explicit admitted file");
        }
        let bytes = self
            .bytes(selected_path)
            .context("SQL module file is outside the admitted selection")?;
        self.revalidate()?;
        let edit = compile(bytes)?;
        check()?;
        self.revalidate()?;
        Ok(edit)
    }

    pub fn bytes(&self, path: &str) -> Option<&[u8]> {
        self.selected.get(path).map(|bytes| bytes.as_slice())
    }

    /// Revalidates original bytes, file/ancestor witnesses and the bounded
    /// anchored file/directory census, including empty directories.
    /// This is a revalidation boundary, not an atomic filesystem freeze.
    /// This must precede the guarded SQL publication boundary, never follow it.
    pub fn revalidate(&self) -> Result<()> {
        self.source.require_unchanged().map_err(Into::into)
    }
}

/// Server-owned prepared existing module operation. No fields, raw rows,
/// profile assertions or compiled payload can be supplied by an RPC caller.
pub struct CommonModulePlan {
    source: SourceAdmission,
    source_dir: std::path::PathBuf,
    original: readset::Readset,
    edit: selected::ModuleEdit,
    storage: snapshot::CommonModuleStorageProfile,
    policy: transaction::WritePolicy,
    summary: CommonModulePreflight,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonModulePreflight {
    backend: &'static str,
    target: String,
    files: Vec<String>,
    missing_references: Vec<crate::file_import::MissingReference>,
    ready: bool,
    staged_rows: usize,
}
/// Unsupported request shape, distinct from source, target or SQL failures.
#[derive(Debug)]
pub struct RequestRefused(&'static str);
impl std::fmt::Display for RequestRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for RequestRefused {}

#[derive(Clone, Copy)]
enum SelectedModuleKind {
    CommonModule,
    DocumentFormModule,
}
impl SelectedModuleKind {
    fn refusal(self) -> &'static str {
        match self {
            Self::CommonModule => "SQL preflight currently supports one existing CommonModule file",
            Self::DocumentFormModule => {
                "SQL preflight currently supports one existing Document Form module file"
            }
        }
    }

    fn backend(self) -> &'static str {
        match self {
            Self::CommonModule => "mssql-common-module-configsave",
            Self::DocumentFormModule => "mssql-document-form-module-configsave",
        }
    }

    fn validate(self, selection: &Selection, verify: Option<bool>) -> Result<&str> {
        if verify == Some(false) {
            return Err(RequestRefused("SQL editor import requires mandatory verification").into());
        }
        if selection.files.len() != 1 || !selection.objects.is_empty() || selection.changed {
            return Err(RequestRefused(self.refusal()).into());
        }
        let path = &selection.files[0];
        let component = |name: &str| {
            !name.is_empty() && name.chars().all(|ch| ch.is_alphanumeric() || ch == '_')
        };
        let accepted = match self {
            Self::CommonModule => {
                let route = crate::compiler::families::assets::SourceAssetRegistry
                    .route(
                        "CommonModule",
                        crate::compiler::families::assets::SourceAssetRole::Module,
                    )
                    .context("CommonModule file route is unavailable")?;
                let folder = crate::mssql_dump::root_family_folder(route.owner_family())
                    .context("CommonModule source folder is unavailable")?;
                let prefix = format!("{folder}/");
                let suffix = format!("/{}", route.relative_path());
                path.strip_prefix(&prefix)
                    .and_then(|tail| tail.strip_suffix(&suffix))
                    .is_some_and(component)
            }
            Self::DocumentFormModule => {
                let route = crate::compiler::families::assets::SourceAssetRegistry
                    .route(
                        "Form",
                        crate::compiler::families::assets::SourceAssetRole::FormModule,
                    )
                    .context("Form module file route is unavailable")?;
                let folder = crate::mssql_dump::root_family_folder("Document")
                    .context("Document source folder is unavailable")?;
                let forms = crate::metadata_model::index::NESTED_COLLECTIONS
                    .iter()
                    .find_map(|(collection, kind)| {
                        (*kind == route.owner_family()).then_some(*collection)
                    })
                    .context("Form source folder is unavailable")?;
                let prefix = format!("{folder}/");
                let suffix = format!("/{}", route.relative_path());
                path.strip_prefix(&prefix)
                    .and_then(|tail| tail.strip_suffix(&suffix))
                    .is_some_and(|owner| {
                        let mut parts = owner.split('/');
                        matches!((parts.next(), parts.next(), parts.next(), parts.next()),
                            (Some(document), Some(collection), Some(form), None)
                                if collection == forms && component(document) && component(form))
                    })
            }
        };
        if !accepted {
            return Err(RequestRefused(self.refusal()).into());
        }
        Ok(path)
    }
}

impl CommonModulePlan {
    /// Read-only preparation of one explicit canonical existing module path.
    /// There is no platform-build inference, arbitrary multipart policy or
    /// user-supplied schema admission; full snapshot CAS remains mandatory.
    pub fn prepare(
        client: &dyn crate::sql::SqlClient,
        database: &str,
        source_dir: &Path,
        selection: &Selection,
        verify: Option<bool>,
        check: readset::Check,
    ) -> Result<Self> {
        Self::prepare_kind(
            client,
            database,
            source_dir,
            selection,
            verify,
            check,
            SelectedModuleKind::CommonModule,
        )
    }

    /// Prepare either admitted module role, classifying only request syntax
    /// before source or SQL I/O. A failed compiler never falls back to another role.
    pub(super) fn prepare_supported(
        client: &dyn crate::sql::SqlClient,
        database: &str,
        source_dir: &Path,
        selection: &Selection,
        verify: Option<bool>,
        check: readset::Check,
    ) -> Result<Self> {
        if SelectedModuleKind::DocumentFormModule
            .validate(selection, verify)
            .is_ok()
        {
            Self::prepare_document_form(client, database, source_dir, selection, verify, check)
        } else {
            Self::prepare(client, database, source_dir, selection, verify, check)
        }
    }

    /// Read-only preparation of one existing Document-owned native Form module.
    pub(super) fn prepare_document_form(
        client: &dyn crate::sql::SqlClient,
        database: &str,
        source_dir: &Path,
        selection: &Selection,
        verify: Option<bool>,
        check: readset::Check,
    ) -> Result<Self> {
        Self::prepare_kind(
            client,
            database,
            source_dir,
            selection,
            verify,
            check,
            SelectedModuleKind::DocumentFormModule,
        )
    }

    fn prepare_kind(
        client: &dyn crate::sql::SqlClient,
        database: &str,
        source_dir: &Path,
        selection: &Selection,
        verify: Option<bool>,
        check: readset::Check,
        kind: SelectedModuleKind,
    ) -> Result<Self> {
        check()?;
        let path = kind.validate(selection, verify)?;
        let source =
            SourceAdmission::prepare(source_dir, selection, verify, SourceLimits::default())?;
        check()?;
        let limits = readset::Limits::default();
        let storage =
            snapshot::observe_common_module_profile(client, database, limits, check.clone())?;
        let original =
            snapshot::capture(client, storage.snapshot_profile(), limits, check.clone())?;
        let edit = match kind {
            SelectedModuleKind::CommonModule => source.prepare_common_module_layout(
                &original,
                path,
                selected::Limits::default(),
                &check,
            )?,
            SelectedModuleKind::DocumentFormModule => source.prepare_document_form_module_layout(
                &original,
                path,
                selected::Limits::default(),
                &check,
            )?,
        };
        let policy = transaction::WritePolicy::existing_single_part(1024 * 1024, 4 * 1024 * 1024)?;
        let staged_rows = transaction::preflight_selected_parts(&original, &edit, &policy, &check)?;
        check()?;
        source.revalidate()?;
        let summary = CommonModulePreflight {
            backend: kind.backend(),
            target: database.into(),
            files: selection.files.clone(),
            missing_references: Vec::new(),
            ready: true,
            staged_rows,
        };
        Ok(Self {
            source,
            source_dir: source_dir.into(),
            original,
            edit,
            storage,
            policy,
            summary,
        })
    }
    pub fn summary(&self) -> &CommonModulePreflight {
        &self.summary
    }
    pub fn staged_rows(&self) -> usize {
        self.summary.staged_rows
    }
    /// Backend identity is retained by the prepared operation, never supplied by confirmation.
    pub(super) fn backend(&self) -> &'static str {
        self.summary.backend
    }

    /// Consumes the prepared operation. Callers must first consume the one-shot
    /// target/connection-bound token and obtain explicit user confirmation.
    /// Publication never compiles another path or rereads replacement contents.
    pub fn publish(
        self,
        client: &dyn crate::sql::SqlClient,
        current_database: &str,
        current_source_dir: &Path,
        check: readset::Check,
    ) -> transaction::Report {
        let before = (|| -> Result<()> {
            check()?;
            if current_database != self.storage.snapshot_profile().identity.database
                || current_source_dir != self.source_dir.as_path()
            {
                bail!("SQL preflight binding changed; prepare again");
            }
            self.source.revalidate()
        })();
        if let Err(error) = before {
            return transaction::Report {
                state: transaction::State::NotPublished,
                error: Some(error),
                cleanup_uncertain: false,
            };
        }
        transaction::stage(
            client,
            &self.original,
            &self.edit,
            self.storage.snapshot_profile(),
            &self.policy,
            check,
            &|| self.source.revalidate(),
        )
    }
}

#[cfg(test)]
mod selected_plan_request_tests {
    use super::*;

    const FORM: &str = "Documents/Synthetic/Forms/Main/Ext/Form/Module.bsl";

    fn files(paths: &[&str]) -> Selection {
        Selection {
            files: paths.iter().map(|path| (*path).into()).collect(),
            ..Selection::default()
        }
    }

    #[test]
    fn canonical_paths_keep_common_and_document_form_admissions_separate() {
        for path in [FORM, "Documents/Документ/Forms/Форма_1/Ext/Form/Module.bsl"] {
            assert_eq!(
                SelectedModuleKind::DocumentFormModule
                    .validate(&files(&[path]), Some(true))
                    .unwrap(),
                path
            );
            assert!(
                SelectedModuleKind::CommonModule
                    .validate(&files(&[path]), None)
                    .is_err()
            );
        }
        let common = "CommonModules/Synthetic/Ext/Module.bsl";
        assert_eq!(
            SelectedModuleKind::CommonModule
                .validate(&files(&[common]), None)
                .unwrap(),
            common
        );
        assert!(
            SelectedModuleKind::DocumentFormModule
                .validate(&files(&[common]), None)
                .is_err()
        );
        assert_eq!(
            SelectedModuleKind::CommonModule.backend(),
            "mssql-common-module-configsave"
        );
        assert_eq!(
            SelectedModuleKind::DocumentFormModule.backend(),
            "mssql-document-form-module-configsave"
        );
    }

    #[test]
    fn supported_roles_dispatch_before_source_without_compiler_fallback() {
        let sql = crate::sql::SqlExec::detached("SQL must not be contacted");
        let client = sql.client().unwrap();
        let root = crate::test_scratch::TestScratch::new("sql-selected-role");
        let absent = root.join("missing-source");
        let check: readset::Check = Arc::new(|| Ok(()));
        for path in [FORM, "CommonModules/Synthetic/Ext/Module.bsl"] {
            let error = CommonModulePlan::prepare_supported(
                client,
                "Synthetic",
                &absent,
                &files(&[path]),
                Some(true),
                check.clone(),
            )
            .err()
            .unwrap();
            assert!(
                error.downcast_ref::<RequestRefused>().is_none(),
                "{error:#}"
            );
        }
        for (selection, verify) in [
            (files(&[FORM]), Some(false)),
            (
                files(&["Documents/Synthetic/Forms/Main/Ext/Form.xml"]),
                Some(true),
            ),
            (files(&[FORM, FORM]), Some(true)),
        ] {
            let error = CommonModulePlan::prepare_supported(
                client,
                "Synthetic",
                &absent,
                &selection,
                verify,
                check.clone(),
            )
            .err()
            .unwrap();
            assert!(
                error.downcast_ref::<RequestRefused>().is_some(),
                "{error:#}"
            );
        }
        assert!(!absent.exists());
    }

    #[test]
    fn document_form_scope_refuses_before_source_or_sql() {
        let sql = crate::sql::SqlExec::detached("SQL must not be contacted");
        let client = sql.client().unwrap();
        let absent = Path::new("nonexistent-document-form-plan-source");
        let check: readset::Check = Arc::new(|| Ok(()));
        let mut cases = vec![
            (files(&[FORM]), Some(false)),
            (Selection::default(), None),
            (files(&[FORM, FORM]), None),
            (
                Selection {
                    objects: vec!["Document.Synthetic.Form.Main".into()],
                    ..Selection::default()
                },
                None,
            ),
            (
                Selection {
                    changed: true,
                    ..Selection::default()
                },
                None,
            ),
            (
                Selection {
                    changed: true,
                    ..files(&[FORM])
                },
                None,
            ),
            (
                Selection {
                    objects: vec!["Document.Synthetic".into()],
                    ..files(&[FORM])
                },
                None,
            ),
        ];
        for path in [
            "Documents/Synthetic/Forms/Main/Ext/Form.xml",
            "Documents/Synthetic/Forms/Main.xml",
            "Catalogs/Synthetic/Forms/Main/Ext/Form/Module.bsl",
            "CommonForms/Main/Ext/Form/Module.bsl",
            "Documents/Synthetic/Ext/ObjectModule.bsl",
            "documents/Synthetic/Forms/Main/Ext/Form/Module.bsl",
            "Documents/Synthetic/forms/Main/Ext/Form/Module.bsl",
            "Documents/Synthetic/Forms/Main/Ext/Form/module.bsl",
            "Documents/Synthetic/Forms/Main/Extra/Ext/Form/Module.bsl",
            "Documents/Synthetic/Forms/../Ext/Form/Module.bsl",
            "Documents//Forms/Main/Ext/Form/Module.bsl",
            "Documents/Synthetic/Forms//Ext/Form/Module.bsl",
            "Documents/Synthetic/Forms/Main/Ext/Form/Module.bsl/",
            "Documents\\Synthetic\\Forms\\Main\\Ext\\Form\\Module.bsl",
        ] {
            cases.push((files(&[path]), None));
        }
        for (selection, verify) in cases {
            let error = CommonModulePlan::prepare_document_form(
                client,
                "Synthetic",
                absent,
                &selection,
                verify,
                check.clone(),
            )
            .err()
            .unwrap();
            let refusal = error
                .downcast_ref::<RequestRefused>()
                .expect("must refuse before I/O");
            assert_eq!(
                refusal.to_string(),
                if verify == Some(false) {
                    "SQL editor import requires mandatory verification"
                } else {
                    "SQL preflight currently supports one existing Document Form module file"
                }
            );
        }
    }
}
