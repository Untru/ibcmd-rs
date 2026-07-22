//! Base-free native codecs for BOOT-004 service metadata families.
//!
//! Every family owns an explicit platform-profile layout. Future layouts remain
//! profile-selection failures until their full native shell is independently
//! evidenced.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter, Write as _};
use std::io::{self, Read, Write};

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use ibcmd_core::artifact::{ProfileId, StorageProfileId};
use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::model::CanonicalObject;
use ibcmd_core::profile::EffectiveProfile;
use ibcmd_core::storage::{
    MultipartIdentity, StorageBuildError, StoragePatchBuildError, StoragePatchEntry,
    StoragePatchOutcome, StoragePatchTarget, StorageProvenance,
};
use ibcmd_core::validate::ValidatedConfiguration;
use ibcmd_core::value::{
    CanonicalField, CanonicalValue, CanonicalValueKind, MAX_CANONICAL_COLLECTION_ITEMS,
    MAX_CANONICAL_RETAINED_BYTES, MAX_CANONICAL_TEXT_BYTES,
};
use ibcmd_core::version::PlatformBuild;

use super::super::CompileAxes;
use super::super::graph::BootstrapGraph;

const SCHEDULED_JOB_LAYOUT_KEY: &str = "bootstrap.metadata.scheduled_job.layout";
const SCHEDULED_JOB_LAYOUT: &str = "scheduled-job-v1-crlf-utf8-bom";
const EVENT_SUBSCRIPTION_LAYOUT_KEY: &str = "bootstrap.metadata.event_subscription.layout";
const EVENT_SUBSCRIPTION_LAYOUT: &str = "event-subscription-v1-crlf-utf8-bom";
const XDTO_PACKAGE_LAYOUT_KEY: &str = "bootstrap.metadata.xdto_package.layout";
const XDTO_PACKAGE_LAYOUT: &str = "xdto-package-v1-crlf-utf8-bom";
const HTTP_SERVICE_LAYOUT_KEY: &str = "bootstrap.metadata.http_service.layout";
const HTTP_SERVICE_LAYOUT: &str = "http-service-v1-crlf-utf8-bom";
const WEB_SERVICE_LAYOUT_KEY: &str = "bootstrap.metadata.web_service.layout";
const WEB_SERVICE_LAYOUT: &str = "web-service-v1-crlf-utf8-bom";
const WS_REFERENCE_LAYOUT_KEY: &str = "bootstrap.metadata.ws_reference.layout";
const WS_REFERENCE_LAYOUT: &str = "ws-reference-v1-crlf-utf8-bom";
const INTEGRATION_SERVICE_LAYOUT_KEY: &str = "bootstrap.metadata.integration_service.layout";
const INTEGRATION_SERVICE_LAYOUT: &str = "integration-service-v1-crlf-utf8-bom";
const SUPPORTED_STORAGE_PROFILE: &str = "storage:mssql-config-configsave";
const UTF8_BOM: &[u8; 3] = b"\xef\xbb\xbf";
const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";
const MAX_SERVICE_METADATA_PLAIN_BYTES: usize = MAX_CANONICAL_RETAINED_BYTES + 4 * 1_048_576;
// HTTP/Web service trees nest collection, route, method, header, and identity
// shells; keep the parser bounded while covering the deepest evidenced row.
const MAX_NATIVE_DEPTH: usize = 12;
const MAX_NATIVE_NODES: usize = 100_000;
const MAX_LANGUAGE_CODE_BYTES: usize = 256;
const HTTP_URL_COLLECTION_UUID: &str = "ec6896c2-9b28-42d8-9140-48491146b8ea";
const HTTP_METHOD_COLLECTION_UUID: &str = "21c96ea8-c8fc-424a-a0b4-e1ffb2fa1a73";
const WEB_OPERATION_COLLECTION_UUID: &str = "36186084-c23a-43bd-876c-a3a8ba1a9622";
const WEB_PARAMETER_COLLECTION_UUID: &str = "b78a00b2-2260-4ef5-a70c-17889cfee695";
const INTEGRATION_CHANNEL_COLLECTION_UUID: &str = "acb7e81f-0637-4ebd-88ff-954ba075ae51";
const DESIGN_TIME_REFERENCE_CLASS_UUID: &str = "157fa490-4ce9-11d4-9415-008048da11f9";
const XDTO_XML_SCHEMA_NAMESPACE: &str = "http://www.w3.org/2001/XMLSchema";
const XDTO_CORE_NAMESPACE: &str = "http://v8.1c.ru/8.1/data/core";
const XDTO_XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
const XDTO_XMLNS_NAMESPACE: &str = "http://www.w3.org/2000/xmlns/";

/// BOOT-004 families. Each variant evolves through its own layout constant.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ServiceFamily {
    ScheduledJob,
    EventSubscription,
    HttpService,
    WebService,
    IntegrationService,
    WsReference,
    XdtoPackage,
}

impl ServiceFamily {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ScheduledJob => "ScheduledJob",
            Self::EventSubscription => "EventSubscription",
            Self::HttpService => "HTTPService",
            Self::WebService => "WebService",
            Self::IntegrationService => "IntegrationService",
            Self::WsReference => "WSReference",
            Self::XdtoPackage => "XDTOPackage",
        }
    }

    fn from_kind(kind: &str) -> Option<Self> {
        match kind {
            "ScheduledJob" => Some(Self::ScheduledJob),
            "EventSubscription" => Some(Self::EventSubscription),
            "HTTPService" => Some(Self::HttpService),
            "WebService" => Some(Self::WebService),
            "IntegrationService" => Some(Self::IntegrationService),
            "WSReference" => Some(Self::WsReference),
            "XDTOPackage" => Some(Self::XdtoPackage),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ServiceLayout {
    ScheduledJobV1,
    EventSubscriptionV1,
    XdtoPackageV1,
    HttpServiceV1,
    WebServiceV1,
    WsReferenceV1,
    IntegrationServiceV1,
}

/// Independent target coordinates plus one service-family layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceMetadataProfile {
    profile_id: ProfileId,
    platform_build: PlatformBuild,
    storage_profile: StorageProfileId,
    family: ServiceFamily,
    layout: ServiceLayout,
}

impl ServiceMetadataProfile {
    pub fn from_effective_for_family(
        profile: &EffectiveProfile,
        family: ServiceFamily,
    ) -> Result<Self, ServiceMetadataProfileError> {
        let platform_build = profile
            .platform_build
            .as_ref()
            .map(|value| value.value.clone())
            .ok_or_else(|| ServiceMetadataProfileError::MissingCoordinate {
                profile: profile.id.clone(),
                coordinate: "platform_build",
            })?;
        let storage_profile = profile
            .storage_profile
            .as_ref()
            .map(|value| value.value.clone())
            .ok_or_else(|| ServiceMetadataProfileError::MissingCoordinate {
                profile: profile.id.clone(),
                coordinate: "storage_profile",
            })?;
        if storage_profile.as_str() != SUPPORTED_STORAGE_PROFILE {
            return Err(ServiceMetadataProfileError::UnsupportedCoordinate {
                profile: profile.id.clone(),
                coordinate: "storage_profile",
                value: storage_profile.to_string(),
            });
        }
        let (key, expected, layout) = match family {
            ServiceFamily::ScheduledJob => (
                SCHEDULED_JOB_LAYOUT_KEY,
                SCHEDULED_JOB_LAYOUT,
                ServiceLayout::ScheduledJobV1,
            ),
            ServiceFamily::EventSubscription => (
                EVENT_SUBSCRIPTION_LAYOUT_KEY,
                EVENT_SUBSCRIPTION_LAYOUT,
                ServiceLayout::EventSubscriptionV1,
            ),
            ServiceFamily::XdtoPackage => (
                XDTO_PACKAGE_LAYOUT_KEY,
                XDTO_PACKAGE_LAYOUT,
                ServiceLayout::XdtoPackageV1,
            ),
            ServiceFamily::HttpService => (
                HTTP_SERVICE_LAYOUT_KEY,
                HTTP_SERVICE_LAYOUT,
                ServiceLayout::HttpServiceV1,
            ),
            ServiceFamily::WebService => (
                WEB_SERVICE_LAYOUT_KEY,
                WEB_SERVICE_LAYOUT,
                ServiceLayout::WebServiceV1,
            ),
            ServiceFamily::WsReference => (
                WS_REFERENCE_LAYOUT_KEY,
                WS_REFERENCE_LAYOUT,
                ServiceLayout::WsReferenceV1,
            ),
            ServiceFamily::IntegrationService => (
                INTEGRATION_SERVICE_LAYOUT_KEY,
                INTEGRATION_SERVICE_LAYOUT,
                ServiceLayout::IntegrationServiceV1,
            ),
        };
        let value = profile.constants.get(key).ok_or_else(|| {
            ServiceMetadataProfileError::MissingConstant {
                profile: profile.id.clone(),
                key,
            }
        })?;
        if value.value != expected {
            return Err(ServiceMetadataProfileError::UnsupportedLayout {
                profile: profile.id.clone(),
                family,
                key,
                value: value.value.clone(),
            });
        }
        Ok(Self {
            profile_id: profile.id.clone(),
            platform_build,
            storage_profile,
            family,
            layout,
        })
    }

    pub const fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    pub const fn family(&self) -> ServiceFamily {
        self.family
    }

    #[cfg(test)]
    fn scheduled_job_fixture(profile_id: &str) -> Self {
        Self {
            profile_id: ProfileId::parse(profile_id).unwrap(),
            platform_build: PlatformBuild::parse("8.3.27.1989").unwrap(),
            storage_profile: StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            family: ServiceFamily::ScheduledJob,
            layout: ServiceLayout::ScheduledJobV1,
        }
    }

    #[cfg(test)]
    fn event_subscription_fixture(profile_id: &str) -> Self {
        Self {
            profile_id: ProfileId::parse(profile_id).unwrap(),
            platform_build: PlatformBuild::parse("8.3.27.1989").unwrap(),
            storage_profile: StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            family: ServiceFamily::EventSubscription,
            layout: ServiceLayout::EventSubscriptionV1,
        }
    }

    #[cfg(test)]
    fn xdto_package_fixture(profile_id: &str) -> Self {
        Self {
            profile_id: ProfileId::parse(profile_id).unwrap(),
            platform_build: PlatformBuild::parse("8.3.27.1989").unwrap(),
            storage_profile: StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            family: ServiceFamily::XdtoPackage,
            layout: ServiceLayout::XdtoPackageV1,
        }
    }

    #[cfg(test)]
    fn http_service_fixture(profile_id: &str) -> Self {
        Self {
            profile_id: ProfileId::parse(profile_id).unwrap(),
            platform_build: PlatformBuild::parse("8.3.27.1989").unwrap(),
            storage_profile: StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            family: ServiceFamily::HttpService,
            layout: ServiceLayout::HttpServiceV1,
        }
    }

    #[cfg(test)]
    fn web_service_fixture(profile_id: &str) -> Self {
        Self {
            profile_id: ProfileId::parse(profile_id).unwrap(),
            platform_build: PlatformBuild::parse("8.3.27.1989").unwrap(),
            storage_profile: StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            family: ServiceFamily::WebService,
            layout: ServiceLayout::WebServiceV1,
        }
    }

    #[cfg(test)]
    fn ws_reference_fixture(profile_id: &str) -> Self {
        Self {
            profile_id: ProfileId::parse(profile_id).unwrap(),
            platform_build: PlatformBuild::parse("8.3.27.1989").unwrap(),
            storage_profile: StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            family: ServiceFamily::WsReference,
            layout: ServiceLayout::WsReferenceV1,
        }
    }

    #[cfg(test)]
    fn integration_service_fixture(profile_id: &str) -> Self {
        Self {
            profile_id: ProfileId::parse(profile_id).unwrap(),
            platform_build: PlatformBuild::parse("8.3.27.1989").unwrap(),
            storage_profile: StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            family: ServiceFamily::IntegrationService,
            layout: ServiceLayout::IntegrationServiceV1,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceMetadataProfileError {
    MissingCoordinate {
        profile: ProfileId,
        coordinate: &'static str,
    },
    MissingConstant {
        profile: ProfileId,
        key: &'static str,
    },
    UnsupportedCoordinate {
        profile: ProfileId,
        coordinate: &'static str,
        value: String,
    },
    UnsupportedLayout {
        profile: ProfileId,
        family: ServiceFamily,
        key: &'static str,
        value: String,
    },
}

impl Display for ServiceMetadataProfileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCoordinate {
                profile,
                coordinate,
            } => write!(
                formatter,
                "profile `{profile}` has no `{coordinate}` coordinate"
            ),
            Self::MissingConstant { profile, key } => {
                write!(
                    formatter,
                    "profile `{profile}` has no required `{key}` constant"
                )
            }
            Self::UnsupportedCoordinate {
                profile,
                coordinate,
                value,
            } => write!(
                formatter,
                "profile `{profile}` declares unsupported `{coordinate}` value `{value}`"
            ),
            Self::UnsupportedLayout {
                profile,
                family,
                key,
                value,
            } => write!(
                formatter,
                "profile `{profile}` declares unsupported {} layout `{key}={value}`",
                family.as_str()
            ),
        }
    }
}

impl Error for ServiceMetadataProfileError {}

/// One native localized string in storage order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceLocalizedString {
    pub language: String,
    pub content: String,
}

/// Complete base-free native IR for a `ScheduledJob` row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledJobNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<ServiceLocalizedString>,
    pub comment: String,
    pub description: String,
    pub key: String,
    pub use_job: bool,
    pub predefined: bool,
    pub module_uuid: ObjectUuid,
    pub method_name: String,
    pub restart_count_on_failure: u32,
    pub restart_interval_on_failure: u32,
}

impl ScheduledJobNativeIr {
    /// Renders standalone XCF using an exact `CommonModule.<name>` mapping.
    pub fn to_xml(
        &self,
        profile: &ProfileId,
        modules: &std::collections::BTreeMap<ObjectUuid, String>,
    ) -> Result<Vec<u8>, ServiceMetadataBuildError> {
        let version = xml_profile_version(profile)
            .ok_or_else(|| ServiceMetadataBuildError::InvalidXmlProfile(profile.clone()))?;
        let module = modules.get(&self.module_uuid).ok_or(
            ServiceMetadataBuildError::MissingReadableReference(self.module_uuid),
        )?;
        if !valid_common_module_reference(module) || !valid_identifier_segment(&self.method_name) {
            return Err(native(
                "ScheduledJob module or method readable name is not exact",
            ));
        }
        let method_name = format!("{module}.{}", self.method_name);
        let mut xml = String::new();
        xml.push('\u{feff}');
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
        write!(
            &mut xml,
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"{version}\">\r\n\t<ScheduledJob uuid=\"{}\">\r\n\t\t<Properties>\r\n",
            self.uuid
        )
        .expect("writing to String cannot fail");
        write_xml_text_element(&mut xml, "\t\t\t", "Name", &self.name);
        write_synonyms(&mut xml, &self.synonyms);
        write_xml_text_element(&mut xml, "\t\t\t", "Comment", &self.comment);
        write_xml_text_element(&mut xml, "\t\t\t", "MethodName", &method_name);
        write_xml_text_element(&mut xml, "\t\t\t", "Description", &self.description);
        write_xml_text_element(&mut xml, "\t\t\t", "Key", &self.key);
        write_xml_text_element(
            &mut xml,
            "\t\t\t",
            "Use",
            if self.use_job { "true" } else { "false" },
        );
        write_xml_text_element(
            &mut xml,
            "\t\t\t",
            "Predefined",
            if self.predefined { "true" } else { "false" },
        );
        write_xml_text_element(
            &mut xml,
            "\t\t\t",
            "RestartCountOnFailure",
            &self.restart_count_on_failure.to_string(),
        );
        write_xml_text_element(
            &mut xml,
            "\t\t\t",
            "RestartIntervalOnFailure",
            &self.restart_interval_on_failure.to_string(),
        );
        xml.push_str("\t\t</Properties>\r\n\t</ScheduledJob>\r\n</MetaDataObject>");
        Ok(xml.into_bytes())
    }
}

/// Readable XCF spelling for one native EventSubscription TypeId.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventSourceReference {
    pub reference: String,
    pub type_set: bool,
}

/// Complete base-free native IR for an `EventSubscription` row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventSubscriptionNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<ServiceLocalizedString>,
    pub comment: String,
    pub source_type_ids: Vec<ObjectUuid>,
    pub event: String,
    pub module_uuid: ObjectUuid,
    pub method_name: String,
}

impl EventSubscriptionNativeIr {
    /// Renders standalone XCF from exact TypeId and CommonModule mappings.
    pub fn to_xml(
        &self,
        profile: &ProfileId,
        sources: &std::collections::BTreeMap<ObjectUuid, EventSourceReference>,
        modules: &std::collections::BTreeMap<ObjectUuid, String>,
    ) -> Result<Vec<u8>, ServiceMetadataBuildError> {
        let version = xml_profile_version(profile)
            .ok_or_else(|| ServiceMetadataBuildError::InvalidXmlProfile(profile.clone()))?;
        let native_event = native_event_name(&self.event)
            .ok_or_else(|| native("EventSubscription Event has no evidenced mapping"))?;
        if event_from_native(native_event) != Some(self.event.as_str()) {
            return Err(native("EventSubscription Event mapping is not reversible"));
        }
        let module = modules.get(&self.module_uuid).ok_or(
            ServiceMetadataBuildError::MissingReadableReference(self.module_uuid),
        )?;
        if !valid_common_module_reference(module) || !valid_identifier_segment(&self.method_name) {
            return Err(native(
                "EventSubscription module or method readable name is not exact",
            ));
        }
        if self.source_type_ids.is_empty() {
            return Err(native("EventSubscription Source type pattern is empty"));
        }
        let mut unique = BTreeSet::new();
        let mut readable_sources = Vec::with_capacity(self.source_type_ids.len());
        for type_id in &self.source_type_ids {
            if !unique.insert(*type_id) {
                return Err(native("EventSubscription Source TypeId is duplicated"));
            }
            let source =
                sources
                    .get(type_id)
                    .ok_or(ServiceMetadataBuildError::MissingReadableReference(
                        *type_id,
                    ))?;
            if !valid_cfg_reference(&source.reference) {
                return Err(native(
                    "EventSubscription Source reference is not an exact cfg:* name",
                ));
            }
            readable_sources.push(source);
        }

        let mut xml = String::new();
        xml.push('\u{feff}');
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
        write!(
            &mut xml,
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"{version}\">\r\n\t<EventSubscription uuid=\"{}\">\r\n\t\t<Properties>\r\n",
            self.uuid
        )
        .expect("writing to String cannot fail");
        write_xml_text_element(&mut xml, "\t\t\t", "Name", &self.name);
        write_synonyms(&mut xml, &self.synonyms);
        write_xml_text_element(&mut xml, "\t\t\t", "Comment", &self.comment);
        xml.push_str("\t\t\t<Source>\r\n");
        for source in readable_sources {
            write_xml_text_element(
                &mut xml,
                "\t\t\t\t",
                if source.type_set {
                    "v8:TypeSet"
                } else {
                    "v8:Type"
                },
                &source.reference,
            );
        }
        xml.push_str("\t\t\t</Source>\r\n");
        write_xml_text_element(&mut xml, "\t\t\t", "Event", &self.event);
        write_xml_text_element(
            &mut xml,
            "\t\t\t",
            "Handler",
            &format!("{module}.{}", self.method_name),
        );
        xml.push_str("\t\t</Properties>\r\n\t</EventSubscription>\r\n</MetaDataObject>");
        Ok(xml.into_bytes())
    }
}

/// Complete base-free native IR for an `XDTOPackage` metadata row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XdtoPackageNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<ServiceLocalizedString>,
    pub comment: String,
    pub namespace: String,
}

impl XdtoPackageNativeIr {
    /// Renders the metadata XML; `Ext/Package.bin` remains a separate raw asset.
    pub fn to_xml(&self, profile: &ProfileId) -> Result<Vec<u8>, ServiceMetadataBuildError> {
        let version = xml_profile_version(profile)
            .ok_or_else(|| ServiceMetadataBuildError::InvalidXmlProfile(profile.clone()))?;
        if !valid_xdto_namespace(&self.namespace) {
            return Err(native("XDTOPackage Namespace is not exact"));
        }
        let mut xml = String::new();
        xml.push('\u{feff}');
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
        write!(
            &mut xml,
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"{version}\">\r\n\t<XDTOPackage uuid=\"{}\">\r\n\t\t<Properties>\r\n",
            self.uuid
        )
        .expect("writing to String cannot fail");
        write_xml_text_element(&mut xml, "\t\t\t", "Name", &self.name);
        write_synonyms(&mut xml, &self.synonyms);
        write_xml_text_element(&mut xml, "\t\t\t", "Comment", &self.comment);
        write_xml_text_element(&mut xml, "\t\t\t", "Namespace", &self.namespace);
        xml.push_str("\t\t</Properties>\r\n\t</XDTOPackage>\r\n</MetaDataObject>");
        Ok(xml.into_bytes())
    }
}

/// Complete base-free native IR for a `WSReference` metadata row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WsReferenceNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<ServiceLocalizedString>,
    pub comment: String,
    pub location_url: String,
    pub manager_type_id: ObjectUuid,
    pub manager_value_id: ObjectUuid,
}

impl WsReferenceNativeIr {
    /// Renders standalone XCF; `Ext/WSDefinition.xml` remains a separate asset.
    pub fn to_xml(&self, profile: &ProfileId) -> Result<Vec<u8>, ServiceMetadataBuildError> {
        let version = xml_profile_version(profile)
            .ok_or_else(|| ServiceMetadataBuildError::InvalidXmlProfile(profile.clone()))?;
        validate_ws_reference_ir(self)?;
        let mut xml = String::new();
        xml.push('\u{feff}');
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
        write!(
            &mut xml,
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" version=\"{version}\">\r\n\t<WSReference uuid=\"{}\">\r\n\t\t<InternalInfo>\r\n\t\t\t<xr:GeneratedType name=\"WSReferenceManager.",
            self.uuid
        )
        .expect("writing to String cannot fail");
        push_xml_attribute(&mut xml, &self.name);
        xml.push_str("\" category=\"Manager\">\r\n");
        write_xml_text_element(
            &mut xml,
            "\t\t\t\t",
            "xr:TypeId",
            &self.manager_type_id.to_string(),
        );
        write_xml_text_element(
            &mut xml,
            "\t\t\t\t",
            "xr:ValueId",
            &self.manager_value_id.to_string(),
        );
        xml.push_str("\t\t\t</xr:GeneratedType>\r\n\t\t</InternalInfo>\r\n\t\t<Properties>\r\n");
        write_xml_text_element(&mut xml, "\t\t\t", "Name", &self.name);
        write_synonyms_at(&mut xml, "\t\t\t", &self.synonyms);
        write_xml_text_element(&mut xml, "\t\t\t", "Comment", &self.comment);
        write_xml_text_element(&mut xml, "\t\t\t", "LocationURL", &self.location_url);
        xml.push_str("\t\t</Properties>\r\n\t</WSReference>\r\n</MetaDataObject>");
        Ok(xml.into_bytes())
    }
}

/// One channel nested under an `IntegrationService`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegrationServiceChannelNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<ServiceLocalizedString>,
    pub comment: String,
    pub manager_type_id: ObjectUuid,
    pub manager_value_id: ObjectUuid,
    pub external_name: String,
    pub receive_message_processing: String,
    pub message_direction: String,
    pub transactioned: bool,
}

/// Complete base-free native IR for an `IntegrationService` primary row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegrationServiceNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<ServiceLocalizedString>,
    pub comment: String,
    pub manager_type_id: ObjectUuid,
    pub manager_value_id: ObjectUuid,
    pub external_address: String,
    pub channels: Vec<IntegrationServiceChannelNativeIr>,
}

impl IntegrationServiceNativeIr {
    /// Renders standalone XCF for the complete channel tree.
    pub fn to_xml(&self, profile: &ProfileId) -> Result<Vec<u8>, ServiceMetadataBuildError> {
        let version = xml_profile_version(profile)
            .ok_or_else(|| ServiceMetadataBuildError::InvalidXmlProfile(profile.clone()))?;
        validate_integration_service_ir(self)?;
        let mut xml = String::new();
        xml.push('\u{feff}');
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
        write!(
            &mut xml,
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" version=\"{version}\">\r\n\t<IntegrationService uuid=\"{}\">\r\n\t\t<InternalInfo>\r\n\t\t\t<xr:GeneratedType name=\"IntegrationServiceManager.",
            self.uuid
        )
        .expect("writing to String cannot fail");
        push_xml_attribute(&mut xml, &self.name);
        xml.push_str("\" category=\"Manager\">\r\n");
        write_xml_text_element(
            &mut xml,
            "\t\t\t\t",
            "xr:TypeId",
            &self.manager_type_id.to_string(),
        );
        write_xml_text_element(
            &mut xml,
            "\t\t\t\t",
            "xr:ValueId",
            &self.manager_value_id.to_string(),
        );
        xml.push_str("\t\t\t</xr:GeneratedType>\r\n\t\t</InternalInfo>\r\n\t\t<Properties>\r\n");
        write_xml_text_element(&mut xml, "\t\t\t", "Name", &self.name);
        write_synonyms_at(&mut xml, "\t\t\t", &self.synonyms);
        write_xml_text_element(&mut xml, "\t\t\t", "Comment", &self.comment);
        write_xml_text_element(
            &mut xml,
            "\t\t\t",
            "ExternalIntegrationServiceAddress",
            &self.external_address,
        );
        xml.push_str("\t\t</Properties>\r\n\t\t<ChildObjects>\r\n");
        for channel in &self.channels {
            write!(
                &mut xml,
                "\t\t\t<IntegrationServiceChannel uuid=\"{}\">\r\n\t\t\t\t<InternalInfo>\r\n\t\t\t\t\t<xr:GeneratedType name=\"IntegrationServiceChannelManager.",
                channel.uuid
            )
            .expect("writing to String cannot fail");
            push_xml_attribute(&mut xml, &self.name);
            xml.push('.');
            push_xml_attribute(&mut xml, &channel.name);
            xml.push_str("\" category=\"Manager\">\r\n");
            write_xml_text_element(
                &mut xml,
                "\t\t\t\t\t\t",
                "xr:TypeId",
                &channel.manager_type_id.to_string(),
            );
            write_xml_text_element(
                &mut xml,
                "\t\t\t\t\t\t",
                "xr:ValueId",
                &channel.manager_value_id.to_string(),
            );
            xml.push_str("\t\t\t\t\t</xr:GeneratedType>\r\n\t\t\t\t</InternalInfo>\r\n\t\t\t\t<Properties>\r\n");
            write_xml_text_element(&mut xml, "\t\t\t\t\t", "Name", &channel.name);
            write_synonyms_at(&mut xml, "\t\t\t\t\t", &channel.synonyms);
            write_xml_text_element(&mut xml, "\t\t\t\t\t", "Comment", &channel.comment);
            write_xml_text_element(
                &mut xml,
                "\t\t\t\t\t",
                "ExternalIntegrationServiceChannelName",
                &channel.external_name,
            );
            write_xml_text_element(
                &mut xml,
                "\t\t\t\t\t",
                "MessageDirection",
                &channel.message_direction,
            );
            write_xml_text_element(
                &mut xml,
                "\t\t\t\t\t",
                "ReceiveMessageProcessing",
                &channel.receive_message_processing,
            );
            write_xml_text_element(
                &mut xml,
                "\t\t\t\t\t",
                "Transactioned",
                if channel.transactioned {
                    "true"
                } else {
                    "false"
                },
            );
            xml.push_str("\t\t\t\t</Properties>\r\n\t\t\t</IntegrationServiceChannel>\r\n");
        }
        xml.push_str("\t\t</ChildObjects>\r\n\t</IntegrationService>\r\n</MetaDataObject>");
        Ok(xml.into_bytes())
    }
}

/// One HTTP method nested under a URL template.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpMethodNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<ServiceLocalizedString>,
    pub comment: String,
    pub http_method: String,
    pub handler: String,
}

/// One URL template and its methods in native storage order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpUrlTemplateNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<ServiceLocalizedString>,
    pub comment: String,
    pub template: String,
    pub methods: Vec<HttpMethodNativeIr>,
}

/// Complete base-free native IR for an `HTTPService` primary row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpServiceNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<ServiceLocalizedString>,
    pub comment: String,
    pub root_url: String,
    pub reuse_sessions: String,
    pub session_max_age: u32,
    pub urls: Vec<HttpUrlTemplateNativeIr>,
}

impl HttpServiceNativeIr {
    /// Renders standalone XCF for the complete URLTemplate/Method tree.
    pub fn to_xml(&self, profile: &ProfileId) -> Result<Vec<u8>, ServiceMetadataBuildError> {
        let version = xml_profile_version(profile)
            .ok_or_else(|| ServiceMetadataBuildError::InvalidXmlProfile(profile.clone()))?;
        validate_http_ir(self)?;
        let mut xml = String::new();
        xml.push('\u{feff}');
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
        write!(
            &mut xml,
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"{version}\">\r\n\t<HTTPService uuid=\"{}\">\r\n\t\t<Properties>\r\n",
            self.uuid
        )
        .expect("writing to String cannot fail");
        write_xml_text_element(&mut xml, "\t\t\t", "Name", &self.name);
        write_synonyms_at(&mut xml, "\t\t\t", &self.synonyms);
        write_xml_text_element(&mut xml, "\t\t\t", "Comment", &self.comment);
        write_xml_text_element(&mut xml, "\t\t\t", "RootURL", &self.root_url);
        write_xml_text_element(&mut xml, "\t\t\t", "ReuseSessions", &self.reuse_sessions);
        write_xml_text_element(
            &mut xml,
            "\t\t\t",
            "SessionMaxAge",
            &self.session_max_age.to_string(),
        );
        xml.push_str("\t\t</Properties>\r\n\t\t<ChildObjects>\r\n");
        for url in &self.urls {
            write!(
                &mut xml,
                "\t\t\t<URLTemplate uuid=\"{}\">\r\n\t\t\t\t<Properties>\r\n",
                url.uuid
            )
            .expect("writing to String cannot fail");
            write_xml_text_element(&mut xml, "\t\t\t\t\t", "Name", &url.name);
            write_synonyms_at(&mut xml, "\t\t\t\t\t", &url.synonyms);
            write_xml_text_element(&mut xml, "\t\t\t\t\t", "Comment", &url.comment);
            write_xml_text_element(&mut xml, "\t\t\t\t\t", "Template", &url.template);
            xml.push_str("\t\t\t\t</Properties>\r\n\t\t\t\t<ChildObjects>\r\n");
            for method in &url.methods {
                write!(
                    &mut xml,
                    "\t\t\t\t\t<Method uuid=\"{}\">\r\n\t\t\t\t\t\t<Properties>\r\n",
                    method.uuid
                )
                .expect("writing to String cannot fail");
                write_xml_text_element(&mut xml, "\t\t\t\t\t\t\t", "Name", &method.name);
                write_synonyms_at(&mut xml, "\t\t\t\t\t\t\t", &method.synonyms);
                write_xml_text_element(&mut xml, "\t\t\t\t\t\t\t", "Comment", &method.comment);
                write_xml_text_element(
                    &mut xml,
                    "\t\t\t\t\t\t\t",
                    "HTTPMethod",
                    &method.http_method,
                );
                write_xml_text_element(&mut xml, "\t\t\t\t\t\t\t", "Handler", &method.handler);
                xml.push_str("\t\t\t\t\t\t</Properties>\r\n\t\t\t\t\t</Method>\r\n");
            }
            xml.push_str("\t\t\t\t</ChildObjects>\r\n\t\t\t</URLTemplate>\r\n");
        }
        xml.push_str("\t\t</ChildObjects>\r\n\t</HTTPService>\r\n</MetaDataObject>");
        Ok(xml.into_bytes())
    }
}

/// One native XDTO qualified type used by a Web service operation or parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebServiceXdtoTypeNativeIr {
    pub namespace: String,
    pub name: String,
}

/// One Web service operation parameter in native storage order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebServiceParameterNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<ServiceLocalizedString>,
    pub comment: String,
    pub value_type: WebServiceXdtoTypeNativeIr,
    pub nillable: bool,
    pub transfer_direction: String,
}

/// One Web service operation and its parameter collection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebServiceOperationNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<ServiceLocalizedString>,
    pub comment: String,
    pub returning_value_type: WebServiceXdtoTypeNativeIr,
    pub nillable: bool,
    pub transactioned: bool,
    pub procedure_name: String,
    pub data_lock_control_mode: String,
    pub parameters: Vec<WebServiceParameterNativeIr>,
}

/// Complete base-free native IR for a `WebService` primary row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebServiceNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<ServiceLocalizedString>,
    pub comment: String,
    pub namespace: String,
    pub xdto_package_uuids: Vec<ObjectUuid>,
    pub xdto_namespaces: Vec<String>,
    pub descriptor_file_name: String,
    pub reuse_sessions: String,
    pub session_max_age: u32,
    pub operations: Vec<WebServiceOperationNativeIr>,
}

impl WebServiceNativeIr {
    /// Renders standalone XCF using exact `XDTOPackage.<name>` mappings.
    pub fn to_xml(
        &self,
        profile: &ProfileId,
        packages: &BTreeMap<ObjectUuid, String>,
    ) -> Result<Vec<u8>, ServiceMetadataBuildError> {
        let version = xml_profile_version(profile)
            .ok_or_else(|| ServiceMetadataBuildError::InvalidXmlProfile(profile.clone()))?;
        validate_web_service_ir(self)?;

        let mut readable_packages = Vec::with_capacity(self.xdto_package_uuids.len());
        let mut seen_readable_packages = BTreeSet::new();
        for uuid in &self.xdto_package_uuids {
            let readable = packages
                .get(uuid)
                .ok_or(ServiceMetadataBuildError::MissingReadableReference(*uuid))?;
            if !valid_xdto_package_reference(readable)
                || !seen_readable_packages.insert(readable.as_str())
            {
                return Err(native("WebService XDTOPackage readable name is not exact"));
            }
            readable_packages.push(readable.as_str());
        }

        let mut custom_namespaces = BTreeSet::new();
        for operation in &self.operations {
            collect_custom_xdto_namespace(
                &operation.returning_value_type.namespace,
                &mut custom_namespaces,
            );
            for parameter in &operation.parameters {
                collect_custom_xdto_namespace(
                    &parameter.value_type.namespace,
                    &mut custom_namespaces,
                );
            }
        }
        let prefixes: BTreeMap<&str, String> = custom_namespaces
            .iter()
            .enumerate()
            .map(|(index, namespace)| (namespace.as_str(), format!("ws{}", index + 1)))
            .collect();

        let mut xml = String::new();
        xml.push('\u{feff}');
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
        write!(
            &mut xml,
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"{XDTO_CORE_NAMESPACE}\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"{XDTO_XML_SCHEMA_NAMESPACE}\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\""
        )
        .expect("writing to String cannot fail");
        for (namespace, prefix) in &prefixes {
            write!(&mut xml, " xmlns:{prefix}=\"").expect("writing to String cannot fail");
            push_xml_attribute(&mut xml, namespace);
            xml.push('"');
        }
        write!(
            &mut xml,
            " version=\"{version}\">\r\n\t<WebService uuid=\"{}\">\r\n\t\t<Properties>\r\n",
            self.uuid
        )
        .expect("writing to String cannot fail");
        write_xml_text_element(&mut xml, "\t\t\t", "Name", &self.name);
        write_synonyms_at(&mut xml, "\t\t\t", &self.synonyms);
        write_xml_text_element(&mut xml, "\t\t\t", "Comment", &self.comment);
        write_xml_text_element(&mut xml, "\t\t\t", "Namespace", &self.namespace);
        if readable_packages.is_empty() && self.xdto_namespaces.is_empty() {
            xml.push_str("\t\t\t<XDTOPackages/>\r\n");
        } else {
            xml.push_str("\t\t\t<XDTOPackages>\r\n");
            for readable in readable_packages {
                write_web_package_item(&mut xml, "xr:MDObjectRef", readable);
            }
            for namespace in &self.xdto_namespaces {
                write_web_package_item(&mut xml, "xs:string", namespace);
            }
            xml.push_str("\t\t\t</XDTOPackages>\r\n");
        }
        write_xml_text_element(
            &mut xml,
            "\t\t\t",
            "DescriptorFileName",
            &self.descriptor_file_name,
        );
        write_xml_text_element(&mut xml, "\t\t\t", "ReuseSessions", &self.reuse_sessions);
        write_xml_text_element(
            &mut xml,
            "\t\t\t",
            "SessionMaxAge",
            &self.session_max_age.to_string(),
        );
        xml.push_str("\t\t</Properties>\r\n\t\t<ChildObjects>\r\n");
        for operation in &self.operations {
            write!(
                &mut xml,
                "\t\t\t<Operation uuid=\"{}\">\r\n\t\t\t\t<Properties>\r\n",
                operation.uuid
            )
            .expect("writing to String cannot fail");
            write_xml_text_element(&mut xml, "\t\t\t\t\t", "Name", &operation.name);
            write_synonyms_at(&mut xml, "\t\t\t\t\t", &operation.synonyms);
            write_xml_text_element(&mut xml, "\t\t\t\t\t", "Comment", &operation.comment);
            write_xdto_type_element(
                &mut xml,
                "\t\t\t\t\t",
                "XDTOReturningValueType",
                &operation.returning_value_type,
                &prefixes,
            );
            write_xml_text_element(
                &mut xml,
                "\t\t\t\t\t",
                "Nillable",
                if operation.nillable { "true" } else { "false" },
            );
            write_xml_text_element(
                &mut xml,
                "\t\t\t\t\t",
                "Transactioned",
                if operation.transactioned {
                    "true"
                } else {
                    "false"
                },
            );
            write_xml_text_element(
                &mut xml,
                "\t\t\t\t\t",
                "ProcedureName",
                &operation.procedure_name,
            );
            write_xml_text_element(
                &mut xml,
                "\t\t\t\t\t",
                "DataLockControlMode",
                &operation.data_lock_control_mode,
            );
            xml.push_str("\t\t\t\t</Properties>\r\n");
            if operation.parameters.is_empty() {
                xml.push_str("\t\t\t\t<ChildObjects/>\r\n");
            } else {
                xml.push_str("\t\t\t\t<ChildObjects>\r\n");
                for parameter in &operation.parameters {
                    write!(
                        &mut xml,
                        "\t\t\t\t\t<Parameter uuid=\"{}\">\r\n\t\t\t\t\t\t<Properties>\r\n",
                        parameter.uuid
                    )
                    .expect("writing to String cannot fail");
                    write_xml_text_element(&mut xml, "\t\t\t\t\t\t\t", "Name", &parameter.name);
                    write_synonyms_at(&mut xml, "\t\t\t\t\t\t\t", &parameter.synonyms);
                    write_xml_text_element(
                        &mut xml,
                        "\t\t\t\t\t\t\t",
                        "Comment",
                        &parameter.comment,
                    );
                    write_xdto_type_element(
                        &mut xml,
                        "\t\t\t\t\t\t\t",
                        "XDTOValueType",
                        &parameter.value_type,
                        &prefixes,
                    );
                    write_xml_text_element(
                        &mut xml,
                        "\t\t\t\t\t\t\t",
                        "Nillable",
                        if parameter.nillable { "true" } else { "false" },
                    );
                    write_xml_text_element(
                        &mut xml,
                        "\t\t\t\t\t\t\t",
                        "TransferDirection",
                        &parameter.transfer_direction,
                    );
                    xml.push_str("\t\t\t\t\t\t</Properties>\r\n\t\t\t\t\t</Parameter>\r\n");
                }
                xml.push_str("\t\t\t\t</ChildObjects>\r\n");
            }
            xml.push_str("\t\t\t</Operation>\r\n");
        }
        xml.push_str("\t\t</ChildObjects>\r\n\t</WebService>\r\n</MetaDataObject>");
        Ok(xml.into_bytes())
    }
}

#[derive(Debug)]
pub enum ServiceMetadataBuildError {
    Profile(ServiceMetadataProfileError),
    ProfileMismatch {
        graph: ProfileId,
        service: ProfileId,
    },
    AxisMismatch {
        axis: &'static str,
        expected: String,
        actual: String,
    },
    UnknownObject(ObjectUuid),
    MissingPrimaryRoute(ObjectUuid),
    InvalidModel {
        object: ObjectUuid,
        reason: &'static str,
    },
    UnsupportedFamily(ServiceFamily),
    InvalidXmlProfile(ProfileId),
    MissingReadableReference(ObjectUuid),
    Native(String),
    PlainPayloadTooLarge {
        maximum: usize,
        actual: usize,
    },
    Deflate(io::Error),
    Inflate(io::Error),
    Storage(StorageBuildError),
    Patch(StoragePatchBuildError),
}

impl Display for ServiceMetadataBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(source) => Display::fmt(source, formatter),
            Self::ProfileMismatch { graph, service } => write!(
                formatter,
                "bootstrap graph profile `{graph}` does not match service profile `{service}`"
            ),
            Self::AxisMismatch {
                axis,
                expected,
                actual,
            } => write!(
                formatter,
                "service compiler axis `{axis}` expected `{expected}`, got `{actual}`"
            ),
            Self::UnknownObject(uuid) => write!(formatter, "unknown metadata object `{uuid}`"),
            Self::MissingPrimaryRoute(uuid) => {
                write!(
                    formatter,
                    "metadata object `{uuid}` has no primary storage route"
                )
            }
            Self::InvalidModel { object, reason } => {
                write!(formatter, "metadata object `{object}` is invalid: {reason}")
            }
            Self::UnsupportedFamily(family) => {
                write!(
                    formatter,
                    "{} native layout is unsupported",
                    family.as_str()
                )
            }
            Self::InvalidXmlProfile(profile) => {
                write!(formatter, "XML profile `{profile}` is not 2.20 or 2.21")
            }
            Self::MissingReadableReference(uuid) => {
                write!(
                    formatter,
                    "native UUID `{uuid}` has no readable source mapping"
                )
            }
            Self::Native(reason) => write!(formatter, "invalid native service row: {reason}"),
            Self::PlainPayloadTooLarge { maximum, actual } => write!(
                formatter,
                "service metadata plaintext is {actual} bytes, maximum is {maximum}"
            ),
            Self::Deflate(source) => write!(formatter, "raw DEFLATE encode failed: {source}"),
            Self::Inflate(source) => write!(formatter, "raw DEFLATE decode failed: {source}"),
            Self::Storage(source) => Display::fmt(source, formatter),
            Self::Patch(source) => Display::fmt(source, formatter),
        }
    }
}

impl Error for ServiceMetadataBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Profile(source) => Some(source),
            Self::Deflate(source) | Self::Inflate(source) => Some(source),
            Self::Storage(source) => Some(source),
            Self::Patch(source) => Some(source),
            _ => None,
        }
    }
}

impl From<ServiceMetadataProfileError> for ServiceMetadataBuildError {
    fn from(source: ServiceMetadataProfileError) -> Self {
        Self::Profile(source)
    }
}

impl From<StorageBuildError> for ServiceMetadataBuildError {
    fn from(source: StorageBuildError) -> Self {
        Self::Storage(source)
    }
}

impl From<StoragePatchBuildError> for ServiceMetadataBuildError {
    fn from(source: StoragePatchBuildError) -> Self {
        Self::Patch(source)
    }
}

/// Compiles one validated service object into its primary storage row.
pub fn compile_service_metadata(
    validated: &ValidatedConfiguration<'_>,
    graph: &BootstrapGraph,
    object_uuid: ObjectUuid,
    axes: &CompileAxes,
    profile: &ServiceMetadataProfile,
) -> Result<StoragePatchEntry, ServiceMetadataBuildError> {
    validate_coordinates(graph, axes, profile)?;
    let object_index = validated
        .graph()
        .object_index_by_uuid(object_uuid)
        .ok_or(ServiceMetadataBuildError::UnknownObject(object_uuid))?;
    let object = &validated.configuration().objects()[object_index];
    let family = ServiceFamily::from_kind(object.kind().as_str()).ok_or(
        ServiceMetadataBuildError::InvalidModel {
            object: object_uuid,
            reason: "metadata kind is outside BOOT-004",
        },
    )?;
    if family != profile.family {
        return Err(ServiceMetadataBuildError::AxisMismatch {
            axis: "family",
            expected: profile.family.as_str().to_owned(),
            actual: family.as_str().to_owned(),
        });
    }
    let expected_source_profile = format!("xml-{}", axes.xml_dialect());
    if object.provenance().source_profile().as_str() != expected_source_profile {
        return Err(ServiceMetadataBuildError::AxisMismatch {
            axis: "xml_dialect",
            expected: object.provenance().source_profile().to_string(),
            actual: axes.xml_dialect().to_string(),
        });
    }
    let route = graph
        .primary_object_entry(object_uuid)
        .ok_or(ServiceMetadataBuildError::MissingPrimaryRoute(object_uuid))?;
    let plaintext = match (family, profile.layout) {
        (ServiceFamily::ScheduledJob, ServiceLayout::ScheduledJobV1) => {
            serialize_scheduled_job(&project_scheduled_job(validated, object)?)
        }
        (ServiceFamily::EventSubscription, ServiceLayout::EventSubscriptionV1) => {
            serialize_event_subscription(&project_event_subscription(validated, object)?)
        }
        (ServiceFamily::XdtoPackage, ServiceLayout::XdtoPackageV1) => {
            serialize_xdto_package(&project_xdto_package(validated, object)?)
        }
        (ServiceFamily::HttpService, ServiceLayout::HttpServiceV1) => {
            serialize_http_service(&project_http_service(validated, object)?)
        }
        (ServiceFamily::WebService, ServiceLayout::WebServiceV1) => {
            serialize_web_service(&project_web_service(validated, object)?)
        }
        (ServiceFamily::WsReference, ServiceLayout::WsReferenceV1) => {
            serialize_ws_reference(&project_ws_reference(validated, object)?)
        }
        (ServiceFamily::IntegrationService, ServiceLayout::IntegrationServiceV1) => {
            serialize_integration_service(&project_integration_service(validated, object)?)
        }
        (family, _) => return Err(ServiceMetadataBuildError::UnsupportedFamily(family)),
    };
    let bytes = raw_deflate(&plaintext)?;
    let provenance = StorageProvenance::new(&format!(
        "bootstrap:{}:metadata:{}",
        profile.profile_id,
        family.as_str()
    ))?;
    Ok(StoragePatchEntry::new(
        StoragePatchTarget::new(route.key().clone(), MultipartIdentity::single(), provenance),
        StoragePatchOutcome::compiled(bytes)?,
    ))
}

/// Strictly decodes a raw-DEFLATE `ScheduledJob` row.
pub fn decode_scheduled_job_blob(
    blob: &[u8],
    profile: &ServiceMetadataProfile,
) -> Result<ScheduledJobNativeIr, ServiceMetadataBuildError> {
    if profile.family != ServiceFamily::ScheduledJob
        || profile.layout != ServiceLayout::ScheduledJobV1
    {
        return Err(ServiceMetadataBuildError::UnsupportedFamily(profile.family));
    }
    parse_scheduled_job(&inflate_bounded(blob)?)
}

/// Strictly decodes a raw-DEFLATE `EventSubscription` row.
pub fn decode_event_subscription_blob(
    blob: &[u8],
    profile: &ServiceMetadataProfile,
) -> Result<EventSubscriptionNativeIr, ServiceMetadataBuildError> {
    if profile.family != ServiceFamily::EventSubscription
        || profile.layout != ServiceLayout::EventSubscriptionV1
    {
        return Err(ServiceMetadataBuildError::UnsupportedFamily(profile.family));
    }
    parse_event_subscription(&inflate_bounded(blob)?)
}

/// Strictly decodes a raw-DEFLATE `XDTOPackage` metadata row.
pub fn decode_xdto_package_blob(
    blob: &[u8],
    profile: &ServiceMetadataProfile,
) -> Result<XdtoPackageNativeIr, ServiceMetadataBuildError> {
    if profile.family != ServiceFamily::XdtoPackage
        || profile.layout != ServiceLayout::XdtoPackageV1
    {
        return Err(ServiceMetadataBuildError::UnsupportedFamily(profile.family));
    }
    parse_xdto_package(&inflate_bounded(blob)?)
}

/// Strictly decodes a raw-DEFLATE `HTTPService` primary row.
pub fn decode_http_service_blob(
    blob: &[u8],
    profile: &ServiceMetadataProfile,
) -> Result<HttpServiceNativeIr, ServiceMetadataBuildError> {
    if profile.family != ServiceFamily::HttpService
        || profile.layout != ServiceLayout::HttpServiceV1
    {
        return Err(ServiceMetadataBuildError::UnsupportedFamily(profile.family));
    }
    parse_http_service(&inflate_bounded(blob)?)
}

/// Strictly decodes a raw-DEFLATE `WebService` primary row.
pub fn decode_web_service_blob(
    blob: &[u8],
    profile: &ServiceMetadataProfile,
) -> Result<WebServiceNativeIr, ServiceMetadataBuildError> {
    if profile.family != ServiceFamily::WebService || profile.layout != ServiceLayout::WebServiceV1
    {
        return Err(ServiceMetadataBuildError::UnsupportedFamily(profile.family));
    }
    parse_web_service(&inflate_bounded(blob)?)
}

/// Strictly decodes a raw-DEFLATE `WSReference` metadata row.
pub fn decode_ws_reference_blob(
    blob: &[u8],
    profile: &ServiceMetadataProfile,
) -> Result<WsReferenceNativeIr, ServiceMetadataBuildError> {
    if profile.family != ServiceFamily::WsReference
        || profile.layout != ServiceLayout::WsReferenceV1
    {
        return Err(ServiceMetadataBuildError::UnsupportedFamily(profile.family));
    }
    parse_ws_reference(&inflate_bounded(blob)?)
}

/// Strictly decodes a raw-DEFLATE `IntegrationService` primary row.
pub fn decode_integration_service_blob(
    blob: &[u8],
    profile: &ServiceMetadataProfile,
) -> Result<IntegrationServiceNativeIr, ServiceMetadataBuildError> {
    if profile.family != ServiceFamily::IntegrationService
        || profile.layout != ServiceLayout::IntegrationServiceV1
    {
        return Err(ServiceMetadataBuildError::UnsupportedFamily(profile.family));
    }
    parse_integration_service(&inflate_bounded(blob)?)
}

fn validate_coordinates(
    graph: &BootstrapGraph,
    axes: &CompileAxes,
    profile: &ServiceMetadataProfile,
) -> Result<(), ServiceMetadataBuildError> {
    if graph.profile_id() != &profile.profile_id {
        return Err(ServiceMetadataBuildError::ProfileMismatch {
            graph: graph.profile_id().clone(),
            service: profile.profile_id.clone(),
        });
    }
    let actual_platform = axes
        .platform_build()
        .map(ToString::to_string)
        .unwrap_or_else(|| "<missing>".to_owned());
    if axes.platform_build() != Some(&profile.platform_build) {
        return Err(ServiceMetadataBuildError::AxisMismatch {
            axis: "platform_build",
            expected: profile.platform_build.to_string(),
            actual: actual_platform,
        });
    }
    if axes.storage_profile() != &profile.storage_profile {
        return Err(ServiceMetadataBuildError::AxisMismatch {
            axis: "storage_profile",
            expected: profile.storage_profile.to_string(),
            actual: axes.storage_profile().to_string(),
        });
    }
    if axes.compatibility_mode().is_some() {
        return Err(ServiceMetadataBuildError::AxisMismatch {
            axis: "compatibility_mode",
            expected: "<unspecified for evidenced layout>".to_owned(),
            actual: axes.compatibility_mode().unwrap().to_string(),
        });
    }
    if axes.container_revision().is_some() {
        return Err(ServiceMetadataBuildError::AxisMismatch {
            axis: "container_revision",
            expected: "<unspecified for evidenced layout>".to_owned(),
            actual: axes.container_revision().unwrap().to_string(),
        });
    }
    if !matches!(axes.xml_dialect().to_string().as_str(), "2.20" | "2.21") {
        return Err(ServiceMetadataBuildError::AxisMismatch {
            axis: "xml_dialect",
            expected: "2.20 or 2.21".to_owned(),
            actual: axes.xml_dialect().to_string(),
        });
    }
    Ok(())
}

fn project_scheduled_job(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
) -> Result<ScheduledJobNativeIr, ServiceMetadataBuildError> {
    let uuid = object.identity().uuid();
    if object.owner().is_some()
        || !object.references().is_empty()
        || !object.generated_types().is_empty()
        || !object.assets().is_empty()
    {
        return invalid_model(
            uuid,
            "ScheduledJob must be top-level without references, generated types, or assets",
        );
    }
    if validated
        .configuration()
        .objects()
        .iter()
        .any(|candidate| candidate.owner() == Some(uuid))
    {
        return invalid_model(uuid, "ScheduledJob cannot own child objects");
    }
    let expected = [
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
    if object.properties().len() != expected.len()
        || object
            .properties()
            .iter()
            .zip(expected)
            .any(|(field, expected)| field.name().as_str() != expected)
    {
        return invalid_model(uuid, "typed property schema is not exact");
    }
    let name = text_property(object, "Name")?.to_owned();
    if name.is_empty() {
        return invalid_model(uuid, "Name must not be empty");
    }
    let method = text_property(object, "MethodName")?;
    let mut parts = method.split('.');
    if parts.next() != Some("CommonModule") {
        return invalid_model(uuid, "MethodName does not start with CommonModule");
    }
    let module_name = parts.next().unwrap_or_default();
    let method_name = parts.next().unwrap_or_default();
    if parts.next().is_some()
        || !valid_identifier_segment(module_name)
        || !valid_identifier_segment(method_name)
    {
        return invalid_model(uuid, "MethodName is not CommonModule.<name>.<method>");
    }
    let module_uuid = resolve_common_module(validated, uuid, module_name)?;
    Ok(ScheduledJobNativeIr {
        uuid,
        name,
        synonyms: synonym_property(object, "Synonym")?,
        comment: text_property(object, "Comment")?.to_owned(),
        description: text_property(object, "Description")?.to_owned(),
        key: text_property(object, "Key")?.to_owned(),
        use_job: bool_property(object, "Use")?,
        predefined: bool_property(object, "Predefined")?,
        module_uuid,
        method_name: method_name.to_owned(),
        restart_count_on_failure: u32_property(object, "RestartCountOnFailure")?,
        restart_interval_on_failure: u32_property(object, "RestartIntervalOnFailure")?,
    })
}

fn project_event_subscription(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
) -> Result<EventSubscriptionNativeIr, ServiceMetadataBuildError> {
    let uuid = object.identity().uuid();
    if object.owner().is_some()
        || !object.references().is_empty()
        || !object.generated_types().is_empty()
        || !object.assets().is_empty()
    {
        return invalid_model(
            uuid,
            "EventSubscription must be top-level without references, generated types, or assets",
        );
    }
    if validated
        .configuration()
        .objects()
        .iter()
        .any(|candidate| candidate.owner() == Some(uuid))
    {
        return invalid_model(uuid, "EventSubscription cannot own child objects");
    }
    let expected = ["Name", "Synonym", "Comment", "Source", "Event", "Handler"];
    if object.properties().len() != expected.len()
        || object
            .properties()
            .iter()
            .zip(expected)
            .any(|(field, expected)| field.name().as_str() != expected)
    {
        return invalid_model(uuid, "typed property schema is not exact");
    }
    let name = text_property(object, "Name")?.to_owned();
    if name.is_empty() {
        return invalid_model(uuid, "Name must not be empty");
    }
    let source_names = event_source_property(object)?;
    let generated_types = generated_type_reference_index(validated, uuid)?;
    let mut source_type_ids = Vec::with_capacity(source_names.len());
    let mut unique = BTreeSet::new();
    for (_, reference) in source_names {
        let type_id = generated_types.get(&reference).copied().ok_or(
            ServiceMetadataBuildError::InvalidModel {
                object: uuid,
                reason: "EventSubscription Source TypeId is unresolved",
            },
        )?;
        if !unique.insert(type_id) {
            return invalid_model(uuid, "EventSubscription Source TypeId is duplicated");
        }
        source_type_ids.push(type_id);
    }
    let event = text_property(object, "Event")?.to_owned();
    if native_event_name(&event).is_none() {
        return invalid_model(uuid, "Event has no evidenced native mapping");
    }
    let handler = text_property(object, "Handler")?;
    let mut parts = handler.split('.');
    if parts.next() != Some("CommonModule") {
        return invalid_model(uuid, "Handler does not start with CommonModule");
    }
    let module_name = parts.next().unwrap_or_default();
    let method_name = parts.next().unwrap_or_default();
    if parts.next().is_some()
        || !valid_identifier_segment(module_name)
        || !valid_identifier_segment(method_name)
    {
        return invalid_model(uuid, "Handler is not CommonModule.<name>.<method>");
    }
    Ok(EventSubscriptionNativeIr {
        uuid,
        name,
        synonyms: synonym_property(object, "Synonym")?,
        comment: text_property(object, "Comment")?.to_owned(),
        source_type_ids,
        event,
        module_uuid: resolve_common_module(validated, uuid, module_name)?,
        method_name: method_name.to_owned(),
    })
}

fn event_source_property(
    object: &CanonicalObject,
) -> Result<Vec<(bool, String)>, ServiceMetadataBuildError> {
    let uuid = object.identity().uuid();
    let values = property(object, "Source")?.as_sequence().ok_or(
        ServiceMetadataBuildError::InvalidModel {
            object: uuid,
            reason: "EventSubscription Source is not a sequence",
        },
    )?;
    if values.is_empty() {
        return invalid_model(uuid, "EventSubscription Source is empty");
    }
    let mut unique = BTreeSet::new();
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let fields = value
            .as_record()
            .ok_or(ServiceMetadataBuildError::InvalidModel {
                object: uuid,
                reason: "EventSubscription Source item is not a record",
            })?;
        if fields.len() != 2
            || fields[0].name().as_str() != "kind"
            || fields[1].name().as_str() != "reference"
        {
            return invalid_model(uuid, "EventSubscription Source item schema is not exact");
        }
        let type_set = match fields[0].value().kind() {
            CanonicalValueKind::EnumToken(value) if value.as_str() == "Type" => false,
            CanonicalValueKind::EnumToken(value) if value.as_str() == "TypeSet" => true,
            _ => return invalid_model(uuid, "EventSubscription Source kind is unsupported"),
        };
        let reference = canonical_text(fields[1].value(), uuid)?.to_owned();
        if !valid_cfg_reference(&reference) || !unique.insert((type_set, reference.clone())) {
            return invalid_model(
                uuid,
                "EventSubscription Source reference is invalid or duplicated",
            );
        }
        result.push((type_set, reference));
    }
    Ok(result)
}

fn generated_type_reference_index(
    validated: &ValidatedConfiguration<'_>,
    compiling: ObjectUuid,
) -> Result<BTreeMap<String, ObjectUuid>, ServiceMetadataBuildError> {
    let mut references = BTreeMap::new();
    for object in validated.configuration().objects() {
        if object.generated_types().is_empty() {
            continue;
        }
        let Some(name) = object
            .properties()
            .iter()
            .find(|field| field.name().as_str() == "Name")
            .and_then(|field| match field.value().kind() {
                CanonicalValueKind::Text(value) if valid_identifier_segment(value.as_str()) => {
                    Some(value.as_str())
                }
                _ => None,
            })
        else {
            continue;
        };
        for generated in object.generated_types() {
            let readable_kind = if object.kind().as_str() == "DefinedType"
                && generated.kind().as_str() == "DefinedType"
            {
                "DefinedType".to_owned()
            } else {
                format!("{}{}", object.kind().as_str(), generated.kind().as_str())
            };
            let readable = format!("cfg:{readable_kind}.{name}");
            if references.insert(readable, generated.uuid()).is_some() {
                return invalid_model(compiling, "readable generated type name is ambiguous");
            }
        }
    }
    Ok(references)
}

fn project_xdto_package(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
) -> Result<XdtoPackageNativeIr, ServiceMetadataBuildError> {
    let uuid = object.identity().uuid();
    if object.owner().is_some()
        || !object.references().is_empty()
        || !object.generated_types().is_empty()
        || !object.assets().is_empty()
    {
        return invalid_model(
            uuid,
            "XDTOPackage metadata must be top-level without inline references, generated types, or assets",
        );
    }
    if validated
        .configuration()
        .objects()
        .iter()
        .any(|candidate| candidate.owner() == Some(uuid))
    {
        return invalid_model(uuid, "XDTOPackage metadata cannot own child objects");
    }
    let expected = ["Name", "Synonym", "Comment", "Namespace"];
    if object.properties().len() != expected.len()
        || object
            .properties()
            .iter()
            .zip(expected)
            .any(|(field, expected)| field.name().as_str() != expected)
    {
        return invalid_model(uuid, "typed property schema is not exact");
    }
    let name = text_property(object, "Name")?.to_owned();
    if name.is_empty() {
        return invalid_model(uuid, "Name must not be empty");
    }
    let namespace = text_property(object, "Namespace")?.to_owned();
    if !valid_web_xdto_namespace(&namespace) {
        return invalid_model(uuid, "XDTOPackage Namespace is not exact");
    }
    Ok(XdtoPackageNativeIr {
        uuid,
        name,
        synonyms: synonym_property(object, "Synonym")?,
        comment: text_property(object, "Comment")?.to_owned(),
        namespace,
    })
}

fn project_ws_reference(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
) -> Result<WsReferenceNativeIr, ServiceMetadataBuildError> {
    let uuid = object.identity().uuid();
    if object.owner().is_some()
        || !object.references().is_empty()
        || !object.assets().is_empty()
        || validated
            .configuration()
            .objects()
            .iter()
            .any(|candidate| candidate.owner() == Some(uuid))
    {
        return invalid_model(
            uuid,
            "WSReference must be top-level without children, references, or inline assets",
        );
    }
    require_property_schema(object, &["Name", "Synonym", "Comment", "LocationURL"])?;
    let generated = object.generated_types();
    if generated.len() != 1 || generated[0].kind().as_str() != "Manager" {
        return invalid_model(uuid, "WSReference requires one Manager generated type");
    }
    let manager_type_id = generated[0].uuid();
    let manager_value_id =
        generated[0]
            .value_id()
            .ok_or(ServiceMetadataBuildError::InvalidModel {
                object: uuid,
                reason: "WSReference Manager ValueId is missing",
            })?;
    let location_url = text_property(object, "LocationURL")?.to_owned();
    let (name, synonyms, comment) = project_service_header(object)?;
    let result = WsReferenceNativeIr {
        uuid,
        name,
        synonyms,
        comment,
        location_url,
        manager_type_id,
        manager_value_id,
    };
    validate_ws_reference_ir(&result)?;
    Ok(result)
}

fn project_integration_service(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
) -> Result<IntegrationServiceNativeIr, ServiceMetadataBuildError> {
    let uuid = object.identity().uuid();
    if object.owner().is_some() || !object.references().is_empty() || !object.assets().is_empty() {
        return invalid_model(
            uuid,
            "IntegrationService must be top-level without references or inline assets",
        );
    }
    require_property_schema(
        object,
        &[
            "Name",
            "Synonym",
            "Comment",
            "ExternalIntegrationServiceAddress",
        ],
    )?;
    let generated = object.generated_types();
    if generated.len() != 1 || generated[0].kind().as_str() != "Manager" {
        return invalid_model(
            uuid,
            "IntegrationService requires one Manager generated type",
        );
    }
    let manager_type_id = generated[0].uuid();
    let manager_value_id =
        generated[0]
            .value_id()
            .ok_or(ServiceMetadataBuildError::InvalidModel {
                object: uuid,
                reason: "IntegrationService Manager ValueId is missing",
            })?;
    let external_address = text_property(object, "ExternalIntegrationServiceAddress")?.to_owned();
    let (name, synonyms, comment) = project_service_header(object)?;
    let mut channels = Vec::new();
    for channel in validated
        .configuration()
        .objects()
        .iter()
        .filter(|candidate| candidate.owner() == Some(uuid))
    {
        let channel_uuid = channel.identity().uuid();
        if channel.kind().as_str() != "IntegrationServiceChannel"
            || !channel.references().is_empty()
            || !channel.assets().is_empty()
            || validated
                .configuration()
                .objects()
                .iter()
                .any(|candidate| candidate.owner() == Some(channel_uuid))
        {
            return invalid_model(uuid, "IntegrationService owns an unsupported child object");
        }
        require_property_schema(
            channel,
            &[
                "Name",
                "Synonym",
                "Comment",
                "ExternalIntegrationServiceChannelName",
                "MessageDirection",
                "ReceiveMessageProcessing",
                "Transactioned",
            ],
        )?;
        let generated = channel.generated_types();
        if generated.len() != 1 || generated[0].kind().as_str() != "Manager" {
            return invalid_model(
                channel_uuid,
                "IntegrationServiceChannel requires one Manager generated type",
            );
        }
        let channel_manager_value_id =
            generated[0]
                .value_id()
                .ok_or(ServiceMetadataBuildError::InvalidModel {
                    object: channel_uuid,
                    reason: "IntegrationServiceChannel Manager ValueId is missing",
                })?;
        let (channel_name, channel_synonyms, channel_comment) = project_service_header(channel)?;
        channels.push(IntegrationServiceChannelNativeIr {
            uuid: channel_uuid,
            name: channel_name,
            synonyms: channel_synonyms,
            comment: channel_comment,
            manager_type_id: generated[0].uuid(),
            manager_value_id: channel_manager_value_id,
            external_name: text_property(channel, "ExternalIntegrationServiceChannelName")?
                .to_owned(),
            receive_message_processing: text_property(channel, "ReceiveMessageProcessing")?
                .to_owned(),
            message_direction: enum_property(channel, "MessageDirection")?.to_owned(),
            transactioned: bool_property(channel, "Transactioned")?,
        });
    }
    let result = IntegrationServiceNativeIr {
        uuid,
        name,
        synonyms,
        comment,
        manager_type_id,
        manager_value_id,
        external_address,
        channels,
    };
    validate_integration_service_ir(&result)?;
    Ok(result)
}

fn project_http_service(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
) -> Result<HttpServiceNativeIr, ServiceMetadataBuildError> {
    let uuid = object.identity().uuid();
    if object.owner().is_some()
        || !object.references().is_empty()
        || !object.generated_types().is_empty()
        || !object.assets().is_empty()
    {
        return invalid_model(
            uuid,
            "HTTPService must be top-level without references, generated types, or assets",
        );
    }
    require_property_schema(
        object,
        &[
            "Name",
            "Synonym",
            "Comment",
            "RootURL",
            "ReuseSessions",
            "SessionMaxAge",
        ],
    )?;
    let root_url = text_property(object, "RootURL")?.to_owned();
    if root_url.is_empty() || root_url.chars().any(char::is_whitespace) {
        return invalid_model(uuid, "HTTPService RootURL is not exact");
    }
    let reuse_sessions = enum_property(object, "ReuseSessions")?.to_owned();
    if reuse_sessions_code(&reuse_sessions).is_none() {
        return invalid_model(uuid, "HTTPService ReuseSessions is unsupported");
    }
    let (name, synonyms, comment) = project_service_header(object)?;
    let mut urls = Vec::new();
    for url in validated
        .configuration()
        .objects()
        .iter()
        .filter(|candidate| candidate.owner() == Some(uuid))
    {
        if url.kind().as_str() != "URLTemplate"
            || !url.references().is_empty()
            || !url.generated_types().is_empty()
            || !url.assets().is_empty()
        {
            return invalid_model(uuid, "HTTPService owns an unsupported child object");
        }
        require_property_schema(url, &["Name", "Synonym", "Comment", "Template"])?;
        let template = text_property(url, "Template")?.to_owned();
        if template.is_empty() || template.chars().any(char::is_whitespace) {
            return invalid_model(url.identity().uuid(), "URLTemplate Template is not exact");
        }
        let (url_name, url_synonyms, url_comment) = project_service_header(url)?;
        let mut methods = Vec::new();
        for method in validated
            .configuration()
            .objects()
            .iter()
            .filter(|candidate| candidate.owner() == Some(url.identity().uuid()))
        {
            if method.kind().as_str() != "Method"
                || !method.references().is_empty()
                || !method.generated_types().is_empty()
                || !method.assets().is_empty()
            {
                return invalid_model(
                    url.identity().uuid(),
                    "URLTemplate owns an unsupported child object",
                );
            }
            if validated
                .configuration()
                .objects()
                .iter()
                .any(|candidate| candidate.owner() == Some(method.identity().uuid()))
            {
                return invalid_model(method.identity().uuid(), "HTTP Method cannot own children");
            }
            require_property_schema(
                method,
                &["Name", "Synonym", "Comment", "HTTPMethod", "Handler"],
            )?;
            let http_method = enum_property(method, "HTTPMethod")?.to_owned();
            if http_method_code(&http_method).is_none() {
                return invalid_model(method.identity().uuid(), "HTTPMethod is unsupported");
            }
            let handler = text_property(method, "Handler")?.to_owned();
            if !valid_identifier_segment(&handler) {
                return invalid_model(method.identity().uuid(), "HTTP Handler is not exact");
            }
            let (method_name, method_synonyms, method_comment) = project_service_header(method)?;
            methods.push(HttpMethodNativeIr {
                uuid: method.identity().uuid(),
                name: method_name,
                synonyms: method_synonyms,
                comment: method_comment,
                http_method,
                handler,
            });
        }
        if methods.is_empty() {
            return invalid_model(url.identity().uuid(), "URLTemplate has no HTTP Method");
        }
        urls.push(HttpUrlTemplateNativeIr {
            uuid: url.identity().uuid(),
            name: url_name,
            synonyms: url_synonyms,
            comment: url_comment,
            template,
            methods,
        });
    }
    if urls.is_empty() {
        return invalid_model(uuid, "HTTPService has no URLTemplate");
    }
    let result = HttpServiceNativeIr {
        uuid,
        name,
        synonyms,
        comment,
        root_url,
        reuse_sessions,
        session_max_age: u32_property(object, "SessionMaxAge")?,
        urls,
    };
    validate_http_ir(&result)?;
    Ok(result)
}

fn project_web_service(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
) -> Result<WebServiceNativeIr, ServiceMetadataBuildError> {
    let uuid = object.identity().uuid();
    if object.owner().is_some()
        || !object.references().is_empty()
        || !object.generated_types().is_empty()
        || !object.assets().is_empty()
    {
        return invalid_model(
            uuid,
            "WebService must be top-level without references, generated types, or assets",
        );
    }
    require_property_schema(
        object,
        &[
            "Name",
            "Synonym",
            "Comment",
            "Namespace",
            "XDTOPackages",
            "DescriptorFileName",
            "ReuseSessions",
            "SessionMaxAge",
        ],
    )?;
    let namespace = text_property(object, "Namespace")?.to_owned();
    if !valid_xdto_namespace(&namespace) {
        return invalid_model(uuid, "WebService Namespace is not exact");
    }
    let descriptor_file_name = text_property(object, "DescriptorFileName")?.to_owned();
    if !valid_web_descriptor_file_name(&descriptor_file_name) {
        return invalid_model(uuid, "WebService DescriptorFileName is not exact");
    }
    let reuse_sessions = enum_property(object, "ReuseSessions")?.to_owned();
    if reuse_sessions != "DontUse" {
        return invalid_model(uuid, "WebService ReuseSessions is unsupported");
    }
    let session_max_age = u32_property(object, "SessionMaxAge")?;
    if session_max_age != 20 {
        return invalid_model(uuid, "WebService SessionMaxAge is unsupported");
    }
    let (xdto_package_uuids, xdto_namespaces) =
        project_web_packages(validated, object, property(object, "XDTOPackages")?)?;
    let (name, synonyms, comment) = project_service_header(object)?;

    let mut operations = Vec::new();
    for operation in validated
        .configuration()
        .objects()
        .iter()
        .filter(|candidate| candidate.owner() == Some(uuid))
    {
        if operation.kind().as_str() != "Operation"
            || !operation.references().is_empty()
            || !operation.generated_types().is_empty()
            || !operation.assets().is_empty()
        {
            return invalid_model(uuid, "WebService owns an unsupported child object");
        }
        require_property_schema(
            operation,
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
        )?;
        let procedure_name = text_property(operation, "ProcedureName")?.to_owned();
        if !valid_identifier_segment(&procedure_name) {
            return invalid_model(
                operation.identity().uuid(),
                "WebService ProcedureName is not exact",
            );
        }
        let transactioned = bool_property(operation, "Transactioned")?;
        if transactioned {
            return invalid_model(
                operation.identity().uuid(),
                "WebService Transactioned value is unsupported",
            );
        }
        let data_lock_control_mode = enum_property(operation, "DataLockControlMode")?.to_owned();
        if data_lock_control_mode != "Managed" {
            return invalid_model(
                operation.identity().uuid(),
                "WebService DataLockControlMode is unsupported",
            );
        }
        let (operation_name, operation_synonyms, operation_comment) =
            project_service_header(operation)?;
        let mut parameters = Vec::new();
        for parameter in validated
            .configuration()
            .objects()
            .iter()
            .filter(|candidate| candidate.owner() == Some(operation.identity().uuid()))
        {
            if parameter.kind().as_str() != "Parameter"
                || !parameter.references().is_empty()
                || !parameter.generated_types().is_empty()
                || !parameter.assets().is_empty()
                || validated
                    .configuration()
                    .objects()
                    .iter()
                    .any(|candidate| candidate.owner() == Some(parameter.identity().uuid()))
            {
                return invalid_model(
                    operation.identity().uuid(),
                    "WebService Operation owns an unsupported descendant",
                );
            }
            require_property_schema(
                parameter,
                &[
                    "Name",
                    "Synonym",
                    "Comment",
                    "XDTOValueType",
                    "Nillable",
                    "TransferDirection",
                ],
            )?;
            let transfer_direction = enum_property(parameter, "TransferDirection")?.to_owned();
            if transfer_direction_code(&transfer_direction).is_none() {
                return invalid_model(
                    parameter.identity().uuid(),
                    "WebService TransferDirection is unsupported",
                );
            }
            let (parameter_name, parameter_synonyms, parameter_comment) =
                project_service_header(parameter)?;
            parameters.push(WebServiceParameterNativeIr {
                uuid: parameter.identity().uuid(),
                name: parameter_name,
                synonyms: parameter_synonyms,
                comment: parameter_comment,
                value_type: project_web_xdto_type(parameter, "XDTOValueType")?,
                nillable: bool_property(parameter, "Nillable")?,
                transfer_direction,
            });
        }
        operations.push(WebServiceOperationNativeIr {
            uuid: operation.identity().uuid(),
            name: operation_name,
            synonyms: operation_synonyms,
            comment: operation_comment,
            returning_value_type: project_web_xdto_type(operation, "XDTOReturningValueType")?,
            nillable: bool_property(operation, "Nillable")?,
            transactioned,
            procedure_name,
            data_lock_control_mode,
            parameters,
        });
    }
    if operations.is_empty() {
        return invalid_model(uuid, "WebService has no Operation");
    }
    let result = WebServiceNativeIr {
        uuid,
        name,
        synonyms,
        comment,
        namespace,
        xdto_package_uuids,
        xdto_namespaces,
        descriptor_file_name,
        reuse_sessions,
        session_max_age,
        operations,
    };
    validate_web_service_ir(&result)?;
    Ok(result)
}

fn project_web_packages(
    validated: &ValidatedConfiguration<'_>,
    compiling: &CanonicalObject,
    value: &CanonicalValue,
) -> Result<(Vec<ObjectUuid>, Vec<String>), ServiceMetadataBuildError> {
    let fields = value
        .as_record()
        .ok_or(ServiceMetadataBuildError::InvalidModel {
            object: compiling.identity().uuid(),
            reason: "WebService XDTOPackages is not a record",
        })?;
    require_canonical_record_schema(
        fields,
        &["References", "Namespaces"],
        compiling.identity().uuid(),
        "WebService XDTOPackages schema is not exact",
    )?;
    let references = canonical_text_sequence(
        fields[0].value(),
        compiling.identity().uuid(),
        "WebService XDTOPackage references are not a text sequence",
    )?;
    let namespaces = canonical_text_sequence(
        fields[1].value(),
        compiling.identity().uuid(),
        "WebService XDTO namespaces are not a text sequence",
    )?;

    let mut package_names = BTreeMap::new();
    for candidate in validated.configuration().objects() {
        if candidate.kind().as_str() != "XDTOPackage" || candidate.owner().is_some() {
            continue;
        }
        let Some(name) = candidate
            .properties()
            .iter()
            .find(|field| field.name().as_str() == "Name")
            .and_then(|field| match field.value().kind() {
                CanonicalValueKind::Text(value) => Some(value.as_str()),
                _ => None,
            })
        else {
            continue;
        };
        if package_names
            .insert(name.to_owned(), candidate.identity().uuid())
            .is_some()
        {
            return invalid_model(
                compiling.identity().uuid(),
                "WebService XDTOPackage readable name is ambiguous",
            );
        }
    }

    let mut seen_uuids = BTreeSet::new();
    let mut resolved = Vec::with_capacity(references.len());
    for reference in references {
        let Some(name) = reference.strip_prefix("XDTOPackage.") else {
            return invalid_model(
                compiling.identity().uuid(),
                "WebService XDTOPackage reference is not exact",
            );
        };
        if !valid_identifier_segment(name) {
            return invalid_model(
                compiling.identity().uuid(),
                "WebService XDTOPackage reference is not exact",
            );
        }
        let uuid =
            package_names
                .get(name)
                .copied()
                .ok_or(ServiceMetadataBuildError::InvalidModel {
                    object: compiling.identity().uuid(),
                    reason: "WebService XDTOPackage reference is unresolved",
                })?;
        if !seen_uuids.insert(uuid) {
            return invalid_model(
                compiling.identity().uuid(),
                "WebService XDTOPackage reference is duplicated",
            );
        }
        resolved.push(uuid);
    }
    let mut seen_namespaces = BTreeSet::new();
    for namespace in &namespaces {
        if !valid_web_xdto_namespace(namespace) || !seen_namespaces.insert(namespace.as_str()) {
            return invalid_model(
                compiling.identity().uuid(),
                "WebService XDTO namespace is invalid or duplicated",
            );
        }
    }
    Ok((resolved, namespaces))
}

fn project_web_xdto_type(
    object: &CanonicalObject,
    property_name: &str,
) -> Result<WebServiceXdtoTypeNativeIr, ServiceMetadataBuildError> {
    let fields = property(object, property_name)?.as_record().ok_or(
        ServiceMetadataBuildError::InvalidModel {
            object: object.identity().uuid(),
            reason: "WebService XDTO type is not a record",
        },
    )?;
    require_canonical_record_schema(
        fields,
        &["Namespace", "Name"],
        object.identity().uuid(),
        "WebService XDTO type schema is not exact",
    )?;
    let value = WebServiceXdtoTypeNativeIr {
        namespace: canonical_text(fields[0].value(), object.identity().uuid())?.to_owned(),
        name: canonical_text(fields[1].value(), object.identity().uuid())?.to_owned(),
    };
    validate_web_xdto_type(&value)?;
    Ok(value)
}

fn project_service_header(
    object: &CanonicalObject,
) -> Result<(String, Vec<ServiceLocalizedString>, String), ServiceMetadataBuildError> {
    let name = text_property(object, "Name")?.to_owned();
    if name.is_empty() {
        return invalid_model(object.identity().uuid(), "Name must not be empty");
    }
    Ok((
        name,
        synonym_property(object, "Synonym")?,
        text_property(object, "Comment")?.to_owned(),
    ))
}

fn require_property_schema(
    object: &CanonicalObject,
    expected: &[&str],
) -> Result<(), ServiceMetadataBuildError> {
    if object.properties().len() != expected.len()
        || object
            .properties()
            .iter()
            .zip(expected)
            .any(|(field, expected)| field.name().as_str() != *expected)
    {
        invalid_model(
            object.identity().uuid(),
            "typed property schema is not exact",
        )
    } else {
        Ok(())
    }
}

fn resolve_common_module(
    validated: &ValidatedConfiguration<'_>,
    compiling: ObjectUuid,
    name: &str,
) -> Result<ObjectUuid, ServiceMetadataBuildError> {
    let mut resolved = None;
    for object in validated.configuration().objects() {
        if object.kind().as_str() != "CommonModule" {
            continue;
        }
        let Some(candidate) = object
            .properties()
            .iter()
            .find(|field| field.name().as_str() == "Name")
            .and_then(|field| match field.value().kind() {
                CanonicalValueKind::Text(value) => Some(value.as_str()),
                _ => None,
            })
        else {
            continue;
        };
        if candidate == name && resolved.replace(object.identity().uuid()).is_some() {
            return invalid_model(compiling, "MethodName CommonModule is ambiguous");
        }
    }
    resolved.ok_or(ServiceMetadataBuildError::InvalidModel {
        object: compiling,
        reason: "MethodName CommonModule is unresolved",
    })
}

fn property<'a>(
    object: &'a CanonicalObject,
    name: &str,
) -> Result<&'a CanonicalValue, ServiceMetadataBuildError> {
    object
        .properties()
        .iter()
        .find(|field| field.name().as_str() == name)
        .map(CanonicalField::value)
        .ok_or(ServiceMetadataBuildError::InvalidModel {
            object: object.identity().uuid(),
            reason: "required typed property is missing",
        })
}

fn text_property<'a>(
    object: &'a CanonicalObject,
    name: &str,
) -> Result<&'a str, ServiceMetadataBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::Text(value) => Ok(value.as_str()),
        _ => invalid_model(object.identity().uuid(), "typed property is not text"),
    }
}

fn enum_property<'a>(
    object: &'a CanonicalObject,
    name: &str,
) -> Result<&'a str, ServiceMetadataBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::EnumToken(value) => Ok(value.as_str()),
        _ => invalid_model(
            object.identity().uuid(),
            "typed property is not an enum token",
        ),
    }
}

fn bool_property(object: &CanonicalObject, name: &str) -> Result<bool, ServiceMetadataBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::Bool(value) => Ok(value),
        _ => invalid_model(object.identity().uuid(), "typed property is not boolean"),
    }
}

fn u32_property(object: &CanonicalObject, name: &str) -> Result<u32, ServiceMetadataBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::Integer(value) => value
            .as_str()
            .parse::<u32>()
            .ok()
            .filter(|parsed| parsed.to_string() == value.as_str())
            .ok_or(ServiceMetadataBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "typed property is not canonical u32",
            }),
        _ => invalid_model(object.identity().uuid(), "typed property is not integer"),
    }
}

fn synonym_property(
    object: &CanonicalObject,
    name: &str,
) -> Result<Vec<ServiceLocalizedString>, ServiceMetadataBuildError> {
    let values =
        property(object, name)?
            .as_sequence()
            .ok_or(ServiceMetadataBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "Synonym is not a sequence",
            })?;
    let mut languages = BTreeSet::new();
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let fields = value
            .as_record()
            .ok_or(ServiceMetadataBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "Synonym item is not a record",
            })?;
        if fields.len() != 2
            || fields[0].name().as_str() != "lang"
            || fields[1].name().as_str() != "content"
        {
            return invalid_model(object.identity().uuid(), "Synonym item schema is not exact");
        }
        let language = canonical_text(fields[0].value(), object.identity().uuid())?.to_owned();
        let content = canonical_text(fields[1].value(), object.identity().uuid())?.to_owned();
        if language.is_empty()
            || language.len() > MAX_LANGUAGE_CODE_BYTES
            || !languages.insert(language.clone())
        {
            return invalid_model(
                object.identity().uuid(),
                "Synonym language is empty, duplicated, or too long",
            );
        }
        result.push(ServiceLocalizedString { language, content });
    }
    Ok(result)
}

fn canonical_text(
    value: &CanonicalValue,
    object: ObjectUuid,
) -> Result<&str, ServiceMetadataBuildError> {
    match value.kind() {
        CanonicalValueKind::Text(value) => Ok(value.as_str()),
        _ => invalid_model(object, "canonical value is not text"),
    }
}

fn canonical_text_sequence(
    value: &CanonicalValue,
    object: ObjectUuid,
    reason: &'static str,
) -> Result<Vec<String>, ServiceMetadataBuildError> {
    let values = value
        .as_sequence()
        .ok_or(ServiceMetadataBuildError::InvalidModel { object, reason })?;
    values
        .iter()
        .map(|value| canonical_text(value, object).map(ToOwned::to_owned))
        .collect()
}

fn require_canonical_record_schema(
    fields: &[CanonicalField],
    expected: &[&str],
    object: ObjectUuid,
    reason: &'static str,
) -> Result<(), ServiceMetadataBuildError> {
    if fields.len() != expected.len()
        || fields
            .iter()
            .zip(expected)
            .any(|(field, expected)| field.name().as_str() != *expected)
    {
        invalid_model(object, reason)
    } else {
        Ok(())
    }
}

fn invalid_model<T>(
    object: ObjectUuid,
    reason: &'static str,
) -> Result<T, ServiceMetadataBuildError> {
    Err(ServiceMetadataBuildError::InvalidModel { object, reason })
}

fn valid_identifier_segment(value: &str) -> bool {
    !value.is_empty()
        && !value.contains('.')
        && !value.chars().any(char::is_whitespace)
        && value.len() <= MAX_CANONICAL_TEXT_BYTES
}

fn valid_common_module_reference(value: &str) -> bool {
    value
        .strip_prefix("CommonModule.")
        .is_some_and(valid_identifier_segment)
}

fn valid_cfg_reference(value: &str) -> bool {
    let Some(tail) = value.strip_prefix("cfg:") else {
        return false;
    };
    !tail.is_empty() && !tail.chars().any(char::is_whitespace)
}

fn valid_xdto_namespace(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_whitespace)
}

fn valid_xdto_package_reference(value: &str) -> bool {
    value
        .strip_prefix("XDTOPackage.")
        .is_some_and(valid_identifier_segment)
}

fn valid_web_descriptor_file_name(value: &str) -> bool {
    value.ends_with(".1cws")
        && value.len() > ".1cws".len()
        && !value.chars().any(char::is_whitespace)
}

fn valid_web_xdto_namespace(value: &str) -> bool {
    valid_xdto_namespace(value)
        && !value.chars().any(char::is_control)
        && !matches!(value, XDTO_XML_NAMESPACE | XDTO_XMLNS_NAMESPACE)
}

fn transfer_direction_code(value: &str) -> Option<&'static str> {
    match value {
        "In" => Some("0"),
        "Out" => Some("1"),
        "InOut" => Some("2"),
        _ => None,
    }
}

fn transfer_direction_from_code(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("In"),
        "1" => Some("Out"),
        "2" => Some("InOut"),
        _ => None,
    }
}

fn message_direction_code(value: &str) -> Option<&'static str> {
    match value {
        "Send" => Some("0"),
        "Receive" => Some("1"),
        _ => None,
    }
}

fn message_direction_from_code(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("Send"),
        "1" => Some("Receive"),
        _ => None,
    }
}

fn reuse_sessions_code(value: &str) -> Option<&'static str> {
    match value {
        "DontUse" => Some("0"),
        "Use" => Some("1"),
        "AutoUse" => Some("2"),
        _ => None,
    }
}

fn reuse_sessions_from_code(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("DontUse"),
        "1" => Some("Use"),
        "2" => Some("AutoUse"),
        _ => None,
    }
}

fn http_method_code(value: &str) -> Option<&'static str> {
    match value {
        "DELETE" => Some("2"),
        "GET" => Some("3"),
        "POST" => Some("11"),
        "PUT" => Some("14"),
        _ => None,
    }
}

fn http_method_from_code(value: &str) -> Option<&'static str> {
    match value {
        "2" => Some("DELETE"),
        "3" => Some("GET"),
        "11" => Some("POST"),
        "14" => Some("PUT"),
        _ => None,
    }
}

fn validate_ws_reference_ir(value: &WsReferenceNativeIr) -> Result<(), ServiceMetadataBuildError> {
    if value.location_url.is_empty()
        || value.location_url.chars().any(char::is_whitespace)
        || value.uuid.to_string() == NIL_UUID
        || value.manager_type_id.to_string() == NIL_UUID
        || value.manager_value_id.to_string() == NIL_UUID
        || value.uuid == value.manager_type_id
        || value.uuid == value.manager_value_id
        || value.manager_type_id == value.manager_value_id
        || !valid_identifier_segment(&value.name)
    {
        return Err(native("WSReference identity or LocationURL is not exact"));
    }
    validate_native_text(&value.location_url, "LocationURL")?;
    validate_native_text(&value.comment, "Comment")?;
    if value.synonyms.len() > MAX_CANONICAL_COLLECTION_ITEMS {
        return Err(native("WSReference Synonym collection is too large"));
    }
    let mut languages = BTreeSet::new();
    for synonym in &value.synonyms {
        validate_native_text(&synonym.language, "Synonym language")?;
        validate_native_text(&synonym.content, "Synonym content")?;
        if synonym.language.is_empty()
            || synonym.language.len() > MAX_LANGUAGE_CODE_BYTES
            || !languages.insert(synonym.language.as_str())
        {
            return Err(native(
                "WSReference Synonym language is invalid or duplicated",
            ));
        }
    }
    Ok(())
}

fn validate_integration_service_ir(
    value: &IntegrationServiceNativeIr,
) -> Result<(), ServiceMetadataBuildError> {
    if value.channels.is_empty()
        || value.channels.len() > MAX_CANONICAL_COLLECTION_ITEMS
        || !valid_identifier_segment(&value.name)
        || value.external_address.chars().any(char::is_whitespace)
    {
        return Err(native("IntegrationService root properties are not exact"));
    }
    validate_native_text(&value.external_address, "ExternalIntegrationServiceAddress")?;
    validate_integration_header(value.uuid, &value.name, &value.synonyms, &value.comment)?;
    let mut identities = BTreeSet::new();
    for identity in [value.uuid, value.manager_type_id, value.manager_value_id] {
        if identity.to_string() == NIL_UUID || !identities.insert(identity) {
            return Err(native(
                "IntegrationService root identity is nil or duplicated",
            ));
        }
    }
    let mut names = BTreeSet::new();
    for channel in &value.channels {
        if !valid_identifier_segment(&channel.name)
            || !names.insert(channel.name.to_lowercase())
            || channel.external_name.is_empty()
            || channel.external_name.chars().any(char::is_whitespace)
            || message_direction_code(&channel.message_direction).is_none()
        {
            return Err(native("IntegrationServiceChannel properties are not exact"));
        }
        for identity in [
            channel.uuid,
            channel.manager_type_id,
            channel.manager_value_id,
        ] {
            if identity.to_string() == NIL_UUID || !identities.insert(identity) {
                return Err(native(
                    "IntegrationServiceChannel identity is nil or duplicated",
                ));
            }
        }
        let direction_properties_are_exact = match channel.message_direction.as_str() {
            "Receive" => {
                valid_identifier_segment(&channel.receive_message_processing)
                    && !channel.transactioned
            }
            "Send" => channel.receive_message_processing.is_empty() && channel.transactioned,
            _ => false,
        };
        if !direction_properties_are_exact {
            return Err(native(
                "IntegrationServiceChannel direction-specific properties are not exact",
            ));
        }
        validate_integration_header(
            channel.uuid,
            &channel.name,
            &channel.synonyms,
            &channel.comment,
        )?;
        validate_native_text(
            &channel.external_name,
            "ExternalIntegrationServiceChannelName",
        )?;
        validate_native_text(
            &channel.receive_message_processing,
            "ReceiveMessageProcessing",
        )?;
    }
    Ok(())
}

fn validate_integration_header(
    uuid: ObjectUuid,
    name: &str,
    synonyms: &[ServiceLocalizedString],
    comment: &str,
) -> Result<(), ServiceMetadataBuildError> {
    if uuid.to_string() == NIL_UUID || !valid_identifier_segment(name) {
        return Err(native(
            "IntegrationService metadata header is nil or unnamed",
        ));
    }
    validate_native_text(name, "Name")?;
    validate_native_text(comment, "Comment")?;
    if synonyms.len() > MAX_CANONICAL_COLLECTION_ITEMS {
        return Err(native("IntegrationService Synonym collection is too large"));
    }
    let mut languages = BTreeSet::new();
    for synonym in synonyms {
        validate_native_text(&synonym.language, "Synonym language")?;
        validate_native_text(&synonym.content, "Synonym content")?;
        if synonym.language.is_empty()
            || synonym.language.len() > MAX_LANGUAGE_CODE_BYTES
            || !languages.insert(synonym.language.as_str())
        {
            return Err(native(
                "IntegrationService Synonym language is invalid or duplicated",
            ));
        }
    }
    Ok(())
}

fn validate_http_ir(value: &HttpServiceNativeIr) -> Result<(), ServiceMetadataBuildError> {
    if value.urls.is_empty() || value.urls.len() > MAX_CANONICAL_COLLECTION_ITEMS {
        return Err(native(
            "HTTPService URLTemplate collection is empty or too large",
        ));
    }
    if value.root_url.is_empty()
        || value.root_url.chars().any(char::is_whitespace)
        || reuse_sessions_code(&value.reuse_sessions).is_none()
    {
        return Err(native("HTTPService root properties are not exact"));
    }
    let mut uuids = BTreeSet::from([value.uuid]);
    validate_http_header(value.uuid, &value.name, &value.synonyms, &value.comment)?;
    for url in &value.urls {
        if !uuids.insert(url.uuid)
            || url.template.is_empty()
            || url.template.chars().any(char::is_whitespace)
            || url.methods.is_empty()
            || url.methods.len() > MAX_CANONICAL_COLLECTION_ITEMS
        {
            return Err(native("HTTPService URLTemplate is not exact"));
        }
        validate_http_header(url.uuid, &url.name, &url.synonyms, &url.comment)?;
        for method in &url.methods {
            if !uuids.insert(method.uuid)
                || http_method_code(&method.http_method).is_none()
                || !valid_identifier_segment(&method.handler)
            {
                return Err(native("HTTPService Method is not exact"));
            }
            validate_http_header(method.uuid, &method.name, &method.synonyms, &method.comment)?;
        }
    }
    Ok(())
}

fn validate_http_header(
    uuid: ObjectUuid,
    name: &str,
    synonyms: &[ServiceLocalizedString],
    comment: &str,
) -> Result<(), ServiceMetadataBuildError> {
    if uuid.to_string() == NIL_UUID || name.is_empty() {
        return Err(native("HTTPService metadata header is nil or unnamed"));
    }
    validate_native_text(name, "Name")?;
    validate_native_text(comment, "Comment")?;
    if synonyms.len() > MAX_CANONICAL_COLLECTION_ITEMS {
        return Err(native("HTTPService Synonym collection is too large"));
    }
    let mut languages = BTreeSet::new();
    for synonym in synonyms {
        validate_native_text(&synonym.language, "Synonym language")?;
        validate_native_text(&synonym.content, "Synonym content")?;
        if synonym.language.is_empty()
            || synonym.language.len() > MAX_LANGUAGE_CODE_BYTES
            || !languages.insert(synonym.language.as_str())
        {
            return Err(native(
                "HTTPService Synonym language is invalid or duplicated",
            ));
        }
    }
    Ok(())
}

fn validate_web_service_ir(value: &WebServiceNativeIr) -> Result<(), ServiceMetadataBuildError> {
    if value.operations.is_empty()
        || value.operations.len() > MAX_CANONICAL_COLLECTION_ITEMS
        || value.xdto_package_uuids.len() > MAX_CANONICAL_COLLECTION_ITEMS
        || value.xdto_namespaces.len() > MAX_CANONICAL_COLLECTION_ITEMS
        || !valid_web_xdto_namespace(&value.namespace)
        || !valid_web_descriptor_file_name(&value.descriptor_file_name)
        || value.reuse_sessions != "DontUse"
        || value.session_max_age != 20
    {
        return Err(native("WebService root properties are not exact"));
    }
    validate_web_header(value.uuid, &value.name, &value.synonyms, &value.comment)?;

    let mut uuids = BTreeSet::from([value.uuid]);
    for uuid in &value.xdto_package_uuids {
        if uuid.to_string() == NIL_UUID || !uuids.insert(*uuid) {
            return Err(native("WebService XDTOPackage UUID is nil or duplicated"));
        }
    }
    let mut namespaces = BTreeSet::new();
    for namespace in &value.xdto_namespaces {
        if !valid_web_xdto_namespace(namespace) || !namespaces.insert(namespace.as_str()) {
            return Err(native("WebService XDTO namespace is invalid or duplicated"));
        }
    }

    let mut operation_names = BTreeSet::new();
    for operation in &value.operations {
        if !uuids.insert(operation.uuid)
            || !operation_names.insert(case_insensitive_name_key(&operation.name))
            || operation.transactioned
            || operation.data_lock_control_mode != "Managed"
            || !valid_identifier_segment(&operation.procedure_name)
            || operation.parameters.len() > MAX_CANONICAL_COLLECTION_ITEMS
        {
            return Err(native("WebService Operation is not exact"));
        }
        validate_web_header(
            operation.uuid,
            &operation.name,
            &operation.synonyms,
            &operation.comment,
        )?;
        validate_web_xdto_type(&operation.returning_value_type)?;
        let mut parameter_names = BTreeSet::new();
        for parameter in &operation.parameters {
            if !uuids.insert(parameter.uuid)
                || !parameter_names.insert(case_insensitive_name_key(&parameter.name))
                || transfer_direction_code(&parameter.transfer_direction).is_none()
            {
                return Err(native("WebService Parameter is not exact"));
            }
            validate_web_header(
                parameter.uuid,
                &parameter.name,
                &parameter.synonyms,
                &parameter.comment,
            )?;
            validate_web_xdto_type(&parameter.value_type)?;
        }
    }
    Ok(())
}

fn validate_web_header(
    uuid: ObjectUuid,
    name: &str,
    synonyms: &[ServiceLocalizedString],
    comment: &str,
) -> Result<(), ServiceMetadataBuildError> {
    if uuid.to_string() == NIL_UUID || !valid_identifier_segment(name) {
        return Err(native("WebService metadata header is nil or unnamed"));
    }
    validate_native_text(name, "Name")?;
    validate_native_text(comment, "Comment")?;
    if synonyms.len() > MAX_CANONICAL_COLLECTION_ITEMS {
        return Err(native("WebService Synonym collection is too large"));
    }
    let mut languages = BTreeSet::new();
    for synonym in synonyms {
        validate_native_text(&synonym.language, "Synonym language")?;
        validate_native_text(&synonym.content, "Synonym content")?;
        if synonym.language.is_empty()
            || synonym.language.len() > MAX_LANGUAGE_CODE_BYTES
            || !languages.insert(synonym.language.as_str())
        {
            return Err(native(
                "WebService Synonym language is invalid or duplicated",
            ));
        }
    }
    Ok(())
}

fn validate_web_xdto_type(
    value: &WebServiceXdtoTypeNativeIr,
) -> Result<(), ServiceMetadataBuildError> {
    if !valid_web_xdto_namespace(&value.namespace) || !is_xml_ncname(&value.name) {
        return Err(native("WebService XDTO type is not exact"));
    }
    Ok(())
}

fn case_insensitive_name_key(value: &str) -> String {
    value.chars().flat_map(char::to_lowercase).collect()
}

fn is_xml_ncname(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    is_xml_ncname_start_char(first) && chars.all(is_xml_ncname_char)
}

fn is_xml_ncname_start_char(character: char) -> bool {
    matches!(
        character,
        'A'..='Z'
            | '_'
            | 'a'..='z'
            | '\u{00c0}'..='\u{00d6}'
            | '\u{00d8}'..='\u{00f6}'
            | '\u{00f8}'..='\u{02ff}'
            | '\u{0370}'..='\u{037d}'
            | '\u{037f}'..='\u{1fff}'
            | '\u{200c}'..='\u{200d}'
            | '\u{2070}'..='\u{218f}'
            | '\u{2c00}'..='\u{2fef}'
            | '\u{3001}'..='\u{d7ff}'
            | '\u{f900}'..='\u{fdcf}'
            | '\u{fdf0}'..='\u{fffd}'
            | '\u{10000}'..='\u{effff}'
    )
}

fn is_xml_ncname_char(character: char) -> bool {
    is_xml_ncname_start_char(character)
        || matches!(
            character,
            '-' | '.' | '0'..='9' | '\u{00b7}' | '\u{0300}'..='\u{036f}' | '\u{203f}'..='\u{2040}'
        )
}

fn native_event_name(event: &str) -> Option<&'static str> {
    match event {
        "BeforeDelete" => Some("BeforeDelete_ПередУдалением"),
        "BeforeWrite" => Some("BeforeWrite_ПередЗаписью"),
        "FillCheckProcessing" => Some("FillCheckProcessing_ОбработкаПроверкиЗаполнения"),
        "Filling" => Some("Filling_ОбработкаЗаполнения"),
        "FormGetProcessing" => Some("FormGetProcessing_ОбработкаПолученияФормы"),
        "OnReceiveDataFromMaster" => Some("OnReceiveDataFromMaster_ПриПолученииДанныхОтГлавного"),
        "OnReceiveDataFromSlave" => Some("OnReceiveDataFromSlave_ПриПолученииДанныхОтПодчиненного"),
        "OnSendDataToMaster" => Some("OnSendDataToMaster_ПриОтправкеДанныхГлавному"),
        "OnSendDataToSlave" => Some("OnSendDataToSlave_ПриОтправкеДанныхПодчиненному"),
        "OnSendNodeDataToSlave" => Some("OnSendNodeDataToSlave_ПриОтправкеДанныхУзлаПодчиненному"),
        "OnSetNewNumber" => Some("OnSetNewNumber_ПриУстановкеНовогоНомера"),
        "OnWrite" => Some("OnWrite_ПриЗаписи"),
        "Posting" => Some("Posting_ОбработкаПроведения"),
        "PresentationFieldsGetProcessing" => {
            Some("PresentationFieldsGetProcessing_ОбработкаПолученияПолейПредставления")
        }
        "PresentationGetProcessing" => {
            Some("PresentationGetProcessing_ОбработкаПолученияПредставления")
        }
        _ => None,
    }
}

fn event_from_native(value: &str) -> Option<&'static str> {
    const EVENTS: [&str; 15] = [
        "BeforeDelete",
        "BeforeWrite",
        "FillCheckProcessing",
        "Filling",
        "FormGetProcessing",
        "OnReceiveDataFromMaster",
        "OnReceiveDataFromSlave",
        "OnSendDataToMaster",
        "OnSendDataToSlave",
        "OnSendNodeDataToSlave",
        "OnSetNewNumber",
        "OnWrite",
        "Posting",
        "PresentationFieldsGetProcessing",
        "PresentationGetProcessing",
    ];
    EVENTS
        .into_iter()
        .find(|event| native_event_name(event) == Some(value))
}

fn serialize_scheduled_job(value: &ScheduledJobNativeIr) -> Vec<u8> {
    let mut plain = String::new();
    plain.push_str("{1,\r\n{2,\r\n");
    push_native_header(
        &mut plain,
        value.uuid,
        &value.name,
        &value.synonyms,
        &value.comment,
    );
    plain.push(',');
    push_1c_string(&mut plain, &value.description);
    plain.push(',');
    push_1c_string(&mut plain, &value.key);
    plain.push(',');
    plain.push(if value.use_job { '1' } else { '0' });
    plain.push(',');
    plain.push(if value.predefined { '1' } else { '0' });
    plain.push(',');
    plain.push_str(&value.module_uuid.to_string());
    plain.push(',');
    push_1c_string(&mut plain, &value.method_name);
    write!(
        &mut plain,
        ",{},{}}},0}}",
        value.restart_count_on_failure, value.restart_interval_on_failure
    )
    .expect("writing to String cannot fail");
    native_plaintext(plain)
}

fn serialize_event_subscription(value: &EventSubscriptionNativeIr) -> Vec<u8> {
    let mut plain = String::new();
    plain.push_str("{1,\r\n{1,\r\n");
    push_native_header(
        &mut plain,
        value.uuid,
        &value.name,
        &value.synonyms,
        &value.comment,
    );
    plain.push_str(",\r\n{\"Pattern\"");
    for type_id in &value.source_type_ids {
        plain.push_str(",\r\n{\"#\",");
        plain.push_str(&type_id.to_string());
        plain.push('}');
    }
    plain.push_str("\r\n},");
    push_1c_string(
        &mut plain,
        native_event_name(&value.event)
            .expect("projection accepts only independently evidenced Event values"),
    );
    plain.push(',');
    plain.push_str(&value.module_uuid.to_string());
    plain.push(',');
    push_1c_string(&mut plain, &value.method_name);
    plain.push_str("},0}");
    native_plaintext(plain)
}

fn serialize_xdto_package(value: &XdtoPackageNativeIr) -> Vec<u8> {
    let mut plain = String::new();
    plain.push_str("{1,\r\n{1,\r\n");
    push_native_header(
        &mut plain,
        value.uuid,
        &value.name,
        &value.synonyms,
        &value.comment,
    );
    plain.push(',');
    push_1c_string(&mut plain, &value.namespace);
    plain.push_str("},0}");
    native_plaintext(plain)
}

fn serialize_ws_reference(value: &WsReferenceNativeIr) -> Vec<u8> {
    let mut plain = String::new();
    plain.push_str("{1,\r\n{2,\r\n{");
    push_1c_string(&mut plain, &value.location_url);
    plain.push_str(",0},\r\n");
    push_native_header(
        &mut plain,
        value.uuid,
        &value.name,
        &value.synonyms,
        &value.comment,
    );
    plain.push(',');
    plain.push_str(&value.manager_type_id.to_string());
    plain.push(',');
    plain.push_str(&value.manager_value_id.to_string());
    plain.push_str("},0}");
    native_plaintext(plain)
}

fn serialize_integration_service(value: &IntegrationServiceNativeIr) -> Vec<u8> {
    let mut plain = String::new();
    plain.push_str("{1,\r\n{0,\r\n");
    push_integration_native_header(
        &mut plain,
        value.uuid,
        &value.name,
        &value.synonyms,
        &value.comment,
    );
    plain.push(',');
    plain.push_str(&value.manager_type_id.to_string());
    plain.push(',');
    plain.push_str(&value.manager_value_id.to_string());
    plain.push(',');
    push_1c_string(&mut plain, &value.external_address);
    plain.push_str("},1,\r\n{");
    plain.push_str(INTEGRATION_CHANNEL_COLLECTION_UUID);
    write!(&mut plain, ",{},\r\n{{", value.channels.len()).expect("writing to String cannot fail");
    for channel in &value.channels {
        plain.push_str("\r\n{\r\n{1,\r\n");
        push_integration_native_header(
            &mut plain,
            channel.uuid,
            &channel.name,
            &channel.synonyms,
            &channel.comment,
        );
        plain.push(',');
        plain.push_str(&channel.manager_type_id.to_string());
        plain.push(',');
        plain.push_str(&channel.manager_value_id.to_string());
        plain.push(',');
        push_1c_string(&mut plain, &channel.external_name);
        plain.push(',');
        push_1c_string(&mut plain, &channel.receive_message_processing);
        plain.push(',');
        plain.push_str(
            message_direction_code(&channel.message_direction)
                .expect("validated IntegrationService channel direction has an evidenced code"),
        );
        plain.push(',');
        plain.push(if channel.transactioned { '1' } else { '0' });
        plain.push_str("},0}");
    }
    plain.push_str("\r\n}\r\n}\r\n}");
    native_plaintext(plain)
}

fn serialize_http_service(value: &HttpServiceNativeIr) -> Vec<u8> {
    let mut plain = String::new();
    plain.push_str("{1,\r\n{2,");
    push_1c_string(&mut plain, &value.root_url);
    plain.push_str(",\r\n");
    push_native_header(
        &mut plain,
        value.uuid,
        &value.name,
        &value.synonyms,
        &value.comment,
    );
    plain.push(',');
    plain.push_str(
        reuse_sessions_code(&value.reuse_sessions)
            .expect("validated HTTPService ReuseSessions has an evidenced code"),
    );
    write!(&mut plain, ",{}}},1,\r\n", value.session_max_age)
        .expect("writing to String cannot fail");
    write!(
        &mut plain,
        "{{{HTTP_URL_COLLECTION_UUID},{}",
        value.urls.len()
    )
    .expect("writing to String cannot fail");
    for url in &value.urls {
        plain.push_str(",\r\n{\r\n{0,");
        push_1c_string(&mut plain, &url.template);
        plain.push_str(",\r\n");
        push_native_header(&mut plain, url.uuid, &url.name, &url.synonyms, &url.comment);
        plain.push_str("\r\n},1,\r\n");
        write!(
            &mut plain,
            "{{{HTTP_METHOD_COLLECTION_UUID},{}",
            url.methods.len()
        )
        .expect("writing to String cannot fail");
        for method in &url.methods {
            plain.push_str(",\r\n{\r\n{0,");
            push_1c_string(&mut plain, &method.handler);
            plain.push(',');
            plain.push_str(
                http_method_code(&method.http_method)
                    .expect("validated HTTP method has an evidenced code"),
            );
            plain.push_str(",\r\n");
            push_native_header(
                &mut plain,
                method.uuid,
                &method.name,
                &method.synonyms,
                &method.comment,
            );
            plain.push_str("\r\n},0}");
        }
        plain.push_str("\r\n}\r\n}");
    }
    plain.push_str("\r\n}\r\n}");
    native_plaintext(plain)
}

fn serialize_web_service(value: &WebServiceNativeIr) -> Vec<u8> {
    let mut plain = String::new();
    plain.push_str("{1,\r\n{4,");
    push_1c_string(&mut plain, &value.namespace);
    plain.push_str(",\r\n");
    push_native_header(
        &mut plain,
        value.uuid,
        &value.name,
        &value.synonyms,
        &value.comment,
    );
    plain.push_str(",\r\n{0,");
    write!(&mut plain, "{}", value.xdto_package_uuids.len())
        .expect("writing to String cannot fail");
    for uuid in &value.xdto_package_uuids {
        plain.push_str(",\r\n{\"#\",");
        plain.push_str(DESIGN_TIME_REFERENCE_CLASS_UUID);
        plain.push_str(",\r\n{1,");
        plain.push_str(&uuid.to_string());
        plain.push_str("}\r\n}");
    }
    if !value.xdto_package_uuids.is_empty() {
        plain.push_str("\r\n");
    }
    plain.push_str("},");
    push_1c_string(&mut plain, &value.descriptor_file_name);
    plain.push_str(",\r\n{");
    write!(&mut plain, "{}", value.xdto_namespaces.len()).expect("writing to String cannot fail");
    for namespace in &value.xdto_namespaces {
        plain.push(',');
        push_1c_string(&mut plain, namespace);
    }
    write!(
        &mut plain,
        "}},0,{}}},1,\r\n{{{WEB_OPERATION_COLLECTION_UUID},{}",
        value.session_max_age,
        value.operations.len()
    )
    .expect("writing to String cannot fail");
    for operation in &value.operations {
        plain.push_str(",\r\n{\r\n{1,\r\n");
        push_native_header(
            &mut plain,
            operation.uuid,
            &operation.name,
            &operation.synonyms,
            &operation.comment,
        );
        plain.push_str(",\r\n");
        push_native_xdto_type(&mut plain, &operation.returning_value_type);
        plain.push(',');
        plain.push(if operation.nillable { '1' } else { '0' });
        plain.push(',');
        plain.push(if operation.transactioned { '1' } else { '0' });
        plain.push(',');
        push_1c_string(&mut plain, &operation.procedure_name);
        plain.push_str(",1},1,\r\n{");
        plain.push_str(WEB_PARAMETER_COLLECTION_UUID);
        write!(&mut plain, ",{}", operation.parameters.len())
            .expect("writing to String cannot fail");
        for parameter in &operation.parameters {
            plain.push_str(",\r\n{\r\n{0,\r\n");
            push_native_header(
                &mut plain,
                parameter.uuid,
                &parameter.name,
                &parameter.synonyms,
                &parameter.comment,
            );
            plain.push_str(",\r\n");
            push_native_xdto_type(&mut plain, &parameter.value_type);
            plain.push(',');
            plain.push(if parameter.nillable { '1' } else { '0' });
            plain.push(',');
            plain.push_str(
                transfer_direction_code(&parameter.transfer_direction)
                    .expect("validated WebService parameter direction has an evidenced code"),
            );
            plain.push_str("},0}");
        }
        if !operation.parameters.is_empty() {
            plain.push_str("\r\n");
        }
        plain.push_str("}\r\n}");
    }
    plain.push_str("\r\n}\r\n}");
    native_plaintext(plain)
}

fn push_native_xdto_type(output: &mut String, value: &WebServiceXdtoTypeNativeIr) {
    output.push_str("{0,");
    push_1c_string(output, &value.namespace);
    output.push(',');
    push_1c_string(output, &value.name);
    output.push('}');
}

fn native_plaintext(plain: String) -> Vec<u8> {
    let mut output = Vec::with_capacity(UTF8_BOM.len() + plain.len());
    output.extend_from_slice(UTF8_BOM);
    output.extend_from_slice(plain.as_bytes());
    output
}

fn push_native_header(
    output: &mut String,
    uuid: ObjectUuid,
    name: &str,
    synonyms: &[ServiceLocalizedString],
    comment: &str,
) {
    output.push_str("{3,\r\n{1,0,");
    output.push_str(&uuid.to_string());
    output.push_str("},");
    push_1c_string(output, name);
    output.push_str(",\r\n");
    write!(output, "{{{}", synonyms.len()).expect("writing to String cannot fail");
    for synonym in synonyms {
        output.push(',');
        push_1c_string(output, &synonym.language);
        output.push(',');
        push_1c_string(output, &synonym.content);
    }
    output.push_str("},");
    push_1c_string(output, comment);
    output.push_str(",0,0,");
    output.push_str(NIL_UUID);
    output.push_str(",0}");
}

fn push_integration_native_header(
    output: &mut String,
    uuid: ObjectUuid,
    name: &str,
    synonyms: &[ServiceLocalizedString],
    comment: &str,
) {
    output.push_str("{3,\r\n{1,0,");
    output.push_str(&uuid.to_string());
    output.push_str("},");
    push_1c_string(output, name);
    output.push(',');
    write!(output, "{{{}", synonyms.len()).expect("writing to String cannot fail");
    for synonym in synonyms {
        output.push(',');
        push_1c_string(output, &synonym.language);
        output.push(',');
        push_1c_string(output, &synonym.content);
    }
    output.push_str("},");
    push_1c_string(output, comment);
    output.push_str(",0,0,");
    output.push_str(NIL_UUID);
    output.push_str(",0}");
}

fn push_1c_string(output: &mut String, value: &str) {
    output.push('"');
    for character in value.chars() {
        if character == '"' {
            output.push('"');
        }
        output.push(character);
    }
    output.push('"');
}

fn raw_deflate(plain: &[u8]) -> Result<Vec<u8>, ServiceMetadataBuildError> {
    if plain.len() > MAX_SERVICE_METADATA_PLAIN_BYTES {
        return Err(ServiceMetadataBuildError::PlainPayloadTooLarge {
            maximum: MAX_SERVICE_METADATA_PLAIN_BYTES,
            actual: plain.len(),
        });
    }
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(plain)
        .map_err(ServiceMetadataBuildError::Deflate)?;
    encoder.finish().map_err(ServiceMetadataBuildError::Deflate)
}

fn inflate_bounded(blob: &[u8]) -> Result<Vec<u8>, ServiceMetadataBuildError> {
    let limit = MAX_SERVICE_METADATA_PLAIN_BYTES
        .checked_add(1)
        .expect("service metadata plaintext bound is below usize::MAX");
    let mut decoder = DeflateDecoder::new(blob).take(limit as u64);
    let mut plain = Vec::new();
    decoder
        .read_to_end(&mut plain)
        .map_err(ServiceMetadataBuildError::Inflate)?;
    if plain.len() > MAX_SERVICE_METADATA_PLAIN_BYTES {
        return Err(ServiceMetadataBuildError::PlainPayloadTooLarge {
            maximum: MAX_SERVICE_METADATA_PLAIN_BYTES,
            actual: plain.len(),
        });
    }
    Ok(plain)
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum NativeValue {
    Token(String),
    Text(String),
    List(Vec<NativeValue>),
}

struct NativeParser<'a> {
    input: &'a [u8],
    offset: usize,
    nodes: usize,
}

impl<'a> NativeParser<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            offset: 0,
            nodes: 0,
        }
    }

    fn parse(mut self) -> Result<NativeValue, ServiceMetadataBuildError> {
        if !self.input.starts_with(UTF8_BOM) {
            return Err(native("missing UTF-8 BOM for service metadata layout"));
        }
        self.offset = UTF8_BOM.len();
        let value = self.value(0)?;
        self.whitespace();
        if self.offset != self.input.len() {
            return Err(native("trailing bytes after root value"));
        }
        Ok(value)
    }

    fn value(&mut self, depth: usize) -> Result<NativeValue, ServiceMetadataBuildError> {
        if depth > MAX_NATIVE_DEPTH {
            return Err(native("native value exceeds nesting bound"));
        }
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or_else(|| native("native node count overflow"))?;
        if self.nodes > MAX_NATIVE_NODES {
            return Err(native("native value exceeds node bound"));
        }
        self.whitespace();
        match self.input.get(self.offset) {
            Some(b'{') => self.list(depth),
            Some(b'"') => self.text(),
            Some(_) => self.token(),
            None => Err(native("unexpected end of input")),
        }
    }

    fn list(&mut self, depth: usize) -> Result<NativeValue, ServiceMetadataBuildError> {
        self.offset += 1;
        self.whitespace();
        let mut values = Vec::new();
        if self.input.get(self.offset) == Some(&b'}') {
            self.offset += 1;
            return Ok(NativeValue::List(values));
        }
        loop {
            values.push(self.value(depth + 1)?);
            self.whitespace();
            match self.input.get(self.offset) {
                Some(b',') => {
                    self.offset += 1;
                    self.whitespace();
                    if self.input.get(self.offset) == Some(&b'}') {
                        return Err(native("trailing comma in native list"));
                    }
                }
                Some(b'}') => {
                    self.offset += 1;
                    return Ok(NativeValue::List(values));
                }
                _ => return Err(native("expected comma or closing brace")),
            }
        }
    }

    fn text(&mut self) -> Result<NativeValue, ServiceMetadataBuildError> {
        self.offset += 1;
        let mut output = Vec::new();
        while let Some(byte) = self.input.get(self.offset).copied() {
            if byte == b'"' {
                if self.input.get(self.offset + 1) == Some(&b'"') {
                    output.push(b'"');
                    self.offset += 2;
                } else {
                    self.offset += 1;
                    return String::from_utf8(output)
                        .map(NativeValue::Text)
                        .map_err(|_| native("quoted field is not UTF-8"));
                }
            } else {
                output.push(byte);
                self.offset += 1;
            }
        }
        Err(native("unterminated quoted field"))
    }

    fn token(&mut self) -> Result<NativeValue, ServiceMetadataBuildError> {
        let start = self.offset;
        while let Some(byte) = self.input.get(self.offset) {
            if matches!(byte, b',' | b'}') {
                break;
            }
            self.offset += 1;
        }
        let token = std::str::from_utf8(&self.input[start..self.offset])
            .map_err(|_| native("token is not UTF-8"))?
            .trim();
        if token.is_empty() {
            return Err(native("empty native token"));
        }
        Ok(NativeValue::Token(token.to_owned()))
    }

    fn whitespace(&mut self) {
        while self
            .input
            .get(self.offset)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            self.offset += 1;
        }
    }
}

fn parse_scheduled_job(plain: &[u8]) -> Result<ScheduledJobNativeIr, ServiceMetadataBuildError> {
    let root = NativeParser::new(plain).parse()?;
    let root = exact_list(&root, 3, "root")?;
    exact_token(&root[0], "1", "root discriminator")?;
    exact_token(&root[2], "0", "root tail")?;
    let object = exact_list(&root[1], 10, "ScheduledJob object")?;
    exact_token(&object[0], "2", "ScheduledJob discriminator")?;
    let header = parse_native_header(&object[1])?;
    let description = text(&object[2], "Description")?.to_owned();
    let key = text(&object[3], "Key")?.to_owned();
    let use_job = bool_token(&object[4], "Use")?;
    let predefined = bool_token(&object[5], "Predefined")?;
    let module_uuid = canonical_uuid_token(&object[6], "CommonModule UUID")?;
    if module_uuid.to_string() == NIL_UUID {
        return Err(native("ScheduledJob CommonModule UUID is nil"));
    }
    let method_name = text(&object[7], "method name")?.to_owned();
    if !valid_identifier_segment(&method_name) {
        return Err(native("ScheduledJob method name is not exact"));
    }
    let restart_count_on_failure = canonical_u32_token(&object[8], "restart count")?;
    let restart_interval_on_failure = canonical_u32_token(&object[9], "restart interval")?;
    validate_native_text(&description, "Description")?;
    validate_native_text(&key, "Key")?;
    Ok(ScheduledJobNativeIr {
        uuid: header.uuid,
        name: header.name,
        synonyms: header.synonyms,
        comment: header.comment,
        description,
        key,
        use_job,
        predefined,
        module_uuid,
        method_name,
        restart_count_on_failure,
        restart_interval_on_failure,
    })
}

fn parse_event_subscription(
    plain: &[u8],
) -> Result<EventSubscriptionNativeIr, ServiceMetadataBuildError> {
    let root = NativeParser::new(plain).parse()?;
    let root = exact_list(&root, 3, "root")?;
    exact_token(&root[0], "1", "root discriminator")?;
    exact_token(&root[2], "0", "root tail")?;
    let object = exact_list(&root[1], 6, "EventSubscription object")?;
    exact_token(&object[0], "1", "EventSubscription discriminator")?;
    let header = parse_native_header(&object[1])?;
    let source_type_ids = parse_event_source_type_ids(&object[2])?;
    let native_event = text(&object[3], "Event")?;
    let event = event_from_native(native_event)
        .ok_or_else(|| native("EventSubscription Event has no evidenced mapping"))?
        .to_owned();
    let module_uuid = canonical_uuid_token(&object[4], "CommonModule UUID")?;
    if module_uuid.to_string() == NIL_UUID {
        return Err(native("EventSubscription CommonModule UUID is nil"));
    }
    let method_name = text(&object[5], "method name")?.to_owned();
    if !valid_identifier_segment(&method_name) {
        return Err(native("EventSubscription method name is not exact"));
    }
    Ok(EventSubscriptionNativeIr {
        uuid: header.uuid,
        name: header.name,
        synonyms: header.synonyms,
        comment: header.comment,
        source_type_ids,
        event,
        module_uuid,
        method_name,
    })
}

fn parse_event_source_type_ids(
    value: &NativeValue,
) -> Result<Vec<ObjectUuid>, ServiceMetadataBuildError> {
    let fields = list(value, "EventSubscription Source")?;
    if fields.len() < 2 {
        return Err(native("EventSubscription Source type pattern is empty"));
    }
    if text(&fields[0], "EventSubscription Source marker")? != "Pattern" {
        return Err(native("EventSubscription Source marker is not Pattern"));
    }
    if fields.len() - 1 > MAX_CANONICAL_COLLECTION_ITEMS {
        return Err(native(
            "EventSubscription Source exceeds canonical collection bound",
        ));
    }
    let mut unique = BTreeSet::new();
    let mut result = Vec::with_capacity(fields.len() - 1);
    for item in &fields[1..] {
        let item = exact_list(item, 2, "EventSubscription Source item")?;
        if text(&item[0], "EventSubscription Source item marker")? != "#" {
            return Err(native("EventSubscription Source item is not a TypeId"));
        }
        let type_id = canonical_uuid_token(&item[1], "EventSubscription Source TypeId")?;
        if type_id.to_string() == NIL_UUID || !unique.insert(type_id) {
            return Err(native(
                "EventSubscription Source TypeId is nil or duplicated",
            ));
        }
        result.push(type_id);
    }
    Ok(result)
}

fn parse_xdto_package(plain: &[u8]) -> Result<XdtoPackageNativeIr, ServiceMetadataBuildError> {
    let root = NativeParser::new(plain).parse()?;
    let root = exact_list(&root, 3, "root")?;
    exact_token(&root[0], "1", "root discriminator")?;
    exact_token(&root[2], "0", "root tail")?;
    let object = exact_list(&root[1], 3, "XDTOPackage object")?;
    exact_token(&object[0], "1", "XDTOPackage discriminator")?;
    let header = parse_native_header(&object[1])?;
    let namespace = text(&object[2], "Namespace")?.to_owned();
    if !valid_xdto_namespace(&namespace) {
        return Err(native("XDTOPackage Namespace is not exact"));
    }
    Ok(XdtoPackageNativeIr {
        uuid: header.uuid,
        name: header.name,
        synonyms: header.synonyms,
        comment: header.comment,
        namespace,
    })
}

fn parse_ws_reference(plain: &[u8]) -> Result<WsReferenceNativeIr, ServiceMetadataBuildError> {
    let root = NativeParser::new(plain).parse()?;
    let root = exact_list(&root, 3, "WSReference root")?;
    exact_token(&root[0], "1", "WSReference root discriminator")?;
    exact_token(&root[2], "0", "WSReference root tail")?;
    let object = exact_list(&root[1], 5, "WSReference object")?;
    exact_token(&object[0], "2", "WSReference discriminator")?;
    let location = exact_list(&object[1], 2, "WSReference location")?;
    let location_url = text(&location[0], "WSReference LocationURL")?.to_owned();
    exact_token(&location[1], "0", "WSReference location tail")?;
    let header = parse_native_header(&object[2])?;
    let manager_type_id = canonical_uuid_token(&object[3], "WSReference Manager TypeId")?;
    let manager_value_id = canonical_uuid_token(&object[4], "WSReference Manager ValueId")?;
    let result = WsReferenceNativeIr {
        uuid: header.uuid,
        name: header.name,
        synonyms: header.synonyms,
        comment: header.comment,
        location_url,
        manager_type_id,
        manager_value_id,
    };
    validate_ws_reference_ir(&result)?;
    Ok(result)
}

fn parse_integration_service(
    plain: &[u8],
) -> Result<IntegrationServiceNativeIr, ServiceMetadataBuildError> {
    let root = NativeParser::new(plain).parse()?;
    let root = exact_list(&root, 4, "IntegrationService root")?;
    exact_token(&root[0], "1", "IntegrationService root discriminator")?;
    exact_token(&root[2], "1", "IntegrationService channel marker")?;
    let service = exact_list(&root[1], 5, "IntegrationService object")?;
    exact_token(&service[0], "0", "IntegrationService discriminator")?;
    let header = parse_native_header(&service[1])?;
    let manager_type_id = canonical_uuid_token(&service[2], "IntegrationService Manager TypeId")?;
    let manager_value_id = canonical_uuid_token(&service[3], "IntegrationService Manager ValueId")?;
    let external_address = text(&service[4], "ExternalIntegrationServiceAddress")?.to_owned();

    let collection = exact_list(&root[3], 3, "IntegrationService channel collection")?;
    exact_token(
        &collection[0],
        INTEGRATION_CHANNEL_COLLECTION_UUID,
        "IntegrationService channel collection UUID",
    )?;
    let count = canonical_usize_token(&collection[1], "IntegrationService channel count")?;
    if count == 0 || count > MAX_CANONICAL_COLLECTION_ITEMS {
        return Err(native("IntegrationService channel count is out of bounds"));
    }
    let wrappers = exact_list(&collection[2], count, "IntegrationService channel wrappers")?;
    let mut channels = Vec::with_capacity(count);
    for wrapper in wrappers {
        let wrapper = exact_list(wrapper, 2, "IntegrationService channel entry")?;
        exact_token(&wrapper[1], "0", "IntegrationService channel tail")?;
        let channel = exact_list(&wrapper[0], 8, "IntegrationService channel")?;
        exact_token(&channel[0], "1", "IntegrationService channel discriminator")?;
        let channel_header = parse_native_header(&channel[1])?;
        let message_direction =
            message_direction_from_code(token(&channel[6], "MessageDirection")?)
                .ok_or_else(|| native("IntegrationService MessageDirection code is unsupported"))?
                .to_owned();
        channels.push(IntegrationServiceChannelNativeIr {
            uuid: channel_header.uuid,
            name: channel_header.name,
            synonyms: channel_header.synonyms,
            comment: channel_header.comment,
            manager_type_id: canonical_uuid_token(
                &channel[2],
                "IntegrationServiceChannel Manager TypeId",
            )?,
            manager_value_id: canonical_uuid_token(
                &channel[3],
                "IntegrationServiceChannel Manager ValueId",
            )?,
            external_name: text(&channel[4], "ExternalIntegrationServiceChannelName")?.to_owned(),
            receive_message_processing: text(&channel[5], "ReceiveMessageProcessing")?.to_owned(),
            message_direction,
            transactioned: bool_token(&channel[7], "Transactioned")?,
        });
    }
    let result = IntegrationServiceNativeIr {
        uuid: header.uuid,
        name: header.name,
        synonyms: header.synonyms,
        comment: header.comment,
        manager_type_id,
        manager_value_id,
        external_address,
        channels,
    };
    validate_integration_service_ir(&result)?;
    Ok(result)
}

fn parse_http_service(plain: &[u8]) -> Result<HttpServiceNativeIr, ServiceMetadataBuildError> {
    let root = NativeParser::new(plain).parse()?;
    let root = exact_list(&root, 4, "HTTPService root")?;
    exact_token(&root[0], "1", "HTTPService root discriminator")?;
    exact_token(&root[2], "1", "HTTPService collection marker")?;
    let service = exact_list(&root[1], 5, "HTTPService object")?;
    exact_token(&service[0], "2", "HTTPService discriminator")?;
    let root_url = text(&service[1], "RootURL")?.to_owned();
    let header = parse_native_header(&service[2])?;
    let reuse_sessions = reuse_sessions_from_code(token(&service[3], "ReuseSessions")?)
        .ok_or_else(|| native("HTTPService ReuseSessions code is unsupported"))?
        .to_owned();
    let session_max_age = canonical_u32_token(&service[4], "SessionMaxAge")?;

    let url_collection = list(&root[3], "HTTPService URL collection")?;
    if url_collection.len() < 2 {
        return Err(native("HTTPService URL collection is truncated"));
    }
    exact_token(
        &url_collection[0],
        HTTP_URL_COLLECTION_UUID,
        "HTTPService URL collection UUID",
    )?;
    let url_count = canonical_usize_token(&url_collection[1], "URLTemplate count")?;
    if url_count == 0 || url_count > MAX_CANONICAL_COLLECTION_ITEMS {
        return Err(native("HTTPService URLTemplate count is out of bounds"));
    }
    if url_collection.len() != url_count + 2 {
        return Err(native("HTTPService URLTemplate count is mismatched"));
    }
    let mut urls = Vec::with_capacity(url_count);
    for url_value in &url_collection[2..] {
        let url_entry = exact_list(url_value, 3, "HTTPService URL entry")?;
        exact_token(&url_entry[1], "1", "HTTPService URL method marker")?;
        let url = exact_list(&url_entry[0], 3, "HTTPService URLTemplate")?;
        exact_token(&url[0], "0", "HTTPService URLTemplate discriminator")?;
        let template = text(&url[1], "URLTemplate Template")?.to_owned();
        let url_header = parse_native_header(&url[2])?;

        let method_collection = list(&url_entry[2], "HTTPService Method collection")?;
        if method_collection.len() < 2 {
            return Err(native("HTTPService Method collection is truncated"));
        }
        exact_token(
            &method_collection[0],
            HTTP_METHOD_COLLECTION_UUID,
            "HTTPService Method collection UUID",
        )?;
        let method_count = canonical_usize_token(&method_collection[1], "HTTP Method count")?;
        if method_count == 0 || method_count > MAX_CANONICAL_COLLECTION_ITEMS {
            return Err(native("HTTPService Method count is out of bounds"));
        }
        if method_collection.len() != method_count + 2 {
            return Err(native("HTTPService Method count is mismatched"));
        }
        let mut methods = Vec::with_capacity(method_count);
        for method_value in &method_collection[2..] {
            let method_entry = exact_list(method_value, 2, "HTTPService Method entry")?;
            exact_token(&method_entry[1], "0", "HTTPService Method tail")?;
            let method = exact_list(&method_entry[0], 4, "HTTPService Method")?;
            exact_token(&method[0], "0", "HTTPService Method discriminator")?;
            let handler = text(&method[1], "HTTPService Handler")?.to_owned();
            let http_method = http_method_from_code(token(&method[2], "HTTP method code")?)
                .ok_or_else(|| native("HTTP method code is unsupported"))?
                .to_owned();
            let method_header = parse_native_header(&method[3])?;
            methods.push(HttpMethodNativeIr {
                uuid: method_header.uuid,
                name: method_header.name,
                synonyms: method_header.synonyms,
                comment: method_header.comment,
                http_method,
                handler,
            });
        }
        urls.push(HttpUrlTemplateNativeIr {
            uuid: url_header.uuid,
            name: url_header.name,
            synonyms: url_header.synonyms,
            comment: url_header.comment,
            template,
            methods,
        });
    }
    let result = HttpServiceNativeIr {
        uuid: header.uuid,
        name: header.name,
        synonyms: header.synonyms,
        comment: header.comment,
        root_url,
        reuse_sessions,
        session_max_age,
        urls,
    };
    validate_http_ir(&result)?;
    Ok(result)
}

fn parse_web_service(plain: &[u8]) -> Result<WebServiceNativeIr, ServiceMetadataBuildError> {
    let root = NativeParser::new(plain).parse()?;
    let root = exact_list(&root, 4, "WebService root")?;
    exact_token(&root[0], "1", "WebService root discriminator")?;
    exact_token(&root[2], "1", "WebService collection marker")?;
    let service = exact_list(&root[1], 8, "WebService object")?;
    exact_token(&service[0], "4", "WebService discriminator")?;
    let namespace = text(&service[1], "WebService Namespace")?.to_owned();
    let header = parse_native_header(&service[2])?;
    let xdto_package_uuids = parse_web_package_references(&service[3])?;
    let descriptor_file_name = text(&service[4], "DescriptorFileName")?.to_owned();
    let xdto_namespaces = parse_web_namespace_packages(&service[5])?;
    exact_token(&service[6], "0", "WebService ReuseSessions")?;
    let session_max_age = canonical_u32_token(&service[7], "SessionMaxAge")?;
    if session_max_age != 20 {
        return Err(native("WebService SessionMaxAge code is unsupported"));
    }

    let collection = list(&root[3], "WebService Operation collection")?;
    if collection.len() < 2 {
        return Err(native("WebService Operation collection is truncated"));
    }
    exact_token(
        &collection[0],
        WEB_OPERATION_COLLECTION_UUID,
        "WebService Operation collection UUID",
    )?;
    let count = canonical_usize_token(&collection[1], "WebService Operation count")?;
    if count == 0 || count > MAX_CANONICAL_COLLECTION_ITEMS || collection.len() != count + 2 {
        return Err(native(
            "WebService Operation count is out of bounds or mismatched",
        ));
    }
    let mut operations = Vec::with_capacity(count);
    for value in &collection[2..] {
        let entry = exact_list(value, 3, "WebService Operation entry")?;
        exact_token(&entry[1], "1", "WebService Operation parameter marker")?;
        let operation = exact_list(&entry[0], 7, "WebService Operation")?;
        exact_token(&operation[0], "1", "WebService Operation discriminator")?;
        let operation_header = parse_native_header(&operation[1])?;
        let returning_value_type = parse_web_xdto_type(&operation[2])?;
        let nillable = bool_token(&operation[3], "WebService Operation Nillable")?;
        let transactioned = bool_token(&operation[4], "WebService Transactioned")?;
        if transactioned {
            return Err(native("WebService Transactioned code is unsupported"));
        }
        let procedure_name = text(&operation[5], "WebService ProcedureName")?.to_owned();
        exact_token(&operation[6], "1", "WebService DataLockControlMode")?;

        let parameter_collection = list(&entry[2], "WebService Parameter collection")?;
        if parameter_collection.len() < 2 {
            return Err(native("WebService Parameter collection is truncated"));
        }
        exact_token(
            &parameter_collection[0],
            WEB_PARAMETER_COLLECTION_UUID,
            "WebService Parameter collection UUID",
        )?;
        let parameter_count =
            canonical_usize_token(&parameter_collection[1], "WebService Parameter count")?;
        if parameter_count > MAX_CANONICAL_COLLECTION_ITEMS
            || parameter_collection.len() != parameter_count + 2
        {
            return Err(native(
                "WebService Parameter count is out of bounds or mismatched",
            ));
        }
        let mut parameters = Vec::with_capacity(parameter_count);
        for parameter_value in &parameter_collection[2..] {
            let parameter_entry = exact_list(parameter_value, 2, "WebService Parameter entry")?;
            exact_token(&parameter_entry[1], "0", "WebService Parameter tail")?;
            let parameter = exact_list(&parameter_entry[0], 5, "WebService Parameter")?;
            exact_token(&parameter[0], "0", "WebService Parameter discriminator")?;
            let parameter_header = parse_native_header(&parameter[1])?;
            let value_type = parse_web_xdto_type(&parameter[2])?;
            let parameter_nillable = bool_token(&parameter[3], "WebService Nillable")?;
            let transfer_direction =
                transfer_direction_from_code(token(&parameter[4], "TransferDirection")?)
                    .ok_or_else(|| native("WebService TransferDirection code is unsupported"))?
                    .to_owned();
            parameters.push(WebServiceParameterNativeIr {
                uuid: parameter_header.uuid,
                name: parameter_header.name,
                synonyms: parameter_header.synonyms,
                comment: parameter_header.comment,
                value_type,
                nillable: parameter_nillable,
                transfer_direction,
            });
        }
        operations.push(WebServiceOperationNativeIr {
            uuid: operation_header.uuid,
            name: operation_header.name,
            synonyms: operation_header.synonyms,
            comment: operation_header.comment,
            returning_value_type,
            nillable,
            transactioned,
            procedure_name,
            data_lock_control_mode: "Managed".to_owned(),
            parameters,
        });
    }
    let result = WebServiceNativeIr {
        uuid: header.uuid,
        name: header.name,
        synonyms: header.synonyms,
        comment: header.comment,
        namespace,
        xdto_package_uuids,
        xdto_namespaces,
        descriptor_file_name,
        reuse_sessions: "DontUse".to_owned(),
        session_max_age,
        operations,
    };
    validate_web_service_ir(&result)?;
    Ok(result)
}

fn parse_web_package_references(
    value: &NativeValue,
) -> Result<Vec<ObjectUuid>, ServiceMetadataBuildError> {
    let fields = list(value, "WebService XDTOPackage references")?;
    if fields.len() < 2 {
        return Err(native("WebService XDTOPackage references are truncated"));
    }
    exact_token(&fields[0], "0", "WebService XDTOPackage marker")?;
    let count = canonical_usize_token(&fields[1], "WebService XDTOPackage count")?;
    if count > MAX_CANONICAL_COLLECTION_ITEMS || fields.len() != count + 2 {
        return Err(native(
            "WebService XDTOPackage count is out of bounds or mismatched",
        ));
    }
    let mut result = Vec::with_capacity(count);
    for value in &fields[2..] {
        let typed = exact_list(value, 3, "WebService XDTOPackage reference")?;
        if text(&typed[0], "WebService XDTOPackage reference marker")? != "#" {
            return Err(native(
                "WebService XDTOPackage reference marker is unsupported",
            ));
        }
        exact_token(
            &typed[1],
            DESIGN_TIME_REFERENCE_CLASS_UUID,
            "WebService XDTOPackage reference class",
        )?;
        let target = exact_list(&typed[2], 2, "WebService XDTOPackage target")?;
        exact_token(&target[0], "1", "WebService XDTOPackage target marker")?;
        result.push(canonical_uuid_token(
            &target[1],
            "WebService XDTOPackage UUID",
        )?);
    }
    Ok(result)
}

fn parse_web_namespace_packages(
    value: &NativeValue,
) -> Result<Vec<String>, ServiceMetadataBuildError> {
    let fields = list(value, "WebService XDTO namespace packages")?;
    if fields.is_empty() {
        return Err(native("WebService XDTO namespace packages are truncated"));
    }
    let count = canonical_usize_token(&fields[0], "WebService XDTO namespace count")?;
    if count > MAX_CANONICAL_COLLECTION_ITEMS || fields.len() != count + 1 {
        return Err(native(
            "WebService XDTO namespace count is out of bounds or mismatched",
        ));
    }
    fields[1..]
        .iter()
        .map(|value| text(value, "WebService XDTO namespace").map(ToOwned::to_owned))
        .collect()
}

fn parse_web_xdto_type(
    value: &NativeValue,
) -> Result<WebServiceXdtoTypeNativeIr, ServiceMetadataBuildError> {
    let fields = exact_list(value, 3, "WebService XDTO type")?;
    exact_token(&fields[0], "0", "WebService XDTO type discriminator")?;
    let result = WebServiceXdtoTypeNativeIr {
        namespace: text(&fields[1], "WebService XDTO type namespace")?.to_owned(),
        name: text(&fields[2], "WebService XDTO type name")?.to_owned(),
    };
    validate_web_xdto_type(&result)?;
    Ok(result)
}

struct NativeHeader {
    uuid: ObjectUuid,
    name: String,
    synonyms: Vec<ServiceLocalizedString>,
    comment: String,
}

fn parse_native_header(value: &NativeValue) -> Result<NativeHeader, ServiceMetadataBuildError> {
    let header = exact_list(value, 9, "metadata header")?;
    exact_token(&header[0], "3", "metadata header discriminator")?;
    let identity = exact_list(&header[1], 3, "metadata identity")?;
    exact_token(&identity[0], "1", "identity discriminator")?;
    exact_token(&identity[1], "0", "identity tail")?;
    let uuid = canonical_uuid_token(&identity[2], "object UUID")?;
    let name = text(&header[2], "Name")?.to_owned();
    let synonyms = parse_synonyms(&header[3])?;
    let comment = text(&header[4], "Comment")?.to_owned();
    exact_token(&header[5], "0", "header flag 1")?;
    exact_token(&header[6], "0", "header flag 2")?;
    exact_token(&header[7], NIL_UUID, "header nil UUID")?;
    exact_token(&header[8], "0", "header tail")?;
    validate_native_text(&name, "Name")?;
    validate_native_text(&comment, "Comment")?;
    if name.is_empty() {
        return Err(native("Name must not be empty"));
    }
    Ok(NativeHeader {
        uuid,
        name,
        synonyms,
        comment,
    })
}

fn parse_synonyms(
    value: &NativeValue,
) -> Result<Vec<ServiceLocalizedString>, ServiceMetadataBuildError> {
    let fields = list(value, "Synonym")?;
    if fields.is_empty() {
        return Err(native("Synonym list has no count"));
    }
    let count = canonical_usize_token(&fields[0], "Synonym count")?;
    if count > MAX_CANONICAL_COLLECTION_ITEMS || fields.len() != 1 + count * 2 {
        return Err(native("Synonym count is out of bounds or mismatched"));
    }
    let mut languages = BTreeSet::new();
    let mut synonyms = Vec::with_capacity(count);
    for pair in fields[1..].chunks_exact(2) {
        let language = text(&pair[0], "Synonym language")?.to_owned();
        let content = text(&pair[1], "Synonym content")?.to_owned();
        validate_native_text(&language, "Synonym language")?;
        validate_native_text(&content, "Synonym content")?;
        if language.is_empty()
            || language.len() > MAX_LANGUAGE_CODE_BYTES
            || !languages.insert(language.clone())
        {
            return Err(native("Synonym language is empty, duplicated, or too long"));
        }
        synonyms.push(ServiceLocalizedString { language, content });
    }
    Ok(synonyms)
}

fn canonical_uuid_token(
    value: &NativeValue,
    context: &'static str,
) -> Result<ObjectUuid, ServiceMetadataBuildError> {
    let value = token(value, context)?;
    let uuid = ObjectUuid::parse(value).map_err(|_| native("native UUID is not canonical"))?;
    if uuid.to_string() != value {
        return Err(native("native UUID spelling is not canonical"));
    }
    Ok(uuid)
}

fn canonical_u32_token(
    value: &NativeValue,
    context: &'static str,
) -> Result<u32, ServiceMetadataBuildError> {
    let value = token(value, context)?;
    value
        .parse::<u32>()
        .ok()
        .filter(|parsed| parsed.to_string() == value)
        .ok_or_else(|| native("native integer is not canonical u32"))
}

fn canonical_usize_token(
    value: &NativeValue,
    context: &'static str,
) -> Result<usize, ServiceMetadataBuildError> {
    let value = token(value, context)?;
    value
        .parse::<usize>()
        .ok()
        .filter(|parsed| parsed.to_string() == value)
        .ok_or_else(|| native("native count is not canonical decimal"))
}

fn bool_token(
    value: &NativeValue,
    context: &'static str,
) -> Result<bool, ServiceMetadataBuildError> {
    match token(value, context)? {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(native("native boolean is not 0 or 1")),
    }
}

fn validate_native_text(
    value: &str,
    _field: &'static str,
) -> Result<(), ServiceMetadataBuildError> {
    if value.len() > MAX_CANONICAL_TEXT_BYTES {
        return Err(native("native text exceeds canonical bound"));
    }
    if value.chars().any(|character| character == '\0') {
        return Err(native("native text contains NUL"));
    }
    Ok(())
}

fn exact_list<'a>(
    value: &'a NativeValue,
    expected: usize,
    context: &'static str,
) -> Result<&'a [NativeValue], ServiceMetadataBuildError> {
    let values = list(value, context)?;
    if values.len() != expected {
        return Err(native("native list has unexpected field count"));
    }
    Ok(values)
}

fn list<'a>(
    value: &'a NativeValue,
    _context: &'static str,
) -> Result<&'a [NativeValue], ServiceMetadataBuildError> {
    match value {
        NativeValue::List(values) => Ok(values),
        _ => Err(native("native value is not a list")),
    }
}

fn exact_token(
    value: &NativeValue,
    expected: &str,
    context: &'static str,
) -> Result<(), ServiceMetadataBuildError> {
    if token(value, context)? != expected {
        return Err(native("native token does not match evidenced value"));
    }
    Ok(())
}

fn token<'a>(
    value: &'a NativeValue,
    _context: &'static str,
) -> Result<&'a str, ServiceMetadataBuildError> {
    match value {
        NativeValue::Token(value) => Ok(value),
        _ => Err(native("native value is not a token")),
    }
}

fn text<'a>(
    value: &'a NativeValue,
    _context: &'static str,
) -> Result<&'a str, ServiceMetadataBuildError> {
    match value {
        NativeValue::Text(value) => Ok(value),
        _ => Err(native("native value is not quoted text")),
    }
}

fn native(reason: &str) -> ServiceMetadataBuildError {
    ServiceMetadataBuildError::Native(reason.to_owned())
}

fn xml_profile_version(profile: &ProfileId) -> Option<&'static str> {
    match profile.as_str() {
        "xml-2.20" => Some("2.20"),
        "xml-2.21" => Some("2.21"),
        _ => None,
    }
}

fn write_synonyms(output: &mut String, synonyms: &[ServiceLocalizedString]) {
    write_synonyms_at(output, "\t\t\t", synonyms);
}

fn write_synonyms_at(output: &mut String, indent: &str, synonyms: &[ServiceLocalizedString]) {
    if synonyms.is_empty() {
        output.push_str(indent);
        output.push_str("<Synonym/>\r\n");
        return;
    }
    output.push_str(indent);
    output.push_str("<Synonym>\r\n");
    let item_indent = format!("{indent}\t");
    let value_indent = format!("{indent}\t\t");
    for synonym in synonyms {
        output.push_str(&item_indent);
        output.push_str("<v8:item>\r\n");
        write_xml_text_element(output, &value_indent, "v8:lang", &synonym.language);
        write_xml_text_element(output, &value_indent, "v8:content", &synonym.content);
        output.push_str(&item_indent);
        output.push_str("</v8:item>\r\n");
    }
    output.push_str(indent);
    output.push_str("</Synonym>\r\n");
}

fn write_xml_text_element(output: &mut String, indent: &str, name: &str, value: &str) {
    output.push_str(indent);
    output.push('<');
    output.push_str(name);
    if value.is_empty() {
        output.push_str("/>\r\n");
        return;
    }
    output.push('>');
    push_xml_text(output, value);
    output.push_str("</");
    output.push_str(name);
    output.push_str(">\r\n");
}

fn collect_custom_xdto_namespace(namespace: &str, target: &mut BTreeSet<String>) {
    if !matches!(namespace, XDTO_XML_SCHEMA_NAMESPACE | XDTO_CORE_NAMESPACE) {
        target.insert(namespace.to_owned());
    }
}

fn write_web_package_item(output: &mut String, value_type: &str, value: &str) {
    output.push_str("\t\t\t\t<xr:Item>\r\n");
    output.push_str("\t\t\t\t\t<xr:Presentation/>\r\n");
    output.push_str("\t\t\t\t\t<xr:CheckState>0</xr:CheckState>\r\n");
    output.push_str("\t\t\t\t\t<xr:Value xsi:type=\"");
    output.push_str(value_type);
    output.push_str("\">");
    push_xml_text(output, value);
    output.push_str("</xr:Value>\r\n");
    output.push_str("\t\t\t\t</xr:Item>\r\n");
}

fn write_xdto_type_element(
    output: &mut String,
    indent: &str,
    element: &str,
    value: &WebServiceXdtoTypeNativeIr,
    prefixes: &BTreeMap<&str, String>,
) {
    let prefix = match value.namespace.as_str() {
        XDTO_XML_SCHEMA_NAMESPACE => "xs",
        XDTO_CORE_NAMESPACE => "v8",
        namespace => prefixes
            .get(namespace)
            .expect("custom XDTO namespace was collected")
            .as_str(),
    };
    output.push_str(indent);
    output.push('<');
    output.push_str(element);
    output.push('>');
    output.push_str(prefix);
    output.push(':');
    output.push_str(&value.name);
    output.push_str("</");
    output.push_str(element);
    output.push_str(">\r\n");
}

fn push_xml_attribute(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&apos;"),
            _ => output.push(character),
        }
    }
}

fn push_xml_text(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            _ => output.push(character),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ibcmd_core::diagnostic::{ObjectPath, PathSegment, PropertyPath};
    use ibcmd_core::family::FamilyId;
    use ibcmd_core::identity::LogicalIdentity;
    use ibcmd_core::model::{
        CanonicalConfiguration, CanonicalObjectParts, GeneratedType, GeneratedTypeKind,
        MetadataKind,
    };
    use ibcmd_core::profile::{ProfileSourceKind, parse_profile_source, resolve_profiles};
    use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
    use ibcmd_core::validate::validate_configuration;
    use ibcmd_core::value::{CanonicalText, CanonicalValue};
    use ibcmd_core::version::XmlDialect;
    use ibcmd_xml::{XmlReader, bundled_metadata_registry};

    use crate::compiler::graph::{ObjectStorageRoute, build_bootstrap_graph};
    use crate::compiler::identity::collect_bootstrap_identities;

    use super::*;

    const CONFIGURATION_UUID: &str = "11111111-1111-4111-8111-111111111111";
    const MODULE_UUID: &str = "dc2b7a9e-132b-4a30-b7a8-ec72bf3d2e63";
    const JOB_UUID: &str = "c7ffd8ab-15e9-4cf1-a7fd-d05534dff000";
    const EVENT_UUID: &str = "a64b15fa-fc34-43fe-a366-d27c0f1c3df2";
    const EVENT_MODULE_UUID: &str = "f96cbb27-7619-41e0-8577-e623ef02dc58";
    const SOURCE_ONE_TYPE_ID: &str = "48667aa7-1fec-4d7b-b697-2bfe84bbb82b";
    const SOURCE_TWO_TYPE_ID: &str = "db719ff3-91c0-4d23-8d18-afa5fc3221cf";
    const XDTO_UUID: &str = "ac7ea771-4b10-4d43-9c0a-9cd36e4c49a4";
    const HTTP_UUID: &str = "db821e7a-ff22-4889-b166-1a1bc1118587";
    const HTTP_URL_UUID: &str = "bbd4d7c8-2488-474c-b92c-8f689a56e62e";
    const HTTP_METHOD_UUID: &str = "f909d950-4db8-490c-aaf6-7a2e975a310d";
    const WEB_UUID: &str = "a4ed8b24-bd23-45a7-9f34-61b25b91d0c6";
    const WEB_OPERATION_UUID: &str = "65efaa10-3239-4f0f-a08e-88c89d9d8d5a";
    const WEB_PARAMETER_IN_UUID: &str = "93aa5247-6823-4f70-9d47-e5a0f409828a";
    const WEB_PARAMETER_OUT_UUID: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    const WEB_PARAMETER_IN_OUT_UUID: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    const WS_UUID: &str = "b409116f-3ba2-4303-9bdc-f14961c879d6";
    const WS_MANAGER_TYPE_ID: &str = "651f0326-6551-49a6-a840-b6e604b61639";
    const WS_MANAGER_VALUE_ID: &str = "dd7a8d59-2aeb-4921-a33b-913be961ec98";
    const INTEGRATION_UUID: &str = "c512a1cd-1240-4e46-8bad-8b7b27c5c25a";
    const INTEGRATION_MANAGER_TYPE_ID: &str = "5362f1d1-1f56-4a61-a52e-6519a060293e";
    const INTEGRATION_MANAGER_VALUE_ID: &str = "ad884943-3c3a-4073-ab34-ed12a0d67556";
    const INTEGRATION_CHANNEL_UUID: &str = "1ef0581c-b1d8-4115-87f1-7856f6c06bb6";
    const INTEGRATION_CHANNEL_TYPE_ID: &str = "71313d47-3c6e-464a-8776-f7eb0626fd6b";
    const INTEGRATION_CHANNEL_VALUE_ID: &str = "bb1ff475-725d-46cb-8cbc-9ff08970cccc";

    fn xml(version: &str, module: &str) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"{version}\">\r\n\
\t<ScheduledJob uuid=\"{JOB_UUID}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>ЗагрузкаКурсовВалют</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Загрузка курсов валют</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<MethodName>CommonModule.{module}.ПриЗагрузкеАктуальныхКурсов</MethodName>\r\n\
\t\t\t<Description/>\r\n\
\t\t\t<Key/>\r\n\
\t\t\t<Use>false</Use>\r\n\
\t\t\t<Predefined>true</Predefined>\r\n\
\t\t\t<RestartCountOnFailure>10</RestartCountOnFailure>\r\n\
\t\t\t<RestartIntervalOnFailure>600</RestartIntervalOnFailure>\r\n\
\t\t</Properties>\r\n\
\t</ScheduledJob>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    fn event_xml(version: &str, module: &str, first_source: &str) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"{version}\">\r\n\
\t<EventSubscription uuid=\"{EVENT_UUID}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>ВариантыОтчетовПередУдалениемИдентификатораОбъектаМетаданных</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Варианты отчетов перед удалением идентификатора объекта метаданных</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<Source><v8:Type>cfg:CatalogObject.{first_source}</v8:Type><v8:Type>cfg:CatalogObject.ИдентификаторыОбъектовМетаданных</v8:Type></Source>\r\n\
\t\t\t<Event>BeforeDelete</Event>\r\n\
\t\t\t<Handler>CommonModule.{module}.ПередУдалениемИдентификатораОбъектаМетаданных</Handler>\r\n\
\t\t</Properties>\r\n\
\t</EventSubscription>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    fn xdto_xml(version: &str, namespace: &str) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"{version}\">\r\n\
\t<XDTOPackage uuid=\"{XDTO_UUID}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>АдминистрированиеОбменаДанными_2_4_5_1</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Администрирование обмена данными 2.4.5.1</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<Namespace>{namespace}</Namespace>\r\n\
\t\t</Properties>\r\n\
\t</XDTOPackage>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    fn http_xml(version: &str, method: &str) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"{version}\">\r\n\
\t<HTTPService uuid=\"{HTTP_UUID}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>Биллинг</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Биллинг</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<RootURL>billing</RootURL>\r\n\
\t\t\t<ReuseSessions>AutoUse</ReuseSessions>\r\n\
\t\t\t<SessionMaxAge>20</SessionMaxAge>\r\n\
\t\t</Properties>\r\n\
\t\t<ChildObjects>\r\n\
\t\t\t<URLTemplate uuid=\"{HTTP_URL_UUID}\">\r\n\
\t\t\t\t<Properties>\r\n\
\t\t\t\t\t<Name>Версия</Name><Synonym/><Comment/><Template>/version</Template>\r\n\
\t\t\t\t</Properties>\r\n\
\t\t\t\t<ChildObjects>\r\n\
\t\t\t\t\t<Method uuid=\"{HTTP_METHOD_UUID}\">\r\n\
\t\t\t\t\t\t<Properties>\r\n\
\t\t\t\t\t\t\t<Name>Получить</Name><Synonym/><Comment/><HTTPMethod>{method}</HTTPMethod><Handler>ВерсияПолучить</Handler>\r\n\
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

    fn web_xml(version: &str, lock_mode: &str) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"{XDTO_CORE_NAMESPACE}\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"{XDTO_XML_SCHEMA_NAMESPACE}\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:svc=\"http://example.test/types\" version=\"{version}\">\r\n\
\t<WebService uuid=\"{WEB_UUID}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>InterfaceVersion</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Interface version</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<Namespace>http://www.1c.ru/SaaS/1.0/WS</Namespace>\r\n\
\t\t\t<XDTOPackages>\r\n\
\t\t\t\t<xr:Item><xr:Presentation/><xr:CheckState>0</xr:CheckState><xr:Value xsi:type=\"xr:MDObjectRef\">XDTOPackage.АдминистрированиеОбменаДанными_2_4_5_1</xr:Value></xr:Item>\r\n\
\t\t\t\t<xr:Item><xr:Presentation/><xr:CheckState>0</xr:CheckState><xr:Value xsi:type=\"xs:string\">{XDTO_CORE_NAMESPACE}</xr:Value></xr:Item>\r\n\
\t\t\t</XDTOPackages>\r\n\
\t\t\t<DescriptorFileName>InterfaceVersion.1cws</DescriptorFileName>\r\n\
\t\t\t<ReuseSessions>DontUse</ReuseSessions>\r\n\
\t\t\t<SessionMaxAge>20</SessionMaxAge>\r\n\
\t\t</Properties>\r\n\
\t\t<ChildObjects>\r\n\
\t\t\t<Operation uuid=\"{WEB_OPERATION_UUID}\">\r\n\
\t\t\t\t<Properties>\r\n\
\t\t\t\t\t<Name>GetVersions</Name><Synonym/><Comment/>\r\n\
\t\t\t\t\t<XDTOReturningValueType>svc:Result</XDTOReturningValueType>\r\n\
\t\t\t\t\t<Nillable>true</Nillable><Transactioned>false</Transactioned>\r\n\
\t\t\t\t\t<ProcedureName>GetVersions</ProcedureName><DataLockControlMode>{lock_mode}</DataLockControlMode>\r\n\
\t\t\t\t</Properties>\r\n\
\t\t\t\t<ChildObjects>\r\n\
\t\t\t\t\t<Parameter uuid=\"{WEB_PARAMETER_IN_UUID}\"><Properties><Name>InterfaceName</Name><Synonym/><Comment/><XDTOValueType>xs:string</XDTOValueType><Nillable>false</Nillable><TransferDirection>In</TransferDirection></Properties></Parameter>\r\n\
\t\t\t\t\t<Parameter uuid=\"{WEB_PARAMETER_OUT_UUID}\"><Properties><Name>ResultId</Name><Synonym/><Comment/><XDTOValueType>v8:UUID</XDTOValueType><Nillable>true</Nillable><TransferDirection>Out</TransferDirection></Properties></Parameter>\r\n\
\t\t\t\t\t<Parameter uuid=\"{WEB_PARAMETER_IN_OUT_UUID}\"><Properties><Name>Payload</Name><Synonym/><Comment/><XDTOValueType>svc:Input</XDTOValueType><Nillable>false</Nillable><TransferDirection>InOut</TransferDirection></Properties></Parameter>\r\n\
\t\t\t\t</ChildObjects>\r\n\
\t\t\t</Operation>\r\n\
\t\t</ChildObjects>\r\n\
\t</WebService>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    fn ws_reference_xml(version: &str, location_url: &str) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" version=\"{version}\">\r\n\
\t<WSReference uuid=\"{WS_UUID}\">\r\n\
\t\t<InternalInfo><xr:GeneratedType name=\"WSReferenceManager.UpdateFilesApiImplService\" category=\"Manager\"><xr:TypeId>{WS_MANAGER_TYPE_ID}</xr:TypeId><xr:ValueId>{WS_MANAGER_VALUE_ID}</xr:ValueId></xr:GeneratedType></InternalInfo>\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>UpdateFilesApiImplService</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Update files api impl service</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/><LocationURL>{location_url}</LocationURL>\r\n\
\t\t</Properties>\r\n\
\t</WSReference>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    fn integration_service_xml(
        version: &str,
        direction: &str,
        handler: &str,
        transactioned: &str,
    ) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" version=\"{version}\">\r\n\
\t<IntegrationService uuid=\"{INTEGRATION_UUID}\">\r\n\
\t\t<InternalInfo><xr:GeneratedType name=\"IntegrationServiceManager.MessageExchange\" category=\"Manager\"><xr:TypeId>{INTEGRATION_MANAGER_TYPE_ID}</xr:TypeId><xr:ValueId>{INTEGRATION_MANAGER_VALUE_ID}</xr:ValueId></xr:GeneratedType></InternalInfo>\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>MessageExchange</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>en</v8:lang><v8:content>Message exchange</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/><ExternalIntegrationServiceAddress/>\r\n\
\t\t</Properties>\r\n\
\t\t<ChildObjects>\r\n\
\t\t\t<IntegrationServiceChannel uuid=\"{INTEGRATION_CHANNEL_UUID}\">\r\n\
\t\t\t\t<InternalInfo><xr:GeneratedType name=\"IntegrationServiceChannelManager.MessageExchange.input\" category=\"Manager\"><xr:TypeId>{INTEGRATION_CHANNEL_TYPE_ID}</xr:TypeId><xr:ValueId>{INTEGRATION_CHANNEL_VALUE_ID}</xr:ValueId></xr:GeneratedType></InternalInfo>\r\n\
\t\t\t\t<Properties>\r\n\
\t\t\t\t\t<Name>input</Name><Synonym/><Comment/>\r\n\
\t\t\t\t\t<ExternalIntegrationServiceChannelName>external.input</ExternalIntegrationServiceChannelName>\r\n\
\t\t\t\t\t<MessageDirection>{direction}</MessageDirection>\r\n\
\t\t\t\t\t<ReceiveMessageProcessing>{handler}</ReceiveMessageProcessing>\r\n\
\t\t\t\t\t<Transactioned>{transactioned}</Transactioned>\r\n\
\t\t\t\t</Properties>\r\n\
\t\t\t</IntegrationServiceChannel>\r\n\
\t\t</ChildObjects>\r\n\
\t</IntegrationService>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    fn object(version: &str, uuid: &str, kind: &str, name: &str) -> CanonicalObject {
        let path = ObjectPath::new(vec![
            PathSegment::name(&format!(
                "{}-{}",
                kind.to_ascii_lowercase(),
                name.to_ascii_lowercase()
            ))
            .unwrap(),
        ])
        .unwrap();
        let provenance = SourceProvenance::new(
            ProfileId::parse(&format!("xml-{version}")).unwrap(),
            CanonicalAnchor::new(path.clone(), PropertyPath::root()),
        );
        let mut parts = CanonicalObjectParts::new(
            LogicalIdentity::new(ObjectUuid::parse(uuid).unwrap(), path),
            MetadataKind::new(kind).unwrap(),
            provenance,
        );
        parts.properties.push(
            CanonicalField::named(
                "Name",
                CanonicalValue::text(CanonicalText::new(name).unwrap()),
            )
            .unwrap(),
        );
        CanonicalObject::new(parts).unwrap()
    }

    fn object_with_generated_type(
        version: &str,
        uuid: &str,
        kind: &str,
        name: &str,
        type_id: &str,
        generated_kind: &str,
    ) -> CanonicalObject {
        let mut object = object(version, uuid, kind, name);
        let path = object.identity().path().clone();
        let provenance = object.provenance().clone();
        let mut parts = CanonicalObjectParts::new(
            LogicalIdentity::new(object.identity().uuid(), path),
            object.kind().clone(),
            provenance,
        );
        parts.properties = object.properties().to_vec();
        parts.generated_types.push(GeneratedType::new(
            ObjectUuid::parse(type_id).unwrap(),
            GeneratedTypeKind::new(generated_kind).unwrap(),
        ));
        object = CanonicalObject::new(parts).unwrap();
        object
    }

    fn configuration(version: &str, method_module: &str) -> CanonicalConfiguration {
        let document = XmlReader::from_slice(&xml(version, method_module)).unwrap();
        let job = bundled_metadata_registry()
            .decode(
                &FamilyId::parse("ScheduledJob").unwrap(),
                &document,
                ProfileId::parse(&format!("xml-{version}")).unwrap(),
                ObjectPath::root(),
            )
            .unwrap()
            .root()
            .clone();
        CanonicalConfiguration::new(vec![
            object(version, CONFIGURATION_UUID, "Configuration", "Fixture"),
            object(
                version,
                MODULE_UUID,
                "CommonModule",
                "РаботаСКурсамиВалютЛокализация",
            ),
            job,
        ])
        .unwrap()
    }

    fn event_configuration(
        version: &str,
        method_module: &str,
        first_source: &str,
    ) -> CanonicalConfiguration {
        let document =
            XmlReader::from_slice(&event_xml(version, method_module, first_source)).unwrap();
        let event = bundled_metadata_registry()
            .decode(
                &FamilyId::parse("EventSubscription").unwrap(),
                &document,
                ProfileId::parse(&format!("xml-{version}")).unwrap(),
                ObjectPath::root(),
            )
            .unwrap()
            .root()
            .clone();
        CanonicalConfiguration::new(vec![
            object(version, CONFIGURATION_UUID, "Configuration", "Fixture"),
            object(
                version,
                EVENT_MODULE_UUID,
                "CommonModule",
                "ВариантыОтчетов",
            ),
            object_with_generated_type(
                version,
                "22222222-2222-4222-8222-222222222222",
                "Catalog",
                "ИдентификаторыОбъектовРасширений",
                SOURCE_ONE_TYPE_ID,
                "Object",
            ),
            object_with_generated_type(
                version,
                "33333333-3333-4333-8333-333333333333",
                "Catalog",
                "ИдентификаторыОбъектовМетаданных",
                SOURCE_TWO_TYPE_ID,
                "Object",
            ),
            event,
        ])
        .unwrap()
    }

    fn xdto_configuration(version: &str, namespace: &str) -> CanonicalConfiguration {
        let document = XmlReader::from_slice(&xdto_xml(version, namespace)).unwrap();
        let package = bundled_metadata_registry()
            .decode(
                &FamilyId::parse("XDTOPackage").unwrap(),
                &document,
                ProfileId::parse(&format!("xml-{version}")).unwrap(),
                ObjectPath::root(),
            )
            .unwrap()
            .root()
            .clone();
        CanonicalConfiguration::new(vec![
            object(version, CONFIGURATION_UUID, "Configuration", "Fixture"),
            package,
        ])
        .unwrap()
    }

    fn http_configuration(version: &str, method: &str) -> CanonicalConfiguration {
        let document = XmlReader::from_slice(&http_xml(version, method)).unwrap();
        let envelope = bundled_metadata_registry()
            .decode(
                &FamilyId::parse("HTTPService").unwrap(),
                &document,
                ProfileId::parse(&format!("xml-{version}")).unwrap(),
                ObjectPath::root(),
            )
            .unwrap();
        let mut objects = vec![
            object(version, CONFIGURATION_UUID, "Configuration", "Fixture"),
            envelope.root().clone(),
        ];
        objects.extend(envelope.descendants().iter().cloned());
        CanonicalConfiguration::new(objects).unwrap()
    }

    fn web_configuration(version: &str, lock_mode: &str) -> CanonicalConfiguration {
        let package_document = XmlReader::from_slice(&xdto_xml(
            version,
            "http://www.1c.ru/SaaS/ExchangeAdministration/Common/2.4.5.1",
        ))
        .unwrap();
        let package = bundled_metadata_registry()
            .decode(
                &FamilyId::parse("XDTOPackage").unwrap(),
                &package_document,
                ProfileId::parse(&format!("xml-{version}")).unwrap(),
                ObjectPath::new(vec![PathSegment::name("xdto-package").unwrap()]).unwrap(),
            )
            .unwrap()
            .root()
            .clone();
        let web_document = XmlReader::from_slice(&web_xml(version, lock_mode)).unwrap();
        let envelope = bundled_metadata_registry()
            .decode(
                &FamilyId::parse("WebService").unwrap(),
                &web_document,
                ProfileId::parse(&format!("xml-{version}")).unwrap(),
                ObjectPath::new(vec![PathSegment::name("web-service").unwrap()]).unwrap(),
            )
            .unwrap();
        let mut objects = vec![
            object(version, CONFIGURATION_UUID, "Configuration", "Fixture"),
            package,
            envelope.root().clone(),
        ];
        objects.extend(envelope.descendants().iter().cloned());
        CanonicalConfiguration::new(objects).unwrap()
    }

    fn ws_reference_configuration(version: &str, location_url: &str) -> CanonicalConfiguration {
        let document = XmlReader::from_slice(&ws_reference_xml(version, location_url)).unwrap();
        let reference = bundled_metadata_registry()
            .decode(
                &FamilyId::parse("WSReference").unwrap(),
                &document,
                ProfileId::parse(&format!("xml-{version}")).unwrap(),
                ObjectPath::root(),
            )
            .unwrap()
            .root()
            .clone();
        CanonicalConfiguration::new(vec![
            object(version, CONFIGURATION_UUID, "Configuration", "Fixture"),
            reference,
        ])
        .unwrap()
    }

    fn integration_service_configuration(
        version: &str,
        direction: &str,
        handler: &str,
        transactioned: &str,
    ) -> CanonicalConfiguration {
        let document = XmlReader::from_slice(&integration_service_xml(
            version,
            direction,
            handler,
            transactioned,
        ))
        .unwrap();
        let envelope = bundled_metadata_registry()
            .decode(
                &FamilyId::parse("IntegrationService").unwrap(),
                &document,
                ProfileId::parse(&format!("xml-{version}")).unwrap(),
                ObjectPath::root(),
            )
            .unwrap();
        let mut objects = vec![
            object(version, CONFIGURATION_UUID, "Configuration", "Fixture"),
            envelope.root().clone(),
        ];
        objects.extend(envelope.descendants().iter().cloned());
        CanonicalConfiguration::new(objects).unwrap()
    }

    fn axes(version: &str) -> CompileAxes {
        CompileAxes::new(
            XmlDialect::parse(version).unwrap(),
            Some(PlatformBuild::parse("8.3.27.1989").unwrap()),
            None,
            StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            None,
        )
    }

    fn graph<'a>(
        validated: &ValidatedConfiguration<'a>,
    ) -> (BootstrapGraph, ServiceMetadataProfile) {
        let identities = collect_bootstrap_identities(validated).unwrap();
        let graph = build_bootstrap_graph(
            &identities,
            ProfileId::parse("platform-test").unwrap(),
            [CONFIGURATION_UUID, MODULE_UUID, JOB_UUID]
                .into_iter()
                .map(|uuid| {
                    ObjectStorageRoute::new(ObjectUuid::parse(uuid).unwrap(), Vec::new()).unwrap()
                })
                .collect(),
        )
        .unwrap();
        (
            graph,
            ServiceMetadataProfile::scheduled_job_fixture("platform-test"),
        )
    }

    fn event_graph<'a>(
        validated: &ValidatedConfiguration<'a>,
    ) -> (BootstrapGraph, ServiceMetadataProfile) {
        let identities = collect_bootstrap_identities(validated).unwrap();
        let graph = build_bootstrap_graph(
            &identities,
            ProfileId::parse("platform-test").unwrap(),
            [
                CONFIGURATION_UUID,
                EVENT_MODULE_UUID,
                "22222222-2222-4222-8222-222222222222",
                "33333333-3333-4333-8333-333333333333",
                EVENT_UUID,
            ]
            .into_iter()
            .map(|uuid| {
                ObjectStorageRoute::new(ObjectUuid::parse(uuid).unwrap(), Vec::new()).unwrap()
            })
            .collect(),
        )
        .unwrap();
        (
            graph,
            ServiceMetadataProfile::event_subscription_fixture("platform-test"),
        )
    }

    fn xdto_graph<'a>(
        validated: &ValidatedConfiguration<'a>,
    ) -> (BootstrapGraph, ServiceMetadataProfile) {
        let identities = collect_bootstrap_identities(validated).unwrap();
        let graph = build_bootstrap_graph(
            &identities,
            ProfileId::parse("platform-test").unwrap(),
            [CONFIGURATION_UUID, XDTO_UUID]
                .into_iter()
                .map(|uuid| {
                    ObjectStorageRoute::new(ObjectUuid::parse(uuid).unwrap(), Vec::new()).unwrap()
                })
                .collect(),
        )
        .unwrap();
        (
            graph,
            ServiceMetadataProfile::xdto_package_fixture("platform-test"),
        )
    }

    fn http_graph<'a>(
        validated: &ValidatedConfiguration<'a>,
    ) -> (BootstrapGraph, ServiceMetadataProfile) {
        let identities = collect_bootstrap_identities(validated).unwrap();
        let graph = build_bootstrap_graph(
            &identities,
            ProfileId::parse("platform-test").unwrap(),
            [CONFIGURATION_UUID, HTTP_UUID]
                .into_iter()
                .map(|uuid| {
                    ObjectStorageRoute::new(ObjectUuid::parse(uuid).unwrap(), Vec::new()).unwrap()
                })
                .collect(),
        )
        .unwrap();
        (
            graph,
            ServiceMetadataProfile::http_service_fixture("platform-test"),
        )
    }

    fn web_graph<'a>(
        validated: &ValidatedConfiguration<'a>,
    ) -> (BootstrapGraph, ServiceMetadataProfile) {
        let identities = collect_bootstrap_identities(validated).unwrap();
        let graph = build_bootstrap_graph(
            &identities,
            ProfileId::parse("platform-test").unwrap(),
            [CONFIGURATION_UUID, XDTO_UUID, WEB_UUID]
                .into_iter()
                .map(|uuid| {
                    ObjectStorageRoute::new(ObjectUuid::parse(uuid).unwrap(), Vec::new()).unwrap()
                })
                .collect(),
        )
        .unwrap();
        (
            graph,
            ServiceMetadataProfile::web_service_fixture("platform-test"),
        )
    }

    fn ws_reference_graph<'a>(
        validated: &ValidatedConfiguration<'a>,
    ) -> (BootstrapGraph, ServiceMetadataProfile) {
        let identities = collect_bootstrap_identities(validated).unwrap();
        let graph = build_bootstrap_graph(
            &identities,
            ProfileId::parse("platform-test").unwrap(),
            [CONFIGURATION_UUID, WS_UUID]
                .into_iter()
                .map(|uuid| {
                    ObjectStorageRoute::new(ObjectUuid::parse(uuid).unwrap(), Vec::new()).unwrap()
                })
                .collect(),
        )
        .unwrap();
        (
            graph,
            ServiceMetadataProfile::ws_reference_fixture("platform-test"),
        )
    }

    fn integration_service_graph<'a>(
        validated: &ValidatedConfiguration<'a>,
    ) -> (BootstrapGraph, ServiceMetadataProfile) {
        let identities = collect_bootstrap_identities(validated).unwrap();
        let graph = build_bootstrap_graph(
            &identities,
            ProfileId::parse("platform-test").unwrap(),
            [CONFIGURATION_UUID, INTEGRATION_UUID]
                .into_iter()
                .map(|uuid| {
                    ObjectStorageRoute::new(ObjectUuid::parse(uuid).unwrap(), Vec::new()).unwrap()
                })
                .collect(),
        )
        .unwrap();
        (
            graph,
            ServiceMetadataProfile::integration_service_fixture("platform-test"),
        )
    }

    #[test]
    fn scheduled_job_roundtrips_without_a_base_for_both_dialects() {
        for version in ["2.20", "2.21"] {
            let configuration = configuration(version, "РаботаСКурсамиВалютЛокализация");
            let validated = validate_configuration(&configuration).unwrap();
            let (graph, profile) = graph(&validated);
            let uuid = ObjectUuid::parse(JOB_UUID).unwrap();
            let first =
                compile_service_metadata(&validated, &graph, uuid, &axes(version), &profile)
                    .unwrap();
            let second =
                compile_service_metadata(&validated, &graph, uuid, &axes(version), &profile)
                    .unwrap();
            assert_eq!(first, second);
            let ir = decode_scheduled_job_blob(
                first.outcome().compiled_payload().unwrap().bytes(),
                &profile,
            )
            .unwrap();
            assert_eq!(ir.module_uuid, ObjectUuid::parse(MODULE_UUID).unwrap());
            assert_eq!(ir.restart_interval_on_failure, 600);
            let modules = BTreeMap::from([(
                ObjectUuid::parse(MODULE_UUID).unwrap(),
                "CommonModule.РаботаСКурсамиВалютЛокализация".to_owned(),
            )]);
            let output = ir
                .to_xml(
                    &ProfileId::parse(&format!("xml-{version}")).unwrap(),
                    &modules,
                )
                .unwrap();
            let document = XmlReader::from_slice(&output).unwrap();
            bundled_metadata_registry()
                .decode(
                    &FamilyId::parse("ScheduledJob").unwrap(),
                    &document,
                    ProfileId::parse(&format!("xml-{version}")).unwrap(),
                    ObjectPath::root(),
                )
                .unwrap();
        }
    }

    #[test]
    fn scheduled_job_plaintext_matches_observed_golden() {
        let configuration = configuration("2.20", "РаботаСКурсамиВалютЛокализация");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = graph(&validated);
        let entry = compile_service_metadata(
            &validated,
            &graph,
            ObjectUuid::parse(JOB_UUID).unwrap(),
            &axes("2.20"),
            &profile,
        )
        .unwrap();
        let plain = inflate_bounded(entry.outcome().compiled_payload().unwrap().bytes()).unwrap();
        assert!(plain.starts_with(UTF8_BOM));
        assert_eq!(
            &plain[UTF8_BOM.len()..],
            format!(
                "{{1,\r\n{{2,\r\n{{3,\r\n{{1,0,{JOB_UUID}}},\"ЗагрузкаКурсовВалют\",\r\n{{1,\"ru\",\"Загрузка курсов валют\"}},\"\",0,0,{NIL_UUID},0}},\"\",\"\",0,1,{MODULE_UUID},\"ПриЗагрузкеАктуальныхКурсов\",10,600}},0}}"
            )
            .as_bytes()
        );
    }

    #[test]
    fn scheduled_job_unresolved_module_and_native_extra_field_fail_closed() {
        let configuration = configuration("2.20", "Missing");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = graph(&validated);
        assert!(matches!(
            compile_service_metadata(
                &validated,
                &graph,
                ObjectUuid::parse(JOB_UUID).unwrap(),
                &axes("2.20"),
                &profile,
            ),
            Err(ServiceMetadataBuildError::InvalidModel {
                reason: "MethodName CommonModule is unresolved",
                ..
            })
        ));

        let observed = format!(
            "{{1,{{2,{{3,{{1,0,{JOB_UUID}}},\"Job\",{{0}},\"\",0,0,{NIL_UUID},0}},\"\",\"\",0,1,{MODULE_UUID},\"Run\",10,600,future}},0}}"
        );
        let observed = format!("\u{feff}{observed}");
        assert!(matches!(
            parse_scheduled_job(observed.as_bytes()),
            Err(ServiceMetadataBuildError::Native(_))
        ));
    }

    #[test]
    fn event_subscription_roundtrips_without_a_base_for_both_dialects() {
        for version in ["2.20", "2.21"] {
            let configuration = event_configuration(
                version,
                "ВариантыОтчетов",
                "ИдентификаторыОбъектовРасширений",
            );
            let validated = validate_configuration(&configuration).unwrap();
            let (graph, profile) = event_graph(&validated);
            let uuid = ObjectUuid::parse(EVENT_UUID).unwrap();
            let first =
                compile_service_metadata(&validated, &graph, uuid, &axes(version), &profile)
                    .unwrap();
            let second =
                compile_service_metadata(&validated, &graph, uuid, &axes(version), &profile)
                    .unwrap();
            assert_eq!(first, second);
            let ir = decode_event_subscription_blob(
                first.outcome().compiled_payload().unwrap().bytes(),
                &profile,
            )
            .unwrap();
            assert_eq!(
                ir.source_type_ids,
                [SOURCE_ONE_TYPE_ID, SOURCE_TWO_TYPE_ID]
                    .map(|value| ObjectUuid::parse(value).unwrap())
            );
            assert_eq!(ir.event, "BeforeDelete");
            let sources = BTreeMap::from([
                (
                    ObjectUuid::parse(SOURCE_ONE_TYPE_ID).unwrap(),
                    EventSourceReference {
                        reference: "cfg:CatalogObject.ИдентификаторыОбъектовРасширений".to_owned(),
                        type_set: false,
                    },
                ),
                (
                    ObjectUuid::parse(SOURCE_TWO_TYPE_ID).unwrap(),
                    EventSourceReference {
                        reference: "cfg:CatalogObject.ИдентификаторыОбъектовМетаданных".to_owned(),
                        type_set: false,
                    },
                ),
            ]);
            let modules = BTreeMap::from([(
                ObjectUuid::parse(EVENT_MODULE_UUID).unwrap(),
                "CommonModule.ВариантыОтчетов".to_owned(),
            )]);
            let output = ir
                .to_xml(
                    &ProfileId::parse(&format!("xml-{version}")).unwrap(),
                    &sources,
                    &modules,
                )
                .unwrap();
            let document = XmlReader::from_slice(&output).unwrap();
            bundled_metadata_registry()
                .decode(
                    &FamilyId::parse("EventSubscription").unwrap(),
                    &document,
                    ProfileId::parse(&format!("xml-{version}")).unwrap(),
                    ObjectPath::root(),
                )
                .unwrap();
        }
    }

    #[test]
    fn event_subscription_plaintext_matches_observed_golden() {
        let configuration = event_configuration(
            "2.20",
            "ВариантыОтчетов",
            "ИдентификаторыОбъектовРасширений",
        );
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = event_graph(&validated);
        let entry = compile_service_metadata(
            &validated,
            &graph,
            ObjectUuid::parse(EVENT_UUID).unwrap(),
            &axes("2.20"),
            &profile,
        )
        .unwrap();
        let plain = inflate_bounded(entry.outcome().compiled_payload().unwrap().bytes()).unwrap();
        assert!(plain.starts_with(UTF8_BOM));
        assert_eq!(
            &plain[UTF8_BOM.len()..],
            format!(
                "{{1,\r\n{{1,\r\n{{3,\r\n{{1,0,{EVENT_UUID}}},\"ВариантыОтчетовПередУдалениемИдентификатораОбъектаМетаданных\",\r\n{{1,\"ru\",\"Варианты отчетов перед удалением идентификатора объекта метаданных\"}},\"\",0,0,{NIL_UUID},0}},\r\n{{\"Pattern\",\r\n{{\"#\",{SOURCE_ONE_TYPE_ID}}},\r\n{{\"#\",{SOURCE_TWO_TYPE_ID}}}\r\n}},\"BeforeDelete_ПередУдалением\",{EVENT_MODULE_UUID},\"ПередУдалениемИдентификатораОбъектаМетаданных\"}},0}}"
            )
            .as_bytes()
        );
    }

    #[test]
    fn event_subscription_unresolved_type_and_native_extra_field_fail_closed() {
        let configuration = event_configuration("2.20", "ВариантыОтчетов", "ОтсутствующийИсточник");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = event_graph(&validated);
        assert!(matches!(
            compile_service_metadata(
                &validated,
                &graph,
                ObjectUuid::parse(EVENT_UUID).unwrap(),
                &axes("2.20"),
                &profile,
            ),
            Err(ServiceMetadataBuildError::InvalidModel {
                reason: "EventSubscription Source TypeId is unresolved",
                ..
            })
        ));
        let observed = format!(
            "{{1,{{1,{{3,{{1,0,{EVENT_UUID}}},\"Event\",{{0}},\"\",0,0,{NIL_UUID},0}},{{\"Pattern\",{{\"#\",{SOURCE_ONE_TYPE_ID}}}}},\"BeforeDelete_ПередУдалением\",{EVENT_MODULE_UUID},\"Run\",future}},0}}"
        );
        let observed = format!("\u{feff}{observed}");
        assert!(matches!(
            parse_event_subscription(observed.as_bytes()),
            Err(ServiceMetadataBuildError::Native(_))
        ));
    }

    #[test]
    fn xdto_package_roundtrips_without_a_base_for_both_dialects() {
        let namespace = "http://www.1c.ru/SaaS/ExchangeAdministration/Common/2.4.5.1";
        for version in ["2.20", "2.21"] {
            let configuration = xdto_configuration(version, namespace);
            let validated = validate_configuration(&configuration).unwrap();
            let (graph, profile) = xdto_graph(&validated);
            let uuid = ObjectUuid::parse(XDTO_UUID).unwrap();
            let first =
                compile_service_metadata(&validated, &graph, uuid, &axes(version), &profile)
                    .unwrap();
            let second =
                compile_service_metadata(&validated, &graph, uuid, &axes(version), &profile)
                    .unwrap();
            assert_eq!(first, second);
            let ir = decode_xdto_package_blob(
                first.outcome().compiled_payload().unwrap().bytes(),
                &profile,
            )
            .unwrap();
            assert_eq!(ir.namespace, namespace);
            let output = ir
                .to_xml(&ProfileId::parse(&format!("xml-{version}")).unwrap())
                .unwrap();
            let document = XmlReader::from_slice(&output).unwrap();
            bundled_metadata_registry()
                .decode(
                    &FamilyId::parse("XDTOPackage").unwrap(),
                    &document,
                    ProfileId::parse(&format!("xml-{version}")).unwrap(),
                    ObjectPath::root(),
                )
                .unwrap();
        }
    }

    #[test]
    fn xdto_package_plaintext_matches_observed_golden() {
        let namespace = "http://www.1c.ru/SaaS/ExchangeAdministration/Common/2.4.5.1";
        let configuration = xdto_configuration("2.20", namespace);
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = xdto_graph(&validated);
        let entry = compile_service_metadata(
            &validated,
            &graph,
            ObjectUuid::parse(XDTO_UUID).unwrap(),
            &axes("2.20"),
            &profile,
        )
        .unwrap();
        let plain = inflate_bounded(entry.outcome().compiled_payload().unwrap().bytes()).unwrap();
        assert!(plain.starts_with(UTF8_BOM));
        assert_eq!(
            &plain[UTF8_BOM.len()..],
            format!(
                "{{1,\r\n{{1,\r\n{{3,\r\n{{1,0,{XDTO_UUID}}},\"АдминистрированиеОбменаДанными_2_4_5_1\",\r\n{{1,\"ru\",\"Администрирование обмена данными 2.4.5.1\"}},\"\",0,0,{NIL_UUID},0}},\"{namespace}\"}},0}}"
            )
            .as_bytes()
        );
    }

    #[test]
    fn xdto_package_native_unknown_field_and_invalid_xml_profile_fail_closed() {
        let namespace = "http://www.1c.ru/SaaS/ExchangeAdministration/Common/2.4.5.1";
        let observed = format!(
            "{{1,{{1,{{3,{{1,0,{XDTO_UUID}}},\"Package\",{{0}},\"\",0,0,{NIL_UUID},0}},\"{namespace}\",future}},0}}"
        );
        let observed = format!("\u{feff}{observed}");
        assert!(matches!(
            parse_xdto_package(observed.as_bytes()),
            Err(ServiceMetadataBuildError::Native(_))
        ));
        let ir = XdtoPackageNativeIr {
            uuid: ObjectUuid::parse(XDTO_UUID).unwrap(),
            name: "Package".to_owned(),
            synonyms: Vec::new(),
            comment: String::new(),
            namespace: namespace.to_owned(),
        };
        assert!(matches!(
            ir.to_xml(&ProfileId::parse("xml-future").unwrap()),
            Err(ServiceMetadataBuildError::InvalidXmlProfile(_))
        ));
    }

    #[test]
    fn http_service_roundtrips_without_a_base_for_both_dialects() {
        for version in ["2.20", "2.21"] {
            let configuration = http_configuration(version, "GET");
            let validated = validate_configuration(&configuration).unwrap();
            let (graph, profile) = http_graph(&validated);
            let uuid = ObjectUuid::parse(HTTP_UUID).unwrap();
            let first =
                compile_service_metadata(&validated, &graph, uuid, &axes(version), &profile)
                    .unwrap();
            let second =
                compile_service_metadata(&validated, &graph, uuid, &axes(version), &profile)
                    .unwrap();
            assert_eq!(first, second);
            let ir = decode_http_service_blob(
                first.outcome().compiled_payload().unwrap().bytes(),
                &profile,
            )
            .unwrap();
            assert_eq!(ir.root_url, "billing");
            assert_eq!(ir.reuse_sessions, "AutoUse");
            assert_eq!(ir.urls.len(), 1);
            assert_eq!(ir.urls[0].methods.len(), 1);
            assert_eq!(ir.urls[0].methods[0].http_method, "GET");
            let output = ir
                .to_xml(&ProfileId::parse(&format!("xml-{version}")).unwrap())
                .unwrap();
            let document = XmlReader::from_slice(&output).unwrap();
            bundled_metadata_registry()
                .decode(
                    &FamilyId::parse("HTTPService").unwrap(),
                    &document,
                    ProfileId::parse(&format!("xml-{version}")).unwrap(),
                    ObjectPath::root(),
                )
                .unwrap();
        }
    }

    #[test]
    fn http_service_plaintext_matches_observed_golden() {
        let configuration = http_configuration("2.20", "GET");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = http_graph(&validated);
        let entry = compile_service_metadata(
            &validated,
            &graph,
            ObjectUuid::parse(HTTP_UUID).unwrap(),
            &axes("2.20"),
            &profile,
        )
        .unwrap();
        let plain = inflate_bounded(entry.outcome().compiled_payload().unwrap().bytes()).unwrap();
        assert!(plain.starts_with(UTF8_BOM));
        assert_eq!(
            &plain[UTF8_BOM.len()..],
            format!(
                "{{1,\r\n{{2,\"billing\",\r\n{{3,\r\n{{1,0,{HTTP_UUID}}},\"Биллинг\",\r\n{{1,\"ru\",\"Биллинг\"}},\"\",0,0,{NIL_UUID},0}},2,20}},1,\r\n{{{HTTP_URL_COLLECTION_UUID},1,\r\n{{\r\n{{0,\"/version\",\r\n{{3,\r\n{{1,0,{HTTP_URL_UUID}}},\"Версия\",\r\n{{0}},\"\",0,0,{NIL_UUID},0}}\r\n}},1,\r\n{{{HTTP_METHOD_COLLECTION_UUID},1,\r\n{{\r\n{{0,\"ВерсияПолучить\",3,\r\n{{3,\r\n{{1,0,{HTTP_METHOD_UUID}}},\"Получить\",\r\n{{0}},\"\",0,0,{NIL_UUID},0}}\r\n}},0}}\r\n}}\r\n}}\r\n}}\r\n}}"
            )
            .as_bytes()
        );
    }

    #[test]
    fn http_service_unknown_native_code_and_extra_field_fail_closed() {
        let configuration = http_configuration("2.20", "GET");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = http_graph(&validated);
        let entry = compile_service_metadata(
            &validated,
            &graph,
            ObjectUuid::parse(HTTP_UUID).unwrap(),
            &axes("2.20"),
            &profile,
        )
        .unwrap();
        let plain = String::from_utf8(
            inflate_bounded(entry.outcome().compiled_payload().unwrap().bytes()).unwrap(),
        )
        .unwrap();
        let unknown_code = plain.replacen("\"ВерсияПолучить\",3,", "\"ВерсияПолучить\",99,", 1);
        assert!(matches!(
            parse_http_service(unknown_code.as_bytes()),
            Err(ServiceMetadataBuildError::Native(_))
        ));
        let extra_field = format!("{},future}}", &plain[..plain.len() - 1]);
        assert!(matches!(
            parse_http_service(extra_field.as_bytes()),
            Err(ServiceMetadataBuildError::Native(_))
        ));
    }

    #[test]
    fn web_service_roundtrips_without_a_base_for_both_dialects() {
        for version in ["2.20", "2.21"] {
            let configuration = web_configuration(version, "Managed");
            let validated = validate_configuration(&configuration).unwrap();
            let (graph, profile) = web_graph(&validated);
            let uuid = ObjectUuid::parse(WEB_UUID).unwrap();
            let first =
                compile_service_metadata(&validated, &graph, uuid, &axes(version), &profile)
                    .unwrap();
            let second =
                compile_service_metadata(&validated, &graph, uuid, &axes(version), &profile)
                    .unwrap();
            assert_eq!(first, second);
            let ir = decode_web_service_blob(
                first.outcome().compiled_payload().unwrap().bytes(),
                &profile,
            )
            .unwrap();
            assert_eq!(
                ir.xdto_package_uuids,
                [ObjectUuid::parse(XDTO_UUID).unwrap()]
            );
            assert_eq!(ir.xdto_namespaces, [XDTO_CORE_NAMESPACE]);
            assert_eq!(ir.operations.len(), 1);
            assert_eq!(ir.operations[0].parameters.len(), 3);
            assert_eq!(
                ir.operations[0]
                    .parameters
                    .iter()
                    .map(|parameter| parameter.transfer_direction.as_str())
                    .collect::<Vec<_>>(),
                ["In", "Out", "InOut"]
            );
            let packages = BTreeMap::from([(
                ObjectUuid::parse(XDTO_UUID).unwrap(),
                "XDTOPackage.АдминистрированиеОбменаДанными_2_4_5_1".to_owned(),
            )]);
            let output = ir
                .to_xml(
                    &ProfileId::parse(&format!("xml-{version}")).unwrap(),
                    &packages,
                )
                .unwrap();
            assert!(
                String::from_utf8_lossy(&output)
                    .contains("xmlns:ws1=\"http://example.test/types\"")
            );
            let document = XmlReader::from_slice(&output).unwrap();
            bundled_metadata_registry()
                .decode(
                    &FamilyId::parse("WebService").unwrap(),
                    &document,
                    ProfileId::parse(&format!("xml-{version}")).unwrap(),
                    ObjectPath::root(),
                )
                .unwrap();
        }
    }

    #[test]
    fn web_service_plaintext_matches_observed_remote_control_golden() {
        let value = WebServiceNativeIr {
            uuid: ObjectUuid::parse("03d08c14-f814-4e12-8f96-020c36cca2bf").unwrap(),
            name: "RemoteControl".to_owned(),
            synonyms: vec![ServiceLocalizedString {
                language: "ru".to_owned(),
                content: "Remote control".to_owned(),
            }],
            comment: String::new(),
            namespace: "http://www.1c.ru/SSL/RemoteControl_1_0_0_1".to_owned(),
            xdto_package_uuids: Vec::new(),
            xdto_namespaces: Vec::new(),
            descriptor_file_name: "RemoteControl.1cws".to_owned(),
            reuse_sessions: "DontUse".to_owned(),
            session_max_age: 20,
            operations: vec![WebServiceOperationNativeIr {
                uuid: ObjectUuid::parse("a1a7aab6-cf35-42ce-97bb-c112638a2748").unwrap(),
                name: "GetCurrentState".to_owned(),
                synonyms: vec![ServiceLocalizedString {
                    language: "ru".to_owned(),
                    content: "Get current state".to_owned(),
                }],
                comment: String::new(),
                returning_value_type: WebServiceXdtoTypeNativeIr {
                    namespace: XDTO_XML_SCHEMA_NAMESPACE.to_owned(),
                    name: "boolean".to_owned(),
                },
                nillable: false,
                transactioned: false,
                procedure_name: "GetCurrentState".to_owned(),
                data_lock_control_mode: "Managed".to_owned(),
                parameters: Vec::new(),
            }],
        };
        let plain = serialize_web_service(&value);
        assert!(plain.starts_with(UTF8_BOM));
        assert_eq!(
            &plain[UTF8_BOM.len()..],
            concat!(
                "{1,\r\n{4,\"http://www.1c.ru/SSL/RemoteControl_1_0_0_1\",\r\n",
                "{3,\r\n{1,0,03d08c14-f814-4e12-8f96-020c36cca2bf},\"RemoteControl\",\r\n",
                "{1,\"ru\",\"Remote control\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0},\r\n",
                "{0,0},\"RemoteControl.1cws\",\r\n{0},0,20},1,\r\n",
                "{36186084-c23a-43bd-876c-a3a8ba1a9622,1,\r\n{\r\n{1,\r\n",
                "{3,\r\n{1,0,a1a7aab6-cf35-42ce-97bb-c112638a2748},\"GetCurrentState\",\r\n",
                "{1,\"ru\",\"Get current state\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0},\r\n",
                "{0,\"http://www.w3.org/2001/XMLSchema\",\"boolean\"},0,0,\"GetCurrentState\",1},1,\r\n",
                "{b78a00b2-2260-4ef5-a70c-17889cfee695,0}\r\n}\r\n}\r\n}"
            )
            .as_bytes()
        );
        assert_eq!(parse_web_service(&plain).unwrap(), value);
    }

    #[test]
    fn web_service_unknown_native_code_extra_field_and_reference_fail_closed() {
        let configuration = web_configuration("2.20", "Managed");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = web_graph(&validated);
        let entry = compile_service_metadata(
            &validated,
            &graph,
            ObjectUuid::parse(WEB_UUID).unwrap(),
            &axes("2.20"),
            &profile,
        )
        .unwrap();
        let plain = String::from_utf8(
            inflate_bounded(entry.outcome().compiled_payload().unwrap().bytes()).unwrap(),
        )
        .unwrap();
        let unknown_lock = plain.replacen("\"GetVersions\",1},1,", "\"GetVersions\",99},1,", 1);
        assert!(matches!(
            parse_web_service(unknown_lock.as_bytes()),
            Err(ServiceMetadataBuildError::Native(_))
        ));
        let wrong_reference_class = plain.replacen(
            DESIGN_TIME_REFERENCE_CLASS_UUID,
            "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
            1,
        );
        assert!(matches!(
            parse_web_service(wrong_reference_class.as_bytes()),
            Err(ServiceMetadataBuildError::Native(_))
        ));
        let extra_field = format!("{},future}}", &plain[..plain.len() - 1]);
        assert!(matches!(
            parse_web_service(extra_field.as_bytes()),
            Err(ServiceMetadataBuildError::Native(_))
        ));
    }

    #[test]
    fn ws_reference_roundtrips_without_a_base_for_both_dialects() {
        let location = "https://update-api.1c.ru/ws/files?wsdl";
        for version in ["2.20", "2.21"] {
            let configuration = ws_reference_configuration(version, location);
            let validated = validate_configuration(&configuration).unwrap();
            let (graph, profile) = ws_reference_graph(&validated);
            let uuid = ObjectUuid::parse(WS_UUID).unwrap();
            let first =
                compile_service_metadata(&validated, &graph, uuid, &axes(version), &profile)
                    .unwrap();
            let second =
                compile_service_metadata(&validated, &graph, uuid, &axes(version), &profile)
                    .unwrap();
            assert_eq!(first, second);
            let ir = decode_ws_reference_blob(
                first.outcome().compiled_payload().unwrap().bytes(),
                &profile,
            )
            .unwrap();
            assert_eq!(ir.location_url, location);
            assert_eq!(
                ir.manager_type_id,
                ObjectUuid::parse(WS_MANAGER_TYPE_ID).unwrap()
            );
            assert_eq!(
                ir.manager_value_id,
                ObjectUuid::parse(WS_MANAGER_VALUE_ID).unwrap()
            );
            let output = ir
                .to_xml(&ProfileId::parse(&format!("xml-{version}")).unwrap())
                .unwrap();
            let document = XmlReader::from_slice(&output).unwrap();
            bundled_metadata_registry()
                .decode(
                    &FamilyId::parse("WSReference").unwrap(),
                    &document,
                    ProfileId::parse(&format!("xml-{version}")).unwrap(),
                    ObjectPath::root(),
                )
                .unwrap();
        }
    }

    #[test]
    fn ws_reference_plaintext_matches_repository_native_fixture() {
        let configuration =
            ws_reference_configuration("2.20", "https://update-api.1c.ru/ws/files?wsdl");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = ws_reference_graph(&validated);
        let entry = compile_service_metadata(
            &validated,
            &graph,
            ObjectUuid::parse(WS_UUID).unwrap(),
            &axes("2.20"),
            &profile,
        )
        .unwrap();
        let plain = inflate_bounded(entry.outcome().compiled_payload().unwrap().bytes()).unwrap();
        assert!(plain.starts_with(UTF8_BOM));
        assert_eq!(
            &plain[UTF8_BOM.len()..],
            format!(
                "{{1,\r\n{{2,\r\n{{\"https://update-api.1c.ru/ws/files?wsdl\",0}},\r\n{{3,\r\n{{1,0,{WS_UUID}}},\"UpdateFilesApiImplService\",\r\n{{1,\"ru\",\"Update files api impl service\"}},\"\",0,0,{NIL_UUID},0}},{WS_MANAGER_TYPE_ID},{WS_MANAGER_VALUE_ID}}},0}}"
            )
            .as_bytes()
        );
    }

    #[test]
    fn ws_reference_unknown_native_wrapper_and_extra_field_fail_closed() {
        let configuration =
            ws_reference_configuration("2.20", "https://update-api.1c.ru/ws/files?wsdl");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = ws_reference_graph(&validated);
        let entry = compile_service_metadata(
            &validated,
            &graph,
            ObjectUuid::parse(WS_UUID).unwrap(),
            &axes("2.20"),
            &profile,
        )
        .unwrap();
        let plain = String::from_utf8(
            inflate_bounded(entry.outcome().compiled_payload().unwrap().bytes()).unwrap(),
        )
        .unwrap();
        let unknown_location_tail = plain.replacen("?wsdl\",0}", "?wsdl\",1}", 1);
        assert!(matches!(
            parse_ws_reference(unknown_location_tail.as_bytes()),
            Err(ServiceMetadataBuildError::Native(_))
        ));
        let extra_field = plain.replacen(
            &format!(",{WS_MANAGER_VALUE_ID}}},0}}"),
            &format!(",{WS_MANAGER_VALUE_ID},future}},0}}"),
            1,
        );
        assert!(matches!(
            parse_ws_reference(extra_field.as_bytes()),
            Err(ServiceMetadataBuildError::Native(_))
        ));
    }

    #[test]
    fn integration_service_roundtrips_without_a_base_for_both_dialects() {
        for version in ["2.20", "2.21"] {
            for (direction, handler, transactioned) in
                [("Receive", "HandleInput", "false"), ("Send", "", "true")]
            {
                let configuration =
                    integration_service_configuration(version, direction, handler, transactioned);
                let validated = validate_configuration(&configuration).unwrap();
                let (graph, profile) = integration_service_graph(&validated);
                let uuid = ObjectUuid::parse(INTEGRATION_UUID).unwrap();
                let first =
                    compile_service_metadata(&validated, &graph, uuid, &axes(version), &profile)
                        .unwrap();
                let second =
                    compile_service_metadata(&validated, &graph, uuid, &axes(version), &profile)
                        .unwrap();
                assert_eq!(first, second);
                let ir = decode_integration_service_blob(
                    first.outcome().compiled_payload().unwrap().bytes(),
                    &profile,
                )
                .unwrap();
                assert_eq!(ir.channels.len(), 1);
                assert_eq!(ir.channels[0].message_direction, direction);
                assert_eq!(ir.channels[0].receive_message_processing, handler);
                assert_eq!(ir.channels[0].transactioned, transactioned == "true");
                let output = ir
                    .to_xml(&ProfileId::parse(&format!("xml-{version}")).unwrap())
                    .unwrap();
                let document = XmlReader::from_slice(&output).unwrap();
                bundled_metadata_registry()
                    .decode(
                        &FamilyId::parse("IntegrationService").unwrap(),
                        &document,
                        ProfileId::parse(&format!("xml-{version}")).unwrap(),
                        ObjectPath::root(),
                    )
                    .unwrap();
            }
        }
    }

    #[test]
    fn integration_service_plaintext_matches_repository_native_fixture() {
        let configuration =
            integration_service_configuration("2.20", "Receive", "HandleInput", "false");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = integration_service_graph(&validated);
        let entry = compile_service_metadata(
            &validated,
            &graph,
            ObjectUuid::parse(INTEGRATION_UUID).unwrap(),
            &axes("2.20"),
            &profile,
        )
        .unwrap();
        let plain = inflate_bounded(entry.outcome().compiled_payload().unwrap().bytes()).unwrap();
        assert!(plain.starts_with(UTF8_BOM));
        assert_eq!(
            &plain[UTF8_BOM.len()..],
            format!(
                "{{1,\r\n{{0,\r\n{{3,\r\n{{1,0,{INTEGRATION_UUID}}},\"MessageExchange\",{{1,\"en\",\"Message exchange\"}},\"\",0,0,{NIL_UUID},0}},{INTEGRATION_MANAGER_TYPE_ID},{INTEGRATION_MANAGER_VALUE_ID},\"\"}},1,\r\n{{{INTEGRATION_CHANNEL_COLLECTION_UUID},1,\r\n{{\r\n{{\r\n{{1,\r\n{{3,\r\n{{1,0,{INTEGRATION_CHANNEL_UUID}}},\"input\",{{0}},\"\",0,0,{NIL_UUID},0}},{INTEGRATION_CHANNEL_TYPE_ID},{INTEGRATION_CHANNEL_VALUE_ID},\"external.input\",\"HandleInput\",1,0}},0}}\r\n}}\r\n}}\r\n}}"
            )
            .as_bytes()
        );
    }

    #[test]
    fn integration_service_unknown_native_shells_fail_closed() {
        let configuration =
            integration_service_configuration("2.20", "Receive", "HandleInput", "false");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = integration_service_graph(&validated);
        let entry = compile_service_metadata(
            &validated,
            &graph,
            ObjectUuid::parse(INTEGRATION_UUID).unwrap(),
            &axes("2.20"),
            &profile,
        )
        .unwrap();
        let plain = String::from_utf8(
            inflate_bounded(entry.outcome().compiled_payload().unwrap().bytes()).unwrap(),
        )
        .unwrap();
        let unknown_collection = plain.replacen(
            INTEGRATION_CHANNEL_COLLECTION_UUID,
            "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
            1,
        );
        assert!(matches!(
            parse_integration_service(unknown_collection.as_bytes()),
            Err(ServiceMetadataBuildError::Native(_))
        ));
        let unknown_direction =
            plain.replacen("\"HandleInput\",1,0},0}", "\"HandleInput\",9,0},0}", 1);
        assert!(matches!(
            parse_integration_service(unknown_direction.as_bytes()),
            Err(ServiceMetadataBuildError::Native(_))
        ));
        let wrong_wrapper_tail =
            plain.replacen("\"HandleInput\",1,0},0}", "\"HandleInput\",1,0},1}", 1);
        assert!(matches!(
            parse_integration_service(wrong_wrapper_tail.as_bytes()),
            Err(ServiceMetadataBuildError::Native(_))
        ));
        let extra_field = plain.replacen(
            "\"HandleInput\",1,0},0}",
            "\"HandleInput\",1,0,future},0}",
            1,
        );
        assert!(matches!(
            parse_integration_service(extra_field.as_bytes()),
            Err(ServiceMetadataBuildError::Native(_))
        ));
    }

    #[test]
    fn all_service_profiles_are_explicit() {
        let json = format!(
            r#"{{
                "schema_version": 1,
                "id": "platform-test",
                "status": "experimental",
                "platform_build": "8.3.27.1989",
                "storage_profile": "{SUPPORTED_STORAGE_PROFILE}",
                "constants": {{
                    "{SCHEDULED_JOB_LAYOUT_KEY}": "{SCHEDULED_JOB_LAYOUT}",
                    "{EVENT_SUBSCRIPTION_LAYOUT_KEY}": "{EVENT_SUBSCRIPTION_LAYOUT}",
                    "{XDTO_PACKAGE_LAYOUT_KEY}": "{XDTO_PACKAGE_LAYOUT}",
                    "{HTTP_SERVICE_LAYOUT_KEY}": "{HTTP_SERVICE_LAYOUT}",
                    "{WEB_SERVICE_LAYOUT_KEY}": "{WEB_SERVICE_LAYOUT}",
                    "{WS_REFERENCE_LAYOUT_KEY}": "{WS_REFERENCE_LAYOUT}",
                    "{INTEGRATION_SERVICE_LAYOUT_KEY}": "{INTEGRATION_SERVICE_LAYOUT}"
                }}
            }}"#
        );
        let source =
            parse_profile_source("services.json", ProfileSourceKind::Bundled, &json).unwrap();
        let registry = resolve_profiles([source]).unwrap();
        let effective = registry
            .get(&ProfileId::parse("platform-test").unwrap())
            .unwrap();
        assert_eq!(
            ServiceMetadataProfile::from_effective_for_family(
                effective,
                ServiceFamily::ScheduledJob
            )
            .unwrap()
            .family(),
            ServiceFamily::ScheduledJob
        );
        assert_eq!(
            ServiceMetadataProfile::from_effective_for_family(
                effective,
                ServiceFamily::EventSubscription
            )
            .unwrap()
            .family(),
            ServiceFamily::EventSubscription
        );
        assert_eq!(
            ServiceMetadataProfile::from_effective_for_family(
                effective,
                ServiceFamily::XdtoPackage,
            )
            .unwrap()
            .family(),
            ServiceFamily::XdtoPackage
        );
        assert_eq!(
            ServiceMetadataProfile::from_effective_for_family(
                effective,
                ServiceFamily::HttpService,
            )
            .unwrap()
            .family(),
            ServiceFamily::HttpService
        );
        assert_eq!(
            ServiceMetadataProfile::from_effective_for_family(
                effective,
                ServiceFamily::WebService,
            )
            .unwrap()
            .family(),
            ServiceFamily::WebService
        );
        assert_eq!(
            ServiceMetadataProfile::from_effective_for_family(
                effective,
                ServiceFamily::WsReference,
            )
            .unwrap()
            .family(),
            ServiceFamily::WsReference
        );
        assert_eq!(
            ServiceMetadataProfile::from_effective_for_family(
                effective,
                ServiceFamily::IntegrationService,
            )
            .unwrap()
            .family(),
            ServiceFamily::IntegrationService
        );
    }
}
