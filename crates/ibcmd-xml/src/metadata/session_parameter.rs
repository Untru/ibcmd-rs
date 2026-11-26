//! Strict canonical codec for `SessionParameter`.
//!
//! The source type list is retained as typed canonical records. Readable
//! `cfg:*` names are resolved to generated `TypeId` UUIDs only by the compiler,
//! after the complete validated configuration graph is available.

use std::collections::BTreeSet;

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::ObjectPath;
use ibcmd_core::family::FamilyId;
use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::model::CanonicalObject;
use ibcmd_core::value::{CanonicalInteger, CanonicalValue, EnumToken};

use super::common::{
    MD_NAMESPACE, MetadataDecodeError, MetadataEnvelope, ResolvedNamespaces, V8_NAMESPACE,
    XR_NAMESPACE, element_text, resolve_namespaces, typed, uri_of,
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

const SESSION_PARAMETER_FAMILY: &str = "SessionParameter";
const CFG_NAMESPACE: &str = "http://v8.1c.ru/8.1/data/enterprise/current-config";
const XS_NAMESPACE: &str = "http://www.w3.org/2001/XMLSchema";

/// Registers the exact `SessionParameter` codec.
pub fn register_session_parameter_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(SessionParameterCodec {
        family: FamilyId::parse(SESSION_PARAMETER_FAMILY).expect("family id is stable"),
    }))
}

struct SessionParameterCodec {
    family: FamilyId,
}

impl MetadataFamilyCodec for SessionParameterCodec {
    fn family_id(&self) -> &FamilyId {
        &self.family
    }

    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        decode_session_parameter(document, source, path)
    }

    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        encode_session_parameter(envelope, target)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SourceType {
    Boolean,
    String {
        length: u32,
        allowed_length: String,
    },
    Number {
        digits: u32,
        fraction_digits: u32,
        allowed_sign: String,
    },
    DateTime {
        date_fractions: String,
    },
    Reference(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Projection {
    comment: String,
    types: Vec<SourceType>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TypePatternGeneratedPolicy {
    Forbidden,
    DefinedType,
}

fn decode_session_parameter(
    document: &XmlDocument,
    source: ProfileId,
    path: ObjectPath,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    decode_type_pattern_family(
        document,
        source,
        path,
        SESSION_PARAMETER_FAMILY,
        TypePatternGeneratedPolicy::Forbidden,
    )
}

pub(super) fn decode_type_pattern_family(
    document: &XmlDocument,
    source: ProfileId,
    path: ObjectPath,
    family: &'static str,
    generated_policy: TypePatternGeneratedPolicy,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    validate_decode_profile(document, &source, &path)?;
    let projection = project(document, family, generated_policy)?;
    let generic = decode_metadata_envelope(document, source, path)?;
    if generic.root().kind().as_str() != family {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "type-pattern codec requires its exact family",
        ));
    }
    if !generic.descendants().is_empty()
        || !generic.root().references().is_empty()
        || !generic.root().assets().is_empty()
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "type-pattern metadata cannot contain children, references, or assets",
        ));
    }
    match generated_policy {
        TypePatternGeneratedPolicy::Forbidden if !generic.root().generated_types().is_empty() => {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "SessionParameter cannot contain generated types",
            ));
        }
        TypePatternGeneratedPolicy::DefinedType => {
            let generated = generic.root().generated_types();
            if generated.len() != 1
                || generated[0].kind().as_str() != "DefinedType"
                || generated[0].value_id().is_none()
            {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "DefinedType requires one typed TypeId/ValueId pair",
                ));
            }
        }
        TypePatternGeneratedPolicy::Forbidden => {}
    }
    let mut parts = copy_object_parts(generic.root());
    parts.properties.push(canonical_field(
        "Comment",
        CanonicalValue::text(canonical_text(&projection.comment)?),
    )?);
    let values = projection
        .types
        .iter()
        .map(source_type_value)
        .collect::<Result<Vec<_>, _>>()?;
    parts.properties.push(canonical_field(
        "Type",
        CanonicalValue::sequence(values)
            .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
    )?);
    let root = CanonicalObject::new(parts)
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))?;
    MetadataEnvelope::from_parts(root, Vec::new(), document.clone())
}

fn source_type_value(value: &SourceType) -> Result<CanonicalValue, MetadataDecodeError> {
    let mut fields = vec![canonical_field(
        "kind",
        enum_value(match value {
            SourceType::Boolean => "Boolean",
            SourceType::String { .. } => "String",
            SourceType::Number { .. } => "Number",
            SourceType::DateTime { .. } => "DateTime",
            SourceType::Reference(_) => "Reference",
        })?,
    )?];
    match value {
        SourceType::Boolean => {}
        SourceType::String {
            length,
            allowed_length,
        } => {
            fields.push(canonical_field("length", integer_value(*length)?)?);
            fields.push(canonical_field(
                "allowed_length",
                enum_value(allowed_length)?,
            )?);
        }
        SourceType::Number {
            digits,
            fraction_digits,
            allowed_sign,
        } => {
            fields.push(canonical_field("digits", integer_value(*digits)?)?);
            fields.push(canonical_field(
                "fraction_digits",
                integer_value(*fraction_digits)?,
            )?);
            fields.push(canonical_field("allowed_sign", enum_value(allowed_sign)?)?);
        }
        SourceType::DateTime { date_fractions } => fields.push(canonical_field(
            "date_fractions",
            enum_value(date_fractions)?,
        )?),
        SourceType::Reference(reference) => fields.push(canonical_field(
            "reference",
            CanonicalValue::text(canonical_text(reference)?),
        )?),
    }
    CanonicalValue::record(fields).map_err(|error| MetadataDecodeError::Core(error.to_string()))
}

fn enum_value(value: &str) -> Result<CanonicalValue, MetadataDecodeError> {
    EnumToken::new(value)
        .map(CanonicalValue::enum_token)
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))
}

fn integer_value(value: u32) -> Result<CanonicalValue, MetadataDecodeError> {
    CanonicalInteger::new(&value.to_string())
        .map(CanonicalValue::integer)
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))
}

fn project(
    document: &XmlDocument,
    family: &'static str,
    generated_policy: TypePatternGeneratedPolicy,
) -> Result<Projection, MetadataDecodeError> {
    let uris = resolve_namespaces(document.root())?;
    let expected = uri_of(document.root(), &uris);
    if !matches!(expected, None | Some(MD_NAMESPACE)) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "type-pattern metadata root namespace",
        ));
    }
    require_attributes(document.root(), &["version"])?;
    for (prefix, uri) in [
        ("v8", V8_NAMESPACE),
        ("cfg", CFG_NAMESPACE),
        ("xs", XS_NAMESPACE),
    ] {
        if !namespace_declared(document.root(), prefix, uri) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "type-pattern metadata requires exact v8, cfg, and xs namespace bindings",
            ));
        }
    }
    let object = required_child(document.root(), family, expected, &uris)?;
    require_attributes(object, &["uuid"])?;
    let properties = only_properties_child(object, expected, &uris, generated_policy)?;
    require_attributes(properties, &[])?;

    let mut seen = BTreeSet::new();
    let mut name = None;
    let mut comment = None;
    let mut types = None;
    for node in properties.children() {
        let XmlNode::Element(child) = node else {
            continue;
        };
        let local = child.name().local();
        if !matches!(local, "Name" | "Synonym" | "Comment" | "Type")
            || !typed(child, local, expected, &uris)
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "unknown type-pattern metadata property",
            ));
        }
        if !seen.insert(local) {
            return Err(MetadataDecodeError::Duplicate(match local {
                "Name" => "Name",
                "Synonym" => "Synonym",
                "Comment" => "Comment",
                "Type" => "Type",
                _ => unreachable!("allowed above"),
            }));
        }
        match local {
            "Name" => {
                require_attributes(child, &[])?;
                name = Some(element_text(child)?.ok_or(MetadataDecodeError::Missing("Name text"))?);
            }
            "Synonym" => validate_synonym_attributes(child)?,
            "Comment" => {
                require_attributes(child, &[])?;
                comment = Some(element_text(child)?.ok_or(
                    MetadataDecodeError::InvalidEnvelope("Comment must contain text only"),
                )?);
            }
            "Type" => {
                require_attributes(child, &[])?;
                types = Some(parse_types(child, &uris)?);
            }
            _ => unreachable!("allowed above"),
        }
    }
    for required in ["Name", "Synonym", "Comment", "Type"] {
        if !seen.contains(required) {
            return Err(MetadataDecodeError::Missing(required));
        }
    }
    if generated_policy == TypePatternGeneratedPolicy::DefinedType {
        validate_defined_type_internal_info(
            object,
            expected,
            &uris,
            name.as_deref().expect("presence checked"),
        )?;
    }
    Ok(Projection {
        comment: comment.expect("presence checked"),
        types: types.expect("presence checked"),
    })
}

fn parse_types(
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<Vec<SourceType>, MetadataDecodeError> {
    let mut names = Vec::new();
    let mut unique = BTreeSet::new();
    let mut string_qualifiers = None;
    let mut number_qualifiers = None;
    let mut date_qualifiers = None;
    for node in element.children() {
        let XmlNode::Element(child) = node else {
            continue;
        };
        if uri_of(child, uris) != Some(V8_NAMESPACE) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "Type contains an element outside the v8 namespace",
            ));
        }
        require_attributes(child, &[])?;
        match child.name().local() {
            "Type" => {
                let value = element_text(child)?.ok_or(MetadataDecodeError::InvalidEnvelope(
                    "Type scalar must contain text only",
                ))?;
                let value = value.trim().to_owned();
                if value.is_empty() || !unique.insert(value.clone()) {
                    return Err(MetadataDecodeError::InvalidEnvelope(
                        "Type scalar is empty or duplicated",
                    ));
                }
                names.push(value);
            }
            "StringQualifiers" => set_once(
                &mut string_qualifiers,
                parse_string_qualifiers(child, uris)?,
                "StringQualifiers",
            )?,
            "NumberQualifiers" => set_once(
                &mut number_qualifiers,
                parse_number_qualifiers(child, uris)?,
                "NumberQualifiers",
            )?,
            "DateQualifiers" => set_once(
                &mut date_qualifiers,
                parse_date_qualifiers(child, uris)?,
                "DateQualifiers",
            )?,
            _ => {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "unknown SessionParameter Type child",
                ));
            }
        }
    }
    if names.is_empty() {
        return Err(MetadataDecodeError::Missing("Type scalar"));
    }
    let string_count = names
        .iter()
        .filter(|name| name.as_str() == "xs:string")
        .count();
    let number_count = names
        .iter()
        .filter(|name| name.as_str() == "xs:decimal")
        .count();
    let date_count = names
        .iter()
        .filter(|name| name.as_str() == "xs:dateTime")
        .count();
    if string_count > 1 || number_count > 1 || date_count > 1 {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "qualified primitive Type occurs more than once",
        ));
    }
    if string_count != usize::from(string_qualifiers.is_some())
        || number_count != usize::from(number_qualifiers.is_some())
        || date_count != usize::from(date_qualifiers.is_some())
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "Type qualifiers do not match primitive scalars",
        ));
    }

    let mut result = Vec::with_capacity(names.len());
    for name in names {
        result.push(match name.as_str() {
            "xs:boolean" => SourceType::Boolean,
            "xs:string" => {
                let (length, allowed_length) =
                    string_qualifiers.clone().expect("presence matched above");
                SourceType::String {
                    length,
                    allowed_length,
                }
            }
            "xs:decimal" => {
                let (digits, fraction_digits, allowed_sign) =
                    number_qualifiers.clone().expect("presence matched above");
                SourceType::Number {
                    digits,
                    fraction_digits,
                    allowed_sign,
                }
            }
            "xs:dateTime" => SourceType::DateTime {
                date_fractions: date_qualifiers.clone().expect("presence matched above"),
            },
            reference if supported_reference_name(reference) => {
                SourceType::Reference(reference.to_owned())
            }
            _ => {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "unsupported SessionParameter Type scalar",
                ));
            }
        });
    }
    Ok(result)
}

fn supported_reference_name(value: &str) -> bool {
    matches!(
        value,
        "v8:FixedArray" | "v8:FixedMap" | "v8:FixedStructure" | "v8:UUID" | "v8:ValueStorage"
    ) || value
        .strip_prefix("cfg:")
        .is_some_and(|value| !value.is_empty() && !value.chars().any(char::is_whitespace))
}

fn parse_string_qualifiers(
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<(u32, String), MetadataDecodeError> {
    let length = parse_u32(&required_v8_text(element, "Length", uris)?)?;
    let allowed = required_v8_text(element, "AllowedLength", uris)?;
    let allowed = allowed.trim();
    if !matches!(allowed, "Fixed" | "Variable") {
        return Err(MetadataDecodeError::InvalidEnvelope("AllowedLength"));
    }
    reject_unknown_children(element, &["Length", "AllowedLength"], uris)?;
    Ok((length, allowed.to_owned()))
}

fn parse_number_qualifiers(
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<(u32, u32, String), MetadataDecodeError> {
    let digits = parse_u32(&required_v8_text(element, "Digits", uris)?)?;
    let fraction_digits = parse_u32(&required_v8_text(element, "FractionDigits", uris)?)?;
    if fraction_digits > digits {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "FractionDigits exceeds Digits",
        ));
    }
    let allowed = required_v8_text(element, "AllowedSign", uris)?;
    let allowed = allowed.trim();
    if !matches!(allowed, "Any" | "Nonnegative") {
        return Err(MetadataDecodeError::InvalidEnvelope("AllowedSign"));
    }
    reject_unknown_children(element, &["Digits", "FractionDigits", "AllowedSign"], uris)?;
    Ok((digits, fraction_digits, allowed.to_owned()))
}

fn parse_date_qualifiers(
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<String, MetadataDecodeError> {
    let fractions = required_v8_text(element, "DateFractions", uris)?;
    let fractions = fractions.trim();
    if !matches!(fractions, "Date" | "Time" | "DateTime") {
        return Err(MetadataDecodeError::InvalidEnvelope("DateFractions"));
    }
    reject_unknown_children(element, &["DateFractions"], uris)?;
    Ok(fractions.to_owned())
}

fn required_v8_text(
    parent: &XmlElement,
    local: &'static str,
    uris: &ResolvedNamespaces,
) -> Result<String, MetadataDecodeError> {
    let child = required_child(parent, local, Some(V8_NAMESPACE), uris)?;
    require_attributes(child, &[])?;
    element_text(child)?.ok_or(MetadataDecodeError::InvalidEnvelope(
        "typed qualifier must contain text only",
    ))
}

fn reject_unknown_children(
    parent: &XmlElement,
    allowed: &[&str],
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    for node in parent.children() {
        if let XmlNode::Element(child) = node
            && (uri_of(child, uris) != Some(V8_NAMESPACE)
                || !allowed.contains(&child.name().local()))
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "unknown SessionParameter qualifier child",
            ));
        }
    }
    Ok(())
}

fn parse_u32(value: &str) -> Result<u32, MetadataDecodeError> {
    value
        .trim()
        .parse::<u32>()
        .map_err(|_| MetadataDecodeError::InvalidEnvelope("qualifier is not u32"))
}

fn set_once<T>(
    slot: &mut Option<T>,
    value: T,
    field: &'static str,
) -> Result<(), MetadataDecodeError> {
    if slot.replace(value).is_some() {
        return Err(MetadataDecodeError::Duplicate(field));
    }
    Ok(())
}

fn only_properties_child<'a>(
    object: &'a XmlElement,
    expected: Option<&str>,
    uris: &ResolvedNamespaces,
    generated_policy: TypePatternGeneratedPolicy,
) -> Result<&'a XmlElement, MetadataDecodeError> {
    let properties = required_child(object, "Properties", expected, uris)?;
    let elements = object
        .children()
        .iter()
        .filter_map(|node| match node {
            XmlNode::Element(child) => Some(child),
            _ => None,
        })
        .collect::<Vec<_>>();
    let valid = match generated_policy {
        TypePatternGeneratedPolicy::Forbidden => {
            elements.len() == 1 && std::ptr::eq(elements[0], properties)
        }
        TypePatternGeneratedPolicy::DefinedType => {
            elements.len() == 2
                && typed(elements[0], "InternalInfo", expected, uris)
                && std::ptr::eq(elements[1], properties)
        }
    };
    if !valid {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "type-pattern metadata object children or order are unsupported",
        ));
    }
    Ok(properties)
}

fn validate_defined_type_internal_info(
    object: &XmlElement,
    expected: Option<&str>,
    uris: &ResolvedNamespaces,
    name: &str,
) -> Result<(), MetadataDecodeError> {
    let internal = required_child(object, "InternalInfo", expected, uris)?;
    require_attributes(internal, &[])?;
    let generated = internal
        .children()
        .iter()
        .filter_map(|node| match node {
            XmlNode::Element(child) => Some(child),
            _ => None,
        })
        .collect::<Vec<_>>();
    if generated.len() != 1 || !typed(generated[0], "GeneratedType", Some(XR_NAMESPACE), uris) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "DefinedType InternalInfo must contain one xr:GeneratedType",
        ));
    }
    let generated = generated[0];
    require_attributes(generated, &["name", "category"])?;
    let expected_name = format!("DefinedType.{name}");
    if unprefixed_attribute(generated, "name") != Some(expected_name.as_str())
        || unprefixed_attribute(generated, "category") != Some("DefinedType")
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "DefinedType generated type name or category is inconsistent",
        ));
    }
    let values = generated
        .children()
        .iter()
        .filter_map(|node| match node {
            XmlNode::Element(child) => Some(child),
            _ => None,
        })
        .collect::<Vec<_>>();
    if values.len() != 2
        || !typed(values[0], "TypeId", Some(XR_NAMESPACE), uris)
        || !typed(values[1], "ValueId", Some(XR_NAMESPACE), uris)
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "DefinedType generated TypeId/ValueId fields or order are unsupported",
        ));
    }
    for value in values {
        require_attributes(value, &[])?;
        let text = element_text(value)?.ok_or(MetadataDecodeError::InvalidEnvelope(
            "DefinedType generated UUID must contain text only",
        ))?;
        let uuid = ObjectUuid::parse(text.trim())
            .map_err(|_| MetadataDecodeError::InvalidUuid(text.clone()))?;
        if uuid.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "DefinedType generated UUID cannot be nil",
            ));
        }
    }
    Ok(())
}

fn unprefixed_attribute<'a>(element: &'a XmlElement, local: &str) -> Option<&'a str> {
    element.attributes().iter().find_map(|attribute| {
        if let AttributeKind::Ordinary(name) = attribute.kind()
            && name.prefix().is_none()
            && name.local() == local
        {
            Some(attribute.value())
        } else {
            None
        }
    })
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
                    "unknown type-pattern metadata semantic attribute",
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

fn encode_session_parameter(
    envelope: &MetadataEnvelope,
    target: &ProfileId,
) -> Result<Vec<u8>, MetadataEncodeError> {
    encode_type_pattern_family(
        envelope,
        target,
        SESSION_PARAMETER_FAMILY,
        TypePatternGeneratedPolicy::Forbidden,
    )
}

pub(super) fn encode_type_pattern_family(
    envelope: &MetadataEnvelope,
    target: &ProfileId,
    family: &'static str,
    generated_policy: TypePatternGeneratedPolicy,
) -> Result<Vec<u8>, MetadataEncodeError> {
    let path = envelope.root().identity().path().clone();
    let target_version =
        profile_version(target).ok_or_else(|| MetadataEncodeError::UnsupportedProfile {
            object_path: path.clone(),
            profile: target.clone(),
        })?;
    if envelope.root().kind().as_str() != family {
        return Err(invalid_model(&path, "kind"));
    }
    let source_profile = envelope.root().provenance().source_profile().clone();
    validate_decode_profile(envelope.source_document(), &source_profile, &path)
        .map_err(decode_to_encode)?;
    let source = decode_type_pattern_family(
        envelope.source_document(),
        source_profile,
        path.clone(),
        family,
        generated_policy,
    )
    .map_err(decode_to_encode)?;
    if source.root() != envelope.root() || source.descendants() != envelope.descendants() {
        return Err(invalid_model(
            &path,
            "type-pattern metadata mutation is not implemented",
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

    fn fixture(version: &str, type_body: &str, extra: &str) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"{MD_NAMESPACE}\" xmlns:v8=\"{V8_NAMESPACE}\" xmlns:cfg=\"{CFG_NAMESPACE}\" xmlns:xs=\"{XS_NAMESPACE}\" version=\"{version}\">\r\n\
\t<SessionParameter uuid=\"{UUID}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>CurrentUser</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>en</v8:lang><v8:content>Current user</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<Type>{type_body}</Type>{extra}\r\n\
\t\t</Properties>\r\n\
\t</SessionParameter>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    fn profile(version: &str) -> ProfileId {
        ProfileId::parse(&format!("xml-{version}")).unwrap()
    }

    #[test]
    fn mixed_types_are_typed_and_cross_version_roundtrip_is_strict() {
        let type_body = "<v8:Type>xs:boolean</v8:Type><v8:Type>xs:string</v8:Type><v8:StringQualifiers><v8:Length>80</v8:Length><v8:AllowedLength>Variable</v8:AllowedLength></v8:StringQualifiers><v8:Type>cfg:CatalogRef.Users</v8:Type>";
        for version in ["2.20", "2.21"] {
            let document = XmlReader::from_slice(&fixture(version, type_body, "")).unwrap();
            let mut registry = MetadataRegistry::default();
            register_session_parameter_codec(&mut registry).unwrap();
            let envelope = registry
                .decode(
                    &FamilyId::parse(SESSION_PARAMETER_FAMILY).unwrap(),
                    &document,
                    profile(version),
                    ObjectPath::root(),
                )
                .unwrap();
            let types = envelope.root().properties()[3]
                .value()
                .as_sequence()
                .unwrap();
            assert_eq!(types.len(), 3);
            assert!(matches!(types[0].kind(), CanonicalValueKind::Record(_)));
            let target = if version == "2.20" { "2.21" } else { "2.20" };
            let output = registry.encode(&envelope, &profile(target)).unwrap();
            let reparsed = XmlReader::from_slice(&output).unwrap();
            registry
                .decode(
                    &FamilyId::parse(SESSION_PARAMETER_FAMILY).unwrap(),
                    &reparsed,
                    profile(target),
                    ObjectPath::root(),
                )
                .unwrap();
        }
    }

    #[test]
    fn qualifier_mismatch_and_unknown_property_fail_closed() {
        let mut registry = MetadataRegistry::default();
        register_session_parameter_codec(&mut registry).unwrap();
        let mismatch = XmlReader::from_slice(&fixture(
            "2.20",
            "<v8:Type>xs:boolean</v8:Type><v8:StringQualifiers><v8:Length>1</v8:Length><v8:AllowedLength>Variable</v8:AllowedLength></v8:StringQualifiers>",
            "",
        ))
        .unwrap();
        assert!(
            registry
                .decode(
                    &FamilyId::parse(SESSION_PARAMETER_FAMILY).unwrap(),
                    &mismatch,
                    profile("2.20"),
                    ObjectPath::root(),
                )
                .is_err()
        );
        let unknown = XmlReader::from_slice(&fixture(
            "2.20",
            "<v8:Type>xs:boolean</v8:Type>",
            "<Future/>",
        ))
        .unwrap();
        assert!(
            registry
                .decode(
                    &FamilyId::parse(SESSION_PARAMETER_FAMILY).unwrap(),
                    &unknown,
                    profile("2.20"),
                    ObjectPath::root(),
                )
                .is_err()
        );
    }
}
