//! What a Config row is, from its name alone, and what its role means for
//! the data structure.
//!
//! A row is named by the uuid of its owner, and a body row also by a suffix
//! (`<uuid>.0`, `<uuid>.1c`); what the suffix means depends on the owner's
//! kind. The table below is the platform's own: it is read off the
//! `ConfigDumpInfo.xml` of the four reference exports (БСП and ERP УХ, 8.3.27
//! and 8.5), which names every versioned storage record by the role it
//! plays. Applying a module, a form, a template, a picture, a help page or a
//! set of rights copies rows; the predefined items, the flowchart of a
//! business process, the content of an exchange plan and the aggregates and
//! additional indexes of a register are data the platform stores and keeps
//! up to date, so a change there is a restructuring.

/// What a body row's role does to the data structure when it changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Applying copies the row.
    Safe,
    /// The platform changes tables or indexes for it.
    Structure,
    /// The platform changes stored data for it.
    Data,
}

/// The role of a body row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyRole {
    pub name: &'static str,
    pub effect: Effect,
}

const fn safe(name: &'static str) -> BodyRole {
    BodyRole {
        name,
        effect: Effect::Safe,
    }
}

const fn structure(name: &'static str) -> BodyRole {
    BodyRole {
        name,
        effect: Effect::Structure,
    }
}

const fn data(name: &'static str) -> BodyRole {
    BodyRole {
        name,
        effect: Effect::Data,
    }
}

/// (owner kind, suffix) -> role. The owner kind is the kind of the object
/// whose uuid the row is named by: a top-level object, an owned form,
/// template or subsystem, a nested command (`Command`), or `Configuration`
/// for the module group the configuration's own assets are stored under.
const ROLES: &[(&str, &str, BodyRole)] = &[
    ("Configuration", "0", safe("OrdinaryApplicationModule")),
    ("Configuration", "2", safe("Splash")),
    ("Configuration", "3", safe("Help")),
    ("Configuration", "4", safe("ParentConfigurations")),
    ("Configuration", "5", safe("ExternalConnectionModule")),
    ("Configuration", "6", safe("ManagedApplicationModule")),
    ("Configuration", "7", safe("SessionModule")),
    ("Configuration", "8", safe("HomePageWorkArea")),
    ("Configuration", "9", safe("MainSectionCommandInterface")),
    ("Configuration", "a", safe("CommandInterface")),
    ("Configuration", "b", safe("ClientApplicationInterface")),
    ("Configuration", "c", safe("MainSectionPicture")),
    ("Configuration", "f", safe("StandaloneConfigurationContent")),
    ("Configuration", "10", safe("MobileClientSignature")),
    ("AccountingRegister", "5", safe("Help")),
    ("AccountingRegister", "6", safe("RecordSetModule")),
    ("AccountingRegister", "7", safe("ManagerModule")),
    ("AccumulationRegister", "0", safe("Help")),
    ("AccumulationRegister", "1", safe("RecordSetModule")),
    ("AccumulationRegister", "2", safe("ManagerModule")),
    ("AccumulationRegister", "3", structure("Aggregates")),
    ("AccumulationRegister", "4", structure("AdditionalIndexes")),
    ("Bot", "1", safe("Module")),
    ("BusinessProcess", "5", safe("Help")),
    ("BusinessProcess", "6", safe("ObjectModule")),
    ("BusinessProcess", "7", data("Flowchart")),
    ("BusinessProcess", "8", safe("ManagerModule")),
    ("CalculationRegister", "0", safe("Help")),
    ("CalculationRegister", "1", safe("RecordSetModule")),
    ("CalculationRegister", "2", safe("ManagerModule")),
    ("Catalog", "0", safe("ObjectModule")),
    ("Catalog", "1", safe("Help")),
    ("Catalog", "1c", data("Predefined")),
    ("Catalog", "3", safe("ManagerModule")),
    ("ChartOfAccounts", "14", safe("ObjectModule")),
    ("ChartOfAccounts", "15", safe("ManagerModule")),
    ("ChartOfAccounts", "5", safe("Help")),
    ("ChartOfAccounts", "9", data("Predefined")),
    ("ChartOfCalculationTypes", "0", safe("ObjectModule")),
    ("ChartOfCalculationTypes", "1", safe("Help")),
    ("ChartOfCalculationTypes", "2", data("Predefined")),
    ("ChartOfCalculationTypes", "3", safe("ManagerModule")),
    ("ChartOfCharacteristicTypes", "15", safe("ObjectModule")),
    ("ChartOfCharacteristicTypes", "16", safe("ManagerModule")),
    ("ChartOfCharacteristicTypes", "5", safe("Help")),
    ("ChartOfCharacteristicTypes", "7", data("Predefined")),
    ("Command", "2", safe("CommandModule")),
    ("CommonCommand", "1", safe("Help")),
    ("CommonCommand", "2", safe("CommandModule")),
    ("CommonForm", "0", safe("Form")),
    ("CommonForm", "1", safe("Help")),
    ("CommonModule", "0", safe("Module")),
    ("CommonPicture", "0", safe("Picture")),
    ("CommonTemplate", "0", safe("Template")),
    ("Constant", "0", safe("ValueManagerModule")),
    ("Constant", "1", safe("ManagerModule")),
    ("DataProcessor", "0", safe("ObjectModule")),
    ("DataProcessor", "1", safe("Help")),
    ("DataProcessor", "2", safe("ManagerModule")),
    ("Document", "0", safe("ObjectModule")),
    ("Document", "1", safe("Help")),
    ("Document", "2", safe("ManagerModule")),
    ("Document", "3", structure("AdditionalIndexes")),
    ("DocumentJournal", "0", safe("Help")),
    ("DocumentJournal", "1", safe("ManagerModule")),
    ("Enum", "0", safe("ManagerModule")),
    ("Enum", "1", safe("Help")),
    ("ExchangePlan", "0", safe("Help")),
    ("ExchangePlan", "1", structure("Content")),
    ("ExchangePlan", "2", safe("ObjectModule")),
    ("ExchangePlan", "3", safe("ManagerModule")),
    ("FilterCriterion", "0", safe("ManagerModule")),
    ("Form", "0", safe("Form")),
    ("Form", "1", safe("Help")),
    ("HTTPService", "0", safe("Module")),
    ("InformationRegister", "0", safe("Help")),
    ("InformationRegister", "1", safe("RecordSetModule")),
    ("InformationRegister", "2", safe("ManagerModule")),
    ("IntegrationService", "0", safe("Module")),
    ("Report", "0", safe("ObjectModule")),
    ("Report", "1", safe("Help")),
    ("Report", "2", safe("ManagerModule")),
    ("Role", "0", safe("Rights")),
    ("ScheduledJob", "0", safe("Schedule")),
    ("Sequence", "0", safe("RecordSetModule")),
    ("SettingsStorage", "8", safe("ManagerModule")),
    ("Style", "0", safe("Style")),
    ("Subsystem", "0", safe("Help")),
    ("Subsystem", "1", safe("CommandInterface")),
    ("Task", "5", safe("Help")),
    ("Task", "6", safe("ObjectModule")),
    ("Task", "7", safe("ManagerModule")),
    ("Template", "0", safe("Template")),
    ("WSReference", "0", safe("WSDefinition")),
    ("WebService", "0", safe("Module")),
    ("XDTOPackage", "0", safe("Package")),
];

/// The role of `<owner>.<suffix>` when the owner is of `owner_kind`.
pub fn body_role(owner_kind: &str, suffix: &str) -> Option<BodyRole> {
    ROLES
        .iter()
        .find(|(kind, known, _)| *kind == owner_kind && *known == suffix)
        .map(|(_, _, role)| *role)
}

/// The shapes a Config row name takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowName<'a> {
    /// `root`, `version` or `versions`.
    Service(&'a str),
    /// The dynamic-update history of the table.
    DynamicMarker,
    /// `<base>_dynupdate_<generation>[.<suffix>]`, a row of one generation
    /// of an online update.
    Alias,
    /// `<uuid>`: a metadata object's descriptor.
    Descriptor(&'a str),
    /// `<uuid>.<suffix>`.
    Body { owner: &'a str, suffix: &'a str },
    /// Anything else.
    Other,
}

pub const DYNAMIC_UPDATE_INFIX: &str = "_dynupdate_";
pub const DYNAMIC_MARKER: &str = "DynamicallyUpdated";

pub fn is_uuid(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

pub fn parse_row_name(name: &str) -> RowName<'_> {
    match name {
        "root" | "version" | "versions" => return RowName::Service(name),
        DYNAMIC_MARKER => return RowName::DynamicMarker,
        _ => {}
    }
    if name.contains(DYNAMIC_UPDATE_INFIX) {
        return RowName::Alias;
    }
    match name.split_once('.') {
        None if is_uuid(name) => RowName::Descriptor(name),
        Some((owner, suffix)) if is_uuid(owner) && !suffix.is_empty() => {
            RowName::Body { owner, suffix }
        }
        _ => RowName::Other,
    }
}

/// The role of a file of a source tree, from where it lies under its
/// object's `Ext` folder: the name of its first component is the role's.
pub fn file_role(rel: &str) -> Option<BodyRole> {
    let parts = rel.split('/').collect::<Vec<_>>();
    let ext = parts.iter().rposition(|part| *part == "Ext")?;
    let first = *parts.get(ext + 1)?;
    Some(match first {
        "Predefined.xml" => data("Predefined"),
        "Flowchart.xml" => data("Flowchart"),
        "Content.xml" => structure("Content"),
        "Aggregates.xml" => structure("Aggregates"),
        "AdditionalIndexes.xml" => structure("AdditionalIndexes"),
        "ObjectModule.bsl"
        | "ManagerModule.bsl"
        | "RecordSetModule.bsl"
        | "ValueManagerModule.bsl"
        | "Module.bsl"
        | "CommandModule.bsl"
        | "OrdinaryApplicationModule.bsl"
        | "ManagedApplicationModule.bsl"
        | "SessionModule.bsl"
        | "ExternalConnectionModule.bsl" => safe("Module"),
        "Form.xml" | "Form" => safe("Form"),
        "Help.xml" | "Help" => safe("Help"),
        "Picture.xml" | "Picture" => safe("Picture"),
        "Template.xml" | "Template.bin" | "Template.txt" | "Template" => safe("Template"),
        "Rights.xml" => safe("Rights"),
        "CommandInterface.xml" => safe("CommandInterface"),
        "Schedule.xml" => safe("Schedule"),
        "Package.bin" => safe("Package"),
        "Style.xml" => safe("Style"),
        "WSDefinition.xml" => safe("WSDefinition"),
        "Splash.xml" | "Splash" => safe("Splash"),
        "HomePageWorkArea.xml" => safe("HomePageWorkArea"),
        "MainSectionCommandInterface.xml" => safe("MainSectionCommandInterface"),
        "MainSectionPicture.xml" | "MainSectionPicture" => safe("MainSectionPicture"),
        "ClientApplicationInterface.xml" => safe("ClientApplicationInterface"),
        "ParentConfigurations.bin" | "ParentConfigurations" => safe("ParentConfigurations"),
        "MobileClientSignature.bin" => safe("MobileClientSignature"),
        "StandaloneConfigurationContent.bin" => safe("StandaloneConfigurationContent"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const U: &str = "6bc2c3f8-6027-411e-97c5-6f1b7e4cea61";

    #[test]
    fn names_are_told_apart() {
        assert_eq!(parse_row_name("root"), RowName::Service("root"));
        assert_eq!(parse_row_name("versions"), RowName::Service("versions"));
        assert_eq!(parse_row_name("DynamicallyUpdated"), RowName::DynamicMarker);
        assert_eq!(parse_row_name(U), RowName::Descriptor(U));
        let body = format!("{U}.1c");
        assert_eq!(
            parse_row_name(&body),
            RowName::Body {
                owner: U,
                suffix: "1c"
            }
        );
        assert_eq!(
            parse_row_name(&format!("{U}_dynupdate_{U}.0")),
            RowName::Alias
        );
        assert_eq!(
            parse_row_name(&format!("versions_dynupdate_{U}")),
            RowName::Alias
        );
        assert_eq!(parse_row_name("Files.MobileVersions.dat"), RowName::Other);
        assert_eq!(parse_row_name(&format!("{U}.")), RowName::Other);
        assert_eq!(parse_row_name("x-y"), RowName::Other);
    }

    #[test]
    fn the_roles_that_carry_data_are_not_safe() {
        let predefined = body_role("Catalog", "1c").unwrap();
        assert_eq!(predefined.name, "Predefined");
        assert_eq!(predefined.effect, Effect::Data);
        assert_eq!(
            body_role("BusinessProcess", "7").unwrap().effect,
            Effect::Data
        );
        assert_eq!(body_role("ExchangePlan", "1").unwrap().name, "Content");
        assert_eq!(
            body_role("ExchangePlan", "1").unwrap().effect,
            Effect::Structure
        );
        assert_eq!(
            body_role("Document", "3").unwrap().effect,
            Effect::Structure,
            "additional indexes"
        );
        assert_eq!(body_role("Catalog", "0").unwrap().effect, Effect::Safe);
        assert_eq!(body_role("Form", "0").unwrap().effect, Effect::Safe);
        assert_eq!(body_role("Role", "0").unwrap().name, "Rights");
    }

    #[test]
    fn a_suffix_the_table_does_not_know_has_no_role() {
        assert!(body_role("Catalog", "99").is_none());
        assert!(body_role("NoSuchKind", "0").is_none());
        // Same suffix, different meaning by owner kind.
        assert_eq!(
            body_role("ChartOfCharacteristicTypes", "7").unwrap().name,
            "Predefined"
        );
        assert_eq!(
            body_role("Configuration", "7").unwrap().name,
            "SessionModule"
        );
        assert_eq!(body_role("Task", "7").unwrap().name, "ManagerModule");
    }

    #[test]
    fn files_of_a_tree_are_told_apart_by_their_place_under_ext() {
        let role = |rel: &str| file_role(rel).map(|role| (role.name, role.effect));
        assert_eq!(
            role("Catalogs/X/Ext/ObjectModule.bsl"),
            Some(("Module", Effect::Safe))
        );
        assert_eq!(
            role("Catalogs/X/Forms/F/Ext/Form/Module.bsl"),
            Some(("Form", Effect::Safe))
        );
        assert_eq!(
            role("Catalogs/X/Forms/F/Ext/Form/Items/A/RowsPicture.png"),
            Some(("Form", Effect::Safe))
        );
        assert_eq!(
            role("Roles/R/Ext/Rights.xml"),
            Some(("Rights", Effect::Safe))
        );
        assert_eq!(
            role("Catalogs/X/Ext/Predefined.xml"),
            Some(("Predefined", Effect::Data))
        );
        assert_eq!(
            role("ExchangePlans/E/Ext/Content.xml"),
            Some(("Content", Effect::Structure))
        );
        assert_eq!(
            role("BusinessProcesses/B/Ext/Flowchart.xml"),
            Some(("Flowchart", Effect::Data))
        );
        assert_eq!(
            role("Ext/ManagedApplicationModule.bsl").unwrap().1,
            Effect::Safe
        );
        assert_eq!(role("Catalogs/X/Ext/Something.new"), None);
        assert_eq!(role("Catalogs/X.xml"), None);
    }

    #[test]
    fn no_pair_is_listed_twice() {
        for (index, (kind, suffix, _)) in ROLES.iter().enumerate() {
            assert!(
                ROLES[index + 1..]
                    .iter()
                    .all(|(other, other_suffix, _)| other != kind || other_suffix != suffix),
                "{kind}.{suffix} is listed twice"
            );
        }
    }
}
