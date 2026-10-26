//! Bounded managed-form source comparison for the final configuration guard.

use std::collections::{BTreeMap, BTreeSet};

use crate::{AttributeKind, QName, XmlElement, XmlNode, XmlReader};

const FORM: &str = "http://v8.1c.ru/8.3/xcf/logform";
const CORE: &str = "http://v8.1c.ru/8.1/data/core";
const CFG: &str = "http://v8.1c.ru/8.1/data/enterprise/current-config";
const XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
const XML: &str = "http://www.w3.org/XML/1998/namespace";
type Bindings = BTreeMap<String, String>;

/// Compares XML 2.20 managed forms using only evidenced native defaults.
///
/// After selecting this exact form cohort, the final guard must treat a refusal
/// as authoritative, without falling through to a local-name-only comparison.
/// Unknown coordinates, namespaces, scalar values and collection order remain
/// strict. Only known property owners permit different property names to reorder.
pub fn equal_form_source_documents(left: &[u8], right: &[u8]) -> bool {
    fn canonical(input: &[u8]) -> Option<(Vec<XmlNode>, Element, Vec<XmlNode>)> {
        let document = XmlReader::from_slice(input).ok()?;
        let root = document.root();
        let bindings = bindings_for(root, &Bindings::new())?;
        if root.name().raw() != "Form"
            || resolved(root.name(), &bindings, false)?.uri != FORM
            || ordinary(root, "version") != Some("2.20")
        {
            return None;
        }
        let facts = Facts::from_root(root, &bindings);
        Some((
            shell(document.before_root()),
            normalize(root, &Bindings::new(), Role::Root, &facts, false, 0)?,
            shell(document.after_root()),
        ))
    }
    if left == right {
        return canonical(left).is_some();
    }
    // Validate original namespace scopes and expanded attributes before the
    // optional declaration proof can remove an unused malformed binding.
    if canonical(left).is_none() || canonical(right).is_none() {
        return false;
    }
    let (left, right) = namespace_inputs(left, right);
    match (canonical(&left), canonical(&right)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

fn namespace_inputs(left: &[u8], right: &[u8]) -> (Vec<u8>, Vec<u8>) {
    use crate::dcs_comparison::{
        without_proven_unused_declarations, without_proven_unused_declarations_except,
    };
    fn retained_prefixes(input: &[u8]) -> Option<BTreeSet<String>> {
        let document = XmlReader::from_slice(input).ok()?;
        let mut pending = vec![document.root()];
        let mut prefixes = BTreeSet::new();
        while let Some(element) = pending.pop() {
            for attribute in element.attributes() {
                if let AttributeKind::Namespace(Some(prefix)) = attribute.kind() {
                    prefixes.insert(prefix.clone());
                }
            }
            pending.extend(element.children().iter().filter_map(|node| match node {
                XmlNode::Element(element) => Some(element),
                _ => None,
            }));
        }
        Some(prefixes)
    }
    let candidates = (
        without_proven_unused_declarations(left),
        without_proven_unused_declarations(right),
    );
    if let (Some(candidate_left), Some(candidate_right)) = candidates
        && let (Some(mut protected), Some(right_prefixes)) = (
            retained_prefixes(&candidate_left),
            retained_prefixes(&candidate_right),
        )
    {
        protected.extend(right_prefixes);
        if protected.is_empty() {
            return (candidate_left, candidate_right);
        }
        if let (Some(left), Some(right)) = (
            without_proven_unused_declarations_except(left, &protected),
            without_proven_unused_declarations_except(right, &protected),
        ) {
            return (left, right);
        }
    }
    // An uncertain proof on either side retains all original declarations.
    (left.to_vec(), right.to_vec())
}

fn shell(nodes: &[XmlNode]) -> Vec<XmlNode> {
    nodes
        .iter()
        .filter(|node| !indentation(node))
        .cloned()
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Name {
    raw: String,
    uri: String,
    local: String,
}

fn bindings_for(element: &XmlElement, inherited: &Bindings) -> Option<Bindings> {
    let mut bindings = inherited.clone();
    bindings.insert("xml".into(), XML.into());
    for attribute in element.attributes() {
        if let AttributeKind::Namespace(prefix) = attribute.kind() {
            let prefix = prefix.as_deref().unwrap_or_default();
            let uri = attribute.value();
            if prefix == "xmlns"
                || (prefix == "xml" && uri != XML)
                || (prefix != "xml" && uri == XML)
                || (!prefix.is_empty() && uri.is_empty())
                || uri == "http://www.w3.org/2000/xmlns/"
            {
                return None;
            }
            bindings.insert(prefix.to_owned(), uri.to_owned());
        }
    }
    Some(bindings)
}

fn resolved(name: &QName, bindings: &Bindings, attribute: bool) -> Option<Name> {
    let uri = match name.prefix() {
        Some(prefix) => bindings.get(prefix)?.clone(),
        None if attribute => String::new(),
        None => bindings.get("").cloned().unwrap_or_default(),
    };
    Some(Name {
        raw: name.raw().to_owned(),
        uri,
        local: name.local().to_owned(),
    })
}

fn ordinary<'a>(element: &'a XmlElement, key: &str) -> Option<&'a str> {
    element.attributes().iter().find_map(|attribute| {
        matches!(attribute.kind(), AttributeKind::Ordinary(name) if name.raw() == key)
            .then_some(attribute.value())
    })
}

fn qualified(element: &XmlElement, inherited: &Bindings, uri: &str, local: &str) -> bool {
    bindings_for(element, inherited)
        .and_then(|bindings| resolved(element.name(), &bindings, false))
        .is_some_and(|name| name.uri == uri && name.local == local)
}

fn indentation(node: &XmlNode) -> bool {
    matches!(node, XmlNode::Text(_))
        && node.raw().is_some_and(|raw| {
            raw.bytes()
                .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
        })
}

fn elements(element: &XmlElement) -> Option<Vec<&XmlElement>> {
    element
        .children()
        .iter()
        .filter(|node| !indentation(node))
        .map(|node| match node {
            XmlNode::Element(element) => Some(element),
            _ => None,
        })
        .collect()
}

fn text(element: &XmlElement, namespace_attributes: bool) -> Option<String> {
    if element.attributes().iter().any(|attribute| {
        !namespace_attributes || !matches!(attribute.kind(), AttributeKind::Namespace(_))
    }) {
        return None;
    }
    element
        .children()
        .iter()
        .map(|node| match node {
            XmlNode::Text(text) => Some(text.value()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .map(|parts| parts.concat())
}

fn metadata_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_alphabetic())
        && characters.all(|character| character == '_' || character.is_alphanumeric())
}

fn type_value(attribute: &XmlElement, bindings: &Bindings) -> Option<(String, String)> {
    let bindings = bindings_for(attribute, bindings)?;
    let children = elements(attribute)?;
    let types = children
        .iter()
        .filter(|child| child.name().local() == "Type")
        .collect::<Vec<_>>();
    let [container] = types.as_slice() else {
        return None;
    };
    if !qualified(container, &bindings, FORM, "Type")
        || container
            .attributes()
            .iter()
            .any(|attribute| !matches!(attribute.kind(), AttributeKind::Namespace(_)))
    {
        return None;
    }
    let bindings = bindings_for(container, &bindings)?;
    let members = elements(container)?;
    let [member] = members.as_slice() else {
        return None;
    };
    if !qualified(member, &bindings, CORE, "Type") {
        return None;
    }
    let bindings = bindings_for(member, &bindings)?;
    let value = text(member, true)?;
    let (prefix, local) = value.split_once(':')?;
    if prefix.is_empty() || local.is_empty() || local.contains(':') {
        return None;
    }
    Some((bindings.get(prefix)?.clone(), local.to_owned()))
}

#[derive(Default)]
struct Facts {
    value_tables: BTreeSet<String>,
    document: bool,
}

impl Facts {
    fn from_root(root: &XmlElement, bindings: &Bindings) -> Self {
        fn collect(root: &XmlElement, bindings: &Bindings) -> Option<Facts> {
            let children = elements(root)?;
            let groups = children
                .iter()
                .filter(|child| child.name().local() == "Attributes")
                .collect::<Vec<_>>();
            let [attributes] = groups.as_slice() else {
                return None;
            };
            if !qualified(attributes, bindings, FORM, "Attributes") {
                return None;
            }
            let bindings = bindings_for(attributes, bindings)?;
            let mut names = BTreeMap::<String, (usize, bool)>::new();
            let mut main = Vec::new();
            for attribute in elements(attributes)? {
                if !qualified(attribute, &bindings, FORM, "Attribute") {
                    return None;
                }
                let scope = bindings_for(attribute, &bindings)?;
                let name = ordinary(attribute, "name")?;
                let value = type_value(attribute, &bindings);
                let entry = names.entry(name.to_owned()).or_default();
                entry.0 += 1;
                entry.1 = value
                    .as_ref()
                    .is_some_and(|(uri, local)| uri == CORE && local == "ValueTable");
                let children = elements(attribute)?;
                let flags = children
                    .iter()
                    .filter(|child| child.name().local() == "MainAttribute")
                    .collect::<Vec<_>>();
                if flags.len() > 1 {
                    return None;
                }
                if let Some(flag) = flags.first() {
                    if !qualified(flag, &scope, FORM, "MainAttribute") {
                        return None;
                    }
                    match text(flag, false)?.as_str() {
                        "true" => main.push(value),
                        "false" => {}
                        _ => return None,
                    }
                }
            }
            let document = names.values().all(|(count, _)| *count == 1)
                && match main.as_slice() {
                    [Some((uri, local))] if uri == CFG => local
                        .strip_prefix("DocumentObject.")
                        .is_some_and(metadata_name),
                    _ => false,
                };
            Some(Facts {
                value_tables: names
                    .into_iter()
                    .filter_map(|(name, (count, value_table))| {
                        (count == 1 && value_table && metadata_name(&name)).then_some(name)
                    })
                    .collect(),
                document,
            })
        }
        collect(root, bindings).unwrap_or_default()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Role {
    Root,
    RootItems,
    Items,
    RootButton,
    RootCommandBar,
    Control,
    Attributes,
    Attribute,
    Columns,
    Column,
    Other,
}

impl Role {
    fn properties(self) -> bool {
        matches!(
            self,
            Self::Root
                | Self::RootButton
                | Self::RootCommandBar
                | Self::Control
                | Self::Attribute
                | Self::Column
        )
    }
    fn child(self, owner: &str, name: &Name) -> Self {
        if name.uri != FORM {
            return Self::Other;
        }
        match (self, name.local.as_str()) {
            (Self::Root, "ChildItems") => Self::RootItems,
            (Self::Control | Self::RootCommandBar, "ChildItems")
                if matches!(
                    owner,
                    "UsualGroup"
                        | "Pages"
                        | "Page"
                        | "Table"
                        | "ColumnGroup"
                        | "CommandBar"
                        | "AutoCommandBar"
                        | "ButtonGroup"
                        | "Popup"
                        | "ContextMenu"
                ) =>
            {
                Self::Items
            }
            (Self::Root, "Attributes") => Self::Attributes,
            (Self::Attributes, "Attribute") => Self::Attribute,
            (Self::Attribute | Self::Column, "Columns") => Self::Columns,
            (Self::Columns, "Column") => Self::Column,
            (Self::Root, "AutoCommandBar") => Self::Control,
            (
                Self::Control | Self::RootButton | Self::RootCommandBar,
                "ContextMenu"
                | "AutoCommandBar"
                | "SearchStringAddition"
                | "ViewStatusAddition"
                | "SearchControlAddition",
            ) => Self::Control,
            (Self::RootItems, "Button") => Self::RootButton,
            (Self::RootItems, "CommandBar") => Self::RootCommandBar,
            (Self::RootItems, tag) if known_control(tag) => Self::Control,
            (Self::Items, tag) if known_control(tag) => Self::Control,
            _ => Self::Other,
        }
    }
}

fn known_control(tag: &str) -> bool {
    matches!(
        tag,
        "InputField"
            | "LabelField"
            | "CheckBoxField"
            | "PictureField"
            | "RadioButtonField"
            | "UsualGroup"
            | "CommandBar"
            | "AutoCommandBar"
            | "ColumnGroup"
            | "Popup"
            | "Pages"
            | "Page"
            | "ButtonGroup"
            | "ContextMenu"
            | "Button"
            | "Table"
            | "LabelDecoration"
            | "PictureDecoration"
            | "SearchStringAddition"
            | "ViewStatusAddition"
            | "SearchControlAddition"
            | "ProgressBarField"
            | "TrackBarField"
    )
}

fn known_property(role: Role, owner: &str, property: &Name) -> bool {
    if property.uri != FORM {
        return false;
    }
    if role == Role::Control && owner == "InputField" && property.local == "AutoMaxHeight" {
        return true;
    }
    match role {
        Role::Root => matches!(
            property.local.as_str(),
            "Title"
                | "AutoTitle"
                | "AutoCommandBar"
                | "Events"
                | "ChildItems"
                | "Attributes"
                | "Parameters"
                | "Commands"
                | "Width"
                | "Height"
                | "AutoTime"
                | "UsePostingMode"
                | "RepostOnWrite"
        ),
        Role::Control | Role::RootButton | Role::RootCommandBar => matches!(
            property.local.as_str(),
            "AutoMaxWidth"
                | "ContextMenu"
                | "DataPath"
                | "Events"
                | "ExtendedTooltip"
                | "Height"
                | "HorizontalStretch"
                | "MultiLine"
                | "ReadOnly"
                | "Title"
                | "VerticalStretch"
                | "Width"
                | "CheckBoxType"
                | "AutoCommandBar"
                | "ChildItems"
                | "Representation"
                | "RowFilter"
                | "SearchControlAddition"
                | "SearchStringAddition"
                | "ViewStatusAddition"
                | "Group"
                | "ShowTitle"
                | "Visible"
                | "CommandName"
                | "Type"
                | "Autofill"
                | "AdditionSource"
        ),
        Role::Attribute | Role::Column => matches!(
            property.local.as_str(),
            "Type" | "MainAttribute" | "SavedData" | "Columns"
        ),
        _ => false,
    }
}

fn value_table_bound(element: &XmlElement, bindings: &Bindings, facts: &Facts) -> bool {
    let Some(children) = elements(element) else {
        return false;
    };
    let paths = children
        .iter()
        .filter(|child| child.name().local() == "DataPath")
        .collect::<Vec<_>>();
    let [path] = paths.as_slice() else {
        return false;
    };
    qualified(path, bindings, FORM, "DataPath")
        && text(path, false).is_some_and(|path| facts.value_tables.contains(&path))
}

fn neutral(
    property: &XmlElement,
    bindings: &Bindings,
    owner: &XmlElement,
    role: Role,
    facts: &Facts,
) -> bool {
    if !qualified(property, bindings, FORM, property.name().local()) {
        return false;
    }
    let key = property.name().local();
    let expected = match (role, owner.name().local(), key) {
        (Role::Control, "CheckBoxField", "CheckBoxType") => Some("Auto"),
        (Role::Control, "UsualGroup", "ShowTitle") => Some("true"),
        (Role::RootButton, "Button", "Type") => Some("UsualButton"),
        (Role::Root, _, "AutoTime") if facts.document => Some("CurrentOrLast"),
        (Role::Root, _, "UsePostingMode") if facts.document => Some("Auto"),
        (Role::Root, _, "RepostOnWrite") if facts.document => Some("true"),
        _ => None,
    };
    if let Some(expected) = expected {
        return text(property, false).as_deref() == Some(expected);
    }
    if role != Role::Control
        || owner.name().local() != "Table"
        || key != "RowFilter"
        || !value_table_bound(owner, bindings, facts)
        || !property.children().is_empty()
    {
        return false;
    }
    let [attribute] = property.attributes() else {
        return false;
    };
    let AttributeKind::Ordinary(name) = attribute.kind() else {
        return false;
    };
    resolved(name, bindings, true).is_some_and(|name| name.uri == XSI && name.local == "nil")
        && attribute.value() == "true"
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
enum AttributeName {
    Namespace(Option<String>),
    Ordinary(Name),
}

#[derive(Debug, Eq, PartialEq)]
struct Element {
    name: Name,
    attributes: BTreeMap<AttributeName, String>,
    children: Vec<Node>,
}

#[derive(Debug, Eq, PartialEq)]
enum Node {
    Element(Box<Element>),
    Text(String),
    CData(String),
    Comment(String),
    Instruction(String),
}

fn unsupported_group_arrangement(
    element: &XmlElement,
    name: &Name,
    bindings: &Bindings,
    role: Role,
) -> bool {
    role == Role::Control
        && name.uri == FORM
        && name.local == "UsualGroup"
        && element.children().iter().any(|node| {
            let XmlNode::Element(property) = node else {
                return false;
            };
            qualified(property, bindings, FORM, "Group")
                && property
                    .children()
                    .iter()
                    .filter_map(|node| match node {
                        XmlNode::Text(text) => Some(text.value()),
                        XmlNode::CData(text) => Some(text.value()),
                        _ => None,
                    })
                    .collect::<String>()
                    .trim()
                    == "HorizontalIfPossible"
        })
}

fn normalize(
    element: &XmlElement,
    inherited: &Bindings,
    role: Role,
    facts: &Facts,
    inherited_space: bool,
    depth: usize,
) -> Option<Element> {
    // Fail closed on pathological documents instead of growing recursive work
    // past the shallow hierarchy evidenced by native managed forms.
    if depth > 256 {
        return None;
    }
    let bindings = bindings_for(element, inherited)?;
    let name = resolved(element.name(), &bindings, false)?;
    // XML 2.20 does not admit this arrangement on UsualGroup. Validate the
    // domain before canonical equality, including its byte-identity fast path.
    if unsupported_group_arrangement(element, &name, &bindings, role) {
        return None;
    }
    let mut attributes = BTreeMap::new();
    let mut expanded = BTreeSet::new();
    let mut preserve = inherited_space;
    for attribute in element.attributes() {
        let key = match attribute.kind() {
            AttributeKind::Namespace(prefix) => AttributeName::Namespace(prefix.clone()),
            AttributeKind::Ordinary(name) => {
                let name = resolved(name, &bindings, true)?;
                if !expanded.insert((name.uri.clone(), name.local.clone())) {
                    return None;
                }
                if name.uri == XML && name.local == "space" {
                    preserve = attribute.value() != "default";
                }
                AttributeName::Ordinary(name)
            }
        };
        if attributes
            .insert(key, attribute.value().to_owned())
            .is_some()
        {
            return None;
        }
    }
    let mut counts = BTreeMap::<&str, usize>::new();
    for child in element.children() {
        if let XmlNode::Element(child) = child {
            *counts.entry(child.name().local()).or_default() += 1;
        }
    }
    // An empty root UI command bar is a container even when no child element
    // demonstrates its layout. Only its literal indentation is neutral; extra
    // attributes, opaque content and all other empty coordinates stay strict.
    let empty_root_command_bar = role == Role::RootCommandBar
        && name.uri == FORM
        && name.local == "CommandBar"
        && element.attributes().len() == 2
        && ordinary(element, "name").is_some()
        && ordinary(element, "id").is_some()
        && element.children().iter().all(indentation);
    let structural = !preserve
        && (empty_root_command_bar
            || (element
                .children()
                .iter()
                .any(|node| matches!(node, XmlNode::Element(_)))
                && element
                    .children()
                    .iter()
                    .all(|node| matches!(node, XmlNode::Element(_)) || indentation(node))));
    let mut children = Vec::new();
    for child in element.children() {
        if structural && indentation(child) {
            continue;
        }
        let next = match child {
            XmlNode::Element(child) => {
                if structural
                    && counts[child.name().local()] == 1
                    && neutral(child, &bindings, element, role, facts)
                {
                    continue;
                }
                let scope = bindings_for(child, &bindings)?;
                let child_name = resolved(child.name(), &scope, false)?;
                Node::Element(Box::new(normalize(
                    child,
                    &bindings,
                    role.child(&name.local, &child_name),
                    facts,
                    preserve,
                    depth + 1,
                )?))
            }
            XmlNode::Text(text) => {
                if let Some(Node::Text(previous)) = children.last_mut() {
                    previous.push_str(text.value());
                    continue;
                }
                Node::Text(text.value().to_owned())
            }
            XmlNode::CData(_) => Node::CData(child.raw()?.to_owned()),
            XmlNode::Comment(_) => Node::Comment(child.raw()?.to_owned()),
            XmlNode::ProcessingInstruction(_) => Node::Instruction(child.raw()?.to_owned()),
            XmlNode::DocType(_) => return None,
        };
        children.push(next);
    }
    // Different prefixes can denote the same property. Any repeated expanded
    // name keeps the complete owner order rather than sorting raw QName aliases.
    // Collections and unknown containers also retain full heterogeneous order.
    let unique_properties = {
        let mut property_names = BTreeSet::new();
        children.iter().all(|node| match node {
            Node::Element(element) => {
                property_names.insert((&element.name.uri, &element.name.local))
            }
            _ => false,
        })
    };
    if !preserve
        && role.properties()
        && unique_properties
        && children.iter().all(
            |node| matches!(node, Node::Element(element) if known_property(role, &name.local, &element.name)),
        )
    {
        children.sort_by(|left, right| match (left, right) {
            (Node::Element(left), Node::Element(right)) => left.name.cmp(&right.name),
            _ => std::cmp::Ordering::Equal,
        });
    }
    Some(Element {
        name,
        attributes,
        children,
    })
}
#[cfg(test)]
mod tests {
    use super::equal_form_source_documents;

    fn form(content: &str) -> String {
        format!(
            "<Form xmlns=\"http://v8.1c.ru/8.3/xcf/logform\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">{content}</Form>"
        )
    }

    fn control(kind: &str, properties: &str) -> String {
        form(&format!(
            "<ChildItems><{kind} name=\"Probe\" id=\"7\">{properties}<ExtendedTooltip name=\"ProbeTip\" id=\"8\"/></{kind}></ChildItems>"
        ))
    }

    fn value_table(table_properties: &str, attribute: &str) -> String {
        form(&format!(
            "<ChildItems><UsualGroup name=\"Group\" id=\"2\"><ChildItems><Table name=\"Grid\" id=\"7\"><DataPath>Rows</DataPath>{table_properties}<ExtendedTooltip name=\"GridTip\" id=\"8\"/></Table></ChildItems></UsualGroup></ChildItems><Attributes>{attribute}</Attributes>"
        ))
    }

    fn rows_attribute() -> &'static str {
        "<Attribute name=\"Rows\" id=\"9\"><Type><v8:Type>v8:ValueTable</v8:Type></Type></Attribute>"
    }

    fn document_form(properties: &str, attributes: &str) -> String {
        form(&format!(
            "{properties}<Attributes>{attributes}</Attributes>"
        ))
    }

    fn document_attribute() -> &'static str {
        "<Attribute name=\"Object\" id=\"9\"><Type><v8:Type>cfg:DocumentObject.Probe</v8:Type></Type><MainAttribute>true</MainAttribute></Attribute>"
    }

    fn equal(left: &str, right: &str) {
        assert!(
            equal_form_source_documents(left.as_bytes(), right.as_bytes()),
            "{left}\n{right}"
        );
        assert!(
            equal_form_source_documents(right.as_bytes(), left.as_bytes()),
            "reverse: {left}\n{right}"
        );
    }

    fn different(left: &str, right: &str) {
        assert!(
            !equal_form_source_documents(left.as_bytes(), right.as_bytes()),
            "{left}\n{right}"
        );
        assert!(
            !equal_form_source_documents(right.as_bytes(), left.as_bytes()),
            "reverse: {left}\n{right}"
        );
    }

    #[test]
    fn identical_forms_and_structural_indentation_are_controls() {
        let source = control("CheckBoxField", "<ReadOnly>true</ReadOnly>");
        equal(&source, &source);
        equal(&source, &source.replace("<ChildItems>", "<ChildItems>\n\t"));
    }

    #[test]
    fn only_exact_form_namespace_and_dialect_are_supported() {
        let source = form("<Title>Probe</Title>");
        for unsupported in [
            source.replace("version=\"2.20\"", "version=\"2.21\""),
            source.replace("version=\"2.20\"", ""),
            source.replace("http://v8.1c.ru/8.3/xcf/logform", "urn:foreign"),
            source.replace("Form", "Other"),
        ] {
            different(&unsupported, &unsupported);
        }
    }

    #[test]
    fn checkbox_auto_is_the_only_evidenced_default() {
        equal(
            &control("CheckBoxField", ""),
            &control("CheckBoxField", "<CheckBoxType>Auto</CheckBoxType>"),
        );
    }

    #[test]
    fn checkbox_nondefaults_unknown_coordinates_and_wrong_owners_are_strict() {
        let absent = control("CheckBoxField", "");
        for property in [
            "<CheckBoxType>Switch</CheckBoxType>",
            "<CheckBoxType> Auto</CheckBoxType>",
            "<CheckBoxType extra=\"1\">Auto</CheckBoxType>",
            "<CheckBoxType><Unknown/>Auto</CheckBoxType>",
            "<CheckBoxType>Auto<!--extra--></CheckBoxType>",
            "<CheckBoxType>Auto</CheckBoxType><CheckBoxType>Auto</CheckBoxType>",
            "<CheckBoxType xmlns=\"urn:foreign\">Auto</CheckBoxType>",
        ] {
            different(&absent, &control("CheckBoxField", property));
        }
        for owner in ["InputField", "Unknown", "UsualGroup"] {
            different(
                &control(owner, ""),
                &control(owner, "<CheckBoxType>Auto</CheckBoxType>"),
            );
        }
        different(
            &form("<Unknown><CheckBoxField/></Unknown>"),
            &form(
                "<Unknown><CheckBoxField><CheckBoxType>Auto</CheckBoxType></CheckBoxField></Unknown>",
            ),
        );
    }

    #[test]
    fn usual_group_true_title_is_the_only_evidenced_group_default() {
        equal(
            &control("UsualGroup", ""),
            &control("UsualGroup", "<ShowTitle>true</ShowTitle>"),
        );
    }

    #[test]
    fn group_title_false_unknown_coordinates_and_other_owners_are_strict() {
        for property in [
            "<ShowTitle>false</ShowTitle>",
            "<ShowTitle>true </ShowTitle>",
            "<ShowTitle extra=\"1\">true</ShowTitle>",
            "<ShowTitle><Unknown/>true</ShowTitle>",
            "<ShowTitle>true</ShowTitle><ShowTitle>true</ShowTitle>",
            "<ShowTitle xmlns=\"urn:foreign\">true</ShowTitle>",
        ] {
            different(&control("UsualGroup", ""), &control("UsualGroup", property));
        }
        for owner in ["Page", "Table", "CheckBoxField", "Unknown"] {
            different(
                &control(owner, ""),
                &control(owner, "<ShowTitle>true</ShowTitle>"),
            );
        }
    }

    #[test]
    fn nil_row_filter_requires_a_direct_singleton_value_table_binding() {
        equal(
            &value_table("", rows_attribute()),
            &value_table("<RowFilter xsi:nil=\"true\"/>", rows_attribute()),
        );
    }

    #[test]
    fn row_filter_unknown_attributes_content_and_duplicate_properties_are_strict() {
        let absent = value_table("", rows_attribute());
        for property in [
            "<RowFilter/>",
            "<RowFilter xsi:nil=\"false\"/>",
            "<RowFilter xsi:nil=\"true\" extra=\"1\"/>",
            "<RowFilter xsi:nil=\"true\"><Unknown/></RowFilter>",
            "<RowFilter xsi:nil=\"true\">value</RowFilter>",
            "<RowFilter xsi:nil=\"true\"/><RowFilter xsi:nil=\"true\"/>",
            "<RowFilter xmlns:xsi=\"urn:foreign\" xsi:nil=\"true\"/>",
            "<RowFilter xmlns=\"urn:foreign\" xsi:nil=\"true\"/>",
        ] {
            different(&absent, &value_table(property, rows_attribute()));
        }
    }

    #[test]
    fn unresolved_ambiguous_foreign_and_non_value_table_bindings_are_strict() {
        for attribute in [
            String::new(),
            rows_attribute().replace("name=\"Rows\"", "name=\"Other\""),
            rows_attribute().repeat(2),
            rows_attribute().replace("v8:ValueTable", "v8:DynamicList"),
            rows_attribute().replace("v8:ValueTable", "v8:ValueList"),
            rows_attribute().replace("<Type>", "<Type xmlns=\"urn:foreign\">"),
            rows_attribute().replace("<v8:Type>", "<v8:Type xmlns:v8=\"urn:foreign\">"),
            rows_attribute().replace(
                "<v8:Type>v8:ValueTable</v8:Type>",
                "<v8:Type>v8:ValueTable</v8:Type><v8:Type>xs:string</v8:Type>",
            ),
            rows_attribute().replace("<Type>", "<Type><Unknown/>"),
        ] {
            different(
                &value_table("", &attribute),
                &value_table("<RowFilter xsi:nil=\"true\"/>", &attribute),
            );
        }
        let absent = value_table("", rows_attribute());
        let nil = value_table("<RowFilter xsi:nil=\"true\"/>", rows_attribute());
        for (old, new) in [
            ("<DataPath>Rows</DataPath>", ""),
            (
                "<DataPath>Rows</DataPath>",
                "<DataPath>Object.Rows</DataPath>",
            ),
            (
                "<DataPath>Rows</DataPath>",
                "<DataPath>Rows</DataPath><DataPath>Rows</DataPath>",
            ),
        ] {
            different(&absent.replace(old, new), &nil.replace(old, new));
        }
    }

    #[test]
    fn metadata_qualified_bindings_resolve_the_value_prefix_at_its_own_scope() {
        let attribute = rows_attribute()
            .replace("v8:ValueTable", "local:ValueTable")
            .replace(
                "<Type>",
                "<Type xmlns:local=\"http://v8.1c.ru/8.1/data/core\">",
            );
        equal(
            &value_table("", &attribute),
            &value_table("<RowFilter xsi:nil=\"true\"/>", &attribute),
        );
        let foreign = attribute.replace(
            "xmlns:local=\"http://v8.1c.ru/8.1/data/core\"",
            "xmlns:local=\"urn:foreign\"",
        );
        different(
            &value_table("", &foreign),
            &value_table("<RowFilter xsi:nil=\"true\"/>", &foreign),
        );
    }

    #[test]
    fn unique_main_document_attribute_enables_only_the_three_native_root_defaults() {
        let explicit = "<AutoTime>CurrentOrLast</AutoTime><UsePostingMode>Auto</UsePostingMode><RepostOnWrite>true</RepostOnWrite>";
        equal(
            &document_form("", document_attribute()),
            &document_form(explicit, document_attribute()),
        );
        for property in [
            "<AutoTime>CurrentOrLast</AutoTime>",
            "<UsePostingMode>Auto</UsePostingMode>",
            "<RepostOnWrite>true</RepostOnWrite>",
        ] {
            equal(
                &document_form("", document_attribute()),
                &document_form(property, document_attribute()),
            );
        }
    }

    #[test]
    fn document_defaults_require_a_unique_typed_main_document_attribute() {
        for attribute in [
            String::new(),
            document_attribute().replace("<MainAttribute>true</MainAttribute>", ""),
            document_attribute().replace("<MainAttribute>true", "<MainAttribute>false"),
            document_attribute().replace("cfg:DocumentObject.Probe", "cfg:CatalogObject.Probe"),
            document_attribute().replace("cfg:DocumentObject.Probe", "cfg:DocumentObject."),
            document_attribute().replace("cfg:DocumentObject.Probe", "cfg:DocumentObject.Bad.Name"),
            document_attribute().replace("cfg:DocumentObject.Probe", "cfg:DocumentObject.Bad Name"),
            document_attribute().replace("<v8:Type>", "<v8:Type xmlns:cfg=\"urn:foreign\">"),
            document_attribute().replace("<v8:Type>", "<v8:Type xmlns:v8=\"urn:foreign\">"),
            document_attribute().repeat(2),
            document_attribute().replace(
                "<MainAttribute>true</MainAttribute>",
                "<MainAttribute>true</MainAttribute><MainAttribute>true</MainAttribute>",
            ),
            document_attribute().replace("<MainAttribute>", "<MainAttribute extra=\"1\">"),
            document_attribute().replace("<Type>", "<Type><Unknown/>"),
        ] {
            different(
                &document_form("", &attribute),
                &document_form("<AutoTime>CurrentOrLast</AutoTime>", &attribute),
            );
        }
    }

    #[test]
    fn document_nondefaults_and_unknown_property_coordinates_are_strict() {
        for property in [
            "<AutoTime>Current</AutoTime>",
            "<UsePostingMode>Regular</UsePostingMode>",
            "<RepostOnWrite>false</RepostOnWrite>",
            "<AutoTime extra=\"1\">CurrentOrLast</AutoTime>",
            "<AutoTime><Unknown/>CurrentOrLast</AutoTime>",
            "<AutoTime>CurrentOrLast</AutoTime><AutoTime>CurrentOrLast</AutoTime>",
            "<UsePostingMode xmlns=\"urn:foreign\">Auto</UsePostingMode>",
        ] {
            different(
                &document_form("", document_attribute()),
                &document_form(property, document_attribute()),
            );
        }
    }

    #[test]
    fn different_property_names_may_reorder_but_repeated_named_elements_may_not() {
        equal(
            &control(
                "InputField",
                "<ReadOnly>true</ReadOnly><DataPath>Amount</DataPath>",
            ),
            &control(
                "InputField",
                "<DataPath>Amount</DataPath><ReadOnly>true</ReadOnly>",
            ),
        );
        let item = |name: &str| {
            format!(
                "<InputField name=\"{name}\" id=\"{name}\"><DataPath>{name}</DataPath></InputField>"
            )
        };
        different(
            &form(&format!(
                "<ChildItems>{}{}</ChildItems>",
                item("A"),
                item("B")
            )),
            &form(&format!(
                "<ChildItems>{}{}</ChildItems>",
                item("B"),
                item("A")
            )),
        );
    }

    #[test]
    fn heterogeneous_semantic_collections_preserve_full_child_order() {
        for (container, first, second) in [
            (
                "ChildItems",
                "<InputField name=\"A\" id=\"1\"/>",
                "<Button name=\"B\" id=\"2\"/>",
            ),
            (
                "Attributes",
                "<Attribute name=\"A\" id=\"1\"/>",
                "<Unknown name=\"B\"/>",
            ),
            (
                "Commands",
                "<Command name=\"A\" id=\"1\"/>",
                "<Unknown name=\"B\"/>",
            ),
            (
                "Columns",
                "<Column name=\"A\" id=\"1\"/>",
                "<Unknown name=\"B\"/>",
            ),
            ("Unknown", "<First/>", "<Second/>"),
        ] {
            different(
                &form(&format!("<{container}>{first}{second}</{container}>")),
                &form(&format!("<{container}>{second}{first}</{container}>")),
            );
        }
        let group = |first: &str, second: &str| {
            control(
                "UsualGroup",
                &format!("<ChildItems>{first}{second}</ChildItems>"),
            )
        };
        different(
            &group("<InputField name=\"A\"/>", "<Button name=\"B\"/>"),
            &group("<Button name=\"B\"/>", "<InputField name=\"A\"/>"),
        );
    }

    #[test]
    fn only_proven_unused_prefixed_namespace_declarations_may_differ() {
        let left =
            form("<Title>Probe</Title>").replace(" version=", " xmlns:extra=\"urn:one\" version=");
        let right =
            form("<Title>Probe</Title>").replace(" version=", " xmlns:other=\"urn:two\" version=");
        equal(&left, &right);
        for text in [
            "extra:Thing",
            "extra&#58;Thing",
            "ex<!--gap-->tra:Thing",
            "ex<![CDATA[tra]]>:Thing",
        ] {
            different(
                &left.replace("Probe", text),
                &left
                    .replace("Probe", text)
                    .replace("urn:one", "urn:changed"),
            );
        }
    }

    #[test]
    fn foreign_namespaces_and_used_binding_changes_are_never_erased() {
        let source = control("InputField", "<ReadOnly>true</ReadOnly>");
        different(
            &source,
            &source.replace("<ReadOnly>", "<ReadOnly xmlns=\"urn:foreign\">"),
        );
        let qualified = control("InputField", "<v8:Unknown>one</v8:Unknown>");
        different(
            &qualified,
            &qualified.replace("http://v8.1c.ru/8.1/data/core", "urn:foreign"),
        );
        let qname_value = control("InputField", "<Unknown>cfg:Thing</Unknown>");
        different(
            &qname_value,
            &qname_value.replace(
                "http://v8.1c.ru/8.1/data/enterprise/current-config",
                "urn:foreign",
            ),
        );
    }

    #[test]
    fn unknown_attributes_children_ids_scalar_whitespace_and_entities_remain_strict() {
        let source = control(
            "InputField",
            "<DataPath>Amount</DataPath><Unknown value=\"1\">one</Unknown>",
        );
        for changed in [
            source.replace("id=\"7\"", "id=\"9\""),
            source.replace("value=\"1\"", "value=\"2\""),
            source.replace("<Unknown value=\"1\">one</Unknown>", ""),
            source.replace("<DataPath>Amount", "<DataPath> Amount"),
            source.replace("one</Unknown>", "two</Unknown>"),
            source.replace("one</Unknown>", "one&#49;</Unknown>"),
            source.replace("one</Unknown>", "one&missing;</Unknown>"),
        ] {
            different(&source, &changed);
        }
        let mixed = control("InputField", "<Unknown>a<Part/>b</Unknown>");
        different(&mixed, &mixed.replace("a<Part/>b", "ab<Part/>"));
        different(
            &control("UsualGroup", ""),
            &control(
                "UsualGroup",
                "<GroupHorizontalAlign>If</GroupHorizontalAlign>",
            ),
        );
        different(
            &control("Button", ""),
            &control("Button", "<ToolTip><v8:item/></ToolTip>"),
        );
    }

    #[test]
    fn document_shell_case_and_empty_leaf_spelling_do_not_change_values() {
        let source = form("<Unknown/><ChildItems/>");
        equal(
            &format!("\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n{source}"),
            &format!(
                "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n{}",
                source.replace("<Unknown/>", "<Unknown></Unknown>")
            ),
        );
        different(&source, &source.replace("<Unknown/>", ""));
    }

    #[test]
    fn namespace_invalid_and_expanded_duplicate_attributes_fail_closed() {
        for invalid in [
            form("<Unknown/>").replace(" version=", " xmlns:empty=\"\" version="),
            form("<Unknown/>").replace(
                " version=",
                " xmlns:alias=\"http://www.w3.org/XML/1998/namespace\" version=",
            ),
            form("<Unknown/>").replace(
                " version=",
                " xmlns:alias=\"http://www.w3.org/2000/xmlns/\" version=",
            ),
            form("<ChildItems><InputField missing:flag=\"1\"/></ChildItems>"),
            form("<ChildItems><missing:InputField/></ChildItems>"),
            form(
                "<ChildItems><InputField xmlns:a=\"urn:one\" xmlns:b=\"urn:one\" a:flag=\"1\" b:flag=\"2\"/></ChildItems>",
            ),
        ] {
            different(&invalid, &invalid);
        }
    }

    #[test]
    fn preserve_space_and_foreign_or_unproven_container_paths_disable_defaults() {
        let absent = control("CheckBoxField", "");
        let explicit = control("CheckBoxField", "<CheckBoxType>Auto</CheckBoxType>");
        for (before, after) in [
            ("<ChildItems>", "<ChildItems xml:space=\"preserve\">"),
            ("<ChildItems>", "<ChildItems xmlns=\"urn:foreign\">"),
        ] {
            different(
                &absent.replace(before, after),
                &explicit.replace(before, after),
            );
        }
        let wrap = |inner: &str| {
            form(&format!(
                "<ChildItems><InputField name=\"Outer\"><ChildItems>{inner}</ChildItems></InputField></ChildItems>"
            ))
        };
        different(
            &wrap("<CheckBoxField/>"),
            &wrap("<CheckBoxField><CheckBoxType>Auto</CheckBoxType></CheckBoxField>"),
        );
        different(
            &control("InputField", "<DataPath>A</DataPath><Unknown/>"),
            &control("InputField", "<Unknown/><DataPath>A</DataPath>"),
        );
    }

    #[test]
    fn stricter_binding_and_main_attribute_ambiguities_do_not_enable_defaults() {
        for attribute in [
            rows_attribute().replace("<Type>", "<Type extra=\"1\">"),
            rows_attribute().replace("<v8:Type>", "<v8:Type extra=\"1\">"),
            rows_attribute().replace("<Type><v8:Type>v8:ValueTable</v8:Type></Type>", "<Type><v8:Type>v8:ValueTable</v8:Type></Type><Type><v8:Type>v8:ValueTable</v8:Type></Type>"),
        ] {
            different(&value_table("", &attribute), &value_table("<RowFilter xsi:nil=\"true\"/>", &attribute));
        }
        let absent = value_table("", rows_attribute());
        let nil = value_table("<RowFilter xsi:nil=\"true\"/>", rows_attribute());
        different(
            &absent.replace("<DataPath>", "<DataPath extra=\"1\">"),
            &nil.replace("<DataPath>", "<DataPath extra=\"1\">"),
        );
        different(
            &absent,
            &value_table("<RowFilter xsi:nil=\"1\"/>", rows_attribute()),
        );
        let ambiguous = format!(
            "{}{}",
            document_attribute(),
            document_attribute()
                .replace("name=\"Object\"", "name=\"Other\"")
                .replace("cfg:DocumentObject.Probe", "cfg:CatalogObject.Other")
        );
        different(
            &document_form("", &ambiguous),
            &document_form("<AutoTime>CurrentOrLast</AutoTime>", &ambiguous),
        );
    }

    #[test]
    fn root_ordinary_button_uses_only_the_native_usual_button_default() {
        equal(
            &control("Button", ""),
            &control("Button", "<Type>UsualButton</Type>"),
        );
        for property in [
            "<Type>CommandBarButton</Type>",
            "<Type extra=\"1\">UsualButton</Type>",
            "<Type>UsualButton</Type><Type>UsualButton</Type>",
        ] {
            different(&control("Button", ""), &control("Button", property));
        }
        let wrap = |property: &str| {
            control(
                "UsualGroup",
                &format!("<ChildItems><Button name=\"Nested\">{property}</Button></ChildItems>"),
            )
        };
        different(&wrap(""), &wrap("<Type>UsualButton</Type>"));
    }
    #[test]
    fn uncertain_namespace_proof_retains_cdata_pi_and_every_binding() {
        let cdata = form("<Unknown><![CDATA[a & b]]></Unknown>");
        equal(&cdata, &cdata);
        different(&cdata, &cdata.replace("a & b", "a & c"));
        let pi = form("<?probe value='a & b'?><Unknown/>");
        equal(&pi, &pi);
        let declared = pi.replace(" version=", " xmlns:extra=\"urn:one\" version=");
        different(&declared, &declared.replace("urn:one", "urn:two"));
        different(&pi, &pi.replace("a & b", "a & c"));
    }

    #[test]
    fn mixed_control_content_never_enables_default_elision() {
        different(
            &control("CheckBoxField", "left"),
            &control("CheckBoxField", "left<CheckBoxType>Auto</CheckBoxType>"),
        );
    }
    #[test]
    fn unused_namespace_proof_handles_bom_without_changing_each_policy_shell_contract() {
        let left =
            form("<Title>Probe</Title>").replace(" version=", " xmlns:left=\"urn:one\" version=");
        let right =
            form("<Title>Probe</Title>").replace(" version=", " xmlns:right=\"urn:two\" version=");
        equal(&format!("\u{feff}{left}"), &format!("\u{feff}{right}"));

        // Form shell spelling is neutral; the existing DCS policy retains
        // its lexical BOM contract even through namespace normalization.
        equal(&format!("\u{feff}{left}"), &right);
        assert!(crate::equal_dcs_source_documents(
            format!("\u{feff}{left}").as_bytes(),
            format!("\u{feff}{right}").as_bytes(),
        ));
        assert!(!crate::equal_dcs_source_documents(
            format!("\u{feff}{left}").as_bytes(),
            right.as_bytes(),
        ));
        different(
            &format!("\u{feff}{left}"),
            &format!("\u{feff}{}", right.replace("Probe", "Changed")),
        );
    }

    #[test]
    fn input_field_auto_max_height_is_a_known_property_without_ignoring_its_value() {
        let authored = control(
            "InputField",
            "<AutoMaxWidth>true</AutoMaxWidth><Width>10</Width><AutoMaxHeight>true</AutoMaxHeight>",
        );
        let native_order = control(
            "InputField",
            "<Width>10</Width><AutoMaxHeight>true</AutoMaxHeight><AutoMaxWidth>true</AutoMaxWidth>",
        );
        equal(&authored, &native_order);
        different(
            &authored,
            &native_order.replace("<AutoMaxHeight>true", "<AutoMaxHeight>false"),
        );
        different(&authored, &native_order.replace("<Width>10", "<Width>11"));
    }

    #[test]
    fn duplicate_expanded_property_aliases_preserve_their_full_order() {
        let wrapped = |properties: &str| {
            control("InputField", properties).replace(
                " version=",
                " xmlns:p=\"http://v8.1c.ru/8.3/xcf/logform\" xmlns:q=\"http://v8.1c.ru/8.3/xcf/logform\" version=",
            )
        };
        let first = wrapped("<p:ReadOnly>true</p:ReadOnly><q:ReadOnly>false</q:ReadOnly>");
        let reversed = wrapped("<q:ReadOnly>false</q:ReadOnly><p:ReadOnly>true</p:ReadOnly>");
        different(&first, &reversed);
    }

    #[test]
    fn usual_group_horizontal_if_possible_is_refused_even_for_identical_xml() {
        let unsupported = control("UsualGroup", "<Group>HorizontalIfPossible</Group>");
        different(&unsupported, &unsupported);
        different(
            &unsupported,
            &unsupported.replace("<ChildItems>", "<ChildItems>\n\t"),
        );
        let nested = control(
            "UsualGroup",
            "<ChildItems><UsualGroup name=\"Nested\" id=\"10\"><Group>HorizontalIfPossible</Group></UsualGroup></ChildItems>",
        );
        different(&nested, &nested);
        let qualified = unsupported
            .replace(
                "<UsualGroup ",
                "<f:UsualGroup xmlns:f=\"http://v8.1c.ru/8.3/xcf/logform\" ",
            )
            .replace("</UsualGroup>", "</f:UsualGroup>");
        different(&qualified, &qualified);
    }

    #[test]
    fn horizontal_if_possible_refusal_is_scoped_to_the_known_usual_group_domain() {
        for supported in [
            control("Page", "<Group>HorizontalIfPossible</Group>"),
            form("<Group>HorizontalIfPossible</Group><ChildItems/>"),
            control(
                "UsualGroup",
                "<Group xmlns=\"urn:foreign\">HorizontalIfPossible</Group>",
            ),
            form("<Unknown><UsualGroup><Group>HorizontalIfPossible</Group></UsualGroup></Unknown>"),
            control("UsualGroup", "<Group>Vertical</Group>"),
            control("UsualGroup", "<Group>Horizontal</Group>"),
            control("UsualGroup", "<Group>AlwaysHorizontal</Group>"),
        ] {
            equal(&supported, &supported);
        }
    }

    #[test]
    fn empty_root_command_bar_literal_indentation_is_neutral() {
        let wrapped = |body: &str| {
            form(&format!(
                "<ChildItems><CommandBar name=\"Actions\" id=\"7\">{body}</CommandBar></ChildItems>"
            ))
        };
        let authored = wrapped("\n\t\t");
        equal(&authored, &wrapped("\r\n\t\t"));
        equal(&authored, &wrapped(""));
        equal(
            &authored,
            &form("<ChildItems><CommandBar name=\"Actions\" id=\"7\"/></ChildItems>"),
        );
    }

    #[test]
    fn empty_command_bar_indentation_neutrality_is_scoped_to_root_ui_owner() {
        let wrapped = |bar: &str| form(&format!("<ChildItems>{bar}</ChildItems>"));
        for bar in [
            "<CommandBar xmlns=\"urn:foreign\" name=\"Actions\" id=\"7\">\n\t</CommandBar>",
            "<CommandBar name=\"Actions\" id=\"7\" custom=\"true\">\n\t</CommandBar>",
            "<CommandBar name=\"Actions\" id=\"7\" xml:space=\"preserve\">\n\t</CommandBar>",
            "<InputField name=\"Actions\" id=\"7\">\n\t</InputField>",
            "<Unknown><CommandBar name=\"Actions\" id=\"7\">\n\t</CommandBar></Unknown>",
            "<UsualGroup name=\"Group\" id=\"2\"><ChildItems><CommandBar name=\"Actions\" id=\"7\">\n\t</CommandBar></ChildItems></UsualGroup>",
        ] {
            different(&wrapped(bar), &wrapped(&bar.replace('\n', "\r\n")));
        }
        let ordinary = wrapped("<CommandBar name=\"Actions\" id=\"7\">\n\t</CommandBar>");
        let preserved = ordinary.replace("<ChildItems>", "<ChildItems xml:space=\"preserve\">");
        different(&preserved, &preserved.replace('\n', "\r\n"));
        let preserved = ordinary.replace(" version=", " xml:space=\"preserve\" version=");
        different(&preserved, &preserved.replace('\n', "\r\n"));
        different(&ordinary, &ordinary.replace("id=\"7\"", "id=\"8\""));
        different(
            &ordinary,
            &ordinary.replace("name=\"Actions\"", "name=\"Changed\""),
        );
        different(&ordinary, &form("<ChildItems/>"));
        let scalar = form("<Title>\n\t</Title>");
        different(&scalar, &scalar.replace('\n', "\r\n"));
    }

    #[test]
    fn empty_root_command_bar_keeps_nonliteral_and_semantic_content_strict() {
        let wrapped = |body: &str| {
            form(&format!(
                "<ChildItems><CommandBar name=\"Actions\" id=\"7\">{body}</CommandBar></ChildItems>"
            ))
        };
        let empty = wrapped("");
        for content in [
            "&#10;\t",
            "&#13;\n\t",
            "<![CDATA[\n\t]]>",
            "<!--spacing-->",
            "<?spacing probe?>",
            "value",
            "<Unknown/>",
            "<Title>\n\t</Title>",
        ] {
            different(&empty, &wrapped(content));
        }
        let scalar = wrapped("<Title>\n\t</Title>");
        different(&scalar, &scalar.replace('\n', "\r\n"));
    }
}
