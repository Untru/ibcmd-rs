use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::rc::Rc;
use std::sync::Arc;

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::asset::{AssetReference, MAX_ASSET_BYTES, MediaKind};
use ibcmd_core::diagnostic::{ObjectPath, PathSegment, PropertyPath};
use ibcmd_core::family::FamilyId;
use ibcmd_core::identity::{LogicalIdentity, ObjectUuid};
use ibcmd_core::model::{
    CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, GeneratedType,
    GeneratedTypeKind, MetadataKind,
};
use ibcmd_core::opaque::{OpaqueFacet, OpaqueFacets, OpaquePlacement};
use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
use ibcmd_core::source_policy::SourceOperationPolicy;
use ibcmd_core::storage::Sha256Digest;
use ibcmd_core::value::{CanonicalField, CanonicalText, CanonicalValue, CanonicalValueKind};

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
// Bounded source-file envelopes allow more sibling references than family codecs.
// Explicit source operations use checked accounting and iterative traversal.
#[derive(Clone, Copy, Debug)]
struct MetadataShapePolicy {
    nodes: usize,
    facets: usize,
    core: SourceOperationPolicy,
}
impl MetadataShapePolicy {
    const fn limit(self, bounded: usize) -> usize {
        match self.core {
            SourceOperationPolicy::Bounded => bounded,
            SourceOperationPolicy::Source => usize::MAX,
        }
    }
    const fn source_with(core: SourceOperationPolicy) -> Self {
        match core {
            SourceOperationPolicy::Bounded => SOURCE_METADATA_POLICY,
            SourceOperationPolicy::Source => Self {
                nodes: usize::MAX,
                facets: usize::MAX,
                core,
            },
        }
    }
}
const DEFAULT_METADATA_POLICY: MetadataShapePolicy = MetadataShapePolicy {
    nodes: MAX_METADATA_NODES,
    facets: MAX_METADATA_FACETS,
    core: SourceOperationPolicy::Bounded,
};
const SOURCE_METADATA_POLICY: MetadataShapePolicy = MetadataShapePolicy {
    nodes: 1_048_576,
    facets: 65_536,
    core: SourceOperationPolicy::Bounded,
};
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
    policy: MetadataShapePolicy,
    content: SourceFacetContent,
}
type SourceFacetContent = Rc<RefCell<BTreeMap<Sha256Digest, Arc<Vec<u8>>>>>;
type VerifiedFacetContent = Arc<BTreeMap<Sha256Digest, Arc<Vec<u8>>>>;
impl FacetSet {
    fn root(policy: MetadataShapePolicy) -> Self {
        Self {
            values: Vec::new(),
            budget: Rc::new(RefCell::new(FacetBudget::default())),
            policy,
            content: Rc::default(),
        }
    }
    fn child_with(parent: &Self, values: Vec<OpaqueFacet>) -> Self {
        Self {
            values,
            budget: Rc::clone(&parent.budget),
            policy: parent.policy,
            content: Rc::clone(&parent.content),
        }
    }
    fn reserve(&self, bytes: usize) -> Result<(), MetadataDecodeError> {
        let mut budget = self.budget.borrow_mut();
        let count = budget
            .count
            .checked_add(1)
            .ok_or(MetadataDecodeError::ResourceLimit("opaque facets"))?;
        let retained = budget
            .bytes
            .checked_add(bytes)
            .ok_or(MetadataDecodeError::ResourceLimit("opaque bytes"))?;
        if count > self.policy.facets {
            return Err(MetadataDecodeError::ResourceLimit("opaque facets"));
        }
        if retained > self.policy.limit(MAX_METADATA_BYTES) {
            return Err(MetadataDecodeError::ResourceLimit("opaque bytes"));
        }
        budget.count = count;
        budget.bytes = retained;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct MetadataEnvelope {
    root: CanonicalObject,
    descendants: Vec<CanonicalObject>,
    fallback: Fallback,
    source_model_unchanged: bool,
    shape_policy: MetadataShapePolicy,
    content: VerifiedFacetContent,
    external_binding: Option<super::package::ExternalSourceBinding>,
}
impl MetadataEnvelope {
    pub fn from_parts(
        root: CanonicalObject,
        descendants: Vec<CanonicalObject>,
        source_document: XmlDocument,
    ) -> Result<Self, MetadataDecodeError> {
        Self::from_parts_with_state(
            root,
            descendants,
            source_document,
            false,
            DEFAULT_METADATA_POLICY,
            Arc::default(),
        )
    }
    fn from_parts_with_state(
        root: CanonicalObject,
        descendants: Vec<CanonicalObject>,
        source_document: XmlDocument,
        source_model_unchanged: bool,
        shape_policy: MetadataShapePolicy,
        content: VerifiedFacetContent,
    ) -> Result<Self, MetadataDecodeError> {
        Self::from_parts_with_external_state(
            root,
            descendants,
            source_document,
            source_model_unchanged,
            shape_policy,
            content,
            None,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn from_parts_with_external_state(
        root: CanonicalObject,
        descendants: Vec<CanonicalObject>,
        source_document: XmlDocument,
        source_model_unchanged: bool,
        shape_policy: MetadataShapePolicy,
        content: VerifiedFacetContent,
        external_binding: Option<super::package::ExternalSourceBinding>,
    ) -> Result<Self, MetadataDecodeError> {
        let actual = inspect_metadata_family_with_policy(&source_document, shape_policy)?;
        let family_matches = if let Some(binding) = external_binding {
            binding.validate(&source_document)?;
            actual.as_str() == binding.kind().external_kind()
                && root.kind().as_str() == binding.kind().internal_kind()
                && root.identity().uuid()
                    == binding
                        .identity()
                        .contained
                        .expect("validated contained identity")
                        .object_id
                && root.owner().is_none()
        } else {
            actual.as_str() == root.kind().as_str()
        };
        if !family_matches {
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
        if facet_count > shape_policy.facets {
            return Err(MetadataDecodeError::ResourceLimit("opaque facets"));
        }
        if facet_bytes > shape_policy.limit(MAX_METADATA_BYTES) {
            return Err(MetadataDecodeError::ResourceLimit("opaque bytes"));
        }
        let envelope = Self {
            root,
            descendants,
            fallback: Fallback::new(source_document),
            source_model_unchanged,
            shape_policy,
            content,
            external_binding,
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
        Self::from_parts_with_external_state(
            root,
            descendants,
            self.fallback.into_document(),
            false,
            self.shape_policy,
            self.content,
            self.external_binding,
        )
    }
    pub const fn external_source_binding(&self) -> Option<super::package::ExternalSourceBinding> {
        self.external_binding
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
        CanonicalConfiguration::new_with_policy(objects, self.shape_policy.core)
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
                if let Some(bytes) = facet
                    .asset_reference()
                    .and_then(|reference| self.content.get(&reference.sha256()))
                {
                    facet
                        .resolve_emit_permit(target, bytes)
                        .map_err(super::registry::MetadataEncodeError::Opaque)?;
                } else {
                    facet
                        .emit_permit(target)
                        .map_err(super::registry::MetadataEncodeError::Opaque)?;
                }
            }
        }
        if self.shape_policy.core == SourceOperationPolicy::Source {
            crate::XmlWriter::to_vec_with_policy(
                self.fallback.document(),
                LexicalPolicy::Preserve,
                self.shape_policy.core,
            )
            .map_err(super::fallback::FallbackEmitError::from)
            .map_err(Into::into)
        } else {
            self.fallback.emit().map_err(Into::into)
        }
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
    /// A complex property carries content that the compile direction has no
    /// evidenced model for. `owner` names the enclosing shape (family, child
    /// object, or standard attribute) and `property` the exact XML property,
    /// so the coordinate is identified without reading the decoder source.
    UnevidencedProperty {
        owner: String,
        property: &'static str,
    },
    /// A property that every evidenced platform tree emits empty carries
    /// stray text.  This is a lexical defect of the input, not a missing
    /// compile model, and is reported apart from `UnevidencedProperty`.
    UnexpectedContent {
        owner: String,
        property: &'static str,
    },
    /// A standard-attribute sub-property deviates from the platform default
    /// profile, so the block carries object data the compile direction cannot
    /// represent.  `expected` is the evidenced default, `actual` what was read.
    UnevidencedStandardAttribute {
        owner: String,
        property: &'static str,
        expected: &'static str,
        actual: String,
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
    inspect_metadata_family_with_policy(document, DEFAULT_METADATA_POLICY)
}

fn inspect_metadata_family_with_policy(
    document: &XmlDocument,
    shape_policy: MetadataShapePolicy,
) -> Result<FamilyId, MetadataDecodeError> {
    check_document_with_policy(document, shape_policy)?;
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

/// Decode a source-file envelope, preserving known named sibling descriptors
/// as ordered reference facets. This is independent of physical CF codec
/// availability; unknown bare children still require UUIDs and fail closed.
///
/// Complete source configurations can exceed the generic/family shape (for
/// example, the independently inventoried UH root has 25,977 bare references).
/// This entry point explicitly permits at most 1,048,576 nodes and 65,536
/// non-guard opaque facets; generic/family entry points keep 16,384 and 4,096.
/// The shared 32 MiB document/opaque byte limits, depth, attributes and namespace
/// bounds do not change. Exceeding a bound returns an error without truncation.
/// Revalidation through [`MetadataEnvelope::with_model`] retains this policy.
pub fn decode_source_metadata_envelope(
    document: &XmlDocument,
    source_profile: ProfileId,
    object_path: ObjectPath,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    decode_source_metadata_envelope_with_policy(
        document,
        source_profile,
        object_path,
        SourceOperationPolicy::Bounded,
    )
}
/// Decode an explicitly owned source operation. Bounded defaults retain their
/// existing limits. Source accounting removes fixed shape/byte ceilings and uses
/// iterative typed-object traversal. Large opaque facets use verified references;
/// the core inline-asset contract remains unchanged.
/// Identity, namespace, ownership, ordering and provenance validation are common.
pub fn decode_source_metadata_envelope_with_policy(
    document: &XmlDocument,
    source_profile: ProfileId,
    object_path: ObjectPath,
    operation: SourceOperationPolicy,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    let shape_policy = MetadataShapePolicy::source_with(operation);
    let family = inspect_metadata_family_with_policy(document, shape_policy)?;
    if family.as_str() == "Configuration" {
        return decode_configuration_envelope_with_policy(
            document,
            source_profile,
            object_path,
            shape_policy,
        );
    }
    let references: &[&str] = match family.as_str() {
        "Subsystem" => &["Subsystem"],
        "ExternalDataSource" => &["Table"],
        "FilterCriterion" => &["Form"],
        "CalculationRegister" => &["Form", "Template", "Recalculation"],
        "Catalog"
        | "Document"
        | "Enum"
        | "Report"
        | "DataProcessor"
        | "SettingsStorage"
        | "DocumentJournal"
        | "ExchangePlan"
        | "BusinessProcess"
        | "Task"
        | "InformationRegister"
        | "AccumulationRegister"
        | "AccountingRegister"
        | "ChartOfAccounts"
        | "ChartOfCalculationTypes"
        | "ChartOfCharacteristicTypes" => &["Form", "Template"],
        _ => &[],
    };
    decode_metadata_envelope_with_policy(
        document,
        source_profile,
        object_path,
        references,
        shape_policy,
    )
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
    decode_configuration_envelope_with_policy(
        document,
        source_profile,
        object_path,
        DEFAULT_METADATA_POLICY,
    )
}

fn decode_configuration_envelope_with_policy(
    document: &XmlDocument,
    source_profile: ProfileId,
    object_path: ObjectPath,
    shape_policy: MetadataShapePolicy,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    if inspect_metadata_family_with_policy(document, shape_policy)?.as_str() != "Configuration" {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "metadata object is not Configuration",
        ));
    }
    decode_metadata_envelope_with_policy(
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
            "PaletteColor",
        ],
        shape_policy,
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
    decode_metadata_envelope_with_policy(
        document,
        source_profile,
        object_path,
        child_reference_kinds,
        DEFAULT_METADATA_POLICY,
    )
}

fn decode_metadata_envelope_with_policy(
    document: &XmlDocument,
    source_profile: ProfileId,
    object_path: ObjectPath,
    child_reference_kinds: &[&str],
    shape_policy: MetadataShapePolicy,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    decode_metadata_envelope_with_binding(
        document,
        source_profile,
        object_path,
        child_reference_kinds,
        shape_policy,
        None,
    )
}
pub(super) fn decode_external_metadata_envelope(
    document: &XmlDocument,
    source_profile: ProfileId,
    object_path: ObjectPath,
    claims: &super::external_objects::ExternalRootClaims<'_>,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    let binding = claims.binding_for(document)?;
    decode_metadata_envelope_with_binding(
        document,
        source_profile,
        object_path,
        &["Form", "Template"],
        DEFAULT_METADATA_POLICY,
        Some(binding),
    )
}
fn decode_metadata_envelope_with_binding(
    document: &XmlDocument,
    source_profile: ProfileId,
    object_path: ObjectPath,
    child_reference_kinds: &[&str],
    shape_policy: MetadataShapePolicy,
    binding: Option<super::package::ExternalSourceBinding>,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    check_document_with_policy(document, shape_policy)?;
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
    let mut facet_set = FacetSet::root(shape_policy);
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
    let root = decode_object_with_binding(
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
        binding,
    )?;
    MetadataEnvelope::from_parts_with_external_state(
        root,
        descendants,
        document.clone(),
        true,
        shape_policy,
        Arc::new(std::mem::take(&mut *facet_set.content.borrow_mut())),
        binding,
    )
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

struct ObjectDecoder<'a> {
    element: &'a XmlElement,
    uuid: ObjectUuid,
    profile: ProfileId,
    path: ObjectPath,
    facets: FacetSet,
    parts: CanonicalObjectParts,
    names: BTreeSet<String>,
    containers: BTreeSet<&'static str>,
    generated_source: Option<&'static str>,
    generated_names: BTreeMap<ObjectUuid, Option<String>>,
    external_claims: bool,
}
impl<'a> ObjectDecoder<'a> {
    fn new(
        element: &'a XmlElement,
        profile: ProfileId,
        path: ObjectPath,
        owner: Option<ObjectUuid>,
        parent_facets: &FacetSet,
        initial_facets: Vec<OpaqueFacet>,
    ) -> Result<Self, MetadataDecodeError> {
        let e = element;
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
        Ok(Self {
            element,
            uuid,
            profile,
            path,
            facets: local_facets,
            parts,
            names: BTreeSet::new(),
            containers: BTreeSet::new(),
            generated_source: None,
            generated_names: BTreeMap::new(),
            external_claims: false,
        })
    }
    fn finish(mut self) -> Result<CanonicalObject, MetadataDecodeError> {
        if !self.names.contains("Name") {
            return Err(MetadataDecodeError::Missing("Name"));
        }
        // Genuine native EnumList declarations can share the Enum's own TypeId.
        // Claim only the complete source name/role/value relationship; all other
        // identity collisions remain ordinary graph-validation failures.
        if self.parts.kind.as_str() == "Enum" {
            let name = self.parts.properties.iter().find_map(|field| {
                if field.name().as_str() == "Name"
                    && let CanonicalValueKind::Text(value) = field.value().kind()
                {
                    Some(value.as_str())
                } else {
                    None
                }
            });
            if let Some(name) = name {
                let expected_name = format!("EnumList.{name}");
                for generated in &mut self.parts.generated_types {
                    if generated.kind().as_str() == "List"
                        && generated.uuid() == self.uuid
                        && generated.value_id().is_some_and(|value| {
                            value != self.uuid && value.as_bytes().iter().any(|byte| *byte != 0)
                        })
                        && self
                            .generated_names
                            .get(&generated.uuid())
                            .and_then(Option::as_deref)
                            == Some(expected_name.as_str())
                    {
                        *generated = generated.clone().with_owner_identity_alias();
                    }
                }
            }
        }
        push_family_guard(&self.profile, &self.path, &mut self.facets)?;
        self.parts.opaque_facets =
            OpaqueFacets::new_with_policy(self.facets.values, self.facets.policy.core)
                .map_err(|x| MetadataDecodeError::Core(x.to_string()))?;
        CanonicalObject::new_with_policy(self.parts, self.facets.policy.core)
            .map_err(|x| MetadataDecodeError::Core(x.to_string()))
    }
}

fn decode_object_node<'a>(
    state: &mut ObjectDecoder<'a>,
    ordinal: usize,
    node: &'a XmlNode,
    uris: &ResolvedNamespaces,
    expected: Option<&str>,
) -> Result<Option<&'a XmlElement>, MetadataDecodeError> {
    let profile = &state.profile;
    let path = &state.path;
    let local_facets = &mut state.facets;
    let parts = &mut state.parts;
    let names = &mut state.names;
    let containers = &mut state.containers;
    let generated_source = &mut state.generated_source;
    let XmlNode::Element(child) = node else {
        retain(node, ordinal, profile, path, local_facets)?;
        return Ok(None);
    };
    match child.name().local() {
        "Properties" if typed(child, "Properties", expected, uris) => {
            if !containers.insert("Properties") {
                return Err(MetadataDecodeError::Duplicate("Properties"));
            }
            retain_unknown_start_tag(
                child,
                &[],
                profile,
                path,
                "properties.attributes",
                "xml:properties-start-tag-projection",
                local_facets,
            )?;
            let has_generated = decode_properties(
                child,
                &mut parts.properties,
                &mut parts.generated_types,
                &mut state.generated_names,
                names,
                profile,
                path,
                local_facets,
                uris,
                expected,
                state.external_claims,
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
                &mut state.generated_names,
                profile,
                path,
                local_facets,
                uris,
                expected,
                DIRECT_GENERATED_LAYOUT,
                state.external_claims,
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
                &mut state.generated_names,
                profile,
                path,
                local_facets,
                uris,
                if expected.is_none() {
                    None
                } else {
                    Some(XR_NAMESPACE)
                },
                INTERNAL_INFO_GENERATED_LAYOUT,
                state.external_claims,
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
                profile,
                path,
                "child_objects.attributes",
                "xml:child-objects-start-tag-projection",
                local_facets,
            )?;
            return Ok(Some(child));
        }
        _ => retain(node, ordinal, profile, path, local_facets)?,
    }
    Ok(None)
}

#[allow(clippy::too_many_arguments)]
fn next_typed_child<'a>(
    node: &'a XmlNode,
    ordinal: usize,
    typed_index: &mut u32,
    profile: &ProfileId,
    parent_path: &ObjectPath,
    facets: &mut FacetSet,
    uris: &ResolvedNamespaces,
    expected: Option<&str>,
    child_reference_kinds: &[&str],
) -> Result<Option<(&'a XmlElement, ObjectPath)>, MetadataDecodeError> {
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
        return Ok(None);
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
        return Ok(None);
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
        return Ok(None);
    }
    uuid_attr(child)?;
    let mut path = parent_path.clone();
    path.push_with_policy(
        PathSegment::name_with_policy("children", facets.policy.core)
            .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
        facets.policy.core,
    )
    .map_err(|x| MetadataDecodeError::Core(x.to_string()))?;
    path.push_with_policy(PathSegment::index(*typed_index), facets.policy.core)
        .map_err(|x| MetadataDecodeError::Core(x.to_string()))?;
    *typed_index = typed_index
        .checked_add(1)
        .ok_or(MetadataDecodeError::ResourceLimit("child ordinal"))?;
    Ok(Some((child, path)))
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
    decode_object_with_binding(
        e,
        profile,
        path,
        owner,
        descendants,
        parent_facets,
        initial_facets,
        uris,
        expected,
        child_reference_kinds,
        None,
    )
}
#[allow(clippy::too_many_arguments)]
fn decode_object_with_binding(
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
    binding: Option<super::package::ExternalSourceBinding>,
) -> Result<CanonicalObject, MetadataDecodeError> {
    let mut state = ObjectDecoder::new(e, profile, path, owner, parent_facets, initial_facets)?;
    if let Some(binding) = binding {
        state.uuid = binding
            .identity()
            .contained
            .expect("validated contained identity")
            .object_id;
        state.parts.identity = LogicalIdentity::new(state.uuid, state.path.clone());
        state.parts.kind = MetadataKind::new(binding.kind().internal_kind())
            .map_err(|x| MetadataDecodeError::Core(x.to_string()))?;
        state.external_claims = true;
    }
    if state.facets.policy.core == SourceOperationPolicy::Source {
        return decode_source_objects_iteratively(
            state,
            descendants,
            uris,
            expected,
            child_reference_kinds,
        );
    }
    // The legacy document preflight retains its small depth limit. Share the
    // node grammar and child inspection with the iterative source decoder.
    for (ordinal, node) in e.children().iter().enumerate() {
        if let Some(container) = decode_object_node(&mut state, ordinal, node, uris, expected)? {
            let mut typed_index = 0;
            for (ordinal, node) in container.children().iter().enumerate() {
                if let Some((child, path)) = next_typed_child(
                    node,
                    ordinal,
                    &mut typed_index,
                    &state.profile,
                    &state.path,
                    &mut state.facets,
                    uris,
                    expected,
                    child_reference_kinds,
                )? {
                    let mut nested = Vec::new();
                    let object = decode_object(
                        child,
                        state.profile.clone(),
                        path,
                        Some(state.uuid),
                        &mut nested,
                        &mut state.facets,
                        Vec::new(),
                        uris,
                        expected,
                        &[],
                    )?;
                    descendants.push(object);
                    descendants.extend(nested);
                }
            }
        }
    }
    state.finish()
}

struct ChildCursor<'a> {
    container: &'a XmlElement,
    next: usize,
    typed_index: u32,
}
struct ObjectContinuation<'a> {
    state: ObjectDecoder<'a>,
    next: usize,
    children: Option<ChildCursor<'a>>,
    slot: usize,
}
fn decode_source_objects_iteratively<'a>(
    root: ObjectDecoder<'a>,
    descendants: &mut Vec<CanonicalObject>,
    uris: &ResolvedNamespaces,
    expected: Option<&str>,
    child_reference_kinds: &[&str],
) -> Result<CanonicalObject, MetadataDecodeError> {
    enum Action<'a> {
        Continue,
        Descend(Box<ObjectDecoder<'a>>),
        Complete,
    }
    let mut pending = vec![ObjectContinuation {
        state: root,
        next: 0,
        children: None,
        slot: 0,
    }];
    // Slots are reserved on entry (DFS preorder), but populated on exit. The
    // parent remains suspended while each child is fully validated, preserving
    // the original decoder's first-error priority and owner/facet sequence.
    let mut completed = vec![None];
    while !pending.is_empty() {
        let action = {
            let frame = pending.last_mut().expect("nonempty continuation stack");
            if let Some(cursor) = frame.children.as_mut() {
                if let Some(node) = cursor.container.children().get(cursor.next) {
                    let ordinal = cursor.next;
                    cursor.next += 1;
                    let references = if frame.slot == 0 {
                        child_reference_kinds
                    } else {
                        &[]
                    };
                    match next_typed_child(
                        node,
                        ordinal,
                        &mut cursor.typed_index,
                        &frame.state.profile,
                        &frame.state.path,
                        &mut frame.state.facets,
                        uris,
                        expected,
                        references,
                    )? {
                        Some((child, path)) => Action::Descend(Box::new(ObjectDecoder::new(
                            child,
                            frame.state.profile.clone(),
                            path,
                            Some(frame.state.uuid),
                            &frame.state.facets,
                            Vec::new(),
                        )?)),
                        None => Action::Continue,
                    }
                } else {
                    frame.children = None;
                    Action::Continue
                }
            } else if let Some(node) = frame.state.element.children().get(frame.next) {
                let ordinal = frame.next;
                frame.next += 1;
                if let Some(container) =
                    decode_object_node(&mut frame.state, ordinal, node, uris, expected)?
                {
                    frame.children = Some(ChildCursor {
                        container,
                        next: 0,
                        typed_index: 0,
                    });
                }
                Action::Continue
            } else {
                Action::Complete
            }
        };
        match action {
            Action::Continue => {}
            Action::Descend(state) => {
                let slot = completed.len();
                completed.push(None);
                pending.push(ObjectContinuation {
                    state: *state,
                    next: 0,
                    children: None,
                    slot,
                });
            }
            Action::Complete => {
                let frame = pending.pop().expect("completion has a frame");
                completed[frame.slot] = Some(frame.state.finish()?);
            }
        }
    }
    let mut objects = completed
        .into_iter()
        .map(|object| object.expect("each frame completed"));
    let root = objects.next().expect("root frame completed");
    descendants.extend(objects);
    Ok(root)
}

#[allow(clippy::too_many_arguments)]
fn decode_properties(
    e: &XmlElement,
    out: &mut Vec<CanonicalField>,
    generated: &mut Vec<GeneratedType>,
    generated_names: &mut BTreeMap<ObjectUuid, Option<String>>,
    names: &mut BTreeSet<String>,
    profile: &ProfileId,
    path: &ObjectPath,
    facets: &mut FacetSet,
    uris: &ResolvedNamespaces,
    expected: Option<&str>,
    external_claims: bool,
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
                generated_names,
                profile,
                path,
                facets,
                uris,
                expected,
                PROPERTIES_GENERATED_LAYOUT,
                external_claims,
            )?;
            continue;
        }
        // External codec has already projected/validated every other root
        // property. Its CURRENT typed value must not become opaque old XML.
        if external_claims && local != "Name" && local != "Synonym" {
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
            let value = synonym_value(child, uris, facets.policy.core)?;
            if !external_claims {
                retain_as(
                    node,
                    ordinal,
                    profile,
                    path,
                    "properties.synonym",
                    "xml:synonym-projection",
                    facets,
                )?;
            }
            value
        } else {
            CanonicalValue::text(
                CanonicalText::new_with_policy(
                    &element_text_with_policy(child, facets.policy.core)?
                        .ok_or(MetadataDecodeError::Missing("common property text"))?,
                    facets.policy.core,
                )
                .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
            )
        };
        out.push(
            CanonicalField::named_with_policy(local, value, facets.policy.core)
                .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
        );
    }
    if !names.contains("Name") {
        return Err(MetadataDecodeError::Missing("Name"));
    }
    Ok(has_generated)
}

pub(super) fn synonym_value(
    e: &XmlElement,
    uris: &ResolvedNamespaces,
    policy: SourceOperationPolicy,
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
                    lang = element_text_with_policy(field, policy)?
                }
                "content" => {
                    if seen_content {
                        return Err(MetadataDecodeError::Duplicate("Synonym content"));
                    }
                    seen_content = true;
                    content = element_text_with_policy(field, policy)?
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
            CanonicalValue::record_with_policy(
                vec![
                    CanonicalField::named_with_policy(
                        "lang",
                        CanonicalValue::text(
                            CanonicalText::new_with_policy(&lang, policy)
                                .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
                        ),
                        policy,
                    )
                    .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
                    CanonicalField::named_with_policy(
                        "content",
                        CanonicalValue::text(
                            CanonicalText::new_with_policy(&content, policy)
                                .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
                        ),
                        policy,
                    )
                    .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
                ],
                policy,
            )
            .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
        );
    }
    CanonicalValue::sequence_with_policy(values, policy)
        .map_err(|x| MetadataDecodeError::Core(x.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn decode_generated_types(
    e: &XmlElement,
    out: &mut Vec<GeneratedType>,
    generated_names: &mut BTreeMap<ObjectUuid, Option<String>>,
    profile: &ProfileId,
    path: &ObjectPath,
    facets: &mut FacetSet,
    uris: &ResolvedNamespaces,
    expected_type_namespace: Option<&str>,
    layout: GeneratedLayout,
    external_claims: bool,
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
        if external_claims && typed(child, "ContainedObject", expected_type_namespace, uris) {
            continue;
        }
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
        let mut source_name = None;
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
            if let AttributeKind::Ordinary(name) = attr.kind()
                && name.local() == "name"
                && name.prefix().is_none()
            {
                if source_name.is_some() {
                    return Err(MetadataDecodeError::Duplicate("GeneratedType name"));
                }
                source_name = Some(attr.value().to_owned());
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
        generated_names.insert(uuid, source_name);
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
        if !external_claims {
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
    }
    Ok(any)
}

pub(super) fn uuid_attr(e: &XmlElement) -> Result<ObjectUuid, MetadataDecodeError> {
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
    element_text_with_policy(e, SourceOperationPolicy::Bounded)
}
fn element_text_with_policy(
    e: &XmlElement,
    policy: SourceOperationPolicy,
) -> Result<Option<String>, MetadataDecodeError> {
    let mut value = String::new();
    let mut length = 0usize;
    for node in e.children() {
        match node {
            XmlNode::Text(x) => {
                length = length
                    .checked_add(x.value().len())
                    .ok_or(MetadataDecodeError::ResourceLimit("canonical text"))?;
                if length
                    > MetadataShapePolicy::source_with(policy)
                        .limit(ibcmd_core::value::MAX_CANONICAL_TEXT_BYTES)
                {
                    return Err(MetadataDecodeError::ResourceLimit("canonical text"));
                }
                value.push_str(x.value());
            }
            XmlNode::CData(x) => {
                length = length
                    .checked_add(x.value().len())
                    .ok_or(MetadataDecodeError::ResourceLimit("canonical text"))?;
                if length
                    > MetadataShapePolicy::source_with(policy)
                        .limit(ibcmd_core::value::MAX_CANONICAL_TEXT_BYTES)
                {
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
    if normalized_bytes > facets.policy.limit(MAX_METADATA_BYTES) {
        return Err(MetadataDecodeError::ResourceLimit("normalized bytes"));
    }
    facets.reserve(preserve_bytes)?;
    let bytes = match facets.policy.core {
        SourceOperationPolicy::Bounded => {
            crate::writer::element_start_to_vec(element, LexicalPolicy::Preserve)
        }
        SourceOperationPolicy::Source => crate::writer::element_start_to_vec_with_policy(
            element,
            LexicalPolicy::Preserve,
            facets.policy.core,
        ),
    }
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
    if node_normalized_len(node)? > facets.policy.limit(MAX_METADATA_BYTES) {
        return Err(MetadataDecodeError::ResourceLimit("normalized bytes"));
    }
    facets.reserve(preserve_bytes)?;
    let bytes = match facets.policy.core {
        SourceOperationPolicy::Bounded => crate::writer::node_to_vec(node, LexicalPolicy::Preserve),
        SourceOperationPolicy::Source => crate::writer::node_to_vec_with_policy(
            node,
            LexicalPolicy::Preserve,
            facets.policy.core,
        ),
    }
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
    let provenance = provenance(profile, path, anchor)?;
    let placement = OpaquePlacement::new(
        placement,
        u32::try_from(ordinal).map_err(|_| MetadataDecodeError::ResourceLimit("opaque ordinal"))?,
    )
    .map_err(|x| MetadataDecodeError::Core(x.to_string()))?;
    let media = MediaKind::new("application/xml").expect("static media kind");
    let facet =
        if facets.policy.core == SourceOperationPolicy::Source && bytes.len() > MAX_ASSET_BYTES {
            let reference = AssetReference::new(
                Sha256Digest::for_bytes(&bytes),
                u64::try_from(bytes.len())
                    .map_err(|_| MetadataDecodeError::ResourceLimit("opaque bytes"))?,
                media,
            )
            .map_err(|x| MetadataDecodeError::Core(x.to_string()))?;
            facets
                .content
                .borrow_mut()
                .entry(reference.sha256())
                .or_insert_with(|| Arc::new(bytes));
            OpaqueFacet::from_reference(provenance, placement, reference)
        } else {
            OpaqueFacet::new(provenance, placement, bytes, media)
                .map_err(|x| MetadataDecodeError::Core(x.to_string()))?
        };
    facets.values.push(facet);
    Ok(())
}

// Use the writer's authoritative iterative accounting rather than a second
// recursive implementation of XML lexical/escaping rules.
fn node_lexical_len(node: &XmlNode) -> Result<usize, MetadataDecodeError> {
    crate::writer::node_output_len(node, LexicalPolicy::Preserve)
        .map_err(|error| MetadataDecodeError::Xml(error.to_string()))
}
fn node_normalized_len(node: &XmlNode) -> Result<usize, MetadataDecodeError> {
    crate::writer::node_output_len(node, LexicalPolicy::Normalized)
        .map_err(|error| MetadataDecodeError::Xml(error.to_string()))
}
fn document_lexical_len(document: &XmlDocument) -> Result<usize, MetadataDecodeError> {
    crate::writer::document_output_len(document, LexicalPolicy::Preserve)
        .map_err(|error| MetadataDecodeError::Xml(error.to_string()))
}
fn document_normalized_len(document: &XmlDocument) -> Result<usize, MetadataDecodeError> {
    crate::writer::document_output_len(document, LexicalPolicy::Normalized)
        .map_err(|error| MetadataDecodeError::Xml(error.to_string()))
}

type NamespaceScope = BTreeMap<String, Rc<str>>;
pub(super) struct ResolvedNamespaces {
    element_uris: BTreeMap<usize, Option<Rc<str>>>,
    scopes: BTreeMap<usize, Rc<NamespaceScope>>,
}

impl ResolvedNamespaces {
    fn new() -> Self {
        Self {
            element_uris: BTreeMap::new(),
            scopes: BTreeMap::new(),
        }
    }

    pub(super) fn qname_uri<'a>(&'a self, element: &XmlElement, prefix: &str) -> Option<&'a str> {
        self.scopes
            .get(&element_key(element))?
            .get(prefix)
            .map(|x| x.as_ref())
    }

    fn get(&self, key: &usize) -> Option<&Option<Rc<str>>> {
        self.element_uris.get(key)
    }

    fn insert(&mut self, key: usize, uri: Option<Rc<str>>, scope: Rc<NamespaceScope>) {
        self.element_uris.insert(key, uri);
        self.scopes.insert(key, scope);
    }
}

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

pub(super) fn namespace_uri_for_prefix<'a>(
    element: &XmlElement,
    prefix: &str,
    uris: &'a ResolvedNamespaces,
) -> Option<&'a str> {
    uris.qname_uri(element, prefix)
}

pub(super) fn resolve_namespaces(
    root: &XmlElement,
) -> Result<ResolvedNamespaces, MetadataDecodeError> {
    let mut scope = NamespaceScope::new();
    scope.insert("xml".to_owned(), Rc::from(XML_NAMESPACE));
    let mut uris = ResolvedNamespaces::new();
    collect_namespaces(root, Rc::new(scope), &mut uris)?;
    Ok(uris)
}

fn collect_namespaces(
    root: &XmlElement,
    inherited_scope: Rc<NamespaceScope>,
    uris: &mut ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let mut pending = vec![(root, inherited_scope)];
    while let Some((element, inherited)) = pending.pop() {
        let scope = collect_element_namespaces(element, inherited, uris)?;
        for node in element.children().iter().rev() {
            if let XmlNode::Element(child) = node {
                pending.push((child, Rc::clone(&scope)));
            }
        }
    }
    Ok(())
}

fn collect_element_namespaces(
    element: &XmlElement,
    inherited_scope: Rc<NamespaceScope>,
    uris: &mut ResolvedNamespaces,
) -> Result<Rc<NamespaceScope>, MetadataDecodeError> {
    let mut seen = BTreeSet::new();
    let mut declared_scope: Option<NamespaceScope> = None;
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
                let scope = declared_scope.get_or_insert_with(|| inherited_scope.as_ref().clone());
                if uri.is_empty() {
                    scope.remove(key);
                } else {
                    scope.insert(owned_key, Rc::from(uri));
                }
            }
            AttributeKind::Ordinary(_) => {}
        }
    }
    let scope = declared_scope
        .map(Rc::new)
        .unwrap_or_else(|| Rc::clone(&inherited_scope));
    let uri = match element.name().prefix() {
        Some(prefix) => Some(scope.get(prefix).cloned().ok_or(
            MetadataDecodeError::InvalidEnvelope("unbound element prefix"),
        )?),
        None => scope.get("").cloned(),
    };
    uris.insert(element_key(element), uri, Rc::clone(&scope));
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
    Ok(scope)
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
#[cfg(test)]
fn check_document(document: &XmlDocument) -> Result<(), MetadataDecodeError> {
    check_document_with_policy(document, DEFAULT_METADATA_POLICY)
}
fn check_document_with_policy(
    document: &XmlDocument,
    shape_policy: MetadataShapePolicy,
) -> Result<(), MetadataDecodeError> {
    let mut budget = Budget::default();
    if document.has_utf8_bom() {
        checked_add(
            &mut budget.bytes,
            3,
            shape_policy.limit(MAX_METADATA_BYTES),
            "bytes",
        )?;
    }
    if let Some(value) = document
        .declaration_raw()
        .or_else(|| document.declaration())
    {
        checked_add(
            &mut budget.bytes,
            value.len(),
            shape_policy.limit(MAX_METADATA_BYTES),
            "bytes",
        )?;
    }
    for node in document.before_root().iter().chain(document.after_root()) {
        check_node(node, 0, &mut budget, shape_policy)?;
    }
    check_tree(document.root(), 0, &mut budget, shape_policy)?;
    if document_lexical_len(document)? > shape_policy.limit(MAX_METADATA_BYTES) {
        return Err(MetadataDecodeError::ResourceLimit("bytes"));
    }
    if document_normalized_len(document)? > shape_policy.limit(MAX_METADATA_BYTES) {
        return Err(MetadataDecodeError::ResourceLimit("normalized bytes"));
    }
    crate::writer::validate_document(document)
        .map_err(|error| MetadataDecodeError::Xml(error.to_string()))?;
    Ok(())
}
fn check_node(
    node: &XmlNode,
    depth: usize,
    b: &mut Budget,
    shape_policy: MetadataShapePolicy,
) -> Result<(), MetadataDecodeError> {
    if let XmlNode::Element(element) = node {
        return check_tree(element, depth, b, shape_policy);
    }
    checked_add(&mut b.nodes, 1, shape_policy.nodes, "nodes")?;
    if let Some(raw) = node.raw() {
        return checked_add(
            &mut b.bytes,
            raw.len(),
            shape_policy.limit(MAX_METADATA_BYTES),
            "bytes",
        );
    }
    match node {
        XmlNode::Element(_) => unreachable!("handled above"),
        XmlNode::Text(x) => checked_add(
            &mut b.bytes,
            x.value().len(),
            shape_policy.limit(MAX_METADATA_BYTES),
            "bytes",
        ),
        XmlNode::CData(x) => checked_add(
            &mut b.bytes,
            x.value().len(),
            shape_policy.limit(MAX_METADATA_BYTES),
            "bytes",
        ),
        XmlNode::Comment(x) => checked_add(
            &mut b.bytes,
            x.value().len(),
            shape_policy.limit(MAX_METADATA_BYTES),
            "bytes",
        ),
        XmlNode::ProcessingInstruction(x) | XmlNode::DocType(x) => checked_add(
            &mut b.bytes,
            x.value().len(),
            shape_policy.limit(MAX_METADATA_BYTES),
            "bytes",
        ),
    }
}
fn check_tree(
    root: &XmlElement,
    depth: usize,
    b: &mut Budget,
    shape_policy: MetadataShapePolicy,
) -> Result<(), MetadataDecodeError> {
    enum Pending<'a> {
        Element(&'a XmlElement, usize),
        Node(&'a XmlNode, usize),
    }
    let mut pending = vec![Pending::Element(root, depth)];
    while let Some(item) = pending.pop() {
        match item {
            Pending::Element(element, depth) => {
                check_element(element, depth, b, shape_policy)?;
                let child_depth = depth
                    .checked_add(1)
                    .ok_or(MetadataDecodeError::ResourceLimit("depth"))?;
                for node in element.children().iter().rev() {
                    pending.push(match node {
                        XmlNode::Element(child) => Pending::Element(child, child_depth),
                        _ => Pending::Node(node, child_depth),
                    });
                }
            }
            Pending::Node(node, depth) => check_node(node, depth, b, shape_policy)?,
        }
    }
    Ok(())
}

fn check_element(
    e: &XmlElement,
    depth: usize,
    b: &mut Budget,
    shape_policy: MetadataShapePolicy,
) -> Result<(), MetadataDecodeError> {
    if depth > shape_policy.limit(MAX_METADATA_DEPTH) {
        return Err(MetadataDecodeError::ResourceLimit("depth"));
    }
    checked_add(&mut b.nodes, 1, shape_policy.nodes, "nodes")?;
    if let Some(raw) = e.raw_start() {
        checked_add(
            &mut b.bytes,
            raw.len(),
            shape_policy.limit(MAX_METADATA_BYTES),
            "bytes",
        )?;
    }
    if let Some(raw) = e.raw_end() {
        checked_add(
            &mut b.bytes,
            raw.len(),
            shape_policy.limit(MAX_METADATA_BYTES),
            "bytes",
        )?;
    }
    checked_add(
        &mut b.bytes,
        e.name().raw().len(),
        shape_policy.limit(MAX_METADATA_BYTES),
        "bytes",
    )?;
    for attribute in e.attributes() {
        checked_add(
            &mut b.attributes,
            1,
            shape_policy.limit(MAX_METADATA_ATTRIBUTES),
            "attributes",
        )?;
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
            checked_add(
                &mut b.namespaces,
                1,
                shape_policy.limit(MAX_METADATA_NAMESPACES),
                "namespaces",
            )?;
            let prefix_len = match attribute.kind() {
                AttributeKind::Namespace(prefix) => prefix.as_deref().map_or(0, str::len),
                _ => 0,
            };
            checked_add(
                &mut b.namespace_bytes,
                prefix_len + attribute.value().len(),
                shape_policy.limit(MAX_METADATA_NAMESPACE_BYTES),
                "namespace bytes",
            )?;
        }
        if e.raw_start().is_none() {
            let name_len = match attribute.kind() {
                AttributeKind::Ordinary(name) => name.raw().len(),
                AttributeKind::Namespace(prefix) => 5 + prefix.as_deref().map_or(0, str::len),
            };
            checked_add(
                &mut b.bytes,
                name_len,
                shape_policy.limit(MAX_METADATA_BYTES),
                "bytes",
            )?;
            checked_add(
                &mut b.bytes,
                attribute.value().len(),
                shape_policy.limit(MAX_METADATA_BYTES),
                "bytes",
            )?;
        }
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

    fn enum_owner_alias_fixture(layout: &str) -> String {
        let generated = "<GeneratedType name='EnumList.ВидыКадровыхСобытий' category='List' future='retained'><TypeId>b2c547fd-343e-4bc1-9763-795b28132ed4</TypeId><ValueId>b2c547fd-343e-fbc1-9763-795b28132ed4</ValueId><Future>retained</Future></GeneratedType>";
        let properties = "<Properties><Name>ВидыКадровыхСобытий</Name></Properties>";
        let body = match layout {
            "InternalInfo" => format!(
                "<InternalInfo>{}</InternalInfo>{properties}",
                generated
                    .replace("<GeneratedType", "<xr:GeneratedType")
                    .replace("</GeneratedType>", "</xr:GeneratedType>")
                    .replace("<TypeId>", "<xr:TypeId>")
                    .replace("</TypeId>", "</xr:TypeId>")
                    .replace("<ValueId>", "<xr:ValueId>")
                    .replace("</ValueId>", "</xr:ValueId>")
            ),
            "GeneratedTypes" => format!("{properties}<GeneratedTypes>{generated}</GeneratedTypes>"),
            "Properties/GeneratedTypes" => format!(
                "<Properties><Name>ВидыКадровыхСобытий</Name><GeneratedTypes>{generated}</GeneratedTypes></Properties>"
            ),
            _ => unreachable!("known generated layout"),
        };
        format!(
            "<MetaDataObject xmlns='{MD_NAMESPACE}' xmlns:xr='{XR_NAMESPACE}'><Enum uuid='b2c547fd-343e-4bc1-9763-795b28132ed4'>{body}</Enum></MetaDataObject>"
        )
    }

    #[test]
    fn enum_owner_identity_alias_requires_exact_source_relationship_in_all_layouts() {
        for layout in [
            "InternalInfo",
            "GeneratedTypes",
            "Properties/GeneratedTypes",
        ] {
            let xml = enum_owner_alias_fixture(layout);
            let document = XmlReader::from_slice(xml.as_bytes()).unwrap();
            for policy in [
                SourceOperationPolicy::Bounded,
                SourceOperationPolicy::Source,
            ] {
                let envelope = decode_source_metadata_envelope_with_policy(
                    &document,
                    profile(),
                    path(),
                    policy,
                )
                .unwrap();
                let generated = &envelope.root().generated_types()[0];
                assert!(generated.is_owner_identity_alias(), "{layout} {policy:?}");
                assert_eq!(generated.uuid(), envelope.root().identity().uuid());
                assert_ne!(generated.value_id(), Some(generated.uuid()));
                assert_eq!(envelope.configuration().unwrap().objects().len(), 1);
                assert_eq!(envelope.emit(&profile()).unwrap(), xml.as_bytes());
                assert!(
                    envelope
                        .root()
                        .opaque_facets()
                        .as_slice()
                        .iter()
                        .any(|facet| {
                            facet
                                .emit_permit(&profile())
                                .unwrap()
                                .bytes()
                                .windows(b"future='retained'".len())
                                .any(|window| window == b"future='retained'")
                        })
                );
                let unchanged_model = envelope
                    .clone()
                    .with_model(envelope.root().clone(), Vec::new())
                    .unwrap();
                assert!(unchanged_model.emit(&profile()).is_err());
            }
            assert!(
                decode_metadata_envelope(&document, profile(), path())
                    .unwrap()
                    .root()
                    .generated_types()[0]
                    .is_owner_identity_alias()
            );
        }
    }

    #[test]
    fn enum_owner_identity_alias_does_not_claim_other_collisions_or_spoofed_names() {
        let xml = enum_owner_alias_fixture("GeneratedTypes");
        let replacements = [
            (
                "name='EnumList.ВидыКадровыхСобытий'",
                "name='EnumList.Other'",
            ),
            (
                "name='EnumList.ВидыКадровыхСобытий'",
                "name='EnumRef.ВидыКадровыхСобытий'",
            ),
            (
                "name='EnumList.ВидыКадровыхСобытий'",
                "name='enumList.ВидыКадровыхСобытий'",
            ),
            ("name='EnumList.ВидыКадровыхСобытий'", ""),
            (
                "name='EnumList.ВидыКадровыхСобытий'",
                "xmlns:fake='urn:unknown' fake:name='EnumList.ВидыКадровыхСобытий'",
            ),
            ("category='List'", "category='list'"),
            ("category='List'", "category='Manager'"),
            ("category='List'", ""),
            ("<Name>ВидыКадровыхСобытий</Name>", "<Name>Other</Name>"),
            (
                "<ValueId>b2c547fd-343e-fbc1-9763-795b28132ed4</ValueId>",
                "",
            ),
            (
                "<ValueId>b2c547fd-343e-fbc1-9763-795b28132ed4</ValueId>",
                "<ValueId>b2c547fd-343e-4bc1-9763-795b28132ed4</ValueId>",
            ),
            (
                "<ValueId>b2c547fd-343e-fbc1-9763-795b28132ed4</ValueId>",
                "<ValueId>00000000-0000-0000-0000-000000000000</ValueId>",
            ),
        ];
        let mut negatives: Vec<String> = replacements
            .iter()
            .map(|(from, to)| xml.replace(from, to))
            .collect();
        negatives.push(
            xml.replace("<Enum ", "<Catalog ")
                .replace("</Enum>", "</Catalog>"),
        );
        negatives.push(xml.replace("</GeneratedTypes>", "<GeneratedType name='EnumList.ВидыКадровыхСобытий' category='List'><TypeId>b2c547fd-343e-4bc1-9763-795b28132ed4</TypeId><ValueId>33333333-3333-4333-8333-333333333333</ValueId></GeneratedType></GeneratedTypes>"));
        for (index, negative) in negatives.iter().enumerate() {
            let document = XmlReader::from_slice(negative.as_bytes()).unwrap();
            for policy in [
                SourceOperationPolicy::Bounded,
                SourceOperationPolicy::Source,
            ] {
                assert!(
                    decode_source_metadata_envelope_with_policy(
                        &document,
                        profile(),
                        path(),
                        policy
                    )
                    .is_err(),
                    "negative {index} {policy:?}"
                );
            }
        }
        let distinct = xml.replace(
            "<TypeId>b2c547fd-343e-4bc1-9763-795b28132ed4</TypeId>",
            "<TypeId>11111111-1111-4111-8111-111111111111</TypeId>",
        );
        let document = XmlReader::from_slice(distinct.as_bytes()).unwrap();
        let envelope = decode_source_metadata_envelope_with_policy(
            &document,
            profile(),
            path(),
            SourceOperationPolicy::Source,
        )
        .unwrap();
        assert!(!envelope.root().generated_types()[0].is_owner_identity_alias());
        assert_eq!(envelope.emit(&profile()).unwrap(), distinct.as_bytes());
    }

    #[test]
    #[ignore = "requires independently prepared real UH native Enum directory"]
    fn genuine_enum_owner_identity_aliases_keep_all_source_bytes_and_roles() {
        let directory =
            std::env::var_os("IBCMD_ENUM_ALIAS_NATIVE").expect("IBCMD_ENUM_ALIAS_NATIVE");
        let mut accepted = 0;
        for item in std::fs::read_dir(directory).unwrap() {
            let path_on_disk = item.unwrap().path();
            if path_on_disk
                .extension()
                .is_none_or(|extension| extension != "xml")
            {
                continue;
            }
            let bytes = std::fs::read(path_on_disk).unwrap();
            let document = XmlReader::from_slice(&bytes).unwrap();
            let current_profile = if document.root().attributes().iter().any(|attribute| {
                matches!(attribute.kind(), AttributeKind::Ordinary(name) if name.local() == "version" && name.prefix().is_none())
                    && attribute.value() == "2.21"
            }) {
                ProfileId::parse("xml:2.21").unwrap()
            } else {
                profile()
            };
            let root = document
                .root()
                .children()
                .iter()
                .find_map(|node| {
                    if let XmlNode::Element(element) = node {
                        Some(element)
                    } else {
                        None
                    }
                })
                .unwrap();
            let uid = uuid_attr(root).unwrap();
            let Some(XmlNode::Element(info)) = root.children().iter().find(|node| matches!(node, XmlNode::Element(element) if element.name().local() == "InternalInfo")) else { continue; };
            let has_own_type = info.children().iter().any(|node| {
                let XmlNode::Element(generated) = node else { return false; };
                generated.children().iter().any(|node| matches!(node, XmlNode::Element(type_id) if type_id.name().local() == "TypeId" && element_text(type_id).unwrap().is_some_and(|value| value == uid.to_string())))
            });
            if !has_own_type {
                continue;
            }
            for policy in [
                SourceOperationPolicy::Bounded,
                SourceOperationPolicy::Source,
            ] {
                let envelope = decode_source_metadata_envelope_with_policy(
                    &document,
                    current_profile.clone(),
                    path(),
                    policy,
                )
                .unwrap();
                let aliases: Vec<_> = envelope
                    .root()
                    .generated_types()
                    .iter()
                    .filter(|generated| generated.is_owner_identity_alias())
                    .collect();
                assert_eq!(aliases.len(), 1);
                assert_eq!(aliases[0].kind().as_str(), "List");
                assert_eq!(aliases[0].uuid(), uid);
                assert_eq!(
                    envelope.configuration().unwrap().objects().len(),
                    1 + envelope.descendants().len()
                );
                assert_eq!(envelope.emit(&current_profile).unwrap(), bytes);
            }
            accepted += 1;
        }
        assert_eq!(
            accepted, 13,
            "independently inventoried real EnumList aliases"
        );
    }

    #[test]
    fn source_operation_typed_object_continuations_preserve_preorder_and_first_errors() {
        let xml = b"<MetaDataObject><Catalog uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>Root</Name></Properties><ChildObjects> <TabularSection uuid='22222222-2222-4222-8222-222222222222'><Properties><Name>Table</Name></Properties><ChildObjects><Attribute uuid='33333333-3333-4333-8333-333333333333'><Properties><Name>Nested</Name></Properties></Attribute></ChildObjects></TabularSection> <Attribute uuid='44444444-4444-4444-8444-444444444444'><Properties><Name>Sibling</Name></Properties></Attribute> </ChildObjects><Future>unchanged</Future></Catalog></MetaDataObject>";
        let document = XmlReader::from_slice(xml).unwrap();
        let bounded = decode_source_metadata_envelope(&document, profile(), path()).unwrap();
        let source = decode_source_metadata_envelope_with_policy(
            &document,
            profile(),
            path(),
            SourceOperationPolicy::Source,
        )
        .unwrap();
        assert_eq!(source.root(), bounded.root());
        assert_eq!(source.descendants(), bounded.descendants());
        assert_eq!(source.descendants().len(), 3);
        assert_eq!(
            source.descendants()[0].owner(),
            Some(source.root().identity().uuid())
        );
        assert_eq!(
            source.descendants()[1].owner(),
            Some(source.descendants()[0].identity().uuid())
        );
        assert_eq!(
            source.descendants()[2].owner(),
            Some(source.root().identity().uuid())
        );
        assert_eq!(source.emit(&profile()).unwrap(), xml);
        for invalid in [
            // Child UUID must be checked before the suspended parent's missing Name.
            "<MetaDataObject><Catalog uuid='11111111-1111-4111-8111-111111111111'><ChildObjects><Attribute uuid='invalid'><Properties><Name>Child</Name></Properties></Attribute></ChildObjects></Catalog></MetaDataObject>",
            // The deep child's duplicate Name precedes the later duplicate parent container.
            "<MetaDataObject><Catalog uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>Root</Name></Properties><ChildObjects><Attribute uuid='22222222-2222-4222-8222-222222222222'><Properties><Name>A</Name><Name>B</Name></Properties></Attribute></ChildObjects><Properties/></Catalog></MetaDataObject>",
        ] {
            let document = XmlReader::from_slice(invalid.as_bytes()).unwrap();
            assert_eq!(
                decode_source_metadata_envelope(&document, profile(), path()).unwrap_err(),
                decode_source_metadata_envelope_with_policy(
                    &document,
                    profile(),
                    path(),
                    SourceOperationPolicy::Source
                )
                .unwrap_err()
            );
        }
    }

    #[test]
    fn source_operation_typed_objects_deep_512_are_iterative_and_reject_late_corruption() {
        std::thread::Builder::new().stack_size(128 * 1024).spawn(|| {
            const OBJECTS: usize = 512;
            let uuids: Vec<_> = (0..OBJECTS).map(|index| format!("{index:08x}-1111-4111-8111-111111111111")).collect();
            let mut xml = String::from("<MetaDataObject>");
            for (index, uuid) in uuids.iter().enumerate() {
                let kind = if index == 0 { "Catalog" } else { "Attribute" };
                xml.push_str(&format!("<{kind} uuid='{uuid}' future='retained'><Properties><Name>N{index}</Name><Future>opaque{index}</Future></Properties>"));
                if index + 1 != OBJECTS { xml.push_str("<ChildObjects>"); }
            }
            xml.push_str("</Attribute>");
            for index in (0..OBJECTS-1).rev() {
                xml.push_str(if index == 0 { "</ChildObjects></Catalog>" } else { "</ChildObjects></Attribute>" });
            }
            xml.push_str("</MetaDataObject>");
            let document = XmlReader::from_slice(xml.as_bytes()).unwrap();
            assert!(matches!(decode_source_metadata_envelope(&document, profile(), path()), Err(MetadataDecodeError::ResourceLimit("depth"))));
            let envelope = decode_source_metadata_envelope_with_policy(&document, profile(), path(), SourceOperationPolicy::Source).unwrap();
            assert_eq!(envelope.configuration().unwrap().len(), OBJECTS);
            assert_eq!(envelope.descendants().len(), OBJECTS-1);
            for (index, object) in envelope.descendants().iter().enumerate() {
                assert_eq!(object.identity().uuid().to_string(), uuids[index+1]);
                assert_eq!(object.owner().unwrap().to_string(), uuids[index]);
                assert_eq!(object.identity().path().segments().len(), 1+2*(index+1));
                assert_eq!(object.identity().path().segments().last().unwrap().as_index(), Some(0));
                assert_eq!(object.opaque_facets().as_slice().len(), 3);
            }
            assert_eq!(envelope.emit(&profile()).unwrap(), xml.as_bytes());
            let copy = envelope.clone();
            assert_eq!(copy.source_document(), envelope.source_document());
            assert_eq!(copy.root(), envelope.root());
            assert_eq!(copy.descendants(), envelope.descendants());
            let edited = copy.with_model(envelope.root().clone(), envelope.descendants().to_vec()).unwrap();
            assert!(matches!(edited.emit(&profile()), Err(MetadataEncodeError::ModelChanged { .. })));
            let duplicate = xml.replace(&format!("uuid='{}'", uuids[OBJECTS-1]), &format!("uuid='{}'", uuids[0]));
            let duplicate = XmlReader::from_slice(duplicate.as_bytes()).unwrap();
            assert!(decode_source_metadata_envelope_with_policy(&duplicate, profile(), path(), SourceOperationPolicy::Source).is_err());
            let malformed = xml.replace(&format!("uuid='{}'", uuids[OBJECTS-1]), "uuid='invalid'");
            let malformed = XmlReader::from_slice(malformed.as_bytes()).unwrap();
            assert!(matches!(decode_source_metadata_envelope_with_policy(&malformed, profile(), path(), SourceOperationPolicy::Source), Err(MetadataDecodeError::InvalidUuid(_))));
        }).unwrap().join().unwrap();
    }

    #[test]
    fn source_operation_large_unknown_facet_requires_verified_content_and_unchanged_model() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MetadataEnvelope>();
        let text = "x".repeat(MAX_ASSET_BYTES + 1);
        let xml = format!(
            "<MetaDataObject><Catalog uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>Owner</Name><Future>{text}</Future></Properties></Catalog></MetaDataObject>"
        );
        let document = XmlReader::from_slice(xml.as_bytes()).unwrap();
        assert!(decode_source_metadata_envelope(&document, profile(), path()).is_err());
        let mut envelope = decode_source_metadata_envelope_with_policy(
            &document,
            profile(),
            path(),
            SourceOperationPolicy::Source,
        )
        .unwrap();
        let facet = envelope
            .root()
            .opaque_facets()
            .as_slice()
            .iter()
            .find(|facet| facet.asset_reference().is_some())
            .expect("large unknown data must have an external reference");
        assert!(facet.byte_len() > MAX_ASSET_BYTES as u64);
        assert!(facet.asset_reference().is_some());
        assert!(facet.emit_permit(&profile()).is_err());
        let digest = facet.asset_reference().unwrap().sha256();
        let original = envelope.content.get(&digest).unwrap().clone();
        assert_eq!(Sha256Digest::for_bytes(&original), digest);
        assert_eq!(envelope.emit(&profile()).unwrap(), xml.as_bytes());
        assert!(
            envelope
                .emit(&ProfileId::parse("xml:2.21").unwrap())
                .is_err()
        );

        // Content metadata cannot authorize missing, truncated, or same-length
        // changed bytes. Cache state is operational and is never serialized.
        Arc::make_mut(&mut envelope.content).remove(&digest);
        assert!(envelope.emit(&profile()).is_err());
        Arc::make_mut(&mut envelope.content).insert(digest, Arc::new(vec![b'x']));
        assert!(envelope.emit(&profile()).is_err());
        let mut tampered = original.as_ref().clone();
        *tampered.last_mut().unwrap() ^= 1;
        Arc::make_mut(&mut envelope.content).insert(digest, Arc::new(tampered));
        assert!(envelope.emit(&profile()).is_err());
        Arc::make_mut(&mut envelope.content).insert(digest, original);

        let mut parts = CanonicalObjectParts::new(
            envelope.root().identity().clone(),
            envelope.root().kind().clone(),
            envelope.root().provenance().clone(),
        );
        parts.properties = vec![
            CanonicalField::named(
                "Name",
                CanonicalValue::text(CanonicalText::new("Edited").unwrap()),
            )
            .unwrap(),
        ];
        parts.opaque_facets = envelope.root().opaque_facets().clone();
        let root = CanonicalObject::new_with_policy(parts, SourceOperationPolicy::Source).unwrap();
        let edited = envelope.clone().with_model(root, Vec::new()).unwrap();
        assert!(matches!(
            edited.emit(&profile()),
            Err(MetadataEncodeError::ModelChanged { .. })
        ));
        assert_eq!(envelope.emit(&profile()).unwrap(), xml.as_bytes());
    }

    #[test]
    fn source_operation_deep_unknown_xml_is_iterative_and_namespace_scoped() {
        std::thread::Builder::new().stack_size(128 * 1024).spawn(|| {
            let mut xml = String::from("<MetaDataObject><Catalog uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>Owner</Name><Future xmlns:p='urn:outer'>");
            for _ in 0..4096 { xml.push_str("<p:n>"); }
            xml.push_str("<p:inner xmlns:p='urn:inner' p:value='unchanged'/><p:sibling/>");
            for _ in 0..4096 { xml.push_str("</p:n>"); }
            xml.push_str("</Future></Properties></Catalog></MetaDataObject>");
            let document = XmlReader::from_slice(xml.as_bytes()).unwrap();
            assert!(matches!(decode_source_metadata_envelope(&document, profile(), path()), Err(MetadataDecodeError::ResourceLimit("depth"))));
            let envelope = decode_source_metadata_envelope_with_policy(&document, profile(), path(), SourceOperationPolicy::Source).unwrap();
            assert_eq!(envelope.root().opaque_facets().as_slice().iter().filter(|facet| facet.byte_len() > 0).count(), 1);
            assert_eq!(envelope.emit(&profile()).unwrap(), xml.as_bytes());
            let namespaces = resolve_namespaces(document.root()).unwrap();
            // The two siblings resolve using the same parent binding; the
            // inner declaration never leaks into the following sibling.
            let mut pending = vec![document.root()];
            let mut observed = Vec::new();
            while let Some(element) = pending.pop() {
                if matches!(element.name().local(), "inner" | "sibling") { observed.push((element.name().local(), uri_of(element, &namespaces))); }
                pending.extend(element.children().iter().filter_map(|node| if let XmlNode::Element(element) = node { Some(element) } else { None }));
            }
            assert!(observed.contains(&("inner", Some("urn:inner"))));
            assert!(observed.contains(&("sibling", Some("urn:outer"))));
            let copy = envelope.clone();
            assert_eq!(copy.source_document(), envelope.source_document());
        }).unwrap().join().unwrap();
    }

    #[test]
    fn source_operation_keeps_long_exact_object_coordinates() {
        let name = "Ж".repeat(65);
        assert!(PathSegment::name(&name).is_err());
        let source = SourceOperationPolicy::Source;
        let path = ObjectPath::new_with_policy(
            vec![PathSegment::name_with_policy(&name, source).unwrap()],
            source,
        )
        .unwrap();
        let document = XmlReader::from_slice(b"<MetaDataObject><Catalog uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>Owner</Name></Properties><ChildObjects><Attribute uuid='22222222-2222-4222-8222-222222222222'><Properties><Name>Field</Name></Properties></Attribute></ChildObjects></Catalog></MetaDataObject>").unwrap();
        let envelope =
            decode_source_metadata_envelope_with_policy(&document, profile(), path, source)
                .unwrap();
        assert_eq!(envelope.descendants().len(), 1);
        assert_eq!(
            envelope.descendants()[0].identity().path().segments()[0].as_name(),
            Some(name.as_str())
        );
        assert_eq!(
            envelope.emit(&profile()).unwrap(),
            crate::XmlWriter::to_vec(&document, LexicalPolicy::Preserve).unwrap()
        );
    }

    #[test]
    fn source_operation_policy_preserves_large_typed_text_and_revalidation() {
        let text = "x".repeat(ibcmd_core::value::MAX_CANONICAL_TEXT_BYTES + 1);
        let xml = format!(
            "<MetaDataObject xmlns='{MD_NAMESPACE}'><Catalog uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>Owner</Name><Synonym><item xmlns='{V8_NAMESPACE}'><lang>ru</lang><content>{text}</content></item></Synonym></Properties></Catalog></MetaDataObject>"
        );
        let document = XmlReader::from_slice(xml.as_bytes()).unwrap();
        assert!(matches!(
            decode_source_metadata_envelope(&document, profile(), path()),
            Err(MetadataDecodeError::ResourceLimit("canonical text"))
        ));
        let envelope = decode_source_metadata_envelope_with_policy(
            &document,
            profile(),
            path(),
            SourceOperationPolicy::Source,
        )
        .unwrap();
        assert_eq!(
            MetadataRegistry::default()
                .encode(&envelope, &profile())
                .unwrap(),
            xml.as_bytes()
        );
        let configuration = envelope.configuration().unwrap();
        assert_eq!(configuration.len(), 1);
        use ibcmd_core::value::CanonicalValueKind;
        let CanonicalValueKind::Sequence(items) = envelope.root().properties()[1].value().kind()
        else {
            panic!("Synonym sequence was dropped")
        };
        assert_eq!(items.len(), 1);
        let CanonicalValueKind::Record(fields) = items[0].kind() else {
            panic!("Synonym item was dropped")
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[1].name().as_str(), "content");
        let CanonicalValueKind::Text(actual) = fields[1].value().kind() else {
            panic!("Synonym text was dropped")
        };
        assert_eq!(actual.as_str(), text);
        let edited = envelope
            .clone()
            .with_model(envelope.root().clone(), envelope.descendants().to_vec())
            .unwrap();
        assert_eq!(edited.root(), envelope.root());
        assert!(matches!(
            MetadataRegistry::default().encode(&edited, &profile()),
            Err(MetadataEncodeError::ModelChanged { .. })
        ));
        // Explicit operation policy is retained inside the envelope, never
        // inferred from retained data or allowed to enable source-byte replay.
        let reconstructed =
            MetadataEnvelope::from_parts(envelope.root().clone(), Vec::new(), document).unwrap();
        assert!(!reconstructed.source_model_unchanged());
        assert!(matches!(
            MetadataRegistry::default().encode(&reconstructed, &profile()),
            Err(MetadataEncodeError::ModelChanged { .. })
        ));
    }

    #[test]
    fn source_operation_counters_are_explicit_checked_and_keep_depth_safety() {
        let source = MetadataShapePolicy::source_with(SourceOperationPolicy::Source);
        let facets = FacetSet::root(source);
        facets.budget.borrow_mut().count = SOURCE_METADATA_POLICY.facets;
        facets.reserve(MAX_METADATA_BYTES + 1).unwrap();
        assert_eq!(
            facets.budget.borrow().count,
            SOURCE_METADATA_POLICY.facets + 1
        );
        let child = FacetSet::child_with(&facets, Vec::new());
        assert_eq!(child.policy.core, SourceOperationPolicy::Source);
        child.budget.borrow_mut().count = usize::MAX;
        assert!(matches!(
            child.reserve(0),
            Err(MetadataDecodeError::ResourceLimit("opaque facets"))
        ));
        let node = XmlElement::new(QName::new("X").unwrap());
        let mut budget = Budget {
            nodes: SOURCE_METADATA_POLICY.nodes,
            ..Budget::default()
        };
        check_tree(&node, 0, &mut budget, source).unwrap();
        budget.nodes = usize::MAX;
        assert!(matches!(
            check_tree(&node, 0, &mut budget, source),
            Err(MetadataDecodeError::ResourceLimit("nodes"))
        ));
        assert!(matches!(
            check_tree(
                &node,
                MAX_METADATA_DEPTH + 1,
                &mut Budget::default(),
                DEFAULT_METADATA_POLICY
            ),
            Err(MetadataDecodeError::ResourceLimit("depth"))
        ));
    }

    #[test]
    fn source_operation_policy_keeps_same_semantics_and_rejects_bad_source() {
        let xml = b"<MetaDataObject><Catalog uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>Owner</Name></Properties></Catalog></MetaDataObject>";
        let document = XmlReader::from_slice(xml).unwrap();
        let bounded = decode_source_metadata_envelope(&document, profile(), path()).unwrap();
        let source = decode_source_metadata_envelope_with_policy(
            &document,
            profile(),
            path(),
            SourceOperationPolicy::Source,
        )
        .unwrap();
        assert_eq!(source.root(), bounded.root());
        assert_eq!(source.descendants(), bounded.descendants());
        for input in [
            String::from_utf8(xml.to_vec())
                .unwrap()
                .replace("</Name>", "</Name><Name>Duplicate</Name>"),
            String::from_utf8(xml.to_vec())
                .unwrap()
                .replace("11111111-1111-4111-8111-111111111111", "not-a-uuid"),
            String::from_utf8(xml.to_vec())
                .unwrap()
                .replace("<Properties>", "<Properties xmlns='urn:unknown'>"),
        ] {
            let document = XmlReader::from_slice(input.as_bytes()).unwrap();
            assert!(
                decode_source_metadata_envelope_with_policy(
                    &document,
                    profile(),
                    path(),
                    SourceOperationPolicy::Source
                )
                .is_err()
            );
        }
    }

    #[test]
    fn source_configuration_retains_palette_references_and_rejects_unknown_bare_kinds() {
        let xml = b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' version='2.21'><Configuration uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>Owner</Name></Properties><ChildObjects><PaletteColor>Accent</PaletteColor></ChildObjects></Configuration></MetaDataObject>";
        let document = XmlReader::from_slice(xml).unwrap();
        let envelope = decode_source_metadata_envelope(&document, profile(), path()).unwrap();
        assert!(envelope.descendants().is_empty());
        assert_eq!(envelope.source_document(), &document);
        let unknown = String::from_utf8(xml.to_vec())
            .unwrap()
            .replace("PaletteColor", "UnknownPalette");
        let unknown = XmlReader::from_slice(unknown.as_bytes()).unwrap();
        assert!(decode_source_metadata_envelope(&unknown, profile(), path()).is_err());
    }

    #[test]
    fn source_configuration_preserves_large_ordered_reference_inventory_and_policy() {
        // The independent UH corpus has this many bare Configuration references.
        // Interleaved whitespace also exercises the aggregate opaque-facet bound.
        const REFERENCES: usize = 25_977;
        let mut xml = String::from(
            "<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' version='2.20'><Configuration uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>Owner</Name></Properties><ChildObjects>\n  ",
        );
        for index in 0..REFERENCES {
            xml.push_str(&format!("<Catalog>Sibling{index:05}</Catalog>\n  "));
        }
        xml.push_str("</ChildObjects></Configuration></MetaDataObject>");
        let document = XmlReader::from_slice(xml.as_bytes()).unwrap();
        for result in [
            decode_metadata_envelope(&document, profile(), path()),
            decode_configuration_envelope(&document, profile(), path()),
        ] {
            assert!(matches!(
                result,
                Err(MetadataDecodeError::ResourceLimit("nodes"))
            ));
        }
        let envelope = decode_source_metadata_envelope(&document, profile(), path()).unwrap();
        assert!(envelope.descendants().is_empty());
        let references: Vec<_> = envelope
            .root()
            .opaque_facets()
            .as_slice()
            .iter()
            .filter(|facet| facet.placement().kind().as_str() == "xml:child-object-reference")
            .collect();
        assert_eq!(references.len(), REFERENCES);
        for (index, reference) in references.iter().enumerate() {
            let permit = reference.emit_permit(&profile()).unwrap();
            assert_eq!(
                permit.bytes(),
                format!("<Catalog>Sibling{index:05}</Catalog>").as_bytes()
            );
            assert_eq!(reference.placement().ordinal() as usize, index * 2 + 1);
        }
        assert_eq!(
            MetadataRegistry::default()
                .encode(&envelope, &profile())
                .unwrap(),
            xml.as_bytes()
        );
        // Revalidating an edited source envelope retains its explicit source
        // shape, while still refusing unchanged-source emission after an edit.
        let edited = envelope
            .clone()
            .with_model(envelope.root().clone(), Vec::new())
            .unwrap();
        assert!(!edited.source_model_unchanged());
        assert!(matches!(
            MetadataRegistry::default().encode(&edited, &profile()),
            Err(MetadataEncodeError::ModelChanged { .. })
        ));
        assert!(
            MetadataEnvelope::from_parts(envelope.root().clone(), Vec::new(), document).is_err()
        );
    }

    #[test]
    fn source_shape_bounds_still_reject_without_allocating_the_maximum() {
        let element = XmlElement::new(QName::new("X").unwrap());
        let mut budget = Budget {
            nodes: SOURCE_METADATA_POLICY.nodes,
            ..Budget::default()
        };
        assert!(matches!(
            check_tree(&element, 0, &mut budget, SOURCE_METADATA_POLICY),
            Err(MetadataDecodeError::ResourceLimit("nodes"))
        ));
        let facets = FacetSet::root(SOURCE_METADATA_POLICY);
        facets.budget.borrow_mut().count = SOURCE_METADATA_POLICY.facets;
        assert!(matches!(
            facets.reserve(0),
            Err(MetadataDecodeError::ResourceLimit("opaque facets"))
        ));
        let facets = FacetSet::root(SOURCE_METADATA_POLICY);
        assert!(matches!(
            facets.reserve(MAX_METADATA_BYTES + 1),
            Err(MetadataDecodeError::ResourceLimit("opaque bytes"))
        ));
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
    fn source_envelopes_preserve_exact_sibling_reference_kinds_without_weakening_generic_decode() {
        for (family, child) in [
            ("Catalog", "Form"),
            ("Document", "Template"),
            ("Subsystem", "Subsystem"),
            ("CalculationRegister", "Recalculation"),
            ("ExternalDataSource", "Table"),
            ("FilterCriterion", "Form"),
        ] {
            let source = format!(
                "<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' version='2.20'><{family} uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>Owner</Name></Properties><ChildObjects><{child}>Sibling</{child}></ChildObjects></{family}></MetaDataObject>"
            );
            let document = XmlReader::from_slice(source.as_bytes()).unwrap();
            assert!(decode_metadata_envelope(&document, profile(), path()).is_err());
            let envelope = decode_source_metadata_envelope(&document, profile(), path()).unwrap();
            assert!(envelope.descendants().is_empty());
            assert!(
                envelope
                    .root()
                    .opaque_facets()
                    .as_slice()
                    .iter()
                    .any(|facet| facet.placement().kind().as_str() == "xml:child-object-reference")
            );
            assert_eq!(
                MetadataRegistry::default()
                    .encode(&envelope, &profile())
                    .unwrap(),
                source.as_bytes()
            );
            let unknown = source.replace(
                &format!("<{child}>Sibling</{child}>"),
                "<FutureChild>Sibling</FutureChild>",
            );
            assert!(
                decode_source_metadata_envelope(
                    &XmlReader::from_slice(unknown.as_bytes()).unwrap(),
                    profile(),
                    path()
                )
                .is_err()
            );
        }
    }
    #[test]
    fn source_filter_criterion_forms_are_references_but_commands_require_identity() {
        let source = "<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' version='2.20'><FilterCriterion uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>Owner</Name></Properties><ChildObjects><Form>List</Form><Command uuid='22222222-2222-4222-8222-222222222222'><Properties><Name>Open</Name></Properties></Command></ChildObjects></FilterCriterion></MetaDataObject>";
        for policy in [
            SourceOperationPolicy::Bounded,
            SourceOperationPolicy::Source,
        ] {
            let envelope = decode_source_metadata_envelope_with_policy(
                &XmlReader::from_slice(source.as_bytes()).unwrap(),
                profile(),
                path(),
                policy,
            )
            .unwrap();
            assert_eq!(envelope.descendants().len(), 1);
            assert_eq!(envelope.descendants()[0].kind().as_str(), "Command");
            assert_eq!(
                envelope.descendants()[0].owner(),
                Some(envelope.root().identity().uuid())
            );
            assert_eq!(
                MetadataRegistry::default()
                    .encode(&envelope, &profile())
                    .unwrap(),
                source.as_bytes()
            );
            for invalid in [
                source.replace(" uuid='22222222-2222-4222-8222-222222222222'", ""),
                source.replace("<Form>List</Form>", "<Template>List</Template>"),
                source.replace("<Form>List</Form>", "<FutureChild>List</FutureChild>"),
            ] {
                assert!(
                    decode_source_metadata_envelope_with_policy(
                        &XmlReader::from_slice(invalid.as_bytes()).unwrap(),
                        profile(),
                        path(),
                        policy,
                    )
                    .is_err()
                );
            }
        }
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
        let root_scope = uris.scopes.get(&element_key(&root)).unwrap();
        for node in root.children() {
            let XmlNode::Element(child) = node else {
                unreachable!()
            };
            let child_uri = uris.get(&element_key(child)).unwrap().as_ref().unwrap();
            assert!(Rc::ptr_eq(root_uri, child_uri));
            let child_scope = uris.scopes.get(&element_key(child)).unwrap();
            assert!(Rc::ptr_eq(root_scope, child_scope));
        }
        assert_eq!(Rc::strong_count(root_scope), root.children().len() + 1);
        assert_eq!(Rc::strong_count(root_uri), root.children().len() + 2);
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

        let mut facets = FacetSet::root(DEFAULT_METADATA_POLICY);
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
        let facets = FacetSet::root(DEFAULT_METADATA_POLICY);
        assert!(matches!(
            facets.reserve(predicted),
            Err(MetadataDecodeError::ResourceLimit("opaque bytes"))
        ));
    }
}
