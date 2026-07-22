//! Shared standalone native codec for Report, DataProcessor, Enum and SettingsStorage.

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

pub(crate) const REPORT_LAYOUT_KEY: &str = "bootstrap.metadata.report.layout";
pub(crate) const REPORT_LAYOUT: &str = "report-v1-crlf-utf8-bom";
pub(crate) const DATA_PROCESSOR_LAYOUT_KEY: &str = "bootstrap.metadata.data_processor.layout";
pub(crate) const DATA_PROCESSOR_LAYOUT: &str = "data-processor-v1-crlf-utf8-bom";
pub(crate) const ENUM_LAYOUT_KEY: &str = "bootstrap.metadata.enum.layout";
pub(crate) const ENUM_LAYOUT: &str = "enum-v1-crlf-utf8-bom";
pub(crate) const SETTINGS_STORAGE_LAYOUT_KEY: &str = "bootstrap.metadata.settings_storage.layout";
pub(crate) const SETTINGS_STORAGE_LAYOUT: &str = "settings-storage-v1-crlf-utf8-bom";

const SUPPORTED_STORAGE_PROFILE: &str = "storage:mssql-config-configsave";
const UTF8_BOM: &[u8; 3] = b"\xef\xbb\xbf";
const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";
const COMMAND_VALUE_UUID: &str = "078a6af8-d22c-4248-9c33-7e90075a3d2c";

const TEMPLATE_COLLECTION_UUID: &str = "3daea016-69b7-4ed4-9453-127911372fe6";
const REPORT_ATTRIBUTE_COLLECTION_UUID: &str = "7e7123e0-29e2-11d6-a3c7-0050bae0a776";
const REPORT_FORM_COLLECTION_UUID: &str = "a3b368c0-29e2-11d6-a3c7-0050bae0a776";
const REPORT_TABULAR_COLLECTION_UUID: &str = "b077d780-29e2-11d6-a3c7-0050bae0a776";
const REPORT_TABULAR_ATTRIBUTE_COLLECTION_UUID: &str = "c339c860-29e2-11d6-a3c7-0050bae0a776";
const REPORT_COMMAND_COLLECTION_UUID: &str = "e7ff38c0-ec3c-47a0-ae90-20c73ca72246";
const PROCESSOR_TABULAR_COLLECTION_UUID: &str = "2bcef0d1-0981-11d6-b9b8-0050bae0a95d";
const PROCESSOR_COMMAND_COLLECTION_UUID: &str = "45556acb-826a-4f73-898a-6025fc9536e1";
const PROCESSOR_FORM_COLLECTION_UUID: &str = "d5b0e5ed-256d-401c-9c36-f630cafd8a62";
const PROCESSOR_ATTRIBUTE_COLLECTION_UUID: &str = "ec6bb5e5-b7a8-4d75-bec9-658107a699cf";
const PROCESSOR_TABULAR_ATTRIBUTE_COLLECTION_UUID: &str = "5d24a9d1-098e-11d6-b9b8-0050bae0a95d";
const ENUM_FORM_COLLECTION_UUID: &str = "33f2e54b-37ce-4a7a-a569-b648d7aa4634";
const ENUM_RESERVED_COLLECTION_UUID: &str = "6d8d73a7-ba29-401d-9032-3872ec2d6433";
const ENUM_VALUE_COLLECTION_UUID: &str = "bee0a08c-07eb-40c0-8544-5c364c171465";
const SETTINGS_FORM_COLLECTION_UUID: &str = "b8533c0c-2342-4db3-91a2-c2b08cbf6b23";

const MAX_PLAIN_BYTES: usize = 64 * 1_048_576;
const MAX_NATIVE_DEPTH: usize = 32;
const MAX_NATIVE_NODES: usize = 500_000;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum UtilityFamily {
    Report,
    DataProcessor,
    Enum,
    SettingsStorage,
}

impl UtilityFamily {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Report => "Report",
            Self::DataProcessor => "DataProcessor",
            Self::Enum => "Enum",
            Self::SettingsStorage => "SettingsStorage",
        }
    }

    const fn layout_key(self) -> &'static str {
        match self {
            Self::Report => REPORT_LAYOUT_KEY,
            Self::DataProcessor => DATA_PROCESSOR_LAYOUT_KEY,
            Self::Enum => ENUM_LAYOUT_KEY,
            Self::SettingsStorage => SETTINGS_STORAGE_LAYOUT_KEY,
        }
    }

    const fn layout_value(self) -> &'static str {
        match self {
            Self::Report => REPORT_LAYOUT,
            Self::DataProcessor => DATA_PROCESSOR_LAYOUT,
            Self::Enum => ENUM_LAYOUT,
            Self::SettingsStorage => SETTINGS_STORAGE_LAYOUT,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UtilityMetadataProfile {
    profile_id: ProfileId,
    platform_build: PlatformBuild,
    storage_profile: StorageProfileId,
    family: UtilityFamily,
}

impl UtilityMetadataProfile {
    pub(crate) fn from_effective(
        profile: &EffectiveProfile,
        family: UtilityFamily,
    ) -> Result<Self, UtilityProfileError> {
        let platform_build = profile
            .platform_build
            .as_ref()
            .map(|value| value.value.clone())
            .ok_or_else(|| UtilityProfileError::MissingCoordinate {
                profile: profile.id.clone(),
                coordinate: "platform_build",
            })?;
        let storage_profile = profile
            .storage_profile
            .as_ref()
            .map(|value| value.value.clone())
            .ok_or_else(|| UtilityProfileError::MissingCoordinate {
                profile: profile.id.clone(),
                coordinate: "storage_profile",
            })?;
        if storage_profile.as_str() != SUPPORTED_STORAGE_PROFILE {
            return Err(UtilityProfileError::UnsupportedCoordinate {
                profile: profile.id.clone(),
                coordinate: "storage_profile",
                value: storage_profile.to_string(),
            });
        }
        let constant = profile.constants.get(family.layout_key()).ok_or_else(|| {
            UtilityProfileError::MissingConstant {
                profile: profile.id.clone(),
                key: family.layout_key(),
            }
        })?;
        if constant.value != family.layout_value() {
            return Err(UtilityProfileError::UnsupportedLayout {
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
        })
    }

    #[cfg(test)]
    fn fixture(profile_id: &str, family: UtilityFamily) -> Self {
        Self {
            profile_id: ProfileId::parse(profile_id).unwrap(),
            platform_build: PlatformBuild::parse("8.3.27.1989").unwrap(),
            storage_profile: StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            family,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UtilityProfileError {
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
        family: UtilityFamily,
        key: &'static str,
        value: String,
    },
}

impl Display for UtilityProfileError {
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

impl Error for UtilityProfileError {}

#[derive(Debug)]
pub enum UtilityBuildError {
    Profile(UtilityProfileError),
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
        expected: UtilityFamily,
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

impl Display for UtilityBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(source) => write!(formatter, "unsupported utility profile: {source}"),
            Self::ProfileMismatch { graph, codec } => write!(
                formatter,
                "bootstrap graph profile `{graph}` differs from utility profile `{codec}`"
            ),
            Self::AxisMismatch {
                axis,
                expected,
                actual,
            } => write!(
                formatter,
                "utility metadata `{axis}` axis mismatch: expected `{expected}`, got `{actual}`"
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
            Self::InvalidModel { object, reason } => {
                write!(
                    formatter,
                    "object {object} is not compilable utility metadata: {reason}"
                )
            }
            Self::Native(reason) => write!(formatter, "invalid native utility row: {reason}"),
            Self::PlainPayloadTooLarge { maximum, actual } => write!(
                formatter,
                "native utility plaintext has {actual} bytes, exceeding the {maximum}-byte bound"
            ),
            Self::Deflate(source) => {
                write!(formatter, "failed to raw-deflate utility row: {source}")
            }
            Self::Inflate(source) => write!(formatter, "failed to inflate utility row: {source}"),
            Self::Storage(source) => write!(formatter, "invalid utility storage target: {source}"),
            Self::Patch(source) => write!(formatter, "invalid utility storage payload: {source}"),
        }
    }
}

impl Error for UtilityBuildError {
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

impl From<UtilityProfileError> for UtilityBuildError {
    fn from(source: UtilityProfileError) -> Self {
        Self::Profile(source)
    }
}

impl From<StorageBuildError> for UtilityBuildError {
    fn from(source: StorageBuildError) -> Self {
        Self::Storage(source)
    }
}

impl From<StoragePatchBuildError> for UtilityBuildError {
    fn from(source: StoragePatchBuildError) -> Self {
        Self::Patch(source)
    }
}

pub(crate) fn compile_utility_metadata(
    validated: &ValidatedConfiguration<'_>,
    graph: &BootstrapGraph,
    object_uuid: ObjectUuid,
    axes: &CompileAxes,
    profile: &UtilityMetadataProfile,
) -> Result<StoragePatchEntry, UtilityBuildError> {
    validate_coordinates(graph, axes, profile)?;
    let object_index = validated
        .graph()
        .object_index_by_uuid(object_uuid)
        .ok_or(UtilityBuildError::UnknownObject(object_uuid))?;
    let object = &validated.configuration().objects()[object_index];
    if object.kind().as_str() != profile.family.as_str() {
        return Err(UtilityBuildError::FamilyMismatch {
            expected: profile.family,
            actual: object.kind().as_str().to_owned(),
        });
    }
    let expected_source_profile = format!("xml-{}", axes.xml_dialect());
    if object.provenance().source_profile().as_str() != expected_source_profile {
        return Err(UtilityBuildError::AxisMismatch {
            axis: "xml_dialect",
            expected: expected_source_profile,
            actual: object.provenance().source_profile().to_string(),
        });
    }
    let route = graph
        .primary_object_entry(object_uuid)
        .ok_or(UtilityBuildError::MissingPrimaryRoute(object_uuid))?;
    let indexes = ReferenceIndexes::build(validated, object_uuid)?;
    let root = match profile.family {
        UtilityFamily::Report => build_report(validated, object, &indexes)?,
        UtilityFamily::DataProcessor => build_data_processor(validated, object, &indexes)?,
        UtilityFamily::Enum => build_enum(validated, object, &indexes)?,
        UtilityFamily::SettingsStorage => build_settings_storage(validated, object, &indexes)?,
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

fn validate_coordinates(
    graph: &BootstrapGraph,
    axes: &CompileAxes,
    profile: &UtilityMetadataProfile,
) -> Result<(), UtilityBuildError> {
    if graph.profile_id() != &profile.profile_id {
        return Err(UtilityBuildError::ProfileMismatch {
            graph: graph.profile_id().clone(),
            codec: profile.profile_id.clone(),
        });
    }
    let actual_platform = axes
        .platform_build()
        .map(ToString::to_string)
        .unwrap_or_else(|| "<missing>".to_owned());
    if axes.platform_build() != Some(&profile.platform_build) {
        return Err(UtilityBuildError::AxisMismatch {
            axis: "platform_build",
            expected: profile.platform_build.to_string(),
            actual: actual_platform,
        });
    }
    if axes.storage_profile() != &profile.storage_profile {
        return Err(UtilityBuildError::AxisMismatch {
            axis: "storage_profile",
            expected: profile.storage_profile.to_string(),
            actual: axes.storage_profile().to_string(),
        });
    }
    if axes.compatibility_mode().is_some() || axes.container_revision().is_some() {
        return Err(UtilityBuildError::AxisMismatch {
            axis: "unevidenced optional coordinate",
            expected: "<unspecified for evidenced layout>".to_owned(),
            actual: "specified".to_owned(),
        });
    }
    if !matches!(axes.xml_dialect().to_string().as_str(), "2.20" | "2.21") {
        return Err(UtilityBuildError::AxisMismatch {
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
}

impl ReferenceIndexes {
    fn build(
        validated: &ValidatedConfiguration<'_>,
        compiling: ObjectUuid,
    ) -> Result<Self, UtilityBuildError> {
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
        for object in validated.configuration().objects() {
            let Some(name) = text_property_optional(object, "Name") else {
                continue;
            };
            if name.is_empty() || name.contains('.') {
                continue;
            }
            for generated in object.generated_types() {
                let category = generated.kind().as_str();
                let readable = if object.kind().as_str() == "DefinedType"
                    && category == "DefinedType"
                {
                    format!("cfg:DefinedType.{name}")
                } else if object.kind().as_str() == "TabularSection" {
                    let owner_uuid = object.owner().ok_or(UtilityBuildError::InvalidModel {
                        object: compiling,
                        reason: "TabularSection generated type has no owner",
                    })?;
                    let owner_index = validated.graph().object_index_by_uuid(owner_uuid).ok_or(
                        UtilityBuildError::InvalidModel {
                            object: compiling,
                            reason: "TabularSection owner is missing",
                        },
                    )?;
                    let owner = &validated.configuration().objects()[owner_index];
                    let owner_name = text_property_optional(owner, "Name").ok_or(
                        UtilityBuildError::InvalidModel {
                            object: compiling,
                            reason: "TabularSection owner has no Name",
                        },
                    )?;
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
        }
        let kinds = validated
            .configuration()
            .objects()
            .iter()
            .map(|object| (object.identity().uuid(), object.kind().as_str().to_owned()))
            .collect();
        Ok(Self {
            objects,
            generated_types,
            kinds,
        })
    }

    fn object(
        &self,
        compiling: ObjectUuid,
        reference: &str,
    ) -> Result<ObjectUuid, UtilityBuildError> {
        self.objects
            .get(reference)
            .copied()
            .ok_or(UtilityBuildError::InvalidModel {
                object: compiling,
                reason: "readable metadata reference is unresolved",
            })
    }

    fn type_id(
        &self,
        compiling: ObjectUuid,
        reference: &str,
    ) -> Result<ObjectUuid, UtilityBuildError> {
        builtin_type_uuid(reference)
            .or_else(|| self.generated_types.get(reference).copied())
            .ok_or(UtilityBuildError::InvalidModel {
                object: compiling,
                reason: "readable Type reference is unresolved",
            })
    }

    fn kind(&self, uuid: ObjectUuid) -> Option<&str> {
        self.kinds.get(&uuid).map(String::as_str)
    }
}

fn insert_reference(
    values: &mut BTreeMap<String, ObjectUuid>,
    reference: String,
    uuid: ObjectUuid,
    compiling: ObjectUuid,
) -> Result<(), UtilityBuildError> {
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

const REPORT_PROPERTY_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "DefaultForm",
    "AuxiliaryForm",
    "MainDataCompositionSchema",
    "DefaultSettingsForm",
    "AuxiliarySettingsForm",
    "DefaultVariantForm",
    "VariantsStorage",
    "SettingsStorage",
    "IncludeHelpInContents",
    "ExtendedPresentation",
    "Explanation",
    "ChildForms",
    "ChildTemplates",
];

const DATA_PROCESSOR_PROPERTY_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "DefaultForm",
    "AuxiliaryForm",
    "IncludeHelpInContents",
    "ExtendedPresentation",
    "Explanation",
    "ChildForms",
    "ChildTemplates",
];

const ENUM_PROPERTY_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "HasStandardAttributes",
    "QuickChoice",
    "ChoiceMode",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "ChoiceHistoryOnInput",
    "ChildForms",
    "ChildTemplates",
];

const SETTINGS_PROPERTY_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "DefaultSaveForm",
    "DefaultLoadForm",
    "AuxiliarySaveForm",
    "AuxiliaryLoadForm",
    "ChildForms",
    "ChildTemplates",
];

struct CompiledChildren {
    attributes: Vec<NativeValue>,
    tabular_sections: Vec<NativeValue>,
    commands: Vec<NativeValue>,
    enum_values: Vec<NativeValue>,
    forms: Vec<ObjectUuid>,
    templates: Vec<ObjectUuid>,
}

fn build_report(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, UtilityBuildError> {
    validate_root_object(validated, object, REPORT_PROPERTY_SCHEMA)?;
    let children = collect_children(validated, object, UtilityFamily::Report, indexes)?;
    let generated = generated_pairs(object, &["Object", "Manager"])?;
    let fields = vec![
        token("19"),
        uuid_value(generated[0].0),
        uuid_value(generated[0].1),
        list(vec![token("0"), native_header(object)?]),
        optional_owned_reference(object, "DefaultForm", &children.forms, "Form", indexes)?,
        optional_owned_reference(
            object,
            "MainDataCompositionSchema",
            &children.templates,
            "Template",
            indexes,
        )?,
        optional_owned_reference(
            object,
            "DefaultSettingsForm",
            &children.forms,
            "Form",
            indexes,
        )?,
        bool_token(object, "UseStandardCommands")?,
        optional_metadata_reference(object, "VariantsStorage", indexes)?,
        optional_metadata_reference(object, "SettingsStorage", indexes)?,
        optional_owned_reference(
            object,
            "DefaultVariantForm",
            &children.forms,
            "Form",
            indexes,
        )?,
        bool_token(object, "IncludeHelpInContents")?,
        uuid_value(generated[1].0),
        uuid_value(generated[1].1),
        optional_owned_reference(object, "AuxiliaryForm", &children.forms, "Form", indexes)?,
        localized_value(object, "ExtendedPresentation", "language")?,
        localized_value(object, "Explanation", "language")?,
        optional_owned_reference(
            object,
            "AuxiliarySettingsForm",
            &children.forms,
            "Form",
            indexes,
        )?,
    ];
    Ok(list(vec![
        token("1"),
        list(fields),
        token("5"),
        native_collection(
            TEMPLATE_COLLECTION_UUID,
            children.templates.into_iter().map(uuid_value).collect(),
        ),
        native_collection(REPORT_ATTRIBUTE_COLLECTION_UUID, children.attributes),
        native_collection(
            REPORT_FORM_COLLECTION_UUID,
            children.forms.into_iter().map(uuid_value).collect(),
        ),
        native_collection(REPORT_TABULAR_COLLECTION_UUID, children.tabular_sections),
        native_collection(REPORT_COMMAND_COLLECTION_UUID, children.commands),
    ]))
}

fn build_data_processor(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, UtilityBuildError> {
    validate_root_object(validated, object, DATA_PROCESSOR_PROPERTY_SCHEMA)?;
    let children = collect_children(validated, object, UtilityFamily::DataProcessor, indexes)?;
    let generated = generated_pairs(object, &["Object", "Manager"])?;
    let fields = vec![
        token("17"),
        uuid_value(generated[0].0),
        uuid_value(generated[0].1),
        list(vec![token("0"), native_header(object)?]),
        optional_owned_reference(object, "DefaultForm", &children.forms, "Form", indexes)?,
        bool_token(object, "UseStandardCommands")?,
        bool_token(object, "IncludeHelpInContents")?,
        uuid_value(generated[1].0),
        uuid_value(generated[1].1),
        optional_owned_reference(object, "AuxiliaryForm", &children.forms, "Form", indexes)?,
        localized_value(object, "ExtendedPresentation", "language")?,
        localized_value(object, "Explanation", "language")?,
    ];
    Ok(list(vec![
        token("1"),
        list(fields),
        token("5"),
        native_collection(PROCESSOR_TABULAR_COLLECTION_UUID, children.tabular_sections),
        native_collection(
            TEMPLATE_COLLECTION_UUID,
            children.templates.into_iter().map(uuid_value).collect(),
        ),
        native_collection(PROCESSOR_COMMAND_COLLECTION_UUID, children.commands),
        native_collection(
            PROCESSOR_FORM_COLLECTION_UUID,
            children.forms.into_iter().map(uuid_value).collect(),
        ),
        native_collection(PROCESSOR_ATTRIBUTE_COLLECTION_UUID, children.attributes),
    ]))
}

fn build_enum(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, UtilityBuildError> {
    validate_root_object(validated, object, ENUM_PROPERTY_SCHEMA)?;
    if !bool_property(object, "HasStandardAttributes")? {
        return invalid_model(
            object.identity().uuid(),
            "Enum standard attributes are required",
        );
    }
    let children = collect_children(validated, object, UtilityFamily::Enum, indexes)?;
    let generated = generated_pairs(object, &["Ref", "Manager", "List"])?;
    let fields = vec![
        token("20"),
        uuid_value(generated[0].0),
        uuid_value(generated[0].1),
        uuid_value(generated[1].0),
        uuid_value(generated[1].1),
        list(vec![token("0"), native_header(object)?]),
        bool_token(object, "UseStandardCommands")?,
        uuid_value(generated[2].0),
        uuid_value(generated[2].1),
        optional_owned_reference(object, "DefaultListForm", &children.forms, "Form", indexes)?,
        optional_owned_reference(
            object,
            "DefaultChoiceForm",
            &children.forms,
            "Form",
            indexes,
        )?,
        enum_code(
            object,
            "ChoiceMode",
            &[("FromForm", "0"), ("QuickChoice", "1"), ("BothWays", "2")],
        )?,
        bool_token(object, "QuickChoice")?,
        optional_owned_reference(
            object,
            "AuxiliaryListForm",
            &children.forms,
            "Form",
            indexes,
        )?,
        optional_owned_reference(
            object,
            "AuxiliaryChoiceForm",
            &children.forms,
            "Form",
            indexes,
        )?,
        localized_value(object, "ListPresentation", "language")?,
        localized_value(object, "ExtendedListPresentation", "language")?,
        localized_value(object, "Explanation", "language")?,
        standard_attributes(&["-3", "-2"])?,
        list(vec![token("0"), list(vec![token("0")])]),
        enum_code(
            object,
            "ChoiceHistoryOnInput",
            &[("Auto", "0"), ("DontUse", "1")],
        )?,
    ];
    Ok(list(vec![
        token("1"),
        list(fields),
        token("4"),
        native_collection(
            ENUM_FORM_COLLECTION_UUID,
            children.forms.into_iter().map(uuid_value).collect(),
        ),
        native_collection(
            TEMPLATE_COLLECTION_UUID,
            children.templates.into_iter().map(uuid_value).collect(),
        ),
        native_collection(ENUM_RESERVED_COLLECTION_UUID, Vec::new()),
        native_collection(ENUM_VALUE_COLLECTION_UUID, children.enum_values),
    ]))
}

fn build_settings_storage(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, UtilityBuildError> {
    validate_root_object(validated, object, SETTINGS_PROPERTY_SCHEMA)?;
    let children = collect_children(validated, object, UtilityFamily::SettingsStorage, indexes)?;
    if !children.templates.is_empty() {
        return invalid_model(
            object.identity().uuid(),
            "SettingsStorage templates are not evidenced",
        );
    }
    if !text_property(object, "AuxiliarySaveForm")?.is_empty()
        || !text_property(object, "AuxiliaryLoadForm")?.is_empty()
    {
        return invalid_model(
            object.identity().uuid(),
            "SettingsStorage auxiliary forms are not evidenced",
        );
    }
    let generated = generated_pairs(object, &["Manager"])?;
    let fields = vec![
        token("2"),
        list(vec![token("0"), native_header(object)?]),
        uuid_value(generated[0].0),
        uuid_value(generated[0].1),
        optional_owned_reference(object, "DefaultLoadForm", &children.forms, "Form", indexes)?,
        optional_owned_reference(object, "DefaultSaveForm", &children.forms, "Form", indexes)?,
        token(NIL_UUID),
        token(NIL_UUID),
    ];
    Ok(list(vec![
        token("1"),
        list(fields),
        token("2"),
        native_collection(TEMPLATE_COLLECTION_UUID, Vec::new()),
        native_collection(
            SETTINGS_FORM_COLLECTION_UUID,
            children.forms.into_iter().map(uuid_value).collect(),
        ),
    ]))
}

fn collect_children(
    validated: &ValidatedConfiguration<'_>,
    root: &CanonicalObject,
    family: UtilityFamily,
    indexes: &ReferenceIndexes,
) -> Result<CompiledChildren, UtilityBuildError> {
    let root_uuid = root.identity().uuid();
    let forms = reference_sequence_targets(root, "ChildForms", indexes)?;
    let templates = reference_sequence_targets(root, "ChildTemplates", indexes)?;
    validate_named_children(root_uuid, &forms, "Form", indexes)?;
    validate_named_children(root_uuid, &templates, "Template", indexes)?;
    let mut attributes = Vec::new();
    let mut tabular_sections = Vec::new();
    let mut commands = Vec::new();
    let mut enum_values = Vec::new();
    let mut accepted = BTreeSet::new();
    for object in validated.configuration().objects() {
        if object.owner() != Some(root_uuid) {
            continue;
        }
        match (family, object.kind().as_str()) {
            (UtilityFamily::Report | UtilityFamily::DataProcessor, "Attribute") => {
                attributes.push(build_attribute(object, false, indexes)?);
                accepted.insert(object.identity().uuid());
            }
            (UtilityFamily::Report | UtilityFamily::DataProcessor, "TabularSection") => {
                let mut nested = Vec::new();
                for candidate in validated.configuration().objects() {
                    if candidate.owner() == Some(object.identity().uuid()) {
                        if candidate.kind().as_str() != "Attribute" {
                            return invalid_model(
                                root_uuid,
                                "utility TabularSection contains a non-Attribute object",
                            );
                        }
                        nested.push(build_attribute(candidate, true, indexes)?);
                        accepted.insert(candidate.identity().uuid());
                    }
                }
                tabular_sections.push(build_tabular_section(object, family, nested)?);
                accepted.insert(object.identity().uuid());
            }
            (UtilityFamily::Report | UtilityFamily::DataProcessor, "Command") => {
                commands.push(build_command(object, indexes)?);
                accepted.insert(object.identity().uuid());
            }
            (UtilityFamily::Enum, "EnumValue") => {
                enum_values.push(build_enum_value(object)?);
                accepted.insert(object.identity().uuid());
            }
            (_, "Form" | "Template") => {}
            _ => {
                return invalid_model(
                    root_uuid,
                    "utility metadata contains an unsupported direct child",
                );
            }
        }
    }
    for object in validated.configuration().objects() {
        if matches!(
            object.kind().as_str(),
            "Attribute" | "TabularSection" | "Command" | "EnumValue"
        ) && is_descendant_of(validated, object, root_uuid)
            && !accepted.contains(&object.identity().uuid())
        {
            return invalid_model(root_uuid, "embedded utility inventory is not exact");
        }
    }
    Ok(CompiledChildren {
        attributes,
        tabular_sections,
        commands,
        enum_values,
        forms,
        templates,
    })
}

fn build_attribute(
    object: &CanonicalObject,
    nested: bool,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, UtilityBuildError> {
    validate_embedded_object(object, "Attribute")?;
    require_attribute_schema(object, nested)?;
    let mut payload = vec![
        token("27"),
        list(vec![
            token("2"),
            native_header(object)?,
            type_pattern(object, indexes)?,
        ]),
        bool_token(object, "PasswordMode")?,
        list(vec![token("0")]),
        localized_value(object, "ToolTip", "language")?,
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
        optional_metadata_reference(object, "ChoiceForm", indexes)?,
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
        if nested {
            bool_token(object, "FillFromFillingValue")?
        } else {
            token("0")
        },
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
    if payload.len() != 23 {
        return native("utility Attribute payload field count is not exact");
    }
    Ok(list(vec![
        list(vec![token("0"), list(std::mem::take(&mut payload))]),
        token("0"),
    ]))
}

fn build_tabular_section(
    object: &CanonicalObject,
    family: UtilityFamily,
    nested_attributes: Vec<NativeValue>,
) -> Result<NativeValue, UtilityBuildError> {
    validate_embedded_object(object, "TabularSection")?;
    require_property_schema(
        object,
        &[
            "Name",
            "Synonym",
            "Comment",
            "ToolTip",
            "FillChecking",
            "HasLineNumberStandardAttribute",
        ],
    )?;
    if !bool_property(object, "HasLineNumberStandardAttribute")? {
        return invalid_model(
            object.identity().uuid(),
            "TabularSection LineNumber standard attribute is required",
        );
    }
    let generated = generated_pairs(object, &["TabularSection", "TabularSectionRow"])?;
    let mut payload = vec![token("11")];
    for pair in generated {
        payload.push(uuid_value(pair.0));
        payload.push(uuid_value(pair.1));
    }
    payload.push(list(vec![token("0"), native_header(object)?]));
    payload.push(enum_code(
        object,
        "FillChecking",
        &[("DontCheck", "0"), ("ShowError", "1")],
    )?);
    payload.push(standard_attributes(&["-3"])?);
    payload.push(localized_value(object, "ToolTip", "language")?);
    let marker = match family {
        UtilityFamily::Report => REPORT_TABULAR_ATTRIBUTE_COLLECTION_UUID,
        UtilityFamily::DataProcessor => PROCESSOR_TABULAR_ATTRIBUTE_COLLECTION_UUID,
        _ => {
            return invalid_model(
                object.identity().uuid(),
                "family does not support TabularSection",
            );
        }
    };
    Ok(list(vec![
        list(vec![token("0"), list(payload)]),
        token("1"),
        native_collection(marker, nested_attributes),
    ]))
}

fn build_command(
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, UtilityBuildError> {
    validate_embedded_object(object, "Command")?;
    require_property_schema(
        object,
        &[
            "Name",
            "Synonym",
            "Comment",
            "Group",
            "ParameterUseMode",
            "ModifiesData",
            "Representation",
            "OnMainServerUnavalableBehavior",
        ],
    )?;
    let group = text_property(object, "Group")?;
    let group_uuid = builtin_command_group_uuid(group)
        .or_else(|| indexes.objects.get(group).copied())
        .ok_or(UtilityBuildError::InvalidModel {
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

fn build_enum_value(object: &CanonicalObject) -> Result<NativeValue, UtilityBuildError> {
    validate_embedded_object(object, "EnumValue")?;
    let fields = object.properties();
    let valid = (fields.len() == 3
        && fields
            .iter()
            .map(|field| field.name().as_str())
            .eq(["Name", "Synonym", "Comment"]))
        || (fields.len() == 4
            && fields
                .iter()
                .map(|field| field.name().as_str())
                .eq(["Name", "Synonym", "Comment", "Color"])
            && enum_property(object, "Color")? == "auto");
    if !valid {
        return invalid_model(object.identity().uuid(), "EnumValue schema is not exact");
    }
    Ok(list(vec![
        list(vec![token("0"), native_header(object)?]),
        token("0"),
    ]))
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

const ATTRIBUTE_PROPERTY_ORDER: &[&str] = &[
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
    "ToolTip",
    "MarkNegatives",
    "MultiLine",
    "ExtendedEdit",
    "FillFromFillingValue",
    "FillChecking",
    "ChoiceFoldersAndItems",
    "QuickChoice",
    "CreateOnInput",
    "ChoiceHistoryOnInput",
    "Mask",
    "ChoiceForm",
];

fn validate_root_object(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    schema: &[&str],
) -> Result<(), UtilityBuildError> {
    let uuid = object.identity().uuid();
    if object.owner().is_some() {
        return invalid_model(uuid, "utility root must be top-level");
    }
    if !object.references().is_empty() || !object.assets().is_empty() {
        return invalid_model(uuid, "utility root has unsupported references or assets");
    }
    require_property_schema(object, schema)?;
    let name = text_property(object, "Name")?;
    if name.is_empty() || name.contains('.') {
        return invalid_model(uuid, "utility Name is empty or qualified");
    }
    if !matches!(
        object.provenance().source_profile().as_str(),
        "xml-2.20" | "xml-2.21"
    ) {
        return invalid_model(uuid, "source profile is not xml-2.20 or xml-2.21");
    }
    if validated.graph().object_index_by_uuid(uuid).is_none() {
        return Err(UtilityBuildError::UnknownObject(uuid));
    }
    Ok(())
}

fn validate_embedded_object(
    object: &CanonicalObject,
    expected_kind: &'static str,
) -> Result<(), UtilityBuildError> {
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
) -> Result<(), UtilityBuildError> {
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
    nested: bool,
) -> Result<(), UtilityBuildError> {
    let mut allowed_index = 0usize;
    for property in object.properties() {
        let Some(relative) = ATTRIBUTE_PROPERTY_ORDER[allowed_index..]
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
        "ToolTip",
        "MarkNegatives",
        "MultiLine",
        "ExtendedEdit",
        "FillChecking",
        "ChoiceFoldersAndItems",
        "QuickChoice",
        "CreateOnInput",
        "ChoiceHistoryOnInput",
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
    if nested != property_optional(object, "FillFromFillingValue").is_some() {
        return invalid_model(
            object.identity().uuid(),
            "Attribute filling property does not match its nesting",
        );
    }
    Ok(())
}

fn property<'a>(
    object: &'a CanonicalObject,
    name: &str,
) -> Result<&'a CanonicalValue, UtilityBuildError> {
    property_optional(object, name).ok_or(UtilityBuildError::InvalidModel {
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
) -> Result<&'a str, UtilityBuildError> {
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

fn bool_property(object: &CanonicalObject, name: &str) -> Result<bool, UtilityBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::Bool(value) => Ok(value),
        _ => invalid_model(object.identity().uuid(), "typed property is not boolean"),
    }
}

fn bool_token(object: &CanonicalObject, name: &str) -> Result<NativeValue, UtilityBuildError> {
    Ok(token(if bool_property(object, name)? {
        "1"
    } else {
        "0"
    }))
}

fn enum_property<'a>(
    object: &'a CanonicalObject,
    name: &str,
) -> Result<&'a str, UtilityBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::EnumToken(value) => Ok(value.as_str()),
        _ => invalid_model(
            object.identity().uuid(),
            "typed property is not an enum token",
        ),
    }
}

fn enum_code(
    object: &CanonicalObject,
    name: &str,
    mapping: &[(&str, &str)],
) -> Result<NativeValue, UtilityBuildError> {
    let value = enum_property(object, name)?;
    mapping
        .iter()
        .find_map(|(candidate, code)| (*candidate == value).then(|| token(*code)))
        .ok_or(UtilityBuildError::InvalidModel {
            object: object.identity().uuid(),
            reason: "enum value has no evidenced native code",
        })
}

fn native_header(object: &CanonicalObject) -> Result<NativeValue, UtilityBuildError> {
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

fn localized_value(
    object: &CanonicalObject,
    name: &str,
    language_field: &str,
) -> Result<NativeValue, UtilityBuildError> {
    let values = property(object, name)?
        .as_sequence()
        .ok_or(UtilityBuildError::InvalidModel {
            object: object.identity().uuid(),
            reason: "localized property is not a sequence",
        })?;
    let mut output = vec![token(values.len().to_string())];
    let mut languages = BTreeSet::new();
    for value in values {
        let fields = value.as_record().ok_or(UtilityBuildError::InvalidModel {
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
) -> Result<Vec<(ObjectUuid, ObjectUuid)>, UtilityBuildError> {
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
                .ok_or(UtilityBuildError::InvalidModel {
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

fn reference_sequence_targets(
    object: &CanonicalObject,
    name: &str,
    indexes: &ReferenceIndexes,
) -> Result<Vec<ObjectUuid>, UtilityBuildError> {
    let values = property(object, name)?
        .as_sequence()
        .ok_or(UtilityBuildError::InvalidModel {
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

fn validate_named_children(
    root: ObjectUuid,
    children: &[ObjectUuid],
    expected_kind: &'static str,
    indexes: &ReferenceIndexes,
) -> Result<(), UtilityBuildError> {
    for child in children {
        if indexes.kind(*child) != Some(expected_kind) {
            return invalid_model(root, "named child reference has wrong kind or owner");
        }
    }
    Ok(())
}

fn optional_metadata_reference(
    object: &CanonicalObject,
    name: &str,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, UtilityBuildError> {
    let reference = text_property(object, name)?;
    if reference.is_empty() {
        return Ok(token(NIL_UUID));
    }
    Ok(uuid_value(
        indexes.object(object.identity().uuid(), reference)?,
    ))
}

fn optional_owned_reference(
    object: &CanonicalObject,
    name: &str,
    owned: &[ObjectUuid],
    expected_kind: &'static str,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, UtilityBuildError> {
    let reference = text_property(object, name)?;
    if reference.is_empty() {
        return Ok(token(NIL_UUID));
    }
    let uuid = indexes.object(object.identity().uuid(), reference)?;
    if !owned.contains(&uuid) || indexes.kind(uuid) != Some(expected_kind) {
        return invalid_model(
            object.identity().uuid(),
            "default child reference is not an owned child of the expected kind",
        );
    }
    Ok(uuid_value(uuid))
}

fn type_pattern(
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, UtilityBuildError> {
    let types =
        property(object, "Types")?
            .as_sequence()
            .ok_or(UtilityBuildError::InvalidModel {
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
                        let code = match canonical_text_or_enum(object, allowed)? {
                            "Fixed" => "0",
                            "Variable" => "1",
                            _ => {
                                return invalid_model(
                                    object.identity().uuid(),
                                    "String AllowedLength is unsupported",
                                );
                            }
                        };
                        list(vec![
                            text("S"),
                            token(canonical_u32_value(object, length)?.to_string()),
                            token(code),
                        ])
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
) -> Result<u32, UtilityBuildError> {
    let raw = match value.kind() {
        CanonicalValueKind::Text(value) => value.as_str(),
        CanonicalValueKind::Integer(value) => value.as_str(),
        _ => {
            return invalid_model(
                object.identity().uuid(),
                "type qualifier is not text/integer",
            );
        }
    };
    raw.parse::<u32>()
        .map_err(|_| UtilityBuildError::InvalidModel {
            object: object.identity().uuid(),
            reason: "type qualifier is not u32",
        })
}

fn canonical_text_or_enum<'a>(
    object: &CanonicalObject,
    value: &'a CanonicalValue,
) -> Result<&'a str, UtilityBuildError> {
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

fn invalid_model<T>(object: ObjectUuid, reason: &'static str) -> Result<T, UtilityBuildError> {
    Err(UtilityBuildError::InvalidModel { object, reason })
}

fn native<T>(reason: impl Into<String>) -> Result<T, UtilityBuildError> {
    Err(UtilityBuildError::Native(reason.into()))
}

// Exact shared 8.3.27 descriptor; parsed and compared as a native tree, never interpolated.
const STANDARD_ATTRIBUTE_DESCRIPTOR: &str = r##"{14,25,1183c14f-f814-49c6-9233-a3c26b3f64cf,{"#",9ad557b1-249e-48dc-824b-3e149ecf10a6,{3,0,0}},2723eb98-b4c1-498a-a6f3-70444757902f,{"#",98ea8e5a-b586-442b-b944-6e3447734aa7,0},2bbba66b-fabf-4863-8ba3-54b3c64c896e,{"B",0},2c8143d5-4248-4c43-8bfb-307c0be2e415,{"B",0},33c74a4d-561f-4bc0-9eaa-8d21c893c0a9,{"#",ad3615c5-aae6-4725-89be-91827523abd9,{ad3615c5-aae6-4725-89be-91827523abd9,0}},3b10624f-1e3d-495d-8093-25225efc5313,{"#",502b7765-f89c-4fd0-924f-0a28d3dc09b7,{502b7765-f89c-4fd0-924f-0a28d3dc09b7,0}},3eaf5a8b-06d6-47b0-ac7d-a9698247f499,{"U"},4690ff70-e3fa-4914-9127-6a9acc5fc949,{"#",87024738-fc2a-4436-ada1-df79d395c424,{0}},4de03908-56f4-4396-a61e-17253afca9ac,{"B",0},580c29e2-8af4-4258-882a-7cf8073e61c8,{"#",87024738-fc2a-4436-ada1-df79d395c424,{0}},6c4f7074-e7d4-48eb-b31b-132873666262,{"#",157fa490-4ce9-11d4-9415-008048da11f9,{1,00000000-0000-0000-0000-000000000000}},6e3a1131-37a3-4da5-8895-572d9d0c9db6,{"#",ace3fd07-11b2-477e-ab7f-36f0ea37c8dd,{ace3fd07-11b2-477e-ab7f-36f0ea37c8dd,2}},7ba608f2-e654-42a3-8885-334fe88ca910,{"#",12ca4003-ac70-450e-b897-37faf86bd313,0},88149a78-9448-4767-867b-0e650d165d2e,{"#",87024738-fc2a-4436-ada1-df79d395c424,{0}},90ae4b5d-e0fd-49ef-a008-d67c1e75038c,{"B",0},9288a8ed-b259-46d0-a8e3-70d87956ff2d,{"#",d46ea122-3201-4e5e-bed4-e669c6e463c8,{d46ea122-3201-4e5e-bed4-e669c6e463c8,1}},b02800e9-a8d1-42ab-9a12-f673e92be968,{"B",0},c65a541f-0b91-4f33-bc88-fbaaa57f9992,{"U"},cf4abea3-37b2-11d4-940f-008048da11f9,{"#",87024738-fc2a-4436-ada1-df79d395c424,{0}},cf4abea4-37b2-11d4-940f-008048da11f9,{"S",""},d4232326-022b-421e-b6d3-88e418f74327,{"#",3b8e6bdd-d648-49d5-af2f-d46d84f87dd5,{3b8e6bdd-d648-49d5-af2f-d46d84f87dd5,1}},e3da683b-c54a-457a-a243-b9b4f9bf76dd,{"#",b76a58b9-2a56-4e46-bb31-8e04ad9f31ae,{5006,0}},e6b3f5f3-bdf3-4ad0-bc60-7323b3feb208,{"U"},f49e4ced-4033-4e6c-8755-9fbaaccd6078,{"S",""},fcf503b8-1c06-454a-970c-06413e64aee5,{"#",f2eaae14-91a7-47b9-9d69-097877f41580,{0,0}}}"##;

pub(super) const SHARED_STANDARD_ATTRIBUTE_DESCRIPTOR: &str = STANDARD_ATTRIBUTE_DESCRIPTOR;

fn standard_attributes(markers: &[&str]) -> Result<NativeValue, UtilityBuildError> {
    if markers.is_empty() {
        return native("standard attribute marker inventory is empty");
    }
    let mut source = Vec::with_capacity(UTF8_BOM.len() + STANDARD_ATTRIBUTE_DESCRIPTOR.len());
    source.extend_from_slice(UTF8_BOM);
    source.extend_from_slice(STANDARD_ATTRIBUTE_DESCRIPTOR.as_bytes());
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

fn serialize_native(value: &NativeValue) -> Result<Vec<u8>, UtilityBuildError> {
    let mut output = Vec::new();
    output.extend_from_slice(UTF8_BOM);
    write_native_value(value, &mut output, 0)?;
    if output.len() > MAX_PLAIN_BYTES {
        return Err(UtilityBuildError::PlainPayloadTooLarge {
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
) -> Result<(), UtilityBuildError> {
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
        return Err(UtilityBuildError::PlainPayloadTooLarge {
            maximum: MAX_PLAIN_BYTES,
            actual: output.len(),
        });
    }
    Ok(())
}

fn raw_deflate(plain: &[u8]) -> Result<Vec<u8>, UtilityBuildError> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(plain)
        .map_err(UtilityBuildError::Deflate)?;
    encoder.finish().map_err(UtilityBuildError::Deflate)
}

fn inflate_bounded(blob: &[u8]) -> Result<Vec<u8>, UtilityBuildError> {
    let limit = MAX_PLAIN_BYTES
        .checked_add(1)
        .expect("native plaintext bound is below usize::MAX");
    let mut decoder = DeflateDecoder::new(blob).take(limit as u64);
    let mut plain = Vec::new();
    decoder
        .read_to_end(&mut plain)
        .map_err(UtilityBuildError::Inflate)?;
    if plain.len() > MAX_PLAIN_BYTES {
        return Err(UtilityBuildError::PlainPayloadTooLarge {
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

    fn parse(mut self) -> Result<NativeValue, UtilityBuildError> {
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

    fn value(&mut self, depth: usize) -> Result<NativeValue, UtilityBuildError> {
        if depth > MAX_NATIVE_DEPTH {
            return native("native value exceeds nesting bound");
        }
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or_else(|| UtilityBuildError::Native("native node count overflow".to_owned()))?;
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

    fn list(&mut self, depth: usize) -> Result<NativeValue, UtilityBuildError> {
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

    fn text(&mut self) -> Result<NativeValue, UtilityBuildError> {
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
                            UtilityBuildError::Native("quoted value is not UTF-8".to_owned())
                        });
                }
            } else {
                output.push(byte);
                self.offset += 1;
            }
        }
        native("unterminated quoted value")
    }

    fn token(&mut self) -> Result<NativeValue, UtilityBuildError> {
        let start = self.offset;
        while let Some(byte) = self.input.get(self.offset) {
            if matches!(byte, b',' | b'}') {
                break;
            }
            self.offset += 1;
        }
        let value = std::str::from_utf8(&self.input[start..self.offset])
            .map_err(|_| UtilityBuildError::Native("native token is not UTF-8".to_owned()))?
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilityTabularNativeIr {
    pub uuid: ObjectUuid,
    pub generated_types: Vec<(ObjectUuid, ObjectUuid)>,
    pub attribute_uuids: Vec<ObjectUuid>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilityNativeIr {
    pub family: UtilityFamily,
    pub uuid: ObjectUuid,
    pub name: String,
    pub generated_types: Vec<(ObjectUuid, ObjectUuid)>,
    pub attribute_uuids: Vec<ObjectUuid>,
    pub tabular_sections: Vec<UtilityTabularNativeIr>,
    pub command_uuids: Vec<ObjectUuid>,
    pub enum_value_uuids: Vec<ObjectUuid>,
    pub form_uuids: Vec<ObjectUuid>,
    pub template_uuids: Vec<ObjectUuid>,
}

pub(crate) fn decode_utility_blob(
    blob: &[u8],
    profile: &UtilityMetadataProfile,
) -> Result<UtilityNativeIr, UtilityBuildError> {
    let plain = inflate_bounded(blob)?;
    let value = NativeParser::new(&plain).parse()?;
    decode_native_ir(&value, profile.family)
}

fn decode_native_ir(
    value: &NativeValue,
    family: UtilityFamily,
) -> Result<UtilityNativeIr, UtilityBuildError> {
    let (root_len, field_len, discriminator, collection_count, header_slot, generated_slots) =
        match family {
            UtilityFamily::Report => (8, 18, "19", "5", 3, &[1usize, 12usize][..]),
            UtilityFamily::DataProcessor => (8, 12, "17", "5", 3, &[1usize, 7usize][..]),
            UtilityFamily::Enum => (7, 21, "20", "4", 5, &[1usize, 3usize, 7usize][..]),
            UtilityFamily::SettingsStorage => (5, 8, "2", "2", 1, &[2usize][..]),
        };
    let root = exact_list(value, root_len, "utility root")?;
    exact_token(&root[0], "1", "root discriminator")?;
    exact_token(&root[2], collection_count, "root collection count")?;
    let fields = exact_list(&root[1], field_len, "utility fields")?;
    exact_token(&fields[0], discriminator, "utility discriminator")?;
    let header_wrapper = exact_list(&fields[header_slot], 2, "utility header wrapper")?;
    exact_token(
        &header_wrapper[0],
        "0",
        "utility header wrapper discriminator",
    )?;
    let (uuid, name) = parse_header(&header_wrapper[1])?;
    let mut generated_types = Vec::with_capacity(generated_slots.len());
    for slot in generated_slots {
        generated_types.push((
            non_nil_uuid(&fields[*slot], "generated TypeId")?,
            non_nil_uuid(&fields[*slot + 1], "generated ValueId")?,
        ));
    }

    let mut attributes = Vec::new();
    let mut tabular_sections = Vec::new();
    let mut commands = Vec::new();
    let mut enum_values = Vec::new();
    let (forms, templates) = match family {
        UtilityFamily::Report => {
            let templates = parse_uuid_collection(&root[3], TEMPLATE_COLLECTION_UUID, "templates")?;
            attributes = parse_attributes(parse_collection(
                &root[4],
                REPORT_ATTRIBUTE_COLLECTION_UUID,
                "attributes",
            )?)?;
            let forms = parse_uuid_collection(&root[5], REPORT_FORM_COLLECTION_UUID, "forms")?;
            tabular_sections = parse_tabular_sections(
                parse_collection(&root[6], REPORT_TABULAR_COLLECTION_UUID, "tabular sections")?,
                REPORT_TABULAR_ATTRIBUTE_COLLECTION_UUID,
            )?;
            commands = parse_commands(parse_collection(
                &root[7],
                REPORT_COMMAND_COLLECTION_UUID,
                "commands",
            )?)?;
            validate_optional_child_references(
                fields,
                &[4, 6, 10, 14, 17],
                &forms,
                "Report form reference",
            )?;
            validate_optional_child_references(
                fields,
                &[5],
                &templates,
                "Report template reference",
            )?;
            (forms, templates)
        }
        UtilityFamily::DataProcessor => {
            tabular_sections = parse_tabular_sections(
                parse_collection(
                    &root[3],
                    PROCESSOR_TABULAR_COLLECTION_UUID,
                    "tabular sections",
                )?,
                PROCESSOR_TABULAR_ATTRIBUTE_COLLECTION_UUID,
            )?;
            let templates = parse_uuid_collection(&root[4], TEMPLATE_COLLECTION_UUID, "templates")?;
            commands = parse_commands(parse_collection(
                &root[5],
                PROCESSOR_COMMAND_COLLECTION_UUID,
                "commands",
            )?)?;
            let forms = parse_uuid_collection(&root[6], PROCESSOR_FORM_COLLECTION_UUID, "forms")?;
            attributes = parse_attributes(parse_collection(
                &root[7],
                PROCESSOR_ATTRIBUTE_COLLECTION_UUID,
                "attributes",
            )?)?;
            validate_optional_child_references(
                fields,
                &[4, 9],
                &forms,
                "DataProcessor form reference",
            )?;
            (forms, templates)
        }
        UtilityFamily::Enum => {
            let forms = parse_uuid_collection(&root[3], ENUM_FORM_COLLECTION_UUID, "forms")?;
            let templates = parse_uuid_collection(&root[4], TEMPLATE_COLLECTION_UUID, "templates")?;
            if !parse_collection(
                &root[5],
                ENUM_RESERVED_COLLECTION_UUID,
                "reserved collection",
            )?
            .is_empty()
            {
                return native("Enum reserved collection is not empty");
            }
            enum_values = parse_enum_values(parse_collection(
                &root[6],
                ENUM_VALUE_COLLECTION_UUID,
                "enum values",
            )?)?;
            validate_optional_child_references(
                fields,
                &[9, 10, 13, 14],
                &forms,
                "Enum form reference",
            )?;
            validate_standard_attributes(&fields[18], &["-3", "-2"])?;
            let characteristics = exact_list(&fields[19], 2, "Enum characteristics")?;
            exact_token(
                &characteristics[0],
                "0",
                "Enum characteristics discriminator",
            )?;
            let nested = exact_list(&characteristics[1], 1, "Enum characteristics body")?;
            exact_token(&nested[0], "0", "Enum characteristics body discriminator")?;
            (forms, templates)
        }
        UtilityFamily::SettingsStorage => {
            if !parse_collection(&root[3], TEMPLATE_COLLECTION_UUID, "templates")?.is_empty() {
                return native("SettingsStorage template collection is not empty");
            }
            let forms = parse_uuid_collection(&root[4], SETTINGS_FORM_COLLECTION_UUID, "forms")?;
            validate_optional_child_references(
                fields,
                &[4, 5],
                &forms,
                "SettingsStorage form reference",
            )?;
            exact_token(&fields[6], NIL_UUID, "SettingsStorage auxiliary save form")?;
            exact_token(&fields[7], NIL_UUID, "SettingsStorage auxiliary load form")?;
            (forms, Vec::new())
        }
    };
    validate_native_identity_inventory(
        uuid,
        &generated_types,
        &attributes,
        &tabular_sections,
        &commands,
        &enum_values,
        &forms,
        &templates,
    )?;
    Ok(UtilityNativeIr {
        family,
        uuid,
        name,
        generated_types,
        attribute_uuids: attributes,
        tabular_sections,
        command_uuids: commands,
        enum_value_uuids: enum_values,
        form_uuids: forms,
        template_uuids: templates,
    })
}

fn parse_collection<'a>(
    value: &'a NativeValue,
    marker: &str,
    label: &'static str,
) -> Result<&'a [NativeValue], UtilityBuildError> {
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
) -> Result<Vec<ObjectUuid>, UtilityBuildError> {
    parse_collection(value, marker, label)?
        .iter()
        .map(|value| non_nil_uuid(value, label))
        .collect()
}

fn parse_attributes(values: &[NativeValue]) -> Result<Vec<ObjectUuid>, UtilityBuildError> {
    values.iter().map(parse_attribute_uuid).collect()
}

fn parse_attribute_uuid(value: &NativeValue) -> Result<ObjectUuid, UtilityBuildError> {
    let item = exact_list(value, 2, "Attribute item")?;
    exact_token(&item[1], "0", "Attribute item tail")?;
    let wrapper = exact_list(&item[0], 2, "Attribute wrapper")?;
    exact_token(&wrapper[0], "0", "Attribute wrapper discriminator")?;
    let payload = exact_list(&wrapper[1], 23, "Attribute payload")?;
    exact_token(&payload[0], "27", "Attribute payload discriminator")?;
    let typed = exact_list(&payload[1], 3, "Attribute typed body")?;
    exact_token(&typed[0], "2", "Attribute typed discriminator")?;
    validate_type_pattern(&typed[2])?;
    Ok(parse_header(&typed[1])?.0)
}

fn parse_tabular_sections(
    values: &[NativeValue],
    nested_marker: &str,
) -> Result<Vec<UtilityTabularNativeIr>, UtilityBuildError> {
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let item = exact_list(value, 3, "TabularSection item")?;
        exact_token(&item[1], "1", "TabularSection item discriminator")?;
        let wrapper = exact_list(&item[0], 2, "TabularSection wrapper")?;
        exact_token(&wrapper[0], "0", "TabularSection wrapper discriminator")?;
        let payload = exact_list(&wrapper[1], 9, "TabularSection payload")?;
        exact_token(&payload[0], "11", "TabularSection payload discriminator")?;
        let generated_types = vec![
            (
                non_nil_uuid(&payload[1], "TabularSection TypeId")?,
                non_nil_uuid(&payload[2], "TabularSection ValueId")?,
            ),
            (
                non_nil_uuid(&payload[3], "TabularSectionRow TypeId")?,
                non_nil_uuid(&payload[4], "TabularSectionRow ValueId")?,
            ),
        ];
        let header_wrapper = exact_list(&payload[5], 2, "TabularSection header wrapper")?;
        exact_token(
            &header_wrapper[0],
            "0",
            "TabularSection header wrapper discriminator",
        )?;
        let uuid = parse_header(&header_wrapper[1])?.0;
        validate_standard_attributes(&payload[7], &["-3"])?;
        let nested = parse_collection(&item[2], nested_marker, "TabularSection attributes")?;
        result.push(UtilityTabularNativeIr {
            uuid,
            generated_types,
            attribute_uuids: parse_attributes(nested)?,
        });
    }
    Ok(result)
}

fn parse_commands(values: &[NativeValue]) -> Result<Vec<ObjectUuid>, UtilityBuildError> {
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
        if parse_header(&properties[9])?.0 != uuid {
            return native("Command identity and header UUID differ");
        }
        result.push(uuid);
    }
    Ok(result)
}

fn parse_enum_values(values: &[NativeValue]) -> Result<Vec<ObjectUuid>, UtilityBuildError> {
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let item = exact_list(value, 2, "EnumValue item")?;
        exact_token(&item[1], "0", "EnumValue item tail")?;
        let wrapper = exact_list(&item[0], 2, "EnumValue wrapper")?;
        exact_token(&wrapper[0], "0", "EnumValue wrapper discriminator")?;
        result.push(parse_header(&wrapper[1])?.0);
    }
    Ok(result)
}

fn validate_standard_attributes(
    value: &NativeValue,
    markers: &[&str],
) -> Result<(), UtilityBuildError> {
    let wrapper = exact_list(value, 2, "standard attributes wrapper")?;
    exact_token(
        &wrapper[0],
        "1",
        "standard attributes wrapper discriminator",
    )?;
    let body = as_list(&wrapper[1], "standard attributes body")?;
    if body.len() != markers.len() * 3 + 2 {
        return native("standard attributes body field count is not exact");
    }
    exact_token(&body[0], "1", "standard attributes body discriminator")?;
    if usize_token(&body[1], "standard attributes count")? != markers.len() {
        return native("standard attributes count is not exact");
    }
    let mut source = Vec::with_capacity(UTF8_BOM.len() + STANDARD_ATTRIBUTE_DESCRIPTOR.len());
    source.extend_from_slice(UTF8_BOM);
    source.extend_from_slice(STANDARD_ATTRIBUTE_DESCRIPTOR.as_bytes());
    let descriptor = NativeParser::new(&source).parse()?;
    for (index, marker) in markers.iter().enumerate() {
        let offset = 2 + index * 3;
        let marker_value = exact_list(&body[offset], 1, "standard attribute marker")?;
        exact_token(&marker_value[0], marker, "standard attribute marker")?;
        exact_token(
            &body[offset + 1],
            "510405d3-2a0c-4fea-960a-7fee59b32f9b",
            "standard attribute descriptor type",
        )?;
        if body[offset + 2] != descriptor {
            return native("standard attribute descriptor differs from evidenced layout");
        }
    }
    Ok(())
}

fn validate_type_pattern(value: &NativeValue) -> Result<(), UtilityBuildError> {
    let values = as_list(value, "type pattern")?;
    if values.len() < 2 {
        return native("type pattern has no item");
    }
    exact_text(&values[0], "Pattern", "type pattern discriminator")?;
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

fn parse_header(value: &NativeValue) -> Result<(ObjectUuid, String), UtilityBuildError> {
    let fields = exact_list(value, 9, "native header")?;
    exact_token(&fields[0], "3", "native header discriminator")?;
    let identity = exact_list(&fields[1], 3, "native header identity")?;
    exact_token(&identity[0], "1", "native header identity discriminator")?;
    exact_token(&identity[1], "0", "native header identity reserved slot")?;
    let uuid = non_nil_uuid(&identity[2], "native header UUID")?;
    let name = text_value(&fields[2], "native header Name")?;
    if name.is_empty() || name.contains('.') {
        return native("native header Name is empty or qualified");
    }
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
    Ok((uuid, name.to_owned()))
}

fn validate_optional_child_references(
    fields: &[NativeValue],
    slots: &[usize],
    inventory: &[ObjectUuid],
    label: &'static str,
) -> Result<(), UtilityBuildError> {
    for slot in slots {
        let value = token_value(&fields[*slot], label)?;
        if value == NIL_UUID {
            continue;
        }
        let uuid = non_nil_uuid(&fields[*slot], label)?;
        if !inventory.contains(&uuid) {
            return native(format!("{label} is not present in its child collection"));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_native_identity_inventory(
    root: ObjectUuid,
    generated: &[(ObjectUuid, ObjectUuid)],
    attributes: &[ObjectUuid],
    sections: &[UtilityTabularNativeIr],
    commands: &[ObjectUuid],
    enum_values: &[ObjectUuid],
    forms: &[ObjectUuid],
    templates: &[ObjectUuid],
) -> Result<(), UtilityBuildError> {
    let mut seen = BTreeSet::from([root]);
    for (type_id, value_id) in generated {
        if !seen.insert(*type_id) || !seen.insert(*value_id) {
            return native("native identity inventory contains duplicates");
        }
    }
    for uuid in attributes
        .iter()
        .chain(commands)
        .chain(enum_values)
        .chain(forms)
        .chain(templates)
    {
        if !seen.insert(*uuid) {
            return native("native identity inventory contains duplicates");
        }
    }
    for section in sections {
        if !seen.insert(section.uuid) {
            return native("native identity inventory contains duplicates");
        }
        for (type_id, value_id) in &section.generated_types {
            if !seen.insert(*type_id) || !seen.insert(*value_id) {
                return native("native identity inventory contains duplicates");
            }
        }
        for uuid in &section.attribute_uuids {
            if !seen.insert(*uuid) {
                return native("native identity inventory contains duplicates");
            }
        }
    }
    Ok(())
}

fn as_list<'a>(
    value: &'a NativeValue,
    label: &str,
) -> Result<&'a [NativeValue], UtilityBuildError> {
    match value {
        NativeValue::List(values) => Ok(values),
        _ => native(format!("{label} is not a list")),
    }
}

fn exact_list<'a>(
    value: &'a NativeValue,
    expected: usize,
    label: &str,
) -> Result<&'a [NativeValue], UtilityBuildError> {
    let values = as_list(value, label)?;
    if values.len() != expected {
        return native(format!(
            "{label} has {} fields, expected {expected}",
            values.len()
        ));
    }
    Ok(values)
}

fn token_value<'a>(value: &'a NativeValue, label: &str) -> Result<&'a str, UtilityBuildError> {
    match value {
        NativeValue::Token(value) => Ok(value),
        _ => native(format!("{label} is not a token")),
    }
}

fn text_value<'a>(value: &'a NativeValue, label: &str) -> Result<&'a str, UtilityBuildError> {
    match value {
        NativeValue::Text(value) => Ok(value),
        _ => native(format!("{label} is not quoted text")),
    }
}

fn exact_token(value: &NativeValue, expected: &str, label: &str) -> Result<(), UtilityBuildError> {
    if token_value(value, label)? != expected {
        return native(format!("{label} is not `{expected}`"));
    }
    Ok(())
}

fn exact_text(value: &NativeValue, expected: &str, label: &str) -> Result<(), UtilityBuildError> {
    if text_value(value, label)? != expected {
        return native(format!("{label} is not quoted `{expected}`"));
    }
    Ok(())
}

fn usize_token(value: &NativeValue, label: &str) -> Result<usize, UtilityBuildError> {
    let value = token_value(value, label)?;
    let parsed = value
        .parse::<usize>()
        .map_err(|_| UtilityBuildError::Native(format!("{label} is not usize")))?;
    if parsed.to_string() != value {
        return native(format!("{label} is not canonical usize"));
    }
    Ok(parsed)
}

fn non_nil_uuid(value: &NativeValue, label: &str) -> Result<ObjectUuid, UtilityBuildError> {
    let value = token_value(value, label)?;
    let uuid = ObjectUuid::parse(value)
        .map_err(|_| UtilityBuildError::Native(format!("{label} is not UUID")))?;
    if uuid.to_string() != value || value == NIL_UUID {
        return native(format!("{label} is nil or not canonical UUID"));
    }
    Ok(uuid)
}

#[cfg(test)]
mod tests {
    use ibcmd_core::artifact::{ProfileId, StorageProfileId};
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
    const REPORT_UUID: &str = "00000000-0000-4000-8000-000000001000";
    const PROCESSOR_UUID: &str = "00000000-0000-4000-8000-000000002000";
    const ENUM_UUID: &str = "00000000-0000-4000-8000-000000003000";
    const SETTINGS_UUID: &str = "00000000-0000-4000-8000-000000004000";

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

    fn root_generated(family: UtilityFamily, name: &str, seed: u32) -> String {
        let categories: &[&str] = match family {
            UtilityFamily::Report | UtilityFamily::DataProcessor => &["Object", "Manager"],
            UtilityFamily::Enum => &["Ref", "Manager", "List"],
            UtilityFamily::SettingsStorage => &["Manager"],
        };
        categories
            .iter()
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

    fn standard_attribute(name: &str) -> String {
        format!(
            "<xr:StandardAttribute name=\"{name}\"><xr:LinkByType/><xr:FillChecking>DontCheck</xr:FillChecking><xr:MultiLine>false</xr:MultiLine><xr:FillFromFillingValue>false</xr:FillFromFillingValue><xr:CreateOnInput>Auto</xr:CreateOnInput><xr:TypeReductionMode>TransformValues</xr:TypeReductionMode><xr:MaxValue/><xr:ToolTip/><xr:ExtendedEdit>false</xr:ExtendedEdit><xr:Format/><xr:ChoiceForm/><xr:QuickChoice>Auto</xr:QuickChoice><xr:ChoiceHistoryOnInput>Auto</xr:ChoiceHistoryOnInput><xr:EditFormat/><xr:PasswordMode>false</xr:PasswordMode><xr:DataHistory>Use</xr:DataHistory><xr:MarkNegatives>false</xr:MarkNegatives><xr:MinValue/><xr:Synonym/><xr:Comment/><xr:FullTextSearch>Use</xr:FullTextSearch><xr:ChoiceParameterLinks/><xr:FillValue/><xr:Mask/><xr:ChoiceParameters/></xr:StandardAttribute>"
        )
    }

    fn attribute_xml(uuid: &str, name: &str, nested: bool) -> String {
        let filling = if nested {
            "<FillFromFillingValue>false</FillFromFillingValue><FillValue/>"
        } else {
            ""
        };
        format!(
            "<Attribute uuid=\"{uuid}\"><Properties><Name>{name}</Name><Synonym/><Comment/><Type><v8:Type>xs:string</v8:Type><v8:StringQualifiers><v8:Length>40</v8:Length><v8:AllowedLength>Variable</v8:AllowedLength></v8:StringQualifiers></Type><PasswordMode>false</PasswordMode><Format/><EditFormat/><ToolTip/><MarkNegatives>false</MarkNegatives><Mask/><MultiLine>false</MultiLine><ExtendedEdit>false</ExtendedEdit><MinValue/><MaxValue/>{filling}<FillChecking>DontCheck</FillChecking><ChoiceFoldersAndItems>Items</ChoiceFoldersAndItems><ChoiceParameterLinks/><ChoiceParameters/><QuickChoice>Auto</QuickChoice><CreateOnInput>Auto</CreateOnInput><ChoiceForm/><LinkByType/><ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput></Properties><ChildObjects/></Attribute>"
        )
    }

    fn tabular_xml(family: UtilityFamily, seed: u32) -> String {
        let uuid = fixture_uuid(seed);
        let nested_uuid = fixture_uuid(seed + 1);
        format!(
            "<TabularSection uuid=\"{uuid}\"><InternalInfo>{}{}</InternalInfo><Properties><Name>Lines</Name><Synonym/><Comment/><ToolTip/><FillChecking>DontCheck</FillChecking><StandardAttributes>{}</StandardAttributes></Properties><ChildObjects>{}</ChildObjects></TabularSection>",
            generated(
                &format!("{}TabularSection.Lines", family.as_str()),
                "TabularSection",
                seed + 10,
            ),
            generated(
                &format!("{}TabularSectionRow.Lines", family.as_str()),
                "TabularSectionRow",
                seed + 20,
            ),
            standard_attribute("LineNumber"),
            attribute_xml(&nested_uuid, "Product", true),
        )
    }

    fn command_xml(uuid: &str) -> String {
        format!(
            "<Command uuid=\"{uuid}\"><Properties><Name>Open</Name><Synonym/><Comment/><Group>FormCommandBarImportant</Group><CommandParameterType/><ParameterUseMode>Single</ParameterUseMode><ModifiesData>false</ModifiesData><Representation>Auto</Representation><ToolTip/><Picture/><Shortcut/><OnMainServerUnavalableBehavior>Auto</OnMainServerUnavalableBehavior></Properties><ChildObjects/></Command>"
        )
    }

    fn report_xml(child_rich: bool) -> Vec<u8> {
        let children = if child_rich {
            format!(
                "{}{}{}<Form>Main</Form><Template>Schema</Template>",
                attribute_xml(&fixture_uuid(1_100), "Period", false),
                tabular_xml(UtilityFamily::Report, 1_200),
                command_xml(&fixture_uuid(1_300)),
            )
        } else {
            String::new()
        };
        let default_form = child_rich
            .then_some("Report.Analysis.Form.Main")
            .unwrap_or("");
        let schema = child_rich
            .then_some("Report.Analysis.Template.Schema")
            .unwrap_or("");
        format!(
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" version=\"2.20\"><Report uuid=\"{REPORT_UUID}\"><InternalInfo>{}</InternalInfo><Properties><Name>Analysis</Name><Synonym/><Comment/><UseStandardCommands>true</UseStandardCommands><DefaultForm>{default_form}</DefaultForm><AuxiliaryForm/><MainDataCompositionSchema>{schema}</MainDataCompositionSchema><DefaultSettingsForm/><AuxiliarySettingsForm/><DefaultVariantForm/><VariantsStorage/><SettingsStorage/><IncludeHelpInContents>false</IncludeHelpInContents><ExtendedPresentation/><Explanation/></Properties><ChildObjects>{children}</ChildObjects></Report></MetaDataObject>",
            root_generated(UtilityFamily::Report, "Analysis", 1_000),
        )
        .into_bytes()
    }

    fn processor_xml(child_rich: bool) -> Vec<u8> {
        let children = if child_rich {
            format!(
                "{}{}{}<Form>Main</Form><Template>Print</Template>",
                attribute_xml(&fixture_uuid(2_100), "Mode", false),
                tabular_xml(UtilityFamily::DataProcessor, 2_200),
                command_xml(&fixture_uuid(2_300)),
            )
        } else {
            String::new()
        };
        let default_form = child_rich
            .then_some("DataProcessor.Loader.Form.Main")
            .unwrap_or("");
        format!(
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" version=\"2.20\"><DataProcessor uuid=\"{PROCESSOR_UUID}\"><InternalInfo>{}</InternalInfo><Properties><Name>Loader</Name><Synonym/><Comment/><UseStandardCommands>true</UseStandardCommands><DefaultForm>{default_form}</DefaultForm><AuxiliaryForm/><IncludeHelpInContents>false</IncludeHelpInContents><ExtendedPresentation/><Explanation/></Properties><ChildObjects>{children}</ChildObjects></DataProcessor></MetaDataObject>",
            root_generated(UtilityFamily::DataProcessor, "Loader", 2_000),
        )
        .into_bytes()
    }

    fn enum_xml() -> Vec<u8> {
        format!(
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" version=\"2.20\"><Enum uuid=\"{ENUM_UUID}\"><InternalInfo>{}</InternalInfo><Properties><Name>Modes</Name><Synonym/><Comment/><UseStandardCommands>true</UseStandardCommands><StandardAttributes>{}{}</StandardAttributes><Characteristics/><QuickChoice>true</QuickChoice><ChoiceMode>BothWays</ChoiceMode><DefaultListForm/><DefaultChoiceForm/><AuxiliaryListForm/><AuxiliaryChoiceForm/><ListPresentation/><ExtendedListPresentation/><Explanation/><ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput></Properties><ChildObjects><EnumValue uuid=\"{}\"><Properties><Name>First</Name><Synonym/><Comment/></Properties><ChildObjects/></EnumValue><EnumValue uuid=\"{}\"><Properties><Name>Second</Name><Synonym/><Comment/><Color>auto</Color></Properties><ChildObjects/></EnumValue></ChildObjects></Enum></MetaDataObject>",
            root_generated(UtilityFamily::Enum, "Modes", 3_000),
            standard_attribute("Order"),
            standard_attribute("Ref"),
            fixture_uuid(3_100),
            fixture_uuid(3_101),
        )
        .into_bytes()
    }

    fn settings_xml(with_form: bool) -> Vec<u8> {
        let child = if with_form { "<Form>Main</Form>" } else { "" };
        let default = with_form
            .then_some("SettingsStorage.User.Form.Main")
            .unwrap_or("");
        format!(
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" version=\"2.20\"><SettingsStorage uuid=\"{SETTINGS_UUID}\"><InternalInfo>{}</InternalInfo><Properties><Name>User</Name><Synonym/><Comment/><DefaultSaveForm>{default}</DefaultSaveForm><DefaultLoadForm>{default}</DefaultLoadForm><AuxiliarySaveForm/><AuxiliaryLoadForm/></Properties><ChildObjects>{child}</ChildObjects></SettingsStorage></MetaDataObject>",
            root_generated(UtilityFamily::SettingsStorage, "User", 4_000),
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

    fn axes(dialect: &str) -> CompileAxes {
        CompileAxes::new(
            XmlDialect::parse(dialect).unwrap(),
            Some(PlatformBuild::parse("8.3.27.1989").unwrap()),
            None,
            StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            None,
        )
    }

    fn configuration(
        family: UtilityFamily,
        child_rich: bool,
        source_dialect: &str,
    ) -> CanonicalConfiguration {
        let xml = match family {
            UtilityFamily::Report => report_xml(child_rich),
            UtilityFamily::DataProcessor => processor_xml(child_rich),
            UtilityFamily::Enum => enum_xml(),
            UtilityFamily::SettingsStorage => settings_xml(child_rich),
        };
        let document = XmlReader::from_slice(&xml).unwrap();
        let registry = bundled_metadata_registry();
        let envelope = registry
            .decode(
                &FamilyId::parse(family.as_str()).unwrap(),
                &document,
                ProfileId::parse("xml-2.20").unwrap(),
                ObjectPath::root(),
            )
            .unwrap();
        let converted = registry
            .encode(&envelope, &ProfileId::parse("xml-2.21").unwrap())
            .unwrap();
        let converted_document = XmlReader::from_slice(&converted).unwrap();
        let converted_envelope = registry
            .decode(
                &FamilyId::parse(family.as_str()).unwrap(),
                &converted_document,
                ProfileId::parse("xml-2.21").unwrap(),
                ObjectPath::root(),
            )
            .unwrap();
        assert_eq!(
            envelope.root().identity(),
            converted_envelope.root().identity()
        );
        assert_eq!(envelope.root().kind(), converted_envelope.root().kind());
        assert_eq!(
            envelope.root().generated_types(),
            converted_envelope.root().generated_types()
        );
        assert_eq!(
            envelope.root().properties(),
            converted_envelope.root().properties()
        );
        assert_eq!(
            envelope.descendants().len(),
            converted_envelope.descendants().len()
        );
        for (source, target) in envelope
            .descendants()
            .iter()
            .zip(converted_envelope.descendants())
        {
            assert_eq!(source.identity(), target.identity());
            assert_eq!(source.kind(), target.kind());
            assert_eq!(source.owner(), target.owner());
            assert_eq!(source.generated_types(), target.generated_types());
            assert_eq!(source.properties(), target.properties());
        }
        let selected = if source_dialect == "2.21" {
            &converted_envelope
        } else {
            assert_eq!(source_dialect, "2.20");
            &envelope
        };
        let mut objects = vec![simple_object(
            1,
            CONFIGURATION_UUID,
            "Configuration",
            "Fixture",
            None,
        )];
        objects.push(selected.root().clone());
        objects.extend(selected.descendants().iter().cloned());
        if child_rich {
            let (owner, template) = match family {
                UtilityFamily::Report => ("Report.Analysis", Some("Schema")),
                UtilityFamily::DataProcessor => ("DataProcessor.Loader", Some("Print")),
                UtilityFamily::SettingsStorage => ("SettingsStorage.User", None),
                UtilityFamily::Enum => ("Enum.Modes", None),
            };
            objects.push(simple_object(
                2,
                &fixture_uuid(9_001 + family as u32),
                "Form",
                "Main",
                Some(&format!("{owner}.Form.Main")),
            ));
            if let Some(template) = template {
                objects.push(simple_object(
                    3,
                    &fixture_uuid(9_101 + family as u32),
                    "Template",
                    template,
                    Some(&format!("{owner}.Template.{template}")),
                ));
            }
        }
        CanonicalConfiguration::new(objects).unwrap()
    }

    fn compile_and_decode(
        family: UtilityFamily,
        child_rich: bool,
        source_dialect: &str,
    ) -> UtilityNativeIr {
        let configuration = configuration(family, child_rich, source_dialect);
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
            UtilityFamily::Report => REPORT_UUID,
            UtilityFamily::DataProcessor => PROCESSOR_UUID,
            UtilityFamily::Enum => ENUM_UUID,
            UtilityFamily::SettingsStorage => SETTINGS_UUID,
        })
        .unwrap();
        let profile = UtilityMetadataProfile::fixture("platform-test", family);
        let first = compile_utility_metadata(
            &validated,
            &graph,
            root_uuid,
            &axes(source_dialect),
            &profile,
        )
        .unwrap();
        let second = compile_utility_metadata(
            &validated,
            &graph,
            root_uuid,
            &axes(source_dialect),
            &profile,
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.target().key().as_str(), root_uuid.to_string());
        decode_utility_blob(
            first.outcome().compiled_payload().unwrap().bytes(),
            &profile,
        )
        .unwrap()
    }

    #[test]
    fn all_utility_families_compile_deterministically_and_decode_strictly() {
        for family in [
            UtilityFamily::Report,
            UtilityFamily::DataProcessor,
            UtilityFamily::Enum,
            UtilityFamily::SettingsStorage,
        ] {
            let ir = compile_and_decode(family, family != UtilityFamily::Enum, "2.20");
            assert_eq!(ir.family, family);
            assert!(!ir.name.is_empty());
            assert_eq!(
                ir.generated_types.len(),
                match family {
                    UtilityFamily::Report | UtilityFamily::DataProcessor => 2,
                    UtilityFamily::Enum => 3,
                    UtilityFamily::SettingsStorage => 1,
                }
            );
            match family {
                UtilityFamily::Report | UtilityFamily::DataProcessor => {
                    assert_eq!(ir.attribute_uuids.len(), 1);
                    assert_eq!(ir.tabular_sections.len(), 1);
                    assert_eq!(ir.tabular_sections[0].attribute_uuids.len(), 1);
                    assert_eq!(ir.command_uuids.len(), 1);
                    assert_eq!(ir.form_uuids.len(), 1);
                    assert_eq!(ir.template_uuids.len(), 1);
                }
                UtilityFamily::Enum => assert_eq!(ir.enum_value_uuids.len(), 2),
                UtilityFamily::SettingsStorage => assert_eq!(ir.form_uuids.len(), 1),
            }
        }
        for family in [
            UtilityFamily::Report,
            UtilityFamily::DataProcessor,
            UtilityFamily::Enum,
            UtilityFamily::SettingsStorage,
        ] {
            assert_eq!(compile_and_decode(family, false, "2.21").family, family);
        }
    }

    #[test]
    fn bundled_profile_selects_every_utility_layout_and_future_values_fail_closed() {
        let registry = crate::profile_registry::load_bundled_profile_registry().unwrap();
        let effective = registry
            .get(&ProfileId::parse("platform-8.3.27.1989").unwrap())
            .unwrap();
        for family in [
            UtilityFamily::Report,
            UtilityFamily::DataProcessor,
            UtilityFamily::Enum,
            UtilityFamily::SettingsStorage,
        ] {
            assert_eq!(
                UtilityMetadataProfile::from_effective(effective, family)
                    .unwrap()
                    .family,
                family
            );
        }
        let mut future = effective.clone();
        future.constants.get_mut(REPORT_LAYOUT_KEY).unwrap().value = "report-v2-future".to_owned();
        assert!(matches!(
            UtilityMetadataProfile::from_effective(&future, UtilityFamily::Report),
            Err(UtilityProfileError::UnsupportedLayout {
                family: UtilityFamily::Report,
                key: REPORT_LAYOUT_KEY,
                ..
            })
        ));
    }

    #[test]
    fn decoder_rejects_unknown_root_marker_and_extra_field() {
        let family = UtilityFamily::SettingsStorage;
        let configuration = configuration(family, false, "2.20");
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
        let uuid = ObjectUuid::parse(SETTINGS_UUID).unwrap();
        let profile = UtilityMetadataProfile::fixture("platform-test", family);
        let entry =
            compile_utility_metadata(&validated, &graph, uuid, &axes("2.20"), &profile).unwrap();
        let plain = inflate_bounded(entry.outcome().compiled_payload().unwrap().bytes()).unwrap();
        let parsed = NativeParser::new(&plain).parse().unwrap();

        let mut unknown = parsed.clone();
        let NativeValue::List(root) = &mut unknown else {
            panic!("compiler must emit a root list");
        };
        root[2] = token("3");
        let blob = raw_deflate(&serialize_native(&unknown).unwrap()).unwrap();
        assert!(matches!(
            decode_utility_blob(&blob, &profile),
            Err(UtilityBuildError::Native(_))
        ));

        let mut extra = parsed;
        let NativeValue::List(root) = &mut extra else {
            panic!("compiler must emit a root list");
        };
        root.push(token("future"));
        let blob = raw_deflate(&serialize_native(&extra).unwrap()).unwrap();
        assert!(matches!(
            decode_utility_blob(&blob, &profile),
            Err(UtilityBuildError::Native(_))
        ));
    }
}
