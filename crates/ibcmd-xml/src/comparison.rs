//! Exact source comparison allowing CRLF/LF in element-only indentation.

use crate::{AttributeKind, XmlElement, XmlNode, XmlReader};

/// Returns whether two source XML documents differ only in structural CRLF/LF.
/// String values, including whitespace-only values, retain their raw spelling.
/// Invalid documents are never equivalent through this relaxed comparison.
pub fn equal_ignoring_structural_line_endings(left: &[u8], right: &[u8]) -> bool {
    let (Ok(left), Ok(right)) = (XmlReader::from_slice(left), XmlReader::from_slice(right)) else {
        return false;
    };
    if left.has_utf8_bom() != right.has_utf8_bom()
        || left.declaration_raw() != right.declaration_raw()
    {
        return false;
    }
    let mut pending = vec![
        (left.before_root(), right.before_root(), true, false),
        (left.after_root(), right.after_root(), true, false),
    ];
    let mut elements = vec![(left.root(), right.root(), false)];
    loop {
        while let Some((left, right, inherited_space)) = elements.pop() {
            if left.raw_start() != right.raw_start() || left.raw_end() != right.raw_end() {
                return false;
            }
            let preserve_space = preserves_space(left, inherited_space);
            pending.push((
                left.children(),
                right.children(),
                !preserve_space && element_only(left) && element_only(right),
                preserve_space,
            ));
        }
        let Some((left, right, structural, preserve_space)) = pending.pop() else {
            return true;
        };
        if left.len() != right.len() {
            return false;
        }
        for (left, right) in left.iter().zip(right) {
            match (left, right) {
                (XmlNode::Element(left), XmlNode::Element(right)) => {
                    elements.push((left, right, preserve_space));
                }
                (XmlNode::Text(_), XmlNode::Text(_)) if structural => {
                    let (Some(left), Some(right)) = (left.raw(), right.raw()) else {
                        return false;
                    };
                    if !normalized_whitespace(left).eq(normalized_whitespace(right)) {
                        return false;
                    }
                }
                _ if left != right => return false,
                _ => {}
            }
        }
    }
}

fn preserves_space(element: &XmlElement, inherited: bool) -> bool {
    element.attributes().iter().find_map(|attribute| {
        if matches!(attribute.kind(), AttributeKind::Ordinary(name) if name.raw() == "xml:space") {
            Some(attribute.value() != "default")
        } else {
            None
        }
    }).unwrap_or(inherited)
}

fn element_only(element: &XmlElement) -> bool {
    element
        .children()
        .iter()
        .any(|node| matches!(node, XmlNode::Element(_)))
        && element.children().iter().all(|node| match node {
            XmlNode::Text(text) => text.value().bytes().all(xml_whitespace),
            XmlNode::Element(_) | XmlNode::Comment(_) | XmlNode::ProcessingInstruction(_) => true,
            _ => false,
        })
}

fn xml_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
}

fn normalized_whitespace(text: &str) -> impl Iterator<Item = u8> + '_ {
    text.bytes().enumerate().filter_map(|(index, byte)| {
        (byte != b'\r' || text.as_bytes().get(index + 1) != Some(&b'\n')).then_some(byte)
    })
}
#[cfg(test)]
mod tests {
    use super::equal_ignoring_structural_line_endings as same;

    #[test]
    fn structural_line_endings_are_equivalent() {
        assert!(same(
            b"\xef\xbb\xbf<?xml version=\"1.0\"?>\r\n<root>\r\n\t<value>text</value>\r\n</root>\r\n",
            b"\xef\xbb\xbf<?xml version=\"1.0\"?>\n<root>\n\t<value>text</value>\n</root>\n",
        ));
    }

    #[test]
    fn text_line_endings_remain_distinct() {
        for (left, right) in [
            (
                "<root><value>first\r\nsecond</value></root>",
                "<root><value>first\nsecond</value></root>",
            ),
            (
                "<root><value>\r\n</value></root>",
                "<root><value>\n</value></root>",
            ),
            (
                "<root><![CDATA[first\r\nsecond]]></root>",
                "<root><![CDATA[first\nsecond]]></root>",
            ),
            (
                "<root>first\r\n<value/>second</root>",
                "<root>first\n<value/>second</root>",
            ),
            (
                "<root xml:space=\"preserve\">\r\n<value/></root>",
                "<root xml:space=\"preserve\">\n<value/></root>",
            ),
            (
                "<root xml:space=\"preserve\"><group>\r\n<value/></group></root>",
                "<root xml:space=\"preserve\"><group>\n<value/></group></root>",
            ),
        ] {
            assert!(!same(left.as_bytes(), right.as_bytes()), "{left:?}");
        }
    }

    #[test]
    fn other_lexical_differences_remain_distinct() {
        for (left, right) in [
            ("<root>\r\n<value/></root>", "<root>\n <value/></root>"),
            ("<root value=\"a\r\nb\"/>", "<root value=\"a\nb\"/>"),
            (
                "<root><value>&#13;\n</value></root>",
                "<root><value>\n</value></root>",
            ),
            ("<root><!--a\r\nb--></root>", "<root><!--a\nb--></root>"),
        ] {
            assert!(!same(left.as_bytes(), right.as_bytes()), "{left:?}");
        }
    }
}
