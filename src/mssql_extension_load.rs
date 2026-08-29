//! Compile extension XML sources and stage an overlay in `ConfigCASSave`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use flate2::Compression;
use flate2::write::DeflateEncoder;
use ibcmd_core::artifact::ProfileId;
use ibcmd_core::storage::{MultipartIdentity, StorageImage, StoragePatch, StoragePatchOutcome};
use ibcmd_xml::source_tree::{SourceEntry, SourceTree};
use serde::Serialize;
use uuid::Uuid;

use crate::cli::{
    MssqlActivateStagedExtensionArgs, MssqlExtensionListArgs, MssqlExtensionListFormat,
    MssqlLoadExtensionArgs,
};
use crate::compiler::bootstrap::compile_extension_overlay_source_tree;
use crate::mssql_dump::cas::{
    CasHash, MssqlStorageTable, fetch_cas_storage_image_with_manifest_bcp,
};
use crate::mssql_extension_stage::{
    ConfigInfoIdentity, ExtensionRegistrySnapshot, ExtensionStagePlan, ExtensionStageRow,
    extension_namespace_prefix, prepare_extension_stage,
};
use crate::mssql_extensions::{MssqlExtensionInfo, list_extensions};
use crate::profile_registry::load_bundled_profile_registry;

#[derive(Debug, Serialize)]
pub struct MssqlExtensionLoadReport {
    pub schema_version: u32,
    pub database: String,
    pub all_extensions: bool,
    pub activation_required: bool,
    pub extensions: Vec<MssqlExtensionLoadEntry>,
}

#[derive(Debug, Serialize)]
pub struct MssqlExtensionLoadEntry {
    pub name: String,
    pub input_dir: String,
    pub staged_rows: usize,
    pub staged_bytes: u64,
    pub compiled_targets: usize,
    pub retained_base_targets: usize,
}

#[derive(Debug, Serialize)]
pub struct MssqlExtensionActivationReport {
    pub database: String,
    pub extension: String,
    pub dry_run: bool,
    pub executed: bool,
    pub activation: crate::mssql_extension_activation::ExtensionActivationDryRun,
    pub script: PathBuf,
    pub recovery: PathBuf,
}

struct PreparedLoad {
    extension: MssqlExtensionInfo,
    input_dir: PathBuf,
    plan: ExtensionStagePlan,
    compiled_targets: usize,
    retained_base_targets: usize,
}

pub fn load_extensions(args: &MssqlLoadExtensionArgs) -> Result<MssqlExtensionLoadReport> {
    if !args.allow_non_lab {
        bail!("direct extension writes require explicit --allow-non-lab");
    }
    let password = resolve_password(args)?;
    let registry = list_extensions(&MssqlExtensionListArgs {
        sqlcmd: args.sqlcmd.clone(),
        server: args.server.clone(),
        sql_user: args.sql_user.clone(),
        sql_pwd: args.sql_pwd.clone(),
        sql_pwd_env: args.sql_pwd_env.clone(),
        database: args.database.clone(),
        format: MssqlExtensionListFormat::Json,
    })?;
    let selected = select_extensions(
        &registry.extensions,
        args.extension.as_deref(),
        args.all_extensions,
    )?;
    let profiles = load_bundled_profile_registry()?;
    let profile_id = ProfileId::parse("platform-8.3.27.1989")?;
    let target_profile = profiles
        .get(&profile_id)
        .ok_or_else(|| anyhow!("bundled 8.3.27 target profile is missing"))?;

    // Compile and fetch every selected extension before the first write. This
    // prevents --all-extensions from publishing a prefix after a later source
    // tree has already failed validation.
    let mut prepared = Vec::with_capacity(selected.len());
    for extension in selected {
        let input_dir = if args.all_extensions {
            args.input_dir.join(&extension.name)
        } else {
            args.input_dir.clone()
        };
        let root = CasHash::parse_hex(&extension.active_cas_root)?;
        let (active, manifest) = fetch_cas_storage_image_with_manifest_bcp(
            &args.sqlcmd,
            &args.bcp_executable,
            &args.server,
            args.sql_user.as_deref(),
            password.as_deref(),
            &args.database,
            MssqlStorageTable::ConfigCas,
            root,
        )
        .with_context(|| format!("failed to fetch active extension {:?}", extension.name))?;
        let tree = ibcmd_xml::source_tree::read_source_tree(&input_dir).with_context(|| {
            format!(
                "failed to read extension source tree {}",
                input_dir.display()
            )
        })?;
        let (compile_tree, retained_metadata) = sanitize_extension_tree(&tree)?;
        let patch = compile_extension_overlay_source_tree(
            &compile_tree,
            args.source_version.version_axes().xml_dialect().clone(),
            target_profile,
        )
        .with_context(|| format!("failed to compile extension {:?}", extension.name))?
        .into_patch();
        let (plan, compiled_targets, retained_base_targets) = overlay_stage_plan(
            extension.physical_registry_id,
            manifest.identity(),
            &active,
            &patch,
            &retained_metadata,
        )
        .with_context(|| format!("failed to prepare extension {:?}", extension.name))?;
        prepared.push(PreparedLoad {
            extension: extension.clone(),
            input_dir,
            plan,
            compiled_targets,
            retained_base_targets,
        });
    }

    let mut reports = Vec::with_capacity(prepared.len());
    for item in prepared {
        let snapshot = ExtensionRegistrySnapshot {
            extension_id: item.extension.physical_registry_id,
            version: item.extension.registry_version,
            zipped_info: item.extension.zipped_info.clone(),
        };
        let execution = crate::mssql_extension_stage::execute_configcassave_stage(
            &args.sqlcmd,
            &args.server,
            args.sql_user.as_deref(),
            password.as_deref(),
            &args.database,
            &snapshot,
            &item.plan,
            args.replace_staging,
            args.allow_non_lab,
            args.sqlcmd_trust_cert,
        )?;
        reports.push(MssqlExtensionLoadEntry {
            name: item.extension.name,
            input_dir: item.input_dir.display().to_string(),
            staged_rows: execution.expected_rows,
            staged_bytes: execution.expected_bytes,
            compiled_targets: item.compiled_targets,
            retained_base_targets: item.retained_base_targets,
        });
    }
    Ok(MssqlExtensionLoadReport {
        schema_version: 1,
        database: args.database.clone(),
        all_extensions: args.all_extensions,
        activation_required: true,
        extensions: reports,
    })
}

pub fn activate_staged_extension(
    args: &MssqlActivateStagedExtensionArgs,
) -> Result<MssqlExtensionActivationReport> {
    if !args.allow_non_lab {
        bail!("direct extension writes require explicit --allow-non-lab");
    }
    let password = args
        .sql_pwd
        .clone()
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var(&args.sql_pwd_env).ok());
    let registry = list_extensions(&MssqlExtensionListArgs {
        sqlcmd: args.sqlcmd.clone(),
        server: args.server.clone(),
        sql_user: args.sql_user.clone(),
        sql_pwd: args.sql_pwd.clone(),
        sql_pwd_env: args.sql_pwd_env.clone(),
        database: args.database.clone(),
        format: MssqlExtensionListFormat::Json,
    })?;
    let extension = registry
        .extensions
        .iter()
        .find(|item| item.name == args.extension)
        .ok_or_else(|| anyhow!("extension {:?} was not found", args.extension))?;
    let namespace = extension_namespace_prefix(extension.physical_registry_id);
    let prefix = format!("{namespace}__");
    let all_stage = crate::mssql_dump::fetch_main_activation_rows_bcp(
        &args.sqlcmd,
        &args.bcp_executable,
        &args.server,
        args.sql_user.as_deref(),
        password.as_deref(),
        &args.database,
        "ConfigCASSave",
        &BTreeSet::new(),
    )?;
    let rows = all_stage
        .into_iter()
        .filter_map(|row| {
            row.file_name
                .strip_prefix(&prefix)
                .map(|logical_name| ExtensionStageRow {
                    logical_name: logical_name.to_owned(),
                    attributes: 0,
                    binary_data: row.binary_data,
                    part_no: row.part_no,
                })
        })
        .collect::<Vec<_>>();
    let stage = ExtensionStagePlan::from_complete_rows(extension.physical_registry_id, rows)?;
    let marker_name = format!("dbStruFinal{namespace}");
    let marker_rows = crate::mssql_dump::fetch_main_activation_rows_bcp(
        &args.sqlcmd,
        &args.bcp_executable,
        &args.server,
        args.sql_user.as_deref(),
        password.as_deref(),
        &args.database,
        "ConfigCAS",
        &BTreeSet::from([marker_name]),
    )?;
    if marker_rows.len() > 1 {
        bail!("selected extension has duplicate dbStruFinal rows");
    }
    let snapshot = ExtensionRegistrySnapshot {
        extension_id: extension.physical_registry_id,
        version: extension.registry_version,
        zipped_info: extension.zipped_info.clone(),
    };
    let plan = crate::mssql_extension_activation::prepare_extension_activation(
        crate::mssql_extension_activation::ExtensionActivationMode::Exclusive,
        snapshot,
        &stage,
        crate::mssql_extension_activation::ExtensionServiceMarkerSnapshot {
            present: marker_rows.len() == 1,
        },
        args.allow_non_lab,
    )?;
    let rendered =
        crate::mssql_extension_activation::render_extension_activation_sql(&args.database, &plan)?;
    let artifact_root = std::env::temp_dir().join("ibcmd-rs");
    fs::create_dir_all(&artifact_root)?;
    let root_token = &rendered.report.published_root_sha1[..16];
    let script = args.script_output.clone().unwrap_or_else(|| {
        artifact_root.join(format!("activate_extension_{namespace}_{root_token}.sql"))
    });
    let recovery = args.recovery_output.clone().unwrap_or_else(|| {
        artifact_root.join(format!("recovery_extension_{namespace}_{root_token}.json"))
    });
    write_activation_artifact(&script, rendered.sql().as_bytes())?;
    write_activation_artifact(&recovery, &serde_json::to_vec_pretty(&rendered.recovery)?)?;
    if !args.dry_run && !rendered.sql().is_empty() {
        run_activation_sqlcmd(args, password.as_deref(), &script)?;
    }
    Ok(MssqlExtensionActivationReport {
        database: args.database.clone(),
        extension: args.extension.clone(),
        dry_run: args.dry_run,
        executed: !args.dry_run && !rendered.sql().is_empty(),
        activation: rendered.report,
        script,
        recovery,
    })
}

fn write_activation_artifact(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::read(path) {
        Ok(existing) if existing == bytes => Ok(()),
        Ok(_) => bail!(
            "refusing to overwrite activation artifact {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
        }
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn run_activation_sqlcmd(
    args: &MssqlActivateStagedExtensionArgs,
    password: Option<&str>,
    script: &Path,
) -> Result<()> {
    let mut command = Command::new(&args.sqlcmd);
    command.arg("-S").arg(&args.server);
    if let Some(user) = args.sql_user.as_deref() {
        command.arg("-U").arg(user);
        if let Some(password) = password {
            command.arg("-P").arg(password);
        }
    } else {
        command.arg("-E");
    }
    if args.sqlcmd_trust_cert {
        command.arg("-C");
    }
    let output = command
        .arg("-x")
        .arg("-f")
        .arg("65001")
        .arg("-b")
        .arg("-r")
        .arg("1")
        .arg("-i")
        .arg(script)
        .output()
        .with_context(|| format!("failed to launch {}", args.sqlcmd.display()))?;
    if !output.status.success() {
        bail!(
            "extension activation sqlcmd failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn overlay_stage_plan(
    extension_id: [u8; 16],
    identity: &ConfigInfoIdentity,
    active: &StorageImage,
    patch: &StoragePatch,
    retained_metadata: &std::collections::BTreeSet<String>,
) -> Result<(ExtensionStagePlan, usize, usize)> {
    let mut rows = active
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.logical_key().as_str().to_owned(),
                ExtensionStageRow::new(
                    entry.logical_key().as_str(),
                    entry.packed_payload().to_vec(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut compiled = 0;
    let mut retained = 0;
    for entry in patch.entries() {
        if entry.target().multipart() != MultipartIdentity::single() {
            bail!(
                "compiled target {:?} is multipart; extension staging supports single rows only",
                entry.target().key().as_str()
            );
        }
        let target = entry.target().key().as_str();
        if target == identity.configuration_id.hyphenated().to_string()
            || matches!(target, "root" | "version" | "versions")
            || retained_metadata.contains(target)
            || Uuid::parse_str(target).is_ok()
        {
            retained += 1;
            continue;
        }
        match entry.outcome() {
            StoragePatchOutcome::Compiled(payload) => {
                rows.insert(
                    target.to_owned(),
                    ExtensionStageRow::new(target, deflate_raw(payload.bytes())?),
                );
                compiled += 1;
            }
            StoragePatchOutcome::NeedsBase { required, .. } => {
                let base = rows.get(required.as_str()).cloned().ok_or_else(|| {
                    anyhow!(
                        "compiled target {target:?} requires missing active row {:?}",
                        required.as_str()
                    )
                })?;
                rows.entry(target.to_owned()).or_insert(base);
                retained += 1;
            }
            StoragePatchOutcome::Unsupported { reason } => {
                bail!(
                    "compiled target {target:?} is unsupported: {}",
                    reason.as_str()
                );
            }
        }
    }
    let rows = rows.into_values().collect::<Vec<_>>();
    let plan = prepare_extension_stage(extension_id, identity, rows)?;
    Ok((plan, compiled, retained))
}

fn sanitize_extension_tree(
    tree: &SourceTree,
) -> Result<(SourceTree, std::collections::BTreeSet<String>)> {
    const OPEN: &str = "<ObjectBelonging>";
    const CLOSE: &str = "</ObjectBelonging>";
    let mut retained = std::collections::BTreeSet::new();
    let mut entries = Vec::with_capacity(tree.entries().len());
    for entry in tree.entries() {
        let mut bytes = entry.bytes().to_vec();
        if entry.path().as_str().to_ascii_lowercase().ends_with(".xml") {
            let text = std::str::from_utf8(&bytes)
                .with_context(|| format!("extension XML {} is not UTF-8", entry.path().as_str()))?;
            if let Some(start) = text.find(OPEN) {
                let value_start = start + OPEN.len();
                let relative_end = text[value_start..].find(CLOSE).ok_or_else(|| {
                    anyhow!("unterminated ObjectBelonging in {}", entry.path().as_str())
                })?;
                let end = value_start + relative_end + CLOSE.len();
                if let Some(uuid) = entry.uuid() {
                    retained.insert(uuid.to_string());
                }
                let mut sanitized = String::with_capacity(text.len());
                sanitized.push_str(&text[..start]);
                sanitized.push_str(&text[end..]);
                let sanitized = remove_xml_element(&sanitized, "InternalInfo")?;
                bytes = sanitized.into_bytes();
            }
        }
        entries.push(SourceEntry::from_bytes(entry.path().clone(), bytes)?);
    }
    Ok((SourceTree::new(entries)?, retained))
}

fn remove_xml_element(text: &str, local_name: &str) -> Result<String> {
    let empty = format!("<{local_name}/>");
    if let Some(start) = text.find(&empty) {
        let mut output = text.to_owned();
        output.replace_range(start..start + empty.len(), "");
        return Ok(output);
    }
    let open = format!("<{local_name}>");
    let close = format!("</{local_name}>");
    let Some(start) = text.find(&open) else {
        return Ok(text.to_owned());
    };
    let relative_end = text[start + open.len()..]
        .find(&close)
        .ok_or_else(|| anyhow!("unterminated {local_name} element"))?;
    let end = start + open.len() + relative_end + close.len();
    let mut output = String::with_capacity(text.len());
    output.push_str(&text[..start]);
    output.push_str(&text[end..]);
    Ok(output)
}

fn deflate_raw(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes)?;
    Ok(encoder.finish()?)
}

fn select_extensions<'a>(
    extensions: &'a [MssqlExtensionInfo],
    requested: Option<&str>,
    all: bool,
) -> Result<Vec<&'a MssqlExtensionInfo>> {
    match (requested, all) {
        (Some(_), true) | (None, false) => {
            bail!("exactly one of --extension or --all-extensions must be specified")
        }
        (None, true) => Ok(extensions.iter().collect()),
        (Some(name), false) => {
            let matches = extensions
                .iter()
                .filter(|extension| extension.name == name)
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [extension] => Ok(vec![*extension]),
                [] => bail!("configuration extension {name:?} was not found"),
                _ => bail!("configuration extension name {name:?} is ambiguous"),
            }
        }
    }
}

fn resolve_password(args: &MssqlLoadExtensionArgs) -> Result<Option<String>> {
    if args.sql_user.is_none() {
        return Ok(None);
    }
    args.sql_pwd
        .as_ref()
        .filter(|value| !value.is_empty())
        .cloned()
        .or_else(|| {
            std::env::var(&args.sql_pwd_env)
                .ok()
                .filter(|value| !value.is_empty())
        })
        .map(Some)
        .ok_or_else(|| anyhow!("SQL login requires --sql-pwd or {}", args.sql_pwd_env))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ibcmd_core::artifact::StorageProfileId;
    use ibcmd_core::storage::{
        CompressionKind, OpaqueStorageMetadata, StorageEntry, StorageKey, StorageName,
        StorageOrigin, StoragePayloads, StorageProvenance,
    };

    #[test]
    fn overlay_preserves_unmentioned_active_rows() {
        let origin = StorageOrigin::new(
            StorageProfileId::parse("storage:test").unwrap(),
            StorageProvenance::new("test").unwrap(),
        );
        let packed = deflate_raw(b"old").unwrap();
        let active = StorageImage::new(vec![
            StorageEntry::new(
                StorageName::new("old").unwrap(),
                StorageKey::new("old").unwrap(),
                MultipartIdentity::single(),
                OpaqueStorageMetadata::new(Vec::new(), Vec::new()).unwrap(),
                StoragePayloads::new(packed, b"old".to_vec()).unwrap(),
                CompressionKind::raw_deflate(),
                origin,
            )
            .unwrap(),
        ])
        .unwrap();
        assert_eq!(active.len(), 1);
    }
}
