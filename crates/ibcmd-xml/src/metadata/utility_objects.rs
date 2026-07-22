//! Strict XCF codecs for report/processor/enum/settings metadata.

use std::collections::BTreeMap;

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::ObjectPath;
use ibcmd_core::family::FamilyId;
use ibcmd_core::model::{CanonicalObject, CanonicalObjectParts};
use ibcmd_core::value::CanonicalValue;

use super::business_objects::{
    collect_embedded_elements, exact_object_sections, exact_property_map, only_element_child,
    project_command, project_name_only_children, project_type, push_bool, push_enum,
    push_localized, push_text, require_empty, required_properties, text_field,
};
use super::common::decode_metadata_envelope_with_child_references;
use super::common::{
    MD_NAMESPACE, MetadataDecodeError, MetadataEnvelope, ResolvedNamespaces, XR_NAMESPACE,
    element_text, resolve_namespaces, typed, uri_of,
};
use super::language::{
    canonical_field, copy_object_parts, decode_to_encode, invalid_model, profile_version,
    root_version, set_unprefixed_attribute, validate_decode_profile,
};
use super::registry::{
    MetadataEncodeError, MetadataFamilyCodec, MetadataRegistry, MetadataRegistryError,
};
use crate::{AttributeKind, LexicalPolicy, XmlDocument, XmlElement, XmlNode, XmlWriter};

const REPORT: &str = "Report";
const DATA_PROCESSOR: &str = "DataProcessor";
const ENUM: &str = "Enum";
const SETTINGS_STORAGE: &str = "SettingsStorage";

const REPORT_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "DefaultForm",
    "AuxiliaryForm",
    "MainDataCompositionSchema",
    "DefaultSettingsForm",
    "AuxiliarySettingsForm",
    "DefaultVariantForm",
    "VariantsStorage",
    "SettingsStorage",
    "IncludeHelpInContents",
    "ExtendedPresentation",
    "Explanation",
];

const DATA_PROCESSOR_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "DefaultForm",
    "AuxiliaryForm",
    "IncludeHelpInContents",
    "ExtendedPresentation",
    "Explanation",
];

const ENUM_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "StandardAttributes",
    "Characteristics",
    "QuickChoice",
    "ChoiceMode",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "ChoiceHistoryOnInput",
];

const SETTINGS_STORAGE_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "DefaultSaveForm",
    "DefaultLoadForm",
    "AuxiliarySaveForm",
    "AuxiliaryLoadForm",
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
    "FillChecking",
    "ChoiceFoldersAndItems",
    "ChoiceParameterLinks",
    "ChoiceParameters",
    "QuickChoice",
    "CreateOnInput",
    "ChoiceForm",
    "LinkByType",
    "ChoiceHistoryOnInput",
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
];

const TABULAR_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "ToolTip",
    "FillChecking",
    "StandardAttributes",
];

const STANDARD_ATTRIBUTE_PROPERTIES: &[&str] = &[
    "LinkByType",
    "FillChecking",
    "MultiLine",
    "FillFromFillingValue",
    "CreateOnInput",
    "TypeReductionMode",
    "MaxValue",
    "ToolTip",
    "ExtendedEdit",
    "Format",
    "ChoiceForm",
    "QuickChoice",
    "ChoiceHistoryOnInput",
    "EditFormat",
    "PasswordMode",
    "DataHistory",
    "MarkNegatives",
    "MinValue",
    "Synonym",
    "Comment",
    "FullTextSearch",
    "ChoiceParameterLinks",
    "FillValue",
    "Mask",
    "ChoiceParameters",
];

pub fn register_report_codec(registry: &mut MetadataRegistry) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(UtilityObjectCodec::new(REPORT)))
}

pub fn register_data_processor_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(UtilityObjectCodec::new(DATA_PROCESSOR)))
}

pub fn register_enum_codec(registry: &mut MetadataRegistry) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(UtilityObjectCodec::new(ENUM)))
}

pub fn register_settings_storage_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(UtilityObjectCodec::new(SETTINGS_STORAGE)))
}

struct UtilityObjectCodec {
    family: FamilyId,
}

impl UtilityObjectCodec {
    fn new(family: &str) -> Self {
        Self {
            family: FamilyId::parse(family).expect("utility metadata family literal is valid"),
        }
    }
}

impl MetadataFamilyCodec for UtilityObjectCodec {
    fn family_id(&self) -> &FamilyId {
        &self.family
    }

    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        decode_utility_object(document, source, path, self.family.as_str())
    }

    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        encode_utility_object(envelope, target, self.family.as_str())
    }
}

fn decode_utility_object(
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
    if generic.root().kind().as_str() != family
        || !matches!(family, REPORT | DATA_PROCESSOR | ENUM | SETTINGS_STORAGE)
    {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "utility metadata codec family differs from XML",
        ));
    }

    let uris = resolve_namespaces(document.root())?;
    let expected = uri_of(document.root(), &uris);
    if expected != Some(MD_NAMESPACE) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "utility metadata requires the MDClasses namespace",
        ));
    }
    let object = only_element_child(document.root(), "metadata object")?;
    if !typed(object, family, expected, &uris) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "utility metadata element is not exact",
        ));
    }
    let sections = exact_object_sections(object, family, &uris)?;
    let property_names = match family {
        REPORT => REPORT_PROPERTIES,
        DATA_PROCESSOR => DATA_PROCESSOR_PROPERTIES,
        ENUM => ENUM_PROPERTIES,
        SETTINGS_STORAGE => SETTINGS_STORAGE_PROPERTIES,
        _ => unreachable!(),
    };
    let properties = exact_property_map(sections.properties, property_names, &uris)?;

    let mut root_parts = copy_object_parts(generic.root());
    project_root_properties(&mut root_parts, family, &properties, &uris)?;
    project_name_only_children(
        &mut root_parts,
        family,
        text_field(&properties, "Name")?,
        sections.children,
        &uris,
    )?;
    let root = CanonicalObject::new(root_parts)
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))?;

    let element_by_uuid = collect_embedded_elements(sections.children, &uris)?;
    if element_by_uuid.len() != generic.descendants().len() {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "utility metadata descendant inventory is not exact",
        ));
    }
    let mut descendants = Vec::with_capacity(generic.descendants().len());
    for descendant in generic.descendants() {
        let element = element_by_uuid.get(&descendant.identity().uuid()).ok_or(
            MetadataDecodeError::InvalidEnvelope("utility descendant has no XML element"),
        )?;
        let mut parts = copy_object_parts(descendant);
        match (family, descendant.kind().as_str()) {
            (REPORT | DATA_PROCESSOR, "Attribute") => {
                project_attribute(&mut parts, element, root.identity().uuid(), &uris)?
            }
            (REPORT | DATA_PROCESSOR, "TabularSection") => {
                project_tabular_section(&mut parts, element, &uris)?
            }
            (REPORT | DATA_PROCESSOR, "Command")
                if descendant.owner() == Some(root.identity().uuid()) =>
            {
                project_command(&mut parts, element, &uris)?
            }
            (ENUM, "EnumValue") if descendant.owner() == Some(root.identity().uuid()) => {
                project_enum_value(&mut parts, element, &uris)?
            }
            _ => {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "utility metadata contains an unsupported embedded child",
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
    properties: &BTreeMap<&str, &XmlElement>,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    push_text(parts, properties, "Comment")?;
    match family {
        REPORT => {
            push_bool(parts, properties, "UseStandardCommands")?;
            for name in [
                "DefaultForm",
                "AuxiliaryForm",
                "MainDataCompositionSchema",
                "DefaultSettingsForm",
                "AuxiliarySettingsForm",
                "DefaultVariantForm",
                "VariantsStorage",
                "SettingsStorage",
            ] {
                push_text(parts, properties, name)?;
            }
            push_bool(parts, properties, "IncludeHelpInContents")?;
            push_localized(parts, properties, "ExtendedPresentation", uris)?;
            push_localized(parts, properties, "Explanation", uris)?;
        }
        DATA_PROCESSOR => {
            push_bool(parts, properties, "UseStandardCommands")?;
            push_text(parts, properties, "DefaultForm")?;
            push_text(parts, properties, "AuxiliaryForm")?;
            push_bool(parts, properties, "IncludeHelpInContents")?;
            push_localized(parts, properties, "ExtendedPresentation", uris)?;
            push_localized(parts, properties, "Explanation", uris)?;
        }
        ENUM => {
            push_bool(parts, properties, "UseStandardCommands")?;
            validate_standard_attributes(
                properties["StandardAttributes"],
                &["Order", "Ref"],
                uris,
            )?;
            parts.properties.push(canonical_field(
                "HasStandardAttributes",
                CanonicalValue::boolean(true),
            )?);
            require_empty(properties["Characteristics"], "Characteristics")?;
            push_bool(parts, properties, "QuickChoice")?;
            push_enum(parts, properties, "ChoiceMode")?;
            for name in [
                "DefaultListForm",
                "DefaultChoiceForm",
                "AuxiliaryListForm",
                "AuxiliaryChoiceForm",
            ] {
                push_text(parts, properties, name)?;
            }
            for name in [
                "ListPresentation",
                "ExtendedListPresentation",
                "Explanation",
            ] {
                push_localized(parts, properties, name, uris)?;
            }
            push_enum(parts, properties, "ChoiceHistoryOnInput")?;
        }
        SETTINGS_STORAGE => {
            for name in [
                "DefaultSaveForm",
                "DefaultLoadForm",
                "AuxiliarySaveForm",
                "AuxiliaryLoadForm",
            ] {
                push_text(parts, properties, name)?;
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn project_attribute(
    parts: &mut CanonicalObjectParts,
    element: &XmlElement,
    root_uuid: ibcmd_core::identity::ObjectUuid,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let properties = required_properties(element, uris)?;
    let nested = parts.owner.is_some_and(|owner| owner != root_uuid);
    let expected = if nested {
        NESTED_ATTRIBUTE_PROPERTIES
    } else {
        ATTRIBUTE_PROPERTIES
    };
    let map = exact_property_map(properties, expected, uris)?;
    push_text(parts, &map, "Comment")?;
    project_type(parts, map["Type"], uris)?;
    push_bool(parts, &map, "PasswordMode")?;
    push_localized(parts, &map, "ToolTip", uris)?;
    for name in ["MarkNegatives", "MultiLine", "ExtendedEdit"] {
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
    ] {
        push_enum(parts, &map, name)?;
    }
    push_text(parts, &map, "Mask")?;
    push_text(parts, &map, "ChoiceForm")?;
    for name in [
        "Format",
        "EditFormat",
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

fn project_tabular_section(
    parts: &mut CanonicalObjectParts,
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let properties = required_properties(element, uris)?;
    let map = exact_property_map(properties, TABULAR_PROPERTIES, uris)?;
    push_text(parts, &map, "Comment")?;
    push_localized(parts, &map, "ToolTip", uris)?;
    push_enum(parts, &map, "FillChecking")?;
    validate_standard_attributes(map["StandardAttributes"], &["LineNumber"], uris)?;
    parts.properties.push(canonical_field(
        "HasLineNumberStandardAttribute",
        CanonicalValue::boolean(true),
    )?);
    Ok(())
}

fn project_enum_value(
    parts: &mut CanonicalObjectParts,
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let properties = required_properties(element, uris)?;
    let elements = properties
        .children()
        .iter()
        .filter_map(|node| match node {
            XmlNode::Element(element) => Some(element),
            _ => None,
        })
        .collect::<Vec<_>>();
    let names: &[&str] = if elements.len() == 4 {
        &["Name", "Synonym", "Comment", "Color"]
    } else {
        &["Name", "Synonym", "Comment"]
    };
    let map = exact_property_map(properties, names, uris)?;
    push_text(parts, &map, "Comment")?;
    if let Some(color) = map.get("Color") {
        if element_text(color)?.as_deref() != Some("auto") {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "EnumValue Color is not the evidenced auto value",
            ));
        }
        push_enum(parts, &map, "Color")?;
    }
    Ok(())
}

pub(super) fn validate_standard_attributes(
    container: &XmlElement,
    expected_names: &[&str],
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let attributes = container
        .children()
        .iter()
        .filter_map(|node| match node {
            XmlNode::Element(element) => Some(element),
            _ => None,
        })
        .collect::<Vec<_>>();
    if attributes.len() != expected_names.len() {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "standard attribute inventory is not exact",
        ));
    }
    for (attribute, expected_name) in attributes.into_iter().zip(expected_names) {
        if !typed(attribute, "StandardAttribute", Some(XR_NAMESPACE), uris)
            || ordinary_attribute(attribute, "name") != Some(*expected_name)
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "standard attribute identity is not exact",
            ));
        }
        let properties = exact_namespaced_property_map(
            attribute,
            STANDARD_ATTRIBUTE_PROPERTIES,
            XR_NAMESPACE,
            uris,
        )?;
        for (name, expected) in [
            ("FillChecking", "DontCheck"),
            ("MultiLine", "false"),
            ("FillFromFillingValue", "false"),
            ("CreateOnInput", "Auto"),
            ("TypeReductionMode", "TransformValues"),
            ("ExtendedEdit", "false"),
            ("QuickChoice", "Auto"),
            ("ChoiceHistoryOnInput", "Auto"),
            ("PasswordMode", "false"),
            ("DataHistory", "Use"),
            ("MarkNegatives", "false"),
            ("FullTextSearch", "Use"),
        ] {
            if element_text(properties[name])?.as_deref() != Some(expected) {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "standard attribute scalar value is not evidenced",
                ));
            }
        }
        for name in [
            "LinkByType",
            "MaxValue",
            "ToolTip",
            "Format",
            "ChoiceForm",
            "EditFormat",
            "MinValue",
            "Synonym",
            "Comment",
            "ChoiceParameterLinks",
            "FillValue",
            "Mask",
            "ChoiceParameters",
        ] {
            require_empty(properties[name], name)?;
        }
    }
    Ok(())
}

fn exact_namespaced_property_map<'a>(
    parent: &'a XmlElement,
    expected_names: &[&'static str],
    namespace: &str,
    uris: &ResolvedNamespaces,
) -> Result<BTreeMap<&'static str, &'a XmlElement>, MetadataDecodeError> {
    let elements = parent
        .children()
        .iter()
        .filter_map(|node| match node {
            XmlNode::Element(element) => Some(element),
            _ => None,
        })
        .collect::<Vec<_>>();
    if elements.len() != expected_names.len() {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "namespaced property inventory is not exact",
        ));
    }
    let mut output = BTreeMap::new();
    for (element, expected) in elements.into_iter().zip(expected_names) {
        if !typed(element, expected, Some(namespace), uris) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "namespaced property order is not exact",
            ));
        }
        output.insert(*expected, element);
    }
    Ok(output)
}

fn ordinary_attribute<'a>(element: &'a XmlElement, local: &str) -> Option<&'a str> {
    let mut value = None;
    for attribute in element.attributes() {
        if let AttributeKind::Ordinary(name) = attribute.kind()
            && name.prefix().is_none()
            && name.local() == local
        {
            if value.is_some() {
                return None;
            }
            value = Some(attribute.value());
        } else if !matches!(attribute.kind(), AttributeKind::Namespace(_)) {
            return None;
        }
    }
    value
}

fn encode_utility_object(
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
    let source = decode_utility_object(
        envelope.source_document(),
        source_profile,
        path.clone(),
        family,
    )
    .map_err(decode_to_encode)?;
    if source.root() != envelope.root() || source.descendants() != envelope.descendants() {
        return Err(invalid_model(
            &path,
            "utility metadata semantic mutation is not implemented",
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
