//! Shared declared external package identity and XML property vocabulary.
//! The contained metadata owner and physical main descriptor are distinct.

pub const DATA_PROCESSOR_CLASS: &str = "c3831ec8-d8d5-4f93-8a22-f9bfae07327f";
pub const REPORT_CLASS: &str = "e41aff26-25cf-4bb6-b6c1-3f478a75f374";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalArtifactKind {
    DataProcessor,
    Report,
}

impl ExternalArtifactKind {
    pub fn from_class_id(class_id: &str) -> Option<Self> {
        match class_id.trim().to_ascii_lowercase().as_str() {
            DATA_PROCESSOR_CLASS => Some(Self::DataProcessor),
            REPORT_CLASS => Some(Self::Report),
            _ => None,
        }
    }
    pub const fn class_id(self) -> &'static str {
        match self {
            Self::DataProcessor => DATA_PROCESSOR_CLASS,
            Self::Report => REPORT_CLASS,
        }
    }
    pub const fn internal_kind(self) -> &'static str {
        match self {
            Self::DataProcessor => "DataProcessor",
            Self::Report => "Report",
        }
    }
    pub const fn internal_folder(self) -> &'static str {
        match self {
            Self::DataProcessor => "DataProcessors",
            Self::Report => "Reports",
        }
    }
    pub const fn external_kind(self) -> &'static str {
        match self {
            Self::DataProcessor => "ExternalDataProcessor",
            Self::Report => "ExternalReport",
        }
    }
    pub const fn object_type_prefix(self) -> &'static str {
        match self {
            Self::DataProcessor => "ExternalDataProcessorObject",
            Self::Report => "ExternalReportObject",
        }
    }
    pub const fn properties(self) -> &'static [&'static str] {
        match self {
            Self::DataProcessor => &["Name", "Synonym", "Comment", "DefaultForm", "AuxiliaryForm"],
            Self::Report => &[
                "Name",
                "Synonym",
                "Comment",
                "DefaultForm",
                "AuxiliaryForm",
                "MainDataCompositionSchema",
                "DefaultSettingsForm",
                "AuxiliarySettingsForm",
                "DefaultVariantForm",
                "AuxiliaryVariantForm",
                "VariantsStorage",
                "SettingsStorage",
            ],
        }
    }
    pub fn property_available(self, name: &str, version: &str) -> bool {
        matches!(version, "2.20" | "2.21")
            && self.properties().contains(&name)
            && (name != "AuxiliaryVariantForm" || version == "2.21")
    }
}
