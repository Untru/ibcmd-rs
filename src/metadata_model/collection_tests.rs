//! The kind-to-collection table (`index::ROOT_COLLECTIONS`) is the only one:
//! `mssql_dump::root_family_folder`, `index::collection_of_kind` and
//! `index::kind_of_collection` all read it. The expected pairs below are the
//! two tables it replaced (the exporter's own `root_family_folder`, 46 original kinds,
//! and the model's `kind_of_collection`, those 46 plus five nested kinds),
//! carried as data.

use std::collections::BTreeSet;

use crate::metadata_model::index::{
    NESTED_COLLECTIONS, ROOT_COLLECTIONS, collection_of_kind, kind_of_collection,
};
use crate::mssql_dump::root_family_folder;

/// Original root families plus the independently supported WebSocketClient family.
const ROOT_KINDS: [(&str, &str); 47] = [
    ("Role", "Roles"),
    ("CommonTemplate", "CommonTemplates"),
    ("CommonModule", "CommonModules"),
    ("HTTPService", "HTTPServices"),
    ("WebSocketClient", "WebSocketClients"),
    ("ScheduledJob", "ScheduledJobs"),
    ("CommonAttribute", "CommonAttributes"),
    ("SessionParameter", "SessionParameters"),
    ("FunctionalOptionsParameter", "FunctionalOptionsParameters"),
    ("Subsystem", "Subsystems"),
    ("Style", "Styles"),
    ("FilterCriterion", "FilterCriteria"),
    ("SettingsStorage", "SettingsStorages"),
    ("EventSubscription", "EventSubscriptions"),
    ("StyleItem", "StyleItems"),
    ("Bot", "Bots"),
    ("CommonPicture", "CommonPictures"),
    ("ExchangePlan", "ExchangePlans"),
    ("WebService", "WebServices"),
    ("Language", "Languages"),
    ("FunctionalOption", "FunctionalOptions"),
    ("DefinedType", "DefinedTypes"),
    ("XDTOPackage", "XDTOPackages"),
    ("WSReference", "WSReferences"),
    ("Constant", "Constants"),
    ("Document", "Documents"),
    ("CommonForm", "CommonForms"),
    ("InformationRegister", "InformationRegisters"),
    ("CommandGroup", "CommandGroups"),
    ("CommonCommand", "CommonCommands"),
    ("DocumentNumerator", "DocumentNumerators"),
    ("DocumentJournal", "DocumentJournals"),
    ("Report", "Reports"),
    ("ChartOfCharacteristicTypes", "ChartsOfCharacteristicTypes"),
    ("AccumulationRegister", "AccumulationRegisters"),
    ("Sequence", "Sequences"),
    ("DataProcessor", "DataProcessors"),
    ("Catalog", "Catalogs"),
    ("Enum", "Enums"),
    ("ChartOfAccounts", "ChartsOfAccounts"),
    ("AccountingRegister", "AccountingRegisters"),
    ("ChartOfCalculationTypes", "ChartsOfCalculationTypes"),
    ("CalculationRegister", "CalculationRegisters"),
    ("Task", "Tasks"),
    ("BusinessProcess", "BusinessProcesses"),
    ("ExternalDataSource", "ExternalDataSources"),
    ("IntegrationService", "IntegrationServices"),
];

/// `(kind, folder)` of the five nested kinds the model's table also knew.
const NESTED_KINDS: [(&str, &str); 5] = [
    ("Form", "Forms"),
    ("Template", "Templates"),
    ("Recalculation", "Recalculations"),
    ("Interface", "Interfaces"),
    ("PaletteColor", "PaletteColors"),
];

#[test]
fn every_root_kind_has_its_folder_in_both_directions() {
    for (kind, folder) in ROOT_KINDS {
        assert_eq!(collection_of_kind(kind), Some(folder), "{kind}");
        assert_eq!(root_family_folder(kind), Some(folder), "{kind}");
        assert_eq!(kind_of_collection(folder), Some(kind), "{folder}");
        assert_eq!(
            collection_of_kind(kind).and_then(kind_of_collection),
            Some(kind),
            "{kind}"
        );
    }
}

#[test]
fn a_nested_kind_has_a_collection_but_no_root_family() {
    for (kind, folder) in NESTED_KINDS {
        assert_eq!(kind_of_collection(folder), Some(kind), "{folder}");
        assert_eq!(collection_of_kind(kind), None, "{kind}");
        assert_eq!(root_family_folder(kind), None, "{kind}");
    }
}

#[test]
fn a_name_that_is_no_kind_and_no_folder_is_none() {
    for name in [
        "",
        "Unknown",
        "catalog",
        "Catalogs ",
        "Catalog.",
        "Configuration",
        "Form.",
        "Roles/",
        "Recalculation.X",
    ] {
        assert_eq!(collection_of_kind(name), None, "{name:?}");
        assert_eq!(root_family_folder(name), None, "{name:?}");
        assert_eq!(kind_of_collection(name), None, "{name:?}");
    }
    // A folder is not a kind and a kind is not a folder.
    assert_eq!(collection_of_kind("Catalogs"), None);
    assert_eq!(kind_of_collection("Catalog"), None);
}

#[test]
fn the_table_is_exactly_the_two_it_replaced() {
    let table: BTreeSet<_> = ROOT_COLLECTIONS.iter().copied().collect();
    let expected: BTreeSet<_> = ROOT_KINDS
        .iter()
        .map(|&(kind, folder)| (folder, kind))
        .collect();
    assert_eq!(ROOT_COLLECTIONS.len(), 47);
    assert_eq!(table, expected);
    let nested: BTreeSet<_> = NESTED_COLLECTIONS.iter().copied().collect();
    let expected_nested: BTreeSet<_> = NESTED_KINDS
        .iter()
        .map(|&(kind, folder)| (folder, kind))
        .collect();
    assert_eq!(NESTED_COLLECTIONS.len(), 5);
    assert_eq!(nested, expected_nested);
    // One folder per kind and one kind per folder, root and nested together.
    let all: Vec<_> = ROOT_COLLECTIONS.iter().chain(NESTED_COLLECTIONS).collect();
    assert_eq!(
        all.iter().map(|pair| pair.0).collect::<BTreeSet<_>>().len(),
        all.len()
    );
    assert_eq!(
        all.iter().map(|pair| pair.1).collect::<BTreeSet<_>>().len(),
        all.len()
    );
}
