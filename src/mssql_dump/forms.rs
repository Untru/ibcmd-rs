use super::*;

pub(super) fn is_form_metadata_text(text: &str, uuid: &str) -> bool {
    matches!(parse_metadata_object_code(text), Some(0 | 1 | 4 | 13))
        && (contains_wrapped_metadata_object_code(text, 13, uuid)
            || contains_wrapped_metadata_object_code(text, 14, uuid))
}

pub(super) fn owned_form_uuid_values_matching(
    text: &str,
    allowed_refs: &BTreeSet<String>,
) -> Option<BTreeSet<String>> {
    owned_form_uuid_values_matching_in_text_order(text, allowed_refs)
        .map(|values| values.into_iter().collect())
}

pub(super) fn owned_form_uuid_values_matching_in_text_order(
    text: &str,
    allowed_refs: &BTreeSet<String>,
) -> Option<Vec<String>> {
    counted_uuid_values_in_text_order(text, allowed_refs)
}

fn counted_uuid_values_in_text_order(
    text: &str,
    allowed_refs: &BTreeSet<String>,
) -> Option<Vec<String>> {
    let mut refs = Vec::new();
    let mut seen = BTreeSet::new();
    collect_counted_uuid_block_refs(text, allowed_refs, &mut refs, &mut seen).then_some(refs)
}

fn collect_counted_uuid_block_refs(
    text: &str,
    allowed_refs: &BTreeSet<String>,
    refs: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
) -> bool {
    let mut offset = 0usize;
    let mut found_block = false;
    while let Some(relative_index) = text[offset..].find('{') {
        let block_start = offset + relative_index;
        offset = block_start + 1;

        let marker_start = block_start + 1;
        let marker_end = marker_start + 36;
        let Some(marker) = text.get(marker_start..marker_end) else {
            continue;
        };
        if !is_uuid_like_ascii(marker.as_bytes()) {
            continue;
        }
        if text.as_bytes().get(marker_end).copied() != Some(b',') {
            continue;
        }

        let Some(block_end) = scan_1c_braced_value(text, block_start) else {
            continue;
        };
        let Some(fields) = split_1c_braced_fields(&text[block_start..block_end], 0) else {
            continue;
        };
        if fields.first().map(|field| field.trim()) != Some(marker) {
            continue;
        }
        let Some(count) = fields
            .get(1)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        let mut block_ref_count = 0usize;
        for field in fields.iter().skip(2).take(count) {
            let value = field.trim().trim_matches('"').to_ascii_lowercase();
            if is_uuid_like_ascii(value.as_bytes()) && allowed_refs.contains(&value) {
                block_ref_count += 1;
                if seen.insert(value.clone()) {
                    refs.push(value);
                }
            }
        }
        found_block |= block_ref_count > 0;
    }

    found_block
}
pub(super) fn simple_metadata_form_template_child_objects_xml(
    kind: &str,
    owner_folder: &str,
    owner_name: &str,
    text: &str,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
) -> String {
    if !metadata_kind_uses_simple_form_template_child_objects(kind) {
        return String::new();
    }

    let mut xml = String::new();
    for form in owned_metadata_form_names_in_text_order(text, owner_folder, owner_name, form_refs) {
        xml.push_str(&format!(
            "\t\t\t<Form>{}</Form>\r\n",
            escape_xml_element_text(&form)
        ));
    }
    for template in
        owned_metadata_template_names_in_text_order(text, owner_folder, owner_name, template_refs)
    {
        xml.push_str(&format!(
            "\t\t\t<Template>{}</Template>\r\n",
            escape_xml_element_text(&template)
        ));
    }
    xml
}
pub(super) fn metadata_kind_uses_simple_form_template_child_objects(kind: &str) -> bool {
    matches!(
        kind,
        "AccumulationRegister"
            | "AccountingRegister"
            | "BusinessProcess"
            | "CalculationRegister"
            | "DocumentJournal"
            | "ExchangePlan"
            | "InformationRegister"
            | "SettingsStorage"
            | "Task"
    )
}
pub(super) fn owned_metadata_form_names_in_text_order(
    text: &str,
    owner_folder: &str,
    owner_name: &str,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = BTreeSet::new();
    let known_form_uuids = form_refs.keys().cloned().collect::<BTreeSet<_>>();
    if let Some(form_uuids) = owned_form_uuid_values_matching_in_text_order(text, &known_form_uuids)
    {
        for uuid in form_uuids {
            let Some(form_ref) = form_refs.get(&uuid) else {
                continue;
            };
            if form_ref.kind != "Form"
                || !is_owned_metadata_child_path(
                    &form_ref.relative_path,
                    owner_folder,
                    owner_name,
                    "Forms",
                )
            {
                continue;
            }
            let Some(name) = source_path_file_stem(&form_ref.relative_path) else {
                continue;
            };
            if seen.insert(name.clone()) {
                names.push(name);
            }
        }
    }

    let mut path_names = form_refs
        .values()
        .filter(|form_ref| {
            form_ref.kind == "Form"
                && is_owned_metadata_child_path(
                    &form_ref.relative_path,
                    owner_folder,
                    owner_name,
                    "Forms",
                )
        })
        .filter_map(|form_ref| {
            source_path_file_stem(&form_ref.relative_path)
                .map(|name| (form_ref.relative_path.clone(), name))
        })
        .collect::<Vec<_>>();
    path_names.sort_by(|(left_path, _), (right_path, _)| left_path.cmp(right_path));
    for (_, name) in path_names {
        if seen.insert(name.clone()) {
            names.push(name);
        }
    }

    names
}
pub(super) fn form_source_reference_name(form_ref: &FormSourceReference) -> Option<String> {
    let parts = form_ref
        .relative_path
        .iter()
        .filter_map(|part| part.to_str())
        .collect::<Vec<_>>();
    if parts.len() == 2 && parts.first() == Some(&"CommonForms") {
        let form_name = Path::new(parts[1]).file_stem()?.to_str()?;
        return Some(format!("CommonForm.{form_name}"));
    }
    if parts.len() == 4 && parts.get(2) == Some(&"Forms") {
        let owner_kind = metadata_kind_for_source_folder(parts[0])?;
        let owner_name = parts[1];
        let form_name = Path::new(parts[3]).file_stem()?.to_str()?;
        return Some(format!("{owner_kind}.{owner_name}.Form.{form_name}"));
    }
    None
}
pub(super) fn metadata_kind_for_source_folder(folder: &str) -> Option<&'static str> {
    match folder {
        "Catalogs" => Some("Catalog"),
        "Documents" => Some("Document"),
        "DocumentJournals" => Some("DocumentJournal"),
        "Enums" => Some("Enum"),
        "Reports" => Some("Report"),
        "DataProcessors" => Some("DataProcessor"),
        "InformationRegisters" => Some("InformationRegister"),
        "AccumulationRegisters" => Some("AccumulationRegister"),
        "AccountingRegisters" => Some("AccountingRegister"),
        "CalculationRegisters" => Some("CalculationRegister"),
        "ChartsOfAccounts" => Some("ChartOfAccounts"),
        "ChartsOfCharacteristicTypes" => Some("ChartOfCharacteristicTypes"),
        "ChartsOfCalculationTypes" => Some("ChartOfCalculationTypes"),
        "BusinessProcesses" => Some("BusinessProcess"),
        "Tasks" => Some("Task"),
        "ExchangePlans" => Some("ExchangePlan"),
        _ => None,
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormMetadataProperties {
    pub(super) include_help_in_contents: bool,
    pub(super) use_purposes: Vec<&'static str>,
    pub(super) use_standard_commands: bool,
    pub(super) extended_presentation: Vec<(String, String)>,
    pub(super) explanation: Vec<(String, String)>,
}

impl Default for FormMetadataProperties {
    fn default() -> Self {
        Self {
            include_help_in_contents: false,
            use_purposes: default_form_use_purposes(),
            use_standard_commands: false,
            extended_presentation: Vec::new(),
            explanation: Vec::new(),
        }
    }
}

pub(super) fn parse_form_metadata_properties_from_text(
    text: &str,
    kind: &str,
    uuid: &str,
) -> FormMetadataProperties {
    let mut properties = FormMetadataProperties::default();
    if let Some(form_fields) = form_metadata_fields_from_text(text, uuid) {
        properties.include_help_in_contents =
            parse_1c_bool_field(form_fields.get(2).copied()).unwrap_or(false);
        properties.use_purposes = parse_form_use_purposes(form_fields.get(4).copied())
            .unwrap_or_else(default_form_use_purposes);
    }

    if kind == "CommonForm"
        && let Some(common_form_fields) = metadata_object_fields(text)
        && common_form_fields.first().map(|field| field.trim()) == Some("4")
    {
        properties.extended_presentation =
            parse_1c_synonyms(common_form_fields.get(2).copied().unwrap_or("{0}"));
        properties.explanation =
            parse_1c_synonyms(common_form_fields.get(3).copied().unwrap_or("{0}"));
        properties.use_standard_commands =
            parse_1c_bool_field(common_form_fields.get(4).copied()).unwrap_or(false);
    }

    properties
}

fn form_metadata_fields_from_text<'a>(text: &'a str, uuid: &str) -> Option<Vec<&'a str>> {
    let mut offset = 0usize;
    while let Some(relative_index) = text[offset..].find('{') {
        let block_start = offset + relative_index;
        offset = block_start + 1;
        let Some(block_end) = scan_1c_braced_value(text, block_start) else {
            continue;
        };
        let Some(fields) = split_1c_braced_fields(&text[block_start..block_end], 0) else {
            continue;
        };
        if matches!(fields.first().map(|field| field.trim()), Some("13" | "14"))
            && metadata_header_field_index(&fields, uuid).is_some()
        {
            return Some(fields);
        }
    }
    None
}

fn parse_form_use_purposes(value: Option<&str>) -> Option<Vec<&'static str>> {
    let fields = split_1c_braced_fields(value?, 0)?;
    let mut purposes = Vec::new();
    for field in fields.iter().skip(1) {
        let item_fields = split_1c_braced_fields(field, 0)?;
        let purpose = match parse_1c_u32_field(item_fields.last().copied())? {
            1 => "PlatformApplication",
            2 => "MobilePlatformApplication",
            _ => continue,
        };
        purposes.push(purpose);
    }
    (!purposes.is_empty()).then_some(purposes)
}

fn default_form_use_purposes() -> Vec<&'static str> {
    vec!["PlatformApplication", "MobilePlatformApplication"]
}

pub(super) fn format_form_source_xml(
    kind: &str,
    header: &MetadataHeader,
    properties: &FormMetadataProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let palette_namespace = if source_version == InfobaseConfigSourceVersion::V2_21 {
        " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\""
    } else {
        ""
    };
    let mut xml = format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:app=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:cmi=\"http://v8.1c.ru/8.2/managed-application/cmi\" xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" xmlns:lf=\"http://v8.1c.ru/8.2/managed-application/logform\"{palette_namespace} xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xen=\"http://v8.1c.ru/8.3/xcf/enums\" xmlns:xpr=\"http://v8.1c.ru/8.3/xcf/predef\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"{version}\">\r\n\
\t<{kind} uuid=\"{uuid}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>{name}</Name>\r\n",
        palette_namespace = palette_namespace,
        version = source_version.as_str(),
        uuid = escape_xml_text(&header.uuid),
        name = escape_xml_text(&header.name),
    );
    if header.synonyms.is_empty() {
        xml.push_str("\t\t\t<Synonym/>\r\n");
    } else {
        xml.push_str("\t\t\t<Synonym>\r\n");
        for (lang, content) in &header.synonyms {
            xml.push_str("\t\t\t\t<v8:item>\r\n");
            xml.push_str(&format!(
                "\t\t\t\t\t<v8:lang>{}</v8:lang>\r\n",
                escape_xml_element_text(lang)
            ));
            xml.push_str(&format!(
                "\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
                escape_xml_element_text(content)
            ));
            xml.push_str("\t\t\t\t</v8:item>\r\n");
        }
        xml.push_str("\t\t\t</Synonym>\r\n");
    }
    if header.comment.is_empty() {
        xml.push_str("\t\t\t<Comment/>\r\n");
    } else {
        xml.push_str(&format!(
            "\t\t\t<Comment>{}</Comment>\r\n",
            escape_xml_element_text(&header.comment)
        ));
    }
    xml.push_str(&format!(
        "\t\t\t<FormType>Managed</FormType>\r\n\
\t\t\t<IncludeHelpInContents>{}</IncludeHelpInContents>\r\n\
\t\t\t<UsePurposes>\r\n",
        xml_bool(properties.include_help_in_contents)
    ));
    for purpose in &properties.use_purposes {
        xml.push_str(&format!(
            "\t\t\t\t<v8:Value xsi:type=\"app:ApplicationUsePurpose\">{}</v8:Value>\r\n",
            escape_xml_element_text(purpose)
        ));
    }
    xml.push_str("\t\t\t</UsePurposes>\r\n");
    if source_version == InfobaseConfigSourceVersion::V2_21 {
        xml.push_str(
            "\t\t\t<UseInInterfaceCompatibilityMode>Any</UseInInterfaceCompatibilityMode>\r\n",
        );
    }
    if kind == "CommonForm" {
        xml.push_str(&format!(
            "\t\t\t<UseStandardCommands>{}</UseStandardCommands>\r\n",
            xml_bool(properties.use_standard_commands)
        ));
        push_localized_property(
            &mut xml,
            "\t\t\t",
            "ExtendedPresentation",
            &properties.extended_presentation,
        );
        push_localized_property(&mut xml, "\t\t\t", "Explanation", &properties.explanation);
    } else if kind == "Form" && !properties.extended_presentation.is_empty() {
        push_localized_property(
            &mut xml,
            "\t\t\t",
            "ExtendedPresentation",
            &properties.extended_presentation,
        );
    }
    xml.push_str(&format!(
        "\t\t</Properties>\r\n\
\t</{kind}>\r\n\
</MetaDataObject>"
    ));
    xml
}
