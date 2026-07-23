use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::rc::Rc;

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::asset::MediaKind;
use ibcmd_core::diagnostic::{ObjectPath, PathSegment, PropertyPath};
use ibcmd_core::family::FamilyId;
use ibcmd_core::identity::{LogicalIdentity, ObjectUuid};
use ibcmd_core::model::{
    CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, GeneratedType,
    GeneratedTypeKind, MetadataKind,
};
use ibcmd_core::opaque::{OpaqueFacet, OpaqueFacets, OpaquePlacement};
use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
use ibcmd_core::value::{CanonicalField, CanonicalText, CanonicalValue};

use super::fallback::Fallback;
use crate::{
    AttributeKind, DialectDetection, DialectRegistry, LexicalPolicy, XmlDocument, XmlElement,
    XmlNode,
};

const MAX_METADATA_DEPTH: usize = 64;
const MAX_METADATA_NODES: usize = 16_384;
const MAX_METADATA_FACETS: usize = 4_096;
const MAX_METADATA_BYTES: usize = 33_554_432;
const MAX_METADATA_ATTRIBUTES: usize = 65_536;
const MAX_METADATA_NAMESPACES: usize = 4_096;
const MAX_METADATA_NAMESPACE_BYTES: usize = 1_048_576;
pub(super) const MD_NAMESPACE: &str = "http://v8.1c.ru/8.3/MDClasses";
pub(super) const V8_NAMESPACE: &str = "http://v8.1c.ru/8.1/data/core";
pub(super) const XR_NAMESPACE: &str = "http://v8.1c.ru/8.3/xcf/readable";
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
const XMLNS_NAMESPACE: &str = "http://www.w3.org/2000/xmlns/";

#[derive(Clone, Copy)]
struct GeneratedLayout {
    container_anchor: &'static str,
    container_placement: &'static str,
    tail_anchor: &'static str,
    tail_placement: &'static str,
    projection_anchor: &'static str,
    projection_placement: &'static str,
}

const PROPERTIES_GENERATED_LAYOUT: GeneratedLayout = GeneratedLayout {
    container_anchor: "properties.generated_types.attributes",
    container_placement: "xml:properties-generated-types-start-tag-projection",
    tail_anchor: "properties.generated_types",
    tail_placement: "xml:properties-generated-types-child",
    projection_anchor: "properties.generated_types.generated_type",
    projection_placement: "xml:properties-generated-type-projection",
};
const DIRECT_GENERATED_LAYOUT: GeneratedLayout = GeneratedLayout {
    container_anchor: "generated_types.attributes",
    container_placement: "xml:generated-types-start-tag-projection",
    tail_anchor: "generated_types",
    tail_placement: "xml:generated-types-child",
    projection_anchor: "generated_types.generated_type",
    projection_placement: "xml:generated-type-projection",
};
const INTERNAL_INFO_GENERATED_LAYOUT: GeneratedLayout = GeneratedLayout {
    container_anchor: "internal_info.attributes",
    container_placement: "xml:internal-info-start-tag-projection",
    tail_anchor: "internal_info",
    tail_placement: "xml:internal-info-child",
    projection_anchor: "internal_info.generated_type",
    projection_placement: "xml:internal-info-generated-type-projection",
};

#[derive(Default)]
struct FacetBudget {
    count: usize,
    bytes: usize,
}
struct FacetSet {
    values: Vec<OpaqueFacet>,
    budget: Rc<RefCell<FacetBudget>>,
}
impl FacetSet {
    fn root() -> Self {
        Self {
            values: Vec::new(),
            budget: Rc::new(RefCell::new(FacetBudget::default())),
        }
    }
    fn child_with(parent: &Self, values: Vec<OpaqueFacet>) -> Self {
        Self {
            values,
            budget: Rc::clone(&parent.budget),
        }
    }
    fn reserve(&self, bytes: usize) -> Result<(), MetadataDecodeError> {
        let mut budget = self.budget.borrow_mut();
        budget.count = budget
            .count
            .checked_add(1)
            .ok_or(MetadataDecodeError::ResourceLimit("opaque facets"))?;
        budget.bytes = budget
            .bytes
            .checked_add(bytes)
            .ok_or(MetadataDecodeError::ResourceLimit("opaque bytes"))?;
        if budget.count > MAX_METADATA_FACETS {
            return Err(MetadataDecodeError::ResourceLimit("opaque facets"));
        }
        if budget.bytes > MAX_METADATA_BYTES {
            return Err(MetadataDecodeError::ResourceLimit("opaque bytes"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct MetadataEnvelope {
    root: CanonicalObject,
    descendants: Vec<CanonicalObject>,
    fallback: Fallback,
    source_model_unchanged: bool,
}
impl MetadataEnvelope {
    pub fn from_parts(
        root: CanonicalObject,
        descendants: Vec<CanonicalObject>,
        source_document: XmlDocument,
    ) -> Result<Self, MetadataDecodeError> {
        Self::from_parts_with_state(root, descendants, source_document, false)
    }
    fn from_parts_with_state(
        root: CanonicalObject,
        descendants: Vec<CanonicalObject>,
        source_document: XmlDocument,
        source_model_unchanged: bool,
    ) -> Result<Self, MetadataDecodeError> {
        let actual = inspect_metadata_family(&source_document)?;
        if actual.as_str() != root.kind().as_str() {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "source document family differs from canonical root",
            ));
        }
        let source_profile = root.provenance().source_profile();
        let mut facet_count = 0usize;
        let mut facet_bytes = 0usize;
        for object in std::iter::once(&root).chain(&descendants) {
            if object.provenance().source_profile() != source_profile {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "canonical object source profiles differ",
                ));
            }
            let mut guards = object
                .opaque_facets()
                .as_slice()
                .iter()
                .filter(|facet| facet.placement().kind().as_str() == "xml:family-fallback");
            let guard = guards.next().ok_or(MetadataDecodeError::InvalidEnvelope(
                "each canonical object requires exactly one family fallback guard",
            ))?;
            if guards.next().is_some() {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "each canonical object requires exactly one family fallback guard",
                ));
            }
            if guard.anchor().object_path() != object.identity().path() {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "family fallback guard path differs from canonical object",
                ));
            }
            if guard.byte_len() != 0 {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "family fallback guard payload must be empty",
                ));
            }
            if object
                .opaque_facets()
                .as_slice()
                .iter()
                .any(|facet| facet.source_profile() != source_profile)
            {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "opaque facet source profile differs from object",
                ));
            }
            let retained = object
                .opaque_facets()
                .as_slice()
                .iter()
                .filter(|facet| facet.placement().kind().as_str() != "xml:family-fallback");
            for facet in retained {
                facet_count = facet_count
                    .checked_add(1)
                    .ok_or(MetadataDecodeError::ResourceLimit("opaque facets"))?;
                facet_bytes = facet_bytes
                    .checked_add(
                        usize::try_from(facet.byte_len())
                            .map_err(|_| MetadataDecodeError::ResourceLimit("opaque bytes"))?,
                    )
                    .ok_or(MetadataDecodeError::ResourceLimit("opaque bytes"))?;
            }
        }
        if facet_count > MAX_METADATA_FACETS {
            return Err(MetadataDecodeError::ResourceLimit("opaque facets"));
        }
        if facet_bytes > MAX_METADATA_BYTES {
            return Err(MetadataDecodeError::ResourceLimit("opaque bytes"));
        }
        let envelope = Self {
            root,
            descendants,
            fallback: Fallback::new(source_document),
            source_model_unchanged,
        };
        let configuration = envelope
            .configuration()
            .map_err(|x| MetadataDecodeError::Core(x.to_string()))?;
        ibcmd_core::validate::validate_configuration(&configuration)
            .map_err(|x| MetadataDecodeError::Core(format!("{x:?}")))?;
        Ok(envelope)
    }
    pub fn source_document(&self) -> &XmlDocument {
        self.fallback.document()
    }
    pub const fn source_model_unchanged(&self) -> bool {
        self.source_model_unchanged
    }
    pub fn with_model(
        self,
        root: CanonicalObject,
        descendants: Vec<CanonicalObject>,
    ) -> Result<Self, MetadataDecodeError> {
        Self::from_parts(root, descendants, self.fallback.into_document())
    }
    pub fn root(&self) -> &CanonicalObject {
        &self.root
    }
    pub fn descendants(&self) -> &[CanonicalObject] {
        &self.descendants
    }
    pub fn configuration(
        &self,
    ) -> Result<CanonicalConfiguration, ibcmd_core::model::ModelBuildError> {
        let mut objects = Vec::with_capacity(self.descendants.len() + 1);
        objects.push(self.root.clone());
        objects.extend(self.descendants.clone());
        CanonicalConfiguration::new(objects)
    }
    pub(crate) fn emit(
        &self,
        target: &ProfileId,
    ) -> Result<Vec<u8>, super::registry::MetadataEncodeError> {
        if !self.source_model_unchanged {
            return Err(super::registry::MetadataEncodeError::ModelChanged {
                object_path: self.root.identity().path().clone(),
            });
        }
        for object in std::iter::once(&self.root).chain(&self.descendants) {
            for facet in object.opaque_facets().as_slice() {
                facet
                    .emit_permit(target)
                    .map_err(super::registry::MetadataEncodeError::Opaque)?;
            }
        }
        self.fallback.emit().map_err(Into::into)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetadataDecodeError {
    InvalidEnvelope(&'static str),
    Duplicate(&'static str),
    Missing(&'static str),
    InvalidUuid(String),
    ResourceLimit(&'static str),
    Core(String),
    Xml(String),
    UnsupportedProfile {
        object_path: ObjectPath,
        profile: ProfileId,
    },
    ProfileVersionMismatch {
        object_path: ObjectPath,
    },
}
impl Display for MetadataDecodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl Error for MetadataDecodeError {}

pub(crate) fn inspect_metadata_family(
    document: &XmlDocument,
) -> Result<FamilyId, MetadataDecodeError> {
    check_document(document)?;
    let uris = resolve_namespaces(document.root())?;
    let expected = uri_of(document.root(), &uris);
    if !matches!(expected, None | Some(MD_NAMESPACE))
        || !typed(document.root(), "MetaDataObject", expected, &uris)
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "root is not MetaDataObject",
        ));
    }
    let mut semantic = None;
    for node in document.root().children() {
        if let XmlNode::Element(element) = node {
            if semantic.is_some() {
                return Err(MetadataDecodeError::Duplicate("metadata object"));
            }
            semantic = Some(element);
        }
    }
    let semantic = semantic.ok_or(MetadataDecodeError::Missing("metadata object"))?;
    if uri_of(semantic, &uris) != expected {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "metadata object namespace differs from envelope",
        ));
    }
    FamilyId::parse(semantic.name().local()).map_err(|x| MetadataDecodeError::Core(x.to_string()))
}

/// Decodes common XCF metadata fields. `source_profile` is caller supplied;
/// it is deliberately never inferred from the root version.
pub fn decode_metadata_envelope(
    document: &XmlDocument,
    source_profile: ProfileId,
    object_path: ObjectPath,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    decode_metadata_envelope_with_child_references(document, source_profile, object_path, &[])
}

/// Decodes the root `Configuration` object while treating its named
/// `ChildObjects` entries as references to sibling source files.
///
/// Unlike embedded metadata children, configuration collection members do
/// not carry UUIDs in `Configuration.xml`; their identity is supplied by the
/// corresponding family XML file.  Keeping this distinction in the XML
/// adapter prevents the bootstrap planner from guessing ownership.
pub fn decode_configuration_envelope(
    document: &XmlDocument,
    source_profile: ProfileId,
    object_path: ObjectPath,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    if inspect_metadata_family(document)?.as_str() != "Configuration" {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "metadata object is not Configuration",
        ));
    }
    decode_metadata_envelope_with_child_references(
        document,
        source_profile,
        object_path,
        &[
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
        ],
    )
}

/// Decodes common XCF metadata while retaining selected name-only entries in
/// `ChildObjects` as lossless reference projections.
///
/// Catalogs and documents use `<Form>Name</Form>` and
/// `<Template>Name</Template>` beside UUID-bearing embedded objects.  These
/// entries are references to separately stored metadata rows, not malformed
/// child objects.  The generic public decoder remains strict; family codecs
/// opt in to the exact reference element names they understand.
pub(super) fn decode_metadata_envelope_with_child_references(
    document: &XmlDocument,
    source_profile: ProfileId,
    object_path: ObjectPath,
    child_reference_kinds: &[&str],
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    check_document(document)?;
    let uris = resolve_namespaces(document.root())?;
    let expected = uri_of(document.root(), &uris);
    if !matches!(expected, None | Some(MD_NAMESPACE))
        || !typed(document.root(), "MetaDataObject", expected, &uris)
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "root is not MetaDataObject",
        ));
    }
    let semantic = document
        .root()
        .children()
        .iter()
        .find_map(|n| match n {
            XmlNode::Element(e) => Some(e),
            _ => None,
        })
        .ok_or(MetadataDecodeError::Missing("metadata object"))?;
    if document
        .root()
        .children()
        .iter()
        .filter(|n| matches!(n, XmlNode::Element(_)))
        .count()
        != 1
    {
        return Err(MetadataDecodeError::Duplicate("metadata object"));
    }
    if uri_of(semantic, &uris) != expected {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "metadata object namespace differs from envelope",
        ));
    }
    let mut descendants = Vec::new();
    let mut facet_set = FacetSet::root();
    for (ordinal, node) in document.before_root().iter().enumerate() {
        retain_as(
            node,
            ordinal,
            &source_profile,
            &object_path,
            "document.prolog",
            "xml:document-prolog-node",
            &mut facet_set,
        )?;
    }
    retain_unknown_start_tag(
        document.root(),
        &["version"],
        &source_profile,
        &object_path,
        "metadata_object.attributes",
        "xml:metadata-object-start-tag-projection",
        &mut facet_set,
    )?;
    for (ordinal, node) in document.root().children().iter().enumerate() {
        if !matches!(node, XmlNode::Element(_)) {
            retain_as(
                node,
                ordinal,
                &source_profile,
                &object_path,
                "metadata_object",
                "xml:metadata-object-child",
                &mut facet_set,
            )?;
        }
    }
    for (ordinal, node) in document.after_root().iter().enumerate() {
        retain_as(
            node,
            ordinal,
            &source_profile,
            &object_path,
            "document.epilog",
            "xml:document-epilog-node",
            &mut facet_set,
        )?;
    }
    let initial_facets = std::mem::take(&mut facet_set.values);
    let root = decode_object(
        semantic,
        source_profile.clone(),
        object_path,
        None,
        &mut descendants,
        &mut facet_set,
        initial_facets,
        &uris,
        expected,
        child_reference_kinds,
    )?;
    MetadataEnvelope::from_parts_with_state(root, descendants, document.clone(), true)
}

/// Decodes after checking that caller-selected exact source profile is one of
/// the compatible dialect candidates. It never derives a profile from version.
pub fn decode_metadata_envelope_with_dialect(
    document: &XmlDocument,
    dialects: &DialectRegistry,
    source_profile: ProfileId,
    object_path: ObjectPath,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    match dialects
        .detect(document)
        .map_err(|x| MetadataDecodeError::Xml(x.to_string()))?
    {
        DialectDetection::Exact { candidate, .. } if candidate.profile_id() == &source_profile => {}
        DialectDetection::Ambiguous { candidates, .. }
            if candidates
                .iter()
                .any(|candidate| candidate.profile_id() == &source_profile) => {}
        _ => {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "source profile is incompatible with XML dialect evidence",
            ));
        }
    }
    decode_metadata_envelope(document, source_profile, object_path)
}

#[allow(clippy::too_many_arguments)]
fn decode_object(
    e: &XmlElement,
    profile: ProfileId,
    path: ObjectPath,
    owner: Option<ObjectUuid>,
    descendants: &mut Vec<CanonicalObject>,
    parent_facets: &mut FacetSet,
    initial_facets: Vec<OpaqueFacet>,
    uris: &ResolvedNamespaces,
    expected: Option<&str>,
    child_reference_kinds: &[&str],
) -> Result<CanonicalObject, MetadataDecodeError> {
    let mut local_facets = FacetSet::child_with(parent_facets, initial_facets);
    let uuid = uuid_attr(e)?;
    retain_unknown_start_tag(
        e,
        &["uuid"],
        &profile,
        &path,
        "object.attributes",
        "xml:object-start-tag-projection",
        &mut local_facets,
    )?;
    let mut parts = CanonicalObjectParts::new(
        LogicalIdentity::new(uuid, path.clone()),
        MetadataKind::new(e.name().local())
            .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
        provenance(&profile, &path, "object")?,
    );
    parts.owner = owner;
    let mut names = BTreeSet::new();
    let mut containers = BTreeSet::new();
    let mut generated_source = None;
    for (ordinal, node) in e.children().iter().enumerate() {
        let XmlNode::Element(child) = node else {
            retain(node, ordinal, &profile, &path, &mut local_facets)?;
            continue;
        };
        match child.name().local() {
            "Properties" if typed(child, "Properties", expected, uris) => {
                if !containers.insert("Properties") {
                    return Err(MetadataDecodeError::Duplicate("Properties"));
                }
                retain_unknown_start_tag(
                    child,
                    &[],
                    &profile,
                    &path,
                    "properties.attributes",
                    "xml:properties-start-tag-projection",
                    &mut local_facets,
                )?;
                let has_generated = decode_properties(
                    child,
                    &mut parts.properties,
                    &mut parts.generated_types,
                    &mut names,
                    &profile,
                    &path,
                    &mut local_facets,
                    uris,
                    expected,
                )?;
                if has_generated
                    && generated_source
                        .replace("Properties/GeneratedTypes")
                        .is_some()
                {
                    return Err(MetadataDecodeError::Duplicate("generated types source"));
                }
            }
            "GeneratedTypes" if typed(child, "GeneratedTypes", expected, uris) => {
                if !containers.insert("GeneratedTypes") {
                    return Err(MetadataDecodeError::Duplicate("GeneratedTypes"));
                }
                let has_generated = decode_generated_types(
                    child,
                    &mut parts.generated_types,
                    &profile,
                    &path,
                    &mut local_facets,
                    uris,
                    expected,
                    DIRECT_GENERATED_LAYOUT,
                )?;
                if has_generated && generated_source.replace("GeneratedTypes").is_some() {
                    return Err(MetadataDecodeError::Duplicate("generated types source"));
                }
            }
            "InternalInfo" if typed(child, "InternalInfo", expected, uris) => {
                if !containers.insert("InternalInfo") {
                    return Err(MetadataDecodeError::Duplicate("InternalInfo"));
                }
                let has_generated = decode_generated_types(
                    child,
                    &mut parts.generated_types,
                    &profile,
                    &path,
                    &mut local_facets,
                    uris,
                    if expected.is_none() {
                        None
                    } else {
                        Some(XR_NAMESPACE)
                    },
                    INTERNAL_INFO_GENERATED_LAYOUT,
                )?;
                if has_generated && generated_source.replace("InternalInfo").is_some() {
                    return Err(MetadataDecodeError::Duplicate("generated types source"));
                }
            }
            "ChildObjects" if typed(child, "ChildObjects", expected, uris) => {
                if !containers.insert("ChildObjects") {
                    return Err(MetadataDecodeError::Duplicate("ChildObjects"));
                }
                retain_unknown_start_tag(
                    child,
                    &[],
                    &profile,
                    &path,
                    "child_objects.attributes",
                    "xml:child-objects-start-tag-projection",
                    &mut local_facets,
                )?;
                decode_children(
                    child,
                    &profile,
                    &path,
                    uuid,
                    descendants,
                    &mut local_facets,
                    uris,
                    expected,
                    child_reference_kinds,
                )?
            }
            _ => retain(node, ordinal, &profile, &path, &mut local_facets)?,
        }
    }
    if !names.contains("Name") {
        return Err(MetadataDecodeError::Missing("Name"));
    }
    push_family_guard(&profile, &path, &mut local_facets)?;
    parts.opaque_facets = OpaqueFacets::new(local_facets.values)
        .map_err(|x| MetadataDecodeError::Core(x.to_string()))?;
    CanonicalObject::new(parts).map_err(|x| MetadataDecodeError::Core(x.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn decode_children(
    container: &XmlElement,
    profile: &ProfileId,
    parent_path: &ObjectPath,
    owner: ObjectUuid,
    descendants: &mut Vec<CanonicalObject>,
    facets: &mut FacetSet,
    uris: &ResolvedNamespaces,
    expected: Option<&str>,
    child_reference_kinds: &[&str],
) -> Result<(), MetadataDecodeError> {
    let mut typed_index = 0u32;
    for (ordinal, node) in container.children().iter().enumerate() {
        let XmlNode::Element(child) = node else {
            retain_as(
                node,
                ordinal,
                profile,
                parent_path,
                "child_objects",
                "xml:child-objects-child",
                facets,
            )?;
            continue;
        };
        if uri_of(child, uris) != expected {
            retain_as(
                node,
                ordinal,
                profile,
                parent_path,
                "child_objects",
                "xml:child-objects-child",
                facets,
            )?;
            continue;
        }
        let has_uuid = child.attributes().iter().any(|attribute| {
            matches!(
                attribute.kind(),
                AttributeKind::Ordinary(name) if name.prefix().is_none() && name.local() == "uuid"
            )
        });
        if !has_uuid
            && child_reference_kinds
                .iter()
                .any(|candidate| *candidate == child.name().local())
        {
            retain_as(
                node,
                ordinal,
                profile,
                parent_path,
                "child_objects",
                "xml:child-object-reference",
                facets,
            )?;
            continue;
        }
        uuid_attr(child)?;
        let mut path = parent_path.clone();
        path.push(
            PathSegment::name("children").map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
        )
        .map_err(|x| MetadataDecodeError::Core(x.to_string()))?;
        path.push(PathSegment::index(typed_index))
            .map_err(|x| MetadataDecodeError::Core(x.to_string()))?;
        typed_index = typed_index
            .checked_add(1)
            .ok_or(MetadataDecodeError::ResourceLimit("child ordinal"))?;
        let mut nested = Vec::new();
        let child_object = decode_object(
            child,
            profile.clone(),
            path,
            Some(owner),
            &mut nested,
            facets,
            Vec::new(),
            uris,
            expected,
            &[],
        )?;
        descendants.push(child_object);
        descendants.extend(nested);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn decode_properties(
    e: &XmlElement,
    out: &mut Vec<CanonicalField>,
    generated: &mut Vec<GeneratedType>,
    names: &mut BTreeSet<String>,
    profile: &ProfileId,
    path: &ObjectPath,
    facets: &mut FacetSet,
    uris: &ResolvedNamespaces,
    expected: Option<&str>,
) -> Result<bool, MetadataDecodeError> {
    let mut seen_generated = false;
    let mut has_generated = false;
    for (ordinal, node) in e.children().iter().enumerate() {
        let XmlNode::Element(child) = node else {
            retain_as(
                node,
                ordinal,
                profile,
                path,
                "properties",
                "xml:properties-child",
                facets,
            )?;
            continue;
        };
        let local = child.name().local();
        if local == "GeneratedTypes" && typed(child, "GeneratedTypes", expected, uris) {
            if seen_generated {
                return Err(MetadataDecodeError::Duplicate("Properties/GeneratedTypes"));
            }
            seen_generated = true;
            has_generated |= decode_generated_types(
                child,
                generated,
                profile,
                path,
                facets,
                uris,
                expected,
                PROPERTIES_GENERATED_LAYOUT,
            )?;
            continue;
        }
        if (local != "Name" && local != "Synonym") || !typed(child, local, expected, uris) {
            retain_as(
                node,
                ordinal,
                profile,
                path,
                "properties",
                "xml:properties-child",
                facets,
            )?;
            continue;
        }
        if !names.insert(local.to_owned()) {
            return Err(MetadataDecodeError::Duplicate(if local == "Name" {
                "Name"
            } else {
                "Synonym"
            }));
        }
        if local == "Name" {
            retain_unknown_start_tag(
                child,
                &[],
                profile,
                path,
                "properties.name.attributes",
                "xml:name-start-tag-projection",
                facets,
            )?;
        }
        let value = if local == "Synonym" {
            let value = synonym_value(child, uris)?;
            retain_as(
                node,
                ordinal,
                profile,
                path,
                "properties.synonym",
                "xml:synonym-projection",
                facets,
            )?;
            value
        } else {
            CanonicalValue::text(
                CanonicalText::new(
                    &element_text(child)?
                        .ok_or(MetadataDecodeError::Missing("common property text"))?,
                )
                .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
            )
        };
        out.push(
            CanonicalField::named(local, value)
                .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
        );
    }
    if !names.contains("Name") {
        return Err(MetadataDecodeError::Missing("Name"));
    }
    Ok(has_generated)
}

fn synonym_value(
    e: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<CanonicalValue, MetadataDecodeError> {
    let mut values = Vec::new();
    let mut languages = BTreeSet::new();
    let mut mode: Option<Option<String>> = None;
    for node in e.children() {
        let XmlNode::Element(item) = node else {
            continue;
        };
        if item.name().local() != "item" {
            continue;
        }
        let item_uri = uri_of(item, uris).map(str::to_owned);
        if !matches!(item_uri.as_deref(), None | Some(V8_NAMESPACE)) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "Synonym item namespace",
            ));
        }
        match &mode {
            Some(expected) if expected != &item_uri => {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "mixed Synonym namespaces",
                ));
            }
            None => mode = Some(item_uri.clone()),
            _ => {}
        }
        let mut lang = None;
        let mut content = None;
        let mut seen_lang = false;
        let mut seen_content = false;
        for node in item.children() {
            let XmlNode::Element(field) = node else {
                continue;
            };
            if uri_of(field, uris) != item_uri.as_deref() {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "mixed Synonym field namespaces",
                ));
            }
            match field.name().local() {
                "lang" => {
                    if seen_lang {
                        return Err(MetadataDecodeError::Duplicate("Synonym lang"));
                    }
                    seen_lang = true;
                    lang = element_text(field)?
                }
                "content" => {
                    if seen_content {
                        return Err(MetadataDecodeError::Duplicate("Synonym content"));
                    }
                    seen_content = true;
                    content = element_text(field)?
                }
                _ => {}
            }
        }
        let lang = lang.ok_or(MetadataDecodeError::Missing("Synonym item lang"))?;
        if !languages.insert(lang.clone()) {
            return Err(MetadataDecodeError::Duplicate("Synonym item language"));
        }
        let content = content.ok_or(MetadataDecodeError::Missing("Synonym item content"))?;
        values.push(
            CanonicalValue::record(vec![
                CanonicalField::named(
                    "lang",
                    CanonicalValue::text(
                        CanonicalText::new(&lang)
                            .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
                    ),
                )
                .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
                CanonicalField::named(
                    "content",
                    CanonicalValue::text(
                        CanonicalText::new(&content)
                            .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
                    ),
                )
                .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
            ])
            .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
        );
    }
    CanonicalValue::sequence(values).map_err(|x| MetadataDecodeError::Core(x.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn decode_generated_types(
    e: &XmlElement,
    out: &mut Vec<GeneratedType>,
    profile: &ProfileId,
    path: &ObjectPath,
    facets: &mut FacetSet,
    uris: &ResolvedNamespaces,
    expected_type_namespace: Option<&str>,
    layout: GeneratedLayout,
) -> Result<bool, MetadataDecodeError> {
    retain_unknown_start_tag(
        e,
        &[],
        profile,
        path,
        layout.container_anchor,
        layout.container_placement,
        facets,
    )?;
    let mut generated = BTreeSet::new();
    let mut any = false;
    for (ordinal, node) in e.children().iter().enumerate() {
        let XmlNode::Element(child) = node else {
            retain_as(
                node,
                ordinal,
                profile,
                path,
                layout.tail_anchor,
                layout.tail_placement,
                facets,
            )?;
            continue;
        };
        if !typed(child, "GeneratedType", expected_type_namespace, uris) {
            retain_as(
                node,
                ordinal,
                profile,
                path,
                layout.tail_anchor,
                layout.tail_placement,
                facets,
            )?;
            continue;
        }
        let mut type_id = None;
        let mut seen_type_id = false;
        let mut value_id = None;
        let mut seen_value_id = false;
        for node in child.children() {
            if let XmlNode::Element(value) = node {
                if typed(value, "TypeId", expected_type_namespace, uris) {
                    if seen_type_id {
                        return Err(MetadataDecodeError::Duplicate("GeneratedType TypeId"));
                    }
                    seen_type_id = true;
                    type_id = element_text(value)?;
                } else if typed(value, "ValueId", expected_type_namespace, uris) {
                    if seen_value_id {
                        return Err(MetadataDecodeError::Duplicate("GeneratedType ValueId"));
                    }
                    seen_value_id = true;
                    value_id = element_text(value)?;
                }
            }
        }
        let type_id = type_id.ok_or(MetadataDecodeError::Missing("GeneratedType TypeId"))?;
        let mut category = None;
        for attr in child.attributes() {
            if let AttributeKind::Ordinary(name) = attr.kind()
                && name.local() == "category"
                && name.prefix().is_none()
            {
                if category.is_some() {
                    return Err(MetadataDecodeError::Duplicate("GeneratedType category"));
                }
                category = Some(attr.value());
            }
        }
        let category = category.unwrap_or("generated");
        let uuid =
            ObjectUuid::parse(&type_id).map_err(|_| MetadataDecodeError::InvalidUuid(type_id))?;
        if uuid.as_bytes().iter().all(|byte| *byte == 0) {
            // Legacy storage can expose an all-zero placeholder in a generated
            // type slot. It is not a graph identity, so retain the complete XML
            // projection as opaque data instead of inventing a replacement ID.
            retain_as(
                node,
                ordinal,
                profile,
                path,
                layout.projection_anchor,
                layout.projection_placement,
                facets,
            )?;
            continue;
        }
        let value_id = if seen_value_id {
            let value_id =
                value_id.ok_or(MetadataDecodeError::Missing("GeneratedType ValueId text"))?;
            let value_id = ObjectUuid::parse(&value_id)
                .map_err(|_| MetadataDecodeError::InvalidUuid(value_id))?;
            if value_id.as_bytes().iter().all(|byte| *byte == 0) {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "GeneratedType ValueId cannot be nil",
                ));
            }
            Some(value_id)
        } else {
            None
        };
        if !generated.insert(uuid) {
            return Err(MetadataDecodeError::Duplicate("GeneratedType UUID"));
        }
        let generated_type = GeneratedType::new(
            uuid,
            GeneratedTypeKind::new(category)
                .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
        );
        out.push(match value_id {
            Some(value_id) => generated_type.with_value_id(value_id),
            None => generated_type,
        });
        any = true;
        retain_as(
            node,
            ordinal,
            profile,
            path,
            layout.projection_anchor,
            layout.projection_placement,
            facets,
        )?;
    }
    Ok(any)
}

fn uuid_attr(e: &XmlElement) -> Result<ObjectUuid, MetadataDecodeError> {
    let mut value = None;
    for attribute in e.attributes() {
        if let AttributeKind::Ordinary(name) = attribute.kind()
            && name.local() == "uuid"
            && name.prefix().is_none()
        {
            if value.is_some() {
                return Err(MetadataDecodeError::Duplicate("uuid"));
            }
            value = Some(attribute.value());
        }
    }
    let value = value.ok_or(MetadataDecodeError::Missing("uuid"))?;
    ObjectUuid::parse(value).map_err(|_| MetadataDecodeError::InvalidUuid(value.to_owned()))
}
pub(super) fn element_text(e: &XmlElement) -> Result<Option<String>, MetadataDecodeError> {
    let mut value = String::new();
    let mut length = 0usize;
    for node in e.children() {
        match node {
            XmlNode::Text(x) => {
                length = length
                    .checked_add(x.value().len())
                    .ok_or(MetadataDecodeError::ResourceLimit("canonical text"))?;
                if length > ibcmd_core::value::MAX_CANONICAL_TEXT_BYTES {
                    return Err(MetadataDecodeError::ResourceLimit("canonical text"));
                }
                value.push_str(x.value());
            }
            XmlNode::CData(x) => {
                length = length
                    .checked_add(x.value().len())
                    .ok_or(MetadataDecodeError::ResourceLimit("canonical text"))?;
                if length > ibcmd_core::value::MAX_CANONICAL_TEXT_BYTES {
                    return Err(MetadataDecodeError::ResourceLimit("canonical text"));
                }
                value.push_str(x.value());
            }
            _ => return Ok(None),
        }
    }
    Ok(Some(value))
}
fn provenance(
    profile: &ProfileId,
    path: &ObjectPath,
    property: &str,
) -> Result<SourceProvenance, MetadataDecodeError> {
    Ok(SourceProvenance::new(
        profile.clone(),
        CanonicalAnchor::new(
            path.clone(),
            PropertyPath::new(vec![
                PathSegment::name(property)
                    .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
            ])
            .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
        ),
    ))
}
fn retain(
    node: &XmlNode,
    ordinal: usize,
    profile: &ProfileId,
    path: &ObjectPath,
    facets: &mut FacetSet,
) -> Result<(), MetadataDecodeError> {
    retain_as(
        node,
        ordinal,
        profile,
        path,
        "opaque",
        "xml:object-child",
        facets,
    )
}

#[allow(clippy::too_many_arguments)]
fn retain_unknown_start_tag(
    element: &XmlElement,
    known_unprefixed: &[&str],
    profile: &ProfileId,
    path: &ObjectPath,
    anchor: &str,
    placement: &str,
    facets: &mut FacetSet,
) -> Result<(), MetadataDecodeError> {
    let has_unknown = element.attributes().iter().any(|attribute| {
        let AttributeKind::Ordinary(name) = attribute.kind() else {
            return false;
        };
        name.prefix().is_some() || !known_unprefixed.contains(&name.local())
    });
    if !has_unknown {
        return Ok(());
    }
    let preserve_bytes = crate::writer::element_start_len(element, LexicalPolicy::Preserve)
        .map_err(|error| MetadataDecodeError::Xml(error.to_string()))?;
    let normalized_bytes = crate::writer::element_start_len(element, LexicalPolicy::Normalized)
        .map_err(|error| MetadataDecodeError::Xml(error.to_string()))?;
    if normalized_bytes > MAX_METADATA_BYTES {
        return Err(MetadataDecodeError::ResourceLimit("normalized bytes"));
    }
    facets.reserve(preserve_bytes)?;
    let bytes = crate::writer::element_start_to_vec(element, LexicalPolicy::Preserve)
        .map_err(|error| MetadataDecodeError::Xml(error.to_string()))?;
    debug_assert_eq!(bytes.len(), preserve_bytes);
    push_retained(bytes, 0, profile, path, anchor, placement, facets)
}

fn retain_as(
    node: &XmlNode,
    ordinal: usize,
    profile: &ProfileId,
    path: &ObjectPath,
    anchor: &str,
    placement: &str,
    facets: &mut FacetSet,
) -> Result<(), MetadataDecodeError> {
    let preserve_bytes = node_lexical_len(node)?;
    if node_normalized_len(node)? > MAX_METADATA_BYTES {
        return Err(MetadataDecodeError::ResourceLimit("normalized bytes"));
    }
    facets.reserve(preserve_bytes)?;
    let bytes = crate::writer::node_to_vec(node, LexicalPolicy::Preserve)
        .map_err(|x| MetadataDecodeError::Xml(x.to_string()))?;
    debug_assert_eq!(bytes.len(), preserve_bytes);
    push_retained(bytes, ordinal, profile, path, anchor, placement, facets)
}

fn push_family_guard(
    profile: &ProfileId,
    path: &ObjectPath,
    facets: &mut FacetSet,
) -> Result<(), MetadataDecodeError> {
    facets.values.push(
        OpaqueFacet::new(
            provenance(profile, path, "family")?,
            OpaquePlacement::new("xml:family-fallback", 0)
                .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
            Vec::new(),
            MediaKind::new("application/xml").expect("static media kind"),
        )
        .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push_retained(
    bytes: Vec<u8>,
    ordinal: usize,
    profile: &ProfileId,
    path: &ObjectPath,
    anchor: &str,
    placement: &str,
    facets: &mut FacetSet,
) -> Result<(), MetadataDecodeError> {
    facets.values.push(
        OpaqueFacet::new(
            provenance(profile, path, anchor)?,
            OpaquePlacement::new(
                placement,
                u32::try_from(ordinal)
                    .map_err(|_| MetadataDecodeError::ResourceLimit("opaque ordinal"))?,
            )
            .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
            bytes,
            MediaKind::new("application/xml").expect("static media kind"),
        )
        .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
    );
    Ok(())
}

fn node_lexical_len(node: &XmlNode) -> Result<usize, MetadataDecodeError> {
    if let Some(raw) = node.raw() {
        return Ok(raw.len());
    }
    match node {
        XmlNode::Element(element) => element_lexical_len(element),
        XmlNode::Text(x) => escaped_len(x.value(), false),
        XmlNode::CData(x) => Ok(x.value().len() + 12),
        XmlNode::Comment(x) => Ok(x.value().len() + 7),
        XmlNode::ProcessingInstruction(x) => Ok(x.value().len() + 4),
        XmlNode::DocType(x) => Ok(x.value().len() + 11),
    }
}
fn element_lexical_len(element: &XmlElement) -> Result<usize, MetadataDecodeError> {
    let use_raw_start = element.raw_start().is_some()
        && (element.children().is_empty() || element.raw_end().is_some());
    let mut total = if use_raw_start {
        element.raw_start().expect("checked above").len()
    } else {
        let mut size = element.name().raw().len() + 1;
        for attribute in element.attributes() {
            let name = match attribute.kind() {
                AttributeKind::Ordinary(name) => name.raw().len(),
                AttributeKind::Namespace(None) => 5,
                AttributeKind::Namespace(Some(prefix)) => 6 + prefix.len(),
            };
            size = size
                .checked_add(1 + name + 3 + escaped_len(attribute.value(), true)?)
                .ok_or(MetadataDecodeError::ResourceLimit("opaque bytes"))?;
        }
        size
    };
    if element.children().is_empty() {
        let suffix = if use_raw_start {
            element.raw_end().map_or(0, str::len)
        } else {
            2
        };
        return total
            .checked_add(suffix)
            .ok_or(MetadataDecodeError::ResourceLimit("opaque bytes"));
    }
    if !use_raw_start {
        total = total
            .checked_add(1)
            .ok_or(MetadataDecodeError::ResourceLimit("opaque bytes"))?;
    }
    for child in element.children() {
        total = total
            .checked_add(node_lexical_len(child)?)
            .ok_or(MetadataDecodeError::ResourceLimit("opaque bytes"))?;
    }
    let suffix = element
        .raw_end()
        .map_or(element.name().raw().len() + 3, str::len);
    total
        .checked_add(suffix)
        .ok_or(MetadataDecodeError::ResourceLimit("opaque bytes"))
}
fn escaped_len(value: &str, attribute: bool) -> Result<usize, MetadataDecodeError> {
    value.chars().try_fold(0usize, |sum, character| {
        let width = match character {
            '&' => 5,
            '<' | '>' => 4,
            '"' | '\'' if attribute => 6,
            _ => character.len_utf8(),
        };
        sum.checked_add(width)
            .ok_or(MetadataDecodeError::ResourceLimit("bytes"))
    })
}
fn document_lexical_len(document: &XmlDocument) -> Result<usize, MetadataDecodeError> {
    let mut total = usize::from(document.has_utf8_bom()) * 3;
    total = total
        .checked_add(document.declaration_raw().map_or_else(
            || document.declaration().map_or(0, |value| value.len() + 4),
            str::len,
        ))
        .ok_or(MetadataDecodeError::ResourceLimit("bytes"))?;
    for node in document.before_root() {
        total = total
            .checked_add(node_lexical_len(node)?)
            .ok_or(MetadataDecodeError::ResourceLimit("bytes"))?;
    }
    total = total
        .checked_add(element_lexical_len(document.root())?)
        .ok_or(MetadataDecodeError::ResourceLimit("bytes"))?;
    for node in document.after_root() {
        total = total
            .checked_add(node_lexical_len(node)?)
            .ok_or(MetadataDecodeError::ResourceLimit("bytes"))?;
    }
    Ok(total)
}
fn node_normalized_len(node: &XmlNode) -> Result<usize, MetadataDecodeError> {
    match node {
        XmlNode::Element(element) => element_normalized_len(element),
        XmlNode::Text(value) => escaped_len(value.value(), false),
        XmlNode::CData(value) => Ok(value.value().len() + 12),
        XmlNode::Comment(value) => Ok(value.value().len() + 7),
        XmlNode::ProcessingInstruction(value) => Ok(value.value().len() + 4),
        XmlNode::DocType(value) => Ok(value.value().len() + 11),
    }
}
fn element_normalized_len(element: &XmlElement) -> Result<usize, MetadataDecodeError> {
    let mut total = element.name().raw().len() + 1;
    for attribute in element.attributes() {
        let name = match attribute.kind() {
            AttributeKind::Ordinary(name) => name.raw().len(),
            AttributeKind::Namespace(None) => 5,
            AttributeKind::Namespace(Some(prefix)) => 6 + prefix.len(),
        };
        total = total
            .checked_add(1 + name + 3 + escaped_len(attribute.value(), true)?)
            .ok_or(MetadataDecodeError::ResourceLimit("normalized bytes"))?;
    }
    if element.children().is_empty() {
        return total
            .checked_add(2)
            .ok_or(MetadataDecodeError::ResourceLimit("normalized bytes"));
    }
    total = total
        .checked_add(1)
        .ok_or(MetadataDecodeError::ResourceLimit("normalized bytes"))?;
    for child in element.children() {
        total = total
            .checked_add(node_normalized_len(child)?)
            .ok_or(MetadataDecodeError::ResourceLimit("normalized bytes"))?;
    }
    total
        .checked_add(element.name().raw().len() + 3)
        .ok_or(MetadataDecodeError::ResourceLimit("normalized bytes"))
}
fn document_normalized_len(document: &XmlDocument) -> Result<usize, MetadataDecodeError> {
    let mut total = document.declaration().map_or(0, |value| value.len() + 4);
    for node in document.before_root() {
        total = total
            .checked_add(node_normalized_len(node)?)
            .ok_or(MetadataDecodeError::ResourceLimit("normalized bytes"))?;
    }
    total = total
        .checked_add(element_normalized_len(document.root())?)
        .ok_or(MetadataDecodeError::ResourceLimit("normalized bytes"))?;
    for node in document.after_root() {
        total = total
            .checked_add(node_normalized_len(node)?)
            .ok_or(MetadataDecodeError::ResourceLimit("normalized bytes"))?;
    }
    Ok(total)
}

type NamespaceScope = BTreeMap<String, Rc<str>>;
pub(super) type ResolvedNamespaces = BTreeMap<usize, Option<Rc<str>>>;

fn element_key(element: &XmlElement) -> usize {
    element as *const XmlElement as usize
}
pub(super) fn uri_of<'a>(element: &XmlElement, uris: &'a ResolvedNamespaces) -> Option<&'a str> {
    uris.get(&element_key(element))
        .and_then(|value| value.as_deref())
}
pub(super) fn typed(
    element: &XmlElement,
    local: &str,
    expected: Option<&str>,
    uris: &ResolvedNamespaces,
) -> bool {
    element.name().local() == local && uri_of(element, uris) == expected
}

pub(super) fn resolve_namespaces(
    root: &XmlElement,
) -> Result<ResolvedNamespaces, MetadataDecodeError> {
    let mut scope = NamespaceScope::new();
    scope.insert("xml".to_owned(), Rc::from(XML_NAMESPACE));
    let mut uris = ResolvedNamespaces::new();
    collect_namespaces(root, &mut scope, &mut uris)?;
    Ok(uris)
}

fn collect_namespaces(
    element: &XmlElement,
    scope: &mut NamespaceScope,
    uris: &mut ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let mut seen = BTreeSet::new();
    let mut changes = Vec::new();
    for attribute in element.attributes() {
        match attribute.kind() {
            AttributeKind::Ordinary(name)
                if name.raw() == "xmlns" || name.prefix() == Some("xmlns") =>
            {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "namespace declaration encoded as ordinary attribute",
                ));
            }
            AttributeKind::Namespace(prefix) => {
                if prefix
                    .as_deref()
                    .is_some_and(|prefix| !crate::node::valid_name(prefix))
                {
                    return Err(MetadataDecodeError::InvalidEnvelope(
                        "invalid namespace prefix",
                    ));
                }
                let key = prefix.as_deref().unwrap_or_default();
                if !seen.insert(key) {
                    return Err(MetadataDecodeError::Duplicate("namespace declaration"));
                }
                let uri = attribute.value();
                if prefix.is_some() && uri.is_empty()
                    || key == "xmlns"
                    || uri == XMLNS_NAMESPACE
                    || uri == XML_NAMESPACE && key != "xml"
                    || key == "xml" && uri != XML_NAMESPACE
                {
                    return Err(MetadataDecodeError::InvalidEnvelope(
                        "invalid reserved namespace binding",
                    ));
                }
                let owned_key = key.to_owned();
                let previous = if uri.is_empty() {
                    scope.remove(key)
                } else {
                    scope.insert(owned_key.clone(), Rc::from(uri))
                };
                changes.push((owned_key, previous));
            }
            AttributeKind::Ordinary(_) => {}
        }
    }
    let uri = match element.name().prefix() {
        Some(prefix) => Some(scope.get(prefix).cloned().ok_or(
            MetadataDecodeError::InvalidEnvelope("unbound element prefix"),
        )?),
        None => scope.get("").cloned(),
    };
    uris.insert(element_key(element), uri);
    {
        // Attribute expanded names use their explicit prefix binding only;
        // unlike element names, an unprefixed attribute has no namespace.
        // Keep these borrows scoped before child traversal mutates `scope`.
        let mut expanded_names = HashSet::with_capacity(element.attributes().len());
        for attribute in element.attributes() {
            if let AttributeKind::Ordinary(name) = attribute.kind() {
                let namespace = match name.prefix() {
                    Some(prefix) => Some(scope.get(prefix).ok_or(
                        MetadataDecodeError::InvalidEnvelope("unbound attribute prefix"),
                    )?),
                    None => None,
                };
                let key = (namespace.map(|uri| uri.as_ref()), name.local());
                if !expanded_names.insert(key) {
                    return Err(MetadataDecodeError::Duplicate("attribute expanded name"));
                }
            }
        }
    }
    for node in element.children() {
        if let XmlNode::Element(child) = node {
            collect_namespaces(child, scope, uris)?;
        }
    }
    for (key, previous) in changes.into_iter().rev() {
        if let Some(previous) = previous {
            scope.insert(key, previous);
        } else {
            scope.remove(key.as_str());
        }
    }
    Ok(())
}
#[derive(Default)]
struct Budget {
    nodes: usize,
    bytes: usize,
    attributes: usize,
    namespaces: usize,
    namespace_bytes: usize,
}
fn checked_add(
    target: &mut usize,
    value: usize,
    limit: usize,
    what: &'static str,
) -> Result<(), MetadataDecodeError> {
    *target = target
        .checked_add(value)
        .ok_or(MetadataDecodeError::ResourceLimit(what))?;
    if *target > limit {
        return Err(MetadataDecodeError::ResourceLimit(what));
    }
    Ok(())
}
fn check_document(document: &XmlDocument) -> Result<(), MetadataDecodeError> {
    let mut budget = Budget::default();
    if document.has_utf8_bom() {
        checked_add(&mut budget.bytes, 3, MAX_METADATA_BYTES, "bytes")?;
    }
    if let Some(value) = document
        .declaration_raw()
        .or_else(|| document.declaration())
    {
        checked_add(&mut budget.bytes, value.len(), MAX_METADATA_BYTES, "bytes")?;
    }
    for node in document.before_root().iter().chain(document.after_root()) {
        check_node(node, 0, &mut budget)?;
    }
    check_tree(document.root(), 0, &mut budget)?;
    if document_lexical_len(document)? > MAX_METADATA_BYTES {
        return Err(MetadataDecodeError::ResourceLimit("bytes"));
    }
    if document_normalized_len(document)? > MAX_METADATA_BYTES {
        return Err(MetadataDecodeError::ResourceLimit("normalized bytes"));
    }
    crate::writer::validate_document(document)
        .map_err(|error| MetadataDecodeError::Xml(error.to_string()))?;
    Ok(())
}
fn check_node(node: &XmlNode, depth: usize, b: &mut Budget) -> Result<(), MetadataDecodeError> {
    if let XmlNode::Element(element) = node {
        return check_tree(element, depth, b);
    }
    checked_add(&mut b.nodes, 1, MAX_METADATA_NODES, "nodes")?;
    if let Some(raw) = node.raw() {
        return checked_add(&mut b.bytes, raw.len(), MAX_METADATA_BYTES, "bytes");
    }
    match node {
        XmlNode::Element(_) => unreachable!("handled above"),
        XmlNode::Text(x) => checked_add(&mut b.bytes, x.value().len(), MAX_METADATA_BYTES, "bytes"),
        XmlNode::CData(x) => {
            checked_add(&mut b.bytes, x.value().len(), MAX_METADATA_BYTES, "bytes")
        }
        XmlNode::Comment(x) => {
            checked_add(&mut b.bytes, x.value().len(), MAX_METADATA_BYTES, "bytes")
        }
        XmlNode::ProcessingInstruction(x) | XmlNode::DocType(x) => {
            checked_add(&mut b.bytes, x.value().len(), MAX_METADATA_BYTES, "bytes")
        }
    }
}
fn check_tree(e: &XmlElement, depth: usize, b: &mut Budget) -> Result<(), MetadataDecodeError> {
    if depth > MAX_METADATA_DEPTH {
        return Err(MetadataDecodeError::ResourceLimit("depth"));
    }
    checked_add(&mut b.nodes, 1, MAX_METADATA_NODES, "nodes")?;
    if let Some(raw) = e.raw_start() {
        checked_add(&mut b.bytes, raw.len(), MAX_METADATA_BYTES, "bytes")?;
    }
    if let Some(raw) = e.raw_end() {
        checked_add(&mut b.bytes, raw.len(), MAX_METADATA_BYTES, "bytes")?;
    }
    checked_add(
        &mut b.bytes,
        e.name().raw().len(),
        MAX_METADATA_BYTES,
        "bytes",
    )?;
    for attribute in e.attributes() {
        checked_add(&mut b.attributes, 1, MAX_METADATA_ATTRIBUTES, "attributes")?;
        if let AttributeKind::Ordinary(name) = attribute.kind()
            && (name.raw() == "xmlns" || name.prefix() == Some("xmlns"))
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "namespace declaration encoded as ordinary attribute",
            ));
        }
        if let AttributeKind::Namespace(Some(prefix)) = attribute.kind()
            && !crate::node::valid_name(prefix)
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "invalid namespace prefix",
            ));
        }
        if matches!(attribute.kind(), AttributeKind::Namespace(_)) {
            checked_add(&mut b.namespaces, 1, MAX_METADATA_NAMESPACES, "namespaces")?;
            let prefix_len = match attribute.kind() {
                AttributeKind::Namespace(prefix) => prefix.as_deref().map_or(0, str::len),
                _ => 0,
            };
            checked_add(
                &mut b.namespace_bytes,
                prefix_len + attribute.value().len(),
                MAX_METADATA_NAMESPACE_BYTES,
                "namespace bytes",
            )?;
        }
        if e.raw_start().is_none() {
            let name_len = match attribute.kind() {
                AttributeKind::Ordinary(name) => name.raw().len(),
                AttributeKind::Namespace(prefix) => 5 + prefix.as_deref().map_or(0, str::len),
            };
            checked_add(&mut b.bytes, name_len, MAX_METADATA_BYTES, "bytes")?;
            checked_add(
                &mut b.bytes,
                attribute.value().len(),
                MAX_METADATA_BYTES,
                "bytes",
            )?;
        }
    }
    for n in e.children() {
        check_node(n, depth + 1, b)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use ibcmd_core::artifact::ProfileId;
    use ibcmd_core::diagnostic::{ObjectPath, PathSegment};

    use crate::{Attribute, MetadataEncodeError, MetadataRegistry, QName, XmlDocument, XmlReader};

    use super::*;

    fn path() -> ObjectPath {
        ObjectPath::new(vec![PathSegment::name("modules").unwrap()]).unwrap()
    }
    fn profile() -> ProfileId {
        ProfileId::parse("xml:2.20").unwrap()
    }

    fn metadata_ast(root_attributes: Vec<Attribute>) -> XmlDocument {
        let name = XmlElement::with_parts(
            QName::new("Name").unwrap(),
            Vec::new(),
            vec![XmlNode::text("X")],
        );
        let properties = XmlElement::with_parts(
            QName::new("Properties").unwrap(),
            Vec::new(),
            vec![XmlNode::Element(name)],
        );
        let object = XmlElement::with_parts(
            QName::new("X").unwrap(),
            vec![Attribute::ordinary(
                QName::new("uuid").unwrap(),
                "11111111-1111-4111-8111-111111111111",
            )],
            vec![XmlNode::Element(properties)],
        );
        XmlDocument::new(XmlElement::with_parts(
            QName::new("MetaDataObject").unwrap(),
            root_attributes,
            vec![XmlNode::Element(object)],
        ))
    }

    fn replace_family_guard(
        root: &CanonicalObject,
        guard_path: &ObjectPath,
        bytes: Vec<u8>,
    ) -> CanonicalObject {
        let mut replacement = Some(bytes);
        let facets = root
            .opaque_facets()
            .as_slice()
            .iter()
            .map(|facet| {
                if facet.placement().kind().as_str() == "xml:family-fallback" {
                    OpaqueFacet::new(
                        provenance(&profile(), guard_path, "family").unwrap(),
                        facet.placement().clone(),
                        replacement.take().unwrap(),
                        facet.media_kind().clone(),
                    )
                    .unwrap()
                } else {
                    facet.clone()
                }
            })
            .collect();
        let mut parts = CanonicalObjectParts::new(
            root.identity().clone(),
            root.kind().clone(),
            root.provenance().clone(),
        );
        parts.owner = root.owner();
        parts.properties = root.properties().to_vec();
        parts.references = root.references().to_vec();
        parts.generated_types = root.generated_types().to_vec();
        parts.assets = root.assets().to_vec();
        parts.opaque_facets = OpaqueFacets::new(facets).unwrap();
        CanonicalObject::new(parts).unwrap()
    }

    #[test]
    fn typed_envelope_keeps_unknown_slots_and_same_profile_bytes() {
        let input = b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' version='2.20'><CommonModule uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>Portable</Name><Synonym><v8:item xmlns:v8='http://v8.1c.ru/8.1/data/core'><v8:lang>ru</v8:lang><v8:content>\xD0\x9F</v8:content></v8:item></Synonym><Future x='1'/></Properties><!-- retained --></CommonModule></MetaDataObject>";
        let doc = XmlReader::from_slice(input).unwrap();
        let envelope = decode_metadata_envelope(&doc, profile(), path()).unwrap();
        assert_eq!(envelope.root().kind().as_str(), "CommonModule");
        assert_eq!(envelope.root().properties().len(), 2);
        assert!(!envelope.root().opaque_facets().is_empty());
        assert_eq!(
            MetadataRegistry::default()
                .encode(&envelope, &profile())
                .unwrap(),
            input
        );
    }

    #[test]
    fn typed_slot_unknown_attributes_have_linear_start_tag_projections() {
        let input = b"<!--prolog--><MetaDataObject version='2.20' future='wrapper'><!--inside-before--><X uuid='11111111-1111-4111-8111-111111111111' future='object'><Properties future='properties'><Name future='name'>X</Name><GeneratedTypes future='properties-generated'/></Properties><GeneratedTypes future='direct-generated'/><InternalInfo future='internal-info'/><ChildObjects future='child-objects'/></X><?inside after?></MetaDataObject><!--epilog-->";
        let document = XmlReader::from_slice(input).unwrap();
        let envelope = decode_metadata_envelope(&document, profile(), path()).unwrap();
        let expected = [
            (
                "xml:metadata-object-start-tag-projection",
                "metadata_object.attributes",
                b"<MetaDataObject version='2.20' future='wrapper'>".as_slice(),
            ),
            (
                "xml:object-start-tag-projection",
                "object.attributes",
                b"<X uuid='11111111-1111-4111-8111-111111111111' future='object'>".as_slice(),
            ),
            (
                "xml:properties-start-tag-projection",
                "properties.attributes",
                b"<Properties future='properties'>".as_slice(),
            ),
            (
                "xml:name-start-tag-projection",
                "properties.name.attributes",
                b"<Name future='name'>".as_slice(),
            ),
            (
                "xml:properties-generated-types-start-tag-projection",
                "properties.generated_types.attributes",
                b"<GeneratedTypes future='properties-generated'/>".as_slice(),
            ),
            (
                "xml:generated-types-start-tag-projection",
                "generated_types.attributes",
                b"<GeneratedTypes future='direct-generated'/>".as_slice(),
            ),
            (
                "xml:internal-info-start-tag-projection",
                "internal_info.attributes",
                b"<InternalInfo future='internal-info'/>".as_slice(),
            ),
            (
                "xml:child-objects-start-tag-projection",
                "child_objects.attributes",
                b"<ChildObjects future='child-objects'/>".as_slice(),
            ),
        ];
        let facets = envelope.root().opaque_facets().as_slice();
        for (placement, anchor, expected_bytes) in expected {
            let facet = facets
                .iter()
                .find(|facet| facet.placement().kind().as_str() == placement)
                .unwrap();
            let bytes = facet.emit_permit(&profile()).unwrap().bytes();
            assert!(!bytes.is_empty());
            assert_eq!(bytes, expected_bytes);
            assert_eq!(
                facet.anchor().property_path().segments()[0].as_name(),
                Some(anchor)
            );
        }
        for (placement, expected_bytes) in [
            ("xml:document-prolog-node", b"<!--prolog-->".as_slice()),
            ("xml:document-epilog-node", b"<!--epilog-->".as_slice()),
        ] {
            let facet = facets
                .iter()
                .find(|facet| facet.placement().kind().as_str() == placement)
                .unwrap();
            assert_eq!(
                facet.emit_permit(&profile()).unwrap().bytes(),
                expected_bytes
            );
            assert_eq!(facet.anchor().object_path(), &path());
        }
        let wrapper_nodes: Vec<_> = facets
            .iter()
            .filter(|facet| facet.placement().kind().as_str() == "xml:metadata-object-child")
            .map(|facet| facet.emit_permit(&profile()).unwrap().bytes())
            .collect();
        assert!(wrapper_nodes.contains(&b"<!--inside-before-->".as_slice()));
        assert!(wrapper_nodes.contains(&b"<?inside after?>".as_slice()));
        let retained: u64 = facets
            .iter()
            .filter(|facet| facet.placement().kind().as_str() != "xml:family-fallback")
            .map(OpaqueFacet::byte_len)
            .sum();
        assert!(retained <= input.len() as u64);
        assert_eq!(
            MetadataRegistry::default()
                .encode(&envelope, &profile())
                .unwrap(),
            input
        );
    }

    #[test]
    fn public_ast_start_projection_is_normalized_and_bounded() {
        let document = metadata_ast(vec![Attribute::ordinary(
            QName::new("future").unwrap(),
            "wrapper",
        )]);
        let envelope = decode_metadata_envelope(&document, profile(), path()).unwrap();
        let facet = envelope
            .root()
            .opaque_facets()
            .as_slice()
            .iter()
            .find(|facet| {
                facet.placement().kind().as_str() == "xml:metadata-object-start-tag-projection"
            })
            .unwrap();
        assert_eq!(
            facet.emit_permit(&profile()).unwrap().bytes(),
            b"<MetaDataObject future=\"wrapper\">"
        );
    }

    #[test]
    fn self_closing_start_projection_normalizes_after_children_are_added() {
        let parsed = XmlReader::from_slice(b"<MetaDataObject future='wrapper'/>").unwrap();
        let semantic_child = metadata_ast(Vec::new()).root().children()[0].clone();
        let document = XmlDocument::new(parsed.root().with_children(vec![semantic_child]));
        let expected_start = b"<MetaDataObject future=\"wrapper\">";

        assert_eq!(
            crate::writer::element_start_to_vec(document.root(), LexicalPolicy::Preserve).unwrap(),
            expected_start
        );
        let emitted = crate::XmlWriter::to_vec(&document, LexicalPolicy::Preserve).unwrap();
        assert!(emitted.starts_with(expected_start));

        let envelope = decode_metadata_envelope(&document, profile(), path()).unwrap();
        let facet = envelope
            .root()
            .opaque_facets()
            .as_slice()
            .iter()
            .find(|facet| {
                facet.placement().kind().as_str() == "xml:metadata-object-start-tag-projection"
            })
            .unwrap();
        assert_eq!(
            facet.emit_permit(&profile()).unwrap().bytes(),
            expected_start
        );
    }

    #[test]
    fn different_profile_is_path_addressed_opaque_failure() {
        let doc = XmlReader::from_slice(b"<MetaDataObject><FutureFamily uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties></FutureFamily></MetaDataObject>").unwrap();
        let envelope = decode_metadata_envelope(&doc, profile(), path()).unwrap();
        let other = ProfileId::parse("xml:2.21").unwrap();
        let error = MetadataRegistry::default()
            .encode(&envelope, &other)
            .unwrap_err();
        let MetadataEncodeError::Opaque(error) = error else {
            panic!("expected opaque error")
        };
        assert_eq!(error.diagnostic().object_path(), &path());
    }

    #[test]
    fn duplicate_or_bad_uuid_fails_closed() {
        for xml in [
            b"<MetaDataObject><X uuid='bad'><Properties><Name>X</Name></Properties></X></MetaDataObject>".as_slice(),
            b"<MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111' uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties></X></MetaDataObject>".as_slice(),
        ] {
            if let Ok(doc) = XmlReader::from_slice(xml) {
                assert!(decode_metadata_envelope(&doc, profile(), path()).is_err());
            }
        }
    }

    #[test]
    fn namespace_alias_is_typed_and_spoofed_properties_are_not() {
        let aliased = XmlReader::from_slice(b"<m:MetaDataObject xmlns:m='http://v8.1c.ru/8.3/MDClasses'><m:X uuid='11111111-1111-4111-8111-111111111111'><m:Properties><m:Name>X</m:Name></m:Properties></m:X></m:MetaDataObject>").unwrap();
        assert!(decode_metadata_envelope(&aliased, profile(), path()).is_ok());
        let evil = XmlReader::from_slice(b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses'><X uuid='11111111-1111-4111-8111-111111111111'><Properties xmlns='urn:evil'><Name>X</Name></Properties></X></MetaDataObject>").unwrap();
        assert!(matches!(
            decode_metadata_envelope(&evil, profile(), path()),
            Err(MetadataDecodeError::Missing("Name"))
        ));
    }

    #[test]
    fn ordinary_xmlns_attributes_are_rejected_before_namespace_resolution() {
        for raw_name in ["xmlns", "xmlns:future"] {
            let document = metadata_ast(vec![Attribute::ordinary(
                QName::new(raw_name).unwrap(),
                "urn:evil",
            )]);
            assert!(matches!(
                check_document(&document),
                Err(MetadataDecodeError::InvalidEnvelope(
                    "namespace declaration encoded as ordinary attribute"
                ))
            ));
            assert!(matches!(
                resolve_namespaces(document.root()),
                Err(MetadataDecodeError::InvalidEnvelope(
                    "namespace declaration encoded as ordinary attribute"
                ))
            ));
            assert!(matches!(
                decode_metadata_envelope(&document, profile(), path()),
                Err(MetadataDecodeError::InvalidEnvelope(
                    "namespace declaration encoded as ordinary attribute"
                ))
            ));
        }

        let proper = metadata_ast(vec![Attribute::namespace(None, MD_NAMESPACE)]);
        assert!(decode_metadata_envelope(&proper, profile(), path()).is_ok());
    }

    #[test]
    fn invalid_programmatic_namespace_prefixes_fail_all_preflights() {
        for prefix in ["", "a:b", "1bad", "bad prefix"] {
            let document = metadata_ast(vec![Attribute::namespace(
                Some(prefix.to_owned()),
                "urn:test",
            )]);
            assert!(crate::XmlWriter::to_vec(&document, LexicalPolicy::Preserve).is_err());
            assert!(matches!(
                check_document(&document),
                Err(MetadataDecodeError::InvalidEnvelope(
                    "invalid namespace prefix"
                ))
            ));
            assert!(matches!(
                resolve_namespaces(document.root()),
                Err(MetadataDecodeError::InvalidEnvelope(
                    "invalid namespace prefix"
                ))
            ));
            assert!(decode_metadata_envelope(&document, profile(), path()).is_err());
        }

        let valid = metadata_ast(vec![Attribute::namespace(
            Some("future".to_owned()),
            "urn:test",
        )]);
        assert!(crate::XmlWriter::to_vec(&valid, LexicalPolicy::Normalized).is_ok());
        assert!(decode_metadata_envelope(&valid, profile(), path()).is_ok());
    }

    #[test]
    fn parsed_duplicate_attribute_expanded_names_fail_before_retention() {
        let document = XmlReader::from_slice(b"<MetaDataObject xmlns:a='urn:same' xmlns:b='urn:same' a:q='one' b:q='two'><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties></X></MetaDataObject>").unwrap();
        assert!(crate::XmlWriter::to_vec(&document, LexicalPolicy::Preserve).is_ok());
        assert!(matches!(
            resolve_namespaces(document.root()),
            Err(MetadataDecodeError::Duplicate("attribute expanded name"))
        ));
        assert!(matches!(
            decode_metadata_envelope(&document, profile(), path()),
            Err(MetadataDecodeError::Duplicate("attribute expanded name"))
        ));
    }

    #[test]
    fn public_ast_attribute_expanded_names_are_unique_by_uri_and_local() {
        let duplicate = metadata_ast(vec![
            Attribute::namespace(Some("a".to_owned()), "urn:same"),
            Attribute::namespace(Some("b".to_owned()), "urn:same"),
            Attribute::ordinary(QName::new("a:q").unwrap(), "one"),
            Attribute::ordinary(QName::new("b:q").unwrap(), "two"),
        ]);
        assert!(crate::XmlWriter::to_vec(&duplicate, LexicalPolicy::Normalized).is_ok());
        assert!(matches!(
            decode_metadata_envelope(&duplicate, profile(), path()),
            Err(MetadataDecodeError::Duplicate("attribute expanded name"))
        ));

        let distinct_uris = metadata_ast(vec![
            Attribute::namespace(Some("a".to_owned()), "urn:a"),
            Attribute::namespace(Some("b".to_owned()), "urn:b"),
            Attribute::ordinary(QName::new("a:q").unwrap(), "one"),
            Attribute::ordinary(QName::new("b:q").unwrap(), "two"),
        ]);
        assert!(decode_metadata_envelope(&distinct_uris, profile(), path()).is_ok());

        let default_does_not_apply_to_attributes = metadata_ast(vec![
            Attribute::namespace(None, MD_NAMESPACE),
            Attribute::namespace(Some("a".to_owned()), MD_NAMESPACE),
            Attribute::ordinary(QName::new("q").unwrap(), "unprefixed"),
            Attribute::ordinary(QName::new("a:q").unwrap(), "prefixed"),
        ]);
        assert!(
            decode_metadata_envelope(&default_does_not_apply_to_attributes, profile(), path())
                .is_ok()
        );
    }

    #[test]
    fn inherited_namespace_uris_are_pointer_shared() {
        let large_uri = format!("urn:shared:{}", "x".repeat(65_536));
        let children = (0..256)
            .map(|_| {
                XmlNode::Element(XmlElement::with_parts(
                    QName::new("Child").unwrap(),
                    Vec::new(),
                    Vec::new(),
                ))
            })
            .collect();
        let root = XmlElement::with_parts(
            QName::new("Root").unwrap(),
            vec![Attribute::namespace(None, large_uri)],
            children,
        );
        let uris = resolve_namespaces(&root).unwrap();
        let root_uri = uris.get(&element_key(&root)).unwrap().as_ref().unwrap();
        for node in root.children() {
            let XmlNode::Element(child) = node else {
                unreachable!()
            };
            let child_uri = uris.get(&element_key(child)).unwrap().as_ref().unwrap();
            assert!(Rc::ptr_eq(root_uri, child_uri));
        }
        assert_eq!(Rc::strong_count(root_uri), root.children().len() + 1);
    }

    #[test]
    fn child_objects_are_preorder_and_owned() {
        let xml = b"<MetaDataObject><Root uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>R</Name></Properties><ChildObjects><A uuid='22222222-2222-4222-8222-222222222222'><Properties><Name>A</Name></Properties><ChildObjects><N uuid='33333333-3333-4333-8333-333333333333'><Properties><Name>N</Name></Properties></N></ChildObjects></A><B uuid='44444444-4444-4444-8444-444444444444'><Properties><Name>B</Name></Properties></B></ChildObjects></Root></MetaDataObject>";
        let envelope =
            decode_metadata_envelope(&XmlReader::from_slice(xml).unwrap(), profile(), path())
                .unwrap();
        assert_eq!(envelope.descendants().len(), 3);
        assert_eq!(envelope.descendants()[0].kind().as_str(), "A");
        assert_eq!(envelope.descendants()[1].kind().as_str(), "N");
        assert_eq!(envelope.descendants()[2].kind().as_str(), "B");
        assert_eq!(
            envelope.descendants()[0].owner(),
            Some(envelope.root().identity().uuid())
        );
        assert_eq!(
            envelope.descendants()[1].owner(),
            Some(envelope.descendants()[0].identity().uuid())
        );
    }

    #[test]
    fn foreign_child_object_is_opaque_and_typed_indexes_ignore_raw_nodes() {
        let input = b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' xmlns:f='urn:future'><Root uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>R</Name></Properties><ChildObjects>\n <!--before--><f:Future/>\n <A uuid='22222222-2222-4222-8222-222222222222'><Properties><Name>A</Name></Properties></A> <!--between-->\n<B uuid='33333333-3333-4333-8333-333333333333'><Properties><Name>B</Name></Properties></B></ChildObjects></Root></MetaDataObject>";
        let document = XmlReader::from_slice(input).unwrap();
        let envelope = decode_metadata_envelope(&document, profile(), path()).unwrap();
        assert_eq!(envelope.descendants().len(), 2);
        assert_eq!(
            envelope.descendants()[0]
                .identity()
                .path()
                .segments()
                .last()
                .and_then(PathSegment::as_index),
            Some(0)
        );
        assert_eq!(
            envelope.descendants()[1]
                .identity()
                .path()
                .segments()
                .last()
                .and_then(PathSegment::as_index),
            Some(1)
        );
        let foreign = envelope
            .root()
            .opaque_facets()
            .as_slice()
            .iter()
            .find(|facet| {
                facet.placement().kind().as_str() == "xml:child-objects-child"
                    && facet.placement().ordinal() == 2
            })
            .unwrap();
        assert_eq!(
            foreign.emit_permit(&profile()).unwrap().bytes(),
            b"<f:Future/>"
        );
        assert_eq!(
            MetadataRegistry::default()
                .encode(&envelope, &profile())
                .unwrap(),
            input
        );
    }

    #[test]
    fn duplicate_container_and_malformed_child_fail() {
        for xml in [
            b"<MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties><Properties/></X></MetaDataObject>".as_slice(),
            b"<MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties><ChildObjects><Y><Properties><Name>Y</Name></Properties></Y></ChildObjects></X></MetaDataObject>".as_slice(),
        ] { let doc = XmlReader::from_slice(xml).unwrap(); assert!(decode_metadata_envelope(&doc, profile(), path()).is_err()); }
    }

    #[test]
    fn compact_family_always_has_fallback_guard() {
        let doc = XmlReader::from_slice(b"<MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties></X></MetaDataObject>").unwrap();
        let envelope = decode_metadata_envelope(&doc, profile(), path()).unwrap();
        let guard = envelope
            .root()
            .opaque_facets()
            .as_slice()
            .iter()
            .find(|facet| facet.placement().kind().as_str() == "xml:family-fallback")
            .unwrap();
        assert_eq!(guard.byte_len(), 0);
        assert_eq!(guard.anchor().object_path(), &path());
    }

    #[test]
    fn generated_projection_retains_complete_node() {
        let doc = XmlReader::from_slice(b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' xmlns:xr='http://v8.1c.ru/8.3/xcf/readable'><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties><InternalInfo><xr:GeneratedType category='ref'><xr:TypeId>22222222-2222-4222-8222-222222222222</xr:TypeId><xr:ValueId>33333333-3333-4333-8333-333333333333</xr:ValueId></xr:GeneratedType></InternalInfo></X></MetaDataObject>").unwrap();
        let envelope = decode_metadata_envelope(&doc, profile(), path()).unwrap();
        assert_eq!(envelope.root().generated_types().len(), 1);
        assert_eq!(
            envelope.root().generated_types()[0]
                .value_id()
                .unwrap()
                .to_string(),
            "33333333-3333-4333-8333-333333333333"
        );
        assert!(
            envelope
                .root()
                .opaque_facets()
                .as_slice()
                .iter()
                .any(|facet| facet.placement().kind().as_str()
                    == "xml:internal-info-generated-type-projection")
        );
    }

    #[test]
    fn duplicate_generated_type_id_fails_closed() {
        let doc = XmlReader::from_slice(b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' xmlns:xr='http://v8.1c.ru/8.3/xcf/readable'><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties><InternalInfo><xr:GeneratedType><xr:TypeId>22222222-2222-4222-8222-222222222222</xr:TypeId><xr:TypeId>33333333-3333-4333-8333-333333333333</xr:TypeId></xr:GeneratedType></InternalInfo></X></MetaDataObject>").unwrap();
        assert!(matches!(
            decode_metadata_envelope(&doc, profile(), path()),
            Err(MetadataDecodeError::Duplicate("GeneratedType TypeId"))
        ));
    }

    #[test]
    fn duplicate_or_nil_generated_value_id_fails_closed() {
        let duplicate = XmlReader::from_slice(b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' xmlns:xr='http://v8.1c.ru/8.3/xcf/readable'><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties><InternalInfo><xr:GeneratedType><xr:TypeId>22222222-2222-4222-8222-222222222222</xr:TypeId><xr:ValueId>33333333-3333-4333-8333-333333333333</xr:ValueId><xr:ValueId>44444444-4444-4444-8444-444444444444</xr:ValueId></xr:GeneratedType></InternalInfo></X></MetaDataObject>").unwrap();
        assert!(matches!(
            decode_metadata_envelope(&duplicate, profile(), path()),
            Err(MetadataDecodeError::Duplicate("GeneratedType ValueId"))
        ));
        let nil = XmlReader::from_slice(b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' xmlns:xr='http://v8.1c.ru/8.3/xcf/readable'><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties><InternalInfo><xr:GeneratedType><xr:TypeId>22222222-2222-4222-8222-222222222222</xr:TypeId><xr:ValueId>00000000-0000-0000-0000-000000000000</xr:ValueId></xr:GeneratedType></InternalInfo></X></MetaDataObject>").unwrap();
        assert!(matches!(
            decode_metadata_envelope(&nil, profile(), path()),
            Err(MetadataDecodeError::InvalidEnvelope(
                "GeneratedType ValueId cannot be nil"
            ))
        ));
    }

    #[test]
    fn duplicate_presence_is_independent_of_first_value_parse() {
        let cases: &[(&[u8], &str)] = &[
            (
                b"<MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name><GeneratedTypes/><GeneratedTypes/></Properties></X></MetaDataObject>",
                "Properties/GeneratedTypes",
            ),
            (
                b"<MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name><Synonym><item><lang><Bad/></lang><lang>ru</lang><content>X</content></item></Synonym></Properties></X></MetaDataObject>",
                "Synonym lang",
            ),
            (
                b"<MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name><Synonym><item><lang>ru</lang><content><Bad/></content><content>X</content></item></Synonym></Properties></X></MetaDataObject>",
                "Synonym content",
            ),
            (
                b"<MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties><GeneratedTypes><GeneratedType><TypeId><Bad/></TypeId><TypeId>22222222-2222-4222-8222-222222222222</TypeId></GeneratedType></GeneratedTypes></X></MetaDataObject>",
                "GeneratedType TypeId",
            ),
        ];
        for (xml, duplicate) in cases {
            let document = XmlReader::from_slice(xml).unwrap();
            assert!(matches!(
                decode_metadata_envelope(&document, profile(), path()),
                Err(MetadataDecodeError::Duplicate(actual)) if actual == *duplicate
            ));
        }
    }

    #[test]
    fn comments_cdata_pi_and_inherited_prefix_round_trip_exactly() {
        let input = b"<MetaDataObject xmlns:p='urn:future'><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties>text<!--c--><![CDATA[d]]><?go now?><p:Future/></X></MetaDataObject>";
        let doc = XmlReader::from_slice(input).unwrap();
        let envelope = decode_metadata_envelope(&doc, profile(), path()).unwrap();
        assert!(
            envelope
                .root()
                .opaque_facets()
                .as_slice()
                .iter()
                .filter(|facet| facet.placement().kind().as_str() != "xml:family-fallback")
                .all(|facet| facet.byte_len() > 0)
        );
        assert_eq!(
            MetadataRegistry::default()
                .encode(&envelope, &profile())
                .unwrap(),
            input
        );
    }

    #[test]
    fn non_element_node_budget_rejects_before_fallback_clone() {
        let mut xml = String::from(
            "<MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties>",
        );
        for _ in 0..=MAX_METADATA_NODES {
            xml.push_str("<!--x-->");
        }
        xml.push_str("</X></MetaDataObject>");
        let document = XmlReader::from_slice(xml.as_bytes()).unwrap();
        assert!(matches!(
            decode_metadata_envelope(&document, profile(), path()),
            Err(MetadataDecodeError::ResourceLimit("nodes"))
        ));
    }

    #[test]
    fn properties_generated_types_layout_is_typed() {
        let doc = XmlReader::from_slice(b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses'><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name><GeneratedTypes><GeneratedType category='ref'><TypeId>22222222-2222-4222-8222-222222222222</TypeId><ValueId>33333333-3333-4333-8333-333333333333</ValueId></GeneratedType></GeneratedTypes></Properties></X></MetaDataObject>").unwrap();
        let envelope = decode_metadata_envelope(&doc, profile(), path()).unwrap();
        assert_eq!(envelope.root().generated_types().len(), 1);
        assert!(
            envelope
                .root()
                .opaque_facets()
                .as_slice()
                .iter()
                .any(|facet| facet.placement().kind().as_str()
                    == "xml:properties-generated-type-projection")
        );
    }

    #[test]
    fn generated_layouts_use_distinct_opaque_coordinates() {
        let cases: &[(&[u8], &str, &str, &str, &str)] = &[
            (
                b"<MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name><GeneratedTypes><!--tail--><GeneratedType><TypeId>22222222-2222-4222-8222-222222222222</TypeId></GeneratedType><Future/></GeneratedTypes></Properties></X></MetaDataObject>",
                "xml:properties-generated-types-child",
                "properties.generated_types",
                "xml:properties-generated-type-projection",
                "properties.generated_types.generated_type",
            ),
            (
                b"<MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties><GeneratedTypes><!--tail--><GeneratedType><TypeId>22222222-2222-4222-8222-222222222222</TypeId></GeneratedType><Future/></GeneratedTypes></X></MetaDataObject>",
                "xml:generated-types-child",
                "generated_types",
                "xml:generated-type-projection",
                "generated_types.generated_type",
            ),
            (
                b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' xmlns:xr='http://v8.1c.ru/8.3/xcf/readable'><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties><InternalInfo><!--tail--><xr:GeneratedType><xr:TypeId>22222222-2222-4222-8222-222222222222</xr:TypeId></xr:GeneratedType><xr:Future/></InternalInfo></X></MetaDataObject>",
                "xml:internal-info-child",
                "internal_info",
                "xml:internal-info-generated-type-projection",
                "internal_info.generated_type",
            ),
        ];
        for (xml, tail_placement, tail_anchor, projection_placement, projection_anchor) in cases {
            let document = XmlReader::from_slice(xml).unwrap();
            let envelope = decode_metadata_envelope(&document, profile(), path()).unwrap();
            let facets = envelope.root().opaque_facets().as_slice();
            let tails: Vec<_> = facets
                .iter()
                .filter(|facet| facet.placement().kind().as_str() == *tail_placement)
                .collect();
            assert_eq!(
                tails
                    .iter()
                    .map(|facet| facet.placement().ordinal())
                    .collect::<Vec<_>>(),
                vec![0, 2]
            );
            assert!(tails.iter().all(|facet| {
                facet.anchor().property_path().segments()[0].as_name() == Some(*tail_anchor)
            }));
            let projection = facets
                .iter()
                .find(|facet| facet.placement().kind().as_str() == *projection_placement)
                .unwrap();
            assert_eq!(projection.placement().ordinal(), 1);
            assert_eq!(
                projection.anchor().property_path().segments()[0].as_name(),
                Some(*projection_anchor)
            );
        }
    }

    #[test]
    fn mixed_generated_sources_and_spoofed_namespace_fail() {
        let mixed = XmlReader::from_slice(b"<MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name><GeneratedTypes><GeneratedType><TypeId>22222222-2222-4222-8222-222222222222</TypeId></GeneratedType></GeneratedTypes></Properties><InternalInfo><GeneratedType><TypeId>33333333-3333-4333-8333-333333333333</TypeId></GeneratedType></InternalInfo></X></MetaDataObject>").unwrap();
        assert!(matches!(
            decode_metadata_envelope(&mixed, profile(), path()),
            Err(MetadataDecodeError::Duplicate("generated types source"))
        ));
        let spoofed = XmlReader::from_slice(b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses'><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name><GeneratedTypes><GeneratedType xmlns='urn:evil'><TypeId>22222222-2222-4222-8222-222222222222</TypeId></GeneratedType></GeneratedTypes></Properties></X></MetaDataObject>").unwrap();
        let spoofed = decode_metadata_envelope(&spoofed, profile(), path()).unwrap();
        assert!(spoofed.root().generated_types().is_empty());
        let service_only = XmlReader::from_slice(b"<MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name><GeneratedTypes><GeneratedType><TypeId>22222222-2222-4222-8222-222222222222</TypeId></GeneratedType></GeneratedTypes></Properties><InternalInfo><Service/></InternalInfo></X></MetaDataObject>").unwrap();
        assert_eq!(
            decode_metadata_envelope(&service_only, profile(), path())
                .unwrap()
                .root()
                .generated_types()
                .len(),
            1
        );
    }

    #[test]
    fn mixed_synonym_namespaces_fail_closed() {
        let doc = XmlReader::from_slice(b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' xmlns:v8='http://v8.1c.ru/8.1/data/core'><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name><Synonym><v8:item><v8:lang>ru</v8:lang><content xmlns=''>X</content></v8:item></Synonym></Properties></X></MetaDataObject>").unwrap();
        assert!(matches!(
            decode_metadata_envelope(&doc, profile(), path()),
            Err(MetadataDecodeError::InvalidEnvelope(
                "mixed Synonym field namespaces"
            ))
        ));
    }

    #[test]
    fn nested_family_guards_do_not_duplicate_source_subtrees() {
        let mut xml = String::from("<MetaDataObject>");
        for index in 0..18u32 {
            xml.push_str(&format!("<X uuid='00000000-0000-4000-8000-{index:012x}'><Properties><Name>X</Name></Properties><ChildObjects>"));
        }
        xml.push_str("<Leaf uuid='ffffffff-ffff-4fff-8fff-ffffffffffff'><Properties><Name>L</Name></Properties><Blob>");
        xml.push_str(&"x".repeat(2_000_000));
        xml.push_str("</Blob></Leaf>");
        for _ in 0..18 {
            xml.push_str("</ChildObjects></X>");
        }
        xml.push_str("</MetaDataObject>");
        let document = XmlReader::from_slice(xml.as_bytes()).unwrap();
        let envelope = decode_metadata_envelope(&document, profile(), path()).unwrap();
        assert_eq!(envelope.descendants().len(), 18);
        assert!(
            std::iter::once(envelope.root())
                .chain(envelope.descendants())
                .all(|object| object
                    .opaque_facets()
                    .as_slice()
                    .iter()
                    .find(|facet| facet.placement().kind().as_str() == "xml:family-fallback")
                    .is_some_and(|guard| guard.byte_len() == 0))
        );
        assert_eq!(
            MetadataRegistry::default()
                .encode(&envelope, &profile())
                .unwrap(),
            xml.as_bytes()
        );
    }

    #[test]
    fn genuine_opaque_slots_share_the_aggregate_facet_budget() {
        let mut xml = String::from(
            "<MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties>",
        );
        for _ in 0..=MAX_METADATA_FACETS {
            xml.push_str("<Future/>");
        }
        xml.push_str("</X></MetaDataObject>");
        let document = XmlReader::from_slice(xml.as_bytes()).unwrap();
        assert!(matches!(
            decode_metadata_envelope(&document, profile(), path()),
            Err(MetadataDecodeError::ResourceLimit("opaque facets"))
        ));
    }

    #[test]
    fn reserved_namespace_bindings_are_strict() {
        for xml in [
            b"<MetaDataObject xmlns:p=''><X/></MetaDataObject>".as_slice(),
            b"<MetaDataObject xmlns='http://www.w3.org/XML/1998/namespace'><X/></MetaDataObject>"
                .as_slice(),
            b"<MetaDataObject xmlns:p='http://www.w3.org/XML/1998/namespace'><X/></MetaDataObject>"
                .as_slice(),
            b"<MetaDataObject xmlns:p='http://www.w3.org/2000/xmlns/'><X/></MetaDataObject>"
                .as_slice(),
        ] {
            if let Ok(document) = XmlReader::from_slice(xml) {
                assert!(decode_metadata_envelope(&document, profile(), path()).is_err());
            }
        }
    }

    #[test]
    fn bundled_dialect_profile_is_checked_exactly() {
        let document = XmlReader::from_slice(b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' version='2.20'><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties></X></MetaDataObject>").unwrap();
        let registry = crate::bundled_dialect_registry().unwrap();
        assert!(
            decode_metadata_envelope_with_dialect(
                &document,
                &registry,
                ProfileId::parse("xml-2.20").unwrap(),
                path()
            )
            .is_ok()
        );
        assert!(
            decode_metadata_envelope_with_dialect(
                &document,
                &registry,
                ProfileId::parse("xml-2.21").unwrap(),
                path()
            )
            .is_err()
        );
    }

    #[test]
    fn checked_envelope_constructor_rejects_guardless_and_mismatched_model() {
        let document = XmlReader::from_slice(b"<MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties></X></MetaDataObject>").unwrap();
        let decoded = decode_metadata_envelope(&document, profile(), path()).unwrap();
        let mut parts = CanonicalObjectParts::new(
            decoded.root().identity().clone(),
            MetadataKind::new("X").unwrap(),
            decoded.root().provenance().clone(),
        );
        parts.properties = decoded.root().properties().to_vec();
        assert!(matches!(
            MetadataEnvelope::from_parts(
                CanonicalObject::new(parts).unwrap(),
                Vec::new(),
                document.clone()
            ),
            Err(MetadataDecodeError::InvalidEnvelope(_))
        ));
        let mut mismatch = CanonicalObjectParts::new(
            decoded.root().identity().clone(),
            MetadataKind::new("Y").unwrap(),
            decoded.root().provenance().clone(),
        );
        mismatch.properties = decoded.root().properties().to_vec();
        mismatch.opaque_facets = decoded.root().opaque_facets().clone();
        assert!(matches!(
            MetadataEnvelope::from_parts(
                CanonicalObject::new(mismatch).unwrap(),
                Vec::new(),
                document
            ),
            Err(MetadataDecodeError::InvalidEnvelope(
                "source document family differs from canonical root"
            ))
        ));
    }

    #[test]
    fn checked_envelope_constructor_rejects_guard_for_another_path() {
        let document = XmlReader::from_slice(b"<MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties></X></MetaDataObject>").unwrap();
        let decoded = decode_metadata_envelope(&document, profile(), path()).unwrap();
        let root = decoded.root();
        let wrong_path = ObjectPath::new(vec![PathSegment::name("wrong").unwrap()]).unwrap();
        assert!(matches!(
            MetadataEnvelope::from_parts(
                replace_family_guard(root, &wrong_path, Vec::new()),
                Vec::new(),
                document.clone()
            ),
            Err(MetadataDecodeError::InvalidEnvelope(
                "family fallback guard path differs from canonical object"
            ))
        ));
        assert!(matches!(
            MetadataEnvelope::from_parts(
                replace_family_guard(root, root.identity().path(), vec![1]),
                Vec::new(),
                document
            ),
            Err(MetadataDecodeError::InvalidEnvelope(
                "family fallback guard payload must be empty"
            ))
        ));
    }

    #[test]
    fn compact_parsed_lexemes_are_bounded_before_writer_allocation() {
        let xml = format!(
            "<Future>{}</Future>",
            ">".repeat(MAX_METADATA_BYTES / 4 + 1)
        );
        let document = XmlReader::from_slice(xml.as_bytes()).unwrap();
        assert!(document_lexical_len(&document).unwrap() < MAX_METADATA_BYTES);
        assert!(document_normalized_len(&document).unwrap() > MAX_METADATA_BYTES);
        assert!(matches!(
            check_document(&document),
            Err(MetadataDecodeError::ResourceLimit("normalized bytes"))
        ));
        assert!(crate::XmlWriter::to_vec(&document, LexicalPolicy::Preserve).is_err());

        let mut facets = FacetSet::root();
        assert!(matches!(
            retain_as(
                &document.root().children()[0],
                0,
                &profile(),
                &path(),
                "future",
                "xml:future",
                &mut facets
            ),
            Err(MetadataDecodeError::ResourceLimit("normalized bytes"))
        ));
        assert_eq!(facets.budget.borrow().count, 0);
    }

    #[test]
    fn metadata_preflight_reuses_writer_validation_before_retention() {
        let parsed = XmlReader::from_slice(b"<!DOCTYPE MetaDataObject><MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties></X></MetaDataObject>").unwrap();
        let mut moved_children = vec![parsed.before_root()[0].clone()];
        moved_children.extend_from_slice(parsed.root().children());
        let moved_doctype = XmlDocument::new(parsed.root().with_children(moved_children));
        assert!(crate::writer::validate_document(&moved_doctype).is_err());
        assert!(matches!(
            check_document(&moved_doctype),
            Err(MetadataDecodeError::Xml(message))
                if message.contains("document type is only valid in the prolog")
        ));
        assert!(matches!(
            decode_metadata_envelope(&moved_doctype, profile(), path()),
            Err(MetadataDecodeError::Xml(_))
        ));

        let duplicate_attributes = metadata_ast(vec![
            Attribute::ordinary(QName::new("version").unwrap(), "2.20"),
            Attribute::ordinary(QName::new("version").unwrap(), "2.21"),
        ]);
        let base = metadata_ast(Vec::new());
        let invalid_comment = XmlDocument::new(base.root().with_children(vec![
            XmlNode::comment("bad--comment"),
            base.root().children()[0].clone(),
        ]));
        let invalid_cdata = XmlDocument::new(base.root().with_children(vec![
            XmlNode::cdata("bad]]>cdata"),
            base.root().children()[0].clone(),
        ]));
        for document in [duplicate_attributes, invalid_comment, invalid_cdata] {
            assert!(crate::writer::validate_document(&document).is_err());
            assert!(matches!(
                check_document(&document),
                Err(MetadataDecodeError::Xml(_))
            ));
            assert!(matches!(
                decode_metadata_envelope(&document, profile(), path()),
                Err(MetadataDecodeError::Xml(_))
            ));
        }
    }

    #[test]
    fn self_closing_with_children_uses_normalized_size_before_emission() {
        let parsed = XmlReader::from_slice(b"<Future a='&amp;'/>").unwrap();
        let changed = parsed
            .root()
            .with_children(vec![XmlNode::text("&".repeat(6_800_000))]);
        let node = XmlNode::Element(changed);
        let predicted = node_lexical_len(&node).unwrap();
        assert!(predicted > MAX_METADATA_BYTES);
        let facets = FacetSet::root();
        assert!(matches!(
            facets.reserve(predicted),
            Err(MetadataDecodeError::ResourceLimit("opaque bytes"))
        ));
    }
}
