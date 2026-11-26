//! Source-input equivalence for measured Designer form defaults. Only the
//! expected input is completed; a writer's missing or incorrect value fails.
use crate::source_comparison::documents_equal;
use crate::{AttributeKind, QName, XmlElement, XmlNode, XmlReader};

fn elements(e: &XmlElement) -> impl Iterator<Item = &XmlElement> {
    e.children().iter().filter_map(|n| {
        if let XmlNode::Element(e) = n {
            Some(e)
        } else {
            None
        }
    })
}
fn attr<'a>(e: &'a XmlElement, key: &str) -> Option<&'a str> {
    e.attributes().iter().find_map(|a| match a.kind() {
        AttributeKind::Ordinary(n) if n.raw() == key => Some(a.value()),
        _ => None,
    })
}
fn scalar<'a>(e: &'a XmlElement, name: &str) -> Option<&'a str> {
    let mut found = elements(e).filter(|n| n.name().raw() == name);
    let e = found.next()?;
    if found.next().is_some() || !e.attributes().is_empty() {
        return None;
    }
    match e.children() {
        [XmlNode::Text(t)] => Some(t.value()),
        _ => None,
    }
}
fn counterpart<'a>(e: &XmlElement, actual: Option<&'a XmlElement>) -> Option<&'a XmlElement> {
    elements(actual?).find(|candidate| {
        candidate.name() == e.name()
            && attr(candidate, "id") == attr(e, "id")
            && attr(candidate, "name") == attr(e, "name")
    })
}
fn complete(children: &mut Vec<XmlNode>, actual: Option<&XmlElement>, name: &str, value: &str) {
    if children
        .iter()
        .any(|n| matches!(n,XmlNode::Element(e) if e.name().raw()==name))
    {
        return;
    }
    // Only the position comes from the canonical rendering; its scalar value
    // is never trusted as an input default.
    let following = actual.and_then(|a| {
        let mut fields = elements(a).skip_while(|e| e.name().raw() != name);
        fields.next()?;
        fields.next().map(|e| e.name().raw())
    });
    let at = following
        .and_then(|next| {
            children
                .iter()
                .position(|n| matches!(n,XmlNode::Element(e) if e.name().raw()==next))
        })
        .unwrap_or(children.len());
    children.insert(
        at,
        XmlNode::Element(XmlElement::with_parts(
            QName::new(name).expect("static source property name"),
            vec![],
            vec![XmlNode::text(value)],
        )),
    );
}
fn normalized(
    expected: &XmlElement,
    actual: Option<&XmlElement>,
    edition: &str,
    body: bool,
    descriptor_form: bool,
    parent: &str,
    depth: usize,
) -> XmlElement {
    let mut children = expected
        .children()
        .iter()
        .map(|n| match n {
            XmlNode::Element(e) => XmlNode::Element(normalized(
                e,
                counterpart(e, actual),
                edition,
                body,
                descriptor_form,
                expected.name().raw(),
                depth + 1,
            )),
            _ => n.clone(),
        })
        .collect::<Vec<_>>();
    if body && parent == "ChildItems" && expected.name().raw() == "Button" {
        complete(
            &mut children,
            actual,
            "Type",
            ibcmd_schema::form_source_defaults::DESIGNER_BUTTON_TYPE,
        );
    }
    if body
        && depth == 0
        && let Some(value) = scalar(expected, "CommandBarLocation")
            .and_then(|l| ibcmd_schema::form_source_defaults::derived_show_command_bar(edition, l))
    {
        complete(&mut children, actual, "ShowCommandBar", value);
    }
    if descriptor_form
        && depth == 2
        && parent == "Form"
        && expected.name().raw() == "Properties"
        && ibcmd_schema::external_named::ExternalNamedKind::Form
            .property_available("UseInInterfaceCompatibilityMode", edition)
    {
        complete(
            &mut children,
            actual,
            "UseInInterfaceCompatibilityMode",
            ibcmd_schema::external_named::FORM_INTERFACE_COMPATIBILITY_DEFAULT,
        );
    }
    expected.with_children(children)
}
fn namespace(e: &XmlElement) -> Option<&str> {
    e.attributes().iter().find_map(|a| match a.kind() {
        AttributeKind::Namespace(None) => Some(a.value()),
        _ => None,
    })
}
fn compare(expected: &[u8], actual: &[u8], body: bool) -> bool {
    let (Ok(expected), Ok(actual)) = (
        XmlReader::from_slice(expected),
        XmlReader::from_slice(actual),
    ) else {
        return false;
    };
    let root = expected.root();
    let edition = attr(root, "version").unwrap_or("");
    let valid = if body {
        root.name().raw() == "Form" && namespace(root) == Some("http://v8.1c.ru/8.3/xcf/logform")
    } else {
        root.name().raw() == "MetaDataObject"
            && namespace(root) == Some("http://v8.1c.ru/8.3/MDClasses")
    };
    if !valid || !matches!(edition, "2.20" | "2.21") {
        return false;
    }
    let descriptor_form = !body
        && elements(root).count() == 1
        && elements(root)
            .next()
            .is_some_and(|e| e.name().raw() == "Form");
    let completed = normalized(
        root,
        Some(actual.root()),
        edition,
        body,
        descriptor_form,
        "",
        0,
    );
    documents_equal(&expected.with_root(completed), &actual)
}
/// Full form-body input equivalence after completing only measured omitted
/// Designer values. Explicit non-defaults and every unknown property remain.
pub fn form_source_input_equal(expected: &[u8], actual: &[u8]) -> bool {
    compare(expected, actual, true)
}
/// Metadata descriptor equivalence; the 2.21 Form compatibility mode is the
/// only completed property. Opaque XML resources never use this comparison.
pub fn metadata_source_input_equal(expected: &[u8], actual: &[u8]) -> bool {
    compare(expected, actual, false)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_source_omissions_require_the_exact_rendered_values() {
        let wrap = |body: &str| {
            format!("<Form xmlns='http://v8.1c.ru/8.3/xcf/logform' version='2.21'>{body}</Form>")
        };
        let source = wrap(
            "<CommandBarLocation>None</CommandBarLocation><ChildItems><Button name='B' id='1'><CommandName>C</CommandName></Button></ChildItems>",
        );
        let rendered = wrap(
            "<CommandBarLocation>None</CommandBarLocation><ShowCommandBar>false</ShowCommandBar><ChildItems><Button name='B' id='1'><Type>UsualButton</Type><CommandName>C</CommandName></Button></ChildItems>",
        );
        assert!(form_source_input_equal(
            source.as_bytes(),
            rendered.as_bytes()
        ));
        assert!(!form_source_input_equal(
            source.as_bytes(),
            rendered
                .replace("UsualButton", "CommandBarButton")
                .as_bytes()
        ));
        assert!(!form_source_input_equal(
            source.as_bytes(),
            rendered.replace(">false<", ">true<").as_bytes()
        ));
        assert!(!form_source_input_equal(
            source.as_bytes(),
            source.as_bytes()
        ));
        assert!(!form_source_input_equal(
            source.as_bytes(),
            rendered
                .replace(
                    "<CommandName>C</CommandName>",
                    "<CommandName>C</CommandName><Unknown>1</Unknown>"
                )
                .as_bytes()
        ));
        let source = "<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' version='2.21'><Form><Properties><Name>F</Name></Properties></Form></MetaDataObject>";
        let rendered = source.replace(
            "</Properties>",
            "<UseInInterfaceCompatibilityMode>Any</UseInInterfaceCompatibilityMode></Properties>",
        );
        assert!(metadata_source_input_equal(
            source.as_bytes(),
            rendered.as_bytes()
        ));
        assert!(!metadata_source_input_equal(
            source.as_bytes(),
            rendered.replace(">Any<", ">Other<").as_bytes()
        ));
    }
}
