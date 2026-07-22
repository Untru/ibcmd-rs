//! Strict XCF codecs for registers, recalculations, and chart metadata.
//!
//! The supported profile is the normalized full 2.20/2.21 representation.
//! Compact chart listing files omit semantic properties and therefore fail
//! closed instead of receiving guessed defaults.

use std::collections::{BTreeMap, BTreeSet};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::ObjectPath;
use ibcmd_core::family::FamilyId;
use ibcmd_core::model::{CanonicalObject, CanonicalObjectParts};
use ibcmd_core::value::{CanonicalValue, UnresolvedReference};

use super::business_objects::{
    collect_embedded_elements, exact_object_sections, exact_property_map, only_element_child,
    project_name_only_children, project_type, push_bool, push_enum, push_field_collection,
    push_localized, push_reference_collection, push_text, push_u32, require_empty,
    required_properties, text_field,
};
use super::common::{
    MD_NAMESPACE, MetadataDecodeError, MetadataEnvelope, ResolvedNamespaces,
    decode_metadata_envelope_with_child_references, element_text, resolve_namespaces, typed,
    uri_of,
};
use super::language::{
    canonical_field, copy_object_parts, decode_to_encode, invalid_model, profile_version,
    root_version, set_unprefixed_attribute, validate_decode_profile,
};
use super::registry::{
    MetadataEncodeError, MetadataFamilyCodec, MetadataRegistry, MetadataRegistryError,
};
use crate::{LexicalPolicy, XmlDocument, XmlElement, XmlNode, XmlWriter};

pub const INFORMATION_REGISTER: &str = "InformationRegister";
pub const ACCUMULATION_REGISTER: &str = "AccumulationRegister";
pub const ACCOUNTING_REGISTER: &str = "AccountingRegister";
pub const CALCULATION_REGISTER: &str = "CalculationRegister";
pub const RECALCULATION: &str = "Recalculation";
pub const CHART_OF_CHARACTERISTIC_TYPES: &str = "ChartOfCharacteristicTypes";
pub const CHART_OF_ACCOUNTS: &str = "ChartOfAccounts";
pub const CHART_OF_CALCULATION_TYPES: &str = "ChartOfCalculationTypes";

const INFORMATION_REGISTER_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "EditType",
    "DefaultRecordForm",
    "DefaultListForm",
    "AuxiliaryRecordForm",
    "AuxiliaryListForm",
    "StandardAttributes",
    "InformationRegisterPeriodicity",
    "WriteMode",
    "MainFilterOnPeriod",
    "IncludeHelpInContents",
    "DataLockControlMode",
    "FullTextSearch",
    "EnableTotalsSliceFirst",
    "EnableTotalsSliceLast",
    "RecordPresentation",
    "ExtendedRecordPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "DataHistory",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
];

const ACCUMULATION_REGISTER_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "DefaultListForm",
    "AuxiliaryListForm",
    "RegisterType",
    "IncludeHelpInContents",
    "StandardAttributes",
    "DataLockControlMode",
    "FullTextSearch",
    "EnableTotalsSplitting",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
];

const ACCOUNTING_REGISTER_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "IncludeHelpInContents",
    "BasedOn",
    "ChartOfAccounts",
    "Correspondence",
    "PeriodAdjustmentLength",
    "DefaultListForm",
    "AuxiliaryListForm",
    "StandardAttributes",
    "DataLockControlMode",
    "EnableTotalsSplitting",
    "FullTextSearch",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
];

const CALCULATION_REGISTER_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "DefaultListForm",
    "AuxiliaryListForm",
    "Periodicity",
    "ActionPeriod",
    "BasePeriod",
    "Schedule",
    "ScheduleValue",
    "ScheduleDate",
    "ChartOfCalculationTypes",
    "IncludeHelpInContents",
    "StandardAttributes",
    "DataLockControlMode",
    "FullTextSearch",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
];

const RECALCULATION_PROPERTIES: &[&str] = &["Name", "Synonym", "Comment", "DataLockControlMode"];

const CCT_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "IncludeHelpInContents",
    "CharacteristicExtValues",
    "Type",
    "Hierarchical",
    "FoldersOnTop",
    "CodeLength",
    "CodeAllowedLength",
    "DescriptionLength",
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
    "CreateOnInput",
    "SearchStringModeOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceHistoryOnInput",
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
    "BasedOn",
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

const COA_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "IncludeHelpInContents",
    "BasedOn",
    "ExtDimensionTypes",
    "MaxExtDimensionCount",
    "CodeMask",
    "CodeLength",
    "DescriptionLength",
    "CodeSeries",
    "CheckUnique",
    "DefaultPresentation",
    "StandardAttributes",
    "Characteristics",
    "StandardTabularSections",
    "PredefinedDataUpdate",
    "EditType",
    "QuickChoice",
    "ChoiceMode",
    "InputByString",
    "SearchStringModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "CreateOnInput",
    "ChoiceHistoryOnInput",
    "DefaultObjectForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "AutoOrderByCode",
    "OrderLength",
    "DataLockFields",
    "DataLockControlMode",
    "FullTextSearch",
    "DataHistory",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
];

const COT_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "CodeLength",
    "DescriptionLength",
    "CodeType",
    "CodeAllowedLength",
    "DefaultPresentation",
    "EditType",
    "QuickChoice",
    "ChoiceMode",
    "InputByString",
    "SearchStringModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "CreateOnInput",
    "ChoiceHistoryOnInput",
    "DefaultObjectForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "BasedOn",
    "DependenceOnCalculationTypes",
    "BaseCalculationTypes",
    "ActionPeriodUse",
    "StandardAttributes",
    "Characteristics",
    "StandardTabularSections",
    "PredefinedDataUpdate",
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

const RECALCULATION_DIMENSION_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "RegisterDimension",
    "LeadingRegisterData",
];

pub fn register_information_register_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    register(registry, INFORMATION_REGISTER)
}

pub fn register_accumulation_register_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    register(registry, ACCUMULATION_REGISTER)
}

pub fn register_accounting_register_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    register(registry, ACCOUNTING_REGISTER)
}

pub fn register_calculation_register_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    register(registry, CALCULATION_REGISTER)
}

pub fn register_recalculation_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    register(registry, RECALCULATION)
}

pub fn register_chart_of_characteristic_types_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    register(registry, CHART_OF_CHARACTERISTIC_TYPES)
}

pub fn register_chart_of_accounts_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    register(registry, CHART_OF_ACCOUNTS)
}

pub fn register_chart_of_calculation_types_codec(
    registry: &mut MetadataRegistry,
) -> Result<(), MetadataRegistryError> {
    register(registry, CHART_OF_CALCULATION_TYPES)
}

fn register(registry: &mut MetadataRegistry, family: &str) -> Result<(), MetadataRegistryError> {
    registry.register(Box::new(RegisterObjectCodec {
        family: FamilyId::parse(family).expect("register family literal is valid"),
    }))
}

struct RegisterObjectCodec {
    family: FamilyId,
}

impl MetadataFamilyCodec for RegisterObjectCodec {
    fn family_id(&self) -> &FamilyId {
        &self.family
    }

    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        decode_register_object(document, source, path, self.family.as_str())
    }

    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        encode_register_object(envelope, target, self.family.as_str())
    }
}

fn decode_register_object(
    document: &XmlDocument,
    source: ProfileId,
    path: ObjectPath,
    family: &str,
) -> Result<MetadataEnvelope, MetadataDecodeError> {
    validate_decode_profile(document, &source, &path)?;
    let references: &[&str] = match family {
        CALCULATION_REGISTER => &["Form", "Template", RECALCULATION],
        RECALCULATION => &[],
        _ => &["Form", "Template"],
    };
    let generic =
        decode_metadata_envelope_with_child_references(document, source, path, references)?;
    if generic.root().kind().as_str() != family || !is_supported_family(family) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "register codec family differs from XML",
        ));
    }

    let uris = resolve_namespaces(document.root())?;
    if uri_of(document.root(), &uris) != Some(MD_NAMESPACE) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "register object requires the MDClasses namespace",
        ));
    }
    let object = only_element_child(document.root(), "metadata object")?;
    if !typed(object, family, Some(MD_NAMESPACE), &uris) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "register object element is not exact",
        ));
    }
    let sections = exact_object_sections(object, family, &uris)?;
    let map = exact_property_map(sections.properties, property_schema(family), &uris)?;

    let mut root_parts = copy_object_parts(generic.root());
    project_root(&mut root_parts, family, &map, &uris)?;
    if family != RECALCULATION {
        project_name_only_children(
            &mut root_parts,
            family,
            text_field(&map, "Name")?,
            sections.children,
            &uris,
        )?;
    }
    if family == CALCULATION_REGISTER {
        project_recalculation_references(
            &mut root_parts,
            text_field(&map, "Name")?,
            sections.children,
            &uris,
        )?;
    }
    let root = CanonicalObject::new(root_parts)
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))?;

    let element_by_uuid = collect_embedded_elements(sections.children, &uris)?;
    if element_by_uuid.len() != generic.descendants().len() {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "register descendant inventory is not exact",
        ));
    }
    let mut descendants = Vec::with_capacity(generic.descendants().len());
    for descendant in generic.descendants() {
        let element = element_by_uuid.get(&descendant.identity().uuid()).ok_or(
            MetadataDecodeError::InvalidEnvelope("register descendant has no XML element"),
        )?;
        let mut parts = copy_object_parts(descendant);
        if family == RECALCULATION && descendant.kind().as_str() == "Dimension" {
            project_recalculation_dimension(&mut parts, element, &uris)?;
        } else {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "register layout contains an unsupported embedded child",
            ));
        }
        descendants.push(
            CanonicalObject::new(parts)
                .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
        );
    }
    MetadataEnvelope::from_parts(root, descendants, document.clone())
}

fn is_supported_family(family: &str) -> bool {
    matches!(
        family,
        INFORMATION_REGISTER
            | ACCUMULATION_REGISTER
            | ACCOUNTING_REGISTER
            | CALCULATION_REGISTER
            | RECALCULATION
            | CHART_OF_CHARACTERISTIC_TYPES
            | CHART_OF_ACCOUNTS
            | CHART_OF_CALCULATION_TYPES
    )
}

fn property_schema(family: &str) -> &'static [&'static str] {
    match family {
        INFORMATION_REGISTER => INFORMATION_REGISTER_PROPERTIES,
        ACCUMULATION_REGISTER => ACCUMULATION_REGISTER_PROPERTIES,
        ACCOUNTING_REGISTER => ACCOUNTING_REGISTER_PROPERTIES,
        CALCULATION_REGISTER => CALCULATION_REGISTER_PROPERTIES,
        RECALCULATION => RECALCULATION_PROPERTIES,
        CHART_OF_CHARACTERISTIC_TYPES => CCT_PROPERTIES,
        CHART_OF_ACCOUNTS => COA_PROPERTIES,
        CHART_OF_CALCULATION_TYPES => COT_PROPERTIES,
        _ => &[],
    }
}

fn project_root(
    parts: &mut CanonicalObjectParts,
    family: &str,
    map: &BTreeMap<&str, &XmlElement>,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    push_text(parts, map, "Comment")?;
    match family {
        INFORMATION_REGISTER => {
            for name in [
                "UseStandardCommands",
                "MainFilterOnPeriod",
                "IncludeHelpInContents",
                "EnableTotalsSliceFirst",
                "EnableTotalsSliceLast",
                "UpdateDataHistoryImmediatelyAfterWrite",
                "ExecuteAfterWriteDataHistoryVersionProcessing",
            ] {
                push_bool(parts, map, name)?;
            }
            for name in [
                "EditType",
                "InformationRegisterPeriodicity",
                "WriteMode",
                "DataLockControlMode",
                "FullTextSearch",
                "DataHistory",
            ] {
                push_enum(parts, map, name)?;
            }
            push_forms(
                parts,
                map,
                &[
                    "DefaultRecordForm",
                    "DefaultListForm",
                    "AuxiliaryRecordForm",
                    "AuxiliaryListForm",
                ],
            )?;
            require_empty(map["StandardAttributes"], "StandardAttributes")?;
            push_presentations(
                parts,
                map,
                uris,
                &[
                    "RecordPresentation",
                    "ExtendedRecordPresentation",
                    "ListPresentation",
                    "ExtendedListPresentation",
                    "Explanation",
                ],
            )?;
        }
        ACCUMULATION_REGISTER => {
            for name in [
                "UseStandardCommands",
                "IncludeHelpInContents",
                "EnableTotalsSplitting",
            ] {
                push_bool(parts, map, name)?;
            }
            for name in ["RegisterType", "DataLockControlMode", "FullTextSearch"] {
                push_enum(parts, map, name)?;
            }
            push_forms(parts, map, &["DefaultListForm", "AuxiliaryListForm"])?;
            require_empty(map["StandardAttributes"], "StandardAttributes")?;
            push_presentations(
                parts,
                map,
                uris,
                &[
                    "ListPresentation",
                    "ExtendedListPresentation",
                    "Explanation",
                ],
            )?;
        }
        ACCOUNTING_REGISTER => {
            for name in [
                "UseStandardCommands",
                "IncludeHelpInContents",
                "Correspondence",
                "EnableTotalsSplitting",
            ] {
                push_bool(parts, map, name)?;
            }
            push_u32(parts, map, "PeriodAdjustmentLength")?;
            for name in ["DataLockControlMode", "FullTextSearch"] {
                push_enum(parts, map, name)?;
            }
            push_forms(
                parts,
                map,
                &["ChartOfAccounts", "DefaultListForm", "AuxiliaryListForm"],
            )?;
            require_empty(map["BasedOn"], "BasedOn")?;
            require_empty(map["StandardAttributes"], "StandardAttributes")?;
            push_presentations(
                parts,
                map,
                uris,
                &[
                    "ListPresentation",
                    "ExtendedListPresentation",
                    "Explanation",
                ],
            )?;
        }
        CALCULATION_REGISTER => {
            for name in [
                "UseStandardCommands",
                "ActionPeriod",
                "BasePeriod",
                "IncludeHelpInContents",
            ] {
                push_bool(parts, map, name)?;
            }
            for name in ["Periodicity", "DataLockControlMode", "FullTextSearch"] {
                push_enum(parts, map, name)?;
            }
            push_forms(
                parts,
                map,
                &[
                    "DefaultListForm",
                    "AuxiliaryListForm",
                    "Schedule",
                    "ScheduleValue",
                    "ScheduleDate",
                    "ChartOfCalculationTypes",
                ],
            )?;
            require_empty(map["StandardAttributes"], "StandardAttributes")?;
            push_presentations(
                parts,
                map,
                uris,
                &[
                    "ListPresentation",
                    "ExtendedListPresentation",
                    "Explanation",
                ],
            )?;
        }
        RECALCULATION => push_enum(parts, map, "DataLockControlMode")?,
        CHART_OF_CHARACTERISTIC_TYPES => project_cct(parts, map, uris)?,
        CHART_OF_ACCOUNTS => project_coa(parts, map, uris)?,
        CHART_OF_CALCULATION_TYPES => project_cot(parts, map, uris)?,
        _ => unreachable!("family guard is exhaustive"),
    }
    Ok(())
}

fn project_cct(
    parts: &mut CanonicalObjectParts,
    map: &BTreeMap<&str, &XmlElement>,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    for name in [
        "UseStandardCommands",
        "IncludeHelpInContents",
        "Hierarchical",
        "FoldersOnTop",
        "CheckUnique",
        "Autonumbering",
        "QuickChoice",
        "UpdateDataHistoryImmediatelyAfterWrite",
        "ExecuteAfterWriteDataHistoryVersionProcessing",
    ] {
        push_bool(parts, map, name)?;
    }
    for name in ["CodeLength", "DescriptionLength"] {
        push_u32(parts, map, name)?;
    }
    for name in [
        "CodeAllowedLength",
        "CodeSeries",
        "DefaultPresentation",
        "PredefinedDataUpdate",
        "EditType",
        "ChoiceMode",
        "CreateOnInput",
        "SearchStringModeOnInputByString",
        "ChoiceDataGetModeOnInputByString",
        "FullTextSearchOnInputByString",
        "ChoiceHistoryOnInput",
        "DataLockControlMode",
        "FullTextSearch",
        "DataHistory",
    ] {
        push_enum(parts, map, name)?;
    }
    push_forms(
        parts,
        map,
        &[
            "CharacteristicExtValues",
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
        ],
    )?;
    project_type(parts, map["Type"], uris)?;
    push_field_collection(parts, map, "InputByString", uris)?;
    for name in [
        "StandardAttributes",
        "Characteristics",
        "BasedOn",
        "DataLockFields",
    ] {
        require_empty(map[name], name)?;
    }
    push_presentations(
        parts,
        map,
        uris,
        &[
            "ObjectPresentation",
            "ExtendedObjectPresentation",
            "ListPresentation",
            "ExtendedListPresentation",
            "Explanation",
        ],
    )
}

fn project_coa(
    parts: &mut CanonicalObjectParts,
    map: &BTreeMap<&str, &XmlElement>,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    for name in [
        "UseStandardCommands",
        "IncludeHelpInContents",
        "CheckUnique",
        "QuickChoice",
        "AutoOrderByCode",
        "UpdateDataHistoryImmediatelyAfterWrite",
        "ExecuteAfterWriteDataHistoryVersionProcessing",
    ] {
        push_bool(parts, map, name)?;
    }
    for name in [
        "MaxExtDimensionCount",
        "CodeLength",
        "DescriptionLength",
        "OrderLength",
    ] {
        push_u32(parts, map, name)?;
    }
    for name in [
        "CodeSeries",
        "DefaultPresentation",
        "PredefinedDataUpdate",
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
        push_enum(parts, map, name)?;
    }
    push_forms(
        parts,
        map,
        &[
            "ExtDimensionTypes",
            "CodeMask",
            "DefaultObjectForm",
            "DefaultListForm",
            "DefaultChoiceForm",
            "AuxiliaryObjectForm",
            "AuxiliaryListForm",
            "AuxiliaryChoiceForm",
        ],
    )?;
    push_field_collection(parts, map, "InputByString", uris)?;
    for name in [
        "BasedOn",
        "StandardAttributes",
        "Characteristics",
        "StandardTabularSections",
        "DataLockFields",
    ] {
        require_empty(map[name], name)?;
    }
    push_presentations(
        parts,
        map,
        uris,
        &[
            "ObjectPresentation",
            "ExtendedObjectPresentation",
            "ListPresentation",
            "ExtendedListPresentation",
            "Explanation",
        ],
    )
}

fn project_cot(
    parts: &mut CanonicalObjectParts,
    map: &BTreeMap<&str, &XmlElement>,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    for name in [
        "UseStandardCommands",
        "QuickChoice",
        "ActionPeriodUse",
        "IncludeHelpInContents",
        "UpdateDataHistoryImmediatelyAfterWrite",
        "ExecuteAfterWriteDataHistoryVersionProcessing",
    ] {
        push_bool(parts, map, name)?;
    }
    for name in ["CodeLength", "DescriptionLength"] {
        push_u32(parts, map, name)?;
    }
    for name in [
        "CodeType",
        "CodeAllowedLength",
        "DefaultPresentation",
        "EditType",
        "ChoiceMode",
        "SearchStringModeOnInputByString",
        "FullTextSearchOnInputByString",
        "ChoiceDataGetModeOnInputByString",
        "CreateOnInput",
        "ChoiceHistoryOnInput",
        "DependenceOnCalculationTypes",
        "PredefinedDataUpdate",
        "DataLockControlMode",
        "FullTextSearch",
        "DataHistory",
    ] {
        push_enum(parts, map, name)?;
    }
    push_forms(
        parts,
        map,
        &[
            "DefaultObjectForm",
            "DefaultListForm",
            "DefaultChoiceForm",
            "AuxiliaryObjectForm",
            "AuxiliaryListForm",
            "AuxiliaryChoiceForm",
        ],
    )?;
    push_field_collection(parts, map, "InputByString", uris)?;
    push_reference_collection(parts, map, "BaseCalculationTypes", uris)?;
    for name in [
        "BasedOn",
        "StandardAttributes",
        "Characteristics",
        "StandardTabularSections",
        "DataLockFields",
    ] {
        require_empty(map[name], name)?;
    }
    push_presentations(
        parts,
        map,
        uris,
        &[
            "ObjectPresentation",
            "ExtendedObjectPresentation",
            "ListPresentation",
            "ExtendedListPresentation",
            "Explanation",
        ],
    )
}

fn push_forms(
    parts: &mut CanonicalObjectParts,
    map: &BTreeMap<&str, &XmlElement>,
    names: &[&str],
) -> Result<(), MetadataDecodeError> {
    for name in names {
        push_text(parts, map, name)?;
    }
    Ok(())
}

fn push_presentations(
    parts: &mut CanonicalObjectParts,
    map: &BTreeMap<&str, &XmlElement>,
    uris: &ResolvedNamespaces,
    names: &[&str],
) -> Result<(), MetadataDecodeError> {
    for name in names {
        push_localized(parts, map, name, uris)?;
    }
    Ok(())
}

fn project_recalculation_references(
    parts: &mut CanonicalObjectParts,
    owner_name: String,
    children: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let mut values = Vec::new();
    let mut seen = BTreeSet::new();
    for node in children.children() {
        let XmlNode::Element(element) = node else {
            continue;
        };
        if !typed(element, RECALCULATION, Some(MD_NAMESPACE), uris) {
            continue;
        }
        let name = element_text(element)?.ok_or(MetadataDecodeError::InvalidEnvelope(
            "CalculationRegister Recalculation reference is empty",
        ))?;
        if name.is_empty() || name.contains('.') || !seen.insert(name.to_lowercase()) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "CalculationRegister Recalculation reference is invalid",
            ));
        }
        let target = format!("CalculationRegister.{owner_name}.Recalculation.{name}");
        values.push(CanonicalValue::reference(
            UnresolvedReference::new("metadata", &target)
                .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
        ));
    }
    parts.properties.push(canonical_field(
        "ChildRecalculations",
        CanonicalValue::sequence(values)
            .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
    )?);
    Ok(())
}

fn project_recalculation_dimension(
    parts: &mut CanonicalObjectParts,
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let properties = required_properties(element, uris)?;
    let map = exact_property_map(properties, RECALCULATION_DIMENSION_PROPERTIES, uris)?;
    push_text(parts, &map, "Comment")?;
    push_text(parts, &map, "RegisterDimension")?;
    let expected = text_field(&map, "RegisterDimension")?;
    let mut actual = None;
    for node in map["LeadingRegisterData"].children() {
        let XmlNode::Element(item) = node else {
            continue;
        };
        if !typed(item, "Item", Some(super::common::XR_NAMESPACE), uris)
            || actual.replace(element_text(item)?).is_some()
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "Recalculation LeadingRegisterData is not exact",
            ));
        }
    }
    if actual.flatten().as_deref() != Some(expected.as_str()) {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "Recalculation leading data differs from RegisterDimension",
        ));
    }
    Ok(())
}

fn encode_register_object(
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
    let source = decode_register_object(
        envelope.source_document(),
        source_profile,
        path.clone(),
        family,
    )
    .map_err(decode_to_encode)?;
    if source.root() != envelope.root() || source.descendants() != envelope.descendants() {
        return Err(invalid_model(
            &path,
            "register semantic mutation is not implemented",
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
