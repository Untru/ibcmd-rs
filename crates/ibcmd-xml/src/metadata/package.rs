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

/// Physical package identity, separate from the one canonical metadata graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageRootIdentity {
    pub intent: PackageIntent,
    pub main_uuid: ibcmd_core::identity::ObjectUuid,
    pub contained: Option<PackageContainedIdentity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageContainedIdentity {
    pub class_id: ibcmd_core::identity::ObjectUuid,
    pub object_id: ibcmd_core::identity::ObjectUuid,
}

/// Read exact direct root identities through the canonical expanded-name resolver.
/// This does not admit root properties or synthesize internal metadata objects.
pub fn inspect_package_identity(
    document: &XmlDocument,
) -> Result<Option<PackageRootIdentity>, MetadataDecodeError> {
    use super::common::{XR_NAMESPACE, element_text, uuid_attr};
    use ibcmd_core::identity::ObjectUuid;
    let Some(intent) = inspect_package_intent(document)? else {
        return Ok(None);
    };
    let uris = resolve_namespaces(document.root())?;
    let root = document
        .root()
        .children()
        .iter()
        .find_map(|node| match node {
            XmlNode::Element(e) => Some(e),
            _ => None,
        })
        .ok_or(MetadataDecodeError::Missing("package root"))?;
    let main_uuid = uuid_attr(root)?;
    if main_uuid.as_bytes().iter().all(|b| *b == 0) {
        return Err(MetadataDecodeError::InvalidUuid(main_uuid.to_string()));
    }
    let external = matches!(
        intent,
        PackageIntent::ExternalDataProcessor | PackageIntent::ExternalReport
    );
    let mut contained = None;
    if external {
        let namespace = uri_of(root, &uris);
        let mut internal = None;
        for node in root.children() {
            if let XmlNode::Element(e) = node
                && e.name().local() == "InternalInfo"
            {
                if uri_of(e, &uris) != namespace {
                    return Err(MetadataDecodeError::InvalidEnvelope(
                        "foreign package InternalInfo",
                    ));
                }
                if internal.replace(e).is_some() {
                    return Err(MetadataDecodeError::Duplicate("InternalInfo"));
                }
            }
        }
        for node in internal
            .ok_or(MetadataDecodeError::Missing("InternalInfo"))?
            .children()
        {
            let XmlNode::Element(e) = node else {
                continue;
            };
            if e.name().local() != "ContainedObject" {
                continue;
            }
            if uri_of(e, &uris) != Some(XR_NAMESPACE) {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "foreign ContainedObject",
                ));
            }
            let mut class_id = None;
            let mut object_id = None;
            for node in e.children() {
                let XmlNode::Element(field) = node else {
                    continue;
                };
                if uri_of(field, &uris) != Some(XR_NAMESPACE) {
                    return Err(MetadataDecodeError::InvalidEnvelope(
                        "foreign contained identity field",
                    ));
                }
                let value = element_text(field)?
                    .ok_or(MetadataDecodeError::Missing("contained identity value"))?;
                let value = ObjectUuid::parse(value.trim())
                    .map_err(|_| MetadataDecodeError::InvalidUuid(value.clone()))?;
                if value.as_bytes().iter().all(|b| *b == 0) {
                    return Err(MetadataDecodeError::InvalidUuid(value.to_string()));
                }
                let slot = match field.name().local() {
                    "ClassId" => &mut class_id,
                    "ObjectId" => &mut object_id,
                    _ => {
                        return Err(MetadataDecodeError::InvalidEnvelope(
                            "unknown contained identity field",
                        ));
                    }
                };
                if slot.replace(value).is_some() {
                    return Err(MetadataDecodeError::Duplicate("contained identity field"));
                }
            }
            let value = PackageContainedIdentity {
                class_id: class_id.ok_or(MetadataDecodeError::Missing("ClassId"))?,
                object_id: object_id.ok_or(MetadataDecodeError::Missing("ObjectId"))?,
            };
            if contained.replace(value).is_some() {
                return Err(MetadataDecodeError::Duplicate("ContainedObject"));
            }
        }
        if contained.is_none() {
            return Err(MetadataDecodeError::Missing("ContainedObject"));
        }
    }
    Ok(Some(PackageRootIdentity {
        intent,
        main_uuid,
        contained,
    }))
}

// These are package signals, not a second property/value schema. Ordinary
// Configuration owns both shared fields too; only the extension-specific
// purpose/adoption/mapping families grant Extension intent. Full property
// validation remains with the selected artifact compiler.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum ConfigurationPackageProperty {
    NamePrefix,
    ExtensionCompatibility,
    ExtensionPurpose,
    ExtensionIdMapping,
    ObjectBelonging,
    ExtendedObject,
}
impl ConfigurationPackageProperty {
    fn for_name(name: &str) -> Option<Self> {
        Some(match name {
            "NamePrefix" => Self::NamePrefix,
            "ConfigurationExtensionCompatibilityMode" => Self::ExtensionCompatibility,
            "ConfigurationExtensionPurpose" => Self::ExtensionPurpose,
            "KeepMappingToExtendedConfigurationObjectsByIDs" => Self::ExtensionIdMapping,
            "ObjectBelonging" => Self::ObjectBelonging,
            "ExtendedConfigurationObject" => Self::ExtendedObject,
            _ => return None,
        })
    }
    fn is_extension_marker(self) -> bool {
        !matches!(self, Self::NamePrefix | Self::ExtensionCompatibility)
    }
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
            let mut seen_signals = std::collections::BTreeSet::new();
            if let Some(properties) = properties {
                for node in properties.children() {
                    let XmlNode::Element(element) = node else {
                        continue;
                    };
                    if let Some(property) =
                        ConfigurationPackageProperty::for_name(element.name().local())
                    {
                        if uri_of(element, &uris) != namespace {
                            return Err(MetadataDecodeError::InvalidEnvelope(
                                "configuration package property namespace differs from package root",
                            ));
                        }
                        if !seen_signals.insert(property) {
                            return Err(MetadataDecodeError::Duplicate(
                                "configuration package property",
                            ));
                        }
                        extension |= property.is_extension_marker();
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
        assert_eq!(inspect(&xml).unwrap(), Some(PackageIntent::Configuration));
        let extension = xml.replace("</b:Properties>",
            "<c:ConfigurationExtensionPurpose xmlns:c=\"http://v8.1c.ru/8.3/MDClasses\">Customization</c:ConfigurationExtensionPurpose></b:Properties>");
        assert_eq!(inspect(&extension).unwrap(), Some(PackageIntent::Extension));
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
    #[test]
    fn ordinary_shared_prefix_and_extension_compatibility_are_not_package_purpose() {
        assert_eq!(
            inspect("<MetaDataObject><Configuration><Properties><NamePrefix/><ConfigurationExtensionCompatibilityMode>Version8_3_27</ConfigurationExtensionCompatibilityMode></Properties></Configuration></MetaDataObject>").unwrap(),
            Some(PackageIntent::Configuration),
        );
        for prefix in ["", "Own_", "Склад_"] {
            for compatibility in ["Version8_3_24", "Version8_3_27", "Version8_5_1"] {
                let xml = format!(
                    r#"<MetaDataObject xmlns="{MD_NAMESPACE}"><Configuration><Properties><NamePrefix>{prefix}</NamePrefix><ConfigurationExtensionCompatibilityMode>{compatibility}</ConfigurationExtensionCompatibilityMode><CompatibilityMode>Version8_3_24</CompatibilityMode></Properties></Configuration></MetaDataObject>"#
                );
                assert_eq!(inspect(&xml).unwrap(), Some(PackageIntent::Configuration));
                let extension = xml.replace("</Properties>",
                    "<ConfigurationExtensionPurpose>Customization</ConfigurationExtensionPurpose></Properties>");
                assert_eq!(inspect(&extension).unwrap(), Some(PackageIntent::Extension));
            }
        }
    }

    #[test]
    fn exact_extension_markers_foreign_namespaces_and_duplicate_signals_keep_refusal() {
        for property in [
            "<ConfigurationExtensionPurpose>Patch</ConfigurationExtensionPurpose>",
            "<ConfigurationExtensionPurpose>Customization</ConfigurationExtensionPurpose>",
            "<ConfigurationExtensionPurpose>AddOn</ConfigurationExtensionPurpose>",
            "<ObjectBelonging>Adopted</ObjectBelonging>",
            "<ObjectBelonging>Own</ObjectBelonging>",
            "<KeepMappingToExtendedConfigurationObjectsByIDs>false</KeepMappingToExtendedConfigurationObjectsByIDs>",
            "<KeepMappingToExtendedConfigurationObjectsByIDs>true</KeepMappingToExtendedConfigurationObjectsByIDs>",
            "<ExtendedConfigurationObject>Configuration.Main</ExtendedConfigurationObject>",
        ] {
            let xml = format!(
                r#"<MetaDataObject xmlns="{MD_NAMESPACE}"><Configuration><Properties><NamePrefix/><ConfigurationExtensionCompatibilityMode>Version8_3_27</ConfigurationExtensionCompatibilityMode>{property}</Properties></Configuration></MetaDataObject>"#
            );
            assert_eq!(inspect(&xml).unwrap(), Some(PackageIntent::Extension));
            let foreign = xml.replace(
                property,
                &property.replacen('>', " xmlns=\"urn:foreign\">", 1),
            );
            assert!(inspect(&foreign).is_err());
            let duplicate = xml.replace(property, &format!("{property}{property}"));
            assert!(inspect(&duplicate).is_err());
        }
        for property in ["NamePrefix", "ConfigurationExtensionCompatibilityMode"] {
            let foreign = format!(
                r#"<MetaDataObject xmlns="{MD_NAMESPACE}"><Configuration><Properties><{property} xmlns="urn:foreign"/></Properties></Configuration></MetaDataObject>"#
            );
            assert!(inspect(&foreign).is_err());
            let duplicate = format!(
                r#"<MetaDataObject xmlns="{MD_NAMESPACE}"><Configuration><Properties><{property}/><{property}/></Properties></Configuration></MetaDataObject>"#
            );
            assert!(inspect(&duplicate).is_err());
        }
        let nested = format!(
            r#"<MetaDataObject xmlns="{MD_NAMESPACE}"><Configuration><Properties><Comment><ConfigurationExtensionPurpose>Patch</ConfigurationExtensionPurpose></Comment></Properties></Configuration></MetaDataObject>"#
        );
        assert_eq!(
            inspect(&nested).unwrap(),
            Some(PackageIntent::Configuration)
        );
        let duplicate_properties = format!(
            r#"<MetaDataObject xmlns="{MD_NAMESPACE}"><Configuration><Properties/><Properties/></Configuration></MetaDataObject>"#
        );
        assert!(inspect(&duplicate_properties).is_err());
    }
}
