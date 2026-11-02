//! Finite lexical diagnostics of a parsed final SELECT. No source text is
//! retained in the report, and no absence here proves semantic nonselection.
use super::{
    ObserverQuerySelection as FormDynamicListQuerySelection, form_data_path_name_eq_ignore_case,
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SourceMatch {
    MissingMainTable,
    NoMatchingFinalSource,
    AmbiguousSource,
    MissingAlias,
    AliasCollision,
    Unique,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum StandardField {
    ReferenceEnglish,
    ReferenceRussian,
    DeletionMarkEnglish,
    DeletionMarkRussian,
}
const FIELDS: [(StandardField, &str, &str, &str); 4] = [
    (StandardField::ReferenceEnglish, "Ref", "Ref", "Ссылка"),
    (StandardField::ReferenceRussian, "Ссылка", "Ref", "Ссылка"),
    (
        StandardField::DeletionMarkEnglish,
        "DeletionMark",
        "DeletionMark",
        "ПометкаУдаления",
    ),
    (
        StandardField::DeletionMarkRussian,
        "ПометкаУдаления",
        "DeletionMark",
        "ПометкаУдаления",
    ),
];

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AliasCategory {
    English,
    Russian,
    /// Neither standard alias even under casefold; excludes case-only variants.
    Other,
}
const ALIASES: [AliasCategory; 3] = [
    AliasCategory::English,
    AliasCategory::Russian,
    AliasCategory::Other,
];

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Serialize)]
pub(super) struct MatchCounts {
    pub(super) exact: usize,
    /// Inclusive: exact matches are included, not subtracted.
    pub(super) case_folded: usize,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub(super) struct IdentityCell {
    pub(super) source_field: StandardField,
    /// Relative to the source identity (Ref or DeletionMark), not its language.
    pub(super) result_alias: AliasCategory,
    pub(super) occurrences: MatchCounts,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub(super) struct LexicalAliasPresence {
    pub(super) field: StandardField,
    /// Result-alias set only; independent of source/path identity and stars.
    pub(super) aliases: MatchCounts,
}

#[derive(Debug, Default, Eq, PartialEq, Serialize)]
pub(super) struct SourceScopeCounts {
    pub(super) main: usize,
    pub(super) other: usize,
    pub(super) unresolved: usize,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub(super) struct ResolvedObservations {
    pub(super) identity_paths: Vec<IdentityCell>,
    pub(super) plain_paths: SourceScopeCounts,
    pub(super) stars: SourceScopeCounts,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub(super) struct QueryIdentityObservation {
    pub(super) source_match: SourceMatch,
    pub(super) final_source_count: usize,
    pub(super) matching_source_count: usize,
    /// Null for unavailable/ambiguous source identity, never zero observations.
    pub(super) observations: Option<ResolvedObservations>,
    pub(super) lexical_aliases: Vec<LexicalAliasPresence>,
    pub(super) unclassified_selection_expression_count: usize,
    pub(super) semantic_unselected_proven: bool,
}

fn alias_category(alias: &str, english: &str, russian: &str, folded: bool) -> AliasCategory {
    let equals = |value| {
        if folded {
            form_data_path_name_eq_ignore_case(alias, value)
        } else {
            alias == value
        }
    };
    if equals(english) {
        AliasCategory::English
    } else if equals(russian) {
        AliasCategory::Russian
    } else {
        AliasCategory::Other
    }
}

/// Match the entire canonical reference, never just an object-name suffix.
/// Source and qualifier resolution is case-insensitive. Exact/folded counters
/// refer to the selected field/result-alias spellings, not source casing.
pub(super) fn observe_query_identity(
    selection: &FormDynamicListQuerySelection,
    canonical_main_table: &str,
) -> QueryIdentityObservation {
    let matching: Vec<usize> = selection
        .sources
        .iter()
        .enumerate()
        .filter_map(|(index, (reference, _))| {
            reference
                .as_deref()
                .filter(|reference| {
                    form_data_path_name_eq_ignore_case(reference, canonical_main_table)
                })
                .map(|_| index)
        })
        .collect();
    let lexical_aliases = FIELDS
        .iter()
        .map(|(field, spelling, _, _)| LexicalAliasPresence {
            field: *field,
            aliases: MatchCounts {
                exact: selection
                    .aliases
                    .iter()
                    .filter(|alias| alias.as_str() == *spelling)
                    .count(),
                case_folded: selection
                    .aliases
                    .iter()
                    .filter(|alias| form_data_path_name_eq_ignore_case(alias, spelling))
                    .count(),
            },
        })
        .collect();
    let unique_qualifier = |qualifier: &str| {
        let mut found = selection
            .sources
            .iter()
            .enumerate()
            .filter_map(|(index, (_, alias))| {
                alias
                    .as_deref()
                    .filter(|alias| form_data_path_name_eq_ignore_case(alias, qualifier))
                    .map(|_| index)
            });
        let first = found.next()?;
        found.next().is_none().then_some(first)
    };
    let source_match = if canonical_main_table.trim().is_empty() {
        SourceMatch::MissingMainTable
    } else if matching.is_empty() {
        SourceMatch::NoMatchingFinalSource
    } else if matching.len() != 1 {
        SourceMatch::AmbiguousSource
    } else {
        match selection.sources[matching[0]]
            .1
            .as_deref()
            .filter(|alias| !alias.is_empty())
        {
            None => SourceMatch::MissingAlias,
            Some(alias) if unique_qualifier(alias) != Some(matching[0]) => {
                SourceMatch::AliasCollision
            }
            Some(_) => SourceMatch::Unique,
        }
    };
    let observations = (source_match == SourceMatch::Unique).then(|| {
        let main = matching[0];
        let mut identity_paths = Vec::with_capacity(12);
        for (field, _, _, _) in FIELDS {
            for result_alias in ALIASES {
                identity_paths.push(IdentityCell {
                    source_field: field,
                    result_alias,
                    occurrences: MatchCounts::default(),
                });
            }
        }
        let mut plain_paths = SourceScopeCounts::default();
        for (qualifier, field, result_alias) in &selection.plain_path_occurrences {
            match unique_qualifier(qualifier) {
                Some(index) if index == main => plain_paths.main += 1,
                Some(_) => {
                    plain_paths.other += 1;
                    continue;
                }
                None => {
                    plain_paths.unresolved += 1;
                    continue;
                }
            }
            for (field_index, (_, spelling, english, russian)) in FIELDS.iter().enumerate() {
                for (alias_index, category) in ALIASES.iter().enumerate() {
                    let cell = &mut identity_paths[field_index * 3 + alias_index];
                    // Other means neither standard alias under casefold;
                    // capitalization alone is not a renamed identity.
                    let exact_alias = alias_category(result_alias, english, russian, false)
                        == *category
                        && (*category != AliasCategory::Other
                            || alias_category(result_alias, english, russian, true)
                                == AliasCategory::Other);
                    if field.as_str() == *spelling && exact_alias {
                        cell.occurrences.exact += 1;
                    }
                    if form_data_path_name_eq_ignore_case(field, spelling)
                        && alias_category(result_alias, english, russian, true) == *category
                    {
                        cell.occurrences.case_folded += 1;
                    }
                }
            }
        }
        let mut stars = SourceScopeCounts::default();
        for qualifier in &selection.stars {
            // None conflates bare and unnameable qualifiers in the existing
            // parser. Keep it unresolved even with a single final source.
            match qualifier.as_deref().and_then(unique_qualifier) {
                Some(index) if index == main => stars.main += 1,
                Some(_) => stars.other += 1,
                None => stars.unresolved += 1,
            }
        }
        ResolvedObservations {
            identity_paths,
            plain_paths,
            stars,
        }
    });
    QueryIdentityObservation {
        source_match,
        final_source_count: selection.sources.len(),
        matching_source_count: matching.len(),
        observations,
        lexical_aliases,
        unclassified_selection_expression_count: selection.unclassified_selection_expression_count,
        semantic_unselected_proven: false,
    }
}

#[cfg(test)]
mod tests {
    use super::super::parse_form_dynamic_list_query_selection_diagnostic;
    use super::*;

    fn selected(query: &str) -> FormDynamicListQuerySelection {
        parse_form_dynamic_list_query_selection_diagnostic(query).expect("public synthetic SELECT")
    }
    fn cell(
        report: &QueryIdentityObservation,
        field: StandardField,
        alias: AliasCategory,
    ) -> MatchCounts {
        report
            .observations
            .as_ref()
            .unwrap()
            .identity_paths
            .iter()
            .find(|cell| cell.source_field == field && cell.result_alias == alias)
            .unwrap()
            .occurrences
    }

    #[test]
    fn production_parser_keeps_legacy_projection_and_diagnostic_allocations_empty() {
        let source =
            "SELECT X.Ref AS Ref, X.Ref AS Other, (1 + 2) AS Expression FROM Document.Owned AS X";
        let mut diagnostic = selected(source);
        let ordinary = super::super::observe_query_selection(source, false).unwrap();
        assert_eq!(diagnostic.plain_path_occurrences.len(), 2);
        assert_eq!(diagnostic.unclassified_selection_expression_count, 1);
        assert!(ordinary.plain_path_occurrences.is_empty());
        assert_eq!(ordinary.unclassified_selection_expression_count, 0);
        diagnostic.plain_path_occurrences.clear();
        diagnostic.unclassified_selection_expression_count = 0;
        assert_eq!(ordinary, diagnostic);
    }

    #[test]
    fn russian_identity_to_russian_and_other_aliases_are_distinct() {
        let q = selected(
            "ВЫБРАТЬ X.ПометкаУдаления КАК ПометкаУдаления, X.ПометкаУдаления КАК OwnedAlias ИЗ Документ.Owned КАК X",
        );
        let r = observe_query_identity(&q, "Document.Owned");
        assert_eq!(
            cell(
                &r,
                StandardField::DeletionMarkRussian,
                AliasCategory::Russian
            ),
            MatchCounts {
                exact: 1,
                case_folded: 1
            }
        );
        assert_eq!(
            cell(&r, StandardField::DeletionMarkRussian, AliasCategory::Other),
            MatchCounts {
                exact: 1,
                case_folded: 1
            }
        );
        assert!(!r.semantic_unselected_proven);
    }

    #[test]
    fn english_and_case_folded_spellings_have_separate_counts() {
        let q = selected(
            "SELECT x.DeletionMark AS DeletionMark, x.DeletionMark AS deletionmark, x.deletionmark AS DELETIONMARK FROM Document.Owned AS x",
        );
        let r = observe_query_identity(&q, "document.OWNED");
        assert_eq!(
            cell(
                &r,
                StandardField::DeletionMarkEnglish,
                AliasCategory::English
            ),
            MatchCounts {
                exact: 1,
                case_folded: 3
            }
        );
        assert_eq!(
            r.lexical_aliases
                .iter()
                .find(|a| a.field == StandardField::DeletionMarkEnglish)
                .unwrap()
                .aliases,
            MatchCounts {
                exact: 1,
                case_folded: 3
            }
        );
        assert_eq!(
            cell(&r, StandardField::DeletionMarkEnglish, AliasCategory::Other),
            MatchCounts::default()
        );
    }

    #[test]
    fn cross_language_aliases_preserve_the_source_spelling() {
        let q = selected(
            "SELECT X.Ref AS Ссылка, X.ПометкаУдаления AS DeletionMark FROM Document.Owned AS X",
        );
        let r = observe_query_identity(&q, "Document.Owned");
        assert_eq!(
            cell(&r, StandardField::ReferenceEnglish, AliasCategory::Russian),
            MatchCounts {
                exact: 1,
                case_folded: 1
            }
        );
        assert_eq!(
            cell(
                &r,
                StandardField::DeletionMarkRussian,
                AliasCategory::English
            ),
            MatchCounts {
                exact: 1,
                case_folded: 1
            }
        );
    }

    #[test]
    fn missing_main_table_and_alias_leave_identity_observations_null() {
        let mut q = selected("SELECT X.Ref AS Ref FROM Document.Owned AS X");
        let absent = observe_query_identity(&q, "");
        assert_eq!(absent.source_match, SourceMatch::MissingMainTable);
        assert!(absent.observations.is_none());
        q.sources[0].1 = None;
        let no_alias = observe_query_identity(&q, "Document.Owned");
        assert_eq!(no_alias.source_match, SourceMatch::MissingAlias);
        assert!(no_alias.observations.is_none());
        assert_eq!(
            no_alias
                .lexical_aliases
                .iter()
                .find(|a| a.field == StandardField::ReferenceEnglish)
                .unwrap()
                .aliases
                .exact,
            1
        );
    }

    #[test]
    fn duplicate_occurrences_survive_the_legacy_path_map() {
        let q = selected("SELECT X.Ref AS Ref, X.Ref AS Ref FROM Document.Owned AS X");
        assert_eq!(q.paths.len(), 1);
        let r = observe_query_identity(&q, "Document.Owned");
        assert_eq!(
            cell(&r, StandardField::ReferenceEnglish, AliasCategory::English),
            MatchCounts {
                exact: 2,
                case_folded: 2
            }
        );
    }

    #[test]
    fn unrelated_full_references_and_duplicate_sources_are_unavailable() {
        let q = selected("SELECT X.Ref FROM Document.Owned AS X");
        let absent = observe_query_identity(&q, "Catalog.Owned");
        assert_eq!(absent.source_match, SourceMatch::NoMatchingFinalSource);
        assert!(absent.observations.is_none());
        let mut duplicate = q.clone();
        duplicate.sources.push(duplicate.sources[0].clone());
        let r = observe_query_identity(&duplicate, "Document.Owned");
        assert_eq!(r.source_match, SourceMatch::AmbiguousSource);
        assert!(r.observations.is_none());
        let mut collision = q;
        collision
            .sources
            .push((Some("Catalog.Other".into()), Some("x".into())));
        assert_eq!(
            observe_query_identity(&collision, "Document.Owned").source_match,
            SourceMatch::AliasCollision
        );
    }

    #[test]
    fn qualified_stars_are_scoped_and_bare_stars_stay_unresolved() {
        let q =
            selected("SELECT X.*, Y.*, *, Missing.* FROM Document.Owned AS X, Catalog.Other AS Y");
        let r = observe_query_identity(&q, "Document.Owned");
        assert_eq!(
            r.observations.unwrap().stars,
            SourceScopeCounts {
                main: 1,
                other: 1,
                unresolved: 2
            }
        );
        let q = selected("SELECT * FROM Document.Owned AS X");
        assert_eq!(
            observe_query_identity(&q, "Document.Owned")
                .observations
                .unwrap()
                .stars
                .unresolved,
            1
        );
    }

    #[test]
    fn renamed_reference_is_identity_not_lexical_reference_presence() {
        let q = selected("SELECT X.Ссылка AS OwnedRename FROM Document.Owned AS X");
        let r = observe_query_identity(&q, "Document.Owned");
        assert_eq!(
            cell(&r, StandardField::ReferenceRussian, AliasCategory::Other),
            MatchCounts {
                exact: 1,
                case_folded: 1
            }
        );
        assert_eq!(
            r.lexical_aliases
                .iter()
                .find(|a| a.field == StandardField::ReferenceRussian)
                .unwrap()
                .aliases,
            MatchCounts::default()
        );
    }

    #[test]
    fn final_selection_excludes_temporary_and_nested_paths() {
        let q = selected(
            "SELECT T.DeletionMark AS DeletionMark INTO OwnedTemp FROM Document.Owned AS T; SELECT X.Ref AS Ref FROM Document.Owned AS X WHERE X.Ref IN (SELECT N.Ref FROM Catalog.Other AS N)",
        );
        let r = observe_query_identity(&q, "Document.Owned");
        assert_eq!(r.final_source_count, 1);
        assert_eq!(
            cell(&r, StandardField::ReferenceEnglish, AliasCategory::English).exact,
            1
        );
        assert_eq!(
            cell(
                &r,
                StandardField::DeletionMarkEnglish,
                AliasCategory::English
            )
            .exact,
            0
        );
        let nested =
            selected("SELECT N.Ref AS Ref FROM (SELECT X.Ref FROM Document.Owned AS X) AS N");
        let r = observe_query_identity(&nested, "Document.Owned");
        assert_eq!(r.source_match, SourceMatch::NoMatchingFinalSource);
        assert!(r.observations.is_none());
    }

    #[test]
    fn report_is_value_free_and_expressions_never_prove_nonselection() {
        let q = selected(
            "SELECT PRIVATE_SENTINEL_71.Ref AS PRIVATE_ALIAS_72, 1 + 2 AS Ref FROM Document.PRIVATE_OBJECT_73 AS PRIVATE_SENTINEL_71",
        );
        let r = observe_query_identity(&q, "Document.PRIVATE_OBJECT_73");
        assert!(r.unclassified_selection_expression_count > 0);
        let json = serde_json::to_string(&r).unwrap();
        let debug = format!("{r:?}");
        for token in [
            "PRIVATE_SENTINEL_71",
            "PRIVATE_ALIAS_72",
            "PRIVATE_OBJECT_73",
            "SELECT",
        ] {
            assert!(!json.contains(token));
            assert!(!debug.contains(token));
        }
        assert!(!r.semantic_unselected_proven);
    }
}
