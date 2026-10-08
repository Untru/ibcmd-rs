//! Existing metadata enumeration domains and source-only external named owners.
//! These facts select source properties, never a native Form record version.
pub const TEMPLATE_TYPES: &[(&str, i64)] = &[
    ("SpreadsheetDocument", 0),
    ("BinaryData", 1),
    ("ActiveDocument", 2),
    ("HTMLDocument", 3),
    ("TextDocument", 4),
    ("GeographicalSchema", 5),
    ("DataCompositionSchema", 6),
    ("DataCompositionAppearanceTemplate", 7),
    ("GraphicalSchema", 8),
    ("AddIn", 9),
];
pub const USE_PURPOSES: &[(&str, i64)] =
    &[("PlatformApplication", 1), ("MobilePlatformApplication", 2)];

pub const FORM_TYPES: &[(&str, i64)] = &[("Ordinary", 0), ("Managed", 1)];

pub const INTERFACE_COMPATIBILITY_MODES: &[(&str, i64)] = &[("Any", 0)];
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalNamedKind {
    Form,
    Template,
}
impl ExternalNamedKind {
    pub const fn family(self) -> &'static str {
        match self {
            Self::Form => "Form",
            Self::Template => "Template",
        }
    }
    pub const fn collection(self) -> &'static str {
        match self {
            Self::Form => "Forms",
            Self::Template => "Templates",
        }
    }
    pub const fn declaration(self) -> &'static str {
        match self {
            Self::Form => "ChildForms",
            Self::Template => "ChildTemplates",
        }
    }
    pub const fn properties(self) -> &'static [&'static str] {
        match self {
            Self::Form => &[
                "Name",
                "Synonym",
                "Comment",
                "FormType",
                "IncludeHelpInContents",
                "UsePurposes",
                "ExtendedPresentation",
                "UseInInterfaceCompatibilityMode",
            ],
            Self::Template => &["Name", "Synonym", "Comment", "TemplateType"],
        }
    }
    pub fn property_available(self, property: &str, edition: &str) -> bool {
        self.properties().contains(&property)
            && (property != "UseInInterfaceCompatibilityMode" || edition == "2.21")
    }
}

/// Source envelope facts bind owned bytes without compiling native bodies.
pub fn body_xml_envelope(kind: &str) -> Option<(&'static str, &'static str)> {
    match kind {
        "Form" => Some(("Form", "http://v8.1c.ru/8.3/xcf/logform")),
        "DataCompositionSchema" => Some((
            "DataCompositionSchema",
            "http://v8.1c.ru/8.1/data-composition-system/schema",
        )),
        "DataCompositionAppearanceTemplate" => Some((
            "AppearanceTemplate",
            "http://v8.1c.ru/8.1/data-composition-system/appearance-template",
        )),
        "GraphicalSchema" => Some(("GraphicalSchema", "http://v8.1c.ru/8.3/xcf/scheme")),
        "SpreadsheetDocument" => Some(("document", "http://v8.1c.ru/8.2/data/spreadsheet")),
        _ => None,
    }
}
