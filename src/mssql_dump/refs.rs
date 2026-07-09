use super::*;

#[allow(dead_code)]
pub(super) fn build_metadata_command_reference_index(
    rows: &[ConfigRow],
) -> BTreeMap<String, MetadataCommandReference> {
    let metadata_texts = build_metadata_text_rows(rows);
    build_metadata_command_reference_index_from_texts(&metadata_texts)
}

pub(super) fn build_metadata_command_reference_index_from_texts(
    rows: &[MetadataTextRow],
) -> BTreeMap<String, MetadataCommandReference> {
    let mut index = BTreeMap::new();
    for row in rows {
        let (Some(kind), Some(header)) = (row.kind.as_deref(), row.header.as_ref()) else {
            continue;
        };
        index.insert(
            row.file_name.clone(),
            MetadataCommandReference {
                kind: kind.to_string(),
                name: header.name.clone(),
            },
        );
    }
    index
}

#[allow(dead_code)]
pub(super) fn build_metadata_object_reference_index(
    rows: &[ConfigRow],
) -> BTreeMap<String, String> {
    let metadata_texts = build_metadata_text_rows(rows);
    build_metadata_object_reference_index_from_texts(&metadata_texts)
}

pub(super) fn build_metadata_object_reference_index_from_texts(
    rows: &[MetadataTextRow],
) -> BTreeMap<String, String> {
    let mut index = BTreeMap::new();
    let empty_form_refs = BTreeMap::new();
    let empty_template_refs = BTreeMap::new();
    let subsystem_refs = build_subsystem_source_reference_index_from_texts(rows);
    let headers_by_uuid = metadata_headers_by_uuid(rows);
    for row in rows {
        if let Some(name) = parse_configuration_reference_text(&row.text) {
            index.insert(row.file_name.clone(), format!("Configuration.{name}"));
            continue;
        }
        let (Some(kind), Some(header)) = (row.kind.as_deref(), row.header.as_ref()) else {
            continue;
        };
        let reference = if kind == "Subsystem" {
            subsystem_refs
                .get(&header.uuid)
                .and_then(subsystem_source_reference_name)
                .unwrap_or_else(|| format!("{kind}.{}", header.name))
        } else {
            format!("{kind}.{}", header.name)
        };
        index.insert(row.file_name.clone(), reference);
        if kind == "Enum" {
            for value in parse_enum_values_from_text(&row.text) {
                index.insert(
                    value.uuid,
                    format!("Enum.{}.EnumValue.{}", header.name, value.name),
                );
            }
        }
        if kind == "CalculationRegister" {
            for uuid in calculation_register_recalculation_uuids_from_text(&row.text) {
                if let Some(recalculation) = headers_by_uuid.get(&uuid) {
                    index.insert(
                        uuid,
                        format!(
                            "CalculationRegister.{}.Recalculation.{}",
                            header.name, recalculation.name
                        ),
                    );
                }
            }
        }
        for command in nested_command_headers_from_text(&row.text, &row.file_name) {
            index.insert(
                command.uuid,
                format!("{}.{}.Command.{}", kind, header.name, command.name),
            );
        }
        for (child, marker_start) in
            nested_headers_with_offsets_from_text(&row.text, &row.file_name, |_| true)
        {
            if let Some(reference) = standalone_child_reference(
                kind,
                &header.name,
                &header.uuid,
                &row.text,
                marker_start,
                &child,
                &empty_form_refs,
                &empty_template_refs,
            ) {
                index.entry(child.uuid).or_insert(reference);
            }
        }
        if kind == "WebService" {
            for operation in
                nested_web_service_operation_headers_from_text(&row.text, &row.file_name)
            {
                index.insert(
                    operation.uuid,
                    format!("WebService.{}.Operation.{}", header.name, operation.name),
                );
            }
        }
        if kind == "HTTPService" {
            insert_http_service_child_role_refs(&mut index, &row.text, &header.uuid, &header.name);
        }
    }
    index
}

fn metadata_headers_by_uuid(rows: &[MetadataTextRow]) -> BTreeMap<String, MetadataHeader> {
    rows.iter()
        .filter_map(|row| {
            row.header
                .as_ref()
                .map(|header| (header.uuid.clone(), header.clone()))
        })
        .collect()
}

pub(super) fn build_role_rights_object_reference_index(
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> BTreeMap<String, String> {
    let mut refs = object_refs.clone();
    for (uuid, form_ref) in form_refs {
        if let Some(reference) = form_source_reference_name(form_ref) {
            refs.insert(uuid.clone(), reference);
        }
    }
    refs
}

pub(super) fn build_metadata_order_index_from_texts(
    rows: &[MetadataTextRow],
) -> BTreeMap<String, usize> {
    let mut index = BTreeMap::new();
    for row in rows {
        let Some(header_uuid) = parse_configuration_header_uuid(&row.text) else {
            continue;
        };
        for (order, child) in
            parse_configuration_child_objects(&row.text, &row.file_name, &header_uuid)
                .into_iter()
                .enumerate()
        {
            index.entry(child.header.uuid).or_insert(order);
        }
    }
    index
}

pub(super) fn parse_configuration_header_uuid(text: &str) -> Option<String> {
    if !text.trim_start().starts_with("{2,") {
        return None;
    }
    let marker = "{1,0,";
    let marker_start = text.find(marker)?;
    let header_uuid_start = marker_start + marker.len();
    let header_uuid_end = header_uuid_start + 36;
    let header_uuid = text.get(header_uuid_start..header_uuid_end)?;
    if !is_uuid_text(header_uuid) || !is_metadata_header_marker(text, header_uuid_end) {
        return None;
    }
    Some(header_uuid.to_string())
}

pub(super) fn insert_http_service_child_role_refs(
    index: &mut BTreeMap<String, String>,
    text: &str,
    owner_uuid: &str,
    owner_name: &str,
) {
    for template in parse_http_service_url_templates_from_text(text, owner_uuid) {
        let template_ref = format!(
            "HTTPService.{owner_name}.URLTemplate.{}",
            template.header.name
        );
        index.insert(template.header.uuid.clone(), template_ref.clone());
        for method in template.methods {
            index.insert(
                method.header.uuid,
                format!("{template_ref}.Method.{}", method.header.name),
            );
        }
    }
}

pub(super) fn build_standalone_content_references(
    rows: &[MetadataTextRow],
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> StandaloneContentReferences {
    let mut standalone_object_refs = object_refs.clone();
    for (uuid, form_ref) in form_refs {
        if let Some(reference) = form_source_reference_name(form_ref) {
            standalone_object_refs.insert(uuid.clone(), reference);
        }
    }
    for (uuid, template_ref) in template_refs {
        if let Some(reference) = template_source_reference_name(template_ref) {
            standalone_object_refs.insert(uuid.clone(), reference);
        }
    }
    for (uuid, subsystem_ref) in subsystem_refs {
        if let Some(reference) = subsystem_source_reference_name(subsystem_ref) {
            standalone_object_refs.insert(uuid.clone(), reference);
        }
    }

    for row in rows {
        let (Some(kind), Some(header)) = (row.kind.as_deref(), row.header.as_ref()) else {
            continue;
        };
        let mut seen = BTreeSet::new();
        for (child, marker_start) in
            nested_headers_with_offsets_from_text(&row.text, &row.file_name, |_| true)
        {
            if let Some(reference) = standalone_child_reference(
                kind,
                &header.name,
                &header.uuid,
                &row.text,
                marker_start,
                &child,
                form_refs,
                template_refs,
            ) && seen.insert(child.uuid.clone())
            {
                standalone_object_refs.insert(child.uuid, reference);
            }
        }
        for uuid in uuid_like_values(&row.text) {
            if standalone_object_refs.contains_key(&uuid) {
                continue;
            }
            if let Some(reference) = form_refs.get(&uuid).and_then(form_source_reference_name) {
                standalone_object_refs.insert(uuid, reference);
            } else if let Some(reference) = template_refs
                .get(&uuid)
                .and_then(template_source_reference_name)
            {
                standalone_object_refs.insert(uuid, reference);
            }
        }
    }

    StandaloneContentReferences {
        object_refs: standalone_object_refs,
    }
}

pub(super) fn build_standalone_object_reference_index_from_texts(
    rows: &[MetadataTextRow],
    required_refs: &BTreeSet<String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> BTreeMap<String, String> {
    let mut index = BTreeMap::new();
    if required_refs.is_empty() {
        return index;
    }

    for row in rows {
        if required_refs.contains(&row.file_name) {
            if let Some(name) = parse_configuration_reference_text(&row.text) {
                index.insert(row.file_name.clone(), format!("Configuration.{name}"));
                continue;
            }
            let (Some(kind), Some(header)) = (row.kind.as_deref(), row.header.as_ref()) else {
                continue;
            };
            let reference = if kind == "Subsystem" {
                subsystem_refs
                    .get(&header.uuid)
                    .and_then(subsystem_source_reference_name)
                    .unwrap_or_else(|| format!("{kind}.{}", header.name))
            } else {
                format!("{kind}.{}", header.name)
            };
            index.insert(row.file_name.clone(), reference);
        }

        let (Some(kind), Some(header)) = (row.kind.as_deref(), row.header.as_ref()) else {
            continue;
        };
        if kind == "Enum" {
            for value in parse_enum_values_from_text(&row.text) {
                if required_refs.contains(&value.uuid) {
                    index.insert(
                        value.uuid,
                        format!("Enum.{}.EnumValue.{}", header.name, value.name),
                    );
                }
            }
        }
        if kind == "HTTPService" {
            insert_required_http_service_child_role_refs(
                &mut index,
                &row.text,
                &header.uuid,
                &header.name,
                required_refs,
            );
        }
        for (child, marker_start) in
            nested_headers_with_offsets_matching_uuids(&row.text, &row.file_name, required_refs)
        {
            if index.contains_key(&child.uuid) {
                continue;
            }
            if let Some(reference) = standalone_child_reference(
                kind,
                &header.name,
                &header.uuid,
                &row.text,
                marker_start,
                &child,
                form_refs,
                template_refs,
            ) {
                index.insert(child.uuid, reference);
            }
        }
    }

    index
}

pub(super) fn insert_required_http_service_child_role_refs(
    index: &mut BTreeMap<String, String>,
    text: &str,
    owner_uuid: &str,
    owner_name: &str,
    required_refs: &BTreeSet<String>,
) {
    for template in parse_http_service_url_templates_from_text(text, owner_uuid) {
        let template_ref = format!(
            "HTTPService.{owner_name}.URLTemplate.{}",
            template.header.name
        );
        if required_refs.contains(&template.header.uuid) {
            index.insert(template.header.uuid.clone(), template_ref.clone());
        }
        for method in template.methods {
            if required_refs.contains(&method.header.uuid) {
                index.insert(
                    method.header.uuid,
                    format!("{template_ref}.Method.{}", method.header.name),
                );
            }
        }
    }
}

pub(super) fn build_standalone_content_references_for_uuids(
    rows: &[MetadataTextRow],
    required_refs: &BTreeSet<String>,
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> StandaloneContentReferences {
    let mut standalone_object_refs = object_refs.clone();
    for uuid in required_refs {
        if standalone_object_refs.contains_key(uuid) {
            continue;
        }
        if let Some(reference) = form_refs.get(uuid).and_then(form_source_reference_name) {
            standalone_object_refs.insert(uuid.clone(), reference);
        } else if let Some(reference) = template_refs
            .get(uuid)
            .and_then(template_source_reference_name)
        {
            standalone_object_refs.insert(uuid.clone(), reference);
        } else if let Some(reference) = subsystem_refs
            .get(uuid)
            .and_then(subsystem_source_reference_name)
        {
            standalone_object_refs.insert(uuid.clone(), reference);
        }
    }

    for row in rows {
        let (Some(kind), Some(header)) = (row.kind.as_deref(), row.header.as_ref()) else {
            continue;
        };
        for (child, marker_start) in
            nested_headers_with_offsets_matching_uuids(&row.text, &row.file_name, required_refs)
        {
            if standalone_object_refs.contains_key(&child.uuid) {
                continue;
            }
            if let Some(reference) = standalone_child_reference(
                kind,
                &header.name,
                &header.uuid,
                &row.text,
                marker_start,
                &child,
                form_refs,
                template_refs,
            ) {
                standalone_object_refs.insert(child.uuid, reference);
            }
        }
    }

    StandaloneContentReferences {
        object_refs: standalone_object_refs,
    }
}

pub(super) fn build_help_reference_index(
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> BTreeMap<String, String> {
    let mut refs = object_refs.clone();
    for (uuid, form_ref) in form_refs {
        if let Some(reference) = form_source_reference_name(form_ref) {
            refs.insert(uuid.clone(), reference);
        }
    }
    for (uuid, template_ref) in template_refs {
        if let Some(reference) = template_source_reference_name(template_ref) {
            refs.insert(uuid.clone(), reference);
        }
    }
    for (uuid, subsystem_ref) in subsystem_refs {
        if let Some(reference) = subsystem_source_reference_name(subsystem_ref) {
            refs.insert(uuid.clone(), reference);
        }
    }
    refs
}

pub(super) fn build_functional_option_reference_index_from_texts(
    rows: &[MetadataTextRow],
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> BTreeMap<String, String> {
    let mut refs = object_refs.clone();
    for (uuid, subsystem_ref) in subsystem_refs {
        if let Some(reference) = subsystem_source_reference_name(subsystem_ref) {
            refs.insert(uuid.clone(), reference);
        }
    }
    let required_refs = functional_option_reference_uuids_from_texts(rows);
    if required_refs.is_empty() {
        return refs;
    }
    for row in rows {
        let (Some(kind), Some(header)) = (row.kind.as_deref(), row.header.as_ref()) else {
            continue;
        };
        let mut seen = BTreeSet::new();
        for (child, marker_start) in
            nested_headers_with_offsets_matching_uuids(&row.text, &row.file_name, &required_refs)
        {
            if refs.contains_key(&child.uuid) || seen.contains(&child.uuid) {
                continue;
            }
            if let Some(reference) = standalone_child_reference(
                kind,
                &header.name,
                &header.uuid,
                &row.text,
                marker_start,
                &child,
                form_refs,
                template_refs,
            ) {
                seen.insert(child.uuid.clone());
                refs.insert(child.uuid, reference);
            }
        }
    }
    refs
}

pub(super) fn functional_option_reference_uuids_from_texts(
    rows: &[MetadataTextRow],
) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    for row in rows {
        if row.kind.as_deref() != Some("FunctionalOption") {
            continue;
        }
        let Some(fields) = metadata_object_fields(&row.text) else {
            continue;
        };
        if let Some(uuid) = fields
            .get(2)
            .and_then(|field| parse_non_zero_uuid(field.trim()))
        {
            refs.insert(uuid);
        }
        if let Some(content) = fields.get(3) {
            refs.extend(uuid_like_values_in_text_order(content));
        }
    }
    refs
}

pub(super) fn nested_headers_with_offsets_matching_uuids(
    text: &str,
    owner_uuid: &str,
    uuids: &BTreeSet<String>,
) -> Vec<(MetadataHeader, usize)> {
    let mut headers = Vec::new();
    let mut seen = BTreeSet::new();
    let mut offset = 0usize;
    let marker = "{1,0,";

    while let Some(relative) = text[offset..].find(marker) {
        let marker_start = offset + relative;
        let uuid_start = marker_start + marker.len();
        let uuid_end = uuid_start + 36;
        offset = uuid_start;

        let Some(uuid) = text.get(uuid_start..uuid_end) else {
            continue;
        };
        if uuid == owner_uuid
            || !uuids.contains(uuid)
            || !is_uuid_text(uuid)
            || !is_metadata_header_marker(text, uuid_end)
            || !seen.insert(uuid.to_string())
        {
            continue;
        }
        if let Some(header) = parse_metadata_header_from_text(text, uuid) {
            headers.push((header, marker_start));
        }
    }

    headers
}

pub(super) fn standalone_child_reference(
    owner_kind: &str,
    owner_name: &str,
    owner_uuid: &str,
    text: &str,
    marker_start: usize,
    child: &MetadataHeader,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
) -> Option<String> {
    if let Some(reference) = form_refs
        .get(&child.uuid)
        .and_then(form_source_reference_name)
    {
        return Some(reference);
    }
    if let Some(reference) = template_refs
        .get(&child.uuid)
        .and_then(template_source_reference_name)
    {
        return Some(reference);
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 9) {
        return Some(format!("{owner_kind}.{owner_name}.Command.{}", child.name));
    }
    if owner_kind == "WebService" && is_offset_inside_metadata_object_code(text, marker_start, 1) {
        return Some(format!("WebService.{owner_name}.Operation.{}", child.name));
    }
    if owner_kind == "IntegrationService"
        && is_offset_inside_metadata_object_code(text, marker_start, 1)
    {
        return Some(format!(
            "IntegrationService.{owner_name}.IntegrationServiceChannel.{}",
            child.name
        ));
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 11) {
        if let Some(tabular_section) = enclosing_metadata_header_for_code(text, marker_start, 11)
            && tabular_section.uuid != child.uuid
        {
            return Some(format!(
                "{owner_kind}.{owner_name}.TabularSection.{}.Attribute.{}",
                tabular_section.name, child.name
            ));
        }
        return Some(format!(
            "{owner_kind}.{owner_name}.TabularSection.{}",
            child.name
        ));
    }
    if let Some(tabular_section) = enclosing_metadata_header_for_code(text, marker_start, 11)
        && tabular_section.uuid != child.uuid
    {
        return Some(format!(
            "{owner_kind}.{owner_name}.TabularSection.{}.Attribute.{}",
            tabular_section.name, child.name
        ));
    }
    if let Some(reference) = tabular_section_attribute_reference(
        owner_kind,
        owner_name,
        owner_uuid,
        text,
        marker_start,
        child,
    ) {
        return Some(reference);
    }
    if metadata_kind_uses_register_resources(owner_kind)
        && is_offset_inside_metadata_object_code(text, marker_start, 5)
        && is_offset_inside_register_resource_list(text, marker_start)
    {
        return Some(format!("{owner_kind}.{owner_name}.Resource.{}", child.name));
    }
    if owner_kind == "CalculationRegister"
        && is_offset_inside_metadata_object_code(text, marker_start, 4)
        && is_offset_inside_calculation_register_recalculation_list(text, marker_start)
    {
        return Some(format!(
            "CalculationRegister.{owner_name}.Recalculation.{}",
            child.name
        ));
    }
    if owner_kind == "Task"
        && is_offset_inside_metadata_object_code(text, marker_start, 4)
        && is_offset_inside_metadata_object_code(text, marker_start, 27)
    {
        return Some(format!(
            "Task.{owner_name}.AddressingAttribute.{}",
            child.name
        ));
    }
    if owner_kind == "DataProcessor"
        && is_offset_inside_metadata_object_code(text, marker_start, 27)
        && is_offset_inside_tabular_section_attribute_list(text, marker_start)
        && let Some((tabular_section, tabular_end)) =
            preceding_metadata_header_for_code_with_bounds(text, marker_start, 11)
        && tabular_section.uuid != child.uuid
        && !contains_metadata_header_uuid_between(text, tabular_end, marker_start, owner_uuid)
        && !contains_metadata_header_name_between(text, tabular_end, marker_start, owner_name)
    {
        return Some(format!(
            "{owner_kind}.{owner_name}.TabularSection.{}.Attribute.{}",
            tabular_section.name, child.name
        ));
    }
    if owner_kind == "DataProcessor"
        && is_offset_inside_metadata_object_code(text, marker_start, 27)
    {
        return Some(format!(
            "{owner_kind}.{owner_name}.Attribute.{}",
            child.name
        ));
    }
    if metadata_kind_uses_code27_attributes(owner_kind)
        && is_offset_inside_metadata_object_code(text, marker_start, 27)
    {
        return Some(format!(
            "{owner_kind}.{owner_name}.Attribute.{}",
            child.name
        ));
    }
    if metadata_kind_uses_code4_attributes(owner_kind)
        && is_offset_inside_metadata_object_code(text, marker_start, 4)
        && is_offset_inside_metadata_object_code(text, marker_start, 27)
    {
        return Some(format!(
            "{owner_kind}.{owner_name}.Attribute.{}",
            child.name
        ));
    }
    if owner_kind == "BusinessProcess"
        && is_offset_inside_metadata_object_code(text, marker_start, 3)
        && is_offset_inside_metadata_object_code(text, marker_start, 27)
        && !is_offset_inside_metadata_object_code(text, marker_start, 8)
    {
        return Some(format!(
            "{owner_kind}.{owner_name}.Attribute.{}",
            child.name
        ));
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 5) {
        return Some(format!(
            "{owner_kind}.{owner_name}.Attribute.{}",
            child.name
        ));
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 6) {
        if let Some(tabular_section) = enclosing_metadata_header_for_code(text, marker_start, 11)
            && tabular_section.uuid != child.uuid
        {
            return Some(format!(
                "{owner_kind}.{owner_name}.TabularSection.{}.Attribute.{}",
                tabular_section.name, child.name
            ));
        }
        return Some(format!(
            "{owner_kind}.{owner_name}.Attribute.{}",
            child.name
        ));
    }
    if metadata_kind_uses_register_resources(owner_kind)
        && is_offset_inside_metadata_object_code(text, marker_start, 8)
        && is_offset_inside_register_dimension_list(text, marker_start)
    {
        return Some(format!(
            "{owner_kind}.{owner_name}.Dimension.{}",
            child.name
        ));
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 8) {
        if let Some(tabular_section) = preceding_metadata_header_for_code(text, marker_start, 11) {
            return Some(format!(
                "{owner_kind}.{owner_name}.TabularSection.{}.Attribute.{}",
                tabular_section.name, child.name
            ));
        }
        return Some(format!("{owner_kind}.{owner_name}.Resource.{}", child.name));
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 10) {
        return Some(format!(
            "{owner_kind}.{owner_name}.Dimension.{}",
            child.name
        ));
    }
    None
}

pub(super) fn tabular_section_attribute_reference(
    owner_kind: &str,
    owner_name: &str,
    _owner_uuid: &str,
    text: &str,
    marker_start: usize,
    child: &MetadataHeader,
) -> Option<String> {
    if !is_offset_inside_tabular_section_attribute_list(text, marker_start) {
        return None;
    }
    let (tabular_section, tabular_end) =
        preceding_metadata_header_for_code_with_bounds(text, marker_start, 11)?;
    if tabular_section.uuid == child.uuid
        || contains_any_metadata_header_between(text, tabular_end, marker_start)
    {
        return None;
    }
    Some(format!(
        "{owner_kind}.{owner_name}.TabularSection.{}.Attribute.{}",
        tabular_section.name, child.name
    ))
}

pub(super) fn metadata_kind_uses_register_resources(kind: &str) -> bool {
    matches!(
        kind,
        "AccumulationRegister"
            | "AccountingRegister"
            | "CalculationRegister"
            | "InformationRegister"
    )
}

pub(super) fn metadata_kind_uses_code27_attributes(kind: &str) -> bool {
    matches!(kind, "Report" | "Task" | "ChartOfCharacteristicTypes")
}

pub(super) fn metadata_kind_uses_code4_attributes(kind: &str) -> bool {
    kind == "ExchangePlan" || metadata_kind_uses_register_resources(kind)
}

pub(super) fn is_offset_inside_register_resource_list(text: &str, offset: usize) -> bool {
    const REGISTER_RESOURCE_LIST_MARKER: &str = "{b64d9a41-1642-11d6-a3c7-0050bae0a776,";
    let Some(start) = text[..offset].rfind(REGISTER_RESOURCE_LIST_MARKER) else {
        return false;
    };
    scan_1c_braced_value(text, start)
        .map(|end| offset < end)
        .unwrap_or(false)
}

pub(super) fn is_offset_inside_register_dimension_list(text: &str, offset: usize) -> bool {
    const REGISTER_DIMENSION_LIST_MARKER: &str = "{b64d9a43-1642-11d6-a3c7-0050bae0a776,";
    let Some(start) = text[..offset].rfind(REGISTER_DIMENSION_LIST_MARKER) else {
        return false;
    };
    scan_1c_braced_value(text, start)
        .map(|end| offset < end)
        .unwrap_or(false)
}

pub(super) fn is_offset_inside_calculation_register_recalculation_list(
    text: &str,
    offset: usize,
) -> bool {
    const RECALCULATION_LIST_MARKER: &str = "{274bf899-db0e-4df6-8ab5-67bf6371ec0b,";
    let Some(start) = text[..offset].rfind(RECALCULATION_LIST_MARKER) else {
        return false;
    };
    scan_1c_braced_value(text, start)
        .map(|end| offset < end)
        .unwrap_or(false)
}

pub(super) fn calculation_register_recalculation_uuids_from_text(text: &str) -> Vec<String> {
    const RECALCULATION_LIST_MARKER: &str = "{274bf899-db0e-4df6-8ab5-67bf6371ec0b,";
    let mut uuids = Vec::new();
    let mut seen = BTreeSet::new();
    let mut offset = 0usize;
    while let Some(relative_start) = text[offset..].find(RECALCULATION_LIST_MARKER) {
        let start = offset + relative_start;
        offset = start + RECALCULATION_LIST_MARKER.len();
        let Some(end) = scan_1c_braced_value(text, start) else {
            continue;
        };
        let Some(fields) = split_1c_braced_fields(&text[start..end], 0) else {
            continue;
        };
        let count = fields
            .get(1)
            .and_then(|field| field.trim().parse::<usize>().ok())
            .unwrap_or(0);
        for uuid in fields
            .iter()
            .skip(2)
            .take(count)
            .filter_map(|field| parse_non_zero_uuid(field.trim()))
        {
            if seen.insert(uuid.clone()) {
                uuids.push(uuid);
            }
        }
    }
    uuids
}

pub(super) fn is_offset_inside_tabular_section_attribute_list(text: &str, offset: usize) -> bool {
    const TABULAR_SECTION_ATTRIBUTE_LIST_MARKER: &str = "{5d24a9d1-098e-11d6-b9b8-0050bae0a95d,";
    let Some(start) = text[..offset].rfind(TABULAR_SECTION_ATTRIBUTE_LIST_MARKER) else {
        return false;
    };
    scan_1c_braced_value(text, start)
        .map(|end| offset < end)
        .unwrap_or(false)
}

pub(super) fn is_offset_inside_data_processor_legacy_attribute_list(
    text: &str,
    offset: usize,
) -> bool {
    const DATA_PROCESSOR_LEGACY_ATTRIBUTE_LIST_MARKER: &str =
        "{ec6bb5e5-b7a8-4d75-bec9-658107a699cf,";
    let Some(start) = text[..offset].rfind(DATA_PROCESSOR_LEGACY_ATTRIBUTE_LIST_MARKER) else {
        return false;
    };
    scan_1c_braced_value(text, start)
        .map(|end| offset < end)
        .unwrap_or(false)
}

pub(super) fn template_source_reference_name(
    template_ref: &TemplateSourceReference,
) -> Option<String> {
    let parts = template_ref
        .relative_path
        .iter()
        .filter_map(|part| part.to_str())
        .collect::<Vec<_>>();
    if parts.len() == 2 && parts.first() == Some(&"CommonTemplates") {
        let template_name = Path::new(parts[1]).file_stem()?.to_str()?;
        return Some(format!("CommonTemplate.{template_name}"));
    }
    if parts.len() == 4 && parts.get(2) == Some(&"Templates") {
        let owner_kind = metadata_kind_for_source_folder(parts[0])?;
        let owner_name = parts[1];
        let template_name = Path::new(parts[3]).file_stem()?.to_str()?;
        return Some(format!(
            "{owner_kind}.{owner_name}.Template.{template_name}"
        ));
    }
    None
}

pub(super) fn subsystem_source_reference_name(
    subsystem_ref: &SubsystemSourceReference,
) -> Option<String> {
    let mut names = Vec::new();
    for part in subsystem_ref
        .relative_path
        .iter()
        .filter_map(|part| part.to_str())
    {
        if part == "Subsystems" {
            continue;
        }
        let name = Path::new(part).file_stem()?.to_str()?;
        names.push(name.to_string());
    }
    let mut names = names.into_iter();
    let first = names.next()?;
    let mut reference = format!("Subsystem.{first}");
    for name in names {
        reference.push_str(".Subsystem.");
        reference.push_str(&name);
    }
    Some(reference)
}

#[allow(dead_code)]
pub(super) fn build_metadata_field_reference_index(rows: &[ConfigRow]) -> BTreeMap<String, String> {
    let metadata_texts = build_metadata_text_rows(rows);
    build_metadata_field_reference_index_from_texts(&metadata_texts)
}

pub(super) fn build_metadata_field_reference_index_from_texts(
    rows: &[MetadataTextRow],
) -> BTreeMap<String, String> {
    let mut index = BTreeMap::new();
    for row in rows {
        for header in nested_metadata_headers_from_text(&row.text, &row.file_name) {
            index.insert(header.uuid, header.name);
        }
    }
    index
}

#[allow(dead_code)]
pub(super) fn build_form_source_reference_index(
    rows: &[ConfigRow],
) -> BTreeMap<String, FormSourceReference> {
    let metadata_texts = build_metadata_text_rows(rows);
    build_form_source_reference_index_from_texts(&metadata_texts)
}

pub(super) fn build_form_source_reference_index_from_texts(
    rows: &[MetadataTextRow],
) -> BTreeMap<String, FormSourceReference> {
    let mut forms = Vec::<MetadataHeader>::new();
    let mut owner_paths_by_ref = BTreeMap::<String, BTreeSet<PathBuf>>::new();

    for row in rows {
        if is_form_metadata_text(&row.text, &row.file_name) {
            if let Some(header) = row.header.as_ref() {
                forms.push(header.clone());
            }
        }
    }
    let form_uuids = forms
        .iter()
        .map(|form| form.uuid.clone())
        .collect::<BTreeSet<_>>();

    for row in rows {
        if is_form_metadata_text(&row.text, &row.file_name) {
            continue;
        }
        let (Some(kind), Some(folder), Some(header)) =
            (row.kind.as_deref(), row.folder, row.header.as_ref())
        else {
            continue;
        };
        if !metadata_kind_can_own_forms(kind) {
            continue;
        }
        let owner_path = PathBuf::from(folder).join(sanitize_source_path_segment(&header.name));
        let Some(references) = owned_form_uuid_values_matching(&row.text, &form_uuids) else {
            continue;
        };
        for reference in references {
            owner_paths_by_ref
                .entry(reference)
                .or_default()
                .insert(owner_path.clone());
        }
    }

    let mut index = BTreeMap::new();
    for form in forms {
        let owner_matches = owner_paths_by_ref.get(&form.uuid).map(BTreeSet::iter);
        let relative_path = if let Some(mut owner_paths) = owner_matches {
            let first = owner_paths.next();
            let second = owner_paths.next();
            if let (Some(owner_path), None) = (first, second) {
                owner_path
                    .join("Forms")
                    .join(sanitize_source_path_segment(&form.name))
                    .with_extension("xml")
            } else {
                PathBuf::from("CommonForms")
                    .join(sanitize_source_path_segment(&form.name))
                    .with_extension("xml")
            }
        } else {
            PathBuf::from("CommonForms")
                .join(sanitize_source_path_segment(&form.name))
                .with_extension("xml")
        };
        let kind = if relative_path.starts_with("CommonForms") {
            "CommonForm"
        } else {
            "Form"
        };
        index.insert(
            form.uuid,
            FormSourceReference {
                relative_path,
                kind,
            },
        );
    }

    index
}

pub(super) fn build_form_owner_resolution_diagnostics_from_texts(
    rows: &[MetadataTextRow],
) -> BTreeMap<String, String> {
    let mut forms = Vec::<MetadataHeader>::new();
    let mut owner_paths_by_ref = BTreeMap::<String, BTreeSet<PathBuf>>::new();

    for row in rows {
        if is_form_metadata_text(&row.text, &row.file_name) {
            if let Some(header) = row.header.as_ref() {
                forms.push(header.clone());
            }
        }
    }
    let form_uuids = forms
        .iter()
        .map(|form| form.uuid.clone())
        .collect::<BTreeSet<_>>();

    for row in rows {
        if is_form_metadata_text(&row.text, &row.file_name) {
            continue;
        }
        let (Some(kind), Some(folder), Some(header)) =
            (row.kind.as_deref(), row.folder, row.header.as_ref())
        else {
            continue;
        };
        if !metadata_kind_can_own_forms(kind) {
            continue;
        }
        let owner_path = PathBuf::from(folder).join(sanitize_source_path_segment(&header.name));
        let Some(references) = owned_form_uuid_values_matching(&row.text, &form_uuids) else {
            continue;
        };
        for reference in references {
            owner_paths_by_ref
                .entry(reference)
                .or_default()
                .insert(owner_path.clone());
        }
    }

    let mut diagnostics = BTreeMap::new();
    for form in forms {
        let owner_paths = owner_paths_by_ref.get(&form.uuid);
        let owner_count = owner_paths.map_or(0, BTreeSet::len);
        if owner_count == 1 {
            continue;
        }

        let candidates = owner_paths
            .map(|paths| {
                paths
                    .iter()
                    .map(|path| path.to_string_lossy().replace('\\', "/"))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let candidates = if candidates.is_empty() {
            "none".to_string()
        } else {
            candidates.join(", ")
        };
        diagnostics.insert(
            format!("{}.0", form.uuid),
            format!(
                "form \"{}\" ({}) owner resolution expected exactly 1 owner, found {}; candidates: {}; fallback path: CommonForms/{}.xml",
                form.name,
                form.uuid,
                owner_count,
                candidates,
                sanitize_source_path_segment(&form.name)
            ),
        );
    }

    diagnostics
}

// Platform-level 1C markers for owned form lists in metadata blobs. These are
// not configuration object UUIDs and must not be replaced with DB-specific IDs.
#[allow(dead_code)]
pub(super) fn build_template_source_reference_index(
    rows: &[ConfigRow],
) -> BTreeMap<String, TemplateSourceReference> {
    let metadata_texts = build_metadata_text_rows(rows);
    build_template_source_reference_index_from_texts(rows, &metadata_texts)
}

pub(super) fn build_template_source_reference_index_from_texts(
    rows: &[ConfigRow],
    metadata_texts: &[MetadataTextRow],
) -> BTreeMap<String, TemplateSourceReference> {
    let rows_by_file_name = rows
        .iter()
        .map(|row| (row.file_name.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let mut templates = Vec::<MetadataHeader>::new();
    let mut owner_paths_by_ref = BTreeMap::<String, Vec<PathBuf>>::new();

    for row in metadata_texts {
        if is_template_metadata_text(&row.text, &row.file_name) {
            if let Some(header) = row.header.as_ref() {
                templates.push(header.clone());
            }
            continue;
        }
        let (Some(kind), Some(folder), Some(header)) =
            (row.kind.as_deref(), row.folder, row.header.as_ref())
        else {
            continue;
        };
        if !metadata_kind_can_own_templates(kind) {
            continue;
        }
        let owner_path = PathBuf::from(folder).join(sanitize_source_path_segment(&header.name));
        for reference in uuid_like_values(&row.text) {
            owner_paths_by_ref
                .entry(reference)
                .or_default()
                .push(owner_path.clone());
        }
    }

    let mut index = BTreeMap::new();
    for template in templates {
        let owner_matches = owner_paths_by_ref
            .get(&template.uuid)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let relative_path = if let [owner_path] = owner_matches {
            owner_path
                .join("Templates")
                .join(sanitize_source_path_segment(&template.name))
                .with_extension("xml")
        } else {
            PathBuf::from("CommonTemplates")
                .join(sanitize_source_path_segment(&template.name))
                .with_extension("xml")
        };
        let kind = if relative_path.starts_with("CommonTemplates") {
            "CommonTemplate"
        } else {
            "Template"
        };
        let body_id = format!("{}.0", template.uuid);
        let template_type = template_template_type_from_metadata(&template)
            .or_else(|| {
                rows_by_file_name
                    .get(body_id.as_str())
                    .and_then(|row| decode_hex(&row.binary_hex).ok())
                    .and_then(|bytes| infer_template_type_from_body(&bytes))
            })
            .unwrap_or("BinaryData");
        index.insert(
            template.uuid,
            TemplateSourceReference {
                relative_path,
                kind,
                template_type,
            },
        );
    }

    index
}

#[allow(dead_code)]
pub(super) fn build_subsystem_source_reference_index(
    rows: &[ConfigRow],
) -> BTreeMap<String, SubsystemSourceReference> {
    let metadata_texts = build_metadata_text_rows(rows);
    build_subsystem_source_reference_index_from_texts(&metadata_texts)
}

pub(super) fn build_subsystem_source_reference_index_from_texts(
    rows: &[MetadataTextRow],
) -> BTreeMap<String, SubsystemSourceReference> {
    let mut subsystems = BTreeMap::<String, (MetadataHeader, String)>::new();

    for row in rows {
        let Some(kind) = row.kind.as_deref() else {
            continue;
        };
        if kind != "Subsystem" {
            continue;
        }
        let Some(header) = row.header.as_ref() else {
            continue;
        };
        subsystems.insert(header.uuid.clone(), (header.clone(), row.text.clone()));
    }

    let subsystem_uuids = subsystems.keys().cloned().collect::<BTreeSet<_>>();
    let mut owners_by_child = BTreeMap::<String, Vec<String>>::new();
    for (owner_uuid, (_, owner_text)) in &subsystems {
        for reference in uuid_like_values(owner_text) {
            if reference != *owner_uuid && subsystem_uuids.contains(&reference) {
                owners_by_child
                    .entry(reference)
                    .or_default()
                    .push(owner_uuid.clone());
            }
        }
    }
    let mut parent_by_child = BTreeMap::<String, String>::new();
    for (child_uuid, owners) in owners_by_child {
        if let [owner_uuid] = owners.as_slice() {
            parent_by_child.insert(child_uuid, owner_uuid.clone());
        }
    }

    let mut memo = BTreeMap::<String, PathBuf>::new();
    for uuid in subsystems.keys() {
        let mut visiting = BTreeSet::<String>::new();
        let _ = resolve_subsystem_source_path(
            uuid,
            &subsystems,
            &parent_by_child,
            &mut memo,
            &mut visiting,
        );
    }

    memo.into_iter()
        .map(|(uuid, relative_path)| (uuid, SubsystemSourceReference { relative_path }))
        .collect()
}

pub(super) fn resolve_subsystem_source_path(
    uuid: &str,
    subsystems: &BTreeMap<String, (MetadataHeader, String)>,
    parent_by_child: &BTreeMap<String, String>,
    memo: &mut BTreeMap<String, PathBuf>,
    visiting: &mut BTreeSet<String>,
) -> Option<PathBuf> {
    if let Some(path) = memo.get(uuid) {
        return Some(path.clone());
    }
    if !visiting.insert(uuid.to_string()) {
        return None;
    }
    let (header, _) = subsystems.get(uuid)?;
    let name = sanitize_source_path_segment(&header.name);
    let relative_path = if let Some(parent_uuid) = parent_by_child.get(uuid) {
        resolve_subsystem_source_path(parent_uuid, subsystems, parent_by_child, memo, visiting)
            .map(|parent_path| {
                parent_path
                    .with_extension("")
                    .join("Subsystems")
                    .join(&name)
                    .with_extension("xml")
            })
            .unwrap_or_else(|| {
                PathBuf::from("Subsystems")
                    .join(&name)
                    .with_extension("xml")
            })
    } else {
        PathBuf::from("Subsystems")
            .join(&name)
            .with_extension("xml")
    };
    visiting.remove(uuid);
    memo.insert(uuid.to_string(), relative_path.clone());
    Some(relative_path)
}

pub(super) fn uuid_like_values(text: &str) -> BTreeSet<String> {
    uuid_like_values_in_text_order(text).into_iter().collect()
}

pub(super) fn uuid_like_values_in_text_order(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut values = Vec::new();
    let mut seen = BTreeSet::new();
    if bytes.len() < 36 {
        return values;
    }
    for start in 0..=bytes.len() - 36 {
        let value = &bytes[start..start + 36];
        if is_uuid_like_ascii(value) {
            let value = String::from_utf8_lossy(value).to_ascii_lowercase();
            if seen.insert(value.clone()) {
                values.push(value);
            }
        }
    }
    values
}

pub(super) fn is_uuid_like_ascii(value: &[u8]) -> bool {
    if value.len() != 36 {
        return false;
    }
    for (index, byte) in value.iter().copied().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            if byte != b'-' {
                return false;
            }
        } else if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

pub(super) fn infer_template_type_from_body(bytes: &[u8]) -> Option<&'static str> {
    let inflated = inflate_raw_deflate(bytes).ok()?;
    if inflated.starts_with(b"MOXCEL") {
        return Some("SpreadsheetDocument");
    }
    let Ok(text) = std::str::from_utf8(&inflated) else {
        return Some("BinaryData");
    };
    let text = text.trim_start_matches('\u{feff}').trim_start();
    let xml_text = text
        .starts_with("<?xml")
        .then_some(text)
        .or_else(|| text.find("<?xml").map(|index| &text[index..]));
    if xml_text.is_some_and(|xml| xml.contains("data-composition-system/appearance-template")) {
        Some("DataCompositionAppearanceTemplate")
    } else if xml_text.is_some_and(|xml| xml.contains("data-composition-system/schema")) {
        Some("DataCompositionSchema")
    } else if xml_text.is_some_and(|xml| xml.contains("8.3/xcf/scheme")) {
        Some("GraphicalSchema")
    } else if text.starts_with("<!DOCTYPE")
        || text.starts_with("<html")
        || text.starts_with("<?xml") && text.contains("<html")
    {
        Some("HTMLDocument")
    } else {
        Some("TextDocument")
    }
}

pub(super) fn template_template_type_from_metadata(
    header: &MetadataHeader,
) -> Option<&'static str> {
    template_type_from_code(header.template_type_code?)
}

pub(super) fn template_type_from_code(code: u32) -> Option<&'static str> {
    match code {
        0 => Some("SpreadsheetDocument"),
        1 => Some("BinaryData"),
        3 => Some("HTMLDocument"),
        4 => Some("TextDocument"),
        6 => Some("DataCompositionSchema"),
        7 => Some("DataCompositionAppearanceTemplate"),
        9 => Some("AddIn"),
        _ => None,
    }
}

pub(super) fn form_help_asset_paths(
    rows: &[ConfigRow],
    rows_by_file_name: &BTreeMap<&str, &ConfigRow>,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> BTreeMap<String, SourceAsset> {
    let file_names = rows
        .iter()
        .map(|row| row.file_name.as_str())
        .collect::<BTreeSet<_>>();
    let mut paths = BTreeMap::new();
    for (form_uuid, form_ref) in form_refs {
        let row_prefix = format!("{form_uuid}.");
        let mut form_dir = form_ref.relative_path.clone();
        form_dir.set_extension("");
        for body_id in file_names
            .iter()
            .filter(|file_name| file_name.starts_with(&row_prefix))
        {
            let module_body_id = format!("{form_uuid}.0");
            if *body_id == module_body_id.as_str() {
                continue;
            }
            if let Some(row) = rows_by_file_name.get(*body_id)
                && let Ok(bytes) = decode_hex(&row.binary_hex)
                && parse_help_blob_pages(&bytes).is_some()
            {
                paths.insert(
                    (*body_id).to_string(),
                    SourceAsset {
                        primary_path: form_dir.join("Ext").join("Help.xml"),
                        kind: SourceAssetKind::Help,
                    },
                );
            }
        }
    }
    paths
}

#[allow(dead_code)]
pub(super) fn parse_configuration_reference_blob(blob: &[u8]) -> Option<String> {
    let inflated = inflate_raw_deflate(blob).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let text = text.trim_start_matches('\u{feff}');
    parse_configuration_reference_text(text)
}

pub(super) fn parse_configuration_reference_text(text: &str) -> Option<String> {
    if !text.trim_start().starts_with("{2,") {
        return None;
    }
    let marker = "{1,0,";
    let marker_start = text.find(marker)?;
    let uuid_start = marker_start + marker.len();
    let uuid_end = uuid_start + 36;
    let uuid = text.get(uuid_start..uuid_end)?;
    if !is_uuid_text(uuid) || !is_metadata_header_marker(text, uuid_end) {
        return None;
    }
    parse_metadata_header_from_text(text, uuid).map(|header| header.name)
}

pub(super) fn extract_configuration_source_xml(
    text: &str,
    uuid: &str,
    object_refs: &BTreeMap<String, String>,
    source_version: InfobaseConfigSourceVersion,
) -> Option<String> {
    if !text.trim_start().starts_with("{2,") {
        return None;
    }
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "2" {
        return None;
    }
    let uuid_fields = split_1c_braced_fields(fields.get(1)?, 0)?;
    if uuid_fields.first()?.trim() != uuid {
        return None;
    }
    let marker = "{1,0,";
    let marker_start = text.find(marker)?;
    let header_uuid_start = marker_start + marker.len();
    let header_uuid_end = header_uuid_start + 36;
    let header_uuid = text.get(header_uuid_start..header_uuid_end)?;
    if !is_uuid_text(header_uuid) || !is_metadata_header_marker(text, header_uuid_end) {
        return None;
    }
    let mut header = parse_metadata_header_from_text(text, header_uuid)?;
    header.uuid = uuid.to_string();
    let properties =
        parse_configuration_properties_from_text(text, object_refs).unwrap_or_default();
    let child_objects = parse_configuration_child_objects(text, uuid, header_uuid);
    let mut xml = format_configuration_source_xml(&header, &properties, source_version);
    if !child_objects.is_empty() {
        let mut child_xml = String::new();
        for child_object in &child_objects {
            push_metadata_header_child_object_xml(
                &mut child_xml,
                child_object.tag,
                &child_object.header,
            );
        }
        insert_metadata_child_objects_xml(&mut xml, "Configuration", &child_xml);
    }
    Some(xml)
}

pub(super) fn parse_configuration_properties_from_text(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<ConfigurationProperties> {
    let fields = configuration_root_fields(text)?;
    Some(ConfigurationProperties {
        name_prefix: fields
            .get(2)
            .and_then(|field| parse_1c_quoted_string(field.trim())),
        configuration_extension_compatibility_mode: fields
            .get(26)
            .and_then(|field| configuration_compatibility_mode_xml(field.trim())),
        default_run_mode: fields
            .get(3)
            .and_then(|field| configuration_default_run_mode_xml(field.trim())),
        brief_information: parse_configuration_localized_property(&fields, 4),
        detailed_information: parse_configuration_localized_property(&fields, 5),
        copyright: parse_configuration_localized_property(&fields, 6),
        vendor_information_address: parse_configuration_localized_property(&fields, 7),
        configuration_information_address: parse_configuration_localized_property(&fields, 8),
        default_style: parse_configuration_root_reference(&fields, 9, object_refs, "Style."),
        default_language: parse_configuration_root_reference(&fields, 10, object_refs, "Language."),
        script_variant: fields
            .get(13)
            .and_then(|field| configuration_script_variant_xml(field.trim())),
        default_roles: fields
            .get(39)
            .map(|field| parse_configuration_default_roles(field, object_refs))
            .unwrap_or_default(),
        vendor: fields
            .get(14)
            .and_then(|field| parse_1c_quoted_string(field.trim())),
        version: fields
            .get(15)
            .and_then(|field| parse_1c_quoted_string(field.trim())),
        update_catalog_address: fields
            .get(16)
            .and_then(|field| parse_1c_quoted_string(field.trim())),
        common_settings_storage: parse_configuration_root_reference_slot(
            &fields,
            22,
            object_refs,
            "SettingsStorage.",
        ),
        reports_user_settings_storage: parse_configuration_root_reference_slot(
            &fields,
            23,
            object_refs,
            "SettingsStorage.",
        ),
        reports_variants_storage: parse_configuration_root_reference_slot(
            &fields,
            24,
            object_refs,
            "SettingsStorage.",
        ),
        form_data_settings_storage: parse_configuration_root_reference_slot(
            &fields,
            25,
            object_refs,
            "SettingsStorage.",
        ),
        compatibility_mode: fields
            .get(43)
            .and_then(|field| configuration_compatibility_mode_xml(field.trim())),
    })
}

pub(super) fn parse_configuration_default_roles(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Vec<String> {
    let Some(fields) = split_1c_braced_fields(field, 0) else {
        return Vec::new();
    };
    let Some(count) = fields
        .get(1)
        .and_then(|field| field.trim().parse::<usize>().ok())
    else {
        return Vec::new();
    };

    fields
        .iter()
        .skip(2)
        .take(count)
        .filter_map(|field| parse_design_time_reference(field, object_refs))
        .filter(|reference| reference.starts_with("Role."))
        .collect()
}

pub(super) fn configuration_root_fields(text: &str) -> Option<Vec<&str>> {
    let start = text.find("{68,")?;
    split_1c_braced_fields(text, start)
}

pub(super) fn parse_configuration_localized_property(
    fields: &[&str],
    index: usize,
) -> Vec<(String, String)> {
    fields
        .get(index)
        .map(|field| parse_1c_synonyms(field))
        .unwrap_or_default()
}

pub(super) fn parse_configuration_root_reference(
    fields: &[&str],
    index: usize,
    object_refs: &BTreeMap<String, String>,
    expected_prefix: &str,
) -> Option<String> {
    parse_configuration_root_reference_slot(fields, index, object_refs, expected_prefix)?.value
}

pub(super) fn parse_configuration_root_reference_slot(
    fields: &[&str],
    index: usize,
    object_refs: &BTreeMap<String, String>,
    expected_prefix: &str,
) -> Option<ConfigurationRootReference> {
    let field = fields.get(index)?.trim();
    if field == "00000000-0000-0000-0000-000000000000" {
        return Some(ConfigurationRootReference { value: None });
    }
    let uuid = parse_non_zero_uuid(field)?;
    let reference = object_refs.get(&uuid)?;
    reference
        .starts_with(expected_prefix)
        .then(|| ConfigurationRootReference {
            value: Some(reference.clone()),
        })
}

pub(super) fn configuration_default_run_mode_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("OrdinaryApplication"),
        "1" => Some("ManagedApplication"),
        _ => None,
    }
}

pub(super) fn configuration_script_variant_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("Russian"),
        "1" => Some("English"),
        _ => None,
    }
}

pub(super) fn configuration_compatibility_mode_xml(value: &str) -> Option<String> {
    if let Some(value) = parse_1c_quoted_string(value) {
        return if value.is_empty() { None } else { Some(value) };
    }
    let version = value.parse::<u32>().ok()?;
    if version < 80000 {
        return None;
    }
    Some(format!(
        "Version{}_{}_{}",
        version / 10000,
        (version / 100) % 100,
        version % 100
    ))
}

struct ConfigurationChildObject {
    tag: &'static str,
    header: MetadataHeader,
}

fn parse_configuration_child_objects(
    text: &str,
    uuid: &str,
    header_uuid: &str,
) -> Vec<ConfigurationChildObject> {
    nested_headers_with_offsets_from_text(text, uuid, |_| true)
        .into_iter()
        .filter_map(|(header, marker_start)| {
            if header.uuid == header_uuid {
                return None;
            }
            configuration_child_object_tag(text, marker_start, &header.uuid)
                .map(|tag| ConfigurationChildObject { tag, header })
        })
        .collect()
}

pub(super) fn configuration_child_object_tag(
    text: &str,
    marker_start: usize,
    child_uuid: &str,
) -> Option<&'static str> {
    let mut search_end = marker_start;
    let mut tag = None;
    while let Some(start) = text[..search_end].rfind('{') {
        search_end = start;
        let Some(end) = scan_1c_braced_value(text, start) else {
            continue;
        };
        if marker_start >= end {
            continue;
        }
        let object_text = &text[start..end];
        if object_text.contains("{68,") {
            continue;
        }
        let Some(fields) = split_1c_braced_fields(object_text, 0) else {
            continue;
        };
        let Some(code) = fields
            .first()
            .and_then(|field| field.trim().parse::<u32>().ok())
        else {
            continue;
        };
        if let Some((kind, _)) = metadata_source_for_object_text(code, object_text, child_uuid) {
            if matches!(kind, "CommonForm" | "CommonTemplate") {
                return None;
            }
            tag = Some(kind);
        }
    }
    tag
}

#[allow(dead_code)]
pub(super) fn build_command_interface_reference_index(
    rows: &[ConfigRow],
) -> BTreeMap<String, String> {
    let metadata_texts = build_metadata_text_rows(rows);
    build_command_interface_reference_index_from_texts(&metadata_texts)
}

pub(super) fn build_command_interface_reference_index_from_texts(
    rows: &[MetadataTextRow],
) -> BTreeMap<String, String> {
    let row_entries = parallel::install(|| {
        rows.par_iter()
            .enumerate()
            .map(|(index, row)| (index, command_interface_reference_entries_from_text(row)))
            .collect::<Vec<_>>()
    })
    .unwrap_or_else(|_| {
        rows.iter()
            .enumerate()
            .map(|(index, row)| (index, command_interface_reference_entries_from_text(row)))
            .collect::<Vec<_>>()
    });
    let mut index = BTreeMap::new();
    for (_, entries) in row_entries {
        for (uuid, reference) in entries {
            index.insert(uuid, reference);
        }
    }
    index
}

pub(super) fn command_interface_reference_entries_from_text(
    row: &MetadataTextRow,
) -> Vec<(String, String)> {
    let (Some(kind), Some(header)) = (row.kind.as_deref(), row.header.as_ref()) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    if kind == "CommonCommand" {
        entries.push((
            row.file_name.clone(),
            format!("CommonCommand.{}", header.name),
        ));
    }
    entries.extend(
        nested_command_headers_from_text(&row.text, &row.file_name)
            .into_iter()
            .map(|command| {
                (
                    command.uuid,
                    format!("{}.{}.Command.{}", kind, header.name, command.name),
                )
            }),
    );
    entries
}

#[allow(dead_code)]
pub(super) fn parse_metadata_command_reference_blob(
    blob: &[u8],
    uuid: &str,
) -> Option<(String, MetadataHeader, String)> {
    let inflated = inflate_raw_deflate(blob).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let text = text.trim_start_matches('\u{feff}').to_string();
    let object_code = parse_metadata_object_code(&text)?;
    let kind = if object_code == 12 {
        "CommonModule"
    } else {
        metadata_source_for_text(object_code, &text, uuid)?.0
    };
    let header = parse_metadata_header_from_text(&text, uuid)?;
    Some((kind.to_string(), header, text))
}
