//! Offline canonical codec for the `Constant` metadata family.
//!
//! The stable canonical schema extends the common `Name` / `Synonym` fields
//! with `Comment: Text`, `UseStandardCommands: Bool`, and `Type: Record`.
//! `Type.kind` is one of `Boolean`, `String`, `Number`, `DateTime`,
//! `Reference`, or `ReferenceTypeSet`; the remaining record fields are the
//! corresponding XML qualifiers in source order.

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::ObjectPath;
use ibcmd_core::family::FamilyId;
use ibcmd_core::model::{CanonicalObject, CanonicalObjectParts, GeneratedType};
use ibcmd_core::value::{
    CanonicalField, CanonicalInteger, CanonicalText, CanonicalValue, CanonicalValueKind, EnumToken,
};

use super::common::{
    MD_NAMESPACE, MetadataDecodeError, MetadataEnvelope, ResolvedNamespaces, V8_NAMESPACE,
    XR_NAMESPACE, element_text, resolve_namespaces, typed, uri_of,
};
use super::decode_metadata_envelope;
use super::registry::{
    MetadataEncodeError, MetadataFamilyCodec, MetadataRegistry, MetadataRegistryError,
};
use crate::{
    Attribute, AttributeKind, LexicalPolicy, QName, XmlDocument, XmlElement, XmlNode, XmlWriter,
};

const CONSTANT_FAMILY: &str = "Constant";
const PROFILE_220: &str = "xml-2.20";
const PROFILE_221: &str = "xml-2.21";
const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";

/// Registers the exact `Constant` family codec in a caller-owned registry.
pub fn register_constant_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(ConstantCodec {
        family: FamilyId::parse(CONSTANT_FAMILY).expect("constant family id is stable"),
    }))
}

/// Returns the built-in offline metadata registry.
///
/// `MetadataRegistry::default()` deliberately remains empty so applications
/// can choose their codec set without hidden global mutation.
pub fn bundled_metadata_registry() -> MetadataRegistry {
    let mut registry = MetadataRegistry::default();
    register_constant_codec(&mut registry).expect("bundled families are unique and bounded");
    super::defined_type::register_defined_type_codec(&mut registry)
        .expect("bundled families are unique and bounded");
    super::functional_option::register_functional_option_codec(&mut registry)
        .expect("bundled families are unique and bounded");
    super::functional_options_parameter::register_functional_options_parameter_codec(&mut registry)
        .expect("bundled families are unique and bounded");
    super::language::register_language_codec(&mut registry)
        .expect("bundled families are unique and bounded");
    super::session_parameter::register_session_parameter_codec(&mut registry)
        .expect("bundled families are unique and bounded");
    super::services::register_event_subscription_codec(&mut registry)
        .expect("bundled families are unique and bounded");
    super::services::register_http_service_codec(&mut registry)
        .expect("bundled families are unique and bounded");
    super::services::register_web_service_codec(&mut registry)
        .expect("bundled families are unique and bounded");
    super::services::register_ws_reference_codec(&mut registry)
        .expect("bundled families are unique and bounded");
    super::services::register_scheduled_job_codec(&mut registry)
        .expect("bundled families are unique and bounded");
    super::services::register_xdto_package_codec(&mut registry)
        .expect("bundled families are unique and bounded");
    registry
}

struct ConstantCodec {
    family: FamilyId,
}

impl MetadataFamilyCodec for ConstantCodec {
    fn family_id(&self) -> &FamilyId {
        &self.family
    }

    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        decode_constant(document, source, path)
    }

    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        encode_constant(envelope, target)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ConstantType {
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
    Reference {
        reference: String,
    },
    ReferenceTypeSet {
        reference: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ConstantProjection {
    comment: String,
    value_type: ConstantType,
    use_standard_commands: bool,
}

fn profile_version(profile: &ProfileId) -> Option<&'static str> {
    match profile.as_str() {
        PROFILE_220 => Some("2.20"),
        PROFILE_221 => Some("2.21"),
        _ => None,
    }
}

fn root_version(root: &XmlElement) -> Result<&str, MetadataDecodeError> {
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

fn validate_decode_profile(
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

fn decode_constant(
    document: &XmlDocument,
    source: ProfileId,
    path: ObjectPath,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    validate_decode_profile(document, &source, &path)?;
    let generic = decode_metadata_envelope(document, source, path)?;
    if generic.root().kind().as_str() != CONSTANT_FAMILY {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "constant codec requires Constant family",
        ));
    }
    let projection = project_constant(document)?;
    let mut parts = copy_object_parts(generic.root());
    parts.properties.push(canonical_field(
        "Comment",
        CanonicalValue::text(canonical_text(&projection.comment)?),
    )?);
    parts.properties.push(canonical_field(
        "Type",
        constant_type_value(&projection.value_type)?,
    )?);
    parts.properties.push(canonical_field(
        "UseStandardCommands",
        CanonicalValue::boolean(projection.use_standard_commands),
    )?);
    let root = CanonicalObject::new(parts)
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))?;
    MetadataEnvelope::from_parts(root, generic.descendants().to_vec(), document.clone())
}

fn copy_object_parts(object: &CanonicalObject) -> CanonicalObjectParts {
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

fn canonical_text(value: &str) -> Result<CanonicalText, MetadataDecodeError> {
    CanonicalText::new(value).map_err(|error| MetadataDecodeError::Core(error.to_string()))
}

fn canonical_field(
    name: &str,
    value: CanonicalValue,
) -> Result<CanonicalField, MetadataDecodeError> {
    CanonicalField::named(name, value).map_err(|error| MetadataDecodeError::Core(error.to_string()))
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

fn constant_type_value(value: &ConstantType) -> Result<CanonicalValue, MetadataDecodeError> {
    let mut fields = vec![canonical_field(
        "kind",
        enum_value(match value {
            ConstantType::Boolean => "Boolean",
            ConstantType::String { .. } => "String",
            ConstantType::Number { .. } => "Number",
            ConstantType::DateTime { .. } => "DateTime",
            ConstantType::Reference { .. } => "Reference",
            ConstantType::ReferenceTypeSet { .. } => "ReferenceTypeSet",
        })?,
    )?];
    match value {
        ConstantType::Boolean => {}
        ConstantType::String {
            length,
            allowed_length,
        } => {
            fields.push(canonical_field("length", integer_value(*length)?)?);
            fields.push(canonical_field(
                "allowed_length",
                enum_value(allowed_length)?,
            )?);
        }
        ConstantType::Number {
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
        ConstantType::DateTime { date_fractions } => fields.push(canonical_field(
            "date_fractions",
            enum_value(date_fractions)?,
        )?),
        ConstantType::Reference { reference } | ConstantType::ReferenceTypeSet { reference } => {
            fields.push(canonical_field(
                "reference",
                CanonicalValue::text(canonical_text(reference)?),
            )?)
        }
    }
    CanonicalValue::record(fields).map_err(|error| MetadataDecodeError::Core(error.to_string()))
}

fn project_constant(document: &XmlDocument) -> Result<ConstantProjection, MetadataDecodeError> {
    let uris = resolve_namespaces(document.root())?;
    let expected = uri_of(document.root(), &uris);
    if !matches!(expected, None | Some(MD_NAMESPACE)) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "constant root namespace",
        ));
    }
    let object = required_child(document.root(), CONSTANT_FAMILY, expected, &uris)?;
    let properties = required_child(object, "Properties", expected, &uris)?;
    let comment = required_text_child(properties, "Comment", expected, &uris)?;
    let type_element = required_child(properties, "Type", expected, &uris)?;
    let value_type = parse_constant_type(type_element, &uris)?;
    let use_standard_commands =
        match required_text_child(properties, "UseStandardCommands", expected, &uris)?.trim() {
            "true" => true,
            "false" => false,
            _ => {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "UseStandardCommands boolean",
                ));
            }
        };
    Ok(ConstantProjection {
        comment,
        value_type,
        use_standard_commands,
    })
}

fn required_child<'a>(
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
        "typed property must contain text only",
    ))
}

fn parse_constant_type(
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<ConstantType, MetadataDecodeError> {
    let mut scalar: Option<(&str, String)> = None;
    let mut string_qualifiers = None;
    let mut number_qualifiers = None;
    let mut date_qualifiers = None;
    for node in element.children() {
        let XmlNode::Element(child) = node else {
            continue;
        };
        if uri_of(child, uris) != Some(V8_NAMESPACE) {
            continue;
        }
        match child.name().local() {
            "Type" | "TypeSet" => {
                if scalar.is_some() {
                    return Err(MetadataDecodeError::Duplicate("Type scalar"));
                }
                scalar = Some((
                    child.name().local(),
                    element_text(child)?.ok_or(MetadataDecodeError::InvalidEnvelope(
                        "Type scalar must contain text only",
                    ))?,
                ));
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
            _ => {}
        }
    }
    let (tag, value) = scalar.ok_or(MetadataDecodeError::Missing("Type scalar"))?;
    let value = value.trim();
    let result = match (tag, value) {
        ("Type", "xs:boolean") => ConstantType::Boolean,
        ("Type", "xs:string") => {
            let (length, allowed_length) = string_qualifiers
                .take()
                .ok_or(MetadataDecodeError::Missing("StringQualifiers"))?;
            ConstantType::String {
                length,
                allowed_length,
            }
        }
        ("Type", "xs:decimal") => {
            let (digits, fraction_digits, allowed_sign) = number_qualifiers
                .take()
                .ok_or(MetadataDecodeError::Missing("NumberQualifiers"))?;
            ConstantType::Number {
                digits,
                fraction_digits,
                allowed_sign,
            }
        }
        ("Type", "xs:dateTime") => ConstantType::DateTime {
            date_fractions: date_qualifiers
                .take()
                .ok_or(MetadataDecodeError::Missing("DateQualifiers"))?,
        },
        ("Type", reference) if !reference.is_empty() => ConstantType::Reference {
            reference: reference.to_owned(),
        },
        ("TypeSet", reference) if !reference.is_empty() => ConstantType::ReferenceTypeSet {
            reference: reference.to_owned(),
        },
        _ => return Err(MetadataDecodeError::InvalidEnvelope("Constant Type scalar")),
    };
    if string_qualifiers.is_some() || number_qualifiers.is_some() || date_qualifiers.is_some() {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "qualifier does not match Constant Type",
        ));
    }
    Ok(result)
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

fn parse_string_qualifiers(
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<(u32, String), MetadataDecodeError> {
    let length = parse_u32(&required_text_child(
        element,
        "Length",
        Some(V8_NAMESPACE),
        uris,
    )?)?;
    let allowed = required_text_child(element, "AllowedLength", Some(V8_NAMESPACE), uris)?;
    let allowed = allowed.trim();
    if !matches!(allowed, "Fixed" | "Variable") {
        return Err(MetadataDecodeError::InvalidEnvelope("AllowedLength"));
    }
    Ok((length, allowed.to_owned()))
}

fn parse_number_qualifiers(
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<(u32, u32, String), MetadataDecodeError> {
    let digits = parse_u32(&required_text_child(
        element,
        "Digits",
        Some(V8_NAMESPACE),
        uris,
    )?)?;
    let fraction_digits = parse_u32(&required_text_child(
        element,
        "FractionDigits",
        Some(V8_NAMESPACE),
        uris,
    )?)?;
    let allowed = required_text_child(element, "AllowedSign", Some(V8_NAMESPACE), uris)?;
    let allowed = allowed.trim();
    if !matches!(allowed, "Any" | "Nonnegative") {
        return Err(MetadataDecodeError::InvalidEnvelope("AllowedSign"));
    }
    Ok((digits, fraction_digits, allowed.to_owned()))
}

fn parse_date_qualifiers(
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<String, MetadataDecodeError> {
    let fractions = required_text_child(element, "DateFractions", Some(V8_NAMESPACE), uris)?;
    let fractions = fractions.trim();
    if !matches!(fractions, "Date" | "Time" | "DateTime") {
        return Err(MetadataDecodeError::InvalidEnvelope("DateFractions"));
    }
    Ok(fractions.to_owned())
}

fn parse_u32(value: &str) -> Result<u32, MetadataDecodeError> {
    value
        .trim()
        .parse()
        .map_err(|_| MetadataDecodeError::InvalidEnvelope("u32 qualifier"))
}

fn encode_constant(
    envelope: &MetadataEnvelope,
    target: &ProfileId,
) -> Result<Vec<u8>, MetadataEncodeError> {
    let path = envelope.root().identity().path().clone();
    let target_version =
        profile_version(target).ok_or_else(|| MetadataEncodeError::UnsupportedProfile {
            object_path: path.clone(),
            profile: target.clone(),
        })?;
    if envelope.root().kind().as_str() != CONSTANT_FAMILY {
        return Err(invalid_model(&path, "kind"));
    }
    let source_profile = envelope.root().provenance().source_profile().clone();
    validate_decode_profile(envelope.source_document(), &source_profile, &path)
        .map_err(decode_to_encode)?;
    let source = decode_constant(envelope.source_document(), source_profile, path.clone())
        .map_err(decode_to_encode)?;
    validate_patchable_model(&source, envelope, &path)?;
    let desired_projection = projection_from_object(envelope.root(), &path)?;
    let source_projection = projection_from_object(source.root(), &path)?;
    let uris = resolve_namespaces(envelope.source_document().root()).map_err(decode_to_encode)?;
    let document = patch_document(
        envelope.source_document(),
        &uris,
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

fn decode_to_encode(error: MetadataDecodeError) -> MetadataEncodeError {
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

fn invalid_model(path: &ObjectPath, field: &'static str) -> MetadataEncodeError {
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
    if source_root.assets() != desired_root.assets() {
        return Err(invalid_model(path, "assets"));
    }
    if source_root.opaque_facets() != desired_root.opaque_facets() {
        return Err(invalid_model(path, "opaque facets"));
    }
    if source.descendants() != desired.descendants() {
        return Err(invalid_model(path, "descendants"));
    }
    if source_root.generated_types().len() != desired_root.generated_types().len() {
        return Err(invalid_model(path, "generated type count"));
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

fn projection_from_object(
    object: &CanonicalObject,
    path: &ObjectPath,
) -> Result<ConstantProjection, MetadataEncodeError> {
    let comment = value_text(property(object, "Comment", path)?, path, "Comment")?.to_owned();
    let value_type = constant_type_from_value(property(object, "Type", path)?, path)?;
    let use_standard_commands = match property(object, "UseStandardCommands", path)?.kind() {
        CanonicalValueKind::Bool(value) => value,
        _ => return Err(invalid_model(path, "UseStandardCommands")),
    };
    Ok(ConstantProjection {
        comment,
        value_type,
        use_standard_commands,
    })
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

fn value_enum<'a>(
    value: &'a CanonicalValue,
    path: &ObjectPath,
    field: &'static str,
) -> Result<&'a str, MetadataEncodeError> {
    match value.kind() {
        CanonicalValueKind::EnumToken(value) => Ok(value.as_str()),
        _ => Err(invalid_model(path, field)),
    }
}

fn value_u32(
    value: &CanonicalValue,
    path: &ObjectPath,
    field: &'static str,
) -> Result<u32, MetadataEncodeError> {
    match value.kind() {
        CanonicalValueKind::Integer(value) => value
            .as_str()
            .parse()
            .map_err(|_| invalid_model(path, field)),
        _ => Err(invalid_model(path, field)),
    }
}

fn record_field<'a>(
    fields: &'a [CanonicalField],
    name: &'static str,
    path: &ObjectPath,
) -> Result<&'a CanonicalValue, MetadataEncodeError> {
    fields
        .iter()
        .find(|field| field.name().as_str() == name)
        .map(CanonicalField::value)
        .ok_or_else(|| invalid_model(path, "Type"))
}

fn constant_type_from_value(
    value: &CanonicalValue,
    path: &ObjectPath,
) -> Result<ConstantType, MetadataEncodeError> {
    let fields = value
        .as_record()
        .ok_or_else(|| invalid_model(path, "Type"))?;
    let kind = value_enum(record_field(fields, "kind", path)?, path, "Type")?;
    let expected_fields: &[&str] = match kind {
        "Boolean" => &["kind"],
        "String" => &["kind", "length", "allowed_length"],
        "Number" => &["kind", "digits", "fraction_digits", "allowed_sign"],
        "DateTime" => &["kind", "date_fractions"],
        "Reference" | "ReferenceTypeSet" => &["kind", "reference"],
        _ => return Err(invalid_model(path, "Type.kind")),
    };
    if fields.len() != expected_fields.len()
        || fields
            .iter()
            .zip(expected_fields)
            .any(|(field, expected)| field.name().as_str() != *expected)
    {
        return Err(invalid_model(path, "Type schema"));
    }
    match kind {
        "Boolean" => Ok(ConstantType::Boolean),
        "String" => {
            let allowed_length = value_enum(
                record_field(fields, "allowed_length", path)?,
                path,
                "Type.allowed_length",
            )?;
            if !matches!(allowed_length, "Fixed" | "Variable") {
                return Err(invalid_model(path, "Type.allowed_length"));
            }
            Ok(ConstantType::String {
                length: value_u32(record_field(fields, "length", path)?, path, "Type.length")?,
                allowed_length: allowed_length.to_owned(),
            })
        }
        "Number" => {
            let allowed_sign = value_enum(
                record_field(fields, "allowed_sign", path)?,
                path,
                "Type.allowed_sign",
            )?;
            if !matches!(allowed_sign, "Any" | "Nonnegative") {
                return Err(invalid_model(path, "Type.allowed_sign"));
            }
            Ok(ConstantType::Number {
                digits: value_u32(record_field(fields, "digits", path)?, path, "Type.digits")?,
                fraction_digits: value_u32(
                    record_field(fields, "fraction_digits", path)?,
                    path,
                    "Type.fraction_digits",
                )?,
                allowed_sign: allowed_sign.to_owned(),
            })
        }
        "DateTime" => {
            let fractions = value_enum(
                record_field(fields, "date_fractions", path)?,
                path,
                "Type.date_fractions",
            )?;
            if !matches!(fractions, "Date" | "Time" | "DateTime") {
                return Err(invalid_model(path, "Type.date_fractions"));
            }
            Ok(ConstantType::DateTime {
                date_fractions: fractions.to_owned(),
            })
        }
        "Reference" | "ReferenceTypeSet" => {
            let reference = value_text(
                record_field(fields, "reference", path)?,
                path,
                "Type.reference",
            )?;
            if reference.is_empty() {
                return Err(invalid_model(path, "Type.reference"));
            }
            if kind == "Reference" {
                Ok(ConstantType::Reference {
                    reference: reference.to_owned(),
                })
            } else {
                Ok(ConstantType::ReferenceTypeSet {
                    reference: reference.to_owned(),
                })
            }
        }
        _ => unreachable!("kind checked above"),
    }
}

#[allow(clippy::too_many_arguments)]
fn patch_document(
    document: &XmlDocument,
    uris: &ResolvedNamespaces,
    source: &CanonicalObject,
    desired: &CanonicalObject,
    source_projection: &ConstantProjection,
    desired_projection: &ConstantProjection,
    target_version: &str,
    path: &ObjectPath,
) -> Result<XmlDocument, MetadataEncodeError> {
    let expected = uri_of(document.root(), uris);
    let object = required_child(document.root(), CONSTANT_FAMILY, expected, uris)
        .map_err(decode_to_encode)?;
    let patched_object = patch_object(
        object,
        uris,
        expected,
        source,
        desired,
        source_projection,
        desired_projection,
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
        set_unprefixed_attribute(&root, "version", target_version, false, path)?
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
    source_projection: &ConstantProjection,
    desired_projection: &ConstantProjection,
    path: &ObjectPath,
) -> Result<XmlElement, MetadataEncodeError> {
    let properties =
        required_child(object, "Properties", expected, uris).map_err(decode_to_encode)?;
    let type_element =
        required_child(properties, "Type", expected, uris).map_err(decode_to_encode)?;
    let core_prefix = type_element.children().iter().find_map(|node| match node {
        XmlNode::Element(element) if uri_of(element, uris) == Some(V8_NAMESPACE) => {
            element.name().prefix().map(str::to_owned)
        }
        _ => None,
    });
    let mut generated_index = 0usize;
    let mut children = Vec::with_capacity(object.children().len());
    for node in object.children() {
        let patched = match node {
            XmlNode::Element(child) if typed(child, "Properties", expected, uris) => {
                XmlNode::Element(patch_properties(
                    child,
                    uris,
                    expected,
                    source,
                    desired,
                    source_projection,
                    desired_projection,
                    core_prefix.as_deref(),
                    &mut generated_index,
                    path,
                )?)
            }
            XmlNode::Element(child) if typed(child, "GeneratedTypes", expected, uris) => {
                XmlNode::Element(patch_generated_container(
                    child,
                    uris,
                    expected,
                    source.generated_types(),
                    desired.generated_types(),
                    &mut generated_index,
                    path,
                )?)
            }
            XmlNode::Element(child) if typed(child, "InternalInfo", expected, uris) => {
                XmlNode::Element(patch_generated_container(
                    child,
                    uris,
                    if expected.is_none() {
                        None
                    } else {
                        Some(XR_NAMESPACE)
                    },
                    source.generated_types(),
                    desired.generated_types(),
                    &mut generated_index,
                    path,
                )?)
            }
            _ => node.clone(),
        };
        children.push(patched);
    }
    if generated_index != desired.generated_types().len() {
        return Err(invalid_model(path, "generated type slots"));
    }
    let patched = object.with_children(children);
    if source.identity().uuid() == desired.identity().uuid() {
        Ok(patched)
    } else {
        set_unprefixed_attribute(
            &patched,
            "uuid",
            &desired.identity().uuid().to_string(),
            false,
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
    source_projection: &ConstantProjection,
    desired_projection: &ConstantProjection,
    core_prefix: Option<&str>,
    generated_index: &mut usize,
    path: &ObjectPath,
) -> Result<XmlElement, MetadataEncodeError> {
    let source_name = property(source, "Name", path)?;
    let desired_name = property(desired, "Name", path)?;
    let source_synonym = optional_property(source, "Synonym");
    let desired_synonym = optional_property(desired, "Synonym");
    if source_synonym.is_some() != desired_synonym.is_some() {
        return Err(invalid_model(path, "Synonym source slot"));
    }
    let mut children = Vec::with_capacity(properties.children().len());
    for node in properties.children() {
        let patched = match node {
            XmlNode::Element(child) if typed(child, "Name", expected, uris) => {
                if source_name == desired_name {
                    node.clone()
                } else {
                    XmlNode::Element(replace_text(child, value_text(desired_name, path, "Name")?))
                }
            }
            XmlNode::Element(child) if typed(child, "Synonym", expected, uris) => {
                if source_synonym == desired_synonym {
                    node.clone()
                } else {
                    XmlNode::Element(render_synonym(
                        child,
                        desired_synonym.ok_or_else(|| invalid_model(path, "Synonym"))?,
                        core_prefix,
                        path,
                    )?)
                }
            }
            XmlNode::Element(child) if typed(child, "Comment", expected, uris) => {
                if source_projection.comment == desired_projection.comment {
                    node.clone()
                } else {
                    XmlNode::Element(replace_text(child, &desired_projection.comment))
                }
            }
            XmlNode::Element(child) if typed(child, "Type", expected, uris) => {
                if source_projection.value_type == desired_projection.value_type {
                    node.clone()
                } else {
                    XmlNode::Element(render_type(
                        child,
                        &desired_projection.value_type,
                        core_prefix,
                        uris,
                    )?)
                }
            }
            XmlNode::Element(child) if typed(child, "UseStandardCommands", expected, uris) => {
                if source_projection.use_standard_commands
                    == desired_projection.use_standard_commands
                {
                    node.clone()
                } else {
                    XmlNode::Element(replace_text(
                        child,
                        if desired_projection.use_standard_commands {
                            "true"
                        } else {
                            "false"
                        },
                    ))
                }
            }
            XmlNode::Element(child) if typed(child, "GeneratedTypes", expected, uris) => {
                XmlNode::Element(patch_generated_container(
                    child,
                    uris,
                    expected,
                    source.generated_types(),
                    desired.generated_types(),
                    generated_index,
                    path,
                )?)
            }
            _ => node.clone(),
        };
        children.push(patched);
    }
    Ok(properties.with_children(children))
}

fn patch_generated_container(
    container: &XmlElement,
    uris: &ResolvedNamespaces,
    expected: Option<&str>,
    source: &[GeneratedType],
    desired: &[GeneratedType],
    index: &mut usize,
    path: &ObjectPath,
) -> Result<XmlElement, MetadataEncodeError> {
    let mut children = Vec::with_capacity(container.children().len());
    for node in container.children() {
        if let XmlNode::Element(child) = node
            && typed(child, "GeneratedType", expected, uris)
        {
            if generated_type_is_placeholder(child, uris, expected)? {
                children.push(node.clone());
                continue;
            }
            let source_type = source
                .get(*index)
                .ok_or_else(|| invalid_model(path, "generated type source slot"))?;
            let desired_type = desired
                .get(*index)
                .ok_or_else(|| invalid_model(path, "generated type target slot"))?;
            *index += 1;
            children.push(XmlNode::Element(patch_generated_type(
                child,
                uris,
                expected,
                source_type,
                desired_type,
                path,
            )?));
        } else {
            children.push(node.clone());
        }
    }
    Ok(container.with_children(children))
}

fn generated_type_is_placeholder(
    element: &XmlElement,
    uris: &ResolvedNamespaces,
    expected: Option<&str>,
) -> Result<bool, MetadataEncodeError> {
    for node in element.children() {
        if let XmlNode::Element(child) = node
            && typed(child, "TypeId", expected, uris)
        {
            return Ok(element_text(child)
                .map_err(decode_to_encode)?
                .is_some_and(|value| value.trim() == NIL_UUID));
        }
    }
    Ok(false)
}

fn patch_generated_type(
    element: &XmlElement,
    uris: &ResolvedNamespaces,
    expected: Option<&str>,
    source: &GeneratedType,
    desired: &GeneratedType,
    path: &ObjectPath,
) -> Result<XmlElement, MetadataEncodeError> {
    if source == desired {
        return Ok(element.clone());
    }
    if source.value_id() != desired.value_id() {
        return Err(invalid_model(path, "GeneratedType ValueId mutation"));
    }
    let mut seen_type_id = false;
    let children = element
        .children()
        .iter()
        .map(|node| {
            if let XmlNode::Element(child) = node
                && typed(child, "TypeId", expected, uris)
            {
                seen_type_id = true;
                XmlNode::Element(replace_text(child, &desired.uuid().to_string()))
            } else {
                node.clone()
            }
        })
        .collect();
    if !seen_type_id {
        return Err(invalid_model(path, "GeneratedType TypeId"));
    }
    let element = element.with_children(children);
    if source.kind() == desired.kind() {
        Ok(element)
    } else {
        set_unprefixed_attribute(&element, "category", desired.kind().as_str(), true, path)
    }
}

fn replace_text(element: &XmlElement, value: &str) -> XmlElement {
    element.with_children(vec![XmlNode::text(value)])
}

fn core_qname(prefix: Option<&str>, local: &str) -> Result<QName, MetadataEncodeError> {
    let raw = prefix.map_or_else(|| local.to_owned(), |prefix| format!("{prefix}:{local}"));
    QName::new(raw).map_err(MetadataEncodeError::Xml)
}

fn generated_element(
    prefix: Option<&str>,
    local: &str,
    children: Vec<XmlNode>,
) -> Result<XmlElement, MetadataEncodeError> {
    Ok(XmlElement::with_parts(
        core_qname(prefix, local)?,
        Vec::new(),
        children,
    ))
}

fn generated_text_element(
    prefix: Option<&str>,
    local: &str,
    value: &str,
) -> Result<XmlElement, MetadataEncodeError> {
    generated_element(prefix, local, vec![XmlNode::text(value)])
}

fn render_type(
    source: &XmlElement,
    value: &ConstantType,
    prefix: Option<&str>,
    uris: &ResolvedNamespaces,
) -> Result<XmlElement, MetadataEncodeError> {
    let scalar_attributes = source
        .children()
        .iter()
        .find_map(|node| match node {
            XmlNode::Element(element)
                if uri_of(element, uris) == Some(V8_NAMESPACE)
                    && matches!(element.name().local(), "Type" | "TypeSet") =>
            {
                Some(element.attributes().to_vec())
            }
            _ => None,
        })
        .unwrap_or_default();
    let (scalar_tag, scalar_value) = match value {
        ConstantType::Boolean => ("Type", "xs:boolean"),
        ConstantType::String { .. } => ("Type", "xs:string"),
        ConstantType::Number { .. } => ("Type", "xs:decimal"),
        ConstantType::DateTime { .. } => ("Type", "xs:dateTime"),
        ConstantType::Reference { reference } => ("Type", reference.as_str()),
        ConstantType::ReferenceTypeSet { reference } => ("TypeSet", reference.as_str()),
    };
    let scalar = XmlElement::with_parts(
        core_qname(prefix, scalar_tag)?,
        scalar_attributes,
        vec![XmlNode::text(scalar_value)],
    );
    let mut children = vec![XmlNode::Element(scalar)];
    match value {
        ConstantType::Boolean
        | ConstantType::Reference { .. }
        | ConstantType::ReferenceTypeSet { .. } => {}
        ConstantType::String {
            length,
            allowed_length,
        } => children.push(XmlNode::Element(generated_element(
            prefix,
            "StringQualifiers",
            vec![
                XmlNode::Element(generated_text_element(
                    prefix,
                    "Length",
                    &length.to_string(),
                )?),
                XmlNode::Element(generated_text_element(
                    prefix,
                    "AllowedLength",
                    allowed_length,
                )?),
            ],
        )?)),
        ConstantType::Number {
            digits,
            fraction_digits,
            allowed_sign,
        } => children.push(XmlNode::Element(generated_element(
            prefix,
            "NumberQualifiers",
            vec![
                XmlNode::Element(generated_text_element(
                    prefix,
                    "Digits",
                    &digits.to_string(),
                )?),
                XmlNode::Element(generated_text_element(
                    prefix,
                    "FractionDigits",
                    &fraction_digits.to_string(),
                )?),
                XmlNode::Element(generated_text_element(prefix, "AllowedSign", allowed_sign)?),
            ],
        )?)),
        ConstantType::DateTime { date_fractions } => {
            children.push(XmlNode::Element(generated_element(
                prefix,
                "DateQualifiers",
                vec![XmlNode::Element(generated_text_element(
                    prefix,
                    "DateFractions",
                    date_fractions,
                )?)],
            )?));
        }
    }
    Ok(source.with_children(children))
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
    prefix: Option<&str>,
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

fn set_unprefixed_attribute(
    element: &XmlElement,
    local: &'static str,
    value: &str,
    allow_insert: bool,
    path: &ObjectPath,
) -> Result<XmlElement, MetadataEncodeError> {
    let mut found = false;
    let mut attributes = Vec::with_capacity(element.attributes().len() + usize::from(allow_insert));
    for attribute in element.attributes() {
        if let AttributeKind::Ordinary(name) = attribute.kind()
            && name.prefix().is_none()
            && name.local() == local
        {
            found = true;
            attributes.push(Attribute::ordinary(name.clone(), value));
        } else {
            attributes.push(attribute.clone());
        }
    }
    if !found {
        if !allow_insert {
            return Err(invalid_model(path, local));
        }
        attributes.push(Attribute::ordinary(
            QName::new(local).map_err(MetadataEncodeError::Xml)?,
            value,
        ));
    }
    let raw_start = element.raw_start().and_then(|raw| {
        if found {
            rewrite_raw_attribute(raw, local, value)
        } else {
            insert_raw_attribute(raw, local, value)
        }
    });
    Ok(element.rewritten(attributes, element.children().to_vec(), raw_start))
}

fn rewrite_raw_attribute(raw: &str, name: &str, value: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut index = 1usize;
    while index < bytes.len() && !xml_space(bytes[index]) && !matches!(bytes[index], b'/' | b'>') {
        index += 1;
    }
    while index < bytes.len() {
        while index < bytes.len() && xml_space(bytes[index]) {
            index += 1;
        }
        if index >= bytes.len() || matches!(bytes[index], b'/' | b'>') {
            break;
        }
        let name_start = index;
        while index < bytes.len()
            && !xml_space(bytes[index])
            && !matches!(bytes[index], b'=' | b'/' | b'>')
        {
            index += 1;
        }
        let name_end = index;
        while index < bytes.len() && xml_space(bytes[index]) {
            index += 1;
        }
        if bytes.get(index) != Some(&b'=') {
            return None;
        }
        index += 1;
        while index < bytes.len() && xml_space(bytes[index]) {
            index += 1;
        }
        let quote = *bytes.get(index)?;
        if !matches!(quote, b'\'' | b'"') {
            return None;
        }
        index += 1;
        let value_start = index;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        if index == bytes.len() {
            return None;
        }
        let value_end = index;
        index += 1;
        if &raw[name_start..name_end] == name {
            let escaped = escape_attribute(value, quote);
            let mut output =
                String::with_capacity(raw.len() - (value_end - value_start) + escaped.len());
            output.push_str(&raw[..value_start]);
            output.push_str(&escaped);
            output.push_str(&raw[value_end..]);
            return Some(output);
        }
    }
    None
}

fn insert_raw_attribute(raw: &str, name: &str, value: &str) -> Option<String> {
    let close = raw.rfind('>')?;
    let bytes = raw.as_bytes();
    let mut insert = close;
    while insert > 0 && xml_space(bytes[insert - 1]) {
        insert -= 1;
    }
    if insert > 0 && bytes[insert - 1] == b'/' {
        insert -= 1;
    }
    let escaped = escape_attribute(value, b'"');
    let mut output = String::with_capacity(raw.len() + name.len() + escaped.len() + 4);
    output.push_str(&raw[..insert]);
    output.push(' ');
    output.push_str(name);
    output.push_str("=\"");
    output.push_str(&escaped);
    output.push('"');
    output.push_str(&raw[insert..]);
    Some(output)
}

fn escape_attribute(value: &str, quote: u8) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' if quote == b'"' => output.push_str("&quot;"),
            '\'' if quote == b'\'' => output.push_str("&apos;"),
            _ => output.push(character),
        }
    }
    output
}

fn xml_space(value: u8) -> bool {
    matches!(value, b' ' | b'\t' | b'\r' | b'\n')
}

#[cfg(test)]
mod tests {
    use ibcmd_core::identity::ObjectUuid;
    use ibcmd_core::model::{CanonicalObject, GeneratedType, GeneratedTypeKind};
    use ibcmd_core::semantic::semantic_digest;
    use ibcmd_core::validate::validate_configuration;

    use super::*;
    use crate::XmlReader;

    const OBJECT_UUID: &str = "11111111-1111-4111-8111-111111111111";
    const TYPE_UUID: &str = "22222222-2222-4222-8222-222222222222";
    const VALUE_UUID: &str = "33333333-3333-4333-8333-333333333333";

    fn profile(version: &str) -> ProfileId {
        ProfileId::parse(&format!("xml-{version}")).unwrap()
    }

    fn fixture(version: &str, type_body: &str, future: bool) -> Vec<u8> {
        let future = if future {
            "\r\n\t\t\t<f:Future f:flag='yes'>opaque&amp;</f:Future>"
        } else {
            ""
        };
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"{MD_NAMESPACE}\" xmlns:f=\"urn:future\" xmlns:v8=\"{V8_NAMESPACE}\" xmlns:xr=\"{XR_NAMESPACE}\" version=\"{version}\">\r\n\
\t<Constant uuid=\"{OBJECT_UUID}\">\r\n\
\t\t<InternalInfo>\r\n\
\t\t\t<xr:GeneratedType name=\"ConstantManager.UseFeature\" category=\"Manager\">\r\n\
\t\t\t\t<xr:TypeId>{TYPE_UUID}</xr:TypeId>\r\n\
\t\t\t\t<xr:ValueId>{VALUE_UUID}</xr:ValueId>\r\n\
\t\t\t</xr:GeneratedType>\r\n\
\t\t</InternalInfo>\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>UseFeature</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>en</v8:lang><v8:content>Use feature</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment>Feature flag</Comment>\r\n\
\t\t\t<Type>{type_body}</Type>\r\n\
\t\t\t<UseStandardCommands>true</UseStandardCommands>{future}\r\n\
\t\t</Properties>\r\n\
\t</Constant>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    fn boolean_fixture(version: &str, future: bool) -> Vec<u8> {
        fixture(version, "<v8:Type>xs:boolean</v8:Type>", future)
    }

    fn decode(bytes: &[u8], version: &str) -> MetadataEnvelope {
        let document = XmlReader::from_slice(bytes).unwrap();
        bundled_metadata_registry()
            .decode(
                &FamilyId::parse(CONSTANT_FAMILY).unwrap(),
                &document,
                profile(version),
                ObjectPath::root(),
            )
            .unwrap()
    }

    fn field<'a>(object: &'a CanonicalObject, name: &str) -> &'a CanonicalValue {
        object
            .properties()
            .iter()
            .find(|field| field.name().as_str() == name)
            .unwrap()
            .value()
    }

    fn replace_fields(
        envelope: MetadataEnvelope,
        replacements: &[(&str, CanonicalValue)],
        generated: Option<GeneratedType>,
    ) -> MetadataEnvelope {
        let descendants = envelope.descendants().to_vec();
        let mut parts = copy_object_parts(envelope.root());
        for (name, value) in replacements {
            let field = parts
                .properties
                .iter_mut()
                .find(|field| field.name().as_str() == *name)
                .unwrap();
            *field = CanonicalField::named(name, value.clone()).unwrap();
        }
        if let Some(generated) = generated {
            parts.generated_types[0] = generated;
        }
        envelope
            .with_model(CanonicalObject::new(parts).unwrap(), descendants)
            .unwrap()
    }

    #[test]
    fn bundled_registry_is_explicit_and_constant_schema_is_stable() {
        let family = FamilyId::parse(CONSTANT_FAMILY).unwrap();
        assert!(!MetadataRegistry::default().contains(&family));
        assert!(bundled_metadata_registry().contains(&family));

        let envelope = decode(&boolean_fixture("2.20", false), "2.20");
        assert_eq!(envelope.root().kind().as_str(), CONSTANT_FAMILY);
        assert_eq!(envelope.root().identity().uuid().to_string(), OBJECT_UUID);
        assert_eq!(
            value_text(
                field(envelope.root(), "Comment"),
                &ObjectPath::root(),
                "Comment"
            )
            .unwrap(),
            "Feature flag"
        );
        assert!(matches!(
            field(envelope.root(), "UseStandardCommands").kind(),
            CanonicalValueKind::Bool(true)
        ));
        let fields = field(envelope.root(), "Type").as_record().unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name().as_str(), "kind");
        assert_eq!(
            value_enum(fields[0].value(), &ObjectPath::root(), "Type").unwrap(),
            "Boolean"
        );
        assert_eq!(envelope.root().generated_types().len(), 1);
        assert_eq!(
            envelope.root().generated_types()[0].uuid().to_string(),
            TYPE_UUID
        );
        assert_eq!(
            envelope.root().generated_types()[0].kind().as_str(),
            "Manager"
        );
        assert_eq!(
            envelope.root().generated_types()[0]
                .value_id()
                .unwrap()
                .to_string(),
            VALUE_UUID
        );
    }

    #[test]
    fn same_and_cross_profile_round_trip_preserve_bom_unknown_and_semantics() {
        let input_20 = boolean_fixture("2.20", true);
        let input_21 = boolean_fixture("2.21", true);
        let registry = bundled_metadata_registry();
        let envelope_20 = decode(&input_20, "2.20");
        let envelope_21 = decode(&input_21, "2.21");

        assert_eq!(
            registry.encode(&envelope_20, &profile("2.20")).unwrap(),
            input_20
        );
        assert_eq!(
            registry.encode(&envelope_20, &profile("2.21")).unwrap(),
            input_21
        );
        assert_eq!(
            registry.encode(&envelope_21, &profile("2.20")).unwrap(),
            input_20
        );
        let cross = registry.encode(&envelope_21, &profile("2.20")).unwrap();
        assert!(cross.starts_with(b"\xef\xbb\xbf"));
        assert!(
            std::str::from_utf8(&cross)
                .unwrap()
                .contains("<f:Future f:flag='yes'>opaque&amp;</f:Future>")
        );

        let configuration_20 = envelope_20.configuration().unwrap();
        let configuration_21 = envelope_21.configuration().unwrap();
        let validated_20 = validate_configuration(&configuration_20).unwrap();
        let validated_21 = validate_configuration(&configuration_21).unwrap();
        assert_eq!(
            semantic_digest(&validated_20),
            semantic_digest(&validated_21)
        );
    }

    #[test]
    fn constant_type_matrix_is_typed_and_byte_exact() {
        for (body, expected) in [
            ("<v8:Type>xs:boolean</v8:Type>", ConstantType::Boolean),
            (
                "<v8:Type>xs:string</v8:Type><v8:StringQualifiers><v8:Length>40</v8:Length><v8:AllowedLength>Variable</v8:AllowedLength></v8:StringQualifiers>",
                ConstantType::String {
                    length: 40,
                    allowed_length: "Variable".into(),
                },
            ),
            (
                "<v8:Type>xs:decimal</v8:Type><v8:NumberQualifiers><v8:Digits>12</v8:Digits><v8:FractionDigits>3</v8:FractionDigits><v8:AllowedSign>Nonnegative</v8:AllowedSign></v8:NumberQualifiers>",
                ConstantType::Number {
                    digits: 12,
                    fraction_digits: 3,
                    allowed_sign: "Nonnegative".into(),
                },
            ),
            (
                "<v8:Type>xs:dateTime</v8:Type><v8:DateQualifiers><v8:DateFractions>DateTime</v8:DateFractions></v8:DateQualifiers>",
                ConstantType::DateTime {
                    date_fractions: "DateTime".into(),
                },
            ),
            (
                "<v8:Type>cfg:CatalogRef.Products</v8:Type>",
                ConstantType::Reference {
                    reference: "cfg:CatalogRef.Products".into(),
                },
            ),
            (
                "<v8:TypeSet>cfg:DefinedType.Custom</v8:TypeSet>",
                ConstantType::ReferenceTypeSet {
                    reference: "cfg:DefinedType.Custom".into(),
                },
            ),
        ] {
            let input = fixture("2.20", body, false);
            let envelope = decode(&input, "2.20");
            assert_eq!(
                constant_type_from_value(field(envelope.root(), "Type"), &ObjectPath::root())
                    .unwrap(),
                expected
            );
            assert_eq!(
                bundled_metadata_registry()
                    .encode(&envelope, &profile("2.20"))
                    .unwrap(),
                input
            );
        }
    }

    #[test]
    fn mutated_ir_patches_known_slots_and_keeps_unknown_source_bytes() {
        let input = boolean_fixture("2.20", true);
        let envelope = decode(&input, "2.20");
        let changed_type = ConstantType::String {
            length: 17,
            allowed_length: "Fixed".into(),
        };
        let changed_generated = GeneratedType::new(
            ObjectUuid::parse("44444444-4444-4444-8444-444444444444").unwrap(),
            GeneratedTypeKind::new("ValueManager").unwrap(),
        )
        .with_value_id(ObjectUuid::parse(VALUE_UUID).unwrap());
        let envelope = replace_fields(
            envelope,
            &[
                (
                    "Comment",
                    CanonicalValue::text(CanonicalText::new("Changed & confirmed").unwrap()),
                ),
                ("Type", constant_type_value(&changed_type).unwrap()),
                ("UseStandardCommands", CanonicalValue::boolean(false)),
            ],
            Some(changed_generated),
        );
        let output = bundled_metadata_registry()
            .encode(&envelope, &profile("2.21"))
            .unwrap();
        let text = std::str::from_utf8(&output).unwrap();
        assert_ne!(output, input);
        assert!(output.starts_with(b"\xef\xbb\xbf"));
        assert!(text.contains("version=\"2.21\""));
        assert!(text.contains("<Comment>Changed &amp; confirmed</Comment>"));
        assert!(text.contains("<v8:Length>17</v8:Length>"));
        assert!(text.contains("<v8:AllowedLength>Fixed</v8:AllowedLength>"));
        assert!(text.contains("<UseStandardCommands>false</UseStandardCommands>"));
        assert!(text.contains("44444444-4444-4444-8444-444444444444"));
        assert!(text.contains("category=\"ValueManager\""));
        assert!(text.contains(&format!("<xr:ValueId>{VALUE_UUID}</xr:ValueId>")));
        assert!(text.contains("<f:Future f:flag='yes'>opaque&amp;</f:Future>"));
        decode(&output, "2.21");
    }

    #[test]
    fn nil_generated_type_placeholder_remains_opaque_and_byte_exact() {
        let input = String::from_utf8(boolean_fixture("2.20", false))
            .unwrap()
            .replace(TYPE_UUID, NIL_UUID)
            .into_bytes();
        let envelope = decode(&input, "2.20");
        assert!(envelope.root().generated_types().is_empty());
        assert_eq!(
            bundled_metadata_registry()
                .encode(&envelope, &profile("2.20"))
                .unwrap(),
            input
        );
        let expected_cross = String::from_utf8(input)
            .unwrap()
            .replace("version=\"2.20\"", "version=\"2.21\"")
            .into_bytes();
        assert_eq!(
            bundled_metadata_registry()
                .encode(&envelope, &profile("2.21"))
                .unwrap(),
            expected_cross
        );
    }

    #[test]
    fn mixed_nil_and_valid_generated_types_keep_canonical_alignment() {
        let valid_start =
            "\t\t\t<xr:GeneratedType name=\"ConstantManager.UseFeature\" category=\"Manager\">";
        let placeholder = format!(
            "\t\t\t<xr:GeneratedType name=\"Placeholder\" category=\"Manager\">\r\n\
\t\t\t\t<xr:TypeId>{NIL_UUID}</xr:TypeId>\r\n\
\t\t\t\t<xr:ValueId>{NIL_UUID}</xr:ValueId>\r\n\
\t\t\t</xr:GeneratedType>\r\n"
        );
        let input = String::from_utf8(boolean_fixture("2.20", false))
            .unwrap()
            .replacen(valid_start, &format!("{placeholder}{valid_start}"), 1)
            .into_bytes();
        let envelope = decode(&input, "2.20");
        assert_eq!(envelope.root().generated_types().len(), 1);
        assert_eq!(
            envelope.root().generated_types()[0].uuid().to_string(),
            TYPE_UUID
        );
        assert_eq!(
            bundled_metadata_registry()
                .encode(&envelope, &profile("2.20"))
                .unwrap(),
            input
        );
        let expected_cross = String::from_utf8(input)
            .unwrap()
            .replace("version=\"2.20\"", "version=\"2.21\"")
            .into_bytes();
        assert_eq!(
            bundled_metadata_registry()
                .encode(&envelope, &profile("2.21"))
                .unwrap(),
            expected_cross
        );
    }

    #[test]
    fn profile_version_namespace_and_malformed_fields_fail_closed() {
        let input = boolean_fixture("2.20", false);
        let document = XmlReader::from_slice(&input).unwrap();
        let registry = bundled_metadata_registry();
        let family = FamilyId::parse(CONSTANT_FAMILY).unwrap();
        assert!(matches!(
            registry.decode(&family, &document, profile("2.21"), ObjectPath::root()),
            Err(MetadataDecodeError::ProfileVersionMismatch { .. })
        ));
        assert!(matches!(
            registry.decode(
                &family,
                &document,
                ProfileId::parse("xml:2.20").unwrap(),
                ObjectPath::root()
            ),
            Err(MetadataDecodeError::UnsupportedProfile { .. })
        ));
        let envelope = decode(&input, "2.20");
        assert!(matches!(
            registry.encode(&envelope, &ProfileId::parse("xml:2.21").unwrap()),
            Err(MetadataEncodeError::UnsupportedProfile { .. })
        ));

        let spoofed = String::from_utf8(input.clone()).unwrap().replace(
            "<Comment>Feature flag</Comment>",
            "<f:Comment>spoof</f:Comment>",
        );
        assert!(matches!(
            registry.decode(
                &family,
                &XmlReader::from_slice(spoofed.as_bytes()).unwrap(),
                profile("2.20"),
                ObjectPath::root()
            ),
            Err(MetadataDecodeError::Missing("Comment"))
        ));
        let duplicate = String::from_utf8(input).unwrap().replace(
            "<Comment>Feature flag</Comment>",
            "<Comment>one</Comment><Comment>two</Comment>",
        );
        assert!(matches!(
            registry.decode(
                &family,
                &XmlReader::from_slice(duplicate.as_bytes()).unwrap(),
                profile("2.20"),
                ObjectPath::root()
            ),
            Err(MetadataDecodeError::Duplicate("Comment"))
        ));
    }
}
