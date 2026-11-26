//! ExternalDataSource descriptor facts. Layout corroborated against a native
//! 8.3.27.2214 and 8.5.1.1529 CF/XML and the pinned morph1c source; enum 1/2
//! are source facts, not a claim of native acceptance of this compiler.
//!
//! These are schema identifiers. Object/type/value identities come from input.

pub const KIND: &str = "ExternalDataSource";
pub const COLLECTION: &str = "ExternalDataSources";
/// Declared root-reference prefix; descriptor identity still needs validation.
pub const FULL_NAME_PREFIX: &str = "ExternalDataSource.";
pub const OUTER_ARITY: usize = 6;
pub const BODY_ARITY: usize = 10;
pub const RECORD_CODE: &str = "2";
/// Exact declared-family header slot; reading it alone does not admit a body.
pub const HEADER_PATH: [usize; 3] = [1, 1, 1];
/// The deepest scalar in this fixed empty-family grammar: identity UUID or
/// a localized header string. This is a layout fact, not a configuration cap.
pub const MAX_VALUE_DEPTH: usize = 5;
pub const EMPTY_COLLECTIONS: [&str; 3] = [
    "2bb208ca-e441-4023-9ab9-32a7807a85d0",
    "a98a108a-49a0-4dcf-bfd0-aa4b89eeb3f5",
    "e3403acd-1c95-421b-87e4-4dfa29d38b52",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedCategory {
    pub category: &'static str,
    pub name_stem: &'static str,
    pub type_slot: usize,
    pub value_slot: usize,
}

pub const GENERATED_CATEGORIES: [GeneratedCategory; 3] = [
    GeneratedCategory {
        category: "Manager",
        name_stem: "ExternalDataSourceManager",
        type_slot: 2,
        value_slot: 3,
    },
    GeneratedCategory {
        category: "TablesManager",
        name_stem: "ExternalDataSourceTablesManager",
        type_slot: 4,
        value_slot: 5,
    },
    GeneratedCategory {
        category: "CubesManager",
        name_stem: "ExternalDataSourceCubesManager",
        type_slot: 6,
        value_slot: 7,
    },
];

pub fn generated_name(category: GeneratedCategory, object_name: &str) -> String {
    format!("{}.{object_name}", category.name_stem)
}

pub fn data_lock_code(value: &str) -> Option<i64> {
    match value {
        "Automatic" => Some(0),
        "Managed" => Some(1),
        "AutomaticAndManaged" => Some(2),
        _ => None,
    }
}

pub fn data_lock_name(code: &str) -> Option<&'static str> {
    match code {
        "0" => Some("Automatic"),
        "1" => Some("Managed"),
        "2" => Some("AutomaticAndManaged"),
        _ => None,
    }
}
