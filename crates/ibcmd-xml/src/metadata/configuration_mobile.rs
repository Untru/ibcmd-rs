//! Actual Configuration mobile flags, before compact metadata loses namespaces.

use super::MetadataDecodeError;
use super::common::{MD_NAMESPACE, resolve_namespaces, typed, uri_of};
use crate::{AttributeKind, XmlDocument, XmlElement, XmlNode};

const PROPERTY: &str = "UsedMobileApplicationFunctionalities";
const APP_NAMESPACE: &str = "http://v8.1c.ru/8.2/managed-application/core";

fn invalid(reason: &'static str) -> MetadataDecodeError {
    MetadataDecodeError::InvalidEnvelope(reason)
}

fn elements(element: &XmlElement) -> Result<Vec<&XmlElement>, MetadataDecodeError> {
    let mut children = Vec::new();
    for node in element.children() {
        match node {
            XmlNode::Element(child) => children.push(child),
            XmlNode::Text(text) if text.value().trim().is_empty() => {}
            XmlNode::Comment(_) => {}
            _ => {
                return Err(invalid(
                    "mobile functionality container has unexpected content",
                ));
            }
        }
    }
    Ok(children)
}

fn no_value_attributes(element: &XmlElement) -> Result<(), MetadataDecodeError> {
    if element
        .attributes()
        .iter()
        .any(|attribute| matches!(attribute.kind(), AttributeKind::Ordinary(_)))
    {
        return Err(invalid(
            "mobile functionality value has unexpected attributes",
        ));
    }
    Ok(())
}

fn text(element: &XmlElement) -> Result<String, MetadataDecodeError> {
    no_value_attributes(element)?;
    let mut value = String::new();
    for node in element.children() {
        match node {
            XmlNode::Text(text) => value.push_str(text.value()),
            XmlNode::CData(text) => value.push_str(text.value()),
            XmlNode::Comment(_) => {}
            _ => return Err(invalid("mobile functionality leaf is not a simple value")),
        }
    }
    Ok(value.trim().to_owned())
}

/// Returns the IDs whose *own XML* use flag is true. Absence of the whole
/// property retains the existing minimal Configuration contract. A present
/// block must carry the complete ordered roster of its measured source dialect;
/// missing or malformed flags never become factory defaults. Prefix spelling
/// is irrelevant, but every expanded entry/leaf name must have the same APP URI.
/// Wholly unqualified legacy metadata also admits wholly unqualified values.
pub fn parse_configuration_mobile_functionalities(
    document: &XmlDocument,
) -> Result<Option<Vec<u32>>, MetadataDecodeError> {
    let root = document.root();
    let uris = resolve_namespaces(root)?;
    let md = uri_of(root, &uris);
    if !matches!(md, None | Some(MD_NAMESPACE)) || root.name().local() != "MetaDataObject" {
        return Err(invalid("mobile functionality metadata envelope namespace"));
    }
    let roots = elements(root)?;
    if roots.len() != 1 || !typed(roots[0], "Configuration", md, &uris) {
        return Err(invalid(
            "mobile functionality owner is not one Configuration",
        ));
    }
    let mut properties = None;
    for child in elements(roots[0])? {
        if child.name().local() == "Properties" {
            if !typed(child, "Properties", md, &uris) {
                return Err(invalid("mobile functionality Properties namespace"));
            }
            if properties.replace(child).is_some() {
                return Err(MetadataDecodeError::Duplicate("Configuration Properties"));
            }
        }
    }
    let properties = properties.ok_or(MetadataDecodeError::Missing("Configuration Properties"))?;
    let mut block = None;
    for child in elements(properties)? {
        if child.name().local() == PROPERTY {
            if !typed(child, PROPERTY, md, &uris) {
                return Err(invalid("mobile functionality property namespace"));
            }
            if block.replace(child).is_some() {
                return Err(MetadataDecodeError::Duplicate(PROPERTY));
            }
        }
    }
    let Some(block) = block else { return Ok(None) };
    no_value_attributes(block)?;
    let version = root.attributes().iter().find_map(|attribute| {
        matches!(attribute.kind(), AttributeKind::Ordinary(name) if name.raw() == "version")
            .then_some(attribute.value())
    });
    let roster = ibcmd_schema::configuration_mobile::source_functionalities(version)
        .ok_or_else(|| invalid("mobile functionality source dialect is not measured"))?;
    let entries = elements(block)?;
    if entries.len() != roster.len() {
        return Err(invalid(
            "mobile functionality block is not the complete dialect roster",
        ));
    }
    let app = uri_of(entries[0], &uris);
    if app != Some(APP_NAMESPACE) && !(md.is_none() && app.is_none()) {
        return Err(invalid("mobile functionality application namespace"));
    }
    let mut enabled = Vec::new();
    for (entry, (id, expected_name)) in entries.into_iter().zip(roster) {
        if !typed(entry, "functionality", app, &uris) {
            return Err(invalid("mobile functionality entry expanded name"));
        }
        no_value_attributes(entry)?;
        let leaves = elements(entry)?;
        if leaves.len() != 2
            || !typed(leaves[0], "functionality", app, &uris)
            || !typed(leaves[1], "use", app, &uris)
        {
            return Err(invalid(
                "mobile functionality entry requires one name then one use",
            ));
        }
        if text(leaves[0])? != *expected_name {
            return Err(invalid(
                "mobile functionality name is unknown, duplicated or out of order",
            ));
        }
        match text(leaves[1])?.as_str() {
            "true" => enabled.push(*id),
            "false" => {}
            _ => return Err(invalid("mobile functionality use is not true or false")),
        }
    }
    Ok(Some(enabled))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::XmlReader;

    fn entry(name: &str, flag: &str) -> String {
        format!(
            "<a:functionality><a:functionality>{name}</a:functionality><a:use>{flag}</a:use></a:functionality>"
        )
    }
    fn entries(version: &str, enabled: &[u32]) -> String {
        ibcmd_schema::configuration_mobile::source_functionalities(Some(version))
            .unwrap()
            .iter()
            .map(|(id, name)| {
                entry(
                    name,
                    if enabled.contains(id) {
                        "true"
                    } else {
                        "false"
                    },
                )
            })
            .collect()
    }
    fn document(version: &str, entries: &str) -> String {
        format!(
            r#"<m:MetaDataObject xmlns:m="{MD_NAMESPACE}" xmlns:a="{APP_NAMESPACE}" version="{version}"><m:Configuration><m:Properties><m:UsedMobileApplicationFunctionalities>{entries}</m:UsedMobileApplicationFunctionalities></m:Properties></m:Configuration></m:MetaDataObject>"#
        )
    }
    fn parse(xml: &str) -> Result<Option<Vec<u32>>, MetadataDecodeError> {
        parse_configuration_mobile_functionalities(&XmlReader::from_slice(xml.as_bytes()).unwrap())
    }
    #[test]
    fn own_flags_and_older_roster_project_without_factory_substitution() {
        for version in ["2.17", "2.20", "2.21"] {
            let all = ibcmd_schema::configuration_mobile::source_functionalities(Some(version))
                .unwrap()
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>();
            for enabled in [vec![], vec![1, 18, 35], all] {
                assert_eq!(
                    parse(&document(version, &entries(version, &enabled))).unwrap(),
                    Some(enabled)
                );
            }
        }
        assert_eq!(
            parse("<MetaDataObject><Configuration><Properties/></Configuration></MetaDataObject>")
                .unwrap(),
            None
        );
    }
    #[test]
    fn namespace_aliases_local_declarations_and_legacy_unqualified_values() {
        let xml = document("2.20", &entries("2.20", &[1, 41]));
        let alias = xml
            .replace("a:", "alternate:")
            .replace("xmlns:a=", "xmlns:alternate=");
        assert_eq!(parse(&alias).unwrap(), Some(vec![1, 41]));
        let local = xml
            .replace(&format!(r#" xmlns:a="{APP_NAMESPACE}""#), "")
            .replace(
                "<a:functionality>",
                &format!(r#"<a:functionality xmlns:a="{APP_NAMESPACE}">"#),
            );
        assert_eq!(parse(&local).unwrap(), Some(vec![1, 41]));
        let legacy = xml
            .replace("m:", "")
            .replace("a:", "")
            .replace(&format!(r#" xmlns:m="{MD_NAMESPACE}""#), "")
            .replace(&format!(r#" xmlns:a="{APP_NAMESPACE}""#), "");
        assert_eq!(parse(&legacy).unwrap(), Some(vec![1, 41]));
    }
    #[test]
    fn malformed_present_values_are_never_dropped_or_defaulted() {
        let body = entries("2.20", &[1, 41]);
        let first = entry("Biometrics", "false");
        let second = entry("Location", "true");
        let xml = document("2.20", &body);
        let mutations = [
            document("2.20", &body.replacen(&first, "", 1)),
            document("2.20", &body.replacen(&second, &first, 1)),
            document("2.20", &body.replacen(&(first.clone() + &second), &(second + &first), 1)),
            xml.replacen("Biometrics", "Future", 1),
            xml.replacen("<a:use>false</a:use>", "<a:use>1</a:use>", 1),
            xml.replacen("<a:use>false</a:use>", "<a:use/>", 1),
            xml.replacen("<a:use>false</a:use>", "<a:use><a:value>false</a:value></a:use>", 1),
            xml.replacen("<a:use>false</a:use>", "<a:use>false</a:use><a:use>false</a:use>", 1),
            xml.replacen("<a:use>", "<a:use unexpected=\"yes\">", 1),
            xml.replacen("<a:functionality>", "<a:functionality>not whitespace", 1),
            xml.replacen("<a:use>", "<a:use xmlns:a=\"urn:foreign\">", 1),
            xml.replace(APP_NAMESPACE, "urn:foreign"),
            xml.replacen("<m:UsedMobileApplicationFunctionalities>", "<m:UsedMobileApplicationFunctionalities xmlns:m=\"urn:foreign\">", 1),
            xml.replace("</m:Properties>", &format!("<m:UsedMobileApplicationFunctionalities>{body}</m:UsedMobileApplicationFunctionalities></m:Properties>")),
            xml.replace("</m:UsedMobileApplicationFunctionalities>", "<a:permissionMessage/></m:UsedMobileApplicationFunctionalities>"),
            document("2.17", &body),
            document("2.21", &entries("2.17", &[1])),
            document("2.18", &body),
        ];
        for mutated in mutations {
            assert!(
                parse(&mutated).is_err(),
                "admitted malformed mobile block: {mutated}"
            );
        }
    }
}
