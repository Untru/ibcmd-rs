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

/// Whether a kind names an external data source child (`Cube`, `Table`,
/// `DimensionTable`): the families whose forms and templates are separate
/// descriptor rows of a nested owner.
#[must_use]
pub const fn is_child_kind(kind: &str) -> bool {
    matches!(kind.as_bytes(), b"Cube" | b"Table" | b"DimensionTable")
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

/// `Cube` -> the collection its forms are separate rows in, ...
#[must_use]
pub const fn forms_collection(kind: &str) -> &'static str {
    match kind.as_bytes() {
        b"Cube" => CUBE_FORMS_COLLECTION,
        b"Table" => TABLE_FORMS_COLLECTION,
        b"DimensionTable" => DIMENSION_TABLE_FORMS_COLLECTION,
        _ => "",
    }
}

/// `Cube` -> the collection its inline command records live in, ...
#[must_use]
pub const fn commands_collection(kind: &str) -> &'static str {
    match kind.as_bytes() {
        b"Cube" => CUBE_COMMANDS_COLLECTION,
        b"Table" => TABLE_COMMANDS_COLLECTION,
        b"DimensionTable" => DIMENSION_TABLE_COMMANDS_COLLECTION,
        _ => "",
    }
}

/// Unrendered body properties. A slot listed here still holds the measured
/// default of every corpus; any other value is refused instead of silently
/// rewritten. A one-member brace wrapper keeps scalar/string/list distinct.
pub struct UnmodeledDefault {
    pub body_slot: usize,
    pub wrapped_value: &'static str,
}

// Slot map measured on the external-data-source matrix: 162 controlled
// variants, one property each, rows extracted from the platform-saved CF and
// paired with the platform XML. The stored order differs from the Designer
// order, so every property keeps its own named slot.

pub const CUBE_NAME_IN_DATA_SOURCE_SLOT: usize = 14;
pub const CUBE_USE_STANDARD_COMMANDS_SLOT: usize = 15;
pub const CUBE_INCLUDE_HELP_IN_CONTENTS_SLOT: usize = 16;
pub const CUBE_DEFAULT_LIST_FORM_SLOT: usize = 17;
pub const CUBE_DEFAULT_RECORD_FORM_SLOT: usize = 18;
pub const CUBE_RECORD_PRESENTATION_SLOT: usize = 19;
pub const CUBE_EXTENDED_RECORD_PRESENTATION_SLOT: usize = 20;
pub const CUBE_LIST_PRESENTATION_SLOT: usize = 21;
pub const CUBE_EXTENDED_LIST_PRESENTATION_SLOT: usize = 22;
pub const CUBE_EXPLANATION_SLOT: usize = 23;
pub const CUBE_CHARACTERISTICS_SLOT: usize = 24;

pub const TABLE_TABLE_DATA_TYPE_SLOT: usize = 16;
pub const TABLE_NAME_IN_DATA_SOURCE_SLOT: usize = 17;
/// Public PresentationField reference (a bare field uuid).
pub const TABLE_PRESENTATION_FIELD_SLOT: usize = 19;
pub const TABLE_KEY_FIELDS_SLOT: usize = 18;
pub const TABLE_QUICK_CHOICE_SLOT: usize = 20;
pub const TABLE_USE_STANDARD_COMMANDS_SLOT: usize = 21;
pub const TABLE_INCLUDE_HELP_IN_CONTENTS_SLOT: usize = 22;
pub const TABLE_DEFAULT_CHOICE_FORM_SLOT: usize = 23;
pub const TABLE_DEFAULT_LIST_FORM_SLOT: usize = 24;
pub const TABLE_DEFAULT_RECORD_FORM_SLOT: usize = 25;
pub const TABLE_DEFAULT_OBJECT_FORM_SLOT: usize = 26;
pub const TABLE_RECORD_PRESENTATION_SLOT: usize = 27;
pub const TABLE_EXTENDED_RECORD_PRESENTATION_SLOT: usize = 28;
pub const TABLE_LIST_PRESENTATION_SLOT: usize = 29;
pub const TABLE_EXTENDED_LIST_PRESENTATION_SLOT: usize = 30;
pub const TABLE_EXPLANATION_SLOT: usize = 31;
pub const TABLE_CHARACTERISTICS_SLOT: usize = 32;
pub const TABLE_INPUT_BY_STRING_SLOT: usize = 33;
pub const TABLE_PARENT_FIELD_SLOT: usize = 38;
pub const TABLE_UNFILLED_PARENT_VALUE_SLOT: usize = 39;
pub const TABLE_OBJECT_PRESENTATION_SLOT: usize = 34;
pub const TABLE_EXTENDED_OBJECT_PRESENTATION_SLOT: usize = 35;
pub const TABLE_CREATE_ON_INPUT_SLOT: usize = 40;
/// `{search,2,choiceDataGet}`: the two input-by-string modes share one bag.
pub const TABLE_SEARCH_MODES_SLOT: usize = 41;
pub const TABLE_CHOICE_HISTORY_ON_INPUT_SLOT: usize = 42;
pub const TABLE_TYPE_SLOT: usize = 43;
/// `{0,expression,presentation,{0,0,0}}`: the expression source of a table.
pub const TABLE_EXPRESSION_IN_DATA_SOURCE_SLOT: usize = 44;
pub const TABLE_READ_ONLY_SLOT: usize = 45;
pub const TABLE_TRANSACTIONS_ISOLATION_LEVEL_SLOT: usize = 46;
pub const TABLE_DATA_VERSION_FIELD_SLOT: usize = 47;
pub const TABLE_EDIT_TYPE_SLOT: usize = 48;
pub const TABLE_BASED_ON_SLOT: usize = 49;
/// Typed field list, the same wrapper shape as InputByString.
pub const TABLE_DATA_LOCK_FIELDS_SLOT: usize = 50;
pub const TABLE_DATA_LOCK_CONTROL_MODE_SLOT: usize = 51;

pub const DIMENSION_DEFAULT_CHOICE_FORM_SLOT: usize = 10;
pub const DIMENSION_DEFAULT_LIST_FORM_SLOT: usize = 11;
pub const DIMENSION_DEFAULT_OBJECT_FORM_SLOT: usize = 12;
pub const DIMENSION_NAME_IN_DATA_SOURCE_SLOT: usize = 13;
/// The public PresentationField reference (the internal member-field typed
/// reference sits at 14 and is never rendered).
pub const DIMENSION_PRESENTATION_FIELD_SLOT: usize = 15;
pub const DIMENSION_QUICK_CHOICE_SLOT: usize = 16;
pub const DIMENSION_USE_STANDARD_COMMANDS_SLOT: usize = 17;
pub const DIMENSION_LIST_PRESENTATION_SLOT: usize = 18;
pub const DIMENSION_EXTENDED_LIST_PRESENTATION_SLOT: usize = 19;
pub const DIMENSION_OBJECT_PRESENTATION_SLOT: usize = 20;
pub const DIMENSION_EXTENDED_OBJECT_PRESENTATION_SLOT: usize = 21;
pub const DIMENSION_EXPLANATION_SLOT: usize = 23;
pub const DIMENSION_INCLUDE_HELP_IN_CONTENTS_SLOT: usize = 24;
pub const DIMENSION_HIERARCHY_NAME_IN_DATA_SOURCE_SLOT: usize = 26;
pub const DIMENSION_LEVEL_NUMBER_SLOT: usize = 27;
pub const DIMENSION_HIERARCHICAL_SLOT: usize = 30;
pub const DIMENSION_UNFILLED_PARENT_VALUE_SLOT: usize = 29;

/// Every slot of a cube is decoded; the list stays for the shape guard.
pub const CUBE_UNMODELED_DEFAULTS: &[UnmodeledDefault] = &[];

pub const TABLE_UNMODELED_DEFAULTS: &[UnmodeledDefault] = &[];

pub const DIMENSION_TABLE_UNMODELED_DEFAULTS: &[UnmodeledDefault] = &[
    UnmodeledDefault {
        body_slot: 22,
        wrapped_value: r#"{{0,{0}}}"#,
    },
    UnmodeledDefault {
        body_slot: 25,
        wrapped_value: r#"{{0,{0}}}"#,
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
/// The internal typed reference to the `MEMBER_UNIQUE_NAME` field. The
/// platform never writes it into the XML.
pub const DIMENSION_MEMBER_TYPED_REF_SLOT: usize = 14;
pub const DIMENSION_MEMBER_FIELD_SLOT: usize = 31;
pub const DIMENSION_PARENT_FIELD_SLOT: usize = 32;
pub const KEY_FIELD_REF_TYPE: &str = "157fa490-4ce9-11d4-9415-008048da11f9";
pub const INPUT_BY_STRING_REF_TYPE: &str = "60ea359f-3a6e-48bb-8e71-d2a457572918";
pub const INPUT_BY_STRING_ITEMS_SLOT: usize = 1;

/// Known collections and whether their members are rendered inline in the
/// owner's own XML. `false` means the members are separate descriptor rows
/// written as their own files (`Forms/*.xml`, `Templates/*.xml`,
/// `Commands`), so a non-empty collection of that kind is expected.
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
