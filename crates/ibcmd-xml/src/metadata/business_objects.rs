//! Strict canonical XCF codecs for `Catalog` and `Document` metadata.
//!
//! These families mix UUID-bearing embedded objects with name-only references
//! to separately stored forms and templates.  The generic envelope decoder is
//! used for identity, ownership, generated types, and lossless facets; this
//! module adds the complete BOOT-005 semantic projection supported by the
//! native compiler and rejects every unevidenced shape.

use std::collections::{BTreeMap, BTreeSet};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::ObjectPath;
use ibcmd_core::family::FamilyId;
use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::model::{CanonicalObject, CanonicalObjectParts};
use ibcmd_core::value::{CanonicalInteger, CanonicalValue, EnumToken, UnresolvedReference};

use super::common::decode_metadata_envelope_with_child_references;
use super::common::{
    MD_NAMESPACE, MetadataDecodeError, MetadataEnvelope, ResolvedNamespaces, V8_NAMESPACE,
    element_text, resolve_namespaces, typed, uri_of,
};
use super::language::{
    canonical_field, canonical_text, copy_object_parts, decode_to_encode, invalid_model,
    profile_version, root_version, set_unprefixed_attribute, validate_decode_profile,
};
use super::registry::{
    MetadataEncodeError, MetadataFamilyCodec, MetadataRegistry, MetadataRegistryError,
};
use crate::{Attribute, AttributeKind, LexicalPolicy, XmlDocument, XmlElement, XmlNode, XmlWriter};

const CATALOG: &str = "Catalog";
const DOCUMENT: &str = "Document";
const PALETTE_NAMESPACE: &str = "http://v8.1c.ru/8.1/data/ui/colors/palette";
const PALETTE_PREFIX: &str = "pal";

const CATALOG_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "Hierarchical",
    "HierarchyType",
    "LimitLevelCount",
    "LevelCount",
    "FoldersOnTop",
    "UseStandardCommands",
    "Owners",
    "SubordinationUse",
    "CodeLength",
    "DescriptionLength",
    "CodeType",
    "CodeAllowedLength",
    "CodeSeries",
    "CheckUnique",
    "Autonumbering",
    "DefaultPresentation",
    "StandardAttributes",
    "Characteristics",
    "PredefinedDataUpdate",
    "EditType",
    "QuickChoice",
    "ChoiceMode",
    "InputByString",
    "SearchStringModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "DefaultObjectForm",
    "DefaultFolderForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "DefaultFolderChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryFolderForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "AuxiliaryFolderChoiceForm",
    "IncludeHelpInContents",
    "BasedOn",
    "DataLockFields",
    "DataLockControlMode",
    "FullTextSearch",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "CreateOnInput",
    "ChoiceHistoryOnInput",
    "DataHistory",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
];

const DOCUMENT_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "Numerator",
    "NumberType",
    "NumberLength",
    "NumberAllowedLength",
    "NumberPeriodicity",
    "CheckUnique",
    "Autonumbering",
    "StandardAttributes",
    "Characteristics",
    "BasedOn",
    "InputByString",
    "CreateOnInput",
    "SearchStringModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "DefaultObjectForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "Posting",
    "RealTimePosting",
    "RegisterRecordsDeletion",
    "RegisterRecordsWritingOnPost",
    "SequenceFilling",
    "RegisterRecords",
    "PostInPrivilegedMode",
    "UnpostInPrivilegedMode",
    "IncludeHelpInContents",
    "DataLockFields",
    "DataLockControlMode",
    "FullTextSearch",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "ChoiceHistoryOnInput",
    "DataHistory",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
];

const ATTRIBUTE_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "Type",
    "PasswordMode",
    "Format",
    "EditFormat",
    "ToolTip",
    "MarkNegatives",
    "Mask",
    "MultiLine",
    "ExtendedEdit",
    "MinValue",
    "MaxValue",
    "FillFromFillingValue",
    "FillValue",
    "FillChecking",
    "ChoiceFoldersAndItems",
    "ChoiceParameterLinks",
    "ChoiceParameters",
    "QuickChoice",
    "CreateOnInput",
    "ChoiceForm",
    "LinkByType",
    "ChoiceHistoryOnInput",
    "Use",
    "Indexing",
    "FullTextSearch",
    "DataHistory",
];

const DOCUMENT_ATTRIBUTE_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "Type",
    "PasswordMode",
    "Format",
    "EditFormat",
    "ToolTip",
    "MarkNegatives",
    "Mask",
    "MultiLine",
    "ExtendedEdit",
    "MinValue",
    "MaxValue",
    "FillFromFillingValue",
    "FillValue",
    "FillChecking",
    "ChoiceFoldersAndItems",
    "ChoiceParameterLinks",
    "ChoiceParameters",
    "QuickChoice",
    "CreateOnInput",
    "ChoiceForm",
    "LinkByType",
    "ChoiceHistoryOnInput",
    "Indexing",
    "FullTextSearch",
    "DataHistory",
];

const NESTED_ATTRIBUTE_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "Type",
    "PasswordMode",
    "Format",
    "EditFormat",
    "ToolTip",
    "MarkNegatives",
    "Mask",
    "MultiLine",
    "ExtendedEdit",
    "MinValue",
    "MaxValue",
    "FillChecking",
    "ChoiceFoldersAndItems",
    "ChoiceParameterLinks",
    "ChoiceParameters",
    "QuickChoice",
    "CreateOnInput",
    "ChoiceForm",
    "LinkByType",
    "ChoiceHistoryOnInput",
    "Indexing",
    "FullTextSearch",
    "DataHistory",
];

const TABULAR_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "ToolTip",
    "FillChecking",
    "StandardAttributes",
    "Use",
    "LineNumberLength",
];

const DOCUMENT_TABULAR_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "ToolTip",
    "FillChecking",
    "StandardAttributes",
    "LineNumberLength",
];

const COMMAND_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "Group",
    "CommandParameterType",
    "ParameterUseMode",
    "ModifiesData",
    "Representation",
    "ToolTip",
    "Picture",
    "Shortcut",
    "OnMainServerUnavalableBehavior",
];

pub fn register_catalog_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(BusinessObjectCodec::new(CATALOG)))
}

pub fn register_document_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(BusinessObjectCodec::new(DOCUMENT)))
}

struct BusinessObjectCodec {
    family: FamilyId,
}

impl BusinessObjectCodec {
    fn new(family: &str) -> Self {
        Self {
            family: FamilyId::parse(family).expect("business object family literal is valid"),
        }
    }
}

impl MetadataFamilyCodec for BusinessObjectCodec {
    fn family_id(&self) -> &FamilyId {
        &self.family
    }

    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        decode_business_object(document, source, path, self.family.as_str())
    }

    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        encode_business_object(envelope, target, self.family.as_str())
    }
}

fn decode_business_object(
    document: &XmlDocument,
    source: ProfileId,
    path: ObjectPath,
    family: &str,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    validate_decode_profile(document, &source, &path)?;
    let generic = decode_metadata_envelope_with_child_references(
        document,
        source,
        path,
        &["Form", "Template"],
    )?;
    if generic.root().kind().as_str() != family || !matches!(family, CATALOG | DOCUMENT) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "business object codec family differs from XML",
        ));
    }

    let uris = resolve_namespaces(document.root())?;
    let expected = uri_of(document.root(), &uris);
    if expected != Some(MD_NAMESPACE) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "business object requires the MDClasses namespace",
        ));
    }
    let object = only_element_child(document.root(), "metadata object")?;
    if !typed(object, family, expected, &uris) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "business object element is not exact",
        ));
    }
    let sections = exact_object_sections(object, family, &uris)?;
    let properties = sections.properties;
    let expected_properties = if family == CATALOG {
        CATALOG_PROPERTIES
    } else {
        DOCUMENT_PROPERTIES
    };
    let property_map = exact_property_map(properties, expected_properties, &uris)?;

    let mut root_parts = copy_object_parts(generic.root());
    project_root_properties(&mut root_parts, family, &property_map, &uris)?;
    project_name_only_children(
        &mut root_parts,
        family,
        text_field(&property_map, "Name")?,
        sections.children,
        &uris,
    )?;
    let root = CanonicalObject::new(root_parts)
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))?;

    let element_by_uuid = collect_embedded_elements(sections.children, &uris)?;
    if element_by_uuid.len() != generic.descendants().len() {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "business object descendant inventory is not exact",
        ));
    }
    let mut descendants = Vec::with_capacity(generic.descendants().len());
    for descendant in generic.descendants() {
        let element = element_by_uuid.get(&descendant.identity().uuid()).ok_or(
            MetadataDecodeError::InvalidEnvelope("business object descendant has no XML element"),
        )?;
        let mut parts = copy_object_parts(descendant);
        match descendant.kind().as_str() {
            "Attribute" => {
                project_attribute(&mut parts, element, family, root.identity().uuid(), &uris)?
            }
            "TabularSection" => project_tabular_section(&mut parts, element, family, &uris)?,
            "Command" if descendant.owner() == Some(root.identity().uuid()) => {
                project_command(&mut parts, element, &uris)?
            }
            _ => {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "business object contains an unsupported embedded child",
                ));
            }
        }
        descendants.push(
            CanonicalObject::new(parts)
                .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
        );
    }
    MetadataEnvelope::from_parts(root, descendants, document.clone())
}

pub(super) struct ObjectSections<'a> {
    pub(super) properties: &'a XmlElement,
    pub(super) children: &'a XmlElement,
}

pub(super) fn exact_object_sections<'a>(
    object: &'a XmlElement,
    family: &str,
    uris: &ResolvedNamespaces,
) -> Result<ObjectSections<'a>, MetadataDecodeError> {
    let expected = Some(MD_NAMESPACE);
    let mut properties = None;
    let mut children = None;
    for node in object.children() {
        let XmlNode::Element(element) = node else {
            continue;
        };
        if typed(element, "InternalInfo", expected, uris) {
            continue;
        }
        if typed(element, "Properties", expected, uris) && properties.replace(element).is_none() {
            continue;
        }
        if typed(element, "ChildObjects", expected, uris) && children.replace(element).is_none() {
            continue;
        }
        return Err(MetadataDecodeError::InvalidEnvelope(if family == CATALOG {
            "Catalog contains an unknown section"
        } else {
            "Document contains an unknown section"
        }));
    }
    Ok(ObjectSections {
        properties: properties.ok_or(MetadataDecodeError::Missing("Properties"))?,
        children: children.ok_or(MetadataDecodeError::Missing("ChildObjects"))?,
    })
}

pub(super) fn exact_property_map<'a>(
    properties: &'a XmlElement,
    expected_names: &[&'static str],
    uris: &ResolvedNamespaces,
) -> Result<BTreeMap<&'static str, &'a XmlElement>, MetadataDecodeError> {
    let elements = properties
        .children()
        .iter()
        .filter_map(|node| match node {
            XmlNode::Element(element) => Some(element),
            _ => None,
        })
        .collect::<Vec<_>>();
    if elements.len() != expected_names.len() {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "business object property inventory is not exact",
        ));
    }
    let mut result = BTreeMap::new();
    for (element, expected) in elements.into_iter().zip(expected_names) {
        if !typed(element, expected, Some(MD_NAMESPACE), uris) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "business object property order is not exact",
            ));
        }
        result.insert(*expected, element);
    }
    Ok(result)
}

fn project_root_properties(
    parts: &mut CanonicalObjectParts,
    family: &str,
    properties: &BTreeMap<&str, &XmlElement>,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    push_text(parts, properties, "Comment")?;
    if family == CATALOG {
        for name in [
            "Hierarchical",
            "LimitLevelCount",
            "FoldersOnTop",
            "UseStandardCommands",
            "CheckUnique",
            "Autonumbering",
            "QuickChoice",
            "IncludeHelpInContents",
            "UpdateDataHistoryImmediatelyAfterWrite",
            "ExecuteAfterWriteDataHistoryVersionProcessing",
        ] {
            push_bool(parts, properties, name)?;
        }
        for name in ["LevelCount", "CodeLength", "DescriptionLength"] {
            push_u32(parts, properties, name)?;
        }
        for name in [
            "HierarchyType",
            "SubordinationUse",
            "CodeType",
            "CodeAllowedLength",
            "CodeSeries",
            "DefaultPresentation",
            "PredefinedDataUpdate",
            "EditType",
            "ChoiceMode",
            "SearchStringModeOnInputByString",
            "FullTextSearchOnInputByString",
            "ChoiceDataGetModeOnInputByString",
            "DataLockControlMode",
            "FullTextSearch",
            "CreateOnInput",
            "ChoiceHistoryOnInput",
            "DataHistory",
        ] {
            push_enum(parts, properties, name)?;
        }
        for name in [
            "DefaultObjectForm",
            "DefaultFolderForm",
            "DefaultListForm",
            "DefaultChoiceForm",
            "DefaultFolderChoiceForm",
            "AuxiliaryObjectForm",
            "AuxiliaryFolderForm",
            "AuxiliaryListForm",
            "AuxiliaryChoiceForm",
            "AuxiliaryFolderChoiceForm",
        ] {
            push_text(parts, properties, name)?;
        }
        for name in ["Owners", "BasedOn", "DataLockFields"] {
            push_reference_collection(parts, properties, name, uris)?;
        }
    } else {
        for name in [
            "UseStandardCommands",
            "CheckUnique",
            "Autonumbering",
            "PostInPrivilegedMode",
            "UnpostInPrivilegedMode",
            "IncludeHelpInContents",
            "UpdateDataHistoryImmediatelyAfterWrite",
            "ExecuteAfterWriteDataHistoryVersionProcessing",
        ] {
            push_bool(parts, properties, name)?;
        }
        push_u32(parts, properties, "NumberLength")?;
        for name in [
            "NumberType",
            "NumberAllowedLength",
            "NumberPeriodicity",
            "CreateOnInput",
            "SearchStringModeOnInputByString",
            "FullTextSearchOnInputByString",
            "ChoiceDataGetModeOnInputByString",
            "Posting",
            "RealTimePosting",
            "RegisterRecordsDeletion",
            "RegisterRecordsWritingOnPost",
            "SequenceFilling",
            "DataLockControlMode",
            "FullTextSearch",
            "ChoiceHistoryOnInput",
            "DataHistory",
        ] {
            push_enum(parts, properties, name)?;
        }
        for name in [
            "Numerator",
            "DefaultObjectForm",
            "DefaultListForm",
            "DefaultChoiceForm",
            "AuxiliaryObjectForm",
            "AuxiliaryListForm",
            "AuxiliaryChoiceForm",
        ] {
            push_text(parts, properties, name)?;
        }
        for name in ["BasedOn", "RegisterRecords", "DataLockFields"] {
            push_reference_collection(parts, properties, name, uris)?;
        }
    }

    for name in ["StandardAttributes", "Characteristics"] {
        require_empty(properties[name], name)?;
    }
    push_field_collection(parts, properties, "InputByString", uris)?;
    for name in [
        "ObjectPresentation",
        "ExtendedObjectPresentation",
        "ListPresentation",
        "ExtendedListPresentation",
        "Explanation",
    ] {
        push_localized(parts, properties, name, uris)?;
    }
    Ok(())
}

pub(super) fn project_name_only_children(
    parts: &mut CanonicalObjectParts,
    family: &str,
    owner_name: String,
    children: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let mut forms = Vec::new();
    let mut templates = Vec::new();
    let mut seen = BTreeSet::new();
    for node in children.children() {
        let XmlNode::Element(element) = node else {
            continue;
        };
        let local = element.name().local();
        if !matches!(local, "Form" | "Template") {
            continue;
        }
        if !typed(element, local, Some(MD_NAMESPACE), uris) || !element.attributes().is_empty() {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "business object child reference is not exact",
            ));
        }
        let name = element_text(element)?.ok_or(MetadataDecodeError::InvalidEnvelope(
            "business object child reference is empty",
        ))?;
        if name.is_empty() || name.contains('.') {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "business object child reference name is invalid",
            ));
        }
        let target = format!("{family}.{owner_name}.{local}.{name}");
        if !seen.insert(target.to_lowercase()) {
            return Err(MetadataDecodeError::Duplicate(
                "business object child reference",
            ));
        }
        let value = CanonicalValue::reference(
            UnresolvedReference::new("metadata", &target)
                .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
        );
        if local == "Form" {
            forms.push(value);
        } else {
            templates.push(value);
        }
    }
    parts.properties.push(canonical_field(
        "ChildForms",
        CanonicalValue::sequence(forms)
            .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
    )?);
    parts.properties.push(canonical_field(
        "ChildTemplates",
        CanonicalValue::sequence(templates)
            .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
    )?);
    Ok(())
}

pub(super) fn collect_embedded_elements<'a>(
    children: &'a XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<BTreeMap<ObjectUuid, &'a XmlElement>, MetadataDecodeError> {
    fn visit<'a>(
        container: &'a XmlElement,
        uris: &ResolvedNamespaces,
        output: &mut BTreeMap<ObjectUuid, &'a XmlElement>,
    ) -> Result<(), MetadataDecodeError> {
        for node in container.children() {
            let XmlNode::Element(element) = node else {
                continue;
            };
            if matches!(element.name().local(), "Form" | "Template") {
                continue;
            }
            if uri_of(element, uris) != Some(MD_NAMESPACE) {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "business object child namespace is not exact",
                ));
            }
            let uuid = uuid_attribute(element)?;
            if output.insert(uuid, element).is_some() {
                return Err(MetadataDecodeError::Duplicate("business object child uuid"));
            }
            for child in element.children() {
                if let XmlNode::Element(nested) = child
                    && typed(nested, "ChildObjects", Some(MD_NAMESPACE), uris)
                {
                    visit(nested, uris, output)?;
                }
            }
        }
        Ok(())
    }

    let mut output = BTreeMap::new();
    visit(children, uris, &mut output)?;
    Ok(output)
}

pub(super) fn project_attribute(
    parts: &mut CanonicalObjectParts,
    element: &XmlElement,
    family: &str,
    root_uuid: ObjectUuid,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let properties = required_properties(element, uris)?;
    let nested = parts.owner.is_some_and(|owner| owner != root_uuid);
    let expected = if nested {
        NESTED_ATTRIBUTE_PROPERTIES
    } else if family == DOCUMENT {
        DOCUMENT_ATTRIBUTE_PROPERTIES
    } else {
        ATTRIBUTE_PROPERTIES
    };
    let map = exact_property_map(properties, expected, uris)?;
    push_text(parts, &map, "Comment")?;
    project_type(parts, map["Type"], uris)?;
    for name in ["PasswordMode", "MarkNegatives", "MultiLine", "ExtendedEdit"] {
        push_bool(parts, &map, name)?;
    }
    if map.contains_key("FillFromFillingValue") {
        push_bool(parts, &map, "FillFromFillingValue")?;
    }
    for name in [
        "FillChecking",
        "ChoiceFoldersAndItems",
        "QuickChoice",
        "CreateOnInput",
        "ChoiceHistoryOnInput",
        "Indexing",
        "FullTextSearch",
        "DataHistory",
    ] {
        push_enum(parts, &map, name)?;
    }
    if map.contains_key("Use") {
        push_enum(parts, &map, "Use")?;
    }
    for name in ["Mask", "ChoiceForm"] {
        push_text(parts, &map, name)?;
    }
    for name in [
        "Format",
        "EditFormat",
        "ToolTip",
        "MinValue",
        "MaxValue",
        "FillValue",
        "ChoiceParameterLinks",
        "ChoiceParameters",
        "LinkByType",
    ] {
        if let Some(element) = map.get(name) {
            require_empty(element, name)?;
        }
    }
    Ok(())
}

pub(super) fn project_type(
    parts: &mut CanonicalObjectParts,
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let mut types = Vec::new();
    let mut string_length = None;
    let mut string_allowed = None;
    let mut number_digits = None;
    let mut number_fraction = None;
    let mut number_sign = None;
    let mut date_fractions = None;
    for node in element.children() {
        let XmlNode::Element(child) = node else {
            continue;
        };
        if typed(child, "Type", Some(V8_NAMESPACE), uris)
            || typed(child, "TypeSet", Some(V8_NAMESPACE), uris)
        {
            let text = element_text(child)?.ok_or(MetadataDecodeError::InvalidEnvelope(
                "attribute type reference is empty",
            ))?;
            types.push(CanonicalValue::text(canonical_text(&text)?));
        } else if typed(child, "StringQualifiers", Some(V8_NAMESPACE), uris) {
            string_length = Some(nested_text(child, "Length", V8_NAMESPACE, uris)?);
            string_allowed = Some(nested_text(child, "AllowedLength", V8_NAMESPACE, uris)?);
        } else if typed(child, "NumberQualifiers", Some(V8_NAMESPACE), uris) {
            number_digits = Some(nested_text(child, "Digits", V8_NAMESPACE, uris)?);
            number_fraction = Some(nested_text(child, "FractionDigits", V8_NAMESPACE, uris)?);
            number_sign = Some(nested_text(child, "AllowedSign", V8_NAMESPACE, uris)?);
        } else if typed(child, "DateQualifiers", Some(V8_NAMESPACE), uris) {
            date_fractions = Some(nested_text(child, "DateFractions", V8_NAMESPACE, uris)?);
        } else {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "attribute type contains an unknown qualifier",
            ));
        }
    }
    if types.is_empty() {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "attribute type collection is empty",
        ));
    }
    parts.properties.push(canonical_field(
        "Types",
        CanonicalValue::sequence(types)
            .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
    )?);
    for (name, value) in [
        ("StringLength", string_length),
        ("StringAllowedLength", string_allowed),
        ("NumberDigits", number_digits),
        ("NumberFractionDigits", number_fraction),
        ("NumberAllowedSign", number_sign),
        ("DateFractions", date_fractions),
    ] {
        if let Some(value) = value {
            parts.properties.push(canonical_field(
                name,
                CanonicalValue::text(canonical_text(&value)?),
            )?);
        }
    }
    Ok(())
}

pub(super) fn project_tabular_section(
    parts: &mut CanonicalObjectParts,
    element: &XmlElement,
    family: &str,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let properties = required_properties(element, uris)?;
    let expected = if family == CATALOG {
        TABULAR_PROPERTIES
    } else {
        DOCUMENT_TABULAR_PROPERTIES
    };
    let map = exact_property_map(properties, expected, uris)?;
    push_text(parts, &map, "Comment")?;
    require_empty(map["ToolTip"], "ToolTip")?;
    require_empty(map["StandardAttributes"], "StandardAttributes")?;
    push_enum(parts, &map, "FillChecking")?;
    if map.contains_key("Use") {
        push_enum(parts, &map, "Use")?;
    }
    push_u32(parts, &map, "LineNumberLength")?;
    Ok(())
}

pub(super) fn project_command(
    parts: &mut CanonicalObjectParts,
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let properties = required_properties(element, uris)?;
    let map = exact_property_map(properties, COMMAND_PROPERTIES, uris)?;
    push_text(parts, &map, "Comment")?;
    push_text(parts, &map, "Group")?;
    push_enum(parts, &map, "ParameterUseMode")?;
    push_bool(parts, &map, "ModifiesData")?;
    push_enum(parts, &map, "Representation")?;
    push_enum(parts, &map, "OnMainServerUnavalableBehavior")?;
    for name in ["CommandParameterType", "ToolTip", "Picture", "Shortcut"] {
        require_empty(map[name], name)?;
    }
    Ok(())
}

pub(super) fn required_properties<'a>(
    object: &'a XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<&'a XmlElement, MetadataDecodeError> {
    let mut result = None;
    for node in object.children() {
        let XmlNode::Element(element) = node else {
            continue;
        };
        if typed(element, "InternalInfo", Some(MD_NAMESPACE), uris)
            || typed(element, "ChildObjects", Some(MD_NAMESPACE), uris)
        {
            continue;
        }
        if typed(element, "Properties", Some(MD_NAMESPACE), uris)
            && result.replace(element).is_none()
        {
            continue;
        }
        return Err(MetadataDecodeError::InvalidEnvelope(
            "embedded business object contains an unknown section",
        ));
    }
    result.ok_or(MetadataDecodeError::Missing("embedded Properties"))
}

pub(super) fn push_text(
    parts: &mut CanonicalObjectParts,
    map: &BTreeMap<&str, &XmlElement>,
    name: &str,
) -> Result<(), MetadataDecodeError> {
    let value = text_field(map, name)?;
    parts.properties.push(canonical_field(
        name,
        CanonicalValue::text(canonical_text(&value)?),
    )?);
    Ok(())
}

pub(super) fn push_bool(
    parts: &mut CanonicalObjectParts,
    map: &BTreeMap<&str, &XmlElement>,
    name: &str,
) -> Result<(), MetadataDecodeError> {
    let value = match text_field(map, name)?.as_str() {
        "true" => true,
        "false" => false,
        _ => {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "business object boolean is not canonical",
            ));
        }
    };
    parts
        .properties
        .push(canonical_field(name, CanonicalValue::boolean(value))?);
    Ok(())
}

pub(super) fn push_u32(
    parts: &mut CanonicalObjectParts,
    map: &BTreeMap<&str, &XmlElement>,
    name: &str,
) -> Result<(), MetadataDecodeError> {
    let value = text_field(map, name)?;
    let parsed = value
        .parse::<u32>()
        .ok()
        .filter(|candidate| candidate.to_string() == value)
        .ok_or(MetadataDecodeError::InvalidEnvelope(
            "business object integer is not canonical u32",
        ))?;
    parts.properties.push(canonical_field(
        name,
        CanonicalValue::integer(
            CanonicalInteger::new(&parsed.to_string())
                .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
        ),
    )?);
    Ok(())
}

pub(super) fn push_enum(
    parts: &mut CanonicalObjectParts,
    map: &BTreeMap<&str, &XmlElement>,
    name: &str,
) -> Result<(), MetadataDecodeError> {
    let value = text_field(map, name)?;
    parts.properties.push(canonical_field(
        name,
        CanonicalValue::enum_token(
            EnumToken::new(&value).map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
        ),
    )?);
    Ok(())
}

pub(super) fn push_reference_collection(
    parts: &mut CanonicalObjectParts,
    map: &BTreeMap<&str, &XmlElement>,
    name: &str,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let mut values = Vec::new();
    for node in map[name].children() {
        let XmlNode::Element(element) = node else {
            continue;
        };
        if !typed(element, "Item", Some(super::common::XR_NAMESPACE), uris) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "business object reference collection contains an unknown item",
            ));
        }
        let value = element_text(element)?.ok_or(MetadataDecodeError::InvalidEnvelope(
            "business object reference is empty",
        ))?;
        values.push(CanonicalValue::reference(
            UnresolvedReference::new("metadata", &value)
                .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
        ));
    }
    parts.properties.push(canonical_field(
        name,
        CanonicalValue::sequence(values)
            .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
    )?);
    Ok(())
}

pub(super) fn push_field_collection(
    parts: &mut CanonicalObjectParts,
    map: &BTreeMap<&str, &XmlElement>,
    name: &str,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let mut values = Vec::new();
    for node in map[name].children() {
        let XmlNode::Element(element) = node else {
            continue;
        };
        if !typed(element, "Field", Some(super::common::XR_NAMESPACE), uris) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "business object field collection contains an unknown item",
            ));
        }
        let value = element_text(element)?.ok_or(MetadataDecodeError::InvalidEnvelope(
            "business object field reference is empty",
        ))?;
        values.push(CanonicalValue::text(canonical_text(&value)?));
    }
    parts.properties.push(canonical_field(
        name,
        CanonicalValue::sequence(values)
            .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
    )?);
    Ok(())
}

pub(super) fn push_localized(
    parts: &mut CanonicalObjectParts,
    map: &BTreeMap<&str, &XmlElement>,
    name: &str,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let mut values = Vec::new();
    let mut languages = BTreeSet::new();
    for node in map[name].children() {
        let XmlNode::Element(item) = node else {
            continue;
        };
        if !typed(item, "item", Some(V8_NAMESPACE), uris) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "localized value contains an unknown item",
            ));
        }
        let language = nested_text(item, "lang", V8_NAMESPACE, uris)?;
        let content = nested_text(item, "content", V8_NAMESPACE, uris)?;
        if !languages.insert(language.clone()) {
            return Err(MetadataDecodeError::Duplicate("localized language"));
        }
        values.push(
            CanonicalValue::record(vec![
                canonical_field("language", CanonicalValue::text(canonical_text(&language)?))?,
                canonical_field("content", CanonicalValue::text(canonical_text(&content)?))?,
            ])
            .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
        );
    }
    parts.properties.push(canonical_field(
        name,
        CanonicalValue::sequence(values)
            .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
    )?);
    Ok(())
}

fn nested_text(
    parent: &XmlElement,
    local: &'static str,
    namespace: &str,
    uris: &ResolvedNamespaces,
) -> Result<String, MetadataDecodeError> {
    let mut result = None;
    for node in parent.children() {
        if let XmlNode::Element(element) = node
            && typed(element, local, Some(namespace), uris)
        {
            if result.is_some() {
                return Err(MetadataDecodeError::Duplicate(local));
            }
            result = element_text(element)?;
        }
    }
    result.ok_or(MetadataDecodeError::Missing(local))
}

pub(super) fn text_field(
    map: &BTreeMap<&str, &XmlElement>,
    name: &str,
) -> Result<String, MetadataDecodeError> {
    element_text(map[name])?.ok_or(MetadataDecodeError::InvalidEnvelope(
        "business object property must contain text only",
    ))
}

pub(super) fn require_empty(element: &XmlElement, _name: &str) -> Result<(), MetadataDecodeError> {
    if element.children().iter().any(|node| match node {
        XmlNode::Element(_) => true,
        XmlNode::Text(text) => !text.value().trim().is_empty(),
        _ => false,
    }) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "business object unevidenced complex property is not empty",
        ));
    }
    Ok(())
}

pub(super) fn only_element_child<'a>(
    parent: &'a XmlElement,
    label: &'static str,
) -> Result<&'a XmlElement, MetadataDecodeError> {
    let mut result = None;
    for node in parent.children() {
        if let XmlNode::Element(element) = node
            && result.replace(element).is_some()
        {
            return Err(MetadataDecodeError::Duplicate(label));
        }
    }
    result.ok_or(MetadataDecodeError::Missing(label))
}

pub(super) fn uuid_attribute(element: &XmlElement) -> Result<ObjectUuid, MetadataDecodeError> {
    let mut value = None;
    for attribute in element.attributes() {
        if let crate::AttributeKind::Ordinary(name) = attribute.kind()
            && name.prefix().is_none()
            && name.local() == "uuid"
            && value.replace(attribute.value()).is_some()
        {
            return Err(MetadataDecodeError::Duplicate("uuid"));
        }
    }
    let value = value.ok_or(MetadataDecodeError::Missing("uuid"))?;
    ObjectUuid::parse(value).map_err(|_| MetadataDecodeError::InvalidUuid(value.to_owned()))
}

fn encode_business_object(
    envelope: &MetadataEnvelope,
    target: &ProfileId,
    family: &str,
) -> Result<Vec<u8>, MetadataEncodeError> {
    let path = envelope.root().identity().path().clone();
    let target_version =
        profile_version(target).ok_or_else(|| MetadataEncodeError::UnsupportedProfile {
            object_path: path.clone(),
            profile: target.clone(),
        })?;
    if envelope.root().kind().as_str() != family {
        return Err(invalid_model(&path, "kind"));
    }
    let source_profile = envelope.root().provenance().source_profile().clone();
    validate_decode_profile(envelope.source_document(), &source_profile, &path)
        .map_err(decode_to_encode)?;
    let source = decode_business_object(
        envelope.source_document(),
        source_profile,
        path.clone(),
        family,
    )
    .map_err(decode_to_encode)?;
    if source.root() != envelope.root() || source.descendants() != envelope.descendants() {
        return Err(invalid_model(
            &path,
            "business object semantic mutation is not implemented",
        ));
    }
    let root = if root_version(envelope.source_document().root()).map_err(decode_to_encode)?
        == target_version
    {
        envelope.source_document().root().clone()
    } else {
        set_unprefixed_attribute(
            envelope.source_document().root(),
            "version",
            target_version,
            &path,
        )?
    };
    let root = if family == CATALOG {
        rewrite_catalog_palette_namespace(&root, target_version, &path)?
    } else {
        root
    };
    XmlWriter::to_vec(
        &envelope.source_document().with_root(root),
        LexicalPolicy::Preserve,
    )
    .map_err(|error| MetadataEncodeError::Xml(error.to_string()))
}

fn rewrite_catalog_palette_namespace(
    root: &XmlElement,
    target_version: &str,
    path: &ObjectPath,
) -> Result<XmlElement, MetadataEncodeError> {
    let target_has_palette = target_version == "2.21";
    let mut attributes =
        Vec::with_capacity(root.attributes().len() + usize::from(target_has_palette));
    let mut exact_binding = false;
    for attribute in root.attributes() {
        match attribute.kind() {
            AttributeKind::Namespace(Some(prefix)) if prefix == PALETTE_PREFIX => {
                if attribute.value() != PALETTE_NAMESPACE || exact_binding {
                    return Err(invalid_model(path, "palette namespace"));
                }
                exact_binding = true;
                if target_has_palette {
                    attributes.push(attribute.clone());
                }
            }
            AttributeKind::Namespace(_) if attribute.value() == PALETTE_NAMESPACE => {
                return Err(invalid_model(path, "palette namespace prefix"));
            }
            _ => attributes.push(attribute.clone()),
        }
    }
    if target_has_palette && !exact_binding {
        let insertion = attributes
            .iter()
            .position(|attribute| match attribute.kind() {
                AttributeKind::Namespace(None) => false,
                AttributeKind::Namespace(Some(prefix)) => prefix.as_str() > PALETTE_PREFIX,
                AttributeKind::Ordinary(_) => true,
            })
            .unwrap_or(attributes.len());
        attributes.insert(
            insertion,
            Attribute::namespace(Some(PALETTE_PREFIX.to_string()), PALETTE_NAMESPACE),
        );
    }
    Ok(XmlElement::with_parts(
        root.name().clone(),
        attributes,
        root.children().to_vec(),
    ))
}

#[cfg(test)]
mod tests {
    use ibcmd_core::diagnostic::ObjectPath;

    use super::*;
    use crate::XmlReader;

    const CATALOG_UUID: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    const DOCUMENT_UUID: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";

    fn generated(prefix: &str, category: &str, seed: u8) -> String {
        format!(
            "<xr:GeneratedType name=\"{prefix}.Products\" category=\"{category}\"><xr:TypeId>00000000-0000-4000-8000-{seed:012x}</xr:TypeId><xr:ValueId>00000000-0000-4000-8000-{:012x}</xr:ValueId></xr:GeneratedType>",
            seed + 1
        )
    }

    fn catalog_xml() -> Vec<u8> {
        let generated = [
            generated("CatalogObject", "Object", 10),
            generated("CatalogRef", "Ref", 20),
            generated("CatalogSelection", "Selection", 30),
            generated("CatalogList", "List", 40),
            generated("CatalogManager", "Manager", 50),
        ]
        .join("");
        format!(
            "<MetaDataObject xmlns=\"{MD_NAMESPACE}\" xmlns:v8=\"{V8_NAMESPACE}\" xmlns:xr=\"{}\" version=\"2.20\"><Catalog uuid=\"{CATALOG_UUID}\"><InternalInfo>{generated}</InternalInfo><Properties><Name>Products</Name><Synonym/><Comment/><Hierarchical>false</Hierarchical><HierarchyType>HierarchyFoldersAndItems</HierarchyType><LimitLevelCount>false</LimitLevelCount><LevelCount>2</LevelCount><FoldersOnTop>true</FoldersOnTop><UseStandardCommands>true</UseStandardCommands><Owners/><SubordinationUse>ToItems</SubordinationUse><CodeLength>9</CodeLength><DescriptionLength>100</DescriptionLength><CodeType>String</CodeType><CodeAllowedLength>Variable</CodeAllowedLength><CodeSeries>WholeCatalog</CodeSeries><CheckUnique>true</CheckUnique><Autonumbering>true</Autonumbering><DefaultPresentation>AsDescription</DefaultPresentation><StandardAttributes/><Characteristics/><PredefinedDataUpdate>Auto</PredefinedDataUpdate><EditType>InDialog</EditType><QuickChoice>false</QuickChoice><ChoiceMode>BothWays</ChoiceMode><InputByString/><SearchStringModeOnInputByString>Begin</SearchStringModeOnInputByString><FullTextSearchOnInputByString>DontUse</FullTextSearchOnInputByString><ChoiceDataGetModeOnInputByString>Directly</ChoiceDataGetModeOnInputByString><DefaultObjectForm/><DefaultFolderForm/><DefaultListForm/><DefaultChoiceForm/><DefaultFolderChoiceForm/><AuxiliaryObjectForm/><AuxiliaryFolderForm/><AuxiliaryListForm/><AuxiliaryChoiceForm/><AuxiliaryFolderChoiceForm/><IncludeHelpInContents>false</IncludeHelpInContents><BasedOn/><DataLockFields/><DataLockControlMode>Managed</DataLockControlMode><FullTextSearch>Use</FullTextSearch><ObjectPresentation/><ExtendedObjectPresentation/><ListPresentation/><ExtendedListPresentation/><Explanation/><CreateOnInput>Use</CreateOnInput><ChoiceHistoryOnInput>DontUse</ChoiceHistoryOnInput><DataHistory>DontUse</DataHistory><UpdateDataHistoryImmediatelyAfterWrite>false</UpdateDataHistoryImmediatelyAfterWrite><ExecuteAfterWriteDataHistoryVersionProcessing>false</ExecuteAfterWriteDataHistoryVersionProcessing></Properties><ChildObjects/></Catalog></MetaDataObject>",
            super::super::common::XR_NAMESPACE
        )
        .into_bytes()
    }

    fn document_xml() -> Vec<u8> {
        let generated = [
            generated("DocumentObject", "Object", 60),
            generated("DocumentRef", "Ref", 70),
            generated("DocumentSelection", "Selection", 80),
            generated("DocumentList", "List", 90),
            generated("DocumentManager", "Manager", 100),
        ]
        .join("")
        .replace(".Products", ".Invoices");
        format!(
            "<MetaDataObject xmlns=\"{MD_NAMESPACE}\" xmlns:v8=\"{V8_NAMESPACE}\" xmlns:xr=\"{}\" version=\"2.20\"><Document uuid=\"{DOCUMENT_UUID}\"><InternalInfo>{generated}</InternalInfo><Properties><Name>Invoices</Name><Synonym/><Comment/><UseStandardCommands>true</UseStandardCommands><Numerator/><NumberType>String</NumberType><NumberLength>11</NumberLength><NumberAllowedLength>Variable</NumberAllowedLength><NumberPeriodicity>Year</NumberPeriodicity><CheckUnique>true</CheckUnique><Autonumbering>true</Autonumbering><StandardAttributes/><Characteristics/><BasedOn/><InputByString/><CreateOnInput>DontUse</CreateOnInput><SearchStringModeOnInputByString>Begin</SearchStringModeOnInputByString><FullTextSearchOnInputByString>DontUse</FullTextSearchOnInputByString><ChoiceDataGetModeOnInputByString>Directly</ChoiceDataGetModeOnInputByString><DefaultObjectForm/><DefaultListForm/><DefaultChoiceForm/><AuxiliaryObjectForm/><AuxiliaryListForm/><AuxiliaryChoiceForm/><Posting>Allow</Posting><RealTimePosting>Allow</RealTimePosting><RegisterRecordsDeletion>AutoDelete</RegisterRecordsDeletion><RegisterRecordsWritingOnPost>WriteSelected</RegisterRecordsWritingOnPost><SequenceFilling>AutoFill</SequenceFilling><RegisterRecords/><PostInPrivilegedMode>false</PostInPrivilegedMode><UnpostInPrivilegedMode>false</UnpostInPrivilegedMode><IncludeHelpInContents>false</IncludeHelpInContents><DataLockFields/><DataLockControlMode>Managed</DataLockControlMode><FullTextSearch>Use</FullTextSearch><ObjectPresentation/><ExtendedObjectPresentation/><ListPresentation/><ExtendedListPresentation/><Explanation/><ChoiceHistoryOnInput>DontUse</ChoiceHistoryOnInput><DataHistory>DontUse</DataHistory><UpdateDataHistoryImmediatelyAfterWrite>false</UpdateDataHistoryImmediatelyAfterWrite><ExecuteAfterWriteDataHistoryVersionProcessing>false</ExecuteAfterWriteDataHistoryVersionProcessing></Properties><ChildObjects/></Document></MetaDataObject>",
            super::super::common::XR_NAMESPACE
        )
        .into_bytes()
    }

    #[test]
    fn catalog_minimal_projection_is_strict_and_typed() {
        let document = XmlReader::from_slice(&catalog_xml()).unwrap();
        let envelope = decode_business_object(
            &document,
            ProfileId::parse("xml-2.20").unwrap(),
            ObjectPath::root(),
            CATALOG,
        )
        .unwrap();
        assert_eq!(envelope.root().identity().uuid().to_string(), CATALOG_UUID);
        assert_eq!(envelope.root().generated_types().len(), 5);
        assert!(envelope.descendants().is_empty());
        assert!(envelope.root().properties().len() > 40);
    }

    #[test]
    fn document_minimal_projection_is_strict_and_typed() {
        let document = XmlReader::from_slice(&document_xml()).unwrap();
        let envelope = decode_business_object(
            &document,
            ProfileId::parse("xml-2.20").unwrap(),
            ObjectPath::root(),
            DOCUMENT,
        )
        .unwrap();
        assert_eq!(envelope.root().identity().uuid().to_string(), DOCUMENT_UUID);
        assert_eq!(envelope.root().generated_types().len(), 5);
        assert!(envelope.descendants().is_empty());
        assert!(envelope.root().properties().len() > 40);
    }

    #[test]
    fn nonempty_unevidenced_standard_attributes_fail_closed() {
        let xml = String::from_utf8(catalog_xml()).unwrap().replace(
            "<StandardAttributes/>",
            "<StandardAttributes><xr:StandardAttribute name=\"Code\"/></StandardAttributes>",
        );
        let document = XmlReader::from_slice(xml.as_bytes()).unwrap();
        assert!(
            decode_business_object(
                &document,
                ProfileId::parse("xml-2.20").unwrap(),
                ObjectPath::root(),
                CATALOG,
            )
            .is_err()
        );
    }

    #[test]
    fn catalog_cross_profile_encoding_applies_evidenced_palette_delta() {
        let document = XmlReader::from_slice(&catalog_xml()).unwrap();
        let envelope = decode_business_object(
            &document,
            ProfileId::parse("xml-2.20").unwrap(),
            ObjectPath::root(),
            CATALOG,
        )
        .unwrap();

        let upgraded =
            encode_business_object(&envelope, &ProfileId::parse("xml-2.21").unwrap(), CATALOG)
                .unwrap();
        let upgraded_text = std::str::from_utf8(&upgraded).unwrap();
        assert!(upgraded_text.contains("version=\"2.21\""));
        assert!(upgraded_text.contains(&format!("xmlns:{PALETTE_PREFIX}=\"{PALETTE_NAMESPACE}\"")));
        let upgraded_document = XmlReader::from_slice(&upgraded).unwrap();
        let upgraded_envelope = decode_business_object(
            &upgraded_document,
            ProfileId::parse("xml-2.21").unwrap(),
            ObjectPath::root(),
            CATALOG,
        )
        .unwrap();
        assert_eq!(
            upgraded_envelope.root().identity(),
            envelope.root().identity()
        );
        assert_eq!(upgraded_envelope.root().kind(), envelope.root().kind());
        assert_eq!(upgraded_envelope.root().owner(), envelope.root().owner());
        assert_eq!(
            upgraded_envelope.root().properties(),
            envelope.root().properties()
        );
        assert_eq!(
            upgraded_envelope.root().references(),
            envelope.root().references()
        );
        assert_eq!(
            upgraded_envelope.root().generated_types(),
            envelope.root().generated_types()
        );
        assert_eq!(upgraded_envelope.root().assets(), envelope.root().assets());

        let downgraded = encode_business_object(
            &upgraded_envelope,
            &ProfileId::parse("xml-2.20").unwrap(),
            CATALOG,
        )
        .unwrap();
        let downgraded_text = std::str::from_utf8(&downgraded).unwrap();
        assert!(downgraded_text.contains("version=\"2.20\""));
        assert!(!downgraded_text.contains(PALETTE_NAMESPACE));
    }
}
