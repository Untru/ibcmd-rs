//! One measured Interface descriptor model for source load and row export.

use anyhow::{Context, Result, ensure};
use ibcmd_schema::interface::InterfaceLayout as Layout;

use super::brace::Brace;
use super::common::Header;
use super::export::values::{header, header_elements};
use super::export::{Build, ObjectNames, atom, el, item, leaf, list, string};
use super::xml::Element;
use super::{ObjectXml, parse_bool};
use crate::brace_list;

pub(crate) struct Interface {
    head: Header,
    switchable: bool,
}

/// Check expanded names on the original bytes, before the compact DOM drops
/// namespace bindings. Prefix aliases are allowed; foreign lookalikes are not.
pub(crate) fn validate_source(bytes: &[u8], version: &str) -> Result<()> {
    use ibcmd_xml::{AttributeKind, XmlElement, XmlNode, XmlReader};
    type Scope = std::collections::BTreeMap<Option<String>, String>;
    fn visit(node: &XmlElement, parent: &Scope, localized: bool) -> Result<()> {
        let mut scope = parent.clone();
        for attr in node.attributes() {
            if let AttributeKind::Namespace(prefix) = attr.kind() {
                scope.insert(prefix.clone(), attr.value().into());
            }
        }
        let uri = scope
            .get(&node.name().prefix().map(str::to_owned))
            .map(String::as_str);
        let expected = if localized {
            "http://v8.1c.ru/8.1/data/core"
        } else {
            "http://v8.1c.ru/8.3/MDClasses"
        };
        ensure!(
            uri == Some(expected),
            "Interface foreign or unbound namespace at {}",
            node.name().local()
        );
        for attr in node.attributes() {
            if let AttributeKind::Ordinary(name) = attr.kind() {
                ensure!(
                    name.prefix().is_none()
                        && matches!(
                            (node.name().local(), name.local()),
                            ("MetaDataObject", "version") | ("Interface", "uuid")
                        ),
                    "Interface unsupported XML attribute"
                );
            }
        }
        for child in node.children() {
            if let XmlNode::Element(child) = child {
                visit(child, &scope, localized || node.name().local() == "Synonym")?;
            }
        }
        Ok(())
    }
    let document = XmlReader::from_slice(bytes)?;
    let root = document.root();
    ensure!(
        root.name().local() == "MetaDataObject",
        "Interface invalid XML envelope"
    );
    let objects: Vec<_> = root
        .children()
        .iter()
        .filter_map(|node| match node {
            XmlNode::Element(element) => Some(element),
            _ => None,
        })
        .collect();
    ensure!(
        objects.len() == 1 && objects[0].name().local() == "Interface",
        "Interface invalid XML root"
    );
    let versions: Vec<_> = root
        .attributes()
        .iter()
        .filter(|a| {
            matches!(a.kind(),
        AttributeKind::Ordinary(name) if name.local() == "version")
        })
        .collect();
    ensure!(
        matches!(version, "2.20" | "2.21") && versions.len() == 1 && versions[0].value() == version,
        "Interface unsupported XML dialect"
    );
    visit(root, &Scope::new(), false)
}

impl Interface {
    pub(crate) fn from_xml(object: &ObjectXml<'_>) -> Result<Self> {
        ensure!(
            object.element.name == Layout::KIND,
            "Interface object kind differs"
        );
        ensure!(
            object.element.attrs.len() == 1
                && object.element.attr("uuid").is_some()
                && object.element.text.trim().is_empty()
                && object.element.children.len() == 1,
            "Interface unsupported object attributes, text or children"
        );
        let p = object.properties()?;
        ensure!(
            p.attrs.is_empty() && p.text.trim().is_empty(),
            "Interface invalid Properties"
        );
        ensure!(
            p.children.len() == 4,
            "Interface requires Name, Synonym, Comment and Switchable"
        );
        for name in ["Name", "Synonym", "Comment", "Switchable"] {
            ensure!(
                p.children_named(name).count() == 1,
                "Interface missing or duplicate {name}"
            );
            let v = object.prop(name)?;
            ensure!(
                v.attrs.is_empty(),
                "Interface {name} has unsupported attributes"
            );
            if name != "Synonym" {
                ensure!(v.children.is_empty(), "Interface {name} must be scalar");
            }
        }
        let synonym = object.prop("Synonym")?;
        ensure!(
            synonym.text.trim().is_empty(),
            "Interface invalid Synonym text"
        );
        let mut languages = std::collections::BTreeSet::new();
        for entry in &synonym.children {
            ensure!(
                entry.name == "item"
                    && entry.attrs.is_empty()
                    && entry.text.trim().is_empty()
                    && entry.children.len() == 2,
                "Interface invalid Synonym item"
            );
            for (field, name) in entry.children.iter().zip(["lang", "content"]) {
                ensure!(
                    field.name == name && field.attrs.is_empty() && field.children.is_empty(),
                    "Interface invalid Synonym {name}"
                );
            }
            let language = &entry.children[0].text;
            ensure!(
                !language.is_empty() && languages.insert(language),
                "Interface duplicate or empty synonym language"
            );
        }
        let head = Header::of(object)?;
        Self::validate_identity(&head)?;
        Ok(Self {
            head,
            switchable: parse_bool(object.prop_text("Switchable")?)?,
        })
    }

    fn validate_identity(head: &Header) -> Result<()> {
        let uuid = uuid::Uuid::parse_str(&head.uuid).context("Interface invalid UUID")?;
        ensure!(
            !uuid.is_nil() && !head.name.is_empty() && !head.name.contains('.'),
            "Interface nil identity or invalid metadata name"
        );
        let mut languages = std::collections::BTreeSet::new();
        ensure!(
            head.synonym
                .iter()
                .all(|(lang, _)| !lang.is_empty() && languages.insert(lang)),
            "Interface duplicate or empty synonym language"
        );
        Ok(())
    }

    pub(crate) fn from_brace(row: &Brace) -> Result<Self> {
        (|| {
            let outer = list(row)?;
            ensure!(
                outer.len() == Layout::OUTER_ARITY
                    && atom(&outer[0])? == "1"
                    && atom(&outer[Layout::CHILD_COUNT])? == "0",
                "Interface unsupported outer layout"
            );
            let body = list(&outer[Layout::PAYLOAD])?;
            ensure!(
                body.len() == Layout::BODY_ARITY
                    && atom(&body[0])? == Layout::RECORD_CODE.to_string()
                    && atom(&body[Layout::RESERVED])? == "1",
                "Interface unsupported descriptor layout"
            );
            let switchable = Layout::switchable_from_code(atom(&body[Layout::SWITCHABLE])?)
                .context("Interface invalid Switchable code")?;
            let decoded = header(item(body, Layout::HEADER)?)?;
            header_elements(&decoded)?; // Validate the localized tuple's arity.
            let values = list(&decoded.synonym)?;
            let synonym = values[1..]
                .chunks_exact(2)
                .map(|pair| Ok((string(&pair[0])?.to_owned(), string(&pair[1])?.to_owned())))
                .collect::<Result<Vec<_>>>()?;
            let head = Header {
                uuid: decoded.uuid,
                name: decoded.name,
                synonym,
                comment: decoded.comment,
            };
            Self::validate_identity(&head)?;
            ensure!(
                head.to_brace() == body[Layout::HEADER],
                "Interface unsupported header members"
            );
            Ok(Self { head, switchable })
        })()
        .context("Interface descriptor")
    }

    pub(crate) fn to_brace(&self) -> Brace {
        brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(i64::from(Layout::RECORD_CODE)),
                Brace::num(Layout::switchable_code(self.switchable)),
                self.head.to_brace(),
                Brace::num(1)
            ],
            Brace::num(0)
        ]
    }

    pub(crate) fn to_xml(&self) -> Result<Element> {
        let decoded = header(&self.head.to_brace())?;
        let p = el("Properties")
            .children(header_elements(&decoded)?)
            .child(leaf("Switchable", self.switchable.to_string()));
        Ok(el(Layout::KIND).attr("uuid", &self.head.uuid).child(p))
    }

    pub(crate) fn names(&self) -> ObjectNames {
        ObjectNames {
            uuid: self.head.uuid.clone(),
            full_name: format!("Interface.{}", self.head.name),
            children: Vec::new(),
            types: Vec::new(),
        }
    }
}
