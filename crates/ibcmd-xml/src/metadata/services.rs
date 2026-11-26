//! Strict canonical codecs for service metadata families.
//!
//! BOOT-004 grows this module one independently evidenced family at a time.
//! The first slice is `ScheduledJob`: every semantic property is typed and
//! unknown properties fail closed, while document trivia remains available to
//! the lossless XML writer.

use std::collections::{BTreeMap, BTreeSet};

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
    profile_version, root_version, set_unprefixed_attribute, validate_decode_profile,
};
use super::registry::{
    MetadataEncodeError, MetadataFamilyCodec, MetadataRegistry, MetadataRegistryError,
};
use crate::{AttributeKind, LexicalPolicy, XmlDocument, XmlElement, XmlNode, XmlWriter};

const SCHEDULED_JOB_FAMILY: &str = "ScheduledJob";
const EVENT_SUBSCRIPTION_FAMILY: &str = "EventSubscription";
const XDTO_PACKAGE_FAMILY: &str = "XDTOPackage";
const HTTP_SERVICE_FAMILY: &str = "HTTPService";
const WEB_SERVICE_FAMILY: &str = "WebService";
const WS_REFERENCE_FAMILY: &str = "WSReference";
const INTEGRATION_SERVICE_FAMILY: &str = "IntegrationService";
const XML_SCHEMA_NAMESPACE: &str = "http://www.w3.org/2001/XMLSchema";
const XSI_NAMESPACE: &str = "http://www.w3.org/2001/XMLSchema-instance";

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

/// Registers the strict `HTTPService` metadata codec.
pub fn register_http_service_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(HttpServiceCodec {
        family: FamilyId::parse(HTTP_SERVICE_FAMILY).expect("family id is stable"),
    }))
}

/// Registers the strict `WebService` metadata codec.
pub fn register_web_service_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(WebServiceCodec {
        family: FamilyId::parse(WEB_SERVICE_FAMILY).expect("family id is stable"),
    }))
}

/// Registers the strict `WSReference` metadata codec.
pub fn register_ws_reference_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(WsReferenceCodec {
        family: FamilyId::parse(WS_REFERENCE_FAMILY).expect("family id is stable"),
    }))
}

/// Registers the strict `IntegrationService` metadata codec.
pub fn register_integration_service_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(IntegrationServiceCodec {
        family: FamilyId::parse(INTEGRATION_SERVICE_FAMILY).expect("family id is stable"),
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

struct HttpServiceCodec {
    family: FamilyId,
}

struct WebServiceCodec {
    family: FamilyId,
}

struct WsReferenceCodec {
    family: FamilyId,
}

struct IntegrationServiceCodec {
    family: FamilyId,
}

impl MetadataFamilyCodec for IntegrationServiceCodec {
    fn family_id(&self) -> &FamilyId {
        &self.family
    }

    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        decode_integration_service(document, source, path)
    }

    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        encode_integration_service(envelope, target)
    }
}

impl MetadataFamilyCodec for WsReferenceCodec {
    fn family_id(&self) -> &FamilyId {
        &self.family
    }

    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        decode_ws_reference(document, source, path)
    }

    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        encode_ws_reference(envelope, target)
    }
}

impl MetadataFamilyCodec for WebServiceCodec {
    fn family_id(&self) -> &FamilyId {
        &self.family
    }

    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        decode_web_service(document, source, path)
    }

    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        encode_web_service(envelope, target)
    }
}

impl MetadataFamilyCodec for HttpServiceCodec {
    fn family_id(&self) -> &FamilyId {
        &self.family
    }

    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        decode_http_service(document, source, path)
    }

    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        encode_http_service(envelope, target)
    }
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct HttpMethodProjection {
    uuid: ObjectUuid,
    comment: String,
    http_method: String,
    handler: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HttpUrlProjection {
    uuid: ObjectUuid,
    comment: String,
    template: String,
    methods: Vec<HttpMethodProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HttpServiceProjection {
    comment: String,
    root_url: String,
    reuse_sessions: String,
    session_max_age: u32,
    urls: Vec<HttpUrlProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct XdtoTypeProjection {
    namespace: String,
    name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WebParameterProjection {
    uuid: ObjectUuid,
    comment: String,
    value_type: XdtoTypeProjection,
    nillable: bool,
    transfer_direction: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WebOperationProjection {
    uuid: ObjectUuid,
    comment: String,
    returning_type: XdtoTypeProjection,
    nillable: bool,
    transactioned: bool,
    procedure_name: String,
    data_lock_control_mode: String,
    parameters: Vec<WebParameterProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WebServiceProjection {
    comment: String,
    namespace: String,
    package_references: Vec<String>,
    package_namespaces: Vec<String>,
    descriptor_file_name: String,
    reuse_sessions: String,
    session_max_age: u32,
    operations: Vec<WebOperationProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WsReferenceProjection {
    comment: String,
    location_url: String,
    manager_type_id: ObjectUuid,
    manager_value_id: ObjectUuid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IntegrationServiceChannelProjection {
    uuid: ObjectUuid,
    comment: String,
    manager_type_id: ObjectUuid,
    manager_value_id: ObjectUuid,
    external_name: String,
    message_direction: String,
    receive_message_processing: String,
    transactioned: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IntegrationServiceProjection {
    comment: String,
    manager_type_id: ObjectUuid,
    manager_value_id: ObjectUuid,
    external_address: String,
    channels: Vec<IntegrationServiceChannelProjection>,
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

fn decode_http_service(
    document: &XmlDocument,
    source: ProfileId,
    path: ObjectPath,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    validate_decode_profile(document, &source, &path)?;
    let projection = project_http_service(document)?;
    let generic = decode_metadata_envelope(document, source, path)?;
    if generic.root().kind().as_str() != HTTP_SERVICE_FAMILY {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "HTTPService codec requires its exact family",
        ));
    }
    if !generic.root().generated_types().is_empty()
        || !generic.root().references().is_empty()
        || !generic.root().assets().is_empty()
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "HTTPService cannot contain generated types, references, or assets",
        ));
    }

    let mut root_parts = copy_object_parts(generic.root());
    for (name, value) in [
        (
            "Comment",
            CanonicalValue::text(canonical_text(&projection.comment)?),
        ),
        (
            "RootURL",
            CanonicalValue::text(canonical_text(&projection.root_url)?),
        ),
        ("ReuseSessions", enum_value(&projection.reuse_sessions)?),
        ("SessionMaxAge", integer_value(projection.session_max_age)?),
    ] {
        root_parts.properties.push(canonical_field(name, value)?);
    }
    let root = CanonicalObject::new(root_parts)
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))?;

    let mut urls = BTreeMap::new();
    let mut methods = BTreeMap::new();
    for url in &projection.urls {
        if urls.insert(url.uuid, url).is_some() {
            return Err(MetadataDecodeError::Duplicate("URLTemplate uuid"));
        }
        for method in &url.methods {
            if methods.insert(method.uuid, method).is_some() {
                return Err(MetadataDecodeError::Duplicate("HTTP Method uuid"));
            }
        }
    }
    if generic.descendants().len() != urls.len() + methods.len() {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "HTTPService descendant inventory is not exact",
        ));
    }
    let mut descendants = Vec::with_capacity(generic.descendants().len());
    for descendant in generic.descendants() {
        let uuid = descendant.identity().uuid();
        let mut parts = copy_object_parts(descendant);
        match descendant.kind().as_str() {
            "URLTemplate" => {
                let projected = urls
                    .remove(&uuid)
                    .ok_or(MetadataDecodeError::InvalidEnvelope(
                        "unknown HTTPService URLTemplate",
                    ))?;
                for (name, value) in [
                    (
                        "Comment",
                        CanonicalValue::text(canonical_text(&projected.comment)?),
                    ),
                    (
                        "Template",
                        CanonicalValue::text(canonical_text(&projected.template)?),
                    ),
                ] {
                    parts.properties.push(canonical_field(name, value)?);
                }
            }
            "Method" => {
                let projected =
                    methods
                        .remove(&uuid)
                        .ok_or(MetadataDecodeError::InvalidEnvelope(
                            "unknown HTTPService Method",
                        ))?;
                for (name, value) in [
                    (
                        "Comment",
                        CanonicalValue::text(canonical_text(&projected.comment)?),
                    ),
                    ("HTTPMethod", enum_value(&projected.http_method)?),
                    (
                        "Handler",
                        CanonicalValue::text(canonical_text(&projected.handler)?),
                    ),
                ] {
                    parts.properties.push(canonical_field(name, value)?);
                }
            }
            _ => {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "unknown HTTPService descendant kind",
                ));
            }
        }
        if !descendant.generated_types().is_empty()
            || !descendant.references().is_empty()
            || !descendant.assets().is_empty()
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "HTTPService descendants cannot contain generated types, references, or assets",
            ));
        }
        descendants.push(
            CanonicalObject::new(parts)
                .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
        );
    }
    if !urls.is_empty() || !methods.is_empty() {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "HTTPService projected descendants are missing from canonical inventory",
        ));
    }
    MetadataEnvelope::from_parts(root, descendants, document.clone())
}

fn decode_web_service(
    document: &XmlDocument,
    source: ProfileId,
    path: ObjectPath,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    validate_decode_profile(document, &source, &path)?;
    let projection = project_web_service(document)?;
    let generic = decode_metadata_envelope(document, source, path)?;
    if generic.root().kind().as_str() != WEB_SERVICE_FAMILY {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WebService codec requires its exact family",
        ));
    }
    if !generic.root().generated_types().is_empty()
        || !generic.root().references().is_empty()
        || !generic.root().assets().is_empty()
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WebService cannot contain generated types, references, or assets",
        ));
    }

    let mut root_parts = copy_object_parts(generic.root());
    for (name, value) in [
        (
            "Comment",
            CanonicalValue::text(canonical_text(&projection.comment)?),
        ),
        (
            "Namespace",
            CanonicalValue::text(canonical_text(&projection.namespace)?),
        ),
        (
            "XDTOPackages",
            web_packages_value(
                &projection.package_references,
                &projection.package_namespaces,
            )?,
        ),
        (
            "DescriptorFileName",
            CanonicalValue::text(canonical_text(&projection.descriptor_file_name)?),
        ),
        ("ReuseSessions", enum_value(&projection.reuse_sessions)?),
        ("SessionMaxAge", integer_value(projection.session_max_age)?),
    ] {
        root_parts.properties.push(canonical_field(name, value)?);
    }
    let root = CanonicalObject::new(root_parts)
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))?;

    let mut operations = BTreeMap::new();
    let mut parameters = BTreeMap::new();
    for operation in &projection.operations {
        if operations.insert(operation.uuid, operation).is_some() {
            return Err(MetadataDecodeError::Duplicate("WebService Operation uuid"));
        }
        for parameter in &operation.parameters {
            if parameters.insert(parameter.uuid, parameter).is_some() {
                return Err(MetadataDecodeError::Duplicate("WebService Parameter uuid"));
            }
        }
    }
    if generic.descendants().len() != operations.len() + parameters.len() {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WebService descendant inventory is not exact",
        ));
    }
    let mut descendants = Vec::with_capacity(generic.descendants().len());
    for descendant in generic.descendants() {
        let uuid = descendant.identity().uuid();
        let mut parts = copy_object_parts(descendant);
        match descendant.kind().as_str() {
            "Operation" => {
                let projected =
                    operations
                        .remove(&uuid)
                        .ok_or(MetadataDecodeError::InvalidEnvelope(
                            "unknown WebService Operation",
                        ))?;
                for (name, value) in [
                    (
                        "Comment",
                        CanonicalValue::text(canonical_text(&projected.comment)?),
                    ),
                    (
                        "XDTOReturningValueType",
                        xdto_type_value(&projected.returning_type)?,
                    ),
                    ("Nillable", CanonicalValue::boolean(projected.nillable)),
                    (
                        "Transactioned",
                        CanonicalValue::boolean(projected.transactioned),
                    ),
                    (
                        "ProcedureName",
                        CanonicalValue::text(canonical_text(&projected.procedure_name)?),
                    ),
                    (
                        "DataLockControlMode",
                        enum_value(&projected.data_lock_control_mode)?,
                    ),
                ] {
                    parts.properties.push(canonical_field(name, value)?);
                }
            }
            "Parameter" => {
                let projected =
                    parameters
                        .remove(&uuid)
                        .ok_or(MetadataDecodeError::InvalidEnvelope(
                            "unknown WebService Parameter",
                        ))?;
                for (name, value) in [
                    (
                        "Comment",
                        CanonicalValue::text(canonical_text(&projected.comment)?),
                    ),
                    ("XDTOValueType", xdto_type_value(&projected.value_type)?),
                    ("Nillable", CanonicalValue::boolean(projected.nillable)),
                    (
                        "TransferDirection",
                        enum_value(&projected.transfer_direction)?,
                    ),
                ] {
                    parts.properties.push(canonical_field(name, value)?);
                }
            }
            _ => {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "unknown WebService descendant kind",
                ));
            }
        }
        if !descendant.generated_types().is_empty()
            || !descendant.references().is_empty()
            || !descendant.assets().is_empty()
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "WebService descendants cannot contain generated types, references, or assets",
            ));
        }
        descendants.push(
            CanonicalObject::new(parts)
                .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
        );
    }
    if !operations.is_empty() || !parameters.is_empty() {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WebService projected descendants are missing from canonical inventory",
        ));
    }
    MetadataEnvelope::from_parts(root, descendants, document.clone())
}

fn decode_ws_reference(
    document: &XmlDocument,
    source: ProfileId,
    path: ObjectPath,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    validate_decode_profile(document, &source, &path)?;
    let projection = project_ws_reference(document)?;
    let generic = decode_metadata_envelope(document, source, path)?;
    if generic.root().kind().as_str() != WS_REFERENCE_FAMILY {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WSReference codec requires its exact family",
        ));
    }
    if !generic.descendants().is_empty()
        || !generic.root().references().is_empty()
        || !generic.root().assets().is_empty()
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WSReference cannot contain children, references, or assets",
        ));
    }
    let generated = generic.root().generated_types();
    if generated.len() != 1
        || generated[0].kind().as_str() != "Manager"
        || generated[0].uuid() != projection.manager_type_id
        || generated[0].value_id() != Some(projection.manager_value_id)
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WSReference Manager TypeId/ValueId is not exact",
        ));
    }
    let mut parts = copy_object_parts(generic.root());
    parts.properties.push(canonical_field(
        "Comment",
        CanonicalValue::text(canonical_text(&projection.comment)?),
    )?);
    parts.properties.push(canonical_field(
        "LocationURL",
        CanonicalValue::text(canonical_text(&projection.location_url)?),
    )?);
    let root = CanonicalObject::new(parts)
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))?;
    MetadataEnvelope::from_parts(root, Vec::new(), document.clone())
}

fn decode_integration_service(
    document: &XmlDocument,
    source: ProfileId,
    path: ObjectPath,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    validate_decode_profile(document, &source, &path)?;
    let projection = project_integration_service(document)?;
    let generic = decode_metadata_envelope(document, source, path)?;
    if generic.root().kind().as_str() != INTEGRATION_SERVICE_FAMILY {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "IntegrationService codec requires its exact family",
        ));
    }
    if !generic.root().references().is_empty() || !generic.root().assets().is_empty() {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "IntegrationService cannot contain references or assets",
        ));
    }
    let generated = generic.root().generated_types();
    if generated.len() != 1
        || generated[0].kind().as_str() != "Manager"
        || generated[0].uuid() != projection.manager_type_id
        || generated[0].value_id() != Some(projection.manager_value_id)
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "IntegrationService Manager TypeId/ValueId is not exact",
        ));
    }
    let mut root_parts = copy_object_parts(generic.root());
    root_parts.properties.push(canonical_field(
        "Comment",
        CanonicalValue::text(canonical_text(&projection.comment)?),
    )?);
    root_parts.properties.push(canonical_field(
        "ExternalIntegrationServiceAddress",
        CanonicalValue::text(canonical_text(&projection.external_address)?),
    )?);
    let root = CanonicalObject::new(root_parts)
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))?;

    let mut projected_channels = projection
        .channels
        .iter()
        .map(|channel| (channel.uuid, channel))
        .collect::<BTreeMap<_, _>>();
    if projected_channels.len() != projection.channels.len()
        || generic.descendants().len() != projection.channels.len()
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "IntegrationService channel inventory is not exact",
        ));
    }
    let mut descendants = Vec::with_capacity(generic.descendants().len());
    for channel in generic.descendants() {
        if channel.kind().as_str() != "IntegrationServiceChannel"
            || channel.owner() != Some(root.identity().uuid())
            || !channel.references().is_empty()
            || !channel.assets().is_empty()
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "IntegrationService contains an unsupported child",
            ));
        }
        let projected = projected_channels
            .remove(&channel.identity().uuid())
            .ok_or(MetadataDecodeError::InvalidEnvelope(
                "unknown IntegrationService channel",
            ))?;
        let generated = channel.generated_types();
        if generated.len() != 1
            || generated[0].kind().as_str() != "Manager"
            || generated[0].uuid() != projected.manager_type_id
            || generated[0].value_id() != Some(projected.manager_value_id)
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "IntegrationServiceChannel Manager TypeId/ValueId is not exact",
            ));
        }
        let mut parts = copy_object_parts(channel);
        for (name, value) in [
            (
                "Comment",
                CanonicalValue::text(canonical_text(&projected.comment)?),
            ),
            (
                "ExternalIntegrationServiceChannelName",
                CanonicalValue::text(canonical_text(&projected.external_name)?),
            ),
            (
                "MessageDirection",
                enum_value(&projected.message_direction)?,
            ),
            (
                "ReceiveMessageProcessing",
                CanonicalValue::text(canonical_text(&projected.receive_message_processing)?),
            ),
            (
                "Transactioned",
                CanonicalValue::boolean(projected.transactioned),
            ),
        ] {
            parts.properties.push(canonical_field(name, value)?);
        }
        descendants.push(
            CanonicalObject::new(parts)
                .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
        );
    }
    if !projected_channels.is_empty() {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "IntegrationService projected channels are missing",
        ));
    }
    MetadataEnvelope::from_parts(root, descendants, document.clone())
}

fn xdto_type_value(value: &XdtoTypeProjection) -> Result<CanonicalValue, MetadataDecodeError> {
    CanonicalValue::record(vec![
        canonical_field(
            "Namespace",
            CanonicalValue::text(canonical_text(&value.namespace)?),
        )?,
        canonical_field("Name", CanonicalValue::text(canonical_text(&value.name)?))?,
    ])
    .map_err(|error| MetadataDecodeError::Core(error.to_string()))
}

fn web_packages_value(
    references: &[String],
    namespaces: &[String],
) -> Result<CanonicalValue, MetadataDecodeError> {
    CanonicalValue::record(vec![
        canonical_field("References", text_sequence(references)?)?,
        canonical_field("Namespaces", text_sequence(namespaces)?)?,
    ])
    .map_err(|error| MetadataDecodeError::Core(error.to_string()))
}

fn text_sequence(values: &[String]) -> Result<CanonicalValue, MetadataDecodeError> {
    let values = values
        .iter()
        .map(|value| canonical_text(value).map(CanonicalValue::text))
        .collect::<Result<Vec<_>, _>>()?;
    CanonicalValue::sequence(values).map_err(|error| MetadataDecodeError::Core(error.to_string()))
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

fn project_http_service(
    document: &XmlDocument,
) -> Result<HttpServiceProjection, MetadataDecodeError> {
    let uris = resolve_namespaces(document.root())?;
    let expected = uri_of(document.root(), &uris);
    if !matches!(expected, None | Some(MD_NAMESPACE)) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "HTTPService root namespace",
        ));
    }
    reject_attributes(document.root(), &["version"])?;
    let object = required_child(document.root(), HTTP_SERVICE_FAMILY, expected, &uris)?;
    reject_attributes(object, &["uuid"])?;
    let (properties, child_objects) = exact_http_sections(object, expected, &uris, true)?;
    reject_exact_service_properties(
        properties,
        &[
            "Name",
            "Synonym",
            "Comment",
            "RootURL",
            "ReuseSessions",
            "SessionMaxAge",
        ],
        expected,
        &uris,
    )?;
    let root_url = required_text(properties, "RootURL", expected, &uris)?;
    if root_url.is_empty() || root_url.chars().any(char::is_whitespace) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "HTTPService RootURL is empty or contains whitespace",
        ));
    }
    let reuse_sessions = required_text(properties, "ReuseSessions", expected, &uris)?;
    if !matches!(reuse_sessions.as_str(), "DontUse" | "Use" | "AutoUse") {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "HTTPService ReuseSessions is unsupported",
        ));
    }
    let mut uuids = BTreeSet::new();
    let mut urls = Vec::new();
    let child_objects = child_objects.expect("required above");
    reject_attributes(child_objects, &[])?;
    for node in child_objects.children() {
        let XmlNode::Element(url) = node else {
            continue;
        };
        if !typed(url, "URLTemplate", expected, &uris) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "HTTPService contains an unknown child",
            ));
        }
        reject_attributes(url, &["uuid"])?;
        let uuid = required_uuid(url)?;
        if !uuids.insert(uuid) {
            return Err(MetadataDecodeError::Duplicate("HTTPService child uuid"));
        }
        let (url_properties, method_objects) = exact_http_sections(url, expected, &uris, true)?;
        reject_exact_service_properties(
            url_properties,
            &["Name", "Synonym", "Comment", "Template"],
            expected,
            &uris,
        )?;
        let template = required_text(url_properties, "Template", expected, &uris)?;
        if template.is_empty() || template.chars().any(char::is_whitespace) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "HTTPService URLTemplate Template is empty or contains whitespace",
            ));
        }
        let method_objects = method_objects.expect("required above");
        reject_attributes(method_objects, &[])?;
        let mut methods = Vec::new();
        for method_node in method_objects.children() {
            let XmlNode::Element(method) = method_node else {
                continue;
            };
            if !typed(method, "Method", expected, &uris) {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "HTTPService URLTemplate contains an unknown child",
                ));
            }
            reject_attributes(method, &["uuid"])?;
            let method_uuid = required_uuid(method)?;
            if !uuids.insert(method_uuid) {
                return Err(MetadataDecodeError::Duplicate("HTTPService child uuid"));
            }
            let (method_properties, nested) = exact_http_sections(method, expected, &uris, false)?;
            if nested.is_some() {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "HTTPService Method cannot own child objects",
                ));
            }
            reject_exact_service_properties(
                method_properties,
                &["Name", "Synonym", "Comment", "HTTPMethod", "Handler"],
                expected,
                &uris,
            )?;
            let http_method = required_text(method_properties, "HTTPMethod", expected, &uris)?;
            if !matches!(http_method.as_str(), "DELETE" | "GET" | "POST" | "PUT") {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "HTTPService HTTPMethod has no evidenced native code",
                ));
            }
            let handler = required_text(method_properties, "Handler", expected, &uris)?;
            if !valid_identifier(&handler) {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "HTTPService Handler is not an exact identifier",
                ));
            }
            methods.push(HttpMethodProjection {
                uuid: method_uuid,
                comment: required_text(method_properties, "Comment", expected, &uris)?,
                http_method,
                handler,
            });
        }
        if methods.is_empty() {
            return Err(MetadataDecodeError::Missing("HTTPService Method"));
        }
        urls.push(HttpUrlProjection {
            uuid,
            comment: required_text(url_properties, "Comment", expected, &uris)?,
            template,
            methods,
        });
    }
    if urls.is_empty() {
        return Err(MetadataDecodeError::Missing("HTTPService URLTemplate"));
    }
    Ok(HttpServiceProjection {
        comment: required_text(properties, "Comment", expected, &uris)?,
        root_url,
        reuse_sessions,
        session_max_age: required_u32(properties, "SessionMaxAge", expected, &uris)?,
        urls,
    })
}

fn project_web_service(
    document: &XmlDocument,
) -> Result<WebServiceProjection, MetadataDecodeError> {
    let uris = resolve_namespaces(document.root())?;
    let expected = uri_of(document.root(), &uris);
    if !matches!(expected, None | Some(MD_NAMESPACE)) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WebService root namespace",
        ));
    }
    reject_attributes(document.root(), &["version"])?;
    let object = required_child(document.root(), WEB_SERVICE_FAMILY, expected, &uris)?;
    reject_attributes(object, &["uuid"])?;
    let (properties, child_objects) = exact_http_sections(object, expected, &uris, true)?;
    reject_exact_web_root_properties(properties, expected, &uris)?;
    let namespace = required_text(properties, "Namespace", expected, &uris)?;
    if !valid_namespace(&namespace) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WebService Namespace is empty or contains whitespace",
        ));
    }
    let descriptor_file_name = required_text(properties, "DescriptorFileName", expected, &uris)?;
    if !valid_descriptor_file_name(&descriptor_file_name) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WebService DescriptorFileName is invalid",
        ));
    }
    let reuse_sessions = required_text(properties, "ReuseSessions", expected, &uris)?;
    if reuse_sessions != "DontUse" {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WebService ReuseSessions has no evidenced native code",
        ));
    }
    let (package_references, package_namespaces) = project_web_packages(
        required_child(properties, "XDTOPackages", expected, &uris)?,
        document.root(),
        &uris,
    )?;

    let child_objects = child_objects.expect("required above");
    reject_attributes(child_objects, &[])?;
    let mut uuids = BTreeSet::new();
    let mut operations = Vec::new();
    for node in child_objects.children() {
        let XmlNode::Element(operation) = node else {
            continue;
        };
        if !typed(operation, "Operation", expected, &uris) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "WebService contains an unknown child",
            ));
        }
        reject_attributes(operation, &["uuid"])?;
        let uuid = required_uuid(operation)?;
        if !uuids.insert(uuid) {
            return Err(MetadataDecodeError::Duplicate("WebService child uuid"));
        }
        let (operation_properties, parameter_objects) =
            exact_http_sections(operation, expected, &uris, true)?;
        reject_exact_service_properties(
            operation_properties,
            &[
                "Name",
                "Synonym",
                "Comment",
                "XDTOReturningValueType",
                "Nillable",
                "Transactioned",
                "ProcedureName",
                "DataLockControlMode",
            ],
            expected,
            &uris,
        )?;
        let procedure_name = required_text(operation_properties, "ProcedureName", expected, &uris)?;
        if !valid_identifier(&procedure_name) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "WebService ProcedureName is not an exact identifier",
            ));
        }
        let data_lock_control_mode =
            required_text(operation_properties, "DataLockControlMode", expected, &uris)?;
        if data_lock_control_mode != "Managed" {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "WebService DataLockControlMode has no evidenced native code",
            ));
        }
        let transactioned = required_bool(operation_properties, "Transactioned", expected, &uris)?;
        if transactioned {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "WebService Transactioned has no evidenced native code",
            ));
        }
        let parameter_objects = parameter_objects.expect("required above");
        reject_attributes(parameter_objects, &[])?;
        let mut parameters = Vec::new();
        for parameter_node in parameter_objects.children() {
            let XmlNode::Element(parameter) = parameter_node else {
                continue;
            };
            if !typed(parameter, "Parameter", expected, &uris) {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "WebService Operation contains an unknown child",
                ));
            }
            reject_attributes(parameter, &["uuid"])?;
            let parameter_uuid = required_uuid(parameter)?;
            if !uuids.insert(parameter_uuid) {
                return Err(MetadataDecodeError::Duplicate("WebService child uuid"));
            }
            let (parameter_properties, nested) =
                exact_http_sections(parameter, expected, &uris, false)?;
            if nested.is_some() {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "WebService Parameter cannot own child objects",
                ));
            }
            reject_exact_service_properties(
                parameter_properties,
                &[
                    "Name",
                    "Synonym",
                    "Comment",
                    "XDTOValueType",
                    "Nillable",
                    "TransferDirection",
                ],
                expected,
                &uris,
            )?;
            let transfer_direction =
                required_text(parameter_properties, "TransferDirection", expected, &uris)?;
            if !matches!(transfer_direction.as_str(), "In" | "Out" | "InOut") {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "WebService TransferDirection has no evidenced native code",
                ));
            }
            parameters.push(WebParameterProjection {
                uuid: parameter_uuid,
                comment: required_text(parameter_properties, "Comment", expected, &uris)?,
                value_type: project_xdto_type(
                    parameter_properties,
                    "XDTOValueType",
                    expected,
                    &uris,
                    document.root(),
                )?,
                nillable: required_bool(parameter_properties, "Nillable", expected, &uris)?,
                transfer_direction,
            });
        }
        operations.push(WebOperationProjection {
            uuid,
            comment: required_text(operation_properties, "Comment", expected, &uris)?,
            returning_type: project_xdto_type(
                operation_properties,
                "XDTOReturningValueType",
                expected,
                &uris,
                document.root(),
            )?,
            nillable: required_bool(operation_properties, "Nillable", expected, &uris)?,
            transactioned,
            procedure_name,
            data_lock_control_mode,
            parameters,
        });
    }
    if operations.is_empty() {
        return Err(MetadataDecodeError::Missing("WebService Operation"));
    }
    Ok(WebServiceProjection {
        comment: required_text(properties, "Comment", expected, &uris)?,
        namespace,
        package_references,
        package_namespaces,
        descriptor_file_name,
        reuse_sessions,
        session_max_age: required_u32(properties, "SessionMaxAge", expected, &uris)?,
        operations,
    })
}

fn project_ws_reference(
    document: &XmlDocument,
) -> Result<WsReferenceProjection, MetadataDecodeError> {
    let uris = resolve_namespaces(document.root())?;
    let expected = uri_of(document.root(), &uris);
    if !matches!(expected, None | Some(MD_NAMESPACE)) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WSReference root namespace",
        ));
    }
    reject_attributes(document.root(), &["version"])?;
    let object = required_child(document.root(), WS_REFERENCE_FAMILY, expected, &uris)?;
    reject_attributes(object, &["uuid"])?;
    let sections = object
        .children()
        .iter()
        .filter_map(|node| match node {
            XmlNode::Element(child) => Some(child),
            _ => None,
        })
        .collect::<Vec<_>>();
    if sections.len() != 2
        || !typed(sections[0], "InternalInfo", expected, &uris)
        || !typed(sections[1], "Properties", expected, &uris)
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WSReference sections or order are unsupported",
        ));
    }
    let internal = sections[0];
    let properties = sections[1];
    reject_attributes(internal, &[])?;
    reject_exact_service_properties(
        properties,
        &["Name", "Synonym", "Comment", "LocationURL"],
        expected,
        &uris,
    )?;
    let name = required_text(properties, "Name", expected, &uris)?;
    if !valid_identifier(&name) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WSReference Name is not an exact identifier",
        ));
    }
    let generated = internal
        .children()
        .iter()
        .filter_map(|node| match node {
            XmlNode::Element(child) => Some(child),
            _ => None,
        })
        .collect::<Vec<_>>();
    if generated.len() != 1 || !typed(generated[0], "GeneratedType", Some(XR_NAMESPACE), &uris) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WSReference InternalInfo must contain one xr:GeneratedType",
        ));
    }
    let generated = generated[0];
    reject_attributes(generated, &["name", "category"])?;
    let expected_generated_name = format!("WSReferenceManager.{name}");
    if unprefixed_attribute(generated, "name") != Some(expected_generated_name.as_str())
        || unprefixed_attribute(generated, "category") != Some("Manager")
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WSReference generated type name or category is inconsistent",
        ));
    }
    let identities = generated
        .children()
        .iter()
        .filter_map(|node| match node {
            XmlNode::Element(child) => Some(child),
            _ => None,
        })
        .collect::<Vec<_>>();
    if identities.len() != 2
        || !typed(identities[0], "TypeId", Some(XR_NAMESPACE), &uris)
        || !typed(identities[1], "ValueId", Some(XR_NAMESPACE), &uris)
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WSReference generated TypeId/ValueId fields or order are unsupported",
        ));
    }
    let manager_type_id = exact_generated_uuid(identities[0], "WSReference Manager TypeId")?;
    let manager_value_id = exact_generated_uuid(identities[1], "WSReference Manager ValueId")?;
    if manager_type_id == manager_value_id {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WSReference generated TypeId and ValueId are not independent",
        ));
    }
    let location_url = required_text(properties, "LocationURL", expected, &uris)?;
    if location_url.is_empty() || location_url.chars().any(char::is_whitespace) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WSReference LocationURL is not exact",
        ));
    }
    Ok(WsReferenceProjection {
        comment: required_text(properties, "Comment", expected, &uris)?,
        location_url,
        manager_type_id,
        manager_value_id,
    })
}

fn project_integration_service(
    document: &XmlDocument,
) -> Result<IntegrationServiceProjection, MetadataDecodeError> {
    let uris = resolve_namespaces(document.root())?;
    let expected = uri_of(document.root(), &uris);
    if !matches!(expected, None | Some(MD_NAMESPACE)) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "IntegrationService root namespace",
        ));
    }
    reject_attributes(document.root(), &["version"])?;
    let object = required_child(document.root(), INTEGRATION_SERVICE_FAMILY, expected, &uris)?;
    reject_attributes(object, &["uuid"])?;
    let sections = object
        .children()
        .iter()
        .filter_map(|node| match node {
            XmlNode::Element(child) => Some(child),
            _ => None,
        })
        .collect::<Vec<_>>();
    if sections.len() != 3
        || !typed(sections[0], "InternalInfo", expected, &uris)
        || !typed(sections[1], "Properties", expected, &uris)
        || !typed(sections[2], "ChildObjects", expected, &uris)
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "IntegrationService sections or order are unsupported",
        ));
    }
    let properties = sections[1];
    reject_exact_service_properties(
        properties,
        &[
            "Name",
            "Synonym",
            "Comment",
            "ExternalIntegrationServiceAddress",
        ],
        expected,
        &uris,
    )?;
    let name = required_text(properties, "Name", expected, &uris)?;
    if !valid_identifier(&name) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "IntegrationService Name is not an exact identifier",
        ));
    }
    let expected_manager_name = format!("IntegrationServiceManager.{name}");
    let (manager_type_id, manager_value_id) =
        project_exact_manager_generated(sections[0], &uris, &expected_manager_name)?;
    let root_uuid = required_uuid(object)?;
    let mut identities = BTreeSet::from([root_uuid, manager_type_id, manager_value_id]);
    if identities.len() != 3 {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "IntegrationService root identities are duplicated",
        ));
    }
    let external_address = required_text(
        properties,
        "ExternalIntegrationServiceAddress",
        expected,
        &uris,
    )?;
    if external_address.chars().any(char::is_whitespace) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "IntegrationService external address is not exact",
        ));
    }

    let child_objects = sections[2];
    reject_attributes(child_objects, &[])?;
    let mut channels = Vec::new();
    let mut names = BTreeSet::new();
    for node in child_objects.children() {
        let XmlNode::Element(channel) = node else {
            continue;
        };
        if !typed(channel, "IntegrationServiceChannel", expected, &uris) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "IntegrationService contains an unknown child",
            ));
        }
        reject_attributes(channel, &["uuid"])?;
        let uuid = required_uuid(channel)?;
        let channel_sections = channel
            .children()
            .iter()
            .filter_map(|node| match node {
                XmlNode::Element(child) => Some(child),
                _ => None,
            })
            .collect::<Vec<_>>();
        if channel_sections.len() != 2
            || !typed(channel_sections[0], "InternalInfo", expected, &uris)
            || !typed(channel_sections[1], "Properties", expected, &uris)
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "IntegrationServiceChannel sections or order are unsupported",
            ));
        }
        let channel_properties = channel_sections[1];
        reject_exact_service_properties(
            channel_properties,
            &[
                "Name",
                "Synonym",
                "Comment",
                "ExternalIntegrationServiceChannelName",
                "MessageDirection",
                "ReceiveMessageProcessing",
                "Transactioned",
            ],
            expected,
            &uris,
        )?;
        let channel_name = required_text(channel_properties, "Name", expected, &uris)?;
        if !valid_identifier(&channel_name)
            || !names.insert(
                channel_name
                    .chars()
                    .flat_map(char::to_lowercase)
                    .collect::<String>(),
            )
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "IntegrationServiceChannel Name is invalid or duplicated",
            ));
        }
        let expected_channel_manager =
            format!("IntegrationServiceChannelManager.{name}.{channel_name}");
        let (channel_type_id, channel_value_id) =
            project_exact_manager_generated(channel_sections[0], &uris, &expected_channel_manager)?;
        for identity in [uuid, channel_type_id, channel_value_id] {
            if !identities.insert(identity) {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "IntegrationService identity is duplicated",
                ));
            }
        }
        let external_name = required_text(
            channel_properties,
            "ExternalIntegrationServiceChannelName",
            expected,
            &uris,
        )?;
        if external_name.is_empty() || external_name.chars().any(char::is_whitespace) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "IntegrationServiceChannel external name is not exact",
            ));
        }
        let message_direction =
            required_text(channel_properties, "MessageDirection", expected, &uris)?;
        if !matches!(message_direction.as_str(), "Send" | "Receive") {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "IntegrationServiceChannel MessageDirection is unsupported",
            ));
        }
        let receive_message_processing = required_text(
            channel_properties,
            "ReceiveMessageProcessing",
            expected,
            &uris,
        )?;
        let transactioned = required_bool(channel_properties, "Transactioned", expected, &uris)?;
        let exact_direction_properties = match message_direction.as_str() {
            "Receive" => valid_identifier(&receive_message_processing) && !transactioned,
            "Send" => receive_message_processing.is_empty() && transactioned,
            _ => unreachable!("validated above"),
        };
        if !exact_direction_properties {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "IntegrationServiceChannel direction-specific properties are unsupported",
            ));
        }
        channels.push(IntegrationServiceChannelProjection {
            uuid,
            comment: required_text(channel_properties, "Comment", expected, &uris)?,
            manager_type_id: channel_type_id,
            manager_value_id: channel_value_id,
            external_name,
            message_direction,
            receive_message_processing,
            transactioned,
        });
    }
    if channels.is_empty() {
        return Err(MetadataDecodeError::Missing("IntegrationServiceChannel"));
    }
    Ok(IntegrationServiceProjection {
        comment: required_text(properties, "Comment", expected, &uris)?,
        manager_type_id,
        manager_value_id,
        external_address,
        channels,
    })
}

fn project_exact_manager_generated(
    internal: &XmlElement,
    uris: &ResolvedNamespaces,
    expected_name: &str,
) -> Result<(ObjectUuid, ObjectUuid), MetadataDecodeError> {
    reject_attributes(internal, &[])?;
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
            "Manager InternalInfo must contain one xr:GeneratedType",
        ));
    }
    let generated = generated[0];
    reject_attributes(generated, &["name", "category"])?;
    if unprefixed_attribute(generated, "name") != Some(expected_name)
        || unprefixed_attribute(generated, "category") != Some("Manager")
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "Manager generated type name or category is inconsistent",
        ));
    }
    let identities = generated
        .children()
        .iter()
        .filter_map(|node| match node {
            XmlNode::Element(child) => Some(child),
            _ => None,
        })
        .collect::<Vec<_>>();
    if identities.len() != 2
        || !typed(identities[0], "TypeId", Some(XR_NAMESPACE), uris)
        || !typed(identities[1], "ValueId", Some(XR_NAMESPACE), uris)
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "Manager generated TypeId/ValueId fields or order are unsupported",
        ));
    }
    let type_id = exact_generated_uuid(identities[0], "Manager TypeId")?;
    let value_id = exact_generated_uuid(identities[1], "Manager ValueId")?;
    if type_id == value_id {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "Manager TypeId and ValueId are not independent",
        ));
    }
    Ok((type_id, value_id))
}

fn exact_generated_uuid(
    element: &XmlElement,
    _context: &'static str,
) -> Result<ObjectUuid, MetadataDecodeError> {
    reject_attributes(element, &[])?;
    let value = element_text(element)?.ok_or(MetadataDecodeError::InvalidEnvelope(
        "generated UUID must contain text only",
    ))?;
    let uuid = ObjectUuid::parse(&value).map_err(|_| MetadataDecodeError::InvalidUuid(value))?;
    if uuid.as_bytes().iter().all(|byte| *byte == 0) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "generated UUID cannot be nil",
        ));
    }
    Ok(uuid)
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

fn reject_exact_web_root_properties(
    properties: &XmlElement,
    expected: Option<&str>,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    const REQUIRED: [&str; 8] = [
        "Name",
        "Synonym",
        "Comment",
        "Namespace",
        "XDTOPackages",
        "DescriptorFileName",
        "ReuseSessions",
        "SessionMaxAge",
    ];
    reject_attributes(properties, &[])?;
    let mut seen = BTreeSet::new();
    for node in properties.children() {
        let XmlNode::Element(child) = node else {
            continue;
        };
        let local = child.name().local();
        if !REQUIRED.contains(&local) || !typed(child, local, expected, uris) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "WebService contains an unknown property",
            ));
        }
        if local == "XDTOPackages" {
            reject_attributes(child, &[])?;
        } else {
            reject_attributes_recursive(child)?;
        }
        if !seen.insert(local) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "WebService contains a duplicate property",
            ));
        }
    }
    if REQUIRED.iter().any(|required| !seen.contains(required)) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WebService is missing a required property",
        ));
    }
    Ok(())
}

fn project_web_packages(
    packages: &XmlElement,
    root: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<(Vec<String>, Vec<String>), MetadataDecodeError> {
    let mut references = Vec::new();
    let mut namespaces = Vec::new();
    let mut seen_references = BTreeSet::new();
    let mut seen_namespaces = BTreeSet::new();
    for node in packages.children() {
        let XmlNode::Element(item) = node else {
            continue;
        };
        if !typed(item, "Item", Some(XR_NAMESPACE), uris) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "WebService XDTOPackages contains an unknown item",
            ));
        }
        reject_attributes(item, &[])?;
        let presentation = required_child(item, "Presentation", Some(XR_NAMESPACE), uris)?;
        let check_state = required_child(item, "CheckState", Some(XR_NAMESPACE), uris)?;
        let value = required_child(item, "Value", Some(XR_NAMESPACE), uris)?;
        if item
            .children()
            .iter()
            .filter(|node| matches!(node, XmlNode::Element(_)))
            .count()
            != 3
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "WebService XDTOPackages item is not exact",
            ));
        }
        reject_attributes_recursive(presentation)?;
        if element_text(presentation)?.as_deref() != Some("") {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "WebService XDTOPackages Presentation is not empty",
            ));
        }
        reject_attributes_recursive(check_state)?;
        if element_text(check_state)?.as_deref() != Some("0") {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "WebService XDTOPackages CheckState is not zero",
            ));
        }
        let value_type = exact_xsi_type(value)?;
        let content = element_text(value)?.ok_or(MetadataDecodeError::InvalidEnvelope(
            "WebService XDTOPackages Value is not text-only",
        ))?;
        match value_type {
            "xr:MDObjectRef" => {
                require_namespace_binding(root, "xr", XR_NAMESPACE)?;
                let Some(name) = content.strip_prefix("XDTOPackage.") else {
                    return Err(MetadataDecodeError::InvalidEnvelope(
                        "WebService XDTOPackage reference has an invalid prefix",
                    ));
                };
                if !valid_identifier(name) || !seen_references.insert(content.clone()) {
                    return Err(MetadataDecodeError::InvalidEnvelope(
                        "WebService XDTOPackage reference is invalid or duplicated",
                    ));
                }
                references.push(content);
            }
            "xs:string" => {
                require_namespace_binding(root, "xs", XML_SCHEMA_NAMESPACE)?;
                if !valid_namespace(&content) || !seen_namespaces.insert(content.clone()) {
                    return Err(MetadataDecodeError::InvalidEnvelope(
                        "WebService XDTO namespace is invalid or duplicated",
                    ));
                }
                namespaces.push(content);
            }
            _ => {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "WebService XDTOPackages Value type is unsupported",
                ));
            }
        }
    }
    if !references.is_empty() || !namespaces.is_empty() {
        require_namespace_binding(root, "xsi", XSI_NAMESPACE)?;
    }
    Ok((references, namespaces))
}

fn exact_xsi_type(value: &XmlElement) -> Result<&str, MetadataDecodeError> {
    let mut found = None;
    for attribute in value.attributes() {
        match attribute.kind() {
            AttributeKind::Ordinary(name)
                if name.prefix() == Some("xsi") && name.local() == "type" =>
            {
                if found.replace(attribute.value()).is_some() {
                    return Err(MetadataDecodeError::Duplicate("xsi:type"));
                }
            }
            _ => {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "WebService XDTOPackages Value attribute is unknown",
                ));
            }
        }
    }
    found.ok_or(MetadataDecodeError::Missing("xsi:type"))
}

fn project_xdto_type(
    properties: &XmlElement,
    local: &'static str,
    expected: Option<&str>,
    uris: &ResolvedNamespaces,
    root: &XmlElement,
) -> Result<XdtoTypeProjection, MetadataDecodeError> {
    let value = required_text(properties, local, expected, uris)?;
    let Some((prefix, name)) = value.split_once(':') else {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WebService XDTO type is not a qualified name",
        ));
    };
    if prefix.is_empty()
        || !valid_identifier(name)
        || name.contains(':')
        || prefix.chars().any(char::is_whitespace)
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WebService XDTO type qualified name is invalid",
        ));
    }
    let namespace = namespace_binding(root, prefix).ok_or(MetadataDecodeError::InvalidEnvelope(
        "WebService XDTO type prefix is not root-bound",
    ))?;
    if !valid_namespace(namespace) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "WebService XDTO type namespace is invalid",
        ));
    }
    Ok(XdtoTypeProjection {
        namespace: namespace.to_owned(),
        name: name.to_owned(),
    })
}

fn require_namespace_binding(
    root: &XmlElement,
    prefix: &str,
    expected: &str,
) -> Result<(), MetadataDecodeError> {
    if namespace_binding(root, prefix) == Some(expected) {
        Ok(())
    } else {
        Err(MetadataDecodeError::InvalidEnvelope(
            "WebService requires an exact root namespace binding",
        ))
    }
}

fn namespace_binding<'a>(root: &'a XmlElement, prefix: &str) -> Option<&'a str> {
    root.attributes().iter().find_map(|attribute| {
        if let AttributeKind::Namespace(Some(candidate)) = attribute.kind()
            && candidate == prefix
        {
            Some(attribute.value())
        } else {
            None
        }
    })
}

fn valid_namespace(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_whitespace)
}

fn valid_descriptor_file_name(value: &str) -> bool {
    value.ends_with(".1cws")
        && value.len() > ".1cws".len()
        && !value.chars().any(char::is_whitespace)
}

fn exact_http_sections<'a>(
    object: &'a XmlElement,
    expected: Option<&str>,
    uris: &ResolvedNamespaces,
    require_children: bool,
) -> Result<(&'a XmlElement, Option<&'a XmlElement>), MetadataDecodeError> {
    let mut properties = None;
    let mut children = None;
    for node in object.children() {
        let XmlNode::Element(child) = node else {
            continue;
        };
        if typed(child, "Properties", expected, uris) {
            if properties.replace(child).is_some() {
                return Err(MetadataDecodeError::Duplicate("Properties"));
            }
        } else if typed(child, "ChildObjects", expected, uris) {
            if children.replace(child).is_some() {
                return Err(MetadataDecodeError::Duplicate("ChildObjects"));
            }
        } else {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "HTTPService object contains an unknown section",
            ));
        }
    }
    let properties = properties.ok_or(MetadataDecodeError::Missing("Properties"))?;
    if require_children && children.is_none() {
        return Err(MetadataDecodeError::Missing("ChildObjects"));
    }
    Ok((properties, children))
}

fn reject_exact_service_properties(
    properties: &XmlElement,
    required: &[&str],
    expected: Option<&str>,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    reject_attributes(properties, &[])?;
    let mut seen = BTreeSet::new();
    for node in properties.children() {
        let XmlNode::Element(child) = node else {
            continue;
        };
        let local = child.name().local();
        if !required.contains(&local) || !typed(child, local, expected, uris) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "service object contains an unknown property",
            ));
        }
        reject_attributes_recursive(child)?;
        if !seen.insert(local) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "service object contains a duplicate property",
            ));
        }
    }
    if required.iter().any(|required| !seen.contains(required)) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "service object is missing a required property",
        ));
    }
    Ok(())
}

fn required_uuid(element: &XmlElement) -> Result<ObjectUuid, MetadataDecodeError> {
    let mut value = None;
    for attribute in element.attributes() {
        if let AttributeKind::Ordinary(name) = attribute.kind()
            && name.prefix().is_none()
            && name.local() == "uuid"
            && value.replace(attribute.value()).is_some()
        {
            return Err(MetadataDecodeError::Duplicate("uuid"));
        }
    }
    let value = value.ok_or(MetadataDecodeError::Missing("uuid"))?;
    ObjectUuid::parse(value).map_err(|_| MetadataDecodeError::InvalidUuid(value.to_owned()))
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

fn encode_http_service(
    envelope: &MetadataEnvelope,
    target: &ProfileId,
) -> Result<Vec<u8>, MetadataEncodeError> {
    let path = envelope.root().identity().path().clone();
    let target_version =
        profile_version(target).ok_or_else(|| MetadataEncodeError::UnsupportedProfile {
            object_path: path.clone(),
            profile: target.clone(),
        })?;
    if envelope.root().kind().as_str() != HTTP_SERVICE_FAMILY {
        return Err(invalid_model(&path, "kind"));
    }
    let source_profile = envelope.root().provenance().source_profile().clone();
    validate_decode_profile(envelope.source_document(), &source_profile, &path)
        .map_err(decode_to_encode)?;
    let source = decode_http_service(envelope.source_document(), source_profile, path.clone())
        .map_err(decode_to_encode)?;
    if source.root() != envelope.root() || source.descendants() != envelope.descendants() {
        return Err(invalid_model(
            &path,
            "HTTPService semantic mutation is not implemented",
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

fn encode_web_service(
    envelope: &MetadataEnvelope,
    target: &ProfileId,
) -> Result<Vec<u8>, MetadataEncodeError> {
    let path = envelope.root().identity().path().clone();
    let target_version =
        profile_version(target).ok_or_else(|| MetadataEncodeError::UnsupportedProfile {
            object_path: path.clone(),
            profile: target.clone(),
        })?;
    if envelope.root().kind().as_str() != WEB_SERVICE_FAMILY {
        return Err(invalid_model(&path, "kind"));
    }
    let source_profile = envelope.root().provenance().source_profile().clone();
    validate_decode_profile(envelope.source_document(), &source_profile, &path)
        .map_err(decode_to_encode)?;
    let source = decode_web_service(envelope.source_document(), source_profile, path.clone())
        .map_err(decode_to_encode)?;
    if source.root() != envelope.root() || source.descendants() != envelope.descendants() {
        return Err(invalid_model(
            &path,
            "WebService semantic mutation is not implemented",
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

fn encode_ws_reference(
    envelope: &MetadataEnvelope,
    target: &ProfileId,
) -> Result<Vec<u8>, MetadataEncodeError> {
    let path = envelope.root().identity().path().clone();
    let target_version =
        profile_version(target).ok_or_else(|| MetadataEncodeError::UnsupportedProfile {
            object_path: path.clone(),
            profile: target.clone(),
        })?;
    if envelope.root().kind().as_str() != WS_REFERENCE_FAMILY {
        return Err(invalid_model(&path, "kind"));
    }
    let source_profile = envelope.root().provenance().source_profile().clone();
    validate_decode_profile(envelope.source_document(), &source_profile, &path)
        .map_err(decode_to_encode)?;
    let source = decode_ws_reference(envelope.source_document(), source_profile, path.clone())
        .map_err(decode_to_encode)?;
    if source.root() != envelope.root() || source.descendants() != envelope.descendants() {
        return Err(invalid_model(
            &path,
            "WSReference semantic mutation is not implemented",
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

fn encode_integration_service(
    envelope: &MetadataEnvelope,
    target: &ProfileId,
) -> Result<Vec<u8>, MetadataEncodeError> {
    let path = envelope.root().identity().path().clone();
    let target_version =
        profile_version(target).ok_or_else(|| MetadataEncodeError::UnsupportedProfile {
            object_path: path.clone(),
            profile: target.clone(),
        })?;
    if envelope.root().kind().as_str() != INTEGRATION_SERVICE_FAMILY {
        return Err(invalid_model(&path, "kind"));
    }
    let source_profile = envelope.root().provenance().source_profile().clone();
    validate_decode_profile(envelope.source_document(), &source_profile, &path)
        .map_err(decode_to_encode)?;
    let source =
        decode_integration_service(envelope.source_document(), source_profile, path.clone())
            .map_err(decode_to_encode)?;
    if source.root() != envelope.root() || source.descendants() != envelope.descendants() {
        return Err(invalid_model(
            &path,
            "IntegrationService semantic mutation is not implemented",
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

    fn http_fixture(version: &str, http_method: &str, extra: &str) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"{MD_NAMESPACE}\" xmlns:v8=\"{V8_NAMESPACE}\" version=\"{version}\">\r\n\
\t<HTTPService uuid=\"db821e7a-ff22-4889-b166-1a1bc1118587\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>Биллинг</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Биллинг</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<RootURL>billing</RootURL>\r\n\
\t\t\t<ReuseSessions>AutoUse</ReuseSessions>\r\n\
\t\t\t<SessionMaxAge>20</SessionMaxAge>{extra}\r\n\
\t\t</Properties>\r\n\
\t\t<ChildObjects>\r\n\
\t\t\t<URLTemplate uuid=\"bbd4d7c8-2488-474c-b92c-8f689a56e62e\">\r\n\
\t\t\t\t<Properties>\r\n\
\t\t\t\t\t<Name>Версия</Name><Synonym/><Comment/><Template>/version</Template>\r\n\
\t\t\t\t</Properties>\r\n\
\t\t\t\t<ChildObjects>\r\n\
\t\t\t\t\t<Method uuid=\"f909d950-4db8-490c-aaf6-7a2e975a310d\">\r\n\
\t\t\t\t\t\t<Properties>\r\n\
\t\t\t\t\t\t\t<Name>Получить</Name><Synonym/><Comment/><HTTPMethod>{http_method}</HTTPMethod><Handler>ВерсияПолучить</Handler>\r\n\
\t\t\t\t\t\t</Properties>\r\n\
\t\t\t\t\t</Method>\r\n\
\t\t\t\t</ChildObjects>\r\n\
\t\t\t</URLTemplate>\r\n\
\t\t</ChildObjects>\r\n\
\t</HTTPService>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    #[test]
    fn http_service_children_are_typed_and_cross_version_roundtrip() {
        let mut registry = MetadataRegistry::default();
        register_http_service_codec(&mut registry).unwrap();
        for version in ["2.20", "2.21"] {
            let document = XmlReader::from_slice(&http_fixture(version, "GET", "")).unwrap();
            let envelope = registry
                .decode(
                    &FamilyId::parse(HTTP_SERVICE_FAMILY).unwrap(),
                    &document,
                    profile(version),
                    ObjectPath::root(),
                )
                .unwrap();
            assert_eq!(envelope.root().properties().len(), 6);
            assert_eq!(envelope.descendants().len(), 2);
            assert_eq!(envelope.descendants()[0].properties().len(), 4);
            assert_eq!(envelope.descendants()[1].properties().len(), 5);
            assert!(matches!(
                envelope.descendants()[1].properties()[3].value().kind(),
                CanonicalValueKind::EnumToken(_)
            ));
            let target = if version == "2.20" { "2.21" } else { "2.20" };
            let output = registry.encode(&envelope, &profile(target)).unwrap();
            let reparsed = XmlReader::from_slice(&output).unwrap();
            registry
                .decode(
                    &FamilyId::parse(HTTP_SERVICE_FAMILY).unwrap(),
                    &reparsed,
                    profile(target),
                    ObjectPath::root(),
                )
                .unwrap();
        }
    }

    #[test]
    fn http_service_unknown_method_property_and_code_fail_closed() {
        let mut registry = MetadataRegistry::default();
        register_http_service_codec(&mut registry).unwrap();
        for input in [
            http_fixture("2.20", "PATCH", ""),
            http_fixture("2.20", "GET", "<Future/>"),
        ] {
            let document = XmlReader::from_slice(&input).unwrap();
            assert!(
                registry
                    .decode(
                        &FamilyId::parse(HTTP_SERVICE_FAMILY).unwrap(),
                        &document,
                        profile("2.20"),
                        ObjectPath::root(),
                    )
                    .is_err()
            );
        }
    }

    fn web_fixture(
        version: &str,
        data_lock_mode: &str,
        package_type: &str,
        extra: &str,
    ) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"{MD_NAMESPACE}\" xmlns:v8=\"{V8_NAMESPACE}\" xmlns:xr=\"{XR_NAMESPACE}\" xmlns:xs=\"{XML_SCHEMA_NAMESPACE}\" xmlns:xsi=\"{XSI_NAMESPACE}\" xmlns:svc=\"http://example.test/types\" version=\"{version}\">\r\n\
\t<WebService uuid=\"a4ed8b24-bd23-45a7-9f34-61b25b91d0c6\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>InterfaceVersion</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Interface version</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<Namespace>http://www.1c.ru/SaaS/1.0/WS</Namespace>\r\n\
\t\t\t<XDTOPackages>\r\n\
\t\t\t\t<xr:Item><xr:Presentation/><xr:CheckState>0</xr:CheckState><xr:Value xsi:type=\"{package_type}\">XDTOPackage.Custom</xr:Value></xr:Item>\r\n\
\t\t\t\t<xr:Item><xr:Presentation/><xr:CheckState>0</xr:CheckState><xr:Value xsi:type=\"xs:string\">http://v8.1c.ru/8.1/data/core</xr:Value></xr:Item>\r\n\
\t\t\t</XDTOPackages>\r\n\
\t\t\t<DescriptorFileName>InterfaceVersion.1cws</DescriptorFileName>\r\n\
\t\t\t<ReuseSessions>DontUse</ReuseSessions>\r\n\
\t\t\t<SessionMaxAge>20</SessionMaxAge>{extra}\r\n\
\t\t</Properties>\r\n\
\t\t<ChildObjects>\r\n\
\t\t\t<Operation uuid=\"65efaa10-3239-4f0f-a08e-88c89d9d8d5a\">\r\n\
\t\t\t\t<Properties>\r\n\
\t\t\t\t\t<Name>GetVersions</Name><Synonym/><Comment/>\r\n\
\t\t\t\t\t<XDTOReturningValueType>svc:Result</XDTOReturningValueType>\r\n\
\t\t\t\t\t<Nillable>true</Nillable><Transactioned>false</Transactioned>\r\n\
\t\t\t\t\t<ProcedureName>GetVersions</ProcedureName><DataLockControlMode>{data_lock_mode}</DataLockControlMode>\r\n\
\t\t\t\t</Properties>\r\n\
\t\t\t\t<ChildObjects>\r\n\
\t\t\t\t\t<Parameter uuid=\"93aa5247-6823-4f70-9d47-e5a0f409828a\"><Properties><Name>InterfaceName</Name><Synonym/><Comment/><XDTOValueType>xs:string</XDTOValueType><Nillable>false</Nillable><TransferDirection>In</TransferDirection></Properties></Parameter>\r\n\
\t\t\t\t\t<Parameter uuid=\"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa\"><Properties><Name>ResultId</Name><Synonym/><Comment/><XDTOValueType>v8:UUID</XDTOValueType><Nillable>true</Nillable><TransferDirection>Out</TransferDirection></Properties></Parameter>\r\n\
\t\t\t\t</ChildObjects>\r\n\
\t\t\t</Operation>\r\n\
\t\t</ChildObjects>\r\n\
\t</WebService>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    #[test]
    fn web_service_tree_packages_and_types_are_typed_and_roundtrip() {
        let mut registry = MetadataRegistry::default();
        register_web_service_codec(&mut registry).unwrap();
        for version in ["2.20", "2.21"] {
            let document =
                XmlReader::from_slice(&web_fixture(version, "Managed", "xr:MDObjectRef", ""))
                    .unwrap();
            let envelope = registry
                .decode(
                    &FamilyId::parse(WEB_SERVICE_FAMILY).unwrap(),
                    &document,
                    profile(version),
                    ObjectPath::root(),
                )
                .unwrap();
            assert_eq!(envelope.root().properties().len(), 8);
            assert_eq!(envelope.descendants().len(), 3);
            assert_eq!(envelope.descendants()[0].properties().len(), 8);
            assert_eq!(envelope.descendants()[1].properties().len(), 6);
            assert!(matches!(
                envelope.root().properties()[4].value().kind(),
                CanonicalValueKind::Record(_)
            ));
            assert!(matches!(
                envelope.descendants()[0].properties()[3].value().kind(),
                CanonicalValueKind::Record(_)
            ));
            let target = if version == "2.20" { "2.21" } else { "2.20" };
            let output = registry.encode(&envelope, &profile(target)).unwrap();
            let reparsed = XmlReader::from_slice(&output).unwrap();
            registry
                .decode(
                    &FamilyId::parse(WEB_SERVICE_FAMILY).unwrap(),
                    &reparsed,
                    profile(target),
                    ObjectPath::root(),
                )
                .unwrap();
        }
    }

    #[test]
    fn web_service_unknown_package_type_lock_mode_and_property_fail_closed() {
        let mut registry = MetadataRegistry::default();
        register_web_service_codec(&mut registry).unwrap();
        let transactioned = String::from_utf8(web_fixture("2.20", "Managed", "xr:MDObjectRef", ""))
            .unwrap()
            .replacen("<Transactioned>false", "<Transactioned>true", 1)
            .into_bytes();
        for input in [
            web_fixture("2.20", "Automatic", "xr:MDObjectRef", ""),
            web_fixture("2.20", "Managed", "xs:int", ""),
            web_fixture("2.20", "Managed", "xr:MDObjectRef", "<Future/>"),
            transactioned,
        ] {
            let document = XmlReader::from_slice(&input).unwrap();
            assert!(
                registry
                    .decode(
                        &FamilyId::parse(WEB_SERVICE_FAMILY).unwrap(),
                        &document,
                        profile("2.20"),
                        ObjectPath::root(),
                    )
                    .is_err()
            );
        }
    }

    fn ws_reference_fixture(
        version: &str,
        generated_name: &str,
        category: &str,
        location: &str,
        extra: &str,
    ) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"{MD_NAMESPACE}\" xmlns:v8=\"{V8_NAMESPACE}\" xmlns:xr=\"{XR_NAMESPACE}\" version=\"{version}\">\r\n\
\t<WSReference uuid=\"b409116f-3ba2-4303-9bdc-f14961c879d6\">\r\n\
\t\t<InternalInfo>\r\n\
\t\t\t<xr:GeneratedType name=\"{generated_name}\" category=\"{category}\">\r\n\
\t\t\t\t<xr:TypeId>651f0326-6551-49a6-a840-b6e604b61639</xr:TypeId>\r\n\
\t\t\t\t<xr:ValueId>dd7a8d59-2aeb-4921-a33b-913be961ec98</xr:ValueId>\r\n\
\t\t\t</xr:GeneratedType>\r\n\
\t\t</InternalInfo>\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>UpdateFilesApiImplService</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Update files api impl service</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<LocationURL>{location}</LocationURL>{extra}\r\n\
\t\t</Properties>\r\n\
\t</WSReference>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    #[test]
    fn ws_reference_manager_and_location_are_typed_and_roundtrip() {
        let mut registry = MetadataRegistry::default();
        register_ws_reference_codec(&mut registry).unwrap();
        for version in ["2.20", "2.21"] {
            let document = XmlReader::from_slice(&ws_reference_fixture(
                version,
                "WSReferenceManager.UpdateFilesApiImplService",
                "Manager",
                "https://update-api.1c.ru/ws/files?wsdl",
                "",
            ))
            .unwrap();
            let envelope = registry
                .decode(
                    &FamilyId::parse(WS_REFERENCE_FAMILY).unwrap(),
                    &document,
                    profile(version),
                    ObjectPath::root(),
                )
                .unwrap();
            assert_eq!(envelope.root().properties().len(), 4);
            assert_eq!(envelope.root().generated_types().len(), 1);
            assert_eq!(
                envelope.root().generated_types()[0].value_id(),
                Some(ObjectUuid::parse("dd7a8d59-2aeb-4921-a33b-913be961ec98").unwrap())
            );
            let target = if version == "2.20" { "2.21" } else { "2.20" };
            let output = registry.encode(&envelope, &profile(target)).unwrap();
            let reparsed = XmlReader::from_slice(&output).unwrap();
            registry
                .decode(
                    &FamilyId::parse(WS_REFERENCE_FAMILY).unwrap(),
                    &reparsed,
                    profile(target),
                    ObjectPath::root(),
                )
                .unwrap();
        }
    }

    #[test]
    fn ws_reference_inconsistent_generated_type_and_unknown_property_fail_closed() {
        let mut registry = MetadataRegistry::default();
        register_ws_reference_codec(&mut registry).unwrap();
        for input in [
            ws_reference_fixture(
                "2.20",
                "WSReferenceManager.Other",
                "Manager",
                "https://example.test/ws?wsdl",
                "",
            ),
            ws_reference_fixture(
                "2.20",
                "WSReferenceManager.UpdateFilesApiImplService",
                "Other",
                "https://example.test/ws?wsdl",
                "",
            ),
            ws_reference_fixture(
                "2.20",
                "WSReferenceManager.UpdateFilesApiImplService",
                "Manager",
                "https://example.test/ws wsdl",
                "",
            ),
            ws_reference_fixture(
                "2.20",
                "WSReferenceManager.UpdateFilesApiImplService",
                "Manager",
                "https://example.test/ws?wsdl",
                "<Future/>",
            ),
        ] {
            let document = XmlReader::from_slice(&input).unwrap();
            assert!(
                registry
                    .decode(
                        &FamilyId::parse(WS_REFERENCE_FAMILY).unwrap(),
                        &document,
                        profile("2.20"),
                        ObjectPath::root(),
                    )
                    .is_err()
            );
        }
    }

    fn integration_service_fixture(
        version: &str,
        receive_direction: &str,
        receive_handler: &str,
        send_transactioned: &str,
        extra: &str,
    ) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"{MD_NAMESPACE}\" xmlns:v8=\"{V8_NAMESPACE}\" xmlns:xr=\"{XR_NAMESPACE}\" version=\"{version}\">\r\n\
\t<IntegrationService uuid=\"c512a1cd-1240-4e46-8bad-8b7b27c5c25a\">\r\n\
\t\t<InternalInfo>\r\n\
\t\t\t<xr:GeneratedType name=\"IntegrationServiceManager.ОбменСообщениями\" category=\"Manager\">\r\n\
\t\t\t\t<xr:TypeId>5362f1d1-1f56-4a61-a52e-6519a060293e</xr:TypeId>\r\n\
\t\t\t\t<xr:ValueId>ad884943-3c3a-4073-ab34-ed12a0d67556</xr:ValueId>\r\n\
\t\t\t</xr:GeneratedType>\r\n\
\t\t</InternalInfo>\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>ОбменСообщениями</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Обмен сообщениями</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<ExternalIntegrationServiceAddress/>{extra}\r\n\
\t\t</Properties>\r\n\
\t\t<ChildObjects>\r\n\
\t\t\t<IntegrationServiceChannel uuid=\"1ef0581c-b1d8-4115-87f1-7856f6c06bb6\">\r\n\
\t\t\t\t<InternalInfo>\r\n\
\t\t\t\t\t<xr:GeneratedType name=\"IntegrationServiceChannelManager.ОбменСообщениями.input_from_SM_normal_priority\" category=\"Manager\">\r\n\
\t\t\t\t\t\t<xr:TypeId>71313d47-3c6e-464a-8776-f7eb0626fd6b</xr:TypeId>\r\n\
\t\t\t\t\t\t<xr:ValueId>bb1ff475-725d-46cb-8cbc-9ff08970cccc</xr:ValueId>\r\n\
\t\t\t\t\t</xr:GeneratedType>\r\n\
\t\t\t\t</InternalInfo>\r\n\
\t\t\t\t<Properties>\r\n\
\t\t\t\t\t<Name>input_from_SM_normal_priority</Name>\r\n\
\t\t\t\t\t<Synonym/>\r\n\
\t\t\t\t\t<Comment/>\r\n\
\t\t\t\t\t<ExternalIntegrationServiceChannelName>e1c::FreshBus::Main::MessageExchange_v2.input_from_SM_normal_priority</ExternalIntegrationServiceChannelName>\r\n\
\t\t\t\t\t<MessageDirection>{receive_direction}</MessageDirection>\r\n\
\t\t\t\t\t<ReceiveMessageProcessing>{receive_handler}</ReceiveMessageProcessing>\r\n\
\t\t\t\t\t<Transactioned>false</Transactioned>\r\n\
\t\t\t\t</Properties>\r\n\
\t\t\t</IntegrationServiceChannel>\r\n\
\t\t\t<IntegrationServiceChannel uuid=\"b017ac62-a4a2-47bd-b963-50e0764a7d4e\">\r\n\
\t\t\t\t<InternalInfo>\r\n\
\t\t\t\t\t<xr:GeneratedType name=\"IntegrationServiceChannelManager.ОбменСообщениями.output_to_SM_high_priority\" category=\"Manager\">\r\n\
\t\t\t\t\t\t<xr:TypeId>fa26d8bb-bc63-4707-926d-64b8c10cd13d</xr:TypeId>\r\n\
\t\t\t\t\t\t<xr:ValueId>301c529a-896f-4da6-946e-a28690af5399</xr:ValueId>\r\n\
\t\t\t\t\t</xr:GeneratedType>\r\n\
\t\t\t\t</InternalInfo>\r\n\
\t\t\t\t<Properties>\r\n\
\t\t\t\t\t<Name>output_to_SM_high_priority</Name>\r\n\
\t\t\t\t\t<Synonym/>\r\n\
\t\t\t\t\t<Comment/>\r\n\
\t\t\t\t\t<ExternalIntegrationServiceChannelName>e1c::FreshBus::Main::MessageExchange_v2.output_to_SM_high_priority</ExternalIntegrationServiceChannelName>\r\n\
\t\t\t\t\t<MessageDirection>Send</MessageDirection>\r\n\
\t\t\t\t\t<ReceiveMessageProcessing/>\r\n\
\t\t\t\t\t<Transactioned>{send_transactioned}</Transactioned>\r\n\
\t\t\t\t</Properties>\r\n\
\t\t\t</IntegrationServiceChannel>\r\n\
\t\t</ChildObjects>\r\n\
\t</IntegrationService>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    #[test]
    fn integration_service_channels_are_typed_and_cross_version_roundtrip() {
        let mut registry = MetadataRegistry::default();
        register_integration_service_codec(&mut registry).unwrap();
        for version in ["2.20", "2.21"] {
            let document = XmlReader::from_slice(&integration_service_fixture(
                version,
                "Receive",
                "ОбработатьСообщениеОбычныйПриоритет",
                "true",
                "",
            ))
            .unwrap();
            let envelope = registry
                .decode(
                    &FamilyId::parse(INTEGRATION_SERVICE_FAMILY).unwrap(),
                    &document,
                    profile(version),
                    ObjectPath::root(),
                )
                .unwrap();
            assert_eq!(envelope.root().properties().len(), 4);
            assert_eq!(envelope.root().generated_types().len(), 1);
            assert_eq!(envelope.descendants().len(), 2);
            assert_eq!(envelope.descendants()[0].properties().len(), 7);
            assert!(matches!(
                envelope.descendants()[0].properties()[4].value().kind(),
                CanonicalValueKind::EnumToken(_)
            ));
            assert!(matches!(
                envelope.descendants()[1].properties()[6].value().kind(),
                CanonicalValueKind::Bool(true)
            ));
            let target = if version == "2.20" { "2.21" } else { "2.20" };
            let output = registry.encode(&envelope, &profile(target)).unwrap();
            let reparsed = XmlReader::from_slice(&output).unwrap();
            registry
                .decode(
                    &FamilyId::parse(INTEGRATION_SERVICE_FAMILY).unwrap(),
                    &reparsed,
                    profile(target),
                    ObjectPath::root(),
                )
                .unwrap();
        }
    }

    #[test]
    fn integration_service_unknown_identity_direction_and_property_fail_closed() {
        let mut registry = MetadataRegistry::default();
        register_integration_service_codec(&mut registry).unwrap();
        let wrong_manager = String::from_utf8(integration_service_fixture(
            "2.20",
            "Receive",
            "ОбработатьСообщениеОбычныйПриоритет",
            "true",
            "",
        ))
        .unwrap()
        .replacen(
            "IntegrationServiceManager.ОбменСообщениями",
            "IntegrationServiceManager.Другое",
            1,
        )
        .into_bytes();
        for input in [
            wrong_manager,
            integration_service_fixture("2.20", "Receive", "", "true", ""),
            integration_service_fixture(
                "2.20",
                "Unknown",
                "ОбработатьСообщениеОбычныйПриоритет",
                "true",
                "",
            ),
            integration_service_fixture(
                "2.20",
                "Receive",
                "ОбработатьСообщениеОбычныйПриоритет",
                "false",
                "",
            ),
            integration_service_fixture(
                "2.20",
                "Receive",
                "ОбработатьСообщениеОбычныйПриоритет",
                "true",
                "<Future/>",
            ),
        ] {
            let document = XmlReader::from_slice(&input).unwrap();
            assert!(
                registry
                    .decode(
                        &FamilyId::parse(INTEGRATION_SERVICE_FAMILY).unwrap(),
                        &document,
                        profile("2.20"),
                        ObjectPath::root(),
                    )
                    .is_err()
            );
        }
    }
}
