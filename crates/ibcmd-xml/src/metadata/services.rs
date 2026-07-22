//! Strict canonical codecs for service metadata families.
//!
//! BOOT-004 grows this module one independently evidenced family at a time.
//! The first slice is `ScheduledJob`: every semantic property is typed and
//! unknown properties fail closed, while document trivia remains available to
//! the lossless XML writer.

use std::collections::BTreeSet;

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::ObjectPath;
use ibcmd_core::family::FamilyId;
use ibcmd_core::model::CanonicalObject;
use ibcmd_core::value::{CanonicalInteger, CanonicalValue, EnumToken};

use super::common::{
    MD_NAMESPACE, MetadataDecodeError, MetadataEnvelope, ResolvedNamespaces, V8_NAMESPACE,
    element_text, resolve_namespaces, typed, uri_of,
};
use super::decode_metadata_envelope;
use super::language::{
    canonical_field, canonical_text, copy_object_parts, decode_to_encode, invalid_model,
    profile_version, root_version, set_unprefixed_attribute, validate_decode_profile,
};
use super::registry::{
    MetadataEncodeError, MetadataFamilyCodec, MetadataRegistry, MetadataRegistryError,
};
use crate::{AttributeKind, LexicalPolicy, XmlDocument, XmlElement, XmlNode, XmlWriter};

const SCHEDULED_JOB_FAMILY: &str = "ScheduledJob";
const EVENT_SUBSCRIPTION_FAMILY: &str = "EventSubscription";
const XDTO_PACKAGE_FAMILY: &str = "XDTOPackage";

/// Registers the strict `ScheduledJob` codec.
pub fn register_scheduled_job_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(ScheduledJobCodec {
        family: FamilyId::parse(SCHEDULED_JOB_FAMILY).expect("family id is stable"),
    }))
}

/// Registers the strict `EventSubscription` codec.
pub fn register_event_subscription_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(EventSubscriptionCodec {
        family: FamilyId::parse(EVENT_SUBSCRIPTION_FAMILY).expect("family id is stable"),
    }))
}

/// Registers the strict `XDTOPackage` metadata codec.
pub fn register_xdto_package_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(XdtoPackageCodec {
        family: FamilyId::parse(XDTO_PACKAGE_FAMILY).expect("family id is stable"),
    }))
}

struct ScheduledJobCodec {
    family: FamilyId,
}

struct EventSubscriptionCodec {
    family: FamilyId,
}

struct XdtoPackageCodec {
    family: FamilyId,
}

impl MetadataFamilyCodec for XdtoPackageCodec {
    fn family_id(&self) -> &FamilyId {
        &self.family
    }

    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        decode_xdto_package(document, source, path)
    }

    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        encode_xdto_package(envelope, target)
    }
}

impl MetadataFamilyCodec for EventSubscriptionCodec {
    fn family_id(&self) -> &FamilyId {
        &self.family
    }

    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        decode_event_subscription(document, source, path)
    }

    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        encode_event_subscription(envelope, target)
    }
}

impl MetadataFamilyCodec for ScheduledJobCodec {
    fn family_id(&self) -> &FamilyId {
        &self.family
    }

    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        decode_scheduled_job(document, source, path)
    }

    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        encode_scheduled_job(envelope, target)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScheduledJobProjection {
    comment: String,
    method_name: String,
    description: String,
    key: String,
    use_job: bool,
    predefined: bool,
    restart_count_on_failure: u32,
    restart_interval_on_failure: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EventSourceProjection {
    kind: &'static str,
    reference: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EventSubscriptionProjection {
    comment: String,
    sources: Vec<EventSourceProjection>,
    event: String,
    handler: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct XdtoPackageProjection {
    comment: String,
    namespace: String,
}

fn decode_scheduled_job(
    document: &XmlDocument,
    source: ProfileId,
    path: ObjectPath,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    validate_decode_profile(document, &source, &path)?;
    let projection = project_scheduled_job(document)?;
    let generic = decode_metadata_envelope(document, source, path)?;
    if generic.root().kind().as_str() != SCHEDULED_JOB_FAMILY {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "ScheduledJob codec requires its exact family",
        ));
    }
    if !generic.descendants().is_empty()
        || !generic.root().generated_types().is_empty()
        || !generic.root().references().is_empty()
        || !generic.root().assets().is_empty()
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "ScheduledJob cannot contain children, generated types, references, or assets",
        ));
    }

    let mut parts = copy_object_parts(generic.root());
    for (name, value) in [
        (
            "Comment",
            CanonicalValue::text(canonical_text(&projection.comment)?),
        ),
        (
            "MethodName",
            CanonicalValue::text(canonical_text(&projection.method_name)?),
        ),
        (
            "Description",
            CanonicalValue::text(canonical_text(&projection.description)?),
        ),
        (
            "Key",
            CanonicalValue::text(canonical_text(&projection.key)?),
        ),
        ("Use", CanonicalValue::boolean(projection.use_job)),
        ("Predefined", CanonicalValue::boolean(projection.predefined)),
        (
            "RestartCountOnFailure",
            integer_value(projection.restart_count_on_failure)?,
        ),
        (
            "RestartIntervalOnFailure",
            integer_value(projection.restart_interval_on_failure)?,
        ),
    ] {
        parts.properties.push(canonical_field(name, value)?);
    }
    let root = CanonicalObject::new(parts)
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))?;
    MetadataEnvelope::from_parts(root, Vec::new(), document.clone())
}

fn decode_event_subscription(
    document: &XmlDocument,
    source: ProfileId,
    path: ObjectPath,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    validate_decode_profile(document, &source, &path)?;
    let projection = project_event_subscription(document)?;
    let generic = decode_metadata_envelope(document, source, path)?;
    if generic.root().kind().as_str() != EVENT_SUBSCRIPTION_FAMILY {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "EventSubscription codec requires its exact family",
        ));
    }
    if !generic.descendants().is_empty()
        || !generic.root().generated_types().is_empty()
        || !generic.root().references().is_empty()
        || !generic.root().assets().is_empty()
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "EventSubscription cannot contain children, generated types, references, or assets",
        ));
    }

    let mut parts = copy_object_parts(generic.root());
    for (name, value) in [
        (
            "Comment",
            CanonicalValue::text(canonical_text(&projection.comment)?),
        ),
        ("Source", event_sources_value(&projection.sources)?),
        (
            "Event",
            CanonicalValue::text(canonical_text(&projection.event)?),
        ),
        (
            "Handler",
            CanonicalValue::text(canonical_text(&projection.handler)?),
        ),
    ] {
        parts.properties.push(canonical_field(name, value)?);
    }
    let root = CanonicalObject::new(parts)
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))?;
    MetadataEnvelope::from_parts(root, Vec::new(), document.clone())
}

fn decode_xdto_package(
    document: &XmlDocument,
    source: ProfileId,
    path: ObjectPath,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    validate_decode_profile(document, &source, &path)?;
    let projection = project_xdto_package(document)?;
    let generic = decode_metadata_envelope(document, source, path)?;
    if generic.root().kind().as_str() != XDTO_PACKAGE_FAMILY {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "XDTOPackage codec requires its exact family",
        ));
    }
    if !generic.descendants().is_empty()
        || !generic.root().generated_types().is_empty()
        || !generic.root().references().is_empty()
        || !generic.root().assets().is_empty()
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "XDTOPackage metadata cannot contain children, generated types, references, or inline assets",
        ));
    }
    let mut parts = copy_object_parts(generic.root());
    for (name, value) in [
        (
            "Comment",
            CanonicalValue::text(canonical_text(&projection.comment)?),
        ),
        (
            "Namespace",
            CanonicalValue::text(canonical_text(&projection.namespace)?),
        ),
    ] {
        parts.properties.push(canonical_field(name, value)?);
    }
    let root = CanonicalObject::new(parts)
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))?;
    MetadataEnvelope::from_parts(root, Vec::new(), document.clone())
}

fn event_sources_value(
    sources: &[EventSourceProjection],
) -> Result<CanonicalValue, MetadataDecodeError> {
    let mut values = Vec::with_capacity(sources.len());
    for source in sources {
        values.push(
            CanonicalValue::record(vec![
                canonical_field("kind", enum_value(source.kind)?)?,
                canonical_field(
                    "reference",
                    CanonicalValue::text(canonical_text(&source.reference)?),
                )?,
            ])
            .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
        );
    }
    CanonicalValue::sequence(values).map_err(|error| MetadataDecodeError::Core(error.to_string()))
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

fn project_scheduled_job(
    document: &XmlDocument,
) -> Result<ScheduledJobProjection, MetadataDecodeError> {
    let uris = resolve_namespaces(document.root())?;
    let expected = uri_of(document.root(), &uris);
    if !matches!(expected, None | Some(MD_NAMESPACE)) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "ScheduledJob root namespace",
        ));
    }
    reject_attributes(document.root(), &["version"])?;
    let object = required_child(document.root(), SCHEDULED_JOB_FAMILY, expected, &uris)?;
    reject_attributes(object, &["uuid"])?;
    let properties = exact_properties_child(object, expected, &uris)?;
    reject_attributes(properties, &[])?;
    reject_scheduled_job_properties(properties, expected, &uris)?;

    let method_name = required_text(properties, "MethodName", expected, &uris)?;
    if method_name.is_empty() {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "ScheduledJob MethodName must not be empty",
        ));
    }
    Ok(ScheduledJobProjection {
        comment: required_text(properties, "Comment", expected, &uris)?,
        method_name,
        description: required_text(properties, "Description", expected, &uris)?,
        key: required_text(properties, "Key", expected, &uris)?,
        use_job: required_bool(properties, "Use", expected, &uris)?,
        predefined: required_bool(properties, "Predefined", expected, &uris)?,
        restart_count_on_failure: required_u32(
            properties,
            "RestartCountOnFailure",
            expected,
            &uris,
        )?,
        restart_interval_on_failure: required_u32(
            properties,
            "RestartIntervalOnFailure",
            expected,
            &uris,
        )?,
    })
}

fn project_event_subscription(
    document: &XmlDocument,
) -> Result<EventSubscriptionProjection, MetadataDecodeError> {
    let uris = resolve_namespaces(document.root())?;
    let expected = uri_of(document.root(), &uris);
    if !matches!(expected, None | Some(MD_NAMESPACE)) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "EventSubscription root namespace",
        ));
    }
    reject_attributes(document.root(), &["version"])?;
    let object = required_child(document.root(), EVENT_SUBSCRIPTION_FAMILY, expected, &uris)?;
    reject_attributes(object, &["uuid"])?;
    let properties = exact_properties_child(object, expected, &uris)?;
    reject_attributes(properties, &[])?;
    reject_event_subscription_properties(properties, expected, &uris)?;

    let event = required_text(properties, "Event", expected, &uris)?;
    if !supported_event(&event) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "EventSubscription Event has no evidenced native mapping",
        ));
    }
    let handler = required_text(properties, "Handler", expected, &uris)?;
    if !valid_handler(&handler) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "EventSubscription Handler is not CommonModule.<name>.<method>",
        ));
    }
    Ok(EventSubscriptionProjection {
        comment: required_text(properties, "Comment", expected, &uris)?,
        sources: parse_event_sources(properties, expected, &uris)?,
        event,
        handler,
    })
}

fn parse_event_sources(
    properties: &XmlElement,
    expected: Option<&str>,
    uris: &ResolvedNamespaces,
) -> Result<Vec<EventSourceProjection>, MetadataDecodeError> {
    let source = required_child(properties, "Source", expected, uris)?;
    reject_attributes(source, &[])?;
    let mut unique = BTreeSet::new();
    let mut sources = Vec::new();
    for node in source.children() {
        let XmlNode::Element(child) = node else {
            continue;
        };
        if uri_of(child, uris) != Some(V8_NAMESPACE)
            || !matches!(child.name().local(), "Type" | "TypeSet")
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "EventSubscription Source contains an unknown element",
            ));
        }
        reject_attributes(child, &[])?;
        let reference = element_text(child)?.ok_or(MetadataDecodeError::InvalidEnvelope(
            "EventSubscription Source item must contain text only",
        ))?;
        if reference.is_empty()
            || !reference.starts_with("cfg:")
            || reference.chars().any(char::is_whitespace)
            || !unique.insert((child.name().local(), reference.clone()))
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "EventSubscription Source reference is invalid or duplicated",
            ));
        }
        sources.push(EventSourceProjection {
            kind: match child.name().local() {
                "Type" => "Type",
                "TypeSet" => "TypeSet",
                _ => unreachable!("source kind checked above"),
            },
            reference,
        });
    }
    if sources.is_empty() {
        return Err(MetadataDecodeError::Missing(
            "EventSubscription Source item",
        ));
    }
    Ok(sources)
}

fn supported_event(value: &str) -> bool {
    matches!(
        value,
        "BeforeDelete"
            | "BeforeWrite"
            | "FillCheckProcessing"
            | "Filling"
            | "FormGetProcessing"
            | "OnReceiveDataFromMaster"
            | "OnReceiveDataFromSlave"
            | "OnSendDataToMaster"
            | "OnSendDataToSlave"
            | "OnSendNodeDataToSlave"
            | "OnSetNewNumber"
            | "OnWrite"
            | "Posting"
            | "PresentationFieldsGetProcessing"
            | "PresentationGetProcessing"
    )
}

fn valid_handler(value: &str) -> bool {
    let mut parts = value.split('.');
    parts.next() == Some("CommonModule")
        && parts.next().is_some_and(valid_identifier)
        && parts.next().is_some_and(valid_identifier)
        && parts.next().is_none()
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_whitespace) && !value.contains('.')
}

fn project_xdto_package(
    document: &XmlDocument,
) -> Result<XdtoPackageProjection, MetadataDecodeError> {
    let uris = resolve_namespaces(document.root())?;
    let expected = uri_of(document.root(), &uris);
    if !matches!(expected, None | Some(MD_NAMESPACE)) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "XDTOPackage root namespace",
        ));
    }
    reject_attributes(document.root(), &["version"])?;
    let object = required_child(document.root(), XDTO_PACKAGE_FAMILY, expected, &uris)?;
    reject_attributes(object, &["uuid"])?;
    let properties = exact_properties_child(object, expected, &uris)?;
    reject_attributes(properties, &[])?;
    reject_xdto_package_properties(properties, expected, &uris)?;
    let namespace = required_text(properties, "Namespace", expected, &uris)?;
    if namespace.is_empty() || namespace.chars().any(char::is_whitespace) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "XDTOPackage Namespace is empty or contains whitespace",
        ));
    }
    Ok(XdtoPackageProjection {
        comment: required_text(properties, "Comment", expected, &uris)?,
        namespace,
    })
}

fn exact_properties_child<'a>(
    object: &'a XmlElement,
    expected: Option<&str>,
    uris: &ResolvedNamespaces,
) -> Result<&'a XmlElement, MetadataDecodeError> {
    let mut properties = None;
    for node in object.children() {
        let XmlNode::Element(child) = node else {
            continue;
        };
        if !typed(child, "Properties", expected, uris) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "unknown ScheduledJob object child",
            ));
        }
        if properties.replace(child).is_some() {
            return Err(MetadataDecodeError::Duplicate("Properties"));
        }
    }
    properties.ok_or(MetadataDecodeError::Missing("Properties"))
}

fn reject_scheduled_job_properties(
    properties: &XmlElement,
    expected: Option<&str>,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    const REQUIRED: [&str; 10] = [
        "Name",
        "Synonym",
        "Comment",
        "MethodName",
        "Description",
        "Key",
        "Use",
        "Predefined",
        "RestartCountOnFailure",
        "RestartIntervalOnFailure",
    ];
    let mut seen = BTreeSet::new();
    for node in properties.children() {
        let XmlNode::Element(child) = node else {
            continue;
        };
        let local = child.name().local();
        if !REQUIRED.contains(&local) || !typed(child, local, expected, uris) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "unknown ScheduledJob property",
            ));
        }
        reject_attributes_recursive(child)?;
        if !seen.insert(local) {
            return Err(MetadataDecodeError::Duplicate(match local {
                "Name" => "Name",
                "Synonym" => "Synonym",
                "Comment" => "Comment",
                "MethodName" => "MethodName",
                "Description" => "Description",
                "Key" => "Key",
                "Use" => "Use",
                "Predefined" => "Predefined",
                "RestartCountOnFailure" => "RestartCountOnFailure",
                "RestartIntervalOnFailure" => "RestartIntervalOnFailure",
                _ => unreachable!("allowed property matched above"),
            }));
        }
    }
    for required in REQUIRED {
        if !seen.contains(required) {
            return Err(MetadataDecodeError::Missing(required));
        }
    }
    Ok(())
}

fn reject_event_subscription_properties(
    properties: &XmlElement,
    expected: Option<&str>,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    const REQUIRED: [&str; 6] = ["Name", "Synonym", "Comment", "Source", "Event", "Handler"];
    let mut seen = BTreeSet::new();
    for node in properties.children() {
        let XmlNode::Element(child) = node else {
            continue;
        };
        let local = child.name().local();
        if !REQUIRED.contains(&local) || !typed(child, local, expected, uris) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "unknown EventSubscription property",
            ));
        }
        reject_attributes_recursive(child)?;
        if !seen.insert(local) {
            return Err(MetadataDecodeError::Duplicate(match local {
                "Name" => "Name",
                "Synonym" => "Synonym",
                "Comment" => "Comment",
                "Source" => "Source",
                "Event" => "Event",
                "Handler" => "Handler",
                _ => unreachable!("allowed property matched above"),
            }));
        }
    }
    for required in REQUIRED {
        if !seen.contains(required) {
            return Err(MetadataDecodeError::Missing(required));
        }
    }
    Ok(())
}

fn reject_xdto_package_properties(
    properties: &XmlElement,
    expected: Option<&str>,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    const REQUIRED: [&str; 4] = ["Name", "Synonym", "Comment", "Namespace"];
    let mut seen = BTreeSet::new();
    for node in properties.children() {
        let XmlNode::Element(child) = node else {
            continue;
        };
        let local = child.name().local();
        if !REQUIRED.contains(&local) || !typed(child, local, expected, uris) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "unknown XDTOPackage property",
            ));
        }
        reject_attributes_recursive(child)?;
        if !seen.insert(local) {
            return Err(MetadataDecodeError::Duplicate(match local {
                "Name" => "Name",
                "Synonym" => "Synonym",
                "Comment" => "Comment",
                "Namespace" => "Namespace",
                _ => unreachable!("allowed property matched above"),
            }));
        }
    }
    for required in REQUIRED {
        if !seen.contains(required) {
            return Err(MetadataDecodeError::Missing(required));
        }
    }
    Ok(())
}

fn reject_attributes(
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
                    "unknown ScheduledJob semantic attribute",
                ));
            }
        }
    }
    Ok(())
}

fn reject_attributes_recursive(element: &XmlElement) -> Result<(), MetadataDecodeError> {
    reject_attributes(element, &[])?;
    for node in element.children() {
        if let XmlNode::Element(child) = node {
            reject_attributes_recursive(child)?;
        }
    }
    Ok(())
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

fn required_text(
    parent: &XmlElement,
    local: &'static str,
    namespace: Option<&str>,
    uris: &ResolvedNamespaces,
) -> Result<String, MetadataDecodeError> {
    let child = required_child(parent, local, namespace, uris)?;
    element_text(child)?.ok_or(MetadataDecodeError::InvalidEnvelope(
        "ScheduledJob property must contain text only",
    ))
}

fn required_bool(
    parent: &XmlElement,
    local: &'static str,
    namespace: Option<&str>,
    uris: &ResolvedNamespaces,
) -> Result<bool, MetadataDecodeError> {
    match required_text(parent, local, namespace, uris)?.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(MetadataDecodeError::InvalidEnvelope(
            "ScheduledJob boolean is not canonical",
        )),
    }
}

fn required_u32(
    parent: &XmlElement,
    local: &'static str,
    namespace: Option<&str>,
    uris: &ResolvedNamespaces,
) -> Result<u32, MetadataDecodeError> {
    let value = required_text(parent, local, namespace, uris)?;
    value
        .parse::<u32>()
        .ok()
        .filter(|parsed| parsed.to_string() == value)
        .ok_or(MetadataDecodeError::InvalidEnvelope(
            "ScheduledJob integer is not canonical u32",
        ))
}

fn encode_scheduled_job(
    envelope: &MetadataEnvelope,
    target: &ProfileId,
) -> Result<Vec<u8>, MetadataEncodeError> {
    let path = envelope.root().identity().path().clone();
    let target_version =
        profile_version(target).ok_or_else(|| MetadataEncodeError::UnsupportedProfile {
            object_path: path.clone(),
            profile: target.clone(),
        })?;
    if envelope.root().kind().as_str() != SCHEDULED_JOB_FAMILY {
        return Err(invalid_model(&path, "kind"));
    }
    let source_profile = envelope.root().provenance().source_profile().clone();
    validate_decode_profile(envelope.source_document(), &source_profile, &path)
        .map_err(decode_to_encode)?;
    let source = decode_scheduled_job(envelope.source_document(), source_profile, path.clone())
        .map_err(decode_to_encode)?;
    if source.root() != envelope.root() || source.descendants() != envelope.descendants() {
        return Err(invalid_model(
            &path,
            "ScheduledJob semantic mutation is not implemented",
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

fn encode_event_subscription(
    envelope: &MetadataEnvelope,
    target: &ProfileId,
) -> Result<Vec<u8>, MetadataEncodeError> {
    let path = envelope.root().identity().path().clone();
    let target_version =
        profile_version(target).ok_or_else(|| MetadataEncodeError::UnsupportedProfile {
            object_path: path.clone(),
            profile: target.clone(),
        })?;
    if envelope.root().kind().as_str() != EVENT_SUBSCRIPTION_FAMILY {
        return Err(invalid_model(&path, "kind"));
    }
    let source_profile = envelope.root().provenance().source_profile().clone();
    validate_decode_profile(envelope.source_document(), &source_profile, &path)
        .map_err(decode_to_encode)?;
    let source =
        decode_event_subscription(envelope.source_document(), source_profile, path.clone())
            .map_err(decode_to_encode)?;
    if source.root() != envelope.root() || source.descendants() != envelope.descendants() {
        return Err(invalid_model(
            &path,
            "EventSubscription semantic mutation is not implemented",
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

fn encode_xdto_package(
    envelope: &MetadataEnvelope,
    target: &ProfileId,
) -> Result<Vec<u8>, MetadataEncodeError> {
    let path = envelope.root().identity().path().clone();
    let target_version =
        profile_version(target).ok_or_else(|| MetadataEncodeError::UnsupportedProfile {
            object_path: path.clone(),
            profile: target.clone(),
        })?;
    if envelope.root().kind().as_str() != XDTO_PACKAGE_FAMILY {
        return Err(invalid_model(&path, "kind"));
    }
    let source_profile = envelope.root().provenance().source_profile().clone();
    validate_decode_profile(envelope.source_document(), &source_profile, &path)
        .map_err(decode_to_encode)?;
    let source = decode_xdto_package(envelope.source_document(), source_profile, path.clone())
        .map_err(decode_to_encode)?;
    if source.root() != envelope.root() || source.descendants() != envelope.descendants() {
        return Err(invalid_model(
            &path,
            "XDTOPackage semantic mutation is not implemented",
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

    const UUID: &str = "c7ffd8ab-15e9-4cf1-a7fd-d05534dff000";

    fn profile(version: &str) -> ProfileId {
        ProfileId::parse(&format!("xml-{version}")).unwrap()
    }

    fn fixture(version: &str, use_job: &str, count: &str, extra: &str) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"{MD_NAMESPACE}\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"{version}\">\r\n\
\t<ScheduledJob uuid=\"{UUID}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>ЗагрузкаКурсовВалют</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Загрузка курсов валют</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<MethodName>CommonModule.РаботаСКурсамиВалютЛокализация.ПриЗагрузкеАктуальныхКурсов</MethodName>\r\n\
\t\t\t<Description/>\r\n\
\t\t\t<Key/>\r\n\
\t\t\t<Use>{use_job}</Use>\r\n\
\t\t\t<Predefined>true</Predefined>\r\n\
\t\t\t<RestartCountOnFailure>{count}</RestartCountOnFailure>\r\n\
\t\t\t<RestartIntervalOnFailure>600</RestartIntervalOnFailure>{extra}\r\n\
\t\t</Properties>\r\n\
\t</ScheduledJob>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    #[test]
    fn scheduled_job_is_typed_and_cross_version_roundtrips() {
        for version in ["2.20", "2.21"] {
            let document = XmlReader::from_slice(&fixture(version, "false", "10", "")).unwrap();
            let mut registry = MetadataRegistry::default();
            register_scheduled_job_codec(&mut registry).unwrap();
            let envelope = registry
                .decode(
                    &FamilyId::parse(SCHEDULED_JOB_FAMILY).unwrap(),
                    &document,
                    profile(version),
                    ObjectPath::root(),
                )
                .unwrap();
            assert_eq!(envelope.root().properties().len(), 10);
            assert!(matches!(
                envelope.root().properties()[6].value().kind(),
                CanonicalValueKind::Bool(false)
            ));
            assert!(matches!(
                envelope.root().properties()[8].value().kind(),
                CanonicalValueKind::Integer(_)
            ));
            let target = if version == "2.20" { "2.21" } else { "2.20" };
            let output = registry.encode(&envelope, &profile(target)).unwrap();
            let reparsed = XmlReader::from_slice(&output).unwrap();
            registry
                .decode(
                    &FamilyId::parse(SCHEDULED_JOB_FAMILY).unwrap(),
                    &reparsed,
                    profile(target),
                    ObjectPath::root(),
                )
                .unwrap();
        }
    }

    #[test]
    fn scheduled_job_unknown_and_noncanonical_values_fail_closed() {
        let mut registry = MetadataRegistry::default();
        register_scheduled_job_codec(&mut registry).unwrap();
        for input in [
            fixture("2.20", "0", "10", ""),
            fixture("2.20", "false", "010", ""),
            fixture("2.20", "false", "10", "<Future/>"),
        ] {
            let document = XmlReader::from_slice(&input).unwrap();
            assert!(
                registry
                    .decode(
                        &FamilyId::parse(SCHEDULED_JOB_FAMILY).unwrap(),
                        &document,
                        profile("2.20"),
                        ObjectPath::root(),
                    )
                    .is_err()
            );
        }
    }

    fn event_fixture(version: &str, event: &str, source_tail: &str) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"{MD_NAMESPACE}\" xmlns:v8=\"{V8_NAMESPACE}\" version=\"{version}\">\r\n\
\t<EventSubscription uuid=\"a64b15fa-fc34-43fe-a366-d27c0f1c3df2\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>ПередУдалением</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Перед удалением</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<Source><v8:Type>cfg:CatalogObject.Объекты</v8:Type>{source_tail}</Source>\r\n\
\t\t\t<Event>{event}</Event>\r\n\
\t\t\t<Handler>CommonModule.Подписки.ПередУдалением</Handler>\r\n\
\t\t</Properties>\r\n\
\t</EventSubscription>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    #[test]
    fn event_subscription_sources_are_typed_and_cross_version_roundtrip() {
        let mut registry = MetadataRegistry::default();
        register_event_subscription_codec(&mut registry).unwrap();
        for version in ["2.20", "2.21"] {
            let document = XmlReader::from_slice(&event_fixture(
                version,
                "BeforeDelete",
                "<v8:TypeSet>cfg:DefinedType.Данные</v8:TypeSet>",
            ))
            .unwrap();
            let envelope = registry
                .decode(
                    &FamilyId::parse(EVENT_SUBSCRIPTION_FAMILY).unwrap(),
                    &document,
                    profile(version),
                    ObjectPath::root(),
                )
                .unwrap();
            let sources = envelope.root().properties()[3]
                .value()
                .as_sequence()
                .unwrap();
            assert_eq!(sources.len(), 2);
            assert!(matches!(
                sources[0].as_record().unwrap()[0].value().kind(),
                CanonicalValueKind::EnumToken(_)
            ));
            let target = if version == "2.20" { "2.21" } else { "2.20" };
            let output = registry.encode(&envelope, &profile(target)).unwrap();
            let reparsed = XmlReader::from_slice(&output).unwrap();
            registry
                .decode(
                    &FamilyId::parse(EVENT_SUBSCRIPTION_FAMILY).unwrap(),
                    &reparsed,
                    profile(target),
                    ObjectPath::root(),
                )
                .unwrap();
        }
    }

    #[test]
    fn event_subscription_unknown_event_source_and_property_fail_closed() {
        let mut registry = MetadataRegistry::default();
        register_event_subscription_codec(&mut registry).unwrap();
        for input in [
            event_fixture("2.20", "FutureEvent", ""),
            event_fixture("2.20", "BeforeDelete", "<v8:Future>cfg:X.Y</v8:Future>"),
            event_fixture("2.20", "BeforeDelete", "<Future/>"),
        ] {
            let document = XmlReader::from_slice(&input).unwrap();
            assert!(
                registry
                    .decode(
                        &FamilyId::parse(EVENT_SUBSCRIPTION_FAMILY).unwrap(),
                        &document,
                        profile("2.20"),
                        ObjectPath::root(),
                    )
                    .is_err()
            );
        }
    }

    fn xdto_fixture(version: &str, namespace: &str, extra: &str) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"{MD_NAMESPACE}\" xmlns:v8=\"{V8_NAMESPACE}\" version=\"{version}\">\r\n\
\t<XDTOPackage uuid=\"ac7ea771-4b10-4d43-9c0a-9cd36e4c49a4\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>АдминистрированиеОбменаДанными_2_4_5_1</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Администрирование обмена данными 2.4.5.1</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<Namespace>{namespace}</Namespace>{extra}\r\n\
\t\t</Properties>\r\n\
\t</XDTOPackage>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    #[test]
    fn xdto_package_is_typed_and_cross_version_roundtrips() {
        let mut registry = MetadataRegistry::default();
        register_xdto_package_codec(&mut registry).unwrap();
        for version in ["2.20", "2.21"] {
            let document = XmlReader::from_slice(&xdto_fixture(
                version,
                "http://www.1c.ru/SaaS/ExchangeAdministration/Common/2.4.5.1",
                "",
            ))
            .unwrap();
            let envelope = registry
                .decode(
                    &FamilyId::parse(XDTO_PACKAGE_FAMILY).unwrap(),
                    &document,
                    profile(version),
                    ObjectPath::root(),
                )
                .unwrap();
            assert_eq!(envelope.root().properties().len(), 4);
            assert!(matches!(
                envelope.root().properties()[3].value().kind(),
                CanonicalValueKind::Text(_)
            ));
            let target = if version == "2.20" { "2.21" } else { "2.20" };
            let output = registry.encode(&envelope, &profile(target)).unwrap();
            let reparsed = XmlReader::from_slice(&output).unwrap();
            registry
                .decode(
                    &FamilyId::parse(XDTO_PACKAGE_FAMILY).unwrap(),
                    &reparsed,
                    profile(target),
                    ObjectPath::root(),
                )
                .unwrap();
        }
    }

    #[test]
    fn xdto_package_unknown_and_invalid_namespace_fail_closed() {
        let mut registry = MetadataRegistry::default();
        register_xdto_package_codec(&mut registry).unwrap();
        for input in [
            xdto_fixture("2.20", "", ""),
            xdto_fixture("2.20", "http://example.test/has space", ""),
            xdto_fixture("2.20", "http://example.test", "<Future/>"),
        ] {
            let document = XmlReader::from_slice(&input).unwrap();
            assert!(
                registry
                    .decode(
                        &FamilyId::parse(XDTO_PACKAGE_FAMILY).unwrap(),
                        &document,
                        profile("2.20"),
                        ObjectPath::root(),
                    )
                    .is_err()
            );
        }
    }
}
