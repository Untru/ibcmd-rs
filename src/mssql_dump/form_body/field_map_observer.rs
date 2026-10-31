//! Bounded, value-free observations of manual DynamicList field-map custody.
//! All identifiers remain local to this function; reports hold no source strings.
mod context;
use super::*;
use context::{BoundQueryGroup, ObservationContext};
use serde::Serialize;
use std::io::Read;

pub const MAX_FORM_OBSERVER_COMPRESSED_BYTES: usize = 8 * 1024 * 1024;
const MAX_INFLATED: usize = 32 * 1024 * 1024;
const MAX_BAG: usize = 256 * 1024;
const MAX_FIELDS: usize = 4096;
const MAX_GROUPS: usize = 64;
const MAX_CANDIDATES: usize = 32;
const MAX_QUERY: usize = 32 * 1024;
const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 250_000;
const MAX_ATTRIBUTES: usize = 1024;
const MAX_QUERY_TOKENS: usize = 8192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormFieldMapObservationError {
    Limit,
    InvalidForm,
    InvalidBag,
    InvalidContext,
}
impl FormFieldMapObservationError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Limit => "FORM_OBSERVER_LIMIT",
            Self::InvalidForm => "FORM_OBSERVER_INVALID_FORM",
            Self::InvalidBag => "FORM_OBSERVER_INVALID_BAG",
            Self::InvalidContext => "FORM_OBSERVER_INVALID_CONTEXT",
        }
    }
}
impl std::fmt::Display for FormFieldMapObservationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for FormFieldMapObservationError {}

#[derive(Debug, Serialize)]
pub struct FormFieldMapObservationReport {
    schema_version: u8,
    full_typed_universe_available: bool,
    manual_dynamic_list_groups: Vec<Group>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_bound_groups: Option<Vec<BoundQueryGroup>>,
}
#[derive(Debug, Serialize)]
struct Group {
    ordinal: usize,
    candidate_count: usize,
    required_candidate_count: usize,
    duplicate_remembered_names_across_ids: Option<bool>,
    multiple_candidates_required: Option<bool>,
    physical_order_matches_numeric_suffix_order: Option<bool>,
    numeric_suffix_order_matches_numeric_id_order: Option<bool>,
    query_selection_available: bool,
    query_contains_wildcards: Option<bool>,
    candidates: Vec<Candidate>,
}
#[derive(Debug, Serialize)]
struct Candidate {
    ordinal: usize,
    required: bool,
    existing_shadowed_verdict: Option<bool>,
    exact_selected_alias_present: Option<bool>,
    twin_selected_alias_present: Option<bool>,
}

/// Read-only diagnostic of one raw-deflated Form body. It deliberately has no
/// metadata index and does not decide why the platform marks a field with `~`.
pub fn observe_form_field_map_blob(
    blob: &[u8],
) -> Result<FormFieldMapObservationReport, FormFieldMapObservationError> {
    observe_with_context(blob, None)
}

/// Optional VM-local context binds exact attribute names to a frozen XML pair.
/// Context strings and source bytes never enter the serializable report.
/// XML parsing and attribute correspondence remain with the reviewed caller;
/// these bytes bind its context to the exact inputs rather than reparsing XML.
pub fn observe_form_field_map_blob_with_context(
    blob: &[u8],
    context_json: &[u8],
    native_xml: &[u8],
    candidate_xml: &[u8],
) -> Result<FormFieldMapObservationReport, FormFieldMapObservationError> {
    let context = ObservationContext::read(context_json, blob, native_xml, candidate_xml)?;
    observe_with_context(blob, Some(context))
}

fn observe_with_context(
    blob: &[u8],
    context: Option<ObservationContext>,
) -> Result<FormFieldMapObservationReport, FormFieldMapObservationError> {
    if blob.len() > MAX_FORM_OBSERVER_COMPRESSED_BYTES {
        return Err(FormFieldMapObservationError::Limit);
    }
    let mut plain = Vec::new();
    flate2::read::DeflateDecoder::new(blob)
        .take((MAX_INFLATED + 1) as u64)
        .read_to_end(&mut plain)
        .map_err(|_| FormFieldMapObservationError::InvalidForm)?;
    if plain.len() > MAX_INFLATED {
        return Err(FormFieldMapObservationError::Limit);
    }
    let plain = String::from_utf8(plain).map_err(|_| FormFieldMapObservationError::InvalidForm)?;
    preflight_form_structure(&plain)?;
    let body = crate::module_blob::parse_form_body_plain(&plain)
        .map_err(|_| FormFieldMapObservationError::InvalidForm)?;
    let raw = form_body_attribute_records(&body.trailing)
        .ok_or(FormFieldMapObservationError::InvalidForm)?;
    if raw.declared_count() > MAX_ATTRIBUTES {
        return Err(FormFieldMapObservationError::Limit);
    }
    if raw.records().len() != raw.declared_count() {
        return Err(FormFieldMapObservationError::InvalidForm);
    }
    // Admit all bags and resource bounds before executing a map or query
    // resolver. Errors stop immediately; no production attribute decode runs.
    let mut admitted = Vec::new();
    let mut context_names = BTreeSet::new();
    for record in raw.into_records() {
        let attribute =
            form_attribute_raw_view(record).ok_or(FormFieldMapObservationError::InvalidForm)?;
        // Context correspondence concerns all raw attributes, including ones
        // later excluded as nonmanual or nondynamic. Do not hide duplicates.
        if context
            .as_ref()
            .is_some_and(|context| context.selects_name(attribute.name()))
            && !context_names.insert(attribute.name().to_owned())
        {
            return Err(FormFieldMapObservationError::InvalidContext);
        }
        let dynamic = attribute
            .pattern()
            .and_then(parse_form_exact_single_type_uuid)
            .is_some_and(|id| id.eq_ignore_ascii_case(form_dynamic_list_type_uuid()));
        if !dynamic {
            continue;
        }
        let Some(bag) = attribute.settings_bag() else {
            continue;
        };
        if let Some(prepared) = preflight_bag(bag)? {
            if admitted.len() >= MAX_GROUPS {
                return Err(FormFieldMapObservationError::Limit);
            }
            admitted.push((attribute.into_name(), prepared));
        }
    }
    let context_bound_groups = context
        .as_ref()
        .map(|context| context.observe(&admitted))
        .transpose()?;
    let groups = admitted
        .into_iter()
        .enumerate()
        .map(|(index, (_, bag))| observe_prepared_bag(bag, index + 1))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FormFieldMapObservationReport {
        schema_version: if context.is_some() { 2 } else { 1 },
        full_typed_universe_available: false,
        manual_dynamic_list_groups: groups,
        context_bound_groups,
    })
}

/// A resource preflight, not a replacement Form parser. Braces inside 1C
/// quoted strings do not contribute to nesting. Bound the root-42 upgrader
/// before any recursive parsing can start.
fn preflight_form_structure(text: &str) -> Result<(), FormFieldMapObservationError> {
    // The existing upgrader starts at the first opening brace. Admit only a
    // single root here so no quoted prefix can conceal its real parse root.
    let text = text.trim_start();
    let text = text.strip_prefix('\u{feff}').unwrap_or(text).trim();
    if !text.starts_with('{') {
        return Err(FormFieldMapObservationError::InvalidForm);
    }
    let mut bytes = text.bytes().peekable();
    let mut quoted = false;
    let mut depth = 0usize;
    let mut nodes = 0usize;
    while let Some(byte) = bytes.next() {
        if byte == b'"' {
            if quoted && bytes.peek() == Some(&b'"') {
                bytes.next();
            } else {
                quoted = !quoted;
            }
        } else if !quoted {
            match byte {
                b'{' => {
                    depth += 1;
                    nodes += 1;
                    if depth > MAX_DEPTH {
                        return Err(FormFieldMapObservationError::Limit);
                    }
                }
                b'}' => {
                    depth = depth
                        .checked_sub(1)
                        .ok_or(FormFieldMapObservationError::InvalidForm)?;
                    if depth == 0 && bytes.peek().is_some() {
                        return Err(FormFieldMapObservationError::InvalidForm);
                    }
                }
                b',' => nodes += 1,
                _ => {}
            }
            if nodes > MAX_NODES {
                return Err(FormFieldMapObservationError::Limit);
            }
        }
    }
    if quoted || depth != 0 {
        return Err(FormFieldMapObservationError::InvalidForm);
    }
    Ok(())
}
struct PreparedBag<'a> {
    fields: Vec<&'a str>,
    query_text: Option<String>,
}
fn preflight_bag(bag: &str) -> Result<Option<PreparedBag<'_>>, FormFieldMapObservationError> {
    if bag.len() > MAX_BAG {
        return Err(FormFieldMapObservationError::Limit);
    }
    let fields = split_1c_braced_fields(bag, 0).ok_or(FormFieldMapObservationError::InvalidBag)?;
    if fields.len() > MAX_FIELDS {
        return Err(FormFieldMapObservationError::Limit);
    }
    let [marker, count, properties @ ..] = fields.as_slice() else {
        return Err(FormFieldMapObservationError::InvalidBag);
    };
    let count = count
        .trim()
        .parse::<usize>()
        .map_err(|_| FormFieldMapObservationError::InvalidBag)?;
    if count > MAX_FIELDS / 2 {
        return Err(FormFieldMapObservationError::Limit);
    }
    if !crate::form_schema::form_record_has_zero_marker(marker)
        || properties.len() / 2 != count
        || properties.len() % 2 != 0
    {
        return Err(FormFieldMapObservationError::InvalidBag);
    }
    let mut manual = None;
    let mut query_text = None;
    for window in fields.windows(2) {
        let [key, value] = window else {
            unreachable!("exact property pair");
        };
        let key = parse_1c_quoted_string_with_len(key.trim())
            .map(|(key, _)| key)
            .unwrap_or_default();
        match crate::form_schema::FormDynamicListQueryProperty::classify(&key) {
            crate::form_schema::FormDynamicListQueryProperty::ManualQuery => {
                let value = parse_form_setting_bool(value)
                    .ok_or(FormFieldMapObservationError::InvalidBag)?;
                if manual.is_some_and(|old| old != value) {
                    return Err(FormFieldMapObservationError::InvalidBag);
                }
                manual = Some(value);
            }
            crate::form_schema::FormDynamicListQueryProperty::QueryText => {
                if value.len() > MAX_QUERY {
                    return Err(FormFieldMapObservationError::Limit);
                }
                let value = parse_form_setting_string(value);
                if query_text
                    .as_ref()
                    .is_some_and(|old| Some(old) != value.as_ref())
                {
                    return Err(FormFieldMapObservationError::InvalidBag);
                }
                query_text = value;
            }
            _ => {}
        }
    }
    if manual != Some(true) {
        return Ok(None);
    }
    if let Some(query) = &query_text {
        let tokens = tokenize_1c_query(query);
        if tokens.len() > MAX_QUERY_TOKENS {
            return Err(FormFieldMapObservationError::Limit);
        }
        let mut depth = 0usize;
        for token in tokens {
            match crate::form_schema::FormQueryLexeme::classify(&token) {
                crate::form_schema::FormQueryLexeme::OpenParenthesis => {
                    depth += 1;
                    if depth > MAX_DEPTH {
                        return Err(FormFieldMapObservationError::Limit);
                    }
                }
                crate::form_schema::FormQueryLexeme::CloseParenthesis => {
                    depth = depth
                        .checked_sub(1)
                        .ok_or(FormFieldMapObservationError::InvalidBag)?;
                }
                _ => {}
            }
        }
        if depth != 0 {
            return Err(FormFieldMapObservationError::InvalidBag);
        }
    }
    Ok(Some(PreparedBag { fields, query_text }))
}

fn is_target(name: &str) -> bool {
    ["DeletionMark", "ПометкаУдаления"]
        .iter()
        .any(|target| form_data_path_name_eq_ignore_case(name, target))
}
#[cfg(test)]
fn observe_bag(bag: &str, ordinal: usize) -> Result<Option<Group>, FormFieldMapObservationError> {
    preflight_bag(bag)?
        .map(|prepared| observe_prepared_bag(prepared, ordinal))
        .transpose()
}
fn observe_prepared_bag(
    prepared: PreparedBag<'_>,
    ordinal: usize,
) -> Result<Group, FormFieldMapObservationError> {
    let PreparedBag { fields, query_text } = prepared;
    let names = parse_form_dynamic_list_field_name_by_item_id(&fields);
    let twins = parse_form_dynamic_list_field_secondary_name_by_item_id(&fields);
    let required = parse_form_dynamic_list_required_item_ids(&fields)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let shadowed = parse_form_dynamic_list_shadowed_field_item_ids(&fields);
    let ids = names
        .iter()
        .filter(|(id, name)| is_target(name) || twins.get(*id).is_some_and(|name| is_target(name)))
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    if ids.len() > MAX_CANDIDATES {
        return Err(FormFieldMapObservationError::Limit);
    }
    // The exact production map collector supplies suffix identities. First
    // appearance is observed separately; no raw record slots are introduced.
    let mut by_suffix = BTreeMap::new();
    let mut ids_by_suffix = BTreeMap::new();
    let mut secondary_by_suffix = BTreeMap::new();
    let mut physical = Vec::new();
    let mut seen = BTreeSet::new();
    let mut ambiguous = fields.len() % 2 != 0;
    for pair in fields.chunks_exact(2) {
        let mut pair_names = BTreeMap::new();
        let mut pair_ids = BTreeMap::new();
        let [left, right] = pair else {
            unreachable!("exact key/value pair");
        };
        for (key, value) in [(*left, *right), (*right, *left)] {
            collect_form_dynamic_list_field_map_item(key, value, &mut pair_names, &mut pair_ids);
            let property = parse_1c_quoted_string_with_len(key.trim())
                .map(|(key, _)| key)
                .unwrap_or_default();
            if let Some(suffix) = property
                .strip_prefix("FiledsMapItemSecondaryName")
                .or_else(|| property.strip_prefix("FieldsMapItemSecondaryName"))
            {
                if let Some(twin) =
                    parse_form_setting_scalar_string(value).filter(|s| !s.is_empty())
                {
                    if secondary_by_suffix
                        .get(suffix)
                        .is_some_and(|old| old != &twin)
                    {
                        ambiguous = true;
                    }
                    secondary_by_suffix.insert(suffix.to_owned(), twin);
                } else {
                    ambiguous = true;
                }
            }
        }
        for (suffix, name) in pair_names {
            if by_suffix.get(&suffix).is_some_and(|old| old != &name) {
                ambiguous = true;
            }
            by_suffix.insert(suffix, name);
        }
        for (suffix, id) in pair_ids {
            if ids_by_suffix.get(&suffix).is_some_and(|old| old != &id) {
                ambiguous = true;
            }
            if seen.insert(suffix.clone()) {
                physical.push(suffix.clone());
            }
            ids_by_suffix.insert(suffix, id);
        }
    }
    // A missing pair companion or conflicting names for one item ID makes
    // the observed relationship unavailable, rather than an invented verdict.
    if by_suffix
        .keys()
        .any(|suffix| !ids_by_suffix.contains_key(suffix))
        || ids_by_suffix
            .keys()
            .any(|suffix| !by_suffix.contains_key(suffix))
    {
        ambiguous = true;
    }
    let mut names_by_id = BTreeMap::new();
    for (suffix, name) in &by_suffix {
        if let Some(id) = ids_by_suffix.get(suffix) {
            if names_by_id.get(id).is_some_and(|old| *old != name) {
                ambiguous = true;
            }
            names_by_id.insert(id, name);
        }
    }
    let mut secondary_by_id = BTreeMap::new();
    for (suffix, twin) in &secondary_by_suffix {
        if let Some(id) = ids_by_suffix.get(suffix) {
            if secondary_by_id.get(id).is_some_and(|old| *old != twin) {
                ambiguous = true;
            }
            secondary_by_id.insert(id, twin);
        } else {
            ambiguous = true;
        }
    }
    let mut numeric = ids_by_suffix
        .keys()
        .map(|suffix| suffix.parse::<u64>().ok().map(|n| (n, suffix.clone())))
        .collect::<Option<Vec<_>>>();
    if let Some(numeric) = &mut numeric {
        numeric.sort();
    }
    let suffix_order = numeric
        .as_ref()
        .map(|v| v.iter().map(|(_, s)| s.clone()).collect::<Vec<_>>());
    let physical_matches = (!ambiguous)
        .then(|| suffix_order.as_ref().map(|order| *order == physical))
        .flatten();
    let mut numeric_ids = ids_by_suffix
        .values()
        .map(|id| id.parse::<u64>().ok())
        .collect::<Option<Vec<_>>>();
    if let Some(v) = &mut numeric_ids {
        v.sort();
    }
    let suffix_ids = suffix_order.as_ref().and_then(|order| {
        order
            .iter()
            .map(|s| ids_by_suffix[s].parse::<u64>().ok())
            .collect::<Option<Vec<_>>>()
    });
    let numeric_matches = if ambiguous {
        None
    } else {
        suffix_ids.zip(numeric_ids).map(|(a, b)| a == b)
    };
    let query = query_text
        .as_deref()
        .and_then(|text| observe_query_selection(text, false));
    let duplicate = ids.iter().any(|id| {
        names
            .iter()
            .any(|(other, name)| other != id && names.get(id) == Some(name))
    });
    let required_count = ids.intersection(&required).count();
    let candidates = ids
        .iter()
        .enumerate()
        .map(|(offset, id)| {
            let name = &names[id];
            let twin = twins.get(id);
            let has_alias = |value: &str| {
                query.as_ref().map(|q| {
                    q.aliases
                        .iter()
                        .any(|alias| form_data_path_name_eq_ignore_case(alias, value))
                })
            };
            Candidate {
                ordinal: offset + 1,
                required: required.contains(id),
                existing_shadowed_verdict: (!ambiguous).then(|| shadowed.contains(id)),
                exact_selected_alias_present: if ambiguous { None } else { has_alias(name) },
                twin_selected_alias_present: if ambiguous {
                    None
                } else {
                    twin.and_then(|t| has_alias(t))
                },
            }
        })
        .collect();
    Ok(Group {
        ordinal,
        candidate_count: ids.len(),
        required_candidate_count: required_count,
        duplicate_remembered_names_across_ids: (!ambiguous).then_some(duplicate),
        multiple_candidates_required: (!ambiguous).then_some(required_count > 1),
        physical_order_matches_numeric_suffix_order: physical_matches,
        numeric_suffix_order_matches_numeric_id_order: numeric_matches,
        query_selection_available: query.is_some(),
        query_contains_wildcards: query.as_ref().map(|q| !q.stars.is_empty()),
        candidates,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::DeflateEncoder};
    use std::io::Write;
    pub(super) fn bag(query: &str) -> String {
        format!(
            r#"{{0,11,"ManualQuery",{{"B",1}},"QueryText",{{"S","{query}"}},"FieldsMapItemId1",{{"N",20}},"FieldsMapItemName1",{{"S","DeletionMark"}},"FieldsMapItemSecondaryName1",{{"S","ПометкаУдаления"}},"FieldsMapItemId0",{{"N",10}},"FieldsMapItemName0",{{"S","DeletionMark"}},"ReqMapFieldId0",{{"N",10}},"ReqMapFieldId1",{{"N",20}},"PrivacySentinel",{{"S","PRIVATE_SENTINEL_71"}},"AutoFillAvailableFields",{{"B",0}}}}"#
        )
    }
    #[test]
    fn exact_map_duplicate_required_order_and_twin_alias_facts_are_value_free() {
        let group = observe_bag(
            &bag("SELECT T.ПометкаУдаления AS ПометкаУдаления FROM Catalog.Test AS T"),
            1,
        )
        .unwrap()
        .unwrap();
        assert_eq!(group.candidate_count, 2);
        assert_eq!(group.required_candidate_count, 2);
        assert_eq!(group.duplicate_remembered_names_across_ids, Some(true));
        assert_eq!(
            group.physical_order_matches_numeric_suffix_order,
            Some(false)
        );
        assert_eq!(
            group.numeric_suffix_order_matches_numeric_id_order,
            Some(true)
        );
        assert!(
            group
                .candidates
                .iter()
                .all(|c| c.exact_selected_alias_present == Some(false))
        );
        assert!(
            group
                .candidates
                .iter()
                .any(|c| c.twin_selected_alias_present == Some(true))
        );
        assert_eq!(
            group
                .candidates
                .iter()
                .filter(|c| c.existing_shadowed_verdict == Some(true))
                .count(),
            1
        );
        let json = serde_json::to_string(&group).unwrap();
        for forbidden in [
            "PRIVATE_SENTINEL_71",
            "DeletionMark",
            "ПометкаУдаления",
            "Catalog",
            "SELECT",
            "20",
            "10",
        ] {
            assert!(!json.contains(forbidden));
        }
    }
    #[test]
    fn nested_selection_does_not_count_its_inner_alias_as_selected() {
        let group = observe_bag(
            &bag("SELECT T.(T.DeletionMark AS DeletionMark) AS Nested FROM Catalog.Test AS T"),
            1,
        )
        .unwrap()
        .unwrap();
        assert!(
            group
                .candidates
                .iter()
                .all(|c| c.exact_selected_alias_present == Some(false))
        );
    }
    #[test]
    fn conflicting_map_pairs_report_unknown_instead_of_an_inferred_verdict() {
        let source = bag("SELECT T.DeletionMark FROM Catalog.Test AS T")
            .replace("FieldsMapItemId0", "FieldsMapItemId1");
        let group = observe_bag(&source, 1).unwrap().unwrap();
        assert!(group.duplicate_remembered_names_across_ids.is_none());
        assert!(
            group
                .candidates
                .iter()
                .all(|candidate| candidate.existing_shadowed_verdict.is_none())
        );
    }
    #[test]
    fn exact_top_level_alias_and_wildcard_are_distinct_observations() {
        let group = observe_bag(
            &bag("SELECT T.DeletionMark AS DeletionMark, T.* FROM Catalog.Test AS T"),
            1,
        )
        .unwrap()
        .unwrap();
        assert_eq!(group.query_contains_wildcards, Some(true));
        assert!(
            group
                .candidates
                .iter()
                .all(|candidate| candidate.exact_selected_alias_present == Some(true))
        );
    }
    #[test]
    fn missing_query_reports_unknown_and_nonmanual_bags_are_excluded() {
        let group = observe_bag(&bag("NOT_A_SELECTION"), 1).unwrap().unwrap();
        assert!(!group.query_selection_available);
        assert!(
            group
                .candidates
                .iter()
                .all(|c| c.exact_selected_alias_present.is_none())
        );
        assert!(observe_bag("{0,0}", 1).unwrap().is_none());
    }
    #[test]
    fn invalid_and_oversized_inputs_have_only_static_errors() {
        assert_eq!(
            observe_form_field_map_blob(b"invalid").unwrap_err(),
            FormFieldMapObservationError::InvalidForm
        );
        assert_eq!(
            observe_form_field_map_blob(&vec![0; MAX_FORM_OBSERVER_COMPRESSED_BYTES + 1])
                .unwrap_err(),
            FormFieldMapObservationError::Limit
        );
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&vec![b'x'; MAX_INFLATED + 1]).unwrap();
        assert_eq!(
            observe_form_field_map_blob(&encoder.finish().unwrap()).unwrap_err(),
            FormFieldMapObservationError::Limit
        );
    }
    pub(super) fn record(settings: &str) -> String {
        r##"{9,{1},0,"PRIVATE_ATTRIBUTE",{1,0},{"Pattern",{"#",TYPE}},{0,{0,{"B",1},0}},{0,{0,{"B",1},0}},{0,0},{0,0},0,0,0,0,BAG,{0,0}}"##
            .replace("TYPE", form_dynamic_list_type_uuid()).replace("BAG", settings)
    }
    fn compressed(plain: &str) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(plain.as_bytes()).unwrap();
        encoder.finish().unwrap()
    }
    pub(super) fn form(records: &[String]) -> Vec<u8> {
        let attributes = format!(
            "{{4,{}{}}}",
            records.len(),
            if records.is_empty() {
                String::new()
            } else {
                format!(",{}", records.join(","))
            }
        );
        compressed(&r#"{4,{0},"PRIVATE_MODULE",ATTRIBUTES}"#.replace("ATTRIBUTES", &attributes))
    }
    #[test]
    fn real_attribute_traversal_observes_only_typed_dynamic_list_bags() {
        let records = vec![record(&bag(
            "SELECT T.DeletionMark AS DeletionMark FROM Catalog.Test AS T",
        ))];
        let report = observe_form_field_map_blob(&form(&records)).unwrap();
        assert_eq!(report.manual_dynamic_list_groups.len(), 1);
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("PRIVATE"));
        assert!(!report.full_typed_universe_available);
        let non_dynamic = records
            .iter()
            .map(|r| {
                r.replace(
                    form_dynamic_list_type_uuid(),
                    "00000000-0000-0000-0000-000000000000",
                )
            })
            .collect::<Vec<_>>();
        assert!(
            observe_form_field_map_blob(&form(&non_dynamic))
                .unwrap()
                .manual_dynamic_list_groups
                .is_empty()
        );
    }
    #[test]
    fn deep_form_and_excess_nodes_are_refused_before_recursive_parser() {
        let deep = format!(
            "{}0{}",
            "{".repeat(MAX_DEPTH + 1),
            "}".repeat(MAX_DEPTH + 1)
        );
        assert_eq!(
            observe_form_field_map_blob(&compressed(&deep)).unwrap_err(),
            FormFieldMapObservationError::Limit
        );
        let many = format!("{{{}}}", vec!["0"; MAX_NODES + 1].join(","));
        assert_eq!(
            observe_form_field_map_blob(&compressed(&many)).unwrap_err(),
            FormFieldMapObservationError::Limit
        );
        // Quoted braces are source strings, not recursive structure.
        preflight_form_structure(r#"{4,{0},"{{{{ PRIVATE }}}}",{4,0}}"#).unwrap();
    }
    #[test]
    fn quoted_prefix_cannot_hide_the_upgraders_root_and_bom_whitespace_is_valid() {
        let valid = r#"{4,{0},"",{4,0}}"#;
        let quoted = format!("\"{}0{}\"", "{".repeat(10_000), "}".repeat(10_000));
        for invalid in [
            format!("{quoted}{valid}"),
            quoted,
            format!("{valid}{{0}}"),
            format!("{valid} PRIVATE_TRAILER"),
        ] {
            let error = observe_form_field_map_blob(&compressed(&invalid)).unwrap_err();
            assert_eq!(error, FormFieldMapObservationError::InvalidForm);
            assert_eq!(error.to_string(), "FORM_OBSERVER_INVALID_FORM");
        }
        let report =
            observe_form_field_map_blob(&compressed(&format!("\u{feff} \t\n{valid}\r\n"))).unwrap();
        assert!(report.manual_dynamic_list_groups.is_empty());
    }

    #[test]
    fn attribute_group_bag_query_and_field_budgets_fail_before_resolution() {
        let invalid_records = vec!["{0}".to_owned(); MAX_ATTRIBUTES + 1];
        assert_eq!(
            observe_form_field_map_blob(&form(&invalid_records)).unwrap_err(),
            FormFieldMapObservationError::Limit
        );
        let r = record(&bag("SELECT T.DeletionMark FROM Catalog.Test AS T"));
        assert_eq!(
            observe_form_field_map_blob(&form(&vec![r; MAX_GROUPS + 1])).unwrap_err(),
            FormFieldMapObservationError::Limit
        );
        assert_eq!(
            observe_bag(&"x".repeat(MAX_BAG + 1), 1).unwrap_err(),
            FormFieldMapObservationError::Limit
        );
        assert_eq!(
            observe_bag(&bag(&"x".repeat(MAX_QUERY + 1)), 1).unwrap_err(),
            FormFieldMapObservationError::Limit
        );
        let deep = format!(
            "SELECT {}T.DeletionMark{} FROM Catalog.Test AS T",
            "(".repeat(MAX_DEPTH + 1),
            ")".repeat(MAX_DEPTH + 1)
        );
        assert_eq!(
            observe_bag(&bag(&deep), 1).unwrap_err(),
            FormFieldMapObservationError::Limit
        );
        let fields = format!(
            "{{0,{},{} }}",
            MAX_FIELDS,
            vec![r#""Unused",{"N",0}"#; MAX_FIELDS].join(",")
        );
        assert_eq!(
            observe_bag(&fields, 1).unwrap_err(),
            FormFieldMapObservationError::Limit
        );
    }
    #[test]
    fn conflicting_secondary_suffix_or_shared_id_never_yields_definite_alias_facts() {
        for source in [
            bag("SELECT T.DeletionMark FROM Catalog.Test AS T")
                .replace("{0,11,", "{0,12,")
                .replace(
                    r#""PrivacySentinel""#,
                    r#""FieldsMapItemSecondaryName1",{"S","PRIVATE_TWIN"},"PrivacySentinel""#,
                ),
            bag("SELECT T.DeletionMark FROM Catalog.Test AS T")
                .replace("{0,11,", "{0,12,")
                .replace(
                    r#""FieldsMapItemId0",{"N",10}"#,
                    r#""FieldsMapItemId0",{"N",20}"#,
                )
                .replace(
                    r#""PrivacySentinel""#,
                    r#""FieldsMapItemSecondaryName0",{"S","PRIVATE_TWIN"},"PrivacySentinel""#,
                ),
        ] {
            let group = observe_bag(&source, 1).unwrap().unwrap();
            assert!(group.duplicate_remembered_names_across_ids.is_none());
            assert!(
                group
                    .candidates
                    .iter()
                    .all(|c| c.exact_selected_alias_present.is_none()
                        && c.twin_selected_alias_present.is_none()
                        && c.existing_shadowed_verdict.is_none())
            );
            assert!(!serde_json::to_string(&group).unwrap().contains("PRIVATE"));
        }
    }
}
