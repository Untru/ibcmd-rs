//! Narrow SDK projections over source XML. Unchanged markup is never reserialized.
use crate::ConvertError;
use quick_xml::{events::Event, Reader};
use std::collections::{BTreeMap, HashMap};
fn error(reason: impl Into<String>) -> ConvertError {
    ConvertError::Read {
        kind: "TemplateBody".into(),
        object: "SDK projection".into(),
        reason: reason.into(),
    }
}

fn output(size: usize) -> Result<Vec<u8>, ConvertError> {
    let mut out = Vec::new();
    out.try_reserve(size)
        .map_err(|e| error(format!("output allocation failed: {e}")))?;
    Ok(out)
}
fn append(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), ConvertError> {
    out.try_reserve(bytes.len())
        .map_err(|e| error(format!("output allocation failed: {e}")))?;
    out.extend_from_slice(bytes);
    Ok(())
}
fn push<T>(items: &mut Vec<T>, value: T) -> Result<(), ConvertError> {
    items
        .try_reserve(1)
        .map_err(|e| error(format!("source traversal allocation failed: {e}")))?;
    items.push(value);
    Ok(())
}
fn string(value: &str) -> Result<String, ConvertError> {
    let mut out = String::new();
    out.try_reserve(value.len())
        .map_err(|e| error(format!("text allocation failed: {e}")))?;
    out.push_str(value);
    Ok(out)
}
// Decode one XML character at a time, retaining its exact lexical source range.
fn characters(
    raw: &[u8],
    cdata: bool,
    mut visit: impl FnMut(char, usize, usize) -> Result<(), ConvertError>,
) -> Result<(), ConvertError> {
    let value = std::str::from_utf8(raw).map_err(|e| error(e.to_string()))?;
    let mut pos = 0;
    while pos < value.len() {
        let (ch, end) = if !cdata && raw[pos] == b'&' {
            let next = value[pos..]
                .find(';')
                .ok_or_else(|| error("unterminated XML entity"))?
                + pos
                + 1;
            let decoded =
                quick_xml::escape::unescape(&value[pos..next]).map_err(|e| error(e.to_string()))?;
            let mut chars = decoded.chars();
            let ch = chars.next().ok_or_else(|| error("empty XML entity"))?;
            if chars.next().is_some() {
                return Err(error("XML entity is not one character"));
            }
            (ch, next)
        } else {
            let ch = value[pos..].chars().next().expect("nonempty UTF-8 suffix");
            (ch, pos + ch.len_utf8())
        };
        if !(matches!(ch, '\t' | '\n' | '\r') || ch >= ' ' && ch != '\u{fffe}' && ch != '\u{ffff}')
        {
            return Err(error("invalid XML character"));
        }
        visit(ch, pos, end)?;
        pos = end;
    }
    Ok(())
}
fn decoded(raw: &[u8], cdata: bool) -> Result<String, ConvertError> {
    let mut value = String::new();
    value
        .try_reserve(raw.len())
        .map_err(|e| error(format!("text allocation failed: {e}")))?;
    characters(raw, cdata, |ch, _, _| {
        value
            .try_reserve(ch.len_utf8())
            .map_err(|e| error(e.to_string()))?;
        value.push(ch);
        Ok(())
    })?;
    Ok(value)
}
fn range(body: &[u8], part: &[u8]) -> Result<(usize, usize), ConvertError> {
    let start = (part.as_ptr() as usize)
        .checked_sub(body.as_ptr() as usize)
        .ok_or_else(|| error("token outside source"))?;
    let end = start
        .checked_add(part.len())
        .ok_or_else(|| error("source offset overflow"))?;
    if end > body.len() {
        return Err(error("token outside source"));
    }
    Ok((start, end))
}

pub(crate) fn text_newlines(body: &[u8], to_crlf: bool) -> Result<Vec<u8>, ConvertError> {
    let mut cursor = MarkupCursor { body, offset: 0 };
    let mut out = output(body.len())?;
    let mut end = 0;
    while let Some((start, next)) = cursor.next()? {
        let text = &body[end..start];
        if text.iter().any(|b| !b.is_ascii_whitespace()) {
            append_newlines(&mut out, text, to_crlf)?;
        } else {
            append(&mut out, text)?;
        }
        append(&mut out, &body[start..next])?;
        end = next;
    }
    append(&mut out, &body[end..])?;
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
        let end = if tail.starts_with(b"<![CDATA[") {
            start
                + 9
                + tail[9..]
                    .windows(3)
                    .position(|w| w == b"]]>")
                    .ok_or_else(|| error("unterminated CDATA"))?
                + 3
        } else if tail.starts_with(b"<!--") {
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

fn append_newlines(out: &mut Vec<u8>, text: &[u8], to_crlf: bool) -> Result<(), ConvertError> {
    let extra = if to_crlf {
        text.iter()
            .enumerate()
            .filter(|&(index, &b)| {
                b == b'\n' && index.checked_sub(1).is_none_or(|p| text[p] != b'\r')
            })
            .count()
    } else {
        0
    };
    let reserve = text
        .len()
        .checked_add(extra)
        .ok_or_else(|| error("output byte count overflow"))?;
    out.try_reserve(reserve)
        .map_err(|e| error(format!("output allocation failed: {e}")))?;
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
    Ok(())
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
    let mut out = output(body.len())?;
    let mut previous_end = 0;
    let mut root_seen = false;
    while let Some((start, end)) = cursor.next()? {
        let text = &body[previous_end..start];
        if stack.last().is_some_and(|frame| frame.projected) {
            append_newlines(&mut out, text, to_crlf)?;
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
            append(&mut out, text)?;
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
        append(&mut out, token)?;
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
    append(&mut out, &body[previous_end..])?;
    Ok(out)
}

const CORE_NS: &str = "http://v8.1c.ru/8.1/data/core";
const DCS_NS: &str = "http://v8.1c.ru/8.1/data-composition-system/schema";
const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";
fn ncname(name: &str) -> bool {
    fn start(c: char) -> bool {
        c == '_'
            || c.is_ascii_alphabetic()
            || matches!(c as u32, 0xc0..=0xd6 | 0xd8..=0xf6 | 0xf8..=0x2ff | 0x370..=0x37d | 0x37f..=0x1fff | 0x200c..=0x200d | 0x2070..=0x218f | 0x2c00..=0x2fef | 0x3001..=0xd7ff | 0xf900..=0xfdcf | 0xfdf0..=0xfffd | 0x10000..=0xeffff)
    }
    let mut chars = name.chars();
    chars.next().is_some_and(start)
        && chars.all(|c| {
            start(c)
                || c.is_ascii_digit()
                || matches!(c, '-' | '.' | '\u{b7}')
                || matches!(c as u32, 0x300..=0x36f | 0x203f..=0x2040)
        })
}
fn qname(value: &str) -> Result<(&str, &str), ConvertError> {
    let (prefix, local) = value.split_once(':').unwrap_or(("", value));
    if !ncname(local) || !prefix.is_empty() && !ncname(prefix) || value.starts_with(':') {
        return Err(error("invalid XML QName"));
    }
    Ok((prefix, local))
}
struct Fragment {
    start: usize,
    end: usize,
    cdata: bool,
}
struct TypeLeaf {
    content_start: usize,
    text: String,
    fragments: Vec<Fragment>,
    inline_prefix: Option<String>,
    children: bool,
}
struct DcsFrame {
    name: String,
    undo: Vec<(String, Option<String>)>,
    leaf: Option<TypeLeaf>,
}
fn restore_namespaces(ns: &mut HashMap<String, String>, undo: Vec<(String, Option<String>)>) {
    for (key, previous) in undo.into_iter().rev() {
        if let Some(value) = previous {
            ns.insert(key, value);
        } else {
            ns.remove(&key);
        }
    }
}
fn finish_leaf(
    body: &[u8],
    leaf: TypeLeaf,
    ns: &HashMap<String, String>,
    to_edt: bool,
    semantic_view: bool,
    content_end: usize,
    out: &mut Vec<u8>,
    emitted: &mut usize,
) -> Result<(), ConvertError> {
    let from = if to_edt { "AnyIBRef" } else { "AnyRef" };
    let Ok((prefix, local)) = qname(&leaf.text) else {
        return Ok(());
    };
    if (local != from && !(semantic_view && matches!(local, "AnyRef" | "AnyIBRef")))
        || ns.get(prefix).map(String::as_str) != Some(formats_xml::type_codec::CURRENT_CONFIG_NS)
    {
        return Ok(());
    }
    if leaf.children || leaf.inline_prefix.as_deref() != Some(prefix) {
        return Err(error(
            "AnyRef TypeSet must be an exact inline-namespace leaf",
        ));
    }
    if semantic_view {
        append(out, &body[*emitted..leaf.content_start])?;
        // Actual expanded QName identity; only this selected typed leaf's text
        // carrier framing is lexical. Keep ordered comments and PI byte-exact.
        if !prefix.is_empty() {
            append(out, prefix.as_bytes())?;
            append(out, b":")?;
        }
        append(out, b"AnyIBRef")?;
        let inner = &body[leaf.content_start..content_end];
        let mut cursor = MarkupCursor {
            body: inner,
            offset: 0,
        };
        while let Some((start, end)) = cursor.next()? {
            let raw = &inner[start..end];
            if raw.starts_with(b"<!--") || raw.starts_with(b"<?") {
                append(out, raw)?;
            }
        }
        *emitted = content_end;
        return Ok(());
    }
    let target = prefix
        .len()
        .checked_add(usize::from(!prefix.is_empty()) + 3)
        .ok_or_else(|| error("QName byte count overflow"))?;
    let mut semantic = 0usize;
    let mut edits = Vec::new();
    for fragment in leaf.fragments {
        characters(
            &body[fragment.start..fragment.end],
            fragment.cdata,
            |ch, begin, end| {
                if semantic == target {
                    push(
                        &mut edits,
                        (
                            fragment.start + begin,
                            if to_edt {
                                fragment.start + end
                            } else {
                                fragment.start + begin
                            },
                            if to_edt { "" } else { "IB" },
                        ),
                    )?;
                } else if to_edt && semantic == target + 1 {
                    push(
                        &mut edits,
                        (fragment.start + begin, fragment.start + end, ""),
                    )?;
                }
                semantic = semantic
                    .checked_add(ch.len_utf8())
                    .ok_or_else(|| error("QName byte count overflow"))?;
                Ok(())
            },
        )?;
    }
    if edits.len() != if to_edt { 2 } else { 1 } {
        return Err(error("QName lexical projection mismatch"));
    }
    for (start, end, text) in edits {
        if start < *emitted {
            return Err(error("overlapping QName projections"));
        }
        append(out, &body[*emitted..start])?;
        append(out, text.as_bytes())?;
        *emitted = end;
    }
    Ok(())
}
/// Iterative namespace-qualified alias projection, preserving arbitrary XML
/// comments, PI, CDATA and all bytes outside the chosen QName characters.
pub(crate) fn dcs_alias(body: &[u8], to_edt: bool) -> Result<Vec<u8>, ConvertError> {
    dcs_scan(body, to_edt, false)
}
/// Semantic view of exact typed DCS template bodies. Always scans current bytes;
/// no stored lexical values or cached fingerprint can override an edit.
pub fn dcs_template_semantic_body(
    owner: &morph1c_core::ir::MetadataObject,
    template: &morph1c_core::ir::Template,
) -> Result<Option<Vec<u8>>, String> {
    use morph1c_core::{ir::PropertyValue, spec::metadata::report_template_ref::F_TEMPLATE_TYPE};
    if !(owner.kind.as_str() == "CommonTemplate" || owner.kind.as_str().ends_with(".TemplateRef"))
        || !matches!(owner.get(F_TEMPLATE_TYPE), Some(PropertyValue::Enum(t)) if t.as_str() == "DataCompositionSchema")
    {
        return Ok(None);
    }
    template
        .body
        .as_deref()
        .map(dcs_qname_semantic_bytes)
        .transpose()
}
/// Current body bytes with only qualified Any[IB]Ref TypeSet text framing
/// represented by its expanded QName; all other bytes remain ordered/exact.
pub fn dcs_qname_semantic_bytes(body: &[u8]) -> Result<Vec<u8>, String> {
    dcs_scan(
        body.strip_prefix(b"\xef\xbb\xbf").unwrap_or(body),
        false,
        true,
    )
    .map_err(|e| e.to_string())
}
fn dcs_scan(body: &[u8], to_edt: bool, semantic_view: bool) -> Result<Vec<u8>, ConvertError> {
    characters(body, true, |_, _, _| Ok(()))?;
    if formats_xml::type_codec::canon_for_config_local("AnyIBRef") != Some("AnyRef") {
        return Err(error(
            "DCS projection alias is absent from the type codec registry",
        ));
    }
    let mut reader = Reader::from_reader(body);
    reader.config_mut().check_comments = true;
    let mut stack: Vec<DcsFrame> = Vec::new();
    let mut ns = HashMap::new();
    ns.insert("xml".into(), XML_NS.into());
    let mut out = output(body.len())?;
    let mut emitted = 0;
    let mut root_seen = false;
    let mut nodes = 0usize;
    let mut declaration = false;
    let mut prolog_markup = false;
    loop {
        let event_start =
            usize::try_from(reader.buffer_position()).map_err(|e| error(e.to_string()))?;
        let event = reader.read_event().map_err(|e| error(e.to_string()))?;
        match event {
            Event::Start(ref element) | Event::Empty(ref element) => {
                let empty = matches!(event, Event::Empty(_));
                nodes = nodes
                    .checked_add(1)
                    .ok_or_else(|| error("XML node count overflow"))?;
                if let Some(parent) = stack.last_mut().and_then(|f| f.leaf.as_mut()) {
                    parent.children = true;
                }
                let name = string(
                    std::str::from_utf8(element.name().as_ref())
                        .map_err(|e| error(e.to_string()))?,
                )?;
                let (prefix, local) = qname(&name)?;
                let mut undo = Vec::new();
                let mut attr_count = 0usize;
                let mut inline_prefix = None;
                let mut attr_names = Vec::new();
                for attr in element.attributes() {
                    let attr = attr.map_err(|e| error(e.to_string()))?;
                    let key =
                        std::str::from_utf8(attr.key.as_ref()).map_err(|e| error(e.to_string()))?;
                    qname(key)?;
                    if attr.value.as_ref().contains(&b'<') {
                        return Err(error("unescaped markup in XML attribute"));
                    }
                    characters(attr.value.as_ref(), false, |_, _, _| Ok(()))?;
                    attr_count = attr_count
                        .checked_add(1)
                        .ok_or_else(|| error("attribute count overflow"))?;
                    if key == "xmlns" || key.starts_with("xmlns:") {
                        let binding = key.strip_prefix("xmlns:").unwrap_or("");
                        let uri = decoded(attr.value.as_ref(), false)?;
                        if binding == "xmlns"
                            || uri == "http://www.w3.org/2000/xmlns/"
                            || binding == "xml" && uri != XML_NS
                            || binding != "xml" && uri == XML_NS
                        {
                            return Err(error("invalid reserved namespace binding"));
                        }
                        if uri == formats_xml::type_codec::CURRENT_CONFIG_NS {
                            inline_prefix = Some(string(binding)?);
                        }
                        ns.try_reserve(1).map_err(|e| error(e.to_string()))?;
                        let old = ns.insert(string(binding)?, uri);
                        push(&mut undo, (string(binding)?, old))?;
                    } else {
                        push(&mut attr_names, string(key)?)?;
                    }
                }
                let mut expanded = std::collections::HashSet::new();
                for attr in &attr_names {
                    let (p, local) = qname(attr)?;
                    if !p.is_empty() && ns.get(p).is_none_or(String::is_empty) {
                        return Err(error("unbound attribute namespace"));
                    }
                    let uri = if p.is_empty() {
                        ""
                    } else {
                        ns.get(p).expect("checked namespace").as_str()
                    };
                    expanded.try_reserve(1).map_err(|e| error(e.to_string()))?;
                    if !expanded.insert((uri, local)) {
                        return Err(error("duplicate expanded XML attribute"));
                    }
                }
                if !prefix.is_empty() && ns.get(prefix).is_none_or(String::is_empty) {
                    return Err(error("unbound element namespace"));
                }
                if stack.is_empty() {
                    if root_seen
                        || local != "DataCompositionSchema"
                        || ns.get(prefix).map(String::as_str) != Some(DCS_NS)
                    {
                        return Err(error("unexpected typed SDK sidecar root/namespace"));
                    }
                    root_seen = true;
                }
                let leaf = (local == "TypeSet"
                    && ns.get(prefix).map(String::as_str) == Some(CORE_NS))
                .then(|| TypeLeaf {
                    content_start: usize::try_from(reader.buffer_position())
                        .expect("in-memory cursor fits usize"),
                    text: String::new(),
                    fragments: Vec::new(),
                    inline_prefix: if attr_count == 1 { inline_prefix } else { None },
                    children: false,
                });
                if empty {
                    restore_namespaces(&mut ns, undo);
                } else {
                    push(&mut stack, DcsFrame { name, undo, leaf })?;
                }
            }
            Event::End(element) => {
                let frame = stack
                    .pop()
                    .ok_or_else(|| error("XML closing tag has no owner"))?;
                if frame.name.as_bytes() != element.name().as_ref() {
                    return Err(error("XML closing tag mismatch"));
                }
                if let Some(leaf) = frame.leaf {
                    finish_leaf(
                        body,
                        leaf,
                        &ns,
                        to_edt,
                        semantic_view,
                        event_start,
                        &mut out,
                        &mut emitted,
                    )?;
                }
                restore_namespaces(&mut ns, frame.undo);
            }
            Event::Text(text) => {
                if text.as_ref().windows(3).any(|w| w == b"]]>") {
                    return Err(error("unescaped CDATA terminator in XML text"));
                }
                characters(text.as_ref(), false, |_, _, _| Ok(()))?;
                if stack.is_empty() && text.as_ref().iter().any(|b| !b.is_ascii_whitespace()) {
                    return Err(error("text outside XML root"));
                }
                if !root_seen {
                    prolog_markup = true;
                }
                if let Some(leaf) = stack.last_mut().and_then(|f| f.leaf.as_mut()) {
                    let value = decoded(text.as_ref(), false)?;
                    leaf.text
                        .try_reserve(value.len())
                        .map_err(|e| error(e.to_string()))?;
                    leaf.text.push_str(&value);
                    let (start, end) = range(body, text.as_ref())?;
                    push(
                        &mut leaf.fragments,
                        Fragment {
                            start,
                            end,
                            cdata: false,
                        },
                    )?;
                }
            }
            Event::CData(text) => {
                if stack.is_empty() {
                    return Err(error("CDATA outside XML root"));
                }
                characters(text.as_ref(), true, |_, _, _| Ok(()))?;
                if let Some(leaf) = stack.last_mut().and_then(|f| f.leaf.as_mut()) {
                    let value = decoded(text.as_ref(), true)?;
                    leaf.text
                        .try_reserve(value.len())
                        .map_err(|e| error(e.to_string()))?;
                    leaf.text.push_str(&value);
                    let (start, end) = range(body, text.as_ref())?;
                    push(
                        &mut leaf.fragments,
                        Fragment {
                            start,
                            end,
                            cdata: true,
                        },
                    )?;
                }
            }
            Event::Decl(decl) => {
                if root_seen || declaration || prolog_markup {
                    return Err(error("misplaced XML declaration"));
                }
                declaration = true;
                if let Some(encoding) = decl.encoding() {
                    if !encoding
                        .map_err(|e| error(e.to_string()))?
                        .as_ref()
                        .eq_ignore_ascii_case(b"UTF-8")
                    {
                        return Err(error("SDK source XML must be UTF-8"));
                    }
                }
            }
            Event::DocType(_) => return Err(error("DTD is forbidden in SDK source projection")),
            Event::Comment(_) | Event::PI(_) => {
                if !root_seen {
                    prolog_markup = true;
                }
            }
            Event::Eof => break,
        }
    }
    if !root_seen || !stack.is_empty() {
        return Err(error("incomplete XML document root"));
    }
    append(&mut out, &body[emitted..])?;
    Ok(out)
}
