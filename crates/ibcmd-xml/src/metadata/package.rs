//! Package roots are selected by expanded XML names, never by archive suffix.

use super::MetadataDecodeError;
use super::common::{MD_NAMESPACE, resolve_namespaces, uri_of};
use crate::{XmlDocument, XmlNode};

/// The artifact represented by a source metadata root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageIntent {
    Configuration,
    Extension,
    ExternalDataProcessor,
    ExternalReport,
}

/// Inspects one complete source document using the canonical namespace resolver.
/// Non-package metadata and export manifests return `None`. This only selects
/// the artifact; its compiler remains responsible for full object validation.
/// Unqualified legacy metadata follows the existing envelope contract.
pub fn inspect_package_intent(
    document: &XmlDocument,
) -> Result<Option<PackageIntent>, MetadataDecodeError> {
    let envelope = document.root();
    if envelope.name().local() != "MetaDataObject" {
        return Ok(None);
    }
    let uris = resolve_namespaces(envelope)?;
    let namespace = uri_of(envelope, &uris);
    if !matches!(namespace, None | Some(MD_NAMESPACE)) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "package metadata envelope namespace is not MDClasses",
        ));
    }
    let mut elements = envelope.children().iter().filter_map(|node| match node {
        XmlNode::Element(element) => Some(element),
        _ => None,
    });
    let Some(root) = elements.next() else {
        // Let the ordinary source compiler address a malformed non-root file.
        return Ok(None);
    };
    if elements.next().is_some() {
        return Err(MetadataDecodeError::Duplicate("metadata object"));
    }
    if uri_of(root, &uris) != namespace {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "package root namespace differs from metadata envelope",
        ));
    }
    match root.name().local() {
        "ExternalDataProcessor" => Ok(Some(PackageIntent::ExternalDataProcessor)),
        "ExternalReport" => Ok(Some(PackageIntent::ExternalReport)),
        "Configuration" => {
            let mut properties = None;
            for node in root.children() {
                if let XmlNode::Element(element) = node
                    && element.name().local() == "Properties"
                {
                    if uri_of(element, &uris) != namespace {
                        return Err(MetadataDecodeError::InvalidEnvelope(
                            "configuration Properties namespace differs from package root",
                        ));
                    }
                    if properties.replace(element).is_some() {
                        return Err(MetadataDecodeError::Duplicate("Properties"));
                    }
                }
            }
            let mut extension = false;
            if let Some(properties) = properties {
                for node in properties.children() {
                    let XmlNode::Element(element) = node else {
                        continue;
                    };
                    if matches!(
                        element.name().local(),
                        "ConfigurationExtensionPurpose"
                            | "KeepMappingToExtendedConfigurationObjectsByIDs"
                            | "NamePrefix"
                            | "ObjectBelonging"
                            | "ExtendedConfigurationObject"
                    ) {
                        if uri_of(element, &uris) != namespace {
                            return Err(MetadataDecodeError::InvalidEnvelope(
                                "extension property namespace differs from package root",
                            ));
                        }
                        extension = true;
                    }
                }
            }
            Ok(Some(if extension {
                PackageIntent::Extension
            } else {
                PackageIntent::Configuration
            }))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::XmlReader;

    fn inspect(xml: &str) -> Result<Option<PackageIntent>, MetadataDecodeError> {
        inspect_package_intent(&XmlReader::from_slice(xml.as_bytes()).unwrap())
    }

    #[test]
    fn package_names_follow_scoped_namespace_bindings() {
        let xml = format!(
            r#"<a:MetaDataObject xmlns:a="{MD_NAMESPACE}"><b:Configuration xmlns:b="{MD_NAMESPACE}"><b:Properties><c:NamePrefix xmlns:c="{MD_NAMESPACE}">Own_</c:NamePrefix></b:Properties></b:Configuration></a:MetaDataObject>"#
        );
        assert_eq!(inspect(&xml).unwrap(), Some(PackageIntent::Extension));
        assert!(
            inspect(&xml.replace(
                &format!(r#"xmlns:c="{MD_NAMESPACE}""#),
                r#"xmlns:c="urn:foreign""#,
            ))
            .is_err()
        );
        assert_eq!(
            inspect(
                "<MetaDataObject><Configuration><Properties/></Configuration></MetaDataObject>"
            )
            .unwrap(),
            Some(PackageIntent::Configuration),
        );
    }

    #[test]
    fn only_direct_extension_properties_select_extension() {
        let xml = format!(
            r#"<MetaDataObject xmlns="{MD_NAMESPACE}"><Configuration><Properties><Comment>&lt;NamePrefix&gt;fake&lt;/NamePrefix&gt;</Comment></Properties><ChildObjects><Catalog><Properties><ObjectBelonging>Own</ObjectBelonging></Properties></Catalog></ChildObjects></Configuration></MetaDataObject>"#
        );
        assert_eq!(inspect(&xml).unwrap(), Some(PackageIntent::Configuration));
        assert!(inspect(&format!(
            r#"<MetaDataObject xmlns="{MD_NAMESPACE}"><ExternalReport xmlns="urn:foreign"/></MetaDataObject>"#
        )).is_err());
        assert!(inspect(&format!(
            r#"<MetaDataObject xmlns="{MD_NAMESPACE}"><ExternalReport/><Configuration/></MetaDataObject>"#
        )).is_err());
    }
}
