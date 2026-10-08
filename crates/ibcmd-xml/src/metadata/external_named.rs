//! Source-only named owners under a currently bound external root.
use super::common::{
    MD_NAMESPACE, MetadataDecodeError, MetadataEnvelope, ResolvedNamespaces, V8_NAMESPACE,
    element_text, resolve_namespaces, synonym_value, typed,
};
use super::external_objects::{attributes, container_content, elements};
use super::language::{
    copy_object_parts, decode_to_encode, invalid_model, root_version, validate_decode_profile,
};
use super::{ExternalSourceBinding, MetadataEncodeError, decode_external_root};
use crate::{AttributeKind, LexicalPolicy, XmlDocument, XmlElement, XmlNode, XmlWriter};
use ibcmd_core::{
    artifact::ProfileId,
    diagnostic::{ObjectPath, PathSegment, PropertyPath},
    identity::{LogicalIdentity, ObjectUuid},
    model::{CanonicalObject, CanonicalObjectParts, MetadataKind},
    provenance::{CanonicalAnchor, SourceProvenance},
    source_policy::SourceOperationPolicy,
    value::{CanonicalField, CanonicalText, CanonicalValue, CanonicalValueKind, EnumToken},
};
use ibcmd_schema::external_named::{
    ExternalNamedKind, FORM_TYPES, INTERFACE_COMPATIBILITY_MODES, TEMPLATE_TYPES, USE_PURPOSES,
};
use std::collections::BTreeSet;
const APP: &str = "http://v8.1c.ru/8.2/managed-application/core";
const XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// Only a complete, same-document root admission can supply this authority.
/// No public constructor accepts an arbitrary owner UUID or filename kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalNamedOwnerContext {
    binding: ExternalSourceBinding,
    root_path: ObjectPath,
    declarations: Vec<(ExternalNamedKind, String)>,
    profile: ProfileId,
    generated_ids: BTreeSet<ObjectUuid>,
}
impl ExternalNamedOwnerContext {
    pub fn from_root(root: &MetadataEnvelope) -> Result<Self, MetadataDecodeError> {
        let decoded = decode_external_root(
            root.source_document(),
            root.root().provenance().source_profile().clone(),
            root.root().identity().path().clone(),
        )?;
        if decoded.root() != root.root()
            || !root.descendants().is_empty()
            || decoded.external_source_binding() != root.external_source_binding()
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "named context is not the current admitted root",
            ));
        }
        let root_name = decoded
            .root()
            .properties()
            .iter()
            .find(|f| f.name().as_str() == "Name")
            .and_then(|f| match f.value().kind() {
                CanonicalValueKind::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .ok_or(MetadataDecodeError::Missing("Name"))?;
        let mut declarations = Vec::new();
        for kind in [ExternalNamedKind::Form, ExternalNamedKind::Template] {
            if let Some(field) = decoded
                .root()
                .properties()
                .iter()
                .find(|f| f.name().as_str() == kind.declaration())
            {
                for value in
                    field
                        .value()
                        .as_sequence()
                        .ok_or(MetadataDecodeError::InvalidEnvelope(
                            "named declaration shape",
                        ))?
                {
                    let CanonicalValueKind::Reference(reference) = value.kind() else {
                        return Err(MetadataDecodeError::InvalidEnvelope(
                            "named declaration type",
                        ));
                    };
                    let prefix = format!(
                        "{}.{}.{}.",
                        decoded
                            .external_source_binding()
                            .ok_or(MetadataDecodeError::Missing("external binding"))?
                            .kind()
                            .external_kind(),
                        root_name,
                        kind.family()
                    );
                    let name = reference
                        .target()
                        .strip_prefix(&prefix)
                        .filter(|n| !n.is_empty() && !n.contains('.'))
                        .ok_or(MetadataDecodeError::InvalidEnvelope(
                            "named declaration target",
                        ))?;
                    if reference.kind() != "metadata" {
                        return Err(MetadataDecodeError::InvalidEnvelope(
                            "named declaration family",
                        ));
                    }
                    declarations.push((kind, name.to_owned()));
                }
            }
        }
        let generated_ids = decoded
            .root()
            .generated_types()
            .iter()
            .flat_map(|g| std::iter::once(g.uuid()).chain(g.value_id()))
            .collect();
        Ok(Self {
            profile: decoded.root().provenance().source_profile().clone(),
            generated_ids,
            declarations,
            binding: decoded
                .external_source_binding()
                .ok_or(MetadataDecodeError::Missing("external binding"))?,
            root_path: root.root().identity().path().clone(),
        })
    }
    pub fn declared_names(&self, kind: ExternalNamedKind) -> impl Iterator<Item = &str> {
        self.declarations
            .iter()
            .filter(move |(k, _)| *k == kind)
            .map(|(_, n)| n.as_str())
    }
    pub fn owner(&self) -> ObjectUuid {
        self.binding
            .identity()
            .contained
            .expect("admitted contained identity")
            .object_id
    }
    pub const fn binding(&self) -> ExternalSourceBinding {
        self.binding
    }
    pub fn root_path(&self) -> &ObjectPath {
        &self.root_path
    }
}
fn core(error: impl std::fmt::Display) -> MetadataDecodeError {
    MetadataDecodeError::Core(error.to_string())
}
fn text(value: &str) -> Result<CanonicalValue, MetadataDecodeError> {
    Ok(CanonicalValue::text(
        CanonicalText::new_with_policy(value, SourceOperationPolicy::Source).map_err(core)?,
    ))
}
fn enum_value(value: &str, domain: &[(&str, i64)]) -> Result<CanonicalValue, MetadataDecodeError> {
    if !domain.iter().any(|(name, _)| *name == value) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "unknown named-owner enum value",
        ));
    }
    Ok(CanonicalValue::enum_token(
        EnumToken::new(value).map_err(core)?,
    ))
}
fn localized(
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<CanonicalValue, MetadataDecodeError> {
    container_content(element, "named owner", "localized property")?;
    for item in elements(element) {
        if !typed(item, "item", Some(V8_NAMESPACE), uris) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "localized item namespace",
            ));
        }
        attributes(item, &[])?;
        container_content(item, "named owner", "localized item")?;
        let mut seen = BTreeSet::new();
        for field in elements(item) {
            if !matches!(field.name().local(), "lang" | "content")
                || !typed(field, field.name().local(), Some(V8_NAMESPACE), uris)
            {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "localized field namespace/name",
                ));
            }
            attributes(field, &[])?;
            if !seen.insert(field.name().local()) {
                return Err(MetadataDecodeError::Duplicate("localized field"));
            }
            element_text(field)?.ok_or(MetadataDecodeError::InvalidEnvelope(
                "localized nested content",
            ))?;
        }
        if seen.len() != 2 {
            return Err(MetadataDecodeError::Missing("localized lang/content"));
        }
    }
    synonym_value(element, uris, SourceOperationPolicy::Source)
}
fn purposes(
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<CanonicalValue, MetadataDecodeError> {
    container_content(element, "Form", "UsePurposes")?;
    let mut seen = BTreeSet::new();
    let mut values = Vec::new();
    for value in elements(element) {
        if !typed(value, "Value", Some(V8_NAMESPACE), uris) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "UsePurposes value namespace",
            ));
        }
        let mut type_seen = false;
        for attr in value.attributes() {
            if let AttributeKind::Ordinary(name) = attr.kind() {
                if name.local() != "type"
                    || name.prefix().and_then(|p| uris.qname_uri(value, p)) != Some(XSI)
                    || type_seen
                {
                    return Err(MetadataDecodeError::InvalidEnvelope(
                        "UsePurposes type attribute",
                    ));
                }
                let (prefix, local) = attr.value().split_once(':').unwrap_or(("", attr.value()));
                if local != "ApplicationUsePurpose" || uris.qname_uri(value, prefix) != Some(APP) {
                    return Err(MetadataDecodeError::InvalidEnvelope(
                        "UsePurposes type QName",
                    ));
                }
                type_seen = true;
            }
        }
        if !type_seen {
            return Err(MetadataDecodeError::Missing("UsePurposes type"));
        }
        let value = element_text(value)?.ok_or(MetadataDecodeError::InvalidEnvelope(
            "UsePurposes nested value",
        ))?;
        if !seen.insert(value.clone()) {
            return Err(MetadataDecodeError::Duplicate("UsePurposes value"));
        }
        values.push(enum_value(&value, USE_PURPOSES)?);
    }
    CanonicalValue::sequence_with_policy(values, SourceOperationPolicy::Source).map_err(core)
}
fn sections<'a>(
    document: &'a XmlDocument,
    kind: ExternalNamedKind,
    uris: &ResolvedNamespaces,
) -> Result<(&'a XmlElement, &'a XmlElement), MetadataDecodeError> {
    let root = document.root();
    if !typed(root, "MetaDataObject", Some(MD_NAMESPACE), uris) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "named envelope namespace",
        ));
    }
    attributes(root, &["version"])?;
    container_content(root, kind.family(), "MetaDataObject")?;
    let mut objects = elements(root);
    let object = objects
        .next()
        .ok_or(MetadataDecodeError::Missing("named object"))?;
    if objects.next().is_some() || !typed(object, kind.family(), Some(MD_NAMESPACE), uris) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "named object family/count/namespace",
        ));
    }
    attributes(object, &["uuid"])?;
    container_content(object, kind.family(), "object sections")?;
    let mut properties = None;
    let mut children = false;
    for section in elements(object) {
        attributes(section, &[])?;
        if typed(section, "Properties", Some(MD_NAMESPACE), uris) && properties.is_none() {
            properties = Some(section);
        } else if typed(section, "ChildObjects", Some(MD_NAMESPACE), uris) && !children {
            children = true;
            container_content(section, kind.family(), "ChildObjects")?;
            if elements(section).next().is_some() {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "named embedded metadata remains unsupported",
                ));
            }
        } else {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "unknown/duplicate named section",
            ));
        }
    }
    Ok((
        object,
        properties.ok_or(MetadataDecodeError::Missing("Properties"))?,
    ))
}
/// Complete canonical source properties, including authored absence and order.
/// This is deliberately separate from the ordinary six-property Form compiler.
pub fn decode_external_named_owner(
    document: &XmlDocument,
    profile: ProfileId,
    path: ObjectPath,
    context: &ExternalNamedOwnerContext,
    kind: ExternalNamedKind,
    declared_name: &str,
) -> Result<CanonicalObject, MetadataDecodeError> {
    if !context
        .declarations
        .iter()
        .any(|(k, n)| *k == kind && n == declared_name)
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "named owner is not declared by the bound root",
        ));
    }
    let mut expected_path = context.root_path.clone();
    for segment in [kind.family(), declared_name] {
        expected_path
            .push_with_policy(
                PathSegment::name_with_policy(segment, SourceOperationPolicy::Source)
                    .map_err(core)?,
                SourceOperationPolicy::Source,
            )
            .map_err(core)?;
    }
    if expected_path != path {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "named owner CURRENT path/context mismatch",
        ));
    }
    if profile != context.profile {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "named root/source profile mismatch",
        ));
    }
    if document
        .before_root()
        .iter()
        .chain(document.after_root())
        .any(|n| matches!(n, XmlNode::DocType(_)))
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "named document doctype",
        ));
    }
    validate_decode_profile(document, &profile, &path)?;
    let version = root_version(document.root())?;
    let uris = resolve_namespaces(document.root())?;
    let (object, properties) = sections(document, kind, &uris)?;
    let uuid = ObjectUuid::parse(
        object
            .attributes()
            .iter()
            .find_map(|attribute| match attribute.kind() {
                AttributeKind::Ordinary(name)
                    if name.prefix().is_none() && name.local() == "uuid" =>
                {
                    Some(attribute.value())
                }
                _ => None,
            })
            .ok_or(MetadataDecodeError::Missing("uuid"))?,
    )
    .map_err(core)?;
    if uuid.as_bytes().iter().all(|b| *b == 0)
        || uuid == context.owner()
        || uuid == context.binding.identity().main_uuid
        || context.generated_ids.contains(&uuid)
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "named owner nil/root identity collision",
        ));
    }
    let provenance = SourceProvenance::new(
        profile,
        CanonicalAnchor::new(path.clone(), PropertyPath::root()),
    );
    let mut parts = CanonicalObjectParts::new(
        LogicalIdentity::new(uuid, path),
        MetadataKind::new(kind.family()).map_err(core)?,
        provenance,
    );
    parts.owner = Some(context.owner());
    container_content(properties, kind.family(), "Properties")?;
    let mut seen = BTreeSet::new();
    for property in elements(properties) {
        let name = property.name().local();
        if !kind.property_available(name, version)
            || !typed(property, name, Some(MD_NAMESPACE), &uris)
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "named property name/namespace/edition",
            ));
        }
        attributes(property, &[])?;
        if !seen.insert(name) {
            return Err(MetadataDecodeError::Duplicate("named property"));
        }
        let value = match name {
            "Synonym" | "ExtendedPresentation" => localized(property, &uris)?,
            "UsePurposes" => purposes(property, &uris)?,
            _ => {
                // Comments and processing instructions are retained lexical
                // nodes, not scalar data or nested metadata.
                let scalar = property.with_children(
                    property
                        .children()
                        .iter()
                        .filter(|node| {
                            !matches!(
                                node,
                                XmlNode::Comment(_) | XmlNode::ProcessingInstruction(_)
                            )
                        })
                        .cloned()
                        .collect(),
                );
                let value = element_text(&scalar)?.ok_or(MetadataDecodeError::InvalidEnvelope(
                    "named scalar nested content",
                ))?;
                match name {
                    "Name" => {
                        if value != declared_name || value.is_empty() {
                            return Err(MetadataDecodeError::InvalidEnvelope(
                                "named declaration/metadata Name mismatch",
                            ));
                        }
                        text(&value)?
                    }
                    "FormType" => enum_value(&value, FORM_TYPES)?,
                    "TemplateType" => enum_value(&value, TEMPLATE_TYPES)?,
                    "UseInInterfaceCompatibilityMode" => {
                        enum_value(&value, INTERFACE_COMPATIBILITY_MODES)?
                    }
                    "IncludeHelpInContents" => match value.as_str() {
                        "true" => CanonicalValue::boolean(true),
                        "false" => CanonicalValue::boolean(false),
                        _ => return Err(MetadataDecodeError::InvalidEnvelope("named boolean")),
                    },
                    _ => text(&value)?,
                }
            }
        };
        parts.properties.push(
            CanonicalField::named_with_policy(name, value, SourceOperationPolicy::Source)
                .map_err(core)?,
        );
    }
    for required in match kind {
        ExternalNamedKind::Form => {
            &["Name", "FormType", "IncludeHelpInContents", "UsePurposes"][..]
        }
        ExternalNamedKind::Template => &["Name", "TemplateType"][..],
    } {
        if !seen.contains(required) {
            return Err(MetadataDecodeError::Missing(required));
        }
    }
    CanonicalObject::new_with_policy(parts, SourceOperationPolicy::Source).map_err(core)
}
/// Edits the current typed values in the retained envelope and independently
/// re-decodes the result. All identity, presence, order and topology are fixed.
pub fn encode_external_named_owner(
    document: &XmlDocument,
    current: &CanonicalObject,
    profile: &ProfileId,
    context: &ExternalNamedOwnerContext,
    kind: ExternalNamedKind,
    declared_name: &str,
) -> Result<Vec<u8>, MetadataEncodeError> {
    let path = current.identity().path();
    let original = decode_external_named_owner(
        document,
        profile.clone(),
        path.clone(),
        context,
        kind,
        declared_name,
    )
    .map_err(decode_to_encode)?;
    if current.provenance().source_profile() != profile
        || original.properties().len() != current.properties().len()
        || original
            .properties()
            .iter()
            .zip(current.properties())
            .any(|(a, b)| a.name() != b.name())
    {
        return Err(invalid_model(
            path,
            "named CURRENT property presence/order/profile",
        ));
    }
    let mut immutable = copy_object_parts(&original);
    immutable.properties = current.properties().to_vec();
    if CanonicalObject::new_with_policy(immutable, SourceOperationPolicy::Source)
        .map_err(|_| invalid_model(path, "named current properties"))?
        != *current
    {
        return Err(invalid_model(
            path,
            "named CURRENT identity/owner/assets/topology",
        ));
    }
    let uris = resolve_namespaces(document.root()).map_err(decode_to_encode)?;
    let (object, properties) = sections(document, kind, &uris).map_err(decode_to_encode)?;
    let mut fields = current.properties().iter();
    let patched = properties.with_children(
        properties
            .children()
            .iter()
            .map(|node| {
                let XmlNode::Element(property) = node else {
                    return Ok(node.clone());
                };
                let desired = fields.next().expect("property count checked");
                let old = original
                    .properties()
                    .iter()
                    .find(|f| f.name() == desired.name())
                    .expect("presence checked");
                if old.value() == desired.value() {
                    return Ok(node.clone());
                }
                let value = match desired.name().as_str() {
                    "Synonym" | "ExtendedPresentation" => {
                        patch_localized(property, desired.value(), path)?
                    }
                    "UsePurposes" => {
                        let values = desired
                            .value()
                            .as_sequence()
                            .ok_or_else(|| invalid_model(path, "UsePurposes"))?;
                        if elements(property).count() != values.len() {
                            return Err(invalid_model(path, "UsePurposes topology"));
                        }
                        let mut values = values.iter();
                        property.with_children(
                            property
                                .children()
                                .iter()
                                .map(|n| {
                                    let XmlNode::Element(e) = n else {
                                        return Ok(n.clone());
                                    };
                                    let CanonicalValueKind::EnumToken(token) =
                                        values.next().expect("count checked").kind()
                                    else {
                                        return Err(invalid_model(path, "UsePurposes enum"));
                                    };
                                    Ok(XmlNode::Element(patch_scalar(e, token.as_str())))
                                })
                                .collect::<Result<Vec<_>, MetadataEncodeError>>()?,
                        )
                    }
                    _ => {
                        let value = match desired.value().kind() {
                            CanonicalValueKind::Text(t) => t.as_str().to_owned(),
                            CanonicalValueKind::EnumToken(t) => t.as_str().to_owned(),
                            CanonicalValueKind::Bool(b) => b.to_string(),
                            _ => return Err(invalid_model(path, "named scalar kind")),
                        };
                        patch_scalar(property, &value)
                    }
                };
                Ok(XmlNode::Element(value))
            })
            .collect::<Result<Vec<_>, MetadataEncodeError>>()?,
    );
    let object = object.with_children(
        object
            .children()
            .iter()
            .map(|n| match n {
                XmlNode::Element(e) if std::ptr::eq(e, properties) => {
                    XmlNode::Element(patched.clone())
                }
                _ => n.clone(),
            })
            .collect(),
    );
    let prepared = document.with_root(
        document.root().with_children(
            document
                .root()
                .children()
                .iter()
                .map(|n| match n {
                    XmlNode::Element(e) if typed(e, kind.family(), Some(MD_NAMESPACE), &uris) => {
                        XmlNode::Element(object.clone())
                    }
                    _ => n.clone(),
                })
                .collect(),
        ),
    );
    let returned = decode_external_named_owner(
        &prepared,
        profile.clone(),
        path.clone(),
        context,
        kind,
        declared_name,
    )
    .map_err(decode_to_encode)?;
    if returned != *current {
        return Err(invalid_model(path, "named CURRENT forward equality"));
    }
    XmlWriter::to_vec(&prepared, LexicalPolicy::Preserve)
        .map_err(|e| MetadataEncodeError::Xml(e.to_string()))
}

/// Namespace admission is not a semantic body decoder or native writer.
pub fn validate_external_owned_body(
    document: &XmlDocument,
    source_kind: &str,
    profile: &ProfileId,
    path: &ObjectPath,
) -> Result<(), MetadataDecodeError> {
    let (name, uri) = ibcmd_schema::external_named::body_xml_envelope(source_kind).ok_or(
        MetadataDecodeError::InvalidEnvelope("unsupported body envelope"),
    )?;
    let uris = resolve_namespaces(document.root())?;
    if !typed(document.root(), name, Some(uri), &uris) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "owned body namespace/kind",
        ));
    }
    if source_kind == "Form" {
        validate_decode_profile(document, profile, path)?;
    }
    Ok(())
}

// Semantic edits preserve comments/PIs and do not normalize the untouched XML.
fn patch_scalar(source: &XmlElement, desired: &str) -> XmlElement {
    let mut inserted = false;
    let mut children = Vec::new();
    for node in source.children() {
        if matches!(node, XmlNode::Text(_) | XmlNode::CData(_)) {
            if !inserted {
                children.push(XmlNode::text(desired));
                inserted = true;
            }
        } else {
            children.push(node.clone());
        }
    }
    if !inserted {
        children.push(XmlNode::text(desired));
    }
    source.with_children(children)
}
fn patch_localized(
    source: &XmlElement,
    desired: &CanonicalValue,
    path: &ObjectPath,
) -> Result<XmlElement, MetadataEncodeError> {
    let values = desired
        .as_sequence()
        .ok_or_else(|| invalid_model(path, "localized sequence"))?;
    if elements(source).count() != values.len() {
        return Err(invalid_model(path, "localized authored item topology"));
    }
    let mut values = values.iter();
    let children = source
        .children()
        .iter()
        .map(|n| {
            let XmlNode::Element(item) = n else {
                return Ok(n.clone());
            };
            let fields = values
                .next()
                .expect("count checked")
                .as_record()
                .ok_or_else(|| invalid_model(path, "localized item"))?;
            if fields.len() != 2
                || fields[0].name().as_str() != "lang"
                || fields[1].name().as_str() != "content"
            {
                return Err(invalid_model(path, "localized fields"));
            }
            let children = item
                .children()
                .iter()
                .map(|n| {
                    let XmlNode::Element(e) = n else {
                        return Ok(n.clone());
                    };
                    let field = fields
                        .iter()
                        .find(|f| f.name().as_str() == e.name().local())
                        .ok_or_else(|| invalid_model(path, "localized field"))?;
                    let CanonicalValueKind::Text(t) = field.value().kind() else {
                        return Err(invalid_model(path, "localized text"));
                    };
                    Ok(XmlNode::Element(patch_scalar(e, t.as_str())))
                })
                .collect::<Result<Vec<_>, MetadataEncodeError>>()?;
            Ok(XmlNode::Element(item.with_children(children)))
        })
        .collect::<Result<Vec<_>, MetadataEncodeError>>()?;
    Ok(source.with_children(children))
}
