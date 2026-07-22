//! Strict canonical XCF codecs for hierarchical and workflow metadata.
//!
//! The codec deliberately projects only the 8.3.27 shapes evidenced by the
//! offline native compiler.  Unsupported standard-attribute customizations
//! and complex design-time values fail closed instead of being discarded.

use std::collections::{BTreeMap, BTreeSet};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::ObjectPath;
use ibcmd_core::family::FamilyId;
use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::model::{CanonicalObject, CanonicalObjectParts};
use ibcmd_core::value::{CanonicalValue, UnresolvedReference};

use super::business_objects::{
    collect_embedded_elements, exact_object_sections, exact_property_map, only_element_child,
    project_attribute, project_command, project_name_only_children, project_type, push_bool,
    push_enum, push_field_collection, push_localized, push_reference_collection, push_text,
    push_u32, require_empty, required_properties, text_field,
};
use super::common::{
    MD_NAMESPACE, MetadataDecodeError, MetadataEnvelope, ResolvedNamespaces, XR_NAMESPACE,
    decode_metadata_envelope_with_child_references, element_text, resolve_namespaces, typed,
    uri_of,
};
use super::language::{
    canonical_field, canonical_text, copy_object_parts, decode_to_encode, invalid_model,
    profile_version, root_version, set_unprefixed_attribute, validate_decode_profile,
};
use super::registry::{
    MetadataEncodeError, MetadataFamilyCodec, MetadataRegistry, MetadataRegistryError,
};
use super::utility_objects::validate_standard_attributes;
use crate::{LexicalPolicy, XmlDocument, XmlElement, XmlNode, XmlWriter};

const SUBSYSTEM: &str = "Subsystem";
const EXCHANGE_PLAN: &str = "ExchangePlan";
const BUSINESS_PROCESS: &str = "BusinessProcess";
const TASK: &str = "Task";

const SUBSYSTEM_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "IncludeHelpInContents",
    "IncludeInCommandInterface",
    "UseOneCommand",
    "Explanation",
    "Picture",
    "Content",
];

const EXCHANGE_PLAN_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "CodeLength",
    "CodeAllowedLength",
    "DescriptionLength",
    "DefaultPresentation",
    "EditType",
    "QuickChoice",
    "ChoiceMode",
    "InputByString",
    "SearchStringModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "DefaultObjectForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "StandardAttributes",
    "Characteristics",
    "BasedOn",
    "DistributedInfoBase",
    "IncludeConfigurationExtensions",
    "CreateOnInput",
    "ChoiceHistoryOnInput",
    "IncludeHelpInContents",
    "DataLockFields",
    "DataLockControlMode",
    "FullTextSearch",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "DataHistory",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
];

const BUSINESS_PROCESS_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "EditType",
    "InputByString",
    "CreateOnInput",
    "SearchStringModeOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "FullTextSearchOnInputByString",
    "DefaultObjectForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "ChoiceHistoryOnInput",
    "NumberType",
    "NumberLength",
    "NumberAllowedLength",
    "CheckUnique",
    "StandardAttributes",
    "Characteristics",
    "Autonumbering",
    "BasedOn",
    "NumberPeriodicity",
    "Task",
    "CreateTaskInPrivilegedMode",
    "DataLockFields",
    "DataLockControlMode",
    "IncludeHelpInContents",
    "FullTextSearch",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "DataHistory",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
];

const TASK_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "NumberType",
    "NumberLength",
    "NumberAllowedLength",
    "CheckUnique",
    "Autonumbering",
    "TaskNumberAutoPrefix",
    "DescriptionLength",
    "Addressing",
    "MainAddressingAttribute",
    "CurrentPerformer",
    "BasedOn",
    "StandardAttributes",
    "Characteristics",
    "DefaultPresentation",
    "EditType",
    "InputByString",
    "SearchStringModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "CreateOnInput",
    "DefaultObjectForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "ChoiceHistoryOnInput",
    "IncludeHelpInContents",
    "DataLockFields",
    "DataLockControlMode",
    "FullTextSearch",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "DataHistory",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
];

const ADDRESSING_ATTRIBUTE_PROPERTIES: &[&str] = &[
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
    "AddressingDimension",
    "FullTextSearch",
    "DataHistory",
];

const EXCHANGE_TABULAR_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "ToolTip",
    "FillChecking",
    "StandardAttributes",
    "LineNumberLength",
];

const BUSINESS_PROCESS_TABULAR_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "ToolTip",
    "FillChecking",
    "LineNumberLength",
];

pub fn register_subsystem_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    register(registry, SUBSYSTEM)
}

pub fn register_exchange_plan_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    register(registry, EXCHANGE_PLAN)
}

pub fn register_business_process_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    register(registry, BUSINESS_PROCESS)
}

pub fn register_task_codec(registry: &mut MetadataRegistry) -> Result<(), MetadataRegistryError> {
    register(registry, TASK)
}

fn register(registry: &mut MetadataRegistry, family: &str) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(HierarchicalObjectCodec {
        family: FamilyId::parse(family).expect("hierarchical family literal is valid"),
    }))
}

struct HierarchicalObjectCodec {
    family: FamilyId,
}

impl MetadataFamilyCodec for HierarchicalObjectCodec {
    fn family_id(&self) -> &FamilyId {
        &self.family
    }

    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        decode_hierarchical_object(document, source, path, self.family.as_str())
    }

    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        encode_hierarchical_object(envelope, target, self.family.as_str())
    }
}

fn decode_hierarchical_object(
    document: &XmlDocument,
    source: ProfileId,
    path: ObjectPath,
    family: &str,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    validate_decode_profile(document, &source, &path)?;
    let child_references: &[&str] = if family == SUBSYSTEM {
        &[SUBSYSTEM]
    } else {
        &["Form", "Template"]
    };
    let generic =
        decode_metadata_envelope_with_child_references(document, source, path, child_references)?;
    if generic.root().kind().as_str() != family
        || !matches!(family, SUBSYSTEM | EXCHANGE_PLAN | BUSINESS_PROCESS | TASK)
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "hierarchical codec family differs from XML",
        ));
    }

    let uris = resolve_namespaces(document.root())?;
    if uri_of(document.root(), &uris) != Some(MD_NAMESPACE) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "hierarchical object requires the MDClasses namespace",
        ));
    }
    let object = only_element_child(document.root(), "metadata object")?;
    if !typed(object, family, Some(MD_NAMESPACE), &uris) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "hierarchical object element is not exact",
        ));
    }
    let sections = exact_object_sections(object, family, &uris)?;
    let expected = match family {
        SUBSYSTEM => SUBSYSTEM_PROPERTIES,
        EXCHANGE_PLAN => EXCHANGE_PLAN_PROPERTIES,
        BUSINESS_PROCESS => BUSINESS_PROCESS_PROPERTIES,
        TASK => TASK_PROPERTIES,
        _ => unreachable!("family guard above is exhaustive"),
    };
    let properties = exact_property_map(sections.properties, expected, &uris)?;

    let mut root_parts = copy_object_parts(generic.root());
    project_root_properties(&mut root_parts, family, object, &properties, &uris)?;
    let owner_name = text_field(&properties, "Name")?;
    if family == SUBSYSTEM {
        project_subsystem_children(&mut root_parts, &owner_name, sections.children, &uris)?;
    } else {
        project_name_only_children(
            &mut root_parts,
            family,
            owner_name,
            sections.children,
            &uris,
        )?;
    }
    let root = CanonicalObject::new(root_parts)
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))?;

    if family == SUBSYSTEM {
        if !generic.descendants().is_empty() {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "Subsystem contains an unsupported embedded object",
            ));
        }
        return MetadataEnvelope::from_parts(root, Vec::new(), document.clone());
    }

    let element_by_uuid = collect_embedded_elements(sections.children, &uris)?;
    if element_by_uuid.len() != generic.descendants().len() {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "hierarchical descendant inventory is not exact",
        ));
    }
    let mut descendants = Vec::with_capacity(generic.descendants().len());
    for descendant in generic.descendants() {
        let element = element_by_uuid.get(&descendant.identity().uuid()).ok_or(
            MetadataDecodeError::InvalidEnvelope("hierarchical descendant has no XML element"),
        )?;
        let mut parts = copy_object_parts(descendant);
        match descendant.kind().as_str() {
            "Attribute" if family != SUBSYSTEM => project_attribute(
                &mut parts,
                element,
                "Document",
                root.identity().uuid(),
                &uris,
            )?,
            "AddressingAttribute" if family == TASK => {
                project_addressing_attribute(&mut parts, element, &uris)?
            }
            "TabularSection" if matches!(family, EXCHANGE_PLAN | BUSINESS_PROCESS) => {
                project_workflow_tabular(&mut parts, element, family, &uris)?
            }
            "Command" if descendant.owner() == Some(root.identity().uuid()) => {
                project_command(&mut parts, element, &uris)?
            }
            _ => {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "hierarchical object contains an unsupported embedded child",
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

fn project_root_properties(
    parts: &mut CanonicalObjectParts,
    family: &str,
    object: &XmlElement,
    properties: &BTreeMap<&str, &XmlElement>,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    push_text(parts, properties, "Comment")?;
    match family {
        SUBSYSTEM => {
            for name in [
                "IncludeHelpInContents",
                "IncludeInCommandInterface",
                "UseOneCommand",
            ] {
                push_bool(parts, properties, name)?;
            }
            push_localized(parts, properties, "Explanation", uris)?;
            project_picture(parts, properties["Picture"], uris)?;
            push_reference_collection(parts, properties, "Content", uris)?;
        }
        EXCHANGE_PLAN => {
            for name in [
                "UseStandardCommands",
                "QuickChoice",
                "DistributedInfoBase",
                "IncludeConfigurationExtensions",
                "IncludeHelpInContents",
                "UpdateDataHistoryImmediatelyAfterWrite",
                "ExecuteAfterWriteDataHistoryVersionProcessing",
            ] {
                push_bool(parts, properties, name)?;
            }
            for name in ["CodeLength", "DescriptionLength"] {
                push_u32(parts, properties, name)?;
            }
            for name in [
                "CodeAllowedLength",
                "DefaultPresentation",
                "EditType",
                "ChoiceMode",
                "SearchStringModeOnInputByString",
                "FullTextSearchOnInputByString",
                "ChoiceDataGetModeOnInputByString",
                "CreateOnInput",
                "ChoiceHistoryOnInput",
                "DataLockControlMode",
                "FullTextSearch",
                "DataHistory",
            ] {
                push_enum(parts, properties, name)?;
            }
            push_internal_this_node(parts, object, uris)?;
            project_shared_forms_and_references(parts, properties, uris, true)?;
            validate_supported_standard_attributes(
                properties["StandardAttributes"],
                &[
                    "ExchangeDate",
                    "ThisNode",
                    "ReceivedNo",
                    "SentNo",
                    "Ref",
                    "DeletionMark",
                    "Description",
                    "Code",
                ],
                uris,
            )?;
            require_empty(properties["Characteristics"], "Characteristics")?;
        }
        BUSINESS_PROCESS => {
            for name in [
                "UseStandardCommands",
                "CheckUnique",
                "Autonumbering",
                "CreateTaskInPrivilegedMode",
                "IncludeHelpInContents",
                "UpdateDataHistoryImmediatelyAfterWrite",
                "ExecuteAfterWriteDataHistoryVersionProcessing",
            ] {
                push_bool(parts, properties, name)?;
            }
            push_u32(parts, properties, "NumberLength")?;
            for name in [
                "EditType",
                "CreateOnInput",
                "SearchStringModeOnInputByString",
                "ChoiceDataGetModeOnInputByString",
                "FullTextSearchOnInputByString",
                "ChoiceHistoryOnInput",
                "NumberType",
                "NumberAllowedLength",
                "NumberPeriodicity",
                "DataLockControlMode",
                "FullTextSearch",
                "DataHistory",
            ] {
                push_enum(parts, properties, name)?;
            }
            push_text(parts, properties, "Task")?;
            project_shared_forms_and_references(parts, properties, uris, false)?;
            validate_supported_standard_attributes(
                properties["StandardAttributes"],
                &[
                    "Started",
                    "HeadTask",
                    "Completed",
                    "Ref",
                    "DeletionMark",
                    "Date",
                    "Number",
                ],
                uris,
            )?;
            require_empty(properties["Characteristics"], "Characteristics")?;
        }
        TASK => {
            for name in [
                "UseStandardCommands",
                "CheckUnique",
                "Autonumbering",
                "IncludeHelpInContents",
                "UpdateDataHistoryImmediatelyAfterWrite",
                "ExecuteAfterWriteDataHistoryVersionProcessing",
            ] {
                push_bool(parts, properties, name)?;
            }
            for name in ["NumberLength", "DescriptionLength"] {
                push_u32(parts, properties, name)?;
            }
            for name in [
                "NumberType",
                "NumberAllowedLength",
                "TaskNumberAutoPrefix",
                "DefaultPresentation",
                "EditType",
                "SearchStringModeOnInputByString",
                "FullTextSearchOnInputByString",
                "ChoiceDataGetModeOnInputByString",
                "CreateOnInput",
                "ChoiceHistoryOnInput",
                "DataLockControlMode",
                "FullTextSearch",
                "DataHistory",
            ] {
                push_enum(parts, properties, name)?;
            }
            for name in ["Addressing", "MainAddressingAttribute", "CurrentPerformer"] {
                push_text(parts, properties, name)?;
            }
            project_shared_forms_and_references(parts, properties, uris, false)?;
            validate_supported_standard_attributes(
                properties["StandardAttributes"],
                &[
                    "Executed",
                    "Description",
                    "RoutePoint",
                    "BusinessProcess",
                    "Ref",
                    "DeletionMark",
                    "Date",
                    "Number",
                ],
                uris,
            )?;
            require_empty(properties["Characteristics"], "Characteristics")?;
        }
        _ => unreachable!("family guard is exhaustive"),
    }
    Ok(())
}

fn project_shared_forms_and_references(
    parts: &mut CanonicalObjectParts,
    properties: &BTreeMap<&str, &XmlElement>,
    uris: &ResolvedNamespaces,
    exchange: bool,
) -> Result<(), MetadataDecodeError> {
    for name in [
        "DefaultObjectForm",
        "DefaultListForm",
        "DefaultChoiceForm",
        "AuxiliaryObjectForm",
        "AuxiliaryListForm",
        "AuxiliaryChoiceForm",
    ] {
        push_text(parts, properties, name)?;
    }
    push_reference_collection(parts, properties, "BasedOn", uris)?;
    push_field_collection(parts, properties, "InputByString", uris)?;
    push_field_collection(parts, properties, "DataLockFields", uris)?;
    for name in [
        "ObjectPresentation",
        "ExtendedObjectPresentation",
        "ListPresentation",
        "ExtendedListPresentation",
        "Explanation",
    ] {
        push_localized(parts, properties, name, uris)?;
    }
    if exchange {
        // Kept as an explicit branch so additions cannot accidentally change
        // the stable canonical order for the other workflow families.
    }
    Ok(())
}

fn project_picture(
    parts: &mut CanonicalObjectParts,
    picture: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let mut reference = None;
    let mut transparent = None;
    for node in picture.children() {
        let XmlNode::Element(element) = node else {
            continue;
        };
        if typed(element, "Ref", Some(XR_NAMESPACE), uris) && reference.is_none() {
            reference = element_text(element)?;
        } else if typed(element, "LoadTransparent", Some(XR_NAMESPACE), uris)
            && transparent.is_none()
        {
            transparent = Some(match element_text(element)?.as_deref() {
                Some("true") => true,
                Some("false") => false,
                _ => {
                    return Err(MetadataDecodeError::InvalidEnvelope(
                        "Subsystem Picture transparency is not canonical",
                    ));
                }
            });
        } else {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "Subsystem Picture shape is not exact",
            ));
        }
    }
    parts.properties.push(canonical_field(
        "Picture",
        CanonicalValue::text(canonical_text(reference.as_deref().unwrap_or(""))?),
    )?);
    parts.properties.push(canonical_field(
        "PictureLoadTransparent",
        CanonicalValue::boolean(transparent.unwrap_or(false)),
    )?);
    Ok(())
}

fn push_internal_this_node(
    parts: &mut CanonicalObjectParts,
    object: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let mut value = None;
    for node in object.children() {
        let XmlNode::Element(section) = node else {
            continue;
        };
        if !typed(section, "InternalInfo", Some(MD_NAMESPACE), uris) {
            continue;
        }
        for child in section.children() {
            if let XmlNode::Element(element) = child
                && typed(element, "ThisNode", Some(XR_NAMESPACE), uris)
            {
                if value.is_some() {
                    return Err(MetadataDecodeError::Duplicate("ThisNode"));
                }
                value = element_text(element)?;
            }
        }
    }
    let value = value.ok_or(MetadataDecodeError::Missing("ThisNode"))?;
    ObjectUuid::parse(&value).map_err(|_| MetadataDecodeError::InvalidUuid(value.clone()))?;
    parts.properties.push(canonical_field(
        "ThisNode",
        CanonicalValue::text(canonical_text(&value)?),
    )?);
    Ok(())
}

fn project_subsystem_children(
    parts: &mut CanonicalObjectParts,
    _owner_name: &str,
    children: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let mut values = Vec::new();
    let mut seen = BTreeSet::new();
    for node in children.children() {
        let XmlNode::Element(element) = node else {
            continue;
        };
        if !typed(element, SUBSYSTEM, Some(MD_NAMESPACE), uris) || !element.attributes().is_empty()
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "Subsystem child reference is not exact",
            ));
        }
        let name = element_text(element)?.ok_or(MetadataDecodeError::InvalidEnvelope(
            "Subsystem child reference is empty",
        ))?;
        if name.is_empty() || name.contains('.') || !seen.insert(name.to_lowercase()) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "Subsystem child reference name is invalid or duplicated",
            ));
        }
        let target = format!("Subsystem.{name}");
        values.push(CanonicalValue::reference(
            UnresolvedReference::new("metadata", &target)
                .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
        ));
    }
    parts.properties.push(canonical_field(
        "ChildSubsystems",
        CanonicalValue::sequence(values)
            .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
    )?);
    Ok(())
}

fn project_addressing_attribute(
    parts: &mut CanonicalObjectParts,
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let properties = required_properties(element, uris)?;
    let map = exact_property_map(properties, ADDRESSING_ATTRIBUTE_PROPERTIES, uris)?;
    push_text(parts, &map, "Comment")?;
    project_type(parts, map["Type"], uris)?;
    for name in [
        "PasswordMode",
        "MarkNegatives",
        "MultiLine",
        "ExtendedEdit",
        "FillFromFillingValue",
    ] {
        push_bool(parts, &map, name)?;
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
    for name in ["Mask", "ChoiceForm", "AddressingDimension"] {
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
        require_empty(map[name], name)?;
    }
    Ok(())
}

fn project_workflow_tabular(
    parts: &mut CanonicalObjectParts,
    element: &XmlElement,
    family: &str,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let properties = required_properties(element, uris)?;
    let expected = if family == EXCHANGE_PLAN {
        EXCHANGE_TABULAR_PROPERTIES
    } else {
        BUSINESS_PROCESS_TABULAR_PROPERTIES
    };
    let map = exact_property_map(properties, expected, uris)?;
    push_text(parts, &map, "Comment")?;
    require_empty(map["ToolTip"], "ToolTip")?;
    push_enum(parts, &map, "FillChecking")?;
    if family == EXCHANGE_PLAN {
        validate_supported_standard_attributes(map["StandardAttributes"], &["LineNumber"], uris)?;
    }
    push_u32(parts, &map, "LineNumberLength")?;
    Ok(())
}

fn validate_supported_standard_attributes(
    container: &XmlElement,
    expected_names: &[&str],
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    if !container
        .children()
        .iter()
        .any(|node| matches!(node, XmlNode::Element(_)))
    {
        return require_empty(container, "StandardAttributes");
    }
    validate_standard_attributes(container, expected_names, uris)
}

fn encode_hierarchical_object(
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
    let source = decode_hierarchical_object(
        envelope.source_document(),
        source_profile,
        path.clone(),
        family,
    )
    .map_err(decode_to_encode)?;
    if source.root() != envelope.root() || source.descendants() != envelope.descendants() {
        return Err(invalid_model(
            &path,
            "hierarchical object semantic mutation is not implemented",
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
    XmlWriter::to_vec(
        &envelope.source_document().with_root(root),
        LexicalPolicy::Preserve,
    )
    .map_err(|error| MetadataEncodeError::Xml(error.to_string()))
}
