//! Schema-driven ordering for metadata writer features.
//!
//! This module deliberately applies only ordering evidence. Provider fallbacks
//! describe how EDT obtains a model feature list; they are not evidence for XML
//! defaults, QName spelling, or `xsi:nil` emission.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use ibcmd_schema::{
    MetadataOrderOperationKind, MetadataOrderSection, MetadataOrderVersionPredicate,
    bundled_metadata_order,
};

const FEATURE_ORDER_PROVIDER: &str = "MetadataObjectFeatureOrderProvider";
const PRODUCED_TYPES_ORDER_PROVIDER: &str = "ProducedTypesOrderProvider";

/// A fail-closed metadata ordering error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetadataOrderError {
    InvalidCorpus(String),
    UnknownRule {
        classifier: String,
        section: MetadataOrderSection,
        version: MetadataOrderVersionPredicate,
    },
    DuplicateFeature(String),
    UnknownFeature(String),
    MissingCursor(String),
    AmbiguousProducedType {
        classifier: String,
        category: String,
    },
}

impl fmt::Display for MetadataOrderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCorpus(message) => {
                write!(formatter, "invalid metadata order corpus: {message}")
            }
            Self::UnknownRule {
                classifier,
                section,
                version,
            } => write!(
                formatter,
                "no verified metadata order for {classifier}/{section:?}/{version:?}"
            ),
            Self::DuplicateFeature(feature) => {
                write!(formatter, "duplicate metadata feature value: {feature}")
            }
            Self::UnknownFeature(feature) => {
                write!(
                    formatter,
                    "metadata feature is absent from verified order: {feature}"
                )
            }
            Self::MissingCursor(feature) => {
                write!(
                    formatter,
                    "metadata order cursor feature is absent: {feature}"
                )
            }
            Self::AmbiguousProducedType {
                classifier,
                category,
            } => write!(
                formatter,
                "produced type category {category} is not uniquely mapped for {classifier}"
            ),
        }
    }
}

impl Error for MetadataOrderError {}

fn provider_for(section: MetadataOrderSection) -> &'static str {
    if section == MetadataOrderSection::ProducedTypes {
        PRODUCED_TYPES_ORDER_PROVIDER
    } else {
        FEATURE_ORDER_PROVIDER
    }
}

/// Applies the exact verified provider rule to a complete model feature order.
///
/// `cursor` selects an existing anchor. Each subsequent `next` moves its
/// feature directly after the current cursor and advances the cursor. Missing
/// anchors/features fail closed because provider fallback is not XML evidence.
pub fn order_metadata_features(
    classifier: &str,
    section: MetadataOrderSection,
    version: MetadataOrderVersionPredicate,
    baseline: &[String],
) -> Result<Vec<String>, MetadataOrderError> {
    let corpus = bundled_metadata_order()
        .map_err(|error| MetadataOrderError::InvalidCorpus(error.to_string()))?;
    let record = corpus
        .order(provider_for(section), classifier, section, version)
        .ok_or_else(|| MetadataOrderError::UnknownRule {
            classifier: classifier.to_owned(),
            section,
            version,
        })?;

    let mut seen = BTreeSet::new();
    for feature in baseline {
        if !seen.insert(feature.as_str()) {
            return Err(MetadataOrderError::DuplicateFeature(feature.clone()));
        }
    }

    match section {
        MetadataOrderSection::ProducedTypes => {
            let available = baseline.iter().map(String::as_str).collect::<BTreeSet<_>>();
            for feature in baseline {
                if !record.ordered_features.contains(feature) {
                    return Err(MetadataOrderError::UnknownFeature(feature.clone()));
                }
            }
            Ok(record
                .ordered_features
                .iter()
                .filter(|feature| available.contains(feature.as_str()))
                .cloned()
                .collect())
        }
        MetadataOrderSection::InternalInfo | MetadataOrderSection::ChildObjects => {
            let expected = record
                .order_operations
                .iter()
                .map(|operation| operation.feature.as_str())
                .collect::<BTreeSet<_>>();
            for feature in baseline {
                if !expected.contains(feature.as_str()) {
                    return Err(MetadataOrderError::UnknownFeature(feature.clone()));
                }
            }
            Ok(record
                .order_operations
                .iter()
                .filter(|operation| baseline.contains(&operation.feature))
                .map(|operation| operation.feature.clone())
                .collect())
        }
        MetadataOrderSection::Properties => {
            let mut ordered = baseline.to_vec();
            let mut cursor = None;
            for operation in &record.order_operations {
                match operation.operation {
                    MetadataOrderOperationKind::Cursor => {
                        cursor = ordered
                            .iter()
                            .position(|feature| feature == &operation.feature);
                        if cursor.is_none() {
                            return Err(MetadataOrderError::MissingCursor(
                                operation.feature.clone(),
                            ));
                        }
                    }
                    MetadataOrderOperationKind::Next => {
                        let Some(cursor_index) = cursor else {
                            return Err(MetadataOrderError::MissingCursor(
                                operation.feature.clone(),
                            ));
                        };
                        let Some(source_index) = ordered
                            .iter()
                            .position(|feature| feature == &operation.feature)
                        else {
                            return Err(MetadataOrderError::UnknownFeature(
                                operation.feature.clone(),
                            ));
                        };
                        let feature = ordered.remove(source_index);
                        let adjusted_cursor = if source_index < cursor_index {
                            cursor_index - 1
                        } else {
                            cursor_index
                        };
                        let target_index = adjusted_cursor + 1;
                        ordered.insert(target_index, feature);
                        cursor = Some(target_index);
                    }
                    MetadataOrderOperationKind::Emit => {
                        return Err(MetadataOrderError::InvalidCorpus(format!(
                            "properties rule for {classifier} contains emit"
                        )));
                    }
                }
            }
            Ok(ordered)
        }
    }
}

fn produced_type_feature(classifier: &str, category: &str) -> Result<String, MetadataOrderError> {
    let corpus = bundled_metadata_order()
        .map_err(|error| MetadataOrderError::InvalidCorpus(error.to_string()))?;
    let record = corpus
        .order(
            PRODUCED_TYPES_ORDER_PROVIDER,
            classifier,
            MetadataOrderSection::ProducedTypes,
            MetadataOrderVersionPredicate::Always,
        )
        .ok_or_else(|| MetadataOrderError::UnknownRule {
            classifier: classifier.to_owned(),
            section: MetadataOrderSection::ProducedTypes,
            version: MetadataOrderVersionPredicate::Always,
        })?;
    let suffix = format!("__{}_TYPE", category.to_ascii_uppercase());
    let matches = record
        .ordered_features
        .iter()
        .filter(|feature| feature.ends_with(&suffix))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [feature] => Ok((*feature).clone()),
        _ => Err(MetadataOrderError::AmbiguousProducedType {
            classifier: classifier.to_owned(),
            category: category.to_owned(),
        }),
    }
}

/// Orders emitted generated types using the verified produced-types provider.
///
/// The writer supplies only the model category (`Object`, `Ref`, etc.). The
/// concrete EReference token and its rank come from the bundled schema corpus.
pub fn order_produced_type_values<T>(
    classifier: &str,
    values: Vec<(&str, T)>,
) -> Result<Vec<T>, MetadataOrderError> {
    let mut by_feature = BTreeMap::new();
    for (category, value) in values {
        let feature = produced_type_feature(classifier, category)?;
        if by_feature.insert(feature.clone(), value).is_some() {
            return Err(MetadataOrderError::DuplicateFeature(feature));
        }
    }
    let baseline = by_feature.keys().cloned().collect::<Vec<_>>();
    let ordered_features = order_metadata_features(
        classifier,
        MetadataOrderSection::ProducedTypes,
        MetadataOrderVersionPredicate::Always,
        &baseline,
    )?;
    Ok(ordered_features
        .into_iter()
        .map(|feature| {
            by_feature
                .remove(&feature)
                .expect("ordered feature was validated against value map")
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produced_types_are_ordered_from_catalog_and_document_rules() {
        for classifier in ["CATALOG_TYPES", "DOCUMENT_TYPES"] {
            let ordered = order_produced_type_values(
                classifier,
                vec![
                    ("Manager", "manager"),
                    ("List", "list"),
                    ("Ref", "ref"),
                    ("Object", "object"),
                    ("Selection", "selection"),
                ],
            )
            .unwrap();
            assert_eq!(ordered, ["object", "ref", "selection", "list", "manager"]);
        }
    }

    #[test]
    fn configuration_version_predicate_is_exact() {
        let corpus = bundled_metadata_order().unwrap();
        let old = corpus
            .order(
                FEATURE_ORDER_PROVIDER,
                "CONFIGURATION",
                MetadataOrderSection::Properties,
                MetadataOrderVersionPredicate::NotGreaterThanV8_3_14,
            )
            .unwrap();
        let new = corpus
            .order(
                FEATURE_ORDER_PROVIDER,
                "CONFIGURATION",
                MetadataOrderSection::Properties,
                MetadataOrderVersionPredicate::GreaterThanV8_3_14,
            )
            .unwrap();
        assert!(
            old.ordered_features
                .contains(&"CONFIGURATION__REQUIRED_MOBILE_APPLICATION_PERMISSIONS".to_owned())
        );
        assert!(
            new.ordered_features
                .contains(&"CONFIGURATION__REQUIRED_MOBILE_APPLICATION_PERMISSIONS8315".to_owned())
        );
    }

    #[test]
    fn cursor_and_next_operations_are_applied_as_anchor_moves() {
        let corpus = bundled_metadata_order().unwrap();
        let record = corpus
            .order(
                FEATURE_ORDER_PROVIDER,
                "CATALOG",
                MetadataOrderSection::Properties,
                MetadataOrderVersionPredicate::Always,
            )
            .unwrap();
        let baseline = record
            .ordered_features
            .iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>();
        let ordered = order_metadata_features(
            "CATALOG",
            MetadataOrderSection::Properties,
            MetadataOrderVersionPredicate::Always,
            &baseline,
        )
        .unwrap();

        let mut cursor = None;
        for operation in &record.order_operations {
            let position = ordered
                .iter()
                .position(|feature| feature == &operation.feature)
                .unwrap();
            match operation.operation {
                MetadataOrderOperationKind::Cursor => cursor = Some(position),
                MetadataOrderOperationKind::Next => {
                    assert_eq!(position, cursor.unwrap() + 1);
                    cursor = Some(position);
                }
                MetadataOrderOperationKind::Emit => unreachable!(),
            }
        }
    }

    #[test]
    fn unknown_classifier_and_category_fail_closed() {
        assert!(matches!(
            order_produced_type_values("UNKNOWN_TYPES", vec![("Object", ())]),
            Err(MetadataOrderError::UnknownRule { .. })
        ));
        assert!(matches!(
            order_produced_type_values("CATALOG_TYPES", vec![("Invented", ())]),
            Err(MetadataOrderError::AmbiguousProducedType { .. })
        ));
    }
}
