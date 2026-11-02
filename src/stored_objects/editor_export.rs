//! Prepare an editor export in a separate empty staging directory. Publication
//! belongs to the client, after Git and unsaved-buffer checks.
use super::*;
use ibcmd_xml::config_dump_info::ConfigDumpInfo;
use std::collections::BTreeSet;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorExportPlan {
    pub objects: Vec<String>,
    pub files: Vec<String>,
    pub removed: Vec<String>,
    pub expected: BTreeMap<String, Option<String>>,
}

impl StoredConfiguration {
    pub fn prepare_editor_export(
        &mut self,
        selectors: &[String],
        changed: bool,
        source: &Path,
        stage: &Path,
        progress: Progress<'_>,
    ) -> Result<EditorExportPlan> {
        if stage == source || !stage.is_dir() || fs::read_dir(stage)?.next().is_some() {
            bail!("editor export requires a separate empty staging directory");
        }
        let source_real = fs::canonicalize(source)?;
        let stage_real = fs::canonicalize(stage)?;
        if source_real.starts_with(&stage_real) || stage_real.starts_with(&source_real) {
            bail!("export staging and source directories must not overlap");
        }
        let view = self.main_view_mut();
        progress(0, 4, "editor export: read inventory");
        let read_scope = match &view.source {
            ConfigSource::FileDb(path) if view.rows_snapshot.is_none() => {
                let _engine = engine();
                Some(crate::mssql_dump::FileDbReadScope::begin(path)?)
            }
            _ => None,
        };
        let before = view.native_config_dump_info()?;
        progress(1, 4, "editor export: resolve selection");
        let current = ConfigDumpInfo::parse(&before)?;
        let snapshot = crate::native_export_status::ConfigDumpInfoSnapshot::parse(&before)?;
        let owners = snapshot.owners();
        if !owners.unmapped_ids.is_empty() {
            bail!("native export inventory contains unmapped rows");
        }
        let reference_path = source.join("ConfigDumpInfo.xml");
        let mut reference_bytes = None;
        let previous = match fs::symlink_metadata(&reference_path) {
            Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => {
                let bytes = fs::read(&reference_path)?;
                let parsed = ConfigDumpInfo::parse(&bytes)?;
                reference_bytes = Some(bytes);
                Some(parsed)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            _ => bail!("ConfigDumpInfo reference must be a regular file"),
        };
        let mut names = BTreeSet::new();
        let mut old_names = BTreeSet::new();
        if changed {
            let previous = previous
                .as_ref()
                .context("Export changed requires ConfigDumpInfo.xml")?;
            let old = crate::native_export_status::ConfigDumpInfoSnapshot::parse(
                &previous.to_native_bytes()?,
            )?;
            let status = snapshot.compare(&old);
            if status.full_required {
                // Native --sync falls back to a full conversion on removals/renames.
                {
                    let _engine = engine();
                    names.extend(view.all_objects()?.into_iter().map(|o| o.full_name));
                }
                old_names.extend(old.owners().by_id.values().map(|o| o.root.clone()));
            } else {
                for row in status.changes {
                    let owner = owners
                        .by_id
                        .get(&row.id)
                        .context("Changed row has no owner")?;
                    names.insert(owner.root.clone());
                }
            }
        } else {
            for selector in selectors {
                if selector.contains('/') {
                    let mut pending = vec![selector.clone()];
                    let mut visited = BTreeSet::new();
                    while let Some(node) = pending.pop() {
                        if !visited.insert(node.clone()) {
                            bail!("cyclic export selection");
                        }
                        for child in view.children(Some(&node))? {
                            if let Some(name) = child.full_name {
                                names.insert(name);
                            } else if child.has_children {
                                pending.push(child.id);
                            }
                        }
                    }
                } else {
                    names.extend(
                        view.resolve(std::slice::from_ref(selector))?
                            .into_iter()
                            .map(|object| object.full_name),
                    );
                }
            }
            if names.is_empty() {
                bail!("No objects selected");
            }
        }
        let selected = |owner: &str| {
            names
                .iter()
                .any(|n| owner == n || owner.starts_with(&format!("{n}.")))
        };
        let mut info = current.clone();
        if !changed {
            // Keep unsynchronized objects' versions, including their nested parts.
            info.entries.retain(|entry| {
                owners
                    .by_id
                    .get(&entry.id)
                    .is_some_and(|o| selected(&o.object))
            });
            if let Some(previous) = previous {
                let old_bytes = previous.to_native_bytes()?;
                let old_owners =
                    crate::native_export_status::ConfigDumpInfoSnapshot::parse(&old_bytes)?
                        .owners();
                info.entries
                    .extend(previous.entries.into_iter().filter(|entry| {
                        !old_owners
                            .by_id
                            .get(&entry.id)
                            .is_some_and(|o| selected(&o.object))
                            && !info_id_selected(&owners, &entry.id, &names)
                    }));
                let order = current
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(i, e)| (e.id.clone(), i))
                    .collect::<BTreeMap<_, _>>();
                info.entries
                    .sort_by_key(|entry| order.get(&entry.id).copied().unwrap_or(usize::MAX));
            }
        }
        let mut files = BTreeSet::new();
        let mut removed = BTreeSet::new();
        // Capture only selected scopes before conversion, so a one-object export
        // never reads the whole ERP source tree.
        let mut original = BTreeMap::new();
        for object in view.resolve(&names.iter().cloned().collect::<Vec<_>>())? {
            for (name, path) in local_files(source, &object)? {
                original.insert(name, sha256_hex(&fs::read(path)?));
            }
        }
        for name in old_names.difference(&names) {
            if let Some(prefixes) = source_prefixes(name) {
                for prefix in prefixes {
                    let descriptor = format!("{prefix}.xml");
                    if source.join(&descriptor).is_file() {
                        original.insert(
                            descriptor.clone(),
                            sha256_hex(&fs::read(source.join(descriptor))?),
                        );
                    }
                    let directory = source.join(&prefix);
                    if directory.is_dir() {
                        for (name, file) in collect_editor_files(source, &directory)? {
                            original.insert(name, sha256_hex(&fs::read(file)?));
                        }
                    }
                }
            }
        }
        if let Some(bytes) = reference_bytes {
            original.insert("ConfigDumpInfo.xml".into(), sha256_hex(&bytes));
        }
        progress(2, 4, "editor export: convert selection");
        let exported = {
            let _engine = engine();
            view.export_checked_unlocked(
                &names.iter().cloned().collect::<Vec<_>>(),
                progress,
                true,
            )?
        };
        for exported in exported {
            for name in local_files(source, &exported.object)?.keys() {
                if !exported.files.contains_key(name) {
                    removed.insert(name.clone());
                }
            }
            for (name, bytes) in &exported.files {
                let target = stage.join(name);
                fs::create_dir_all(target.parent().context("missing stage parent")?)?;
                fs::write(target, filesystem_expected_bytes(name, bytes).as_ref())?;
                files.insert(name.clone());
            }
        }
        for name in old_names.difference(&names) {
            // Resolve old canonical paths without requiring a deleted DB object.
            if let Some(prefixes) = source_prefixes(name) {
                for prefix in prefixes {
                    let descriptor = format!("{prefix}.xml");
                    if source.join(&descriptor).is_file() {
                        removed.insert(descriptor);
                    }
                    let p = source.join(&prefix);
                    if p.is_file() {
                        removed.insert(prefix);
                    } else if p.is_dir() {
                        for (relative, _) in collect_editor_files(source, &p)? {
                            removed.insert(relative);
                        }
                    }
                }
            }
        }
        removed.retain(|name| !files.contains(name));
        // Keep final validation independent of the operation's immutable input.
        drop(read_scope);
        progress(3, 4, "editor export: verify inventory");
        if view.native_config_dump_info()? != before {
            bail!("Database changed during export; retry");
        }
        if !names.is_empty() || !removed.is_empty() {
            fs::write(stage.join("ConfigDumpInfo.xml"), info.to_native_bytes()?)?;
            files.insert("ConfigDumpInfo.xml".into());
        }
        let expected = files
            .iter()
            .chain(removed.iter())
            .map(|name| (name.clone(), original.get(name).cloned()))
            .collect();
        Ok(EditorExportPlan {
            objects: names.into_iter().collect(),
            files: files.into_iter().collect(),
            removed: removed.into_iter().collect(),
            expected,
        })
    }
}
fn info_id_selected(
    owners: &crate::native_export_status::NativeVersionOwners,
    id: &str,
    names: &BTreeSet<String>,
) -> bool {
    owners.by_id.get(id).is_some_and(|o| {
        names
            .iter()
            .any(|n| o.object == *n || o.object.starts_with(&format!("{n}.")))
    })
}
fn collect_editor_files(root: &Path, directory: &Path) -> Result<BTreeMap<String, PathBuf>> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            bail!("linked source path");
        }
        if meta.is_dir() {
            files.extend(collect_editor_files(root, &path)?);
        } else if meta.is_file() {
            files.insert(
                path.strip_prefix(root)?
                    .to_string_lossy()
                    .replace('\\', "/"),
                path,
            );
        }
    }
    Ok(files)
}

#[cfg(test)]
#[path = "editor_export_scope_tests.rs"]
mod read_scope_tests;
