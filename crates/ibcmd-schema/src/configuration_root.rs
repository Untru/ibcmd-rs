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

/// Interface mode stored by the V76 Configuration tuple in members 38/62.
/// Both coordinates matter: native 8.5.1.1529 stores Taxi as 3/3, whereas
/// BSP's Version8_5EnableTaxi stores 3/6. This is not a general enum ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum V76InterfaceCompatibility {
    Taxi,
    Version8_5EnableTaxi,
    TaxiEnableVersion8_2,
}

impl V76InterfaceCompatibility {
    pub fn from_stored_codes(first: u8, second: u8) -> Option<Self> {
        match (first, second) {
            (3, 3) => Some(Self::Taxi),
            (3, 6) => Some(Self::Version8_5EnableTaxi),
            (2, 2) => Some(Self::TaxiEnableVersion8_2),
            _ => None,
        }
    }

    pub fn from_xml_name(name: &str) -> Option<Self> {
        match name {
            "Taxi" => Some(Self::Taxi),
            "Version8_5EnableTaxi" => Some(Self::Version8_5EnableTaxi),
            "TaxiEnableVersion8_2" => Some(Self::TaxiEnableVersion8_2),
            _ => None,
        }
    }

    pub fn xml_name(self) -> &'static str {
        match self {
            Self::Taxi => "Taxi",
            Self::Version8_5EnableTaxi => "Version8_5EnableTaxi",
            Self::TaxiEnableVersion8_2 => "TaxiEnableVersion8_2",
        }
    }

    /// The 8.5 interface lexeme was added in XML 2.21. Common Taxi modes
    /// retain the existing earlier-edition vocabulary.
    pub fn supports_xml_dialect(self, dialect: &str) -> bool {
        self != Self::Version8_5EnableTaxi || dialect == "2.21"
    }

    pub fn stored_codes(self) -> (u8, u8) {
        match self {
            Self::Taxi => (3, 3),
            Self::Version8_5EnableTaxi => (3, 6),
            Self::TaxiEnableVersion8_2 => (2, 2),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v76_interface_pairs_have_exact_independent_inverses() {
        for (a, b, name) in [
            (3, 3, "Taxi"),
            (3, 6, "Version8_5EnableTaxi"),
            (2, 2, "TaxiEnableVersion8_2"),
        ] {
            let mode = V76InterfaceCompatibility::from_stored_codes(a, b).unwrap();
            assert_eq!(mode.xml_name(), name);
            assert_eq!(mode.stored_codes(), (a, b));
            assert!(mode.supports_xml_dialect("2.21"));
            assert_eq!(mode.supports_xml_dialect("2.20"), b != 6);
            assert_eq!(mode.supports_xml_dialect("2.17"), b != 6);
            assert_eq!(V76InterfaceCompatibility::from_xml_name(name), Some(mode));
        }
        for a in 0..=9 {
            for b in 0..=9 {
                if !matches!((a, b), (3, 3) | (3, 6) | (2, 2)) {
                    assert_eq!(V76InterfaceCompatibility::from_stored_codes(a, b), None);
                }
            }
        }
        for name in ["Version8_2", "TaxiEnableVersion8_5", "Taxi ", "taxi", ""] {
            assert_eq!(V76InterfaceCompatibility::from_xml_name(name), None);
        }
    }

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
