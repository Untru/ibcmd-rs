//! VM-local custody and exact-name joins. Private context is never a report.
use super::*;
use crate::mssql_dump::form_body::query_identity_observer::{
    QueryIdentityObservation, observe_query_identity,
};
use serde::Deserialize;

const MAX_CONTEXT_BYTES: usize = 64 * 1024;
const MAX_XML_BYTES: usize = 8 * 1024 * 1024;
const MAX_CONTEXT_NAME_BYTES: usize = 512;
const MAX_MAIN_TABLE_BYTES: usize = 4096;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ObservationContext {
    schema_version: u8,
    body_sha256: String,
    native_xml_sha256: String,
    candidate_xml_sha256: String,
    attributes: Vec<AttributeContext>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AttributeContext {
    affected_xml_ordinal: usize,
    attribute_name: String,
    native_main_table: String,
    candidate_main_table: String,
}
#[derive(Debug, Serialize)]
pub(super) struct BoundQueryGroup {
    affected_xml_ordinal: usize,
    raw_group_ordinal: usize,
    exact_attribute_name_join: bool,
    native_candidate_main_table_equal: bool,
    query: Option<QueryIdentityObservation>,
}

fn invalid() -> FormFieldMapObservationError {
    FormFieldMapObservationError::InvalidContext
}
fn digest_matches(expected: &str, bytes: &[u8]) -> bool {
    expected.len() == 64
        && expected
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        && expected == format!("{:x}", Sha256::digest(bytes))
}
impl ObservationContext {
    pub(super) fn selects_name(&self, name: &str) -> bool {
        self.attributes
            .iter()
            .any(|attribute| attribute.attribute_name == name)
    }

    pub(super) fn read(
        json: &[u8],
        blob: &[u8],
        native_xml: &[u8],
        candidate_xml: &[u8],
    ) -> Result<Self, FormFieldMapObservationError> {
        if json.len() > MAX_CONTEXT_BYTES
            || native_xml.len() > MAX_XML_BYTES
            || candidate_xml.len() > MAX_XML_BYTES
            || blob.len() > MAX_FORM_OBSERVER_COMPRESSED_BYTES
        {
            return Err(FormFieldMapObservationError::Limit);
        }
        let context: Self = serde_json::from_slice(json).map_err(|_| invalid())?;
        if context.schema_version != 1
            || !digest_matches(&context.body_sha256, blob)
            || !digest_matches(&context.native_xml_sha256, native_xml)
            || !digest_matches(&context.candidate_xml_sha256, candidate_xml)
            || context.attributes.is_empty()
            || context.attributes.len() > MAX_GROUPS
        {
            return Err(invalid());
        }
        let mut names = BTreeSet::new();
        let mut ordinals = BTreeSet::new();
        for attribute in &context.attributes {
            if attribute.attribute_name.is_empty()
                || attribute.attribute_name.len() > MAX_CONTEXT_NAME_BYTES
                || attribute.native_main_table.is_empty()
                || attribute.native_main_table.len() > MAX_MAIN_TABLE_BYTES
                || attribute.native_main_table != attribute.candidate_main_table
                || attribute.affected_xml_ordinal == 0
                || attribute.affected_xml_ordinal > MAX_GROUPS
                || !names.insert(&attribute.attribute_name)
                || !ordinals.insert(attribute.affected_xml_ordinal)
            {
                return Err(invalid());
            }
        }
        Ok(context)
    }
    pub(super) fn observe(
        &self,
        admitted: &[(String, PreparedBag<'_>)],
    ) -> Result<Vec<BoundQueryGroup>, FormFieldMapObservationError> {
        self.attributes
            .iter()
            .map(|attribute| {
                let mut matched = admitted
                    .iter()
                    .enumerate()
                    .filter(|(_, (name, _))| name == &attribute.attribute_name);
                let (index, (_, bag)) = matched.next().ok_or_else(invalid)?;
                if matched.next().is_some() {
                    return Err(invalid());
                }
                let query = bag
                    .query_text
                    .as_deref()
                    .and_then(parse_form_dynamic_list_query_selection_diagnostic)
                    .map(|query| observe_query_identity(&query, &attribute.native_main_table));
                Ok(BoundQueryGroup {
                    affected_xml_ordinal: attribute.affected_xml_ordinal,
                    raw_group_ordinal: index + 1,
                    exact_attribute_name_join: true,
                    native_candidate_main_table_equal: true,
                    query,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{bag, form, record};
    use super::*;

    fn source() -> (Vec<u8>, Vec<u8>, Vec<u8>, serde_json::Value) {
        let a = record(&bag(
            "SELECT T.ПометкаУдаления AS ПометкаУдаления FROM Catalog.PRIVATE_TABLE AS T",
        ))
        .replace("PRIVATE_ATTRIBUTE", "PRIVATE_FIRST");
        let b = record(&bag(
            "SELECT T.Ref AS Alias FROM Catalog.PRIVATE_TABLE AS T",
        ))
        .replace("PRIVATE_ATTRIBUTE", "PRIVATE_SECOND");
        let body = form(&[b, a]);
        let native = b"<public native binding fixture/>".to_vec();
        let candidate = b"<public candidate binding fixture/>".to_vec();
        let context = serde_json::json!({
            "schema_version": 1,
            "body_sha256": format!("{:x}", Sha256::digest(&body)),
            "native_xml_sha256": format!("{:x}", Sha256::digest(&native)),
            "candidate_xml_sha256": format!("{:x}", Sha256::digest(&candidate)),
            "attributes": [
                {"affected_xml_ordinal":1,"attribute_name":"PRIVATE_FIRST", "native_main_table":"Catalog.PRIVATE_TABLE", "candidate_main_table":"Catalog.PRIVATE_TABLE"},
                {"affected_xml_ordinal":2,"attribute_name":"PRIVATE_SECOND", "native_main_table":"Catalog.PRIVATE_TABLE", "candidate_main_table":"Catalog.PRIVATE_TABLE"}
            ]
        });
        (body, native, candidate, context)
    }
    fn run(
        body: &[u8],
        n: &[u8],
        c: &[u8],
        context: &serde_json::Value,
    ) -> Result<FormFieldMapObservationReport, FormFieldMapObservationError> {
        observe_form_field_map_blob_with_context(body, &serde_json::to_vec(context).unwrap(), n, c)
    }
    #[test]
    fn exact_name_join_survives_reorder_without_disclosing_private_context() {
        let (body, n, c, context) = source();
        let report = run(&body, &n, &c, &context).unwrap();
        let joined = report.context_bound_groups.as_ref().unwrap();
        assert_eq!(
            joined
                .iter()
                .map(|g| (g.affected_xml_ordinal, g.raw_group_ordinal))
                .collect::<Vec<_>>(),
            [(1, 2), (2, 1)]
        );
        fn assert_private_context_absent(value: &serde_json::Value) {
            let json = serde_json::to_string(value).unwrap();
            for private in [
                "PRIVATE",
                "SELECT",
                "Catalog",
                "public native binding",
                "public candidate binding",
            ] {
                assert!(!json.contains(private));
            }
            fn reject_raw_context_keys(value: &serde_json::Value) {
                match value {
                    serde_json::Value::Object(object) => {
                        for (key, child) in object {
                            assert!(
                                ![
                                    "attribute_name",
                                    "native_main_table",
                                    "candidate_main_table",
                                    "body_sha256",
                                    "native_xml_sha256",
                                    "candidate_xml_sha256",
                                    "attributes",
                                ]
                                .contains(&key.as_str())
                            );
                            reject_raw_context_keys(child);
                        }
                    }
                    serde_json::Value::Array(array) => {
                        for child in array {
                            reject_raw_context_keys(child);
                        }
                    }
                    _ => {}
                }
            }
            reject_raw_context_keys(value);
        }
        let value = serde_json::to_value(&report).unwrap();
        assert_private_context_absent(&value);
        // Fixed public join flags are legitimate schema, never raw context.
        assert_private_context_absent(&serde_json::json!({
            "exact_attribute_name_join": true,
            "native_candidate_main_table_equal": true,
        }));
        // Ensure the privacy oracle still rejects both leaking values and
        // exact raw-context keys, including ones nested inside arrays.
        for poison in [
            serde_json::json!({"public_value": "PRIVATE_POISON"}),
            serde_json::json!({"groups": [{"attribute_name": "public synthetic value"}]}),
        ] {
            assert!(std::panic::catch_unwind(|| assert_private_context_absent(&poison)).is_err());
        }
        let old = serde_json::to_string(&observe_form_field_map_blob(&body).unwrap()).unwrap();
        assert!(!old.contains("context_bound_groups"));
    }
    #[test]
    fn context_custody_hashes_schema_and_duplicate_or_missing_joins_refuse() {
        let (body, n, c, context) = source();
        for key in ["body_sha256", "native_xml_sha256", "candidate_xml_sha256"] {
            let mut altered = context.clone();
            altered[key] = serde_json::json!("0".repeat(64));
            assert_eq!(run(&body, &n, &c, &altered).unwrap_err(), invalid());
        }
        let mut variants = Vec::new();
        let mut x = context.clone();
        x["attributes"][1]["attribute_name"] = serde_json::json!("PRIVATE_FIRST");
        variants.push(x);
        let mut x = context.clone();
        x["attributes"][1]["attribute_name"] = serde_json::json!("PRIVATE_MISSING");
        variants.push(x);
        let mut x = context.clone();
        x["attributes"][1]["affected_xml_ordinal"] = serde_json::json!(1);
        variants.push(x);
        let mut x = context.clone();
        x["attributes"][1]["candidate_main_table"] = serde_json::json!("Catalog.OTHER");
        variants.push(x);
        let mut x = context.clone();
        x["unexpected"] = serde_json::json!("PRIVATE_EXTRA");
        variants.push(x);
        for variant in variants {
            assert_eq!(run(&body, &n, &c, &variant).unwrap_err(), invalid());
        }
        let records = vec![record(&bag("SELECT T.Ref FROM Catalog.PRIVATE_TABLE AS T")); 2];
        let repeated = form(&records);
        let mut x = context.clone();
        x["body_sha256"] = serde_json::json!(format!("{:x}", Sha256::digest(&repeated)));
        x["attributes"].as_array_mut().unwrap().truncate(1);
        x["attributes"][0]["attribute_name"] = serde_json::json!("PRIVATE_ATTRIBUTE");
        assert_eq!(run(&repeated, &n, &c, &x).unwrap_err(), invalid());
    }
    #[test]
    fn duplicate_raw_names_cannot_hide_behind_nonmanual_or_nondynamic_filters() {
        let (_, n, c, mut context) = source();
        context["attributes"].as_array_mut().unwrap().truncate(1);
        context["attributes"][0]["attribute_name"] = serde_json::json!("PRIVATE_ATTRIBUTE");
        let manual = record(&bag("SELECT T.Ref FROM Catalog.PRIVATE_TABLE AS T"));
        let nonmanual = manual.replace(r#""ManualQuery",{"B",1}"#, r#""ManualQuery",{"B",0}"#);
        let nondynamic = manual.replace(
            FORM_DYNAMIC_LIST_TYPE_UUID,
            "00000000-0000-0000-0000-000000000000",
        );
        for other in [nonmanual, nondynamic] {
            let body = form(&[manual.clone(), other]);
            context["body_sha256"] = serde_json::json!(format!("{:x}", Sha256::digest(&body)));
            assert_eq!(run(&body, &n, &c, &context).unwrap_err(), invalid());
        }
    }
    #[test]
    fn context_byte_name_and_xml_bounds_fail_closed() {
        let (body, n, c, mut context) = source();
        assert_eq!(
            observe_form_field_map_blob_with_context(
                &body,
                &vec![0; MAX_CONTEXT_BYTES + 1],
                &n,
                &c
            )
            .unwrap_err(),
            FormFieldMapObservationError::Limit
        );
        assert_eq!(
            observe_form_field_map_blob_with_context(&body, b"{}", &vec![0; MAX_XML_BYTES + 1], &c)
                .unwrap_err(),
            FormFieldMapObservationError::Limit
        );
        context["attributes"][0]["attribute_name"] =
            serde_json::json!("x".repeat(MAX_CONTEXT_NAME_BYTES + 1));
        assert_eq!(run(&body, &n, &c, &context).unwrap_err(), invalid());
    }
}
