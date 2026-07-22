//! Shared, profile-gated native codec for Catalog and Document metadata.
//!
//! The public family entry points live in `catalog` and `document`.  This
//! module owns only the evidenced 8.3.27 layout and deliberately has no
//! storage-reader or base-artifact dependency.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
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
use ibcmd_core::value::{CanonicalField, CanonicalValue, CanonicalValueKind};
use ibcmd_core::version::PlatformBuild;

use super::super::CompileAxes;
use super::super::graph::BootstrapGraph;

pub(crate) const CATALOG_LAYOUT_KEY: &str = "bootstrap.metadata.catalog.layout";
pub(crate) const CATALOG_LAYOUT: &str = "catalog-v1-crlf-utf8-bom";
pub(crate) const DOCUMENT_LAYOUT_KEY: &str = "bootstrap.metadata.document.layout";
pub(crate) const DOCUMENT_LAYOUT: &str = "document-v1-crlf-utf8-bom";
pub(crate) const SUBSYSTEM_LAYOUT_KEY: &str = "bootstrap.metadata.subsystem.layout";
pub(crate) const SUBSYSTEM_LAYOUT: &str = "subsystem-v1-crlf-utf8-bom";
pub(crate) const EXCHANGE_PLAN_LAYOUT_KEY: &str = "bootstrap.metadata.exchange_plan.layout";
pub(crate) const EXCHANGE_PLAN_LAYOUT: &str = "exchange-plan-v1-crlf-utf8-bom";
pub(crate) const BUSINESS_PROCESS_LAYOUT_KEY: &str = "bootstrap.metadata.business_process.layout";
pub(crate) const BUSINESS_PROCESS_LAYOUT: &str = "business-process-v1-crlf-utf8-bom";
pub(crate) const TASK_LAYOUT_KEY: &str = "bootstrap.metadata.task.layout";
pub(crate) const TASK_LAYOUT: &str = "task-v1-crlf-utf8-bom";
const SUPPORTED_STORAGE_PROFILE: &str = "storage:mssql-config-configsave";
const UTF8_BOM: &[u8; 3] = b"\xef\xbb\xbf";
const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";
const METADATA_OBJECT_REF_TYPE_UUID: &str = "157fa490-4ce9-11d4-9415-008048da11f9";
const FIELD_REF_TYPE_UUID: &str = "60ea359f-3a6e-48bb-8e71-d2a457572918";
const COMMAND_VALUE_UUID: &str = "078a6af8-d22c-4248-9c33-7e90075a3d2c";

const TEMPLATE_COLLECTION_UUID: &str = "3daea016-69b7-4ed4-9453-127911372fe6";
const CATALOG_COMMAND_COLLECTION_UUID: &str = "4fe87c89-9ad4-43f6-9fdb-9dc83b3879c6";
const CATALOG_TABULAR_COLLECTION_UUID: &str = "932159f9-95b2-4e76-a8dd-8849fe5c5ded";
const CATALOG_ATTRIBUTE_COLLECTION_UUID: &str = "cf4abea7-37b2-11d4-940f-008048da11f9";
const CATALOG_FORM_COLLECTION_UUID: &str = "fdf816d2-1ead-11d5-b975-0050bae0a95d";
const DOCUMENT_TABULAR_COLLECTION_UUID: &str = "21c53e09-8950-4b5e-a6a0-1054f1bbc274";
const DOCUMENT_ATTRIBUTE_COLLECTION_UUID: &str = "45e46cbc-3e24-4165-8b7b-cc98a6f80211";
const DOCUMENT_COMMAND_COLLECTION_UUID: &str = "b544fc6a-2ba3-4885-8fb2-cb289fb6d65e";
const DOCUMENT_FORM_COLLECTION_UUID: &str = "fb880e93-47d7-4127-9357-a20e69c17545";
const CATALOG_TABULAR_ATTRIBUTE_COLLECTION_UUID: &str = "888744e1-b616-11d4-9436-004095e12fc7";
const DOCUMENT_TABULAR_ATTRIBUTE_COLLECTION_UUID: &str = "888744e1-b616-11d4-9436-004095e12fc7";
const SUBSYSTEM_COLLECTION_UUID: &str = "37f2fa9a-b276-11d4-9435-004095e12fc7";
const EXCHANGE_ATTRIBUTE_COLLECTION_UUID: &str = "1a1b4fea-e093-470d-94ff-1d2f16cda2ab";
const EXCHANGE_TABULAR_COLLECTION_UUID: &str = "52293f4b-f98c-43ea-a80f-41047ae7ab58";
const EXCHANGE_FORM_COLLECTION_UUID: &str = "87c509ab-3d38-4d67-b379-aca796298578";
const EXCHANGE_COMMAND_COLLECTION_UUID: &str = "d5207c64-11d5-4d46-bba2-55b7b07ff4eb";
const BUSINESS_PROCESS_FORM_COLLECTION_UUID: &str = "3f7a8120-b71a-4265-98bf-4d9bc09b7719";
const BUSINESS_PROCESS_COMMAND_COLLECTION_UUID: &str = "7a3e533c-f232-40d5-a932-6a311d2480bf";
const BUSINESS_PROCESS_ATTRIBUTE_COLLECTION_UUID: &str = "87c988de-ecbf-413b-87b0-b9516df05e28";
const BUSINESS_PROCESS_TABULAR_COLLECTION_UUID: &str = "a3fe6537-d787-40f7-8a06-419d2f0c1cfd";
const TASK_FORM_COLLECTION_UUID: &str = "3f58cbfb-4172-4e54-be49-561a579bb38b";
const TASK_ATTRIBUTE_COLLECTION_UUID: &str = "8ddfb495-c5fc-46b9-bdc5-bcf58341bff0";
const TASK_ADDRESSING_ATTRIBUTE_COLLECTION_UUID: &str = "e97c0570-251c-4566-b0f1-10686820f143";
const TASK_RESERVED_COLLECTION_UUID: &str = "ee865d4b-a458-48a0-b38f-5a26898feeb0";
const TASK_COMMAND_COLLECTION_UUID: &str = "f27c2152-a2c9-4c30-adb1-130f5eb2590f";

const MAX_PLAIN_BYTES: usize = 64 * 1_048_576;
const MAX_NATIVE_DEPTH: usize = 32;
const MAX_NATIVE_NODES: usize = 500_000;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BusinessObjectFamily {
    Catalog,
    Document,
    Subsystem,
    ExchangePlan,
    BusinessProcess,
    Task,
}

impl BusinessObjectFamily {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Catalog => "Catalog",
            Self::Document => "Document",
            Self::Subsystem => "Subsystem",
            Self::ExchangePlan => "ExchangePlan",
            Self::BusinessProcess => "BusinessProcess",
            Self::Task => "Task",
        }
    }

    const fn layout_key(self) -> &'static str {
        match self {
            Self::Catalog => CATALOG_LAYOUT_KEY,
            Self::Document => DOCUMENT_LAYOUT_KEY,
            Self::Subsystem => SUBSYSTEM_LAYOUT_KEY,
            Self::ExchangePlan => EXCHANGE_PLAN_LAYOUT_KEY,
            Self::BusinessProcess => BUSINESS_PROCESS_LAYOUT_KEY,
            Self::Task => TASK_LAYOUT_KEY,
        }
    }

    const fn layout_value(self) -> &'static str {
        match self {
            Self::Catalog => CATALOG_LAYOUT,
            Self::Document => DOCUMENT_LAYOUT,
            Self::Subsystem => SUBSYSTEM_LAYOUT,
            Self::ExchangePlan => EXCHANGE_PLAN_LAYOUT,
            Self::BusinessProcess => BUSINESS_PROCESS_LAYOUT,
            Self::Task => TASK_LAYOUT,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BusinessObjectLayout {
    CatalogV1,
    DocumentV1,
    SubsystemV1,
    ExchangePlanV1,
    BusinessProcessV1,
    TaskV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BusinessObjectMetadataProfile {
    profile_id: ProfileId,
    platform_build: PlatformBuild,
    storage_profile: StorageProfileId,
    family: BusinessObjectFamily,
    layout: BusinessObjectLayout,
}

impl BusinessObjectMetadataProfile {
    pub(crate) fn from_effective(
        profile: &EffectiveProfile,
        family: BusinessObjectFamily,
    ) -> Result<Self, BusinessObjectProfileError> {
        let platform_build = profile
            .platform_build
            .as_ref()
            .map(|value| value.value.clone())
            .ok_or_else(|| BusinessObjectProfileError::MissingCoordinate {
                profile: profile.id.clone(),
                coordinate: "platform_build",
            })?;
        let storage_profile = profile
            .storage_profile
            .as_ref()
            .map(|value| value.value.clone())
            .ok_or_else(|| BusinessObjectProfileError::MissingCoordinate {
                profile: profile.id.clone(),
                coordinate: "storage_profile",
            })?;
        if storage_profile.as_str() != SUPPORTED_STORAGE_PROFILE {
            return Err(BusinessObjectProfileError::UnsupportedCoordinate {
                profile: profile.id.clone(),
                coordinate: "storage_profile",
                value: storage_profile.to_string(),
            });
        }
        let constant = profile.constants.get(family.layout_key()).ok_or_else(|| {
            BusinessObjectProfileError::MissingConstant {
                profile: profile.id.clone(),
                key: family.layout_key(),
            }
        })?;
        if constant.value != family.layout_value() {
            return Err(BusinessObjectProfileError::UnsupportedLayout {
                profile: profile.id.clone(),
                family,
                key: family.layout_key(),
                value: constant.value.clone(),
            });
        }
        Ok(Self {
            profile_id: profile.id.clone(),
            platform_build,
            storage_profile,
            family,
            layout: match family {
                BusinessObjectFamily::Catalog => BusinessObjectLayout::CatalogV1,
                BusinessObjectFamily::Document => BusinessObjectLayout::DocumentV1,
                BusinessObjectFamily::Subsystem => BusinessObjectLayout::SubsystemV1,
                BusinessObjectFamily::ExchangePlan => BusinessObjectLayout::ExchangePlanV1,
                BusinessObjectFamily::BusinessProcess => BusinessObjectLayout::BusinessProcessV1,
                BusinessObjectFamily::Task => BusinessObjectLayout::TaskV1,
            },
        })
    }

    #[cfg(test)]
    pub(crate) fn fixture(profile_id: &str, family: BusinessObjectFamily) -> Self {
        Self {
            profile_id: ProfileId::parse(profile_id).unwrap(),
            platform_build: PlatformBuild::parse("8.3.27.1989").unwrap(),
            storage_profile: StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            family,
            layout: match family {
                BusinessObjectFamily::Catalog => BusinessObjectLayout::CatalogV1,
                BusinessObjectFamily::Document => BusinessObjectLayout::DocumentV1,
                BusinessObjectFamily::Subsystem => BusinessObjectLayout::SubsystemV1,
                BusinessObjectFamily::ExchangePlan => BusinessObjectLayout::ExchangePlanV1,
                BusinessObjectFamily::BusinessProcess => BusinessObjectLayout::BusinessProcessV1,
                BusinessObjectFamily::Task => BusinessObjectLayout::TaskV1,
            },
        }
    }

    pub(crate) const fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BusinessObjectProfileError {
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
        family: BusinessObjectFamily,
        key: &'static str,
        value: String,
    },
}

impl Display for BusinessObjectProfileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCoordinate {
                profile,
                coordinate,
            } => write!(
                formatter,
                "profile `{profile}` has no independent `{coordinate}` coordinate"
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

impl Error for BusinessObjectProfileError {}

#[derive(Debug)]
pub enum BusinessObjectBuildError {
    Profile(BusinessObjectProfileError),
    ProfileMismatch {
        graph: ProfileId,
        codec: ProfileId,
    },
    AxisMismatch {
        axis: &'static str,
        expected: String,
        actual: String,
    },
    UnknownObject(ObjectUuid),
    MissingPrimaryRoute(ObjectUuid),
    FamilyMismatch {
        expected: BusinessObjectFamily,
        actual: String,
    },
    InvalidModel {
        object: ObjectUuid,
        reason: &'static str,
    },
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

impl Display for BusinessObjectBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(source) => {
                write!(formatter, "unsupported business-object profile: {source}")
            }
            Self::ProfileMismatch { graph, codec } => write!(
                formatter,
                "bootstrap graph profile `{graph}` differs from business-object profile `{codec}`"
            ),
            Self::AxisMismatch {
                axis,
                expected,
                actual,
            } => write!(
                formatter,
                "business-object `{axis}` axis mismatch: expected `{expected}`, got `{actual}`"
            ),
            Self::UnknownObject(uuid) => write!(formatter, "validated graph has no object {uuid}"),
            Self::MissingPrimaryRoute(uuid) => {
                write!(
                    formatter,
                    "bootstrap graph has no primary row for object {uuid}"
                )
            }
            Self::FamilyMismatch { expected, actual } => write!(
                formatter,
                "{} codec cannot compile `{actual}` metadata",
                expected.as_str()
            ),
            Self::InvalidModel { object, reason } => write!(
                formatter,
                "object {object} is not compilable Catalog/Document metadata: {reason}"
            ),
            Self::Native(reason) => {
                write!(formatter, "invalid native Catalog/Document row: {reason}")
            }
            Self::PlainPayloadTooLarge { maximum, actual } => write!(
                formatter,
                "native Catalog/Document plaintext has {actual} bytes, exceeding the {maximum}-byte bound"
            ),
            Self::Deflate(source) => write!(
                formatter,
                "failed to raw-deflate Catalog/Document row: {source}"
            ),
            Self::Inflate(source) => write!(
                formatter,
                "failed to inflate Catalog/Document row: {source}"
            ),
            Self::Storage(source) => write!(
                formatter,
                "invalid Catalog/Document storage target: {source}"
            ),
            Self::Patch(source) => write!(
                formatter,
                "invalid Catalog/Document storage payload: {source}"
            ),
        }
    }
}

impl Error for BusinessObjectBuildError {
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

impl From<BusinessObjectProfileError> for BusinessObjectBuildError {
    fn from(source: BusinessObjectProfileError) -> Self {
        Self::Profile(source)
    }
}

impl From<StorageBuildError> for BusinessObjectBuildError {
    fn from(source: StorageBuildError) -> Self {
        Self::Storage(source)
    }
}

impl From<StoragePatchBuildError> for BusinessObjectBuildError {
    fn from(source: StoragePatchBuildError) -> Self {
        Self::Patch(source)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BusinessObjectTabularNativeIr {
    pub uuid: ObjectUuid,
    pub attribute_uuids: Vec<ObjectUuid>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BusinessObjectNativeIr {
    pub family: BusinessObjectFamily,
    pub uuid: ObjectUuid,
    pub generated_types: Vec<(ObjectUuid, ObjectUuid)>,
    pub attribute_uuids: Vec<ObjectUuid>,
    pub tabular_sections: Vec<BusinessObjectTabularNativeIr>,
    pub command_uuids: Vec<ObjectUuid>,
    pub form_uuids: Vec<ObjectUuid>,
    pub template_uuids: Vec<ObjectUuid>,
    pub addressing_attribute_uuids: Vec<ObjectUuid>,
    pub content_uuids: Vec<ObjectUuid>,
    pub child_subsystem_uuids: Vec<ObjectUuid>,
}

pub(crate) fn compile_business_object(
    validated: &ValidatedConfiguration<'_>,
    graph: &BootstrapGraph,
    object_uuid: ObjectUuid,
    axes: &CompileAxes,
    profile: &BusinessObjectMetadataProfile,
) -> Result<StoragePatchEntry, BusinessObjectBuildError> {
    validate_coordinates(graph, axes, profile)?;
    let object_index = validated
        .graph()
        .object_index_by_uuid(object_uuid)
        .ok_or(BusinessObjectBuildError::UnknownObject(object_uuid))?;
    let object = &validated.configuration().objects()[object_index];
    if object.kind().as_str() != profile.family.as_str() {
        return Err(BusinessObjectBuildError::FamilyMismatch {
            expected: profile.family,
            actual: object.kind().as_str().to_owned(),
        });
    }
    let expected_source_profile = format!("xml-{}", axes.xml_dialect());
    if object.provenance().source_profile().as_str() != expected_source_profile {
        return Err(BusinessObjectBuildError::AxisMismatch {
            axis: "xml_dialect",
            expected: expected_source_profile,
            actual: object.provenance().source_profile().to_string(),
        });
    }
    let route = graph
        .primary_object_entry(object_uuid)
        .ok_or(BusinessObjectBuildError::MissingPrimaryRoute(object_uuid))?;
    let indexes = ReferenceIndexes::build(validated, object_uuid)?;
    let root = match (profile.family, profile.layout) {
        (BusinessObjectFamily::Catalog, BusinessObjectLayout::CatalogV1) => {
            build_catalog(validated, object, &indexes)?
        }
        (BusinessObjectFamily::Document, BusinessObjectLayout::DocumentV1) => {
            build_document(validated, object, &indexes)?
        }
        (BusinessObjectFamily::Subsystem, BusinessObjectLayout::SubsystemV1) => {
            build_subsystem(validated, object, &indexes)?
        }
        (BusinessObjectFamily::ExchangePlan, BusinessObjectLayout::ExchangePlanV1) => {
            build_exchange_plan(validated, object, &indexes)?
        }
        (BusinessObjectFamily::BusinessProcess, BusinessObjectLayout::BusinessProcessV1) => {
            build_business_process(validated, object, &indexes)?
        }
        (BusinessObjectFamily::Task, BusinessObjectLayout::TaskV1) => {
            build_task(validated, object, &indexes)?
        }
        _ => return native("profile family and layout disagree"),
    };
    let plaintext = serialize_native(&root)?;
    let bytes = raw_deflate(&plaintext)?;
    let provenance = StorageProvenance::new(&format!(
        "bootstrap:{}:metadata:{}",
        profile.profile_id,
        profile.family.as_str()
    ))?;
    Ok(StoragePatchEntry::new(
        StoragePatchTarget::new(route.key().clone(), MultipartIdentity::single(), provenance),
        StoragePatchOutcome::compiled(bytes)?,
    ))
}

pub(crate) fn decode_business_object_blob(
    blob: &[u8],
    profile: &BusinessObjectMetadataProfile,
) -> Result<BusinessObjectNativeIr, BusinessObjectBuildError> {
    let plain = inflate_bounded(blob)?;
    let value = NativeParser::new(&plain).parse()?;
    decode_native_ir(&value, profile.family)
}

fn validate_coordinates(
    graph: &BootstrapGraph,
    axes: &CompileAxes,
    profile: &BusinessObjectMetadataProfile,
) -> Result<(), BusinessObjectBuildError> {
    if graph.profile_id() != profile.profile_id() {
        return Err(BusinessObjectBuildError::ProfileMismatch {
            graph: graph.profile_id().clone(),
            codec: profile.profile_id().clone(),
        });
    }
    let actual_platform = axes
        .platform_build()
        .map(ToString::to_string)
        .unwrap_or_else(|| "<missing>".to_owned());
    if axes.platform_build() != Some(&profile.platform_build) {
        return Err(BusinessObjectBuildError::AxisMismatch {
            axis: "platform_build",
            expected: profile.platform_build.to_string(),
            actual: actual_platform,
        });
    }
    if axes.storage_profile() != &profile.storage_profile {
        return Err(BusinessObjectBuildError::AxisMismatch {
            axis: "storage_profile",
            expected: profile.storage_profile.to_string(),
            actual: axes.storage_profile().to_string(),
        });
    }
    if axes.compatibility_mode().is_some() {
        return Err(BusinessObjectBuildError::AxisMismatch {
            axis: "compatibility_mode",
            expected: "<unspecified for evidenced layout>".to_owned(),
            actual: axes.compatibility_mode().unwrap().to_string(),
        });
    }
    if axes.container_revision().is_some() {
        return Err(BusinessObjectBuildError::AxisMismatch {
            axis: "container_revision",
            expected: "<unspecified for evidenced layout>".to_owned(),
            actual: axes.container_revision().unwrap().to_string(),
        });
    }
    if !matches!(axes.xml_dialect().to_string().as_str(), "2.20" | "2.21") {
        return Err(BusinessObjectBuildError::AxisMismatch {
            axis: "xml_dialect",
            expected: "2.20 or 2.21".to_owned(),
            actual: axes.xml_dialect().to_string(),
        });
    }
    Ok(())
}

struct ReferenceIndexes {
    objects: BTreeMap<String, ObjectUuid>,
    generated_types: BTreeMap<String, ObjectUuid>,
    kinds: BTreeMap<ObjectUuid, String>,
    owners: BTreeMap<ObjectUuid, Option<ObjectUuid>>,
}

impl ReferenceIndexes {
    fn build(
        validated: &ValidatedConfiguration<'_>,
        compiling: ObjectUuid,
    ) -> Result<Self, BusinessObjectBuildError> {
        let mut cache = BTreeMap::<usize, Option<String>>::new();
        let mut visiting = BTreeSet::new();
        let mut objects = BTreeMap::new();
        for index in 0..validated.configuration().objects().len() {
            let Some(reference) =
                readable_reference_for_object(validated, index, &mut cache, &mut visiting)
            else {
                continue;
            };
            let uuid = validated.configuration().objects()[index].identity().uuid();
            insert_reference(&mut objects, reference, uuid, compiling)?;
        }

        let mut generated_types = BTreeMap::new();
        for (index, object) in validated.configuration().objects().iter().enumerate() {
            let Some(name) = text_property_optional(object, "Name") else {
                continue;
            };
            if name.is_empty() || name.contains('.') {
                continue;
            }
            for generated in object.generated_types() {
                let category = generated.kind().as_str();
                let readable =
                    if object.kind().as_str() == "DefinedType" && category == "DefinedType" {
                        format!("cfg:DefinedType.{name}")
                    } else if object.kind().as_str() == "TabularSection" {
                        let Some(owner_uuid) = object.owner() else {
                            return invalid_model(
                                compiling,
                                "TabularSection generated type has no owner",
                            );
                        };
                        let Some(owner_index) = validated.graph().object_index_by_uuid(owner_uuid)
                        else {
                            return invalid_model(compiling, "TabularSection owner is missing");
                        };
                        let owner = &validated.configuration().objects()[owner_index];
                        let Some(owner_name) = text_property_optional(owner, "Name") else {
                            return invalid_model(compiling, "TabularSection owner has no Name");
                        };
                        format!(
                            "cfg:{}{}.{owner_name}.{name}",
                            owner.kind().as_str(),
                            category
                        )
                    } else {
                        format!("cfg:{}{}.{name}", object.kind().as_str(), category)
                    };
                insert_reference(&mut generated_types, readable, generated.uuid(), compiling)?;
            }
            // Keep the cache populated for objects whose readable name was
            // supplied explicitly through QualifiedName.
            let _ = cache.get(&index);
        }
        let kinds = validated
            .configuration()
            .objects()
            .iter()
            .map(|object| (object.identity().uuid(), object.kind().as_str().to_owned()))
            .collect();
        let owners = validated
            .configuration()
            .objects()
            .iter()
            .map(|object| (object.identity().uuid(), object.owner()))
            .collect();
        Ok(Self {
            objects,
            generated_types,
            kinds,
            owners,
        })
    }

    fn object(
        &self,
        compiling: ObjectUuid,
        reference: &str,
    ) -> Result<ObjectUuid, BusinessObjectBuildError> {
        self.objects
            .get(reference)
            .copied()
            .ok_or(BusinessObjectBuildError::InvalidModel {
                object: compiling,
                reason: "readable metadata reference is unresolved",
            })
    }

    fn type_id(
        &self,
        compiling: ObjectUuid,
        reference: &str,
    ) -> Result<ObjectUuid, BusinessObjectBuildError> {
        builtin_type_uuid(reference)
            .or_else(|| self.generated_types.get(reference).copied())
            .ok_or(BusinessObjectBuildError::InvalidModel {
                object: compiling,
                reason: "readable Type reference is unresolved",
            })
    }

    fn kind(&self, uuid: ObjectUuid) -> Option<&str> {
        self.kinds.get(&uuid).map(String::as_str)
    }

    fn owner(&self, uuid: ObjectUuid) -> Option<Option<ObjectUuid>> {
        self.owners.get(&uuid).copied()
    }
}

fn insert_reference(
    values: &mut BTreeMap<String, ObjectUuid>,
    reference: String,
    uuid: ObjectUuid,
    compiling: ObjectUuid,
) -> Result<(), BusinessObjectBuildError> {
    if let Some(existing) = values.insert(reference, uuid)
        && existing != uuid
    {
        return invalid_model(compiling, "readable metadata reference is ambiguous");
    }
    Ok(())
}

fn readable_reference_for_object(
    validated: &ValidatedConfiguration<'_>,
    index: usize,
    cache: &mut BTreeMap<usize, Option<String>>,
    visiting: &mut BTreeSet<usize>,
) -> Option<String> {
    if let Some(cached) = cache.get(&index) {
        return cached.clone();
    }
    if !visiting.insert(index) {
        return None;
    }
    let object = validated.configuration().objects().get(index)?;
    if let Some(qualified) = text_property_optional(object, "QualifiedName")
        && !qualified.is_empty()
        && !qualified.chars().any(char::is_whitespace)
    {
        visiting.remove(&index);
        cache.insert(index, Some(qualified.to_owned()));
        return Some(qualified.to_owned());
    }
    let name = text_property_optional(object, "Name")
        .filter(|name| !name.is_empty() && !name.contains('.'));
    let reference = name.and_then(|name| {
        let own = format!("{}.{}", object.kind().as_str(), name);
        match object.owner() {
            None => Some(own),
            Some(owner) => {
                let owner_index = validated.graph().object_index_by_uuid(owner)?;
                readable_reference_for_object(validated, owner_index, cache, visiting)
                    .map(|parent| format!("{parent}.{own}"))
            }
        }
    });
    visiting.remove(&index);
    cache.insert(index, reference.clone());
    reference
}

const CATALOG_PROPERTY_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "Hierarchical",
    "LimitLevelCount",
    "FoldersOnTop",
    "UseStandardCommands",
    "CheckUnique",
    "Autonumbering",
    "QuickChoice",
    "IncludeHelpInContents",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
    "LevelCount",
    "CodeLength",
    "DescriptionLength",
    "HierarchyType",
    "SubordinationUse",
    "CodeType",
    "CodeAllowedLength",
    "CodeSeries",
    "DefaultPresentation",
    "PredefinedDataUpdate",
    "EditType",
    "ChoiceMode",
    "SearchStringModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "DataLockControlMode",
    "FullTextSearch",
    "CreateOnInput",
    "ChoiceHistoryOnInput",
    "DataHistory",
    "DefaultObjectForm",
    "DefaultFolderForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "DefaultFolderChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryFolderForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "AuxiliaryFolderChoiceForm",
    "Owners",
    "BasedOn",
    "DataLockFields",
    "InputByString",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "ChildForms",
    "ChildTemplates",
];

const DOCUMENT_PROPERTY_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "CheckUnique",
    "Autonumbering",
    "PostInPrivilegedMode",
    "UnpostInPrivilegedMode",
    "IncludeHelpInContents",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
    "NumberLength",
    "NumberType",
    "NumberAllowedLength",
    "NumberPeriodicity",
    "CreateOnInput",
    "SearchStringModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "Posting",
    "RealTimePosting",
    "RegisterRecordsDeletion",
    "RegisterRecordsWritingOnPost",
    "SequenceFilling",
    "DataLockControlMode",
    "FullTextSearch",
    "ChoiceHistoryOnInput",
    "DataHistory",
    "Numerator",
    "DefaultObjectForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "BasedOn",
    "RegisterRecords",
    "DataLockFields",
    "InputByString",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "ChildForms",
    "ChildTemplates",
];

const SUBSYSTEM_PROPERTY_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "IncludeHelpInContents",
    "IncludeInCommandInterface",
    "UseOneCommand",
    "Explanation",
    "Picture",
    "PictureLoadTransparent",
    "Content",
    "ChildSubsystems",
];

const EXCHANGE_PLAN_PROPERTY_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "QuickChoice",
    "DistributedInfoBase",
    "IncludeConfigurationExtensions",
    "IncludeHelpInContents",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
    "CodeLength",
    "DescriptionLength",
    "CodeAllowedLength",
    "DefaultPresentation",
    "EditType",
    "ChoiceMode",
    "SearchStringModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "CreateOnInput",
    "ChoiceHistoryOnInput",
    "DataLockControlMode",
    "FullTextSearch",
    "DataHistory",
    "ThisNode",
    "DefaultObjectForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "BasedOn",
    "InputByString",
    "DataLockFields",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "ChildForms",
    "ChildTemplates",
];

const BUSINESS_PROCESS_PROPERTY_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "CheckUnique",
    "Autonumbering",
    "CreateTaskInPrivilegedMode",
    "IncludeHelpInContents",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
    "NumberLength",
    "EditType",
    "CreateOnInput",
    "SearchStringModeOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceHistoryOnInput",
    "NumberType",
    "NumberAllowedLength",
    "NumberPeriodicity",
    "DataLockControlMode",
    "FullTextSearch",
    "DataHistory",
    "Task",
    "DefaultObjectForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "BasedOn",
    "InputByString",
    "DataLockFields",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "ChildForms",
    "ChildTemplates",
];

const TASK_PROPERTY_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "CheckUnique",
    "Autonumbering",
    "IncludeHelpInContents",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
    "NumberLength",
    "DescriptionLength",
    "NumberType",
    "NumberAllowedLength",
    "TaskNumberAutoPrefix",
    "DefaultPresentation",
    "EditType",
    "SearchStringModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "CreateOnInput",
    "ChoiceHistoryOnInput",
    "DataLockControlMode",
    "FullTextSearch",
    "DataHistory",
    "Addressing",
    "MainAddressingAttribute",
    "CurrentPerformer",
    "DefaultObjectForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "BasedOn",
    "InputByString",
    "DataLockFields",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "ChildForms",
    "ChildTemplates",
];

fn build_catalog(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    let uuid = object.identity().uuid();
    validate_root_object(validated, object, CATALOG_PROPERTY_SCHEMA)?;
    let children = collect_children(validated, object, BusinessObjectFamily::Catalog, indexes)?;
    let generated = generated_pairs(object, &["Object", "Ref", "Selection", "List", "Manager"])?;
    let mut fields = vec![token("0"); 61];
    fields[0] = token("57");
    put_generated_pairs(&mut fields, &[1, 3, 5, 7, 34], &generated);
    fields[9] = list(vec![token("0"), native_header(object)?]);
    fields[10] = token(u32_property(object, "LevelCount")?.to_string());
    fields[11] = enum_code(
        object,
        "EditType",
        &[("InList", "0"), ("InDialog", "1"), ("BothWays", "2")],
    )?;
    fields[12] = metadata_reference_collection(object, "Owners", indexes)?;
    fields[13] = bool_token(object, "FoldersOnTop")?;
    fields[14] = bool_token(object, "CheckUnique")?;
    fields[15] = bool_token(object, "Autonumbering")?;
    fields[16] = enum_code(
        object,
        "CodeSeries",
        &[
            ("WholeCatalog", "0"),
            ("WithinSubordination", "1"),
            ("WithinOwnerSubordination", "2"),
        ],
    )?;
    fields[17] = token(u32_property(object, "CodeLength")?.to_string());
    fields[18] = enum_code(object, "CodeType", &[("Number", "0"), ("String", "1")])?;
    fields[19] = token(u32_property(object, "DescriptionLength")?.to_string());
    fields[20] = enum_code(
        object,
        "DefaultPresentation",
        &[("AsCode", "0"), ("AsDescription", "1")],
    )?;
    let form_names = [
        "DefaultObjectForm",
        "DefaultFolderForm",
        "DefaultListForm",
        "DefaultChoiceForm",
        "DefaultFolderChoiceForm",
        "AuxiliaryObjectForm",
        "AuxiliaryFolderForm",
        "AuxiliaryListForm",
        "AuxiliaryChoiceForm",
        "AuxiliaryFolderChoiceForm",
    ];
    for (slot, name) in (21..=30).zip(form_names) {
        fields[slot] = optional_owned_reference(object, name, &children.forms, indexes)?;
    }
    fields[31] = bool_token(object, "UseStandardCommands")?;
    fields[32] = metadata_reference_collection(object, "BasedOn", indexes)?;
    fields[33] = bool_token(object, "IncludeHelpInContents")?;
    fields[36] = enum_code(
        object,
        "HierarchyType",
        &[("HierarchyFoldersAndItems", "0"), ("HierarchyOfItems", "1")],
    )?;
    fields[37] = bool_token(object, "Hierarchical")?;
    fields[38] = bool_token(object, "LimitLevelCount")?;
    fields[39] = token("0");
    fields[40] = enum_code(
        object,
        "ChoiceMode",
        &[("FromForm", "0"), ("QuickChoice", "1"), ("BothWays", "2")],
    )?;
    fields[41] = bool_token(object, "QuickChoice")?;
    fields[42] = field_reference_collection(
        object,
        "InputByString",
        BusinessObjectFamily::Catalog,
        indexes,
    )?;
    fields[43] = enum_code(
        object,
        "DataLockControlMode",
        &[("Automatic", "0"), ("Managed", "1")],
    )?;
    fields[44] = enum_code(object, "FullTextSearch", &[("DontUse", "0"), ("Use", "1")])?;
    fields[45] = list(vec![token("0")]);
    for (slot, name) in (46..=50).zip([
        "ObjectPresentation",
        "ExtendedObjectPresentation",
        "ListPresentation",
        "ExtendedListPresentation",
        "Explanation",
    ]) {
        fields[slot] = localized_value(object, name, "language")?;
    }
    fields[51] = enum_code(
        object,
        "CodeAllowedLength",
        &[("Fixed", "0"), ("Variable", "1")],
    )?;
    fields[52] = list(vec![token("0"), list(vec![token("0")])]);
    fields[53] = enum_code(
        object,
        "CreateOnInput",
        &[("Auto", "0"), ("DontUse", "1"), ("Use", "2")],
    )?;
    fields[54] = field_reference_collection(
        object,
        "DataLockFields",
        BusinessObjectFamily::Catalog,
        indexes,
    )?;
    fields[55] = enum_code(
        object,
        "PredefinedDataUpdate",
        &[("Auto", "0"), ("AutoUpdate", "1"), ("DontAutoUpdate", "2")],
    )?;
    fields[56] = input_modes(object)?;
    fields[57] = enum_code(
        object,
        "ChoiceHistoryOnInput",
        &[("Auto", "0"), ("DontUse", "1")],
    )?;
    fields[58] = enum_code(object, "DataHistory", &[("DontUse", "0"), ("Use", "1")])?;
    fields[59] = bool_token(object, "UpdateDataHistoryImmediatelyAfterWrite")?;
    fields[60] = bool_token(object, "ExecuteAfterWriteDataHistoryVersionProcessing")?;
    if enum_property(object, "SubordinationUse")? != "ToItems" {
        return invalid_model(uuid, "Catalog SubordinationUse is not evidenced");
    }

    Ok(list(vec![
        token("1"),
        list(fields),
        token("5"),
        native_collection(
            TEMPLATE_COLLECTION_UUID,
            children.templates.into_iter().map(uuid_value).collect(),
        ),
        native_collection(CATALOG_COMMAND_COLLECTION_UUID, children.commands),
        native_collection(CATALOG_TABULAR_COLLECTION_UUID, children.tabular_sections),
        native_collection(CATALOG_ATTRIBUTE_COLLECTION_UUID, children.attributes),
        native_collection(
            CATALOG_FORM_COLLECTION_UUID,
            children.forms.into_iter().map(uuid_value).collect(),
        ),
    ]))
}

fn build_document(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    validate_root_object(validated, object, DOCUMENT_PROPERTY_SCHEMA)?;
    let children = collect_children(validated, object, BusinessObjectFamily::Document, indexes)?;
    let generated = generated_pairs(object, &["Object", "Ref", "Selection", "List", "Manager"])?;
    let mut fields = vec![token("0"); 53];
    fields[0] = token("40");
    put_generated_pairs(&mut fields, &[1, 3, 5, 7, 26], &generated);
    fields[9] = list(vec![token("0"), native_header(object)?]);
    fields[10] = optional_metadata_reference(object, "Numerator", indexes)?;
    fields[11] = enum_code(object, "NumberType", &[("Number", "0"), ("String", "1")])?;
    fields[12] = token(u32_property(object, "NumberLength")?.to_string());
    fields[13] = enum_code(
        object,
        "NumberPeriodicity",
        &[("Nonperiodical", "0"), ("Year", "1")],
    )?;
    fields[14] = bool_token(object, "CheckUnique")?;
    fields[15] = bool_token(object, "Autonumbering")?;
    for (slot, name) in (16..=18).zip(["DefaultObjectForm", "DefaultListForm", "DefaultChoiceForm"])
    {
        fields[slot] = optional_owned_reference(object, name, &children.forms, indexes)?;
    }
    fields[19] = enum_code(object, "Posting", &[("Allow", "0"), ("Deny", "1")])?;
    fields[20] = enum_code(
        object,
        "RegisterRecordsDeletion",
        &[
            ("AutoDelete", "0"),
            ("AutoDeleteOff", "1"),
            ("AutoDeleteOnUnpost", "2"),
        ],
    )?;
    fields[21] = enum_code(object, "RealTimePosting", &[("Allow", "0"), ("Deny", "1")])?;
    fields[22] = metadata_reference_collection(object, "BasedOn", indexes)?;
    fields[23] = bool_token(object, "UseStandardCommands")?;
    fields[24] = metadata_reference_collection(object, "RegisterRecords", indexes)?;
    fields[25] = bool_token(object, "IncludeHelpInContents")?;
    fields[28] = enum_code(
        object,
        "SequenceFilling",
        &[("AutoFill", "0"), ("AutoFillOff", "1")],
    )?;
    fields[29] = field_reference_collection(
        object,
        "InputByString",
        BusinessObjectFamily::Document,
        indexes,
    )?;
    fields[30] = enum_code(
        object,
        "DataLockControlMode",
        &[("Automatic", "0"), ("Managed", "1")],
    )?;
    fields[31] = enum_code(object, "FullTextSearch", &[("DontUse", "0"), ("Use", "1")])?;
    fields[32] = list(vec![token("0")]);
    fields[33] = bool_token(object, "PostInPrivilegedMode")?;
    fields[34] = bool_token(object, "UnpostInPrivilegedMode")?;
    for (slot, name) in (35..=37).zip([
        "AuxiliaryObjectForm",
        "AuxiliaryListForm",
        "AuxiliaryChoiceForm",
    ]) {
        fields[slot] = optional_owned_reference(object, name, &children.forms, indexes)?;
    }
    for (slot, name) in (38..=42).zip([
        "ObjectPresentation",
        "ExtendedObjectPresentation",
        "ListPresentation",
        "ExtendedListPresentation",
        "Explanation",
    ]) {
        fields[slot] = localized_value(object, name, "language")?;
    }
    fields[43] = enum_code(
        object,
        "RegisterRecordsWritingOnPost",
        &[("WriteSelected", "0"), ("WriteModified", "1")],
    )?;
    fields[44] = enum_code(
        object,
        "NumberAllowedLength",
        &[("Fixed", "0"), ("Variable", "1")],
    )?;
    fields[45] = list(vec![token("0"), list(vec![token("0")])]);
    fields[46] = enum_code(
        object,
        "CreateOnInput",
        &[("Auto", "0"), ("DontUse", "1"), ("Use", "2")],
    )?;
    fields[47] = field_reference_collection(
        object,
        "DataLockFields",
        BusinessObjectFamily::Document,
        indexes,
    )?;
    fields[48] = input_modes(object)?;
    fields[49] = enum_code(
        object,
        "ChoiceHistoryOnInput",
        &[("Auto", "0"), ("DontUse", "1")],
    )?;
    fields[50] = enum_code(object, "DataHistory", &[("DontUse", "0"), ("Use", "1")])?;
    fields[51] = bool_token(object, "UpdateDataHistoryImmediatelyAfterWrite")?;
    fields[52] = bool_token(object, "ExecuteAfterWriteDataHistoryVersionProcessing")?;

    Ok(list(vec![
        token("1"),
        list(fields),
        token("5"),
        native_collection(DOCUMENT_TABULAR_COLLECTION_UUID, children.tabular_sections),
        native_collection(
            TEMPLATE_COLLECTION_UUID,
            children.templates.into_iter().map(uuid_value).collect(),
        ),
        native_collection(DOCUMENT_ATTRIBUTE_COLLECTION_UUID, children.attributes),
        native_collection(DOCUMENT_COMMAND_COLLECTION_UUID, children.commands),
        native_collection(
            DOCUMENT_FORM_COLLECTION_UUID,
            children.forms.into_iter().map(uuid_value).collect(),
        ),
    ]))
}

fn build_subsystem(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    let uuid = object.identity().uuid();
    validate_root_object(validated, object, SUBSYSTEM_PROPERTY_SCHEMA)?;
    if !object.generated_types().is_empty() {
        return invalid_model(uuid, "Subsystem generated type inventory is not empty");
    }
    if validated
        .configuration()
        .objects()
        .iter()
        .any(|candidate| candidate.owner() == Some(uuid))
    {
        return invalid_model(uuid, "Subsystem has an unsupported embedded child");
    }
    let child_subsystems = reference_sequence_targets(object, "ChildSubsystems", indexes)?;
    validate_named_children(uuid, &child_subsystems, "Subsystem", indexes)?;
    if child_subsystems.contains(&uuid) {
        return invalid_model(uuid, "Subsystem contains itself as a child");
    }
    let owner = list(vec![
        token("22"),
        native_header(object)?,
        bool_token(object, "IncludeHelpInContents")?,
        list(vec![token("0"), token("0")]),
        bool_token(object, "IncludeInCommandInterface")?,
        subsystem_picture(object, indexes)?,
        localized_value(object, "Explanation", "language")?,
        metadata_reference_collection(object, "Content", indexes)?,
        bool_token(object, "UseOneCommand")?,
    ]);
    Ok(list(vec![
        token("1"),
        owner,
        token("1"),
        native_collection(
            SUBSYSTEM_COLLECTION_UUID,
            child_subsystems.into_iter().map(uuid_value).collect(),
        ),
    ]))
}

fn build_exchange_plan(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    validate_root_object(validated, object, EXCHANGE_PLAN_PROPERTY_SCHEMA)?;
    let children = collect_children(
        validated,
        object,
        BusinessObjectFamily::ExchangePlan,
        indexes,
    )?;
    let generated = generated_pairs(object, &["Object", "Ref", "Selection", "List", "Manager"])?;
    require_enum_value(object, "DefaultPresentation", "AsDescription")?;
    require_enum_value(object, "EditType", "InDialog")?;
    require_enum_value(object, "ChoiceMode", "BothWays")?;
    require_enum_value(object, "CreateOnInput", "DontUse")?;
    require_enum_value(object, "DataHistory", "DontUse")?;
    require_bool_value(object, "UpdateDataHistoryImmediatelyAfterWrite", false)?;
    require_bool_value(
        object,
        "ExecuteAfterWriteDataHistoryVersionProcessing",
        false,
    )?;

    let mut fields = vec![token("0"); 51];
    fields[0] = token("37");
    put_generated_pairs(&mut fields, &[1, 3, 5, 7, 9], &generated);
    fields[11] = parsed_uuid_property(object, "ThisNode")?;
    fields[12] = native_header(object)?;
    fields[13] = bool_token(object, "UseStandardCommands")?;
    fields[14] = optional_owned_reference(object, "DefaultObjectForm", &children.forms, indexes)?;
    fields[15] = token(u32_property(object, "CodeLength")?.to_string());
    fields[16] = token("0");
    fields[17] = token(u32_property(object, "DescriptionLength")?.to_string());
    fields[18] = bool_token(object, "IncludeHelpInContents")?;
    fields[19] = optional_owned_reference(object, "DefaultListForm", &children.forms, indexes)?;
    fields[20] = optional_owned_reference(object, "DefaultChoiceForm", &children.forms, indexes)?;
    fields[21] = token("1");
    fields[22] = token("2");
    fields[23] = bool_token(object, "QuickChoice")?;
    fields[24] = metadata_reference_collection(object, "BasedOn", indexes)?;
    fields[25] = token("1");
    fields[26] = bool_token(object, "DistributedInfoBase")?;
    fields[27] = field_reference_collection(
        object,
        "InputByString",
        BusinessObjectFamily::ExchangePlan,
        indexes,
    )?;
    fields[28] = enum_code(
        object,
        "DataLockControlMode",
        &[("Automatic", "0"), ("Managed", "1")],
    )?;
    fields[29] = enum_code(object, "FullTextSearch", &[("DontUse", "0"), ("Use", "1")])?;
    fields[30] = standard_attributes(&["-14", "-13", "-10", "-9", "-6", "-4", "-3", "-2"])?;
    for (slot, name) in (31..=33).zip([
        "AuxiliaryObjectForm",
        "AuxiliaryListForm",
        "AuxiliaryChoiceForm",
    ]) {
        fields[slot] = optional_owned_reference(object, name, &children.forms, indexes)?;
    }
    for (slot, name) in (34..=38).zip([
        "ObjectPresentation",
        "ExtendedObjectPresentation",
        "ListPresentation",
        "ExtendedListPresentation",
        "Explanation",
    ]) {
        fields[slot] = localized_value(object, name, "language")?;
    }
    fields[39] = enum_code(
        object,
        "CodeAllowedLength",
        &[("Fixed", "0"), ("Variable", "1")],
    )?;
    fields[40] = list(vec![token("0"), list(vec![token("0")])]);
    fields[41] = token("1");
    fields[42] = field_reference_collection(
        object,
        "DataLockFields",
        BusinessObjectFamily::ExchangePlan,
        indexes,
    )?;
    fields[43] = input_modes(object)?;
    fields[44] = enum_code(
        object,
        "ChoiceHistoryOnInput",
        &[("Auto", "0"), ("DontUse", "1")],
    )?;
    fields[45] = bool_token(object, "IncludeConfigurationExtensions")?;
    fields[50] = token("1");

    Ok(list(vec![
        token("1"),
        list(fields),
        token("5"),
        native_collection(EXCHANGE_ATTRIBUTE_COLLECTION_UUID, children.attributes),
        native_collection(
            TEMPLATE_COLLECTION_UUID,
            children.templates.into_iter().map(uuid_value).collect(),
        ),
        native_collection(EXCHANGE_TABULAR_COLLECTION_UUID, children.tabular_sections),
        native_collection(
            EXCHANGE_FORM_COLLECTION_UUID,
            children.forms.into_iter().map(uuid_value).collect(),
        ),
        native_collection(EXCHANGE_COMMAND_COLLECTION_UUID, children.commands),
    ]))
}

fn build_business_process(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    let uuid = object.identity().uuid();
    validate_root_object(validated, object, BUSINESS_PROCESS_PROPERTY_SCHEMA)?;
    let children = collect_children(
        validated,
        object,
        BusinessObjectFamily::BusinessProcess,
        indexes,
    )?;
    if !children.templates.is_empty() || !children.addressing_attributes.is_empty() {
        return invalid_model(
            uuid,
            "BusinessProcess reserved child collection is not empty",
        );
    }
    let generated = generated_pairs(
        object,
        &[
            "Object",
            "Ref",
            "Selection",
            "List",
            "Manager",
            "RoutePointRef",
        ],
    )?;
    require_enum_value(object, "EditType", "InDialog")?;
    require_enum_value(object, "NumberType", "String")?;
    require_enum_value(object, "NumberPeriodicity", "Nonperiodical")?;

    let mut fields = vec![token("0"); 49];
    fields[0] = token("30");
    fields[1] = native_header(object)?;
    fields[2] = bool_token(object, "UseStandardCommands")?;
    put_generated_pairs(&mut fields, &[3, 5, 7, 9, 11, 13], &generated);
    fields[15] = metadata_reference_collection(object, "BasedOn", indexes)?;
    fields[16] = token("1");
    fields[17] = token("0");
    fields[18] = token(u32_property(object, "NumberLength")?.to_string());
    fields[19] = enum_code(
        object,
        "CreateOnInput",
        &[("Auto", "0"), ("DontUse", "1"), ("Use", "2")],
    )?;
    fields[20] = bool_token(object, "CheckUnique")?;
    fields[21] = bool_token(object, "Autonumbering")?;
    for (slot, name) in (22..=24).zip(["DefaultObjectForm", "DefaultListForm", "DefaultChoiceForm"])
    {
        fields[slot] = optional_owned_reference(object, name, &children.forms, indexes)?;
    }
    fields[25] = optional_metadata_reference_kind(object, "Task", "Task", indexes)?;
    fields[26] = bool_token(object, "IncludeHelpInContents")?;
    fields[27] = field_reference_collection(
        object,
        "InputByString",
        BusinessObjectFamily::BusinessProcess,
        indexes,
    )?;
    fields[28] = enum_code(
        object,
        "NumberAllowedLength",
        &[("Fixed", "0"), ("Variable", "1")],
    )?;
    fields[29] = bool_token(object, "CreateTaskInPrivilegedMode")?;
    fields[30] = standard_attributes(&["-9", "-8", "-7", "-5", "-4", "-3", "-2"])?;
    fields[31] = token("1");
    for (slot, name) in (32..=34).zip([
        "AuxiliaryObjectForm",
        "AuxiliaryListForm",
        "AuxiliaryChoiceForm",
    ]) {
        fields[slot] = optional_owned_reference(object, name, &children.forms, indexes)?;
    }
    for (slot, name) in (35..=39).zip([
        "ObjectPresentation",
        "ExtendedObjectPresentation",
        "ListPresentation",
        "ExtendedListPresentation",
        "Explanation",
    ]) {
        fields[slot] = localized_value(object, name, "language")?;
    }
    fields[40] = enum_code(
        object,
        "DataLockControlMode",
        &[("Automatic", "0"), ("Managed", "1")],
    )?;
    fields[41] = list(vec![token("0"), list(vec![token("0")])]);
    fields[42] = enum_code(object, "FullTextSearch", &[("DontUse", "0"), ("Use", "1")])?;
    fields[43] = field_reference_collection(
        object,
        "DataLockFields",
        BusinessObjectFamily::BusinessProcess,
        indexes,
    )?;
    fields[44] = input_modes(object)?;
    fields[45] = enum_code(
        object,
        "ChoiceHistoryOnInput",
        &[("Auto", "0"), ("DontUse", "1")],
    )?;
    fields[46] = enum_code(object, "DataHistory", &[("DontUse", "0"), ("Use", "1")])?;
    fields[47] = bool_token(object, "UpdateDataHistoryImmediatelyAfterWrite")?;
    fields[48] = bool_token(object, "ExecuteAfterWriteDataHistoryVersionProcessing")?;

    Ok(list(vec![
        token("1"),
        list(fields),
        token("5"),
        native_collection(TEMPLATE_COLLECTION_UUID, Vec::new()),
        native_collection(
            BUSINESS_PROCESS_FORM_COLLECTION_UUID,
            children.forms.into_iter().map(uuid_value).collect(),
        ),
        native_collection(BUSINESS_PROCESS_COMMAND_COLLECTION_UUID, children.commands),
        native_collection(
            BUSINESS_PROCESS_ATTRIBUTE_COLLECTION_UUID,
            children.attributes,
        ),
        native_collection(
            BUSINESS_PROCESS_TABULAR_COLLECTION_UUID,
            children.tabular_sections,
        ),
    ]))
}

fn build_task(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    let uuid = object.identity().uuid();
    validate_root_object(validated, object, TASK_PROPERTY_SCHEMA)?;
    let children = collect_children(validated, object, BusinessObjectFamily::Task, indexes)?;
    if !children.templates.is_empty() || !children.tabular_sections.is_empty() {
        return invalid_model(uuid, "Task reserved child collection is not empty");
    }
    let generated = generated_pairs(object, &["Object", "Ref", "Selection", "List", "Manager"])?;
    require_enum_value(object, "TaskNumberAutoPrefix", "BusinessProcessNumber")?;
    require_enum_value(object, "DefaultPresentation", "AsDescription")?;
    require_enum_value(object, "EditType", "InDialog")?;
    require_enum_value(object, "CreateOnInput", "DontUse")?;
    require_enum_value(object, "DataHistory", "DontUse")?;
    require_bool_value(object, "UpdateDataHistoryImmediatelyAfterWrite", false)?;
    require_bool_value(
        object,
        "ExecuteAfterWriteDataHistoryVersionProcessing",
        false,
    )?;

    let addressing =
        optional_metadata_reference_kind(object, "Addressing", "InformationRegister", indexes)?;
    let mut fields = vec![token("0"); 52];
    fields[0] = token("33");
    fields[1] = native_header(object)?;
    fields[2] = bool_token(object, "UseStandardCommands")?;
    put_generated_pairs(&mut fields, &[3, 5, 7, 9, 11], &generated);
    fields[13] = token(NIL_UUID);
    fields[14] = token(NIL_UUID);
    for (slot, name) in (15..=17).zip(["DefaultObjectForm", "DefaultListForm", "DefaultChoiceForm"])
    {
        fields[slot] = optional_owned_reference(object, name, &children.forms, indexes)?;
    }
    fields[18] = enum_code(object, "NumberType", &[("Number", "0"), ("String", "1")])?;
    fields[19] = token(u32_property(object, "NumberLength")?.to_string());
    fields[20] = enum_code(
        object,
        "NumberAllowedLength",
        &[("Variable", "0"), ("Fixed", "1")],
    )?;
    fields[21] = bool_token(object, "CheckUnique")?;
    fields[22] = token(u32_property(object, "DescriptionLength")?.to_string());
    fields[23] = bool_token(object, "Autonumbering")?;
    fields[24] = token("1");
    fields[25] = addressing.clone();
    fields[26] = optional_owned_member_reference(
        object,
        "MainAddressingAttribute",
        &children.addressing_attribute_uuids,
        "AddressingAttribute",
        indexes,
    )?;
    if token_is_nil(&addressing) && !children.addressing_attributes.is_empty() {
        return invalid_model(uuid, "Task addressing attributes exist without Addressing");
    }
    fields[27] = token("1");
    fields[28] =
        field_reference_collection(object, "InputByString", BusinessObjectFamily::Task, indexes)?;
    fields[29] =
        optional_metadata_reference_kind(object, "CurrentPerformer", "SessionParameter", indexes)?;
    fields[30] = metadata_reference_collection_kind(object, "BasedOn", "BusinessProcess", indexes)?;
    fields[31] = token("0");
    fields[32] = enum_code(
        object,
        "DataLockControlMode",
        &[("Automatic", "0"), ("Managed", "1")],
    )?;
    fields[33] = enum_code(object, "FullTextSearch", &[("DontUse", "0"), ("Use", "1")])?;
    fields[34] = standard_attributes(&["-10", "-9", "-8", "-7", "-5", "-4", "-3", "-2"])?;
    for (slot, name) in (35..=37).zip([
        "AuxiliaryObjectForm",
        "AuxiliaryListForm",
        "AuxiliaryChoiceForm",
    ]) {
        fields[slot] = optional_owned_reference(object, name, &children.forms, indexes)?;
    }
    for (slot, name) in (38..=42).zip([
        "ObjectPresentation",
        "ExtendedObjectPresentation",
        "ListPresentation",
        "ExtendedListPresentation",
        "Explanation",
    ]) {
        fields[slot] = localized_value(object, name, "language")?;
    }
    fields[43] = enum_code(
        object,
        "ChoiceHistoryOnInput",
        &[("Auto", "0"), ("DontUse", "1")],
    )?;
    fields[44] = list(vec![token("0"), list(vec![token("0")])]);
    fields[45] = bool_token(object, "IncludeHelpInContents")?;
    fields[46] = field_reference_collection(
        object,
        "DataLockFields",
        BusinessObjectFamily::Task,
        indexes,
    )?;
    fields[47] = input_modes(object)?;

    Ok(list(vec![
        token("1"),
        list(fields),
        token("6"),
        native_collection(TEMPLATE_COLLECTION_UUID, Vec::new()),
        native_collection(
            TASK_FORM_COLLECTION_UUID,
            children.forms.into_iter().map(uuid_value).collect(),
        ),
        native_collection(TASK_ATTRIBUTE_COLLECTION_UUID, children.attributes),
        native_collection(
            TASK_ADDRESSING_ATTRIBUTE_COLLECTION_UUID,
            children.addressing_attributes,
        ),
        native_collection(TASK_RESERVED_COLLECTION_UUID, Vec::new()),
        native_collection(TASK_COMMAND_COLLECTION_UUID, children.commands),
    ]))
}

struct CompiledChildren {
    attributes: Vec<NativeValue>,
    tabular_sections: Vec<NativeValue>,
    commands: Vec<NativeValue>,
    forms: Vec<ObjectUuid>,
    templates: Vec<ObjectUuid>,
    addressing_attributes: Vec<NativeValue>,
    addressing_attribute_uuids: Vec<ObjectUuid>,
}

fn collect_children(
    validated: &ValidatedConfiguration<'_>,
    root: &CanonicalObject,
    family: BusinessObjectFamily,
    indexes: &ReferenceIndexes,
) -> Result<CompiledChildren, BusinessObjectBuildError> {
    let root_uuid = root.identity().uuid();
    let forms = reference_sequence_targets(root, "ChildForms", indexes)?;
    let templates = reference_sequence_targets(root, "ChildTemplates", indexes)?;
    validate_named_children(root_uuid, &forms, "Form", indexes)?;
    validate_named_children(root_uuid, &templates, "Template", indexes)?;
    let addressing_owner = if family == BusinessObjectFamily::Task {
        optional_reference_uuid_kind(root, "Addressing", "InformationRegister", indexes)?
    } else {
        None
    };

    let mut attributes = Vec::new();
    let mut tabular_sections = Vec::new();
    let mut commands = Vec::new();
    let mut addressing_attributes = Vec::new();
    let mut addressing_attribute_uuids = Vec::new();
    let mut addressing_dimensions = BTreeSet::new();
    let mut accepted = BTreeSet::new();
    for object in validated.configuration().objects() {
        if object.owner() != Some(root_uuid) {
            continue;
        }
        match object.kind().as_str() {
            "Attribute" => {
                attributes.push(build_attribute(object, root_uuid, family, false, indexes)?);
                accepted.insert(object.identity().uuid());
            }
            "TabularSection" => {
                if matches!(
                    family,
                    BusinessObjectFamily::Task | BusinessObjectFamily::Subsystem
                ) {
                    return invalid_model(
                        root_uuid,
                        "metadata family does not support TabularSection children",
                    );
                }
                let mut nested = Vec::new();
                for candidate in validated.configuration().objects() {
                    if candidate.owner() == Some(object.identity().uuid()) {
                        if candidate.kind().as_str() != "Attribute" {
                            return invalid_model(
                                root_uuid,
                                "TabularSection contains a non-Attribute embedded object",
                            );
                        }
                        nested.push(build_attribute(
                            candidate, root_uuid, family, true, indexes,
                        )?);
                        accepted.insert(candidate.identity().uuid());
                    }
                }
                tabular_sections.push(build_tabular_section(object, family, nested)?);
                accepted.insert(object.identity().uuid());
            }
            "Command" => {
                commands.push(build_command(object, indexes)?);
                accepted.insert(object.identity().uuid());
            }
            "AddressingAttribute" if family == BusinessObjectFamily::Task => {
                let dimension = indexes.object(
                    object.identity().uuid(),
                    text_property(object, "AddressingDimension")?,
                )?;
                if !addressing_dimensions.insert(dimension) {
                    return invalid_model(
                        root_uuid,
                        "Task AddressingDimension is used more than once",
                    );
                }
                addressing_attributes.push(build_addressing_attribute(
                    object,
                    root_uuid,
                    addressing_owner,
                    indexes,
                )?);
                addressing_attribute_uuids.push(object.identity().uuid());
                accepted.insert(object.identity().uuid());
            }
            "Form" | "Template" => {}
            _ => {
                return invalid_model(
                    root_uuid,
                    "business object contains an unsupported direct child object",
                );
            }
        }
    }
    for object in validated.configuration().objects() {
        if matches!(
            object.kind().as_str(),
            "Attribute" | "TabularSection" | "Command" | "AddressingAttribute"
        ) && is_descendant_of(validated, object, root_uuid)
            && !accepted.contains(&object.identity().uuid())
        {
            return invalid_model(root_uuid, "embedded business-object inventory is not exact");
        }
    }
    Ok(CompiledChildren {
        attributes,
        tabular_sections,
        commands,
        forms,
        templates,
        addressing_attributes,
        addressing_attribute_uuids,
    })
}

fn is_descendant_of(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    root: ObjectUuid,
) -> bool {
    let mut owner = object.owner();
    let mut remaining = validated.configuration().objects().len();
    while let Some(uuid) = owner {
        if uuid == root {
            return true;
        }
        if remaining == 0 {
            return false;
        }
        remaining -= 1;
        let Some(index) = validated.graph().object_index_by_uuid(uuid) else {
            return false;
        };
        owner = validated.configuration().objects()[index].owner();
    }
    false
}

fn validate_named_children(
    compiling: ObjectUuid,
    values: &[ObjectUuid],
    expected_kind: &'static str,
    indexes: &ReferenceIndexes,
) -> Result<(), BusinessObjectBuildError> {
    let mut seen = BTreeSet::new();
    for uuid in values {
        if indexes.kind(*uuid) != Some(expected_kind) {
            return invalid_model(
                compiling,
                "form/template reference resolves to the wrong kind",
            );
        }
        if !seen.insert(*uuid) {
            return invalid_model(compiling, "form/template reference resolves more than once");
        }
    }
    Ok(())
}

fn build_attribute(
    object: &CanonicalObject,
    root_uuid: ObjectUuid,
    family: BusinessObjectFamily,
    nested: bool,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    validate_embedded_object(object, "Attribute")?;
    let uuid = object.identity().uuid();
    let expected_owner = if nested {
        object
            .owner()
            .ok_or(BusinessObjectBuildError::InvalidModel {
                object: uuid,
                reason: "nested Attribute has no owner",
            })?
    } else {
        root_uuid
    };
    if object.owner() != Some(expected_owner) {
        return invalid_model(uuid, "Attribute owner is not exact");
    }
    let allowed = if nested {
        NESTED_ATTRIBUTE_SCHEMA
    } else if family == BusinessObjectFamily::Catalog {
        CATALOG_ATTRIBUTE_SCHEMA
    } else {
        DOCUMENT_ATTRIBUTE_SCHEMA
    };
    require_attribute_schema(object, allowed)?;

    let mut payload = Vec::with_capacity(23);
    payload.push(token("27"));
    payload.push(list(vec![
        token("2"),
        native_header(object)?,
        type_pattern(object, indexes)?,
    ]));
    payload.push(bool_token(object, "PasswordMode")?);
    payload.push(list(vec![token("0")]));
    payload.push(list(vec![token("0")]));
    payload.push(bool_token(object, "MarkNegatives")?);
    payload.push(text(text_property(object, "Mask")?));
    payload.push(bool_token(object, "MultiLine")?);
    payload.push(list(vec![text("U")]));
    payload.push(list(vec![text("U")]));
    payload.push(enum_code(
        object,
        "ChoiceFoldersAndItems",
        &[("Items", "0"), ("Folders", "1"), ("FoldersAndItems", "2")],
    )?);
    payload.push(token(NIL_UUID));
    payload.push(enum_code(
        object,
        "QuickChoice",
        &[("DontUse", "0"), ("Use", "1"), ("Auto", "2")],
    )?);
    payload.push(enum_code(
        object,
        "FillChecking",
        &[("DontCheck", "0"), ("ShowError", "1")],
    )?);
    payload.push(list(vec![token("5006"), token("0")]));
    payload.push(list(vec![token("3"), token("0"), token("0")]));
    payload.push(list(vec![token("0"), token("0")]));
    payload.push(bool_token(object, "ExtendedEdit")?);
    payload.push(list(vec![token("0")]));
    payload.push(list(vec![text("S"), text("")]));
    payload.push(if nested {
        token("0")
    } else {
        bool_token(object, "FillFromFillingValue")?
    });
    payload.push(enum_code(
        object,
        "CreateOnInput",
        &[("Auto", "0"), ("DontUse", "1"), ("Use", "2")],
    )?);
    payload.push(enum_code(
        object,
        "ChoiceHistoryOnInput",
        &[("Auto", "0"), ("DontUse", "1")],
    )?);

    let indexing = enum_code(
        object,
        "Indexing",
        &[
            ("DontIndex", "0"),
            ("Index", "1"),
            ("IndexWithAdditionalOrder", "2"),
        ],
    )?;
    let full_text = enum_code(object, "FullTextSearch", &[("DontUse", "0"), ("Use", "1")])?;
    let data_history = enum_code(object, "DataHistory", &[("DontUse", "0"), ("Use", "1")])?;
    let wrapper = match (family, nested) {
        (BusinessObjectFamily::Catalog, false) => list(vec![
            token("6"),
            list(payload),
            indexing,
            enum_code(
                object,
                "Use",
                &[
                    ("ForItem", "0"),
                    ("ForFolder", "1"),
                    ("ForFolderAndItem", "2"),
                ],
            )?,
            full_text,
            data_history,
            token("0"),
            list(vec![token("1"), token(NIL_UUID)]),
        ]),
        (BusinessObjectFamily::Document, false) => list(vec![
            token("5"),
            list(payload),
            indexing,
            full_text,
            data_history,
        ]),
        (BusinessObjectFamily::ExchangePlan, false) => list(vec![
            token("4"),
            list(payload),
            indexing,
            full_text,
            data_history,
            token("0"),
            list(vec![token("1"), token(NIL_UUID)]),
        ]),
        (BusinessObjectFamily::BusinessProcess | BusinessObjectFamily::Task, false) => list(vec![
            token("2"),
            list(payload),
            indexing,
            full_text,
            data_history,
        ]),
        (BusinessObjectFamily::Subsystem, false) => {
            return invalid_model(uuid, "Subsystem cannot own an Attribute");
        }
        (_, true) => list(vec![
            token("8"),
            list(payload),
            indexing,
            full_text,
            data_history,
        ]),
    };
    Ok(list(vec![wrapper, token("0")]))
}

fn build_tabular_section(
    object: &CanonicalObject,
    family: BusinessObjectFamily,
    nested_attributes: Vec<NativeValue>,
) -> Result<NativeValue, BusinessObjectBuildError> {
    validate_embedded_object(object, "TabularSection")?;
    let schema = if family == BusinessObjectFamily::Catalog {
        CATALOG_TABULAR_SCHEMA
    } else {
        DOCUMENT_TABULAR_SCHEMA
    };
    require_property_schema(object, schema)?;
    if u32_property(object, "LineNumberLength")? != 5 {
        return invalid_model(
            object.identity().uuid(),
            "TabularSection LineNumberLength is not evidenced",
        );
    }
    let generated = generated_pairs(object, &["TabularSection", "TabularSectionRow"])?;
    let mut payload = vec![token("11")];
    for pair in &generated {
        payload.push(uuid_value(pair.0));
        payload.push(uuid_value(pair.1));
    }
    payload.push(native_header(object)?);
    payload.push(enum_code(
        object,
        "FillChecking",
        &[("DontCheck", "0"), ("ShowError", "1")],
    )?);
    payload.push(if family == BusinessObjectFamily::ExchangePlan {
        standard_attributes(&["-10"])?
    } else {
        list(vec![token("0")])
    });
    payload.push(list(vec![token("0")]));
    let wrapper = match family {
        BusinessObjectFamily::Catalog => list(vec![
            token("1"),
            list(payload),
            enum_code(
                object,
                "Use",
                &[("ForItem", "0"), ("ForFolderAndItem", "2")],
            )?,
        ]),
        BusinessObjectFamily::ExchangePlan => list(vec![token("1"), list(payload), token("5")]),
        BusinessObjectFamily::BusinessProcess => list(vec![token("0"), list(payload)]),
        BusinessObjectFamily::Document => list(vec![token("1"), list(payload)]),
        BusinessObjectFamily::Task | BusinessObjectFamily::Subsystem => {
            return invalid_model(
                object.identity().uuid(),
                "metadata family cannot compile a TabularSection",
            );
        }
    };
    let marker = if family == BusinessObjectFamily::Catalog {
        CATALOG_TABULAR_ATTRIBUTE_COLLECTION_UUID
    } else {
        DOCUMENT_TABULAR_ATTRIBUTE_COLLECTION_UUID
    };
    Ok(list(vec![
        wrapper,
        token("1"),
        native_collection(marker, nested_attributes),
    ]))
}

fn build_addressing_attribute(
    object: &CanonicalObject,
    root_uuid: ObjectUuid,
    addressing_owner: Option<ObjectUuid>,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    validate_embedded_object(object, "AddressingAttribute")?;
    let uuid = object.identity().uuid();
    if object.owner() != Some(root_uuid) {
        return invalid_model(uuid, "AddressingAttribute owner is not the Task");
    }
    require_attribute_schema(object, ADDRESSING_ATTRIBUTE_SCHEMA)?;
    let addressing_owner = addressing_owner.ok_or(BusinessObjectBuildError::InvalidModel {
        object: uuid,
        reason: "AddressingAttribute exists without Task Addressing",
    })?;
    let dimension_reference = text_property(object, "AddressingDimension")?;
    let dimension = indexes.object(uuid, dimension_reference)?;
    if indexes.kind(dimension) != Some("Dimension")
        || indexes.owner(dimension) != Some(Some(addressing_owner))
    {
        return invalid_model(
            uuid,
            "AddressingDimension is not owned by the Task addressing register",
        );
    }

    let payload = vec![
        token("27"),
        list(vec![
            token("2"),
            native_header(object)?,
            type_pattern(object, indexes)?,
        ]),
        bool_token(object, "PasswordMode")?,
        list(vec![token("0")]),
        list(vec![token("0")]),
        bool_token(object, "MarkNegatives")?,
        text(text_property(object, "Mask")?),
        bool_token(object, "MultiLine")?,
        list(vec![text("U")]),
        list(vec![text("U")]),
        enum_code(
            object,
            "ChoiceFoldersAndItems",
            &[("Items", "0"), ("Folders", "1"), ("FoldersAndItems", "2")],
        )?,
        token(NIL_UUID),
        enum_code(
            object,
            "QuickChoice",
            &[("DontUse", "0"), ("Use", "1"), ("Auto", "2")],
        )?,
        enum_code(
            object,
            "FillChecking",
            &[("DontCheck", "0"), ("ShowError", "1")],
        )?,
        list(vec![token("5006"), token("0")]),
        list(vec![token("3"), token("0"), token("0")]),
        list(vec![token("0"), token("0")]),
        bool_token(object, "ExtendedEdit")?,
        list(vec![token("0")]),
        list(vec![text("S"), text("")]),
        bool_token(object, "FillFromFillingValue")?,
        enum_code(
            object,
            "CreateOnInput",
            &[("Auto", "0"), ("DontUse", "1"), ("Use", "2")],
        )?,
        enum_code(
            object,
            "ChoiceHistoryOnInput",
            &[("Auto", "0"), ("DontUse", "1")],
        )?,
    ];

    let indexing = enum_code(
        object,
        "Indexing",
        &[
            ("DontIndex", "0"),
            ("Index", "1"),
            ("IndexWithAdditionalOrder", "2"),
        ],
    )?;
    let full_text = enum_code(object, "FullTextSearch", &[("DontUse", "0"), ("Use", "1")])?;
    let data_history = enum_code(object, "DataHistory", &[("DontUse", "0"), ("Use", "1")])?;
    Ok(list(vec![
        list(vec![
            token("4"),
            list(payload),
            indexing,
            uuid_value(dimension),
            full_text,
            data_history,
        ]),
        token("0"),
    ]))
}

fn build_command(
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    validate_embedded_object(object, "Command")?;
    require_property_schema(object, COMMAND_SCHEMA)?;
    let group = text_property(object, "Group")?;
    let group_uuid = builtin_command_group_uuid(group)
        .or_else(|| indexes.objects.get(group).copied())
        .ok_or(BusinessObjectBuildError::InvalidModel {
            object: object.identity().uuid(),
            reason: "Command Group is unresolved",
        })?;
    if enum_property(object, "OnMainServerUnavalableBehavior")? != "Auto" {
        return invalid_model(
            object.identity().uuid(),
            "Command server-unavailable behavior is not evidenced",
        );
    }
    let properties = list(vec![
        token("9"),
        list(vec![
            token("4"),
            token("0"),
            list(vec![token("0")]),
            text(""),
            token("-1"),
            token("-1"),
            token("1"),
            token("0"),
            text(""),
        ]),
        enum_code(
            object,
            "Representation",
            &[
                ("Text", "0"),
                ("Picture", "1"),
                ("PictureAndText", "2"),
                ("Auto", "3"),
            ],
        )?,
        list(vec![token("0")]),
        token("1"),
        list(vec![token("0"), token("0"), token("0")]),
        token("0"),
        list(vec![token("1"), uuid_value(group_uuid)]),
        list(vec![text("Pattern")]),
        native_header(object)?,
        bool_token(object, "ModifiesData")?,
        enum_code(
            object,
            "ParameterUseMode",
            &[("Single", "0"), ("Multiple", "1")],
        )?,
        token("0"),
    ]);
    let body = list(vec![
        token("1"),
        list(vec![
            token("2"),
            uuid_value(object.identity().uuid()),
            token(COMMAND_VALUE_UUID),
        ]),
        properties,
    ]);
    Ok(list(vec![
        list(vec![
            token("0"),
            list(vec![token("0"), token("0"), token("0"), body]),
        ]),
        token("0"),
    ]))
}

const CATALOG_ATTRIBUTE_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "Types",
    "StringLength",
    "StringAllowedLength",
    "NumberDigits",
    "NumberFractionDigits",
    "NumberAllowedSign",
    "DateFractions",
    "PasswordMode",
    "MarkNegatives",
    "MultiLine",
    "ExtendedEdit",
    "FillFromFillingValue",
    "FillChecking",
    "ChoiceFoldersAndItems",
    "QuickChoice",
    "CreateOnInput",
    "ChoiceHistoryOnInput",
    "Indexing",
    "FullTextSearch",
    "DataHistory",
    "Use",
    "Mask",
    "ChoiceForm",
];
const DOCUMENT_ATTRIBUTE_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "Types",
    "StringLength",
    "StringAllowedLength",
    "NumberDigits",
    "NumberFractionDigits",
    "NumberAllowedSign",
    "DateFractions",
    "PasswordMode",
    "MarkNegatives",
    "MultiLine",
    "ExtendedEdit",
    "FillFromFillingValue",
    "FillChecking",
    "ChoiceFoldersAndItems",
    "QuickChoice",
    "CreateOnInput",
    "ChoiceHistoryOnInput",
    "Indexing",
    "FullTextSearch",
    "DataHistory",
    "Mask",
    "ChoiceForm",
];
const NESTED_ATTRIBUTE_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "Types",
    "StringLength",
    "StringAllowedLength",
    "NumberDigits",
    "NumberFractionDigits",
    "NumberAllowedSign",
    "DateFractions",
    "PasswordMode",
    "MarkNegatives",
    "MultiLine",
    "ExtendedEdit",
    "FillChecking",
    "ChoiceFoldersAndItems",
    "QuickChoice",
    "CreateOnInput",
    "ChoiceHistoryOnInput",
    "Indexing",
    "FullTextSearch",
    "DataHistory",
    "Mask",
    "ChoiceForm",
];
const ADDRESSING_ATTRIBUTE_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "Types",
    "StringLength",
    "StringAllowedLength",
    "NumberDigits",
    "NumberFractionDigits",
    "NumberAllowedSign",
    "DateFractions",
    "PasswordMode",
    "MarkNegatives",
    "MultiLine",
    "ExtendedEdit",
    "FillFromFillingValue",
    "FillChecking",
    "ChoiceFoldersAndItems",
    "QuickChoice",
    "CreateOnInput",
    "ChoiceHistoryOnInput",
    "Indexing",
    "FullTextSearch",
    "DataHistory",
    "Mask",
    "ChoiceForm",
    "AddressingDimension",
];
const CATALOG_TABULAR_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "FillChecking",
    "Use",
    "LineNumberLength",
];
const DOCUMENT_TABULAR_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "FillChecking",
    "LineNumberLength",
];
const COMMAND_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "Group",
    "ParameterUseMode",
    "ModifiesData",
    "Representation",
    "OnMainServerUnavalableBehavior",
];

fn validate_root_object(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    schema: &[&str],
) -> Result<(), BusinessObjectBuildError> {
    let uuid = object.identity().uuid();
    if object.owner().is_some() {
        return invalid_model(uuid, "Catalog/Document root must be top-level");
    }
    if !object.references().is_empty() || !object.assets().is_empty() {
        return invalid_model(
            uuid,
            "Catalog/Document root has unsupported references or assets",
        );
    }
    require_property_schema(object, schema)?;
    let name = text_property(object, "Name")?;
    if name.is_empty() || name.contains('.') {
        return invalid_model(uuid, "Catalog/Document Name is empty or qualified");
    }
    if !matches!(
        object.provenance().source_profile().as_str(),
        "xml-2.20" | "xml-2.21"
    ) {
        return invalid_model(uuid, "source profile is not xml-2.20 or xml-2.21");
    }
    if validated.graph().object_index_by_uuid(uuid).is_none() {
        return Err(BusinessObjectBuildError::UnknownObject(uuid));
    }
    Ok(())
}

fn validate_embedded_object(
    object: &CanonicalObject,
    expected_kind: &'static str,
) -> Result<(), BusinessObjectBuildError> {
    let uuid = object.identity().uuid();
    if object.kind().as_str() != expected_kind || object.owner().is_none() {
        return invalid_model(uuid, "embedded object kind or owner is invalid");
    }
    if !object.references().is_empty() || !object.assets().is_empty() {
        return invalid_model(uuid, "embedded object has unsupported references or assets");
    }
    if !matches!(
        object.provenance().source_profile().as_str(),
        "xml-2.20" | "xml-2.21"
    ) {
        return invalid_model(uuid, "embedded object source profile is unsupported");
    }
    Ok(())
}

fn require_property_schema(
    object: &CanonicalObject,
    expected: &[&str],
) -> Result<(), BusinessObjectBuildError> {
    if object.properties().len() != expected.len()
        || object
            .properties()
            .iter()
            .zip(expected)
            .any(|(field, expected)| field.name().as_str() != *expected)
    {
        return invalid_model(
            object.identity().uuid(),
            "typed property schema is not exact",
        );
    }
    Ok(())
}

fn require_attribute_schema(
    object: &CanonicalObject,
    allowed: &[&str],
) -> Result<(), BusinessObjectBuildError> {
    let mut allowed_index = 0usize;
    for property in object.properties() {
        let Some(relative) = allowed[allowed_index..]
            .iter()
            .position(|candidate| *candidate == property.name().as_str())
        else {
            return invalid_model(
                object.identity().uuid(),
                "Attribute property schema is not exact",
            );
        };
        allowed_index += relative + 1;
    }
    for required in [
        "Name",
        "Synonym",
        "Comment",
        "Types",
        "PasswordMode",
        "MarkNegatives",
        "MultiLine",
        "ExtendedEdit",
        "FillChecking",
        "ChoiceFoldersAndItems",
        "QuickChoice",
        "CreateOnInput",
        "ChoiceHistoryOnInput",
        "Indexing",
        "FullTextSearch",
        "DataHistory",
        "Mask",
        "ChoiceForm",
    ] {
        if property_optional(object, required).is_none() {
            return invalid_model(
                object.identity().uuid(),
                "Attribute required property is missing",
            );
        }
    }
    if allowed.contains(&"FillFromFillingValue")
        && object.owner().is_some()
        && property_optional(object, "FillFromFillingValue").is_none()
        && allowed != NESTED_ATTRIBUTE_SCHEMA
    {
        return invalid_model(
            object.identity().uuid(),
            "direct Attribute fill property is missing",
        );
    }
    if allowed.contains(&"Use") && property_optional(object, "Use").is_none() {
        return invalid_model(object.identity().uuid(), "Catalog Attribute Use is missing");
    }
    Ok(())
}

fn property<'a>(
    object: &'a CanonicalObject,
    name: &str,
) -> Result<&'a CanonicalValue, BusinessObjectBuildError> {
    property_optional(object, name).ok_or(BusinessObjectBuildError::InvalidModel {
        object: object.identity().uuid(),
        reason: "required typed property is missing",
    })
}

fn property_optional<'a>(object: &'a CanonicalObject, name: &str) -> Option<&'a CanonicalValue> {
    object
        .properties()
        .iter()
        .find(|field| field.name().as_str() == name)
        .map(CanonicalField::value)
}

fn text_property<'a>(
    object: &'a CanonicalObject,
    name: &str,
) -> Result<&'a str, BusinessObjectBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::Text(value) => Ok(value.as_str()),
        _ => invalid_model(object.identity().uuid(), "typed property is not text"),
    }
}

fn text_property_optional<'a>(object: &'a CanonicalObject, name: &str) -> Option<&'a str> {
    match property_optional(object, name)?.kind() {
        CanonicalValueKind::Text(value) => Some(value.as_str()),
        _ => None,
    }
}

fn bool_property(object: &CanonicalObject, name: &str) -> Result<bool, BusinessObjectBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::Bool(value) => Ok(value),
        _ => invalid_model(object.identity().uuid(), "typed property is not boolean"),
    }
}

fn bool_token(
    object: &CanonicalObject,
    name: &str,
) -> Result<NativeValue, BusinessObjectBuildError> {
    Ok(token(if bool_property(object, name)? {
        "1"
    } else {
        "0"
    }))
}

fn u32_property(object: &CanonicalObject, name: &str) -> Result<u32, BusinessObjectBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::Integer(value) => {
            value
                .as_str()
                .parse::<u32>()
                .map_err(|_| BusinessObjectBuildError::InvalidModel {
                    object: object.identity().uuid(),
                    reason: "typed property is not u32",
                })
        }
        _ => invalid_model(object.identity().uuid(), "typed property is not an integer"),
    }
}

fn enum_property<'a>(
    object: &'a CanonicalObject,
    name: &str,
) -> Result<&'a str, BusinessObjectBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::EnumToken(value) => Ok(value.as_str()),
        _ => invalid_model(
            object.identity().uuid(),
            "typed property is not an enum token",
        ),
    }
}

fn require_enum_value(
    object: &CanonicalObject,
    name: &str,
    expected: &str,
) -> Result<(), BusinessObjectBuildError> {
    if enum_property(object, name)? != expected {
        return invalid_model(
            object.identity().uuid(),
            "enum value differs from the evidenced fixed value",
        );
    }
    Ok(())
}

fn require_bool_value(
    object: &CanonicalObject,
    name: &str,
    expected: bool,
) -> Result<(), BusinessObjectBuildError> {
    if bool_property(object, name)? != expected {
        return invalid_model(
            object.identity().uuid(),
            "boolean differs from the evidenced fixed value",
        );
    }
    Ok(())
}

fn enum_code(
    object: &CanonicalObject,
    name: &str,
    mapping: &[(&str, &str)],
) -> Result<NativeValue, BusinessObjectBuildError> {
    let value = enum_property(object, name)?;
    mapping
        .iter()
        .find_map(|(candidate, code)| (*candidate == value).then(|| token(*code)))
        .ok_or(BusinessObjectBuildError::InvalidModel {
            object: object.identity().uuid(),
            reason: "enum value has no evidenced native code",
        })
}

fn native_header(object: &CanonicalObject) -> Result<NativeValue, BusinessObjectBuildError> {
    Ok(list(vec![
        token("3"),
        list(vec![
            token("1"),
            token("0"),
            uuid_value(object.identity().uuid()),
        ]),
        text(text_property(object, "Name")?),
        localized_value(object, "Synonym", "lang")?,
        text(text_property(object, "Comment")?),
        token("0"),
        token("0"),
        token(NIL_UUID),
        token("0"),
    ]))
}

fn subsystem_picture(
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    let reference = text_property(object, "Picture")?;
    let (kind, payload) = if reference.is_empty() {
        ("0", list(vec![token("0"), token(NIL_UUID)]))
    } else if reference == "StdPicture.Print" {
        ("1", list(vec![token("-13")]))
    } else {
        let uuid = indexes.object(object.identity().uuid(), reference)?;
        if indexes.kind(uuid) != Some("CommonPicture") {
            return invalid_model(
                object.identity().uuid(),
                "Subsystem Picture is not a CommonPicture",
            );
        }
        ("1", list(vec![token("0"), uuid_value(uuid)]))
    };
    Ok(list(vec![
        token("4"),
        token(kind),
        payload,
        text(""),
        token("-1"),
        token("-1"),
        bool_token(object, "PictureLoadTransparent")?,
        token("0"),
        text(""),
    ]))
}

fn localized_value(
    object: &CanonicalObject,
    name: &str,
    language_field: &str,
) -> Result<NativeValue, BusinessObjectBuildError> {
    let values =
        property(object, name)?
            .as_sequence()
            .ok_or(BusinessObjectBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "localized property is not a sequence",
            })?;
    let mut output = vec![token(values.len().to_string())];
    let mut languages = BTreeSet::new();
    for value in values {
        let fields = value
            .as_record()
            .ok_or(BusinessObjectBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "localized item is not a record",
            })?;
        if fields.len() != 2
            || fields[0].name().as_str() != language_field
            || fields[1].name().as_str() != "content"
        {
            return invalid_model(
                object.identity().uuid(),
                "localized item schema is not exact",
            );
        }
        let language = match fields[0].value().kind() {
            CanonicalValueKind::Text(value) => value.as_str(),
            _ => return invalid_model(object.identity().uuid(), "localized language is not text"),
        };
        let content = match fields[1].value().kind() {
            CanonicalValueKind::Text(value) => value.as_str(),
            _ => return invalid_model(object.identity().uuid(), "localized content is not text"),
        };
        if !languages.insert(language) {
            return invalid_model(object.identity().uuid(), "localized language is duplicated");
        }
        output.push(text(language));
        output.push(text(content));
    }
    Ok(list(output))
}

fn generated_pairs(
    object: &CanonicalObject,
    expected_kinds: &[&str],
) -> Result<Vec<(ObjectUuid, ObjectUuid)>, BusinessObjectBuildError> {
    if object.generated_types().len() != expected_kinds.len() {
        return invalid_model(
            object.identity().uuid(),
            "generated type inventory is not exact",
        );
    }
    let mut seen = BTreeSet::new();
    object
        .generated_types()
        .iter()
        .zip(expected_kinds)
        .map(|(generated, expected)| {
            if generated.kind().as_str() != *expected {
                return invalid_model(
                    object.identity().uuid(),
                    "generated type order is not exact",
                );
            }
            let value_id = generated
                .value_id()
                .ok_or(BusinessObjectBuildError::InvalidModel {
                    object: object.identity().uuid(),
                    reason: "generated type has no ValueId",
                })?;
            if !seen.insert(generated.uuid()) || !seen.insert(value_id) {
                return invalid_model(
                    object.identity().uuid(),
                    "generated type identity is duplicated",
                );
            }
            Ok((generated.uuid(), value_id))
        })
        .collect()
}

fn put_generated_pairs(
    fields: &mut [NativeValue],
    type_slots: &[usize],
    pairs: &[(ObjectUuid, ObjectUuid)],
) {
    for (slot, pair) in type_slots.iter().zip(pairs) {
        fields[*slot] = uuid_value(pair.0);
        fields[*slot + 1] = uuid_value(pair.1);
    }
}

fn reference_sequence_targets(
    object: &CanonicalObject,
    name: &str,
    indexes: &ReferenceIndexes,
) -> Result<Vec<ObjectUuid>, BusinessObjectBuildError> {
    let values =
        property(object, name)?
            .as_sequence()
            .ok_or(BusinessObjectBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "reference collection is not a sequence",
            })?;
    let mut output = Vec::with_capacity(values.len());
    let mut seen = BTreeSet::new();
    for value in values {
        let reference = match value.kind() {
            CanonicalValueKind::Reference(value) if value.kind() == "metadata" => value.target(),
            _ => return invalid_model(object.identity().uuid(), "reference item is not metadata"),
        };
        let uuid = indexes.object(object.identity().uuid(), reference)?;
        if !seen.insert(uuid) {
            return invalid_model(object.identity().uuid(), "reference target is duplicated");
        }
        output.push(uuid);
    }
    Ok(output)
}

fn metadata_reference_collection(
    object: &CanonicalObject,
    name: &str,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    let targets = reference_sequence_targets(object, name, indexes)?;
    let mut values = vec![token("0"), token(targets.len().to_string())];
    for uuid in targets {
        values.push(list(vec![
            text("#"),
            token(METADATA_OBJECT_REF_TYPE_UUID),
            list(vec![token("1"), uuid_value(uuid)]),
        ]));
    }
    Ok(list(values))
}

fn optional_metadata_reference(
    object: &CanonicalObject,
    name: &str,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    let reference = text_property(object, name)?;
    if reference.is_empty() {
        return Ok(token(NIL_UUID));
    }
    Ok(uuid_value(
        indexes.object(object.identity().uuid(), reference)?,
    ))
}

fn optional_reference_uuid_kind(
    object: &CanonicalObject,
    name: &str,
    expected_kind: &'static str,
    indexes: &ReferenceIndexes,
) -> Result<Option<ObjectUuid>, BusinessObjectBuildError> {
    let reference = text_property(object, name)?;
    if reference.is_empty() {
        return Ok(None);
    }
    let uuid = indexes.object(object.identity().uuid(), reference)?;
    if indexes.kind(uuid) != Some(expected_kind) {
        return invalid_model(
            object.identity().uuid(),
            "metadata reference resolves to the wrong kind",
        );
    }
    Ok(Some(uuid))
}

fn optional_metadata_reference_kind(
    object: &CanonicalObject,
    name: &str,
    expected_kind: &'static str,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    Ok(
        optional_reference_uuid_kind(object, name, expected_kind, indexes)?
            .map(uuid_value)
            .unwrap_or_else(|| token(NIL_UUID)),
    )
}

fn metadata_reference_collection_kind(
    object: &CanonicalObject,
    name: &str,
    expected_kind: &'static str,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    let targets = reference_sequence_targets(object, name, indexes)?;
    if targets
        .iter()
        .any(|uuid| indexes.kind(*uuid) != Some(expected_kind))
    {
        return invalid_model(
            object.identity().uuid(),
            "metadata reference collection resolves to the wrong kind",
        );
    }
    let mut values = vec![token("0"), token(targets.len().to_string())];
    for uuid in targets {
        values.push(list(vec![
            text("#"),
            token(METADATA_OBJECT_REF_TYPE_UUID),
            list(vec![token("1"), uuid_value(uuid)]),
        ]));
    }
    Ok(list(values))
}

fn optional_owned_reference(
    object: &CanonicalObject,
    name: &str,
    owned: &[ObjectUuid],
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    let reference = text_property(object, name)?;
    if reference.is_empty() {
        return Ok(token(NIL_UUID));
    }
    let uuid = indexes.object(object.identity().uuid(), reference)?;
    if !owned.contains(&uuid) || indexes.kind(uuid) != Some("Form") {
        return invalid_model(
            object.identity().uuid(),
            "default form is not an owned child form",
        );
    }
    Ok(uuid_value(uuid))
}

fn optional_owned_member_reference(
    object: &CanonicalObject,
    name: &str,
    owned: &[ObjectUuid],
    expected_kind: &'static str,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    let reference = text_property(object, name)?;
    if reference.is_empty() {
        return Ok(token(NIL_UUID));
    }
    let uuid = indexes.object(object.identity().uuid(), reference)?;
    if indexes.kind(uuid) != Some(expected_kind)
        || indexes.owner(uuid) != Some(Some(object.identity().uuid()))
        || !owned.contains(&uuid)
    {
        return invalid_model(
            object.identity().uuid(),
            "member reference is not an owned child of the expected kind",
        );
    }
    Ok(uuid_value(uuid))
}

fn parsed_uuid_property(
    object: &CanonicalObject,
    name: &str,
) -> Result<NativeValue, BusinessObjectBuildError> {
    let value = text_property(object, name)?;
    let uuid = ObjectUuid::parse(value).map_err(|_| BusinessObjectBuildError::InvalidModel {
        object: object.identity().uuid(),
        reason: "UUID property is not canonical",
    })?;
    Ok(uuid_value(uuid))
}

fn token_is_nil(value: &NativeValue) -> bool {
    matches!(value, NativeValue::Token(value) if value == NIL_UUID)
}

fn field_reference_collection(
    object: &CanonicalObject,
    name: &str,
    family: BusinessObjectFamily,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    let values =
        property(object, name)?
            .as_sequence()
            .ok_or(BusinessObjectBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "field-reference collection is not a sequence",
            })?;
    let mut items = Vec::with_capacity(values.len());
    let mut seen = BTreeSet::new();
    for value in values {
        let reference = match value.kind() {
            CanonicalValueKind::Text(value) => value.as_str(),
            _ => return invalid_model(object.identity().uuid(), "field reference is not text"),
        };
        if !seen.insert(reference.to_ascii_lowercase()) {
            return invalid_model(object.identity().uuid(), "field reference is duplicated");
        }
        let standard_prefix = format!(
            "{}.{}.StandardAttribute.",
            family.as_str(),
            text_property(object, "Name")?
        );
        let payload = if let Some(attribute) = reference.strip_prefix(&standard_prefix) {
            let marker = standard_attribute_marker(family, attribute).ok_or(
                BusinessObjectBuildError::InvalidModel {
                    object: object.identity().uuid(),
                    reason: "standard field reference has no evidenced marker",
                },
            )?;
            list(vec![token(marker)])
        } else {
            let uuid = indexes.object(object.identity().uuid(), reference)?;
            if indexes.kind(uuid) != Some("Attribute")
                || indexes.owner(uuid) != Some(Some(object.identity().uuid()))
            {
                return invalid_model(
                    object.identity().uuid(),
                    "field reference is not a direct Attribute",
                );
            }
            list(vec![token("0"), uuid_value(uuid)])
        };
        items.push(list(vec![text("#"), token(FIELD_REF_TYPE_UUID), payload]));
    }
    let mut payload = vec![token("0"), token(items.len().to_string())];
    payload.extend(items);
    Ok(list(vec![token("1"), list(payload)]))
}

fn standard_attribute_marker(family: BusinessObjectFamily, name: &str) -> Option<&'static str> {
    let values: &[(&str, &str)] = match family {
        BusinessObjectFamily::Catalog => &[
            ("PredefinedDataName", "-13"),
            ("Predefined", "-10"),
            ("Ref", "-8"),
            ("DeletionMark", "-7"),
            ("IsFolder", "-6"),
            ("Owner", "-5"),
            ("Parent", "-4"),
            ("Description", "-3"),
            ("Code", "-2"),
        ],
        BusinessObjectFamily::Document => &[
            ("Posted", "-7"),
            ("Ref", "-5"),
            ("DeletionMark", "-4"),
            ("Date", "-3"),
            ("Number", "-2"),
        ],
        BusinessObjectFamily::ExchangePlan => &[("Description", "-3"), ("Code", "-2")],
        BusinessObjectFamily::BusinessProcess => &[("HeadTask", "-8"), ("Number", "-2")],
        BusinessObjectFamily::Task => &[
            ("Description", "-9"),
            ("BusinessProcess", "-7"),
            ("Number", "-2"),
        ],
        BusinessObjectFamily::Subsystem => &[],
    };
    values
        .iter()
        .find_map(|(candidate, marker)| (*candidate == name).then_some(*marker))
}

fn input_modes(object: &CanonicalObject) -> Result<NativeValue, BusinessObjectBuildError> {
    let search = match enum_property(object, "SearchStringModeOnInputByString")? {
        "Begin" => "1",
        "AnyPart" => "2",
        _ => {
            return invalid_model(
                object.identity().uuid(),
                "search-string mode is unsupported",
            );
        }
    };
    if enum_property(object, "FullTextSearchOnInputByString")? != "DontUse"
        || enum_property(object, "ChoiceDataGetModeOnInputByString")? != "Directly"
    {
        return invalid_model(
            object.identity().uuid(),
            "input-by-string modes are not evidenced",
        );
    }
    Ok(list(vec![token(search), token("2"), token("0")]))
}

fn type_pattern(
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    let types =
        property(object, "Types")?
            .as_sequence()
            .ok_or(BusinessObjectBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "Attribute Types is not a sequence",
            })?;
    if types.is_empty() {
        return invalid_model(object.identity().uuid(), "Attribute Types is empty");
    }
    let mut output = vec![text("Pattern")];
    let mut seen = BTreeSet::new();
    let mut has_string = false;
    let mut has_number = false;
    let mut has_date = false;
    for value in types {
        let name = match value.kind() {
            CanonicalValueKind::Text(value) => value.as_str(),
            _ => return invalid_model(object.identity().uuid(), "Attribute Type is not text"),
        };
        if !seen.insert(name.to_owned()) {
            return invalid_model(object.identity().uuid(), "Attribute Type is duplicated");
        }
        let native_type = match name {
            "xs:boolean" => list(vec![text("B")]),
            "xs:string" => {
                has_string = true;
                match (
                    property_optional(object, "StringLength"),
                    property_optional(object, "StringAllowedLength"),
                ) {
                    (None, None) => list(vec![text("S")]),
                    (Some(length), Some(allowed)) => {
                        let length = canonical_u32_value(object, length)?;
                        let allowed = canonical_text_or_enum(object, allowed)?;
                        let code = match allowed {
                            "Fixed" => "0",
                            "Variable" => "1",
                            _ => {
                                return invalid_model(
                                    object.identity().uuid(),
                                    "String AllowedLength is unsupported",
                                );
                            }
                        };
                        list(vec![text("S"), token(length.to_string()), token(code)])
                    }
                    _ => {
                        return invalid_model(
                            object.identity().uuid(),
                            "String qualifiers are incomplete",
                        );
                    }
                }
            }
            "xs:decimal" => {
                has_number = true;
                match (
                    property_optional(object, "NumberDigits"),
                    property_optional(object, "NumberFractionDigits"),
                    property_optional(object, "NumberAllowedSign"),
                ) {
                    (None, None, None) => list(vec![text("N")]),
                    (Some(digits), Some(fraction), Some(sign)) => {
                        let digits = canonical_u32_value(object, digits)?;
                        let fraction = canonical_u32_value(object, fraction)?;
                        if fraction > digits {
                            return invalid_model(
                                object.identity().uuid(),
                                "Number FractionDigits exceeds Digits",
                            );
                        }
                        let sign = match canonical_text_or_enum(object, sign)? {
                            "Any" => "0",
                            "Nonnegative" => "1",
                            _ => {
                                return invalid_model(
                                    object.identity().uuid(),
                                    "Number AllowedSign is unsupported",
                                );
                            }
                        };
                        list(vec![
                            text("N"),
                            token(digits.to_string()),
                            token(fraction.to_string()),
                            token(sign),
                        ])
                    }
                    _ => {
                        return invalid_model(
                            object.identity().uuid(),
                            "Number qualifiers are incomplete",
                        );
                    }
                }
            }
            "xs:dateTime" => {
                has_date = true;
                match property_optional(object, "DateFractions") {
                    None => list(vec![text("D")]),
                    Some(value) => match canonical_text_or_enum(object, value)? {
                        "DateTime" => list(vec![text("D")]),
                        "Date" => list(vec![text("D"), text("D")]),
                        "Time" => list(vec![text("D"), text("T")]),
                        _ => {
                            return invalid_model(
                                object.identity().uuid(),
                                "DateFractions is unsupported",
                            );
                        }
                    },
                }
            }
            reference => list(vec![
                text("#"),
                uuid_value(indexes.type_id(object.identity().uuid(), reference)?),
            ]),
        };
        output.push(native_type);
    }
    if (!has_string
        && (property_optional(object, "StringLength").is_some()
            || property_optional(object, "StringAllowedLength").is_some()))
        || (!has_number
            && (property_optional(object, "NumberDigits").is_some()
                || property_optional(object, "NumberFractionDigits").is_some()
                || property_optional(object, "NumberAllowedSign").is_some()))
        || (!has_date && property_optional(object, "DateFractions").is_some())
    {
        return invalid_model(
            object.identity().uuid(),
            "type qualifiers have no matching Type",
        );
    }
    Ok(list(output))
}

fn canonical_u32_value(
    object: &CanonicalObject,
    value: &CanonicalValue,
) -> Result<u32, BusinessObjectBuildError> {
    match value.kind() {
        CanonicalValueKind::Text(value) => {
            value
                .as_str()
                .parse::<u32>()
                .map_err(|_| BusinessObjectBuildError::InvalidModel {
                    object: object.identity().uuid(),
                    reason: "type qualifier is not u32",
                })
        }
        CanonicalValueKind::Integer(value) => {
            value
                .as_str()
                .parse::<u32>()
                .map_err(|_| BusinessObjectBuildError::InvalidModel {
                    object: object.identity().uuid(),
                    reason: "type qualifier is not u32",
                })
        }
        _ => invalid_model(
            object.identity().uuid(),
            "type qualifier is not text/integer",
        ),
    }
}

fn canonical_text_or_enum<'a>(
    object: &CanonicalObject,
    value: &'a CanonicalValue,
) -> Result<&'a str, BusinessObjectBuildError> {
    match value.kind() {
        CanonicalValueKind::Text(value) => Ok(value.as_str()),
        CanonicalValueKind::EnumToken(value) => Ok(value.as_str()),
        _ => invalid_model(object.identity().uuid(), "type qualifier is not text/enum"),
    }
}

fn builtin_type_uuid(name: &str) -> Option<ObjectUuid> {
    let value = match name {
        "v8:ValueTable" => "acf6192e-81ca-46ef-93a6-5a6968b78663",
        "v8ui:FormattedString" => "140b5ff4-37b1-4df5-b5ec-a0bfd2b94f8f",
        "v8ui:Color" => "9cd510c7-abfc-11d4-9434-004095e12fc7",
        "v8ui:Font" => "9cd510c8-abfc-11d4-9434-004095e12fc7",
        "v8:ValueStorage" => "e199ca70-93cf-46ce-a54b-6edc88c3a296",
        "v8:ValueTree" => "e603c0f2-92fb-4d47-8f38-a44a381cf235",
        "v8:UUID" => "fc01b5df-97fe-449b-83d4-218a090e681e",
        "v8:FixedStructure" => "3ee983d7-ace7-40f9-bb7e-2e916fcddd56",
        "v8:FixedArray" => "4500381b-db30-4a10-9db4-990038032acf",
        "v8:FixedMap" => "220455ea-6c85-4513-996f-bbe79ed07774",
        "cfg:AnyIBRef" => "280f5f0e-9c8a-49cc-bf6d-4d296cc17a63",
        "cfg:CatalogRef" => "e61ef7b8-f3e1-4f4b-8ac7-676e90524997",
        "cfg:DocumentRef" => "38bfd075-3e63-4aaa-a93e-94521380d579",
        _ => return None,
    };
    Some(ObjectUuid::parse(value).expect("evidenced built-in TypeId is canonical"))
}

fn builtin_command_group_uuid(name: &str) -> Option<ObjectUuid> {
    let value = match name {
        "NavigationPanelOrdinary" => "77ea1b8f-dd79-4717-9dba-5628e7f348cf",
        "NavigationPanelSeeAlso" => "bc80566a-86a5-4e87-acd4-872239385a2e",
        "NavigationPanelImportant" => "1af6d528-0b86-4fba-ab95-bd7475db03ba",
        "ActionsPanelCreate" => "4f499c31-050b-47c5-aa84-d0366c0a0da8",
        "ActionsPanelReports" => "5b360bff-01a1-49b6-93d2-26e7e8e3a038",
        "ActionsPanelTools" => "aabb34e1-98c1-4bd0-bf7f-243f95437b44",
        "FormCommandBarCreateBasedOn" => "dc2ade0f-383e-4c78-85f2-c0dabc0e2dc0",
        "FormCommandBarImportant" => "cb50f5c0-8013-4262-93a2-f0db379d6b6b",
        "FormNavigationPanelGoTo" => "eacad741-96b9-4b3a-bf79-dde9ecead1a1",
        "FormNavigationPanelSeeAlso" => "8ab1540c-0bfa-4fa6-a1e1-5d5069efc7d8",
        "FormNavigationPanelImportant" => "dc11a6be-de1f-4b64-a7a5-9b17bf4ec9f2",
        _ => return None,
    };
    Some(ObjectUuid::parse(value).expect("evidenced command-group UUID is canonical"))
}

fn native_collection(marker: &str, items: Vec<NativeValue>) -> NativeValue {
    let mut values = Vec::with_capacity(items.len() + 2);
    values.push(token(marker));
    values.push(token(items.len().to_string()));
    values.extend(items);
    list(values)
}

fn invalid_model<T>(
    object: ObjectUuid,
    reason: &'static str,
) -> Result<T, BusinessObjectBuildError> {
    Err(BusinessObjectBuildError::InvalidModel { object, reason })
}

fn native<T>(reason: impl Into<String>) -> Result<T, BusinessObjectBuildError> {
    Err(BusinessObjectBuildError::Native(reason.into()))
}

fn standard_attributes(markers: &[&str]) -> Result<NativeValue, BusinessObjectBuildError> {
    if markers.is_empty() {
        return native("standard attribute marker inventory is empty");
    }
    let descriptor_source = super::utility::SHARED_STANDARD_ATTRIBUTE_DESCRIPTOR;
    let mut source = Vec::with_capacity(UTF8_BOM.len() + descriptor_source.len());
    source.extend_from_slice(UTF8_BOM);
    source.extend_from_slice(descriptor_source.as_bytes());
    let descriptor = NativeParser::new(&source).parse()?;
    let mut body = vec![token("1"), token(markers.len().to_string())];
    for marker in markers {
        body.push(list(vec![token(*marker)]));
        body.push(token("510405d3-2a0c-4fea-960a-7fee59b32f9b"));
        body.push(descriptor.clone());
    }
    Ok(list(vec![token("1"), list(body)]))
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum NativeValue {
    Token(String),
    Text(String),
    List(Vec<NativeValue>),
}

fn token(value: impl Into<String>) -> NativeValue {
    NativeValue::Token(value.into())
}

fn text(value: impl Into<String>) -> NativeValue {
    NativeValue::Text(value.into())
}

fn list(values: Vec<NativeValue>) -> NativeValue {
    NativeValue::List(values)
}

fn uuid_value(uuid: ObjectUuid) -> NativeValue {
    token(uuid.to_string())
}

fn serialize_native(value: &NativeValue) -> Result<Vec<u8>, BusinessObjectBuildError> {
    let mut output = Vec::new();
    output.extend_from_slice(UTF8_BOM);
    write_native_value(value, &mut output, 0)?;
    if output.len() > MAX_PLAIN_BYTES {
        return Err(BusinessObjectBuildError::PlainPayloadTooLarge {
            maximum: MAX_PLAIN_BYTES,
            actual: output.len(),
        });
    }
    Ok(output)
}

fn write_native_value(
    value: &NativeValue,
    output: &mut Vec<u8>,
    depth: usize,
) -> Result<(), BusinessObjectBuildError> {
    if depth > MAX_NATIVE_DEPTH {
        return native("native value exceeds nesting bound while serializing");
    }
    match value {
        NativeValue::Token(value) => {
            if value.is_empty()
                || value.bytes().any(|byte| {
                    byte.is_ascii_whitespace() || matches!(byte, b'{' | b'}' | b',' | b'"')
                })
            {
                return native("native token contains a reserved byte");
            }
            output.extend_from_slice(value.as_bytes());
        }
        NativeValue::Text(value) => {
            output.push(b'"');
            for byte in value.as_bytes() {
                output.push(*byte);
                if *byte == b'"' {
                    output.push(b'"');
                }
            }
            output.push(b'"');
        }
        NativeValue::List(values) => {
            output.push(b'{');
            for (index, child) in values.iter().enumerate() {
                if index != 0 {
                    output.extend_from_slice(b",\r\n");
                }
                write_native_value(child, output, depth + 1)?;
            }
            output.push(b'}');
        }
    }
    if output.len() > MAX_PLAIN_BYTES {
        return Err(BusinessObjectBuildError::PlainPayloadTooLarge {
            maximum: MAX_PLAIN_BYTES,
            actual: output.len(),
        });
    }
    Ok(())
}

fn raw_deflate(plain: &[u8]) -> Result<Vec<u8>, BusinessObjectBuildError> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(plain)
        .map_err(BusinessObjectBuildError::Deflate)?;
    encoder.finish().map_err(BusinessObjectBuildError::Deflate)
}

fn inflate_bounded(blob: &[u8]) -> Result<Vec<u8>, BusinessObjectBuildError> {
    let limit = MAX_PLAIN_BYTES
        .checked_add(1)
        .expect("native plaintext bound is below usize::MAX");
    let mut decoder = DeflateDecoder::new(blob).take(limit as u64);
    let mut plain = Vec::new();
    decoder
        .read_to_end(&mut plain)
        .map_err(BusinessObjectBuildError::Inflate)?;
    if plain.len() > MAX_PLAIN_BYTES {
        return Err(BusinessObjectBuildError::PlainPayloadTooLarge {
            maximum: MAX_PLAIN_BYTES,
            actual: plain.len(),
        });
    }
    Ok(plain)
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

    fn parse(mut self) -> Result<NativeValue, BusinessObjectBuildError> {
        if !self.input.starts_with(UTF8_BOM) {
            return native("missing UTF-8 BOM");
        }
        self.offset = UTF8_BOM.len();
        let value = self.value(0)?;
        self.whitespace();
        if self.offset != self.input.len() {
            return native("trailing bytes after native root");
        }
        Ok(value)
    }

    fn value(&mut self, depth: usize) -> Result<NativeValue, BusinessObjectBuildError> {
        if depth > MAX_NATIVE_DEPTH {
            return native("native value exceeds nesting bound");
        }
        self.nodes = self.nodes.checked_add(1).ok_or_else(|| {
            BusinessObjectBuildError::Native("native node count overflow".to_owned())
        })?;
        if self.nodes > MAX_NATIVE_NODES {
            return native("native value exceeds node bound");
        }
        self.whitespace();
        match self.input.get(self.offset) {
            Some(b'{') => self.list(depth),
            Some(b'"') => self.text(),
            Some(_) => self.token(),
            None => native("unexpected end of native input"),
        }
    }

    fn list(&mut self, depth: usize) -> Result<NativeValue, BusinessObjectBuildError> {
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
                        return native("trailing comma in native list");
                    }
                }
                Some(b'}') => {
                    self.offset += 1;
                    return Ok(NativeValue::List(values));
                }
                _ => return native("expected comma or closing brace"),
            }
        }
    }

    fn text(&mut self) -> Result<NativeValue, BusinessObjectBuildError> {
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
                        .map_err(|_| {
                            BusinessObjectBuildError::Native("quoted value is not UTF-8".to_owned())
                        });
                }
            } else {
                output.push(byte);
                self.offset += 1;
            }
        }
        native("unterminated quoted value")
    }

    fn token(&mut self) -> Result<NativeValue, BusinessObjectBuildError> {
        let start = self.offset;
        while let Some(byte) = self.input.get(self.offset) {
            if matches!(byte, b',' | b'}') {
                break;
            }
            self.offset += 1;
        }
        let value = std::str::from_utf8(&self.input[start..self.offset])
            .map_err(|_| BusinessObjectBuildError::Native("native token is not UTF-8".to_owned()))?
            .trim();
        if value.is_empty() {
            return native("native token is empty");
        }
        Ok(token(value))
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

fn decode_native_ir(
    value: &NativeValue,
    family: BusinessObjectFamily,
) -> Result<BusinessObjectNativeIr, BusinessObjectBuildError> {
    if family == BusinessObjectFamily::Subsystem {
        return decode_subsystem_native_ir(value);
    }
    let root_len = if family == BusinessObjectFamily::Task {
        9
    } else {
        8
    };
    let root = exact_list(value, root_len, "business-object root")?;
    exact_token(&root[0], "1", "root discriminator")?;
    exact_token(
        &root[2],
        if family == BusinessObjectFamily::Task {
            "6"
        } else {
            "5"
        },
        "root collection count",
    )?;
    let (field_count, discriminator, generated_slots): (usize, &str, &[usize]) = match family {
        BusinessObjectFamily::Catalog => (61, "57", &[1, 3, 5, 7, 34]),
        BusinessObjectFamily::Document => (53, "40", &[1, 3, 5, 7, 26]),
        BusinessObjectFamily::ExchangePlan => (51, "37", &[1, 3, 5, 7, 9]),
        BusinessObjectFamily::BusinessProcess => (49, "30", &[3, 5, 7, 9, 11, 13]),
        BusinessObjectFamily::Task => (52, "33", &[3, 5, 7, 9, 11]),
        BusinessObjectFamily::Subsystem => unreachable!("handled above"),
    };
    let fields = exact_list(&root[1], field_count, "business-object fields")?;
    exact_token(&fields[0], discriminator, "business-object discriminator")?;
    let uuid = match family {
        BusinessObjectFamily::Catalog | BusinessObjectFamily::Document => {
            let header_wrapper = exact_list(&fields[9], 2, "business-object header wrapper")?;
            exact_token(
                &header_wrapper[0],
                "0",
                "business-object header wrapper discriminator",
            )?;
            parse_header_uuid(&header_wrapper[1])?
        }
        BusinessObjectFamily::ExchangePlan => parse_header_uuid(&fields[12])?,
        BusinessObjectFamily::BusinessProcess | BusinessObjectFamily::Task => {
            parse_header_uuid(&fields[1])?
        }
        BusinessObjectFamily::Subsystem => unreachable!("handled above"),
    };
    let mut generated_types = Vec::with_capacity(generated_slots.len());
    for slot in generated_slots.iter().copied() {
        generated_types.push((
            non_nil_uuid(&fields[slot], "generated TypeId")?,
            non_nil_uuid(&fields[slot + 1], "generated ValueId")?,
        ));
    }

    let (templates, commands, tabular_sections, attributes, forms, addressing_attributes) =
        match family {
            BusinessObjectFamily::Catalog => {
                let templates =
                    parse_uuid_collection(&root[3], TEMPLATE_COLLECTION_UUID, "templates")?;
                let command_values =
                    parse_collection(&root[4], CATALOG_COMMAND_COLLECTION_UUID, "commands")?;
                let tabular_values = parse_collection(
                    &root[5],
                    CATALOG_TABULAR_COLLECTION_UUID,
                    "tabular sections",
                )?;
                let attribute_values =
                    parse_collection(&root[6], CATALOG_ATTRIBUTE_COLLECTION_UUID, "attributes")?;
                let forms = parse_uuid_collection(&root[7], CATALOG_FORM_COLLECTION_UUID, "forms")?;
                (
                    templates,
                    parse_commands(command_values)?,
                    parse_tabular_sections(tabular_values, family)?,
                    parse_attributes(attribute_values, family, false)?,
                    forms,
                    Vec::new(),
                )
            }
            BusinessObjectFamily::Document => {
                let tabular_values = parse_collection(
                    &root[3],
                    DOCUMENT_TABULAR_COLLECTION_UUID,
                    "tabular sections",
                )?;
                let templates =
                    parse_uuid_collection(&root[4], TEMPLATE_COLLECTION_UUID, "templates")?;
                let attribute_values =
                    parse_collection(&root[5], DOCUMENT_ATTRIBUTE_COLLECTION_UUID, "attributes")?;
                let command_values =
                    parse_collection(&root[6], DOCUMENT_COMMAND_COLLECTION_UUID, "commands")?;
                let forms =
                    parse_uuid_collection(&root[7], DOCUMENT_FORM_COLLECTION_UUID, "forms")?;
                (
                    templates,
                    parse_commands(command_values)?,
                    parse_tabular_sections(tabular_values, family)?,
                    parse_attributes(attribute_values, family, false)?,
                    forms,
                    Vec::new(),
                )
            }
            BusinessObjectFamily::ExchangePlan => {
                let attribute_values =
                    parse_collection(&root[3], EXCHANGE_ATTRIBUTE_COLLECTION_UUID, "attributes")?;
                let templates =
                    parse_uuid_collection(&root[4], TEMPLATE_COLLECTION_UUID, "templates")?;
                let tabular_values = parse_collection(
                    &root[5],
                    EXCHANGE_TABULAR_COLLECTION_UUID,
                    "tabular sections",
                )?;
                let forms =
                    parse_uuid_collection(&root[6], EXCHANGE_FORM_COLLECTION_UUID, "forms")?;
                let command_values =
                    parse_collection(&root[7], EXCHANGE_COMMAND_COLLECTION_UUID, "commands")?;
                (
                    templates,
                    parse_commands(command_values)?,
                    parse_tabular_sections(tabular_values, family)?,
                    parse_attributes(attribute_values, family, false)?,
                    forms,
                    Vec::new(),
                )
            }
            BusinessObjectFamily::BusinessProcess => {
                let templates =
                    parse_uuid_collection(&root[3], TEMPLATE_COLLECTION_UUID, "templates")?;
                if !templates.is_empty() {
                    return native("BusinessProcess template collection is not empty");
                }
                let forms = parse_uuid_collection(
                    &root[4],
                    BUSINESS_PROCESS_FORM_COLLECTION_UUID,
                    "forms",
                )?;
                let command_values = parse_collection(
                    &root[5],
                    BUSINESS_PROCESS_COMMAND_COLLECTION_UUID,
                    "commands",
                )?;
                let attribute_values = parse_collection(
                    &root[6],
                    BUSINESS_PROCESS_ATTRIBUTE_COLLECTION_UUID,
                    "attributes",
                )?;
                let tabular_values = parse_collection(
                    &root[7],
                    BUSINESS_PROCESS_TABULAR_COLLECTION_UUID,
                    "tabular sections",
                )?;
                (
                    templates,
                    parse_commands(command_values)?,
                    parse_tabular_sections(tabular_values, family)?,
                    parse_attributes(attribute_values, family, false)?,
                    forms,
                    Vec::new(),
                )
            }
            BusinessObjectFamily::Task => {
                let templates =
                    parse_uuid_collection(&root[3], TEMPLATE_COLLECTION_UUID, "templates")?;
                if !templates.is_empty() {
                    return native("Task template collection is not empty");
                }
                let forms = parse_uuid_collection(&root[4], TASK_FORM_COLLECTION_UUID, "forms")?;
                let attribute_values =
                    parse_collection(&root[5], TASK_ATTRIBUTE_COLLECTION_UUID, "attributes")?;
                let addressing_values = parse_collection(
                    &root[6],
                    TASK_ADDRESSING_ATTRIBUTE_COLLECTION_UUID,
                    "addressing attributes",
                )?;
                let reserved = parse_collection(
                    &root[7],
                    TASK_RESERVED_COLLECTION_UUID,
                    "Task reserved collection",
                )?;
                if !reserved.is_empty() {
                    return native("Task reserved collection is not empty");
                }
                let command_values =
                    parse_collection(&root[8], TASK_COMMAND_COLLECTION_UUID, "commands")?;
                (
                    templates,
                    parse_commands(command_values)?,
                    Vec::new(),
                    parse_attributes(attribute_values, family, false)?,
                    forms,
                    parse_addressing_attributes(addressing_values)?,
                )
            }
            BusinessObjectFamily::Subsystem => unreachable!("handled above"),
        };
    validate_native_identity_inventory(
        uuid,
        &generated_types,
        &attributes,
        &tabular_sections,
        &commands,
        &forms,
        &templates,
        &addressing_attributes,
    )?;
    Ok(BusinessObjectNativeIr {
        family,
        uuid,
        generated_types,
        attribute_uuids: attributes,
        tabular_sections,
        command_uuids: commands,
        form_uuids: forms,
        template_uuids: templates,
        addressing_attribute_uuids: addressing_attributes,
        content_uuids: Vec::new(),
        child_subsystem_uuids: Vec::new(),
    })
}

fn decode_subsystem_native_ir(
    value: &NativeValue,
) -> Result<BusinessObjectNativeIr, BusinessObjectBuildError> {
    let root = exact_list(value, 4, "Subsystem root")?;
    exact_token(&root[0], "1", "Subsystem root discriminator")?;
    exact_token(&root[2], "1", "Subsystem root collection count")?;
    let fields = exact_list(&root[1], 9, "Subsystem fields")?;
    exact_token(&fields[0], "22", "Subsystem discriminator")?;
    let uuid = parse_header_uuid(&fields[1])?;
    let content = parse_metadata_reference_collection(&fields[7], "Subsystem Content")?;
    let children = parse_uuid_collection(&root[3], SUBSYSTEM_COLLECTION_UUID, "child Subsystems")?;
    if content.iter().copied().collect::<BTreeSet<_>>().len() != content.len()
        || children.iter().copied().collect::<BTreeSet<_>>().len() != children.len()
    {
        return native("Subsystem reference inventory contains duplicates");
    }
    Ok(BusinessObjectNativeIr {
        family: BusinessObjectFamily::Subsystem,
        uuid,
        generated_types: Vec::new(),
        attribute_uuids: Vec::new(),
        tabular_sections: Vec::new(),
        command_uuids: Vec::new(),
        form_uuids: Vec::new(),
        template_uuids: Vec::new(),
        addressing_attribute_uuids: Vec::new(),
        content_uuids: content,
        child_subsystem_uuids: children,
    })
}

fn parse_collection<'a>(
    value: &'a NativeValue,
    marker: &str,
    label: &'static str,
) -> Result<&'a [NativeValue], BusinessObjectBuildError> {
    let values = as_list(value, label)?;
    if values.len() < 2 {
        return native(format!("{label} collection is too short"));
    }
    exact_token(&values[0], marker, label)?;
    let count = usize_token(&values[1], label)?;
    if values.len() != count + 2 {
        return native(format!("{label} collection count is not exact"));
    }
    Ok(&values[2..])
}

fn parse_uuid_collection(
    value: &NativeValue,
    marker: &str,
    label: &'static str,
) -> Result<Vec<ObjectUuid>, BusinessObjectBuildError> {
    parse_collection(value, marker, label)?
        .iter()
        .map(|value| non_nil_uuid(value, label))
        .collect()
}

fn parse_metadata_reference_collection(
    value: &NativeValue,
    label: &'static str,
) -> Result<Vec<ObjectUuid>, BusinessObjectBuildError> {
    let values = as_list(value, label)?;
    if values.len() < 2 {
        return native(format!("{label} is too short"));
    }
    exact_token(&values[0], "0", label)?;
    let count = usize_token(&values[1], label)?;
    if values.len() != count + 2 {
        return native(format!("{label} count is not exact"));
    }
    values[2..]
        .iter()
        .map(|value| {
            let typed = exact_list(value, 3, label)?;
            exact_text(&typed[0], "#", label)?;
            exact_token(&typed[1], METADATA_OBJECT_REF_TYPE_UUID, label)?;
            let payload = exact_list(&typed[2], 2, label)?;
            exact_token(&payload[0], "1", label)?;
            non_nil_uuid(&payload[1], label)
        })
        .collect()
}

fn parse_attributes(
    values: &[NativeValue],
    family: BusinessObjectFamily,
    nested: bool,
) -> Result<Vec<ObjectUuid>, BusinessObjectBuildError> {
    values
        .iter()
        .map(|value| parse_attribute_uuid(value, family, nested))
        .collect()
}

fn parse_addressing_attributes(
    values: &[NativeValue],
) -> Result<Vec<ObjectUuid>, BusinessObjectBuildError> {
    values
        .iter()
        .map(|value| {
            let item = exact_list(value, 2, "AddressingAttribute item")?;
            exact_token(&item[1], "0", "AddressingAttribute item tail")?;
            let wrapper = exact_list(&item[0], 6, "AddressingAttribute wrapper")?;
            exact_token(
                &wrapper[0],
                "4",
                "AddressingAttribute wrapper discriminator",
            )?;
            let _ = non_nil_uuid(&wrapper[3], "AddressingAttribute dimension")?;
            let payload = exact_list(&wrapper[1], 23, "AddressingAttribute payload")?;
            exact_token(
                &payload[0],
                "27",
                "AddressingAttribute payload discriminator",
            )?;
            let typed = exact_list(&payload[1], 3, "AddressingAttribute typed body")?;
            exact_token(&typed[0], "2", "AddressingAttribute typed discriminator")?;
            validate_type_pattern(&typed[2])?;
            parse_header_uuid(&typed[1])
        })
        .collect()
}

fn parse_attribute_uuid(
    value: &NativeValue,
    family: BusinessObjectFamily,
    nested: bool,
) -> Result<ObjectUuid, BusinessObjectBuildError> {
    let item = exact_list(value, 2, "Attribute item")?;
    exact_token(&item[1], "0", "Attribute item tail")?;
    let wrapper = as_list(&item[0], "Attribute wrapper")?;
    let expected_len = match (family, nested) {
        (BusinessObjectFamily::Catalog, false) => 8,
        (BusinessObjectFamily::Document, false) => 5,
        (BusinessObjectFamily::ExchangePlan, false) => 7,
        (BusinessObjectFamily::BusinessProcess | BusinessObjectFamily::Task, false) => 5,
        (BusinessObjectFamily::Subsystem, false) => {
            return native("Subsystem cannot contain an Attribute");
        }
        (_, true) => 5,
    };
    if wrapper.len() != expected_len {
        return native("Attribute wrapper field count is not exact");
    }
    exact_token(
        &wrapper[0],
        match (family, nested) {
            (BusinessObjectFamily::Catalog, false) => "6",
            (BusinessObjectFamily::Document, false) => "5",
            (BusinessObjectFamily::ExchangePlan, false) => "4",
            (BusinessObjectFamily::BusinessProcess | BusinessObjectFamily::Task, false) => "2",
            (BusinessObjectFamily::Subsystem, false) => {
                return native("Subsystem cannot contain an Attribute");
            }
            (_, true) => "8",
        },
        "Attribute wrapper discriminator",
    )?;
    if family == BusinessObjectFamily::ExchangePlan && !nested {
        exact_token(&wrapper[5], "0", "ExchangePlan Attribute reserved field")?;
        let tail = exact_list(&wrapper[6], 2, "ExchangePlan Attribute reserved tail")?;
        exact_token(&tail[0], "1", "ExchangePlan Attribute reserved tail marker")?;
        exact_token(
            &tail[1],
            NIL_UUID,
            "ExchangePlan Attribute reserved tail UUID",
        )?;
    }
    let payload = exact_list(&wrapper[1], 23, "Attribute payload")?;
    exact_token(&payload[0], "27", "Attribute payload discriminator")?;
    let typed = exact_list(&payload[1], 3, "Attribute typed body")?;
    exact_token(&typed[0], "2", "Attribute typed discriminator")?;
    validate_type_pattern(&typed[2])?;
    parse_header_uuid(&typed[1])
}

fn parse_tabular_sections(
    values: &[NativeValue],
    family: BusinessObjectFamily,
) -> Result<Vec<BusinessObjectTabularNativeIr>, BusinessObjectBuildError> {
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let item = exact_list(value, 3, "TabularSection item")?;
        exact_token(&item[1], "1", "TabularSection item discriminator")?;
        let wrapper = as_list(&item[0], "TabularSection wrapper")?;
        let expected_wrapper_len = match family {
            BusinessObjectFamily::Catalog | BusinessObjectFamily::ExchangePlan => 3,
            BusinessObjectFamily::Document | BusinessObjectFamily::BusinessProcess => 2,
            BusinessObjectFamily::Task | BusinessObjectFamily::Subsystem => {
                return native("metadata family cannot contain a TabularSection");
            }
        };
        if wrapper.len() != expected_wrapper_len {
            return native("TabularSection wrapper field count is not exact");
        }
        exact_token(
            &wrapper[0],
            if family == BusinessObjectFamily::BusinessProcess {
                "0"
            } else {
                "1"
            },
            "TabularSection wrapper discriminator",
        )?;
        if family == BusinessObjectFamily::ExchangePlan {
            exact_token(&wrapper[2], "5", "ExchangePlan TabularSection layout")?;
        }
        let payload = exact_list(&wrapper[1], 9, "TabularSection payload")?;
        exact_token(&payload[0], "11", "TabularSection payload discriminator")?;
        for generated in payload.iter().take(5).skip(1) {
            let _ = non_nil_uuid(generated, "TabularSection generated identity")?;
        }
        let uuid = parse_header_uuid(&payload[5])?;
        let marker = if family == BusinessObjectFamily::Catalog {
            CATALOG_TABULAR_ATTRIBUTE_COLLECTION_UUID
        } else {
            DOCUMENT_TABULAR_ATTRIBUTE_COLLECTION_UUID
        };
        let nested = parse_collection(&item[2], marker, "TabularSection attributes")?;
        result.push(BusinessObjectTabularNativeIr {
            uuid,
            attribute_uuids: parse_attributes(nested, family, true)?,
        });
    }
    Ok(result)
}

fn parse_commands(values: &[NativeValue]) -> Result<Vec<ObjectUuid>, BusinessObjectBuildError> {
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let item = exact_list(value, 2, "Command item")?;
        exact_token(&item[1], "0", "Command item tail")?;
        let wrapper = exact_list(&item[0], 2, "Command wrapper")?;
        exact_token(&wrapper[0], "0", "Command wrapper discriminator")?;
        let nested = exact_list(&wrapper[1], 4, "Command nested wrapper")?;
        for value in &nested[..3] {
            exact_token(value, "0", "Command nested wrapper prefix")?;
        }
        let body = exact_list(&nested[3], 3, "Command body")?;
        exact_token(&body[0], "1", "Command body discriminator")?;
        let identity = exact_list(&body[1], 3, "Command identity")?;
        exact_token(&identity[0], "2", "Command identity discriminator")?;
        exact_token(&identity[2], COMMAND_VALUE_UUID, "Command ValueId")?;
        let uuid = non_nil_uuid(&identity[1], "Command UUID")?;
        let properties = exact_list(&body[2], 13, "Command properties")?;
        exact_token(&properties[0], "9", "Command properties discriminator")?;
        if parse_header_uuid(&properties[9])? != uuid {
            return native("Command identity and header UUID differ");
        }
        result.push(uuid);
    }
    Ok(result)
}

fn validate_type_pattern(value: &NativeValue) -> Result<(), BusinessObjectBuildError> {
    let values = as_list(value, "type pattern")?;
    if values.is_empty() {
        return native("type pattern is empty");
    }
    exact_text(&values[0], "Pattern", "type pattern discriminator")?;
    if values.len() == 1 {
        return native("type pattern has no item");
    }
    for item in &values[1..] {
        let fields = as_list(item, "type-pattern item")?;
        let Some(first) = fields.first() else {
            return native("type-pattern item is empty");
        };
        let kind = text_value(first, "type-pattern item kind")?;
        let valid = match kind {
            "B" => fields.len() == 1,
            "S" => fields.len() == 1 || fields.len() == 3,
            "N" => fields.len() == 1 || fields.len() == 4,
            "D" => fields.len() == 1 || fields.len() == 2,
            "#" => fields.len() == 2 && non_nil_uuid(&fields[1], "type-pattern TypeId").is_ok(),
            _ => false,
        };
        if !valid {
            return native("type-pattern item shape is unsupported");
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_native_identity_inventory(
    root: ObjectUuid,
    generated: &[(ObjectUuid, ObjectUuid)],
    attributes: &[ObjectUuid],
    sections: &[BusinessObjectTabularNativeIr],
    commands: &[ObjectUuid],
    forms: &[ObjectUuid],
    templates: &[ObjectUuid],
    addressing_attributes: &[ObjectUuid],
) -> Result<(), BusinessObjectBuildError> {
    let mut seen = BTreeSet::from([root]);
    for (type_id, value_id) in generated {
        if !seen.insert(*type_id) || !seen.insert(*value_id) {
            return native("native identity inventory contains duplicates");
        }
    }
    for uuid in attributes
        .iter()
        .chain(commands)
        .chain(forms)
        .chain(templates)
        .chain(addressing_attributes)
    {
        if !seen.insert(*uuid) {
            return native("native identity inventory contains duplicates");
        }
    }
    for section in sections {
        if !seen.insert(section.uuid) {
            return native("native identity inventory contains duplicates");
        }
        for uuid in &section.attribute_uuids {
            if !seen.insert(*uuid) {
                return native("native identity inventory contains duplicates");
            }
        }
    }
    Ok(())
}

fn parse_header_uuid(value: &NativeValue) -> Result<ObjectUuid, BusinessObjectBuildError> {
    let fields = exact_list(value, 9, "native header")?;
    exact_token(&fields[0], "3", "native header discriminator")?;
    let identity = exact_list(&fields[1], 3, "native header identity")?;
    exact_token(&identity[0], "1", "native header identity discriminator")?;
    exact_token(&identity[1], "0", "native header identity reserved slot")?;
    let uuid = non_nil_uuid(&identity[2], "native header UUID")?;
    let _ = text_value(&fields[2], "native header Name")?;
    let localized = as_list(&fields[3], "native header Synonym")?;
    if localized.is_empty() {
        return native("native header Synonym is empty-shaped");
    }
    let count = usize_token(&localized[0], "native header Synonym count")?;
    if localized.len() != count * 2 + 1 {
        return native("native header Synonym count is not exact");
    }
    for item in &localized[1..] {
        let _ = text_value(item, "native header Synonym text")?;
    }
    let _ = text_value(&fields[4], "native header Comment")?;
    exact_token(&fields[5], "0", "native header slot 5")?;
    exact_token(&fields[6], "0", "native header slot 6")?;
    exact_token(&fields[7], NIL_UUID, "native header slot 7")?;
    exact_token(&fields[8], "0", "native header slot 8")?;
    Ok(uuid)
}

fn as_list<'a>(
    value: &'a NativeValue,
    label: &str,
) -> Result<&'a [NativeValue], BusinessObjectBuildError> {
    match value {
        NativeValue::List(values) => Ok(values),
        _ => native(format!("{label} is not a list")),
    }
}

fn exact_list<'a>(
    value: &'a NativeValue,
    expected: usize,
    label: &str,
) -> Result<&'a [NativeValue], BusinessObjectBuildError> {
    let values = as_list(value, label)?;
    if values.len() != expected {
        return native(format!(
            "{label} has {} fields, expected {expected}",
            values.len()
        ));
    }
    Ok(values)
}

fn token_value<'a>(
    value: &'a NativeValue,
    label: &str,
) -> Result<&'a str, BusinessObjectBuildError> {
    match value {
        NativeValue::Token(value) => Ok(value),
        _ => native(format!("{label} is not a token")),
    }
}

fn text_value<'a>(
    value: &'a NativeValue,
    label: &str,
) -> Result<&'a str, BusinessObjectBuildError> {
    match value {
        NativeValue::Text(value) => Ok(value),
        _ => native(format!("{label} is not quoted text")),
    }
}

fn exact_token(
    value: &NativeValue,
    expected: &str,
    label: &str,
) -> Result<(), BusinessObjectBuildError> {
    if token_value(value, label)? != expected {
        return native(format!("{label} is not `{expected}`"));
    }
    Ok(())
}

fn exact_text(
    value: &NativeValue,
    expected: &str,
    label: &str,
) -> Result<(), BusinessObjectBuildError> {
    if text_value(value, label)? != expected {
        return native(format!("{label} is not quoted `{expected}`"));
    }
    Ok(())
}

fn usize_token(value: &NativeValue, label: &str) -> Result<usize, BusinessObjectBuildError> {
    let value = token_value(value, label)?;
    let parsed = value
        .parse::<usize>()
        .map_err(|_| BusinessObjectBuildError::Native(format!("{label} is not usize")))?;
    if parsed.to_string() != value {
        return native(format!("{label} is not canonical usize"));
    }
    Ok(parsed)
}

fn non_nil_uuid(value: &NativeValue, label: &str) -> Result<ObjectUuid, BusinessObjectBuildError> {
    let value = token_value(value, label)?;
    let uuid = ObjectUuid::parse(value)
        .map_err(|_| BusinessObjectBuildError::Native(format!("{label} is not UUID")))?;
    if uuid.to_string() != value || value == NIL_UUID {
        return native(format!("{label} is nil or not canonical UUID"));
    }
    Ok(uuid)
}

#[cfg(test)]
mod tests {
    use ibcmd_core::artifact::ProfileId;
    use ibcmd_core::diagnostic::{ObjectPath, PathSegment, PropertyPath};
    use ibcmd_core::family::FamilyId;
    use ibcmd_core::identity::LogicalIdentity;
    use ibcmd_core::model::{
        CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, MetadataKind,
    };
    use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
    use ibcmd_core::validate::validate_configuration;
    use ibcmd_core::value::{CanonicalField, CanonicalText, CanonicalValue};
    use ibcmd_core::version::XmlDialect;
    use ibcmd_xml::{XmlReader, bundled_metadata_registry};

    use crate::compiler::graph::{ObjectStorageRoute, build_bootstrap_graph};
    use crate::compiler::identity::collect_bootstrap_identities;

    use super::*;

    const CONFIGURATION_UUID: &str = "00000000-0000-4000-8000-000000000001";
    const CATALOG_UUID: &str = "00000000-0000-4000-8000-000000000100";
    const CATALOG_ATTRIBUTE_UUID: &str = "00000000-0000-4000-8000-000000000110";
    const CATALOG_SECTION_UUID: &str = "00000000-0000-4000-8000-000000000120";
    const CATALOG_NESTED_UUID: &str = "00000000-0000-4000-8000-000000000121";
    const CATALOG_COMMAND_UUID: &str = "00000000-0000-4000-8000-000000000130";
    const CATALOG_FORM_UUID: &str = "00000000-0000-4000-8000-000000000140";
    const CATALOG_TEMPLATE_UUID: &str = "00000000-0000-4000-8000-000000000150";
    const DOCUMENT_UUID: &str = "00000000-0000-4000-8000-000000000200";
    const DOCUMENT_ATTRIBUTE_UUID: &str = "00000000-0000-4000-8000-000000000210";
    const DOCUMENT_SECTION_UUID: &str = "00000000-0000-4000-8000-000000000220";
    const DOCUMENT_NESTED_UUID: &str = "00000000-0000-4000-8000-000000000221";
    const DOCUMENT_COMMAND_UUID: &str = "00000000-0000-4000-8000-000000000230";
    const DOCUMENT_FORM_UUID: &str = "00000000-0000-4000-8000-000000000240";
    const DOCUMENT_TEMPLATE_UUID: &str = "00000000-0000-4000-8000-000000000250";
    const SUBSYSTEM_UUID: &str = "00000000-0000-4000-8000-000000000300";
    const CHILD_SUBSYSTEM_UUID: &str = "00000000-0000-4000-8000-000000000301";
    const SUBSYSTEM_CONTENT_UUID: &str = "00000000-0000-4000-8000-000000000302";
    const EXCHANGE_UUID: &str = "00000000-0000-4000-8000-000000000400";
    const EXCHANGE_ATTRIBUTE_UUID: &str = "00000000-0000-4000-8000-000000000410";
    const EXCHANGE_SECTION_UUID: &str = "00000000-0000-4000-8000-000000000420";
    const EXCHANGE_NESTED_UUID: &str = "00000000-0000-4000-8000-000000000421";
    const EXCHANGE_COMMAND_UUID: &str = "00000000-0000-4000-8000-000000000430";
    const EXCHANGE_FORM_UUID: &str = "00000000-0000-4000-8000-000000000440";
    const EXCHANGE_TEMPLATE_UUID: &str = "00000000-0000-4000-8000-000000000450";
    const BUSINESS_PROCESS_UUID: &str = "00000000-0000-4000-8000-000000000500";
    const BUSINESS_PROCESS_ATTRIBUTE_UUID: &str = "00000000-0000-4000-8000-000000000510";
    const BUSINESS_PROCESS_SECTION_UUID: &str = "00000000-0000-4000-8000-000000000520";
    const BUSINESS_PROCESS_NESTED_UUID: &str = "00000000-0000-4000-8000-000000000521";
    const BUSINESS_PROCESS_COMMAND_UUID: &str = "00000000-0000-4000-8000-000000000530";
    const BUSINESS_PROCESS_FORM_UUID: &str = "00000000-0000-4000-8000-000000000540";
    const TASK_UUID: &str = "00000000-0000-4000-8000-000000000600";
    const TASK_ATTRIBUTE_UUID: &str = "00000000-0000-4000-8000-000000000610";
    const TASK_ADDRESSING_ATTRIBUTE_UUID: &str = "00000000-0000-4000-8000-000000000620";
    const TASK_COMMAND_UUID: &str = "00000000-0000-4000-8000-000000000630";
    const TASK_FORM_UUID: &str = "00000000-0000-4000-8000-000000000640";
    const ADDRESSING_REGISTER_UUID: &str = "00000000-0000-4000-8000-000000000700";
    const ADDRESSING_DIMENSION_UUID: &str = "00000000-0000-4000-8000-000000000710";

    fn fixture_uuid(seed: u32) -> String {
        format!("00000000-0000-4000-8000-{seed:012x}")
    }

    fn generated(prefix: &str, category: &str, seed: u32) -> String {
        format!(
            "<xr:GeneratedType name=\"{prefix}\" category=\"{category}\"><xr:TypeId>{}</xr:TypeId><xr:ValueId>{}</xr:ValueId></xr:GeneratedType>",
            fixture_uuid(seed),
            fixture_uuid(seed + 1)
        )
    }

    fn root_generated(family: BusinessObjectFamily, name: &str, seed: u32) -> String {
        ["Object", "Ref", "Selection", "List", "Manager"]
            .into_iter()
            .enumerate()
            .map(|(index, category)| {
                generated(
                    &format!("{}{category}.{name}", family.as_str()),
                    category,
                    seed + u32::try_from(index).unwrap() * 10,
                )
            })
            .collect()
    }

    fn tabular_generated(family: BusinessObjectFamily, seed: u32) -> String {
        generated(
            &format!("{}TabularSection.Lines", family.as_str()),
            "TabularSection",
            seed,
        ) + &generated(
            &format!("{}TabularSectionRow.Lines", family.as_str()),
            "TabularSectionRow",
            seed + 10,
        )
    }

    fn attribute_xml(
        family: BusinessObjectFamily,
        uuid: &str,
        name: &str,
        nested: bool,
        reference_type: bool,
    ) -> String {
        let type_body = if reference_type {
            let owner = if family == BusinessObjectFamily::Catalog {
                "Products"
            } else {
                "Invoices"
            };
            format!("<v8:Type>cfg:{}Ref.{owner}</v8:Type>", family.as_str())
        } else {
            "<v8:Type>xs:string</v8:Type><v8:StringQualifiers><v8:Length>20</v8:Length><v8:AllowedLength>Variable</v8:AllowedLength></v8:StringQualifiers>".to_owned()
        };
        let fill = if nested {
            String::new()
        } else {
            "<FillFromFillingValue>false</FillFromFillingValue><FillValue/>".to_owned()
        };
        let use_mode = if family == BusinessObjectFamily::Catalog && !nested {
            "<Use>ForItem</Use>"
        } else {
            ""
        };
        format!(
            "<Attribute uuid=\"{uuid}\"><Properties><Name>{name}</Name><Synonym/><Comment/><Type>{type_body}</Type><PasswordMode>false</PasswordMode><Format/><EditFormat/><ToolTip/><MarkNegatives>false</MarkNegatives><Mask/><MultiLine>false</MultiLine><ExtendedEdit>false</ExtendedEdit><MinValue/><MaxValue/>{fill}<FillChecking>DontCheck</FillChecking><ChoiceFoldersAndItems>Items</ChoiceFoldersAndItems><ChoiceParameterLinks/><ChoiceParameters/><QuickChoice>Auto</QuickChoice><CreateOnInput>Auto</CreateOnInput><ChoiceForm/><LinkByType/><ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput>{use_mode}<Indexing>Index</Indexing><FullTextSearch>Use</FullTextSearch><DataHistory>DontUse</DataHistory></Properties><ChildObjects/></Attribute>"
        )
    }

    fn tabular_xml(family: BusinessObjectFamily, seed: u32) -> String {
        let (section_uuid, nested_uuid) = if family == BusinessObjectFamily::Catalog {
            (CATALOG_SECTION_UUID, CATALOG_NESTED_UUID)
        } else {
            (DOCUMENT_SECTION_UUID, DOCUMENT_NESTED_UUID)
        };
        let use_mode = if family == BusinessObjectFamily::Catalog {
            "<Use>ForItem</Use>"
        } else {
            ""
        };
        format!(
            "<TabularSection uuid=\"{section_uuid}\"><InternalInfo>{}</InternalInfo><Properties><Name>Lines</Name><Synonym/><Comment/><ToolTip/><FillChecking>DontCheck</FillChecking><StandardAttributes/>{use_mode}<LineNumberLength>5</LineNumberLength></Properties><ChildObjects>{}</ChildObjects></TabularSection>",
            tabular_generated(family, seed),
            attribute_xml(family, nested_uuid, "Product", true, true)
        )
    }

    fn command_xml(uuid: &str) -> String {
        format!(
            "<Command uuid=\"{uuid}\"><Properties><Name>Open</Name><Synonym/><Comment/><Group>FormCommandBarImportant</Group><CommandParameterType/><ParameterUseMode>Single</ParameterUseMode><ModifiesData>false</ModifiesData><Representation>Auto</Representation><ToolTip/><Picture/><Shortcut/><OnMainServerUnavalableBehavior>Auto</OnMainServerUnavalableBehavior></Properties><ChildObjects/></Command>"
        )
    }

    fn catalog_xml(child_rich: bool) -> Vec<u8> {
        let default_form = if child_rich {
            "Catalog.Products.Form.Main"
        } else {
            ""
        };
        let input = if child_rich {
            "<xr:Field>Catalog.Products.Attribute.CodeText</xr:Field>"
        } else {
            ""
        };
        let children = if child_rich {
            format!(
                "{}<Form>Main</Form>{}<Template>Print</Template>{}",
                attribute_xml(
                    BusinessObjectFamily::Catalog,
                    CATALOG_ATTRIBUTE_UUID,
                    "CodeText",
                    false,
                    false
                ),
                tabular_xml(BusinessObjectFamily::Catalog, 1_200),
                command_xml(CATALOG_COMMAND_UUID),
            )
        } else {
            String::new()
        };
        format!(
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" version=\"2.20\"><Catalog uuid=\"{CATALOG_UUID}\"><InternalInfo>{}</InternalInfo><Properties><Name>Products</Name><Synonym/><Comment/><Hierarchical>false</Hierarchical><HierarchyType>HierarchyFoldersAndItems</HierarchyType><LimitLevelCount>false</LimitLevelCount><LevelCount>2</LevelCount><FoldersOnTop>true</FoldersOnTop><UseStandardCommands>true</UseStandardCommands><Owners/><SubordinationUse>ToItems</SubordinationUse><CodeLength>9</CodeLength><DescriptionLength>100</DescriptionLength><CodeType>String</CodeType><CodeAllowedLength>Variable</CodeAllowedLength><CodeSeries>WholeCatalog</CodeSeries><CheckUnique>true</CheckUnique><Autonumbering>true</Autonumbering><DefaultPresentation>AsDescription</DefaultPresentation><StandardAttributes/><Characteristics/><PredefinedDataUpdate>Auto</PredefinedDataUpdate><EditType>InDialog</EditType><QuickChoice>false</QuickChoice><ChoiceMode>BothWays</ChoiceMode><InputByString>{input}</InputByString><SearchStringModeOnInputByString>Begin</SearchStringModeOnInputByString><FullTextSearchOnInputByString>DontUse</FullTextSearchOnInputByString><ChoiceDataGetModeOnInputByString>Directly</ChoiceDataGetModeOnInputByString><DefaultObjectForm>{default_form}</DefaultObjectForm><DefaultFolderForm/><DefaultListForm/><DefaultChoiceForm/><DefaultFolderChoiceForm/><AuxiliaryObjectForm/><AuxiliaryFolderForm/><AuxiliaryListForm/><AuxiliaryChoiceForm/><AuxiliaryFolderChoiceForm/><IncludeHelpInContents>false</IncludeHelpInContents><BasedOn/><DataLockFields/><DataLockControlMode>Managed</DataLockControlMode><FullTextSearch>Use</FullTextSearch><ObjectPresentation/><ExtendedObjectPresentation/><ListPresentation/><ExtendedListPresentation/><Explanation/><CreateOnInput>Use</CreateOnInput><ChoiceHistoryOnInput>DontUse</ChoiceHistoryOnInput><DataHistory>DontUse</DataHistory><UpdateDataHistoryImmediatelyAfterWrite>false</UpdateDataHistoryImmediatelyAfterWrite><ExecuteAfterWriteDataHistoryVersionProcessing>false</ExecuteAfterWriteDataHistoryVersionProcessing></Properties><ChildObjects>{children}</ChildObjects></Catalog></MetaDataObject>",
            root_generated(BusinessObjectFamily::Catalog, "Products", 1_000),
        ).into_bytes()
    }

    fn document_xml(child_rich: bool) -> Vec<u8> {
        let default_form = if child_rich {
            "Document.Invoices.Form.Main"
        } else {
            ""
        };
        let input = if child_rich {
            "<xr:Field>Document.Invoices.StandardAttribute.Number</xr:Field>"
        } else {
            ""
        };
        let children = if child_rich {
            format!(
                "{}<Form>Main</Form>{}<Template>Print</Template>{}",
                attribute_xml(
                    BusinessObjectFamily::Document,
                    DOCUMENT_ATTRIBUTE_UUID,
                    "Description",
                    false,
                    false
                ),
                tabular_xml(BusinessObjectFamily::Document, 2_200),
                command_xml(DOCUMENT_COMMAND_UUID),
            )
        } else {
            String::new()
        };
        format!(
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" version=\"2.20\"><Document uuid=\"{DOCUMENT_UUID}\"><InternalInfo>{}</InternalInfo><Properties><Name>Invoices</Name><Synonym/><Comment/><UseStandardCommands>true</UseStandardCommands><Numerator/><NumberType>String</NumberType><NumberLength>11</NumberLength><NumberAllowedLength>Variable</NumberAllowedLength><NumberPeriodicity>Year</NumberPeriodicity><CheckUnique>true</CheckUnique><Autonumbering>true</Autonumbering><StandardAttributes/><Characteristics/><BasedOn/><InputByString>{input}</InputByString><CreateOnInput>DontUse</CreateOnInput><SearchStringModeOnInputByString>Begin</SearchStringModeOnInputByString><FullTextSearchOnInputByString>DontUse</FullTextSearchOnInputByString><ChoiceDataGetModeOnInputByString>Directly</ChoiceDataGetModeOnInputByString><DefaultObjectForm>{default_form}</DefaultObjectForm><DefaultListForm/><DefaultChoiceForm/><AuxiliaryObjectForm/><AuxiliaryListForm/><AuxiliaryChoiceForm/><Posting>Allow</Posting><RealTimePosting>Allow</RealTimePosting><RegisterRecordsDeletion>AutoDelete</RegisterRecordsDeletion><RegisterRecordsWritingOnPost>WriteSelected</RegisterRecordsWritingOnPost><SequenceFilling>AutoFill</SequenceFilling><RegisterRecords/><PostInPrivilegedMode>false</PostInPrivilegedMode><UnpostInPrivilegedMode>false</UnpostInPrivilegedMode><IncludeHelpInContents>false</IncludeHelpInContents><DataLockFields/><DataLockControlMode>Managed</DataLockControlMode><FullTextSearch>Use</FullTextSearch><ObjectPresentation/><ExtendedObjectPresentation/><ListPresentation/><ExtendedListPresentation/><Explanation/><ChoiceHistoryOnInput>DontUse</ChoiceHistoryOnInput><DataHistory>DontUse</DataHistory><UpdateDataHistoryImmediatelyAfterWrite>false</UpdateDataHistoryImmediatelyAfterWrite><ExecuteAfterWriteDataHistoryVersionProcessing>false</ExecuteAfterWriteDataHistoryVersionProcessing></Properties><ChildObjects>{children}</ChildObjects></Document></MetaDataObject>",
            root_generated(BusinessObjectFamily::Document, "Invoices", 2_000),
        ).into_bytes()
    }

    fn workflow_generated(family: BusinessObjectFamily, name: &str, seed: u32) -> String {
        let mut result = root_generated(family, name, seed);
        if family == BusinessObjectFamily::BusinessProcess {
            result.push_str(&generated(
                &format!("BusinessProcessRoutePointRef.{name}"),
                "RoutePointRef",
                seed + 50,
            ));
        }
        result
    }

    fn workflow_tabular_xml(
        family: BusinessObjectFamily,
        section_uuid: &str,
        nested_uuid: &str,
        seed: u32,
    ) -> String {
        let standard = if family == BusinessObjectFamily::ExchangePlan {
            "<StandardAttributes/>"
        } else {
            ""
        };
        format!(
            "<TabularSection uuid=\"{section_uuid}\"><InternalInfo>{}</InternalInfo><Properties><Name>Lines</Name><Synonym/><Comment/><ToolTip/><FillChecking>DontCheck</FillChecking>{standard}<LineNumberLength>5</LineNumberLength></Properties><ChildObjects>{}</ChildObjects></TabularSection>",
            tabular_generated(family, seed),
            attribute_xml(family, nested_uuid, "LineValue", true, false),
        )
    }

    fn subsystem_xml() -> Vec<u8> {
        format!(
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" version=\"2.20\"><Subsystem uuid=\"{SUBSYSTEM_UUID}\"><Properties><Name>Main</Name><Synonym/><Comment/><IncludeHelpInContents>false</IncludeHelpInContents><IncludeInCommandInterface>true</IncludeInCommandInterface><UseOneCommand>false</UseOneCommand><Explanation/><Picture/><Content><xr:Item>Constant.ContentTarget</xr:Item></Content></Properties><ChildObjects><Subsystem>Child</Subsystem></ChildObjects></Subsystem></MetaDataObject>"
        )
        .into_bytes()
    }

    fn exchange_plan_xml() -> Vec<u8> {
        let children = format!(
            "{}<Form>Main</Form>{}<Template>Print</Template>{}",
            attribute_xml(
                BusinessObjectFamily::ExchangePlan,
                EXCHANGE_ATTRIBUTE_UUID,
                "ExternalCode",
                false,
                false,
            ),
            workflow_tabular_xml(
                BusinessObjectFamily::ExchangePlan,
                EXCHANGE_SECTION_UUID,
                EXCHANGE_NESTED_UUID,
                4_200,
            ),
            command_xml(EXCHANGE_COMMAND_UUID),
        );
        format!(
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" version=\"2.20\"><ExchangePlan uuid=\"{EXCHANGE_UUID}\"><InternalInfo><xr:ThisNode>{}</xr:ThisNode>{}</InternalInfo><Properties><Name>Mobile</Name><Synonym/><Comment/><UseStandardCommands>true</UseStandardCommands><CodeLength>9</CodeLength><CodeAllowedLength>Variable</CodeAllowedLength><DescriptionLength>100</DescriptionLength><DefaultPresentation>AsDescription</DefaultPresentation><EditType>InDialog</EditType><QuickChoice>false</QuickChoice><ChoiceMode>BothWays</ChoiceMode><InputByString><xr:Field>ExchangePlan.Mobile.StandardAttribute.Description</xr:Field><xr:Field>ExchangePlan.Mobile.StandardAttribute.Code</xr:Field></InputByString><SearchStringModeOnInputByString>Begin</SearchStringModeOnInputByString><FullTextSearchOnInputByString>DontUse</FullTextSearchOnInputByString><ChoiceDataGetModeOnInputByString>Directly</ChoiceDataGetModeOnInputByString><DefaultObjectForm>ExchangePlan.Mobile.Form.Main</DefaultObjectForm><DefaultListForm/><DefaultChoiceForm/><AuxiliaryObjectForm/><AuxiliaryListForm/><AuxiliaryChoiceForm/><StandardAttributes/><Characteristics/><BasedOn/><DistributedInfoBase>true</DistributedInfoBase><IncludeConfigurationExtensions>false</IncludeConfigurationExtensions><CreateOnInput>DontUse</CreateOnInput><ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput><IncludeHelpInContents>false</IncludeHelpInContents><DataLockFields/><DataLockControlMode>Managed</DataLockControlMode><FullTextSearch>Use</FullTextSearch><ObjectPresentation/><ExtendedObjectPresentation/><ListPresentation/><ExtendedListPresentation/><Explanation/><DataHistory>DontUse</DataHistory><UpdateDataHistoryImmediatelyAfterWrite>false</UpdateDataHistoryImmediatelyAfterWrite><ExecuteAfterWriteDataHistoryVersionProcessing>false</ExecuteAfterWriteDataHistoryVersionProcessing></Properties><ChildObjects>{children}</ChildObjects></ExchangePlan></MetaDataObject>",
            fixture_uuid(4_099),
            workflow_generated(BusinessObjectFamily::ExchangePlan, "Mobile", 4_000),
        )
        .into_bytes()
    }

    fn business_process_xml() -> Vec<u8> {
        let children = format!(
            "{}<Form>Main</Form>{}{}",
            attribute_xml(
                BusinessObjectFamily::BusinessProcess,
                BUSINESS_PROCESS_ATTRIBUTE_UUID,
                "Owner",
                false,
                false,
            ),
            workflow_tabular_xml(
                BusinessObjectFamily::BusinessProcess,
                BUSINESS_PROCESS_SECTION_UUID,
                BUSINESS_PROCESS_NESTED_UUID,
                5_200,
            ),
            command_xml(BUSINESS_PROCESS_COMMAND_UUID),
        );
        format!(
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" version=\"2.20\"><BusinessProcess uuid=\"{BUSINESS_PROCESS_UUID}\"><InternalInfo>{}</InternalInfo><Properties><Name>Approval</Name><Synonym/><Comment/><UseStandardCommands>true</UseStandardCommands><EditType>InDialog</EditType><InputByString><xr:Field>BusinessProcess.Approval.StandardAttribute.Number</xr:Field></InputByString><CreateOnInput>DontUse</CreateOnInput><SearchStringModeOnInputByString>Begin</SearchStringModeOnInputByString><ChoiceDataGetModeOnInputByString>Directly</ChoiceDataGetModeOnInputByString><FullTextSearchOnInputByString>DontUse</FullTextSearchOnInputByString><DefaultObjectForm>BusinessProcess.Approval.Form.Main</DefaultObjectForm><DefaultListForm/><DefaultChoiceForm/><AuxiliaryObjectForm/><AuxiliaryListForm/><AuxiliaryChoiceForm/><ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput><NumberType>String</NumberType><NumberLength>11</NumberLength><NumberAllowedLength>Variable</NumberAllowedLength><CheckUnique>true</CheckUnique><StandardAttributes/><Characteristics/><Autonumbering>true</Autonumbering><BasedOn/><NumberPeriodicity>Nonperiodical</NumberPeriodicity><Task/><CreateTaskInPrivilegedMode>false</CreateTaskInPrivilegedMode><DataLockFields><xr:Field>BusinessProcess.Approval.StandardAttribute.HeadTask</xr:Field></DataLockFields><DataLockControlMode>Managed</DataLockControlMode><IncludeHelpInContents>false</IncludeHelpInContents><FullTextSearch>Use</FullTextSearch><ObjectPresentation/><ExtendedObjectPresentation/><ListPresentation/><ExtendedListPresentation/><Explanation/><DataHistory>DontUse</DataHistory><UpdateDataHistoryImmediatelyAfterWrite>false</UpdateDataHistoryImmediatelyAfterWrite><ExecuteAfterWriteDataHistoryVersionProcessing>false</ExecuteAfterWriteDataHistoryVersionProcessing></Properties><ChildObjects>{children}</ChildObjects></BusinessProcess></MetaDataObject>",
            workflow_generated(BusinessObjectFamily::BusinessProcess, "Approval", 5_000),
        )
        .into_bytes()
    }

    fn addressing_attribute_xml() -> String {
        format!(
            "<AddressingAttribute uuid=\"{TASK_ADDRESSING_ATTRIBUTE_UUID}\"><Properties><Name>Performer</Name><Synonym/><Comment/><Type><v8:Type>xs:string</v8:Type></Type><PasswordMode>false</PasswordMode><Format/><EditFormat/><ToolTip/><MarkNegatives>false</MarkNegatives><Mask/><MultiLine>false</MultiLine><ExtendedEdit>false</ExtendedEdit><MinValue/><MaxValue/><FillFromFillingValue>false</FillFromFillingValue><FillValue/><FillChecking>DontCheck</FillChecking><ChoiceFoldersAndItems>Items</ChoiceFoldersAndItems><ChoiceParameterLinks/><ChoiceParameters/><QuickChoice>Auto</QuickChoice><CreateOnInput>Auto</CreateOnInput><ChoiceForm/><LinkByType/><ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput><Indexing>Index</Indexing><AddressingDimension>InformationRegister.Routing.Dimension.Performer</AddressingDimension><FullTextSearch>Use</FullTextSearch><DataHistory>DontUse</DataHistory></Properties><ChildObjects/></AddressingAttribute>"
        )
    }

    fn task_xml() -> Vec<u8> {
        let children = format!(
            "{}{}<Form>Main</Form>{}",
            attribute_xml(
                BusinessObjectFamily::Task,
                TASK_ATTRIBUTE_UUID,
                "CommentText",
                false,
                false,
            ),
            addressing_attribute_xml(),
            command_xml(TASK_COMMAND_UUID),
        );
        format!(
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" version=\"2.20\"><Task uuid=\"{TASK_UUID}\"><InternalInfo>{}</InternalInfo><Properties><Name>Work</Name><Synonym/><Comment/><UseStandardCommands>true</UseStandardCommands><NumberType>String</NumberType><NumberLength>14</NumberLength><NumberAllowedLength>Fixed</NumberAllowedLength><CheckUnique>true</CheckUnique><Autonumbering>true</Autonumbering><TaskNumberAutoPrefix>BusinessProcessNumber</TaskNumberAutoPrefix><DescriptionLength>150</DescriptionLength><Addressing>InformationRegister.Routing</Addressing><MainAddressingAttribute>Task.Work.AddressingAttribute.Performer</MainAddressingAttribute><CurrentPerformer/><BasedOn/><StandardAttributes/><Characteristics/><DefaultPresentation>AsDescription</DefaultPresentation><EditType>InDialog</EditType><InputByString><xr:Field>Task.Work.StandardAttribute.Description</xr:Field><xr:Field>Task.Work.StandardAttribute.Number</xr:Field></InputByString><SearchStringModeOnInputByString>Begin</SearchStringModeOnInputByString><FullTextSearchOnInputByString>DontUse</FullTextSearchOnInputByString><ChoiceDataGetModeOnInputByString>Directly</ChoiceDataGetModeOnInputByString><CreateOnInput>DontUse</CreateOnInput><DefaultObjectForm>Task.Work.Form.Main</DefaultObjectForm><DefaultListForm/><DefaultChoiceForm/><AuxiliaryObjectForm/><AuxiliaryListForm/><AuxiliaryChoiceForm/><ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput><IncludeHelpInContents>false</IncludeHelpInContents><DataLockFields><xr:Field>Task.Work.StandardAttribute.BusinessProcess</xr:Field></DataLockFields><DataLockControlMode>Managed</DataLockControlMode><FullTextSearch>Use</FullTextSearch><ObjectPresentation/><ExtendedObjectPresentation/><ListPresentation/><ExtendedListPresentation/><Explanation/><DataHistory>DontUse</DataHistory><UpdateDataHistoryImmediatelyAfterWrite>false</UpdateDataHistoryImmediatelyAfterWrite><ExecuteAfterWriteDataHistoryVersionProcessing>false</ExecuteAfterWriteDataHistoryVersionProcessing></Properties><ChildObjects>{children}</ChildObjects></Task></MetaDataObject>",
            workflow_generated(BusinessObjectFamily::Task, "Work", 6_000),
        )
        .into_bytes()
    }

    fn simple_object(
        seed: u32,
        uuid: &str,
        kind: &str,
        name: &str,
        qualified: Option<&str>,
    ) -> CanonicalObject {
        let path =
            ObjectPath::new(vec![PathSegment::name(&format!("fixture-{seed}")).unwrap()]).unwrap();
        let provenance = SourceProvenance::new(
            ProfileId::parse("xml-2.20").unwrap(),
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
        if let Some(qualified) = qualified {
            parts.properties.push(
                CanonicalField::named(
                    "QualifiedName",
                    CanonicalValue::text(CanonicalText::new(qualified).unwrap()),
                )
                .unwrap(),
            );
        }
        CanonicalObject::new(parts).unwrap()
    }

    fn simple_owned_object(
        seed: u32,
        uuid: &str,
        kind: &str,
        name: &str,
        owner: &str,
    ) -> CanonicalObject {
        let object = simple_object(seed, uuid, kind, name, None);
        let mut parts = CanonicalObjectParts::new(
            object.identity().clone(),
            object.kind().clone(),
            object.provenance().clone(),
        );
        parts.owner = Some(ObjectUuid::parse(owner).unwrap());
        parts.properties = object.properties().to_vec();
        CanonicalObject::new(parts).unwrap()
    }

    fn hierarchical_configuration(family: BusinessObjectFamily) -> CanonicalConfiguration {
        let xml = match family {
            BusinessObjectFamily::Subsystem => subsystem_xml(),
            BusinessObjectFamily::ExchangePlan => exchange_plan_xml(),
            BusinessObjectFamily::BusinessProcess => business_process_xml(),
            BusinessObjectFamily::Task => task_xml(),
            _ => unreachable!("hierarchical fixture family is exact"),
        };
        hierarchical_configuration_from_xml(family, xml)
    }

    fn hierarchical_configuration_from_xml(
        family: BusinessObjectFamily,
        xml: Vec<u8>,
    ) -> CanonicalConfiguration {
        let document = XmlReader::from_slice(&xml).unwrap();
        let envelope = bundled_metadata_registry()
            .decode(
                &FamilyId::parse(family.as_str()).unwrap(),
                &document,
                ProfileId::parse("xml-2.20").unwrap(),
                ObjectPath::root(),
            )
            .unwrap();
        let mut objects = vec![simple_object(
            900,
            CONFIGURATION_UUID,
            "Configuration",
            "Fixture",
            None,
        )];
        objects.push(envelope.root().clone());
        objects.extend(envelope.descendants().iter().cloned());
        match family {
            BusinessObjectFamily::Subsystem => {
                objects.push(simple_object(
                    901,
                    CHILD_SUBSYSTEM_UUID,
                    "Subsystem",
                    "Child",
                    None,
                ));
                objects.push(simple_object(
                    902,
                    SUBSYSTEM_CONTENT_UUID,
                    "Constant",
                    "ContentTarget",
                    None,
                ));
            }
            BusinessObjectFamily::ExchangePlan => {
                objects.push(simple_object(
                    903,
                    EXCHANGE_FORM_UUID,
                    "Form",
                    "Main",
                    Some("ExchangePlan.Mobile.Form.Main"),
                ));
                objects.push(simple_object(
                    904,
                    EXCHANGE_TEMPLATE_UUID,
                    "Template",
                    "Print",
                    Some("ExchangePlan.Mobile.Template.Print"),
                ));
            }
            BusinessObjectFamily::BusinessProcess => objects.push(simple_object(
                905,
                BUSINESS_PROCESS_FORM_UUID,
                "Form",
                "Main",
                Some("BusinessProcess.Approval.Form.Main"),
            )),
            BusinessObjectFamily::Task => {
                objects.push(simple_object(
                    906,
                    TASK_FORM_UUID,
                    "Form",
                    "Main",
                    Some("Task.Work.Form.Main"),
                ));
                objects.push(simple_object(
                    907,
                    ADDRESSING_REGISTER_UUID,
                    "InformationRegister",
                    "Routing",
                    None,
                ));
                objects.push(simple_owned_object(
                    908,
                    ADDRESSING_DIMENSION_UUID,
                    "Dimension",
                    "Performer",
                    ADDRESSING_REGISTER_UUID,
                ));
            }
            _ => unreachable!("hierarchical fixture family is exact"),
        }
        CanonicalConfiguration::new(objects).unwrap()
    }

    fn hierarchical_root_uuid(family: BusinessObjectFamily) -> ObjectUuid {
        ObjectUuid::parse(match family {
            BusinessObjectFamily::Subsystem => SUBSYSTEM_UUID,
            BusinessObjectFamily::ExchangePlan => EXCHANGE_UUID,
            BusinessObjectFamily::BusinessProcess => BUSINESS_PROCESS_UUID,
            BusinessObjectFamily::Task => TASK_UUID,
            _ => unreachable!("hierarchical fixture family is exact"),
        })
        .unwrap()
    }

    fn compile_hierarchical_and_decode(
        family: BusinessObjectFamily,
    ) -> (BusinessObjectNativeIr, BootstrapGraph) {
        let configuration = hierarchical_configuration(family);
        let validated = validate_configuration(&configuration).unwrap();
        let identities = collect_bootstrap_identities(&validated).unwrap();
        let routes = validated
            .configuration()
            .objects()
            .iter()
            .filter(|object| object.owner().is_none())
            .map(|object| ObjectStorageRoute::new(object.identity().uuid(), Vec::new()).unwrap())
            .collect();
        let graph = build_bootstrap_graph(
            &identities,
            ProfileId::parse("platform-test").unwrap(),
            routes,
        )
        .unwrap();
        let root_uuid = hierarchical_root_uuid(family);
        let profile = BusinessObjectMetadataProfile::fixture("platform-test", family);
        let first =
            compile_business_object(&validated, &graph, root_uuid, &axes(), &profile).unwrap();
        let second =
            compile_business_object(&validated, &graph, root_uuid, &axes(), &profile).unwrap();
        assert_eq!(first, second);
        let ir = decode_business_object_blob(
            first.outcome().compiled_payload().unwrap().bytes(),
            &profile,
        )
        .unwrap();
        (ir, graph)
    }

    fn axes() -> CompileAxes {
        CompileAxes::new(
            XmlDialect::parse("2.20").unwrap(),
            Some(PlatformBuild::parse("8.3.27.1989").unwrap()),
            None,
            StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            None,
        )
    }

    fn configuration(family: BusinessObjectFamily, child_rich: bool) -> CanonicalConfiguration {
        let xml = if family == BusinessObjectFamily::Catalog {
            catalog_xml(child_rich)
        } else {
            document_xml(child_rich)
        };
        let document = XmlReader::from_slice(&xml).unwrap();
        let envelope = bundled_metadata_registry()
            .decode(
                &FamilyId::parse(family.as_str()).unwrap(),
                &document,
                ProfileId::parse("xml-2.20").unwrap(),
                ObjectPath::root(),
            )
            .unwrap();
        let mut objects = vec![simple_object(
            1,
            CONFIGURATION_UUID,
            "Configuration",
            "Fixture",
            None,
        )];
        objects.push(envelope.root().clone());
        objects.extend(envelope.descendants().iter().cloned());
        if child_rich {
            let (form_uuid, template_uuid, owner) = if family == BusinessObjectFamily::Catalog {
                (CATALOG_FORM_UUID, CATALOG_TEMPLATE_UUID, "Catalog.Products")
            } else {
                (
                    DOCUMENT_FORM_UUID,
                    DOCUMENT_TEMPLATE_UUID,
                    "Document.Invoices",
                )
            };
            objects.push(simple_object(
                2,
                form_uuid,
                "Form",
                "Main",
                Some(&format!("{owner}.Form.Main")),
            ));
            objects.push(simple_object(
                3,
                template_uuid,
                "Template",
                "Print",
                Some(&format!("{owner}.Template.Print")),
            ));
        }
        CanonicalConfiguration::new(objects).unwrap()
    }

    fn compile_and_decode(
        family: BusinessObjectFamily,
        child_rich: bool,
    ) -> (BusinessObjectNativeIr, BootstrapGraph) {
        let configuration = configuration(family, child_rich);
        let validated = validate_configuration(&configuration).unwrap();
        let identities = collect_bootstrap_identities(&validated).unwrap();
        let routes = validated
            .configuration()
            .objects()
            .iter()
            .filter(|object| object.owner().is_none())
            .map(|object| ObjectStorageRoute::new(object.identity().uuid(), Vec::new()).unwrap())
            .collect();
        let graph = build_bootstrap_graph(
            &identities,
            ProfileId::parse("platform-test").unwrap(),
            routes,
        )
        .unwrap();
        let root_uuid = ObjectUuid::parse(match family {
            BusinessObjectFamily::Catalog => CATALOG_UUID,
            BusinessObjectFamily::Document => DOCUMENT_UUID,
            _ => unreachable!("legacy fixture covers Catalog and Document only"),
        })
        .unwrap();
        let profile = BusinessObjectMetadataProfile::fixture("platform-test", family);
        let first =
            compile_business_object(&validated, &graph, root_uuid, &axes(), &profile).unwrap();
        let second =
            compile_business_object(&validated, &graph, root_uuid, &axes(), &profile).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.target().key().as_str(), root_uuid.to_string());
        let ir = decode_business_object_blob(
            first.outcome().compiled_payload().unwrap().bytes(),
            &profile,
        )
        .unwrap();
        (ir, graph)
    }

    #[test]
    fn minimal_catalog_and_document_are_deterministic_and_base_free() {
        for family in [
            BusinessObjectFamily::Catalog,
            BusinessObjectFamily::Document,
        ] {
            let (ir, _) = compile_and_decode(family, false);
            assert_eq!(ir.family, family);
            assert_eq!(ir.generated_types.len(), 5);
            assert!(ir.attribute_uuids.is_empty());
            assert!(ir.tabular_sections.is_empty());
            assert!(ir.command_uuids.is_empty());
            assert!(ir.form_uuids.is_empty());
            assert!(ir.template_uuids.is_empty());
        }
    }

    #[test]
    fn child_rich_catalog_and_document_have_exact_embedded_inventory() {
        for family in [
            BusinessObjectFamily::Catalog,
            BusinessObjectFamily::Document,
        ] {
            let (ir, graph) = compile_and_decode(family, true);
            assert_eq!(ir.attribute_uuids.len(), 1);
            assert_eq!(ir.tabular_sections.len(), 1);
            assert_eq!(ir.tabular_sections[0].attribute_uuids.len(), 1);
            assert_eq!(ir.command_uuids.len(), 1);
            assert_eq!(ir.form_uuids.len(), 1);
            assert_eq!(ir.template_uuids.len(), 1);
            for embedded in ir
                .attribute_uuids
                .iter()
                .chain(ir.command_uuids.iter())
                .chain(ir.tabular_sections.iter().map(|section| &section.uuid))
                .chain(
                    ir.tabular_sections
                        .iter()
                        .flat_map(|section| section.attribute_uuids.iter()),
                )
            {
                assert!(!graph.contains_key(&embedded.to_string()));
            }
            for separate in ir.form_uuids.iter().chain(&ir.template_uuids) {
                assert!(graph.contains_key(&separate.to_string()));
            }
        }
    }

    #[test]
    fn hierarchical_and_workflow_families_are_deterministic_and_base_free() {
        let (subsystem, subsystem_graph) =
            compile_hierarchical_and_decode(BusinessObjectFamily::Subsystem);
        assert!(subsystem.generated_types.is_empty());
        assert_eq!(subsystem.content_uuids.len(), 1);
        assert_eq!(subsystem.child_subsystem_uuids.len(), 1);
        assert!(subsystem_graph.contains_key(SUBSYSTEM_CONTENT_UUID));
        assert!(subsystem_graph.contains_key(CHILD_SUBSYSTEM_UUID));

        let (exchange, exchange_graph) =
            compile_hierarchical_and_decode(BusinessObjectFamily::ExchangePlan);
        assert_eq!(exchange.generated_types.len(), 5);
        assert_eq!(
            exchange.attribute_uuids,
            [ObjectUuid::parse(EXCHANGE_ATTRIBUTE_UUID).unwrap()]
        );
        assert_eq!(exchange.tabular_sections.len(), 1);
        assert_eq!(exchange.tabular_sections[0].attribute_uuids.len(), 1);
        assert_eq!(exchange.command_uuids.len(), 1);
        assert_eq!(exchange.form_uuids.len(), 1);
        assert_eq!(exchange.template_uuids.len(), 1);
        assert!(!exchange_graph.contains_key(EXCHANGE_ATTRIBUTE_UUID));
        assert!(exchange_graph.contains_key(EXCHANGE_FORM_UUID));

        let (process, process_graph) =
            compile_hierarchical_and_decode(BusinessObjectFamily::BusinessProcess);
        assert_eq!(process.generated_types.len(), 6);
        assert_eq!(process.attribute_uuids.len(), 1);
        assert_eq!(process.tabular_sections.len(), 1);
        assert_eq!(process.tabular_sections[0].attribute_uuids.len(), 1);
        assert_eq!(process.command_uuids.len(), 1);
        assert_eq!(process.form_uuids.len(), 1);
        assert!(process.template_uuids.is_empty());
        assert!(!process_graph.contains_key(BUSINESS_PROCESS_SECTION_UUID));

        let (task, task_graph) = compile_hierarchical_and_decode(BusinessObjectFamily::Task);
        assert_eq!(task.generated_types.len(), 5);
        assert_eq!(task.attribute_uuids.len(), 1);
        assert_eq!(task.addressing_attribute_uuids.len(), 1);
        assert_eq!(task.command_uuids.len(), 1);
        assert_eq!(task.form_uuids.len(), 1);
        assert!(!task_graph.contains_key(TASK_ADDRESSING_ATTRIBUTE_UUID));
        assert!(task_graph.contains_key(ADDRESSING_REGISTER_UUID));
        assert!(!task_graph.contains_key(ADDRESSING_DIMENSION_UUID));
    }

    #[test]
    fn task_member_reference_must_resolve_to_its_owned_addressing_attribute() {
        let xml = String::from_utf8(task_xml()).unwrap().replace(
            "Task.Work.AddressingAttribute.Performer</MainAddressingAttribute>",
            "Task.Work.Attribute.CommentText</MainAddressingAttribute>",
        );
        let configuration =
            hierarchical_configuration_from_xml(BusinessObjectFamily::Task, xml.into_bytes());
        let validated = validate_configuration(&configuration).unwrap();
        let identities = collect_bootstrap_identities(&validated).unwrap();
        let routes = validated
            .configuration()
            .objects()
            .iter()
            .filter(|object| object.owner().is_none())
            .map(|object| ObjectStorageRoute::new(object.identity().uuid(), Vec::new()).unwrap())
            .collect();
        let graph = build_bootstrap_graph(
            &identities,
            ProfileId::parse("platform-test").unwrap(),
            routes,
        )
        .unwrap();
        let profile =
            BusinessObjectMetadataProfile::fixture("platform-test", BusinessObjectFamily::Task);
        assert!(matches!(
            compile_business_object(
                &validated,
                &graph,
                ObjectUuid::parse(TASK_UUID).unwrap(),
                &axes(),
                &profile,
            ),
            Err(BusinessObjectBuildError::InvalidModel { .. })
        ));
    }

    #[test]
    fn bundled_profile_selects_business_object_layouts_explicitly() {
        let registry = crate::profile_registry::load_bundled_profile_registry().unwrap();
        let effective = registry
            .get(&ProfileId::parse("platform-8.3.27.1989").unwrap())
            .unwrap();

        for family in [
            BusinessObjectFamily::Catalog,
            BusinessObjectFamily::Document,
            BusinessObjectFamily::Subsystem,
            BusinessObjectFamily::ExchangePlan,
            BusinessObjectFamily::BusinessProcess,
            BusinessObjectFamily::Task,
        ] {
            assert_eq!(
                BusinessObjectMetadataProfile::from_effective(effective, family)
                    .unwrap()
                    .family,
                family
            );
        }

        let mut missing = effective.clone();
        missing.constants.remove(CATALOG_LAYOUT_KEY);
        assert!(matches!(
            BusinessObjectMetadataProfile::from_effective(&missing, BusinessObjectFamily::Catalog),
            Err(BusinessObjectProfileError::MissingConstant {
                key: CATALOG_LAYOUT_KEY,
                ..
            })
        ));

        let mut future = effective.clone();
        future.constants.get_mut(DOCUMENT_LAYOUT_KEY).unwrap().value =
            "document-v2-future".to_owned();
        assert!(matches!(
            BusinessObjectMetadataProfile::from_effective(&future, BusinessObjectFamily::Document),
            Err(BusinessObjectProfileError::UnsupportedLayout {
                family: BusinessObjectFamily::Document,
                key: DOCUMENT_LAYOUT_KEY,
                ..
            })
        ));
    }

    #[test]
    fn native_unknown_marker_and_extra_root_field_fail_closed() {
        for family in [
            BusinessObjectFamily::Catalog,
            BusinessObjectFamily::Document,
        ] {
            let configuration = configuration(family, false);
            let validated = validate_configuration(&configuration).unwrap();
            let identities = collect_bootstrap_identities(&validated).unwrap();
            let routes = validated
                .configuration()
                .objects()
                .iter()
                .filter(|object| object.owner().is_none())
                .map(|object| {
                    ObjectStorageRoute::new(object.identity().uuid(), Vec::new()).unwrap()
                })
                .collect();
            let graph = build_bootstrap_graph(
                &identities,
                ProfileId::parse("platform-test").unwrap(),
                routes,
            )
            .unwrap();
            let root_uuid = ObjectUuid::parse(match family {
                BusinessObjectFamily::Catalog => CATALOG_UUID,
                BusinessObjectFamily::Document => DOCUMENT_UUID,
                _ => unreachable!("legacy fixture covers Catalog and Document only"),
            })
            .unwrap();
            let profile = BusinessObjectMetadataProfile::fixture("platform-test", family);
            let entry =
                compile_business_object(&validated, &graph, root_uuid, &axes(), &profile).unwrap();
            let plain =
                inflate_bounded(entry.outcome().compiled_payload().unwrap().bytes()).unwrap();
            let parsed = NativeParser::new(&plain).parse().unwrap();

            let mut unknown_marker = parsed.clone();
            let NativeValue::List(root) = &mut unknown_marker else {
                panic!("compiler must emit a root list");
            };
            root[2] = token("6");
            let blob = raw_deflate(&serialize_native(&unknown_marker).unwrap()).unwrap();
            assert!(matches!(
                decode_business_object_blob(&blob, &profile),
                Err(BusinessObjectBuildError::Native(_))
            ));

            let mut extra_field = parsed;
            let NativeValue::List(root) = &mut extra_field else {
                panic!("compiler must emit a root list");
            };
            root.push(token("future"));
            let blob = raw_deflate(&serialize_native(&extra_field).unwrap()).unwrap();
            assert!(matches!(
                decode_business_object_blob(&blob, &profile),
                Err(BusinessObjectBuildError::Native(_))
            ));
        }
    }
}
