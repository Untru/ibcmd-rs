//! Legacy report attribute identifiers resolve against the exact declared
//! top-level FormAttribute inventory. Source spelling is independent of the
//! canonical attribute name and never wins over an edited reference.
use super::FormError;
use morph1c_core::ir::FormDataAttribute;

pub(crate) fn canonical(
    identifier: &str,
    attributes: &[FormDataAttribute],
) -> Result<String, FormError> {
    if identifier.is_empty()
        || identifier.len() > 19
        || !identifier.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(FormError::Frame(
            "report attribute identifier must be a bounded unsigned integer".into(),
        ));
    }
    let id = identifier
        .parse::<i64>()
        .map_err(|_| FormError::Frame("report attribute identifier overflow".into()))?;
    if id == 0 {
        return Ok("0".into());
    }
    let matches: Vec<_> = attributes.iter().filter(|a| a.id == id).collect();
    match matches.as_slice() {
        [attribute] => Ok(attribute.name.clone()),
        [] => Err(FormError::Frame(format!(
            "report attribute identifier {identifier} has no declared target"
        ))),
        _ => Err(FormError::Frame(format!(
            "report attribute identifier {identifier} has duplicate declared targets"
        ))),
    }
}

pub(crate) fn source_identifier(
    canonical_name: &str,
    spelling: Option<&(String, String)>,
    attributes: &[FormDataAttribute],
) -> Result<Option<String>, FormError> {
    let Some((original_name, identifier)) = spelling else {
        return Ok(None);
    };
    if canonical_name != "0"
        && attributes
            .iter()
            .filter(|a| a.name == canonical_name)
            .count()
            != 1
    {
        return Err(FormError::Frame(format!(
            "report reference {canonical_name:?} has no unique declared target"
        )));
    }
    if original_name == canonical_name
        && canonical(identifier, attributes).ok().as_deref() == Some(canonical_name)
    {
        Ok(Some(identifier.clone()))
    } else {
        Ok(None)
    }
}
