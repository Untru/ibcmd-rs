//! Shared typed CURRENT DataPath grammar. Source marker/layout restoration and
//! transport for noninjective projections are separate from these values.
use super::FormError;
use crate::{descriptor::Element, emit::OutElement};
use morph1c_core::ir::{form::DataPathSpec, PropertyValue};

pub(super) fn canonical(path: DataPathSpec) -> PropertyValue {
    let primary = path.primary();
    if path.extra_paths.is_empty()
        && primary
            .split('.')
            .eq(path.segments.iter().map(String::as_str))
    {
        PropertyValue::Ref(primary)
    } else {
        PropertyValue::DataPath(path)
    }
}

pub(super) fn current(value: &PropertyValue) -> Result<DataPathSpec, FormError> {
    match value {
        // Ref is the existing canonical logical path, not a source spelling.
        PropertyValue::Ref(primary) => Ok(DataPathSpec {
            segments: primary.split('.').map(str::to_owned).collect(),
            extra_paths: Vec::new(),
        }),
        PropertyValue::DataPath(path) => Ok(path.clone()),
        _ => Err(FormError::Frame(
            "DataPath requires a current Ref or DataPathSpec".into(),
        )),
    }
}

pub(super) fn read_edt(el: &Element, tag: &str) -> Result<PropertyValue, FormError> {
    super::fields::claim_xsi(el, tag, "form:DataPath")?;
    el.claim();
    if !el.text.is_empty() {
        return Err(FormError::Frame(format!(
            "<{tag}> DataPath has unexpected text"
        )));
    }
    let mut segments = Vec::new();
    for child in &el.children {
        if !child.prefix.is_empty() || child.local != "segments" {
            return Err(FormError::Frame(format!("<{tag}> unknown DataPath child")));
        }
        if !child.attrs.is_empty() || !child.children.is_empty() {
            return Err(FormError::Frame(format!(
                "<{tag}> segments must be a scalar"
            )));
        }
        child.claim_with_text();
        // FormXmlHelper applies setValue to each authored occurrence in order.
        segments.extend(DataPathSpec::from_form_text(&child.text).segments);
    }
    if el.children.is_empty() {
        return Err(FormError::Frame(format!("<{tag}> no DataPath segments")));
    }
    Ok(canonical(DataPathSpec {
        segments,
        extra_paths: Vec::new(),
    }))
}

pub(super) fn write_edt(tag: &str, value: &PropertyValue) -> Result<OutElement, FormError> {
    let path = current(value)?;
    let mut el = OutElement::branch("", tag).attr("xsi:type", "form:DataPath");
    el.push(OutElement::leaf("", "segments", path.primary()));
    // extraPaths is transient in the original model: typed transport owns it.
    Ok(el)
}

pub(super) fn read_native(text: &str) -> Result<PropertyValue, FormError> {
    if !text.contains('~') {
        return Ok(canonical(DataPathSpec::from_form_text(text)));
    }
    // Original Reader filters empty pieces BEFORE Java String.trim().
    let mut pieces = text
        .split('~')
        .filter(|part| !part.is_empty())
        .map(|part| part.trim_matches(|ch: char| ch <= '\u{20}'));
    let Some(first) = pieces.next() else {
        return Err(FormError::Frame(
            "native DataPath has no logical path".into(),
        ));
    };
    let mut path = DataPathSpec::from_form_text(first);
    path.extra_paths.extend(pieces.map(str::to_owned));
    Ok(canonical(path))
}

/// Standard SDK projection. Transient extras are emitted only on an unresolved
/// primary path; the current transport retains any noninjective remainder.
pub(super) fn write_native(value: &PropertyValue) -> Result<String, FormError> {
    let path = current(value)?;
    let primary = path.primary();
    let mut out = super::availability::rendered_data_path(&primary);
    if super::availability::unavailable(&primary) {
        for extra in &path.extra_paths {
            out.push('~');
            out.push_str(extra);
        }
    }
    Ok(out)
}

pub(super) fn read_native_field(text: &str) -> Result<PropertyValue, FormError> {
    // Original AbstractDataPathReader uses its multilingual branch for a
    // concrete FormField even without a delimiter (Java trim <= U+0020).
    read_native(text.trim_matches(|ch: char| ch <= '\u{20}'))
}
