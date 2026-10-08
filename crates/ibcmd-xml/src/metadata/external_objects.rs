//! Typed external XML roots over existing canonical DataProcessor/Report.
//! This component edits root metadata; it does not construct EPF/ERF storage.
use super::business_objects::{only_element_child, project_name_only_children, push_text};
use super::common::{
    MD_NAMESPACE, MetadataDecodeError, MetadataEnvelope, ResolvedNamespaces, V8_NAMESPACE,
    XR_NAMESPACE, decode_external_metadata_envelope, element_text, resolve_namespaces, typed,
};
use super::language::{
    canonical_field, copy_object_parts, decode_to_encode, invalid_model, profile_version,
    root_version, set_unprefixed_attribute, validate_decode_profile,
};
use super::package::ExternalSourceBinding;
use super::registry::{
    MetadataEncodeError, MetadataFamilyCodec, MetadataRegistry, MetadataRegistryError,
};
use crate::{AttributeKind, LexicalPolicy, XmlDocument, XmlElement, XmlNode, XmlWriter};
use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::ObjectPath;
use ibcmd_core::family::FamilyId;
use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::model::CanonicalObject;
use ibcmd_core::value::{CanonicalValue, CanonicalValueKind};
use ibcmd_schema::external_artifact::ExternalArtifactKind;
use std::collections::{BTreeMap, BTreeSet};

/// Explicit registration keeps the existing same-family Config codecs intact.
pub fn register_external_data_processor_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    register(registry, ExternalArtifactKind::DataProcessor)
}
pub fn register_external_report_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    register(registry, ExternalArtifactKind::Report)
}
fn register(
    registry: &mut MetadataRegistry,
    kind: ExternalArtifactKind,
) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(ExternalCodec {
        kind,
        family: FamilyId::parse(kind.external_kind()).expect("registered external family"),
    }))
}
struct ExternalCodec {
    kind: ExternalArtifactKind,
    family: FamilyId,
}
impl MetadataFamilyCodec for ExternalCodec {
    fn family_id(&self) -> &FamilyId {
        &self.family
    }
    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        let envelope = decode_external_root(document, source, path)?;
        if envelope.external_source_binding().map(|x| x.kind()) != Some(self.kind) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "external registry family differs",
            ));
        }
        Ok(envelope)
    }
    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        if envelope.external_source_binding().map(|x| x.kind()) != Some(self.kind) {
            return Err(invalid_model(
                envelope.root().identity().path(),
                "external registry family",
            ));
        }
        encode_external_root(envelope, target)
    }
}

/// Transient closed admission proof: only this codec can construct it after
/// consuming every known section. It never stores a second source cache.
pub(super) struct ExternalRootClaims<'a> {
    document: &'a XmlDocument,
    binding: ExternalSourceBinding,
}
impl ExternalRootClaims<'_> {
    pub(super) fn binding_for(
        &self,
        document: &XmlDocument,
    ) -> Result<ExternalSourceBinding, MetadataDecodeError> {
        if !std::ptr::eq(self.document, document) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "external claims belong to another current XML document",
            ));
        }
        self.binding.validate(document)?;
        Ok(self.binding)
    }
}

struct RootSections<'a> {
    object: &'a XmlElement,
    internal: &'a XmlElement,
    properties: &'a XmlElement,
    children: &'a XmlElement,
}
fn attributes(element: &XmlElement, allowed: &[&str]) -> Result<(), MetadataDecodeError> {
    for attribute in element.attributes() {
        if let AttributeKind::Ordinary(name) = attribute.kind()
            && (name.prefix().is_some() || !allowed.contains(&name.local()))
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "unknown external XML attribute",
            ));
        }
    }
    Ok(())
}
fn container_content(
    element: &XmlElement,
    owner: &str,
    property: &'static str,
) -> Result<(), MetadataDecodeError> {
    if element.children().iter().any(|node| match node {
        XmlNode::Text(value) => !value.value().trim().is_empty(),
        XmlNode::CData(value) => !value.value().trim().is_empty(),
        XmlNode::DocType(_) => true,
        _ => false,
    }) {
        return Err(MetadataDecodeError::UnexpectedContent {
            owner: owner.to_owned(),
            property,
        });
    }
    Ok(())
}
fn elements(element: &XmlElement) -> impl Iterator<Item = &XmlElement> {
    element.children().iter().filter_map(|x| match x {
        XmlNode::Element(x) => Some(x),
        _ => None,
    })
}
fn sections<'a>(
    document: &'a XmlDocument,
    kind: ExternalArtifactKind,
    uris: &ResolvedNamespaces,
) -> Result<RootSections<'a>, MetadataDecodeError> {
    attributes(document.root(), &["version"])?;
    container_content(document.root(), kind.external_kind(), "MetaDataObject")?;
    if !typed(document.root(), "MetaDataObject", Some(MD_NAMESPACE), uris) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "external envelope namespace",
        ));
    }
    let object = only_element_child(document.root(), "external root")?;
    if !typed(object, kind.external_kind(), Some(MD_NAMESPACE), uris) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "external root namespace/kind",
        ));
    }
    attributes(object, &["uuid"])?;
    container_content(object, kind.external_kind(), "root sections")?;
    let mut map = BTreeMap::new();
    for section in elements(object) {
        if !["InternalInfo", "Properties", "ChildObjects"].contains(&section.name().local())
            || !typed(section, section.name().local(), Some(MD_NAMESPACE), uris)
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "unknown external section",
            ));
        }
        attributes(section, &[])?;
        if map.insert(section.name().local(), section).is_some() {
            return Err(MetadataDecodeError::Duplicate("external section"));
        }
    }
    Ok(RootSections {
        object,
        internal: *map
            .get("InternalInfo")
            .ok_or(MetadataDecodeError::Missing("InternalInfo"))?,
        properties: *map
            .get("Properties")
            .ok_or(MetadataDecodeError::Missing("Properties"))?,
        children: *map
            .get("ChildObjects")
            .ok_or(MetadataDecodeError::Missing("ChildObjects"))?,
    })
}
fn property_map<'a>(
    properties: &'a XmlElement,
    kind: ExternalArtifactKind,
    version: &str,
    uris: &ResolvedNamespaces,
) -> Result<BTreeMap<&'static str, &'a XmlElement>, MetadataDecodeError> {
    container_content(properties, kind.external_kind(), "Properties")?;
    let mut map = BTreeMap::new();
    for element in elements(properties) {
        let name = kind
            .properties()
            .iter()
            .copied()
            .find(|name| *name == element.name().local())
            .ok_or(MetadataDecodeError::InvalidEnvelope(
                "unknown external property",
            ))?;
        if !kind.property_available(name, version)
            || !typed(element, name, Some(MD_NAMESPACE), uris)
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "external property namespace/edition",
            ));
        }
        attributes(element, &[])?;
        if name != "Synonym" {
            element_text(element)?.ok_or(MetadataDecodeError::InvalidEnvelope(
                "external text property contains unknown nested content",
            ))?;
        }
        if map.insert(name, element).is_some() {
            return Err(MetadataDecodeError::Duplicate("external property"));
        }
    }
    if !map.contains_key("Name") {
        return Err(MetadataDecodeError::Missing("Name"));
    }
    if let Some(synonym) = map.get("Synonym") {
        container_content(synonym, kind.external_kind(), "Synonym")?;
        for item in elements(synonym) {
            if !typed(item, "item", Some(V8_NAMESPACE), uris) {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "external Synonym item",
                ));
            }
            attributes(item, &[])?;
            container_content(item, kind.external_kind(), "Synonym item")?;
            let mut fields = BTreeSet::new();
            for value in elements(item) {
                if !matches!(value.name().local(), "lang" | "content")
                    || !typed(value, value.name().local(), Some(V8_NAMESPACE), uris)
                {
                    return Err(MetadataDecodeError::InvalidEnvelope(
                        "external Synonym field",
                    ));
                }
                attributes(value, &[])?;
                element_text(value)?.ok_or(MetadataDecodeError::InvalidEnvelope(
                    "external Synonym text contains unknown content",
                ))?;
                if !fields.insert(value.name().local()) {
                    return Err(MetadataDecodeError::Duplicate("Synonym field"));
                }
            }
            if fields.len() != 2 {
                return Err(MetadataDecodeError::Missing("Synonym lang/content"));
            }
        }
    }
    Ok(map)
}
fn generated_name(
    internal: &XmlElement,
    binding: ExternalSourceBinding,
    name: &str,
    uris: &ResolvedNamespaces,
) -> Result<String, MetadataDecodeError> {
    container_content(internal, binding.kind().external_kind(), "InternalInfo")?;
    let mut generated = None;
    for element in elements(internal) {
        if typed(element, "ContainedObject", Some(XR_NAMESPACE), uris) {
            attributes(element, &[])?;
            container_content(element, binding.kind().external_kind(), "ContainedObject")?;
            for field in elements(element) {
                attributes(field, &[])?;
            }
            // Package identity inspector consumes exactly ClassId/ObjectId.
        } else if typed(element, "GeneratedType", Some(XR_NAMESPACE), uris) {
            attributes(element, &["name", "category"])?;
            container_content(element, binding.kind().external_kind(), "GeneratedType")?;
            let attr = |local| {
                element.attributes().iter().find_map(|x| match x.kind() {
                    AttributeKind::Ordinary(n) if n.prefix().is_none() && n.local() == local => {
                        Some(x.value())
                    }
                    _ => None,
                })
            };
            let expected = format!("{}.{}", binding.kind().object_type_prefix(), name);
            if attr("category") != Some("Object") || attr("name") != Some(expected.as_str()) {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "external generated category/source name",
                ));
            }
            let mut ids = BTreeMap::new();
            for field in elements(element) {
                if !matches!(field.name().local(), "TypeId" | "ValueId")
                    || !typed(field, field.name().local(), Some(XR_NAMESPACE), uris)
                {
                    return Err(MetadataDecodeError::InvalidEnvelope(
                        "unknown external generated identity",
                    ));
                }
                attributes(field, &[])?;
                let text = element_text(field)?
                    .ok_or(MetadataDecodeError::Missing("generated identity"))?;
                let id = ObjectUuid::parse(text.trim())
                    .map_err(|_| MetadataDecodeError::InvalidUuid(text.clone()))?;
                if id.as_bytes().iter().all(|x| *x == 0)
                    || id == binding.identity().main_uuid
                    || id
                        == binding
                            .identity()
                            .contained
                            .expect("validated identity")
                            .object_id
                {
                    return Err(MetadataDecodeError::InvalidEnvelope(
                        "external generated identity is nil or aliases root",
                    ));
                }
                if ids.insert(field.name().local(), id).is_some() {
                    return Err(MetadataDecodeError::Duplicate(
                        "external generated identity",
                    ));
                }
            }
            if ids.len() != 2 || ids.get("TypeId") == ids.get("ValueId") {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "external generated identity pair",
                ));
            }
            if generated.replace(expected).is_some() {
                return Err(MetadataDecodeError::Duplicate(
                    "external Object generated type",
                ));
            }
        } else {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "unknown external InternalInfo entry",
            ));
        }
    }
    generated.ok_or(MetadataDecodeError::Missing(
        "external Object generated type",
    ))
}
fn validate_references(
    root: &CanonicalObject,
    kind: ExternalArtifactKind,
) -> Result<(), MetadataDecodeError> {
    let name = text_value(root, "Name").map_err(|_| MetadataDecodeError::Missing("Name"))?;
    if name.is_empty() || name.contains('.') {
        return Err(MetadataDecodeError::InvalidEnvelope("external root name"));
    }
    let read_targets = |field| -> Vec<&str> {
        property(root, field)
            .and_then(CanonicalValue::as_sequence)
            .map(|xs| {
                xs.iter()
                    .filter_map(|x| match x.kind() {
                        CanonicalValueKind::Reference(r) => Some(r.target()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let forms = read_targets("ChildForms");
    let templates = read_targets("ChildTemplates");
    for field in kind
        .properties()
        .iter()
        .copied()
        .filter(|x| !matches!(*x, "Name" | "Synonym" | "Comment"))
    {
        let Some(value) = property(root, field) else {
            continue;
        };
        let CanonicalValueKind::Text(value) = value.kind() else {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "external root reference value",
            ));
        };
        let value = value.as_str();
        if value.is_empty() {
            continue;
        }
        let roster = if field == "MainDataCompositionSchema" {
            &templates
        } else if matches!(field, "VariantsStorage" | "SettingsStorage") {
            return Err(MetadataDecodeError::UnevidencedProperty {
                owner: kind.external_kind().to_owned(),
                property: field,
            });
        } else {
            &forms
        };
        if !roster.contains(&value) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "external default reference has no own declaration",
            ));
        }
    }
    Ok(())
}

/// Root-only canonical intake. Embedded children require the later typed intake
/// component and are refused here; named owned Form/Template declarations remain
/// exact typed references, not fabricated canonical child objects.
pub fn decode_external_root(
    document: &XmlDocument,
    source: ProfileId,
    path: ObjectPath,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    validate_decode_profile(document, &source, &path)?;
    let binding = ExternalSourceBinding::inspect(document)?;
    let uris = resolve_namespaces(document.root())?;
    let sections = sections(document, binding.kind(), &uris)?;
    let version = root_version(document.root())?;
    let properties = property_map(sections.properties, binding.kind(), version, &uris)?;
    let name = element_text(properties["Name"])?.ok_or(MetadataDecodeError::Missing("Name"))?;
    let generated = generated_name(sections.internal, binding, &name, &uris)?;
    container_content(
        sections.children,
        binding.kind().external_kind(),
        "ChildObjects",
    )?;
    let mut declarations = BTreeSet::new();
    for child in elements(sections.children) {
        if !matches!(child.name().local(), "Form" | "Template") {
            return Err(MetadataDecodeError::UnevidencedProperty {
                owner: binding.kind().external_kind().to_owned(),
                property: "embedded ChildObjects",
            });
        }
        if !typed(child, child.name().local(), Some(MD_NAMESPACE), &uris)
            || !child.attributes().is_empty()
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "external named child declaration namespace/attributes",
            ));
        }
        let child_name =
            element_text(child)?.ok_or(MetadataDecodeError::Missing("external child name"))?;
        if child_name.is_empty() || child_name.contains('.') {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "external child declaration name",
            ));
        }
        if !declarations.insert((child.name().local(), child_name.to_lowercase())) {
            return Err(MetadataDecodeError::Duplicate("external child declaration"));
        }
    }
    let claims = ExternalRootClaims { document, binding };
    let generic = decode_external_metadata_envelope(document, source, path, &claims)?;
    if !generic.descendants().is_empty() {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "external root-only child inventory",
        ));
    }
    let mut parts = copy_object_parts(generic.root());
    for field in binding
        .kind()
        .properties()
        .iter()
        .copied()
        .filter(|x| !matches!(*x, "Name" | "Synonym"))
    {
        if properties.contains_key(field) {
            push_text(&mut parts, &properties, field)?;
        }
    }
    parts.properties.push(canonical_field(
        "ObjectTypeName",
        CanonicalValue::text(
            ibcmd_core::value::CanonicalText::new(&generated)
                .map_err(|x| MetadataDecodeError::Core(x.to_string()))?,
        ),
    )?);
    project_name_only_children(
        &mut parts,
        binding.kind().external_kind(),
        name,
        sections.children,
        &uris,
    )?;
    let root = CanonicalObject::new(parts).map_err(|x| MetadataDecodeError::Core(x.to_string()))?;
    validate_references(&root, binding.kind())?;
    generic.with_model(root, vec![])
}
fn property<'a>(root: &'a CanonicalObject, field: &str) -> Option<&'a CanonicalValue> {
    root.properties()
        .iter()
        .find(|x| x.name().as_str() == field)
        .map(|x| x.value())
}
fn text_value<'a>(
    root: &'a CanonicalObject,
    field: &'static str,
) -> Result<&'a str, MetadataEncodeError> {
    match property(root, field).map(CanonicalValue::kind) {
        Some(CanonicalValueKind::Text(x)) => Ok(x.as_str()),
        _ => Err(invalid_model(root.identity().path(), field)),
    }
}

/// Emit CURRENT root values through the existing XML tree/writer. No service or
/// archive is produced. Root IDs/children/generated identities remain bound;
/// unsupported edits fail before any externally visible publication.
pub fn encode_external_root(
    envelope: &MetadataEnvelope,
    target: &ProfileId,
) -> Result<Vec<u8>, MetadataEncodeError> {
    let path = envelope.root().identity().path();
    let binding = envelope
        .external_source_binding()
        .ok_or_else(|| invalid_model(path, "external source binding"))?;
    let target_version =
        profile_version(target).ok_or_else(|| MetadataEncodeError::UnsupportedProfile {
            object_path: path.clone(),
            profile: target.clone(),
        })?;
    // Cross-edition opaque/provenance migration is a separate owner contract.
    if target_version
        != root_version(envelope.source_document().root()).map_err(decode_to_encode)?
    {
        return Err(MetadataEncodeError::UnsupportedProfile {
            object_path: path.clone(),
            profile: target.clone(),
        });
    }
    let source = decode_external_root(
        envelope.source_document(),
        envelope.root().provenance().source_profile().clone(),
        path.clone(),
    )
    .map_err(decode_to_encode)?;
    if source.external_source_binding() != Some(binding)
        || source.descendants() != envelope.descendants()
    {
        return Err(invalid_model(path, "external binding/descendant topology"));
    }
    let mut same = copy_object_parts(source.root());
    same.properties = envelope.root().properties().to_vec();
    if CanonicalObject::new(same).map_err(|_| invalid_model(path, "external root"))?
        != *envelope.root()
    {
        return Err(invalid_model(
            path,
            "external immutable identity/assets/facets",
        ));
    }
    if source
        .root()
        .properties()
        .iter()
        .map(|x| x.name())
        .ne(envelope.root().properties().iter().map(|x| x.name()))
    {
        return Err(invalid_model(path, "external property presence/order"));
    }
    let document = envelope.source_document();
    let uris = resolve_namespaces(document.root()).map_err(decode_to_encode)?;
    let sections = sections(document, binding.kind(), &uris).map_err(decode_to_encode)?;
    let properties = sections.properties.with_children(
        sections
            .properties
            .children()
            .iter()
            .map(|node| {
                let XmlNode::Element(element) = node else {
                    return Ok(node.clone());
                };
                let field = binding
                    .kind()
                    .properties()
                    .iter()
                    .copied()
                    .find(|x| *x == element.name().local())
                    .ok_or_else(|| invalid_model(path, "external property"))?;
                let desired =
                    property(envelope.root(), field).ok_or_else(|| invalid_model(path, field))?;
                if property(source.root(), field) == Some(desired) {
                    return Ok(node.clone());
                }
                if field == "Synonym" {
                    patch_synonym(element, desired, path).map(XmlNode::Element)
                } else {
                    Ok(XmlNode::Element(element.with_children(vec![
                        XmlNode::text(text_value(envelope.root(), field)?),
                    ])))
                }
            })
            .collect::<Result<Vec<_>, MetadataEncodeError>>()?,
    );
    let internal = sections.internal.with_children(
        sections
            .internal
            .children()
            .iter()
            .map(|node| match node {
                XmlNode::Element(element)
                    if typed(element, "GeneratedType", Some(XR_NAMESPACE), &uris) =>
                {
                    if property(source.root(), "ObjectTypeName")
                        == property(envelope.root(), "ObjectTypeName")
                    {
                        Ok(node.clone())
                    } else {
                        set_unprefixed_attribute(
                            element,
                            "name",
                            text_value(envelope.root(), "ObjectTypeName")?,
                            path,
                        )
                        .map(XmlNode::Element)
                    }
                }
                _ => Ok(node.clone()),
            })
            .collect::<Result<Vec<_>, MetadataEncodeError>>()?,
    );
    let object = sections.object.with_children(
        sections
            .object
            .children()
            .iter()
            .map(|node| match node {
                XmlNode::Element(element) if std::ptr::eq(element, sections.properties) => {
                    XmlNode::Element(properties.clone())
                }
                XmlNode::Element(element) if std::ptr::eq(element, sections.internal) => {
                    XmlNode::Element(internal.clone())
                }
                _ => node.clone(),
            })
            .collect(),
    );
    let root = document.root().with_children(
        document
            .root()
            .children()
            .iter()
            .map(|node| match node {
                XmlNode::Element(element) if std::ptr::eq(element, sections.object) => {
                    XmlNode::Element(object.clone())
                }
                _ => node.clone(),
            })
            .collect(),
    );
    let prepared = document.with_root(root);
    let returned =
        decode_external_root(&prepared, target.clone(), path.clone()).map_err(decode_to_encode)?;
    if returned.root() != envelope.root()
        || returned.descendants() != envelope.descendants()
        || returned.external_source_binding() != Some(binding)
    {
        return Err(invalid_model(path, "external CURRENT forward validation"));
    }
    XmlWriter::to_vec(&prepared, LexicalPolicy::Preserve)
        .map_err(|x| MetadataEncodeError::Xml(x.to_string()))
}

fn patch_synonym(
    source: &XmlElement,
    desired: &CanonicalValue,
    path: &ObjectPath,
) -> Result<XmlElement, MetadataEncodeError> {
    let items = desired
        .as_sequence()
        .ok_or_else(|| invalid_model(path, "Synonym"))?;
    if elements(source).count() != items.len() {
        return Err(invalid_model(path, "Synonym authored item topology"));
    }
    let mut current = items.iter();
    let children = source
        .children()
        .iter()
        .map(|node| {
            let XmlNode::Element(item) = node else {
                return Ok(node.clone());
            };
            let fields = current
                .next()
                .expect("item count verified")
                .as_record()
                .ok_or_else(|| invalid_model(path, "Synonym item"))?;
            if fields.len() != 2
                || fields[0].name().as_str() != "lang"
                || fields[1].name().as_str() != "content"
            {
                return Err(invalid_model(path, "Synonym schema"));
            }
            let children = item
                .children()
                .iter()
                .map(|node| {
                    let XmlNode::Element(value) = node else {
                        return Ok(node.clone());
                    };
                    let field = fields
                        .iter()
                        .find(|x| x.name().as_str() == value.name().local())
                        .ok_or_else(|| invalid_model(path, "Synonym field"))?;
                    let CanonicalValueKind::Text(text) = field.value().kind() else {
                        return Err(invalid_model(path, "Synonym text"));
                    };
                    Ok(XmlNode::Element(
                        value.with_children(vec![XmlNode::text(text.as_str())]),
                    ))
                })
                .collect::<Result<Vec<_>, MetadataEncodeError>>()?;
            Ok(XmlNode::Element(item.with_children(children)))
        })
        .collect::<Result<Vec<_>, MetadataEncodeError>>()?;
    Ok(source.with_children(children))
}
