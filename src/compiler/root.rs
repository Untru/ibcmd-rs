//! Exact `root` and Configuration-body codecs.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter, Write as _};
use std::io::{self, Write};

use flate2::Compression;
use flate2::write::DeflateEncoder;
use ibcmd_core::artifact::ProfileId;
use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::storage::{
    MultipartIdentity, StorageBuildError, StorageKey, StoragePatchBuildError, StoragePatchEntry,
    StoragePatchOutcome, StoragePatchTarget, StorageProvenance,
};
use ibcmd_core::value::MAX_CANONICAL_TEXT_BYTES;

use super::graph::{BootstrapGraph, BootstrapGraphError, SpecialEntryKind};
use super::identity::{BootstrapIdentities, derive_generation_uuid_v8};
use super::version::{SpecialEntryProfile, SpecialEntryProfileError};

const UTF8_BOM: &[u8; 3] = b"\xef\xbb\xbf";
const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";
const DESIGN_TIME_REFERENCE_CLASS_UUID: &str = "157fa490-4ce9-11d4-9415-008048da11f9";
const USE_PURPOSE_CLASS_UUID: &str = "1708fdaa-cbce-4289-b373-07a5a74bee91";
const COMPATIBILITY_OPTION_CLASS_UUID: &str = "e4c53f94-e5f7-4a34-8c10-218bd811cae1";
const MAX_CONFIGURATION_LANGUAGE_BYTES: usize = 64;

/// Failure to compile an evidence-backed special entry.
#[derive(Debug)]
pub enum SpecialEntryBuildError {
    /// The selected profile has no exact special-entry layout.
    Profile(SpecialEntryProfileError),
    /// The bootstrap graph cannot resolve or verify an entry.
    Graph(BootstrapGraphError),
    /// A neutral storage component rejected a derived value.
    Storage(StorageBuildError),
    /// A neutral storage patch component rejected a payload or outcome.
    Patch(StoragePatchBuildError),
    /// Raw-DEFLATE encoding failed.
    Deflate(io::Error),
    /// Physical routes and special-entry layouts came from different profiles.
    ProfileMismatch {
        /// Profile used to resolve graph routes.
        graph: ProfileId,
        /// Profile used to resolve special layouts.
        special: ProfileId,
    },
    /// A serialized service-entry pair count overflowed its machine bound.
    PairCountOverflow {
        /// Service entry being serialized.
        entry: &'static str,
    },
    /// The typed Configuration projection violates the exact layout contract.
    InvalidConfigurationModel {
        /// Stable, path-oriented explanation.
        reason: String,
    },
    /// A top-level family has no slot in the selected Configuration layout.
    UnsupportedConfigurationFamily {
        /// Object that cannot be routed.
        object: ObjectUuid,
        /// Open canonical family name.
        kind: String,
    },
    /// A top-level object is absent from the physical inventory.
    MissingConfigurationRoute {
        /// Object whose primary row cannot be resolved.
        object: ObjectUuid,
    },
}

impl Display for SpecialEntryBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(source) => {
                write!(formatter, "unsupported special-entry profile: {source}")
            }
            Self::Graph(source) => write!(formatter, "invalid bootstrap graph: {source}"),
            Self::Storage(source) => {
                write!(formatter, "invalid special-entry storage value: {source}")
            }
            Self::Patch(source) => write!(formatter, "invalid special-entry patch: {source}"),
            Self::Deflate(source) => {
                write!(formatter, "failed to raw-deflate special entry: {source}")
            }
            Self::ProfileMismatch { graph, special } => write!(
                formatter,
                "bootstrap graph profile `{graph}` differs from special-entry profile `{special}`"
            ),
            Self::PairCountOverflow { entry } => {
                write!(formatter, "{entry} pair count overflowed usize")
            }
            Self::InvalidConfigurationModel { reason } => {
                write!(formatter, "invalid Configuration body model: {reason}")
            }
            Self::UnsupportedConfigurationFamily { object, kind } => write!(
                formatter,
                "top-level Configuration child {object} has unsupported family `{kind}`"
            ),
            Self::MissingConfigurationRoute { object } => write!(
                formatter,
                "top-level Configuration child {object} has no primary storage route"
            ),
        }
    }
}

impl Error for SpecialEntryBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Profile(source) => Some(source),
            Self::Graph(source) => Some(source),
            Self::Storage(source) => Some(source),
            Self::Patch(source) => Some(source),
            Self::Deflate(source) => Some(source),
            Self::ProfileMismatch { .. } => None,
            Self::PairCountOverflow { .. } => None,
            Self::InvalidConfigurationModel { .. }
            | Self::UnsupportedConfigurationFamily { .. }
            | Self::MissingConfigurationRoute { .. } => None,
        }
    }
}

impl From<SpecialEntryProfileError> for SpecialEntryBuildError {
    fn from(source: SpecialEntryProfileError) -> Self {
        Self::Profile(source)
    }
}

impl From<BootstrapGraphError> for SpecialEntryBuildError {
    fn from(source: BootstrapGraphError) -> Self {
        Self::Graph(source)
    }
}

impl From<StorageBuildError> for SpecialEntryBuildError {
    fn from(source: StorageBuildError) -> Self {
        Self::Storage(source)
    }
}

impl From<StoragePatchBuildError> for SpecialEntryBuildError {
    fn from(source: StoragePatchBuildError) -> Self {
        Self::Patch(source)
    }
}

/// Native startup mode retained by the Configuration root properties.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigurationRunMode {
    /// Ordinary application startup.
    OrdinaryApplication,
    /// Managed application startup.
    ManagedApplication,
}

impl ConfigurationRunMode {
    const fn native_code(self) -> &'static str {
        match self {
            Self::OrdinaryApplication => "0",
            Self::ManagedApplication => "1",
        }
    }
}

/// Native script language retained by the Configuration root properties.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigurationScriptVariant {
    /// Russian-language script syntax.
    Russian,
    /// English-language script syntax.
    English,
}

impl ConfigurationScriptVariant {
    const fn native_code(self) -> &'static str {
        match self {
            Self::Russian => "0",
            Self::English => "1",
        }
    }
}

/// One exact language/content pair in a native localized property.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigurationLocalizedString {
    /// Non-empty source language token.
    pub language: String,
    /// Exact Unicode content.
    pub content: String,
}

impl ConfigurationLocalizedString {
    /// Retains a language/content pair; compilation applies the shared bounds.
    pub fn new(language: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            language: language.into(),
            content: content.into(),
        }
    }
}

/// Typed, base-free projection of the Configuration properties supported by
/// the `configuration-v68-seven-sections-v1` profile cohort.
///
/// Fields outside this projection are deliberately not accepted as raw native
/// fragments. An XML adapter must reject non-default opaque Configuration
/// facets before calling this compiler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigurationBodyProperties {
    /// Configuration name.
    pub name: String,
    /// Localized synonym.
    pub synonyms: Vec<ConfigurationLocalizedString>,
    /// Configuration comment.
    pub comment: String,
    /// Optional object-name prefix.
    pub name_prefix: String,
    /// Startup mode.
    pub default_run_mode: ConfigurationRunMode,
    /// Localized brief information.
    pub brief_information: Vec<ConfigurationLocalizedString>,
    /// Localized detailed information.
    pub detailed_information: Vec<ConfigurationLocalizedString>,
    /// Localized copyright text.
    pub copyright: Vec<ConfigurationLocalizedString>,
    /// Localized vendor information address.
    pub vendor_information_address: Vec<ConfigurationLocalizedString>,
    /// Localized configuration information address.
    pub configuration_information_address: Vec<ConfigurationLocalizedString>,
    /// Default Style object, when explicitly selected.
    pub default_style: Option<ObjectUuid>,
    /// Default Language object, when explicitly selected.
    pub default_language: Option<ObjectUuid>,
    /// Script syntax variant.
    pub script_variant: ConfigurationScriptVariant,
    /// Vendor name.
    pub vendor: String,
    /// Application version text.
    pub version: String,
    /// Update catalog address.
    pub update_catalog_address: String,
    /// Four optional SettingsStorage references in native slot order.
    pub settings_storages: [Option<ObjectUuid>; 4],
    /// Exact numeric ConfigurationExtensionCompatibilityMode selector.
    pub extension_compatibility_mode: u32,
    /// Exact numeric CompatibilityMode selector.
    pub compatibility_mode: u32,
    /// Whether the PlatformApplication use purpose is declared.
    pub use_platform_application: bool,
    /// Default Role objects in declared order.
    pub default_roles: Vec<ObjectUuid>,
    /// Enabled mobile functionality IDs (`0..=27` and `32..=41`).
    pub enabled_mobile_functionalities: Vec<u32>,
}

impl ConfigurationBodyProperties {
    /// Creates the smallest fully typed projection for the selected target
    /// compatibility. All optional references and mobile capabilities are off.
    pub fn minimal(name: impl Into<String>, compatibility: u32) -> Self {
        Self {
            name: name.into(),
            synonyms: Vec::new(),
            comment: String::new(),
            name_prefix: String::new(),
            default_run_mode: ConfigurationRunMode::ManagedApplication,
            brief_information: Vec::new(),
            detailed_information: Vec::new(),
            copyright: Vec::new(),
            vendor_information_address: Vec::new(),
            configuration_information_address: Vec::new(),
            default_style: None,
            default_language: None,
            script_variant: ConfigurationScriptVariant::Russian,
            vendor: String::new(),
            version: String::new(),
            update_catalog_address: String::new(),
            settings_storages: [None; 4],
            extension_compatibility_mode: compatibility,
            compatibility_mode: compatibility,
            use_platform_application: true,
            default_roles: Vec::new(),
            enabled_mobile_functionalities: Vec::new(),
        }
    }
}

#[derive(Clone, Copy)]
struct ConfigurationFamilySlot {
    class_id: &'static str,
    kind: Option<&'static str>,
}

impl ConfigurationFamilySlot {
    const fn mapped(class_id: &'static str, kind: &'static str) -> Self {
        Self {
            class_id,
            kind: Some(kind),
        }
    }

    const fn reserved(class_id: &'static str) -> Self {
        Self {
            class_id,
            kind: None,
        }
    }
}

#[derive(Clone, Copy)]
enum ConfigurationSectionWrapper {
    RootProperties,
    Directory,
    ZeroIdentity,
    DirectIdentity,
}

#[derive(Clone, Copy)]
struct ConfigurationSectionLayout {
    class_id: &'static str,
    wrapper: ConfigurationSectionWrapper,
    slots: &'static [ConfigurationFamilySlot],
}

const CONFIGURATION_SECTION_1: [ConfigurationFamilySlot; 25] = [
    ConfigurationFamilySlot::mapped("09736b02-9cac-4e3f-b4f7-d3e9576ab948", "Role"),
    ConfigurationFamilySlot::mapped("0c89c792-16c3-11d5-b96b-0050bae0a95d", "CommonTemplate"),
    ConfigurationFamilySlot::mapped("0fe48980-252d-11d6-a3c7-0050bae0a776", "CommonModule"),
    ConfigurationFamilySlot::mapped("0fffc09c-8f4c-47cc-b41c-8d5c5a221d79", "HTTPService"),
    ConfigurationFamilySlot::mapped("11bdaf85-d5ad-4d91-bb24-aa0eee139052", "ScheduledJob"),
    ConfigurationFamilySlot::mapped("15794563-ccec-41f6-a83c-ec5f7b9a5bc1", "CommonAttribute"),
    ConfigurationFamilySlot::mapped("24c43748-c938-45d0-8d14-01424a72b11e", "SessionParameter"),
    ConfigurationFamilySlot::mapped(
        "30d554db-541e-4f62-8970-a1c6dcfeb2bc",
        "FunctionalOptionsParameter",
    ),
    ConfigurationFamilySlot::mapped("37f2fa9a-b276-11d4-9435-004095e12fc7", "Subsystem"),
    ConfigurationFamilySlot::mapped("39bddf6a-0c3c-452b-921c-d99cfa1c2f1b", "Interface"),
    ConfigurationFamilySlot::mapped("3e5404af-6ef8-4c73-ad11-91bd2dfac4c8", "Style"),
    ConfigurationFamilySlot::mapped("3e7bfcc0-067d-11d6-a3c7-0050bae0a776", "FilterCriterion"),
    ConfigurationFamilySlot::mapped("46b4cd97-fd13-4eaa-aba2-3bddd7699218", "SettingsStorage"),
    ConfigurationFamilySlot::mapped("4e828da6-0f44-4b5b-b1c0-a2b3cfe7bdcc", "EventSubscription"),
    ConfigurationFamilySlot::mapped("58848766-36ea-4076-8800-e91eb49590d7", "StyleItem"),
    ConfigurationFamilySlot::mapped("6e6dc072-b7ac-41e7-8f88-278d25b6da2a", "Bot"),
    ConfigurationFamilySlot::mapped("7dcd43d9-aca5-4926-b549-1842e6a4e8cf", "CommonPicture"),
    ConfigurationFamilySlot::mapped("857c4a91-e5f4-4fac-86ec-787626f1c108", "ExchangePlan"),
    ConfigurationFamilySlot::mapped("8657032e-7740-4e1d-a3ba-5dd6e8afb78f", "WebService"),
    ConfigurationFamilySlot::mapped("9cd510ce-abfc-11d4-9434-004095e12fc7", "Language"),
    ConfigurationFamilySlot::reserved("a7641777-7813-45c6-96ef-9d51587a6ac6"),
    ConfigurationFamilySlot::mapped("af547940-3268-434f-a3e7-e47d6d2638c3", "FunctionalOption"),
    ConfigurationFamilySlot::mapped("c045099e-13b9-4fb6-9d50-fca00202971e", "DefinedType"),
    ConfigurationFamilySlot::mapped("cc9df798-7c94-4616-97d2-7aa0b7bc515e", "XDTOPackage"),
    ConfigurationFamilySlot::mapped("d26096fb-7a5d-4df9-af63-47d04771fa9b", "WSReference"),
];

const CONFIGURATION_SECTION_2: [ConfigurationFamilySlot; 15] = [
    ConfigurationFamilySlot::mapped("0195e80c-b157-11d4-9435-004095e12fc7", "Constant"),
    ConfigurationFamilySlot::mapped("061d872a-5787-460e-95ac-ed74ea3a3e84", "Document"),
    ConfigurationFamilySlot::mapped("07ee8426-87f1-11d5-b99c-0050bae0a95d", "CommonForm"),
    ConfigurationFamilySlot::mapped(
        "13134201-f60b-11d5-a3c7-0050bae0a776",
        "InformationRegister",
    ),
    ConfigurationFamilySlot::mapped("1c57eabe-7349-44b3-b1de-ebfeab67b47d", "CommandGroup"),
    ConfigurationFamilySlot::mapped("2f1a5187-fb0e-4b05-9489-dc5dd6412348", "CommonCommand"),
    ConfigurationFamilySlot::mapped("36a8e346-9aaa-4af9-bdbd-83be3c177977", "DocumentNumerator"),
    ConfigurationFamilySlot::mapped("4612bd75-71b7-4a5c-8cc5-2b0b65f9fa0d", "DocumentJournal"),
    ConfigurationFamilySlot::mapped("631b75a0-29e2-11d6-a3c7-0050bae0a776", "Report"),
    ConfigurationFamilySlot::mapped(
        "82a1b659-b220-4d94-a9bd-14d757b95a48",
        "ChartOfCharacteristicTypes",
    ),
    ConfigurationFamilySlot::mapped(
        "b64d9a40-1642-11d6-a3c7-0050bae0a776",
        "AccumulationRegister",
    ),
    ConfigurationFamilySlot::mapped("bc587f20-35d9-11d6-a3c7-0050bae0a776", "Sequence"),
    ConfigurationFamilySlot::mapped("bf845118-327b-4682-b5c6-285d2a0eb296", "DataProcessor"),
    ConfigurationFamilySlot::mapped("cf4abea6-37b2-11d4-940f-008048da11f9", "Catalog"),
    ConfigurationFamilySlot::mapped("f6a80749-5ad7-400b-8519-39dc5dff2542", "Enum"),
];

const CONFIGURATION_SECTION_3: [ConfigurationFamilySlot; 2] = [
    ConfigurationFamilySlot::mapped("238e7e88-3c5f-48b2-8a3b-81ebbecb20ed", "ChartOfAccounts"),
    ConfigurationFamilySlot::mapped("2deed9b8-0056-4ffe-a473-c20a6c32a0bc", "AccountingRegister"),
];

const CONFIGURATION_SECTION_4: [ConfigurationFamilySlot; 2] = [
    ConfigurationFamilySlot::mapped(
        "30b100d6-b29f-47ac-aec7-cb8ca8a54767",
        "ChartOfCalculationTypes",
    ),
    ConfigurationFamilySlot::mapped(
        "f2de87a8-64e5-45eb-a22d-b3aedab050e7",
        "CalculationRegister",
    ),
];

const CONFIGURATION_SECTION_5: [ConfigurationFamilySlot; 2] = [
    ConfigurationFamilySlot::mapped("3e63355c-1378-4953-be9b-1deb5fb6bec5", "Task"),
    ConfigurationFamilySlot::mapped("fcd3404e-1523-48ce-9bc0-ecdb822684a1", "BusinessProcess"),
];

const CONFIGURATION_SECTION_6: [ConfigurationFamilySlot; 1] = [ConfigurationFamilySlot::mapped(
    "5274d9fc-9c3a-4a71-8f5e-a0db8ab23de5",
    "ExternalDataSource",
)];

const CONFIGURATION_SECTION_7: [ConfigurationFamilySlot; 1] = [ConfigurationFamilySlot::mapped(
    "bf3420b0-f6f9-41a0-b83a-fe9d4ab0b65d",
    "IntegrationService",
)];

const CONFIGURATION_SECTIONS: [ConfigurationSectionLayout; 7] = [
    ConfigurationSectionLayout {
        class_id: "9cd510cd-abfc-11d4-9434-004095e12fc7",
        wrapper: ConfigurationSectionWrapper::RootProperties,
        slots: &CONFIGURATION_SECTION_1,
    },
    ConfigurationSectionLayout {
        class_id: "9fcd25a0-4822-11d4-9414-008048da11f9",
        wrapper: ConfigurationSectionWrapper::Directory,
        slots: &CONFIGURATION_SECTION_2,
    },
    ConfigurationSectionLayout {
        class_id: "e3687481-0a87-462c-a166-9f34594f9bba",
        wrapper: ConfigurationSectionWrapper::ZeroIdentity,
        slots: &CONFIGURATION_SECTION_3,
    },
    ConfigurationSectionLayout {
        class_id: "9de14907-ec23-4a07-96f0-85521cb6b53b",
        wrapper: ConfigurationSectionWrapper::DirectIdentity,
        slots: &CONFIGURATION_SECTION_4,
    },
    ConfigurationSectionLayout {
        class_id: "51f2d5d8-ea4d-4064-8892-82951750031e",
        wrapper: ConfigurationSectionWrapper::ZeroIdentity,
        slots: &CONFIGURATION_SECTION_5,
    },
    ConfigurationSectionLayout {
        class_id: "e68182ea-4237-4383-967f-90c1e3370bc7",
        wrapper: ConfigurationSectionWrapper::DirectIdentity,
        slots: &CONFIGURATION_SECTION_6,
    },
    ConfigurationSectionLayout {
        class_id: "fb282519-d103-4dd3-bc12-cb271d631dfc",
        wrapper: ConfigurationSectionWrapper::DirectIdentity,
        slots: &CONFIGURATION_SECTION_7,
    },
];

/// Compiles the exact evidence-backed native `root` row.
///
/// Plaintext is UTF-8 BOM followed by `{2,<configuration UUID>,}`. The empty
/// third field and trailing comma are part of the native contract.
pub fn compile_root(
    graph: &BootstrapGraph,
    profile: &SpecialEntryProfile,
) -> Result<StoragePatchEntry, SpecialEntryBuildError> {
    ensure_graph_profile(graph, profile)?;
    graph.validate_special_references()?;
    let mut plaintext = Vec::with_capacity(80);
    plaintext.extend_from_slice(UTF8_BOM);
    write!(&mut plaintext, "{{2,{},}}", graph.configuration_uuid())
        .expect("writing to Vec cannot fail");
    compiled_special_entry(SpecialEntryKind::Root, &plaintext, profile)
}

/// Compiles the exact profile-backed Configuration metadata body without a
/// base artifact.
///
/// The seven internal object IDs are deterministic UUIDv8 values derived from
/// the target profile, outer Configuration UUID and section class ID. Family
/// lists are projected from top-level canonical identities in UUID order.
pub fn compile_configuration_body(
    identities: &BootstrapIdentities,
    graph: &BootstrapGraph,
    profile: &SpecialEntryProfile,
    properties: &ConfigurationBodyProperties,
) -> Result<StoragePatchEntry, SpecialEntryBuildError> {
    ensure_graph_profile(graph, profile)?;
    graph.validate_special_references()?;
    if identities.configuration_uuid() != graph.configuration_uuid() {
        return invalid_configuration(
            "identity and storage graphs name different Configuration UUIDs",
        );
    }
    validate_configuration_properties(identities, graph, properties)?;
    let families = collect_configuration_families(identities, graph)?;
    let section_ids = configuration_section_ids(graph, profile);

    let mut plaintext = String::new();
    plaintext.push('\u{feff}');
    write!(
        &mut plaintext,
        "{{2,\r\n{{{}}},7",
        graph.configuration_uuid()
    )
    .expect("writing to String cannot fail");
    for (index, section) in CONFIGURATION_SECTIONS.iter().enumerate() {
        plaintext.push_str(",\r\n{");
        plaintext.push_str(section.class_id);
        plaintext.push_str(",\r\n");
        push_configuration_section(
            &mut plaintext,
            section,
            &section_ids[index],
            &families[index],
            properties,
        );
        plaintext.push('}');
    }
    plaintext.push_str(",\r\n{{0,\"\",\"\"}}\r\n}");

    let key = graph.configuration_uuid().to_string();
    let bytes = raw_deflate(plaintext.as_bytes())?;
    Ok(StoragePatchEntry::new(
        StoragePatchTarget::new(
            StorageKey::new(&key)?,
            MultipartIdentity::single(),
            special_provenance(profile, "configuration-body")?,
        ),
        StoragePatchOutcome::compiled(bytes)?,
    ))
}

fn configuration_section_ids(graph: &BootstrapGraph, profile: &SpecialEntryProfile) -> [String; 7] {
    std::array::from_fn(|index| {
        derive_generation_uuid_v8(
            b"configuration-contained-object",
            &[
                profile.profile_id().as_str().as_bytes(),
                graph.configuration_uuid().as_bytes(),
                CONFIGURATION_SECTIONS[index].class_id.as_bytes(),
            ],
        )
        .to_string()
    })
}

fn collect_configuration_families(
    identities: &BootstrapIdentities,
    graph: &BootstrapGraph,
) -> Result<Vec<Vec<Vec<ObjectUuid>>>, SpecialEntryBuildError> {
    let mut families = CONFIGURATION_SECTIONS
        .iter()
        .map(|section| vec![Vec::new(); section.slots.len()])
        .collect::<Vec<_>>();

    for object in identities.objects() {
        if object.uuid() == identities.configuration_uuid() {
            if object.kind().as_str() != "Configuration" || object.owner().is_some() {
                return invalid_configuration(
                    "the Configuration identity must be a top-level Configuration object",
                );
            }
            continue;
        }
        if object.owner().is_some() {
            continue;
        }
        if graph.primary_object_entry(object.uuid()).is_none() {
            return Err(SpecialEntryBuildError::MissingConfigurationRoute {
                object: object.uuid(),
            });
        }

        let mut target = None;
        for (section_index, section) in CONFIGURATION_SECTIONS.iter().enumerate() {
            for (slot_index, slot) in section.slots.iter().enumerate() {
                if slot.kind == Some(object.kind().as_str()) {
                    target = Some((section_index, slot_index));
                    break;
                }
            }
            if target.is_some() {
                break;
            }
        }
        let Some((section_index, slot_index)) = target else {
            return Err(SpecialEntryBuildError::UnsupportedConfigurationFamily {
                object: object.uuid(),
                kind: object.kind().as_str().to_owned(),
            });
        };
        families[section_index][slot_index].push(object.uuid());
    }

    for section in &mut families {
        for family in section {
            family.sort_unstable();
        }
    }
    Ok(families)
}

fn validate_configuration_properties(
    identities: &BootstrapIdentities,
    graph: &BootstrapGraph,
    properties: &ConfigurationBodyProperties,
) -> Result<(), SpecialEntryBuildError> {
    if properties.name.is_empty() {
        return invalid_configuration("Properties/Name is empty");
    }
    for (path, value) in [
        ("Properties/Name", properties.name.as_str()),
        ("Properties/Comment", properties.comment.as_str()),
        ("Properties/NamePrefix", properties.name_prefix.as_str()),
        ("Properties/Vendor", properties.vendor.as_str()),
        ("Properties/Version", properties.version.as_str()),
        (
            "Properties/UpdateCatalogAddress",
            properties.update_catalog_address.as_str(),
        ),
    ] {
        validate_configuration_text(path, value)?;
    }
    for (path, values) in [
        ("Properties/Synonym", properties.synonyms.as_slice()),
        (
            "Properties/BriefInformation",
            properties.brief_information.as_slice(),
        ),
        (
            "Properties/DetailedInformation",
            properties.detailed_information.as_slice(),
        ),
        ("Properties/Copyright", properties.copyright.as_slice()),
        (
            "Properties/VendorInformationAddress",
            properties.vendor_information_address.as_slice(),
        ),
        (
            "Properties/ConfigurationInformationAddress",
            properties.configuration_information_address.as_slice(),
        ),
    ] {
        validate_localized_strings(path, values)?;
    }
    for (path, value) in [
        (
            "Properties/ConfigurationExtensionCompatibilityMode",
            properties.extension_compatibility_mode,
        ),
        (
            "Properties/CompatibilityMode",
            properties.compatibility_mode,
        ),
    ] {
        if value != 0 && value < 80_000 {
            return invalid_configuration(&format!(
                "{path} selector {value} is neither zero nor a platform compatibility value"
            ));
        }
    }

    validate_reference(
        identities,
        graph,
        "Properties/DefaultStyle",
        properties.default_style,
        "Style",
    )?;
    validate_reference(
        identities,
        graph,
        "Properties/DefaultLanguage",
        properties.default_language,
        "Language",
    )?;
    for (index, reference) in properties.settings_storages.iter().enumerate() {
        validate_reference(
            identities,
            graph,
            &format!("Properties/SettingsStorage[{index}]"),
            *reference,
            "SettingsStorage",
        )?;
    }
    let mut roles = BTreeSet::new();
    for (index, role) in properties.default_roles.iter().copied().enumerate() {
        if !roles.insert(role) {
            return invalid_configuration(&format!(
                "Properties/DefaultRoles[{index}] duplicates {role}"
            ));
        }
        validate_reference(
            identities,
            graph,
            &format!("Properties/DefaultRoles[{index}]"),
            Some(role),
            "Role",
        )?;
    }

    let allowed_mobile_ids = mobile_functionality_ids().collect::<BTreeSet<_>>();
    let mut mobile_ids = BTreeSet::new();
    for id in &properties.enabled_mobile_functionalities {
        if !allowed_mobile_ids.contains(id) {
            return invalid_configuration(&format!(
                "Properties/UsedMobileApplicationFunctionalities contains unsupported ID {id}"
            ));
        }
        if !mobile_ids.insert(*id) {
            return invalid_configuration(&format!(
                "Properties/UsedMobileApplicationFunctionalities duplicates ID {id}"
            ));
        }
    }
    Ok(())
}

fn validate_configuration_text(path: &str, value: &str) -> Result<(), SpecialEntryBuildError> {
    if value.len() > MAX_CANONICAL_TEXT_BYTES {
        invalid_configuration(&format!(
            "{path} has {} UTF-8 bytes, exceeding {MAX_CANONICAL_TEXT_BYTES}",
            value.len()
        ))
    } else {
        Ok(())
    }
}

fn validate_localized_strings(
    path: &str,
    values: &[ConfigurationLocalizedString],
) -> Result<(), SpecialEntryBuildError> {
    let mut languages = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if value.language.is_empty() || value.language.len() > MAX_CONFIGURATION_LANGUAGE_BYTES {
            return invalid_configuration(&format!(
                "{path}[{index}]/lang is empty or exceeds {MAX_CONFIGURATION_LANGUAGE_BYTES} bytes"
            ));
        }
        if !languages.insert(value.language.as_str()) {
            return invalid_configuration(&format!(
                "{path}[{index}] duplicates language `{}`",
                value.language
            ));
        }
        validate_configuration_text(&format!("{path}[{index}]/content"), &value.content)?;
    }
    Ok(())
}

fn validate_reference(
    identities: &BootstrapIdentities,
    graph: &BootstrapGraph,
    path: &str,
    reference: Option<ObjectUuid>,
    expected_kind: &str,
) -> Result<(), SpecialEntryBuildError> {
    let Some(reference) = reference else {
        return Ok(());
    };
    let Some(object) = identities.object(reference) else {
        return invalid_configuration(&format!("{path} references unknown object {reference}"));
    };
    if object.kind().as_str() != expected_kind || object.owner().is_some() {
        return invalid_configuration(&format!(
            "{path} references {} object {reference}; expected top-level {expected_kind}",
            object.kind()
        ));
    }
    if graph.primary_object_entry(reference).is_none() {
        return invalid_configuration(&format!(
            "{path} references {reference} without a primary storage route"
        ));
    }
    Ok(())
}

fn invalid_configuration<T>(reason: &str) -> Result<T, SpecialEntryBuildError> {
    Err(SpecialEntryBuildError::InvalidConfigurationModel {
        reason: reason.to_owned(),
    })
}

fn push_configuration_section(
    output: &mut String,
    section: &ConfigurationSectionLayout,
    object_id: &str,
    families: &[Vec<ObjectUuid>],
    properties: &ConfigurationBodyProperties,
) {
    match section.wrapper {
        ConfigurationSectionWrapper::RootProperties => {
            output.push_str("{1,\r\n");
            push_configuration_properties(output, object_id, properties);
            push_configuration_family_tail(output, section, families);
            output.push('}');
        }
        ConfigurationSectionWrapper::Directory => {
            output.push_str("{6,\r\n{1,{{1,0,");
            output.push_str(object_id);
            output.push_str("},");
            output.push_str(NIL_UUID);
            output.push('}');
            push_configuration_family_tail(output, section, families);
            output.push_str("\r\n}}");
        }
        ConfigurationSectionWrapper::ZeroIdentity => {
            output.push_str("{1,{0,{1,0,");
            output.push_str(object_id);
            output.push_str("}}");
            push_configuration_family_tail(output, section, families);
            output.push('}');
        }
        ConfigurationSectionWrapper::DirectIdentity => {
            output.push_str("{1,{{1,0,");
            output.push_str(object_id);
            output.push_str("}}");
            push_configuration_family_tail(output, section, families);
            output.push('}');
        }
    }
}

fn push_configuration_family_tail(
    output: &mut String,
    section: &ConfigurationSectionLayout,
    families: &[Vec<ObjectUuid>],
) {
    debug_assert_eq!(section.slots.len(), families.len());
    write!(output, ",{}", section.slots.len()).expect("writing to String cannot fail");
    for (slot, objects) in section.slots.iter().zip(families) {
        output.push_str(",\r\n{");
        output.push_str(slot.class_id);
        write!(output, ",{}", objects.len()).expect("writing to String cannot fail");
        for object in objects {
            output.push(',');
            output.push_str(&object.to_string());
        }
        output.push('}');
    }
}

fn push_configuration_properties(
    output: &mut String,
    object_id: &str,
    properties: &ConfigurationBodyProperties,
) {
    let mut fields = Vec::with_capacity(61);
    fields.push("68".to_owned());
    fields.push(format!(
        "{{0,{}}}",
        configuration_header(object_id, properties)
    ));
    fields.push(quoted_1c(&properties.name_prefix));
    fields.push(properties.default_run_mode.native_code().to_owned());
    fields.push(localized_1c(&properties.brief_information));
    fields.push(localized_1c(&properties.detailed_information));
    fields.push(localized_1c(&properties.copyright));
    fields.push(localized_1c(&properties.vendor_information_address));
    fields.push(localized_1c(&properties.configuration_information_address));
    fields.push(uuid_or_nil(properties.default_style));
    fields.push(uuid_or_nil(properties.default_language));
    fields.push(NIL_UUID.to_owned());
    fields.push(NIL_UUID.to_owned());
    fields.push(properties.script_variant.native_code().to_owned());
    fields.push(quoted_1c(&properties.vendor));
    fields.push(quoted_1c(&properties.version));
    fields.push(quoted_1c(&properties.update_catalog_address));
    fields.push("1".to_owned());
    fields.push("{0,0}".to_owned());
    fields.push("1".to_owned());
    fields.push("{0,0}".to_owned());
    fields.push("1".to_owned());
    fields.extend(
        properties
            .settings_storages
            .iter()
            .map(|value| uuid_or_nil(*value)),
    );
    fields.push(properties.extension_compatibility_mode.to_string());
    fields.push("{0,0}".to_owned());
    fields.push("0".to_owned());
    fields.push("0".to_owned());
    fields.push(NIL_UUID.to_owned());
    fields.push(NIL_UUID.to_owned());
    fields.push(NIL_UUID.to_owned());
    fields.push(configuration_use_purpose(
        properties.use_platform_application,
    ));
    fields.push(NIL_UUID.to_owned());
    fields.push(NIL_UUID.to_owned());
    fields.push("1".to_owned());
    fields.push(NIL_UUID.to_owned());
    fields.push("2".to_owned());
    fields.push(configuration_default_roles(&properties.default_roles));
    fields.push(configuration_compatibility_options());
    fields.push("0".to_owned());
    fields.push(quoted_1c(""));
    fields.push(properties.compatibility_mode.to_string());
    fields.push("1".to_owned());
    fields.push("0".to_owned());
    fields.push(NIL_UUID.to_owned());
    fields.push(NIL_UUID.to_owned());
    fields.push(NIL_UUID.to_owned());
    fields.push("1".to_owned());
    fields.push(NIL_UUID.to_owned());
    fields.push(configuration_client_capabilities());
    fields.push("{0,0}".to_owned());
    fields.push(configuration_mobile_capabilities(
        &properties.enabled_mobile_functionalities,
    ));
    fields.push("{0}".to_owned());
    fields.push(NIL_UUID.to_owned());
    fields.push(quoted_1c(""));
    fields.push("0".to_owned());
    fields.push("1".to_owned());
    fields.push("{0}".to_owned());
    fields.push("1".to_owned());
    debug_assert_eq!(fields.len(), 61);

    output.push('{');
    output.push_str(&fields.join(",\r\n"));
    output.push('}');
}

fn configuration_header(object_id: &str, properties: &ConfigurationBodyProperties) -> String {
    format!(
        "{{3,{{1,0,{object_id}}},{},{},{},0,0,{NIL_UUID},0}}",
        quoted_1c(&properties.name),
        localized_1c(&properties.synonyms),
        quoted_1c(&properties.comment),
    )
}

fn configuration_use_purpose(enabled: bool) -> String {
    if enabled {
        format!("{{1,{{\"#\",{USE_PURPOSE_CLASS_UUID},1}}}}")
    } else {
        "{0}".to_owned()
    }
}

fn configuration_default_roles(roles: &[ObjectUuid]) -> String {
    let mut output = format!("{{0,{}", roles.len());
    for role in roles {
        write!(
            &mut output,
            ",{{\"#\",{DESIGN_TIME_REFERENCE_CLASS_UUID},{{1,{role}}}}}"
        )
        .expect("writing to String cannot fail");
    }
    output.push('}');
    output
}

fn configuration_compatibility_options() -> String {
    const VALUES: [Option<u32>; 29] = [
        Some(28),
        None,
        Some(25),
        Some(30),
        Some(24),
        Some(22),
        Some(21),
        Some(19),
        Some(18),
        Some(17),
        Some(13),
        Some(12),
        Some(27),
        Some(10),
        Some(31),
        Some(26),
        Some(9),
        Some(8),
        Some(20),
        Some(7),
        Some(29),
        Some(16),
        Some(6),
        Some(5),
        Some(3),
        Some(2),
        Some(11),
        Some(23),
        Some(1),
    ];
    let mut output = format!("{{{}", VALUES.len());
    for value in VALUES {
        match value {
            Some(value) => write!(
                &mut output,
                ",{{{{\"#\",{COMPATIBILITY_OPTION_CLASS_UUID},{value}}},{{\"#\",0}}}}"
            )
            .expect("writing to String cannot fail"),
            None => output.push_str(",{{\"#\"},{\"#\",0}}"),
        }
    }
    output.push('}');
    output
}

fn configuration_client_capabilities() -> String {
    const IDS: [u32; 35] = [
        1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28,
        29, 30, 31, 32, 33, 34, 35, 36, 37, 38,
    ];
    let mut output = format!("{{2,{}", IDS.len());
    for id in IDS {
        write!(&mut output, ",{{{id},0,0}}").expect("writing to String cannot fail");
    }
    output.push('}');
    output
}

fn configuration_mobile_capabilities(enabled: &[u32]) -> String {
    let enabled = enabled.iter().copied().collect::<BTreeSet<_>>();
    let ids = mobile_functionality_ids().collect::<Vec<_>>();
    let mut output = format!("{{2,{}", ids.len());
    for id in ids {
        write!(
            &mut output,
            ",{{{id},{}}}",
            if enabled.contains(&id) { 1 } else { 0 }
        )
        .expect("writing to String cannot fail");
    }
    output.push_str(",0}");
    output
}

fn mobile_functionality_ids() -> impl Iterator<Item = u32> {
    (0..=27).chain(32..=41)
}

fn localized_1c(values: &[ConfigurationLocalizedString]) -> String {
    let mut output = format!("{{{}", values.len());
    for value in values {
        output.push(',');
        output.push_str(&quoted_1c(&value.language));
        output.push(',');
        output.push_str(&quoted_1c(&value.content));
    }
    output.push('}');
    output
}

fn quoted_1c(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for character in value.chars() {
        if character == '"' {
            output.push('"');
        }
        output.push(character);
    }
    output.push('"');
    output
}

fn uuid_or_nil(value: Option<ObjectUuid>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| NIL_UUID.to_owned())
}

pub(crate) fn ensure_graph_profile(
    graph: &BootstrapGraph,
    profile: &SpecialEntryProfile,
) -> Result<(), SpecialEntryBuildError> {
    if graph.profile_id() == profile.profile_id() {
        Ok(())
    } else {
        Err(SpecialEntryBuildError::ProfileMismatch {
            graph: graph.profile_id().clone(),
            special: profile.profile_id().clone(),
        })
    }
}

pub(crate) fn compiled_special_entry(
    kind: SpecialEntryKind,
    plaintext: &[u8],
    profile: &SpecialEntryProfile,
) -> Result<StoragePatchEntry, SpecialEntryBuildError> {
    let bytes = raw_deflate(plaintext)?;
    Ok(StoragePatchEntry::new(
        StoragePatchTarget::new(
            StorageKey::new(kind.key())?,
            MultipartIdentity::single(),
            special_provenance(profile, kind.key())?,
        ),
        StoragePatchOutcome::compiled(bytes)?,
    ))
}

pub(crate) fn special_provenance(
    profile: &SpecialEntryProfile,
    role: &str,
) -> Result<StorageProvenance, StorageBuildError> {
    StorageProvenance::new(&format!(
        "bootstrap:{}:{role}",
        profile.profile_id().as_str()
    ))
}

fn raw_deflate(plaintext: &[u8]) -> Result<Vec<u8>, SpecialEntryBuildError> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(plaintext)
        .map_err(SpecialEntryBuildError::Deflate)?;
    encoder.finish().map_err(SpecialEntryBuildError::Deflate)
}

#[cfg(test)]
pub(crate) fn inflate_for_test(bytes: &[u8]) -> Vec<u8> {
    use std::io::Read;

    let mut decoder = flate2::read::DeflateDecoder::new(bytes);
    let mut plaintext = Vec::new();
    decoder.read_to_end(&mut plaintext).unwrap();
    plaintext
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use ibcmd_core::artifact::ProfileId;
    use ibcmd_core::diagnostic::{ObjectPath, PathSegment, PropertyPath};
    use ibcmd_core::identity::{LogicalIdentity, ObjectUuid};
    use ibcmd_core::model::{
        CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, MetadataKind,
    };
    use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
    use ibcmd_core::validate::validate_configuration;

    use crate::compiler::graph::{ObjectStorageRoute, build_bootstrap_graph};
    use crate::compiler::identity::{BootstrapIdentities, collect_bootstrap_identities};

    use super::*;

    const CONFIGURATION_UUID: &str = "10000000-0000-4000-8000-000000000001";
    const ALL_ROOT_FAMILIES: [&str; 47] = [
        "Role",
        "CommonTemplate",
        "CommonModule",
        "HTTPService",
        "ScheduledJob",
        "CommonAttribute",
        "SessionParameter",
        "FunctionalOptionsParameter",
        "Subsystem",
        "Interface",
        "Style",
        "FilterCriterion",
        "SettingsStorage",
        "EventSubscription",
        "StyleItem",
        "Bot",
        "CommonPicture",
        "ExchangePlan",
        "WebService",
        "Language",
        "FunctionalOption",
        "DefinedType",
        "XDTOPackage",
        "WSReference",
        "Constant",
        "Document",
        "CommonForm",
        "InformationRegister",
        "CommandGroup",
        "CommonCommand",
        "DocumentNumerator",
        "DocumentJournal",
        "Report",
        "ChartOfCharacteristicTypes",
        "AccumulationRegister",
        "Sequence",
        "DataProcessor",
        "Catalog",
        "Enum",
        "ChartOfAccounts",
        "AccountingRegister",
        "ChartOfCalculationTypes",
        "CalculationRegister",
        "Task",
        "BusinessProcess",
        "ExternalDataSource",
        "IntegrationService",
    ];

    fn uuid(index: usize) -> ObjectUuid {
        ObjectUuid::parse(&format!("10000000-0000-4000-8000-{index:012x}")).unwrap()
    }

    fn object(index: usize, kind: &str) -> CanonicalObject {
        let path = ObjectPath::new(vec![
            PathSegment::name(&format!("object-{index}"))
                .expect("clean-room object path must be valid"),
        ])
        .unwrap();
        let provenance = SourceProvenance::new(
            ProfileId::parse("profile:test").unwrap(),
            CanonicalAnchor::new(path.clone(), PropertyPath::root()),
        );
        CanonicalObject::new(CanonicalObjectParts::new(
            LogicalIdentity::new(uuid(index), path),
            MetadataKind::new(kind).unwrap(),
            provenance,
        ))
        .unwrap()
    }

    fn fixture(kinds: &[&str]) -> (BootstrapIdentities, BootstrapGraph) {
        let mut objects = vec![object(1, "Configuration")];
        objects.extend(
            kinds
                .iter()
                .enumerate()
                .map(|(index, kind)| object(index + 2, kind)),
        );
        let configuration = CanonicalConfiguration::new(objects).unwrap();
        let validated = validate_configuration(&configuration).unwrap();
        let identities = collect_bootstrap_identities(&validated).unwrap();
        let routes = identities
            .objects()
            .iter()
            .filter(|object| object.owner().is_none())
            .map(|object| ObjectStorageRoute::new(object.uuid(), Vec::new()).unwrap())
            .collect();
        let graph = build_bootstrap_graph(
            &identities,
            ProfileId::parse("platform-8.3.27.1989").unwrap(),
            routes,
        )
        .unwrap();
        (identities, graph)
    }

    fn graph() -> BootstrapGraph {
        fixture(&[]).1
    }

    fn profile() -> SpecialEntryProfile {
        SpecialEntryProfile::fixture("platform-8.3.27.1989", 80_327)
    }

    #[test]
    fn root_plaintext_matches_exact_native_golden_and_is_deterministic() {
        let graph = graph();
        let first = compile_root(&graph, &profile()).unwrap();
        let second = compile_root(&graph, &profile()).unwrap();
        assert_eq!(first, second);
        let payload = first.outcome().compiled_payload().unwrap();
        assert_eq!(
            inflate_for_test(payload.bytes()),
            format!("\u{feff}{{2,{CONFIGURATION_UUID},}}").as_bytes()
        );
    }

    #[test]
    fn configuration_body_is_complete_deterministic_and_inventory_exact() {
        let (identities, graph) = fixture(&ALL_ROOT_FAMILIES);
        let by_kind = |kind: &str| {
            identities
                .objects()
                .iter()
                .find(|object| object.kind().as_str() == kind)
                .unwrap()
                .uuid()
        };
        let mut properties = ConfigurationBodyProperties::minimal("Demo \"Configuration\"", 80_327);
        properties.synonyms = vec![ConfigurationLocalizedString::new("en", "Demo")];
        properties.comment = "offline fixture".to_owned();
        properties.default_style = Some(by_kind("Style"));
        properties.default_language = Some(by_kind("Language"));
        properties.settings_storages[0] = Some(by_kind("SettingsStorage"));
        properties.default_roles = vec![by_kind("Role")];
        properties.enabled_mobile_functionalities = vec![0, 41];

        let first =
            compile_configuration_body(&identities, &graph, &profile(), &properties).unwrap();
        let second =
            compile_configuration_body(&identities, &graph, &profile(), &properties).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.target().key().as_str(), CONFIGURATION_UUID);
        let plain = inflate_for_test(first.outcome().compiled_payload().unwrap().bytes());
        let text = std::str::from_utf8(&plain).unwrap();
        let root = braced_fields(text.trim_start_matches('\u{feff}'));
        assert_eq!(root.len(), 11);
        assert_eq!(root[0], "2");
        assert_eq!(braced_fields(root[1]), [CONFIGURATION_UUID]);
        assert_eq!(root[2], "7");
        assert_eq!(root[10], "{{0,\"\",\"\"}}");
        assert!(text.contains("\"Demo \"\"Configuration\"\"\""));

        let expected_objects = identities
            .objects()
            .iter()
            .filter(|object| object.uuid() != identities.configuration_uuid())
            .map(|object| object.uuid().to_string())
            .collect::<BTreeSet<_>>();
        let mut actual_objects = BTreeSet::new();
        let mut contained_ids = BTreeSet::new();
        let mut slot_count = 0;
        for (index, section) in CONFIGURATION_SECTIONS.iter().enumerate() {
            let contained = braced_fields(root[index + 3]);
            assert_eq!(contained.len(), 2);
            assert_eq!(contained[0], section.class_id);
            let payload = braced_fields(contained[1]);
            let directory = matches!(section.wrapper, ConfigurationSectionWrapper::Directory)
                .then(|| braced_fields(payload[1]));
            let (prefix, family_fields) = match section.wrapper {
                ConfigurationSectionWrapper::RootProperties => {
                    let properties = braced_fields(payload[1]);
                    assert_eq!(properties.len(), 61);
                    assert_eq!(properties[0], "68");
                    assert_eq!(properties[60], "1");
                    (&payload[..2], &payload[3..])
                }
                ConfigurationSectionWrapper::Directory => {
                    assert_eq!(payload[0], "6");
                    let nested = directory.as_ref().unwrap();
                    (&nested[..2], &nested[3..])
                }
                ConfigurationSectionWrapper::ZeroIdentity
                | ConfigurationSectionWrapper::DirectIdentity => (&payload[..2], &payload[3..]),
            };
            let prefix_text = prefix.join(",");
            let marker = "{1,0,";
            let marker_start = prefix_text.find(marker).unwrap() + marker.len();
            let object_id = &prefix_text[marker_start..marker_start + 36];
            let object_id = ObjectUuid::parse(object_id).unwrap();
            assert_eq!(object_id.as_bytes()[6] >> 4, 8);
            assert!(contained_ids.insert(object_id));
            assert_eq!(family_fields.len(), section.slots.len());
            slot_count += family_fields.len();
            for (slot, family) in section.slots.iter().zip(family_fields) {
                let fields = braced_fields(family);
                assert_eq!(fields[0], slot.class_id);
                let count = fields[1].parse::<usize>().unwrap();
                assert_eq!(fields.len(), count + 2);
                for object in &fields[2..] {
                    assert!(actual_objects.insert((*object).to_owned()));
                }
            }
        }
        assert_eq!(contained_ids.len(), 7);
        assert_eq!(slot_count, 48);
        assert_eq!(actual_objects, expected_objects);
    }

    #[test]
    fn configuration_body_fails_closed_for_unknown_families_and_wrong_refs() {
        let (identities, graph) = fixture(&["FutureFamily"]);
        let properties = ConfigurationBodyProperties::minimal("Demo", 80_327);
        assert!(matches!(
            compile_configuration_body(&identities, &graph, &profile(), &properties),
            Err(SpecialEntryBuildError::UnsupportedConfigurationFamily { kind, .. })
                if kind == "FutureFamily"
        ));

        let (identities, graph) = fixture(&["Language", "Style"]);
        let language = identities
            .objects()
            .iter()
            .find(|object| object.kind().as_str() == "Language")
            .unwrap()
            .uuid();
        let mut properties = ConfigurationBodyProperties::minimal("Demo", 80_327);
        properties.default_style = Some(language);
        assert!(matches!(
            compile_configuration_body(&identities, &graph, &profile(), &properties),
            Err(SpecialEntryBuildError::InvalidConfigurationModel { reason })
                if reason.contains("DefaultStyle") && reason.contains("expected top-level Style")
        ));
    }

    #[test]
    fn graph_and_special_layout_profiles_cannot_be_mixed() {
        let error = compile_root(
            &graph(),
            &SpecialEntryProfile::fixture("platform-other", 80_327),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            SpecialEntryBuildError::ProfileMismatch { graph, special }
                if graph.as_str() == "platform-8.3.27.1989"
                    && special.as_str() == "platform-other"
        ));
    }

    fn braced_fields(value: &str) -> Vec<&str> {
        let value = value.trim();
        let bytes = value.as_bytes();
        assert_eq!(bytes.first(), Some(&b'{'));
        let mut fields = Vec::new();
        let mut depth = 1_usize;
        let mut quoted = false;
        let mut start = 1_usize;
        let mut index = 1_usize;
        while index < bytes.len() {
            match bytes[index] {
                b'"' if quoted && bytes.get(index + 1) == Some(&b'"') => index += 2,
                b'"' => {
                    quoted = !quoted;
                    index += 1;
                }
                b'{' if !quoted => {
                    depth += 1;
                    index += 1;
                }
                b'}' if !quoted => {
                    depth -= 1;
                    if depth == 0 {
                        fields.push(value[start..index].trim());
                        assert!(value[index + 1..].trim().is_empty());
                        return fields;
                    }
                    index += 1;
                }
                b',' if !quoted && depth == 1 => {
                    fields.push(value[start..index].trim());
                    start = index + 1;
                    index += 1;
                }
                _ => index += 1,
            }
        }
        panic!("unterminated braced value")
    }
}
