//! Complete base-free compilation of one hierarchical XML source tree.
//!
//! This coordinator joins the canonical XML adapters, versioned family
//! codecs, bootstrap identity graph, special entries, and explicit source
//! asset registry.  Every source file must be consumed by exactly one route;
//! unsupported or ambiguous input prevents construction of a `StoragePatch`.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt::{self, Display, Formatter},
};

use ibcmd_core::{
    artifact::{ProfileId, StorageProfileId},
    asset::Asset,
    diagnostic::{ObjectPath, PathSegment},
    family::FamilyId,
    identity::ObjectUuid,
    model::{CanonicalConfiguration, CanonicalObject},
    profile::EffectiveProfile,
    storage::{StoragePatch, StoragePatchEntry},
    validate::{ValidatedConfiguration, validate_configuration},
    value::CanonicalValueKind,
    version::XmlDialect,
};
use ibcmd_xml::{
    DialectDetection, DialectRegistry, XmlDocument, XmlElement, XmlNode, XmlReader,
    bundled_dialect_registry, bundled_metadata_registry,
    metadata::decode_configuration_envelope,
    source_tree::{SourceKind, SourceTree},
};

use super::{
    CompileAxes,
    families::{
        assets::{
            AssetCodecProfile, SourceAssetCodec, SourceAssetPayload, SourceAssetRegistry,
            SourceAssetRoute, compile_source_asset,
        },
        business_process::{BusinessProcessMetadataProfile, compile_business_process_metadata},
        catalog::{CatalogMetadataProfile, compile_catalog_metadata},
        charts::{ChartFamily, ChartMetadataProfile, compile_chart_metadata},
        commands::{CommandMetadataFamily, CommandMetadataProfile, compile_command_metadata},
        data_processor::{DataProcessorMetadataProfile, compile_data_processor_metadata},
        document::{DocumentMetadataProfile, compile_document_metadata},
        r#enum::{EnumMetadataProfile, compile_enum_metadata},
        exchange_plan::{ExchangePlanMetadataProfile, compile_exchange_plan_metadata},
        modules::{CommonModuleProfile, compile_common_module_metadata},
        recalculation::{RecalculationMetadataProfile, compile_recalculation_metadata},
        registers::{RegisterFamily, RegisterMetadataProfile, compile_register_metadata},
        report::{ReportMetadataProfile, compile_report_metadata},
        services::{ServiceFamily, ServiceMetadataProfile, compile_service_metadata},
        settings::{SettingsStorageMetadataProfile, compile_settings_storage_metadata},
        simple::{SimpleFamily, SimpleMetadataProfile, compile_simple_metadata},
        subsystem::{SubsystemMetadataProfile, compile_subsystem_metadata},
        task::{TaskMetadataProfile, compile_task_metadata},
    },
    graph::{
        BootstrapGraph, InventoryScope, ObjectStorageRoute, StorageSuffix, build_bootstrap_graph,
    },
    identity::collect_bootstrap_identities,
    root::{
        ConfigurationBodyProperties, ConfigurationLocalizedString, ConfigurationRunMode,
        ConfigurationScriptVariant, compile_configuration_body, compile_root,
    },
    version::{SpecialEntryProfile, compile_version},
    versions::compile_versions,
};

/// Complete compiler result.  The patch owns every payload and can cross the
/// root-crate/CF-crate boundary without retaining XML documents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapCompilation {
    target_profile: ProfileId,
    storage_profile: StorageProfileId,
    source_files: usize,
    metadata_files: usize,
    asset_files: usize,
    patch: StoragePatch,
}

impl BootstrapCompilation {
    #[must_use]
    pub const fn target_profile(&self) -> &ProfileId {
        &self.target_profile
    }

    #[must_use]
    pub const fn storage_profile(&self) -> &StorageProfileId {
        &self.storage_profile
    }

    #[must_use]
    pub const fn source_files(&self) -> usize {
        self.source_files
    }

    #[must_use]
    pub const fn metadata_files(&self) -> usize {
        self.metadata_files
    }

    #[must_use]
    pub const fn asset_files(&self) -> usize {
        self.asset_files
    }

    #[must_use]
    pub const fn patch(&self) -> &StoragePatch {
        &self.patch
    }

    pub fn into_patch(self) -> StoragePatch {
        self.patch
    }
}

#[derive(Clone, Debug)]
struct MetadataSource {
    path: String,
    owner_directory: String,
    family: String,
    uuid: ObjectUuid,
}

#[derive(Clone, Debug)]
struct AssetSource {
    source_index: usize,
    owner_uuid: ObjectUuid,
    route: &'static SourceAssetRoute,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ConfigurationChildReference {
    family: String,
    name: String,
}

struct ConfigurationProjection {
    properties: ConfigurationBodyProperties,
    children: Vec<ConfigurationChildReference>,
}

/// Compiles one complete, bounded source tree for an explicitly selected XML
/// dialect and target platform profile.
pub fn compile_bootstrap_source_tree(
    tree: &SourceTree,
    xml_dialect: XmlDialect,
    target_profile: &EffectiveProfile,
) -> Result<BootstrapCompilation, BootstrapCompileError> {
    tree.validate()
        .map_err(|source| BootstrapCompileError::SourceTree(source.to_string()))?;
    let platform_build = target_profile
        .platform_build
        .as_ref()
        .map(|coordinate| coordinate.value.clone())
        .ok_or(BootstrapCompileError::MissingTargetCoordinate(
            "platform_build",
        ))?;
    let storage_profile = target_profile
        .storage_profile
        .as_ref()
        .map(|coordinate| coordinate.value.clone())
        .ok_or(BootstrapCompileError::MissingTargetCoordinate(
            "storage_profile",
        ))?;
    let axes = CompileAxes::new(
        xml_dialect.clone(),
        Some(platform_build),
        None,
        storage_profile.clone(),
        None,
    );
    let source_profile = ProfileId::parse(&format!("xml-{xml_dialect}"))
        .map_err(|source| BootstrapCompileError::SourceProfile(source.to_string()))?;
    let dialects = bundled_dialect_registry()
        .map_err(|source| BootstrapCompileError::SourceProfile(source.to_string()))?;
    let registry = bundled_metadata_registry();

    let mut objects = Vec::<CanonicalObject>::new();
    let mut metadata_sources = Vec::<MetadataSource>::new();
    let mut metadata_indexes = BTreeSet::<usize>::new();
    let mut configuration = None::<ConfigurationProjection>;

    for (source_index, source) in tree.entries().iter().enumerate() {
        if !source
            .path()
            .as_str()
            .to_ascii_lowercase()
            .ends_with(".xml")
        {
            continue;
        }
        let document = XmlReader::from_slice(source.bytes()).map_err(|error| {
            BootstrapCompileError::InvalidXml {
                path: source.path().as_str().to_owned(),
                message: error.to_string(),
            }
        })?;
        validate_source_dialect(
            &document,
            &dialects,
            &source_profile,
            source.path().as_str(),
        )?;
        if document.root().name().local() != "MetaDataObject" {
            continue;
        }
        let family = metadata_family(&document).ok_or_else(|| {
            BootstrapCompileError::InvalidMetadataEnvelope {
                path: source.path().as_str().to_owned(),
                message: "MetaDataObject must contain exactly one metadata element".to_owned(),
            }
        })?;
        let object_path = ObjectPath::new(vec![
            PathSegment::name("source").expect("static path segment is valid"),
            PathSegment::index(u32::try_from(source_index).map_err(|_| {
                BootstrapCompileError::InvalidMetadataEnvelope {
                    path: source.path().as_str().to_owned(),
                    message: "source index exceeds canonical path range".to_owned(),
                }
            })?),
        ])
        .expect("bounded source-tree index makes a bounded canonical path");

        let envelope = if family == "Configuration" {
            if source.kind() != SourceKind::ConfigurationRoot {
                return Err(BootstrapCompileError::ConfigurationPath {
                    path: source.path().as_str().to_owned(),
                });
            }
            if configuration.is_some() {
                return Err(BootstrapCompileError::ConfigurationCount { actual: 2 });
            }
            configuration = Some(project_configuration(
                &document,
                SpecialEntryProfile::from_effective(target_profile)
                    .map_err(|error| profile_error("special entries", error))?
                    .compatibility(),
            )?);
            decode_configuration_envelope(&document, source_profile.clone(), object_path)
        } else {
            let family_id = FamilyId::parse(&family).map_err(|error| {
                BootstrapCompileError::InvalidMetadataEnvelope {
                    path: source.path().as_str().to_owned(),
                    message: error.to_string(),
                }
            })?;
            if !registry.contains(&family_id) {
                return Err(BootstrapCompileError::UnsupportedMetadataFamily {
                    path: source.path().as_str().to_owned(),
                    family,
                });
            }
            registry.decode(&family_id, &document, source_profile.clone(), object_path)
        }
        .map_err(|error| BootstrapCompileError::InvalidMetadataEnvelope {
            path: source.path().as_str().to_owned(),
            message: error.to_string(),
        })?;
        let uuid = envelope.root().identity().uuid();
        let actual_family = envelope.root().kind().as_str().to_owned();
        let path = source.path().as_str().to_owned();
        metadata_sources.push(MetadataSource {
            owner_directory: owner_directory(&path, &actual_family),
            path,
            family: actual_family,
            uuid,
        });
        metadata_indexes.insert(source_index);
        objects.push(envelope.root().clone());
        objects.extend(envelope.descendants().iter().cloned());
    }

    let configuration_count = metadata_sources
        .iter()
        .filter(|source| source.family == "Configuration")
        .count();
    if configuration_count != 1 {
        return Err(BootstrapCompileError::ConfigurationCount {
            actual: configuration_count,
        });
    }
    let configuration = configuration.expect("configuration count proved a decoded projection");
    let canonical = CanonicalConfiguration::new(objects)
        .map_err(|source| BootstrapCompileError::Canonical(source.to_string()))?;
    validate_configuration_children(&canonical, &configuration.children)?;
    let validated = validate_configuration(&canonical)
        .map_err(|source| BootstrapCompileError::Canonical(format!("{source:?}")))?;
    let identities = collect_bootstrap_identities(&validated)
        .map_err(|source| BootstrapCompileError::Identity(source.to_string()))?;

    let assets = resolve_assets(tree, &metadata_sources, &metadata_indexes)?;
    let mut suffixes = BTreeMap::<ObjectUuid, Vec<StorageSuffix>>::new();
    for asset in &assets {
        suffixes.entry(asset.owner_uuid).or_default().push(
            StorageSuffix::new(asset.route.suffix())
                .map_err(|source| BootstrapCompileError::Graph(source.to_string()))?,
        );
    }
    for values in suffixes.values_mut() {
        values.sort();
        values.dedup();
    }
    let routes = identities
        .objects()
        .iter()
        .filter(|identity| identity.owner().is_none())
        .map(|identity| {
            ObjectStorageRoute::new(
                identity.uuid(),
                suffixes.remove(&identity.uuid()).unwrap_or_default(),
            )
            .map_err(|source| BootstrapCompileError::Graph(source.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let graph = build_bootstrap_graph(&identities, target_profile.id.clone(), routes)
        .map_err(|source| BootstrapCompileError::Graph(source.to_string()))?;

    let special_profile = SpecialEntryProfile::from_effective(target_profile)
        .map_err(|error| profile_error("special entries", error))?;
    let mut compiled = BTreeMap::<String, StoragePatchEntry>::new();
    insert_compiled(
        &mut compiled,
        compile_root(&graph, &special_profile)
            .map_err(|error| compiler_error("root", None, error))?,
    )?;
    insert_compiled(
        &mut compiled,
        compile_configuration_body(
            &identities,
            &graph,
            &special_profile,
            &configuration.properties,
        )
        .map_err(|error| {
            compiler_error("Configuration", Some(graph.configuration_uuid()), error)
        })?,
    )?;
    insert_compiled(
        &mut compiled,
        compile_version(&graph, &special_profile)
            .map_err(|error| compiler_error("version", None, error))?,
    )?;

    for source in &metadata_sources {
        if source.family == "Configuration" {
            continue;
        }
        let entry = compile_metadata(&validated, &graph, source, &axes, target_profile)?;
        insert_compiled(&mut compiled, entry)?;
    }
    for asset in &assets {
        let source = &tree.entries()[asset.source_index];
        let codec = asset.route.codec();
        let selected = AssetCodecProfile::from_effective_for_codec(target_profile, codec)
            .map_err(|error| profile_error("source asset", error))?;
        let entry = match codec {
            SourceAssetCodec::Module => compile_source_asset(
                &graph,
                asset.owner_uuid,
                asset.route,
                SourceAssetPayload::Module(source.bytes()),
                &axes,
                &selected,
            ),
            SourceAssetCodec::RawBinary => {
                let exact = Asset::from_bytes(source.bytes().to_vec(), "application/octet-stream")
                    .map_err(|error| BootstrapCompileError::Asset {
                        path: source.path().as_str().to_owned(),
                        message: error.to_string(),
                    })?;
                compile_source_asset(
                    &graph,
                    asset.owner_uuid,
                    asset.route,
                    SourceAssetPayload::Binary(&exact),
                    &axes,
                    &selected,
                )
            }
            _ => {
                return Err(BootstrapCompileError::UnsupportedAssetCodec {
                    path: source.path().as_str().to_owned(),
                    family: asset.route.owner_family().to_owned(),
                    codec,
                });
            }
        }
        .map_err(|error| BootstrapCompileError::Asset {
            path: source.path().as_str().to_owned(),
            message: error.to_string(),
        })?;
        insert_compiled(&mut compiled, entry)?;
    }

    let without_versions = StoragePatch::new(compiled.into_values().collect())
        .map_err(|source| BootstrapCompileError::Patch(source.to_string()))?;
    graph
        .validate_patch_inventory(&without_versions, InventoryScope::BeforeVersions)
        .map_err(|source| BootstrapCompileError::Graph(source.to_string()))?;
    let versions = compile_versions(&graph, &without_versions, &special_profile)
        .map_err(|error| compiler_error("versions", None, error))?;
    let mut final_entries = without_versions.into_entries();
    final_entries.push(versions);
    final_entries.sort_by(|left, right| {
        left.target()
            .key()
            .as_str()
            .cmp(right.target().key().as_str())
    });
    let patch = StoragePatch::new(final_entries)
        .map_err(|source| BootstrapCompileError::Patch(source.to_string()))?;
    graph
        .validate_patch_inventory(&patch, InventoryScope::Complete)
        .map_err(|source| BootstrapCompileError::Graph(source.to_string()))?;
    patch
        .preflight()
        .map_err(|source| BootstrapCompileError::Patch(source.to_string()))?;

    Ok(BootstrapCompilation {
        target_profile: target_profile.id.clone(),
        storage_profile,
        source_files: tree.entries().len(),
        metadata_files: metadata_sources.len(),
        asset_files: assets.len(),
        patch,
    })
}

fn validate_source_dialect(
    document: &XmlDocument,
    dialects: &DialectRegistry,
    source_profile: &ProfileId,
    path: &str,
) -> Result<(), BootstrapCompileError> {
    let detection = dialects.detect(document).map_err(|source| {
        BootstrapCompileError::InvalidMetadataEnvelope {
            path: path.to_owned(),
            message: format!("XML dialect detection failed: {source}"),
        }
    })?;
    let matches = match detection {
        DialectDetection::Exact { candidate, .. } => candidate.profile_id() == source_profile,
        DialectDetection::Ambiguous { candidates, .. } => candidates
            .iter()
            .any(|candidate| candidate.profile_id() == source_profile),
        DialectDetection::Unknown { .. } => false,
    };
    if !matches {
        return Err(BootstrapCompileError::InvalidMetadataEnvelope {
            path: path.to_owned(),
            message: format!(
                "XML dialect evidence is incompatible with selected source profile `{source_profile}`"
            ),
        });
    }
    Ok(())
}

fn insert_compiled(
    entries: &mut BTreeMap<String, StoragePatchEntry>,
    entry: StoragePatchEntry,
) -> Result<(), BootstrapCompileError> {
    let key = entry.target().key().as_str().to_owned();
    if entries.insert(key.clone(), entry).is_some() {
        return Err(BootstrapCompileError::DuplicateCompiledEntry { key });
    }
    Ok(())
}

fn compile_metadata(
    validated: &ValidatedConfiguration<'_>,
    graph: &BootstrapGraph,
    source: &MetadataSource,
    axes: &CompileAxes,
    effective: &EffectiveProfile,
) -> Result<StoragePatchEntry, BootstrapCompileError> {
    let uuid = source.uuid;
    let family = source.family.as_str();
    macro_rules! select_compile {
        ($profile:ty, $compiler:path) => {{
            let selected = <$profile>::from_effective(effective)
                .map_err(|error| profile_error(family, error))?;
            $compiler(validated, graph, uuid, axes, &selected)
                .map_err(|error| compiler_error(family, Some(uuid), error))
        }};
    }
    match family {
        "Catalog" => select_compile!(CatalogMetadataProfile, compile_catalog_metadata),
        "Document" => select_compile!(DocumentMetadataProfile, compile_document_metadata),
        "Subsystem" => select_compile!(SubsystemMetadataProfile, compile_subsystem_metadata),
        "ExchangePlan" => {
            select_compile!(ExchangePlanMetadataProfile, compile_exchange_plan_metadata)
        }
        "BusinessProcess" => select_compile!(
            BusinessProcessMetadataProfile,
            compile_business_process_metadata
        ),
        "Task" => select_compile!(TaskMetadataProfile, compile_task_metadata),
        "Recalculation" => {
            select_compile!(RecalculationMetadataProfile, compile_recalculation_metadata)
        }
        "Report" => select_compile!(ReportMetadataProfile, compile_report_metadata),
        "DataProcessor" => select_compile!(
            DataProcessorMetadataProfile,
            compile_data_processor_metadata
        ),
        "Enum" => select_compile!(EnumMetadataProfile, compile_enum_metadata),
        "SettingsStorage" => select_compile!(
            SettingsStorageMetadataProfile,
            compile_settings_storage_metadata
        ),
        "CommonModule" => {
            select_compile!(CommonModuleProfile, compile_common_module_metadata)
        }
        simple => {
            if let Some(simple) = simple_family(simple) {
                let selected = SimpleMetadataProfile::from_effective_for_family(effective, simple)
                    .map_err(|error| profile_error(family, error))?;
                return compile_simple_metadata(validated, graph, uuid, axes, &selected)
                    .map_err(|error| compiler_error(family, Some(uuid), error));
            }
            if let Some(service) = service_family(simple) {
                let selected =
                    ServiceMetadataProfile::from_effective_for_family(effective, service)
                        .map_err(|error| profile_error(family, error))?;
                return compile_service_metadata(validated, graph, uuid, axes, &selected)
                    .map_err(|error| compiler_error(family, Some(uuid), error));
            }
            if let Some(register) = register_family(simple) {
                let selected =
                    RegisterMetadataProfile::from_effective_for_family(effective, register)
                        .map_err(|error| profile_error(family, error))?;
                return compile_register_metadata(validated, graph, uuid, axes, &selected)
                    .map_err(|error| compiler_error(family, Some(uuid), error));
            }
            if let Some(chart) = chart_family(simple) {
                let selected = ChartMetadataProfile::from_effective_for_family(effective, chart)
                    .map_err(|error| profile_error(family, error))?;
                return compile_chart_metadata(validated, graph, uuid, axes, &selected)
                    .map_err(|error| compiler_error(family, Some(uuid), error));
            }
            if let Some(command) = command_family(simple) {
                let selected =
                    CommandMetadataProfile::from_effective_for_family(effective, command)
                        .map_err(|error| profile_error(family, error))?;
                return compile_command_metadata(validated, graph, uuid, axes, &selected)
                    .map_err(|error| compiler_error(family, Some(uuid), error));
            }
            Err(BootstrapCompileError::UnsupportedMetadataFamily {
                path: source.path.clone(),
                family: family.to_owned(),
            })
        }
    }
}

fn simple_family(family: &str) -> Option<SimpleFamily> {
    match family {
        "Constant" => Some(SimpleFamily::Constant),
        "Language" => Some(SimpleFamily::Language),
        "SessionParameter" => Some(SimpleFamily::SessionParameter),
        "DefinedType" => Some(SimpleFamily::DefinedType),
        "FunctionalOption" => Some(SimpleFamily::FunctionalOption),
        "FunctionalOptionsParameter" => Some(SimpleFamily::FunctionalOptionsParameter),
        _ => None,
    }
}

fn service_family(family: &str) -> Option<ServiceFamily> {
    match family {
        "ScheduledJob" => Some(ServiceFamily::ScheduledJob),
        "EventSubscription" => Some(ServiceFamily::EventSubscription),
        "HTTPService" => Some(ServiceFamily::HttpService),
        "WebService" => Some(ServiceFamily::WebService),
        "IntegrationService" => Some(ServiceFamily::IntegrationService),
        "WSReference" => Some(ServiceFamily::WsReference),
        "XDTOPackage" => Some(ServiceFamily::XdtoPackage),
        _ => None,
    }
}

fn register_family(family: &str) -> Option<RegisterFamily> {
    match family {
        "InformationRegister" => Some(RegisterFamily::Information),
        "AccumulationRegister" => Some(RegisterFamily::Accumulation),
        "AccountingRegister" => Some(RegisterFamily::Accounting),
        "CalculationRegister" => Some(RegisterFamily::Calculation),
        _ => None,
    }
}

fn chart_family(family: &str) -> Option<ChartFamily> {
    match family {
        "ChartOfCharacteristicTypes" => Some(ChartFamily::CharacteristicTypes),
        "ChartOfAccounts" => Some(ChartFamily::Accounts),
        "ChartOfCalculationTypes" => Some(ChartFamily::CalculationTypes),
        _ => None,
    }
}

fn command_family(family: &str) -> Option<CommandMetadataFamily> {
    match family {
        "CommonCommand" => Some(CommandMetadataFamily::CommonCommand),
        "CommandGroup" => Some(CommandMetadataFamily::CommandGroup),
        "CommonPicture" => Some(CommandMetadataFamily::CommonPicture),
        _ => None,
    }
}

fn resolve_assets(
    tree: &SourceTree,
    metadata: &[MetadataSource],
    metadata_indexes: &BTreeSet<usize>,
) -> Result<Vec<AssetSource>, BootstrapCompileError> {
    let registry = SourceAssetRegistry;
    let mut assets = Vec::new();
    for (source_index, source) in tree.entries().iter().enumerate() {
        if metadata_indexes.contains(&source_index) {
            continue;
        }
        let path = source.path().as_str();
        let mut matches = Vec::<(ObjectUuid, &'static SourceAssetRoute)>::new();
        for owner in metadata {
            let Some(relative) = relative_to_owner(path, &owner.owner_directory) else {
                continue;
            };
            if let Some(route) = registry.route_by_relative_path(&owner.family, relative) {
                matches.push((owner.uuid, route));
            }
        }
        matches.sort_by_key(|(uuid, route)| (*uuid, route.suffix()));
        matches.dedup();
        match matches.as_slice() {
            [] => {
                return Err(BootstrapCompileError::UnconsumedSource {
                    path: path.to_owned(),
                    kind: source.kind(),
                });
            }
            [(owner_uuid, route)] => assets.push(AssetSource {
                source_index,
                owner_uuid: *owner_uuid,
                route,
            }),
            _ => {
                return Err(BootstrapCompileError::AmbiguousSourceRoute {
                    path: path.to_owned(),
                    candidates: matches
                        .iter()
                        .map(|(uuid, route)| format!("{uuid}{}", route.suffix()))
                        .collect(),
                });
            }
        }
    }
    assets.sort_by(|left, right| {
        let left_key = format!("{}{}", left.owner_uuid, left.route.suffix());
        let right_key = format!("{}{}", right.owner_uuid, right.route.suffix());
        left_key.cmp(&right_key)
    });
    Ok(assets)
}

fn owner_directory(path: &str, family: &str) -> String {
    if family == "Configuration" {
        String::new()
    } else {
        path.strip_suffix(".xml").unwrap_or(path).to_owned()
    }
}

fn relative_to_owner<'a>(path: &'a str, owner: &str) -> Option<&'a str> {
    if owner.is_empty() {
        return path.strip_prefix("Ext/");
    }
    path.strip_prefix(owner)?.strip_prefix('/')
}

fn metadata_family(document: &XmlDocument) -> Option<String> {
    let mut elements = document.root().children().iter().filter_map(|node| {
        if let XmlNode::Element(element) = node {
            Some(element)
        } else {
            None
        }
    });
    let family = elements.next()?.name().local().to_owned();
    elements.next().is_none().then_some(family)
}

fn validate_configuration_children(
    canonical: &CanonicalConfiguration,
    expected: &[ConfigurationChildReference],
) -> Result<(), BootstrapCompileError> {
    let expected = expected.iter().cloned().collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    for object in canonical
        .objects()
        .iter()
        .filter(|object| object.owner().is_none() && object.kind().as_str() != "Configuration")
    {
        let name = object_name(object).ok_or_else(|| {
            BootstrapCompileError::InvalidConfiguration(
                "top-level metadata object has no textual Name".to_owned(),
            )
        })?;
        let reference = ConfigurationChildReference {
            family: object.kind().as_str().to_owned(),
            name: name.to_owned(),
        };
        if !actual.insert(reference.clone()) {
            return Err(BootstrapCompileError::DuplicateConfigurationChild {
                family: reference.family,
                name: reference.name,
            });
        }
    }
    if expected != actual {
        return Err(BootstrapCompileError::ConfigurationInventoryMismatch {
            missing: expected.difference(&actual).map(render_child).collect(),
            extra: actual.difference(&expected).map(render_child).collect(),
        });
    }
    Ok(())
}

fn object_name(object: &CanonicalObject) -> Option<&str> {
    object
        .properties()
        .iter()
        .find(|field| field.name().as_str() == "Name")
        .and_then(|field| match field.value().kind() {
            CanonicalValueKind::Text(value) => Some(value.as_str()),
            _ => None,
        })
}

fn project_configuration(
    document: &XmlDocument,
    target_compatibility: u32,
) -> Result<ConfigurationProjection, BootstrapCompileError> {
    let configuration = only_child_element(document.root(), "MetaDataObject")?;
    if configuration.name().local() != "Configuration" {
        return Err(BootstrapCompileError::InvalidConfiguration(
            "root metadata object is not Configuration".to_owned(),
        ));
    }
    let properties_element = named_child(configuration, "Properties")?.ok_or_else(|| {
        BootstrapCompileError::InvalidConfiguration("Configuration has no Properties".to_owned())
    })?;
    let mut properties = ConfigurationBodyProperties::minimal("", target_compatibility);
    let mut seen = BTreeSet::new();
    for element in child_elements(properties_element)? {
        let name = element.name().local();
        if !seen.insert(name.to_owned()) {
            return Err(BootstrapCompileError::InvalidConfiguration(format!(
                "Configuration property `{name}` is duplicated"
            )));
        }
        match name {
            "Name" => properties.name = simple_text(element)?,
            "Synonym" => properties.synonyms = localized(element)?,
            "Comment" => properties.comment = simple_text(element)?,
            "NamePrefix" => properties.name_prefix = simple_text(element)?,
            "DefaultRunMode" => {
                properties.default_run_mode = match simple_text(element)?.as_str() {
                    "ManagedApplication" => ConfigurationRunMode::ManagedApplication,
                    "OrdinaryApplication" => ConfigurationRunMode::OrdinaryApplication,
                    value => {
                        return Err(BootstrapCompileError::InvalidConfiguration(format!(
                            "unsupported DefaultRunMode `{value}`"
                        )));
                    }
                }
            }
            "ScriptVariant" => {
                properties.script_variant = match simple_text(element)?.as_str() {
                    "Russian" => ConfigurationScriptVariant::Russian,
                    "English" => ConfigurationScriptVariant::English,
                    value => {
                        return Err(BootstrapCompileError::InvalidConfiguration(format!(
                            "unsupported ScriptVariant `{value}`"
                        )));
                    }
                }
            }
            "CompatibilityMode" => {
                properties.compatibility_mode = compatibility_token(&simple_text(element)?)?
            }
            "ConfigurationExtensionCompatibilityMode" => {
                properties.extension_compatibility_mode =
                    compatibility_token(&simple_text(element)?)?
            }
            "BriefInformation" => properties.brief_information = localized(element)?,
            "DetailedInformation" => properties.detailed_information = localized(element)?,
            "Copyright" => properties.copyright = localized(element)?,
            "VendorInformationAddress" => {
                properties.vendor_information_address = localized(element)?
            }
            "ConfigurationInformationAddress" => {
                properties.configuration_information_address = localized(element)?
            }
            "Vendor" => properties.vendor = simple_text(element)?,
            "Version" => properties.version = simple_text(element)?,
            "UpdateCatalogAddress" => properties.update_catalog_address = simple_text(element)?,
            unsupported => {
                return Err(BootstrapCompileError::InvalidConfiguration(format!(
                    "Configuration property `{unsupported}` has no base-free projection"
                )));
            }
        }
    }
    if properties.name.is_empty() {
        return Err(BootstrapCompileError::InvalidConfiguration(
            "Configuration Name must be non-empty".to_owned(),
        ));
    }

    let mut children = Vec::new();
    if let Some(child_objects) = named_child(configuration, "ChildObjects")? {
        for child in child_elements(child_objects)? {
            let name = simple_text(child)?;
            if name.is_empty() {
                return Err(BootstrapCompileError::InvalidConfiguration(format!(
                    "Configuration child `{}` has an empty name",
                    child.name().local()
                )));
            }
            children.push(ConfigurationChildReference {
                family: child.name().local().to_owned(),
                name,
            });
        }
    }
    let unique = children.iter().cloned().collect::<BTreeSet<_>>();
    if unique.len() != children.len() {
        return Err(BootstrapCompileError::InvalidConfiguration(
            "Configuration ChildObjects contains a duplicate family/name reference".to_owned(),
        ));
    }
    Ok(ConfigurationProjection {
        properties,
        children,
    })
}

fn compatibility_token(value: &str) -> Result<u32, BootstrapCompileError> {
    let parts = value
        .strip_prefix("Version")
        .ok_or_else(|| {
            BootstrapCompileError::InvalidConfiguration(format!(
                "unsupported compatibility token `{value}`"
            ))
        })?
        .split('_')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| {
            BootstrapCompileError::InvalidConfiguration(format!(
                "unsupported compatibility token `{value}`"
            ))
        })?;
    if parts.len() != 3 || parts[0] > 99 || parts[1] > 99 || parts[2] > 99 {
        return Err(BootstrapCompileError::InvalidConfiguration(format!(
            "unsupported compatibility token `{value}`"
        )));
    }
    Ok(parts[0] * 10_000 + parts[1] * 100 + parts[2])
}

fn localized(
    element: &XmlElement,
) -> Result<Vec<ConfigurationLocalizedString>, BootstrapCompileError> {
    let mut values = Vec::new();
    for item in child_elements(element)? {
        if item.name().local() != "item" {
            return Err(BootstrapCompileError::InvalidConfiguration(format!(
                "localized property contains unexpected `{}` element",
                item.name().local()
            )));
        }
        let language = named_child(item, "lang")?.ok_or_else(|| {
            BootstrapCompileError::InvalidConfiguration(
                "localized item has no lang element".to_owned(),
            )
        })?;
        let content = named_child(item, "content")?.ok_or_else(|| {
            BootstrapCompileError::InvalidConfiguration(
                "localized item has no content element".to_owned(),
            )
        })?;
        let language = simple_text(language)?;
        if language.is_empty() {
            return Err(BootstrapCompileError::InvalidConfiguration(
                "localized item language is empty".to_owned(),
            ));
        }
        values.push(ConfigurationLocalizedString::new(
            language,
            simple_text(content)?,
        ));
    }
    Ok(values)
}

fn only_child_element<'a>(
    element: &'a XmlElement,
    context: &str,
) -> Result<&'a XmlElement, BootstrapCompileError> {
    let children = child_elements(element)?;
    if children.len() != 1 {
        return Err(BootstrapCompileError::InvalidConfiguration(format!(
            "{context} must contain exactly one element"
        )));
    }
    Ok(children[0])
}

fn named_child<'a>(
    element: &'a XmlElement,
    name: &str,
) -> Result<Option<&'a XmlElement>, BootstrapCompileError> {
    let mut matches = child_elements(element)?
        .into_iter()
        .filter(|child| child.name().local() == name);
    let first = matches.next();
    if matches.next().is_some() {
        return Err(BootstrapCompileError::InvalidConfiguration(format!(
            "element `{name}` is duplicated"
        )));
    }
    Ok(first)
}

fn child_elements(element: &XmlElement) -> Result<Vec<&XmlElement>, BootstrapCompileError> {
    let mut elements = Vec::new();
    for node in element.children() {
        match node {
            XmlNode::Element(child) => elements.push(child),
            XmlNode::Text(text) if text.value().trim().is_empty() => {}
            XmlNode::Comment(_) => {}
            XmlNode::Text(_) | XmlNode::CData(_) => {
                return Err(BootstrapCompileError::InvalidConfiguration(format!(
                    "element `{}` contains mixed text",
                    element.name().local()
                )));
            }
            XmlNode::ProcessingInstruction(_) | XmlNode::DocType(_) => {
                return Err(BootstrapCompileError::InvalidConfiguration(format!(
                    "element `{}` contains unsupported markup",
                    element.name().local()
                )));
            }
        }
    }
    Ok(elements)
}

fn simple_text(element: &XmlElement) -> Result<String, BootstrapCompileError> {
    let mut value = String::new();
    for node in element.children() {
        match node {
            XmlNode::Text(text) => value.push_str(text.value()),
            XmlNode::CData(text) => value.push_str(text.value()),
            XmlNode::Comment(_) => {}
            XmlNode::Element(child) => {
                return Err(BootstrapCompileError::InvalidConfiguration(format!(
                    "simple element `{}` contains nested `{}`",
                    element.name().local(),
                    child.name().local()
                )));
            }
            XmlNode::ProcessingInstruction(_) | XmlNode::DocType(_) => {
                return Err(BootstrapCompileError::InvalidConfiguration(format!(
                    "simple element `{}` contains unsupported markup",
                    element.name().local()
                )));
            }
        }
    }
    Ok(value)
}

fn profile_error(scope: &str, error: impl Display) -> BootstrapCompileError {
    BootstrapCompileError::Profile {
        scope: scope.to_owned(),
        message: error.to_string(),
    }
}

fn compiler_error(
    family: &str,
    uuid: Option<ObjectUuid>,
    error: impl Display,
) -> BootstrapCompileError {
    BootstrapCompileError::Compiler {
        family: family.to_owned(),
        uuid,
        message: error.to_string(),
    }
}

#[derive(Debug)]
pub enum BootstrapCompileError {
    SourceTree(String),
    SourceProfile(String),
    MissingTargetCoordinate(&'static str),
    InvalidXml {
        path: String,
        message: String,
    },
    InvalidMetadataEnvelope {
        path: String,
        message: String,
    },
    ConfigurationPath {
        path: String,
    },
    ConfigurationCount {
        actual: usize,
    },
    InvalidConfiguration(String),
    DuplicateConfigurationChild {
        family: String,
        name: String,
    },
    ConfigurationInventoryMismatch {
        missing: Vec<String>,
        extra: Vec<String>,
    },
    UnsupportedMetadataFamily {
        path: String,
        family: String,
    },
    UnconsumedSource {
        path: String,
        kind: SourceKind,
    },
    AmbiguousSourceRoute {
        path: String,
        candidates: Vec<String>,
    },
    UnsupportedAssetCodec {
        path: String,
        family: String,
        codec: SourceAssetCodec,
    },
    Asset {
        path: String,
        message: String,
    },
    Profile {
        scope: String,
        message: String,
    },
    Canonical(String),
    Identity(String),
    Graph(String),
    Compiler {
        family: String,
        uuid: Option<ObjectUuid>,
        message: String,
    },
    DuplicateCompiledEntry {
        key: String,
    },
    Patch(String),
}

impl Display for BootstrapCompileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceTree(message) => write!(formatter, "invalid source tree: {message}"),
            Self::SourceProfile(message) => write!(formatter, "invalid source profile: {message}"),
            Self::MissingTargetCoordinate(axis) => {
                write!(formatter, "target profile has no `{axis}` coordinate")
            }
            Self::InvalidXml { path, message } => {
                write!(formatter, "source `{path}` is invalid XML: {message}")
            }
            Self::InvalidMetadataEnvelope { path, message } => {
                write!(formatter, "metadata source `{path}` is invalid: {message}")
            }
            Self::ConfigurationPath { path } => write!(
                formatter,
                "Configuration metadata must be the exact Configuration.xml source, got `{path}`"
            ),
            Self::ConfigurationCount { actual } => write!(
                formatter,
                "source tree must contain exactly one Configuration.xml, found {actual}"
            ),
            Self::InvalidConfiguration(message) => {
                write!(
                    formatter,
                    "Configuration.xml cannot be bootstrapped: {message}"
                )
            }
            Self::DuplicateConfigurationChild { family, name } => write!(
                formatter,
                "Configuration contains duplicate top-level `{family}.{name}`"
            ),
            Self::ConfigurationInventoryMismatch { missing, extra } => write!(
                formatter,
                "Configuration ChildObjects inventory mismatch: missing [{}], extra [{}]",
                missing.join(", "),
                extra.join(", ")
            ),
            Self::UnsupportedMetadataFamily { path, family } => write!(
                formatter,
                "metadata source `{path}` uses unsupported family `{family}`"
            ),
            Self::UnconsumedSource { path, kind } => write!(
                formatter,
                "source `{path}` ({kind:?}) has no explicit bootstrap route"
            ),
            Self::AmbiguousSourceRoute { path, candidates } => write!(
                formatter,
                "source `{path}` matches multiple bootstrap routes: {}",
                candidates.join(", ")
            ),
            Self::UnsupportedAssetCodec {
                path,
                family,
                codec,
            } => write!(
                formatter,
                "source `{path}` for `{family}` selects unsupported bootstrap asset codec {codec:?}"
            ),
            Self::Asset { path, message } => {
                write!(
                    formatter,
                    "source asset `{path}` cannot be compiled: {message}"
                )
            }
            Self::Profile { scope, message } => {
                write!(formatter, "cannot select {scope} profile: {message}")
            }
            Self::Canonical(message) => write!(formatter, "invalid canonical graph: {message}"),
            Self::Identity(message) => write!(formatter, "invalid bootstrap identities: {message}"),
            Self::Graph(message) => write!(formatter, "invalid bootstrap storage graph: {message}"),
            Self::Compiler {
                family,
                uuid,
                message,
            } => match uuid {
                Some(uuid) => write!(
                    formatter,
                    "{family} compiler rejected object {uuid}: {message}"
                ),
                None => write!(formatter, "{family} compiler failed: {message}"),
            },
            Self::DuplicateCompiledEntry { key } => {
                write!(
                    formatter,
                    "bootstrap compiler emitted duplicate entry `{key}`"
                )
            }
            Self::Patch(message) => write!(formatter, "invalid bootstrap patch: {message}"),
        }
    }
}

fn render_child(child: &ConfigurationChildReference) -> String {
    format!("{}.{}", child.family, child.name)
}

impl Error for BootstrapCompileError {}

#[cfg(test)]
mod tests {
    use ibcmd_core::artifact::ProfileId;
    use ibcmd_v8::format::Revision;
    use ibcmd_xml::source_tree::{SourceEntry, SourcePath};

    use crate::profile_registry::load_bundled_profile_registry;

    use super::*;

    const CONFIGURATION: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
  <Configuration uuid="10000000-0000-4000-8000-000000000001">
    <Properties>
      <Name>BootstrapFixture</Name>
      <Synonym><v8:item><v8:lang>en</v8:lang><v8:content>Bootstrap fixture</v8:content></v8:item></Synonym>
      <Comment>Clean-room full source tree</Comment>
      <DefaultRunMode>ManagedApplication</DefaultRunMode>
      <ScriptVariant>English</ScriptVariant>
      <CompatibilityMode>Version8_3_24</CompatibilityMode>
    </Properties>
    <ChildObjects><CommonModule>Portable</CommonModule></ChildObjects>
  </Configuration>
</MetaDataObject>"#;

    const COMMON_MODULE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
  <CommonModule uuid="20000000-0000-4000-8000-000000000001">
    <Properties>
      <Name>Portable</Name><Synonym/><Comment/>
      <Global>false</Global><ClientManagedApplication>false</ClientManagedApplication>
      <Server>true</Server><ExternalConnection>false</ExternalConnection>
      <ClientOrdinaryApplication>false</ClientOrdinaryApplication><ServerCall>false</ServerCall>
      <Privileged>false</Privileged><ReturnValuesReuse>DontUse</ReturnValuesReuse>
    </Properties>
  </CommonModule>
</MetaDataObject>"#;

    fn entry(path: &str, bytes: &[u8]) -> SourceEntry {
        SourceEntry::from_bytes(SourcePath::new(path).unwrap(), bytes.to_vec()).unwrap()
    }

    fn full_tree() -> SourceTree {
        SourceTree::new(vec![
            entry("Configuration.xml", CONFIGURATION.as_bytes()),
            entry("CommonModules/Portable.xml", COMMON_MODULE.as_bytes()),
            entry(
                "CommonModules/Portable/Ext/Module.bsl",
                b"Procedure Smoke() Export\nEndProcedure",
            ),
        ])
        .unwrap()
    }

    fn target_profile() -> EffectiveProfile {
        load_bundled_profile_registry()
            .unwrap()
            .get(&ProfileId::parse("platform-8.3.27.1989").unwrap())
            .unwrap()
            .clone()
    }

    #[test]
    fn complete_tree_compiles_to_exact_reachable_inventory() {
        let first = compile_bootstrap_source_tree(
            &full_tree(),
            XmlDialect::parse("2.20").unwrap(),
            &target_profile(),
        )
        .unwrap();
        let second = compile_bootstrap_source_tree(
            &full_tree(),
            XmlDialect::parse("2.20").unwrap(),
            &target_profile(),
        )
        .unwrap();
        assert_eq!(first.patch(), second.patch());
        assert_eq!(first.source_files(), 3);
        assert_eq!(first.metadata_files(), 2);
        assert_eq!(first.asset_files(), 1);
        assert_eq!(
            first
                .patch()
                .entries()
                .iter()
                .map(|entry| entry.target().key().as_str())
                .collect::<Vec<_>>(),
            [
                "10000000-0000-4000-8000-000000000001",
                "20000000-0000-4000-8000-000000000001",
                "20000000-0000-4000-8000-000000000001.0",
                "root",
                "version",
                "versions",
            ]
        );
        first.patch().preflight().unwrap();

        for revision in [Revision::Format15, Revision::Format16] {
            let artifact = ibcmd_cf::bootstrap::assemble_bootstrap_artifact(
                first.patch().clone(),
                ibcmd_cf::bootstrap::BootstrapCfProfile::new(
                    revision,
                    5,
                    first.storage_profile().clone(),
                ),
                ibcmd_core::limits::ResourceLimits::default(),
            )
            .unwrap();
            let mut bytes = Vec::new();
            ibcmd_cf::bootstrap::write_bootstrap_artifact(&mut bytes, &artifact).unwrap();
            ibcmd_cf::bootstrap::validate_bootstrap_artifact(
                std::io::Cursor::new(bytes),
                &artifact,
                ibcmd_core::limits::ResourceLimits::default(),
            )
            .unwrap();
        }
    }

    #[test]
    fn unsupported_or_unreferenced_input_never_produces_a_patch() {
        let mut sources = full_tree().entries().to_vec();
        sources.push(entry("unregistered.dat", b"opaque"));
        let error = compile_bootstrap_source_tree(
            &SourceTree::new(sources).unwrap(),
            XmlDialect::parse("2.20").unwrap(),
            &target_profile(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            BootstrapCompileError::UnconsumedSource { .. }
        ));

        let changed = CONFIGURATION.replace(
            "<ChildObjects><CommonModule>Portable</CommonModule></ChildObjects>",
            "<ChildObjects/>",
        );
        let tree = SourceTree::new(vec![
            entry("Configuration.xml", changed.as_bytes()),
            entry("CommonModules/Portable.xml", COMMON_MODULE.as_bytes()),
            entry(
                "CommonModules/Portable/Ext/Module.bsl",
                b"Procedure Smoke() Export\nEndProcedure",
            ),
        ])
        .unwrap();
        assert!(matches!(
            compile_bootstrap_source_tree(
                &tree,
                XmlDialect::parse("2.20").unwrap(),
                &target_profile(),
            ),
            Err(BootstrapCompileError::ConfigurationInventoryMismatch { .. })
        ));
    }

    #[test]
    fn unknown_configuration_property_fails_closed() {
        let changed = CONFIGURATION.replace(
            "<Comment>Clean-room full source tree</Comment>",
            "<Comment>Clean-room full source tree</Comment><FutureSetting>true</FutureSetting>",
        );
        let tree = SourceTree::new(vec![
            entry("Configuration.xml", changed.as_bytes()),
            entry("CommonModules/Portable.xml", COMMON_MODULE.as_bytes()),
            entry(
                "CommonModules/Portable/Ext/Module.bsl",
                b"Procedure Smoke() Export\nEndProcedure",
            ),
        ])
        .unwrap();
        assert!(matches!(
            compile_bootstrap_source_tree(
                &tree,
                XmlDialect::parse("2.20").unwrap(),
                &target_profile(),
            ),
            Err(BootstrapCompileError::InvalidConfiguration(_))
        ));
    }

    #[test]
    fn selected_xml_dialect_must_match_every_metadata_source() {
        assert!(matches!(
            compile_bootstrap_source_tree(
                &full_tree(),
                XmlDialect::parse("2.21").unwrap(),
                &target_profile(),
            ),
            Err(BootstrapCompileError::InvalidMetadataEnvelope { message, .. })
                if message.contains("incompatible with selected source profile")
        ));
    }
}
