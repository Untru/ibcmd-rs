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

// A lexical cursor owns no per-element inventory. The current markup window
// and ancestor namespace deltas are the only auxiliary state for large MXL.
struct MarkupCursor<'a> {
    body: &'a [u8],
    offset: usize,
}
impl MarkupCursor<'_> {
    fn next(&mut self) -> Result<Option<(usize, usize)>, ConvertError> {
        let Some(relative) = self.body[self.offset..].iter().position(|b| *b == b'<') else {
            return Ok(None);
        };
        let start = self.offset + relative;
        let tail = &self.body[start..];
        let end = if tail.starts_with(b"<!--") {
            start
                + 4
                + tail[4..]
                    .windows(3)
                    .position(|w| w == b"-->")
                    .ok_or_else(|| error("unterminated comment"))?
                + 3
        } else if tail.starts_with(b"<?") {
            start
                + 2
                + tail[2..]
                    .windows(2)
                    .position(|w| w == b"?>")
                    .ok_or_else(|| error("unterminated processing instruction"))?
                + 2
        } else {
            if tail.starts_with(b"<!") {
                return Err(error(
                    "CDATA/DTD is outside the witnessed SDK sidecar projection",
                ));
            }
            let mut quote = None;
            let mut end = None;
            for (i, &byte) in tail.iter().enumerate().skip(1) {
                match (quote, byte) {
                    (Some(q), b) if q == b => quote = None,
                    (None, b'\'' | b'"') => quote = Some(byte),
                    (None, b'>') => {
                        end = Some(start + i + 1);
                        break;
                    }
                    _ => {}
                }
            }
            end.ok_or_else(|| error("unterminated markup"))?
        };
        self.offset = end;
        Ok(Some((start, end)))
    }
}

fn append_newlines(out: &mut Vec<u8>, text: &[u8], to_crlf: bool) {
    let mut i = 0;
    while i < text.len() {
        match text[i] {
            b'\r' if text.get(i + 1) == Some(&b'\n') => {
                out.extend_from_slice(if to_crlf { b"\r\n" } else { b"\n" });
                i += 2;
            }
            b'\n' => {
                out.extend_from_slice(if to_crlf { b"\r\n" } else { b"\n" });
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
}

/// Project only plain qualified MXL localized-content line endings, preserving
/// all other source bytes. Used by the private adapter's streaming body guard.
#[doc(hidden)]
pub fn mxl_newlines(body: &[u8], to_crlf: bool) -> Result<Vec<u8>, ConvertError> {
    struct Frame {
        name: String,
        namespace_undo: Vec<(String, Option<String>)>,
        projected: bool,
    }
    fn restore(ns: &mut BTreeMap<String, String>, undo: Vec<(String, Option<String>)>) {
        for (key, previous) in undo.into_iter().rev() {
            if let Some(value) = previous {
                ns.insert(key, value);
            } else {
                ns.remove(&key);
            }
        }
    }
    let mut cursor = MarkupCursor { body, offset: 0 };
    let mut stack: Vec<Frame> = Vec::new();
    let mut ns = BTreeMap::new();
    let mut out = Vec::with_capacity(body.len());
    let mut previous_end = 0;
    let mut root_seen = false;
    while let Some((start, end)) = cursor.next()? {
        let text = &body[previous_end..start];
        if stack.last().is_some_and(|frame| frame.projected) {
            append_newlines(&mut out, text, to_crlf);
        } else {
            if stack.is_empty()
                && text.iter().any(|b| !b.is_ascii_whitespace())
                && !(previous_end == 0
                    && text
                        .strip_prefix(b"\xef\xbb\xbf")
                        .is_some_and(|rest| rest.iter().all(u8::is_ascii_whitespace)))
            {
                return Err(error("MXL has text outside its document root"));
            }
            out.extend_from_slice(text);
        }
        let token = &body[start..end];
        match token.get(1) {
            Some(b'!' | b'?') => {
                if stack.last().is_some_and(|frame| frame.projected) {
                    return Err(error("MXL localized content has non-text markup"));
                }
            }
            Some(b'/') => {
                let name = std::str::from_utf8(&token[2..token.len() - 1])
                    .map_err(|e| error(e.to_string()))?
                    .trim();
                let frame = stack
                    .pop()
                    .ok_or_else(|| error("MXL closing tag has no owner"))?;
                if frame.name != name {
                    return Err(error("MXL closing tag mismatch"));
                }
                restore(&mut ns, frame.namespace_undo);
            }
            _ => {
                if stack.last().is_some_and(|frame| frame.projected) {
                    return Err(error("MXL localized content has child markup"));
                }
                let empty = token.ends_with(b"/>");
                let mut single = token.to_vec();
                if !empty {
                    single.pop();
                    single.extend_from_slice(b"/>");
                }
                // Parse only this start tag. No full MXL DOM, count-dependent
                // ranges, leaf inventory, replacements or content copies exist.
                let el = formats_xml::parse(&single)
                    .map_err(|e| error(e.to_string()))?
                    .root;
                let mut undo = Vec::new();
                for a in &el.attrs {
                    let key = if a.name == "xmlns" {
                        Some("")
                    } else if a.name == "xml:space" {
                        Some("xml:space")
                    } else {
                        a.name.strip_prefix("xmlns:")
                    };
                    if let Some(key) = key {
                        undo.push((key.to_owned(), ns.insert(key.to_owned(), a.value.clone())));
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
                    && ns.get(&el.prefix).map(String::as_str)
                        == Some("http://v8.1c.ru/8.1/data/core");
                if projected && (!el.attrs.is_empty() || ns.contains_key("xml:space")) {
                    return Err(error("MXL localized content must be a plain text leaf"));
                }
                if empty {
                    restore(&mut ns, undo);
                } else {
                    stack.push(Frame {
                        name: if el.prefix.is_empty() {
                            el.local
                        } else {
                            format!("{}:{}", el.prefix, el.local)
                        },
                        namespace_undo: undo,
                        projected,
                    });
                }
            }
        }
        out.extend_from_slice(token);
        previous_end = end;
    }
    if !root_seen || !stack.is_empty() {
        return Err(error("incomplete MXL root"));
    }
    if body[previous_end..]
        .iter()
        .any(|b| !b.is_ascii_whitespace())
    {
        return Err(error("MXL has text after its document root"));
    }
    out.extend_from_slice(&body[previous_end..]);
    Ok(out)
}

enum Change {
    Alias(String, String),
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
        return Err(error(
            "DCS projection alias is absent from the type codec registry",
        ));
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
