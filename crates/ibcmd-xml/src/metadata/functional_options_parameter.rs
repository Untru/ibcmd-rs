//! Strict canonical codec for `FunctionalOptionsParameter`.
//!
//! The readable `<Use>` references are retained in source order as a typed
//! sequence.  Resolution to native object UUIDs belongs to the validated
//! configuration compiler, not to this single-document XML codec.

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

const FAMILY: &str = "FunctionalOptionsParameter";
const XSI_NAMESPACE: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// Registers the exact `FunctionalOptionsParameter` codec.
pub fn register_functional_options_parameter_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(FunctionalOptionsParameterCodec {
        family: FamilyId::parse(FAMILY).expect("family id is stable"),
    }))
}

struct FunctionalOptionsParameterCodec {
    family: FamilyId,
}

impl MetadataFamilyCodec for FunctionalOptionsParameterCodec {
    fn family_id(&self) -> &FamilyId {
        &self.family
    }

    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        decode_functional_options_parameter(document, source, path)
    }

    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        encode_functional_options_parameter(envelope, target)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Projection {
    comment: String,
    uses: Vec<String>,
}

fn decode_functional_options_parameter(
    document: &XmlDocument,
    source: ProfileId,
    path: ObjectPath,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    validate_decode_profile(document, &source, &path)?;
    let projection = project(document)?;
    let generic = decode_metadata_envelope(document, source, path)?;
    if generic.root().kind().as_str() != FAMILY {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "functional options parameter codec requires its exact family",
        ));
    }
    if !generic.descendants().is_empty()
        || !generic.root().generated_types().is_empty()
        || !generic.root().references().is_empty()
        || !generic.root().assets().is_empty()
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "FunctionalOptionsParameter cannot contain children, generated types, references, or assets",
        ));
    }
    let mut parts = copy_object_parts(generic.root());
    parts.properties.push(canonical_field(
        "Comment",
        CanonicalValue::text(canonical_text(&projection.comment)?),
    )?);
    let uses = projection
        .uses
        .iter()
        .map(|value| {
            CanonicalText::new(value)
                .map(CanonicalValue::text)
                .map_err(|error| MetadataDecodeError::Core(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    parts.properties.push(canonical_field(
        "Use",
        CanonicalValue::sequence(uses)
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
            "functional options parameter root namespace",
        ));
    }
    require_attributes(document.root(), &["version"])?;
    if !namespace_declared(document.root(), "xr", XR_NAMESPACE)
        || !namespace_declared(document.root(), "xsi", XSI_NAMESPACE)
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "FunctionalOptionsParameter requires exact xr and xsi namespace bindings",
        ));
    }
    let object = required_child(document.root(), FAMILY, expected, &uris)?;
    require_attributes(object, &["uuid"])?;
    let properties = only_properties_child(object, expected, &uris)?;
    require_attributes(properties, &[])?;

    let mut seen = BTreeSet::new();
    let mut comment = None;
    let mut uses = None;
    for node in properties.children() {
        let XmlNode::Element(child) = node else {
            continue;
        };
        let local = child.name().local();
        if !matches!(local, "Name" | "Synonym" | "Comment" | "Use")
            || !typed(child, local, expected, &uris)
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "unknown FunctionalOptionsParameter property",
            ));
        }
        if !seen.insert(local) {
            return Err(MetadataDecodeError::Duplicate(match local {
                "Name" => "Name",
                "Synonym" => "Synonym",
                "Comment" => "Comment",
                "Use" => "Use",
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
            "Use" => {
                require_attributes(child, &[])?;
                uses = Some(parse_uses(child, &uris)?);
            }
            _ => unreachable!("allowed above"),
        }
    }
    for required in ["Name", "Synonym", "Comment", "Use"] {
        if !seen.contains(required) {
            return Err(MetadataDecodeError::Missing(required));
        }
    }
    Ok(Projection {
        comment: comment.expect("presence checked"),
        uses: uses.expect("presence checked"),
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
            "unknown FunctionalOptionsParameter object child",
        ));
    }
    Ok(properties)
}

fn parse_uses(
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<Vec<String>, MetadataDecodeError> {
    let mut values = Vec::new();
    let mut unique = BTreeSet::new();
    for node in element.children() {
        let XmlNode::Element(item) = node else {
            continue;
        };
        if item.name().local() != "Item" || uri_of(item, uris) != Some(XR_NAMESPACE) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "Use contains a non-xr:Item element",
            ));
        }
        validate_item_type(item)?;
        let value = element_text(item)?.ok_or(MetadataDecodeError::InvalidEnvelope(
            "Use item must contain text only",
        ))?;
        if value.is_empty() {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "Use item must not be empty",
            ));
        }
        if !unique.insert(value.clone()) {
            return Err(MetadataDecodeError::Duplicate("Use item"));
        }
        values.push(value);
    }
    Ok(values)
}

fn validate_item_type(item: &XmlElement) -> Result<(), MetadataDecodeError> {
    let mut seen = false;
    for attribute in item.attributes() {
        match attribute.kind() {
            AttributeKind::Namespace(_) => {}
            AttributeKind::Ordinary(name)
                if name.prefix() == Some("xsi") && name.local() == "type" =>
            {
                if seen || attribute.value() != "xr:MDObjectRef" {
                    return Err(MetadataDecodeError::InvalidEnvelope(
                        "Use item has an invalid xsi:type",
                    ));
                }
                seen = true;
            }
            AttributeKind::Ordinary(_) => {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "Use item has an unknown semantic attribute",
                ));
            }
        }
    }
    if seen {
        Ok(())
    } else {
        Err(MetadataDecodeError::Missing("Use item xsi:type"))
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
                    "unknown FunctionalOptionsParameter semantic attribute",
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

fn encode_functional_options_parameter(
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
    let source = decode_functional_options_parameter(
        envelope.source_document(),
        source_profile,
        path.clone(),
    )
    .map_err(decode_to_encode)?;
    if source.root() != envelope.root() || source.descendants() != envelope.descendants() {
        return Err(invalid_model(
            &path,
            "FunctionalOptionsParameter mutation is not implemented",
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
    use super::*;
    use crate::XmlReader;

    const UUID: &str = "11111111-1111-4111-8111-111111111111";

    fn fixture(version: &str, extra: &str) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"{MD_NAMESPACE}\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"{XR_NAMESPACE}\" xmlns:xsi=\"{XSI_NAMESPACE}\" version=\"{version}\">\r\n\
\t<FunctionalOptionsParameter uuid=\"{UUID}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>UseFeatureFor</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>en</v8:lang><v8:content>Use feature for</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<Use><xr:Item xsi:type=\"xr:MDObjectRef\">Catalog.Products</xr:Item></Use>{extra}\r\n\
\t\t</Properties>\r\n\
\t</FunctionalOptionsParameter>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    fn profile(version: &str) -> ProfileId {
        ProfileId::parse(&format!("xml-{version}")).unwrap()
    }

    #[test]
    fn use_references_are_typed_and_cross_version_roundtrip_is_strict() {
        for version in ["2.20", "2.21"] {
            let document = XmlReader::from_slice(&fixture(version, "")).unwrap();
            let mut registry = MetadataRegistry::default();
            register_functional_options_parameter_codec(&mut registry).unwrap();
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
                ["Name", "Synonym", "Comment", "Use"]
            );
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
    fn unknown_property_and_wrong_reference_type_fail_closed() {
        let mut registry = MetadataRegistry::default();
        register_functional_options_parameter_codec(&mut registry).unwrap();
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
            .replace("xr:MDObjectRef", "xr:FutureRef");
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
