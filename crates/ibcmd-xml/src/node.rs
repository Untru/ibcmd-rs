//! Public, parser-independent XML tree types.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct QName {
    raw: String,
    prefix: Option<String>,
    local: String,
}
impl QName {
    pub fn new(raw: impl Into<String>) -> Result<Self, String> {
        let raw = raw.into();
        let mut i = raw.split(':');
        let a = i.next().unwrap_or_default();
        let b = i.next();
        if i.next().is_some()
            || a.is_empty()
            || b == Some("")
            || !valid_name(a)
            || b.is_some_and(|s| !valid_name(s))
        {
            return Err(format!("invalid XML QName `{raw}`"));
        }
        Ok(match b {
            Some(b) => Self {
                raw: raw.clone(),
                prefix: Some(a.into()),
                local: b.into(),
            },
            None => Self {
                raw: raw.clone(),
                prefix: None,
                local: raw,
            },
        })
    }
    pub fn raw(&self) -> &str {
        &self.raw
    }
    pub fn prefix(&self) -> Option<&str> {
        self.prefix.as_deref()
    }
    pub fn local(&self) -> &str {
        &self.local
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttributeKind {
    Ordinary(QName),
    Namespace(Option<String>),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    pub(crate) kind: AttributeKind,
    pub(crate) value: String,
}
impl Attribute {
    pub fn ordinary(name: QName, value: impl Into<String>) -> Self {
        Self {
            kind: AttributeKind::Ordinary(name),
            value: value.into(),
        }
    }
    pub fn namespace(prefix: Option<String>, uri: impl Into<String>) -> Self {
        Self {
            kind: AttributeKind::Namespace(prefix),
            value: uri.into(),
        }
    }
    pub fn kind(&self) -> &AttributeKind {
        &self.kind
    }
    pub fn value(&self) -> &str {
        &self.value
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XmlDocument {
    declaration: Option<XmlDeclaration>,
    before_root: Vec<XmlNode>,
    root: XmlElement,
    after_root: Vec<XmlNode>,
}
impl XmlDocument {
    pub fn new(root: XmlElement) -> Self {
        Self {
            declaration: None,
            before_root: vec![],
            root,
            after_root: vec![],
        }
    }
    pub fn root(&self) -> &XmlElement {
        &self.root
    }
    pub fn declaration(&self) -> Option<&str> {
        self.declaration.as_ref().map(XmlDeclaration::value)
    }
    pub fn before_root(&self) -> &[XmlNode] {
        &self.before_root
    }
    pub fn after_root(&self) -> &[XmlNode] {
        &self.after_root
    }
    pub(crate) fn parsed(
        declaration: Option<XmlDeclaration>,
        before_root: Vec<XmlNode>,
        root: XmlElement,
        after_root: Vec<XmlNode>,
    ) -> Self {
        Self {
            declaration,
            before_root,
            root,
            after_root,
        }
    }
    pub(crate) fn declaration_raw(&self) -> Option<&str> {
        self.declaration.as_ref().and_then(|x| x.raw.as_deref())
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct XmlDeclaration {
    value: String,
    raw: Option<String>,
}
impl XmlDeclaration {
    pub(crate) fn parsed(value: String, raw: String) -> Self {
        Self {
            value,
            raw: Some(raw),
        }
    }
    pub(crate) fn value(&self) -> &str {
        &self.value
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XmlElement {
    name: QName,
    attributes: Vec<Attribute>,
    children: Vec<XmlNode>,
    raw_start: Option<String>,
    raw_end: Option<String>,
    empty: bool,
}
impl XmlElement {
    pub fn new(name: QName) -> Self {
        Self {
            name,
            attributes: vec![],
            children: vec![],
            raw_start: None,
            raw_end: None,
            empty: false,
        }
    }
    pub fn with_parts(name: QName, attributes: Vec<Attribute>, children: Vec<XmlNode>) -> Self {
        Self {
            name,
            attributes,
            children,
            raw_start: None,
            raw_end: None,
            empty: false,
        }
    }
    pub fn with_children(&self, children: Vec<XmlNode>) -> Self {
        Self {
            name: self.name.clone(),
            attributes: self.attributes.clone(),
            children,
            raw_start: self.raw_start.clone(),
            raw_end: self.raw_end.clone(),
            empty: self.empty,
        }
    }
    pub fn replaced_children(&self, children: Vec<XmlNode>) -> Self {
        self.with_children(children)
    }
    pub fn name(&self) -> &QName {
        &self.name
    }
    pub fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
    pub fn children(&self) -> &[XmlNode] {
        &self.children
    }
    pub(crate) fn parsed(
        name: QName,
        attributes: Vec<Attribute>,
        children: Vec<XmlNode>,
        raw_start: String,
        raw_end: Option<String>,
        empty: bool,
    ) -> Self {
        Self {
            name,
            attributes,
            children,
            raw_start: Some(raw_start),
            raw_end,
            empty,
        }
    }
    pub(crate) fn raw_start(&self) -> Option<&str> {
        self.raw_start.as_deref()
    }
    pub(crate) fn raw_end(&self) -> Option<&str> {
        self.raw_end.as_deref()
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XmlText {
    value: String,
    raw: Option<String>,
}
impl XmlText {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            raw: None,
        }
    }
    pub fn value(&self) -> &str {
        &self.value
    }
    pub(crate) fn parsed(value: String, raw: String) -> Self {
        Self {
            value,
            raw: Some(raw),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XmlCData {
    value: String,
    raw: Option<String>,
}
impl XmlCData {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            raw: None,
        }
    }
    pub fn value(&self) -> &str {
        &self.value
    }
    pub(crate) fn parsed(value: String, raw: String) -> Self {
        Self {
            value,
            raw: Some(raw),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XmlComment {
    value: String,
    raw: Option<String>,
}
impl XmlComment {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            raw: None,
        }
    }
    pub fn value(&self) -> &str {
        &self.value
    }
    pub(crate) fn parsed(value: String, raw: String) -> Self {
        Self {
            value,
            raw: Some(raw),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XmlRawNode {
    value: String,
    raw: Option<String>,
}
impl XmlRawNode {
    #[cfg(test)]
    pub(crate) fn generated(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            raw: None,
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }
    pub(crate) fn parsed(value: String, raw: String) -> Self {
        Self {
            value,
            raw: Some(raw),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XmlNode {
    Element(XmlElement),
    Text(XmlText),
    CData(XmlCData),
    Comment(XmlComment),
    ProcessingInstruction(XmlRawNode),
    DocType(XmlRawNode),
}
impl XmlNode {
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(XmlText::new(value))
    }
    pub fn cdata(value: impl Into<String>) -> Self {
        Self::CData(XmlCData::new(value))
    }
    pub fn comment(value: impl Into<String>) -> Self {
        Self::Comment(XmlComment::new(value))
    }
    pub(crate) fn raw(&self) -> Option<&str> {
        match self {
            Self::Text(x) => x.raw.as_deref(),
            Self::CData(x) => x.raw.as_deref(),
            Self::Comment(x) => x.raw.as_deref(),
            Self::ProcessingInstruction(x) | Self::DocType(x) => x.raw.as_deref(),
            Self::Element(_) => None,
        }
    }
}
pub(crate) fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(valid_name_start) && chars.all(valid_name_continue)
}
fn valid_name_start(value: char) -> bool {
    value == '_'
        || value.is_ascii_alphabetic()
        || ('\u{c0}'..='\u{d6}').contains(&value)
        || ('\u{d8}'..='\u{f6}').contains(&value)
        || ('\u{f8}'..='\u{2ff}').contains(&value)
        || ('\u{370}'..='\u{37d}').contains(&value)
        || ('\u{37f}'..='\u{1fff}').contains(&value)
        || ('\u{200c}'..='\u{200d}').contains(&value)
        || ('\u{2070}'..='\u{218f}').contains(&value)
        || ('\u{2c00}'..='\u{2fef}').contains(&value)
        || ('\u{3001}'..='\u{d7ff}').contains(&value)
        || ('\u{f900}'..='\u{fdcf}').contains(&value)
        || ('\u{fdf0}'..='\u{fffd}').contains(&value)
        || ('\u{10000}'..='\u{effff}').contains(&value)
}
fn valid_name_continue(value: char) -> bool {
    valid_name_start(value)
        || value == '-'
        || value == '.'
        || value.is_ascii_digit()
        || value == '\u{b7}'
        || ('\u{300}'..='\u{36f}').contains(&value)
        || ('\u{203f}'..='\u{2040}').contains(&value)
}
