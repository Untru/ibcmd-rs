//! Strict canonical codec for `FunctionalOption`.
//!
//! Readable `Location` and `Content` references remain typed canonical
//! values. Resolution to native UUIDs is deliberately deferred until the
//! complete validated configuration graph is available to the compiler.

use std::collections::BTreeSet;

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::ObjectPath;
use ibcmd_core::family::FamilyId;
use ibcmd_core::model::CanonicalObject;
use ibcmd_core::value::{CanonicalText, CanonicalValue};

use super::common::{
    MD_NAMESPACE, MetadataDecodeError, MetadataEnvelope, ResolvedNamespaces, XR_NAMESPACE,
    element_text, resolve_namespaces, typed, uri_of,
};
use super::decode_metadata_envelope;
use super::language::{
    canonical_field, canonical_text, copy_object_parts, decode_to_encode, invalid_model,
    profile_version, required_child, root_version, set_unprefixed_attribute,
    validate_decode_profile,
};
use super::registry::{
    MetadataEncodeError, MetadataFamilyCodec, MetadataRegistry, MetadataRegistryError,
};
use crate::{AttributeKind, LexicalPolicy, XmlDocument, XmlElement, XmlNode, XmlWriter};

const FAMILY: &str = "FunctionalOption";

/// Registers the exact `FunctionalOption` codec.
pub fn register_functional_option_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(FunctionalOptionCodec {
        family: FamilyId::parse(FAMILY).expect("family id is stable"),
    }))
}

struct FunctionalOptionCodec {
    family: FamilyId,
}

impl MetadataFamilyCodec for FunctionalOptionCodec {
    fn family_id(&self) -> &FamilyId {
        &self.family
    }

    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        decode_functional_option(document, source, path)
    }

    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        encode_functional_option(envelope, target)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Projection {
    comment: String,
    location: Option<String>,
    privileged_get_mode: bool,
    content: Vec<String>,
}

fn decode_functional_option(
    document: &XmlDocument,
    source: ProfileId,
    path: ObjectPath,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    validate_decode_profile(document, &source, &path)?;
    let projection = project(document)?;
    let generic = decode_metadata_envelope(document, source, path)?;
    if generic.root().kind().as_str() != FAMILY {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "functional option codec requires its exact family",
        ));
    }
    if !generic.descendants().is_empty()
        || !generic.root().generated_types().is_empty()
        || !generic.root().references().is_empty()
        || !generic.root().assets().is_empty()
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "FunctionalOption cannot contain children, generated types, references, or assets",
        ));
    }
    let mut parts = copy_object_parts(generic.root());
    parts.properties.push(canonical_field(
        "Comment",
        CanonicalValue::text(canonical_text(&projection.comment)?),
    )?);
    let location = match &projection.location {
        Some(value) => CanonicalValue::text(canonical_text(value)?),
        None => CanonicalValue::null(),
    };
    parts
        .properties
        .push(canonical_field("Location", location)?);
    parts.properties.push(canonical_field(
        "PrivilegedGetMode",
        CanonicalValue::boolean(projection.privileged_get_mode),
    )?);
    let content = projection
        .content
        .iter()
        .map(|value| {
            CanonicalText::new(value)
                .map(CanonicalValue::text)
                .map_err(|error| MetadataDecodeError::Core(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    parts.properties.push(canonical_field(
        "Content",
        CanonicalValue::sequence(content)
            .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
    )?);
    let root = CanonicalObject::new(parts)
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))?;
    MetadataEnvelope::from_parts(root, Vec::new(), document.clone())
}

fn project(document: &XmlDocument) -> Result<Projection, MetadataDecodeError> {
    let uris = resolve_namespaces(document.root())?;
    let expected = uri_of(document.root(), &uris);
    if !matches!(expected, None | Some(MD_NAMESPACE)) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "functional option root namespace",
        ));
    }
    require_attributes(document.root(), &["version"])?;
    if !namespace_declared(document.root(), "xr", XR_NAMESPACE) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "FunctionalOption requires the exact xr namespace binding",
        ));
    }
    let object = required_child(document.root(), FAMILY, expected, &uris)?;
    require_attributes(object, &["uuid"])?;
    let properties = only_properties_child(object, expected, &uris)?;
    require_attributes(properties, &[])?;

    let mut seen = BTreeSet::new();
    let mut comment = None;
    let mut location = None;
    let mut privileged_get_mode = None;
    let mut content = None;
    for node in properties.children() {
        let XmlNode::Element(child) = node else {
            continue;
        };
        let local = child.name().local();
        if !matches!(
            local,
            "Name" | "Synonym" | "Comment" | "Location" | "PrivilegedGetMode" | "Content"
        ) || !typed(child, local, expected, &uris)
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "unknown FunctionalOption property",
            ));
        }
        if !seen.insert(local) {
            return Err(MetadataDecodeError::Duplicate(match local {
                "Name" => "Name",
                "Synonym" => "Synonym",
                "Comment" => "Comment",
                "Location" => "Location",
                "PrivilegedGetMode" => "PrivilegedGetMode",
                "Content" => "Content",
                _ => unreachable!("allowed above"),
            }));
        }
        match local {
            "Name" => {
                require_attributes(child, &[])?;
                if element_text(child)?.is_none() {
                    return Err(MetadataDecodeError::Missing("Name text"));
                }
            }
            "Synonym" => validate_synonym_attributes(child)?,
            "Comment" => {
                require_attributes(child, &[])?;
                comment = Some(element_text(child)?.ok_or(
                    MetadataDecodeError::InvalidEnvelope("Comment must contain text only"),
                )?);
            }
            "Location" => {
                require_attributes(child, &[])?;
                let value = element_text(child)?.ok_or(MetadataDecodeError::InvalidEnvelope(
                    "Location must contain text only",
                ))?;
                if value.is_empty() {
                    return Err(MetadataDecodeError::InvalidEnvelope(
                        "Location must not be empty",
                    ));
                }
                location = Some(value);
            }
            "PrivilegedGetMode" => {
                require_attributes(child, &[])?;
                let value = element_text(child)?.ok_or(MetadataDecodeError::InvalidEnvelope(
                    "PrivilegedGetMode must contain text only",
                ))?;
                privileged_get_mode = Some(parse_bool(&value)?);
            }
            "Content" => {
                require_attributes(child, &[])?;
                content = Some(parse_content(child, &uris)?);
            }
            _ => unreachable!("allowed above"),
        }
    }
    for required in ["Name", "Synonym", "Comment", "PrivilegedGetMode", "Content"] {
        if !seen.contains(required) {
            return Err(MetadataDecodeError::Missing(required));
        }
    }
    Ok(Projection {
        comment: comment.expect("presence checked"),
        location,
        privileged_get_mode: privileged_get_mode.expect("presence checked"),
        content: content.expect("presence checked"),
    })
}

fn only_properties_child<'a>(
    object: &'a XmlElement,
    expected: Option<&str>,
    uris: &ResolvedNamespaces,
) -> Result<&'a XmlElement, MetadataDecodeError> {
    let properties = required_child(object, "Properties", expected, uris)?;
    if object
        .children()
        .iter()
        .any(|node| matches!(node, XmlNode::Element(child) if !std::ptr::eq(child, properties)))
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "unknown FunctionalOption object child",
        ));
    }
    Ok(properties)
}

fn parse_content(
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<Vec<String>, MetadataDecodeError> {
    let mut values = Vec::new();
    let mut unique = BTreeSet::new();
    for node in element.children() {
        let XmlNode::Element(item) = node else {
            continue;
        };
        if item.name().local() != "Object" || uri_of(item, uris) != Some(XR_NAMESPACE) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "Content contains a non-xr:Object element",
            ));
        }
        require_attributes(item, &[])?;
        let value = element_text(item)?.ok_or(MetadataDecodeError::InvalidEnvelope(
            "Content object must contain text only",
        ))?;
        if value.is_empty() {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "Content object must not be empty",
            ));
        }
        if !unique.insert(value.clone()) {
            return Err(MetadataDecodeError::Duplicate("Content object"));
        }
        values.push(value);
    }
    Ok(values)
}

fn parse_bool(value: &str) -> Result<bool, MetadataDecodeError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(MetadataDecodeError::InvalidEnvelope(
            "PrivilegedGetMode is not a canonical boolean",
        )),
    }
}

fn validate_synonym_attributes(element: &XmlElement) -> Result<(), MetadataDecodeError> {
    require_attributes(element, &[])?;
    for node in element.children() {
        if let XmlNode::Element(child) = node {
            require_attributes(child, &[])?;
            for node in child.children() {
                if let XmlNode::Element(field) = node {
                    require_attributes(field, &[])?;
                }
            }
        }
    }
    Ok(())
}

fn require_attributes(element: &XmlElement, allowed: &[&str]) -> Result<(), MetadataDecodeError> {
    for attribute in element.attributes() {
        match attribute.kind() {
            AttributeKind::Namespace(_) => {}
            AttributeKind::Ordinary(name)
                if name.prefix().is_none() && allowed.contains(&name.local()) => {}
            AttributeKind::Ordinary(_) => {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "unknown FunctionalOption semantic attribute",
                ));
            }
        }
    }
    Ok(())
}

fn namespace_declared(root: &XmlElement, prefix: &str, uri: &str) -> bool {
    root.attributes().iter().any(|attribute| {
        matches!(attribute.kind(), AttributeKind::Namespace(Some(candidate)) if candidate == prefix)
            && attribute.value() == uri
    })
}

fn encode_functional_option(
    envelope: &MetadataEnvelope,
    target: &ProfileId,
) -> Result<Vec<u8>, MetadataEncodeError> {
    let path = envelope.root().identity().path().clone();
    let target_version =
        profile_version(target).ok_or_else(|| MetadataEncodeError::UnsupportedProfile {
            object_path: path.clone(),
            profile: target.clone(),
        })?;
    if envelope.root().kind().as_str() != FAMILY {
        return Err(invalid_model(&path, "kind"));
    }
    let source_profile = envelope.root().provenance().source_profile().clone();
    validate_decode_profile(envelope.source_document(), &source_profile, &path)
        .map_err(decode_to_encode)?;
    let source = decode_functional_option(envelope.source_document(), source_profile, path.clone())
        .map_err(decode_to_encode)?;
    if source.root() != envelope.root() || source.descendants() != envelope.descendants() {
        return Err(invalid_model(
            &path,
            "FunctionalOption mutation is not implemented",
        ));
    }
    let root = if root_version(envelope.source_document().root()).map_err(decode_to_encode)?
        == target_version
    {
        envelope.source_document().root().clone()
    } else {
        set_unprefixed_attribute(
            envelope.source_document().root(),
            "version",
            target_version,
            &path,
        )?
    };
    XmlWriter::to_vec(
        &envelope.source_document().with_root(root),
        LexicalPolicy::Preserve,
    )
    .map_err(|error| MetadataEncodeError::Xml(error.to_string()))
}

#[cfg(test)]
mod tests {
    use ibcmd_core::value::CanonicalValueKind;

    use super::*;
    use crate::XmlReader;

    const UUID: &str = "11111111-1111-4111-8111-111111111111";

    fn fixture(version: &str, extra: &str) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"{MD_NAMESPACE}\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"{XR_NAMESPACE}\" version=\"{version}\">\r\n\
\t<FunctionalOption uuid=\"{UUID}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>UseFeature</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>en</v8:lang><v8:content>Use feature</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<Location>Constant.FeatureState</Location>\r\n\
\t\t\t<PrivilegedGetMode>true</PrivilegedGetMode>\r\n\
\t\t\t<Content><xr:Object>Catalog.Products</xr:Object></Content>{extra}\r\n\
\t\t</Properties>\r\n\
\t</FunctionalOption>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    fn profile(version: &str) -> ProfileId {
        ProfileId::parse(&format!("xml-{version}")).unwrap()
    }

    #[test]
    fn references_and_boolean_are_typed_for_both_dialects() {
        for version in ["2.20", "2.21"] {
            let document = XmlReader::from_slice(&fixture(version, "")).unwrap();
            let mut registry = MetadataRegistry::default();
            register_functional_option_codec(&mut registry).unwrap();
            let envelope = registry
                .decode(
                    &FamilyId::parse(FAMILY).unwrap(),
                    &document,
                    profile(version),
                    ObjectPath::root(),
                )
                .unwrap();
            assert_eq!(
                envelope
                    .root()
                    .properties()
                    .iter()
                    .map(|field| field.name().as_str())
                    .collect::<Vec<_>>(),
                [
                    "Name",
                    "Synonym",
                    "Comment",
                    "Location",
                    "PrivilegedGetMode",
                    "Content"
                ]
            );
            assert!(matches!(
                envelope.root().properties()[4].value().kind(),
                CanonicalValueKind::Bool(true)
            ));
            let target = if version == "2.20" { "2.21" } else { "2.20" };
            let output = registry.encode(&envelope, &profile(target)).unwrap();
            let reparsed = XmlReader::from_slice(&output).unwrap();
            registry
                .decode(
                    &FamilyId::parse(FAMILY).unwrap(),
                    &reparsed,
                    profile(target),
                    ObjectPath::root(),
                )
                .unwrap();
        }
    }

    #[test]
    fn location_is_optional_and_is_represented_as_null() {
        let xml = String::from_utf8(fixture("2.20", ""))
            .unwrap()
            .replace("\t\t\t<Location>Constant.FeatureState</Location>\r\n", "");
        let document = XmlReader::from_slice(xml.as_bytes()).unwrap();
        let mut registry = MetadataRegistry::default();
        register_functional_option_codec(&mut registry).unwrap();
        let envelope = registry
            .decode(
                &FamilyId::parse(FAMILY).unwrap(),
                &document,
                profile("2.20"),
                ObjectPath::root(),
            )
            .unwrap();
        assert!(matches!(
            envelope.root().properties()[3].value().kind(),
            CanonicalValueKind::Null
        ));
    }

    #[test]
    fn unknown_property_and_noncanonical_boolean_fail_closed() {
        let mut registry = MetadataRegistry::default();
        register_functional_option_codec(&mut registry).unwrap();
        let unknown = XmlReader::from_slice(&fixture("2.20", "<Future/>")).unwrap();
        assert!(
            registry
                .decode(
                    &FamilyId::parse(FAMILY).unwrap(),
                    &unknown,
                    profile("2.20"),
                    ObjectPath::root(),
                )
                .is_err()
        );
        let wrong = String::from_utf8(fixture("2.20", ""))
            .unwrap()
            .replace(">true</PrivilegedGetMode>", ">1</PrivilegedGetMode>");
        let wrong = XmlReader::from_slice(wrong.as_bytes()).unwrap();
        assert!(
            registry
                .decode(
                    &FamilyId::parse(FAMILY).unwrap(),
                    &wrong,
                    profile("2.20"),
                    ObjectPath::root(),
                )
                .is_err()
        );
    }
}
