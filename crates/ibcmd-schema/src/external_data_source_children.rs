//! ExternalDataSource child descriptor facts (`Cube`, `Table`, `DimensionTable`).
//!
//! The native CF layout below was measured on two independent corpora:
//! the target matrix configuration and the stepwise
//! `TEST123/_ВнешнийИсточникДанных` configurations. The XML order is the
//! platform's Designer order. Keep this table declarative: object identities
//! and property values come from the stored row.

pub const CUBE_KIND: &str = "Cube";
pub const CUBE_COLLECTION: &str = "Cubes";
pub const TABLE_KIND: &str = "Table";
pub const TABLE_COLLECTION: &str = "Tables";
pub const DIMENSION_TABLE_KIND: &str = "DimensionTable";
pub const DIMENSION_TABLE_COLLECTION: &str = "DimensionTables";

pub const CUBE_BODY_ARITY: usize = 27;
pub const TABLE_BODY_ARITY: usize = 52;
pub const DIMENSION_TABLE_BODY_ARITY: usize = 33;

/// The native body record discriminator (`{<code>,...}`).
pub const CUBE_RECORD_CODE: &str = "0";
pub const TABLE_RECORD_CODE: &str = "11";
pub const DIMENSION_TABLE_RECORD_CODE: &str = "0";

/// One generated-type category of a child descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedCategory {
    pub category: &'static str,
    pub name_stem: &'static str,
    /// Index of the TypeId in the owner payload; the ValueId follows it.
    pub body_slot: usize,
    /// Position in the XML `InternalInfo`.
    pub xml_order: usize,
}

/// Cube categories: native body order is Manager, RecordSet, Record,
/// RecordKey, List, DimensionTables, with RecordManager in the tail; the
/// XML order is Manager, List, RecordSet, Record, RecordKey, RecordManager,
/// DimensionTables.
pub const CUBE_GENERATED_CATEGORIES: &[GeneratedCategory] = &[
    GeneratedCategory {
        category: "Manager",
        name_stem: "ExternalDataSourceCubeManager",
        body_slot: 2,
        xml_order: 0,
    },
    GeneratedCategory {
        category: "List",
        name_stem: "ExternalDataSourceCubeList",
        body_slot: 10,
        xml_order: 1,
    },
    GeneratedCategory {
        category: "RecordSet",
        name_stem: "ExternalDataSourceCubeRecordSet",
        body_slot: 4,
        xml_order: 2,
    },
    GeneratedCategory {
        category: "Record",
        name_stem: "ExternalDataSourceCubeRecord",
        body_slot: 6,
        xml_order: 3,
    },
    GeneratedCategory {
        category: "RecordKey",
        name_stem: "ExternalDataSourceCubeRecordKey",
        body_slot: 8,
        xml_order: 4,
    },
    GeneratedCategory {
        category: "RecordManager",
        name_stem: "ExternalDataSourceCubeRecordManager",
        body_slot: 25,
        xml_order: 5,
    },
    GeneratedCategory {
        category: "DimensionTables",
        name_stem: "ExternalDataSourceCubeDimensionsTablesManager",
        body_slot: 12,
        xml_order: 6,
    },
];

/// Table categories: native body order is Manager, RecordSet, Record,
/// RecordKey, Object, Ref, List, with RecordManager in the tail; the XML
/// order is Manager, Object, Ref, List, Record, RecordSet, RecordKey,
/// RecordManager.
pub const TABLE_GENERATED_CATEGORIES: &[GeneratedCategory] = &[
    GeneratedCategory {
        category: "Manager",
        name_stem: "ExternalDataSourceTableManager",
        body_slot: 2,
        xml_order: 0,
    },
    GeneratedCategory {
        category: "Object",
        name_stem: "ExternalDataSourceTableObject",
        body_slot: 10,
        xml_order: 1,
    },
    GeneratedCategory {
        category: "Ref",
        name_stem: "ExternalDataSourceTableRef",
        body_slot: 12,
        xml_order: 2,
    },
    GeneratedCategory {
        category: "List",
        name_stem: "ExternalDataSourceTableList",
        body_slot: 14,
        xml_order: 3,
    },
    GeneratedCategory {
        category: "Record",
        name_stem: "ExternalDataSourceTableRecord",
        body_slot: 6,
        xml_order: 4,
    },
    GeneratedCategory {
        category: "RecordSet",
        name_stem: "ExternalDataSourceTableRecordSet",
        body_slot: 4,
        xml_order: 5,
    },
    GeneratedCategory {
        category: "RecordKey",
        name_stem: "ExternalDataSourceTableRecordKey",
        body_slot: 8,
        xml_order: 6,
    },
    GeneratedCategory {
        category: "RecordManager",
        name_stem: "ExternalDataSourceTableRecordManager",
        body_slot: 36,
        xml_order: 7,
    },
];

pub const DIMENSION_TABLE_GENERATED_CATEGORIES: &[GeneratedCategory] = &[
    GeneratedCategory {
        category: "Manager",
        name_stem: "ExternalDataSourceCubeDimensionTableManager",
        body_slot: 2,
        xml_order: 0,
    },
    GeneratedCategory {
        category: "Object",
        name_stem: "ExternalDataSourceCubeDimensionTableObject",
        body_slot: 4,
        xml_order: 1,
    },
    GeneratedCategory {
        category: "Ref",
        name_stem: "ExternalDataSourceCubeDimensionTableRef",
        body_slot: 6,
        xml_order: 2,
    },
    GeneratedCategory {
        category: "List",
        name_stem: "ExternalDataSourceCubeDimensionTableList",
        body_slot: 8,
        xml_order: 3,
    },
];

/// Root ExternalDataSource collections, in native body order.
pub const ROOT_CUBES_COLLECTION: &str = "2bb208ca-e441-4023-9ab9-32a7807a85d0";
pub const ROOT_FORMULAS_COLLECTION: &str = "a98a108a-49a0-4dcf-bfd0-aa4b89eeb3f5";
pub const ROOT_TABLES_COLLECTION: &str = "e3403acd-1c95-421b-87e4-4dfa29d38b52";

pub const CUBE_FORMS_COLLECTION: &str = "3448e506-5add-4fce-a604-7305466b2d8e";
pub const TABLE_FORMS_COLLECTION: &str = "17816ebc-4068-496e-adc4-8879945a832f";
pub const DIMENSION_TABLE_FORMS_COLLECTION: &str = "2367cec0-5648-49d7-95f0-76d387653517";
pub const TEMPLATES_COLLECTION: &str = "3daea016-69b7-4ed4-9453-127911372fe6";
pub const CUBE_COMMANDS_COLLECTION: &str = "4ee40ec7-3469-439f-adb4-aa26ce2d3ec3";
pub const TABLE_COMMANDS_COLLECTION: &str = "5bb6f09e-5d80-41f6-8070-9faa4d15b69b";
pub const DIMENSION_TABLE_COMMANDS_COLLECTION: &str = "8352a2a3-943d-43a2-95d9-1c592b1d337d";
/// Cube `dimensions` collection: file children (`DimensionTable` XML sidecar).
pub const CUBE_DIMENSION_TABLES_COLLECTION: &str = "631411f3-8c65-4066-a78f-57a72e015c1c";
/// Cube `resources` collection in the Python schema name; the platform writes
/// its items as inline `<Dimension>` elements.
pub const CUBE_INLINE_DIMENSIONS_COLLECTION: &str = "7374aaf6-fe8c-4bba-8574-2075a293f0b6";
/// Cube `dimension_tables` collection in the Python schema name; the platform
/// writes its items as inline `<Resource>` elements.
pub const CUBE_INLINE_RESOURCES_COLLECTION: &str = "cead2290-2080-4077-a57b-73051d618683";
pub const FIELDS_COLLECTION: &str = "c7075688-01c1-41ba-bd1c-1023c8189079";

/// `Cube` -> its generated categories.
#[must_use]
pub const fn categories(kind: &str) -> &'static [GeneratedCategory] {
    match kind.as_bytes() {
        b"Cube" => CUBE_GENERATED_CATEGORIES,
        b"Table" => TABLE_GENERATED_CATEGORIES,
        b"DimensionTable" => DIMENSION_TABLE_GENERATED_CATEGORIES,
        _ => &[],
    }
}

/// `Cube` -> `Cubes`, ...
#[must_use]
pub const fn collection(kind: &str) -> &'static str {
    match kind.as_bytes() {
        b"Cube" => CUBE_COLLECTION,
        b"Table" => TABLE_COLLECTION,
        b"DimensionTable" => DIMENSION_TABLE_COLLECTION,
        _ => "",
    }
}

/// `Cube` -> 27, ...
#[must_use]
pub const fn body_arity(kind: &str) -> usize {
    match kind.as_bytes() {
        b"Cube" => CUBE_BODY_ARITY,
        b"Table" => TABLE_BODY_ARITY,
        b"DimensionTable" => DIMENSION_TABLE_BODY_ARITY,
        _ => 0,
    }
}

/// `Cube` -> `0`, `Table` -> `11`, ...
#[must_use]
pub const fn record_code(kind: &str) -> &'static str {
    match kind.as_bytes() {
        b"Cube" => CUBE_RECORD_CODE,
        b"Table" => TABLE_RECORD_CODE,
        b"DimensionTable" => DIMENSION_TABLE_RECORD_CODE,
        _ => "",
    }
}

/// Unrendered body properties. The exact defaults are shared by the public
/// eds-matrix/eds-corpus2 rows and the independent eds-branch table.
/// A one-member brace wrapper retains scalar/string/list distinctions.
pub struct UnmodeledDefault {
    pub body_slot: usize,
    pub wrapped_value: &'static str,
}

pub const TABLE_PRESENTATION_SLOT: usize = 32;
pub const TABLE_KEY_FIELDS_SLOT: usize = 18;
pub const TABLE_INPUT_BY_STRING_SLOT: usize = 33;
pub const CUBE_UNMODELED_DEFAULTS: &[UnmodeledDefault] = &[
    UnmodeledDefault {
        body_slot: 15,
        wrapped_value: r#"{0}"#,
    },
    UnmodeledDefault {
        body_slot: 17,
        wrapped_value: r#"{00000000-0000-0000-0000-000000000000}"#,
    },
    UnmodeledDefault {
        body_slot: 18,
        wrapped_value: r#"{00000000-0000-0000-0000-000000000000}"#,
    },
    UnmodeledDefault {
        body_slot: 19,
        wrapped_value: r#"{{0}}"#,
    },
    UnmodeledDefault {
        body_slot: 20,
        wrapped_value: r#"{{0}}"#,
    },
    UnmodeledDefault {
        body_slot: 21,
        wrapped_value: r#"{{0}}"#,
    },
    UnmodeledDefault {
        body_slot: 22,
        wrapped_value: r#"{{0}}"#,
    },
    UnmodeledDefault {
        body_slot: 23,
        wrapped_value: r#"{{0}}"#,
    },
    UnmodeledDefault {
        body_slot: 24,
        wrapped_value: r#"{{0,{0}}}"#,
    },
];

pub const TABLE_UNMODELED_DEFAULTS: &[UnmodeledDefault] = &[
    UnmodeledDefault {
        body_slot: 19,
        wrapped_value: r#"{00000000-0000-0000-0000-000000000000}"#,
    },
    UnmodeledDefault {
        body_slot: 23,
        wrapped_value: r#"{00000000-0000-0000-0000-000000000000}"#,
    },
    UnmodeledDefault {
        body_slot: 24,
        wrapped_value: r#"{00000000-0000-0000-0000-000000000000}"#,
    },
    UnmodeledDefault {
        body_slot: 25,
        wrapped_value: r#"{00000000-0000-0000-0000-000000000000}"#,
    },
    UnmodeledDefault {
        body_slot: 26,
        wrapped_value: r#"{00000000-0000-0000-0000-000000000000}"#,
    },
    UnmodeledDefault {
        body_slot: 27,
        wrapped_value: r#"{{0}}"#,
    },
    UnmodeledDefault {
        body_slot: 28,
        wrapped_value: r#"{{0}}"#,
    },
    UnmodeledDefault {
        body_slot: 29,
        wrapped_value: r#"{{0}}"#,
    },
    UnmodeledDefault {
        body_slot: 30,
        wrapped_value: r#"{{0}}"#,
    },
    UnmodeledDefault {
        body_slot: 31,
        wrapped_value: r#"{{0}}"#,
    },
    UnmodeledDefault {
        body_slot: TABLE_PRESENTATION_SLOT,
        wrapped_value: r#"{{0,{0}}}"#,
    },
    UnmodeledDefault {
        body_slot: 34,
        wrapped_value: r#"{{0}}"#,
    },
    UnmodeledDefault {
        body_slot: 35,
        wrapped_value: r#"{{0}}"#,
    },
    UnmodeledDefault {
        body_slot: 38,
        wrapped_value: r#"{00000000-0000-0000-0000-000000000000}"#,
    },
    UnmodeledDefault {
        body_slot: 39,
        wrapped_value: r#"{{"U"}}"#,
    },
    UnmodeledDefault {
        body_slot: 40,
        wrapped_value: r#"{0}"#,
    },
    UnmodeledDefault {
        body_slot: 41,
        wrapped_value: r#"{{1,2,0}}"#,
    },
    UnmodeledDefault {
        body_slot: 42,
        wrapped_value: r#"{0}"#,
    },
    UnmodeledDefault {
        body_slot: 43,
        wrapped_value: r#"{0}"#,
    },
    UnmodeledDefault {
        body_slot: 44,
        wrapped_value: r#"{{0,"","",{0,0,0}}}"#,
    },
    UnmodeledDefault {
        body_slot: 45,
        wrapped_value: r#"{0}"#,
    },
    UnmodeledDefault {
        body_slot: 46,
        wrapped_value: r#"{0}"#,
    },
    UnmodeledDefault {
        body_slot: 47,
        wrapped_value: r#"{00000000-0000-0000-0000-000000000000}"#,
    },
    UnmodeledDefault {
        body_slot: 48,
        wrapped_value: r#"{1}"#,
    },
    UnmodeledDefault {
        body_slot: 49,
        wrapped_value: r#"{{0,0}}"#,
    },
    UnmodeledDefault {
        body_slot: 50,
        wrapped_value: r#"{{1,{0,0}}}"#,
    },
    UnmodeledDefault {
        body_slot: 51,
        wrapped_value: r#"{0}"#,
    },
];

pub const DIMENSION_TABLE_UNMODELED_DEFAULTS: &[UnmodeledDefault] = &[
    UnmodeledDefault {
        body_slot: 10,
        wrapped_value: r#"{00000000-0000-0000-0000-000000000000}"#,
    },
    UnmodeledDefault {
        body_slot: 11,
        wrapped_value: r#"{00000000-0000-0000-0000-000000000000}"#,
    },
    UnmodeledDefault {
        body_slot: 12,
        wrapped_value: r#"{00000000-0000-0000-0000-000000000000}"#,
    },
    UnmodeledDefault {
        body_slot: 15,
        wrapped_value: r#"{00000000-0000-0000-0000-000000000000}"#,
    },
    UnmodeledDefault {
        body_slot: 18,
        wrapped_value: r#"{{0}}"#,
    },
    UnmodeledDefault {
        body_slot: 19,
        wrapped_value: r#"{{0}}"#,
    },
    UnmodeledDefault {
        body_slot: 20,
        wrapped_value: r#"{{0}}"#,
    },
    UnmodeledDefault {
        body_slot: 21,
        wrapped_value: r#"{{0}}"#,
    },
    UnmodeledDefault {
        body_slot: 22,
        wrapped_value: r#"{{0,{0}}}"#,
    },
    UnmodeledDefault {
        body_slot: 23,
        wrapped_value: r#"{{0}}"#,
    },
    UnmodeledDefault {
        body_slot: 24,
        wrapped_value: r#"{0}"#,
    },
    UnmodeledDefault {
        body_slot: 25,
        wrapped_value: r#"{{0,{0}}}"#,
    },
    UnmodeledDefault {
        body_slot: 26,
        wrapped_value: r#"{""}"#,
    },
    UnmodeledDefault {
        body_slot: 27,
        wrapped_value: r#"{0}"#,
    },
    UnmodeledDefault {
        body_slot: 29,
        wrapped_value: r#"{{"U"}}"#,
    },
    UnmodeledDefault {
        body_slot: 30,
        wrapped_value: r#"{0}"#,
    },
];

#[must_use]
pub fn unmodeled_defaults(kind: &str) -> &'static [UnmodeledDefault] {
    match kind {
        CUBE_KIND => CUBE_UNMODELED_DEFAULTS,
        TABLE_KIND => TABLE_UNMODELED_DEFAULTS,
        DIMENSION_TABLE_KIND => DIMENSION_TABLE_UNMODELED_DEFAULTS,
        _ => &[],
    }
}

pub const ROW_COLLECTION_COUNT_SLOT: usize = 2;
pub const ROW_COLLECTIONS_START: usize = 3;
pub const COLLECTION_COUNT_SLOT: usize = 1;
pub const COLLECTION_ITEMS_START: usize = 2;
pub const DIMENSION_PRESENTATION_FIELD_SLOT: usize = 14;
pub const DIMENSION_MEMBER_FIELD_SLOT: usize = 31;
pub const DIMENSION_PARENT_FIELD_SLOT: usize = 32;
pub const KEY_FIELD_REF_TYPE: &str = "157fa490-4ce9-11d4-9415-008048da11f9";
pub const INPUT_BY_STRING_REF_TYPE: &str = "60ea359f-3a6e-48bb-8e71-d2a457572918";
pub const INPUT_BY_STRING_ITEMS_SLOT: usize = 1;

/// Known collections and whether their members can be rendered.
#[must_use]
pub fn collection_is_rendered(kind: &str, id: &str) -> Option<bool> {
    let collections: &[(&str, bool)] = match kind {
        CUBE_KIND => &[
            (CUBE_FORMS_COLLECTION, false),
            (TEMPLATES_COLLECTION, false),
            (CUBE_COMMANDS_COLLECTION, false),
            (CUBE_DIMENSION_TABLES_COLLECTION, true),
            (CUBE_INLINE_DIMENSIONS_COLLECTION, true),
            (CUBE_INLINE_RESOURCES_COLLECTION, true),
        ],
        TABLE_KIND => &[
            (TABLE_FORMS_COLLECTION, false),
            (TEMPLATES_COLLECTION, false),
            (TABLE_COMMANDS_COLLECTION, false),
            (FIELDS_COLLECTION, true),
        ],
        DIMENSION_TABLE_KIND => &[
            (DIMENSION_TABLE_FORMS_COLLECTION, false),
            (TEMPLATES_COLLECTION, false),
            (DIMENSION_TABLE_COMMANDS_COLLECTION, false),
            (FIELDS_COLLECTION, true),
        ],
        _ => &[],
    };
    collections
        .iter()
        .find_map(|(known, rendered)| (*known == id).then_some(*rendered))
}
