//! Attach and emit the complete current additional-index sidecar of its owner.
use crate::ConvertError;
use formats_xml::Format;
use morph1c_core::ir::MetadataObject;
use std::path::{Path, PathBuf};

fn sidecar(format: Format, descriptor: &Path) -> Option<PathBuf> {
    match format {
        Format::Edt => Some(descriptor.parent()?.join("AdditionalIndexes.aindex")),
        Format::Designer => Some(
            descriptor
                .parent()?
                .join(descriptor.file_stem()?)
                .join("Ext/AdditionalIndexes.xml"),
        ),
        Format::Cf => None,
    }
}
fn validate_owner(obj: &MetadataObject) -> Result<(), String> {
    let owner = format!("{}.{}", obj.kind.as_str(), obj.name);
    let prefix = format!("{owner}.");
    for index in obj.additional_indexes.as_deref().unwrap_or_default() {
        if index.table == owner {
            continue;
        }
        if index.table.strip_prefix(&prefix).is_some_and(|table| {
            obj.children.iter().any(|child| {
                child.kind.as_str().ends_with(".TabularSection") && child.name == table
            })
        }) {
            continue;
        }
        return Err(
            "additional-index table is not the current owner or its declared tabular section"
                .into(),
        );
    }
    Ok(())
}
pub fn attach(
    format: Format,
    descriptor: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    let Some(path) = sidecar(format, descriptor) else {
        return Ok(());
    };
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(ConvertError::Io {
                path: path.display().to_string(),
                reason: error.to_string(),
            })
        }
        Ok(_) => (),
    }
    if obj.additional_indexes.is_some() {
        return Err(ConvertError::Read {
            kind: obj.kind.as_str().into(),
            object: obj.name.clone(),
            reason: "additional-index attachment is already populated".into(),
        });
    }
    let bytes = crate::form_read::read_regular_source(&path).map_err(|error| ConvertError::Io {
        path: path.display().to_string(),
        reason: error.to_string(),
    })?;
    let indexes = formats_xml::additional_indexes::read(&bytes, format).map_err(|reason| {
        ConvertError::Read {
            kind: obj.kind.as_str().into(),
            object: obj.name.clone(),
            reason,
        }
    })?;
    obj.additional_indexes = Some(indexes);
    validate_owner(obj).map_err(|reason| ConvertError::Read {
        kind: obj.kind.as_str().into(),
        object: obj.name.clone(),
        reason,
    })
}
pub fn write(format: Format, descriptor: &Path, obj: &MetadataObject) -> Result<(), ConvertError> {
    let Some(indexes) = &obj.additional_indexes else {
        return Ok(());
    };
    let fail = |reason| ConvertError::Write {
        kind: obj.kind.as_str().into(),
        object: obj.name.clone(),
        reason,
    };
    validate_owner(obj).map_err(fail)?;
    let path = sidecar(format, descriptor)
        .ok_or_else(|| fail("additional-index sidecar layout is unavailable".into()))?;
    let bytes = formats_xml::additional_indexes::write(indexes, format).map_err(fail)?;
    crate::form_write::write_file(&path, &bytes)
}
