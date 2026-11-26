//! Palette namespace lexical policy for XML exported to older source dialects.

use crate::node::{AttributeKind, XmlElement, XmlNode};
use crate::{LexicalPolicy, XmlReader, XmlWriter};

const PALETTE_URI: &str = "http://v8.1c.ru/8.1/data/ui/colors/palette";
const DECLARATION: &str = " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\"";

/// Drops only the exact palette declaration emitted by newer native writers,
/// and only when the validated XML has no possible use of its `pal:` prefix.
/// Decoded QName values and text retain it, including entity-split text.
/// Invalid XML, unresolved entities and uncertain content retain exact bytes.
pub fn strip_unused_palette_namespace(content: Vec<u8>) -> Vec<u8> {
    if !content
        .windows(DECLARATION.len())
        .any(|bytes| bytes == DECLARATION.as_bytes())
    {
        return content;
    }
    let Ok(document) = XmlReader::from_slice(&content) else {
        return content;
    };
    if element_uses_palette(document.root())
        || nodes_use_palette(document.before_root())
        || nodes_use_palette(document.after_root())
    {
        return content;
    }
    let Some(root) = without_unused_declarations(document.root()) else {
        return content;
    };
    XmlWriter::to_vec(&document.with_root(root), LexicalPolicy::Preserve).unwrap_or(content)
}

fn possible_palette_value(value: &str) -> bool {
    // Conservative for QName-like values broken by XML whitespace. This may
    // retain an unused declaration, but never drops a possible use.
    value
        .chars()
        .filter(|c| !matches!(c, ' ' | '\t' | '\r' | '\n'))
        .collect::<String>()
        .contains("pal:")
}

fn element_uses_palette(element: &XmlElement) -> bool {
    element.name().prefix() == Some("pal") || element.attributes().iter().any(|attribute| {
        matches!(attribute.kind(), AttributeKind::Ordinary(name) if name.prefix() == Some("pal"))
            || possible_palette_value(attribute.value())
    }) || nodes_use_palette(element.children())
}

fn nodes_use_palette(nodes: &[XmlNode]) -> bool {
    let mut text = String::new();
    for node in nodes {
        match node {
            XmlNode::Element(element) => {
                if possible_palette_value(&text) || element_uses_palette(element) {
                    return true;
                }
                text.clear();
            }
            XmlNode::Text(value) => text.push_str(value.value()),
            // Entity spelling has no XML meaning inside these nodes, but
            // conservatively honor possible embedded/template QNames as well.
            XmlNode::CData(value) => {
                let Ok(value) = quick_xml::escape::unescape(value.value()) else {
                    return true;
                };
                text.push_str(&value);
            }
            XmlNode::Comment(value) => {
                let Ok(value) = quick_xml::escape::unescape(value.value()) else {
                    return true;
                };
                text.push_str(&value);
            }
            XmlNode::ProcessingInstruction(value) | XmlNode::DocType(value) => {
                let Ok(value) = quick_xml::escape::unescape(value.value()) else {
                    return true;
                };
                text.push_str(&value);
            }
        }
    }
    possible_palette_value(&text)
}

/// Spans are found only within a parsed start tag, respecting both quote
/// kinds. The parser's ordered attribute facts independently confirm each
/// name/value; an identical string inside another attribute is never selected.
fn declaration_spans(element: &XmlElement) -> Option<Vec<(usize, std::ops::Range<usize>)>> {
    let raw = element.raw_start()?;
    let bytes = raw.as_bytes();
    if bytes.first() != Some(&b'<') {
        return None;
    }
    let space = |byte: u8| matches!(byte, b' ' | b'\t' | b'\r' | b'\n');
    let mut index = 1;
    while index < bytes.len() && !space(bytes[index]) && !matches!(bytes[index], b'/' | b'>') {
        index += 1;
    }
    let mut attribute_index = 0;
    let mut spans = Vec::new();
    loop {
        let preceding = index;
        while index < bytes.len() && space(bytes[index]) {
            index += 1;
        }
        if bytes.get(index) == Some(&b'>') || bytes.get(index..) == Some(b"/>".as_slice()) {
            break;
        }
        if index == preceding {
            return None;
        }
        let start = index;
        while index < bytes.len() && !space(bytes[index]) && bytes[index] != b'=' {
            index += 1;
        }
        let name = raw.get(start..index)?;
        while index < bytes.len() && space(bytes[index]) {
            index += 1;
        }
        if bytes.get(index) != Some(&b'=') {
            return None;
        }
        index += 1;
        while index < bytes.len() && space(bytes[index]) {
            index += 1;
        }
        let quote = *bytes.get(index)?;
        if !matches!(quote, b'\'' | b'"') {
            return None;
        }
        index += 1;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        if index >= bytes.len() {
            return None;
        }
        index += 1;
        let attribute = element.attributes().get(attribute_index)?;
        let actual_name = match attribute.kind() {
            AttributeKind::Ordinary(name) => name.raw().to_owned(),
            AttributeKind::Namespace(None) => "xmlns".to_owned(),
            AttributeKind::Namespace(Some(prefix)) => format!("xmlns:{prefix}"),
        };
        if name != actual_name {
            return None;
        }
        if start > 0
            && bytes[start - 1] == b' '
            && raw.get(start - 1..index)? == DECLARATION
            && matches!(attribute.kind(), AttributeKind::Namespace(Some(prefix)) if prefix == "pal")
            && attribute.value() == PALETTE_URI
        {
            spans.push((attribute_index, start - 1..index));
        }
        attribute_index += 1;
    }
    if attribute_index != element.attributes().len() {
        return None;
    }
    Some(spans)
}

fn without_unused_declarations(element: &XmlElement) -> Option<XmlElement> {
    let spans = declaration_spans(element)?;
    let raw = element.raw_start()?;
    let mut start = String::with_capacity(raw.len());
    let mut previous = 0;
    for (_, span) in &spans {
        start.push_str(raw.get(previous..span.start)?);
        previous = span.end;
    }
    start.push_str(raw.get(previous..)?);
    let attributes = element
        .attributes()
        .iter()
        .enumerate()
        .filter(|(index, _)| !spans.iter().any(|(removed, _)| removed == index))
        .map(|(_, attribute)| attribute.clone())
        .collect();
    let children = element
        .children()
        .iter()
        .map(|node| match node {
            XmlNode::Element(child) => without_unused_declarations(child).map(XmlNode::Element),
            _ => Some(node.clone()),
        })
        .collect::<Option<Vec<_>>>()?;
    Some(element.rewritten(attributes, children, Some(start)))
}

#[cfg(test)]
mod tests {
    use super::strip_unused_palette_namespace;

    #[test]
    fn unused_palette_declaration_is_the_only_removed_lexeme() {
        let xml = b"\xef\xbb\xbf<Settings xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\" keep='exact'/>\r\n";
        assert_eq!(
            strip_unused_palette_namespace(xml.to_vec()),
            b"\xef\xbb\xbf<Settings keep='exact'/>\r\n"
        );
    }

    #[test]
    fn used_palette_prefix_in_names_attribute_values_or_text_keeps_exact_bytes() {
        for usage in [
            "<pal:Color/>",
            "<Color xsi:type='pal:Color'/>",
            "<Value>pal:Color</Value>",
            "<!-- pal:Color -->",
        ] {
            let xml = format!("<Settings xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\">{usage}</Settings>").into_bytes();
            assert_eq!(strip_unused_palette_namespace(xml.clone()), xml);
        }
        let invalid_utf8 = vec![0xff, 0xfe];
        assert_eq!(
            strip_unused_palette_namespace(invalid_utf8.clone()),
            invalid_utf8
        );
    }

    #[test]
    fn declaration_literals_in_cdata_comments_and_single_quoted_values_are_not_attributes() {
        let declaration = " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\"";
        for xml in [
            format!("<Settings{declaration}><![CDATA[{declaration}]]></Settings>"),
            format!("<Settings{declaration}><!--{declaration}--></Settings>"),
            format!("<Settings{declaration} keep='{declaration}'/>"),
        ] {
            let expected = xml.replacen(declaration, "", 1).into_bytes();
            assert_eq!(strip_unused_palette_namespace(xml.into_bytes()), expected);
        }
        // A declaration-looking literal alone must never be erased.
        let xml = format!("<Settings keep='{declaration}'><![CDATA[{declaration}]]><!--{declaration}--></Settings>").into_bytes();
        assert_eq!(strip_unused_palette_namespace(xml.clone()), xml);
    }

    #[test]
    fn decoded_and_entity_split_palette_qnames_retain_exact_bytes() {
        for usage in [
            "<Color type='pal&#58;Color'/>",
            "<Color type='pa&#108;&#x3a;Color'/>",
            "<Color type='pal&#xA;&#58;Color'/>",
            "<Value>pal&#58;Color</Value>",
            "<Value>pa&#108;&#x3a;Color</Value>",
            "<Value>pal&#xA;&#x3a;Color</Value>",
            "<Value>pa<![CDATA[l]]>&#58;Color</Value>",
            "<!-- pal&#58;Color -->",
            "<![CDATA[pal&#58;Color]]>",
        ] {
            let xml = format!("<Settings xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\">{usage}</Settings>").into_bytes();
            assert_eq!(strip_unused_palette_namespace(xml.clone()), xml);
        }
    }

    #[test]
    fn invalid_xml_or_unknown_entities_are_untouched() {
        for suffix in [
            "<Value>&unknown;</Value></Settings>",
            "<Value type='pal&unknown;Color'/></Settings>",
            "<!-- &unknown; --></Settings>",
            "<Value></Settings>",
            "</Settings><Second/>",
        ] {
            let xml = format!(
                "<Settings xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\">{suffix}"
            )
            .into_bytes();
            assert_eq!(strip_unused_palette_namespace(xml.clone()), xml);
        }
    }

    #[test]
    fn actual_declaration_spans_preserve_every_other_lexeme_and_nested_scope() {
        let xml = b"\xef\xbb\xbf<?xml version='1.0'?>\r\n<Settings first='same'  xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\"\r\n keep = 'a &amp; b'>\r\n <Child xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\" />\r\n</Settings>\r\n";
        let expected = b"\xef\xbb\xbf<?xml version='1.0'?>\r\n<Settings first='same' \r\n keep = 'a &amp; b'>\r\n <Child />\r\n</Settings>\r\n";
        assert_eq!(strip_unused_palette_namespace(xml.to_vec()), expected);
        let alternative = b"<Settings xmlns:pal='http://v8.1c.ru/8.1/data/ui/colors/palette'/>";
        assert_eq!(
            strip_unused_palette_namespace(alternative.to_vec()),
            alternative
        );
    }
}
