//! Evidenced metadata record revisions and their absent physical fields.
//! The adapter owns byte slicing; this module owns closed revision/arity facts.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerRecordUpgrade {
    pub target_revision: &'static str,
    pub appended_members: &'static str,
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
        ("54", 59) => ("56", ",0,0"),
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
