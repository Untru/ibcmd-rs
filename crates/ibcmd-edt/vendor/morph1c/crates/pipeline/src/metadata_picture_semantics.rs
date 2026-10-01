//! IO bridge for the closed metadata PictureRef semantic resource.
use crate::{ConvertError, Format};
use formats_xml::metadata_picture_semantics as carrier;
use morph1c_core::ir::MetadataObject;
use std::path::Path;

pub(crate) fn attach(
    format: Format,
    path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if format != Format::Edt {
        return Ok(());
    }
    let resource = path
        .parent()
        .expect("descriptor parent")
        .join(carrier::RESOURCE);
    if !resource.exists() {
        return Ok(());
    }
    let bytes = std::fs::read(&resource).map_err(|e| ConvertError::Io {
        path: resource.display().to_string(),
        reason: e.to_string(),
    })?;
    carrier::apply(obj, &bytes).map_err(|reason| ConvertError::Read {
        kind: obj.kind.as_str().into(),
        object: obj.name.clone(),
        reason,
    })
}
pub(crate) fn emit(path: &Path, bytes: Option<&[u8]>) -> Result<(), ConvertError> {
    if let Some(bytes) = bytes {
        let resource = path
            .parent()
            .expect("descriptor parent")
            .join(carrier::RESOURCE);
        crate::fsio::write(&resource, bytes).map_err(|e| ConvertError::Io {
            path: resource.display().to_string(),
            reason: e.to_string(),
        })?;
    }
    Ok(())
}
