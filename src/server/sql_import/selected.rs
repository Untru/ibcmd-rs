//! Snapshot-only compilation of existing top-level CommonModule text edits.
//!
//! No SqlClient, fetch, observer, publisher or filesystem path is consumed here.
//! The resulting delta is not a transaction capability. A publisher must still
//! revalidate the complete captured readset, source custody and session policy.
use super::readset::{Check, Readset, Table};
use crate::{
    compiler::families::{
        assets::{SourceAssetRegistry, SourceAssetRole},
        modules::{CommonModuleProfile, decode_common_module_blob, decode_common_module_layout},
    },
    metadata_model::{
        brace::{Brace, parse_row},
        export::configuration_objects,
        root::export::root_configuration_uuid,
    },
    module_blob::{pack_module_blob_bytes, patch_versions_blob_bytes},
    mssql_dump::{validate_sql_versions_change, validate_sql_versions_inventory},
};
use anyhow::{Context, Result, ensure};
use flate2::read::DeflateDecoder;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    io::Read,
};
use uuid::Uuid;

pub mod form;

type Rows = BTreeMap<String, Vec<u8>>;

/// Cumulative checked packed-input/plaintext caps, not RPC arguments.
/// Existing codecs may allocate additional bounded copies of prechecked inputs.
#[derive(Clone, Copy)]
pub struct Limits {
    pub packed_bytes: usize,
    pub inflated_bytes: usize,
    pub source_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            packed_bytes: 64 * 1024 * 1024,
            inflated_bytes: 64 * 1024 * 1024,
            source_bytes: 16 * 1024 * 1024,
        }
    }
}
struct Budget<'a> {
    left: usize,
    check: &'a Check,
}
impl Budget<'_> {
    fn inflate(&mut self, packed: &[u8]) -> Result<Vec<u8>> {
        let mut decoder = DeflateDecoder::new(packed);
        let mut out = Vec::new();
        let mut chunk = [0; 64 * 1024];
        loop {
            (self.check)()?;
            let n = decoder.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            self.left = self
                .left
                .checked_sub(n)
                .context("SQL compiler inflation budget exceeded")?;
            out.extend_from_slice(&chunk[..n]);
        }
        ensure!(
            decoder.total_in() == packed.len() as u64,
            "SQL blob has trailing compressed data"
        );
        Ok(out)
    }
    fn descriptor(&mut self, packed: &[u8]) -> Result<Brace> {
        let plain = self.inflate(packed)?;
        require_nesting_bound(&plain, self.check)?;
        (self.check)()?;
        parse_row(&plain)
    }
}

// A depth preflight for the existing Brace parser, not another descriptor
// grammar. In particular root's valid empty trailing cell stays unchanged.
fn require_nesting_bound(plain: &[u8], check: &Check) -> Result<()> {
    let mut depth = 0usize;
    let mut quoted = false;
    for (index, byte) in plain.iter().enumerate() {
        if index % (64 * 1024) == 0 {
            check()?;
        }
        match byte {
            b'"' => quoted = !quoted,
            b'{' if !quoted => {
                depth += 1;
                ensure!(depth <= 64, "SQL descriptor nesting budget exceeded");
            }
            b'}' if !quoted => {
                depth = depth.checked_sub(1).context("unbalanced SQL descriptor")?;
            }
            _ => {}
        }
    }
    ensure!(!quoted && depth == 0, "unclosed SQL descriptor");
    check()?;
    Ok(())
}

/// An immutable body/versions delta. No SQL headers or write authority are synthesized.
pub struct ModuleEdit {
    readset_digest: [u8; 32],
    replacements: Rows,
}
#[cfg(test)]
pub(super) fn recording_edit(snapshot: &Readset, replacements: Rows) -> ModuleEdit {
    ModuleEdit {
        readset_digest: snapshot.digest().unwrap(),
        replacements,
    }
}

impl ModuleEdit {
    pub fn readset_digest(&self) -> &[u8; 32] {
        &self.readset_digest
    }
    pub fn replacements(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.replacements
    }
}

fn assemble(snapshot: &Readset, table: Table, left: &mut usize, check: &Check) -> Result<Rows> {
    let mut rows: Rows = BTreeMap::new();
    for (key, part) in snapshot.parts(table) {
        check()?;
        *left = left
            .checked_sub(part.binary.len())
            .context("SQL compiler packed budget exceeded")?;
        // Builder already checked contiguity, exact sizes, aliases and part bounds.
        rows.entry(key.file_name.clone())
            .or_default()
            .extend_from_slice(&part.binary);
    }
    Ok(rows)
}
fn plain_key(key: &str) -> Result<()> {
    if matches!(key, "root" | "version" | "versions") {
        return Ok(());
    }
    let owner = key.split('.').next().unwrap_or_default();
    ensure!(
        Uuid::parse_str(owner)?.to_string() == owner,
        "SQL compiler requires canonical plain row names"
    );
    if key != owner {
        crate::compiler::families::assets::ConfigRowId::parse(key)
            .map_err(|_| anyhow::anyhow!("unsupported SQL storage suffix"))?;
    }
    Ok(())
}

struct CapturedView<'a> {
    rows: Rows,
    members: Vec<(String, String)>,
    configuration: String,
    old_versions: Vec<u8>,
    budget: Budget<'a>,
}
fn captured_view<'a>(
    snapshot: &Readset,
    limits: Limits,
    check: &'a Check,
) -> Result<CapturedView<'a>> {
    check()?;
    ensure!(
        limits.packed_bytes > 0 && limits.inflated_bytes > 0 && limits.source_bytes > 0,
        "invalid SQL compiler limits"
    );
    let mut left = limits.packed_bytes;
    let config = assemble(snapshot, Table::Config, &mut left, check)?;
    let saved = assemble(snapshot, Table::ConfigSave, &mut left, check)?;
    for name in config.keys().chain(saved.keys()) {
        check()?;
        plain_key(name)?;
    }
    let mut budget = Budget {
        left: limits.inflated_bytes,
        check,
    };
    let config_names = config.keys().cloned().collect();
    let active_versions = config
        .get("versions")
        .context("SQL snapshot has no active versions")?;
    validate_sql_versions_inventory(&budget.inflate(active_versions)?, &config_names)?;
    ensure!(
        saved.is_empty() || saved.contains_key("versions"),
        "incomplete SQL staging has no versions inventory"
    );
    let mut effective = config;
    effective.extend(saved);
    let names = effective.keys().cloned().collect();
    let versions = effective
        .get("versions")
        .context("SQL snapshot has no effective versions")?;
    let old_versions = budget.inflate(versions)?;
    // Validate against the UNPRUNED union, including CONFIG-only neighbours.
    validate_sql_versions_inventory(&old_versions, &names)?;
    let root = budget.descriptor(effective.get("root").context("SQL snapshot has no root")?)?;
    let configuration = root_configuration_uuid(&root)?;
    let root_row = budget.descriptor(
        effective
            .get(&configuration)
            .context("SQL snapshot lacks Configuration row")?,
    )?;
    let identity = crate::metadata_model::export::object_names("Configuration", &root_row)?;
    ensure!(
        identity.uuid == configuration,
        "SQL Configuration descriptor identity changed"
    );
    let members = configuration_objects(&root_row)?;
    Ok(CapturedView {
        rows: effective,
        members,
        configuration,
        old_versions,
        budget,
    })
}

/// Compiles one selected module from a coherent captured snapshot and immutable
/// original-census source bytes. Other file roles, aliases and incomplete stage
/// inventories refuse. Unselected CONFIGSAVE payloads/headers are never rebuilt.
/// Prepare already-held filesystem bytes. No pathname is opened here.
/// Source custody and target/profile publication guards remain caller-owned.
pub fn prepare_module_file(
    snapshot: &Readset,
    selected_path: &str,
    text: &[u8],
    profile: &CommonModuleProfile,
    limits: Limits,
    check: &Check,
) -> Result<ModuleEdit> {
    prepare_module_text(
        snapshot,
        selected_path,
        text,
        Some(profile),
        limits,
        check,
        true,
    )
}

/// Compile immutable storage text; callers using source-file bytes use
/// prepare_module_file so native filesystem representation is not reimported.
pub fn prepare_module(
    snapshot: &Readset,
    selected_path: &str,
    text: &[u8],
    profile: &CommonModuleProfile,
    limits: Limits,
    check: &Check,
) -> Result<ModuleEdit> {
    prepare_module_text(
        snapshot,
        selected_path,
        text,
        Some(profile),
        limits,
        check,
        false,
    )
}

/// Compile the existing CommonModule grammar without claiming a platform build.
/// Schema/physical operation admission and source custody remain separate.
/// No profile coordinate, write capability or activation authority is inferred.
pub fn prepare_module_file_layout(
    snapshot: &Readset,
    selected_path: &str,
    text: &[u8],
    limits: Limits,
    check: &Check,
) -> Result<ModuleEdit> {
    prepare_module_text(snapshot, selected_path, text, None, limits, check, true)
}

fn prepare_module_text(
    snapshot: &Readset,
    selected_path: &str,
    text: &[u8],
    profile: Option<&CommonModuleProfile>,
    limits: Limits,
    check: &Check,
    filesystem: bool,
) -> Result<ModuleEdit> {
    check()?;
    ensure!(
        limits.packed_bytes > 0 && limits.inflated_bytes > 0 && limits.source_bytes > 0,
        "invalid SQL compiler limits"
    );
    ensure!(
        text.len() <= limits.source_bytes,
        "SQL module source budget exceeded"
    );
    std::str::from_utf8(text).context("SQL module source is not UTF-8")?;
    ensure!(!text.contains(&0), "SQL module source contains NUL");
    let CapturedView {
        rows: effective,
        members,
        old_versions,
        mut budget,
        ..
    } = captured_view(snapshot, limits, check)?;
    let versions = effective
        .get("versions")
        .expect("captured versions inventory");
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut selected = None;
    let route = SourceAssetRegistry
        .route("CommonModule", SourceAssetRole::Module)
        .context("no CommonModule module route")?;
    let folder = crate::mssql_dump::root_family_folder(route.owner_family())
        .context("no CommonModule folder")?;
    for (kind, id) in members {
        check()?;
        ensure!(
            Uuid::parse_str(&id)?.to_string() == id && ids.insert(id.clone()),
            "duplicate or noncanonical SQL owner membership"
        );
        if kind != route.owner_family() {
            continue;
        }
        let blob = effective
            .get(&id)
            .context("SQL module descriptor is missing")?;
        // Bound/depth-check BEFORE the existing typed decoder inflates/parses it.
        let _ = budget.descriptor(blob)?;
        let module = match profile {
            Some(profile) => decode_common_module_blob(blob, profile)?,
            None => decode_common_module_layout(blob)?,
        };
        ensure!(
            module.uuid.to_string() == id,
            "SQL module descriptor identity changed"
        );
        ensure!(
            !module.name.is_empty() && module.name.chars().all(|c| c.is_alphanumeric() || c == '_'),
            "unsupported SQL module name"
        );
        let path = format!(
            "{folder}/{}/{relative}",
            module.name,
            relative = route.relative_path()
        );
        ensure!(
            paths.insert(path.to_lowercase()),
            "ambiguous SQL module source path"
        );
        if path == selected_path {
            selected = Some(format!("{id}{}", route.suffix()));
        }
    }
    let body_key = selected.context("selection is not one existing top-level CommonModule body")?;
    let body = effective
        .get(&body_key)
        .context("selected SQL module body is missing")?;
    let old_container = budget.inflate(body)?;
    let old_elements = crate::v8_container::parse_v8_container(&old_container)?;
    ensure!(
        old_elements.len() == 2 && old_elements[0].name == "info" && old_elements[1].name == "text",
        "unsupported SQL module container members"
    );
    let old_text = &old_elements[1].data;
    std::str::from_utf8(old_text).context("existing SQL module is not UTF-8 source")?;
    // Compare original source bytes to the EXPECTED filesystem projection before
    // applying storage policy. Never rewrite the source or its custody bytes.
    // Native projection cannot shorten storage text, so avoid a large copy when
    // its length already proves that this input file differs.
    let prepared_text = if !filesystem {
        Cow::Borrowed(text)
    } else if old_text.len() <= text.len()
        && crate::mssql_dump::native_module_file_bytes(old_text) == text
    {
        Cow::Borrowed(old_text.as_slice())
    } else {
        Cow::Owned(
            crate::load::module_text_from_bytes(text, selected_path).map_err(anyhow::Error::msg)?,
        )
    };
    check()?;
    ensure!(
        prepared_text.len() <= limits.source_bytes,
        "SQL compiled module source budget exceeded"
    );
    let text = prepared_text.as_ref();
    let mut replacements = Rows::new();
    if old_text.as_slice() != text {
        check()?;
        let packed = pack_module_blob_bytes(text, Some(body), None)?;
        let new_container = budget.inflate(&packed.blob)?;
        let new_elements = crate::v8_container::parse_v8_container(&new_container)?;
        ensure!(
            new_elements.len() == 2
                && new_elements[0] == old_elements[0]
                && new_elements[1].name == old_elements[1].name
                && new_elements[1].header == old_elements[1].header,
            "SQL module compiler changed native auxiliary content"
        );
        ensure!(
            new_elements[1].data == text,
            "compiled SQL module does not match selected bytes"
        );
        let patched = patch_versions_blob_bytes(versions, std::slice::from_ref(&body_key), false)?;
        let new_versions = budget.inflate(&patched.blob)?;
        let changed = BTreeSet::from([body_key.clone()]);
        validate_sql_versions_change(&old_versions, &new_versions, &changed)?;
        replacements.insert(body_key, packed.blob);
        replacements.insert("versions".to_owned(), patched.blob);
    }
    check()?;
    Ok(ModuleEdit {
        readset_digest: snapshot.digest()?,
        replacements,
    })
}

#[cfg(test)]
mod tests {
    use super::super::readset::{Builder, Input, Key, Part};
    use super::*;
    use crate::module_blob::unpack_module_blob_text;
    use flate2::{Compression, write::DeflateEncoder};
    use std::io::Write;
    use std::sync::{Arc, OnceLock};
    pub(super) fn pack(bytes: &[u8]) -> Vec<u8> {
        let mut e = DeflateEncoder::new(Vec::new(), Compression::default());
        e.write_all(bytes).unwrap();
        e.finish().unwrap()
    }
    pub(super) fn fixture() -> Rows {
        static ROWS: OnceLock<Rows> = OnceLock::new();
        ROWS.get_or_init(|| {
            // Only cloned row bytes outlive this closure; the later reader
            // closes before the temporary public fixture is removed.
            let root = crate::test_scratch::TestScratch::new("sql-module-snapshot");
            let gzip = include_bytes!("../../../tests/fixtures/file-infobase/tiny.1CD.gz");
            let mut bytes = Vec::new();
            flate2::read::GzDecoder::new(gzip.as_slice())
                .read_to_end(&mut bytes)
                .unwrap();
            let path = root.join("tiny.1CD");
            std::fs::write(&path, bytes).unwrap();
            let mut db = ibcmd_filedb::FileDb::open(&path).unwrap();
            let rows = crate::mssql_dump::file_db_rows_from_reader(&mut db).unwrap();
            rows.into_iter()
                .map(|(key, bytes)| (key, bytes.as_ref().clone()))
                .collect()
        })
        .clone()
    }
    fn profile() -> CommonModuleProfile {
        let registry = crate::profile_registry::load_bundled_profile_registry().unwrap();
        CommonModuleProfile::from_effective(
            registry
                .get(&ibcmd_core::artifact::ProfileId::parse("platform-8.3.27.1989").unwrap())
                .unwrap(),
        )
        .unwrap()
    }
    pub(super) fn check() -> Check {
        Arc::new(|| Ok(()))
    }
    pub(super) fn snapshot(config: &Rows, saved: &Rows) -> Readset {
        let identity = crate::mssql_live_continue::DatabaseIdentity {
            server: "synthetic".into(),
            database: "synthetic".into(),
            database_guid: Uuid::from_u128(1).to_string(),
            family_guid: Uuid::from_u128(2).to_string(),
            recovery_fork: Uuid::from_u128(3).to_string(),
        };
        let mut builder = Builder::new(
            identity,
            [7; 32],
            super::super::readset::Limits::default(),
            check(),
        )
        .unwrap();
        for (table, rows) in [(Table::Config, config), (Table::ConfigSave, saved)] {
            for (name, bytes) in rows {
                builder
                    .push(
                        table,
                        Input {
                            key: Key {
                                file_name: name.clone(),
                                part_no: 0,
                            },
                            part: Part {
                                data_size: bytes.len() as u64,
                                header: vec![4, 5, 6],
                                binary: Arc::new(bytes.clone()),
                            },
                        },
                    )
                    .unwrap();
            }
        }
        builder.finish().unwrap()
    }
    fn module(rows: &Rows) -> (String, String) {
        let profile = profile();
        rows.iter()
            .find_map(|(key, bytes)| {
                decode_common_module_blob(bytes, &profile).ok().map(|m| {
                    (
                        key.clone(),
                        format!("CommonModules/{}/Ext/Module.bsl", m.name),
                    )
                })
            })
            .unwrap()
    }
    fn versions_text(count: usize, pairs: &[(&str, u128)]) -> Vec<u8> {
        let mut text = format!("{{1,{count},\"\",{}", Uuid::from_u128(1));
        for (name, value) in pairs {
            text.push_str(&format!(",\"{name}\",{}", Uuid::from_u128(*value)));
        }
        text.push('}');
        text.into_bytes()
    }
    #[test]
    fn sql_versions_preserve_unselected_and_beyond_count_entries() {
        let old = versions_text(1, &[("selected.0", 2), ("neighbour.0", 3), ("root", 4)]);
        let changed = BTreeSet::from(["selected.0".into()]);
        let good = versions_text(1, &[("selected.0", 5), ("neighbour.0", 3), ("root", 4)]);
        validate_sql_versions_change(&old, &good, &changed).unwrap();
        for pairs in [
            vec![("selected.0", 5), ("neighbour.0", 6), ("root", 4)],
            vec![("selected.0", 5), ("neighbour.0", 3), ("root", 6)],
            vec![("selected.0", 5), ("root", 4)],
            vec![
                ("selected.0", 5),
                ("neighbour.0", 3),
                ("root", 4),
                ("new", 6),
            ],
            vec![("selected.0", 5), ("selected.0", 3), ("root", 4)],
            vec![("neighbour.0", 3), ("selected.0", 5), ("root", 4)],
        ] {
            assert!(
                validate_sql_versions_change(&old, &versions_text(1, &pairs), &changed).is_err()
            );
        }
        assert!(
            validate_sql_versions_change(
                &old,
                &versions_text(2, &[("selected.0", 5), ("neighbour.0", 3), ("root", 4)]),
                &changed
            )
            .is_err()
        );
        assert!(validate_sql_versions_change(&old, &old, &changed).is_err());
        assert!(
            validate_sql_versions_change(&old, &good, &BTreeSet::from(["absent.0".into()]))
                .is_err()
        );
    }
    fn raw_filesystem_module_case() -> (Rows, String, String, Vec<u8>) {
        let mut rows = fixture();
        let (id, path) = module(&rows);
        let key = format!("{id}.0");
        let raw = b"Procedure Hello() Export\nEndProcedure\n".to_vec();
        assert_eq!(raw.len(), 38);
        let packed = pack_module_blob_bytes(&raw, Some(&rows[&key]), None).unwrap();
        rows.insert(key.clone(), packed.blob);
        (rows, key, path, raw)
    }

    fn admit_module_source(
        path: &str,
        bytes: &[u8],
    ) -> (
        crate::import_plan::ImportScratch,
        crate::server::sql_import::SourceAdmission,
    ) {
        let root = crate::import_plan::ImportScratch::create().unwrap();
        let target = root.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, bytes).unwrap();
        let metadata = path
            .strip_suffix("/Ext/Module.bsl")
            .or_else(|| path.strip_suffix("/Ext/Form/Module.bsl"))
            .unwrap()
            .to_owned()
            + ".xml";
        std::fs::write(root.join(&metadata), b"<MetaDataObject/>").unwrap();
        let admitted = crate::server::sql_import::SourceAdmission::prepare(
            &root.path,
            &crate::file_import::Selection {
                files: vec![path.into()],
                ..Default::default()
            },
            Some(true),
            crate::server::sql_import::SourceLimits::default(),
        )
        .unwrap();
        (root, admitted)
    }

    #[test]
    fn filesystem_module_admission_compiles_only_retained_selected_bytes() {
        let (rows, _, path, raw) = raw_filesystem_module_case();
        let captured = snapshot(&rows, &Rows::new());
        let source = crate::mssql_dump::native_module_file_bytes(&raw);
        let (_root, admitted) = admit_module_source(&path, &source);
        let edit = admitted
            .prepare_common_module(&captured, &path, &profile(), Limits::default(), &check())
            .unwrap();
        assert!(edit.replacements().is_empty());
        assert_eq!(admitted.bytes(&path).unwrap(), source);
        assert!(
            admitted
                .prepare_common_module(
                    &captured,
                    "CommonModules/Other/Ext/Module.bsl",
                    &profile(),
                    Limits::default(),
                    &check()
                )
                .is_err()
        );
    }

    #[test]
    fn filesystem_form_admission_retains_original_bytes_and_refuses_wrong_selection() {
        let (captured, path, raw, profile) =
            form::transaction_fixture(|rows, saved, _| snapshot(rows, saved));
        let source = crate::mssql_dump::native_module_file_bytes(&raw);
        let (root, admitted) = admit_module_source(&path, &source);
        let before = captured.digest().unwrap();
        let edit = admitted
            .prepare_document_form_module(&captured, &path, &profile, Limits::default(), &check())
            .unwrap();
        assert!(edit.replacements().is_empty());
        assert_eq!(captured.digest().unwrap(), before);
        assert_eq!(admitted.bytes(&path).unwrap(), source);
        assert!(
            admitted
                .prepare_document_form_module(
                    &captured,
                    &path.replace("Module.bsl", "Form.xml"),
                    &profile,
                    Limits::default(),
                    &check(),
                )
                .is_err()
        );
        std::fs::write(root.join(&path), [0xff]).unwrap();
        let error = admitted
            .prepare_document_form_module(&captured, &path, &profile, Limits::default(), &check())
            .err()
            .expect("mutated Form source accepted");
        assert!(error.chain().any(|cause| {
            cause
                .downcast_ref::<crate::mssql_source_change::SourceChangeError>()
                .is_some()
        }));
        assert_eq!(admitted.bytes(&path).unwrap(), source);
    }

    #[test]
    fn filesystem_module_admission_detects_mutation_during_compilation_without_reread() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let (rows, _, path, raw) = raw_filesystem_module_case();
        let captured = snapshot(&rows, &Rows::new());
        let source = crate::mssql_dump::native_module_file_bytes(&raw);
        let (root, admitted) = admit_module_source(&path, &source);
        let target = root.join(&path);
        let calls = AtomicUsize::new(0);
        let check: Check = std::sync::Arc::new(move || {
            if calls.fetch_add(1, Ordering::SeqCst) == 1 {
                std::fs::write(&target, [0xff])?;
            }
            Ok(())
        });
        let error = admitted
            .prepare_common_module(&captured, &path, &profile(), Limits::default(), &check)
            .err()
            .expect("changed source accepted");
        assert!(
            error.chain().any(|cause| cause
                .downcast_ref::<crate::mssql_source_change::SourceChangeError>()
                .is_some()),
            "must refuse the original-source witness, not decode replacement bytes: {error:#}"
        );
        assert_eq!(admitted.bytes(&path).unwrap(), source);
    }

    #[test]
    fn filesystem_module_native_noop_keeps_raw_storage_and_source_bytes() {
        let (rows, _, path, raw) = raw_filesystem_module_case();
        let captured = snapshot(&rows, &Rows::new());
        let digest = captured.digest().unwrap();
        let source = crate::mssql_dump::native_module_file_bytes(&raw);
        assert_eq!(source.len(), 43);
        let original = source.clone();
        let edit = prepare_module_file(
            &captured,
            &path,
            &source,
            &profile(),
            Limits::default(),
            &check(),
        )
        .unwrap();
        assert!(
            edit.replacements().is_empty(),
            "unchanged native file changed storage"
        );
        assert_eq!(source, original);
        assert_eq!(captured.digest().unwrap(), digest);
        let layout_edit =
            prepare_module_file_layout(&captured, &path, &source, Limits::default(), &check())
                .unwrap();
        assert!(layout_edit.replacements().is_empty());
        assert_eq!(layout_edit.readset_digest(), edit.readset_digest());
    }

    #[test]
    fn filesystem_module_edit_reuses_loader_policy_without_normalizing_custody() {
        let (rows, key, path, source) = raw_filesystem_module_case();
        let captured = snapshot(&rows, &Rows::new());
        let original = source.clone();
        let expected = crate::load::module_text_from_bytes(&source, &path).unwrap();
        assert_eq!(expected.len(), 40);
        let edit = prepare_module_file(
            &captured,
            &path,
            &source,
            &profile(),
            Limits::default(),
            &check(),
        )
        .unwrap();
        assert_eq!(
            edit.replacements().keys().cloned().collect::<BTreeSet<_>>(),
            BTreeSet::from([key.clone(), "versions".into()])
        );
        assert_eq!(
            unpack_module_blob_text(&edit.replacements()[&key]).unwrap(),
            expected
        );
        assert_eq!(source, original);
        assert_eq!(source.len(), 38);
    }

    #[test]
    fn filesystem_module_bounds_expanded_storage_text() {
        let (rows, _, path, _) = raw_filesystem_module_case();
        let captured = snapshot(&rows, &Rows::new());
        let source = b"\n\n\n\n";
        assert!(
            prepare_module_file(
                &captured,
                &path,
                source,
                &profile(),
                Limits {
                    source_bytes: 7,
                    ..Limits::default()
                },
                &check()
            )
            .is_err()
        );
        let edit = prepare_module_file(
            &captured,
            &path,
            source,
            &profile(),
            Limits {
                source_bytes: 8,
                ..Limits::default()
            },
            &check(),
        )
        .unwrap();
        assert!(!edit.replacements().is_empty());
    }

    #[test]
    fn snapshot_module_compiles_without_fetch_and_preserves_neighbours() {
        let rows = fixture();
        let (id, path) = module(&rows);
        let snapshot = snapshot(&rows, &Rows::new());
        let before = snapshot.digest().unwrap();
        let text = b"Procedure SnapshotOnly() Export\nEndProcedure\n";
        let edit = prepare_module(
            &snapshot,
            &path,
            text,
            &profile(),
            Limits::default(),
            &check(),
        )
        .unwrap();
        let key = format!("{id}.0");
        assert_eq!(
            edit.replacements.keys().cloned().collect::<BTreeSet<_>>(),
            BTreeSet::from([key.clone(), "versions".into()])
        );
        assert_eq!(
            unpack_module_blob_text(&edit.replacements[&key]).unwrap(),
            text
        );
        assert_eq!(edit.readset_digest(), &before);
        assert_eq!(snapshot.digest().unwrap(), before);
        let no_op = prepare_module(
            &snapshot,
            &path,
            &unpack_module_blob_text(&rows[&key]).unwrap(),
            &profile(),
            Limits::default(),
            &check(),
        )
        .unwrap();
        assert!(no_op.replacements().is_empty());
    }
    #[test]
    fn selection_does_not_authorize_descriptor_neighbour_or_wrong_role() {
        let rows = fixture();
        let (_, path) = module(&rows);
        let snapshot = snapshot(&rows, &Rows::new());
        for wrong in [
            path.replace("Module.bsl", "ManagerModule.bsl"),
            path.replace("CommonModules", "CommonForms"),
            path.to_lowercase(),
            "Configuration.xml".into(),
        ] {
            assert!(
                prepare_module(
                    &snapshot,
                    &wrong,
                    b"// new",
                    &profile(),
                    Limits::default(),
                    &check()
                )
                .is_err()
            );
        }
    }
    #[test]
    fn compiler_rejects_missing_body_staged_omission_alias_and_budget() {
        let rows = fixture();
        let (id, path) = module(&rows);
        let mut missing = rows.clone();
        missing.remove(&format!("{id}.0"));
        assert!(
            prepare_module(
                &snapshot(&missing, &Rows::new()),
                &path,
                b"// edit",
                &profile(),
                Limits::default(),
                &check()
            )
            .is_err()
        );
        let mut stage = Rows::new();
        stage.insert(
            "versions".into(),
            pack(&versions_text(1, &[("unrelated", 2)])),
        );
        assert!(
            prepare_module(
                &snapshot(&rows, &stage),
                &path,
                b"// edit",
                &profile(),
                Limits::default(),
                &check()
            )
            .is_err()
        );
        let mut aliases = rows.clone();
        aliases.insert("DynamicallyUpdated".into(), vec![0]);
        assert!(
            prepare_module(
                &snapshot(&aliases, &Rows::new()),
                &path,
                b"// edit",
                &profile(),
                Limits::default(),
                &check()
            )
            .is_err()
        );
        for limits in [
            Limits {
                packed_bytes: 1,
                ..Limits::default()
            },
            Limits {
                inflated_bytes: 1,
                ..Limits::default()
            },
            Limits {
                source_bytes: 1,
                ..Limits::default()
            },
        ] {
            assert!(
                prepare_module(
                    &snapshot(&rows, &Rows::new()),
                    &path,
                    b"// edit",
                    &profile(),
                    limits,
                    &check()
                )
                .is_err()
            );
        }
        let sentinel: Check = Arc::new(|| anyhow::bail!("typed-sentinel"));
        assert_eq!(
            prepare_module(
                &snapshot(&rows, &Rows::new()),
                &path,
                b"// edit",
                &profile(),
                Limits::default(),
                &sentinel
            )
            .err()
            .unwrap()
            .to_string(),
            "typed-sentinel"
        );
    }
    fn refusal(rows: &Rows, saved: &Rows, path: &str, expected: &str) {
        let error = prepare_module(
            &snapshot(rows, saved),
            path,
            b"// edit",
            &profile(),
            Limits::default(),
            &check(),
        )
        .err()
        .unwrap();
        assert!(
            format!("{error:#}").contains(expected),
            "expected {expected}, got {error:#}"
        );
    }
    fn inflate(bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        DeflateDecoder::new(bytes).read_to_end(&mut out).unwrap();
        out
    }
    fn configuration_key(rows: &Rows) -> String {
        root_configuration_uuid(&parse_row(&inflate(&rows["root"])).unwrap()).unwrap()
    }
    pub(super) fn change_members(node: &mut Brace, id: &str, replacement: Option<&str>) -> bool {
        let Some(items) = node.as_list_mut() else {
            return false;
        };
        if items.len() >= 3 && items[2..].iter().any(|v| v.as_atom() == Some(id)) {
            let count = items[1].as_atom().unwrap().parse::<usize>().unwrap();
            if let Some(id) = replacement {
                items.push(Brace::atom(id));
                items[1] = Brace::num((count + 1) as i64);
            } else {
                items.retain(|v| v.as_atom() != Some(id));
                items[1] = Brace::num((count - 1) as i64);
            }
            return true;
        }
        items.iter_mut().any(|v| change_members(v, id, replacement))
    }
    #[test]
    fn compiler_rejects_incomplete_stage_and_owner_binding_changes() {
        let rows = fixture();
        let (id, path) = module(&rows);
        let body = format!("{id}.0");
        let stage = Rows::from([(body, rows[&format!("{id}.0")].clone())]);
        refusal(&rows, &stage, &path, "incomplete SQL staging");
        let key = configuration_key(&rows);
        let mut wrong_root = rows.clone();
        let text = String::from_utf8(inflate(&rows[&key]))
            .unwrap()
            .replace(&key, &Uuid::from_u128(9876).to_string());
        wrong_root.insert(key.clone(), pack(text.as_bytes()));
        refusal(
            &wrong_root,
            &Rows::new(),
            &path,
            "Configuration descriptor identity",
        );
        for (duplicate, expected) in [
            (true, "duplicate or noncanonical SQL owner"),
            (false, "selection is not one existing"),
        ] {
            let mut modified = rows.clone();
            let mut descriptor = parse_row(&inflate(&rows[&key])).unwrap();
            assert!(change_members(
                &mut descriptor,
                &id,
                duplicate.then_some(id.as_str())
            ));
            modified.insert(
                key.clone(),
                pack(&crate::metadata_model::brace::serialize_row(&descriptor)),
            );
            refusal(&modified, &Rows::new(), &path, expected);
        }
        let mut wrong_owner = rows.clone();
        let text = String::from_utf8(inflate(&rows[&id]))
            .unwrap()
            .replace(&id, &Uuid::from_u128(9876).to_string());
        wrong_owner.insert(id, pack(text.as_bytes()));
        refusal(
            &wrong_owner,
            &Rows::new(),
            &path,
            "module descriptor identity",
        );
    }
    #[test]
    fn compiler_refuses_unknown_duplicate_and_missing_module_members() {
        let rows = fixture();
        let (id, path) = module(&rows);
        let key = format!("{id}.0");
        let elements = crate::v8_container::parse_v8_container(&inflate(&rows[&key])).unwrap();
        for members in [
            vec![elements[1].clone()],
            vec![
                elements[0].clone(),
                elements[1].clone(),
                elements[1].clone(),
            ],
            vec![
                elements[0].clone(),
                elements[1].clone(),
                crate::v8_container::V8Element {
                    name: "image".into(),
                    header: crate::v8_container::make_v8_element_header("image"),
                    data: vec![1, 2],
                },
            ],
            vec![
                elements[0].clone(),
                elements[1].clone(),
                crate::v8_container::V8Element {
                    name: "unknown".into(),
                    header: crate::v8_container::make_v8_element_header("unknown"),
                    data: vec![3],
                },
            ],
        ] {
            let mut modified = rows.clone();
            // Duplicate-name construction may itself be refused by the shared writer.
            if let Ok(container) = crate::v8_container::build_v8_container(&members) {
                modified.insert(key.clone(), pack(&container));
                assert!(
                    prepare_module(
                        &snapshot(&modified, &Rows::new()),
                        &path,
                        b"// edit",
                        &profile(),
                        Limits::default(),
                        &check()
                    )
                    .is_err()
                );
            }
        }
    }
    #[test]
    fn complete_stage_neighbour_and_auxiliary_info_remain_exact() {
        let mut rows = fixture();
        let (id, path) = module(&rows);
        let key = format!("{id}.0");
        rows.insert(
            key.clone(),
            pack_module_blob_bytes(b"// old", Some(&rows[&key]), Some(b"custom-info"))
                .unwrap()
                .blob,
        );
        let saved = Rows::from([
            (
                configuration_key(&rows),
                rows[&configuration_key(&rows)].clone(),
            ),
            ("versions".into(), rows["versions"].clone()),
        ]);
        let snapshot = snapshot(&rows, &saved);
        let before = snapshot.digest().unwrap();
        let edit = prepare_module(
            &snapshot,
            &path,
            b"// new",
            &profile(),
            Limits::default(),
            &check(),
        )
        .unwrap();
        let elements =
            crate::v8_container::parse_v8_container(&inflate(&edit.replacements[&key])).unwrap();
        assert_eq!(elements[0].data, b"custom-info");
        assert!(!edit.replacements.contains_key(&configuration_key(&rows)));
        assert_eq!(snapshot.digest().unwrap(), before);
    }
    #[test]
    fn compiler_refuses_case_colliding_names_and_mid_work_cancel() {
        let mut rows = fixture();
        let (id, path) = module(&rows);
        let mut other = decode_common_module_blob(&rows[&id], &profile()).unwrap();
        let other_id = Uuid::from_u128(9009).to_string();
        other.uuid = ibcmd_core::identity::ObjectUuid::parse(&other_id).unwrap();
        other.name = other.name.to_uppercase();
        rows.insert(
            other_id.clone(),
            crate::compiler::families::modules::encode_common_module_blob(&other, &profile())
                .unwrap(),
        );
        let config = configuration_key(&rows);
        let mut descriptor = parse_row(&inflate(&rows[&config])).unwrap();
        assert!(change_members(&mut descriptor, &id, Some(&other_id)));
        rows.insert(
            config,
            pack(&crate::metadata_model::brace::serialize_row(&descriptor)),
        );
        // Keep inventory valid so the test reaches owner/path ambiguity.
        let names = rows
            .keys()
            .filter(|key| !matches!(key.as_str(), "root" | "version" | "versions"))
            .cloned()
            .collect::<Vec<_>>();
        let pairs = names
            .iter()
            .map(|key| (key.as_str(), 2))
            .collect::<Vec<_>>();
        rows.insert(
            "versions".into(),
            pack(&versions_text(pairs.len() + 1, &pairs)),
        );
        refusal(
            &rows,
            &Rows::new(),
            &path,
            "ambiguous SQL module source path",
        );
        let rows = fixture();
        let snapshot = snapshot(&rows, &Rows::new());
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = counter.clone();
        let cancel: Check = Arc::new(move || {
            if seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= 5 {
                anyhow::bail!("mid-work-sentinel");
            }
            Ok(())
        });
        let error = prepare_module(
            &snapshot,
            &module(&rows).1,
            b"// new",
            &profile(),
            Limits::default(),
            &cancel,
        )
        .err()
        .unwrap();
        assert_eq!(error.to_string(), "mid-work-sentinel");
        assert!(counter.load(std::sync::atomic::Ordering::SeqCst) > 5);
    }
    #[test]
    fn generation_case_change_is_not_a_new_generation() {
        let old = versions_text(1, &[("selected.0", 0xabcdef)]);
        let after = String::from_utf8(old.clone())
            .unwrap()
            .replace("abcdef", "ABCDEF");
        assert!(
            validate_sql_versions_change(
                &old,
                after.as_bytes(),
                &BTreeSet::from(["selected.0".into()])
            )
            .is_err()
        );
    }
    #[test]
    fn descriptor_depth_guard_preserves_root_empty_cell_and_quoted_braces() {
        let mut budget = Budget {
            left: 4096,
            check: &check(),
        };
        let root = b"{2,00000000-0000-0000-0000-000000000001,}";
        assert_eq!(
            root_configuration_uuid(&budget.descriptor(&pack(root)).unwrap()).unwrap(),
            Uuid::from_u128(1).to_string()
        );
        assert!(
            budget
                .descriptor(&pack(br#"{1,"text {with} ""quoted"" braces",0}"#))
                .is_ok()
        );
        let deep = format!("{}0{}", "{".repeat(65), "}".repeat(65));
        assert!(
            budget
                .descriptor(&pack(deep.as_bytes()))
                .err()
                .unwrap()
                .to_string()
                .contains("nesting budget")
        );
        assert!(budget.descriptor(&pack(b"{1,\"unterminated}")).is_err());
        assert!(budget.descriptor(&pack(b"{1}}")).is_err());
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = calls.clone();
        let cancel: Check = Arc::new(move || {
            if observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                anyhow::bail!("quote-scan-cancel");
            }
            Ok(())
        });
        let long_quoted = format!("{{\"{}\"}}", "x".repeat(128 * 1024));
        assert_eq!(
            require_nesting_bound(long_quoted.as_bytes(), &cancel)
                .err()
                .unwrap()
                .to_string(),
            "quote-scan-cancel"
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    }
}
