//! Narrow SDK projections over typed XML sidecars. Markup is never reserialized.
use crate::ConvertError;
use std::collections::BTreeMap;
// Typed source assets are external digest/length references in the public core,
// not inline opaque values. Keep the independent physical source file ceiling.
const MAX_SOURCE_ASSET_BYTES: usize = 256 * 1024 * 1024;

fn error(reason: impl Into<String>) -> ConvertError {
    ConvertError::Read {
        kind: "TemplateBody".into(),
        object: "SDK projection".into(),
        reason: reason.into(),
    }
}

// Quote-aware lexical markup boundaries. Comments/PI are opaque; CDATA and DTD
// are outside the witnessed projection and must not be treated as plain text.
fn markup(body: &[u8]) -> Result<Vec<(usize, usize)>, ConvertError> {
    let mut result = Vec::new();
    let mut i = 0;
    let mut elements = 0usize;
    let mut depth = 0usize;
    while i < body.len() {
        if body[i] != b'<' {
            i += 1;
            continue;
        }
        let start = i;
        if body[i..].starts_with(b"<!--") {
            let end = body[i + 4..]
                .windows(3)
                .position(|w| w == b"-->")
                .ok_or_else(|| error("unterminated comment"))?;
            i += 4 + end + 3;
        } else if body[i..].starts_with(b"<?") {
            let end = body[i + 2..]
                .windows(2)
                .position(|w| w == b"?>")
                .ok_or_else(|| error("unterminated processing instruction"))?;
            i += 2 + end + 2;
        } else {
            if body[i..].starts_with(b"<!") {
                return Err(error(
                    "CDATA/DTD is outside the witnessed SDK sidecar projection",
                ));
            }
            let mut quote = None;
            i += 1;
            while i < body.len() {
                match (quote, body[i]) {
                    (Some(q), b) if q == b => quote = None,
                    (None, b'\'' | b'"') => quote = Some(body[i]),
                    (None, b'>') => {
                        i += 1;
                        break;
                    }
                    _ => {}
                }
                i += 1;
            }
            if i > body.len() || body.get(i.wrapping_sub(1)) != Some(&b'>') {
                return Err(error("unterminated markup"));
            }
        }
        match body.get(start + 1) {
            Some(b'/') => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| error("unbalanced XML end tag"))?
            }
            Some(b'?' | b'!') => {}
            _ => {
                elements += 1;
                if elements > 1_048_576 {
                    return Err(error("SDK sidecar element budget exceeded"));
                }
                if !body[start..i].ends_with(b"/>") {
                    depth += 1;
                    if depth > 64 {
                        return Err(error("SDK sidecar XML depth exceeds 64"));
                    }
                }
            }
        }
        if result.len() == 2_097_152 {
            return Err(error("SDK sidecar markup budget exceeded"));
        }
        result.push((start, i));
    }
    Ok(result)
}

fn parsed(body: &[u8], local: &str, uri: &str) -> Result<formats_xml::Descriptor, ConvertError> {
    if body.len() > MAX_SOURCE_ASSET_BYTES {
        return Err(error("SDK XML source asset exceeds 256 MiB"));
    }
    let _bounded = markup(body)?;
    let doc = formats_xml::parse(body).map_err(|e| error(e.to_string()))?;
    let root = &doc.root;
    let ns = if root.prefix.is_empty() {
        "xmlns".to_string()
    } else {
        format!("xmlns:{}", root.prefix)
    };
    if root.local != local || root.attr(&ns).map(|a| a.value.as_str()) != Some(uri) {
        return Err(error("unexpected typed SDK sidecar root/namespace"));
    }
    Ok(doc)
}

pub(crate) fn text_newlines(body: &[u8], to_crlf: bool) -> Result<Vec<u8>, ConvertError> {
    if body.len() > MAX_SOURCE_ASSET_BYTES {
        return Err(error("SDK XML source asset exceeds 256 MiB"));
    }
    let mut out = Vec::with_capacity(body.len());
    let mut end = 0;
    for (start, next) in markup(body)? {
        let text = &body[end..start];
        if text.iter().any(|b| !b.is_ascii_whitespace()) {
            out.extend_from_slice(&crate::template_read::normalize_newlines(text, to_crlf));
        } else {
            out.extend_from_slice(text);
        }
        out.extend_from_slice(&body[start..next]);
        end = next;
    }
    out.extend_from_slice(&body[end..]);
    Ok(out)
}

pub(crate) fn mxl_newlines(body: &[u8], to_crlf: bool) -> Result<Vec<u8>, ConvertError> {
    if body.len() > MAX_SOURCE_ASSET_BYTES {
        return Err(error("MXL source asset exceeds 256 MiB"));
    }
    let ranges = markup(body)?;
    // Retain only the ancestor scopes, never a DOM of a potentially large MXL.
    let mut stack: Vec<(String, BTreeMap<String, String>, bool)> = Vec::new();
    let mut leaves = Vec::new();
    let mut root_seen = false;
    for &(start, end) in &ranges {
        let token = &body[start..end];
        match token.get(1) {
            Some(b'!' | b'?') => continue,
            Some(b'/') => {
                let name = std::str::from_utf8(&token[2..token.len() - 1])
                    .map_err(|e| error(e.to_string()))?
                    .trim();
                let previous = stack
                    .pop()
                    .ok_or_else(|| error("MXL closing tag has no owner"))?;
                if previous.0 != name {
                    return Err(error("MXL closing tag mismatch"));
                }
                continue;
            }
            _ => {}
        }
        if stack.last().is_some_and(|entry| entry.2) {
            return Err(error("MXL localized content has child markup"));
        }
        let empty = token.ends_with(b"/>");
        let mut single = token.to_vec();
        if !empty {
            single.pop();
            single.extend_from_slice(b"/>");
        }
        // Parse one start tag for correctly unescaped attributes/QNames. Memory
        // depends on this tag and depth64, not total document element count.
        let el = formats_xml::parse(&single)
            .map_err(|e| error(e.to_string()))?
            .root;
        let mut ns = stack
            .last()
            .map(|entry| entry.1.clone())
            .unwrap_or_default();
        for a in &el.attrs {
            if a.name == "xmlns" {
                ns.insert(String::new(), a.value.clone());
            }
            if let Some(prefix) = a.name.strip_prefix("xmlns:") {
                ns.insert(prefix.to_string(), a.value.clone());
            }
            if a.name == "xml:space" {
                ns.insert("xml:space".into(), a.value.clone());
            }
        }
        if stack.is_empty() {
            if root_seen
                || el.local != "document"
                || ns.get(&el.prefix).map(String::as_str)
                    != Some("http://v8.1c.ru/8.2/data/spreadsheet")
            {
                return Err(error("unexpected typed MXL root/namespace"));
            }
            root_seen = true;
        }
        let projected = el.local == "content"
            && ns.get(&el.prefix).map(String::as_str) == Some("http://v8.1c.ru/8.1/data/core");
        if projected && (!el.attrs.is_empty() || ns.contains_key("xml:space")) {
            return Err(error("MXL localized content must be a plain text leaf"));
        }
        leaves.push(if projected {
            Some(Change::Newlines(to_crlf))
        } else {
            None
        });
        if !empty {
            let name = if el.prefix.is_empty() {
                el.local
            } else {
                format!("{}:{}", el.prefix, el.local)
            };
            stack.push((name, ns, projected));
        }
    }
    if !root_seen || !stack.is_empty() {
        return Err(error("incomplete MXL root"));
    }
    replace_leaves(body, leaves)
}

enum Change {
    Alias(String, String),
    Newlines(bool),
}

/// EDT's current-config AnyRef spelling has the existing canonical/Designer
/// alias AnyIBRef. Resolve both the element and text QNames; never touch a query,
/// another namespace, another type host or any native-source body on read.
pub(crate) fn dcs_alias(body: &[u8], to_edt: bool) -> Result<Vec<u8>, ConvertError> {
    let doc = parsed(
        body,
        "DataCompositionSchema",
        "http://v8.1c.ru/8.1/data-composition-system/schema",
    )?;
    if formats_xml::type_codec::canon_for_config_local("AnyIBRef") != Some("AnyRef") {
        return Err(error("DCS projection alias is absent from the type codec registry"));
    }
    let mut leaves = Vec::new();
    fn walk(
        el: &formats_xml::Element,
        inherited: &BTreeMap<String, String>,
        depth: usize,
        to_edt: bool,
        leaves: &mut Vec<Option<Change>>,
    ) -> Result<(), ConvertError> {
        if depth > 64 {
            return Err(error("SDK sidecar XML depth exceeds 64"));
        }
        let mut ns = inherited.clone();
        for a in &el.attrs {
            if let Some(prefix) = a.name.strip_prefix("xmlns:") {
                ns.insert(prefix.to_string(), a.value.clone());
            }
        }
        let mut replacement = None;
        let from = if to_edt { "AnyIBRef" } else { "AnyRef" };
        let to = if to_edt { "AnyRef" } else { "AnyIBRef" };
        if el.local == "TypeSet"
            && ns.get(&el.prefix).map(String::as_str) == Some("http://v8.1c.ru/8.1/data/core")
        {
            if let Some((prefix, name)) = el.text.split_once(':') {
                if name == from
                    && ns.get(prefix).map(String::as_str)
                        == Some(formats_xml::type_codec::CURRENT_CONFIG_NS)
                {
                    if !el.children.is_empty()
                        || el.attrs.len() != 1
                        || el.attrs[0].name != format!("xmlns:{prefix}")
                    {
                        return Err(error(
                            "AnyRef TypeSet must be an exact inline-namespace leaf",
                        ));
                    }
                    replacement = Some(Change::Alias(el.text.clone(), format!("{prefix}:{to}")));
                }
            }
        }
        leaves.push(replacement);
        for child in &el.children {
            walk(child, &ns, depth + 1, to_edt, leaves)?;
        }
        Ok(())
    }
    walk(&doc.root, &BTreeMap::new(), 0, to_edt, &mut leaves)?;
    replace_leaves(body, leaves)
}

fn replace_leaves(body: &[u8], leaves: Vec<Option<Change>>) -> Result<Vec<u8>, ConvertError> {
    let ranges = markup(body)?;
    let mut index = 0;
    let mut replacements = Vec::new();
    for &(start, end) in &ranges {
        if matches!(body.get(start + 1), Some(b'/' | b'?' | b'!')) {
            continue;
        }
        let candidate = leaves
            .get(index)
            .ok_or_else(|| error("XML/lexical element inventory mismatch"))?;
        index += 1;
        if let Some(change) = candidate {
            if body[start..end].ends_with(b"/>") {
                continue;
            }
            let text_end = body[end..]
                .iter()
                .position(|b| *b == b'<')
                .map(|n| end + n)
                .ok_or_else(|| error("TypeSet closing tag missing"))?;
            if !body[text_end..].starts_with(b"</") {
                return Err(error("projected leaf has non-text lexical content"));
            }
            let new = match change {
                Change::Alias(old, new) => {
                    if &body[end..text_end] != old.as_bytes() {
                        return Err(error("TypeSet QName has unwitnessed lexical content"));
                    }
                    new.as_bytes().to_vec()
                }
                Change::Newlines(to_crlf) => {
                    crate::template_read::normalize_newlines(&body[end..text_end], *to_crlf)
                }
            };
            replacements.push((end, text_end, new));
        }
    }
    if index != leaves.len() {
        return Err(error("XML/lexical element inventory mismatch"));
    }
    let mut out = Vec::with_capacity(body.len());
    let mut end = 0;
    for (start, next, new) in replacements {
        out.extend_from_slice(&body[end..start]);
        out.extend_from_slice(&new);
        end = next;
    }
    out.extend_from_slice(&body[end..]);
    Ok(out)
}
