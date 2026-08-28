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
    storage::{
        MultipartIdentity, StoragePatch, StoragePatchEntry, StoragePatchOutcome,
        StoragePatchTarget, StorageProvenance,
    },
    validate::{ValidatedConfiguration, validate_configuration},
    value::CanonicalValueKind,
    version::XmlDialect,
};
use ibcmd_schema::ConfigurationPropertyEvidencedDefault;
use ibcmd_xml::{
    AttributeKind, DialectDetection, DialectRegistry, XmlDocument, XmlElement, XmlNode, XmlReader,
    bundled_dialect_registry, bundled_metadata_registry,
    metadata::decode_configuration_envelope,
    source_tree::{SourceKind, SourceTree},
};

use super::{
    CompileAxes,
    bodies::form::{ManagedFormCodecProfile, compile_managed_form},
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
        form::{
            FORM_FAMILY, FormMetadataProfile, compile_form_metadata, decode_form_metadata_source,
        },
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

/// Heap-retention headroom the compiled patch gets over its own source bytes.
///
/// A compiled patch holds one deflated payload per storage row plus target keys
/// and provenance, so it is normally *smaller* than the XML it was compiled
/// from; the factor is deliberate slack for trees whose rows expand rather than
/// a licence to grow without bound. `MAX_STORAGE_PATCH_RETAINED_BYTES` stays
/// the floor, so nothing below it changes behaviour.
const BOOTSTRAP_PATCH_RETENTION_FACTOR: usize = 4;

/// Exact tree-root path of the one non-source document a native `config
/// export` tree carries.
const EXPORT_MANIFEST_PATH: &str = "ConfigDumpInfo.xml";
/// Root element local name of that document.
const EXPORT_MANIFEST_ROOT: &str = "ConfigDumpInfo";
/// Root element namespace of that document.  Deliberately distinct from the
/// `MDClasses`/`xcf` source namespaces, and the bundled dialect registry
/// already refuses to conflate it with any source dialect
/// (`ibcmd-xml/src/dialect.rs`,
/// `repository_xcf_roots_are_recognized_but_dumpinfo_is_not_conflated`).
const EXPORT_MANIFEST_NAMESPACE: &str = "http://v8.1c.ru/8.3/xcf/dumpinfo";

/// Owner-relative directory prefix holding one managed form's two documents.
const FORM_SOURCE_DIRECTORY: &str = "Forms/";
/// Owner-relative path of the form-body structure document.
const FORM_BODY_XML: &str = "Ext/Form.xml";
/// Owner-relative path of the form-body module, which the platform omits when
/// the form has no module at all.
const FORM_BODY_MODULE: &str = "Ext/Form/Module.bsl";
/// Storage suffix of the managed-form body row, read off our own `cf export`
/// report: key `<uuid>.0` produces exactly `Ext/Form/Module.bsl` and
/// `Ext/Form.xml`, while key `<uuid>` produces `Forms/<Name>.xml`.
const FORM_BODY_SUFFIX: &str = ".0";

/// Complete compiler result.  The patch owns every payload and can cross the
/// root-crate/CF-crate boundary without retaining XML documents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapCompilation {
    target_profile: ProfileId,
    storage_profile: StorageProfileId,
    source_files: usize,
    metadata_files: usize,
    asset_files: usize,
    non_source_files: usize,
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

    /// Source-tree files deliberately excluded from CF construction because
    /// they are export-side manifests rather than source documents.
    #[must_use]
    pub const fn non_source_files(&self) -> usize {
        self.non_source_files
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

/// One `Forms/<Name>.xml` document held back until every possible owner
/// document has been decoded.
#[derive(Clone, Debug)]
struct PendingFormSource {
    source_index: usize,
    path: String,
    object_path: ObjectPath,
}

/// The two source files a managed-form body row is compiled from.
#[derive(Clone, Debug)]
struct FormBodySource {
    form_uuid: ObjectUuid,
    /// Tree path of `Ext/Form.xml`, used as the address of any body refusal.
    form_xml_path: String,
    form_xml_index: usize,
    module_index: Option<usize>,
}

impl FormBodySource {
    const fn consumed_files(&self) -> usize {
        1 + if self.module_index.is_some() { 1 } else { 0 }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ConfigurationChildReference {
    family: String,
    name: String,
}

#[derive(Debug)]
struct ConfigurationProjection {
    properties: ConfigurationBodyProperties,
    children: Vec<ConfigurationChildReference>,
    /// Raw `<DefaultLanguage>` reference text (`Language.<name>`), resolved to
    /// an object UUID only once the canonical inventory exists.
    default_language: Option<String>,
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
    let mut non_source_indexes = BTreeSet::<usize>::new();
    let mut configuration = None::<ConfigurationProjection>;
    // Managed forms are decoded after this loop: a `Forms/<Name>.xml` document
    // never names its owning metadata object, so the ownership edge can only be
    // read off the tree layout once every owner document has been seen.
    let mut form_sources = Vec::<PendingFormSource>::new();

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
        if classify_export_manifest(&document, source.path().as_str())? {
            non_source_indexes.insert(source_index);
            continue;
        }
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

        if family == FORM_FAMILY {
            form_sources.push(PendingFormSource {
                source_index,
                path: source.path().as_str().to_owned(),
                object_path,
            });
            continue;
        }

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

    // Managed-form metadata, resolved against the owners collected above.
    for pending in &form_sources {
        let source = &tree.entries()[pending.source_index];
        let document = XmlReader::from_slice(source.bytes()).map_err(|error| {
            BootstrapCompileError::InvalidXml {
                path: pending.path.clone(),
                message: error.to_string(),
            }
        })?;
        let (owner_uuid, declared_name) = resolve_form_owner(&pending.path, &metadata_sources)?;
        let object = decode_form_metadata_source(
            &document,
            &source_profile,
            pending.object_path.clone(),
            owner_uuid,
        )
        .map_err(|error| BootstrapCompileError::InvalidMetadataEnvelope {
            path: pending.path.clone(),
            message: error.to_string(),
        })?;
        let name =
            object_name(&object).ok_or_else(|| BootstrapCompileError::InvalidMetadataEnvelope {
                path: pending.path.clone(),
                message: "Form has no textual Name".to_owned(),
            })?;
        if name != declared_name {
            return Err(BootstrapCompileError::FormNameMismatch {
                path: pending.path.clone(),
                expected: declared_name,
                actual: name.to_owned(),
            });
        }
        metadata_sources.push(MetadataSource {
            owner_directory: owner_directory(&pending.path, FORM_FAMILY),
            path: pending.path.clone(),
            family: FORM_FAMILY.to_owned(),
            uuid: object.identity().uuid(),
        });
        metadata_indexes.insert(pending.source_index);
        objects.push(object);
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
    let mut configuration = configuration.expect("configuration count proved a decoded projection");
    let canonical = CanonicalConfiguration::new(objects)
        .map_err(|source| BootstrapCompileError::Canonical(source.to_string()))?;
    validate_configuration_children(&canonical, &configuration.children)?;
    if let Some(reference) = configuration.default_language.as_deref() {
        configuration.properties.default_language = Some(resolve_configuration_default_language(
            &canonical, reference,
        )?);
    }
    let validated = validate_configuration(&canonical)
        .map_err(|source| BootstrapCompileError::Canonical(format!("{source:?}")))?;
    let identities = collect_bootstrap_identities(&validated)
        .map_err(|source| BootstrapCompileError::Identity(source.to_string()))?;

    // Metadata documents and excluded non-source documents are both already
    // accounted for; only the remainder may claim a source-asset route.
    let mut consumed_indexes = metadata_indexes;
    consumed_indexes.extend(non_source_indexes.iter().copied());
    // A managed-form body is one storage row fed by two source files, so it
    // cannot be expressed as a one-file source-asset route.  Claim those files
    // before the router runs, so the router keeps refusing everything it does
    // not recognise instead of silently absorbing a form file.
    let form_bodies = resolve_form_bodies(tree, &metadata_sources, &mut consumed_indexes)?;
    let assets = resolve_assets(tree, &metadata_sources, &consumed_indexes)?;
    let mut suffixes = BTreeMap::<ObjectUuid, Vec<StorageSuffix>>::new();
    for asset in &assets {
        suffixes.entry(asset.owner_uuid).or_default().push(
            StorageSuffix::new(asset.route.suffix())
                .map_err(|source| BootstrapCompileError::Graph(source.to_string()))?,
        );
    }
    for body in &form_bodies {
        suffixes.entry(body.form_uuid).or_default().push(
            StorageSuffix::new(FORM_BODY_SUFFIX)
                .map_err(|source| BootstrapCompileError::Graph(source.to_string()))?,
        );
    }
    for values in suffixes.values_mut() {
        values.sort();
        values.dedup();
    }
    // Forms are the one owned family that still occupies its own storage rows,
    // so ownership alone no longer decides who gets a route.
    let routes = identities
        .objects()
        .iter()
        .filter(|identity| identity.owner().is_none() || identity.kind().as_str() == FORM_FAMILY)
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
    if !form_bodies.is_empty() {
        let selected = ManagedFormCodecProfile::from_effective(target_profile)
            .map_err(|error| profile_error("managed form body", error))?;
        let suffix = StorageSuffix::new(FORM_BODY_SUFFIX)
            .map_err(|source| BootstrapCompileError::Graph(source.to_string()))?;
        for body in &form_bodies {
            let entry = compile_form_body(tree, &graph, body, &suffix, &selected)?;
            insert_compiled(&mut compiled, entry)?;
        }
    }

    // The heap-retention ceiling for the compiled patch follows the size of the
    // source tree that produced it, which this function already holds, instead
    // of a fixed constant that has no way to know how large the input was. The
    // constant remains the floor, so small trees behave exactly as before.
    let patch_retained_budget = tree
        .entries()
        .iter()
        .map(|entry| entry.bytes().len())
        .sum::<usize>()
        .saturating_mul(BOOTSTRAP_PATCH_RETENTION_FACTOR);
    let without_versions = StoragePatch::with_retained_byte_limit(
        compiled.into_values().collect(),
        patch_retained_budget,
    )
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
    let patch = StoragePatch::with_retained_byte_limit(final_entries, patch_retained_budget)
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
        // Form-body files are payload sources consumed into a storage row just
        // like routed assets are, so they stay inside `asset_files` and keep
        // `source_files == metadata + asset + non_source` true.
        asset_files: assets.len()
            + form_bodies
                .iter()
                .map(FormBodySource::consumed_files)
                .sum::<usize>(),
        non_source_files: non_source_indexes.len(),
        patch,
    })
}

/// Fail-closed classification of the single non-source document a native
/// `config export` tree carries: `ConfigDumpInfo.xml`.
///
/// That file is an inventory manifest of the dump, not a `MetaDataObject`
/// document, and it has no counterpart record inside a CF container — verified
/// with `cf inspect` against all three retained native corpora, whose storage
/// elements are exclusively per-object uuid keys plus `root`/`version`/
/// `versions`.  Feeding it to the metadata route therefore cannot ever be
/// right: it is not part of what a CF stores, so it must be excluded both from
/// dialect detection and from the source-asset router.
///
/// Two independent pieces of evidence must agree before anything is excluded:
/// the exact tree-root path, and the document's own root element identity
/// (local name plus resolved namespace).  Either one alone matching is a hard
/// error rather than a skip:
///
/// * the reserved path holding a foreign root element means real content is
///   parked under that name, and dropping it silently is exactly the swallow
///   this gate exists to prevent;
/// * the manifest root element at any other path is a tree shape this compiler
///   has no evidence for, so it refuses to guess which of the two signals to
///   believe.
///
/// Rejecting instead by the weaker rule "any XML whose root is not
/// `MetaDataObject`" was considered and deliberately not chosen: it would turn
/// every future unrecognized root — including genuine metadata this build does
/// not yet parse — into a silent skip, and the resulting CF would be quietly
/// incomplete.  Under the rule implemented here, unrecognized XML keeps its
/// existing route and still fails, either in dialect detection or as
/// `UnconsumedSource`.
fn classify_export_manifest(
    document: &XmlDocument,
    path: &str,
) -> Result<bool, BootstrapCompileError> {
    let reserved_path = path == EXPORT_MANIFEST_PATH;
    let root = document.root();
    let manifest_root = root.name().local() == EXPORT_MANIFEST_ROOT
        && root_namespace(root) == Some(EXPORT_MANIFEST_NAMESPACE);
    match (reserved_path, manifest_root) {
        (true, true) => Ok(true),
        (false, false) => Ok(false),
        (true, false) => Err(BootstrapCompileError::NonSourceDocumentMismatch {
            path: path.to_owned(),
            message: format!(
                "`{EXPORT_MANIFEST_PATH}` is reserved for the export inventory manifest, but this \
                 document's root element is `{}`; it is neither excluded nor compiled",
                root.name().raw()
            ),
        }),
        (false, true) => Err(BootstrapCompileError::NonSourceDocumentMismatch {
            path: path.to_owned(),
            message: format!(
                "export inventory manifest root `{{{EXPORT_MANIFEST_NAMESPACE}}}{EXPORT_MANIFEST_ROOT}` \
                 is only evidenced at the tree root path `{EXPORT_MANIFEST_PATH}`"
            ),
        }),
    }
}

/// Namespace URI bound to an element's own name, resolved from the element's
/// own declarations.  A source-tree root element always carries its own
/// bindings, so no ancestor scope can apply here.
fn root_namespace(element: &XmlElement) -> Option<&str> {
    let prefix = element.name().prefix();
    element
        .attributes()
        .iter()
        .find_map(|attribute| match attribute.kind() {
            AttributeKind::Namespace(declared) if declared.as_deref() == prefix => {
                Some(attribute.value())
            }
            _ => None,
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
        FORM_FAMILY => select_compile!(FormMetadataProfile, compile_form_metadata),
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
    consumed_indexes: &BTreeSet<usize>,
) -> Result<Vec<AssetSource>, BootstrapCompileError> {
    let registry = SourceAssetRegistry;
    let mut assets = Vec::new();
    for (source_index, source) in tree.entries().iter().enumerate() {
        if consumed_indexes.contains(&source_index) {
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

/// Resolves the metadata object that owns one `Forms/<Name>.xml` document.
///
/// The document itself carries no owner reference — in a native `config export`
/// tree the edge exists only as the file's position under the owner's directory
/// — so it is read from the tree layout and must resolve to exactly one owner.
/// Returning the name segment as well lets the caller prove the document's own
/// `Name` and the file name it is filed under agree instead of trusting either
/// one alone.
fn resolve_form_owner(
    path: &str,
    metadata: &[MetadataSource],
) -> Result<(ObjectUuid, String), BootstrapCompileError> {
    let mut matches = Vec::<(ObjectUuid, String)>::new();
    for owner in metadata {
        // A form never owns a form; skipping them keeps a nested `Forms/`
        // directory from resolving to two owners at once.
        if owner.family == FORM_FAMILY {
            continue;
        }
        let Some(relative) = relative_to_owner(path, &owner.owner_directory) else {
            continue;
        };
        let Some(name) = relative
            .strip_prefix(FORM_SOURCE_DIRECTORY)
            .and_then(|rest| rest.strip_suffix(".xml"))
        else {
            continue;
        };
        if name.is_empty() || name.contains('/') {
            continue;
        }
        matches.push((owner.uuid, name.to_owned()));
    }
    matches.sort();
    matches.dedup();
    match matches.as_slice() {
        [] => Err(BootstrapCompileError::UnownedForm {
            path: path.to_owned(),
        }),
        [single] => Ok(single.clone()),
        _ => Err(BootstrapCompileError::AmbiguousSourceRoute {
            path: path.to_owned(),
            candidates: matches
                .iter()
                .map(|(uuid, name)| format!("{uuid}:{name}"))
                .collect(),
        }),
    }
}

/// Claims the two source files behind each managed-form body row.
///
/// `Ext/Form.xml` is mandatory: a form metadata record without a body row would
/// leave `<uuid>.0` missing from the storage inventory, and inventing an empty
/// body is exactly the silent guess this compiler refuses to make.
/// `Ext/Form/Module.bsl` is optional because the platform omits it for a form
/// that has no module.
fn resolve_form_bodies(
    tree: &SourceTree,
    metadata: &[MetadataSource],
    consumed_indexes: &mut BTreeSet<usize>,
) -> Result<Vec<FormBodySource>, BootstrapCompileError> {
    let index_by_path = tree
        .entries()
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.path().as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut bodies = Vec::new();
    for owner in metadata
        .iter()
        .filter(|source| source.family == FORM_FAMILY)
    {
        let form_xml_path = format!("{}/{FORM_BODY_XML}", owner.owner_directory);
        let module_path = format!("{}/{FORM_BODY_MODULE}", owner.owner_directory);
        let form_xml_index = index_by_path.get(form_xml_path.as_str()).copied().ok_or(
            BootstrapCompileError::MissingFormBody {
                path: owner.path.clone(),
                expected: form_xml_path.clone(),
            },
        )?;
        let module_index = index_by_path.get(module_path.as_str()).copied();
        consumed_indexes.insert(form_xml_index);
        if let Some(index) = module_index {
            consumed_indexes.insert(index);
        }
        bodies.push(FormBodySource {
            form_uuid: owner.uuid,
            form_xml_path,
            form_xml_index,
            module_index,
        });
    }
    Ok(bodies)
}

/// Compiles one managed-form body row through the existing base-free packer.
///
/// `crate::compiler::bodies::form` holds the only managed-form compiler in this
/// build; this function supplies it with the two source files and turns its
/// refusals into an addressed bootstrap blocker rather than a second packer.
fn compile_form_body(
    tree: &SourceTree,
    graph: &BootstrapGraph,
    body: &FormBodySource,
    suffix: &StorageSuffix,
    profile: &ManagedFormCodecProfile,
) -> Result<StoragePatchEntry, BootstrapCompileError> {
    let form_xml = tree.entries()[body.form_xml_index].bytes();
    let module = body.module_index.map(|index| tree.entries()[index].bytes());
    let bytes = compile_managed_form(profile, form_xml, module, None).map_err(|error| {
        BootstrapCompileError::FormBody {
            path: body.form_xml_path.clone(),
            message: error.to_string(),
        }
    })?;
    let target = graph.object_entry(body.form_uuid, suffix).ok_or_else(|| {
        BootstrapCompileError::FormBody {
            path: body.form_xml_path.clone(),
            message: format!(
                "bootstrap graph has no `{}{FORM_BODY_SUFFIX}` row",
                body.form_uuid
            ),
        }
    })?;
    let provenance = StorageProvenance::new(&format!(
        "bootstrap:{}:body:Form:{FORM_BODY_XML}",
        profile.profile_id()
    ))
    .map_err(|error| BootstrapCompileError::FormBody {
        path: body.form_xml_path.clone(),
        message: error.to_string(),
    })?;
    let outcome =
        StoragePatchOutcome::compiled(bytes).map_err(|error| BootstrapCompileError::FormBody {
            path: body.form_xml_path.clone(),
            message: error.to_string(),
        })?;
    Ok(StoragePatchEntry::new(
        StoragePatchTarget::new(
            target.key().clone(),
            MultipartIdentity::single(),
            provenance,
        ),
        outcome,
    ))
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
    let policy = ibcmd_schema::configuration_properties_evidenced_default_block_policy();
    let mut properties = ConfigurationBodyProperties::minimal("", target_compatibility);
    let mut default_language = None::<String>;
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

            // -- the six coordinates MINI-GATE-A-CONFIG-PROPS-01 proved by
            // single-field isolation, read here in the load direction --
            "IncludeHelpInContents" => {
                let value = simple_text(element)?;
                properties
                    .evidenced_property_digits
                    .include_help_in_contents = Some(evidenced_digit(
                    name,
                    &value,
                    policy.include_help_in_contents_digit(&value),
                )?);
            }
            "UseManagedFormInOrdinaryApplication" => {
                let value = simple_text(element)?;
                properties
                    .evidenced_property_digits
                    .use_managed_form_in_ordinary_application = Some(evidenced_digit(
                    name,
                    &value,
                    policy.use_managed_form_in_ordinary_application_digit(&value),
                )?);
            }
            "UseOrdinaryFormInManagedApplication" => {
                let value = simple_text(element)?;
                properties
                    .evidenced_property_digits
                    .use_ordinary_form_in_managed_application = Some(evidenced_digit(
                    name,
                    &value,
                    policy.use_ordinary_form_in_managed_application_digit(&value),
                )?);
            }
            "ModalityUseMode" => {
                let value = simple_text(element)?;
                properties.evidenced_property_digits.modality_use_mode = Some(evidenced_digit(
                    name,
                    &value,
                    policy.modality_use_mode_digit(&value),
                )?);
            }
            "InterfaceCompatibilityMode" => {
                let value = simple_text(element)?;
                properties
                    .evidenced_property_digits
                    .interface_compatibility_mode = Some(evidenced_digit(
                    name,
                    &value,
                    policy.interface_compatibility_mode_digit(&value),
                )?);
            }
            "SynchronousPlatformExtensionAndAddInCallUseMode" => {
                let value = simple_text(element)?;
                properties
                    .evidenced_property_digits
                    .synchronous_platform_extension_and_add_in_call_use_mode =
                    Some(evidenced_digit(
                        name,
                        &value,
                        policy
                            .synchronous_platform_extension_and_add_in_call_use_mode_digit(&value),
                    )?);
            }

            // -- properties whose config-body slot this compiler already
            // fills, corroborated by the evidenced reference tuple --
            "UsePurposes" => properties.use_platform_application = use_purposes(element)?,
            "DefaultLanguage" => {
                let reference = simple_text(element)?;
                default_language = (!reference.is_empty()).then_some(reference);
            }
            "CommonSettingsStorage"
            | "ReportsUserSettingsStorage"
            | "ReportsVariantsStorage"
            | "FormDataSettingsStorage" => {
                // Tuple fields 22..=25 are all-nil in every evidenced corpus,
                // so only the empty reference has a proven encoding.
                reject_unless_empty(name, element)?;
            }
            "UsedMobileApplicationFunctionalities" => {
                reject_unless_evidenced_mobile_default(element)?;
                properties.enabled_mobile_functionalities = policy
                    .used_mobile_application_functionalities_default_tuple_ids()
                    .to_vec();
            }
            "DefaultStyle" => {
                reject_unless_evidenced_default(name, element)?;
                properties.default_style = None;
            }
            "DefaultRoles" => {
                reject_unless_evidenced_default(name, element)?;
                properties.default_roles.clear();
            }

            // -- everything else the evidenced reference covers: the exact
            // platform default is compilable (its bytes are proven), any
            // other value is refused because nothing proves where it lives --
            evidenced if policy.evidenced_default_property(evidenced).is_some() => {
                reject_unless_evidenced_default(name, element)?;
            }

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
        default_language,
    })
}

/// Turns one proven lexeme into the config-body byte the evidenced value map
/// assigns it. A lexeme no evidenced corpus ever produced is refused: the
/// platform's own enumerations are wider than the corpus, and guessing at an
/// unobserved index would write a byte nothing proves.
fn evidenced_digit(
    property: &str,
    value: &str,
    digit: Option<u8>,
) -> Result<u8, BootstrapCompileError> {
    digit.ok_or_else(
        || BootstrapCompileError::ConfigurationPropertyValueOutsideEvidencedMap {
            property: property.to_owned(),
            value: value.to_owned(),
        },
    )
}

fn property_not_projectable(property: &str, value: String) -> BootstrapCompileError {
    BootstrapCompileError::ConfigurationPropertyValueNotProjectable {
        property: property.to_owned(),
        value,
    }
}

/// What one Configuration `<Properties>` element actually carries, without
/// assuming which of the three shapes it is.
enum PropertyContent {
    /// No element children and no non-whitespace text.
    Empty,
    /// No element children; the exact concatenated character data.
    Text(String),
    /// At least one element child.
    Markup,
}

fn property_content(element: &XmlElement) -> PropertyContent {
    let mut text = String::new();
    for node in element.children() {
        match node {
            XmlNode::Element(_) => return PropertyContent::Markup,
            XmlNode::Text(value) => text.push_str(value.value()),
            XmlNode::CData(value) => text.push_str(value.value()),
            _ => {}
        }
    }
    if text.trim().is_empty() {
        PropertyContent::Empty
    } else {
        PropertyContent::Text(text)
    }
}

/// Accepts only a content-free element.
fn reject_unless_empty(property: &str, element: &XmlElement) -> Result<(), BootstrapCompileError> {
    match property_content(element) {
        PropertyContent::Empty => Ok(()),
        PropertyContent::Text(text) => Err(property_not_projectable(property, text)),
        PropertyContent::Markup => Err(property_not_projectable(
            property,
            "<non-empty reference>".to_owned(),
        )),
    }
}

/// Accepts only the exact platform default the evidenced all-default
/// reference proves for this property; every other value is a coordinate this
/// compiler has no evidence for and must refuse rather than approximate.
fn reject_unless_evidenced_default(
    property: &str,
    element: &XmlElement,
) -> Result<(), BootstrapCompileError> {
    let policy = ibcmd_schema::configuration_properties_evidenced_default_block_policy();
    match policy.evidenced_default_property(property) {
        Some(ConfigurationPropertyEvidencedDefault::Empty) => {
            reject_unless_empty(property, element)
        }
        Some(ConfigurationPropertyEvidencedDefault::Text(expected)) => {
            match property_content(element) {
                PropertyContent::Text(text) if text == expected => Ok(()),
                PropertyContent::Text(text) => Err(property_not_projectable(property, text)),
                PropertyContent::Empty => Err(property_not_projectable(property, String::new())),
                PropertyContent::Markup => Err(property_not_projectable(
                    property,
                    "<nested markup>".to_owned(),
                )),
            }
        }
        Some(ConfigurationPropertyEvidencedDefault::Block(_)) | None => Err(
            property_not_projectable(property, "<unclassified default>".to_owned()),
        ),
    }
}

/// `<UsedMobileApplicationFunctionalities>` is compilable only as the exact
/// all-default block: the evidenced reference tuple proves which numeric IDs
/// that block corresponds to, and nothing proves any other combination.
fn reject_unless_evidenced_mobile_default(
    element: &XmlElement,
) -> Result<(), BootstrapCompileError> {
    const PROPERTY: &str = "UsedMobileApplicationFunctionalities";
    let policy = ibcmd_schema::configuration_properties_evidenced_default_block_policy();
    let mut actual = Vec::new();
    for entry in child_elements(element)? {
        if entry.name().local() != "functionality" {
            return Err(property_not_projectable(
                PROPERTY,
                format!("<unexpected `{}` entry>", entry.name().local()),
            ));
        }
        let name = named_child(entry, "functionality")?
            .ok_or_else(|| property_not_projectable(PROPERTY, "<entry without a name>".to_owned()))
            .and_then(simple_text)?;
        let used = named_child(entry, "use")?
            .ok_or_else(|| property_not_projectable(PROPERTY, "<entry without a use>".to_owned()))
            .and_then(simple_text)?;
        let used = match used.as_str() {
            "true" => true,
            "false" => false,
            other => {
                return Err(property_not_projectable(
                    PROPERTY,
                    format!("<{name} uses `{other}`>"),
                ));
            }
        };
        actual.push((name, used));
    }
    let expected = policy.used_mobile_application_functionality_defaults();
    if actual.len() != expected.len()
        || actual
            .iter()
            .zip(expected)
            .any(|((name, used), (expected_name, expected_used))| {
                name != expected_name || used != expected_used
            })
    {
        return Err(property_not_projectable(
            PROPERTY,
            "<block differs from the evidenced platform default>".to_owned(),
        ));
    }
    Ok(())
}

/// `<UsePurposes>` is compilable only as the single `PlatformApplication`
/// value every evidenced corpus carries (config-body tuple field 33).
fn use_purposes(element: &XmlElement) -> Result<bool, BootstrapCompileError> {
    const PROPERTY: &str = "UsePurposes";
    let values = child_elements(element)?;
    let purposes = values
        .iter()
        .map(|value| {
            if value.name().local() != "Value" {
                return Err(property_not_projectable(
                    PROPERTY,
                    format!("<unexpected `{}` entry>", value.name().local()),
                ));
            }
            simple_text(value)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if purposes.len() == 1 && purposes[0] == "PlatformApplication" {
        Ok(true)
    } else {
        Err(property_not_projectable(PROPERTY, purposes.join(", ")))
    }
}

/// Resolves a `<DefaultLanguage>` reference against the tree's own inventory.
/// The evidenced reference tuple carries the referenced Language object's
/// UUID in config-body tuple field 10 (`dcs-area-style-item-uuid` names
/// `Language.Русский`, whose object UUID is exactly the value that field
/// holds), which is what this compiler already emits there.
fn resolve_configuration_default_language(
    canonical: &CanonicalConfiguration,
    reference: &str,
) -> Result<ObjectUuid, BootstrapCompileError> {
    let name = reference
        .strip_prefix("Language.")
        .ok_or_else(|| property_not_projectable("DefaultLanguage", reference.to_owned()))?;
    canonical
        .objects()
        .iter()
        .find(|object| {
            object.owner().is_none()
                && object.kind().as_str() == "Language"
                && object_name(object) == Some(name)
        })
        .map(|object| object.identity().uuid())
        .ok_or_else(|| {
            BootstrapCompileError::InvalidConfiguration(format!(
                "Configuration DefaultLanguage names `{reference}`, which the source tree does not contain"
            ))
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
    NonSourceDocumentMismatch {
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
    /// A Configuration property whose value differs from the platform default
    /// the evidenced all-default config-body reference proves, and whose
    /// non-default form has no corpus-proven config-body coordinate.
    ConfigurationPropertyValueNotProjectable {
        property: String,
        value: String,
    },
    /// A Configuration property that *does* have a corpus-proven config-body
    /// coordinate, carrying a lexeme no evidenced corpus ever mapped to a
    /// byte at that coordinate.
    ConfigurationPropertyValueOutsideEvidencedMap {
        property: String,
        value: String,
    },
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
    /// A `Forms/<Name>.xml` document sits where no metadata object owns it.
    UnownedForm {
        path: String,
    },
    /// A form's own `Name` disagrees with the file name the tree filed it under.
    FormNameMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    /// A form metadata document has no body document beside it.
    MissingFormBody {
        path: String,
        expected: String,
    },
    /// The base-free managed-form packer refused this form's body.
    FormBody {
        path: String,
        message: String,
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

impl BootstrapCompileError {
    /// Stable machine-readable discriminator, one per variant, so a JSON report
    /// can carry *which* structural rule rejected the tree rather than only a
    /// rendered sentence.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::SourceTree(_) => "source_tree_invalid",
            Self::SourceProfile(_) => "source_profile_invalid",
            Self::MissingTargetCoordinate(_) => "missing_target_coordinate",
            Self::InvalidXml { .. } => "invalid_xml",
            Self::InvalidMetadataEnvelope { .. } => "invalid_metadata_envelope",
            Self::NonSourceDocumentMismatch { .. } => "non_source_document_mismatch",
            Self::ConfigurationPath { .. } => "configuration_path",
            Self::ConfigurationCount { .. } => "configuration_count",
            Self::InvalidConfiguration(_) => "invalid_configuration",
            Self::ConfigurationPropertyValueNotProjectable { .. } => {
                "configuration_property_value_not_projectable"
            }
            Self::ConfigurationPropertyValueOutsideEvidencedMap { .. } => {
                "configuration_property_value_outside_evidenced_map"
            }
            Self::DuplicateConfigurationChild { .. } => "duplicate_configuration_child",
            Self::ConfigurationInventoryMismatch { .. } => "configuration_inventory_mismatch",
            Self::UnsupportedMetadataFamily { .. } => "unsupported_metadata_family",
            Self::UnownedForm { .. } => "form_owner_unresolved",
            Self::FormNameMismatch { .. } => "form_name_mismatch",
            Self::MissingFormBody { .. } => "form_body_missing",
            Self::FormBody { .. } => "form_body_compile_failed",
            Self::UnconsumedSource { .. } => "unconsumed_source",
            Self::AmbiguousSourceRoute { .. } => "ambiguous_source_route",
            Self::UnsupportedAssetCodec { .. } => "unsupported_asset_codec",
            Self::Asset { .. } => "asset_compile_failed",
            Self::Profile { .. } => "profile_selection_failed",
            Self::Canonical(_) => "canonical_graph_invalid",
            Self::Identity(_) => "identity_graph_invalid",
            Self::Graph(_) => "storage_graph_invalid",
            Self::Compiler { .. } => "family_compiler_rejected",
            Self::DuplicateCompiledEntry { .. } => "duplicate_compiled_entry",
            Self::Patch(_) => "patch_invalid",
        }
    }

    /// Source-tree path this failure is attributable to, when it is attributable
    /// to one file.  Answers *where* without re-parsing the rendered message.
    #[must_use]
    pub fn source_path(&self) -> Option<&str> {
        match self {
            Self::InvalidXml { path, .. }
            | Self::InvalidMetadataEnvelope { path, .. }
            | Self::NonSourceDocumentMismatch { path, .. }
            | Self::ConfigurationPath { path }
            | Self::UnsupportedMetadataFamily { path, .. }
            | Self::UnownedForm { path }
            | Self::FormNameMismatch { path, .. }
            | Self::MissingFormBody { path, .. }
            | Self::FormBody { path, .. }
            | Self::UnconsumedSource { path, .. }
            | Self::AmbiguousSourceRoute { path, .. }
            | Self::UnsupportedAssetCodec { path, .. }
            | Self::Asset { path, .. } => Some(path.as_str()),
            _ => None,
        }
    }

    /// What the compiler required, for the variants that compare two inventories
    /// or two counts.
    #[must_use]
    pub fn expected(&self) -> Option<String> {
        match self {
            Self::ConfigurationCount { .. } => Some("1".to_owned()),
            Self::ConfigurationInventoryMismatch { missing, .. } if !missing.is_empty() => {
                Some(format!("missing: {}", missing.join(", ")))
            }
            Self::ConfigurationPropertyValueNotProjectable { property, .. } => {
                Some(format!("the evidenced platform default for `{property}`"))
            }
            Self::ConfigurationPropertyValueOutsideEvidencedMap { property, .. } => {
                Some(format!("a corpus-evidenced lexeme for `{property}`"))
            }
            Self::FormNameMismatch { expected, .. } | Self::MissingFormBody { expected, .. } => {
                Some(expected.clone())
            }
            _ => None,
        }
    }

    /// What the compiler actually observed, paired with [`Self::expected`].
    #[must_use]
    pub fn actual(&self) -> Option<String> {
        match self {
            Self::ConfigurationCount { actual } => Some(actual.to_string()),
            Self::ConfigurationInventoryMismatch { extra, .. } if !extra.is_empty() => {
                Some(format!("extra: {}", extra.join(", ")))
            }
            Self::AmbiguousSourceRoute { candidates, .. } => Some(candidates.join(", ")),
            Self::UnsupportedMetadataFamily { family, .. } => Some(family.clone()),
            Self::ConfigurationPropertyValueNotProjectable { value, .. }
            | Self::ConfigurationPropertyValueOutsideEvidencedMap { value, .. } => {
                Some(value.clone())
            }
            Self::FormNameMismatch { actual, .. } => Some(actual.clone()),
            Self::FormBody { message, .. } => Some(message.clone()),
            _ => None,
        }
    }
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
            Self::NonSourceDocumentMismatch { path, message } => {
                write!(
                    formatter,
                    "source `{path}` cannot be classified as source or non-source: {message}"
                )
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
            Self::ConfigurationPropertyValueNotProjectable { property, value } => write!(
                formatter,
                "Configuration property `{property}` carries `{value}`, which is not the platform \
                 default proven by the evidenced config-body reference and has no proven \
                 base-free projection"
            ),
            Self::ConfigurationPropertyValueOutsideEvidencedMap { property, value } => write!(
                formatter,
                "Configuration property `{property}` carries `{value}`, which no evidenced corpus \
                 maps to a config-body byte at its proven coordinate"
            ),
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
            Self::UnownedForm { path } => write!(
                formatter,
                "Form source `{path}` is not filed under any metadata object's `Forms/` directory"
            ),
            Self::FormNameMismatch {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "Form source `{path}` is filed as `{expected}` but declares Name `{actual}`"
            ),
            Self::MissingFormBody { path, expected } => write!(
                formatter,
                "Form source `{path}` has no body document `{expected}`"
            ),
            Self::FormBody { path, message } => write!(
                formatter,
                "Form body `{path}` cannot be compiled base-free: {message}"
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

    /// The three mini-parity corpora whose retained `Configuration.xml` the
    /// export direction already reproduces byte-for-byte (commit 8defc4c):
    /// two all-default trees whose Configuration headers differ in length,
    /// and one that carries three non-default evidenced enum values.
    const T1_ALL_DEFAULT_NATIVE_XML_B64: &str = include_str!(
        "../../tests/fixtures/native-evidence/8.3.27.2214/dcs-area-style-item-uuid/native-configuration.xml.b64"
    );
    const T2_LONGER_HEADER_NATIVE_XML_B64: &str = include_str!(
        "../../tests/fixtures/native-evidence/8.3.27.2214/dcs-form-list-settings-server-state/native-configuration.xml.b64"
    );
    const T3_ENUM_GROUP_NATIVE_XML_B64: &str = include_str!(
        "../../tests/fixtures/native-evidence/8.3.27.2214/configuration-properties-enum-group/native-configuration.xml.b64"
    );

    fn native_configuration_xml(encoded: &str) -> String {
        String::from_utf8(crate::module_blob::decode_base64_mime(encoded.trim()).unwrap()).unwrap()
    }

    fn project_native(encoded: &str) -> Result<ConfigurationProjection, BootstrapCompileError> {
        project_native_text(&native_configuration_xml(encoded))
    }

    fn project_native_text(xml: &str) -> Result<ConfigurationProjection, BootstrapCompileError> {
        let document = XmlReader::from_slice(xml.as_bytes()).unwrap();
        project_configuration(&document, 80_327)
    }

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

    const EXPORT_MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ConfigDumpInfo xmlns="http://v8.1c.ru/8.3/xcf/dumpinfo" xmlns:xen="http://v8.1c.ru/8.3/xcf/enums" format="Hierarchical" version="2.20">
  <ConfigVersions>
    <Metadata name="Configuration.BootstrapFixture" id="10000000-0000-4000-8000-000000000001" configVersion="00"/>
  </ConfigVersions>
</ConfigDumpInfo>"#;

    #[test]
    fn export_inventory_manifest_is_excluded_without_reaching_any_route() {
        let mut sources = full_tree().entries().to_vec();
        sources.push(entry("ConfigDumpInfo.xml", EXPORT_MANIFEST.as_bytes()));
        let compilation = compile_bootstrap_source_tree(
            &SourceTree::new(sources).unwrap(),
            XmlDialect::parse("2.20").unwrap(),
            &target_profile(),
        )
        .unwrap();
        assert_eq!(compilation.source_files(), 4);
        assert_eq!(compilation.metadata_files(), 2);
        assert_eq!(compilation.asset_files(), 1);
        assert_eq!(compilation.non_source_files(), 1);
        // The excluded manifest contributes nothing to the container, matching
        // `cf inspect` on the retained native corpora.
        assert_eq!(compilation.patch(), full_compilation().patch());
    }

    fn full_compilation() -> BootstrapCompilation {
        compile_bootstrap_source_tree(
            &full_tree(),
            XmlDialect::parse("2.20").unwrap(),
            &target_profile(),
        )
        .unwrap()
    }

    #[test]
    fn reserved_manifest_path_holding_other_content_is_rejected_not_skipped() {
        let mut sources = full_tree().entries().to_vec();
        sources.push(entry(
            "ConfigDumpInfo.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20"/>"#,
        ));
        let error = compile_bootstrap_source_tree(
            &SourceTree::new(sources).unwrap(),
            XmlDialect::parse("2.20").unwrap(),
            &target_profile(),
        )
        .unwrap_err();
        assert!(
            matches!(
                &error,
                BootstrapCompileError::NonSourceDocumentMismatch { path, .. }
                    if path == "ConfigDumpInfo.xml"
            ),
            "{error}"
        );
        assert_eq!(error.code(), "non_source_document_mismatch");
        assert_eq!(error.source_path(), Some("ConfigDumpInfo.xml"));
    }

    #[test]
    fn manifest_root_outside_the_tree_root_path_is_rejected_not_skipped() {
        let mut sources = full_tree().entries().to_vec();
        sources.push(entry(
            "CommonModules/ConfigDumpInfo.xml",
            EXPORT_MANIFEST.as_bytes(),
        ));
        let error = compile_bootstrap_source_tree(
            &SourceTree::new(sources).unwrap(),
            XmlDialect::parse("2.20").unwrap(),
            &target_profile(),
        )
        .unwrap_err();
        assert!(
            matches!(
                &error,
                BootstrapCompileError::NonSourceDocumentMismatch { path, .. }
                    if path == "CommonModules/ConfigDumpInfo.xml"
            ),
            "{error}"
        );
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

    // ---------------------------------------------------------------------
    // Managed forms
    // ---------------------------------------------------------------------

    const FORM_CONFIGURATION: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
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
    <ChildObjects><SettingsStorage>Holder</SettingsStorage></ChildObjects>
  </Configuration>
</MetaDataObject>"#;

    const FORM_OWNER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
  <SettingsStorage uuid="20000000-0000-4000-8000-000000000001">
    <InternalInfo>
      <xr:GeneratedType name="SettingsStorageManager.Holder" category="Manager">
        <xr:TypeId>21000000-0000-4000-8000-000000000001</xr:TypeId>
        <xr:ValueId>22000000-0000-4000-8000-000000000001</xr:ValueId>
      </xr:GeneratedType>
    </InternalInfo>
    <Properties>
      <Name>Holder</Name><Synonym/><Comment/>
      <DefaultSaveForm/><DefaultLoadForm/><AuxiliarySaveForm/><AuxiliaryLoadForm/>
    </Properties>
    <ChildObjects><Form>SaveForm</Form></ChildObjects>
  </SettingsStorage>
</MetaDataObject>"#;

    /// Same property inventory as every retained `Forms/<Name>.xml`.
    const FORM_METADATA: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:app="http://v8.1c.ru/8.2/managed-application/core" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
  <Form uuid="30000000-0000-4000-8000-000000000001">
    <Properties>
      <Name>SaveForm</Name>
      <Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>SaveForm</v8:content></v8:item></Synonym>
      <Comment/>
      <FormType>Managed</FormType>
      <IncludeHelpInContents>false</IncludeHelpInContents>
      <UsePurposes>
        <v8:Value xsi:type="app:ApplicationUsePurpose">PlatformApplication</v8:Value>
        <v8:Value xsi:type="app:ApplicationUsePurpose">MobilePlatformApplication</v8:Value>
      </UsePurposes>
    </Properties>
  </Form>
</MetaDataObject>"#;

    /// Deliberately inside the base-free packer's evidenced marker-50 cohort,
    /// so this test measures the routing rather than the packer's coverage.
    const FORM_BODY: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<AutoCommandBar name="FormCommandBar" id="-1"/>
	<ChildItems>
		<UsualGroup name="Main" id="10">
			<ChildItems>
				<InputField name="Description" id="12"><DataPath>Description</DataPath></InputField>
			</ChildItems>
		</UsualGroup>
	</ChildItems>
</Form>"#;

    const FORM_MODULE: &[u8] = b"&AtClient\r\nProcedure Refresh(Command)\r\nEndProcedure";

    fn form_tree_entries() -> Vec<SourceEntry> {
        vec![
            entry("Configuration.xml", FORM_CONFIGURATION.as_bytes()),
            entry("SettingsStorages/Holder.xml", FORM_OWNER.as_bytes()),
            entry(
                "SettingsStorages/Holder/Forms/SaveForm.xml",
                FORM_METADATA.as_bytes(),
            ),
            entry(
                "SettingsStorages/Holder/Forms/SaveForm/Ext/Form.xml",
                FORM_BODY.as_bytes(),
            ),
            entry(
                "SettingsStorages/Holder/Forms/SaveForm/Ext/Form/Module.bsl",
                FORM_MODULE,
            ),
        ]
    }

    fn compile_form_tree(
        entries: Vec<SourceEntry>,
    ) -> Result<BootstrapCompilation, BootstrapCompileError> {
        compile_bootstrap_source_tree(
            &SourceTree::new(entries).unwrap(),
            XmlDialect::parse("2.20").unwrap(),
            &target_profile(),
        )
    }

    #[test]
    fn managed_form_occupies_its_own_metadata_and_body_rows() {
        let compilation = compile_form_tree(form_tree_entries()).unwrap();
        assert_eq!(compilation.source_files(), 5);
        assert_eq!(compilation.metadata_files(), 3);
        // `Ext/Form.xml` plus `Ext/Form/Module.bsl` feed the single body row.
        assert_eq!(compilation.asset_files(), 2);
        assert_eq!(
            compilation
                .patch()
                .entries()
                .iter()
                .map(|entry| entry.target().key().as_str())
                .collect::<Vec<_>>(),
            [
                "10000000-0000-4000-8000-000000000001",
                "20000000-0000-4000-8000-000000000001",
                "30000000-0000-4000-8000-000000000001",
                "30000000-0000-4000-8000-000000000001.0",
                "root",
                "version",
                "versions",
            ]
        );
        compilation.patch().preflight().unwrap();

        // The body row really is the base-free managed-form packer's output.
        let body = compilation
            .patch()
            .entries()
            .iter()
            .find(|entry| entry.target().key().as_str() == "30000000-0000-4000-8000-000000000001.0")
            .unwrap();
        let bytes = body.outcome().compiled_payload().unwrap().bytes();
        let profile = ManagedFormCodecProfile::from_effective(&target_profile()).unwrap();
        let decoded = crate::compiler::bodies::form::decode_managed_form(&profile, bytes).unwrap();
        assert_eq!(
            decoded.module_text(),
            std::str::from_utf8(FORM_MODULE).unwrap()
        );
    }

    #[test]
    fn form_without_a_body_document_is_a_named_refusal() {
        let entries = form_tree_entries()
            .into_iter()
            .filter(|entry| {
                entry.path().as_str() != "SettingsStorages/Holder/Forms/SaveForm/Ext/Form.xml"
            })
            .collect::<Vec<_>>();
        let error = compile_form_tree(entries).unwrap_err();
        assert_eq!(error.code(), "form_body_missing");
        assert_eq!(
            error.source_path(),
            Some("SettingsStorages/Holder/Forms/SaveForm.xml")
        );
        assert_eq!(
            error.expected().as_deref(),
            Some("SettingsStorages/Holder/Forms/SaveForm/Ext/Form.xml")
        );
    }

    #[test]
    fn form_body_outside_the_base_free_cohort_is_addressed_not_swallowed() {
        let entries = form_tree_entries()
            .into_iter()
            .map(|source| {
                if source.path().as_str() == "SettingsStorages/Holder/Forms/SaveForm/Ext/Form.xml" {
                    entry(
                        source.path().as_str(),
                        br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20"><ChildItems><LabelDecoration name="Future" id="1"/></ChildItems></Form>"#,
                    )
                } else {
                    source
                }
            })
            .collect::<Vec<_>>();
        let error = compile_form_tree(entries).unwrap_err();
        assert_eq!(error.code(), "form_body_compile_failed");
        assert_eq!(
            error.source_path(),
            Some("SettingsStorages/Holder/Forms/SaveForm/Ext/Form.xml")
        );
        assert!(
            error
                .actual()
                .unwrap()
                .contains("unsupported base-free element"),
            "{error}"
        );
    }

    #[test]
    fn form_filed_under_no_owner_is_a_named_refusal() {
        let mut entries = form_tree_entries();
        entries.push(entry("Forms/Orphan.xml", FORM_METADATA.as_bytes()));
        let error = compile_form_tree(entries).unwrap_err();
        assert_eq!(error.code(), "form_owner_unresolved");
        assert_eq!(error.source_path(), Some("Forms/Orphan.xml"));
    }

    #[test]
    fn form_name_must_agree_with_the_directory_it_is_filed_under() {
        let entries = form_tree_entries()
            .into_iter()
            .map(|source| {
                if source.path().as_str() == "SettingsStorages/Holder/Forms/SaveForm.xml" {
                    entry(
                        source.path().as_str(),
                        FORM_METADATA
                            .replace("<Name>SaveForm</Name>", "<Name>LoadForm</Name>")
                            .as_bytes(),
                    )
                } else {
                    source
                }
            })
            .collect::<Vec<_>>();
        let error = compile_form_tree(entries).unwrap_err();
        assert_eq!(error.code(), "form_name_mismatch");
        assert_eq!(error.expected().as_deref(), Some("SaveForm"));
        assert_eq!(error.actual().as_deref(), Some("LoadForm"));
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

    // ------------------------------------------------------------------
    // REVERSE-GATE-R2-CONFIG-PROJECTION-01
    // ------------------------------------------------------------------

    /// The load direction must accept, name for name, every Configuration
    /// `<Properties>` element the platform's own export writes. This is the
    /// coverage assertion: nothing in a real native document falls through to
    /// "has no base-free projection" any more.
    #[test]
    fn every_native_configuration_property_name_has_a_projection() {
        for encoded in [
            T1_ALL_DEFAULT_NATIVE_XML_B64,
            T2_LONGER_HEADER_NATIVE_XML_B64,
            T3_ENUM_GROUP_NATIVE_XML_B64,
        ] {
            let xml = native_configuration_xml(encoded);
            let projection = project_native_text(&xml).unwrap();
            assert_eq!(projection.properties.compatibility_mode, 80_327);
            assert_eq!(projection.properties.extension_compatibility_mode, 80_327);
            assert_eq!(
                projection.default_language.as_deref(),
                Some("Language.Русский")
            );
            assert!(projection.properties.use_platform_application);
            assert_eq!(
                projection.properties.enabled_mobile_functionalities,
                [0, 25]
            );
            assert!(projection.properties.default_roles.is_empty());
            assert_eq!(projection.properties.default_style, None);
            assert_eq!(projection.properties.settings_storages, [None; 4]);
        }
    }

    /// T1 and T2 are all-default; T2 additionally exercises a Configuration
    /// header of a different length, which is what shifts every proven byte
    /// offset in the export direction.
    #[test]
    fn all_default_corpora_project_to_the_evidenced_default_digits() {
        for (encoded, name, synonym) in [
            (T1_ALL_DEFAULT_NATIVE_XML_B64, "DcsEvidence", "DCS evidence"),
            (
                T2_LONGER_HEADER_NATIVE_XML_B64,
                "DcsFilterEvidence",
                "DCS Filter Evidence",
            ),
        ] {
            let projection = project_native(encoded).unwrap();
            assert_eq!(projection.properties.name, name);
            assert_eq!(projection.properties.synonyms.len(), 1);
            assert_eq!(projection.properties.synonyms[0].content, synonym);
            let digits = projection.properties.evidenced_property_digits;
            assert_eq!(digits.include_help_in_contents, Some(b'0'));
            assert_eq!(digits.use_managed_form_in_ordinary_application, Some(b'0'));
            assert_eq!(digits.use_ordinary_form_in_managed_application, Some(b'0'));
            assert_eq!(digits.modality_use_mode, Some(b'2'));
            assert_eq!(digits.interface_compatibility_mode, Some(b'2'));
            assert_eq!(
                digits.synchronous_platform_extension_and_add_in_call_use_mode,
                Some(b'2')
            );
        }
    }

    /// T3 differs from T1 in exactly three evidenced enum values, so exactly
    /// three projected digits must differ and nothing else.
    #[test]
    fn enum_group_corpus_projects_its_three_non_default_digits() {
        let base = project_native(T1_ALL_DEFAULT_NATIVE_XML_B64)
            .unwrap()
            .properties
            .evidenced_property_digits;
        let digits = project_native(T3_ENUM_GROUP_NATIVE_XML_B64)
            .unwrap()
            .properties
            .evidenced_property_digits;
        assert_eq!(digits.modality_use_mode, Some(b'0'));
        assert_eq!(digits.interface_compatibility_mode, Some(b'0'));
        assert_eq!(
            digits.synchronous_platform_extension_and_add_in_call_use_mode,
            Some(b'0')
        );
        assert_eq!(
            digits.include_help_in_contents,
            base.include_help_in_contents
        );
        assert_eq!(
            digits.use_managed_form_in_ordinary_application,
            base.use_managed_form_in_ordinary_application
        );
        assert_eq!(
            digits.use_ordinary_form_in_managed_application,
            base.use_ordinary_form_in_managed_application
        );
    }

    /// NEGATIVE 1: a property whose value is not the proven platform default
    /// has no proven config-body coordinate, so it must be refused with a
    /// typed error rather than compiled approximately or dropped in silence.
    #[test]
    fn a_non_default_value_of_an_unproven_property_fails_closed() {
        for (from, to, property, value) in [
            (
                "<DataLockControlMode>Managed</DataLockControlMode>",
                "<DataLockControlMode>Automatic</DataLockControlMode>",
                "DataLockControlMode",
                "Automatic",
            ),
            (
                "<MainClientApplicationWindowMode>Normal</MainClientApplicationWindowMode>",
                "<MainClientApplicationWindowMode>FullscreenWorkplace</MainClientApplicationWindowMode>",
                "MainClientApplicationWindowMode",
                "FullscreenWorkplace",
            ),
            (
                "<ObjectAutonumerationMode>NotAutoFree</ObjectAutonumerationMode>",
                "<ObjectAutonumerationMode>AutoFree</ObjectAutonumerationMode>",
                "ObjectAutonumerationMode",
                "AutoFree",
            ),
            (
                "<DefaultConstantsForm/>",
                "<DefaultConstantsForm>CommonForm.Constants</DefaultConstantsForm>",
                "DefaultConstantsForm",
                "CommonForm.Constants",
            ),
            (
                "<CommonSettingsStorage/>",
                "<CommonSettingsStorage>SettingsStorage.Common</CommonSettingsStorage>",
                "CommonSettingsStorage",
                "SettingsStorage.Common",
            ),
            (
                "<DefaultRoles/>",
                "<DefaultRoles><xr:Item xsi:type=\"xr:MDObjectRef\">Role.Full</xr:Item></DefaultRoles>",
                "DefaultRoles",
                "<non-empty reference>",
            ),
        ] {
            let xml = native_configuration_xml(T1_ALL_DEFAULT_NATIVE_XML_B64).replace(from, to);
            assert!(
                xml.contains(to),
                "the `{property}` negative case must actually patch the native document"
            );
            let error = project_native_text(&xml).unwrap_err();
            match error {
                BootstrapCompileError::ConfigurationPropertyValueNotProjectable {
                    property: actual_property,
                    value: actual_value,
                } => {
                    assert_eq!(actual_property, property);
                    assert_eq!(actual_value, value);
                }
                other => panic!("`{property}` produced {other:?} instead of a typed refusal"),
            }
        }
    }

    /// NEGATIVE 2: an evidenced coordinate carrying an enum lexeme that no
    /// corpus ever mapped to a byte must be refused, not guessed at. The
    /// platform's enumerations are wider than the evidence.
    #[test]
    fn an_evidenced_property_value_outside_the_proven_map_fails_closed() {
        for (from, to, property, value) in [
            // `UseWithWarnings` used to be this row's lexeme. It is now
            // observed -- «1С:Управление торговлей 11.5.27.75» writes `1` in
            // this tuple field and prints it -- so it is a proven map member,
            // not a refusal case. `Auto` is not in the platform's enumeration
            // at all and stays refused.
            (
                "<ModalityUseMode>DontUse</ModalityUseMode>",
                "<ModalityUseMode>Auto</ModalityUseMode>",
                "ModalityUseMode",
                "Auto",
            ),
            // `Taxi` used to be this row's lexeme. It is now observed --
            // WMS5's `МодульWebОбмена_ERP25.cf` writes `3` in this tuple
            // field and prints it -- so it is a proven map member, not a
            // refusal case. `Version8_1` is not in the proven map and stays
            // refused.
            (
                "<InterfaceCompatibilityMode>TaxiEnableVersion8_2</InterfaceCompatibilityMode>",
                "<InterfaceCompatibilityMode>Version8_1</InterfaceCompatibilityMode>",
                "InterfaceCompatibilityMode",
                "Version8_1",
            ),
            (
                "<SynchronousPlatformExtensionAndAddInCallUseMode>DontUse</SynchronousPlatformExtensionAndAddInCallUseMode>",
                "<SynchronousPlatformExtensionAndAddInCallUseMode>UseWithWarnings</SynchronousPlatformExtensionAndAddInCallUseMode>",
                "SynchronousPlatformExtensionAndAddInCallUseMode",
                "UseWithWarnings",
            ),
            (
                "<IncludeHelpInContents>false</IncludeHelpInContents>",
                "<IncludeHelpInContents>FALSE</IncludeHelpInContents>",
                "IncludeHelpInContents",
                "FALSE",
            ),
        ] {
            let xml = native_configuration_xml(T1_ALL_DEFAULT_NATIVE_XML_B64).replace(from, to);
            assert!(xml.contains(to));
            let error = project_native_text(&xml).unwrap_err();
            match error {
                BootstrapCompileError::ConfigurationPropertyValueOutsideEvidencedMap {
                    property: actual_property,
                    value: actual_value,
                } => {
                    assert_eq!(actual_property, property);
                    assert_eq!(actual_value, value);
                }
                other => panic!("`{property}` produced {other:?} instead of a typed refusal"),
            }
        }
    }

    /// NEGATIVE 3: the mobile-functionality block is compilable only as the
    /// exact evidenced default; flipping one entry must fail closed.
    #[test]
    fn a_modified_mobile_functionality_block_fails_closed() {
        let xml = native_configuration_xml(T1_ALL_DEFAULT_NATIVE_XML_B64).replace(
            "<app:functionality>Location</app:functionality>\r\n\t\t\t\t\t<app:use>false</app:use>",
            "<app:functionality>Location</app:functionality>\r\n\t\t\t\t\t<app:use>true</app:use>",
        );
        assert!(matches!(
            project_native_text(&xml).unwrap_err(),
            BootstrapCompileError::ConfigurationPropertyValueNotProjectable { property, .. }
                if property == "UsedMobileApplicationFunctionalities"
        ));

        let xml = native_configuration_xml(T1_ALL_DEFAULT_NATIVE_XML_B64).replace(
            "<v8:Value xsi:type=\"app:ApplicationUsePurpose\">PlatformApplication</v8:Value>",
            "<v8:Value xsi:type=\"app:ApplicationUsePurpose\">MobileDevice</v8:Value>",
        );
        assert!(matches!(
            project_native_text(&xml).unwrap_err(),
            BootstrapCompileError::ConfigurationPropertyValueNotProjectable { property, value }
                if property == "UsePurposes" && value == "MobileDevice"
        ));
    }

    /// An element name the platform never writes is still refused by the
    /// original catch-all, so widening the projection did not widen what the
    /// compiler silently tolerates.
    #[test]
    fn an_unknown_property_name_is_still_refused() {
        let xml = native_configuration_xml(T1_ALL_DEFAULT_NATIVE_XML_B64)
            .replace("<DefaultConstantsForm/>", "<FutureProperty/>");
        assert!(matches!(
            project_native_text(&xml).unwrap_err(),
            BootstrapCompileError::InvalidConfiguration(message)
                if message.contains("`FutureProperty` has no base-free projection")
        ));
    }
}
