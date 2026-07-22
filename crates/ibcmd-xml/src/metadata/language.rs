//! Offline canonical codec for the `Language` metadata family.
//!
//! `Language` is deliberately small enough to be represented without a
//! native base artifact.  The codec makes `Comment` and `LanguageCode`
//! explicit canonical fields in addition to the common `Name` and `Synonym`
//! projection.  Unknown semantic properties fail closed; document trivia and
//! namespace declarations remain lossless in the source document.

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::ObjectPath;
use ibcmd_core::family::FamilyId;
use ibcmd_core::model::{CanonicalObject, CanonicalObjectParts};
use ibcmd_core::value::{CanonicalField, CanonicalText, CanonicalValue, CanonicalValueKind};

use super::common::{
    MD_NAMESPACE, MetadataDecodeError, MetadataEnvelope, ResolvedNamespaces, V8_NAMESPACE,
    element_text, resolve_namespaces, typed, uri_of,
};
use super::decode_metadata_envelope;
use super::registry::{
    MetadataEncodeError, MetadataFamilyCodec, MetadataRegistry, MetadataRegistryError,
};
use crate::{
    Attribute, AttributeKind, LexicalPolicy, QName, XmlDocument, XmlElement, XmlNode, XmlWriter,
};

const LANGUAGE_FAMILY: &str = "Language";
const PROFILE_220: &str = "xml-2.20";
const PROFILE_221: &str = "xml-2.21";

/// Registers the exact `Language` family codec in a caller-owned registry.
pub fn register_language_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(LanguageCodec {
        family: FamilyId::parse(LANGUAGE_FAMILY).expect("language family id is stable"),
    }))
}

struct LanguageCodec {
    family: FamilyId,
}

impl MetadataFamilyCodec for LanguageCodec {
    fn family_id(&self) -> &FamilyId {
        &self.family
    }

    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        decode_language(document, source, path)
    }

    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        encode_language(envelope, target)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LanguageProjection {
    comment: String,
    language_code: String,
}

pub(super) fn profile_version(profile: &ProfileId) -> Option<&'static str> {
    match profile.as_str() {
        PROFILE_220 => Some("2.20"),
        PROFILE_221 => Some("2.21"),
        _ => None,
    }
}

pub(super) fn root_version(root: &XmlElement) -> Result<&str, MetadataDecodeError> {
    let mut value = None;
    for attribute in root.attributes() {
        if let AttributeKind::Ordinary(name) = attribute.kind()
            && name.prefix().is_none()
            && name.local() == "version"
        {
            if value.is_some() {
                return Err(MetadataDecodeError::Duplicate("root version"));
            }
            value = Some(attribute.value());
        }
    }
    value.ok_or(MetadataDecodeError::Missing("root version"))
}

pub(super) fn validate_decode_profile(
    document: &XmlDocument,
    profile: &ProfileId,
    path: &ObjectPath,
) -> Result<(), MetadataDecodeError> {
    let Some(expected) = profile_version(profile) else {
        return Err(MetadataDecodeError::UnsupportedProfile {
            object_path: path.clone(),
            profile: profile.clone(),
        });
    };
    if root_version(document.root())? != expected {
        return Err(MetadataDecodeError::ProfileVersionMismatch {
            object_path: path.clone(),
        });
    }
    Ok(())
}

fn decode_language(
    document: &XmlDocument,
    source: ProfileId,
    path: ObjectPath,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    validate_decode_profile(document, &source, &path)?;
    let projection = project_language(document)?;
    let generic = decode_metadata_envelope(document, source, path)?;
    if generic.root().kind().as_str() != LANGUAGE_FAMILY {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "language codec requires Language family",
        ));
    }
    if !generic.descendants().is_empty()
        || !generic.root().generated_types().is_empty()
        || !generic.root().references().is_empty()
        || !generic.root().assets().is_empty()
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "Language cannot contain children, generated types, references, or assets",
        ));
    }

    let mut parts = copy_object_parts(generic.root());
    parts.properties.push(canonical_field(
        "Comment",
        CanonicalValue::text(canonical_text(&projection.comment)?),
    )?);
    parts.properties.push(canonical_field(
        "LanguageCode",
        CanonicalValue::text(canonical_text(&projection.language_code)?),
    )?);
    let root = CanonicalObject::new(parts)
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))?;
    MetadataEnvelope::from_parts(root, Vec::new(), document.clone())
}

fn project_language(document: &XmlDocument) -> Result<LanguageProjection, MetadataDecodeError> {
    let uris = resolve_namespaces(document.root())?;
    let expected = uri_of(document.root(), &uris);
    if !matches!(expected, None | Some(MD_NAMESPACE)) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "language root namespace",
        ));
    }
    reject_unknown_attributes(document.root(), &["version"])?;
    let object = required_child(document.root(), LANGUAGE_FAMILY, expected, &uris)?;
    reject_unknown_attributes(object, &["uuid"])?;
    reject_unknown_object_children(object, expected, &uris)?;
    let properties = required_child(object, "Properties", expected, &uris)?;
    reject_unknown_attributes(properties, &[])?;
    reject_unknown_properties(properties, expected, &uris)?;
    let comment = required_text_child(properties, "Comment", expected, &uris)?;
    let language_code = required_text_child(properties, "LanguageCode", expected, &uris)?;
    if language_code.is_empty() {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "LanguageCode must not be empty",
        ));
    }
    Ok(LanguageProjection {
        comment,
        language_code,
    })
}

fn reject_unknown_object_children(
    object: &XmlElement,
    expected: Option<&str>,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let mut properties = 0usize;
    for node in object.children() {
        let XmlNode::Element(child) = node else {
            continue;
        };
        if typed(child, "Properties", expected, uris) {
            properties += 1;
        } else {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "unknown Language object child",
            ));
        }
    }
    if properties != 1 {
        return Err(if properties == 0 {
            MetadataDecodeError::Missing("Properties")
        } else {
            MetadataDecodeError::Duplicate("Properties")
        });
    }
    Ok(())
}

fn reject_unknown_properties(
    properties: &XmlElement,
    expected: Option<&str>,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let mut seen = std::collections::BTreeSet::new();
    for node in properties.children() {
        let XmlNode::Element(child) = node else {
            continue;
        };
        let local = child.name().local();
        if !matches!(local, "Name" | "Synonym" | "Comment" | "LanguageCode")
            || !typed(child, local, expected, uris)
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "unknown Language property",
            ));
        }
        reject_unknown_attributes_recursive(child)?;
        if !seen.insert(local) {
            return Err(MetadataDecodeError::Duplicate(match local {
                "Name" => "Name",
                "Synonym" => "Synonym",
                "Comment" => "Comment",
                "LanguageCode" => "LanguageCode",
                _ => unreachable!("allowed properties matched above"),
            }));
        }
    }
    for required in ["Name", "Synonym", "Comment", "LanguageCode"] {
        if !seen.contains(required) {
            return Err(MetadataDecodeError::Missing(required));
        }
    }
    Ok(())
}

fn reject_unknown_attributes(
    element: &XmlElement,
    allowed_unprefixed: &[&str],
) -> Result<(), MetadataDecodeError> {
    for attribute in element.attributes() {
        match attribute.kind() {
            AttributeKind::Namespace(_) => {}
            AttributeKind::Ordinary(name)
                if name.prefix().is_none() && allowed_unprefixed.contains(&name.local()) => {}
            AttributeKind::Ordinary(_) => {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "unknown Language semantic attribute",
                ));
            }
        }
    }
    Ok(())
}

fn reject_unknown_attributes_recursive(element: &XmlElement) -> Result<(), MetadataDecodeError> {
    reject_unknown_attributes(element, &[])?;
    for node in element.children() {
        if let XmlNode::Element(child) = node {
            reject_unknown_attributes_recursive(child)?;
        }
    }
    Ok(())
}

pub(super) fn required_child<'a>(
    parent: &'a XmlElement,
    local: &'static str,
    namespace: Option<&str>,
    uris: &ResolvedNamespaces,
) -> Result<&'a XmlElement, MetadataDecodeError> {
    let mut found = None;
    for node in parent.children() {
        if let XmlNode::Element(child) = node
            && typed(child, local, namespace, uris)
        {
            if found.is_some() {
                return Err(MetadataDecodeError::Duplicate(local));
            }
            found = Some(child);
        }
    }
    found.ok_or(MetadataDecodeError::Missing(local))
}

fn required_text_child(
    parent: &XmlElement,
    local: &'static str,
    namespace: Option<&str>,
    uris: &ResolvedNamespaces,
) -> Result<String, MetadataDecodeError> {
    let element = required_child(parent, local, namespace, uris)?;
    element_text(element)?.ok_or(MetadataDecodeError::InvalidEnvelope(
        "Language property must contain text only",
    ))
}

pub(super) fn copy_object_parts(object: &CanonicalObject) -> CanonicalObjectParts {
    let mut parts = CanonicalObjectParts::new(
        object.identity().clone(),
        object.kind().clone(),
        object.provenance().clone(),
    );
    parts.owner = object.owner();
    parts.properties = object.properties().to_vec();
    parts.references = object.references().to_vec();
    parts.generated_types = object.generated_types().to_vec();
    parts.assets = object.assets().to_vec();
    parts.opaque_facets = object.opaque_facets().clone();
    parts
}

pub(super) fn canonical_text(value: &str) -> Result<CanonicalText, MetadataDecodeError> {
    CanonicalText::new(value).map_err(|error| MetadataDecodeError::Core(error.to_string()))
}

pub(super) fn canonical_field(
    name: &str,
    value: CanonicalValue,
) -> Result<CanonicalField, MetadataDecodeError> {
    CanonicalField::named(name, value).map_err(|error| MetadataDecodeError::Core(error.to_string()))
}

fn encode_language(
    envelope: &MetadataEnvelope,
    target: &ProfileId,
) -> Result<Vec<u8>, MetadataEncodeError> {
    let path = envelope.root().identity().path().clone();
    let target_version =
        profile_version(target).ok_or_else(|| MetadataEncodeError::UnsupportedProfile {
            object_path: path.clone(),
            profile: target.clone(),
        })?;
    if envelope.root().kind().as_str() != LANGUAGE_FAMILY {
        return Err(invalid_model(&path, "kind"));
    }
    let source_profile = envelope.root().provenance().source_profile().clone();
    validate_decode_profile(envelope.source_document(), &source_profile, &path)
        .map_err(decode_to_encode)?;
    let source = decode_language(envelope.source_document(), source_profile, path.clone())
        .map_err(decode_to_encode)?;
    validate_patchable_model(&source, envelope, &path)?;
    let source_projection = projection_from_object(source.root(), &path)?;
    let desired_projection = projection_from_object(envelope.root(), &path)?;
    let document = patch_document(
        envelope.source_document(),
        source.root(),
        envelope.root(),
        &source_projection,
        &desired_projection,
        target_version,
        &path,
    )?;
    XmlWriter::to_vec(&document, LexicalPolicy::Preserve)
        .map_err(|error| MetadataEncodeError::Xml(error.to_string()))
}

pub(super) fn decode_to_encode(error: MetadataDecodeError) -> MetadataEncodeError {
    match error {
        MetadataDecodeError::UnsupportedProfile {
            object_path,
            profile,
        } => MetadataEncodeError::UnsupportedProfile {
            object_path,
            profile,
        },
        MetadataDecodeError::ProfileVersionMismatch { object_path } => {
            MetadataEncodeError::ProfileVersionMismatch { object_path }
        }
        other => MetadataEncodeError::Xml(other.to_string()),
    }
}

pub(super) fn invalid_model(path: &ObjectPath, field: &'static str) -> MetadataEncodeError {
    MetadataEncodeError::InvalidModel {
        object_path: path.clone(),
        field,
    }
}

fn validate_patchable_model(
    source: &MetadataEnvelope,
    desired: &MetadataEnvelope,
    path: &ObjectPath,
) -> Result<(), MetadataEncodeError> {
    let source_root = source.root();
    let desired_root = desired.root();
    if source_root.kind() != desired_root.kind() {
        return Err(invalid_model(path, "kind"));
    }
    if source_root.owner() != desired_root.owner() {
        return Err(invalid_model(path, "owner"));
    }
    if source_root.references() != desired_root.references() {
        return Err(invalid_model(path, "references"));
    }
    if source_root.generated_types() != desired_root.generated_types() {
        return Err(invalid_model(path, "generated types"));
    }
    if source_root.assets() != desired_root.assets() {
        return Err(invalid_model(path, "assets"));
    }
    if source_root.opaque_facets() != desired_root.opaque_facets() {
        return Err(invalid_model(path, "opaque facets"));
    }
    if source.descendants() != desired.descendants() {
        return Err(invalid_model(path, "descendants"));
    }
    if source_root.properties().len() != desired_root.properties().len()
        || source_root
            .properties()
            .iter()
            .zip(desired_root.properties())
            .any(|(source, desired)| source.name() != desired.name())
    {
        return Err(invalid_model(path, "property schema"));
    }
    Ok(())
}

fn property<'a>(
    object: &'a CanonicalObject,
    name: &'static str,
    path: &ObjectPath,
) -> Result<&'a CanonicalValue, MetadataEncodeError> {
    object
        .properties()
        .iter()
        .find(|field| field.name().as_str() == name)
        .map(CanonicalField::value)
        .ok_or_else(|| invalid_model(path, name))
}

fn optional_property<'a>(object: &'a CanonicalObject, name: &str) -> Option<&'a CanonicalValue> {
    object
        .properties()
        .iter()
        .find(|field| field.name().as_str() == name)
        .map(CanonicalField::value)
}

fn value_text<'a>(
    value: &'a CanonicalValue,
    path: &ObjectPath,
    field: &'static str,
) -> Result<&'a str, MetadataEncodeError> {
    match value.kind() {
        CanonicalValueKind::Text(value) => Ok(value.as_str()),
        _ => Err(invalid_model(path, field)),
    }
}

fn projection_from_object(
    object: &CanonicalObject,
    path: &ObjectPath,
) -> Result<LanguageProjection, MetadataEncodeError> {
    Ok(LanguageProjection {
        comment: value_text(property(object, "Comment", path)?, path, "Comment")?.to_owned(),
        language_code: value_text(
            property(object, "LanguageCode", path)?,
            path,
            "LanguageCode",
        )?
        .to_owned(),
    })
}

#[allow(clippy::too_many_arguments)]
fn patch_document(
    document: &XmlDocument,
    source: &CanonicalObject,
    desired: &CanonicalObject,
    source_projection: &LanguageProjection,
    desired_projection: &LanguageProjection,
    target_version: &str,
    path: &ObjectPath,
) -> Result<XmlDocument, MetadataEncodeError> {
    let uris = resolve_namespaces(document.root()).map_err(decode_to_encode)?;
    let expected = uri_of(document.root(), &uris);
    let object = required_child(document.root(), LANGUAGE_FAMILY, expected, &uris)
        .map_err(decode_to_encode)?;
    let v8_prefix = namespace_prefix(document.root(), V8_NAMESPACE).unwrap_or("v8");
    let patched_object = patch_object(
        object,
        &uris,
        expected,
        source,
        desired,
        source_projection,
        desired_projection,
        v8_prefix,
        path,
    )?;
    let children = document
        .root()
        .children()
        .iter()
        .map(|node| match node {
            XmlNode::Element(element) if std::ptr::eq(element, object) => {
                XmlNode::Element(patched_object.clone())
            }
            _ => node.clone(),
        })
        .collect();
    let root = document.root().with_children(children);
    let root = if root_version(document.root()).map_err(decode_to_encode)? == target_version {
        root
    } else {
        set_unprefixed_attribute(&root, "version", target_version, path)?
    };
    Ok(document.with_root(root))
}

#[allow(clippy::too_many_arguments)]
fn patch_object(
    object: &XmlElement,
    uris: &ResolvedNamespaces,
    expected: Option<&str>,
    source: &CanonicalObject,
    desired: &CanonicalObject,
    source_projection: &LanguageProjection,
    desired_projection: &LanguageProjection,
    v8_prefix: &str,
    path: &ObjectPath,
) -> Result<XmlElement, MetadataEncodeError> {
    let properties =
        required_child(object, "Properties", expected, uris).map_err(decode_to_encode)?;
    let children = object
        .children()
        .iter()
        .map(|node| match node {
            XmlNode::Element(element) if std::ptr::eq(element, properties) => patch_properties(
                element,
                uris,
                expected,
                source,
                desired,
                source_projection,
                desired_projection,
                v8_prefix,
                path,
            )
            .map(XmlNode::Element),
            _ => Ok(node.clone()),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let patched = object.with_children(children);
    if source.identity().uuid() == desired.identity().uuid() {
        Ok(patched)
    } else {
        set_unprefixed_attribute(
            &patched,
            "uuid",
            &desired.identity().uuid().to_string(),
            path,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn patch_properties(
    properties: &XmlElement,
    uris: &ResolvedNamespaces,
    expected: Option<&str>,
    source: &CanonicalObject,
    desired: &CanonicalObject,
    source_projection: &LanguageProjection,
    desired_projection: &LanguageProjection,
    v8_prefix: &str,
    path: &ObjectPath,
) -> Result<XmlElement, MetadataEncodeError> {
    let source_name = property(source, "Name", path)?;
    let desired_name = property(desired, "Name", path)?;
    let source_synonym = optional_property(source, "Synonym");
    let desired_synonym = optional_property(desired, "Synonym");
    if source_synonym.is_some() != desired_synonym.is_some() {
        return Err(invalid_model(path, "Synonym source slot"));
    }
    let children = properties
        .children()
        .iter()
        .map(|node| match node {
            XmlNode::Element(child) if typed(child, "Name", expected, uris) => {
                if source_name == desired_name {
                    Ok(node.clone())
                } else {
                    Ok(XmlNode::Element(replace_text(
                        child,
                        value_text(desired_name, path, "Name")?,
                    )))
                }
            }
            XmlNode::Element(child) if typed(child, "Synonym", expected, uris) => {
                if source_synonym == desired_synonym {
                    Ok(node.clone())
                } else {
                    render_synonym(
                        child,
                        desired_synonym.ok_or_else(|| invalid_model(path, "Synonym"))?,
                        v8_prefix,
                        path,
                    )
                    .map(XmlNode::Element)
                }
            }
            XmlNode::Element(child) if typed(child, "Comment", expected, uris) => {
                if source_projection.comment == desired_projection.comment {
                    Ok(node.clone())
                } else {
                    Ok(XmlNode::Element(replace_text(
                        child,
                        &desired_projection.comment,
                    )))
                }
            }
            XmlNode::Element(child) if typed(child, "LanguageCode", expected, uris) => {
                if source_projection.language_code == desired_projection.language_code {
                    Ok(node.clone())
                } else {
                    Ok(XmlNode::Element(replace_text(
                        child,
                        &desired_projection.language_code,
                    )))
                }
            }
            _ => Ok(node.clone()),
        })
        .collect::<Result<Vec<_>, MetadataEncodeError>>()?;
    Ok(properties.with_children(children))
}

fn replace_text(element: &XmlElement, value: &str) -> XmlElement {
    element.with_children(vec![XmlNode::text(value)])
}

fn synonym_items(
    value: &CanonicalValue,
    path: &ObjectPath,
) -> Result<Vec<(String, String)>, MetadataEncodeError> {
    let values = value
        .as_sequence()
        .ok_or_else(|| invalid_model(path, "Synonym"))?;
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let fields = value
            .as_record()
            .ok_or_else(|| invalid_model(path, "Synonym item"))?;
        if fields.len() != 2
            || fields[0].name().as_str() != "lang"
            || fields[1].name().as_str() != "content"
        {
            return Err(invalid_model(path, "Synonym item schema"));
        }
        result.push((
            value_text(fields[0].value(), path, "Synonym.lang")?.to_owned(),
            value_text(fields[1].value(), path, "Synonym.content")?.to_owned(),
        ));
    }
    Ok(result)
}

fn render_synonym(
    source: &XmlElement,
    value: &CanonicalValue,
    prefix: &str,
    path: &ObjectPath,
) -> Result<XmlElement, MetadataEncodeError> {
    let mut children = Vec::new();
    for (lang, content) in synonym_items(value, path)? {
        children.push(XmlNode::Element(generated_element(
            prefix,
            "item",
            vec![
                XmlNode::Element(generated_text_element(prefix, "lang", &lang)?),
                XmlNode::Element(generated_text_element(prefix, "content", &content)?),
            ],
        )?));
    }
    Ok(source.with_children(children))
}

fn generated_element(
    prefix: &str,
    local: &str,
    children: Vec<XmlNode>,
) -> Result<XmlElement, MetadataEncodeError> {
    Ok(XmlElement::with_parts(
        QName::new(format!("{prefix}:{local}")).map_err(MetadataEncodeError::Xml)?,
        Vec::new(),
        children,
    ))
}

fn generated_text_element(
    prefix: &str,
    local: &str,
    value: &str,
) -> Result<XmlElement, MetadataEncodeError> {
    generated_element(prefix, local, vec![XmlNode::text(value)])
}

fn namespace_prefix<'a>(root: &'a XmlElement, uri: &str) -> Option<&'a str> {
    root.attributes().iter().find_map(|attribute| {
        if attribute.value() != uri {
            return None;
        }
        match attribute.kind() {
            AttributeKind::Namespace(Some(prefix)) => Some(prefix.as_str()),
            _ => None,
        }
    })
}

pub(super) fn set_unprefixed_attribute(
    element: &XmlElement,
    local: &'static str,
    value: &str,
    path: &ObjectPath,
) -> Result<XmlElement, MetadataEncodeError> {
    let mut found = false;
    let attributes = element
        .attributes()
        .iter()
        .map(|attribute| {
            if let AttributeKind::Ordinary(name) = attribute.kind()
                && name.prefix().is_none()
                && name.local() == local
            {
                found = true;
                Attribute::ordinary(name.clone(), value)
            } else {
                attribute.clone()
            }
        })
        .collect();
    if !found {
        return Err(invalid_model(path, local));
    }
    Ok(XmlElement::with_parts(
        element.name().clone(),
        attributes,
        element.children().to_vec(),
    ))
}

#[cfg(test)]
mod tests {
    use ibcmd_core::model::{CanonicalObject, CanonicalObjectParts};
    use ibcmd_core::value::{CanonicalField, CanonicalText, CanonicalValue};

    use super::*;
    use crate::XmlReader;

    const UUID: &str = "11111111-1111-4111-8111-111111111111";

    fn fixture(version: &str, extra: &str) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"{MD_NAMESPACE}\" xmlns:v8=\"{V8_NAMESPACE}\" version=\"{version}\">\r\n\
\t<Language uuid=\"{UUID}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>English</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>en</v8:lang><v8:content>English</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment>Primary language</Comment>\r\n\
\t\t\t<LanguageCode>en</LanguageCode>{extra}\r\n\
\t\t</Properties>\r\n\
\t</Language>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    fn profile(version: &str) -> ProfileId {
        ProfileId::parse(&format!("xml-{version}")).unwrap()
    }

    fn decode(bytes: &[u8], version: &str) -> MetadataEnvelope {
        let document = XmlReader::from_slice(bytes).unwrap();
        let family = FamilyId::parse(LANGUAGE_FAMILY).unwrap();
        let mut registry = MetadataRegistry::default();
        register_language_codec(&mut registry).unwrap();
        registry
            .decode(&family, &document, profile(version), ObjectPath::root())
            .unwrap()
    }

    #[test]
    fn language_fields_are_typed_for_both_supported_dialects() {
        for version in ["2.20", "2.21"] {
            let envelope = decode(&fixture(version, ""), version);
            assert_eq!(
                envelope
                    .root()
                    .properties()
                    .iter()
                    .map(|field| field.name().as_str())
                    .collect::<Vec<_>>(),
                ["Name", "Synonym", "Comment", "LanguageCode"]
            );
            assert_eq!(
                value_text(
                    property(envelope.root(), "LanguageCode", &ObjectPath::root()).unwrap(),
                    &ObjectPath::root(),
                    "LanguageCode"
                )
                .unwrap(),
                "en"
            );
        }
    }

    #[test]
    fn encode_patches_typed_fields_and_target_version() {
        let envelope = decode(&fixture("2.20", ""), "2.20");
        let mut parts = copy_object_parts(envelope.root());
        parts.properties = parts
            .properties
            .into_iter()
            .map(|field| {
                if field.name().as_str() == "LanguageCode" {
                    CanonicalField::named(
                        "LanguageCode",
                        CanonicalValue::text(CanonicalText::new("en-US").unwrap()),
                    )
                    .unwrap()
                } else {
                    field
                }
            })
            .collect();
        let desired = CanonicalObject::new(CanonicalObjectParts { ..parts }).unwrap();
        let envelope = envelope.with_model(desired, Vec::new()).unwrap();
        let mut registry = MetadataRegistry::default();
        register_language_codec(&mut registry).unwrap();
        let output = registry.encode(&envelope, &profile("2.21")).unwrap();
        let text = String::from_utf8(output.clone()).unwrap();
        assert!(text.contains("version=\"2.21\""));
        assert!(text.contains("<LanguageCode>en-US</LanguageCode>"));
        decode(&output, "2.21");
    }

    #[test]
    fn unknown_semantic_property_fails_closed() {
        let document = XmlReader::from_slice(&fixture("2.20", "<Future>1</Future>")).unwrap();
        let mut registry = MetadataRegistry::default();
        register_language_codec(&mut registry).unwrap();
        assert!(matches!(
            registry.decode(
                &FamilyId::parse(LANGUAGE_FAMILY).unwrap(),
                &document,
                profile("2.20"),
                ObjectPath::root()
            ),
            Err(MetadataDecodeError::InvalidEnvelope(
                "unknown Language property"
            ))
        ));
    }

    #[test]
    fn unknown_semantic_attribute_fails_closed() {
        let xml = String::from_utf8(fixture("2.20", ""))
            .unwrap()
            .replace("<LanguageCode>", "<LanguageCode future=\"1\">");
        let document = XmlReader::from_slice(xml.as_bytes()).unwrap();
        let mut registry = MetadataRegistry::default();
        register_language_codec(&mut registry).unwrap();
        assert!(matches!(
            registry.decode(
                &FamilyId::parse(LANGUAGE_FAMILY).unwrap(),
                &document,
                profile("2.20"),
                ObjectPath::root()
            ),
            Err(MetadataDecodeError::InvalidEnvelope(
                "unknown Language semantic attribute"
            ))
        ));
    }

    #[test]
    fn declared_profile_is_not_inferred_from_root_version() {
        let document = XmlReader::from_slice(&fixture("2.21", "")).unwrap();
        let mut registry = MetadataRegistry::default();
        register_language_codec(&mut registry).unwrap();
        assert!(matches!(
            registry.decode(
                &FamilyId::parse(LANGUAGE_FAMILY).unwrap(),
                &document,
                profile("2.20"),
                ObjectPath::root()
            ),
            Err(MetadataDecodeError::ProfileVersionMismatch { .. })
        ));
    }
}
