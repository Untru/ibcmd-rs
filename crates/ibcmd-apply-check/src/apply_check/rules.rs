//! The rules: which changes of a descriptor the platform applies without
//! touching tables or stored data.
//!
//! The default is that a change does: whatever is not on a list here is a
//! restructuring, with its property path in the reason. A property is listed
//! only when both of these hold:
//! * the platform was seen to apply it without changing a table, a column, an
//!   index or a stored row: a native `config apply` of a database whose
//!   ConfigSave held the change (see `docs/apply/restructuring-check.md`,
//!   the probe tables), and
//! * it is presentation or behaviour by its meaning, not storage: a property
//!   the platform left alone in one probe but that names a way the data is
//!   kept (`HierarchyType`, the totals of a register) stays off the list.
//!
//! Kinds that own no stored data at all (a common module, a role, a
//! subsystem, a report) are the exception the other way: every change of
//! their descriptors is applied by copying rows.

use super::roles::Effect;
use super::rule_id::RuleId;
use super::tree_diff::{Change, ChangeOp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Safe,
    Structure,
    Data,
}

/// The verdict on one change and the rule that gave it.
#[derive(Debug, Clone, Copy)]
pub struct Decision {
    pub class: Class,
    pub rule: RuleId,
}

const fn safe(rule: RuleId) -> Decision {
    Decision {
        class: Class::Safe,
        rule,
    }
}

const fn structure(rule: RuleId) -> Decision {
    Decision {
        class: Class::Structure,
        rule,
    }
}

const fn data(rule: RuleId) -> Decision {
    Decision {
        class: Class::Data,
        rule,
    }
}

impl From<Effect> for Class {
    fn from(effect: Effect) -> Self {
        match effect {
            Effect::Safe => Class::Safe,
            Effect::Structure => Class::Structure,
            Effect::Data => Class::Data,
        }
    }
}

/// Kinds that own no stored data: every change of their descriptors is a
/// change of rows. Adding or removing one is too.
///
/// Probed on БСП 8.3.27 (every top-level property of at least one object of
/// each; `docs/apply/restructuring-check.md`).
const NO_STORAGE: &[&str] = &[
    "CommonModule",
    "CommonForm",
    "CommonTemplate",
    "CommonPicture",
    "CommonCommand",
    "CommandGroup",
    "Role",
    "Subsystem",
    "EventSubscription",
    "FunctionalOption",
    "FunctionalOptionsParameter",
    "StyleItem",
    "Style",
    "PaletteColor",
    "Interface",
    "FilterCriterion",
    "Report",
    "DataProcessor",
    "Form",
    "Template",
    "WebService",
    "HTTPService",
    "WSReference",
    "XDTOPackage",
    "IntegrationService",
    "Bot",
];

pub fn owns_no_stored_data(kind: &str) -> bool {
    NO_STORAGE.contains(&kind)
}

/// Properties that are presentation or behaviour on every kind that has
/// them.
const PRESENTATION: &[&str] = &[
    "Synonym",
    "Comment",
    "Explanation",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "IncludeHelpInContents",
    "DefaultObjectForm",
    "DefaultFolderForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "DefaultFolderChoiceForm",
    "DefaultRecordForm",
    "DefaultForm",
    "AuxiliaryObjectForm",
    "AuxiliaryFolderForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "AuxiliaryFolderChoiceForm",
    "QuickChoice",
    "ChoiceMode",
    "EditType",
    "CreateOnInput",
    "ChoiceHistoryOnInput",
    "DefaultPresentation",
    "SearchStringModeOnInputByString",
    "FullTextSearchOnInputByString",
    "DataLockControlMode",
    "FullTextSearch",
];

/// Properties of one kind that are safe besides [`PRESENTATION`], by probe.
const SAFE_BY_KIND: &[(&str, &[&str])] = &[
    (
        "Catalog",
        &[
            "UseStandardCommands",
            "Autonumbering",
            "CheckUnique",
            "InputByString",
            "PredefinedDataUpdate",
            "UpdateDataHistoryImmediatelyAfterWrite",
            "ExecuteAfterWriteDataHistoryVersionProcessing",
        ],
    ),
    (
        "Document",
        &[
            "UseStandardCommands",
            "Autonumbering",
            "PostInPrivilegedMode",
            "UnpostInPrivilegedMode",
            "RealTimePosting",
            "RegisterRecordsDeletion",
            "RegisterRecordsWritingOnPost",
            "SequenceFilling",
            "UpdateDataHistoryImmediatelyAfterWrite",
            "ExecuteAfterWriteDataHistoryVersionProcessing",
        ],
    ),
    ("Enum", &["UseStandardCommands"]),
    (
        "Constant",
        &[
            "UseStandardCommands",
            "ExtendedEdit",
            "FillChecking",
            "MarkNegatives",
            "MultiLine",
            "PasswordMode",
            "UpdateDataHistoryImmediatelyAfterWrite",
            "ExecuteAfterWriteDataHistoryVersionProcessing",
        ],
    ),
    (
        "InformationRegister",
        &[
            "UseStandardCommands",
            "UpdateDataHistoryImmediatelyAfterWrite",
            "ExecuteAfterWriteDataHistoryVersionProcessing",
        ],
    ),
    ("AccumulationRegister", &["UseStandardCommands"]),
    ("AccountingRegister", &["UseStandardCommands"]),
    ("CalculationRegister", &["UseStandardCommands"]),
    ("ChartOfAccounts", &["UseStandardCommands"]),
    ("ChartOfCharacteristicTypes", &["UseStandardCommands"]),
    ("ChartOfCalculationTypes", &["UseStandardCommands"]),
    ("ExchangePlan", &["UseStandardCommands"]),
    ("BusinessProcess", &["UseStandardCommands"]),
    ("Task", &["UseStandardCommands"]),
    // The standard commands of a document journal are the one place this
    // property was seen to make the platform rebuild a registration table.
    ("DocumentJournal", &[]),
    ("SessionParameter", &[]),
    // The rows of the scheduled jobs table are the platform's: every property
    // but the text is stored there (see `decide_property`).
    ("ScheduledJob", &[]),
    // Information about the configuration: probed one by one.
    (
        "Configuration",
        &[
            "Version",
            "Vendor",
            "UpdateCatalogAddress",
            "Copyright",
            "BriefInformation",
            "DetailedInformation",
            "VendorInformationAddress",
            "ConfigurationInformationAddress",
        ],
    ),
];

/// Properties of an attribute-like child (an attribute, a tabular section's
/// attribute, a dimension, a resource, a standard attribute) that are
/// presentation or behaviour: probed on catalog attributes.
const SAFE_ATTRIBUTE_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "ToolTip",
    "FillChecking",
    "QuickChoice",
    "ChoiceHistoryOnInput",
    "CreateOnInput",
    "MultiLine",
    "PasswordMode",
    "ExtendedEdit",
    "MarkNegatives",
    "FillFromFillingValue",
    "FullTextSearch",
    "ChoiceFoldersAndItems",
    "DenyIncompleteValues",
];

fn safe_property(kind: &str, tag: &str) -> bool {
    PRESENTATION.contains(&tag)
        || SAFE_BY_KIND
            .iter()
            .any(|(known, tags)| *known == kind && tags.contains(&tag))
}

/// Collections of child objects, and how a change to them is judged.
enum Child {
    /// Attribute-like: adding or removing one changes the table; a property
    /// is judged by [`SAFE_ATTRIBUTE_PROPERTIES`].
    Column,
    /// A tabular section: a table of its own.
    TabularSection,
    /// A value of an enumeration.
    EnumValue,
    /// Commands, forms and templates: rows and lists of names.
    Interface,
}

fn child_of(tag: &str) -> Option<Child> {
    Some(match tag {
        "Attribute"
        | "Dimension"
        | "Resource"
        | "AddressingAttribute"
        | "Column"
        | "AccountingFlag"
        | "ExtDimensionAccountingFlag"
        | "Parameter" => Child::Column,
        "TabularSection" => Child::TabularSection,
        "EnumValue" => Child::EnumValue,
        "Command" | "Form" | "Template" => Child::Interface,
        _ => return None,
    })
}

/// Judges one change of a descriptor of `kind`.
pub fn decide(kind: &str, change: &Change) -> Decision {
    let names = change.names();
    if owns_no_stored_data(kind) {
        return safe(RuleId::KindOwnsNoStoredData);
    }
    let Some(section) = names.first().copied() else {
        return structure(RuleId::ObjectChanged);
    };
    match section {
        "Properties" => decide_property(kind, &names[1..]),
        // The configuration lists its objects by kind: adding or dropping
        // one is what adding or dropping the object itself is.
        "ChildObjects" if kind == "Configuration" => match names.get(1).copied() {
            Some(listed_kind) => decide_lifecycle(listed_kind),
            None => structure(RuleId::ConfigurationObjectsChanged),
        },
        "ChildObjects" => decide_child(&names[1..], change),
        "InternalInfo" => structure(RuleId::GeneratedTypesChanged),
        _ => structure(RuleId::DescriptorPartNotCovered),
    }
}

fn decide_property(kind: &str, names: &[&str]) -> Decision {
    let Some(tag) = names.first().copied() else {
        return structure(RuleId::PropertiesChanged);
    };
    if tag == "StandardAttributes" {
        // `StandardAttributes/StandardAttribute[Code]/FillChecking`
        return match names.get(2).copied() {
            Some(property) if SAFE_ATTRIBUTE_PROPERTIES.contains(&property) => {
                safe(RuleId::StandardAttributePresentation)
            }
            _ => structure(RuleId::StandardAttributePropertyNotCovered),
        };
    }
    if safe_property(kind, tag) {
        safe(RuleId::PresentationProperty)
    } else if kind == "ScheduledJob" {
        data(RuleId::ScheduledJobStored)
    } else {
        structure(RuleId::PropertyNotCovered)
    }
}

fn decide_child(names: &[&str], change: &Change) -> Decision {
    let Some(tag) = names.first().copied() else {
        return structure(RuleId::ChildObjectsChanged);
    };
    let Some(child) = child_of(tag) else {
        return structure(RuleId::ChildKindNotCovered);
    };
    let whole = names.len() == 1;
    match child {
        Child::Interface => safe(RuleId::InterfaceRows),
        Child::Column => {
            if whole {
                return match change.op {
                    ChangeOp::Added | ChangeOp::Removed => structure(RuleId::ColumnAddedOrDropped),
                    _ => structure(RuleId::ColumnsReordered),
                };
            }
            match (names.get(1).copied(), names.get(2).copied()) {
                (Some("Properties"), Some(property))
                    if SAFE_ATTRIBUTE_PROPERTIES.contains(&property) =>
                {
                    safe(RuleId::AttributePresentation)
                }
                (Some("Properties"), Some(_)) => structure(RuleId::AttributePropertyNotCovered),
                _ => structure(RuleId::AttributePartNotCovered),
            }
        }
        Child::TabularSection => {
            if whole {
                return structure(RuleId::TabularSectionAddedDroppedMoved);
            }
            match names.get(1).copied() {
                Some("Properties") => match names.get(2).copied() {
                    Some("Name" | "Synonym" | "Comment" | "ToolTip" | "FillChecking") => {
                        safe(RuleId::TabularSectionPresentation)
                    }
                    Some("StandardAttributes") => match names.get(4).copied() {
                        Some(property) if SAFE_ATTRIBUTE_PROPERTIES.contains(&property) => {
                            safe(RuleId::TabularSectionStandardAttributePresentation)
                        }
                        _ => structure(RuleId::TabularSectionStandardAttributeNotCovered),
                    },
                    _ => structure(RuleId::TabularSectionPropertyNotCovered),
                },
                Some("ChildObjects") => {
                    // The attributes of the tabular section.
                    let rest = &names[2..];
                    match rest.first().copied().and_then(child_of) {
                        Some(Child::Column) if rest.len() == 1 => {
                            structure(RuleId::TabularSectionColumnAddedDroppedMoved)
                        }
                        Some(Child::Column) => match (rest.get(1).copied(), rest.get(2).copied()) {
                            (Some("Properties"), Some(property))
                                if SAFE_ATTRIBUTE_PROPERTIES.contains(&property) =>
                            {
                                safe(RuleId::TabularSectionAttributePresentation)
                            }
                            _ => structure(RuleId::TabularSectionAttributePropertyNotCovered),
                        },
                        _ => structure(RuleId::TabularSectionPartNotCovered),
                    }
                }
                _ => structure(RuleId::TabularSectionPartNotCovered),
            }
        }
        Child::EnumValue => {
            if whole {
                return match change.op {
                    ChangeOp::Added | ChangeOp::Removed => data(RuleId::EnumValueAddedOrDropped),
                    _ => data(RuleId::EnumValuesReordered),
                };
            }
            match (names.get(1).copied(), names.get(2).copied()) {
                (Some("Properties"), Some("Name" | "Synonym" | "Comment")) => {
                    safe(RuleId::EnumValuePresentation)
                }
                _ => structure(RuleId::EnumValuePartNotCovered),
            }
        }
    }
}

/// Judges the appearance or disappearance of a whole object.
pub fn decide_lifecycle(kind: &str) -> Decision {
    if owns_no_stored_data(kind) {
        safe(RuleId::KindOwnsNoStoredData)
    } else {
        structure(RuleId::ObjectWithStorageAddedOrDropped)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tree_diff::{Change, ChangeOp, Seg};
    use super::*;

    fn change(path: &[&str], op: ChangeOp) -> Change {
        Change {
            path: path
                .iter()
                .map(|name| Seg {
                    name: (*name).to_string(),
                    label: None,
                })
                .collect(),
            op,
        }
    }

    fn modified() -> ChangeOp {
        ChangeOp::Modified {
            old: "a".to_string(),
            new: "b".to_string(),
        }
    }

    fn class(kind: &str, path: &[&str], op: ChangeOp) -> Class {
        decide(kind, &change(path, op)).class
    }

    #[test]
    fn presentation_of_a_catalog_is_safe_and_its_storage_is_not() {
        for tag in [
            "Synonym",
            "Comment",
            "Explanation",
            "QuickChoice",
            "DefaultObjectForm",
            "AuxiliaryListForm",
            "ExtendedObjectPresentation",
            "InputByString",
            "CheckUnique",
        ] {
            assert_eq!(
                class("Catalog", &["Properties", tag], modified()),
                Class::Safe,
                "{tag}"
            );
        }
        for tag in [
            "CodeLength",
            "DescriptionLength",
            "Hierarchical",
            "FoldersOnTop",
            "CodeType",
            "DataHistory",
            "Owners",
            "AuxiliaryRecordForm",
        ] {
            assert_eq!(
                class("Catalog", &["Properties", tag], modified()),
                Class::Structure,
                "{tag}"
            );
        }
    }

    #[test]
    fn the_same_property_can_be_safe_on_one_kind_and_not_on_another() {
        assert_eq!(
            class("Catalog", &["Properties", "CheckUnique"], modified()),
            Class::Safe
        );
        assert_eq!(
            class("Document", &["Properties", "CheckUnique"], modified()),
            Class::Structure
        );
        assert_eq!(
            class(
                "DocumentJournal",
                &["Properties", "UseStandardCommands"],
                modified()
            ),
            Class::Structure
        );
        assert_eq!(
            class("Enum", &["Properties", "UseStandardCommands"], modified()),
            Class::Safe
        );
    }

    #[test]
    fn a_kind_without_stored_data_takes_any_change() {
        assert_eq!(
            class("CommonModule", &["Properties", "Global"], modified()),
            Class::Safe
        );
        assert_eq!(
            class("Report", &["ChildObjects", "Attribute"], ChangeOp::Added),
            Class::Safe
        );
        assert_eq!(class("Role", &["InternalInfo"], modified()), Class::Safe);
        assert_eq!(decide_lifecycle("Subsystem").class, Class::Safe);
        assert_eq!(decide_lifecycle("Catalog").class, Class::Structure);
        assert_eq!(decide_lifecycle("SomethingNew").class, Class::Structure);
    }

    #[test]
    fn a_column_added_or_dropped_is_a_restructuring_but_its_presentation_is_not() {
        for op in [ChangeOp::Added, ChangeOp::Removed, ChangeOp::Reordered] {
            assert_eq!(
                class("Catalog", &["ChildObjects", "Attribute"], op),
                Class::Structure
            );
        }
        for tag in ["Synonym", "ToolTip", "FillChecking", "MultiLine", "Name"] {
            assert_eq!(
                class(
                    "Catalog",
                    &["ChildObjects", "Attribute", "Properties", tag],
                    modified()
                ),
                Class::Safe,
                "{tag}"
            );
        }
        for tag in ["Type", "Indexing", "DataHistory", "Use"] {
            assert_eq!(
                class(
                    "Catalog",
                    &["ChildObjects", "Attribute", "Properties", tag],
                    modified()
                ),
                Class::Structure,
                "{tag}"
            );
        }
    }

    #[test]
    fn tabular_sections_and_their_attributes() {
        assert_eq!(
            class(
                "Document",
                &["ChildObjects", "TabularSection"],
                ChangeOp::Added
            ),
            Class::Structure
        );
        assert_eq!(
            class(
                "Document",
                &["ChildObjects", "TabularSection", "Properties", "Synonym"],
                modified()
            ),
            Class::Safe
        );
        assert_eq!(
            class(
                "Document",
                &[
                    "ChildObjects",
                    "TabularSection",
                    "ChildObjects",
                    "Attribute"
                ],
                ChangeOp::Added
            ),
            Class::Structure
        );
        assert_eq!(
            class(
                "Document",
                &[
                    "ChildObjects",
                    "TabularSection",
                    "ChildObjects",
                    "Attribute",
                    "Properties",
                    "ToolTip"
                ],
                modified()
            ),
            Class::Safe
        );
        assert_eq!(
            class(
                "Document",
                &[
                    "ChildObjects",
                    "TabularSection",
                    "ChildObjects",
                    "Attribute",
                    "Properties",
                    "Type"
                ],
                modified()
            ),
            Class::Structure
        );
    }

    #[test]
    fn enumeration_values_are_data() {
        assert_eq!(
            class("Enum", &["ChildObjects", "EnumValue"], ChangeOp::Added),
            Class::Data
        );
        assert_eq!(
            class("Enum", &["ChildObjects", "EnumValue"], ChangeOp::Removed),
            Class::Data
        );
        assert_eq!(
            class(
                "Enum",
                &["ChildObjects", "EnumValue", "Properties", "Synonym"],
                modified()
            ),
            Class::Safe
        );
    }

    #[test]
    fn forms_templates_and_commands_are_rows() {
        for tag in ["Form", "Template", "Command"] {
            assert_eq!(
                class("Catalog", &["ChildObjects", tag], ChangeOp::Added),
                Class::Safe
            );
            assert_eq!(
                class("Catalog", &["ChildObjects", tag], ChangeOp::Removed),
                Class::Safe
            );
        }
    }

    #[test]
    fn standard_attributes_follow_the_attribute_rules() {
        let p = ["Properties", "StandardAttributes", "StandardAttribute"];
        let with = |property: &'static str| {
            let mut path = p.to_vec();
            path.push(property);
            path
        };
        assert_eq!(
            class("Catalog", &with("FillChecking"), modified()),
            Class::Safe
        );
        assert_eq!(
            class("Catalog", &with("DataHistory"), modified()),
            Class::Structure
        );
    }

    #[test]
    fn what_no_rule_covers_is_a_restructuring() {
        assert_eq!(
            class("Catalog", &["Properties", "SomethingNew"], modified()),
            Class::Structure
        );
        assert_eq!(
            class(
                "Catalog",
                &["ChildObjects", "SomethingNew"],
                ChangeOp::Added
            ),
            Class::Structure
        );
        assert_eq!(
            class(
                "Catalog",
                &["InternalInfo", "GeneratedType"],
                ChangeOp::Added
            ),
            Class::Structure
        );
        assert_eq!(class("Catalog", &["@uuid"], modified()), Class::Structure);
        assert_eq!(class("Catalog", &[], modified()), Class::Structure);
    }

    #[test]
    fn information_about_the_configuration_is_safe_and_its_objects_are_judged_by_kind() {
        for tag in [
            "Version",
            "Vendor",
            "Copyright",
            "BriefInformation",
            "UpdateCatalogAddress",
        ] {
            assert_eq!(
                class("Configuration", &["Properties", tag], modified()),
                Class::Safe,
                "{tag}"
            );
        }
        for tag in ["CompatibilityMode", "DefaultRunMode", "ScriptVariant"] {
            assert_eq!(
                class("Configuration", &["Properties", tag], modified()),
                Class::Structure,
                "{tag}"
            );
        }
        // The list of objects: by the kind of the listed object.
        assert_eq!(
            class(
                "Configuration",
                &["ChildObjects", "CommonModule"],
                ChangeOp::Added
            ),
            Class::Safe
        );
        assert_eq!(
            class(
                "Configuration",
                &["ChildObjects", "Catalog"],
                ChangeOp::Added
            ),
            Class::Structure
        );
        assert_eq!(
            class(
                "Configuration",
                &["ChildObjects", "Language"],
                ChangeOp::Removed
            ),
            Class::Structure
        );
    }

    #[test]
    fn scheduled_jobs_are_rows_the_platform_keeps() {
        assert_eq!(
            class("ScheduledJob", &["Properties", "Comment"], modified()),
            Class::Safe
        );
        for tag in [
            "Use",
            "Predefined",
            "Key",
            "Description",
            "MethodName",
            "RestartCountOnFailure",
        ] {
            assert_eq!(
                class("ScheduledJob", &["Properties", tag], modified()),
                Class::Data,
                "{tag}"
            );
        }
    }
}
