//! Declarative schema facts and canonical graph types for metadata owners.
//!
//! Raw 1C braced-field decoding remains in the MSSQL physical adapter. This
//! module owns only family layouts, identity semantics, diagnostic vocabulary,
//! and verified EDT produced-type order.

use std::collections::{BTreeMap, BTreeSet};

use ibcmd_xml::schema::{MetadataOrderSection, MetadataOrderVersionPredicate};
use ibcmd_xml::{MetadataOrderError, order_metadata_features};

pub(crate) const ROOT_DISCRIMINATOR: &str = "1";

pub(crate) const CATALOG_ATTRIBUTE_GROUP_UUID: &str = "cf4abea7-37b2-11d4-940f-008048da11f9";
pub(crate) const CATALOG_COMMAND_COLLECTION_UUID: &str = "4fe87c89-9ad4-43f6-9fdb-9dc83b3879c6";
pub(crate) const CATALOG_TABULAR_SECTION_COLLECTION_UUID: &str =
    "932159f9-95b2-4e76-a8dd-8849fe5c5ded";
pub(crate) const CATALOG_FORM_COLLECTION_UUID: &str = "fdf816d2-1ead-11d5-b975-0050bae0a95d";
pub(crate) const DOCUMENT_ATTRIBUTE_GROUP_UUID: &str = "45e46cbc-3e24-4165-8b7b-cc98a6f80211";
pub(crate) const DOCUMENT_TABULAR_SECTION_COLLECTION_UUID: &str =
    "21c53e09-8950-4b5e-a6a0-1054f1bbc274";
pub(crate) const DOCUMENT_COMMAND_COLLECTION_UUID: &str = "b544fc6a-2ba3-4885-8fb2-cb289fb6d65e";
pub(crate) const DOCUMENT_FORM_COLLECTION_UUID: &str = "fb880e93-47d7-4127-9357-a20e69c17545";
pub(crate) const CCT_ATTRIBUTE_COLLECTION_UUID: &str = "31182525-9346-4595-81f8-6f91a72ebe06";
pub(crate) const CCT_TABULAR_SECTION_COLLECTION_UUID: &str = "54e36536-7863-42fd-bea3-c5edd3122fdc";
pub(crate) const CCT_COMMAND_COLLECTION_UUID: &str = "95b5e1d4-abfa-4a16-818d-a5b07b7d3f73";
pub(crate) const CCT_FORM_COLLECTION_UUID: &str = "eb2b78a8-40a6-4b7e-b1b3-6ca9966cbc94";
pub(crate) const BUSINESS_PROCESS_FORM_COLLECTION_UUID: &str =
    "3f7a8120-b71a-4265-98bf-4d9bc09b7719";
pub(crate) const BUSINESS_PROCESS_COMMAND_COLLECTION_UUID: &str =
    "7a3e533c-f232-40d5-a932-6a311d2480bf";
pub(crate) const BUSINESS_PROCESS_ATTRIBUTE_COLLECTION_UUID: &str =
    "87c988de-ecbf-413b-87b0-b9516df05e28";
pub(crate) const BUSINESS_PROCESS_TABULAR_SECTION_COLLECTION_UUID: &str =
    "a3fe6537-d787-40f7-8a06-419d2f0c1cfd";
pub(crate) const METADATA_TEMPLATE_COLLECTION_UUID: &str = "3daea016-69b7-4ed4-9453-127911372fe6";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerGraphFamily {
    Catalog,
    Document,
    BusinessProcess,
    ChartOfCharacteristicTypes,
}

/// A semantically named child collection in an owner record.
///
/// The physical ordering belongs to the native record layout, but consumers
/// must use this role instead of repeating a family-specific numeric index.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum OwnerCollectionRole {
    Template,
    Command,
    TabularSection,
    DirectAttribute,
    Form,
}

impl OwnerCollectionRole {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Template => "template",
            Self::Command => "command",
            Self::TabularSection => "tabular_section",
            Self::DirectAttribute => "direct_attribute",
            Self::Form => "form",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OwnerCollectionLayout {
    pub(crate) role: OwnerCollectionRole,
    pub(crate) index: usize,
    pub(crate) marker: &'static str,
    /// Stable source vocabulary used by diagnostics. It deliberately carries
    /// no native payload or UUID.
    pub(crate) provenance: &'static str,
}

impl OwnerGraphFamily {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Catalog => "Catalog",
            Self::Document => "Document",
            Self::BusinessProcess => "BusinessProcess",
            Self::ChartOfCharacteristicTypes => "ChartOfCharacteristicTypes",
        }
    }

    pub(crate) const fn layout(self) -> OwnerGraphLayout {
        match self {
            Self::Catalog => OwnerGraphLayout {
                owner_field_count: 61,
                owner_discriminators: &["56", "57"],
                owner_header_slot: 9,
                owner_header_encoding: OwnerHeaderEncoding::Wrapped,
                owner_header_unique: true,
                owner_reserved_fields: &[(39, "0")],
                generated_types: CATALOG_GENERATED_TYPES,
                root_collection_count_token: "5",
                collection_markers: &[
                    METADATA_TEMPLATE_COLLECTION_UUID,
                    CATALOG_COMMAND_COLLECTION_UUID,
                    CATALOG_TABULAR_SECTION_COLLECTION_UUID,
                    CATALOG_ATTRIBUTE_GROUP_UUID,
                    CATALOG_FORM_COLLECTION_UUID,
                ],
                collection_layouts: CATALOG_COLLECTIONS,
                produced_types_classifier: "CATALOG_TYPES",
            },
            Self::Document => OwnerGraphLayout {
                owner_field_count: 53,
                owner_discriminators: &["40"],
                owner_header_slot: 9,
                owner_header_encoding: OwnerHeaderEncoding::Wrapped,
                owner_header_unique: false,
                owner_reserved_fields: &[],
                generated_types: DOCUMENT_GENERATED_TYPES,
                root_collection_count_token: "5",
                collection_markers: &[
                    DOCUMENT_TABULAR_SECTION_COLLECTION_UUID,
                    METADATA_TEMPLATE_COLLECTION_UUID,
                    DOCUMENT_ATTRIBUTE_GROUP_UUID,
                    DOCUMENT_COMMAND_COLLECTION_UUID,
                    DOCUMENT_FORM_COLLECTION_UUID,
                ],
                collection_layouts: DOCUMENT_COLLECTIONS,
                produced_types_classifier: "DOCUMENT_TYPES",
            },
            Self::BusinessProcess => OwnerGraphLayout {
                owner_field_count: 49,
                owner_discriminators: &["30"],
                owner_header_slot: 1,
                owner_header_encoding: OwnerHeaderEncoding::Direct,
                owner_header_unique: false,
                owner_reserved_fields: &[],
                generated_types: BUSINESS_PROCESS_GENERATED_TYPES,
                root_collection_count_token: "5",
                collection_markers: &[
                    METADATA_TEMPLATE_COLLECTION_UUID,
                    BUSINESS_PROCESS_FORM_COLLECTION_UUID,
                    BUSINESS_PROCESS_COMMAND_COLLECTION_UUID,
                    BUSINESS_PROCESS_ATTRIBUTE_COLLECTION_UUID,
                    BUSINESS_PROCESS_TABULAR_SECTION_COLLECTION_UUID,
                ],
                collection_layouts: BUSINESS_PROCESS_COLLECTIONS,
                produced_types_classifier: "BUSINESS_PROCESS_TYPES",
            },
            Self::ChartOfCharacteristicTypes => OwnerGraphLayout {
                owner_field_count: 59,
                owner_discriminators: &["34"],
                owner_header_slot: 13,
                owner_header_encoding: OwnerHeaderEncoding::Wrapped,
                owner_header_unique: true,
                owner_reserved_fields: &[],
                generated_types: CCT_GENERATED_TYPES,
                root_collection_count_token: "5",
                collection_markers: &[
                    CCT_ATTRIBUTE_COLLECTION_UUID,
                    METADATA_TEMPLATE_COLLECTION_UUID,
                    CCT_TABULAR_SECTION_COLLECTION_UUID,
                    CCT_COMMAND_COLLECTION_UUID,
                    CCT_FORM_COLLECTION_UUID,
                ],
                collection_layouts: CCT_COLLECTIONS,
                produced_types_classifier: "CHART_OF_CHARACTERISTIC_TYPES_TYPES",
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerHeaderEncoding {
    Direct,
    Wrapped,
}

#[derive(Clone, Copy)]
pub(crate) struct GeneratedTypeLayout {
    pub(crate) type_slot: usize,
    pub(crate) value_slot: usize,
    name_prefix: &'static str,
    xml_category: &'static str,
    order_feature: &'static str,
}

#[derive(Clone, Copy)]
pub(crate) struct OwnerGraphLayout {
    pub(crate) owner_field_count: usize,
    pub(crate) owner_discriminators: &'static [&'static str],
    pub(crate) owner_header_slot: usize,
    pub(crate) owner_header_encoding: OwnerHeaderEncoding,
    pub(crate) owner_header_unique: bool,
    pub(crate) owner_reserved_fields: &'static [(usize, &'static str)],
    pub(crate) generated_types: &'static [GeneratedTypeLayout],
    pub(crate) root_collection_count_token: &'static str,
    pub(crate) collection_markers: &'static [&'static str],
    collection_layouts: &'static [OwnerCollectionLayout],
    produced_types_classifier: &'static str,
}

impl OwnerGraphLayout {
    pub(crate) fn collection_layout(
        self,
        role: OwnerCollectionRole,
    ) -> Option<OwnerCollectionLayout> {
        self.collection_layouts
            .iter()
            .copied()
            .find(|layout| layout.role == role)
    }

    pub(crate) fn collection_layout_at(self, index: usize) -> Option<OwnerCollectionLayout> {
        self.collection_layouts.get(index).copied()
    }

    pub(crate) fn collection_layouts(self) -> &'static [OwnerCollectionLayout] {
        self.collection_layouts
    }
}

const fn collection(
    role: OwnerCollectionRole,
    index: usize,
    marker: &'static str,
    provenance: &'static str,
) -> OwnerCollectionLayout {
    OwnerCollectionLayout {
        role,
        index,
        marker,
        provenance,
    }
}

const CATALOG_COLLECTIONS: &[OwnerCollectionLayout] = &[
    collection(
        OwnerCollectionRole::Template,
        0,
        METADATA_TEMPLATE_COLLECTION_UUID,
        "catalog.template",
    ),
    collection(
        OwnerCollectionRole::Command,
        1,
        CATALOG_COMMAND_COLLECTION_UUID,
        "catalog.command",
    ),
    collection(
        OwnerCollectionRole::TabularSection,
        2,
        CATALOG_TABULAR_SECTION_COLLECTION_UUID,
        "catalog.tabular_section",
    ),
    collection(
        OwnerCollectionRole::DirectAttribute,
        3,
        CATALOG_ATTRIBUTE_GROUP_UUID,
        "catalog.direct_attribute",
    ),
    collection(
        OwnerCollectionRole::Form,
        4,
        CATALOG_FORM_COLLECTION_UUID,
        "catalog.form",
    ),
];

const DOCUMENT_COLLECTIONS: &[OwnerCollectionLayout] = &[
    collection(
        OwnerCollectionRole::TabularSection,
        0,
        DOCUMENT_TABULAR_SECTION_COLLECTION_UUID,
        "document.tabular_section",
    ),
    collection(
        OwnerCollectionRole::Template,
        1,
        METADATA_TEMPLATE_COLLECTION_UUID,
        "document.template",
    ),
    collection(
        OwnerCollectionRole::DirectAttribute,
        2,
        DOCUMENT_ATTRIBUTE_GROUP_UUID,
        "document.direct_attribute",
    ),
    collection(
        OwnerCollectionRole::Command,
        3,
        DOCUMENT_COMMAND_COLLECTION_UUID,
        "document.command",
    ),
    collection(
        OwnerCollectionRole::Form,
        4,
        DOCUMENT_FORM_COLLECTION_UUID,
        "document.form",
    ),
];

const BUSINESS_PROCESS_COLLECTIONS: &[OwnerCollectionLayout] = &[
    collection(
        OwnerCollectionRole::Template,
        0,
        METADATA_TEMPLATE_COLLECTION_UUID,
        "business_process.template",
    ),
    collection(
        OwnerCollectionRole::Form,
        1,
        BUSINESS_PROCESS_FORM_COLLECTION_UUID,
        "business_process.form",
    ),
    collection(
        OwnerCollectionRole::Command,
        2,
        BUSINESS_PROCESS_COMMAND_COLLECTION_UUID,
        "business_process.command",
    ),
    collection(
        OwnerCollectionRole::DirectAttribute,
        3,
        BUSINESS_PROCESS_ATTRIBUTE_COLLECTION_UUID,
        "business_process.direct_attribute",
    ),
    collection(
        OwnerCollectionRole::TabularSection,
        4,
        BUSINESS_PROCESS_TABULAR_SECTION_COLLECTION_UUID,
        "business_process.tabular_section",
    ),
];

const CCT_COLLECTIONS: &[OwnerCollectionLayout] = &[
    collection(
        OwnerCollectionRole::DirectAttribute,
        0,
        CCT_ATTRIBUTE_COLLECTION_UUID,
        "chart_of_characteristic_types.direct_attribute",
    ),
    collection(
        OwnerCollectionRole::Template,
        1,
        METADATA_TEMPLATE_COLLECTION_UUID,
        "chart_of_characteristic_types.template",
    ),
    collection(
        OwnerCollectionRole::TabularSection,
        2,
        CCT_TABULAR_SECTION_COLLECTION_UUID,
        "chart_of_characteristic_types.tabular_section",
    ),
    collection(
        OwnerCollectionRole::Command,
        3,
        CCT_COMMAND_COLLECTION_UUID,
        "chart_of_characteristic_types.command",
    ),
    collection(
        OwnerCollectionRole::Form,
        4,
        CCT_FORM_COLLECTION_UUID,
        "chart_of_characteristic_types.form",
    ),
];

const fn generated(
    type_slot: usize,
    value_slot: usize,
    name_prefix: &'static str,
    xml_category: &'static str,
    order_feature: &'static str,
) -> GeneratedTypeLayout {
    GeneratedTypeLayout {
        type_slot,
        value_slot,
        name_prefix,
        xml_category,
        order_feature,
    }
}

const CATALOG_GENERATED_TYPES: &[GeneratedTypeLayout] = &[
    generated(
        1,
        2,
        "CatalogObject",
        "Object",
        "BASIC_DB_OBJECT_TYPES__OBJECT_TYPE",
    ),
    generated(3, 4, "CatalogRef", "Ref", "BASIC_DB_OBJECT_TYPES__REF_TYPE"),
    generated(
        5,
        6,
        "CatalogSelection",
        "Selection",
        "BASIC_DB_OBJECT_TYPES__SELECTION_TYPE",
    ),
    generated(
        7,
        8,
        "CatalogList",
        "List",
        "BASIC_DB_OBJECT_TYPES__LIST_TYPE",
    ),
    generated(
        34,
        35,
        "CatalogManager",
        "Manager",
        "BASIC_DB_OBJECT_TYPES__MANAGER_TYPE",
    ),
];

const DOCUMENT_GENERATED_TYPES: &[GeneratedTypeLayout] = &[
    generated(
        1,
        2,
        "DocumentObject",
        "Object",
        "BASIC_DB_OBJECT_TYPES__OBJECT_TYPE",
    ),
    generated(
        3,
        4,
        "DocumentRef",
        "Ref",
        "BASIC_DB_OBJECT_TYPES__REF_TYPE",
    ),
    generated(
        5,
        6,
        "DocumentSelection",
        "Selection",
        "BASIC_DB_OBJECT_TYPES__SELECTION_TYPE",
    ),
    generated(
        7,
        8,
        "DocumentList",
        "List",
        "BASIC_DB_OBJECT_TYPES__LIST_TYPE",
    ),
    generated(
        26,
        27,
        "DocumentManager",
        "Manager",
        "BASIC_DB_OBJECT_TYPES__MANAGER_TYPE",
    ),
];

const BUSINESS_PROCESS_GENERATED_TYPES: &[GeneratedTypeLayout] = &[
    generated(
        3,
        4,
        "BusinessProcessObject",
        "Object",
        "BASIC_DB_OBJECT_TYPES__OBJECT_TYPE",
    ),
    generated(
        5,
        6,
        "BusinessProcessRef",
        "Ref",
        "BASIC_DB_OBJECT_TYPES__REF_TYPE",
    ),
    generated(
        7,
        8,
        "BusinessProcessSelection",
        "Selection",
        "BASIC_DB_OBJECT_TYPES__SELECTION_TYPE",
    ),
    generated(
        9,
        10,
        "BusinessProcessList",
        "List",
        "BASIC_DB_OBJECT_TYPES__LIST_TYPE",
    ),
    generated(
        11,
        12,
        "BusinessProcessManager",
        "Manager",
        "BASIC_DB_OBJECT_TYPES__MANAGER_TYPE",
    ),
    generated(
        13,
        14,
        "BusinessProcessRoutePointRef",
        "RoutePointRef",
        "BUSINESS_PROCESS_TYPES__ROUTE_POINT_REF",
    ),
];

const CCT_GENERATED_TYPES: &[GeneratedTypeLayout] = &[
    generated(
        1,
        2,
        "ChartOfCharacteristicTypesObject",
        "Object",
        "BASIC_DB_OBJECT_TYPES__OBJECT_TYPE",
    ),
    generated(
        3,
        4,
        "ChartOfCharacteristicTypesRef",
        "Ref",
        "BASIC_DB_OBJECT_TYPES__REF_TYPE",
    ),
    generated(
        5,
        6,
        "ChartOfCharacteristicTypesSelection",
        "Selection",
        "BASIC_DB_OBJECT_TYPES__SELECTION_TYPE",
    ),
    generated(
        7,
        8,
        "ChartOfCharacteristicTypesList",
        "List",
        "BASIC_DB_OBJECT_TYPES__LIST_TYPE",
    ),
    generated(
        9,
        10,
        "Characteristic",
        "Characteristic",
        "CHART_OF_CHARACTERISTIC_TYPES_TYPES__CONTAINER_TYPE",
    ),
    generated(
        11,
        12,
        "ChartOfCharacteristicTypesManager",
        "Manager",
        "BASIC_DB_OBJECT_TYPES__MANAGER_TYPE",
    ),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerIdentityRole {
    Root,
    GeneratedType,
    GeneratedValue,
    Child,
    CommandValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GeneratedIdentityRole {
    Type,
    Value,
}

impl From<GeneratedIdentityRole> for OwnerIdentityRole {
    fn from(value: GeneratedIdentityRole) -> Self {
        match value {
            GeneratedIdentityRole::Type => Self::GeneratedType,
            GeneratedIdentityRole::Value => Self::GeneratedValue,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnerIdentityLedger {
    by_uuid: BTreeMap<String, OwnerIdentityRole>,
    /// Command value UUIDs are native type/value proofs, not owned child
    /// identities: several commands are allowed to share one.  Keep them in
    /// the same redacted ledger so all decoded command slots are accounted for
    /// without turning a valid shared value into a false child collision.
    command_value_ids: BTreeSet<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OwnerIdentityCollision {
    pub(crate) previous: OwnerIdentityRole,
    pub(crate) field_index: usize,
    pub(crate) collection_role: Option<OwnerCollectionRole>,
}

impl OwnerIdentityLedger {
    pub(crate) fn new(root_uuid: String) -> Self {
        Self {
            by_uuid: BTreeMap::from([(root_uuid.to_ascii_lowercase(), OwnerIdentityRole::Root)]),
            command_value_ids: BTreeSet::new(),
        }
    }

    pub(crate) fn insert_generated(
        &mut self,
        uuid: String,
        field_index: usize,
        role: GeneratedIdentityRole,
    ) -> Result<(), OwnerIdentityCollision> {
        let key = uuid.to_ascii_lowercase();
        if let Some(previous) = self.by_uuid.get(&key) {
            return Err(OwnerIdentityCollision {
                previous: *previous,
                field_index,
                collection_role: None,
            });
        }
        if self.command_value_ids.contains(&key) {
            return Err(OwnerIdentityCollision {
                previous: OwnerIdentityRole::CommandValue,
                field_index,
                collection_role: None,
            });
        }
        self.by_uuid.insert(key, role.into());
        Ok(())
    }

    /// Records a UUID belonging to a child record. The collection role is
    /// retained only as typed provenance for a collision; raw UUID values are
    /// never exposed by the error.
    pub(crate) fn insert_child(
        &mut self,
        uuid: String,
        field_index: usize,
        collection_role: OwnerCollectionRole,
    ) -> Result<(), OwnerIdentityCollision> {
        let key = uuid.to_ascii_lowercase();
        if let Some(previous) = self.by_uuid.get(&key) {
            return Err(OwnerIdentityCollision {
                previous: *previous,
                field_index,
                collection_role: Some(collection_role),
            });
        }
        if self.command_value_ids.contains(&key) {
            return Err(OwnerIdentityCollision {
                previous: OwnerIdentityRole::CommandValue,
                field_index,
                collection_role: Some(collection_role),
            });
        }
        self.by_uuid.insert(key, OwnerIdentityRole::Child);
        Ok(())
    }

    pub(crate) fn contains(&self, uuid: &str) -> bool {
        self.by_uuid.contains_key(&uuid.to_ascii_lowercase())
    }

    pub(crate) fn observe_command_value(
        &mut self,
        uuid: String,
        field_index: usize,
        collection_role: OwnerCollectionRole,
    ) -> Result<(), OwnerIdentityCollision> {
        let key = uuid.to_ascii_lowercase();
        if let Some(previous) = self.by_uuid.get(&key) {
            return Err(OwnerIdentityCollision {
                previous: *previous,
                field_index,
                collection_role: Some(collection_role),
            });
        }
        self.command_value_ids.insert(key);
        Ok(())
    }

    pub(crate) fn generated_identities(&self) -> BTreeSet<String> {
        self.by_uuid
            .iter()
            .filter_map(|(uuid, role)| {
                matches!(
                    role,
                    OwnerIdentityRole::GeneratedType | OwnerIdentityRole::GeneratedValue
                )
                .then_some(uuid.clone())
            })
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DecodedGeneratedType {
    name: String,
    category: &'static str,
    type_id: String,
    value_id: String,
}

impl DecodedGeneratedType {
    pub(crate) fn new(
        layout: GeneratedTypeLayout,
        owner_name: &str,
        type_id: String,
        value_id: String,
    ) -> Self {
        Self {
            name: format!("{}.{}", layout.name_prefix, owner_name),
            category: layout.xml_category,
            type_id,
            value_id,
        }
    }

    pub(crate) fn into_parts(self) -> (String, &'static str, String, String) {
        (self.name, self.category, self.type_id, self.value_id)
    }
}

#[derive(Debug)]
pub(crate) struct DecodedOwnerCollection<'a> {
    pub(crate) items: Vec<&'a str>,
    provenance: OwnerCollectionProvenance,
}

impl<'a> DecodedOwnerCollection<'a> {
    pub(crate) fn new(items: Vec<&'a str>, provenance: OwnerCollectionProvenance) -> Self {
        Self { items, provenance }
    }

    pub(crate) const fn provenance(&self) -> OwnerCollectionProvenance {
        self.provenance
    }
}

/// A redacted, typed description of a collection location. This is the
/// schema-side contract retained by each `DecodedOwnerCollection`; it contains
/// no raw collection fields or UUIDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OwnerCollectionProvenance {
    pub(crate) role: OwnerCollectionRole,
    pub(crate) index: usize,
    pub(crate) marker: &'static str,
    pub(crate) source: &'static str,
}

impl OwnerCollectionProvenance {
    pub(crate) const fn from_layout(layout: OwnerCollectionLayout) -> Self {
        Self {
            role: layout.role,
            index: layout.index,
            marker: layout.marker,
            source: layout.provenance,
        }
    }
}

pub(crate) struct DecodedOwnerGraph<'a> {
    pub(crate) generated_types: Vec<DecodedGeneratedType>,
    pub(crate) identities: OwnerIdentityLedger,
    pub(crate) owner_fields: Vec<&'a str>,
    pub(crate) collections: Vec<DecodedOwnerCollection<'a>>,
}

impl<'a> DecodedOwnerGraph<'a> {
    /// Resolves an owner child collection by semantic role. This remains
    /// fail-closed while the physical adapter is migrated: the declared index
    /// must exist, and its marker is available from the same schema layout.
    pub(crate) fn collection(
        &self,
        family: OwnerGraphFamily,
        role: OwnerCollectionRole,
    ) -> Result<&DecodedOwnerCollection<'a>, OwnerGraphDiagnosticEvidence> {
        let Some(layout) = family.layout().collection_layout(role) else {
            return Err(OwnerGraphDiagnosticEvidence::missing_collection(role));
        };
        let collection = self
            .collections
            .get(layout.index)
            .ok_or_else(|| OwnerGraphDiagnosticEvidence::missing_collection(role))?;
        (collection.provenance.role == role
            && collection.provenance.index == layout.index
            && collection
                .provenance
                .marker
                .eq_ignore_ascii_case(layout.marker)
            && collection.provenance.source == layout.provenance)
            .then_some(collection)
            .ok_or_else(|| OwnerGraphDiagnosticEvidence::role_mismatch(role, layout.index))
    }

    pub(crate) fn collection_provenance(
        family: OwnerGraphFamily,
        role: OwnerCollectionRole,
    ) -> Result<OwnerCollectionProvenance, OwnerGraphDiagnosticEvidence> {
        family
            .layout()
            .collection_layout(role)
            .map(OwnerCollectionProvenance::from_layout)
            .ok_or_else(|| OwnerGraphDiagnosticEvidence::missing_collection(role))
    }
}

pub(crate) fn order_generated_types(
    family: OwnerGraphFamily,
    generated_types: Vec<DecodedGeneratedType>,
) -> Result<Vec<DecodedGeneratedType>, MetadataOrderError> {
    let layout = family.layout();
    if generated_types.len() != layout.generated_types.len() {
        return Err(MetadataOrderError::AmbiguousProducedType {
            classifier: layout.produced_types_classifier.to_owned(),
            category: "inventory".to_owned(),
        });
    }
    let mut by_feature = BTreeMap::new();
    for (value, generated) in generated_types.into_iter().zip(layout.generated_types) {
        if value.category != generated.xml_category {
            return Err(MetadataOrderError::AmbiguousProducedType {
                classifier: layout.produced_types_classifier.to_owned(),
                category: value.category.to_owned(),
            });
        }
        if by_feature
            .insert(generated.order_feature.to_owned(), value)
            .is_some()
        {
            return Err(MetadataOrderError::DuplicateFeature(
                generated.order_feature.to_owned(),
            ));
        }
    }
    let baseline = by_feature.keys().cloned().collect::<Vec<_>>();
    let ordered_features = order_metadata_features(
        layout.produced_types_classifier,
        MetadataOrderSection::ProducedTypes,
        MetadataOrderVersionPredicate::Always,
        &baseline,
    )?;
    Ok(ordered_features
        .into_iter()
        .map(|feature| {
            by_feature
                .remove(&feature)
                .expect("EDT order was validated against owner-graph features")
        })
        .collect())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerGraphDiagnosticClass {
    Malformed,
    Unsupported,
    Invariant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerGraphReference {
    Root,
    CollectionMarker,
    ChildUuid,
    OwnerHeader,
    GeneratedType,
    GeneratedValue,
    OwnedForm,
    OwnedTemplate,
    OwnedCommand,
}

const OWNER_GRAPH_REFERENCE_OWNED_FORM: &str = "owned_form";
const OWNER_GRAPH_REFERENCE_OWNED_TEMPLATE: &str = "owned_template";
const OWNER_GRAPH_REFERENCE_OWNED_COMMAND: &str = "owned_command";
const OWNER_IDENTITY_ROLE_COMMAND_VALUE: &str = "command_value_id";
const OWNER_GRAPH_OWNED_CHILD_REASON_TOKENS: [&str; 8] = [
    "missing",
    "ambiguous",
    "unexpected",
    "wrong_kind",
    "wrong_owner",
    "header_mismatch",
    "declaration_order",
    "property_parse",
];

/// Redacted, closed vocabulary for post-root owned-child resolution failures.
#[repr(usize)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerGraphOwnedChildReason {
    Missing,
    Ambiguous,
    Unexpected,
    WrongKind,
    WrongOwner,
    HeaderMismatch,
    DeclarationOrder,
    PropertyParse,
}

impl OwnerGraphOwnedChildReason {
    pub(crate) const fn as_str(self) -> &'static str {
        OWNER_GRAPH_OWNED_CHILD_REASON_TOKENS[self as usize]
    }
}

impl OwnerGraphReference {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Root => "root_uuid",
            Self::CollectionMarker => "collection_marker",
            Self::ChildUuid => "child_uuid",
            Self::OwnerHeader => "owner_header",
            Self::GeneratedType => "generated_type_id",
            Self::GeneratedValue => "generated_value_id",
            Self::OwnedForm => OWNER_GRAPH_REFERENCE_OWNED_FORM,
            Self::OwnedTemplate => OWNER_GRAPH_REFERENCE_OWNED_TEMPLATE,
            Self::OwnedCommand => OWNER_GRAPH_REFERENCE_OWNED_COMMAND,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerGraphDiagnosticKind {
    RootBracedShape,
    RootShape,
    RootCollectionCount,
    CollectionBracedShape,
    CollectionMinimumShape,
    CollectionMarker,
    CollectionCount,
    CollectionCountMismatch,
    MissingCollection,
    CollectionRoleMismatch,
    OwnerFieldsBracedShape,
    OwnerFieldCount,
    OwnerDiscriminator,
    OwnerReservedField,
    OwnerHeaderShape,
    OwnerHeaderMismatch,
    OwnerHeaderPlacement,
    GeneratedTypeUuidSyntax,
    GeneratedTypeNilUuid,
    GeneratedValueUuidSyntax,
    GeneratedValueNilUuid,
    ChildUuidSyntax,
    ChildUuidNilUuid,
    ChildIdentityCollision,
    OwnedChildReference,
    DuplicateIdentity,
    EdtFeatureOrder,
}

const OWNER_GRAPH_OWNED_CHILD_REFERENCE_FACTS: (OwnerGraphDiagnosticClass, &str, &str) = (
    OwnerGraphDiagnosticClass::Invariant,
    "owner_graph_owned_child",
    "owned_child_reference",
);

impl OwnerGraphDiagnosticKind {
    pub(crate) const fn facts(self) -> (OwnerGraphDiagnosticClass, &'static str, &'static str) {
        use OwnerGraphDiagnosticClass::{Invariant, Malformed, Unsupported};
        match self {
            Self::RootBracedShape => (Malformed, "owner_graph_root", "root_braced_shape"),
            Self::RootShape => (Malformed, "owner_graph_root", "root_shape"),
            Self::RootCollectionCount => (Malformed, "owner_graph_root", "root_collection_count"),
            Self::CollectionBracedShape => (
                Malformed,
                "owner_graph_collection",
                "collection_braced_shape",
            ),
            Self::CollectionMinimumShape => (
                Malformed,
                "owner_graph_collection",
                "collection_minimum_shape",
            ),
            Self::CollectionMarker => (Invariant, "owner_graph_collection", "collection_marker"),
            Self::CollectionCount => (Malformed, "owner_graph_collection", "collection_count"),
            Self::CollectionCountMismatch => (
                Malformed,
                "owner_graph_collection",
                "collection_count_mismatch",
            ),
            Self::MissingCollection => (Invariant, "owner_graph_collection", "missing_collection"),
            Self::CollectionRoleMismatch => (
                Invariant,
                "owner_graph_collection",
                "collection_role_mismatch",
            ),
            Self::OwnerFieldsBracedShape => {
                (Malformed, "owner_graph_fields", "owner_fields_braced_shape")
            }
            Self::OwnerFieldCount => (Malformed, "owner_graph_fields", "owner_field_count"),
            Self::OwnerDiscriminator => (Unsupported, "owner_graph_fields", "owner_discriminator"),
            Self::OwnerReservedField => (Invariant, "owner_graph_fields", "owner_reserved_field"),
            Self::OwnerHeaderShape => (Malformed, "owner_graph_header", "owner_header_shape"),
            Self::OwnerHeaderMismatch => (Invariant, "owner_graph_header", "owner_header_mismatch"),
            Self::OwnerHeaderPlacement => {
                (Invariant, "owner_graph_header", "owner_header_placement")
            }
            Self::GeneratedTypeUuidSyntax => (Malformed, "generated_type_id", "uuid_syntax"),
            Self::GeneratedTypeNilUuid => (Invariant, "generated_type_id", "nil_uuid"),
            Self::GeneratedValueUuidSyntax => (Malformed, "generated_value_id", "uuid_syntax"),
            Self::GeneratedValueNilUuid => (Invariant, "generated_value_id", "nil_uuid"),
            Self::ChildUuidSyntax => (Malformed, "child_uuid", "uuid_syntax"),
            Self::ChildUuidNilUuid => (Invariant, "child_uuid", "nil_uuid"),
            Self::ChildIdentityCollision => (
                Invariant,
                "owner_identity_ledger",
                "child_identity_collision",
            ),
            Self::OwnedChildReference => OWNER_GRAPH_OWNED_CHILD_REFERENCE_FACTS,
            Self::DuplicateIdentity => (Invariant, "owner_identity_ledger", "duplicate_identity"),
            Self::EdtFeatureOrder => (Invariant, "produced_type_order", "edt_feature_order"),
        }
    }
}

/// Typed diagnostic evidence suitable for a user-facing extraction
/// diagnostic. It intentionally excludes native raw fields, names and UUID
/// values: callers receive only stable schema facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OwnerGraphDiagnosticEvidence {
    pub(crate) kind: OwnerGraphDiagnosticKind,
    pub(crate) reference: Option<OwnerGraphReference>,
    pub(crate) field_index: Option<usize>,
    pub(crate) collection_index: Option<usize>,
    pub(crate) collection_role: Option<OwnerCollectionRole>,
}

impl OwnerGraphDiagnosticEvidence {
    pub(crate) const fn missing_collection(role: OwnerCollectionRole) -> Self {
        Self {
            kind: OwnerGraphDiagnosticKind::MissingCollection,
            reference: Some(OwnerGraphReference::CollectionMarker),
            field_index: None,
            collection_index: None,
            collection_role: Some(role),
        }
    }

    pub(crate) const fn role_mismatch(role: OwnerCollectionRole, index: usize) -> Self {
        Self {
            kind: OwnerGraphDiagnosticKind::CollectionRoleMismatch,
            reference: Some(OwnerGraphReference::CollectionMarker),
            field_index: None,
            collection_index: Some(index),
            collection_role: Some(role),
        }
    }

    pub(crate) const fn child_uuid(
        kind: OwnerGraphDiagnosticKind,
        field_index: usize,
        role: OwnerCollectionRole,
    ) -> Self {
        Self {
            kind,
            reference: Some(OwnerGraphReference::ChildUuid),
            field_index: Some(field_index),
            collection_index: None,
            collection_role: Some(role),
        }
    }

    pub(crate) const fn owned_child(
        role: OwnerCollectionRole,
        item_index: usize,
        reference: OwnerGraphReference,
    ) -> Self {
        Self {
            kind: OwnerGraphDiagnosticKind::OwnedChildReference,
            reference: Some(reference),
            field_index: None,
            collection_index: Some(item_index),
            collection_role: Some(role),
        }
    }
}

impl OwnerIdentityRole {
    pub(crate) const fn diagnostic_reference(self) -> &'static str {
        match self {
            Self::Root => "root_uuid",
            Self::GeneratedType => "generated_type_id",
            Self::GeneratedValue => "generated_value_id",
            Self::Child => "child_uuid",
            Self::CommandValue => OWNER_IDENTITY_ROLE_COMMAND_VALUE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_family_layouts_and_edt_order_are_declarative() {
        for family in [
            OwnerGraphFamily::Catalog,
            OwnerGraphFamily::Document,
            OwnerGraphFamily::BusinessProcess,
            OwnerGraphFamily::ChartOfCharacteristicTypes,
        ] {
            let layout = family.layout();
            assert_eq!(layout.collection_markers.len(), 5);
            let generated = layout
                .generated_types
                .iter()
                .enumerate()
                .map(|(index, definition)| {
                    DecodedGeneratedType::new(
                        *definition,
                        "Owner",
                        format!("type-{index}"),
                        format!("value-{index}"),
                    )
                })
                .collect();
            let expected: &[(&str, &str)] = match family {
                OwnerGraphFamily::Catalog => &[
                    ("CatalogObject", "Object"),
                    ("CatalogRef", "Ref"),
                    ("CatalogSelection", "Selection"),
                    ("CatalogList", "List"),
                    ("CatalogManager", "Manager"),
                ],
                OwnerGraphFamily::Document => &[
                    ("DocumentObject", "Object"),
                    ("DocumentRef", "Ref"),
                    ("DocumentSelection", "Selection"),
                    ("DocumentList", "List"),
                    ("DocumentManager", "Manager"),
                ],
                OwnerGraphFamily::BusinessProcess => &[
                    ("BusinessProcessObject", "Object"),
                    ("BusinessProcessRef", "Ref"),
                    ("BusinessProcessSelection", "Selection"),
                    ("BusinessProcessList", "List"),
                    ("BusinessProcessManager", "Manager"),
                    ("BusinessProcessRoutePointRef", "RoutePointRef"),
                ],
                OwnerGraphFamily::ChartOfCharacteristicTypes => &[
                    ("ChartOfCharacteristicTypesObject", "Object"),
                    ("ChartOfCharacteristicTypesRef", "Ref"),
                    ("ChartOfCharacteristicTypesSelection", "Selection"),
                    ("ChartOfCharacteristicTypesList", "List"),
                    ("Characteristic", "Characteristic"),
                    ("ChartOfCharacteristicTypesManager", "Manager"),
                ],
            };
            let actual = order_generated_types(family, generated)
                .unwrap()
                .into_iter()
                .map(|generated| {
                    let (name, category, _, _) = generated.into_parts();
                    (
                        name.split_once('.')
                            .expect("generated type name has owner suffix")
                            .0
                            .to_owned(),
                        category,
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(
                actual,
                expected
                    .iter()
                    .map(|(prefix, category)| ((*prefix).to_owned(), *category))
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn identity_ledger_distinguishes_duplicate_roles_and_root_collision() {
        assert_eq!(
            OwnerIdentityRole::from(GeneratedIdentityRole::Type),
            OwnerIdentityRole::GeneratedType
        );
        assert_eq!(
            OwnerIdentityRole::from(GeneratedIdentityRole::Value),
            OwnerIdentityRole::GeneratedValue
        );
        let mut ledger = OwnerIdentityLedger::new("root".to_owned());
        ledger
            .insert_generated("type".to_owned(), 1, GeneratedIdentityRole::Type)
            .unwrap();
        let duplicate = ledger
            .insert_generated("TYPE".to_owned(), 2, GeneratedIdentityRole::Value)
            .unwrap_err();
        assert_eq!(duplicate.previous, OwnerIdentityRole::GeneratedType);
        let root = ledger
            .insert_generated("ROOT".to_owned(), 3, GeneratedIdentityRole::Type)
            .unwrap_err();
        assert_eq!(root.previous, OwnerIdentityRole::Root);
    }

    #[test]
    fn collection_roles_are_exact_for_all_four_owner_families() {
        let expected = [
            (
                OwnerGraphFamily::Catalog,
                [
                    OwnerCollectionRole::Template,
                    OwnerCollectionRole::Command,
                    OwnerCollectionRole::TabularSection,
                    OwnerCollectionRole::DirectAttribute,
                    OwnerCollectionRole::Form,
                ],
            ),
            (
                OwnerGraphFamily::Document,
                [
                    OwnerCollectionRole::TabularSection,
                    OwnerCollectionRole::Template,
                    OwnerCollectionRole::DirectAttribute,
                    OwnerCollectionRole::Command,
                    OwnerCollectionRole::Form,
                ],
            ),
            (
                OwnerGraphFamily::BusinessProcess,
                [
                    OwnerCollectionRole::Template,
                    OwnerCollectionRole::Form,
                    OwnerCollectionRole::Command,
                    OwnerCollectionRole::DirectAttribute,
                    OwnerCollectionRole::TabularSection,
                ],
            ),
            (
                OwnerGraphFamily::ChartOfCharacteristicTypes,
                [
                    OwnerCollectionRole::DirectAttribute,
                    OwnerCollectionRole::Template,
                    OwnerCollectionRole::TabularSection,
                    OwnerCollectionRole::Command,
                    OwnerCollectionRole::Form,
                ],
            ),
        ];

        for (family, roles) in expected {
            let layout = family.layout();
            assert_eq!(layout.collection_layouts().len(), roles.len());
            assert_eq!(layout.collection_markers.len(), roles.len());
            for (index, role) in roles.into_iter().enumerate() {
                let declared = layout.collection_layout(role).unwrap();
                assert_eq!(declared.index, index);
                assert_eq!(layout.collection_layout_at(index), Some(declared));
                assert_eq!(declared.marker, layout.collection_markers[index]);
                assert!(!declared.provenance.is_empty());
                assert_eq!(role.as_str(), declared.role.as_str());
            }
        }
    }

    #[test]
    fn role_accessors_are_deterministic_and_fail_closed() {
        let family = OwnerGraphFamily::Document;
        let collections = family
            .layout()
            .collection_layouts()
            .iter()
            .map(|layout| {
                DecodedOwnerCollection::new(
                    vec![layout.provenance],
                    OwnerCollectionProvenance::from_layout(*layout),
                )
            })
            .collect();
        let graph = DecodedOwnerGraph {
            generated_types: Vec::new(),
            identities: OwnerIdentityLedger::new("root".to_owned()),
            owner_fields: Vec::new(),
            collections,
        };
        let forms = graph.collection(family, OwnerCollectionRole::Form).unwrap();
        assert_eq!(forms.items, vec!["document.form"]);
        assert_eq!(forms.provenance().index, 4);
        assert_eq!(
            DecodedOwnerGraph::collection_provenance(family, OwnerCollectionRole::Form)
                .unwrap()
                .marker,
            DOCUMENT_FORM_COLLECTION_UUID
        );

        let truncated = DecodedOwnerGraph {
            generated_types: Vec::new(),
            identities: OwnerIdentityLedger::new("root".to_owned()),
            owner_fields: Vec::new(),
            collections: Vec::new(),
        };
        assert_eq!(
            truncated
                .collection(family, OwnerCollectionRole::Form)
                .unwrap_err()
                .kind,
            OwnerGraphDiagnosticKind::MissingCollection
        );

        let wrong = DecodedOwnerGraph {
            generated_types: Vec::new(),
            identities: OwnerIdentityLedger::new("root".to_owned()),
            owner_fields: Vec::new(),
            collections: vec![DecodedOwnerCollection::new(
                Vec::new(),
                OwnerCollectionProvenance::from_layout(
                    OwnerGraphFamily::Catalog
                        .layout()
                        .collection_layout(OwnerCollectionRole::Template)
                        .unwrap(),
                ),
            )],
        };
        assert_eq!(
            wrong
                .collection(
                    OwnerGraphFamily::Document,
                    OwnerCollectionRole::TabularSection
                )
                .unwrap_err()
                .kind,
            OwnerGraphDiagnosticKind::CollectionRoleMismatch
        );

        for (actual_family, queried_family) in [
            (OwnerGraphFamily::Catalog, OwnerGraphFamily::BusinessProcess),
            (
                OwnerGraphFamily::Document,
                OwnerGraphFamily::ChartOfCharacteristicTypes,
            ),
        ] {
            let collections = actual_family
                .layout()
                .collection_layouts()
                .iter()
                .map(|layout| {
                    DecodedOwnerCollection::new(
                        Vec::new(),
                        OwnerCollectionProvenance::from_layout(*layout),
                    )
                })
                .collect();
            let cross_family = DecodedOwnerGraph {
                generated_types: Vec::new(),
                identities: OwnerIdentityLedger::new("root".to_owned()),
                owner_fields: Vec::new(),
                collections,
            };
            assert_eq!(
                cross_family
                    .collection(queried_family, OwnerCollectionRole::Template)
                    .unwrap_err()
                    .kind,
                OwnerGraphDiagnosticKind::CollectionRoleMismatch
            );
        }
    }

    #[test]
    fn identity_ledger_tracks_child_collisions_without_leaking_uuid() {
        let mut ledger = OwnerIdentityLedger::new("root-uuid".to_owned());
        ledger
            .insert_generated("type-uuid".to_owned(), 1, GeneratedIdentityRole::Type)
            .unwrap();
        ledger
            .insert_child("child-uuid".to_owned(), 9, OwnerCollectionRole::Form)
            .unwrap();
        assert!(ledger.contains("CHILD-UUID"));
        assert_eq!(
            ledger.generated_identities(),
            BTreeSet::from(["type-uuid".to_owned()])
        );
        let duplicate = ledger
            .insert_child("TYPE-UUID".to_owned(), 10, OwnerCollectionRole::Command)
            .unwrap_err();
        assert_eq!(duplicate.previous, OwnerIdentityRole::GeneratedType);
        assert_eq!(
            duplicate.collection_role,
            Some(OwnerCollectionRole::Command)
        );

        let evidence = OwnerGraphDiagnosticEvidence::child_uuid(
            OwnerGraphDiagnosticKind::ChildIdentityCollision,
            duplicate.field_index,
            duplicate.collection_role.unwrap(),
        );
        assert_eq!(evidence.reference, Some(OwnerGraphReference::ChildUuid));
        assert_eq!(evidence.collection_role, Some(OwnerCollectionRole::Command));
        let rendered = format!("{evidence:?}");
        assert!(!rendered.contains("TYPE-UUID"));
        assert!(!rendered.contains("child-uuid"));
    }

    #[test]
    fn command_value_evidence_collides_symmetrically_with_identity_ledger() {
        let mut ledger = OwnerIdentityLedger::new("root".to_owned());
        ledger
            .insert_generated("generated".to_owned(), 1, GeneratedIdentityRole::Type)
            .unwrap();
        let root_collision = ledger
            .observe_command_value("ROOT".to_owned(), 2, OwnerCollectionRole::Command)
            .unwrap_err();
        assert_eq!(root_collision.previous, OwnerIdentityRole::Root);
        let generated_collision = ledger
            .observe_command_value("GENERATED".to_owned(), 3, OwnerCollectionRole::Command)
            .unwrap_err();
        assert_eq!(
            generated_collision.previous,
            OwnerIdentityRole::GeneratedType
        );
        ledger
            .observe_command_value("shared".to_owned(), 4, OwnerCollectionRole::Command)
            .unwrap();
        // Native command collections can intentionally reuse a value UUID.
        ledger
            .observe_command_value("SHARED".to_owned(), 5, OwnerCollectionRole::Command)
            .unwrap();
        let later_child = ledger
            .insert_child("shared".to_owned(), 6, OwnerCollectionRole::Form)
            .unwrap_err();
        assert_eq!(later_child.previous, OwnerIdentityRole::CommandValue);
        assert_eq!(later_child.collection_role, Some(OwnerCollectionRole::Form));
    }

    #[test]
    fn owned_child_reason_vocabulary_is_closed_and_stable() {
        let tokens = [
            (OwnerGraphOwnedChildReason::Missing, "missing"),
            (OwnerGraphOwnedChildReason::Ambiguous, "ambiguous"),
            (OwnerGraphOwnedChildReason::Unexpected, "unexpected"),
            (OwnerGraphOwnedChildReason::WrongKind, "wrong_kind"),
            (OwnerGraphOwnedChildReason::WrongOwner, "wrong_owner"),
            (
                OwnerGraphOwnedChildReason::HeaderMismatch,
                "header_mismatch",
            ),
            (
                OwnerGraphOwnedChildReason::DeclarationOrder,
                "declaration_order",
            ),
            (OwnerGraphOwnedChildReason::PropertyParse, "property_parse"),
        ];
        for (reason, token) in tokens {
            assert_eq!(reason.as_str(), token);
        }
    }
}
