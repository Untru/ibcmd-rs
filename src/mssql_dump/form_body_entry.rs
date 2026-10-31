//! The root's side of the form body reader (`ibcmd_form_body::form_body`): the entry
//! points that decode a stored form through the root's managed-form codec
//! (`compiler::bodies::form`) or build a `FormParseContext` with the state of
//! the extension export running now ([`active_form_extension_export`]), and
//! the test that reads the root's metadata rows. `mssql_dump` reaches them by
//! their old names.

use super::*;

/// The extension export running now, as a form reads it; `None` in an
/// ordinary export. The extended configuration's compatibility mode is read
/// once, right after the export activates its context
/// (`extension::export_extension_records_to_source`), so this is a cached read.
pub(super) fn active_form_extension_export() -> Option<FormExtensionExport> {
    super::extension::active().map(|extension| FormExtensionExport {
        extended_compatibility_mode: extension.extended_compatibility_mode().map(str::to_owned),
    })
}

#[allow(dead_code)]
pub(crate) fn extract_form_body_xml(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let body =
        ibcmd_compiler::compiler::bodies::form::decode_compatible_managed_form(bytes).ok()?;
    // A platform 8.5 body is read through its 8.3.27 down-conversion, the way
    // the source export reads one (`source_assets`). What the members 8.5
    // appends say is only spelled by the 2.21 writer, which this 2.20 reading
    // is not.
    if super::form::layout_8_5_1::is_form_body_8_5_1(body.parsed()) {
        let (converted, _) =
            super::form::layout_8_5_1::down_convert_form_body_8_5_1(body.parsed()).ok()?;
        return extract_form_body_xml_from_body(&converted, object_refs, object_refs);
    }
    extract_form_body_xml_from_body(body.parsed(), object_refs, object_refs)
}

pub(super) fn extract_form_body_xml_from_body(
    body: &ParsedFormBodyBlob,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    extract_form_body_xml_from_body_timed(
        body,
        &FormParseContext::new(
            type_index,
            &BTreeSet::new(),
            &DcsTypeIndex::new(),
            object_refs,
            &Arc::new(BTreeMap::new()),
            &BTreeMap::new(),
            &Arc::new(InformationRegisterMasterDimensionIndex::new()),
            None,
            active_form_extension_export(),
        ),
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn trace_form_body_with_context(
    body: &ParsedFormBodyBlob,
    type_index: &BTreeMap<String, String>,
    dcs_type_index: &DcsTypeIndex,
    object_refs: &BTreeMap<String, String>,
    field_type_refs: &Arc<BTreeMap<String, String>>,
    information_register_field_refs: &InformationRegisterFieldReferenceIndex,
    information_register_master_dimensions: &Arc<InformationRegisterMasterDimensionIndex>,
    form_owner_reference: Option<&str>,
    trace_sink: &dyn FormItemTraceSink,
) -> Option<()> {
    let type_index_collisions = BTreeSet::new();
    let context = FormParseContext::new(
        type_index,
        &type_index_collisions,
        dcs_type_index,
        object_refs,
        field_type_refs,
        information_register_field_refs,
        information_register_master_dimensions,
        form_owner_reference,
        active_form_extension_export(),
    )
    .with_trace_sink(trace_sink);
    extract_form_body_xml_from_body_timed(body, &context, None).map(|_| ())
}

pub(crate) fn unpack_form_body_module_text(blob: &[u8]) -> Option<Vec<u8>> {
    let body = ibcmd_compiler::compiler::bodies::form::decode_compatible_managed_form(blob).ok()?;
    form_body_module_text_bytes(body.parsed())
}

/// The Form.xml this exporter writes for a whole form body, read against the
/// offline context of a saved dump -- the form writer's whole-form harness
/// renders a compiled body and the stored one through it and compares.
#[cfg(test)]
pub(crate) fn render_form_body_xml_offline(
    body: &ParsedFormBodyBlob,
    context: &super::offline_context::OfflineFormContext,
    form_uuid: &str,
) -> Option<String> {
    let collisions = BTreeSet::new();
    let owner = context
        .form_owner_references
        .get(form_uuid)
        .map(String::as_str);
    let parse_context = FormParseContext::new(
        &context.type_index,
        &collisions,
        &context.dcs_type_index,
        &context.object_refs,
        &context.field_type_refs,
        &context.information_register_field_refs,
        &context.information_register_master_dimensions,
        owner,
        active_form_extension_export(),
    );
    extract_form_body_xml_from_body_timed(body, &parse_context, None)
}

#[cfg(test)]
mod reference_alias_tests {
    use super::*;

    #[test]
    fn renamed_reference_removes_auto_fields_in_both_script_variants() {
        for (script_code, reference, description) in
            [("0", "Ref", "Description"), ("1", "Ссылка", "Наименование")]
        {
            let mut fields = ["0"; 61];
            fields[0] = "68";
            fields[3] = script_code;
            let root = MetadataTextRow {
                file_name: "11111111-1111-4111-8111-111111111111".into(),
                text: format!(
                    "{{9cd510cd-abfc-11d4-9434-004095e12fc7,{{{}}}}}",
                    fields.join(",")
                ),
                object_code: None,
                header: None,
                kind: None,
                folder: None,
            };
            let declarations = refs::build_metadata_field_declaration_index_from_texts(
                &[root],
                &BTreeMap::new(),
                &BTreeMap::new(),
            );
            assert_eq!(declarations.english_script(), script_code == "0");
            let query = format!("SELECT P.{reference} AS SelectedRef FROM Catalog.Products AS P");
            let selected = parse_form_dynamic_list_query_selection(&query).unwrap();
            let fields = form_dynamic_list_main_table_auto_fields(
                "Catalog.Products",
                &selected,
                &BTreeMap::new(),
                Some(&declarations),
                None,
            )
            .unwrap();
            assert!(fields.is_empty());

            let query = format!("SELECT P.{reference} AS {reference} FROM Catalog.Products AS P");
            let selected = parse_form_dynamic_list_query_selection(&query).unwrap();
            let fields = form_dynamic_list_main_table_auto_fields(
                "Catalog.Products",
                &selected,
                &BTreeMap::new(),
                Some(&declarations),
                None,
            )
            .unwrap();
            assert!(fields.contains(reference));
            assert!(fields.contains(description));
        }
    }
}
