//! Evidenced metadata record revisions and their absent physical fields.
//! The adapter owns byte slicing; this module owns closed revision/arity facts.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerRecordUpgrade {
    pub target_revision: &'static str,
    pub appended_members: &'static str,
}

/// An information register owner of version 27, 29 or 30 -> 33: older
/// owners stop before the slots later versions append (version 30 lacks the
/// last two data-history flags, version 29 also the data-history slot,
/// version 27 also both totals-slice flags); two 8.3.27 corpora dump them as
/// `false`, `false`, `DontUse`, `false`, `false` (`0` each).
pub fn information_register_owner_upgrade(
    revision: &str,
    members: usize,
) -> Option<OwnerRecordUpgrade> {
    let appended_members = match (revision, members) {
        ("30", 37) => ",0,0",
        ("29", 36) => ",0,0,0",
        ("27", 34) => ",0,0,0,0,0",
        _ => return None,
    };
    Some(OwnerRecordUpgrade {
        target_revision: "33",
        appended_members,
    })
}

pub fn enum_owner_upgrade(revision: &str, members: usize) -> Option<OwnerRecordUpgrade> {
    let appended_members = match (revision, members) {
        ("19", 20) => ",0",
        ("18", 19) => ",{0,{0}},0",
        _ => return None,
    };
    Some(OwnerRecordUpgrade {
        target_revision: "20",
        appended_members,
    })
}

pub fn history_owner_upgrade(revision: &str, members: usize) -> Option<OwnerRecordUpgrade> {
    let (target_revision, appended_members) = match (revision, members) {
        ("29", 47) => ("30", ",0,0"),
        // ЗУП 3.1.18: eight catalogs of revision 55 end where revision 56
        // appends its last two zero slots, as 54 does (`УслугиСервиса`).
        ("54" | "55", 59) => ("56", ",0,0"),
        ("52" | "53", 58) => ("56", ",0,0,0"),
        ("46" | "47", 53) => ("56", ",2,{1,{0,0}},0,{1,2,0},0,0,0,0"),
        ("38", 51) => ("40", ",0,0"),
        ("37", 50) => ("40", ",0,0,0"),
        ("34", 46) => ("40", ",2,{1,{0,0}},{1,2,0},0,0,0,0"),
        _ => return None,
    };
    Some(OwnerRecordUpgrade {
        target_revision,
        appended_members,
    })
}

#[derive(Clone, Copy, Debug)]
pub struct AttributeWrapperUpgrade {
    pub collection_id: &'static str,
    pub source_revision: &'static str,
    pub source_members: usize,
    pub target_revision: &'static str,
    pub appended_members: &'static str,
}

/// Catalog/document/tabular-section wrappers; later variants already contain history.
pub const ATTRIBUTE_WRAPPER_UPGRADES: &[AttributeWrapperUpgrade] = &[
    AttributeWrapperUpgrade {
        collection_id: "cf4abea7-37b2-11d4-940f-008048da11f9",
        source_revision: "3",
        source_members: 5,
        target_revision: "5",
        appended_members: ",1",
    },
    AttributeWrapperUpgrade {
        collection_id: "cf4abea7-37b2-11d4-940f-008048da11f9",
        source_revision: "4",
        source_members: 6,
        target_revision: "5",
        appended_members: "",
    },
    AttributeWrapperUpgrade {
        collection_id: "45e46cbc-3e24-4165-8b7b-cc98a6f80211",
        source_revision: "3",
        source_members: 4,
        target_revision: "5",
        appended_members: ",1",
    },
    AttributeWrapperUpgrade {
        collection_id: "45e46cbc-3e24-4165-8b7b-cc98a6f80211",
        source_revision: "4",
        source_members: 5,
        target_revision: "5",
        appended_members: "",
    },
    AttributeWrapperUpgrade {
        collection_id: "888744e1-b616-11d4-9436-004095e12fc7",
        source_revision: "6",
        source_members: 4,
        target_revision: "8",
        appended_members: ",1",
    },
    AttributeWrapperUpgrade {
        collection_id: "888744e1-b616-11d4-9436-004095e12fc7",
        source_revision: "7",
        source_members: 5,
        target_revision: "8",
        appended_members: "",
    },
    // An information register attribute: revision 3 keeps the members of
    // revision 4 (indexing, full-text search, data history): ЗУП 3.1.18
    // `БухучетПлановыхНачислений.НеИзменятьПриОтменеНачисления` stores
    // `0,1,1` and 8.3.27.2214 writes DontIndex/Use/Use, as БСП's revision 4
    // `1,0,1` writes Index/DontUse/Use.
    AttributeWrapperUpgrade {
        collection_id: "a2207540-1400-11d6-a3c7-0050bae0a776",
        source_revision: "3",
        source_members: 5,
        target_revision: "4",
        appended_members: "",
    },
    // Revision 2 stores indexing and full-text search, not data history,
    // which 8.3.27.2214 writes `Use` (ЗУП 3.1.18, 65 attributes:
    // `ДоступныеЛицензии.ДатаИзменения` `0,1`).
    AttributeWrapperUpgrade {
        collection_id: "a2207540-1400-11d6-a3c7-0050bae0a776",
        source_revision: "2",
        source_members: 4,
        target_revision: "4",
        appended_members: ",1",
    },
    // An information register resource: revision 7 keeps indexing,
    // full-text search and data history; revision 6 the same three (ЗУП
    // `СлужебныеАдресныеСведения.Значение` `0,0,1`: DontIndex/DontUse/Use),
    // revision 5 the first two, data history then `Use` (ЗУП
    // `УсловияОплатыОтпускаПоУходуЗаРебенкомИспр.СреднийДневнойЗаработок`).
    AttributeWrapperUpgrade {
        collection_id: "13134202-f60b-11d5-a3c7-0050bae0a776",
        source_revision: "6",
        source_members: 5,
        target_revision: "7",
        appended_members: "",
    },
    AttributeWrapperUpgrade {
        collection_id: "13134202-f60b-11d5-a3c7-0050bae0a776",
        source_revision: "5",
        source_members: 4,
        target_revision: "7",
        appended_members: ",1",
    },
    // An information register dimension: revision 9 stores master, main
    // filter, indexing, deny-incomplete, full-text search and data history;
    // revision 6 all but the last, which 8.3.27.2214 writes `Use` (ЗУП
    // `УсловияОплатыОтпускаПоУходуЗаРебенкомИспр.Организация` `0,1,0,1,1`:
    // false/true/DontIndex/true/Use).
    AttributeWrapperUpgrade {
        collection_id: "13134203-f60b-11d5-a3c7-0050bae0a776",
        source_revision: "6",
        source_members: 7,
        target_revision: "9",
        appended_members: ",1",
    },
];

impl AttributeWrapperUpgrade {
    pub fn accepts(&self, revision: &str, members: usize, common_record: &str) -> bool {
        members == self.source_members
            && revision == self.source_revision
            && (common_record.starts_with("{27,") || common_record.starts_with("{25,"))
    }
}

/// Structural revisions are tested separately from adapter-owned list traversal.
pub struct LegacyMetadataRecordLayout;
impl LegacyMetadataRecordLayout {
    pub const ENUM_KIND: &'static str = "Enum";
    /// The resource collection class every information register root holds.
    pub const INFORMATION_REGISTER_RESOURCES: &'static str = "13134202-f60b-11d5-a3c7-0050bae0a776";
    /// An information register root: `{1,<owner>,6,<six collections>}`.
    pub fn information_register_root(tag: &str, members: usize, collection_count: &str) -> bool {
        tag == "1" && members == 9 && collection_count == "6"
    }
    pub fn ordinary_root(tag: &str) -> bool {
        tag == "1"
    }
    pub fn history_root(tag: &str, members: usize, collection_count: &str) -> bool {
        tag == "1" && members == 8 && collection_count == "5"
    }
    pub fn document_journal_owner(members: usize, attribute_prefix: &str) -> bool {
        members == 17 && attribute_prefix == "{1,{1,6,{-60003},"
    }
    pub fn command_group_owner(tag: &str, members: usize) -> bool {
        tag == "3" && members == 7
    }
    pub fn command_picture(tag: &str, members: usize) -> bool {
        tag == "3" && members == 8
    }
    pub fn common_command_root(tag: &str, members: usize) -> bool {
        tag == "1" && members == 3
    }
    pub fn common_command_wrapper(tag: &str, members: usize) -> bool {
        tag == "2" && members == 2
    }
    pub fn common_command_identity(tag: &str, members: usize) -> bool {
        tag == "2" && members == 3
    }
    pub fn common_command_envelope(tag: &str, members: usize) -> bool {
        members == 3 && matches!(tag, "0" | "1")
    }
    pub fn common_command_record(tag: &str, members: usize) -> bool {
        tag == "7" && members == 12
    }
    pub fn form_has_nested_wrapper(tag: &str, members: usize) -> bool {
        tag == "1" && matches!(members, 2 | 3)
    }
}

pub struct ConfigurationRecordLayout;
impl ConfigurationRecordLayout {
    pub const PROPERTIES_CLASS_ID: &'static str = "9cd510cd-abfc-11d4-9434-004095e12fc7";
    pub const SCRIPT_VARIANT_SLOT: usize = 3;
    pub const DEFAULT_RUN_MODE_SLOT: usize = 21;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigurationScriptVariant {
    English,
    Russian,
}
impl ConfigurationScriptVariant {
    pub fn from_record_code(code: &str) -> Option<Self> {
        match code {
            "0" => Some(Self::English),
            "1" => Some(Self::Russian),
            _ => None,
        }
    }
    pub fn metadata_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Russian => "Russian",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_or_wrong_arity_does_not_gain_an_upgrade() {
        assert!(enum_owner_upgrade("19", 19).is_none());
        assert!(history_owner_upgrade("46", 54).is_none());
        assert_eq!(
            information_register_owner_upgrade("29", 36)
                .unwrap()
                .appended_members,
            ",0,0,0"
        );
        assert!(information_register_owner_upgrade("29", 37).is_none());
        assert!(LegacyMetadataRecordLayout::information_register_root(
            "1", 9, "6"
        ));
        assert!(history_owner_upgrade("999", 53).is_none());
        assert_eq!(
            history_owner_upgrade("46", 53).unwrap().target_revision,
            "56"
        );
        assert!(!ATTRIBUTE_WRAPPER_UPGRADES[0].accepts("3", 5, "{24,x}"));
        assert!(ATTRIBUTE_WRAPPER_UPGRADES[0].accepts("3", 5, "{27,x}"));
        assert!(!LegacyMetadataRecordLayout::command_picture("4", 8));
        assert!(!LegacyMetadataRecordLayout::common_command_envelope("2", 3));
    }
    #[test]
    fn older_register_children_and_catalog_owners_have_upgrades() {
        let wrapper = |collection: &str, revision: &str, members: usize| {
            ATTRIBUTE_WRAPPER_UPGRADES.iter().find(|upgrade| {
                upgrade.collection_id == collection && upgrade.accepts(revision, members, "{27,x}")
            })
        };
        let attribute = "a2207540-1400-11d6-a3c7-0050bae0a776";
        let resource = "13134202-f60b-11d5-a3c7-0050bae0a776";
        let dimension = "13134203-f60b-11d5-a3c7-0050bae0a776";
        for (collection, revision, members, target, appended) in [
            (attribute, "3", 5, "4", ""),
            (attribute, "2", 4, "4", ",1"),
            (resource, "6", 5, "7", ""),
            (resource, "5", 4, "7", ",1"),
            (dimension, "6", 7, "9", ",1"),
        ] {
            let upgrade = wrapper(collection, revision, members).unwrap();
            assert_eq!(
                (upgrade.target_revision, upgrade.appended_members),
                (target, appended)
            );
        }
        let catalog = history_owner_upgrade("55", 59).unwrap();
        assert_eq!(
            (catalog.target_revision, catalog.appended_members),
            ("56", ",0,0")
        );
    }

    #[test]
    fn script_and_run_mode_coordinates_are_independent() {
        assert_ne!(
            ConfigurationRecordLayout::SCRIPT_VARIANT_SLOT,
            ConfigurationRecordLayout::DEFAULT_RUN_MODE_SLOT
        );
        assert_eq!(
            ConfigurationScriptVariant::from_record_code("0"),
            Some(ConfigurationScriptVariant::English)
        );
        assert_eq!(
            ConfigurationScriptVariant::from_record_code("1"),
            Some(ConfigurationScriptVariant::Russian)
        );
        assert_eq!(ConfigurationScriptVariant::from_record_code("2"), None);
    }
}
