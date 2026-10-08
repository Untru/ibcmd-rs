//! Shared native Configuration XML child order. Binary section/class order is
//! independent. The existing vocabulary retains its relative order; empty
//! ExternalDataSource is witnessed immediately before IntegrationService on
//! native 8.3.27.2214 and 8.5.1.1529. Interface remains unmeasured.

const CHILD_XML_ORDER: &[&str] = &[
    "Language",
    "Subsystem",
    "StyleItem",
    "Style",
    "CommonPicture",
    "SessionParameter",
    "Role",
    "CommonTemplate",
    "FilterCriterion",
    "CommonModule",
    "CommonAttribute",
    "ExchangePlan",
    "XDTOPackage",
    "WebService",
    "HTTPService",
    "WSReference",
    crate::websocket_client::WebSocketClientLayout::KIND,
    "EventSubscription",
    "ScheduledJob",
    "SettingsStorage",
    "FunctionalOption",
    "FunctionalOptionsParameter",
    "DefinedType",
    "PaletteColor",
    "Bot",
    "CommonCommand",
    "CommandGroup",
    "Constant",
    "CommonForm",
    "Catalog",
    "Document",
    "DocumentNumerator",
    "Sequence",
    "DocumentJournal",
    "Enum",
    "Report",
    "DataProcessor",
    "InformationRegister",
    "AccumulationRegister",
    "ChartOfCharacteristicTypes",
    "ChartOfAccounts",
    "AccountingRegister",
    "ChartOfCalculationTypes",
    "CalculationRegister",
    "BusinessProcess",
    "Task",
    crate::external_data_source::KIND,
    "IntegrationService",
];

/// Known top-level kinds in native XML emission order; not an object-count cap.
pub fn child_xml_order() -> &'static [&'static str] {
    CHILD_XML_ORDER
}

/// Rank and canonical XML element name, refusing unmeasured kinds.
pub fn child_xml_kind(kind: &str) -> Option<(usize, &'static str)> {
    child_xml_order()
        .iter()
        .enumerate()
        .find(|(_, candidate)| **candidate == kind)
        .map(|(rank, kind)| (rank, *kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prior_vocabulary_order_is_preserved_with_one_measured_insertion() {
        // Literal predecessor XML vocabulary is a regression oracle only.
        let prior = [
            "Language",
            "Subsystem",
            "StyleItem",
            "Style",
            "CommonPicture",
            "SessionParameter",
            "Role",
            "CommonTemplate",
            "FilterCriterion",
            "CommonModule",
            "CommonAttribute",
            "ExchangePlan",
            "XDTOPackage",
            "WebService",
            "HTTPService",
            "WSReference",
            crate::websocket_client::WebSocketClientLayout::KIND,
            "EventSubscription",
            "ScheduledJob",
            "SettingsStorage",
            "FunctionalOption",
            "FunctionalOptionsParameter",
            "DefinedType",
            "PaletteColor",
            "Bot",
            "CommonCommand",
            "CommandGroup",
            "Constant",
            "CommonForm",
            "Catalog",
            "Document",
            "DocumentNumerator",
            "Sequence",
            "DocumentJournal",
            "Enum",
            "Report",
            "DataProcessor",
            "InformationRegister",
            "AccumulationRegister",
            "ChartOfCharacteristicTypes",
            "ChartOfAccounts",
            "AccountingRegister",
            "ChartOfCalculationTypes",
            "CalculationRegister",
            "BusinessProcess",
            "Task",
            "IntegrationService",
        ];
        let current = child_xml_order();
        assert_eq!(current.len(), prior.len() + 1);
        assert_eq!(
            current
                .iter()
                .copied()
                .filter(|kind| *kind != crate::external_data_source::KIND)
                .collect::<Vec<_>>(),
            prior
        );
        assert_eq!(
            current
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            current.len()
        );
        let (eds, name) = child_xml_kind(crate::external_data_source::KIND).unwrap();
        assert_eq!(name, crate::external_data_source::KIND);
        assert_eq!(current[eds + 1], "IntegrationService");
        let (ws, _) = child_xml_kind(crate::websocket_client::WebSocketClientLayout::KIND).unwrap();
        assert_eq!(current[ws - 1], "WSReference");
        assert_eq!(current[ws + 1], "EventSubscription");
        assert_eq!(child_xml_kind("Interface"), None);
        assert_eq!(child_xml_kind("Unproved435"), None);
        assert_eq!(child_xml_kind("externaldatasource"), None);
    }
}
