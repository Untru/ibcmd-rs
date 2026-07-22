use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::ObjectPath;
use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::semantic::semantic_digest;
use ibcmd_core::storage::Sha256Digest;
use ibcmd_core::validate::validate_configuration;
use ibcmd_core::version::XmlDialect;
use ibcmd_xml::source_tree::{
    SourceEntry, SourceKind, SourcePath, SourceTree, SourceTreeError, SourceTreeReader,
};
use ibcmd_xml::{
    AttributeKind, DialectDetection, DialectFeature, FeatureAvailability, LexicalPolicy,
    MetadataEncodeError, MetadataRegistry, XmlDocument, XmlElement, XmlNode, XmlReader, XmlWriter,
    bundled_dialect_registry, decode_metadata_envelope_with_dialect,
};
use serde::Deserialize;

const DIALECTS: [&str; 3] = ["2.17", "2.20", "2.21"];
const MD_NAMESPACE: &str = "http://v8.1c.ru/8.3/MDClasses";
const V8_NAMESPACE: &str = "http://v8.1c.ru/8.1/data/core";
const FUTURE_NAMESPACE: &str = "urn:ibcmd-rs:xml-corpus:future";
const PALETTE_NAMESPACE: &str = "http://v8.1c.ru/8.1/data/ui/colors/palette";
const CONFIGURATION_UUID: &str = "10000000-0000-4000-8000-000000000001";
const MODULE_UUID: &str = "20000000-0000-4000-8000-000000000001";
const CATALOG_UUID: &str = "30000000-0000-4000-8000-000000000001";
const DESCENDANT_UUIDS: [&str; 4] = [
    "31000000-0000-4000-8000-000000000001",
    "32000000-0000-4000-8000-000000000001",
    "32100000-0000-4000-8000-000000000001",
    "32200000-0000-4000-8000-000000000001",
];
const CONFIGURATION_ORDER: [&str; 7] = [
    "Name",
    "Synonym",
    "Comment",
    "DefaultRunMode",
    "ScriptVariant",
    "CompatibilityMode",
    "FutureSetting",
];
const CONFIGURATION_221_ORDER: [&str; 8] = [
    "Name",
    "Synonym",
    "Comment",
    "DefaultRunMode",
    "ScriptVariant",
    "CompatibilityMode",
    "UseInInterfaceCompatibilityMode",
    "FutureSetting",
];
const COMMON_MODULE_ORDER: [&str; 12] = [
    "Name",
    "Synonym",
    "Comment",
    "Global",
    "ClientManagedApplication",
    "Server",
    "ExternalConnection",
    "ClientOrdinaryApplication",
    "ServerCall",
    "Privileged",
    "ReturnValuesReuse",
    "FutureModuleSetting",
];
const CATALOG_ORDER: [&str; 8] = [
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "Hierarchical",
    "CodeLength",
    "DescriptionLength",
    "FutureCatalogSetting",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureManifest {
    schema_version: u32,
    fixtures: Vec<FixtureRecord>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureRecord {
    id: String,
    path: String,
    artifact_kind: String,
    coordinates: FixtureCoordinates,
    sha256: String,
    provenance: FixtureProvenance,
    features: Vec<String>,
    expected: FixtureExpectation,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureCoordinates {
    xml_dialect: String,
    platform_build: Option<String>,
    storage_version: Option<String>,
    container_revision: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureProvenance {
    origin: String,
    issue: String,
    runtime_dependencies: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureExpectation {
    outcome: String,
    losses: Vec<String>,
}

fn fixture_root(version: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("xml")
        .join(version)
}

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("xml")
}

fn collect_fixture_paths(
    directory: &Path,
    corpus: &Path,
    paths: &mut BTreeSet<String>,
) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("cannot enumerate {}: {error}", directory.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|error| format!("cannot inspect {}: {error}", entry.path().display()))?;
        if file_type.is_dir() {
            collect_fixture_paths(&entry.path(), corpus, paths)?;
        } else if file_type.is_file() {
            let relative = entry
                .path()
                .strip_prefix(corpus)
                .map_err(|_| "fixture escaped corpus root".to_owned())?
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            if !paths.insert(relative.clone()) {
                return Err(format!("duplicate fixture path `{relative}`"));
            }
        } else {
            return Err(format!(
                "unsupported fixture entry {}",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

fn validate_fixture_manifest() -> Result<(), String> {
    let corpus = corpus_root();
    let manifest_bytes = fs::read(corpus.join("manifest.json"))
        .map_err(|error| format!("cannot read fixture manifest: {error}"))?;
    let manifest: FixtureManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("invalid fixture manifest schema: {error}"))?;
    if manifest.schema_version != 1 {
        return Err(format!(
            "unsupported fixture manifest schema {}",
            manifest.schema_version
        ));
    }
    if manifest.fixtures.len() != 12 {
        return Err(format!(
            "fixture manifest must contain 12 records, found {}",
            manifest.fixtures.len()
        ));
    }

    let known_artifacts = BTreeSet::from([
        "xcf-catalog-metadata",
        "xcf-common-module-metadata",
        "xcf-configuration-metadata",
        "xcf-module-source",
    ]);
    let mut ids = BTreeSet::new();
    let mut manifest_paths = BTreeSet::new();
    for fixture in manifest.fixtures {
        if !ids.insert(fixture.id.clone()) {
            return Err(format!("duplicate fixture id `{}`", fixture.id));
        }
        SourcePath::new(&fixture.path)
            .map_err(|error| format!("unsafe manifest path `{}`: {error}", fixture.path))?;
        if !manifest_paths.insert(fixture.path.clone()) {
            return Err(format!("duplicate manifest path `{}`", fixture.path));
        }
        if !known_artifacts.contains(fixture.artifact_kind.as_str()) {
            return Err(format!(
                "unknown artifact kind `{}` for {}",
                fixture.artifact_kind, fixture.path
            ));
        }
        let path_dialect = fixture
            .path
            .split('/')
            .next()
            .ok_or_else(|| format!("missing dialect directory for {}", fixture.path))?;
        if fixture.coordinates.xml_dialect != path_dialect
            || !DIALECTS.contains(&fixture.coordinates.xml_dialect.as_str())
        {
            return Err(format!(
                "independent XML coordinate mismatch for {}",
                fixture.path
            ));
        }
        if fixture.coordinates.platform_build.is_some()
            || fixture.coordinates.storage_version.is_some()
            || fixture.coordinates.container_revision.is_some()
        {
            return Err(format!(
                "fixture {} must not infer platform/storage/container coordinates",
                fixture.path
            ));
        }
        if fixture.provenance.origin != "hand-authored-clean-room"
            || fixture.provenance.issue != "XML-006"
            || !fixture.provenance.runtime_dependencies.is_empty()
        {
            return Err(format!("invalid offline provenance for {}", fixture.path));
        }
        if fixture.features.is_empty() || fixture.features.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(format!(
                "features for {} must be unique and sorted",
                fixture.path
            ));
        }
        if fixture.expected.outcome != "valid" || !fixture.expected.losses.is_empty() {
            return Err(format!(
                "unexpected outcome/loss contract for {}",
                fixture.path
            ));
        }
        let bytes = fs::read(corpus.join(&fixture.path))
            .map_err(|error| format!("cannot read {}: {error}", fixture.path))?;
        let actual = Sha256Digest::for_bytes(&bytes).to_string();
        if fixture.sha256 != actual {
            return Err(format!(
                "SHA-256 mismatch for {}: expected {}, actual {actual}",
                fixture.path, fixture.sha256
            ));
        }
    }

    let mut actual_paths = BTreeSet::new();
    for version in DIALECTS {
        collect_fixture_paths(&fixture_root(version), &corpus, &mut actual_paths)?;
    }
    if manifest_paths != actual_paths {
        return Err(format!(
            "manifest completeness mismatch: manifest={manifest_paths:?}, actual={actual_paths:?}"
        ));
    }
    Ok(())
}

fn assert_manifest_valid() {
    validate_fixture_manifest().expect("fixture manifest must be complete and content-addressed");
}

fn fixture_bytes(version: &str, relative: &str) -> Vec<u8> {
    fs::read(fixture_root(version).join(relative)).expect("checked-in XML fixture must be readable")
}

fn profile(version: &str) -> ProfileId {
    ProfileId::parse(&format!("xml-{version}")).expect("fixture profile ID is stable")
}

fn exact_profile(detection: &DialectDetection) -> Option<&str> {
    match detection {
        DialectDetection::Exact { candidate, .. } => Some(candidate.profile_id().as_str()),
        DialectDetection::Ambiguous { .. } | DialectDetection::Unknown { .. } => None,
    }
}

fn element_children(element: &XmlElement) -> impl Iterator<Item = &XmlElement> {
    element.children().iter().filter_map(|node| match node {
        XmlNode::Element(element) => Some(element),
        _ => None,
    })
}

fn required_child<'a>(element: &'a XmlElement, local: &str) -> &'a XmlElement {
    let matches = element_children(element)
        .filter(|child| child.name().local() == local)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "expected exactly one `{local}` child");
    matches[0]
}

fn metadata_object(document: &XmlDocument) -> &XmlElement {
    let objects = element_children(document.root()).collect::<Vec<_>>();
    assert_eq!(
        objects.len(),
        1,
        "metadata envelope must contain one object"
    );
    objects[0]
}

fn properties(document: &XmlDocument) -> &XmlElement {
    required_child(metadata_object(document), "Properties")
}

fn direct_text(element: &XmlElement) -> Result<String, String> {
    let mut value = String::new();
    for node in element.children() {
        match node {
            XmlNode::Text(text) => value.push_str(text.value()),
            XmlNode::CData(text) => value.push_str(text.value()),
            XmlNode::Element(_) => {
                return Err(format!(
                    "`{}` contains nested elements instead of scalar text",
                    element.name().raw()
                ));
            }
            XmlNode::Comment(_) | XmlNode::ProcessingInstruction(_) | XmlNode::DocType(_) => {}
        }
    }
    Ok(value)
}

fn property_order(document: &XmlDocument) -> Vec<String> {
    element_children(properties(document))
        .map(|element| element.name().local().to_owned())
        .collect()
}

fn require_property_order(document: &XmlDocument, expected: &[&str]) -> Result<(), String> {
    let actual = property_order(document);
    let expected = expected
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "property order mismatch: expected {expected:?}, actual {actual:?}"
        ))
    }
}

fn property_value(document: &XmlDocument, name: &str) -> String {
    direct_text(required_child(properties(document), name))
        .expect("selected fixture property must be scalar")
}

fn namespace_declarations(element: &XmlElement) -> Vec<(Option<String>, String)> {
    element
        .attributes()
        .iter()
        .filter_map(|attribute| match attribute.kind() {
            AttributeKind::Namespace(prefix) => {
                Some((prefix.clone(), attribute.value().to_owned()))
            }
            AttributeKind::Ordinary(_) => None,
        })
        .collect()
}

fn delta_markers(document: &XmlDocument) -> BTreeSet<&'static str> {
    let mut markers = BTreeSet::new();
    if namespace_declarations(document.root())
        .iter()
        .any(|(_, uri)| uri == PALETTE_NAMESPACE)
    {
        markers.insert("palette_namespace");
    }
    if element_children(properties(document)).any(|element| {
        element.name().prefix().is_none()
            && element.name().local() == "UseInInterfaceCompatibilityMode"
            && direct_text(element).is_ok_and(|value| value == "Any")
    }) {
        markers.insert("use_in_interface_compatibility_mode");
    }
    markers
}

fn qualifies_as_221_golden(document: &XmlDocument) -> bool {
    let Ok(detection) = bundled_dialect_registry().and_then(|registry| registry.detect(document))
    else {
        return false;
    };
    exact_profile(&detection) == Some("xml-2.21")
        && delta_markers(document)
            == BTreeSet::from(["palette_namespace", "use_in_interface_compatibility_mode"])
        && require_property_order(document, &CONFIGURATION_221_ORDER).is_ok()
}

#[test]
fn fixture_manifest_schema_completeness_and_hashes_are_valid() {
    assert_manifest_valid();
}

#[test]
fn source_tree_inventory_is_deterministic_for_each_dialect() {
    assert_manifest_valid();
    let expected = vec![
        (
            "Catalogs/Products.xml",
            SourceKind::MetadataXml,
            Some(CATALOG_UUID),
        ),
        (
            "CommonModules/CorpusModule.xml",
            SourceKind::MetadataXml,
            Some(MODULE_UUID),
        ),
        (
            "CommonModules/CorpusModule/Ext/Module.bsl",
            SourceKind::Module,
            None,
        ),
        (
            "Configuration.xml",
            SourceKind::ConfigurationRoot,
            Some(CONFIGURATION_UUID),
        ),
    ];
    let mut common_inventory = None;
    for version in DIALECTS {
        let first = SourceTreeReader::default()
            .read(fixture_root(version))
            .expect("fixture source tree must be valid");
        let second = SourceTreeReader::default()
            .read(fixture_root(version))
            .expect("repeated source tree read must be valid");
        assert_eq!(first, second, "inventory changed for {version}");
        let inventory = first
            .entries()
            .iter()
            .map(|entry| {
                (
                    entry.path().as_str().to_owned(),
                    entry.kind(),
                    entry.uuid().map(|uuid| uuid.to_string()),
                )
            })
            .collect::<Vec<_>>();
        let expected = expected
            .iter()
            .map(|(path, kind, uuid)| ((*path).to_owned(), *kind, uuid.map(ToOwned::to_owned)))
            .collect::<Vec<_>>();
        assert_eq!(inventory, expected, "unexpected inventory for {version}");
        if let Some(common) = &common_inventory {
            assert_eq!(&inventory, common, "logical inventory differs by dialect");
        } else {
            common_inventory = Some(inventory);
        }
    }
}

#[test]
fn corpus_xml_is_byte_exact_reparseable_and_detected_exactly() {
    assert_manifest_valid();
    let dialects = bundled_dialect_registry().expect("bundled dialect profiles must load");
    for version in DIALECTS {
        let expected_profile = profile(version);
        let tree = SourceTreeReader::default()
            .read(fixture_root(version))
            .expect("fixture source tree must be valid");
        for entry in tree
            .entries()
            .iter()
            .filter(|entry| entry.path().as_str().ends_with(".xml"))
        {
            let document = XmlReader::from_slice(entry.bytes()).expect("fixture XML must parse");
            assert!(document.has_utf8_bom(), "{} lacks BOM", entry.path());
            let emitted = XmlWriter::to_vec(&document, LexicalPolicy::Preserve)
                .expect("preserve emission must succeed");
            assert_eq!(
                emitted,
                entry.bytes(),
                "{} was not byte-exact",
                entry.path()
            );
            assert_eq!(
                XmlReader::from_slice(&emitted).expect("emitted XML must reparse"),
                document
            );
            let detection = dialects.detect(&document).expect("detection must succeed");
            assert_eq!(
                exact_profile(&detection),
                Some(expected_profile.as_str()),
                "wrong profile for {version}/{}",
                entry.path()
            );
            assert_eq!(detection.evidence().version(), Some(version));
        }
    }
}

#[test]
fn corpus_namespaces_properties_defaults_and_delta_provenance_are_explicit() {
    assert_manifest_valid();
    let mut common_configuration_values = None;
    for version in DIALECTS {
        let configuration = XmlReader::from_slice(&fixture_bytes(version, "Configuration.xml"))
            .expect("Configuration fixture must parse");
        let mut expected_namespaces = vec![
            (None, MD_NAMESPACE.to_owned()),
            (Some("f".to_owned()), FUTURE_NAMESPACE.to_owned()),
        ];
        if version == "2.21" {
            expected_namespaces.push((Some("pal".to_owned()), PALETTE_NAMESPACE.to_owned()));
        }
        expected_namespaces.push((Some("v8".to_owned()), V8_NAMESPACE.to_owned()));
        assert_eq!(
            namespace_declarations(configuration.root()),
            expected_namespaces
        );
        require_property_order(
            &configuration,
            if version == "2.21" {
                &CONFIGURATION_221_ORDER
            } else {
                &CONFIGURATION_ORDER
            },
        )
        .expect("Configuration property order must be explicit");
        assert_eq!(
            property_value(&configuration, "DefaultRunMode"),
            "ManagedApplication"
        );
        assert_eq!(property_value(&configuration, "ScriptVariant"), "English");
        assert_eq!(
            property_value(&configuration, "CompatibilityMode"),
            "Version8_3_8"
        );
        if version == "2.21" {
            assert_eq!(
                property_value(&configuration, "UseInInterfaceCompatibilityMode"),
                "Any"
            );
        }
        let common_values = [
            "Name",
            "DefaultRunMode",
            "ScriptVariant",
            "CompatibilityMode",
        ]
        .into_iter()
        .map(|name| (name, property_value(&configuration, name)))
        .collect::<BTreeMap<_, _>>();
        if let Some(common) = &common_configuration_values {
            assert_eq!(&common_values, common);
        } else {
            common_configuration_values = Some(common_values);
        }
        let child_order = element_children(required_child(
            metadata_object(&configuration),
            "ChildObjects",
        ))
        .map(|child| child.name().local())
        .collect::<Vec<_>>();
        assert_eq!(child_order, ["CommonModule", "Catalog"]);

        let module =
            XmlReader::from_slice(&fixture_bytes(version, "CommonModules/CorpusModule.xml"))
                .expect("CommonModule fixture must parse");
        require_property_order(&module, &COMMON_MODULE_ORDER)
            .expect("CommonModule property order must be explicit");
        assert_eq!(property_value(&module, "Global"), "false");
        assert_eq!(property_value(&module, "Server"), "true");
        assert_eq!(property_value(&module, "ServerCall"), "true");
        assert_eq!(property_value(&module, "ReturnValuesReuse"), "DontUse");

        let catalog = XmlReader::from_slice(&fixture_bytes(version, "Catalogs/Products.xml"))
            .expect("Catalog fixture must parse");
        require_property_order(&catalog, &CATALOG_ORDER)
            .expect("Catalog property order must be explicit");
        assert_eq!(property_value(&catalog, "UseStandardCommands"), "true");
        assert_eq!(property_value(&catalog, "Hierarchical"), "false");
        assert_eq!(property_value(&catalog, "CodeLength"), "9");
        assert_eq!(property_value(&catalog, "DescriptionLength"), "100");
    }

    let registry = bundled_dialect_registry().expect("bundled dialect profiles must load");
    let descriptor = registry
        .get(&XmlDialect::parse("2.21").expect("known dialect"))
        .expect("2.21 descriptor must exist");
    for feature in ["palette_namespace", "use_in_interface_compatibility_mode"] {
        let feature = DialectFeature::parse(feature).expect("known feature ID");
        assert_eq!(descriptor.feature(&feature), FeatureAvailability::Supported);
        assert_eq!(
            descriptor
                .feature_provenance(&feature)
                .map(ProfileId::as_str),
            Some("xml-2.21")
        );
    }
}

#[test]
fn catalog_envelope_preserves_preorder_semantics_and_unknowns() {
    assert_manifest_valid();
    let dialects = bundled_dialect_registry().expect("bundled dialect profiles must load");
    let metadata = MetadataRegistry::default();
    let expected_root = ObjectUuid::parse(CATALOG_UUID).expect("stable root UUID");
    let expected_descendants =
        DESCENDANT_UUIDS.map(|uuid| ObjectUuid::parse(uuid).expect("stable descendant UUID"));
    let mut common_digest = None;

    for version in DIALECTS {
        let bytes = fixture_bytes(version, "Catalogs/Products.xml");
        let document = XmlReader::from_slice(&bytes).expect("Catalog fixture must parse");
        let source_profile = profile(version);
        let envelope = decode_metadata_envelope_with_dialect(
            &document,
            &dialects,
            source_profile.clone(),
            ObjectPath::root(),
        )
        .expect("generic Catalog envelope must decode");
        assert_eq!(envelope.root().identity().uuid(), expected_root);
        assert_eq!(
            envelope
                .descendants()
                .iter()
                .map(|object| object.identity().uuid())
                .collect::<Vec<_>>(),
            expected_descendants
        );
        assert_eq!(
            envelope
                .descendants()
                .iter()
                .map(|object| object.owner())
                .collect::<Vec<_>>(),
            [
                Some(expected_root),
                Some(expected_root),
                Some(expected_descendants[1]),
                Some(expected_descendants[1]),
            ],
            "nested owner links changed in {version}"
        );
        assert_eq!(
            envelope.root().provenance().source_profile(),
            &source_profile,
            "XML evidence must remain an XML profile, not a platform build"
        );
        assert_eq!(
            envelope
                .root()
                .properties()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            ["Name", "Synonym"]
        );
        let configuration = envelope
            .configuration()
            .expect("canonical configuration must build");
        let validated =
            validate_configuration(&configuration).expect("canonical graph must validate");
        let digest = semantic_digest(&validated);
        if let Some(common) = common_digest {
            assert_eq!(
                digest, common,
                "common Catalog semantics changed in {version}"
            );
        } else {
            common_digest = Some(digest);
        }

        for (kind, ordinal, bytes) in [
            (
                "xml:properties-child",
                15,
                br#"<f:FutureCatalogSetting f:mode="preserve">catalog-opaque</f:FutureCatalogSetting>"#
                    .as_slice(),
            ),
            (
                "xml:child-objects-child",
                3,
                br#"<f:FutureChild f:mode="preserve">child-opaque</f:FutureChild>"#.as_slice(),
            ),
        ] {
            let expected_sha256 = Sha256Digest::for_bytes(bytes);
            assert!(
                envelope.root().opaque_facets().as_slice().iter().any(|facet| {
                    facet.placement().kind().as_str() == kind
                        && facet.placement().ordinal() == ordinal
                        && facet.sha256() == expected_sha256
                }),
                "missing opaque facet {kind}@{ordinal} in {version}"
            );
        }

        let same_profile = metadata
            .encode(&envelope, &source_profile)
            .expect("same-profile fallback must emit");
        assert_eq!(same_profile, bytes);
        let text = std::str::from_utf8(&same_profile).expect("fixture is UTF-8");
        assert!(text.contains(
            "<f:FutureCatalogSetting f:mode=\"preserve\">catalog-opaque</f:FutureCatalogSetting>"
        ));
        assert!(text.contains("<f:FutureChild f:mode=\"preserve\">child-opaque</f:FutureChild>"));
        let emitted_document =
            XmlReader::from_slice(&same_profile).expect("same-profile output must reparse");
        let emitted_envelope = decode_metadata_envelope_with_dialect(
            &emitted_document,
            &dialects,
            source_profile.clone(),
            ObjectPath::root(),
        )
        .expect("same-profile output must decode canonically");
        assert_eq!(
            emitted_envelope.root().identity(),
            envelope.root().identity()
        );
        assert_eq!(
            emitted_envelope
                .descendants()
                .iter()
                .map(|object| object.identity())
                .collect::<Vec<_>>(),
            envelope
                .descendants()
                .iter()
                .map(|object| object.identity())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            emitted_envelope.root().opaque_facets(),
            envelope.root().opaque_facets()
        );
        for (emitted, original) in emitted_envelope
            .descendants()
            .iter()
            .zip(envelope.descendants())
        {
            assert_eq!(emitted.opaque_facets(), original.opaque_facets());
        }
        let emitted_configuration = emitted_envelope
            .configuration()
            .expect("re-decoded canonical configuration must build");
        let emitted_validated = validate_configuration(&emitted_configuration)
            .expect("re-decoded canonical graph must validate");
        assert_eq!(semantic_digest(&emitted_validated), digest);

        let cross_profile = if version == "2.21" {
            profile("2.20")
        } else {
            profile("2.21")
        };
        assert!(matches!(
            metadata.encode(&envelope, &cross_profile),
            Err(MetadataEncodeError::Opaque(_))
        ));
    }
}

#[test]
fn configuration_221_requires_structural_delta_not_only_version() {
    assert_manifest_valid();
    let golden_bytes = fixture_bytes("2.21", "Configuration.xml");
    let golden = XmlReader::from_slice(&golden_bytes).expect("2.21 golden must parse");
    assert!(qualifies_as_221_golden(&golden));
    assert_eq!(
        delta_markers(&golden),
        BTreeSet::from(["palette_namespace", "use_in_interface_compatibility_mode",])
    );

    let source_220 =
        String::from_utf8(fixture_bytes("2.20", "Configuration.xml")).expect("fixture is UTF-8");
    let naive_bytes = source_220
        .replacen("version=\"2.20\"", "version=\"2.21\"", 1)
        .into_bytes();
    let naive = XmlReader::from_slice(&naive_bytes).expect("naive rewrite remains XML");
    let naive_detection = bundled_dialect_registry()
        .expect("bundled dialect profiles must load")
        .detect(&naive)
        .expect("detection must succeed");
    assert_eq!(exact_profile(&naive_detection), Some("xml-2.21"));
    assert!(delta_markers(&naive).is_empty());
    assert!(!qualifies_as_221_golden(&naive));
    assert_ne!(naive_bytes, golden_bytes);

    let conflicting = String::from_utf8(golden_bytes)
        .expect("fixture is UTF-8")
        .replacen("version=\"2.21\"", "version=\"2.20\"", 1);
    assert!(matches!(
        bundled_dialect_registry()
            .expect("bundled dialect profiles must load")
            .detect(&XmlReader::from_slice(conflicting.as_bytes()).expect("XML must parse"))
            .expect("detection must succeed"),
        DialectDetection::Ambiguous { .. }
    ));
}

#[test]
fn malformed_uuid_path_and_order_assumptions_fail_loudly() {
    assert_manifest_valid();
    let simple = |path: &str, uuid: &str| {
        SourceEntry::from_bytes(
            SourcePath::new(path).expect("test path must be safe"),
            format!(
                "<MetaDataObject><Catalog uuid=\"{uuid}\"><Properties><Name>X</Name></Properties></Catalog></MetaDataObject>"
            )
            .into_bytes(),
        )
        .expect("test entry must parse")
    };
    assert!(matches!(
        SourceTree::new(vec![
            simple("Catalogs/One.xml", CATALOG_UUID),
            simple("Catalogs/Two.xml", CATALOG_UUID),
        ]),
        Err(SourceTreeError::UuidConflict { .. })
    ));
    assert!(matches!(
        SourceTree::new(vec![
            simple("Catalogs/Products.xml", CATALOG_UUID),
            simple(
                "catalogs/products.xml",
                "39999999-9999-4999-8999-999999999999"
            ),
        ]),
        Err(SourceTreeError::PathConflict { .. })
    ));

    let duplicate_attribute = format!(
        "<MetaDataObject><Catalog uuid=\"{CATALOG_UUID}\" uuid=\"{CATALOG_UUID}\"><Properties><Name>X</Name></Properties></Catalog></MetaDataObject>"
    );
    assert!(matches!(
        SourceEntry::from_bytes(
            SourcePath::new("Catalogs/Broken.xml").expect("test path must be safe"),
            duplicate_attribute.into_bytes(),
        ),
        Err(SourceTreeError::Xml { .. })
    ));

    let duplicate_graph_uuid = String::from_utf8(fixture_bytes("2.20", "Catalogs/Products.xml"))
        .expect("fixture is UTF-8")
        .replace(DESCENDANT_UUIDS[2], CATALOG_UUID);
    let duplicate_graph =
        XmlReader::from_slice(duplicate_graph_uuid.as_bytes()).expect("mutated XML must parse");
    assert!(
        decode_metadata_envelope_with_dialect(
            &duplicate_graph,
            &bundled_dialect_registry().expect("bundled dialect profiles must load"),
            profile("2.20"),
            ObjectPath::root(),
        )
        .is_err()
    );

    let original =
        String::from_utf8(fixture_bytes("2.20", "Configuration.xml")).expect("fixture is UTF-8");
    let reordered = original.replacen(
        "      <DefaultRunMode>ManagedApplication</DefaultRunMode>\n      <ScriptVariant>English</ScriptVariant>",
        "      <ScriptVariant>English</ScriptVariant>\n      <DefaultRunMode>ManagedApplication</DefaultRunMode>",
        1,
    );
    assert_ne!(reordered, original, "order mutation must be applied");
    let reordered = XmlReader::from_slice(reordered.as_bytes()).expect("reordered XML must parse");
    let error = require_property_order(&reordered, &CONFIGURATION_ORDER)
        .expect_err("wrong property order must fail");
    assert!(error.contains("property order mismatch"));
}
