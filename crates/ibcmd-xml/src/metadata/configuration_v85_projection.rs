//! Strict admission of the XML 2.21 projection of a 67/68 Configuration.

use std::collections::BTreeSet;

use ibcmd_schema::{ConfigurationPropertyEvidencedDefault, configuration_v85_projection};

use super::MetadataDecodeError;
use super::common::{MD_NAMESPACE, resolve_namespaces, typed, uri_of};
use crate::{AttributeKind, XmlDocument, XmlElement, XmlNode};

fn invalid(message: &'static str) -> MetadataDecodeError {
    MetadataDecodeError::InvalidEnvelope(message)
}

fn elements(element: &XmlElement) -> Result<Vec<&XmlElement>, MetadataDecodeError> {
    let mut result = Vec::new();
    for node in element.children() {
        match node {
            XmlNode::Element(child) => result.push(child),
            XmlNode::Text(text) if text.value().trim().is_empty() => {}
            XmlNode::Comment(_) => {}
            _ => return Err(invalid("Configuration projection container content")),
        }
    }
    Ok(result)
}

fn no_value_attributes(element: &XmlElement) -> Result<(), MetadataDecodeError> {
    if element
        .attributes()
        .iter()
        .any(|attribute| matches!(attribute.kind(), AttributeKind::Ordinary(_)))
    {
        return Err(invalid(
            "Configuration 8.5 older-layout property attributes",
        ));
    }
    Ok(())
}

/// Validates only the fourteen properties native XML 2.21 adds to a proven
/// older Configuration layout. Callers must choose this rule from their actual
/// output layout, never from the XML edition alone. The separate 76 codec must
/// retain its own values. No named non-default value can be discarded.
pub fn validate_older_configuration_v85_defaults(
    document: &XmlDocument,
    source_dialect: &str,
) -> Result<(), MetadataDecodeError> {
    let root = document.root();
    let uris = resolve_namespaces(root)?;
    let md = uri_of(root, &uris);
    if root.name().local() != "MetaDataObject" || !matches!(md, None | Some(MD_NAMESPACE)) {
        return Err(invalid(
            "Configuration older-layout metadata envelope namespace",
        ));
    }
    let owners = elements(root)?;
    if owners.len() != 1 || !typed(owners[0], "Configuration", md, &uris) {
        return Err(invalid("Configuration older-layout projection owner"));
    }
    let mut properties = None;
    for child in elements(owners[0])? {
        if child.name().local() == "Properties" {
            if !typed(child, "Properties", md, &uris) {
                return Err(invalid("Configuration older-layout Properties namespace"));
            }
            if properties.replace(child).is_some() {
                return Err(MetadataDecodeError::Duplicate("Configuration Properties"));
            }
        }
    }
    let properties = properties.ok_or(MetadataDecodeError::Missing("Configuration Properties"))?;
    let mut seen = BTreeSet::new();
    for element in elements(properties)? {
        let name = element.name().local();
        let Some(expected) = configuration_v85_projection::default_property(name) else {
            continue;
        };
        let actual_dialect = root.attributes().iter().find_map(|attribute| {
            matches!(attribute.kind(), AttributeKind::Ordinary(name) if name.raw() == "version")
                .then_some(attribute.value())
        });
        if !configuration_v85_projection::supports_xml_dialect(source_dialect)
            || actual_dialect != Some(source_dialect)
        {
            return Err(invalid(
                "Configuration 8.5 older-layout properties require XML 2.21",
            ));
        }
        if !typed(element, name, md, &uris) {
            return Err(invalid("Configuration 8.5 older-layout property namespace"));
        }
        if !seen.insert(name) {
            return Err(MetadataDecodeError::Duplicate(
                "Configuration 8.5 older-layout property",
            ));
        }
        no_value_attributes(properties)?;
        no_value_attributes(element)?;
        let mut text = String::new();
        for node in element.children() {
            match node {
                XmlNode::Text(value) => text.push_str(value.value()),
                XmlNode::CData(value) => text.push_str(value.value()),
                XmlNode::Comment(_) => {}
                _ => {
                    return Err(invalid(
                        "Configuration 8.5 older-layout property is not a simple value",
                    ));
                }
            }
        }
        let expected = match expected {
            ConfigurationPropertyEvidencedDefault::Empty => "",
            ConfigurationPropertyEvidencedDefault::Text(value) => value,
            ConfigurationPropertyEvidencedDefault::Block(_) => {
                return Err(invalid(
                    "Configuration older-layout projection is not a scalar",
                ));
            }
        };
        if text.trim() != expected {
            return Err(invalid(
                "Configuration 8.5 property cannot be retained in an older layout",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::XmlReader;

    fn document(properties: &str) -> XmlDocument {
        XmlReader::from_slice(format!("<MetaDataObject xmlns=\"{MD_NAMESPACE}\" version=\"2.21\"><Configuration><Properties><Name>Own</Name>{properties}</Properties></Configuration></MetaDataObject>").as_bytes()).unwrap()
    }

    #[test]
    fn older_configuration_v85_defaults_validate_expanded_names_and_exact_values() {
        let properties = configuration_v85_projection::DEFAULT_PROPERTIES
            .iter()
            .map(|(name, value)| match value {
                ConfigurationPropertyEvidencedDefault::Empty => format!("<{name}/>"),
                ConfigurationPropertyEvidencedDefault::Text(text) => {
                    format!("<{name}>{text}</{name}>")
                }
                _ => unreachable!(),
            })
            .collect::<String>();
        assert!(validate_older_configuration_v85_defaults(&document(&properties), "2.21").is_ok());
        let alias = properties.replace(
            "<Caption/>",
            &format!("<m:Caption xmlns:m=\"{MD_NAMESPACE}\"/>"),
        );
        assert!(validate_older_configuration_v85_defaults(&document(&alias), "2.21").is_ok());
        for malformed in [
            "<AuxiliaryReportForm>CommonForm.Authored</AuxiliaryReportForm>",
            "<Caption><v8:item xmlns:v8=\"http://v8.1c.ru/8.1/data/core\"/></Caption>",
            "<AuxiliaryReportForm xmlns=\"urn:foreign\"/>",
            "<AuxiliaryReportForm future=\"1\"/>",
            "<AuxiliaryReportForm/><AuxiliaryReportForm/>",
            "<ClientApplicationTheme/>",
            "<Version85InterfaceMigrationMode>Use</Version85InterfaceMigrationMode>",
        ] {
            assert!(
                validate_older_configuration_v85_defaults(&document(malformed), "2.21").is_err(),
                "{malformed}"
            );
        }
        assert!(validate_older_configuration_v85_defaults(&document(&properties), "2.20").is_err());
    }
}
