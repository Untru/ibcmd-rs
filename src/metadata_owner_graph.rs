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
    produced_types_classifier: &'static str,
}

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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnerIdentityLedger {
    by_uuid: BTreeMap<String, OwnerIdentityRole>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OwnerIdentityCollision {
    pub(crate) previous: OwnerIdentityRole,
    pub(crate) field_index: usize,
}

impl OwnerIdentityLedger {
    pub(crate) fn new(root_uuid: String) -> Self {
        Self {
            by_uuid: BTreeMap::from([(root_uuid.to_ascii_lowercase(), OwnerIdentityRole::Root)]),
        }
    }

    pub(crate) fn insert_generated(
        &mut self,
        uuid: String,
        field_index: usize,
        role: OwnerIdentityRole,
    ) -> Result<(), OwnerIdentityCollision> {
        let key = uuid.to_ascii_lowercase();
        if let Some(previous) = self.by_uuid.get(&key) {
            return Err(OwnerIdentityCollision {
                previous: *previous,
                field_index,
            });
        }
        self.by_uuid.insert(key, role);
        Ok(())
    }

    pub(crate) fn generated_identities(&self) -> BTreeSet<String> {
        self.by_uuid
            .iter()
            .filter_map(|(uuid, role)| {
                (!matches!(role, OwnerIdentityRole::Root)).then_some(uuid.clone())
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

pub(crate) struct DecodedOwnerCollection<'a> {
    pub(crate) items: Vec<&'a str>,
}

pub(crate) struct DecodedOwnerGraph<'a> {
    pub(crate) generated_types: Vec<DecodedGeneratedType>,
    pub(crate) identities: OwnerIdentityLedger,
    pub(crate) owner_fields: Vec<&'a str>,
    pub(crate) collections: Vec<DecodedOwnerCollection<'a>>,
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
    CollectionMarker,
    OwnerHeader,
    GeneratedType,
    GeneratedValue,
}

impl OwnerGraphReference {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::CollectionMarker => "collection_marker",
            Self::OwnerHeader => "owner_header",
            Self::GeneratedType => "generated_type_id",
            Self::GeneratedValue => "generated_value_id",
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
    DuplicateIdentity,
    EdtFeatureOrder,
}

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
            Self::DuplicateIdentity => (Invariant, "owner_identity_ledger", "duplicate_identity"),
            Self::EdtFeatureOrder => (Invariant, "produced_type_order", "edt_feature_order"),
        }
    }
}

impl OwnerIdentityRole {
    pub(crate) const fn diagnostic_reference(self) -> &'static str {
        match self {
            Self::Root => "root_uuid",
            Self::GeneratedType => "generated_type_id",
            Self::GeneratedValue => "generated_value_id",
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
        let mut ledger = OwnerIdentityLedger::new("root".to_owned());
        ledger
            .insert_generated("type".to_owned(), 1, OwnerIdentityRole::GeneratedType)
            .unwrap();
        let duplicate = ledger
            .insert_generated("TYPE".to_owned(), 2, OwnerIdentityRole::GeneratedValue)
            .unwrap_err();
        assert_eq!(duplicate.previous, OwnerIdentityRole::GeneratedType);
        let root = ledger
            .insert_generated("ROOT".to_owned(), 3, OwnerIdentityRole::GeneratedType)
            .unwrap_err();
        assert_eq!(root.previous, OwnerIdentityRole::Root);
    }
}
