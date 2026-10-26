//! Conservative namespace-declaration tolerance for the DCS round-trip gate.

use std::collections::BTreeSet;
use std::ops::Range;

use quick_xml::{Reader, events::Event};

use crate::{AttributeKind, XmlElement, XmlNode, XmlReader};

/// Compares DCS source documents while permitting structural indentation and
/// unused prefixed namespace declarations to differ. Default namespaces and
/// every potentially used prefix retain their exact declaration spelling.
///
/// This policy is separate from the general lexical comparison APIs. No QName,
/// attribute, text value or namespace binding is rewritten. A declaration is
/// removed only after a document-wide usage proof, including decoded values
/// and text split by references, comments or child elements. Uncertain proofs
/// fail closed; harmless false refusals are preferable to losing a binding.
pub fn equal_dcs_source_documents(left: &[u8], right: &[u8]) -> bool {
    if crate::equal_ignoring_structural_indentation(left, right) {
        return true;
    }
    let (Some(left), Some(right)) = (
        without_proven_unused_declarations(left),
        without_proven_unused_declarations(right),
    ) else {
        return false;
    };
    crate::equal_ignoring_structural_indentation(&left, &right)
}

fn xml_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
}

pub(crate) fn without_proven_unused_declarations(input: &[u8]) -> Option<Vec<u8>> {
    without_proven_unused_declarations_except(input, &BTreeSet::new())
}

/// A paired policy protects bindings used on either side, even when a typed
/// neutral property is present in only one of the original documents.
pub(crate) fn without_proven_unused_declarations_except(
    input: &[u8],
    protected: &BTreeSet<String>,
) -> Option<Vec<u8>> {
    // quick-xml event positions exclude the UTF-8 BOM. Run the byte-span
    // proof against that same body, then restore the source's exact shell.
    if let Some(body) = input.strip_prefix(b"\xef\xbb\xbf") {
        if body.starts_with(b"\xef\xbb\xbf") {
            return None;
        }
        let normalized = without_proven_unused_declarations_except(body, protected)?;
        let mut output = Vec::with_capacity(normalized.len() + 3);
        output.extend_from_slice(b"\xef\xbb\xbf");
        output.extend_from_slice(&normalized);
        return Some(output);
    }
    let document = XmlReader::from_slice(input).ok()?;
    if document
        .before_root()
        .iter()
        .chain(document.after_root())
        .any(|node| {
            matches!(
                node,
                XmlNode::DocType(_) | XmlNode::ProcessingInstruction(_)
            )
        })
    {
        return None;
    }
    let raw = std::str::from_utf8(input).ok()?;
    // Also inspect embedded/template content in comments, PI and CDATA. An
    // unresolved reference there makes this optional relaxation uncertain.
    let decoded = quick_xml::escape::unescape(raw).ok()?;
    let mut elements = vec![document.root()];
    let mut pending = document.root().children().iter().rev().collect::<Vec<_>>();
    let mut joined_text = String::new();
    while let Some(node) = pending.pop() {
        match node {
            XmlNode::Element(element) => {
                elements.push(element);
                pending.extend(element.children().iter().rev());
            }
            XmlNode::Text(text) => joined_text.push_str(text.value()),
            XmlNode::CData(text) => {
                joined_text.push_str(&quick_xml::escape::unescape(text.value()).ok()?);
            }
            // Excluding comments from XML string-value catches a QName split
            // around one; their own embedded content is checked above.
            XmlNode::Comment(_) => {}
            // Application instructions and DTD semantics are outside this
            // namespace-use proof, even when no literal prefix is visible.
            XmlNode::DocType(_) | XmlNode::ProcessingInstruction(_) => return None,
        }
    }

    let mut declared = BTreeSet::new();
    let mut used_names = BTreeSet::new();
    let mut attribute_values = Vec::new();
    for element in &elements {
        if let Some(prefix) = element.name().prefix() {
            used_names.insert(prefix);
        }
        for attribute in element.attributes() {
            match attribute.kind() {
                AttributeKind::Namespace(Some(prefix)) if prefix != "xml" && prefix != "xmlns" => {
                    declared.insert(prefix.as_str());
                }
                AttributeKind::Ordinary(name) => {
                    if let Some(prefix) = name.prefix() {
                        used_names.insert(prefix);
                    }
                }
                _ => {}
            }
            attribute_values.push(
                attribute
                    .value()
                    .chars()
                    .filter(|character| !matches!(character, ' ' | '\t' | '\r' | '\n'))
                    .collect::<String>(),
            );
        }
    }
    // Whitespace removal here only makes the usage proof more conservative;
    // it never edits a source value or the eventual lexical comparison.
    let compact_text = joined_text
        .bytes()
        .filter(|byte| !xml_whitespace(*byte))
        .collect::<Vec<_>>();
    let compact_text = std::str::from_utf8(&compact_text).ok()?;
    let compact_decoded = decoded
        .chars()
        .filter(|character| !matches!(character, ' ' | '\t' | '\r' | '\n'))
        .collect::<String>();
    let unused = declared
        .into_iter()
        .filter(|prefix| {
            let token = format!("{prefix}:");
            !protected.contains(*prefix)
                && !used_names.contains(prefix)
                && !raw.contains(&token)
                && !decoded.contains(&token)
                && !compact_decoded.contains(&token)
                && !compact_text.contains(&token)
                && !attribute_values.iter().any(|value| value.contains(&token))
        })
        .collect::<BTreeSet<_>>();
    if unused.is_empty() {
        return Some(input.to_vec());
    }

    // Read event spans rather than searching for a tag's text: identical
    // declaration-looking text in a comment, CDATA or value is never selected.
    let mut reader = Reader::from_reader(input);
    let mut buffer = Vec::new();
    let mut element_index = 0usize;
    let mut removals = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer).ok()? {
            Event::Start(_) | Event::Empty(_) => {
                let element = *elements.get(element_index)?;
                element_index += 1;
                let raw = element.raw_start()?;
                let end = usize::try_from(reader.buffer_position()).ok()?;
                let begin = end.checked_sub(raw.len())?;
                if input.get(begin..end)? != raw.as_bytes() {
                    return None;
                }
                for span in declaration_spans(element, &unused)? {
                    removals.push(begin + span.start..begin + span.end);
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    if element_index != elements.len() {
        return None;
    }
    let mut output = Vec::with_capacity(input.len());
    let mut previous = 0;
    for span in removals {
        output.extend_from_slice(input.get(previous..span.start)?);
        previous = span.end;
    }
    output.extend_from_slice(input.get(previous..)?);
    Some(output)
}

/// Quote-aware spans in an already parsed start tag. Ordered attribute names
/// and decoded values independently confirm every scanned attribute. Only the
/// declaration and its immediately preceding separator byte are removed.
fn declaration_spans(element: &XmlElement, unused: &BTreeSet<&str>) -> Option<Vec<Range<usize>>> {
    let raw = element.raw_start()?;
    let bytes = raw.as_bytes();
    if bytes.first() != Some(&b'<') {
        return None;
    }
    let mut index = 1;
    while index < bytes.len()
        && !xml_whitespace(bytes[index])
        && !matches!(bytes[index], b'/' | b'>')
    {
        index += 1;
    }
    if raw.get(1..index)? != element.name().raw() {
        return None;
    }
    let mut attribute_index = 0;
    let mut spans = Vec::new();
    loop {
        let preceding = index;
        while index < bytes.len() && xml_whitespace(bytes[index]) {
            index += 1;
        }
        if bytes.get(index) == Some(&b'>') || bytes.get(index..) == Some(b"/>".as_slice()) {
            break;
        }
        if index == preceding {
            return None;
        }
        let start = index;
        while index < bytes.len() && !xml_whitespace(bytes[index]) && bytes[index] != b'=' {
            index += 1;
        }
        let name = raw.get(start..index)?;
        while index < bytes.len() && xml_whitespace(bytes[index]) {
            index += 1;
        }
        if bytes.get(index) != Some(&b'=') {
            return None;
        }
        index += 1;
        while index < bytes.len() && xml_whitespace(bytes[index]) {
            index += 1;
        }
        let quote = *bytes.get(index)?;
        if !matches!(quote, b'\'' | b'"') {
            return None;
        }
        index += 1;
        let value_start = index;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        let value = quick_xml::escape::unescape(raw.get(value_start..index)?).ok()?;
        if bytes.get(index) != Some(&quote) {
            return None;
        }
        index += 1;
        let attribute = element.attributes().get(attribute_index)?;
        let actual_name = match attribute.kind() {
            AttributeKind::Ordinary(name) => name.raw().to_owned(),
            AttributeKind::Namespace(None) => "xmlns".to_owned(),
            AttributeKind::Namespace(Some(prefix)) => format!("xmlns:{prefix}"),
        };
        if name != actual_name || value != attribute.value() {
            return None;
        }
        if matches!(attribute.kind(), AttributeKind::Namespace(Some(prefix)) if unused.contains(prefix.as_str()))
        {
            spans.push(start - 1..index);
        }
        attribute_index += 1;
    }
    (attribute_index == element.attributes().len()).then_some(spans)
}

#[cfg(test)]
mod tests {
    use super::{equal_dcs_source_documents, without_proven_unused_declarations};

    #[test]
    fn removes_only_verified_attribute_spans_and_their_single_separator() {
        let source = b"<r first='same'  xmlns:p = 'urn:unused'\r\n keep = \"a &amp; b\"><v xmlns:p=\"urn:other\" /></r>";
        let expected = b"<r first='same' \r\n keep = \"a &amp; b\"><v /></r>";
        assert_eq!(
            without_proven_unused_declarations(source).unwrap(),
            expected
        );
    }

    #[test]
    fn declaration_looking_literals_are_never_attribute_spans() {
        let source = br#"<r xmlns:p="urn:unused" keep=' xmlns:p="urn:literal"'><![CDATA[ xmlns:p="urn:literal"]]><!-- xmlns:p="urn:literal" --><v xmlns:p="urn:other"/></r>"#;
        let expected = br#"<r keep=' xmlns:p="urn:literal"'><![CDATA[ xmlns:p="urn:literal"]]><!-- xmlns:p="urn:literal" --><v/></r>"#;
        assert_eq!(
            without_proven_unused_declarations(source).unwrap(),
            expected
        );
    }

    #[test]
    fn possible_embedded_or_split_qnames_retain_every_binding() {
        for usage in [
            "<v>cf<!--gap-->g:Thing</v>",
            "<v>cf<part>g</part>:Thing</v>",
            "<v>c&#102;<!--gap--><![CDATA[g]]>&#58;Thing</v>",
            "<!-- cfg&#58;Thing -->",
            "<![CDATA[cfg&#58;Thing]]>",
            "<v type='cf&#103;&#10;&#58;Thing'/>",
        ] {
            let source = format!("<r xmlns:cfg=\"urn:unused\">{usage}</r>");
            assert_eq!(
                without_proven_unused_declarations(source.as_bytes()).unwrap(),
                source.as_bytes(),
                "{usage}"
            );
        }
    }

    #[test]
    fn uncertain_embedded_entities_or_doctypes_never_justify_namespace_removal() {
        for usage in [
            "<!-- &unknown; -->",
            "<![CDATA[&unknown;]]>",
            "<?probe &unknown;?>",
            "<?probe opaque?>",
        ] {
            let left = format!("<r xmlns:p=\"urn:unused\">{usage}<v/></r>");
            let right = format!("<r>{usage}<v/></r>");
            assert!(!equal_dcs_source_documents(
                left.as_bytes(),
                right.as_bytes()
            ));
        }
        assert!(!equal_dcs_source_documents(
            b"<!DOCTYPE r><r xmlns:p='urn:unused'><v/></r>",
            b"<!DOCTYPE r><r><v/></r>",
        ));
        assert!(!equal_dcs_source_documents(
            b"<?probe opaque?><r xmlns:p='urn:unused'><v/></r>",
            b"<?probe opaque?><r><v/></r>",
        ));
    }
}
