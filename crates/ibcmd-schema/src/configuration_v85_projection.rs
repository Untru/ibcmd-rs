//! XML 2.21 properties projected by 8.5 from an older Configuration tuple.
//!
//! The 67/68 layouts have no slots for these properties. The native reader
//! emits this exact vocabulary when printing those layouts in XML 2.21. This
//! rule admits only that projection; it does not describe the separate 76
//! layout's references, localized captions or measured enumeration pairs.

use crate::ConfigurationPropertyEvidencedDefault;

/// Auxiliary form names, in the order native XML 2.21 prints them.
pub const AUXILIARY_FORM_NAMES: [&str; 8] = [
    "AuxiliaryReportForm",
    "AuxiliaryReportVariantForm",
    "AuxiliaryReportSettingsForm",
    "AuxiliaryDynamicListSettingsForm",
    "AuxiliaryDataHistoryChangeHistoryForm",
    "AuxiliaryDataHistoryVersionDataForm",
    "AuxiliaryDataHistoryVersionDifferencesForm",
    "AuxiliaryCollaborationSystemUsersChoiceForm",
];

pub const INTERFACE_VARIANT: &str = "NavigationLeft";
pub const THEME: &str = "Auto";
pub const WINDOWS_OPEN_VARIANT: &str = "OpenDataInDialogs";
pub const MIGRATION_MODE: &str = "DontUse";

/// Exact values retained by an older-layout round trip. Absence of a property
/// is permitted; a named property must carry this value without attributes or
/// nested content. These facts are not part of the global default table.
pub const DEFAULT_PROPERTIES: [(&str, ConfigurationPropertyEvidencedDefault); 14] = [
    (
        AUXILIARY_FORM_NAMES[0],
        ConfigurationPropertyEvidencedDefault::Empty,
    ),
    (
        AUXILIARY_FORM_NAMES[1],
        ConfigurationPropertyEvidencedDefault::Empty,
    ),
    (
        AUXILIARY_FORM_NAMES[2],
        ConfigurationPropertyEvidencedDefault::Empty,
    ),
    (
        AUXILIARY_FORM_NAMES[3],
        ConfigurationPropertyEvidencedDefault::Empty,
    ),
    (
        AUXILIARY_FORM_NAMES[4],
        ConfigurationPropertyEvidencedDefault::Empty,
    ),
    (
        AUXILIARY_FORM_NAMES[5],
        ConfigurationPropertyEvidencedDefault::Empty,
    ),
    (
        AUXILIARY_FORM_NAMES[6],
        ConfigurationPropertyEvidencedDefault::Empty,
    ),
    (
        AUXILIARY_FORM_NAMES[7],
        ConfigurationPropertyEvidencedDefault::Empty,
    ),
    (
        "MainClientApplicationWindowInterfaceVariant",
        ConfigurationPropertyEvidencedDefault::Text(INTERFACE_VARIANT),
    ),
    (
        "ClientApplicationTheme",
        ConfigurationPropertyEvidencedDefault::Text(THEME),
    ),
    (
        "ClientApplicationWindowsOpenVariant",
        ConfigurationPropertyEvidencedDefault::Text(WINDOWS_OPEN_VARIANT),
    ),
    ("Caption", ConfigurationPropertyEvidencedDefault::Empty),
    ("ShortCaption", ConfigurationPropertyEvidencedDefault::Empty),
    (
        "Version85InterfaceMigrationMode",
        ConfigurationPropertyEvidencedDefault::Text(MIGRATION_MODE),
    ),
];

/// The declared older-layout projection of one XML property.
pub fn default_property(name: &str) -> Option<ConfigurationPropertyEvidencedDefault> {
    DEFAULT_PROPERTIES
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, value)| *value)
}

/// Whether the XML reading edition names this projection. Layout selection
/// remains the caller's responsibility and must already be 67 or 68.
pub fn supports_xml_dialect(dialect: &str) -> bool {
    dialect == "2.21"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn configuration_v85_older_defaults_are_closed_and_not_global_defaults() {
        let names = DEFAULT_PROPERTIES
            .iter()
            .map(|(name, _)| *name)
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), 14);
        assert_eq!(AUXILIARY_FORM_NAMES.len(), 8);
        let global = crate::configuration_properties_evidenced_default_block_policy();
        for name in names {
            assert!(default_property(name).is_some());
            assert_eq!(global.evidenced_default_property(name), None);
        }
        assert_eq!(default_property("FutureAuxiliaryForm"), None);
        assert!(supports_xml_dialect("2.21"));
        assert!(!supports_xml_dialect("2.20"));
        assert!(!supports_xml_dialect("2.17"));
    }
}
