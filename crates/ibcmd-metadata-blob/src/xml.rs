//! A small read-only DOM over a metadata XML: element local names, attributes
//! by local name, direct text. Enough for the kind compilers to read
//! `Properties`, `ChildObjects` and `InternalInfo` without an event loop.

use anyhow::{Context, Result, anyhow, bail};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

#[derive(Clone, Debug, Default)]
pub struct Element {
    /// Local name (`Type` for `v8:Type`).
    pub name: String,
    /// Namespace prefix as written (`v8` for `v8:Type`), empty when none.
    pub prefix: String,
    /// Attributes by local name (`type` for `xsi:type`), in document order.
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Element>,
    /// Concatenated direct text (unescaped).
    pub text: String,
    /// `xmlns:<prefix>="<uri>"` declared on this very element (`""` for a
    /// default `xmlns`), in document order: a QName value such as a web
    /// service's `d6p1:ExchangeFeatures` resolves its prefix here.
    pub namespaces: Vec<(String, String)>,
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
    /// First direct child with this local name.
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|child| child.name == name)
    }
    /// Every direct child with this local name.
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter(move |child| child.name == name)
    }
    /// A descendant by a path of local names (`&["Properties", "Name"]`).
    pub fn path(&self, path: &[&str]) -> Option<&Element> {
        let mut node = self;
        for name in path {
            node = node.child(name)?;
        }
        Some(node)
    }
    /// Direct text of a child, `None` when the child is absent.
    pub fn child_text(&self, name: &str) -> Option<&str> {
        self.child(name).map(|child| child.text.as_str())
    }
    /// `xsi:nil="true"`.
    pub fn is_nil(&self) -> bool {
        self.attr("nil") == Some("true")
    }
    /// `v8:item` pairs of a localized string element (`Synonym`, `ToolTip`, ...):
    /// `(lang, content)` in document order.
    pub fn localized(&self) -> Vec<(String, String)> {
        self.children_named("item")
            .map(|item| {
                (
                    item.child_text("lang").unwrap_or_default().to_string(),
                    item.child_text("content").unwrap_or_default().to_string(),
                )
            })
            .collect()
    }
}

/// A parsed metadata XML: `<MetaDataObject>` and the object element in it.
#[derive(Clone, Debug)]
pub struct MetadataXml {
    pub root: Element,
}

impl MetadataXml {
    /// Line breaks inside a text stay as the file spells them: the platform
    /// stores what the string holds and dumps it byte for byte, so a CR LF it
    /// published came from a stored CR LF. Управление задачами
    /// `Catalogs/узКонфигурации` (a two-line tooltip) loaded back with LF and
    /// dumped 10 bytes short when the reader normalized them.
    pub fn parse(xml: &[u8]) -> Result<Self> {
        let mut root = parse_element_tree_raw_line_breaks(xml)?;
        spell_stored_cr_lf(&mut root);
        Ok(Self { root })
    }
    /// The object element (`<Catalog uuid=...>`).
    pub fn object(&self) -> Result<&Element> {
        self.root
            .children
            .first()
            .ok_or_else(|| anyhow!("<MetaDataObject> is empty"))
    }
    /// `version` of `<MetaDataObject>` (`2.20`, `2.21`).
    pub fn version(&self) -> Option<&str> {
        self.root.attr("version")
    }
}

fn split_name(raw: &[u8]) -> (String, String) {
    let raw = String::from_utf8_lossy(raw);
    match raw.split_once(':') {
        Some((prefix, local)) => (local.to_string(), prefix.to_string()),
        None => (raw.into_owned(), String::new()),
    }
}

fn element_from_start(start: &BytesStart<'_>) -> Result<Element> {
    let (name, prefix) = split_name(start.name().as_ref());
    let mut attrs = Vec::new();
    let mut namespaces = Vec::new();
    for attribute in start.attributes() {
        let attribute = attribute.context("bad XML attribute")?;
        let (key, key_prefix) = split_name(attribute.key.as_ref());
        if key_prefix == "xmlns" || key == "xmlns" {
            let uri = attribute
                .unescape_value()
                .context("bad XML namespace value")?
                .into_owned();
            let declared = if key_prefix == "xmlns" {
                key
            } else {
                String::new()
            };
            namespaces.push((declared, uri));
            continue;
        }
        let value = attribute
            .unescape_value()
            .context("bad XML attribute value")?
            .into_owned();
        attrs.push((key, value));
    }
    Ok(Element {
        name,
        prefix,
        attrs,
        children: Vec::new(),
        text: String::new(),
        namespaces,
    })
}

/// A CR LF inside a text is a CR the string holds before its LF, which the
/// row spells `\r\r\n`; spelling it so here lets the (idempotent)
/// `native_text` leave it alone while it turns a bare LF into CR LF.
fn spell_stored_cr_lf(element: &mut Element) {
    if element.text.contains("\r\n") {
        element.text = element.text.replace("\r\n", "\r\r\n");
    }
    for child in &mut element.children {
        spell_stored_cr_lf(child);
    }
}

/// Parses a whole document into its root element.
pub fn parse_element_tree(xml: &[u8]) -> Result<Element> {
    parse_element_tree_with(xml, true)
}

/// [`parse_element_tree`] keeping a text's line breaks as the file has them
/// (CR LF stays CR LF): where the platform dumps a CR the string holds.
pub fn parse_element_tree_raw_line_breaks(xml: &[u8]) -> Result<Element> {
    parse_element_tree_with(xml, false)
}

fn parse_element_tree_with(xml: &[u8], normalize_line_breaks: bool) -> Result<Element> {
    let xml = xml.strip_prefix(b"\xef\xbb\xbf").unwrap_or(xml);
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(start)) => stack.push(element_from_start(&start)?),
            Ok(Event::Empty(start)) => {
                let element = element_from_start(&start)?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(element),
                    None => root = Some(element),
                }
            }
            Ok(Event::End(_)) => {
                let element = stack.pop().ok_or_else(|| anyhow!("unbalanced XML"))?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(element),
                    None => root = Some(element),
                }
            }
            Ok(Event::Text(text)) => {
                if let Some(current) = stack.last_mut() {
                    let value = if normalize_line_breaks {
                        text.xml_content().context("bad XML text")?
                    } else {
                        text.decode().context("bad XML text")?
                    };
                    let value = quick_xml::escape::unescape(value.as_ref())
                        .context("bad XML text escape")?;
                    current.text.push_str(&value);
                }
            }
            Ok(Event::CData(data)) => {
                if let Some(current) = stack.last_mut() {
                    current
                        .text
                        .push_str(&String::from_utf8_lossy(data.as_ref()));
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if let Some(current) = stack.last_mut() {
                    let name = String::from_utf8_lossy(reference.as_ref()).into_owned();
                    current.text.push_str(&resolve_reference(&name)?);
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => bail!("XML parse error at {}: {error}", reader.error_position()),
        }
        buffer.clear();
    }
    root.ok_or_else(|| anyhow!("XML has no root element"))
}

fn resolve_reference(name: &str) -> Result<String> {
    Ok(match name {
        "lt" => "<".into(),
        "gt" => ">".into(),
        "amp" => "&".into(),
        "apos" => "'".into(),
        "quot" => "\"".into(),
        _ => {
            let code = if let Some(hex) = name.strip_prefix("#x") {
                u32::from_str_radix(hex, 16)?
            } else if let Some(dec) = name.strip_prefix('#') {
                dec.parse::<u32>()?
            } else {
                bail!("unknown XML entity &{name};");
            };
            char::from_u32(code)
                .ok_or_else(|| anyhow!("bad character reference &{name};"))?
                .to_string()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_local_names_attributes_text_and_entities() {
        let xml = br#"<?xml version="1.0"?><MetaDataObject xmlns:v8="x" version="2.20"><Constant uuid="u"><Properties><Name>A&amp;B</Name><Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>&#1044;x</v8:content></v8:item></Synonym><MinValue xsi:nil="true"/></Properties></Constant></MetaDataObject>"#;
        let doc = MetadataXml::parse(xml).unwrap();
        assert_eq!(doc.version(), Some("2.20"));
        let object = doc.object().unwrap();
        assert_eq!(object.name, "Constant");
        assert_eq!(object.attr("uuid"), Some("u"));
        let properties = object.child("Properties").unwrap();
        assert_eq!(properties.child_text("Name"), Some("A&B"));
        assert_eq!(
            properties.child("Synonym").unwrap().localized(),
            vec![("ru".into(), "Дx".into())]
        );
        assert!(properties.child("MinValue").unwrap().is_nil());
    }
}
