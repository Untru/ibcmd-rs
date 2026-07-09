use super::*;

pub(super) fn is_form_metadata_text(text: &str, uuid: &str) -> bool {
    matches!(parse_metadata_object_code(text), Some(0 | 1 | 4 | 13))
        && (contains_wrapped_metadata_object_code(text, 13, uuid)
            || contains_wrapped_metadata_object_code(text, 14, uuid))
}
pub(super) const FORM_LIST_MARKERS: &[&str] = &[
    "fdf816d2-1ead-11d5-b975-0050bae0a95d",
    "fb880e93-47d7-4127-9357-a20e69c17545",
    "13134204-f60b-11d5-a3c7-0050bae0a776",
    "87c509ab-3d38-4d67-b379-aca796298578",
    "b64d9a44-1642-11d6-a3c7-0050bae0a776",
    "d5b0e5ed-256d-401c-9c36-f630cafd8a62",
    "a3b368c0-29e2-11d6-a3c7-0050bae0a776",
    "eb2b78a8-40a6-4b7e-b1b3-6ca9966cbc94",
    "3f7a8120-b71a-4265-98bf-4d9bc09b7719",
    "b8533c0c-2342-4db3-91a2-c2b08cbf6b23",
    "ec81ad10-ca07-11d5-b9a5-0050bae0a95d",
    "33f2e54b-37ce-4a7a-a569-b648d7aa4634",
    "3f58cbfb-4172-4e54-be49-561a579bb38b",
];
pub(super) fn owned_form_uuid_values(text: &str) -> Option<BTreeSet<String>> {
    owned_form_uuid_values_in_text_order(text).map(|values| values.into_iter().collect())
}
pub(super) fn owned_form_uuid_values_in_text_order(text: &str) -> Option<Vec<String>> {
    let mut refs = Vec::new();
    let mut seen = BTreeSet::new();
    let mut found_marker = false;
    for marker in FORM_LIST_MARKERS {
        let mut offset = 0usize;
        while let Some(relative_index) = text[offset..].find(marker) {
            let marker_index = offset + relative_index;
            found_marker = true;
            offset = marker_index + marker.len();

            let Some(block_start) = text[..marker_index].rfind('{') else {
                continue;
            };
            let Some(block_end) = scan_1c_braced_value(text, block_start) else {
                continue;
            };
            let Some(fields) = split_1c_braced_fields(&text[block_start..block_end], 0) else {
                continue;
            };
            if fields.first().map(|field| field.trim()) != Some(*marker) {
                continue;
            }
            let Some(count) = fields
                .get(1)
                .and_then(|field| field.trim().parse::<usize>().ok())
            else {
                continue;
            };
            for field in fields.iter().skip(2).take(count) {
                let value = field.trim().trim_matches('"').to_ascii_lowercase();
                if is_uuid_like_ascii(value.as_bytes()) && seen.insert(value.clone()) {
                    refs.push(value);
                }
            }
        }
    }
    found_marker.then_some(refs)
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
    if let Some(form_uuids) = owned_form_uuid_values_in_text_order(text) {
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
pub(super) fn format_form_source_xml(
    kind: &str,
    header: &MetadataHeader,
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
            escape_xml_text(&header.comment)
        ));
    }
    xml.push_str(
        "\t\t\t<FormType>Managed</FormType>\r\n\
\t\t\t<IncludeHelpInContents>false</IncludeHelpInContents>\r\n\
\t\t\t<UsePurposes>\r\n\
\t\t\t\t<v8:Value xsi:type=\"app:ApplicationUsePurpose\">PlatformApplication</v8:Value>\r\n\
\t\t\t\t<v8:Value xsi:type=\"app:ApplicationUsePurpose\">MobilePlatformApplication</v8:Value>\r\n\
\t\t\t</UsePurposes>\r\n",
    );
    if source_version == InfobaseConfigSourceVersion::V2_21 {
        xml.push_str(
            "\t\t\t<UseInInterfaceCompatibilityMode>Any</UseInInterfaceCompatibilityMode>\r\n",
        );
    }
    if kind == "CommonForm" {
        xml.push_str(
            "\t\t\t<UseStandardCommands>false</UseStandardCommands>\r\n\
\t\t\t<ExtendedPresentation/>\r\n\
\t\t\t<Explanation/>\r\n",
        );
    } else if kind == "Form" {
        xml.push_str("\t\t\t<ExtendedPresentation/>\r\n");
    }
    xml.push_str(&format!(
        "\t\t</Properties>\r\n\
\t</{kind}>\r\n\
</MetaDataObject>"
    ));
    xml
}
