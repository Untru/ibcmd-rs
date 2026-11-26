//! Ordered EDT Help.pages language identifiers, shared with owned sidecar accounting.
use crate::{Element, emit::OutElement};
use morph1c_core::ir::PropertyValue;
use std::collections::BTreeSet;

/// A language is used as one filesystem component; content is otherwise retained exactly.
pub fn valid_language(lang: &str) -> bool {
    !lang.is_empty()
        && lang != "."
        && lang != ".."
        && !lang.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
        && !lang.ends_with(['.', ' '])
}

pub fn languages(value: &PropertyValue) -> Result<Vec<&str>, String> {
    let PropertyValue::List(values) = value else {
        return Err("help expects an ordered language list".into());
    };
    let mut seen = BTreeSet::new();
    values
        .iter()
        .map(|v| {
            let PropertyValue::Str(lang) = v else {
                return Err("help language is not a string".into());
            };
            if !valid_language(lang) || !seen.insert(lang.as_str()) {
                return Err("unsafe or duplicate help language".into());
            }
            Ok(lang.as_str())
        })
        .collect()
}

pub fn decode(host: &Element) -> Result<PropertyValue, String> {
    if !host.attrs.is_empty() || !host.text.is_empty() {
        return Err("help contains attributes or mixed text".into());
    }
    let mut values = Vec::new();
    for page in &host.children {
        if page.local != "pages"
            || !page.prefix.is_empty()
            || !page.attrs.is_empty()
            || !page.text.is_empty()
            || page.children.len() != 1
        {
            return Err("help expects repeated unqualified pages with one lang".into());
        }
        let lang = &page.children[0];
        if lang.local != "lang"
            || !lang.prefix.is_empty()
            || !lang.attrs.is_empty()
            || !lang.children.is_empty()
        {
            return Err("help pages expects one plain unqualified lang".into());
        }
        page.claim();
        lang.claim_with_text();
        values.push(PropertyValue::Str(lang.text.clone()));
    }
    let value = PropertyValue::List(values);
    languages(&value)?;
    Ok(value)
}

pub fn emit(ns: &str, tag: &str, value: &PropertyValue) -> Result<Option<OutElement>, String> {
    let langs = languages(value)?;
    if langs.is_empty() {
        return Ok(None);
    }
    let mut help = OutElement::branch(ns, tag);
    for lang in langs {
        let mut page = OutElement::branch("", "pages");
        page.push(OutElement::leaf("", "lang", lang));
        help.push(page);
    }
    Ok(Some(help))
}
