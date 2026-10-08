//! Closed source admission for the currently implemented empty EDS layout.
//! Namespace/identity/generated types are read by the existing canonical
//! envelope decoder. Unknown fields are refused, never lost by the CF encoder.

use std::collections::BTreeSet;

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::ObjectPath;
use ibcmd_core::source_policy::SourceOperationPolicy;
use ibcmd_schema::external_data_source as schema;

use super::common::{
    MD_NAMESPACE, MetadataDecodeError, MetadataEnvelope, ResolvedNamespaces, V8_NAMESPACE,
    XR_NAMESPACE, decode_source_metadata_envelope_with_policy, resolve_namespaces, typed,
};
use crate::{AttributeKind, XmlDocument, XmlElement, XmlNode};

fn invalid(message: &'static str) -> MetadataDecodeError {
    MetadataDecodeError::InvalidEnvelope(message)
}

fn attributes(element: &XmlElement, allowed: &[&str]) -> Result<(), MetadataDecodeError> {
    for attr in element.attributes() {
        if let AttributeKind::Ordinary(name) = attr.kind()
            && (name.prefix().is_some() || !allowed.contains(&name.local()))
        {
            return Err(invalid("unsupported ExternalDataSource attribute"));
        }
    }
    Ok(())
}

fn elements(element: &XmlElement) -> Result<Vec<&XmlElement>, MetadataDecodeError> {
    let mut children = Vec::new();
    for node in element.children() {
        match node {
            XmlNode::Element(child) => children.push(child),
            XmlNode::Text(text) if text.value().trim().is_empty() => {}
            _ => return Err(invalid("unsupported ExternalDataSource container content")),
        }
    }
    Ok(children)
}

fn scalar(element: &XmlElement) -> Result<String, MetadataDecodeError> {
    attributes(element, &[])?;
    let mut value = String::new();
    for node in element.children() {
        match node {
            XmlNode::Text(text) => value.push_str(text.value()),
            XmlNode::CData(text) => value.push_str(text.value()),
            _ => return Err(invalid("unsupported ExternalDataSource scalar content")),
        }
    }
    Ok(value)
}

fn one<'a>(
    children: &[&'a XmlElement],
    name: &str,
    namespace: &str,
    uris: &ResolvedNamespaces,
) -> Result<&'a XmlElement, MetadataDecodeError> {
    let mut matches = children
        .iter()
        .copied()
        .filter(|child| typed(child, name, Some(namespace), uris));
    let value = matches
        .next()
        .ok_or(invalid("missing ExternalDataSource required field"))?;
    if matches.next().is_some() {
        return Err(invalid("duplicate ExternalDataSource required field"));
    }
    Ok(value)
}

/// Validates the complete native source shape and returns the established
/// canonical model. Source policy has no new file/count/aggregate byte ceiling.
pub fn decode_empty_external_data_source(
    document: &XmlDocument,
    profile: ProfileId,
    path: ObjectPath,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    let uris = resolve_namespaces(document.root())?;
    let root = document.root();
    if !typed(root, "MetaDataObject", Some(MD_NAMESPACE), &uris) {
        return Err(invalid(
            "ExternalDataSource requires the metadata namespace",
        ));
    }
    attributes(root, &["version"])?;
    let version = root.attributes().iter().find_map(|attr| match attr.kind() {
        AttributeKind::Ordinary(name) if name.raw() == "version" => Some(attr.value()),
        _ => None,
    });
    if !matches!(version, Some("2.20" | "2.21")) {
        return Err(invalid("unsupported ExternalDataSource XML dialect"));
    }
    if profile.as_str() != format!("xml-{}", version.expect("checked dialect")) {
        return Err(invalid(
            "ExternalDataSource source profile differs from XML dialect",
        ));
    }
    let children = elements(root)?;
    if children.len() != 1 {
        return Err(invalid("ExternalDataSource requires one metadata object"));
    }
    let object = one(&children, schema::KIND, MD_NAMESPACE, &uris)?;
    attributes(object, &["uuid"])?;
    let parts = elements(object)?;
    if parts.len() != 3 {
        return Err(invalid("unsupported ExternalDataSource object fields"));
    }
    let info = one(&parts, "InternalInfo", MD_NAMESPACE, &uris)?;
    let props = one(&parts, "Properties", MD_NAMESPACE, &uris)?;
    let child_objects = one(&parts, "ChildObjects", MD_NAMESPACE, &uris)?;
    attributes(info, &[])?;
    attributes(props, &[])?;
    attributes(child_objects, &[])?;
    if !elements(child_objects)?.is_empty() {
        return Err(invalid(
            "nonempty ExternalDataSource requires a child codec",
        ));
    }
    let values = elements(props)?;
    if values.len() != 4 {
        return Err(invalid("unsupported ExternalDataSource properties"));
    }
    let name = scalar(one(&values, "Name", MD_NAMESPACE, &uris)?)?;
    if name.is_empty() {
        return Err(invalid("ExternalDataSource name is empty"));
    }
    scalar(one(&values, "Comment", MD_NAMESPACE, &uris)?)?;
    let mode = scalar(one(&values, "DataLockControlMode", MD_NAMESPACE, &uris)?)?;
    if schema::data_lock_code(&mode).is_none() {
        return Err(invalid("unknown ExternalDataSource DataLockControlMode"));
    }
    let synonym = one(&values, "Synonym", MD_NAMESPACE, &uris)?;
    attributes(synonym, &[])?;
    let mut languages = BTreeSet::new();
    for item in elements(synonym)? {
        if !typed(item, "item", Some(V8_NAMESPACE), &uris) {
            return Err(invalid("invalid ExternalDataSource synonym namespace"));
        }
        attributes(item, &[])?;
        let fields = elements(item)?;
        if fields.len() != 2 {
            return Err(invalid("invalid ExternalDataSource synonym fields"));
        }
        let language = scalar(one(&fields, "lang", V8_NAMESPACE, &uris)?)?;
        scalar(one(&fields, "content", V8_NAMESPACE, &uris)?)?;
        if language.is_empty() || !languages.insert(language) {
            return Err(invalid("invalid ExternalDataSource synonym language"));
        }
    }
    let generated = elements(info)?;
    if generated.len() != schema::GENERATED_CATEGORIES.len() {
        return Err(invalid("ExternalDataSource needs three generated types"));
    }
    let mut seen = BTreeSet::new();
    for generated in generated {
        if !typed(generated, "GeneratedType", Some(XR_NAMESPACE), &uris) {
            return Err(invalid("invalid ExternalDataSource generated namespace"));
        }
        attributes(generated, &["name", "category"])?;
        let attr = |key: &str| {
            generated
                .attributes()
                .iter()
                .find_map(|attr| match attr.kind() {
                    AttributeKind::Ordinary(name) if name.raw() == key => Some(attr.value()),
                    _ => None,
                })
        };
        let category = schema::GENERATED_CATEGORIES
            .iter()
            .copied()
            .find(|entry| Some(entry.category) == attr("category"))
            .ok_or(invalid("unknown ExternalDataSource generated category"))?;
        if !seen.insert(category.category)
            || attr("name") != Some(schema::generated_name(category, &name).as_str())
        {
            return Err(invalid(
                "ExternalDataSource generated identity/name differs",
            ));
        }
        let fields = elements(generated)?;
        if fields.len() != 2 {
            return Err(invalid("unsupported ExternalDataSource generated fields"));
        }
        scalar(one(&fields, "TypeId", XR_NAMESPACE, &uris)?)?;
        scalar(one(&fields, "ValueId", XR_NAMESPACE, &uris)?)?;
    }
    let envelope = decode_source_metadata_envelope_with_policy(
        document,
        profile,
        path,
        SourceOperationPolicy::Source,
    )?;
    if envelope.root().generated_types().len() != schema::GENERATED_CATEGORIES.len() {
        return Err(invalid(
            "ExternalDataSource generated identities must be concrete",
        ));
    }
    envelope
        .configuration()
        .map_err(|_| invalid("ExternalDataSource generated identity graph differs"))?;
    let mut identities = BTreeSet::from([envelope.root().identity().uuid()]);
    if envelope
        .root()
        .identity()
        .uuid()
        .as_bytes()
        .iter()
        .all(|byte| *byte == 0)
    {
        return Err(invalid("nil ExternalDataSource object identity"));
    }
    for generated in envelope.root().generated_types() {
        let value_id = generated
            .value_id()
            .ok_or(invalid("missing ExternalDataSource generated ValueId"))?;
        if generated.uuid().as_bytes().iter().all(|byte| *byte == 0)
            || value_id.as_bytes().iter().all(|byte| *byte == 0)
        {
            return Err(invalid("nil ExternalDataSource generated identity"));
        }
        if !identities.insert(generated.uuid()) || !identities.insert(value_id) {
            return Err(invalid("duplicate ExternalDataSource generated identity"));
        }
    }
    Ok(envelope)
}
