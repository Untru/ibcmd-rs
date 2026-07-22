//! Profile-gated source-asset routes and standalone body codecs.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};
use std::str;

use ibcmd_core::artifact::{ProfileId, StorageProfileId};
use ibcmd_core::asset::{Asset, AssetBuildError, MediaKind};
use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::profile::EffectiveProfile;
use ibcmd_core::storage::{
    MultipartIdentity, StorageBuildError, StoragePatchBuildError, StoragePatchEntry,
    StoragePatchOutcome, StoragePatchTarget, StorageProvenance,
};
use ibcmd_core::version::PlatformBuild;

use super::super::CompileAxes;
use super::super::graph::{BootstrapGraph, StorageSuffix};
use super::native::{
    NativeValue, deflate_bytes, exact_list, exact_token, inflate, inline_list, parse_without_bom,
    required_list, required_text, required_token, serialize_without_bom, text, token,
};
use crate::module_blob::{pack_module_blob_bytes_base_free, unpack_module_container_text};

const SUPPORTED_STORAGE_PROFILE: &str = "storage:mssql-config-configsave";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceAssetCodec {
    Module,
    RawBinary,
    Picture,
    Help,
    Deferred,
}

impl SourceAssetCodec {
    const fn layout_key(self) -> Option<&'static str> {
        match self {
            Self::Module => Some("bootstrap.asset.module.layout"),
            Self::RawBinary => Some("bootstrap.asset.raw_binary.layout"),
            Self::Picture => Some("bootstrap.asset.picture.layout"),
            Self::Help => Some("bootstrap.asset.help.layout"),
            Self::Deferred => None,
        }
    }

    const fn layout(self) -> Option<&'static str> {
        match self {
            Self::Module => Some("module-v8-container-v1"),
            Self::RawBinary => Some("raw-deflate-v1"),
            Self::Picture => Some("ext-picture-v1"),
            Self::Help => Some("help-v1"),
            Self::Deferred => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceAssetRole {
    Module,
    OrdinaryApplicationModule,
    ExternalConnectionModule,
    ManagedApplicationModule,
    SessionModule,
    CommandModule,
    FormModule,
    ObjectModule,
    ManagerModule,
    ValueManagerModule,
    RecordSetModule,
    Picture,
    Help,
    Package,
    CommandInterface,
    Splash,
    ParentConfigurations,
    HomePageWorkArea,
    MainSectionCommandInterface,
    MobileClientSignature,
    ClientApplicationInterface,
    MainSectionPicture,
    StandaloneContent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceAssetRoute {
    owner_family: &'static str,
    role: SourceAssetRole,
    suffix: &'static str,
    relative_path: &'static str,
    codec: SourceAssetCodec,
}

impl SourceAssetRoute {
    pub const fn owner_family(self) -> &'static str {
        self.owner_family
    }

    pub const fn role(self) -> SourceAssetRole {
        self.role
    }

    pub const fn suffix(self) -> &'static str {
        self.suffix
    }

    pub const fn relative_path(self) -> &'static str {
        self.relative_path
    }

    pub const fn codec(self) -> SourceAssetCodec {
        self.codec
    }

    pub fn file_name(self) -> &'static str {
        self.relative_path
            .rsplit('/')
            .next()
            .expect("registry paths are non-empty")
    }
}

macro_rules! route {
    ($family:literal, $role:ident, $suffix:literal, $path:literal, $codec:ident) => {
        SourceAssetRoute {
            owner_family: $family,
            role: SourceAssetRole::$role,
            suffix: $suffix,
            relative_path: $path,
            codec: SourceAssetCodec::$codec,
        }
    };
}

const ROUTES: &[SourceAssetRoute] = &[
    route!(
        "Configuration",
        OrdinaryApplicationModule,
        ".0",
        "Ext/OrdinaryApplicationModule.bsl",
        Module
    ),
    route!(
        "Configuration",
        ExternalConnectionModule,
        ".5",
        "Ext/ExternalConnectionModule.bsl",
        Module
    ),
    route!(
        "Configuration",
        ManagedApplicationModule,
        ".6",
        "Ext/ManagedApplicationModule.bsl",
        Module
    ),
    route!(
        "Configuration",
        SessionModule,
        ".7",
        "Ext/SessionModule.bsl",
        Module
    ),
    route!("CommonModule", Module, ".0", "Ext/Module.bsl", Module),
    route!("HTTPService", Module, ".0", "Ext/Module.bsl", Module),
    route!("WebService", Module, ".0", "Ext/Module.bsl", Module),
    route!("Bot", Module, ".1", "Ext/Module.bsl", Module),
    route!("IntegrationService", Module, ".0", "Ext/Module.bsl", Module),
    route!(
        "Subsystem",
        CommandInterface,
        ".1",
        "Ext/CommandInterface.xml",
        Deferred
    ),
    route!(
        "CommonCommand",
        CommandInterface,
        ".0",
        "Ext/CommandInterface.xml",
        Deferred
    ),
    route!(
        "CommonCommand",
        CommandModule,
        ".2",
        "Ext/CommandModule.bsl",
        Module
    ),
    route!(
        "Command",
        CommandModule,
        ".2",
        "Ext/CommandModule.bsl",
        Module
    ),
    route!("Form", FormModule, ".0", "Ext/Form/Module.bsl", Deferred),
    route!(
        "CommonForm",
        FormModule,
        ".0",
        "Ext/Form/Module.bsl",
        Deferred
    ),
    route!(
        "FilterCriterion",
        ManagerModule,
        ".0",
        "Ext/ManagerModule.bsl",
        Module
    ),
    route!(
        "Constant",
        ValueManagerModule,
        ".0",
        "Ext/ValueManagerModule.bsl",
        Module
    ),
    route!(
        "Constant",
        ManagerModule,
        ".1",
        "Ext/ManagerModule.bsl",
        Module
    ),
    route!(
        "SettingsStorage",
        ManagerModule,
        ".8",
        "Ext/ManagerModule.bsl",
        Module
    ),
    route!(
        "Sequence",
        RecordSetModule,
        ".0",
        "Ext/RecordSetModule.bsl",
        Module
    ),
    route!(
        "Catalog",
        ObjectModule,
        ".0",
        "Ext/ObjectModule.bsl",
        Module
    ),
    route!(
        "Catalog",
        ManagerModule,
        ".3",
        "Ext/ManagerModule.bsl",
        Module
    ),
    route!("Report", ObjectModule, ".0", "Ext/ObjectModule.bsl", Module),
    route!(
        "Report",
        ManagerModule,
        ".2",
        "Ext/ManagerModule.bsl",
        Module
    ),
    route!(
        "DataProcessor",
        ObjectModule,
        ".0",
        "Ext/ObjectModule.bsl",
        Module
    ),
    route!(
        "DataProcessor",
        ManagerModule,
        ".2",
        "Ext/ManagerModule.bsl",
        Module
    ),
    route!(
        "Document",
        ObjectModule,
        ".0",
        "Ext/ObjectModule.bsl",
        Module
    ),
    route!(
        "Document",
        ManagerModule,
        ".2",
        "Ext/ManagerModule.bsl",
        Module
    ),
    route!("Enum", ManagerModule, ".0", "Ext/ManagerModule.bsl", Module),
    route!(
        "ExchangePlan",
        ObjectModule,
        ".2",
        "Ext/ObjectModule.bsl",
        Module
    ),
    route!(
        "ExchangePlan",
        ManagerModule,
        ".3",
        "Ext/ManagerModule.bsl",
        Module
    ),
    route!(
        "AccountingRegister",
        RecordSetModule,
        ".6",
        "Ext/RecordSetModule.bsl",
        Module
    ),
    route!(
        "AccountingRegister",
        ManagerModule,
        ".7",
        "Ext/ManagerModule.bsl",
        Module
    ),
    route!(
        "AccumulationRegister",
        RecordSetModule,
        ".1",
        "Ext/RecordSetModule.bsl",
        Module
    ),
    route!(
        "AccumulationRegister",
        ManagerModule,
        ".2",
        "Ext/ManagerModule.bsl",
        Module
    ),
    route!(
        "CalculationRegister",
        RecordSetModule,
        ".1",
        "Ext/RecordSetModule.bsl",
        Module
    ),
    route!(
        "CalculationRegister",
        ManagerModule,
        ".2",
        "Ext/ManagerModule.bsl",
        Module
    ),
    route!(
        "InformationRegister",
        RecordSetModule,
        ".1",
        "Ext/RecordSetModule.bsl",
        Module
    ),
    route!(
        "InformationRegister",
        ManagerModule,
        ".2",
        "Ext/ManagerModule.bsl",
        Module
    ),
    route!(
        "DocumentJournal",
        ManagerModule,
        ".1",
        "Ext/ManagerModule.bsl",
        Module
    ),
    route!("Task", ObjectModule, ".6", "Ext/ObjectModule.bsl", Module),
    route!("Task", ManagerModule, ".7", "Ext/ManagerModule.bsl", Module),
    route!(
        "BusinessProcess",
        ObjectModule,
        ".6",
        "Ext/ObjectModule.bsl",
        Module
    ),
    route!(
        "BusinessProcess",
        ManagerModule,
        ".8",
        "Ext/ManagerModule.bsl",
        Module
    ),
    route!(
        "ChartOfAccounts",
        ObjectModule,
        ".14",
        "Ext/ObjectModule.bsl",
        Module
    ),
    route!(
        "ChartOfAccounts",
        ManagerModule,
        ".15",
        "Ext/ManagerModule.bsl",
        Module
    ),
    route!(
        "ChartOfCalculationTypes",
        ObjectModule,
        ".0",
        "Ext/ObjectModule.bsl",
        Module
    ),
    route!(
        "ChartOfCalculationTypes",
        ManagerModule,
        ".3",
        "Ext/ManagerModule.bsl",
        Module
    ),
    route!(
        "ChartOfCharacteristicTypes",
        ObjectModule,
        ".15",
        "Ext/ObjectModule.bsl",
        Module
    ),
    route!(
        "ChartOfCharacteristicTypes",
        ManagerModule,
        ".16",
        "Ext/ManagerModule.bsl",
        Module
    ),
    route!("CommonPicture", Picture, ".0", "Ext/Picture.xml", Picture),
    route!("CommonCommand", Help, ".5", "Ext/Help.xml", Help),
    route!("XDTOPackage", Package, ".0", "Ext/Package.bin", RawBinary),
    route!("Configuration", Splash, ".2", "Ext/Splash.xml", Picture),
    route!("Configuration", Help, ".3", "Ext/Help.xml", Help),
    route!(
        "Configuration",
        ParentConfigurations,
        ".4",
        "Ext/ParentConfigurations.bin",
        RawBinary
    ),
    route!(
        "Configuration",
        HomePageWorkArea,
        ".8",
        "Ext/HomePageWorkArea.xml",
        Deferred
    ),
    route!(
        "Configuration",
        MainSectionCommandInterface,
        ".9",
        "Ext/MainSectionCommandInterface.xml",
        Deferred
    ),
    route!(
        "Configuration",
        CommandInterface,
        ".a",
        "Ext/CommandInterface.xml",
        Deferred
    ),
    route!(
        "Configuration",
        MobileClientSignature,
        ".10",
        "Ext/MobileClientSignature.bin",
        RawBinary
    ),
    route!(
        "Configuration",
        ClientApplicationInterface,
        ".b",
        "Ext/ClientApplicationInterface.xml",
        Deferred
    ),
    route!(
        "Configuration",
        MainSectionPicture,
        ".c",
        "Ext/MainSectionPicture.xml",
        Picture
    ),
    route!(
        "Configuration",
        StandaloneContent,
        ".f",
        "Ext/StandaloneConfigurationContent.bin",
        Deferred
    ),
];

#[derive(Clone, Copy, Debug, Default)]
pub struct SourceAssetRegistry;

impl SourceAssetRegistry {
    pub fn route(
        self,
        owner_family: &str,
        role: SourceAssetRole,
    ) -> Option<&'static SourceAssetRoute> {
        ROUTES
            .iter()
            .find(|route| route.owner_family == owner_family && route.role == role)
    }

    pub fn route_by_suffix(
        self,
        owner_family: &str,
        suffix: &str,
    ) -> Option<&'static SourceAssetRoute> {
        let normalized = if suffix.starts_with('.') {
            suffix.to_owned()
        } else {
            format!(".{suffix}")
        };
        ROUTES
            .iter()
            .find(|route| route.owner_family == owner_family && route.suffix == normalized.as_str())
    }

    pub fn configuration_routes(self) -> impl Iterator<Item = &'static SourceAssetRoute> {
        ROUTES
            .iter()
            .filter(|route| route.owner_family == "Configuration")
    }

    pub fn module_routes(
        self,
        owner_family: &str,
    ) -> impl Iterator<Item = &'static SourceAssetRoute> {
        ROUTES.iter().filter(move |route| {
            route.owner_family == owner_family && route.codec == SourceAssetCodec::Module
        })
    }

    pub fn source_path(
        self,
        metadata_xml: &Path,
        owner_family: &str,
        role: SourceAssetRole,
    ) -> Option<PathBuf> {
        let route = self.route(owner_family, role)?;
        Some(source_owner_directory(metadata_xml, owner_family).join(route.relative_path))
    }

    pub fn source_path_by_suffix(
        self,
        metadata_xml: &Path,
        owner_family: &str,
        suffix: &str,
    ) -> Option<PathBuf> {
        let route = self.route_by_suffix(owner_family, suffix)?;
        Some(source_owner_directory(metadata_xml, owner_family).join(route.relative_path))
    }

    pub fn help_suffix(self, owner_family: &str) -> Option<&'static str> {
        if matches!(owner_family, "Form" | "CommonForm") {
            return Some(".1");
        }
        Some(".5")
    }

    pub const fn help_relative_path(self) -> &'static str {
        "Ext/Help.xml"
    }

    pub fn help_source_path(self, metadata_xml: &Path, owner_family: &str) -> PathBuf {
        source_owner_directory(metadata_xml, owner_family).join(self.help_relative_path())
    }
}

fn source_owner_directory(metadata_xml: &Path, owner_family: &str) -> PathBuf {
    if owner_family == "Configuration" {
        return metadata_xml
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
    }
    metadata_xml.with_extension("")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetCodecProfile {
    profile_id: ProfileId,
    platform_build: PlatformBuild,
    storage_profile: StorageProfileId,
    codec: SourceAssetCodec,
}

impl AssetCodecProfile {
    pub fn from_effective_for_codec(
        profile: &EffectiveProfile,
        codec: SourceAssetCodec,
    ) -> Result<Self, AssetProfileError> {
        let key = codec.layout_key().ok_or(AssetProfileError::DeferredCodec)?;
        let expected = codec.layout().expect("non-deferred codec has a layout");
        let platform_build = profile
            .platform_build
            .as_ref()
            .map(|value| value.value.clone())
            .ok_or_else(|| AssetProfileError::MissingCoordinate {
                profile: profile.id.clone(),
                coordinate: "platform_build",
            })?;
        let storage_profile = profile
            .storage_profile
            .as_ref()
            .map(|value| value.value.clone())
            .ok_or_else(|| AssetProfileError::MissingCoordinate {
                profile: profile.id.clone(),
                coordinate: "storage_profile",
            })?;
        if storage_profile.as_str() != SUPPORTED_STORAGE_PROFILE {
            return Err(AssetProfileError::UnsupportedCoordinate {
                profile: profile.id.clone(),
                coordinate: "storage_profile",
                value: storage_profile.to_string(),
            });
        }
        let value =
            profile
                .constants
                .get(key)
                .ok_or_else(|| AssetProfileError::MissingConstant {
                    profile: profile.id.clone(),
                    key,
                })?;
        if value.value != expected {
            return Err(AssetProfileError::UnsupportedLayout {
                profile: profile.id.clone(),
                codec,
                value: value.value.clone(),
            });
        }
        Ok(Self {
            profile_id: profile.id.clone(),
            platform_build,
            storage_profile,
            codec,
        })
    }

    #[cfg(test)]
    fn fixture(codec: SourceAssetCodec) -> Self {
        Self {
            profile_id: ProfileId::parse("platform-8.3.27.1989").unwrap(),
            platform_build: PlatformBuild::parse("8.3.27.1989").unwrap(),
            storage_profile: StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            codec,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssetProfileError {
    DeferredCodec,
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
        codec: SourceAssetCodec,
        value: String,
    },
}

impl Display for AssetProfileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeferredCodec => {
                formatter.write_str("deferred source asset has no BOOT-009 codec")
            }
            Self::MissingCoordinate {
                profile,
                coordinate,
            } => {
                write!(
                    formatter,
                    "profile `{profile}` has no `{coordinate}` coordinate"
                )
            }
            Self::MissingConstant { profile, key } => {
                write!(formatter, "profile `{profile}` has no `{key}` constant")
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
                codec,
                value,
            } => write!(
                formatter,
                "profile `{profile}` declares unsupported {codec:?} layout `{value}`"
            ),
        }
    }
}

impl Error for AssetProfileError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedAsset {
    name: String,
    asset: Asset,
}

impl NamedAsset {
    pub fn new(name: &str, asset: Asset) -> Result<Self, AssetCodecError> {
        validate_asset_name(name)?;
        Ok(Self {
            name: name.to_owned(),
            asset,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn asset(&self) -> &Asset {
        &self.asset
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HelpAssets {
    pages: Vec<NamedAsset>,
    files: Vec<NamedAsset>,
}

impl HelpAssets {
    pub fn new(pages: Vec<NamedAsset>, files: Vec<NamedAsset>) -> Result<Self, AssetCodecError> {
        if pages.is_empty() {
            return Err(AssetCodecError::InvalidModel(
                "Help must contain at least one page".into(),
            ));
        }
        let mut names = BTreeSet::new();
        for value in pages.iter().chain(&files) {
            if !names.insert(value.name.as_str()) {
                return Err(AssetCodecError::InvalidModel(
                    "Help page/file name is duplicated".into(),
                ));
            }
        }
        Ok(Self { pages, files })
    }

    pub fn pages(&self) -> &[NamedAsset] {
        &self.pages
    }

    pub fn files(&self) -> &[NamedAsset] {
        &self.files
    }
}

pub enum SourceAssetPayload<'a> {
    Module(&'a [u8]),
    Binary(&'a Asset),
    Picture(&'a Asset),
    Help(&'a HelpAssets),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedSourceAsset {
    Module(Vec<u8>),
    Binary(Vec<u8>),
    Picture(Vec<u8>),
    Help(HelpAssets),
}

pub fn compile_source_asset(
    graph: &BootstrapGraph,
    owner_uuid: ObjectUuid,
    route: &SourceAssetRoute,
    payload: SourceAssetPayload<'_>,
    axes: &CompileAxes,
    profile: &AssetCodecProfile,
) -> Result<StoragePatchEntry, AssetCodecError> {
    validate_coordinates(graph, axes, profile)?;
    if route.codec != profile.codec {
        return Err(AssetCodecError::CodecMismatch {
            route: route.codec,
            profile: profile.codec,
        });
    }
    let bytes = match (route.codec, payload) {
        (SourceAssetCodec::Module, SourceAssetPayload::Module(text)) => encode_module(text)?,
        (SourceAssetCodec::RawBinary, SourceAssetPayload::Binary(asset)) => {
            deflate_bytes(asset.bytes()).map_err(native_error)?
        }
        (SourceAssetCodec::Picture, SourceAssetPayload::Picture(asset)) => encode_picture(asset)?,
        (SourceAssetCodec::Help, SourceAssetPayload::Help(help)) => encode_help(help)?,
        (SourceAssetCodec::Deferred, _) => return Err(AssetCodecError::DeferredRoute),
        _ => return Err(AssetCodecError::PayloadMismatch),
    };
    let suffix = StorageSuffix::new(route.suffix).map_err(|error| {
        AssetCodecError::InvalidModel(format!("registry suffix is invalid: {error}"))
    })?;
    let target = graph
        .object_entry(owner_uuid, &suffix)
        .ok_or(AssetCodecError::MissingRoute {
            object: owner_uuid,
            suffix: route.suffix,
        })?;
    let provenance = StorageProvenance::new(&format!(
        "bootstrap:{}:asset:{}:{}",
        profile.profile_id, route.owner_family, route.relative_path
    ))?;
    Ok(StoragePatchEntry::new(
        StoragePatchTarget::new(
            target.key().clone(),
            MultipartIdentity::single(),
            provenance,
        ),
        StoragePatchOutcome::compiled(bytes)?,
    ))
}

pub fn decode_source_asset(
    blob: &[u8],
    route: &SourceAssetRoute,
    profile: &AssetCodecProfile,
) -> Result<DecodedSourceAsset, AssetCodecError> {
    if route.codec != profile.codec {
        return Err(AssetCodecError::CodecMismatch {
            route: route.codec,
            profile: profile.codec,
        });
    }
    match route.codec {
        SourceAssetCodec::Module => decode_module(blob).map(DecodedSourceAsset::Module),
        SourceAssetCodec::RawBinary => inflate(blob)
            .map(DecodedSourceAsset::Binary)
            .map_err(native_error),
        SourceAssetCodec::Picture => decode_picture(blob).map(DecodedSourceAsset::Picture),
        SourceAssetCodec::Help => decode_help(blob).map(DecodedSourceAsset::Help),
        SourceAssetCodec::Deferred => Err(AssetCodecError::DeferredRoute),
    }
}

fn encode_module(text: &[u8]) -> Result<Vec<u8>, AssetCodecError> {
    str::from_utf8(text)
        .map_err(|_| AssetCodecError::InvalidModel("module is not UTF-8".into()))?;
    let packed = pack_module_blob_bytes_base_free(text, None)
        .map_err(|error| AssetCodecError::Module(error.to_string()))?;
    let container = inflate(&packed.blob).map_err(native_error)?;
    deflate_bytes(&container).map_err(native_error)
}

fn decode_module(blob: &[u8]) -> Result<Vec<u8>, AssetCodecError> {
    let container = inflate(blob).map_err(native_error)?;
    unpack_module_container_text(&container)
        .map_err(|error| AssetCodecError::Module(error.to_string()))
}

fn encode_picture(asset: &Asset) -> Result<Vec<u8>, AssetCodecError> {
    let value = inline_list(vec![
        token("1"),
        inline_list(vec![token("0"), token("0"), token("-1"), token("-1")]),
        inline_list(vec![inline_list(vec![base64_token(asset.bytes())])]),
    ]);
    let plain = serialize_without_bom(&value).map_err(native_error)?;
    deflate_bytes(&plain).map_err(native_error)
}

fn decode_picture(blob: &[u8]) -> Result<Vec<u8>, AssetCodecError> {
    let plain = inflate(blob).map_err(native_error)?;
    let value = parse_without_bom(&plain).map_err(native_error)?;
    let root = exact_list(&value, 3, "picture body").map_err(native_error)?;
    exact_token(&root[0], "1", "picture body marker").map_err(native_error)?;
    let dimensions = exact_list(&root[1], 4, "picture dimensions").map_err(native_error)?;
    for (value, expected) in dimensions.iter().zip(["0", "0", "-1", "-1"]) {
        exact_token(value, expected, "picture dimension").map_err(native_error)?;
    }
    let payload = exact_list(&root[2], 1, "picture payload wrapper").map_err(native_error)?;
    let payload = exact_list(&payload[0], 1, "picture payload").map_err(native_error)?;
    decode_base64_token(&payload[0])
}

fn encode_help(help: &HelpAssets) -> Result<Vec<u8>, AssetCodecError> {
    let mut fields = Vec::with_capacity(3 + help.pages.len() * 2 + help.files.len() * 3);
    fields.push(token("5"));
    fields.push(token(help.pages.len().to_string()));
    for page in &help.pages {
        fields.push(text(&page.name));
        fields.push(inline_list(vec![base64_token(page.asset.bytes())]));
    }
    fields.push(token(help.files.len().to_string()));
    for file in &help.files {
        fields.push(text(&file.name));
        fields.push(token("1"));
        fields.push(inline_list(vec![base64_token(file.asset.bytes())]));
    }
    let plain = serialize_without_bom(&inline_list(fields)).map_err(native_error)?;
    deflate_bytes(&plain).map_err(native_error)
}

fn decode_help(blob: &[u8]) -> Result<HelpAssets, AssetCodecError> {
    let plain = inflate(blob).map_err(native_error)?;
    let value = parse_without_bom(&plain).map_err(native_error)?;
    let fields = required_list(&value, "Help body").map_err(native_error)?;
    if fields.len() < 3 {
        return Err(AssetCodecError::InvalidModel(
            "Help body is truncated".into(),
        ));
    }
    exact_token(&fields[0], "5", "Help marker").map_err(native_error)?;
    let page_count = parse_count(&fields[1], "Help page count")?;
    let mut index = 2usize;
    let mut pages = Vec::with_capacity(page_count);
    for _ in 0..page_count {
        let name = required_text(
            fields
                .get(index)
                .ok_or_else(|| AssetCodecError::InvalidModel("Help page name is missing".into()))?,
            "Help page name",
        )
        .map_err(native_error)?;
        index += 1;
        let bytes =
            decode_base64_field(fields.get(index).ok_or_else(|| {
                AssetCodecError::InvalidModel("Help page body is missing".into())
            })?)?;
        index += 1;
        pages.push(NamedAsset::new(
            name,
            Asset::new(bytes, MediaKind::octet_stream())?,
        )?);
    }
    let file_count = parse_count(
        fields
            .get(index)
            .ok_or_else(|| AssetCodecError::InvalidModel("Help file count is missing".into()))?,
        "Help file count",
    )?;
    index += 1;
    let mut files = Vec::with_capacity(file_count);
    for _ in 0..file_count {
        let name = required_text(
            fields
                .get(index)
                .ok_or_else(|| AssetCodecError::InvalidModel("Help file name is missing".into()))?,
            "Help file name",
        )
        .map_err(native_error)?;
        index += 1;
        exact_token(
            fields.get(index).ok_or_else(|| {
                AssetCodecError::InvalidModel("Help file marker is missing".into())
            })?,
            "1",
            "Help file marker",
        )
        .map_err(native_error)?;
        index += 1;
        let bytes =
            decode_base64_field(fields.get(index).ok_or_else(|| {
                AssetCodecError::InvalidModel("Help file body is missing".into())
            })?)?;
        index += 1;
        files.push(NamedAsset::new(
            name,
            Asset::new(bytes, MediaKind::octet_stream())?,
        )?);
    }
    if index != fields.len() {
        return Err(AssetCodecError::InvalidModel(
            "Help body has trailing fields".into(),
        ));
    }
    HelpAssets::new(pages, files)
}

fn decode_base64_field(value: &NativeValue) -> Result<Vec<u8>, AssetCodecError> {
    let fields = exact_list(value, 1, "base64 field").map_err(native_error)?;
    decode_base64_token(&fields[0])
}

fn base64_token(bytes: &[u8]) -> NativeValue {
    token(format!("#base64:{}", encode_base64(bytes)))
}

fn decode_base64_token(value: &NativeValue) -> Result<Vec<u8>, AssetCodecError> {
    let value = required_token(value, "base64 token").map_err(native_error)?;
    let payload = value
        .strip_prefix("#base64:")
        .ok_or_else(|| AssetCodecError::InvalidModel("base64 field has no marker".into()))?;
    decode_base64(payload)
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = chunk.get(1).copied().unwrap_or(0);
        let c = chunk.get(2).copied().unwrap_or(0);
        output.push(TABLE[(a >> 2) as usize] as char);
        output.push(TABLE[(((a & 0x03) << 4) | (b >> 4)) as usize] as char);
        if chunk.len() > 1 {
            output.push(TABLE[(((b & 0x0f) << 2) | (c >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(TABLE[(c & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

fn decode_base64(value: &str) -> Result<Vec<u8>, AssetCodecError> {
    if !value.len().is_multiple_of(4) || !value.is_ascii() {
        return Err(AssetCodecError::InvalidModel(
            "base64 length is not canonical".into(),
        ));
    }
    let mut output = Vec::with_capacity(value.len() / 4 * 3);
    let chunks = value.as_bytes().chunks_exact(4);
    let chunk_count = chunks.len();
    for (index, chunk) in chunks.enumerate() {
        let last = index + 1 == chunk_count;
        let a = base64_value(chunk[0])?;
        let b = base64_value(chunk[1])?;
        let c_padding = chunk[2] == b'=';
        let d_padding = chunk[3] == b'=';
        if c_padding && !d_padding || d_padding && !last {
            return Err(AssetCodecError::InvalidModel(
                "base64 padding is not canonical".into(),
            ));
        }
        let c = if c_padding {
            0
        } else {
            base64_value(chunk[2])?
        };
        let d = if d_padding {
            0
        } else {
            base64_value(chunk[3])?
        };
        if c_padding && (b & 0x0f) != 0 || d_padding && !c_padding && (c & 0x03) != 0 {
            return Err(AssetCodecError::InvalidModel(
                "base64 padding bits are non-zero".into(),
            ));
        }
        output.push((a << 2) | (b >> 4));
        if !c_padding {
            output.push((b << 4) | (c >> 2));
        }
        if !d_padding {
            output.push((c << 6) | d);
        }
    }
    Ok(output)
}

fn base64_value(byte: u8) -> Result<u8, AssetCodecError> {
    match byte {
        b'A'..=b'Z' => Ok(byte - b'A'),
        b'a'..=b'z' => Ok(byte - b'a' + 26),
        b'0'..=b'9' => Ok(byte - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err(AssetCodecError::InvalidModel(
            "base64 alphabet is invalid".into(),
        )),
    }
}

fn parse_count(value: &NativeValue, field: &'static str) -> Result<usize, AssetCodecError> {
    required_token(value, field)
        .map_err(native_error)?
        .parse::<usize>()
        .map_err(|_| AssetCodecError::InvalidModel(format!("{field} is not usize")))
}

fn validate_asset_name(name: &str) -> Result<(), AssetCodecError> {
    if name.is_empty()
        || name.len() > 1_024
        || name.chars().any(char::is_control)
        || name.contains('/')
        || name.contains('\\')
        || matches!(name, "." | "..")
    {
        return Err(AssetCodecError::InvalidModel(
            "asset name is unsafe or unbounded".into(),
        ));
    }
    Ok(())
}

fn validate_coordinates(
    graph: &BootstrapGraph,
    axes: &CompileAxes,
    profile: &AssetCodecProfile,
) -> Result<(), AssetCodecError> {
    if graph.profile_id() != &profile.profile_id {
        return Err(AssetCodecError::ProfileMismatch {
            graph: graph.profile_id().clone(),
            codec: profile.profile_id.clone(),
        });
    }
    if axes.platform_build() != Some(&profile.platform_build) {
        return Err(AssetCodecError::AxisMismatch("platform_build"));
    }
    if axes.storage_profile() != &profile.storage_profile {
        return Err(AssetCodecError::AxisMismatch("storage_profile"));
    }
    if axes.compatibility_mode().is_some() || axes.container_revision().is_some() {
        return Err(AssetCodecError::AxisMismatch(
            "unevidenced optional coordinate",
        ));
    }
    Ok(())
}

fn native_error(error: impl Display) -> AssetCodecError {
    AssetCodecError::Native(error.to_string())
}

#[derive(Debug)]
pub enum AssetCodecError {
    Profile(AssetProfileError),
    ProfileMismatch {
        graph: ProfileId,
        codec: ProfileId,
    },
    AxisMismatch(&'static str),
    CodecMismatch {
        route: SourceAssetCodec,
        profile: SourceAssetCodec,
    },
    PayloadMismatch,
    DeferredRoute,
    MissingRoute {
        object: ObjectUuid,
        suffix: &'static str,
    },
    InvalidModel(String),
    Native(String),
    Module(String),
    Asset(AssetBuildError),
    Storage(StorageBuildError),
    Patch(StoragePatchBuildError),
}

impl Display for AssetCodecError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(source) => Display::fmt(source, formatter),
            Self::ProfileMismatch { graph, codec } => {
                write!(
                    formatter,
                    "graph profile `{graph}` differs from asset codec `{codec}`"
                )
            }
            Self::AxisMismatch(axis) => write!(formatter, "source asset `{axis}` axis mismatch"),
            Self::CodecMismatch { route, profile } => {
                write!(
                    formatter,
                    "route codec {route:?} differs from profile codec {profile:?}"
                )
            }
            Self::PayloadMismatch => formatter.write_str("payload kind differs from route codec"),
            Self::DeferredRoute => {
                formatter.write_str("source-asset route is deferred to a later body codec")
            }
            Self::MissingRoute { object, suffix } => {
                write!(
                    formatter,
                    "bootstrap graph has no {object}{suffix} asset route"
                )
            }
            Self::InvalidModel(reason) => write!(formatter, "invalid source asset: {reason}"),
            Self::Native(reason) => {
                write!(formatter, "source asset payload codec failed: {reason}")
            }
            Self::Module(reason) => write!(formatter, "module container codec failed: {reason}"),
            Self::Asset(source) => Display::fmt(source, formatter),
            Self::Storage(source) => Display::fmt(source, formatter),
            Self::Patch(source) => Display::fmt(source, formatter),
        }
    }
}

impl Error for AssetCodecError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Profile(source) => Some(source),
            Self::Asset(source) => Some(source),
            Self::Storage(source) => Some(source),
            Self::Patch(source) => Some(source),
            _ => None,
        }
    }
}

impl From<AssetProfileError> for AssetCodecError {
    fn from(source: AssetProfileError) -> Self {
        Self::Profile(source)
    }
}

impl From<AssetBuildError> for AssetCodecError {
    fn from(source: AssetBuildError) -> Self {
        Self::Asset(source)
    }
}

impl From<StorageBuildError> for AssetCodecError {
    fn from(source: StorageBuildError) -> Self {
        Self::Storage(source)
    }
}

impl From<StoragePatchBuildError> for AssetCodecError {
    fn from(source: StoragePatchBuildError) -> Self {
        Self::Patch(source)
    }
}

#[cfg(test)]
mod tests {
    use ibcmd_core::diagnostic::{ObjectPath, PathSegment, PropertyPath};
    use ibcmd_core::identity::LogicalIdentity;
    use ibcmd_core::model::{
        CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, MetadataKind,
    };
    use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
    use ibcmd_core::validate::validate_configuration;
    use ibcmd_core::version::XmlDialect;

    use crate::compiler::graph::{ObjectStorageRoute, build_bootstrap_graph};
    use crate::compiler::identity::collect_bootstrap_identities;

    use super::*;

    fn object(uuid: &str, kind: &str, name: &str) -> CanonicalObject {
        let path = ObjectPath::new(vec![PathSegment::name(name).unwrap()]).unwrap();
        let provenance = SourceProvenance::new(
            ProfileId::parse("xml-2.21").unwrap(),
            CanonicalAnchor::new(path.clone(), PropertyPath::root()),
        );
        CanonicalObject::new(CanonicalObjectParts::new(
            LogicalIdentity::new(ObjectUuid::parse(uuid).unwrap(), path),
            MetadataKind::new(kind).unwrap(),
            provenance,
        ))
        .unwrap()
    }

    #[test]
    fn registry_paths_and_family_suffixes_are_unique_and_safe() {
        let mut keys = BTreeSet::new();
        for route in ROUTES {
            assert!(keys.insert((route.owner_family, route.suffix)));
            assert!(route.relative_path.starts_with("Ext/"));
            assert!(!route.relative_path.contains(".."));
            assert!(!route.relative_path.contains('\\'));
            StorageSuffix::new(route.suffix).unwrap();
        }
        assert_eq!(
            SourceAssetRegistry
                .route("CommonModule", SourceAssetRole::Module)
                .unwrap()
                .suffix(),
            ".0"
        );
        assert_eq!(
            SourceAssetRegistry
                .route_by_suffix("CommonCommand", "2")
                .unwrap()
                .file_name(),
            "CommandModule.bsl"
        );
        assert_eq!(
            SourceAssetRegistry
                .source_path(
                    Path::new(r"CommonPictures\Logo.xml"),
                    "CommonPicture",
                    SourceAssetRole::Picture,
                )
                .unwrap(),
            PathBuf::from(r"CommonPictures\Logo\Ext\Picture.xml")
        );
        assert_eq!(
            SourceAssetRegistry
                .source_path_by_suffix(Path::new("Configuration.xml"), "Configuration", "6")
                .unwrap(),
            PathBuf::from(r"Ext\ManagedApplicationModule.bsl")
        );
        assert_eq!(
            SourceAssetRegistry
                .module_routes("AccountingRegister")
                .map(|route| route.suffix())
                .collect::<Vec<_>>(),
            vec![".6", ".7"]
        );
    }

    #[test]
    fn module_picture_binary_and_help_roundtrip_exact_content_hashes() {
        let module_route = SourceAssetRegistry
            .route("CommonModule", SourceAssetRole::Module)
            .unwrap();
        let module_profile = AssetCodecProfile::fixture(SourceAssetCodec::Module);
        let module = "Процедура Тест()\r\nКонецПроцедуры\r\n".as_bytes();
        let encoded = encode_module(module).unwrap();
        assert_eq!(
            decode_source_asset(&encoded, module_route, &module_profile).unwrap(),
            DecodedSourceAsset::Module(module.to_vec())
        );

        let picture_route = SourceAssetRegistry
            .route("CommonPicture", SourceAssetRole::Picture)
            .unwrap();
        let picture_profile = AssetCodecProfile::fixture(SourceAssetCodec::Picture);
        let picture = Asset::from_bytes(b"PK\x03\x04picture".to_vec(), "application/zip").unwrap();
        let encoded = encode_picture(&picture).unwrap();
        assert_eq!(
            decode_source_asset(&encoded, picture_route, &picture_profile).unwrap(),
            DecodedSourceAsset::Picture(picture.bytes().to_vec())
        );

        let binary_route = SourceAssetRegistry
            .route("XDTOPackage", SourceAssetRole::Package)
            .unwrap();
        let binary_profile = AssetCodecProfile::fixture(SourceAssetCodec::RawBinary);
        let encoded = deflate_bytes(picture.bytes()).unwrap();
        assert_eq!(
            decode_source_asset(&encoded, binary_route, &binary_profile).unwrap(),
            DecodedSourceAsset::Binary(picture.bytes().to_vec())
        );

        let help = HelpAssets::new(
            vec![
                NamedAsset::new(
                    "ru",
                    Asset::from_bytes(b"<html>help</html>".to_vec(), "text/html").unwrap(),
                )
                .unwrap(),
            ],
            vec![
                NamedAsset::new(
                    "icon.png",
                    Asset::from_bytes(b"png".to_vec(), "image/png").unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let encoded = encode_help(&help).unwrap();
        let decoded = decode_help(&encoded).unwrap();
        assert_eq!(
            decoded.pages[0].asset.sha256(),
            help.pages[0].asset.sha256()
        );
        assert_eq!(
            decoded.files[0].asset.sha256(),
            help.files[0].asset.sha256()
        );
    }

    #[test]
    fn help_compilation_uses_the_exact_registered_graph_route() {
        let configuration_uuid = ObjectUuid::parse("11111111-1111-4111-8111-111111111111").unwrap();
        let command_uuid = ObjectUuid::parse("22222222-2222-4222-8222-222222222222").unwrap();
        let configuration = CanonicalConfiguration::new(vec![
            object(
                &configuration_uuid.to_string(),
                "Configuration",
                "configuration",
            ),
            object(&command_uuid.to_string(), "CommonCommand", "command"),
        ])
        .unwrap();
        let validated = validate_configuration(&configuration).unwrap();
        let identities = collect_bootstrap_identities(&validated).unwrap();
        let graph = build_bootstrap_graph(
            &identities,
            ProfileId::parse("platform-8.3.27.1989").unwrap(),
            vec![
                ObjectStorageRoute::new(configuration_uuid, Vec::new()).unwrap(),
                ObjectStorageRoute::new(command_uuid, vec![StorageSuffix::new(".5").unwrap()])
                    .unwrap(),
            ],
        )
        .unwrap();
        let axes = CompileAxes::new(
            XmlDialect::parse("2.21").unwrap(),
            Some(PlatformBuild::parse("8.3.27.1989").unwrap()),
            None,
            StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            None,
        );
        let profile = AssetCodecProfile::fixture(SourceAssetCodec::Help);
        let route = SourceAssetRegistry
            .route("CommonCommand", SourceAssetRole::Help)
            .unwrap();
        let help = HelpAssets::new(
            vec![
                NamedAsset::new(
                    "ru",
                    Asset::from_bytes(b"<html/>".to_vec(), "text/html").unwrap(),
                )
                .unwrap(),
            ],
            Vec::new(),
        )
        .unwrap();

        let entry = compile_source_asset(
            &graph,
            command_uuid,
            route,
            SourceAssetPayload::Help(&help),
            &axes,
            &profile,
        )
        .unwrap();

        assert_eq!(entry.target().key().as_str(), format!("{command_uuid}.5"));
        let StoragePatchOutcome::Compiled(blob) = entry.outcome() else {
            panic!("help route must compile")
        };
        let DecodedSourceAsset::Help(decoded) =
            decode_source_asset(blob.bytes(), route, &profile).unwrap()
        else {
            panic!("help route decoded as another asset kind")
        };
        assert_eq!(decoded.pages[0].name, help.pages[0].name);
        assert_eq!(
            decoded.pages[0].asset.sha256(),
            help.pages[0].asset.sha256()
        );
        assert!(decoded.files.is_empty());
    }

    #[test]
    fn malformed_base64_and_future_profile_fail_closed() {
        assert!(decode_base64("Zh==").is_err());
        assert_eq!(
            AssetCodecProfile::fixture(SourceAssetCodec::Deferred).codec,
            SourceAssetCodec::Deferred
        );
        assert!(matches!(SourceAssetCodec::Deferred.layout_key(), None));
    }
}
