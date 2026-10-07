//! Exact opaque resources of the root ParentConfigurations attachment.
//! The original SDK copies this complete sibling directory without inspecting
//! resource bytes. Relative identities and current bytes remain canonical.
use crate::ConvertError;
use formats_xml::registry::Format;
use morph1c_core::ir::{MetadataObject, ParentConfigurationResource};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn error(path: &Path, reason: impl ToString) -> ConvertError {
    ConvertError::Io {
        path: path.display().to_string(),
        reason: reason.to_string(),
    }
}
fn regular_directory(path: &Path) -> Result<(), ConvertError> {
    for ancestor in path.ancestors().filter(|p| !p.as_os_str().is_empty()) {
        let m = std::fs::symlink_metadata(ancestor).map_err(|e| error(ancestor, e))?;
        if !m.is_dir() || m.file_type().is_symlink() {
            return Err(error(
                ancestor,
                "parent configuration resource directory is not ordinary",
            ));
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if m.file_attributes() & 0x400 != 0 {
                return Err(error(
                    ancestor,
                    "parent configuration resource directory is a reparse point",
                ));
            }
        }
    }
    Ok(())
}
fn relative(path: &str) -> Result<(), String> {
    if path.is_empty() || path.contains('\\') || path.contains(':') {
        return Err("parent configuration resource requires a safe relative path".into());
    }
    for part in path.split('/') {
        if part.is_empty()
            || matches!(part, "." | "..")
            || part.ends_with(['.', ' '])
            || part
                .chars()
                .any(|c| c.is_control() || "<>:\"\\|?*".contains(c))
        {
            return Err("unsafe parent configuration resource component".into());
        }
        let stem = part.split('.').next().unwrap().to_ascii_uppercase();
        if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || matches!(
                stem.strip_prefix("COM"),
                Some("1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            )
            || matches!(
                stem.strip_prefix("LPT"),
                Some("1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            )
        {
            return Err("reserved parent configuration resource component".into());
        }
    }
    Ok(())
}
fn validate_paths<'a>(paths: impl Iterator<Item = &'a str>) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for path in paths {
        relative(path)?;
        if !seen.insert(path.to_lowercase()) {
            return Err("duplicate/case-aliased parent configuration resource".into());
        }
    }
    for path in &seen {
        for (index, _) in path.match_indices('/') {
            if seen.contains(&path[..index]) {
                return Err("parent configuration resource file/directory conflict".into());
            }
        }
    }
    Ok(())
}
fn validate_owner(obj: &MetadataObject, has_resources: bool) -> Result<(), String> {
    if has_resources
        && (obj.kind.as_str() != "Configuration"
            || obj
                .config_blobs
                .iter()
                .filter(|b| b.slot == "ParentConfigurations")
                .count()
                != 1)
    {
        return Err(
            "parent configuration resources require their unique current root binary attachment"
                .into(),
        );
    }
    Ok(())
}

pub(crate) fn attach(
    format: Format,
    ext_dir: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if format == Format::Cf {
        return Ok(());
    }
    let root = ext_dir.join("ParentConfigurations");
    match std::fs::symlink_metadata(&root) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(error(&root, e)),
        Ok(_) => regular_directory(&root)?,
    }
    let mut pending = vec![root.clone()];
    let mut paths = Vec::new();
    while let Some(dir) = pending.pop() {
        regular_directory(&dir)?;
        for entry in std::fs::read_dir(&dir).map_err(|e| error(&dir, e))? {
            let entry = entry.map_err(|e| error(&dir, e))?;
            let path = entry.path();
            let m = std::fs::symlink_metadata(&path).map_err(|e| error(&path, e))?;
            let rel = path
                .strip_prefix(&root)
                .map_err(|e| error(&path, e))?
                .components()
                .map(|component| {
                    if let std::path::Component::Normal(name) = component {
                        name.to_str()
                            .ok_or_else(|| error(&path, "resource path is not UTF-8"))
                    } else {
                        Err(error(&path, "resource path is not relative"))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?
                .join("/");
            relative(&rel).map_err(|e| error(&path, e))?;
            if m.file_type().is_symlink() {
                return Err(error(&path, "resource link is not allowed"));
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if m.file_attributes() & 0x400 != 0 {
                    return Err(error(&path, "resource reparse point is not allowed"));
                }
            }
            if m.is_dir() {
                pending.push(path);
            } else if m.is_file() {
                paths.push((rel, path));
            } else {
                return Err(error(&path, "resource is not a regular file/directory"));
            }
        }
    }
    paths.sort_by(|a, b| a.0.cmp(&b.0));
    validate_paths(paths.iter().map(|p| p.0.as_str())).map_err(|e| error(&root, e))?;
    validate_owner(obj, !paths.is_empty()).map_err(|e| error(&root, e))?;
    let resources = paths
        .into_iter()
        .map(|(path, physical)| {
            crate::form_read::read_regular_source(&physical)
                .map(|bytes| ParentConfigurationResource { path, bytes })
                .map_err(|e| error(&physical, e))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !obj.parent_configuration_resources.is_empty() {
        return Err(error(&root, "duplicate parent resource attachment"));
    }
    obj.parent_configuration_resources = resources;
    Ok(())
}

pub(crate) fn write(
    format: Format,
    ext_dir: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if format == Format::Cf {
        return Ok(());
    }
    let root = ext_dir.join("ParentConfigurations");
    validate_owner(obj, !obj.parent_configuration_resources.is_empty())
        .map_err(|e| error(&root, e))?;
    validate_paths(
        obj.parent_configuration_resources
            .iter()
            .map(|r| r.path.as_str()),
    )
    .map_err(|e| error(&root, e))?;
    // Validate the entire model before creating any resource output.
    for resource in &obj.parent_configuration_resources {
        let path = root.join(&resource.path);
        let mut missing: Vec<PathBuf> = Vec::new();
        for ancestor in path
            .parent()
            .unwrap()
            .ancestors()
            .filter(|p| !p.as_os_str().is_empty())
        {
            match std::fs::symlink_metadata(ancestor) {
                Ok(_) => {
                    regular_directory(ancestor)?;
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    missing.push(ancestor.to_path_buf())
                }
                Err(e) => return Err(error(ancestor, e)),
            }
        }
        if std::fs::symlink_metadata(&path).is_ok() {
            return Err(error(&path, "parent resource output already exists"));
        }
        for dir in missing.iter().rev() {
            crate::fsio::create_dir_all(dir).map_err(|e| error(dir, e))?;
            regular_directory(dir)?;
        }
        crate::form_write::write_file(&path, &resource.bytes)?;
    }
    Ok(())
}
