//! Structural source comparison for metadata import. Scalar text stays exact.

use std::collections::BTreeSet;

use crate::{AttributeKind, QName, XmlDocument, XmlElement, XmlNode, XmlReader};

fn name_continuation(character: char) -> bool {
    QName::new(format!("a{character}")).is_ok()
}
fn text_prefixes(value: &str, used: &mut BTreeSet<String>) {
    for (colon, _) in value.match_indices(':') {
        if !value[colon + 1..]
            .chars()
            .next()
            .is_some_and(name_continuation)
        {
            continue;
        }
        let start = value[..colon]
            .char_indices()
            .rev()
            .find_map(|(at, character)| {
                (!name_continuation(character)).then_some(at + character.len_utf8())
            })
            .unwrap_or(0);
        let prefix = &value[start..colon];
        if QName::new(prefix).is_ok() {
            used.insert(prefix.to_owned());
        }
    }
}
fn used_prefixes(left: &XmlElement, right: &XmlElement) -> BTreeSet<String> {
    let mut used = BTreeSet::new();
    let mut pending = vec![left, right];
    while let Some(element) = pending.pop() {
        if let Some(prefix) = element.name().prefix() {
            used.insert(prefix.to_owned());
        }
        for attribute in element.attributes() {
            if let AttributeKind::Ordinary(name) = attribute.kind() {
                if let Some(prefix) = name.prefix() {
                    used.insert(prefix.to_owned());
                }
                text_prefixes(attribute.value(), &mut used);
            }
        }
        for node in element.children() {
            match node {
                XmlNode::Element(element) => pending.push(element),
                XmlNode::Text(text) => text_prefixes(text.value(), &mut used),
                XmlNode::CData(text) => text_prefixes(text.value(), &mut used),
                _ => {}
            }
        }
    }
    used
}
fn attributes<'a>(element: &'a XmlElement, used: &BTreeSet<String>) -> Vec<(String, &'a str)> {
    let mut values = element
        .attributes()
        .iter()
        .filter(|a| !matches!(a.kind(), AttributeKind::Namespace(Some(prefix)) if !used.contains(prefix)))
        .map(|a| {
            let key = match a.kind() {
                AttributeKind::Ordinary(name) => format!("a:{}", name.raw()),
                AttributeKind::Namespace(prefix) => {
                    format!("n:{}", prefix.as_deref().unwrap_or(""))
                }
            };
            (key, a.value())
        })
        .collect::<Vec<_>>();
    values.sort_unstable();
    values
}
fn preserve_space(element: &XmlElement, inherited: bool) -> bool {
    element
        .attributes()
        .iter()
        .find_map(|a| match a.kind() {
            AttributeKind::Ordinary(name) if name.raw() == "xml:space" => {
                Some(a.value() != "default")
            }
            _ => None,
        })
        .unwrap_or(inherited)
}
fn indentation(nodes: &[XmlNode]) -> bool {
    nodes.iter().any(|n| matches!(n, XmlNode::Element(_)))
        && nodes.iter().all(|n| match n {
            XmlNode::Text(t) => t
                .value()
                .bytes()
                .all(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n')),
            XmlNode::Element(_) | XmlNode::Comment(_) | XmlNode::ProcessingInstruction(_) => true,
            _ => false,
        })
}
fn nodes(nodes: &[XmlNode], structural: bool) -> Vec<&XmlNode> {
    nodes
        .iter()
        .filter(|n| !structural || !matches!(n, XmlNode::Text(_)))
        .collect()
}
/// Compare an element's data while allowing attribute order, indentation and
/// empty-element spelling. Used namespace bindings and scalar whitespace are
/// data. A prefix used anywhere in either tree keeps all its declarations
/// strict, including shadowed scopes; only entirely unused prefixes vanish.
pub fn source_elements_equal(left: &XmlElement, right: &XmlElement) -> bool {
    let used = used_prefixes(left, right);
    let mut pending = vec![(left, right, false)];
    while let Some((left, right, inherited)) = pending.pop() {
        if left.name() != right.name() || attributes(left, &used) != attributes(right, &used) {
            return false;
        }
        let preserve = preserve_space(left, inherited);
        let structural = !preserve && indentation(left.children()) && indentation(right.children());
        let a = nodes(left.children(), structural);
        let b = nodes(right.children(), structural);
        if a.len() != b.len() {
            return false;
        }
        for (a, b) in a.into_iter().zip(b) {
            match (a, b) {
                (XmlNode::Element(a), XmlNode::Element(b)) => pending.push((a, b, preserve)),
                (XmlNode::Text(a), XmlNode::Text(b)) if a.value() == b.value() => {}
                (XmlNode::CData(a), XmlNode::CData(b)) if a.value() == b.value() => {}
                (XmlNode::Comment(a), XmlNode::Comment(b)) if a.value() == b.value() => {}
                (XmlNode::ProcessingInstruction(a), XmlNode::ProcessingInstruction(b))
                    if a.value() == b.value() => {}
                _ => return false,
            }
        }
    }
    true
}
fn outside(nodes: &[XmlNode]) -> Vec<&XmlNode> {
    nodes
        .iter()
        .filter(|n| !matches!(n, XmlNode::Text(t) if t.value().trim().is_empty()))
        .collect()
}
pub(crate) fn documents_equal(left: &XmlDocument, right: &XmlDocument) -> bool {
    outside(left.before_root()) == outside(right.before_root())
        && outside(left.after_root()) == outside(right.after_root())
        && source_elements_equal(left.root(), right.root())
}
/// Formatting equality for an XML metadata source. Invalid XML never matches.
/// BOM and declaration spelling do not change its metadata; document-type and
/// processing-instruction changes are retained in the comparison.
pub fn equal_ignoring_source_formatting(left: &[u8], right: &[u8]) -> bool {
    match (XmlReader::from_slice(left), XmlReader::from_slice(right)) {
        (Ok(left), Ok(right)) => documents_equal(&left, &right),
        _ => false,
    }
}
#[cfg(test)]
mod tests {
    use super::equal_ignoring_source_formatting as same;
    #[test]
    fn formatting_is_not_scalar_or_namespace_data() {
        assert!(same(
            b"<r b='2' a='1'><x/><y> keep </y></r>",
            b"\xef\xbb\xbf<?xml version='1.0'?>\n<r a=\"1\" b=\"2\">\n  <x></x><y> keep </y>\n</r>"
        ));
        for (a, b) in [
            ("<r><x> keep </x></r>", "<r><x>keep</x></r>"),
            ("<r xmlns='old'/>", "<r xmlns='new'/>"),
            ("<r><x/><y/></r>", "<r><y/><x/></r>"),
            (
                "<r xml:space='preserve'> <x/></r>",
                "<r xml:space='preserve'><x/></r>",
            ),
            ("<r><x> </x></r>", "<r><x/></r>"),
        ] {
            assert!(!same(a.as_bytes(), b.as_bytes()), "{a:?} {b:?}");
        }
        assert!(!same(b"<r>", b"<r>"));
    }
    #[test]
    fn unused_namespace_declarations_are_formatting() {
        let source = b"<r><value>text</value></r>";
        let native =
            b"<r xmlns:pal='http://v8.1c.ru/8.1/data/ui/colors/palette'><value>text</value></r>";
        assert!(same(source, native));
        assert!(same(native, source));
        assert!(same(b"<r xmlns:unused='old'/>", b"<r xmlns:unused='new'/>"));
    }
    #[test]
    fn used_namespace_bindings_and_shadowing_remain_strict() {
        for (left, right) in [
            (
                "<r xmlns:p='old'><p:item/></r>",
                "<r xmlns:p='new'><p:item/></r>",
            ),
            (
                "<r xmlns:p='old' p:value='x'/>",
                "<r xmlns:p='new' p:value='x'/>",
            ),
            (
                "<r xmlns:cfg='old'><value>cfg:Catalog</value></r>",
                "<r xmlns:cfg='new'><value>cfg:Catalog</value></r>",
            ),
            (
                "<r xmlns:cfg='old' value='cfg:Catalog'/>",
                "<r xmlns:cfg='new' value='cfg:Catalog'/>",
            ),
            (
                "<r xmlns:cfg='old' value='cfg:Catalog'/>",
                "<r value='cfg:Catalog'/>",
            ),
            (
                "<r xmlns:p='old'><scope xmlns:p='inner'><p:item/></scope></r>",
                "<r xmlns:p='new'><scope xmlns:p='inner'><p:item/></scope></r>",
            ),
            (
                "<r xmlns:p='outer'><scope xmlns:p='old' value='p:Type'/></r>",
                "<r xmlns:p='outer'><scope xmlns:p='new' value='p:Type'/></r>",
            ),
            (
                "<r xmlns:cfg='old'><v><![CDATA[cfg:Type]]></v></r>",
                "<r xmlns:cfg='new'><v><![CDATA[cfg:Type]]></v></r>",
            ),
            ("<r xmlns='old'/>", "<r xmlns='new'/>"),
            (
                "<r xmlns:имя='old' value='имя:Тип'/>",
                "<r xmlns:имя='new' value='имя:Тип'/>",
            ),
        ] {
            assert!(
                !same(left.as_bytes(), right.as_bytes()),
                "{left:?} {right:?}"
            );
        }
    }
}
