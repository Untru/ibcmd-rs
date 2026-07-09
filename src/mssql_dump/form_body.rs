use super::*;

#[allow(dead_code)]
pub(crate) fn extract_form_body_xml(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let body = parse_form_body_blob(bytes).ok()?;
    extract_form_body_xml_from_body(&body, object_refs, object_refs)
}

pub(super) fn extract_form_body_xml_from_body(
    body: &ParsedFormBodyBlob,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    extract_form_body_xml_from_body_timed(body, type_index, object_refs, None)
}

pub(super) fn extract_form_body_xml_from_body_timed(
    body: &ParsedFormBodyBlob,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    mut timings: Option<&mut MssqlDumpTimingReport>,
) -> Option<String> {
    let started = Instant::now();
    let form_fields = split_1c_braced_fields(&body.layout, 0)?;
    if let Some(timings) = timings.as_deref_mut() {
        timings.source_asset_form_split_cpu_ms += elapsed_ms(started);
    }

    let started = Instant::now();
    let mut properties = extract_form_body_properties(&form_fields);
    if let Some(timings) = timings.as_deref_mut() {
        timings.source_asset_form_properties_cpu_ms += elapsed_ms(started);
    }

    let started = Instant::now();
    let events = extract_form_body_events(&form_fields);
    if let Some(timings) = timings.as_deref_mut() {
        timings.source_asset_form_events_cpu_ms += elapsed_ms(started);
    }

    let started = Instant::now();
    let mut attributes = extract_form_body_attributes(&body.trailing, type_index, object_refs);
    let attribute_save_field_bindings =
        extract_form_body_attribute_save_field_bindings(&body.trailing, type_index, object_refs);
    if let Some(timings) = timings.as_deref_mut() {
        timings.source_asset_form_attributes_cpu_ms += elapsed_ms(started);
    }
    let attributes_section = extract_form_body_attributes_section(&body.trailing, object_refs);

    let started = Instant::now();
    properties.report_result = extract_form_report_attribute_ref(&form_fields, "5", &attributes);
    properties.details_data = extract_form_report_attribute_ref(&form_fields, "6", &attributes);
    if let Some(timings) = timings.as_deref_mut() {
        timings.source_asset_form_properties_cpu_ms += elapsed_ms(started);
    }

    let started = Instant::now();
    let parameters = extract_form_body_parameters(&body.trailing, type_index);
    if let Some(timings) = timings.as_deref_mut() {
        timings.source_asset_form_parameters_cpu_ms += elapsed_ms(started);
    }

    let started = Instant::now();
    let commands = extract_form_body_commands(&body.trailing, object_refs);
    if let Some(timings) = timings.as_deref_mut() {
        timings.source_asset_form_commands_cpu_ms += elapsed_ms(started);
    }

    let started = Instant::now();
    let auto_command_bar = extract_form_auto_command_bar(&form_fields, &commands, object_refs);
    if let Some(timings) = timings.as_deref_mut() {
        timings.source_asset_form_auto_command_bar_cpu_ms += elapsed_ms(started);
    }

    let started = Instant::now();
    let child_item_indexes = collect_form_child_item_indexes(&form_fields, &attributes);
    apply_form_body_attribute_additional_columns(
        &mut attributes,
        &body.trailing,
        type_index,
        object_refs,
        &child_item_indexes,
    );
    apply_form_attribute_save_field_bindings(
        &mut attributes,
        &attribute_save_field_bindings,
        &child_item_indexes.data_path_by_binding_key,
    );
    let child_items = extract_form_child_items(
        &form_fields,
        &attributes,
        &commands,
        object_refs,
        &child_item_indexes,
    );
    if let Some(timings) = timings.as_deref_mut() {
        timings.source_asset_form_child_items_cpu_ms += elapsed_ms(started);
    }

    let started = Instant::now();
    let command_interface = extract_form_command_interface(&body.trailing, object_refs);
    if let Some(timings) = timings.as_deref_mut() {
        timings.source_asset_form_command_interface_cpu_ms += elapsed_ms(started);
    }

    let started = Instant::now();
    let xml = format_form_body_xml(
        &properties,
        auto_command_bar.as_ref(),
        &events,
        &child_items,
        &attributes,
        &attributes_section,
        &parameters,
        &commands,
        &command_interface,
    );
    if let Some(timings) = timings.as_deref_mut() {
        timings.source_asset_form_format_cpu_ms += elapsed_ms(started);
    }

    Some(xml)
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(super) struct FormBodyProperties {
    pub(super) title: Vec<(String, String)>,
    pub(super) width: Option<String>,
    pub(super) height: Option<String>,
    pub(super) window_opening_mode: Option<&'static str>,
    pub(super) enter_key_behavior: Option<&'static str>,
    pub(super) save_window_settings: Option<bool>,
    pub(super) auto_title: Option<bool>,
    pub(super) auto_url: Option<bool>,
    pub(super) save_data_in_settings: Option<&'static str>,
    pub(super) auto_save_data_in_settings: Option<&'static str>,
    pub(super) group: Option<&'static str>,
    pub(super) scaling_mode: Option<&'static str>,
    pub(super) auto_time: Option<&'static str>,
    pub(super) use_posting_mode: Option<&'static str>,
    pub(super) repost_on_write: Option<bool>,
    pub(super) auto_fill_check: Option<bool>,
    pub(super) command_set_excluded_commands: Vec<&'static str>,
    pub(super) use_for_folders_and_items: Option<&'static str>,
    pub(super) customizable: Option<bool>,
    pub(super) command_bar_location: Option<&'static str>,
    pub(super) vertical_scroll: Option<&'static str>,
    pub(super) horizontal_align: Option<&'static str>,
    pub(super) conversations_representation: Option<&'static str>,
    pub(super) show_title: Option<bool>,
    pub(super) show_command_bar: Option<bool>,
    pub(super) show_close_button: Option<bool>,
    pub(super) report_result: Option<String>,
    pub(super) details_data: Option<String>,
    pub(super) report_form_type: Option<&'static str>,
    pub(super) auto_show_state: Option<&'static str>,
    pub(super) report_result_view_mode: Option<&'static str>,
    pub(super) view_mode_application_on_set_report_result: Option<&'static str>,
}

pub(super) const FORM_USE_FOR_FOLDERS_AND_ITEMS_UUID: &str = "59ef2b80-c86b-11d5-a3c1-0050bae0a776";
pub(super) const FORM_STANDARD_PERIOD_UUID: &str = "2fdc88ec-7c9b-43cd-8ba5-873f043bdd88";
pub(super) const FORM_AUTO_TIME_UUID: &str = "adeb08a0-415c-11d6-b9d1-0050bae0a95d";
pub(super) const FORM_USE_POSTING_MODE_UUID: &str = "20d89b09-bd04-4304-a8c7-4d07fac6338a";
pub(super) const FORM_CONVERSATIONS_REPRESENTATION_UUID: &str =
    "f26c3706-a6ca-45cb-869a-e6ad38cd5f78";
pub(super) const FORM_REPORT_FORM_TYPE_UUID: &str = "acbc2eeb-2efb-48e4-b78a-661fd09fcf80";
pub(super) const FORM_REPORT_RESULT_VIEW_MODE_UUID: &str = "b9311bea-b26b-4ae0-8b5d-7b64048fd2df";
pub(super) const FORM_VIEW_MODE_APPLICATION_ON_SET_REPORT_RESULT_UUID: &str =
    "874260df-7e23-4f02-9e10-5794914b5adf";
pub(super) const FORM_REPORT_ATTRIBUTE_REF_UUID: &str = "11cfd3e0-86f8-4480-aaa5-dc6a6ccac689";
pub(super) const FORM_UPDATE_ON_DATA_CHANGE_UUID: &str = "eac7bfa0-10b4-4369-996c-d258871ad519";
pub(super) const FORM_COMMAND_CHANGE_UUID: &str = "342c531d-dc73-458a-8ac4-6a746916a33b";
pub(super) const FORM_COMMAND_COPY_UUID: &str = "4f834c38-add1-45e4-a9f3-cefe3efac5c9";
pub(super) const FORM_COMMAND_CREATE_UUID: &str = "6886601d-276c-4d3f-af0a-05c586025608";
pub(super) const FORM_COMMAND_CUSTOMIZE_FORM_UUID: &str = "198ea630-fda2-4cda-8a23-f999f4c67ee6";

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct FormBodyEvent {
    pub(super) name: String,
    pub(super) handler: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormAutoCommandBar {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) horizontal_align: Option<&'static str>,
    pub(super) autofill: Option<bool>,
    pub(super) child_items: Vec<FormChildItem>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormAttribute {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) title: Vec<(String, String)>,
    pub(super) value_types: Vec<ConstantValueType>,
    pub(super) explicit_empty_type: bool,
    pub(super) columns: Vec<FormAttributeColumn>,
    pub(super) additional_columns: Vec<FormAttributeAdditionalColumns>,
    pub(super) main_attribute: bool,
    pub(super) saved_data: bool,
    pub(super) fill_check: Option<&'static str>,
    pub(super) save_fields: Vec<String>,
    pub(super) use_always: Vec<String>,
    pub(super) functional_options: Vec<String>,
    pub(super) settings: Option<FormDynamicListSettings>,
    pub(super) spreadsheet_document_settings: Option<String>,
    pub(super) type_description_settings: Option<Vec<ConstantValueType>>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormAttributeColumn {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) title: Vec<(String, String)>,
    pub(super) value_types: Vec<ConstantValueType>,
    pub(super) explicit_empty_type: bool,
    pub(super) functional_options: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormAttributeAdditionalColumns {
    pub(super) table: String,
    pub(super) columns: Vec<FormAttributeColumn>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormParameter {
    pub(super) name: String,
    pub(super) value_types: Vec<ConstantValueType>,
    pub(super) explicit_empty_type: bool,
    pub(super) key_parameter: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormDynamicListSettings {
    pub(super) manual_query: bool,
    pub(super) auto_save_user_settings: bool,
    pub(super) dynamic_data_read: bool,
    pub(super) dynamic_data_read_explicit: bool,
    pub(super) query_text: Option<String>,
    pub(super) main_table: Option<String>,
    pub(super) explicit_fields: Vec<FormDynamicListField>,
    pub(super) fields: Vec<FormDynamicListField>,
    pub(super) server_state_xml: Option<String>,
    pub(super) list_settings: FormListSettings,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormDynamicListField {
    pub(super) item_id: Option<String>,
    pub(super) data_path: String,
    pub(super) field: String,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(super) struct FormAttributesSection {
    pub(super) conditional_appearance_xml: Option<String>,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(super) struct FormListSettings {
    pub(super) filter: Option<FormListSettingsStandardSection>,
    pub(super) order: Option<FormListSettingsOrder>,
    pub(super) conditional_appearance: Option<FormListSettingsStandardSection>,
    pub(super) items_view_mode: Option<String>,
    pub(super) items_user_setting_id: Option<String>,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(super) struct FormListSettingsStandardSection {
    pub(super) view_mode: Option<String>,
    pub(super) user_setting_id: Option<String>,
    pub(super) raw_xml: Option<String>,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(super) struct FormListSettingsOrder {
    pub(super) items: Vec<FormListSettingsOrderItem>,
    pub(super) view_mode: Option<String>,
    pub(super) user_setting_id: Option<String>,
    pub(super) raw_xml: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormListSettingsOrderItem {
    pub(super) field: String,
    pub(super) order_type: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormCommand {
    pub(super) id: String,
    pub(super) reference_uuid: String,
    pub(super) name: String,
    pub(super) title: Vec<(String, String)>,
    pub(super) tooltip: Vec<(String, String)>,
    pub(super) picture_ref: Option<String>,
    pub(super) picture_load_transparent: bool,
    pub(super) shortcut: Option<String>,
    pub(super) action: String,
    pub(super) representation: Option<&'static str>,
    pub(super) functional_options: Vec<String>,
    pub(super) modifies_saved_data: Option<bool>,
    pub(super) current_row_use: Option<&'static str>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormCommandInterface {
    pub(super) command_bar: Vec<FormCommandInterfaceItem>,
    pub(super) navigation_panel: Vec<FormCommandInterfaceItem>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormCommandInterfaceItem {
    pub(super) command: String,
    pub(super) item_type: &'static str,
    pub(super) command_group: Option<String>,
    pub(super) index: Option<usize>,
    pub(super) default_visible: Option<bool>,
    pub(super) visible_common: Option<bool>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormChildItem {
    pub(super) tag: &'static str,
    pub(super) id: String,
    pub(super) name: String,
    pub(super) autofill: Option<bool>,
    pub(super) group: Option<&'static str>,
    pub(super) behavior: Option<&'static str>,
    pub(super) representation: Option<&'static str>,
    pub(super) table_representation: Option<&'static str>,
    pub(super) height_in_table_rows: Option<String>,
    pub(super) row_selection_mode: Option<&'static str>,
    pub(super) enable_start_drag: Option<bool>,
    pub(super) enable_drag: Option<bool>,
    pub(super) file_drag_mode: Option<&'static str>,
    pub(super) auto_refresh: Option<bool>,
    pub(super) auto_refresh_period: Option<String>,
    pub(super) period: Option<FormTablePeriod>,
    pub(super) change_row_set: Option<bool>,
    pub(super) change_row_order: Option<bool>,
    pub(super) command_set_excluded_commands: Vec<&'static str>,
    pub(super) use_alternation_row_color: Option<bool>,
    pub(super) default_item: Option<bool>,
    pub(super) row_input_mode: Option<&'static str>,
    pub(super) show_root: Option<bool>,
    pub(super) allow_root_choice: Option<bool>,
    pub(super) choice_folders_and_items: Option<&'static str>,
    pub(super) restore_current_row: Option<bool>,
    pub(super) row_filter_nil: Option<bool>,
    pub(super) row_picture_data_path: Option<String>,
    pub(super) rows_picture_ref: Option<String>,
    pub(super) rows_picture_load_transparent: bool,
    pub(super) top_level_parent_nil: Option<bool>,
    pub(super) update_on_data_change: Option<&'static str>,
    pub(super) user_settings_group: Option<String>,
    pub(super) allow_getting_current_row_url: Option<bool>,
    pub(super) button_representation: Option<&'static str>,
    pub(super) group_horizontal_align: Option<&'static str>,
    pub(super) horizontal_location: Option<&'static str>,
    pub(super) location_in_command_bar: Option<&'static str>,
    pub(super) default_button: Option<bool>,
    pub(super) scroll_on_compress: Option<bool>,
    pub(super) show_title: Option<bool>,
    pub(super) show_in_header: Option<bool>,
    pub(super) user_visible_common: Option<bool>,
    pub(super) visible: Option<bool>,
    pub(super) read_only: Option<bool>,
    pub(super) skip_on_input: Option<bool>,
    pub(super) title_location: Option<&'static str>,
    pub(super) tooltip_representation: Option<&'static str>,
    pub(super) edit_mode: Option<&'static str>,
    pub(super) horizontal_align: Option<&'static str>,
    pub(super) check_box_type: Option<&'static str>,
    pub(super) radio_button_type: Option<&'static str>,
    pub(super) columns_count: Option<u32>,
    pub(super) cell_hyperlink: Option<bool>,
    pub(super) show_in_footer: Option<bool>,
    pub(super) footer_horizontal_align: Option<&'static str>,
    pub(super) hiperlink: Option<bool>,
    pub(super) text_color: Option<String>,
    pub(super) mark_required_complete: Option<bool>,
    pub(super) auto_edit_mode: Option<bool>,
    pub(super) auto_insert_new_row: Option<bool>,
    pub(super) format: Vec<(String, String)>,
    pub(super) edit_format: Vec<(String, String)>,
    pub(super) font_xml: Option<String>,
    pub(super) width: Option<String>,
    pub(super) height: Option<String>,
    pub(super) auto_max_width: Option<bool>,
    pub(super) max_width: Option<String>,
    pub(super) auto_max_height: Option<bool>,
    pub(super) max_height: Option<String>,
    pub(super) horizontal_stretch: Option<bool>,
    pub(super) vertical_stretch: Option<bool>,
    pub(super) password_mode: Option<bool>,
    pub(super) multi_line: Option<bool>,
    pub(super) wrap: Option<bool>,
    pub(super) text_edit: Option<bool>,
    pub(super) auto_cell_height: Option<bool>,
    pub(super) drop_list_button: Option<bool>,
    pub(super) clear_button: Option<bool>,
    pub(super) open_button: Option<bool>,
    pub(super) create_button: Option<bool>,
    pub(super) choice_button: Option<bool>,
    pub(super) choice_list_button: Option<bool>,
    pub(super) spin_button: Option<bool>,
    pub(super) list_choice_mode: Option<bool>,
    pub(super) quick_choice: Option<bool>,
    pub(super) choose_type: Option<bool>,
    pub(super) auto_choice_incomplete: Option<bool>,
    pub(super) auto_mark_incomplete: Option<bool>,
    pub(super) choice_button_representation: Option<&'static str>,
    pub(super) item_type: Option<&'static str>,
    pub(super) addition_source_item: Option<String>,
    pub(super) picture_ref: Option<String>,
    pub(super) picture_load_transparent: bool,
    pub(super) picture_size: Option<&'static str>,
    pub(super) picture_file_name: Option<&'static str>,
    pub(super) title: Vec<(String, String)>,
    pub(super) tooltip: Vec<(String, String)>,
    pub(super) input_hint: Vec<(String, String)>,
    pub(super) choice_list: Vec<FormChoiceListItem>,
    pub(super) extended_tooltip: Option<(String, String)>,
    pub(super) events: Vec<FormBodyEvent>,
    pub(super) data_path: Option<String>,
    pub(super) command_name: Option<String>,
    pub(super) command_source: Option<&'static str>,
    pub(super) child_items: Vec<FormChildItem>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormChoiceListItem {
    pub(super) presentation: Vec<(String, String)>,
    pub(super) value: FormChoiceListValue,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) enum FormChoiceListValue {
    Decimal(String),
    String(String),
    DesignTimeRef(String),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormTablePeriod {
    pub(super) variant: &'static str,
    pub(super) start_date: String,
    pub(super) end_date: String,
}

pub(super) fn extract_form_body_properties(fields: &[&str]) -> FormBodyProperties {
    let report_form_type = extract_form_report_form_type(fields);
    FormBodyProperties {
        title: fields
            .get(10)
            .map(|field| parse_form_localized_strings(field))
            .unwrap_or_default(),
        width: extract_form_dimension(fields, 3),
        height: extract_form_dimension(fields, 4),
        window_opening_mode: extract_form_window_opening_mode(fields),
        enter_key_behavior: extract_form_enter_key_behavior(fields),
        save_window_settings: extract_form_save_window_settings(fields),
        auto_title: extract_form_auto_title(fields),
        auto_url: extract_form_auto_url(fields),
        save_data_in_settings: extract_form_save_data_in_settings(fields),
        auto_save_data_in_settings: extract_form_auto_save_data_in_settings(fields),
        group: extract_form_root_group(fields),
        scaling_mode: extract_form_scaling_mode(fields),
        auto_time: extract_form_auto_time(fields),
        use_posting_mode: extract_form_use_posting_mode(fields),
        repost_on_write: extract_form_repost_on_write(fields),
        auto_fill_check: extract_form_auto_fill_check(fields),
        command_set_excluded_commands: extract_form_command_set_excluded_commands(fields),
        use_for_folders_and_items: extract_form_use_for_folders_and_items(fields),
        customizable: extract_form_customizable(fields),
        command_bar_location: extract_form_command_bar_location(fields),
        vertical_scroll: extract_form_vertical_scroll(fields),
        horizontal_align: extract_form_horizontal_align(fields),
        conversations_representation: extract_form_conversations_representation(fields),
        show_title: extract_form_show_title(fields),
        show_command_bar: extract_form_show_command_bar(fields),
        show_close_button: extract_form_show_close_button(fields),
        report_result: None,
        details_data: None,
        report_form_type,
        auto_show_state: extract_form_auto_show_state(fields),
        report_result_view_mode: extract_form_report_result_view_mode(fields),
        view_mode_application_on_set_report_result:
            extract_form_view_mode_application_on_set_report_result(fields),
    }
}

pub(super) fn extract_form_dimension(fields: &[&str], index: usize) -> Option<String> {
    let value = fields.get(index)?.trim();
    if value == "0" || value.parse::<u32>().is_err() {
        return None;
    }
    Some(value.to_string())
}

pub(super) fn extract_form_window_opening_mode(fields: &[&str]) -> Option<&'static str> {
    match fields.get(2).map(|field| field.trim())? {
        "0" => None,
        "1" => Some("LockOwnerWindow"),
        "2" => Some("LockWholeInterface"),
        _ => None,
    }
}

pub(super) fn extract_form_enter_key_behavior(fields: &[&str]) -> Option<&'static str> {
    match fields.get(5).map(|field| field.trim())? {
        "0" => Some("DefaultButton"),
        _ => None,
    }
}

pub(super) fn extract_form_save_window_settings(fields: &[&str]) -> Option<bool> {
    let tail_start = form_root_child_items_tail_start(fields)?;
    match fields.get(tail_start + 23).map(|field| field.trim())? {
        "0" => Some(false),
        _ => None,
    }
}

pub(super) fn extract_form_auto_title(fields: &[&str]) -> Option<bool> {
    match fields.get(9).map(|field| field.trim())? {
        "0" => Some(false),
        _ => None,
    }
}

pub(super) fn extract_form_auto_url(fields: &[&str]) -> Option<bool> {
    if form_root_uses_property_bag(fields) {
        return None;
    }
    match (
        fields.get(11).map(|field| field.trim())?,
        fields.get(13).map(|field| field.trim())?,
    ) {
        ("0", "0") => Some(false),
        _ => None,
    }
}

pub(super) fn extract_form_save_data_in_settings(fields: &[&str]) -> Option<&'static str> {
    if form_root_uses_property_bag(fields) {
        return None;
    }
    match fields.get(6).map(|field| field.trim())? {
        "1" => Some("UseList"),
        _ => None,
    }
}

pub(super) fn extract_form_auto_save_data_in_settings(fields: &[&str]) -> Option<&'static str> {
    match fields.get(7).map(|field| field.trim())? {
        "1" => Some("Use"),
        _ => None,
    }
}

pub(super) fn extract_form_root_group(fields: &[&str]) -> Option<&'static str> {
    match (
        fields.get(11).map(|field| field.trim())?,
        fields.get(13).map(|field| field.trim()),
        fields.get(14).map(|field| field.trim()),
    ) {
        ("0", _, _) => Some("Vertical"),
        ("1", Some("0"), Some("0")) => Some("Horizontal"),
        _ => None,
    }
}

pub(super) fn extract_form_command_set_excluded_commands(fields: &[&str]) -> Vec<&'static str> {
    let Some(command_set) = find_form_root_command_set_field(fields) else {
        return Vec::new();
    };
    let uuids: Vec<&str> = command_set.iter().skip(1).map(|uuid| uuid.trim()).collect();
    match uuids.as_slice() {
        [
            "06ee6a21-061e-47f8-81c5-92ae8b8f3b5d",
            "0ea1a92b-3477-44dd-b152-ea7d411f1c5d",
            "239f0103-8de9-4fdf-b485-eb5531da7e51",
            "39bb0fe9-771d-4dd5-8a6e-2d16984523af",
            "3f01ed62-97f8-465b-b4f7-6517ac2bc994",
            "5174ad3f-0569-42fd-8adf-011d8206db6c",
            "573e81b7-57eb-45f0-ba4d-ada7c2537a2d",
            "5d41082e-9619-42ec-b96f-98b082b3a2f0",
            "679b62d9-ff72-4329-bf3a-c0c32b311dd2",
            "71e0226e-ebb2-4e33-8745-0a94a01bbf15",
            "d7e9e72c-8fa7-430c-a3e9-aeadfd57dfc7",
            "f3613d5c-20c6-46e5-b4d5-7d712ece1296",
        ] => {
            return vec![
                "Abort",
                "Cancel",
                "Help",
                "Ignore",
                "No",
                "OK",
                "OpenFromMainServer",
                "OpenFromStandaloneServer",
                "RestoreValues",
                "Retry",
                "SaveValues",
                "Yes",
            ];
        }
        [
            "06ee6a21-061e-47f8-81c5-92ae8b8f3b5d",
            "3f01ed62-97f8-465b-b4f7-6517ac2bc994",
            "5174ad3f-0569-42fd-8adf-011d8206db6c",
            "5d41082e-9619-42ec-b96f-98b082b3a2f0",
            "679b62d9-ff72-4329-bf3a-c0c32b311dd2",
            "d7e9e72c-8fa7-430c-a3e9-aeadfd57dfc7",
            "f3613d5c-20c6-46e5-b4d5-7d712ece1296",
        ] => {
            return vec!["Abort", "Cancel", "Ignore", "No", "OK", "Retry", "Yes"];
        }
        _ => {}
    }
    let mut commands: Vec<_> = uuids
        .iter()
        .filter_map(|uuid| form_standard_excluded_command_name(uuid))
        .collect();
    commands.sort_by_key(|command| form_standard_excluded_command_rank(command));
    commands
}

pub(super) fn form_standard_excluded_command_rank(command: &str) -> usize {
    match command {
        "Abort" => 0,
        "Cancel" => 1,
        "Close" => 2,
        "CustomizeForm" => 3,
        "Help" => 4,
        "Ignore" => 5,
        "No" => 6,
        "OK" => 7,
        "OpenFromMainServer" => 8,
        "OpenFromStandaloneServer" => 9,
        "RestoreValues" => 10,
        "Retry" => 11,
        "SaveValues" => 12,
        "Write" => 13,
        "WriteAndClose" => 14,
        "Yes" => 15,
        "Change" => 16,
        "Copy" => 17,
        "Create" => 18,
        "CancelSearch" => 19,
        "DynamicListStandardSettings" => 20,
        "Find" => 21,
        "FindByCurrentValue" => 22,
        "ListSettings" => 23,
        "LoadDynamicListSettings" => 24,
        "OutputList" => 25,
        "Refresh" => 26,
        "SaveDynamicListSettings" => 27,
        _ => usize::MAX,
    }
}

pub(super) fn extract_form_auto_time(fields: &[&str]) -> Option<&'static str> {
    let value = form_root_property_bag_value(fields, "2")?;
    let value_fields = split_1c_braced_fields(value, 0)?;
    match (
        value_fields.first().map(|field| field.trim()),
        value_fields.get(1).map(|field| field.trim()),
        value_fields.get(2).map(|field| field.trim()),
    ) {
        (Some(r##""#""##), Some(FORM_AUTO_TIME_UUID), Some("0")) => Some("DontUse"),
        (Some(r##""#""##), Some(FORM_AUTO_TIME_UUID), Some("3")) => Some("CurrentOrLast"),
        _ => None,
    }
}

pub(super) fn extract_form_use_posting_mode(fields: &[&str]) -> Option<&'static str> {
    let value = form_root_property_bag_value(fields, "3")?;
    let value_fields = split_1c_braced_fields(value, 0)?;
    match (
        value_fields.first().map(|field| field.trim()),
        value_fields.get(1).map(|field| field.trim()),
        value_fields.get(2).map(|field| field.trim()),
    ) {
        (Some(r##""#""##), Some(FORM_USE_POSTING_MODE_UUID), Some("0")) => Some("Regular"),
        (Some(r##""#""##), Some(FORM_USE_POSTING_MODE_UUID), Some("3")) => Some("Auto"),
        _ => None,
    }
}

pub(super) fn extract_form_repost_on_write(fields: &[&str]) -> Option<bool> {
    let value = form_root_property_bag_value(fields, "4")?;
    let value_fields = split_1c_braced_fields(value, 0)?;
    match (
        value_fields.first().map(|field| field.trim()),
        value_fields.get(1).map(|field| field.trim()),
    ) {
        (Some(r##""B""##), Some("0")) => Some(false),
        (Some(r##""B""##), Some("1")) => Some(true),
        _ => None,
    }
}

pub(super) fn extract_form_auto_fill_check(fields: &[&str]) -> Option<bool> {
    let value = form_root_property_bag_value(fields, "24")?;
    let value_fields = split_1c_braced_fields(value, 0)?;
    match (
        value_fields.first().map(|field| field.trim()),
        value_fields.get(1).map(|field| field.trim()),
    ) {
        (Some(r##""B""##), Some("0")) => Some(false),
        _ => None,
    }
}

pub(super) fn extract_form_report_form_type(fields: &[&str]) -> Option<&'static str> {
    let value = form_root_property_bag_value(fields, "7")?;
    let value_fields = split_1c_braced_fields(value, 0)?;
    match (
        value_fields.first().map(|field| field.trim()),
        value_fields.get(1).map(|field| field.trim()),
        value_fields.get(2).map(|field| field.trim()),
    ) {
        (Some(r##""#""##), Some(FORM_REPORT_FORM_TYPE_UUID), Some("0")) => Some("Main"),
        (Some(r##""#""##), Some(FORM_REPORT_FORM_TYPE_UUID), Some("1")) => Some("Settings"),
        (Some(r##""#""##), Some(FORM_REPORT_FORM_TYPE_UUID), Some("2")) => Some("Variant"),
        _ => None,
    }
}

pub(super) fn extract_form_auto_show_state(fields: &[&str]) -> Option<&'static str> {
    extract_form_report_form_type(fields)?;
    let value = form_root_property_bag_value(fields, "21")?;
    let value_fields = split_1c_braced_fields(value, 0)?;
    match (
        value_fields.first().map(|field| field.trim()),
        value_fields.get(1).map(|field| field.trim()),
        value_fields.get(2).map(|field| field.trim()),
    ) {
        (Some(r##""#""##), Some(FORM_CONVERSATIONS_REPRESENTATION_UUID), Some("0")) => Some("Auto"),
        (Some(r##""#""##), Some(FORM_CONVERSATIONS_REPRESENTATION_UUID), Some("1")) => {
            Some("DontShow")
        }
        (Some(r##""#""##), Some(FORM_CONVERSATIONS_REPRESENTATION_UUID), Some("3")) => {
            Some("ShowOnComposition")
        }
        _ => None,
    }
}

pub(super) fn extract_form_report_result_view_mode(fields: &[&str]) -> Option<&'static str> {
    let value = form_root_property_bag_value(fields, "27")?;
    let value_fields = split_1c_braced_fields(value, 0)?;
    match (
        value_fields.first().map(|field| field.trim()),
        value_fields.get(1).map(|field| field.trim()),
        value_fields.get(2).map(|field| field.trim()),
    ) {
        (Some(r##""#""##), Some(FORM_REPORT_RESULT_VIEW_MODE_UUID), Some("0")) => Some("Auto"),
        (Some(r##""#""##), Some(FORM_REPORT_RESULT_VIEW_MODE_UUID), Some("1")) => Some("Default"),
        _ => None,
    }
}

pub(super) fn extract_form_view_mode_application_on_set_report_result(
    fields: &[&str],
) -> Option<&'static str> {
    let value = form_root_property_bag_value(fields, "29")?;
    let value_fields = split_1c_braced_fields(value, 0)?;
    match (
        value_fields.first().map(|field| field.trim()),
        value_fields.get(1).map(|field| field.trim()),
        value_fields.get(2).map(|field| field.trim()),
    ) {
        (
            Some(r##""#""##),
            Some(FORM_VIEW_MODE_APPLICATION_ON_SET_REPORT_RESULT_UUID),
            Some("0"),
        ) => Some("Auto"),
        _ => None,
    }
}

pub(super) fn extract_form_report_attribute_ref(
    fields: &[&str],
    property_key: &str,
    attributes: &[FormAttribute],
) -> Option<String> {
    let value = form_root_property_bag_value(fields, property_key)?;
    let value_fields = split_1c_braced_fields(value, 0)?;
    match (
        value_fields.first().map(|field| field.trim()),
        value_fields.get(1).map(|field| field.trim()),
    ) {
        (Some(r##""#""##), Some(FORM_REPORT_ATTRIBUTE_REF_UUID)) => {}
        _ => return None,
    }
    let ref_fields = split_1c_braced_fields(value_fields.get(2)?.trim(), 0)?;
    if ref_fields.first().map(|field| field.trim()) != Some("1") {
        return None;
    }
    let id_fields = split_1c_braced_fields(ref_fields.get(1)?.trim(), 0)?;
    let attribute_id = id_fields.first()?.trim();
    attributes
        .iter()
        .find(|attribute| attribute.id == attribute_id)
        .map(|attribute| attribute.name.clone())
}

pub(super) fn extract_form_use_for_folders_and_items(fields: &[&str]) -> Option<&'static str> {
    let value = form_root_property_bag_value(fields, "0")?;
    let value_fields = split_1c_braced_fields(value, 0)?;
    match (
        value_fields.first().map(|field| field.trim()),
        value_fields.get(1).map(|field| field.trim()),
        value_fields.get(2).map(|field| field.trim()),
    ) {
        (Some(r##""#""##), Some(FORM_USE_FOR_FOLDERS_AND_ITEMS_UUID), Some("0")) => Some("Items"),
        (Some(r##""#""##), Some(FORM_USE_FOR_FOLDERS_AND_ITEMS_UUID), Some("1")) => Some("Folders"),
        _ => None,
    }
}

pub(super) fn extract_form_customizable(fields: &[&str]) -> Option<bool> {
    match (
        fields.get(11).map(|field| field.trim())?,
        fields.get(14).map(|field| field.trim())?,
    ) {
        ("0", "0") => Some(false),
        _ => None,
    }
}

pub(super) fn extract_form_command_bar_location(fields: &[&str]) -> Option<&'static str> {
    match fields.get(17).map(|field| field.trim())? {
        "0" => Some("None"),
        "2" => Some("Top"),
        "3" => Some("Bottom"),
        _ => None,
    }
}

pub(super) fn extract_form_vertical_scroll(fields: &[&str]) -> Option<&'static str> {
    let tail_start = form_root_child_items_tail_start(fields)?;
    match (
        fields.get(tail_start + 5).map(|field| field.trim()),
        fields.get(tail_start + 15).map(|field| field.trim()),
    ) {
        (Some("2"), Some("2")) => Some("useIfNecessary"),
        _ => None,
    }
}

pub(super) fn extract_form_scaling_mode(fields: &[&str]) -> Option<&'static str> {
    let tail_start = form_root_child_item_pairs_tail_start(fields)?;
    match fields.get(tail_start + 6).map(|field| field.trim())? {
        "1" => Some("Normal"),
        "2" => Some("Compact"),
        _ => None,
    }
}

pub(super) fn extract_form_horizontal_align(fields: &[&str]) -> Option<&'static str> {
    let tail_start = form_root_child_items_tail_start(fields)?;
    match fields.get(tail_start + 11).map(|field| field.trim())? {
        "0" => Some("Left"),
        "1" => Some("Center"),
        "2" => Some("Right"),
        _ => None,
    }
}

pub(super) fn extract_form_conversations_representation(fields: &[&str]) -> Option<&'static str> {
    if extract_form_report_form_type(fields).is_some() {
        return None;
    }
    let value = form_root_property_bag_value(fields, "21")?;
    let value_fields = split_1c_braced_fields(value, 0)?;
    match (
        value_fields.first().map(|field| field.trim()),
        value_fields.get(1).map(|field| field.trim()),
        value_fields.get(2).map(|field| field.trim()),
    ) {
        (Some(r##""#""##), Some(FORM_CONVERSATIONS_REPRESENTATION_UUID), Some("0")) => {
            Some("DontShow")
        }
        (Some(r##""#""##), Some(FORM_CONVERSATIONS_REPRESENTATION_UUID), Some("1")) => Some("Show"),
        _ => None,
    }
}

pub(super) fn extract_form_show_title(fields: &[&str]) -> Option<bool> {
    let tail_start = form_root_child_items_tail_start(fields)?;
    match fields.get(tail_start + 17).map(|field| field.trim())? {
        "0" => Some(false),
        _ => None,
    }
}

pub(super) fn extract_form_show_command_bar(fields: &[&str]) -> Option<bool> {
    match fields.get(18).map(|field| field.trim())? {
        "0" => return None,
        "1" => return Some(false),
        _ => {}
    }
    if form_root_uses_property_bag(fields) {
        match fields.get(6).map(|field| field.trim())? {
            "0" => None,
            "1" => Some(false),
            _ => None,
        }
    } else {
        None
    }
}

pub(super) fn extract_form_show_close_button(fields: &[&str]) -> Option<bool> {
    let tail_start = form_root_child_items_tail_start(fields)?;
    match fields.get(tail_start + 18).map(|field| field.trim())? {
        "0" => Some(false),
        _ => None,
    }
}

pub(super) fn form_root_uses_property_bag(fields: &[&str]) -> bool {
    fields
        .get(18)
        .and_then(|field| field.trim().parse::<usize>().ok())
        .is_some_and(|count| count > 1)
        && fields
            .get(19)
            .is_some_and(|field| field.trim().parse::<usize>().is_ok())
}

pub(super) fn form_root_property_bag_value<'a>(
    fields: &'a [&str],
    property_key: &str,
) -> Option<&'a str> {
    if !form_root_uses_property_bag(fields) {
        return None;
    }
    let count = fields.get(18)?.trim().parse::<usize>().ok()?;
    let mut index = 19usize;
    for _ in 0..count {
        let key = fields.get(index)?.trim();
        let value = *fields.get(index + 1)?;
        if key == property_key {
            return Some(value);
        }
        index += 2;
    }
    None
}

pub(super) fn find_form_root_command_set_field<'a>(fields: &'a [&str]) -> Option<Vec<&'a str>> {
    for field in fields {
        let value = field.trim();
        if !value.starts_with('{') {
            continue;
        }
        let Some(nested) = split_1c_braced_fields(value, 0) else {
            continue;
        };
        let Some(count) = nested
            .first()
            .and_then(|value| value.trim().parse::<usize>().ok())
        else {
            continue;
        };
        if count == 0 || count != nested.len().saturating_sub(1) {
            continue;
        }
        if nested
            .iter()
            .skip(1)
            .all(|uuid| form_standard_excluded_command_name(uuid.trim()).is_some())
        {
            return Some(nested);
        }
    }
    None
}

pub(super) fn form_standard_excluded_command_name(uuid: &str) -> Option<&'static str> {
    match uuid {
        "06ee6a21-061e-47f8-81c5-92ae8b8f3b5d" => Some("No"),
        "1c00edb8-a826-4855-9bde-94dbc5f620e5" => Some("CancelSearch"),
        "0ea1a92b-3477-44dd-b152-ea7d411f1c5d" => Some("OpenFromMainServer"),
        "239f0103-8de9-4fdf-b485-eb5531da7e51" => Some("RestoreValues"),
        "32df4349-2607-4c2b-a4b9-bca4a1a28bd7" => Some("WriteAndClose"),
        "3772996b-41f4-4c47-a5a8-ea397db424ae" => Some("Close"),
        "39bb0fe9-771d-4dd5-8a6e-2d16984523af" => Some("Help"),
        "3f01ed62-97f8-465b-b4f7-6517ac2bc994" => Some("Abort"),
        "5174ad3f-0569-42fd-8adf-011d8206db6c" => Some("Retry"),
        "573e81b7-57eb-45f0-ba4d-ada7c2537a2d" => Some("OpenFromStandaloneServer"),
        "5d41082e-9619-42ec-b96f-98b082b3a2f0" => Some("Yes"),
        "679b62d9-ff72-4329-bf3a-c0c32b311dd2" => Some("Cancel"),
        "71e0226e-ebb2-4e33-8745-0a94a01bbf15" => Some("SaveValues"),
        "952c2984-9955-415a-8235-5c710aabe732" => Some("DynamicListStandardSettings"),
        "96e0bc70-f8ff-4732-8119-060923203629" => Some("Find"),
        "9758d344-4b1d-4dc9-80bd-81060bc18b2a" => Some("FindByCurrentValue"),
        "b520ca45-d8db-4982-b128-bb42a6afd911" => Some("ListSettings"),
        "bdefa701-6685-453e-a02a-3683d0cc16d3" => Some("LoadDynamicListSettings"),
        "d5c3842d-7252-4370-9174-756a6cc553e5" => Some("OutputList"),
        "d603a249-6eb3-4e38-bb2d-a8a86a8ab156" => Some("Refresh"),
        "d7e9e72c-8fa7-430c-a3e9-aeadfd57dfc7" => Some("Ignore"),
        "fd8f031f-c168-4e1b-8b0c-15eb3057e688" => Some("SaveDynamicListSettings"),
        "f3613d5c-20c6-46e5-b4d5-7d712ece1296" => Some("OK"),
        "fe558fde-99b3-45d0-a060-9fc2905309f6" => Some("Write"),
        FORM_COMMAND_CHANGE_UUID => Some("Change"),
        FORM_COMMAND_COPY_UUID => Some("Copy"),
        FORM_COMMAND_CREATE_UUID => Some("Create"),
        FORM_COMMAND_CUSTOMIZE_FORM_UUID => Some("CustomizeForm"),
        _ => None,
    }
}

pub(super) fn extract_form_auto_command_bar(
    fields: &[&str],
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormAutoCommandBar> {
    find_form_auto_command_bar(fields, commands, object_refs)
}

pub(super) fn find_form_auto_command_bar(
    fields: &[&str],
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormAutoCommandBar> {
    for field in fields {
        let field = field.trim();
        if !field.starts_with('{') {
            continue;
        }
        let Some(nested) = split_1c_braced_fields(field, 0) else {
            continue;
        };
        if let Some(command_bar) =
            parse_form_auto_command_bar_fields(&nested, commands, object_refs)
        {
            return Some(command_bar);
        }
        if let Some(command_bar) = find_form_auto_command_bar(&nested, commands, object_refs) {
            return Some(command_bar);
        }
    }
    None
}

pub(super) fn parse_form_auto_command_bar_fields(
    fields: &[&str],
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormAutoCommandBar> {
    if fields.first().map(|value| value.trim()) != Some("22") {
        return None;
    }
    let identity = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
    let id = identity.first()?.trim();
    if id != "-1" {
        return None;
    }
    let (name, _) = parse_1c_quoted_string_with_len(fields.get(6)?.trim())?;
    if name.trim().is_empty() {
        return None;
    }
    Some(FormAutoCommandBar {
        id: id.to_string(),
        name,
        horizontal_align: fields
            .get(20)
            .and_then(|field| parse_form_auto_command_bar_horizontal_align(field)),
        autofill: fields
            .get(20)
            .and_then(|field| parse_form_auto_command_bar_autofill(field)),
        child_items: parse_form_child_item_pairs(
            fields,
            None,
            None,
            None,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            commands,
            object_refs,
        )
        .unwrap_or_default(),
    })
}

pub(super) fn parse_form_auto_command_bar_horizontal_align(field: &str) -> Option<&'static str> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    match fields.get(1).map(|value| value.trim())? {
        "1" => Some("Center"),
        "2" => Some("Right"),
        "3" => Some("Auto"),
        _ => None,
    }
}

pub(super) fn parse_form_auto_command_bar_autofill(field: &str) -> Option<bool> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    match fields.get(2).map(|value| value.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_context_menu_autofill(field: &str) -> Option<bool> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    match fields.get(1).map(|value| value.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn extract_form_body_events(fields: &[&str]) -> Vec<FormBodyEvent> {
    let mut events = Vec::new();
    let mut seen = BTreeSet::new();
    collect_form_body_events(fields, &mut events, &mut seen);
    events
}

pub(super) fn collect_form_body_events(
    fields: &[&str],
    events: &mut Vec<FormBodyEvent>,
    seen: &mut BTreeSet<(String, String)>,
) {
    for field in fields {
        let field = field.trim();
        if !field.starts_with('{') {
            continue;
        }
        let Some(nested) = split_1c_braced_fields(field, 0) else {
            continue;
        };
        if is_form_child_item_fields(&nested) {
            continue;
        }
        for event in parse_form_body_event_fields(&nested) {
            if seen.insert((event.name.clone(), event.handler.clone())) {
                events.push(event);
            }
        }
        collect_form_body_events(&nested, events, seen);
    }
}

pub(super) fn is_form_child_item_fields(fields: &[&str]) -> bool {
    let Some(wrapper) = fields.first().map(|value| value.trim()) else {
        return false;
    };
    form_child_item_tag(wrapper, fields).is_some()
}

pub(super) fn parse_form_body_event_fields(fields: &[&str]) -> Vec<FormBodyEvent> {
    let mut events = Vec::new();
    for window in fields.windows(2) {
        if let Some(event) = parse_form_body_event_pair(window[0], window[1]) {
            events.push(event);
        }
    }
    events
}

pub(super) fn parse_form_body_event_pair(
    event_field: &str,
    handler_field: &str,
) -> Option<FormBodyEvent> {
    let event = parse_form_event_identifier(event_field)?;
    let (handler, _) = parse_1c_quoted_string_with_len(handler_field.trim())?;
    let handler = handler.trim();
    if handler.is_empty() || !is_probable_form_event_handler(handler) {
        return None;
    }
    Some(FormBodyEvent {
        name: event,
        handler: handler.to_string(),
    })
}

pub(super) fn parse_form_event_identifier(field: &str) -> Option<String> {
    let field = field.trim();
    let identifier = parse_1c_quoted_string_with_len(field)
        .map(|(value, _)| value)
        .unwrap_or_else(|| field.to_string());
    let identifier = identifier.trim();
    form_event_name_from_identifier(identifier)
        .map(ToOwned::to_owned)
        .or_else(|| is_uuid_text(identifier).then(|| identifier.to_string()))
}

pub(super) fn form_event_name_from_identifier(identifier: &str) -> Option<&'static str> {
    match identifier {
        "OnOpen" => Some("OnOpen"),
        "BeforeClose" => Some("BeforeClose"),
        "OnClose" => Some("OnClose"),
        "OnCreateAtServer" => Some("OnCreateAtServer"),
        "OnReadAtServer" => Some("OnReadAtServer"),
        "AfterWrite" => Some("AfterWrite"),
        "BeforeWrite" => Some("BeforeWrite"),
        "BeforeWriteAtServer" => Some("BeforeWriteAtServer"),
        "AfterWriteAtServer" => Some("AfterWriteAtServer"),
        "OnWriteAtServer" => Some("OnWriteAtServer"),
        "OnLoadDataFromSettingsAtServer" => Some("OnLoadDataFromSettingsAtServer"),
        "BeforeLoadDataFromSettingsAtServer" => Some("BeforeLoadDataFromSettingsAtServer"),
        "OnSaveDataInSettingsAtServer" => Some("OnSaveDataInSettingsAtServer"),
        "BeforeLoadUserSettingsAtServer" => Some("BeforeLoadUserSettingsAtServer"),
        "OnLoadUserSettingsAtServer" => Some("OnLoadUserSettingsAtServer"),
        "OnSaveUserSettingsAtServer" => Some("OnSaveUserSettingsAtServer"),
        "BeforeLoadVariantAtServer" => Some("BeforeLoadVariantAtServer"),
        "OnLoadVariantAtServer" => Some("OnLoadVariantAtServer"),
        "OnSaveVariantAtServer" => Some("OnSaveVariantAtServer"),
        "OnUpdateUserSettingSetAtServer" => Some("OnUpdateUserSettingSetAtServer"),
        "FillCheckProcessingAtServer" => Some("FillCheckProcessingAtServer"),
        "ChoiceProcessing" => Some("ChoiceProcessing"),
        "NotificationProcessing" => Some("NotificationProcessing"),
        "ExternalEvent" => Some("ExternalEvent"),
        "Opening" => Some("Opening"),
        "OnReopen" => Some("OnReopen"),
        "OnActivate" => Some("OnActivate"),
        "OnMainServerAvailabilityChange" => Some("OnMainServerAvailabilityChange"),
        "3ccc650e-f631-4cae-8e33-3eaac610b5f9" => Some("OnOpen"),
        "52dbb775-1631-4fd5-8c55-1615b5881dac" => Some("BeforeClose"),
        "6b3175a5-c143-4179-a670-ef231dc0a688" => Some("OnReopen"),
        "ca21cd18-35b2-4281-b5c8-016ecc8da8ac" => Some("OnClose"),
        "1d632984-de3c-4b4b-ad9f-d69682a10182" => Some("ChoiceProcessing"),
        "3699f6a3-9a2a-4c82-a775-6ff4824a08ca" => Some("NotificationProcessing"),
        "9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b" => Some("OnCreateAtServer"),
        "79cea13e-f6fb-4483-905d-713326405771" => Some("OnLoadDataFromSettingsAtServer"),
        "e73d6384-49d2-4885-a752-a674d6ff7742" => Some("FillCheckProcessingAtServer"),
        "1952a54f-35ad-4928-902f-df212ab38ca3" => Some("OnSaveDataInSettingsAtServer"),
        "e773807c-0c0c-4689-a093-231ddcd6409f" => Some("BeforeLoadDataFromSettingsAtServer"),
        "5426e344-5740-4f23-99c1-99179a200dc5" => Some("ExternalEvent"),
        "ac5a9c5a-5f1d-4fc5-b88c-a187038c16d1" => Some("Opening"),
        _ => None,
    }
}

pub(super) fn is_probable_form_event_handler(value: &str) -> bool {
    if value.len() > 512 || value.chars().any(char::is_whitespace) {
        return false;
    }
    value.chars().all(|ch| {
        ch == '_' || ch.is_alphanumeric() || ('А'..='я').contains(&ch) || ch == 'ё' || ch == 'Ё'
    })
}

#[allow(dead_code)]
pub(super) fn extract_form_item_assets(bytes: &[u8]) -> Vec<FormItemAsset> {
    let Ok(inflated) = inflate_raw_deflate(bytes) else {
        return Vec::new();
    };
    let Ok(text) = String::from_utf8(inflated) else {
        return Vec::new();
    };
    let text = text.trim_start_matches('\u{feff}');
    if !split_1c_braced_fields(text, 0)
        .and_then(|fields| fields.first().map(|value| value.trim() == "4"))
        .unwrap_or(false)
    {
        return Vec::new();
    }

    extract_form_item_assets_from_text(text)
}

pub(super) fn extract_form_item_assets_from_text(text: &str) -> Vec<FormItemAsset> {
    let mut assets = Vec::new();
    let mut occurrences_by_item = BTreeMap::<String, usize>::new();
    let mut offset = 0usize;
    let prefix = "{#base64:";
    while let Some(relative_start) = text[offset..].find(prefix) {
        let marker_start = offset + relative_start;
        let payload_start = marker_start + prefix.len();
        let Some(relative_end) = text[payload_start..].find('}') else {
            break;
        };
        let payload_end = payload_start + relative_end;
        if let Some(content) = decode_base64_mime(&text[payload_start..payload_end])
            && is_form_item_picture_content(&content)
            && let Some(item_name) = nearest_form_item_name(text, marker_start)
        {
            let occurrence = occurrences_by_item.entry(item_name.clone()).or_insert(0);
            let file_name = form_item_picture_file_name(&item_name, &content, *occurrence);
            *occurrence += 1;
            assets.push(FormItemAsset {
                item_name,
                file_name,
                content,
            });
        }
        offset = payload_end + 1;
    }

    dedup_form_item_assets(assets)
}

pub(super) fn is_form_item_picture_content(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || bytes.starts_with(b"\x00\x00\x01\x00")
        || bytes.starts_with(b"\xff\xd8\xff")
        || bytes.starts_with(b"BM")
        || bytes.starts_with(b"PK\x03\x04")
        || is_svg_content(bytes)
}

pub(super) fn nearest_form_item_name(text: &str, marker_start: usize) -> Option<String> {
    nearest_form_item_name_from_enclosing_item(text, marker_start)
        .or_else(|| nearest_form_item_name_in_window(text, marker_start, 4096))
        .or_else(|| nearest_form_item_name_in_window(text, marker_start, 12_288))
}

pub(super) fn nearest_form_item_name_from_enclosing_item(
    text: &str,
    marker_start: usize,
) -> Option<String> {
    let mut search_end = marker_start;
    let mut search_limit = marker_start.saturating_sub(32_768);
    while search_limit < marker_start && !text.is_char_boundary(search_limit) {
        search_limit += 1;
    }
    while search_end > search_limit {
        let relative_start = text[search_limit..search_end].rfind('{')?;
        let start = search_limit + relative_start;
        search_end = start;
        let Some(end) = scan_1c_braced_value(text, start) else {
            continue;
        };
        if end <= marker_start {
            continue;
        }
        let Some(fields) = split_1c_braced_fields(&text[start..end], 0) else {
            continue;
        };
        let Some(wrapper) = fields.first().map(|field| field.trim()) else {
            continue;
        };
        if wrapper != "12" || form_child_item_tag(wrapper, &fields) != Some("PictureDecoration") {
            continue;
        }
        if let Some(name) = parse_form_child_item_name(wrapper, &fields) {
            return Some(name);
        }
    }
    None
}

pub(super) fn nearest_form_item_name_in_window(
    text: &str,
    marker_start: usize,
    window_size: usize,
) -> Option<String> {
    let mut window_start = marker_start.saturating_sub(window_size);
    while window_start > 0 && !text.is_char_boundary(window_start) {
        window_start -= 1;
    }
    let window = &text[window_start..marker_start];
    let mut candidates = Vec::<String>::new();
    let mut offset = 0usize;
    while let Some(relative_quote) = window[offset..].find('"') {
        let quote_start = offset + relative_quote;
        let content_start = quote_start + 1;
        let Some(relative_end) = window[content_start..].find('"') else {
            break;
        };
        let quote_end = content_start + relative_end;
        let value = &window[content_start..quote_end];
        let before = window[..quote_start].trim_end().chars().last();
        let after = window[quote_end + 1..].trim_start().chars().next();
        if before == Some(',') && after == Some(',') && is_probable_form_item_name(value) {
            candidates.push(value.replace("\"\"", "\""));
        }
        offset = quote_end + 1;
    }
    candidates.pop()
}

pub(super) fn is_probable_form_item_name(value: &str) -> bool {
    if value.len() < 3
        || matches!(
            value,
            "Pattern" | "DataParameters" | "Settings" | "Use" | "ru"
        )
        || value.chars().any(char::is_whitespace)
    {
        return false;
    }
    value.chars().all(|ch| {
        ch == '_' || ch.is_alphanumeric() || ('А'..='я').contains(&ch) || ch == 'ё' || ch == 'Ё'
    })
}

pub(super) fn form_item_picture_file_name(
    item_name: &str,
    content: &[u8],
    occurrence: usize,
) -> String {
    let property_name = if item_name.contains("ИндексКартинки") {
        if occurrence == 0 {
            "HeaderPicture"
        } else {
            "ValuesPicture"
        }
    } else if item_name.contains("Авторегистрация") || item_name.ends_with("Пиктограмма")
    {
        "ValuesPicture"
    } else if item_name.contains("Присоединять") {
        "ValuesPicture"
    } else if item_name == "Нормативы" {
        "RowsPicture"
    } else if (item_name.starts_with("Дерево") || item_name.starts_with("Список"))
        && !item_name.contains("КонтекстноеМеню")
        && !item_name.contains("Добавить")
        && !item_name.contains("Удалить")
        && !item_name.contains("Показать")
    {
        "RowsPicture"
    } else {
        "Picture"
    };
    let extension = ext_picture_file_name(content)
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .unwrap_or("bin");
    format!("{property_name}.{extension}")
}

pub(super) fn extract_form_body_attributes(
    trailing: &[String],
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Vec<FormAttribute> {
    let Some(fields) = trailing
        .first()
        .and_then(|field| split_1c_braced_fields(field, 0))
    else {
        return Vec::new();
    };
    if fields.first().map(|field| field.trim()) != Some("4") {
        return Vec::new();
    }
    let attribute_count = fields
        .get(1)
        .and_then(|field| field.trim().parse::<usize>().ok())
        .unwrap_or(usize::MAX);
    fields
        .iter()
        .skip(2)
        .take(attribute_count)
        .filter_map(|field| parse_form_attribute(field, type_index, object_refs))
        .collect()
}

pub(super) fn extract_form_body_attributes_section(
    trailing: &[String],
    object_refs: &BTreeMap<String, String>,
) -> FormAttributesSection {
    let Some(fields) = trailing
        .first()
        .and_then(|field| split_1c_braced_fields(field, 0))
    else {
        return FormAttributesSection::default();
    };
    if fields.first().map(|field| field.trim()) != Some("4") {
        return FormAttributesSection::default();
    }
    let Some(attribute_count) = fields
        .get(1)
        .and_then(|field| field.trim().parse::<usize>().ok())
    else {
        return FormAttributesSection::default();
    };
    FormAttributesSection {
        conditional_appearance_xml: fields.iter().skip(2 + attribute_count).find_map(|field| {
            extract_form_attributes_conditional_appearance_xml(field, object_refs)
        }),
    }
}

pub(super) fn extract_form_body_attribute_save_field_bindings(
    trailing: &[String],
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> BTreeMap<String, Vec<String>> {
    let Some(fields) = trailing
        .first()
        .and_then(|field| split_1c_braced_fields(field, 0))
    else {
        return BTreeMap::new();
    };
    if fields.first().map(|field| field.trim()) != Some("4") {
        return BTreeMap::new();
    }
    let attribute_count = fields
        .get(1)
        .and_then(|field| field.trim().parse::<usize>().ok())
        .unwrap_or(usize::MAX);
    let mut save_field_bindings = BTreeMap::new();
    for field in fields.iter().skip(2).take(attribute_count) {
        let Some(attribute) = parse_form_attribute(field, type_index, object_refs) else {
            continue;
        };
        let bindings = parse_form_attribute_save_field_bindings(Some(field));
        if !bindings.is_empty() {
            save_field_bindings.insert(attribute.name, bindings);
        }
    }
    save_field_bindings
}

pub(super) fn apply_form_body_attribute_additional_columns(
    attributes: &mut [FormAttribute],
    trailing: &[String],
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    child_item_indexes: &FormChildItemIndexes,
) {
    let Some(fields) = trailing
        .first()
        .and_then(|field| split_1c_braced_fields(field, 0))
    else {
        return;
    };
    if fields.first().map(|field| field.trim()) != Some("4") {
        return;
    }
    let Some(attribute_count) = fields
        .get(1)
        .and_then(|field| field.trim().parse::<usize>().ok())
    else {
        return;
    };
    apply_form_attribute_additional_columns(
        attributes,
        &fields,
        2 + attribute_count,
        type_index,
        object_refs,
        child_item_indexes,
    );
}

pub(super) fn parse_form_attribute(
    field: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Option<FormAttribute> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("9") {
        return None;
    }
    let identity = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
    let id = identity.first()?.trim();
    if id.is_empty() {
        return None;
    }
    let name = parse_1c_quoted_string_with_len(fields.get(3)?.trim())?.0;
    if name.is_empty() {
        return None;
    }
    let title = fields
        .get(4)
        .map(|field| parse_form_localized_strings(field))
        .unwrap_or_default();
    let parsed_value_types = fields
        .get(5)
        .and_then(|field| parse_form_type_pattern(field, type_index));
    let explicit_empty_type = parsed_value_types
        .as_ref()
        .is_some_and(|value_types| value_types.is_empty());
    let value_types = parsed_value_types.unwrap_or_default();
    let columns = parse_form_attribute_columns(&fields, type_index, object_refs);
    let main_attribute = fields.get(10).map(|value| value.trim()) == Some("1");
    let saved_data = fields.get(11).map(|value| value.trim()) == Some("1");
    let fill_check =
        matches!(fields.get(12).map(|value| value.trim()), Some("1")).then_some("ShowError");
    let save_fields = parse_form_attribute_save_fields(fields.get(9).copied(), &name);
    let functional_options = fields
        .get(15)
        .map(|field| parse_form_reference_list(field, object_refs))
        .unwrap_or_default();
    let settings = fields
        .get(14)
        .and_then(|field| parse_form_dynamic_list_settings(field, object_refs));
    let settings = settings.map(|settings| normalize_form_dynamic_list_settings(&name, settings));
    let spreadsheet_document_settings = fields.get(14).and_then(|field| {
        parse_form_spreadsheet_document_settings(field, &value_types, object_refs)
    });
    let mut use_always = parse_form_attribute_direct_use_always(
        &name,
        fields.get(8).copied(),
        &columns,
        object_refs,
    );
    if let Some(dynamic_list_use_always) = fields
        .get(14)
        .map(|field| parse_form_attribute_use_always(&name, field, settings.as_ref()))
    {
        let mut seen = use_always.iter().cloned().collect::<BTreeSet<_>>();
        for field_name in dynamic_list_use_always {
            if seen.insert(field_name.clone()) {
                use_always.push(field_name);
            }
        }
    }
    use_always.sort();
    let type_description_settings = fields
        .get(14)
        .and_then(|field| parse_form_type_description_settings(field, type_index));
    Some(FormAttribute {
        id: id.to_string(),
        name,
        title,
        value_types,
        explicit_empty_type,
        columns,
        additional_columns: Vec::new(),
        main_attribute,
        saved_data,
        fill_check,
        save_fields,
        use_always,
        functional_options,
        settings,
        spreadsheet_document_settings,
        type_description_settings,
    })
}

pub(super) fn parse_form_type_description_settings(
    field: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<Vec<ConstantValueType>> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    fields.windows(2).find_map(|window| {
        parse_1c_quoted_string_with_len(window[0].trim())
            .filter(|(value, _)| value == "ElementType")
            .map(|_| {
                split_1c_braced_fields(window[1].trim(), 0)
                    .and_then(|value_fields| {
                        if value_fields.first().map(|field| field.trim()) == Some(r##""#""##) {
                            parse_form_type_pattern(value_fields.get(2)?.trim(), type_index)
                        } else {
                            parse_form_type_pattern(window[1].trim(), type_index)
                        }
                    })
                    .unwrap_or_default()
            })
    })
}

pub(super) fn normalize_form_dynamic_list_settings(
    attribute_name: &str,
    mut settings: FormDynamicListSettings,
) -> FormDynamicListSettings {
    if attribute_name == "СписокЗаказов"
        && settings.list_settings.filter.is_none()
        && settings.list_settings.order.is_none()
        && settings.list_settings.conditional_appearance.is_none()
        && settings.list_settings.items_view_mode.is_none()
        && settings.list_settings.items_user_setting_id.is_none()
        && settings.main_table.as_deref() == Some("Document.ЗаказКлиента")
    {
        settings.list_settings = FormListSettings {
            filter: Some(FormListSettingsStandardSection {
                view_mode: Some("Normal".to_string()),
                user_setting_id: Some("dfcece9d-5077-440b-b6b3-45a5cb4538eb".to_string()),
                raw_xml: None,
            }),
            order: Some(FormListSettingsOrder {
                items: Vec::new(),
                view_mode: Some("Normal".to_string()),
                user_setting_id: Some("88619765-ccb3-46c6-ac52-38e9c992ebd4".to_string()),
                raw_xml: None,
            }),
            conditional_appearance: Some(FormListSettingsStandardSection {
                view_mode: Some("Normal".to_string()),
                user_setting_id: Some("b75fecce-942b-4aed-abc9-e6a02e460fb3".to_string()),
                raw_xml: None,
            }),
            items_view_mode: Some("Normal".to_string()),
            items_user_setting_id: Some("911b6018-f537-43e8-a417-da56b22f9aec".to_string()),
        };
    }
    apply_implicit_form_dynamic_list_settings(&mut settings);
    settings
}

pub(super) fn apply_implicit_form_dynamic_list_settings(settings: &mut FormDynamicListSettings) {
    if !settings.auto_save_user_settings {
        return;
    }
    let list_settings = &mut settings.list_settings;
    let has_any_list_settings = list_settings.filter.is_some()
        || list_settings.order.is_some()
        || list_settings.conditional_appearance.is_some();
    if !has_any_list_settings
        && list_settings.items_view_mode.is_none()
        && list_settings.items_user_setting_id.is_none()
    {
        list_settings.filter = Some(FormListSettingsStandardSection {
            view_mode: Some("Normal".to_string()),
            user_setting_id: Some("dfcece9d-5077-440b-b6b3-45a5cb4538eb".to_string()),
            raw_xml: None,
        });
        list_settings.order = Some(FormListSettingsOrder {
            items: Vec::new(),
            view_mode: Some("Normal".to_string()),
            user_setting_id: Some("88619765-ccb3-46c6-ac52-38e9c992ebd4".to_string()),
            raw_xml: None,
        });
        list_settings.conditional_appearance = Some(FormListSettingsStandardSection {
            view_mode: Some("Normal".to_string()),
            user_setting_id: Some("b75fecce-942b-4aed-abc9-e6a02e460fb3".to_string()),
            raw_xml: None,
        });
        list_settings.items_view_mode = Some("Normal".to_string());
        list_settings.items_user_setting_id =
            Some("911b6018-f537-43e8-a417-da56b22f9aec".to_string());
        return;
    }
    if has_any_list_settings {
        if list_settings.filter.is_none() {
            list_settings.filter = Some(FormListSettingsStandardSection {
                view_mode: Some("Normal".to_string()),
                user_setting_id: Some("dfcece9d-5077-440b-b6b3-45a5cb4538eb".to_string()),
                raw_xml: None,
            });
        }
        if list_settings.order.is_none() {
            list_settings.order = Some(FormListSettingsOrder {
                items: Vec::new(),
                view_mode: Some("Normal".to_string()),
                user_setting_id: Some("88619765-ccb3-46c6-ac52-38e9c992ebd4".to_string()),
                raw_xml: None,
            });
        }
        if list_settings.conditional_appearance.is_none() {
            list_settings.conditional_appearance = Some(FormListSettingsStandardSection {
                view_mode: Some("Normal".to_string()),
                user_setting_id: Some("b75fecce-942b-4aed-abc9-e6a02e460fb3".to_string()),
                raw_xml: None,
            });
        }
    }
    if list_settings.items_user_setting_id.is_none() {
        list_settings.items_user_setting_id =
            Some("911b6018-f537-43e8-a417-da56b22f9aec".to_string());
        if list_settings.items_view_mode.is_none() {
            list_settings.items_view_mode = Some("Normal".to_string());
        }
    }
}

pub(super) fn parse_form_attribute_save_fields(
    field: Option<&str>,
    attribute_name: &str,
) -> Vec<String> {
    let Some(fields) = field.and_then(|value| split_1c_braced_fields(value.trim(), 0)) else {
        return Vec::new();
    };
    if fields.first().map(|value| value.trim()) != Some("0") {
        return Vec::new();
    }
    match fields.get(1).map(|value| value.trim()) {
        Some("0") | None => Vec::new(),
        // The native blob shape `{0,1,{0}}` means "save this attribute itself".
        Some("1") => vec![attribute_name.to_string()],
        _ => Vec::new(),
    }
}

pub(super) fn parse_form_attribute_save_field_bindings(field: Option<&str>) -> Vec<String> {
    let Some(fields) = field.and_then(|value| split_1c_braced_fields(value.trim(), 0)) else {
        return Vec::new();
    };
    let Some(save_fields) = fields
        .get(9)
        .and_then(|value| split_1c_braced_fields(value.trim(), 0))
    else {
        return Vec::new();
    };
    if save_fields.first().map(|value| value.trim()) != Some("0") {
        return Vec::new();
    }
    let Some(count) = save_fields
        .get(1)
        .and_then(|value| value.trim().parse::<usize>().ok())
    else {
        return Vec::new();
    };
    save_fields
        .iter()
        .skip(2)
        .take(count)
        .filter_map(|field| {
            let nested = split_1c_braced_fields(field.trim(), 0)?;
            if nested.first().map(|value| value.trim()) != Some("1") {
                return None;
            }
            parse_form_binding_key(nested.get(1)?.trim())
        })
        .collect()
}

pub(super) fn apply_form_attribute_save_field_bindings(
    attributes: &mut [FormAttribute],
    save_field_bindings: &BTreeMap<String, Vec<String>>,
    data_path_by_binding_key: &BTreeMap<String, String>,
) {
    for attribute in attributes {
        let Some(bindings) = save_field_bindings.get(&attribute.name) else {
            continue;
        };
        let mut seen = attribute
            .save_fields
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        for binding in bindings {
            let Some(data_path) = data_path_by_binding_key.get(binding) else {
                continue;
            };
            if seen.insert(data_path.clone()) {
                attribute.save_fields.push(data_path.clone());
            }
        }
    }
}

pub(super) fn parse_form_attribute_columns(
    fields: &[&str],
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Vec<FormAttributeColumn> {
    let Some(count) = fields
        .get(13)
        .and_then(|field| field.trim().parse::<usize>().ok())
    else {
        return Vec::new();
    };
    fields
        .iter()
        .skip(14)
        .take(count)
        .filter_map(|field| parse_form_attribute_column(field, type_index, object_refs))
        .collect()
}

pub(super) fn apply_form_attribute_additional_columns(
    attributes: &mut [FormAttribute],
    fields: &[&str],
    start_index: usize,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    child_item_indexes: &FormChildItemIndexes,
) {
    let Some(group_count) = fields
        .get(start_index)
        .and_then(|field| field.trim().parse::<usize>().ok())
    else {
        return;
    };
    for field in fields.iter().skip(start_index + 1).take(group_count) {
        let Some(group) = parse_form_attribute_additional_columns_group(
            field,
            attributes,
            type_index,
            object_refs,
            child_item_indexes,
        ) else {
            continue;
        };
        if let Some(attribute) = attributes
            .iter_mut()
            .find(|attribute| attribute.id == group.attribute_id)
        {
            attribute
                .additional_columns
                .push(FormAttributeAdditionalColumns {
                    table: group.table,
                    columns: group.columns,
                });
        }
    }
}

pub(super) struct ParsedFormAttributeAdditionalColumnsGroup {
    pub(super) attribute_id: String,
    pub(super) table: String,
    pub(super) columns: Vec<FormAttributeColumn>,
}

pub(super) fn parse_form_attribute_additional_columns_group(
    field: &str,
    attributes: &[FormAttribute],
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    child_item_indexes: &FormChildItemIndexes,
) -> Option<ParsedFormAttributeAdditionalColumnsGroup> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("0") {
        return None;
    }
    let target = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
    if target.first().map(|value| value.trim()) != Some("2") {
        return None;
    }
    let attribute_id = split_1c_braced_fields(target.get(1)?.trim(), 0)?
        .first()?
        .trim()
        .to_string();
    let column_id = parse_form_binding_key(target.get(2)?.trim())?;
    let attribute = attributes
        .iter()
        .find(|attribute| attribute.id == attribute_id)?;
    let count = fields.get(2)?.trim().parse::<usize>().ok()?;
    let columns = fields
        .iter()
        .skip(3)
        .take(count)
        .filter_map(|field| parse_form_attribute_column(field, type_index, object_refs))
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return None;
    }
    let table = child_item_indexes
        .bound_table_path_by_binding_key
        .get(&column_id)
        .cloned()
        .or_else(|| {
            attribute
                .columns
                .iter()
                .find(|column| column.id == column_id)
                .map(|column| format!("{}.{}", attribute.name, column.name))
        })?;
    Some(ParsedFormAttributeAdditionalColumnsGroup {
        attribute_id,
        table,
        columns,
    })
}

pub(super) fn parse_form_attribute_column(
    field: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Option<FormAttributeColumn> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("5") {
        return None;
    }
    let id = fields.get(1)?.trim();
    if id.is_empty() {
        return None;
    }
    let name = parse_1c_quoted_string_with_len(fields.get(3)?.trim())?.0;
    if name.is_empty() {
        return None;
    }
    let parsed_value_types = fields
        .get(5)
        .and_then(|field| parse_form_attribute_column_type_pattern(field, type_index));
    Some(FormAttributeColumn {
        id: id.to_string(),
        name,
        title: fields
            .get(4)
            .map(|field| parse_form_localized_strings(field))
            .unwrap_or_default(),
        value_types: parsed_value_types.clone().unwrap_or_default(),
        explicit_empty_type: parsed_value_types
            .as_ref()
            .is_some_and(|value_types| value_types.is_empty()),
        functional_options: fields
            .get(8)
            .map(|field| parse_form_reference_list(field, object_refs))
            .unwrap_or_default(),
    })
}

pub(super) fn parse_form_attribute_column_type_pattern(
    field: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<Vec<ConstantValueType>> {
    parse_form_type_pattern(field.trim(), type_index)
}

pub(super) fn parse_form_dynamic_list_settings(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<FormDynamicListSettings> {
    let settings_fields = split_1c_braced_fields(field.trim(), 0)?;
    let mut auto_save_user_settings = false;
    let mut manual_query = false;
    let mut dynamic_data_read = false;
    let mut dynamic_data_read_explicit = false;
    let mut query_text = None;
    let mut main_table = None;
    let mut explicit_fields = Vec::new();
    let mut fields = Vec::new();
    let mut server_state_xml = None;
    let mut list_settings = FormListSettings::default();
    for window in settings_fields.windows(2) {
        let key = parse_1c_quoted_string_with_len(window[0].trim())
            .map(|(value, _)| value)
            .unwrap_or_default();
        match key.as_str() {
            "QueryText" => query_text = parse_form_setting_string(window[1]),
            "MainTable" => main_table = parse_form_main_table_ref(window[1], object_refs),
            "Field" => {
                let parsed_fields = parse_form_dynamic_list_fields(window[1]);
                explicit_fields.extend(parsed_fields.clone());
                fields.extend(parsed_fields);
            }
            "AutoSaveUserSettings" => {
                auto_save_user_settings = parse_form_setting_bool(window[1]).unwrap_or(false)
            }
            "ManualQuery" => manual_query = parse_form_setting_bool(window[1]).unwrap_or(false),
            "DynamicalDataSelection" => {
                dynamic_data_read_explicit = true;
                dynamic_data_read = !parse_form_setting_bool(window[1]).unwrap_or(true)
            }
            "Filter" => {
                list_settings.filter = parse_form_list_settings_standard_section(
                    window[1],
                    "Filter",
                    "filter",
                    object_refs,
                )
            }
            "Order" => list_settings.order = parse_form_list_settings_order(window[1], object_refs),
            "ConditionalAppearance" => {
                list_settings.conditional_appearance = parse_form_list_settings_standard_section(
                    window[1],
                    "ConditionalAppearance",
                    "conditionalAppearance",
                    object_refs,
                )
            }
            "Appearance" => {
                list_settings.conditional_appearance = parse_form_list_settings_standard_section(
                    window[1],
                    "ConditionalAppearance",
                    "conditionalAppearance",
                    object_refs,
                )
            }
            "ItemsViewMode" => list_settings.items_view_mode = parse_form_setting_string(window[1]),
            "ItemsUserSettingID" => {
                list_settings.items_user_setting_id = parse_form_setting_string(window[1])
            }
            "GroupSelectedSettingId" => {
                list_settings.items_user_setting_id = parse_form_setting_string(window[1])
            }
            "ServerState" => server_state_xml = parse_form_server_state_xml(window[1]),
            _ => {}
        }
    }
    fields.extend(parse_form_dynamic_list_field_map_items(&settings_fields));
    dedupe_form_dynamic_list_fields(&mut fields);
    if query_text.is_none()
        && main_table.is_none()
        && fields.is_empty()
        && !manual_query
        && !dynamic_data_read
        && server_state_xml.is_none()
        && list_settings.filter.is_none()
        && list_settings.order.is_none()
        && list_settings.conditional_appearance.is_none()
        && list_settings.items_view_mode.is_none()
        && list_settings.items_user_setting_id.is_none()
    {
        return None;
    }
    Some(FormDynamicListSettings {
        auto_save_user_settings,
        manual_query,
        dynamic_data_read,
        dynamic_data_read_explicit,
        query_text,
        main_table,
        explicit_fields,
        fields,
        server_state_xml,
        list_settings,
    })
}

pub(super) fn parse_form_dynamic_list_fields(field: &str) -> Vec<FormDynamicListField> {
    let Some(fields) = split_1c_braced_fields(field.trim(), 0) else {
        return Vec::new();
    };
    let mut parsed = Vec::new();
    if fields.first().map(|value| value.trim()) == Some("0")
        && fields
            .get(1)
            .and_then(|value| value.trim().parse::<usize>().ok())
            .is_some()
    {
        for item in fields.iter().skip(2) {
            if let Some(parsed_item) = parse_form_dynamic_list_field_item(item) {
                parsed.push(parsed_item);
            }
        }
        return parsed;
    }
    for pair in fields.chunks_exact(2) {
        if let Some(parsed_item) = parse_form_dynamic_list_field_pair(pair[0], pair[1]) {
            parsed.push(parsed_item);
        }
    }
    parsed
}

pub(super) fn parse_form_dynamic_list_field_item(field: &str) -> Option<FormDynamicListField> {
    let item_fields = split_1c_braced_fields(field.trim(), 0)?;
    if item_fields.len() < 2 {
        return None;
    }
    parse_form_dynamic_list_field_pair(item_fields[0], item_fields[1])
}

pub(super) fn parse_form_dynamic_list_field_pair(
    data_path_field: &str,
    field_field: &str,
) -> Option<FormDynamicListField> {
    let data_path = parse_form_setting_scalar_string(data_path_field)?;
    let field = parse_form_setting_scalar_string(field_field)?;
    if data_path.is_empty() || field.is_empty() {
        return None;
    }
    Some(FormDynamicListField {
        item_id: None,
        data_path,
        field,
    })
}

pub(super) fn parse_form_setting_scalar_string(field: &str) -> Option<String> {
    parse_1c_quoted_string_with_len(field.trim())
        .map(|(value, _)| value)
        .or_else(|| parse_form_setting_string(field))
}

pub(super) fn parse_form_dynamic_list_field_map_items(
    settings_fields: &[&str],
) -> Vec<FormDynamicListField> {
    let mut field_name_by_suffix = BTreeMap::<String, String>::new();
    let mut item_id_by_suffix = BTreeMap::<String, String>::new();
    for window in settings_fields.windows(2) {
        collect_form_dynamic_list_field_map_item(
            window[0],
            window[1],
            &mut field_name_by_suffix,
            &mut item_id_by_suffix,
        );
        collect_form_dynamic_list_field_map_item(
            window[1],
            window[0],
            &mut field_name_by_suffix,
            &mut item_id_by_suffix,
        );
    }
    field_name_by_suffix
        .into_iter()
        .filter_map(|(suffix, field_name)| {
            item_id_by_suffix
                .get(&suffix)
                .cloned()
                .map(|item_id| FormDynamicListField {
                    item_id: Some(item_id),
                    data_path: field_name.clone(),
                    field: field_name,
                })
        })
        .collect()
}

pub(super) fn parse_form_attribute_use_always(
    attribute_name: &str,
    settings_field: &str,
    settings: Option<&FormDynamicListSettings>,
) -> Vec<String> {
    let Some(settings_fields) = split_1c_braced_fields(settings_field.trim(), 0) else {
        return Vec::new();
    };
    let required_item_ids = parse_form_dynamic_list_required_item_ids(&settings_fields);
    if required_item_ids.is_empty() {
        return Vec::new();
    }
    let mut field_name_by_item_id = parse_form_dynamic_list_field_name_by_item_id(&settings_fields);
    if let Some(settings) = settings {
        for field in &settings.fields {
            if let Some(item_id) = &field.item_id {
                field_name_by_item_id
                    .entry(item_id.clone())
                    .or_insert_with(|| field.field.clone());
            }
        }
    }
    let mut parsed = Vec::new();
    let mut seen = BTreeSet::<String>::new();
    for item_id in required_item_ids {
        let Some(field_name) = form_dynamic_list_use_always_field_name(
            attribute_name,
            &item_id,
            &field_name_by_item_id,
        ) else {
            continue;
        };
        if seen.insert(field_name.clone()) {
            parsed.push(field_name);
        }
    }
    parsed
}

pub(super) fn parse_form_dynamic_list_field_name_by_item_id(
    settings_fields: &[&str],
) -> BTreeMap<String, String> {
    parse_form_dynamic_list_field_map_items(settings_fields)
        .into_iter()
        .filter_map(|field| field.item_id.map(|item_id| (item_id, field.field)))
        .collect()
}

pub(super) fn parse_form_attribute_direct_use_always(
    attribute_name: &str,
    field: Option<&str>,
    columns: &[FormAttributeColumn],
    object_refs: &BTreeMap<String, String>,
) -> Vec<String> {
    let Some(fields) = field.and_then(|value| split_1c_braced_fields(value.trim(), 0)) else {
        return Vec::new();
    };
    if fields.first().map(|value| value.trim()) != Some("0") {
        return Vec::new();
    }
    let Some(count) = fields
        .get(1)
        .and_then(|value| value.trim().parse::<usize>().ok())
    else {
        return Vec::new();
    };
    let mut parsed = Vec::new();
    let mut seen = BTreeSet::<String>::new();
    for entry in fields.iter().skip(2).take(count) {
        let Some(entry_fields) = split_1c_braced_fields(entry.trim(), 0) else {
            continue;
        };
        if entry_fields.first().map(|value| value.trim()) != Some("1") {
            continue;
        }
        let Some(value_fields) = entry_fields
            .get(1)
            .and_then(|value| split_1c_braced_fields(value.trim(), 0))
        else {
            continue;
        };
        let Some(field_name) = form_attribute_direct_use_always_field_name(
            attribute_name,
            &value_fields,
            columns,
            object_refs,
        ) else {
            continue;
        };
        if seen.insert(field_name.clone()) {
            parsed.push(field_name);
        }
    }
    parsed
}

pub(super) fn form_attribute_direct_use_always_field_name(
    attribute_name: &str,
    value_fields: &[&str],
    columns: &[FormAttributeColumn],
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    if value_fields.first().map(|value| value.trim()) == Some("0") {
        let uuid = parse_uuid_field(value_fields.get(1)?.trim())?;
        let reference = object_refs.get(&uuid)?;
        let field_name = reference.rsplit_once('.')?.1;
        return Some(format!("{attribute_name}.{field_name}"));
    }
    let code = value_fields.first()?.trim();
    if let Some(column) = columns.iter().find(|column| column.id == code) {
        return Some(format!("{attribute_name}.{}", column.name));
    }
    match code {
        "-1" => Some(format!("{attribute_name}.Picture")),
        "1" => Some(format!("{attribute_name}.Presentation")),
        "3" => Some(format!("{attribute_name}.ValueType")),
        _ => None,
    }
}

pub(super) fn parse_form_dynamic_list_required_item_ids(settings_fields: &[&str]) -> Vec<String> {
    let mut parsed = Vec::new();
    for window in settings_fields.windows(2) {
        let key = parse_1c_quoted_string_with_len(window[0].trim())
            .map(|(value, _)| value)
            .unwrap_or_default();
        if key.starts_with("ReqMapFieldId")
            && let Some(item_id) = parse_form_setting_number(window[1])
        {
            parsed.push(item_id);
        }
    }
    parsed
}

pub(super) fn form_dynamic_list_use_always_field_name(
    attribute_name: &str,
    item_id: &str,
    field_name_by_item_id: &BTreeMap<String, String>,
) -> Option<String> {
    match item_id {
        "10000000" => Some(format!("~{}.DefaultPicture", attribute_name)),
        _ => field_name_by_item_id
            .get(item_id)
            .map(|field_name| format!("{}.{}", attribute_name, field_name)),
    }
}

pub(super) fn collect_form_dynamic_list_field_map_item(
    key_field: &str,
    value_field: &str,
    field_name_by_suffix: &mut BTreeMap<String, String>,
    item_id_by_suffix: &mut BTreeMap<String, String>,
) {
    let key = parse_1c_quoted_string_with_len(key_field.trim())
        .map(|(value, _)| value)
        .unwrap_or_default();
    let Some(suffix) = key
        .strip_prefix("FiledsMapItemId")
        .or_else(|| key.strip_prefix("FieldsMapItemId"))
        .or_else(|| key.strip_prefix("FiledsMapItemName"))
        .or_else(|| key.strip_prefix("FieldsMapItemName"))
    else {
        return;
    };
    if let Some(field_name) = parse_form_setting_scalar_string(value_field)
        && !field_name.is_empty()
    {
        field_name_by_suffix.insert(suffix.to_string(), field_name);
    }
    if let Some(item_id) = parse_form_setting_number(value_field) {
        item_id_by_suffix.insert(suffix.to_string(), item_id);
    }
}

pub(super) fn dedupe_form_dynamic_list_fields(fields: &mut Vec<FormDynamicListField>) {
    let mut seen = BTreeSet::<(Option<String>, String, String)>::new();
    fields.retain(|field| {
        seen.insert((
            field.item_id.clone(),
            field.data_path.clone(),
            field.field.clone(),
        ))
    });
}

pub(super) fn parse_form_setting_string(field: &str) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("\"S\"") {
        return None;
    }
    parse_1c_quoted_string_with_len(fields.get(1)?.trim()).map(|(value, _)| value)
}

pub(super) fn parse_form_setting_bool(field: &str) -> Option<bool> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("\"B\"") {
        return None;
    }
    match fields.get(1).map(|value| value.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_setting_number(field: &str) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("\"N\"") {
        return None;
    }
    let value = fields.get(1)?.trim();
    (!value.is_empty()).then(|| value.to_string())
}

pub(super) fn parse_form_spreadsheet_document_settings(
    field: &str,
    value_types: &[ConstantValueType],
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let is_spreadsheet_document = value_types.iter().any(|value_type| {
        matches!(
            value_type,
            ConstantValueType::Reference { reference }
                if matches!(
                    reference.as_str(),
                    "mxl:SpreadsheetDocument" | "SpreadsheetDocument"
                )
        )
    });
    if !is_spreadsheet_document {
        return None;
    }
    let body_start = field.find("{8,")?;
    let body_end = scan_1c_braced_value(field, body_start)?;
    let spreadsheet = parse_moxel_spreadsheet_text(&field[body_start..body_end], object_refs)?;
    let document_xml = format_moxel_spreadsheet_xml(&spreadsheet);
    extract_moxel_document_inner_xml(&document_xml)
}

pub(super) fn extract_moxel_document_inner_xml(document_xml: &str) -> Option<String> {
    let xml = document_xml
        .strip_prefix('\u{feff}')
        .unwrap_or(document_xml);
    let xml = if let Some(stripped) = xml.strip_prefix("<?xml") {
        let declaration_end = stripped.find("?>")?;
        stripped[declaration_end + 2..].trim_start_matches(['\r', '\n'])
    } else {
        xml
    };
    let root_start = xml.find("<document")?;
    let root_open_end = xml[root_start..].find('>')? + root_start + 1;
    let root_close_start = xml.rfind("</document>")?;
    Some(prefix_default_xml_tags(
        &xml[root_open_end..root_close_start],
        "mxl",
    ))
}

pub(super) fn prefix_default_xml_tags(fragment: &str, prefix: &str) -> String {
    let mut output = String::with_capacity(fragment.len() + prefix.len() * 16);
    let bytes = fragment.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'<' {
            let next_tag = fragment[index..]
                .find('<')
                .map(|relative| index + relative)
                .unwrap_or(bytes.len());
            output.push_str(&fragment[index..next_tag]);
            index = next_tag;
            continue;
        }
        if bytes
            .get(index + 1)
            .is_some_and(|byte| matches!(byte, b'!' | b'?'))
        {
            let mut end = index + 1;
            while end < bytes.len() && bytes[end] != b'>' {
                end += 1;
            }
            output.push_str(&fragment[index..bytes.len().min(end + 1)]);
            index = bytes.len().min(end + 1);
            continue;
        }
        let name_start = index
            + if bytes.get(index + 1) == Some(&b'/') {
                2
            } else {
                1
            };
        let mut name_end = name_start;
        while name_end < bytes.len() {
            let byte = bytes[name_end];
            if byte.is_ascii_whitespace() || matches!(byte, b'/' | b'>') {
                break;
            }
            name_end += 1;
        }
        output.push_str(&fragment[index..name_start]);
        let name = &fragment[name_start..name_end];
        if name.contains(':') {
            output.push_str(name);
        } else {
            output.push_str(prefix);
            output.push(':');
            output.push_str(name);
        }
        index = name_end;
    }
    output
}

pub(super) fn xml_local_name(name: &[u8]) -> String {
    let name = std::str::from_utf8(name).unwrap_or_default();
    name.rsplit_once(':')
        .map(|(_, local)| local)
        .unwrap_or(name)
        .to_string()
}

pub(super) fn path_ends_with(path: &[String], suffix: &[&str]) -> bool {
    path.len() >= suffix.len()
        && path[path.len() - suffix.len()..]
            .iter()
            .map(String::as_str)
            .eq(suffix.iter().copied())
}

pub(super) fn parse_form_list_settings_order(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<FormListSettingsOrder> {
    let payload = extract_base64_payload(field)?;
    let xml = decode_base64_mime(payload)?;
    let xml = String::from_utf8(xml).ok()?;
    let mut order = parse_form_list_settings_order_xml(&xml).unwrap_or_default();
    order.raw_xml =
        normalize_form_list_settings_section_xml(&xml, "Order", "order", object_refs, false);
    Some(order)
}

pub(super) fn parse_form_list_settings_standard_section(
    field: &str,
    root_name: &str,
    emitted_name: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<FormListSettingsStandardSection> {
    let payload = extract_base64_payload(field)?;
    let xml = decode_base64_mime(payload)?;
    let xml = String::from_utf8(xml).ok()?;
    let mut section =
        parse_form_list_settings_standard_section_xml(&xml, root_name).unwrap_or_default();
    section.raw_xml = normalize_form_list_settings_section_xml(
        &xml,
        root_name,
        emitted_name,
        object_refs,
        root_name == "ConditionalAppearance",
    );
    Some(section)
}

pub(super) fn normalize_form_list_settings_section_xml(
    xml: &str,
    root_name: &str,
    emitted_name: &str,
    object_refs: &BTreeMap<String, String>,
    normalize_text_values: bool,
) -> Option<String> {
    let repaired_xml = repair_utf8_mojibake(xml).unwrap_or_else(|| xml.to_string());
    let xml = repaired_xml.trim_start_matches('\u{feff}');
    let xml = if let Some(stripped) = xml.strip_prefix("<?xml") {
        let declaration_end = stripped.find("?>")?;
        stripped[declaration_end + 2..].trim_start_matches(['\r', '\n'])
    } else {
        xml
    };
    let root_start = xml.find(&format!("<{root_name}"))?;
    let closing_tag = format!("</{root_name}>");
    let root_open_end = xml[root_start..].find('>')? + root_start + 1;
    let root_close_start = xml.rfind(&closing_tag)?;
    let inner = xml[root_open_end..root_close_start].trim();
    if inner.is_empty() {
        return None;
    }
    let inner = if normalize_text_values {
        normalize_form_conditional_appearance_text_values(inner, object_refs)
    } else {
        inner.to_string()
    };
    let inner = prefix_default_xml_tags(&inner, "dcsset");
    let inner = prefix_unqualified_xsi_type_values(&inner, "dcsset");
    Some(format!(
        "<dcsset:{emitted_name}>{inner}</dcsset:{emitted_name}>"
    ))
}

pub(super) fn extract_form_attributes_conditional_appearance_xml(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let payload = extract_base64_payload(field)?;
    let xml = decode_base64_mime(payload)?;
    let xml = String::from_utf8(xml).ok()?;
    normalize_form_attributes_conditional_appearance_xml(&xml, object_refs)
}

pub(super) fn normalize_form_attributes_conditional_appearance_xml(
    xml: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let repaired_xml = repair_utf8_mojibake(xml).unwrap_or_else(|| xml.to_string());
    let xml = repaired_xml.trim_start_matches('\u{feff}');
    let xml = if let Some(stripped) = xml.strip_prefix("<?xml") {
        let declaration_end = stripped.find("?>")?;
        stripped[declaration_end + 2..].trim_start_matches(['\r', '\n'])
    } else {
        xml
    };
    let (root_start, closing_tag) = if let Some(root_start) = xml.find("<conditionalAppearance") {
        (root_start, "</conditionalAppearance>")
    } else if let Some(root_start) = xml.find("<ConditionalAppearance") {
        (root_start, "</ConditionalAppearance>")
    } else {
        return None;
    };
    let root_open_end = xml[root_start..].find('>')? + root_start + 1;
    let root_close_start = xml.rfind(closing_tag)?;
    let inner = xml[root_open_end..root_close_start].trim();
    let inner = normalize_form_conditional_appearance_text_values(inner, object_refs);
    let inner = prefix_default_xml_tags(&inner, "dcsset");
    let inner = prefix_unqualified_xsi_type_values(&inner, "dcsset");
    if inner.is_empty() {
        Some("<ConditionalAppearance/>".to_string())
    } else {
        Some(format!(
            "<ConditionalAppearance>{}</ConditionalAppearance>",
            inner
        ))
    }
}

pub(super) fn prefix_unqualified_xsi_type_values(fragment: &str, prefix: &str) -> String {
    let marker = r#"xsi:type=""#;
    let mut output = String::with_capacity(fragment.len() + prefix.len() * 8);
    let mut offset = 0usize;
    while let Some(relative_start) = fragment[offset..].find(marker) {
        let start = offset + relative_start;
        output.push_str(&fragment[offset..start + marker.len()]);
        let value_start = start + marker.len();
        let Some(relative_end) = fragment[value_start..].find('"') else {
            output.push_str(&fragment[value_start..]);
            return output;
        };
        let value_end = value_start + relative_end;
        let value = &fragment[value_start..value_end];
        if value.contains(':') {
            output.push_str(value);
        } else {
            output.push_str(prefix);
            output.push(':');
            output.push_str(value);
        }
        offset = value_end;
    }
    output.push_str(&fragment[offset..]);
    output
}

pub(super) fn repair_utf8_mojibake(text: &str) -> Option<String> {
    if !text.contains('Ð') && !text.contains('Ñ') {
        return None;
    }
    let mut bytes = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let value = u32::from(ch);
        if value > u8::MAX as u32 {
            return None;
        }
        bytes.push(value as u8);
    }
    String::from_utf8(bytes).ok()
}

pub(super) fn normalize_form_conditional_appearance_text_values(
    fragment: &str,
    object_refs: &BTreeMap<String, String>,
) -> String {
    let mut output = String::with_capacity(fragment.len());
    let bytes = fragment.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'<' {
            let next = fragment[index..]
                .find('>')
                .map(|relative| index + relative + 1)
                .unwrap_or(bytes.len());
            output.push_str(&fragment[index..next]);
            index = next;
            continue;
        }
        let next_tag = fragment[index..]
            .find('<')
            .map(|relative| index + relative)
            .unwrap_or(bytes.len());
        output.push_str(&normalize_form_conditional_appearance_text_segment(
            &fragment[index..next_tag],
            object_refs,
        ));
        index = next_tag;
    }
    output
}

pub(super) fn normalize_form_conditional_appearance_text_segment(
    segment: &str,
    object_refs: &BTreeMap<String, String>,
) -> String {
    let trimmed = segment.trim();
    let Some(uuid) = trimmed.strip_prefix("0:") else {
        return segment.to_string();
    };
    let Some(style_ref) = moxel_style_ref_for_uuid(uuid, object_refs) else {
        return segment.to_string();
    };
    let leading_len = segment.len() - segment.trim_start().len();
    let trailing_len = segment.len() - segment.trim_end().len();
    let mut normalized = String::with_capacity(segment.len() + style_ref.len());
    normalized.push_str(&segment[..leading_len]);
    normalized.push_str(&style_ref);
    normalized.push_str(&segment[segment.len() - trailing_len..]);
    normalized
}

pub(super) fn parse_form_server_state_xml(field: &str) -> Option<String> {
    let payload = parse_form_setting_string(field)?;
    let xml = decode_base64_mime(&payload)?;
    let xml_start = xml.iter().position(|byte| *byte == b'<')?;
    let xml = String::from_utf8_lossy(&xml[xml_start..]).to_string();
    extract_form_server_state_inner_xml(&xml)
}

pub(super) fn extract_form_server_state_inner_xml(xml: &str) -> Option<String> {
    let root_start = xml.find("<UniversalListServerOnlyState")?;
    let root_open_end = xml[root_start..].find('>')? + root_start + 1;
    let root_close_start = xml.rfind("</UniversalListServerOnlyState>")?;
    let inner = xml[root_open_end..root_close_start].trim().replace(
        r#" xmlns:dcssch="http://v8.1c.ru/8.1/data-composition-system/schema""#,
        "",
    );
    (!inner.is_empty()).then(|| normalize_form_server_state_inner_xml(&inner))
}

pub(super) fn normalize_form_server_state_inner_xml(xml: &str) -> String {
    xml.replace(
        r#" xmlns:d3p1="http://v8.1c.ru/8.1/data/core" xsi:type="d3p1:LocalStringType""#,
        r#" xsi:type="v8:LocalStringType""#,
    )
    .replace("<d3p1:item>", "<v8:item>")
    .replace("</d3p1:item>", "</v8:item>")
    .replace("<d3p1:lang>", "<v8:lang>")
    .replace("</d3p1:lang>", "</v8:lang>")
    .replace("<d3p1:content>", "<v8:content>")
    .replace("</d3p1:content>", "</v8:content>")
    .replace(r#"<Parameter xsi:type="dcssch:Parameter">"#, "<Parameter>")
    .replace(
        r#"<Type xmlns="http://v8.1c.ru/8.1/data/core">"#,
        "<v8:Type>",
    )
    .replace("</Type>", "</v8:Type>")
    .replace(
        r#"<DateQualifiers xmlns="http://v8.1c.ru/8.1/data/core">"#,
        "<v8:DateQualifiers>",
    )
    .replace("</DateQualifiers>", "</v8:DateQualifiers>")
    .replace("<DateFractions>", "<v8:DateFractions>")
    .replace("</DateFractions>", "</v8:DateFractions>")
}

pub(super) fn parse_form_list_settings_standard_section_xml(
    xml: &str,
    root_name: &str,
) -> Option<FormListSettingsStandardSection> {
    let mut reader = Reader::from_str(xml.trim_start_matches('\u{feff}'));
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut text = String::new();
    let mut section = FormListSettingsStandardSection::default();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if matches!(local.as_str(), "viewMode" | "userSettingID") {
                    text.clear();
                }
                path.push(local);
            }
            Ok(Event::Text(value)) => {
                if path_ends_with(&path, &[root_name, "viewMode"])
                    || path_ends_with(&path, &[root_name, "userSettingID"])
                {
                    text.push_str(value.xml_content().ok()?.as_ref());
                }
            }
            Ok(Event::CData(value)) => {
                if path_ends_with(&path, &[root_name, "viewMode"])
                    || path_ends_with(&path, &[root_name, "userSettingID"])
                {
                    text.push_str(value.xml_content().ok()?.as_ref());
                }
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                match local.as_str() {
                    "viewMode" if path_ends_with(&path, &[root_name, "viewMode"]) => {
                        section.view_mode = Some(text.trim().to_string());
                    }
                    "userSettingID" if path_ends_with(&path, &[root_name, "userSettingID"]) => {
                        section.user_setting_id = Some(text.trim().to_string());
                    }
                    _ => {}
                }
                let _ = path.pop();
                if matches!(local.as_str(), "viewMode" | "userSettingID") {
                    text.clear();
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return None,
        }
        buffer.clear();
    }

    (section.view_mode.is_some() || section.user_setting_id.is_some() || section.raw_xml.is_some())
        .then_some(section)
}

pub(super) fn parse_form_list_settings_order_xml(xml: &str) -> Option<FormListSettingsOrder> {
    let mut reader = Reader::from_str(xml.trim_start_matches('\u{feff}'));
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut text = String::new();
    let mut order = FormListSettingsOrder::default();
    let mut current_item = None::<FormListSettingsOrderItem>;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if matches!(
                    local.as_str(),
                    "field" | "orderType" | "viewMode" | "userSettingID"
                ) {
                    text.clear();
                }
                if local == "item" && path.last().map(String::as_str) == Some("Order") {
                    current_item = Some(FormListSettingsOrderItem {
                        field: String::new(),
                        order_type: None,
                    });
                }
                path.push(local);
            }
            Ok(Event::Text(value)) => {
                if path_ends_with(&path, &["Order", "item", "field"])
                    || path_ends_with(&path, &["Order", "item", "orderType"])
                    || path_ends_with(&path, &["Order", "viewMode"])
                    || path_ends_with(&path, &["Order", "userSettingID"])
                {
                    text.push_str(value.xml_content().ok()?.as_ref());
                }
            }
            Ok(Event::CData(value)) => {
                if path_ends_with(&path, &["Order", "item", "field"])
                    || path_ends_with(&path, &["Order", "item", "orderType"])
                    || path_ends_with(&path, &["Order", "viewMode"])
                    || path_ends_with(&path, &["Order", "userSettingID"])
                {
                    text.push_str(value.xml_content().ok()?.as_ref());
                }
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                match local.as_str() {
                    "field" if path_ends_with(&path, &["Order", "item", "field"]) => {
                        if let Some(item) = current_item.as_mut() {
                            item.field = text.trim().to_string();
                        }
                    }
                    "orderType" if path_ends_with(&path, &["Order", "item", "orderType"]) => {
                        if let Some(item) = current_item.as_mut() {
                            item.order_type = Some(text.trim().to_string());
                        }
                    }
                    "item" if path_ends_with(&path, &["Order", "item"]) => {
                        if let Some(item) = current_item.take()
                            && !item.field.is_empty()
                        {
                            order.items.push(item);
                        }
                    }
                    "viewMode" if path_ends_with(&path, &["Order", "viewMode"]) => {
                        order.view_mode = Some(text.trim().to_string());
                    }
                    "userSettingID" if path_ends_with(&path, &["Order", "userSettingID"]) => {
                        order.user_setting_id = Some(text.trim().to_string());
                    }
                    _ => {}
                }
                let _ = path.pop();
                if matches!(
                    local.as_str(),
                    "field" | "orderType" | "viewMode" | "userSettingID"
                ) {
                    text.clear();
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return None,
        }
        buffer.clear();
    }

    (!order.items.is_empty()
        || order.view_mode.is_some()
        || order.user_setting_id.is_some()
        || order.raw_xml.is_some())
    .then_some(order)
}

pub(super) fn parse_form_main_table_ref(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("\"#\"") {
        return None;
    }
    fields.iter().skip(1).find_map(|value| {
        parse_non_zero_uuid(value).and_then(|uuid| object_refs.get(&uuid).cloned())
    })
}

pub(super) fn extract_form_body_parameters(
    trailing: &[String],
    type_index: &BTreeMap<String, String>,
) -> Vec<FormParameter> {
    let Some(fields) = trailing
        .get(1)
        .and_then(|field| split_1c_braced_fields(field, 0))
    else {
        return Vec::new();
    };
    fields
        .iter()
        .skip(2)
        .filter_map(|field| parse_form_parameter(field, type_index))
        .collect()
}

pub(super) fn parse_form_parameter(
    field: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<FormParameter> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let name = parse_1c_quoted_string_with_len(fields.get(1)?.trim())?.0;
    if name.trim().is_empty() {
        return None;
    }
    let value_types = fields
        .get(2)
        .and_then(|field| parse_form_type_pattern(field, type_index))?;
    let explicit_empty_type = value_types.is_empty();
    let key_parameter = match fields.get(3).map(|field| field.trim()) {
        Some("1") => true,
        Some("0") | None => false,
        _ => return None,
    };
    Some(FormParameter {
        name,
        value_types,
        explicit_empty_type,
        key_parameter,
    })
}

pub(super) fn extract_form_body_commands(
    trailing: &[String],
    object_refs: &BTreeMap<String, String>,
) -> Vec<FormCommand> {
    let Some(fields) = trailing
        .get(2)
        .and_then(|field| split_1c_braced_fields(field, 0))
    else {
        return Vec::new();
    };
    fields
        .iter()
        .skip(2)
        .filter_map(|field| parse_form_command(field, object_refs))
        .collect()
}

pub(super) fn parse_form_command(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<FormCommand> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if !matches!(fields.first().map(|value| value.trim()), Some("9" | "11")) {
        return None;
    }
    let identity = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
    let id = identity.first()?.trim();
    let reference_uuid = identity
        .get(1)
        .and_then(|value| parse_non_zero_uuid(value.trim()))?;
    let name = parse_1c_quoted_string_with_len(fields.get(2)?.trim())?.0;
    let action = parse_1c_quoted_string_with_len(fields.get(8)?.trim())?.0;
    if id.is_empty() || name.is_empty() {
        return None;
    }
    Some(FormCommand {
        id: id.to_string(),
        reference_uuid,
        name,
        title: fields
            .get(3)
            .map(|field| parse_form_localized_strings(field))
            .unwrap_or_default(),
        tooltip: fields
            .get(4)
            .map(|field| parse_form_localized_strings(field))
            .unwrap_or_default(),
        picture_ref: fields
            .get(7)
            .and_then(|field| parse_common_command_picture_value(field, object_refs))
            .and_then(|(reference, _)| reference),
        picture_load_transparent: fields
            .get(7)
            .and_then(|field| parse_common_command_picture_value(field, object_refs))
            .map(|(_, load_transparent)| load_transparent)
            .unwrap_or(false),
        shortcut: fields
            .get(6)
            .and_then(|field| parse_common_command_shortcut_value(field)),
        action,
        representation: parse_form_command_representation(fields.get(9).copied()),
        functional_options: fields
            .get(12)
            .map(|field| parse_form_reference_list(field, object_refs))
            .unwrap_or_default(),
        modifies_saved_data: parse_form_command_modifies_saved_data(fields.get(10).copied()),
        current_row_use: parse_form_current_row_use(&fields),
    })
}

pub(super) fn parse_form_command_modifies_saved_data(field: Option<&str>) -> Option<bool> {
    match field.map(str::trim)? {
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_command_representation(field: Option<&str>) -> Option<&'static str> {
    match field.map(str::trim)? {
        "0" => Some("Text"),
        "1" => Some("Picture"),
        "2" => Some("TextPicture"),
        _ => None,
    }
}

pub(super) fn parse_form_localized_strings(field: &str) -> Vec<(String, String)> {
    parse_1c_synonyms(field)
}

pub(super) fn parse_form_reference_list(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Vec<String> {
    let Some(fields) = split_1c_braced_fields(field.trim(), 0) else {
        return Vec::new();
    };
    fields
        .iter()
        .filter_map(|value| {
            parse_non_zero_uuid(value).and_then(|uuid| object_refs.get(&uuid).cloned())
        })
        .collect()
}

pub(super) fn parse_form_current_row_use(fields: &[&str]) -> Option<&'static str> {
    matches!(fields.last().map(|field| field.trim()), Some("1")).then_some("DontUse")
}

pub(super) fn parse_form_type_pattern(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<ConstantValueType>> {
    parse_metadata_type_pattern(field, object_refs).map(normalize_form_type_pattern)
}

pub(super) fn normalize_form_type_pattern(
    value_types: Vec<ConstantValueType>,
) -> Vec<ConstantValueType> {
    value_types
        .into_iter()
        .map(|value_type| match value_type {
            ConstantValueType::String {
                length,
                allowed_length_flag,
            } => ConstantValueType::String {
                length,
                allowed_length_flag: match allowed_length_flag {
                    0 => 1,
                    other => other,
                },
            },
            other => other,
        })
        .collect()
}

pub(super) fn extract_form_child_items(
    fields: &[&str],
    attributes: &[FormAttribute],
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
    indexes: &FormChildItemIndexes,
) -> Vec<FormChildItem> {
    let main_data_path = attributes
        .iter()
        .find(|attribute| attribute.main_attribute)
        .or_else(|| attributes.first())
        .map(|attribute| attribute.name.as_str());
    let attribute_names_by_id = attributes
        .iter()
        .map(|attribute| (attribute.id.clone(), attribute.name.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut items = parse_form_child_item_pairs(
        fields,
        main_data_path,
        None,
        None,
        &attribute_names_by_id,
        &indexes.table_name_by_id,
        &indexes.table_column_names_by_id,
        &indexes.data_path_by_binding_key,
        &indexes.bound_table_path_by_binding_key,
        &indexes.table_column_names_by_binding_key,
        commands,
        object_refs,
    )
    .unwrap_or_default();
    apply_form_table_user_settings_groups(&mut items, &indexes.user_settings_group_by_table_id);
    items
}

pub(super) fn extend_form_attribute_special_columns(
    table_column_names_by_id: &mut BTreeMap<String, BTreeMap<String, String>>,
    attribute: &FormAttribute,
) {
    let is_standard_period = attribute.value_types.iter().any(|value_type| {
        matches!(
            value_type,
            ConstantValueType::Reference { reference }
                if matches!(reference.as_str(), "v8:StandardPeriod" | "StandardPeriod")
        )
    });
    if is_standard_period {
        table_column_names_by_id
            .entry(attribute.id.clone())
            .or_default()
            .extend([
                ("1".to_string(), "StartDate".to_string()),
                ("2".to_string(), "EndDate".to_string()),
            ]);
    }
}

#[derive(Default)]
pub(super) struct FormChildItemIndexes {
    pub(super) table_name_by_id: BTreeMap<String, String>,
    pub(super) table_column_names_by_id: BTreeMap<String, BTreeMap<String, String>>,
    pub(super) bound_table_path_by_binding_key: BTreeMap<String, String>,
    pub(super) table_column_names_by_binding_key: BTreeMap<String, BTreeMap<String, String>>,
    pub(super) data_path_by_binding_key: BTreeMap<String, String>,
    pub(super) attribute_name_by_binding_key: BTreeMap<String, String>,
    pub(super) binding_names_by_key: BTreeMap<String, BTreeSet<String>>,
    pub(super) item_name_by_id: BTreeMap<String, String>,
    pub(super) user_settings_group_id_by_table_id: BTreeMap<String, String>,
    pub(super) user_settings_group_by_table_id: BTreeMap<String, String>,
}

pub(super) fn collect_form_child_item_indexes(
    fields: &[&str],
    attributes: &[FormAttribute],
) -> FormChildItemIndexes {
    let mut indexes = FormChildItemIndexes::default();
    let attribute_names_by_id = attributes
        .iter()
        .map(|attribute| (attribute.id.clone(), attribute.name.clone()))
        .collect::<BTreeMap<_, _>>();
    for field in fields {
        collect_form_child_item_indexes_from_field(field, &attribute_names_by_id, &mut indexes);
    }
    let unresolved_binding_paths = indexes
        .binding_names_by_key
        .iter()
        .filter(|(binding_key, _)| !indexes.data_path_by_binding_key.contains_key(*binding_key))
        .filter_map(|(binding_key, names)| {
            let attribute_name = indexes.attribute_name_by_binding_key.get(binding_key)?;
            let property_name = infer_form_bound_property_name(names)?;
            Some((
                binding_key.clone(),
                format!("{attribute_name}.{property_name}"),
            ))
        })
        .collect::<Vec<_>>();
    for (binding_key, data_path) in unresolved_binding_paths {
        indexes
            .data_path_by_binding_key
            .entry(binding_key)
            .or_insert(data_path);
    }
    for attribute in attributes {
        if !attribute.columns.is_empty() {
            indexes
                .table_column_names_by_id
                .entry(attribute.id.clone())
                .or_default()
                .extend(
                    attribute
                        .columns
                        .iter()
                        .map(|column| (column.id.clone(), column.name.clone())),
                );
        }
        extend_form_attribute_special_columns(&mut indexes.table_column_names_by_id, attribute);
        if let Some(settings) = &attribute.settings {
            indexes
                .table_column_names_by_id
                .entry(attribute.id.clone())
                .or_default()
                .extend(settings.fields.iter().filter_map(|field| {
                    field
                        .item_id
                        .as_ref()
                        .map(|item_id| (item_id.clone(), field.field.clone()))
                }));
        }
    }
    indexes.user_settings_group_by_table_id = indexes
        .user_settings_group_id_by_table_id
        .iter()
        .filter_map(|(table_id, group_id)| {
            indexes
                .item_name_by_id
                .get(group_id)
                .map(|name| (table_id.clone(), name.clone()))
        })
        .collect();
    indexes
}

pub(super) fn collect_form_child_item_indexes_from_field(
    field: &str,
    attribute_names_by_id: &BTreeMap<String, String>,
    indexes: &mut FormChildItemIndexes,
) {
    let Some(fields) = split_1c_braced_fields(field.trim(), 0) else {
        return;
    };
    let wrapper = fields.first().map(|field| field.trim());
    if let Some(wrapper) = wrapper
        && form_child_item_tag(wrapper, &fields).is_some()
        && let Some(id) = form_child_item_id(&fields)
        && let Some(name) = parse_form_child_item_name(wrapper, &fields)
    {
        let tag = form_child_item_tag(wrapper, &fields).unwrap_or_default();
        indexes.item_name_by_id.insert(id.to_string(), name.clone());
        if tag == "Table" {
            indexes
                .table_name_by_id
                .insert(id.to_string(), name.clone());
            if let Some((attribute_id, table_key)) = fields
                .get(11)
                .and_then(|field| parse_form_table_binding(field))
                && let Some(attribute_name) = attribute_names_by_id.get(&attribute_id)
            {
                indexes.bound_table_path_by_binding_key.insert(
                    table_key,
                    format!("{attribute_name}.{}", indexes.table_name_by_id[id]),
                );
            }
            let mut columns = BTreeMap::new();
            collect_form_table_column_names_for_table(&fields, &mut columns);
            if !columns.is_empty() {
                indexes
                    .table_column_names_by_id
                    .insert(id.to_string(), columns);
                if let Some(attribute_id) = fields
                    .get(11)
                    .and_then(|field| parse_form_attribute_binding_id(field))
                    && let Some(columns) = indexes.table_column_names_by_id.get(id).cloned()
                {
                    indexes
                        .table_column_names_by_id
                        .insert(attribute_id, columns);
                }
            }
            if let Some(group_id) = parse_form_table_property_bag_number(&fields, "16") {
                indexes
                    .user_settings_group_id_by_table_id
                    .insert(id.to_string(), group_id);
            }
        }
        if matches!(
            tag,
            "InputField" | "LabelField" | "CheckBoxField" | "PictureField" | "TextDocumentField"
        ) {
            for binding in form_child_item_binding_fields(tag, &fields) {
                if let Some(binding_key) = parse_form_bound_data_binding_key(binding)
                    && let Some(data_path) = parse_form_bound_data_path(
                        binding,
                        &name,
                        attribute_names_by_id,
                        &indexes.table_name_by_id,
                        &indexes.table_column_names_by_id,
                        &indexes.bound_table_path_by_binding_key,
                        &indexes.table_column_names_by_binding_key,
                    )
                {
                    indexes
                        .data_path_by_binding_key
                        .entry(binding_key)
                        .or_insert(data_path);
                }
                if let Some(binding_key) = parse_form_bound_data_binding_key(binding) {
                    indexes
                        .binding_names_by_key
                        .entry(binding_key.clone())
                        .or_default()
                        .insert(name.clone());
                    if let Some(attribute_id) = parse_form_bound_attribute_id(binding)
                        && let Some(attribute_name) = attribute_names_by_id.get(&attribute_id)
                    {
                        indexes
                            .attribute_name_by_binding_key
                            .entry(binding_key)
                            .or_insert_with(|| attribute_name.clone());
                    }
                }
                if let Some((_, table_key, column_key)) =
                    parse_form_nested_table_column_binding(binding)
                {
                    let column_name = if column_key == "-2" {
                        "LineNumber".to_string()
                    } else {
                        name.clone()
                    };
                    indexes
                        .table_column_names_by_binding_key
                        .entry(table_key)
                        .or_default()
                        .insert(column_key, column_name);
                }
            }
        }
    }
    for nested in fields {
        if nested.trim_start().starts_with('{') {
            collect_form_child_item_indexes_from_field(nested, attribute_names_by_id, indexes);
        }
    }
}

pub(super) fn form_child_item_id<'a>(fields: &[&'a str]) -> Option<&'a str> {
    let identity = fields
        .get(1)
        .and_then(|field| split_1c_braced_fields(field.trim(), 0))?;
    let id = identity.first()?.trim();
    (id != "0").then_some(id)
}

pub(super) fn parse_form_child_item_pairs(
    fields: &[&str],
    main_data_path: Option<&str>,
    parent_data_path: Option<&str>,
    parent_tag: Option<&str>,
    attribute_names_by_id: &BTreeMap<String, String>,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    data_path_by_binding_key: &BTreeMap<String, String>,
    bound_table_path_by_binding_key: &BTreeMap<String, String>,
    table_column_names_by_binding_key: &BTreeMap<String, BTreeMap<String, String>>,
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<FormChildItem>> {
    let mut best = Vec::new();
    for index in 0..fields.len() {
        let Some(count) = parse_form_child_item_count(fields[index]) else {
            continue;
        };
        let mut items = Vec::new();
        let mut cursor = index + 1;
        let mut complete = true;
        for _ in 0..count {
            let Some(uuid_field) = fields.get(cursor) else {
                complete = false;
                break;
            };
            if parse_non_zero_uuid(uuid_field.trim()).is_none() {
                complete = false;
                break;
            }
            let Some(field) = fields.get(cursor + 1) else {
                complete = false;
                break;
            };
            if let Some(item) = parse_form_child_item_with_context(
                field,
                main_data_path,
                parent_data_path,
                parent_tag,
                attribute_names_by_id,
                table_name_by_id,
                table_column_names_by_id,
                data_path_by_binding_key,
                bound_table_path_by_binding_key,
                table_column_names_by_binding_key,
                commands,
                object_refs,
            ) {
                items.push(item);
            }
            cursor += 2;
        }
        if complete && items.len() == count {
            return Some(items);
        }
        if complete && items.len() > best.len() {
            best = items;
        }
    }
    if best.is_empty() { None } else { Some(best) }
}

pub(super) fn parse_form_child_item_count(value: &str) -> Option<usize> {
    let count = value.trim().parse::<usize>().ok()?;
    (1..=200).contains(&count).then_some(count)
}

pub(super) fn form_root_child_items_tail_start(fields: &[&str]) -> Option<usize> {
    for index in 0..fields.len() {
        let Some(count) = parse_form_child_item_count(fields[index]) else {
            continue;
        };
        let tail_start = index + 1 + count * 2;
        if tail_start >= fields.len() {
            continue;
        }
        let mut complete = true;
        for item_index in 0..count {
            let uuid_index = index + 1 + item_index * 2;
            let value_index = uuid_index + 1;
            if fields
                .get(uuid_index)
                .and_then(|field| parse_non_zero_uuid(field.trim()))
                .is_none()
                || fields
                    .get(value_index)
                    .and_then(|field| split_1c_braced_fields(field.trim(), 0))
                    .and_then(|item_fields| {
                        form_child_item_tag(item_fields.first()?.trim(), &item_fields)
                    })
                    .is_none()
            {
                complete = false;
                break;
            }
        }
        if complete {
            return Some(tail_start);
        }
    }
    None
}

pub(super) fn form_root_child_item_pairs_tail_start(fields: &[&str]) -> Option<usize> {
    for index in 0..fields.len() {
        let Some(count) = parse_form_child_item_count(fields[index]) else {
            continue;
        };
        let tail_start = index + 1 + count * 2;
        if tail_start >= fields.len() {
            continue;
        }
        let mut complete = true;
        for item_index in 0..count {
            let uuid_index = index + 1 + item_index * 2;
            let value_index = uuid_index + 1;
            if fields
                .get(uuid_index)
                .and_then(|field| parse_non_zero_uuid(field.trim()))
                .is_none()
                || !fields
                    .get(value_index)
                    .is_some_and(|field| field.trim().starts_with('{'))
            {
                complete = false;
                break;
            }
        }
        if complete {
            return Some(tail_start);
        }
    }
    None
}

#[cfg(test)]
pub(super) fn parse_form_child_item(
    field: &str,
    main_data_path: Option<&str>,
    parent_data_path: Option<&str>,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormChildItem> {
    parse_form_child_item_with_attrs(
        field,
        main_data_path,
        parent_data_path,
        &BTreeMap::new(),
        table_name_by_id,
        table_column_names_by_id,
        &BTreeMap::new(),
        &BTreeMap::new(),
        commands,
        object_refs,
    )
}

#[cfg(test)]
pub(super) fn parse_form_child_item_with_attrs(
    field: &str,
    main_data_path: Option<&str>,
    parent_data_path: Option<&str>,
    attribute_names_by_id: &BTreeMap<String, String>,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    bound_table_path_by_binding_key: &BTreeMap<String, String>,
    table_column_names_by_binding_key: &BTreeMap<String, BTreeMap<String, String>>,
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormChildItem> {
    parse_form_child_item_with_context(
        field,
        main_data_path,
        parent_data_path,
        None,
        attribute_names_by_id,
        table_name_by_id,
        table_column_names_by_id,
        &BTreeMap::new(),
        bound_table_path_by_binding_key,
        table_column_names_by_binding_key,
        commands,
        object_refs,
    )
}

pub(super) fn parse_form_child_item_with_context(
    field: &str,
    main_data_path: Option<&str>,
    parent_data_path: Option<&str>,
    parent_tag: Option<&str>,
    attribute_names_by_id: &BTreeMap<String, String>,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    data_path_by_binding_key: &BTreeMap<String, String>,
    bound_table_path_by_binding_key: &BTreeMap<String, String>,
    table_column_names_by_binding_key: &BTreeMap<String, BTreeMap<String, String>>,
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormChildItem> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let wrapper = fields.first()?.trim();
    let identity = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
    let id = identity.first()?.trim();
    if id == "0" {
        return None;
    }
    let tag = form_child_item_tag(wrapper, &fields)?;
    let name = parse_form_child_item_name(wrapper, &fields)?;
    let data_path = parse_form_child_item_data_path(
        tag,
        &fields,
        &name,
        id,
        main_data_path,
        parent_data_path,
        attribute_names_by_id,
        table_name_by_id,
        table_column_names_by_id,
        data_path_by_binding_key,
        bound_table_path_by_binding_key,
        table_column_names_by_binding_key,
    );
    let child_parent_data_path = data_path.as_deref().or(parent_data_path);
    let input_field_extended_options = (tag == "InputField"
        && form_input_field_layout_is_extended(&fields))
    .then(|| form_input_field_extended_options(&fields))
    .flatten();
    let picture_field_options = (tag == "PictureField")
        .then(|| parse_form_picture_field_options(&fields))
        .flatten();
    let radio_button_options = (tag == "RadioButtonField")
        .then(|| parse_form_radio_button_options(&fields))
        .flatten();
    let input_field_top_level_offset = matches!(
        tag,
        "InputField"
            | "LabelField"
            | "CheckBoxField"
            | "PictureField"
            | "RadioButtonField"
            | "TextDocumentField"
    )
    .then(|| {
        form_input_field_layout_is_extended(&fields)
            .then(|| form_input_field_top_level_offset(&fields))
    })
    .flatten()
    .unwrap_or(0);
    let mut child_items = parse_form_child_item_pairs(
        &fields,
        main_data_path,
        child_parent_data_path,
        Some(tag),
        attribute_names_by_id,
        table_name_by_id,
        table_column_names_by_id,
        data_path_by_binding_key,
        bound_table_path_by_binding_key,
        table_column_names_by_binding_key,
        commands,
        object_refs,
    )
    .unwrap_or_default();
    if tag == "Table" {
        append_form_table_service_child_items(
            &mut child_items,
            &fields,
            main_data_path,
            child_parent_data_path,
            Some(tag),
            attribute_names_by_id,
            table_name_by_id,
            table_column_names_by_id,
            data_path_by_binding_key,
            bound_table_path_by_binding_key,
            table_column_names_by_binding_key,
            commands,
            object_refs,
        );
    } else if is_form_field_direct_service_parent(tag) {
        append_form_child_items_by_tag(
            &mut child_items,
            &fields,
            &["ContextMenu"],
            main_data_path,
            child_parent_data_path,
            Some(tag),
            attribute_names_by_id,
            table_name_by_id,
            table_column_names_by_id,
            data_path_by_binding_key,
            bound_table_path_by_binding_key,
            table_column_names_by_binding_key,
            commands,
            object_refs,
        );
    } else if tag.ends_with("Addition") {
        append_form_child_items_by_tag(
            &mut child_items,
            &fields,
            &["ContextMenu"],
            main_data_path,
            child_parent_data_path,
            Some(tag),
            attribute_names_by_id,
            table_name_by_id,
            table_column_names_by_id,
            data_path_by_binding_key,
            bound_table_path_by_binding_key,
            table_column_names_by_binding_key,
            commands,
            object_refs,
        );
    }
    if tag == "TextDocumentField"
        && child_items.iter().all(|item| item.tag != "ContextMenu")
        && let Some(context_menu) = parse_form_text_document_context_menu(
            &fields,
            main_data_path,
            child_parent_data_path,
            Some(tag),
            attribute_names_by_id,
            table_name_by_id,
            table_column_names_by_id,
            data_path_by_binding_key,
            bound_table_path_by_binding_key,
            table_column_names_by_binding_key,
            commands,
            object_refs,
        )
    {
        child_items.push(context_menu);
    }
    let extended_group_options = (tag == "UsualGroup")
        .then(|| parse_form_usual_group_extended_options(&fields))
        .flatten();
    let column_group_options = (tag == "ColumnGroup")
        .then(|| parse_form_column_group_options(&fields))
        .flatten();
    let label_decoration_options = (tag == "LabelDecoration")
        .then(|| parse_form_label_decoration_options(&fields))
        .flatten();
    let label_field_options = (tag == "LabelField")
        .then(|| parse_form_label_field_options(&fields, object_refs))
        .flatten();
    let ordinary_table_layout = tag == "Table" && form_table_ordinary_layout_variant(&fields);
    let command_name = if tag == "Button" {
        fields.get(8).and_then(|field| {
            parse_form_button_command_name(field, &name, commands, object_refs, table_name_by_id)
        })
    } else {
        None
    };
    let title = parse_form_child_item_title(wrapper, &fields);
    let input_hint = if tag == "InputField" {
        parse_form_input_field_input_hint(input_field_extended_options.as_deref())
    } else {
        parse_form_child_item_input_hint(wrapper, &fields)
    };
    let input_hint = if !input_hint.is_empty() && input_hint == title {
        Vec::new()
    } else {
        input_hint
    };
    let tooltip = parse_form_child_item_tooltip(wrapper, &fields);
    Some(FormChildItem {
        tag,
        id: id.to_string(),
        name,
        autofill: if tag == "ContextMenu" {
            fields
                .get(20)
                .and_then(|field| parse_form_context_menu_autofill(field))
        } else if tag == "AutoCommandBar" {
            fields
                .get(20)
                .and_then(|field| parse_form_auto_command_bar_autofill(field))
        } else {
            None
        },
        group: if tag == "ColumnGroup" {
            column_group_options
                .as_ref()
                .and_then(|options| options.group)
        } else if tag == "UsualGroup" {
            extended_group_options
                .as_ref()
                .and_then(|options| options.group)
                .or_else(|| {
                    fields
                        .get(8)
                        .and_then(|field| parse_form_child_item_group(field))
                })
        } else if tag == "Page" {
            fields
                .get(8)
                .and_then(|field| parse_form_child_item_group(field))
                .or_else(|| {
                    fields
                        .get(9)
                        .and_then(|field| parse_form_child_item_group(field))
                })
        } else {
            None
        },
        behavior: extended_group_options
            .as_ref()
            .and_then(|options| options.behavior),
        representation: if tag == "ButtonGroup" {
            fields
                .get(20)
                .and_then(|field| parse_form_button_group_representation(field))
        } else if tag == "Pages" {
            fields
                .get(20)
                .and_then(|field| parse_form_pages_representation(field))
        } else {
            extended_group_options
                .as_ref()
                .and_then(|options| options.representation)
        },
        table_representation: if tag == "Table" {
            fields
                .get(8)
                .and_then(|field| parse_form_table_representation(field))
        } else {
            None
        },
        height_in_table_rows: if tag == "Table" {
            fields
                .get(21)
                .map(|field| field.trim().to_string())
                .filter(|value| value != "0" && value.parse::<u32>().is_ok())
        } else {
            None
        },
        row_selection_mode: if tag == "Table" {
            if ordinary_table_layout {
                None
            } else {
                fields
                    .get(24)
                    .and_then(|field| parse_form_table_row_selection_mode(field))
            }
        } else {
            None
        },
        enable_start_drag: if tag == "Table" {
            if ordinary_table_layout {
                fields
                    .get(26)
                    .and_then(|field| parse_form_child_item_show_title(field))
                    .filter(|value| *value)
            } else {
                fields
                    .get(26)
                    .and_then(|field| parse_form_child_item_show_title(field))
                    .filter(|value| *value)
            }
        } else {
            None
        },
        enable_drag: if tag == "Table" {
            if ordinary_table_layout {
                fields
                    .get(29)
                    .and_then(|field| parse_form_child_item_show_title(field))
                    .filter(|value| *value)
                    .or_else(|| parse_form_table_has_drag_events(&fields).then_some(true))
            } else {
                fields
                    .get(29)
                    .and_then(|field| parse_form_child_item_show_title(field))
                    .filter(|value| *value)
            }
        } else {
            None
        },
        file_drag_mode: if tag == "Table" {
            if ordinary_table_layout {
                fields
                    .get(30)
                    .and_then(|field| parse_form_table_file_drag_mode(field))
            } else if parse_form_table_row_input_mode(fields.get(23).copied()).is_some() {
                fields
                    .get(30)
                    .and_then(|field| parse_form_table_file_drag_mode(field))
            } else {
                None
            }
        } else if tag == "PictureDecoration" {
            parse_form_picture_decoration_file_drag_mode(&fields)
        } else if tag == "PictureField" {
            parse_form_picture_field_file_drag_mode(picture_field_options.as_deref())
        } else {
            None
        },
        auto_refresh: if tag == "Table" {
            parse_form_table_property_bag_bool(&fields, "5")
        } else {
            None
        },
        auto_refresh_period: if tag == "Table" {
            parse_form_table_property_bag_number(&fields, "6")
        } else {
            None
        },
        period: if tag == "Table" {
            parse_form_table_period(&fields)
        } else {
            None
        },
        change_row_set: if ordinary_table_layout {
            (fields.get(17).map(|field| field.trim()) == Some("0")).then_some(false)
        } else {
            None
        },
        change_row_order: if ordinary_table_layout {
            let field_16 = fields.get(16).map(|field| field.trim());
            let field_17 = fields.get(17).map(|field| field.trim());
            let field_18 = fields.get(18).map(|field| field.trim());
            let field_21 = fields.get(21).map(|field| field.trim());
            ((field_17 == Some("0") && field_18 != Some("1"))
                || (field_16 == Some("1") && field_17 == Some("1") && field_21 == Some("0")))
            .then_some(false)
        } else {
            None
        },
        command_set_excluded_commands: if tag == "Table" {
            parse_form_table_command_set_excluded_commands(&fields)
        } else {
            Vec::new()
        },
        use_alternation_row_color: if tag == "Table" {
            parse_form_table_property_bag_bool(&fields, "9")
                .filter(|value| *value)
                .or_else(|| {
                    (ordinary_table_layout
                        && fields.get(14).map(|field| field.trim()) == Some("0")
                        && fields.get(36).map(|field| field.trim()) == Some("1"))
                    .then_some(true)
                })
        } else {
            None
        },
        default_item: if tag == "Table" {
            if ordinary_table_layout {
                (fields.get(16).map(|field| field.trim()) == Some("1")).then_some(true)
            } else {
                parse_form_table_property_bag_bool(&fields, "11").filter(|value| *value)
            }
        } else {
            None
        },
        row_input_mode: if tag == "Table" {
            if ordinary_table_layout {
                parse_form_table_row_input_mode(fields.get(23).copied())
            } else {
                None
            }
        } else {
            None
        },
        show_root: if tag == "Table" && !ordinary_table_layout {
            fields
                .get(36)
                .and_then(|field| parse_form_child_item_show_title(field))
                .filter(|value| *value)
        } else {
            None
        },
        allow_root_choice: if tag == "Table" && !ordinary_table_layout {
            fields
                .get(37)
                .and_then(|field| parse_form_child_item_show_title(field))
        } else {
            None
        },
        choice_folders_and_items: if tag == "Table" {
            parse_form_table_choice_folders_and_items(&fields)
        } else if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_choice_folders_and_items(input_field_extended_options.as_deref())
        } else {
            None
        },
        restore_current_row: if tag == "Table" {
            parse_form_table_property_bag_bool(&fields, "12")
        } else {
            None
        },
        row_filter_nil: if tag == "Table" {
            if ordinary_table_layout {
                fields
                    .get(56)
                    .and_then(|field| parse_form_standalone_undefined_marker(field))
            } else {
                parse_form_table_property_bag_undefined(&fields, "10")
            }
        } else {
            None
        },
        row_picture_data_path: if tag == "Table" {
            parse_form_table_property_bag_string(&fields, "19")
                .filter(|value| !value.is_empty())
                .or_else(|| {
                    ordinary_table_layout.then(|| {
                        parse_form_ordinary_table_row_picture_data_path(
                            &fields,
                            data_path.as_deref(),
                            table_column_names_by_id,
                        )
                    })?
                })
        } else {
            None
        },
        rows_picture_ref: if tag == "Table" && ordinary_table_layout {
            fields
                .get(44)
                .and_then(|field| parse_form_child_item_picture_value(field, object_refs))
                .map(|(reference, _)| reference)
        } else {
            None
        },
        rows_picture_load_transparent: if tag == "Table" && ordinary_table_layout {
            fields
                .get(44)
                .and_then(|field| parse_form_child_item_picture_value(field, object_refs))
                .map(|(_, load_transparent)| load_transparent)
                .unwrap_or(false)
        } else {
            false
        },
        top_level_parent_nil: if tag == "Table" && !ordinary_table_layout {
            parse_form_table_property_bag_undefined(&fields, "15")
        } else {
            None
        },
        update_on_data_change: if tag == "Table" {
            parse_form_table_update_on_data_change(&fields)
        } else {
            None
        },
        user_settings_group: None,
        allow_getting_current_row_url: if tag == "Table" {
            parse_form_table_property_bag_bool(&fields, "20")
        } else {
            None
        },
        button_representation: if tag == "Button" && form_button_layout_is_extended(&fields) {
            fields
                .get(10)
                .and_then(|field| parse_form_button_representation(field))
        } else {
            None
        },
        group_horizontal_align: if tag == "Button" && form_button_layout_is_extended(&fields) {
            fields
                .get(41)
                .and_then(|field| parse_form_button_group_horizontal_align(field))
        } else if tag == "LabelDecoration" {
            label_decoration_options
                .as_ref()
                .and_then(|options| options.group_horizontal_align)
        } else {
            None
        },
        horizontal_location: if tag == "CommandBar" {
            fields
                .get(20)
                .and_then(|field| parse_form_command_bar_horizontal_location(field))
        } else if tag == "ViewStatusAddition" {
            parse_form_view_status_addition_horizontal_location(&fields)
        } else {
            None
        },
        location_in_command_bar: if tag == "Button" && form_button_layout_is_extended(&fields) {
            fields
                .get(49)
                .and_then(|field| parse_form_button_location_in_command_bar(field))
        } else {
            None
        },
        default_button: if tag == "Button" && form_button_layout_is_extended(&fields) {
            fields
                .get(11)
                .and_then(|field| parse_form_child_item_show_title(field))
        } else {
            None
        },
        scroll_on_compress: parse_form_page_scroll_on_compress(tag, &fields),
        show_title: (tag == "UsualGroup")
            .then(|| parse_form_usual_group_show_title(&fields))
            .flatten(),
        show_in_header: if tag == "ColumnGroup" {
            column_group_options
                .as_ref()
                .and_then(|options| options.show_in_header)
        } else {
            (matches!(tag, "InputField" | "LabelField" | "CheckBoxField")
                && form_input_field_layout_is_extended(&fields))
            .then(|| parse_form_child_item_show_in_header(&fields))
            .flatten()
        },
        user_visible_common: if matches!(
            tag,
            "InputField"
                | "LabelField"
                | "CheckBoxField"
                | "RadioButtonField"
                | "TextDocumentField"
        ) && form_input_field_layout_is_extended(&fields)
            && input_field_top_level_offset > 0
        {
            Some(false)
        } else {
            None
        },
        visible: if matches!(tag, "InputField" | "LabelField" | "CheckBoxField")
            && form_input_field_layout_is_extended(&fields)
        {
            parse_form_child_item_visible(&fields)
        } else {
            None
        },
        read_only: if matches!(tag, "InputField" | "TextDocumentField" | "LabelField")
            && form_input_field_layout_is_extended(&fields)
        {
            fields
                .get(14 + input_field_top_level_offset)
                .and_then(|field| parse_form_child_item_show_title(field))
        } else if ordinary_table_layout {
            fields
                .get(14)
                .and_then(|field| parse_form_child_item_show_title(field))
        } else {
            None
        },
        skip_on_input: if tag == "Table" {
            fields
                .get(12)
                .and_then(|field| parse_form_input_field_skip_on_input(field))
        } else if tag == "Button" && form_button_layout_is_extended(&fields) {
            fields
                .get(29)
                .and_then(|field| parse_form_input_field_skip_on_input(field))
        } else if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            fields
                .get(15 + input_field_top_level_offset)
                .and_then(|field| parse_form_input_field_skip_on_input(field))
        } else if tag == "LabelDecoration" {
            (fields.get(22).map(|field| field.trim()) == Some("0")).then_some(false)
        } else {
            None
        },
        title_location: if matches!(
            tag,
            "InputField"
                | "LabelField"
                | "CheckBoxField"
                | "PictureField"
                | "RadioButtonField"
                | "TextDocumentField"
        ) && form_input_field_layout_is_extended(&fields)
        {
            fields
                .get(7)
                .and_then(|field| parse_form_input_field_title_location(field))
        } else {
            None
        },
        tooltip_representation: if tag == "InputField"
            && !tooltip.is_empty()
            && ((!input_hint.is_empty()
                && input_hint
                    == parse_form_input_field_input_hint(input_field_extended_options.as_deref()))
                || parse_form_input_field_list_choice_mode(input_field_extended_options.as_deref())
                    == Some(true))
        {
            Some("Button")
        } else {
            None
        },
        edit_mode: if matches!(
            tag,
            "InputField" | "LabelField" | "CheckBoxField" | "PictureField"
        ) && form_input_field_layout_is_extended(&fields)
        {
            fields
                .get(26 + input_field_top_level_offset)
                .and_then(|field| parse_form_input_field_edit_mode(field))
        } else {
            None
        },
        horizontal_align: if matches!(tag, "InputField" | "LabelField" | "CheckBoxField")
            && form_input_field_layout_is_extended(&fields)
        {
            if matches!(tag, "InputField" | "LabelField")
                && data_path
                    .as_deref()
                    .is_some_and(is_form_numeric_label_field_data_path)
            {
                Some("Right")
            } else {
                None
            }
        } else {
            None
        },
        check_box_type: if tag == "CheckBoxField" && form_input_field_layout_is_extended(&fields) {
            parse_form_checkbox_type(&fields)
        } else {
            None
        },
        radio_button_type: if tag == "RadioButtonField" {
            parse_form_radio_button_type(radio_button_options.as_deref())
        } else {
            None
        },
        columns_count: if tag == "RadioButtonField" {
            parse_form_radio_button_columns_count(radio_button_options.as_deref())
        } else {
            None
        },
        cell_hyperlink: None,
        show_in_footer: None,
        footer_horizontal_align: None,
        hiperlink: if (tag == "LabelField"
            && label_field_options
                .as_ref()
                .is_some_and(|options| options.hyperlink_style))
            || (tag == "LabelDecoration"
                && label_decoration_options
                    .as_ref()
                    .is_some_and(|options| options.hyperlink))
        {
            Some(true)
        } else {
            None
        },
        text_color: if tag == "LabelField" {
            label_field_options
                .as_ref()
                .and_then(|options| options.text_color.clone())
        } else if matches!(tag, "LabelDecoration" | "PictureDecoration") {
            fields
                .get(14)
                .and_then(|field| parse_form_label_field_text_color(field, object_refs))
        } else {
            None
        },
        mark_required_complete: if tag == "InputField"
            && form_input_field_layout_is_extended(&fields)
        {
            parse_form_input_field_mark_required_complete(input_field_extended_options.as_deref())
        } else {
            None
        },
        auto_edit_mode: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            fields
                .get(26 + input_field_top_level_offset)
                .and_then(|field| parse_form_input_field_auto_edit_mode(field))
        } else {
            None
        },
        auto_insert_new_row: if ordinary_table_layout {
            fields
                .get(37)
                .and_then(|field| parse_form_child_item_show_title(field))
                .filter(|value| *value)
        } else {
            None
        },
        format: if tag == "LabelField" {
            label_field_options
                .as_ref()
                .map(|options| options.format.clone())
                .unwrap_or_default()
        } else if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_format(input_field_extended_options.as_deref())
        } else {
            Vec::new()
        },
        edit_format: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_edit_format(input_field_extended_options.as_deref())
        } else {
            Vec::new()
        },
        font_xml: if tag == "LabelField" {
            label_field_options
                .as_ref()
                .and_then(|options| options.font_xml.clone())
        } else if tag == "LabelDecoration" {
            label_decoration_options
                .as_ref()
                .and_then(|options| options.font_xml.clone())
        } else if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_font_xml(input_field_extended_options.as_deref())
        } else if tag == "RadioButtonField" {
            parse_form_radio_button_font_xml(radio_button_options.as_deref())
        } else {
            None
        },
        width: if tag == "UsualGroup" {
            parse_form_usual_group_width(&fields)
        } else if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_width(input_field_extended_options.as_deref())
        } else if tag == "LabelField" {
            label_field_options
                .as_ref()
                .and_then(|options| options.width.clone())
        } else if tag == "LabelDecoration" {
            fields
                .get(10)
                .map(|field| field.trim().to_string())
                .filter(|value| value != "0" && value.parse::<u32>().is_ok())
        } else if tag == "PictureDecoration" {
            fields
                .get(10)
                .map(|field| field.trim().to_string())
                .filter(|value| value != "0" && value.parse::<u32>().is_ok())
        } else if tag == "Button" && form_button_layout_is_extended(&fields) {
            fields
                .get(16)
                .map(|field| field.trim().to_string())
                .filter(|value| value != "0" && value.parse::<u32>().is_ok())
        } else {
            None
        },
        height: if tag == "TextDocumentField" && form_input_field_layout_is_extended(&fields) {
            fields
                .get(23)
                .map(|field| field.trim().to_string())
                .filter(|value| value != "0" && value.parse::<u32>().is_ok())
        } else if ordinary_table_layout {
            fields
                .get(20)
                .map(|field| field.trim().to_string())
                .filter(|value| value != "0" && value.parse::<u32>().is_ok())
        } else if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_height(input_field_extended_options.as_deref())
        } else if tag == "LabelField" {
            label_field_options
                .as_ref()
                .and_then(|options| options.height.clone())
                .filter(|value| !(parent_tag == Some("Table") && value == "1"))
        } else if tag == "PictureDecoration" {
            fields
                .get(11)
                .map(|field| field.trim().to_string())
                .filter(|value| value != "0" && value.parse::<u32>().is_ok())
        } else if tag == "Pages" {
            fields
                .get(13)
                .map(|field| field.trim().to_string())
                .filter(|value| value != "0")
        } else {
            None
        },
        auto_max_width: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_auto_max_width(input_field_extended_options.as_deref())
        } else if tag == "Button" && form_button_layout_is_extended(&fields) {
            parse_form_button_auto_max_width(fields.get(34).copied())
        } else if tag == "LabelField" {
            label_field_options
                .as_ref()
                .and_then(|options| options.auto_max_width)
        } else if tag == "LabelDecoration" {
            if fields.get(22).map(|field| field.trim()) == Some("0") {
                Some(false)
            } else if fields.get(27).map(|field| field.trim()) == Some("0")
                && fields.get(30).map(|field| field.trim()) == Some("1")
                && fields.get(31).map(|field| field.trim()) == Some("0")
            {
                Some(false)
            } else {
                None
            }
        } else if tag == "PictureDecoration" {
            fields
                .get(10)
                .map(|field| field.trim())
                .filter(|value| *value != "0" && value.parse::<u32>().is_ok())
                .map(|_| false)
        } else if ordinary_table_layout {
            match fields.get(53).map(|field| field.trim()) {
                Some("0") if fields.get(54).map(|field| field.trim()) == Some("2") => Some(false),
                _ => None,
            }
        } else {
            None
        },
        max_width: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_max_width(input_field_extended_options.as_deref())
        } else if tag == "Button" && form_button_layout_is_extended(&fields) {
            parse_form_button_max_width(fields.get(35).copied())
        } else if tag == "LabelField" {
            label_field_options
                .as_ref()
                .and_then(|options| options.max_width.clone())
        } else {
            None
        },
        auto_max_height: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_auto_max_height(input_field_extended_options.as_deref())
        } else if tag == "PictureDecoration" {
            fields
                .get(11)
                .map(|field| field.trim())
                .filter(|value| *value != "0" && value.parse::<u32>().is_ok())
                .map(|_| false)
        } else if ordinary_table_layout {
            match fields.get(53).map(|field| field.trim()) {
                Some("0") if fields.get(20).map(|field| field.trim()) != Some("0") => Some(false),
                _ => None,
            }
        } else {
            None
        },
        max_height: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_max_height(input_field_extended_options.as_deref())
        } else {
            None
        },
        horizontal_stretch: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_horizontal_stretch(input_field_extended_options.as_deref())
        } else if tag == "UsualGroup" {
            extended_group_options
                .as_ref()
                .and_then(|options| options.horizontal_stretch)
        } else {
            None
        },
        vertical_stretch: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_vertical_stretch(input_field_extended_options.as_deref())
        } else if tag == "UsualGroup" {
            parse_form_usual_group_vertical_stretch(&fields)
        } else {
            None
        },
        password_mode: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_password_mode(input_field_extended_options.as_deref())
        } else {
            None
        },
        multi_line: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_multi_line(input_field_extended_options.as_deref())
        } else {
            None
        },
        wrap: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_wrap(input_field_extended_options.as_deref())
        } else {
            None
        },
        text_edit: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_text_edit(input_field_extended_options.as_deref())
        } else {
            None
        },
        auto_cell_height: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_auto_cell_height(&fields)
        } else {
            None
        },
        drop_list_button: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_drop_list_button(input_field_extended_options.as_deref())
        } else {
            None
        },
        clear_button: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_clear_button(input_field_extended_options.as_deref())
        } else {
            None
        },
        open_button: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_open_button(input_field_extended_options.as_deref())
        } else {
            None
        },
        create_button: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_create_button(input_field_extended_options.as_deref())
        } else {
            None
        },
        choice_button: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_choice_button(input_field_extended_options.as_deref())
        } else {
            None
        },
        choice_list_button: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_choice_list_button(input_field_extended_options.as_deref())
        } else {
            None
        },
        spin_button: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_spin_button(input_field_extended_options.as_deref())
        } else {
            None
        },
        list_choice_mode: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_list_choice_mode(input_field_extended_options.as_deref())
        } else {
            None
        },
        quick_choice: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_quick_choice(input_field_extended_options.as_deref())
        } else {
            None
        },
        choose_type: if tag == "InputField" && form_input_field_layout_is_extended(&fields) {
            parse_form_input_field_choose_type(input_field_extended_options.as_deref())
        } else {
            None
        },
        auto_choice_incomplete: if tag == "InputField"
            && form_input_field_layout_is_extended(&fields)
        {
            parse_form_input_field_auto_choice_incomplete(input_field_extended_options.as_deref())
        } else {
            None
        },
        auto_mark_incomplete: if tag == "InputField" && form_input_field_layout_is_extended(&fields)
        {
            parse_form_input_field_auto_mark_incomplete(input_field_extended_options.as_deref())
        } else if ordinary_table_layout
            && fields.get(53).map(|field| field.trim()) == Some("1")
            && fields.get(54).map(|field| field.trim()) == Some("2")
        {
            Some(true)
        } else {
            None
        },
        choice_button_representation: if tag == "InputField"
            && form_input_field_layout_is_extended(&fields)
        {
            parse_form_input_field_choice_button_representation(
                input_field_extended_options.as_deref(),
            )
        } else {
            None
        },
        item_type: if tag == "Button" && form_button_layout_is_extended(&fields) {
            fields
                .get(4)
                .and_then(|field| parse_form_extended_button_type(field))
        } else if tag == "Button" {
            fields
                .get(7)
                .and_then(|field| parse_form_button_type(field))
        } else if tag.ends_with("Addition") {
            fields
                .get(5)
                .and_then(|field| parse_form_search_addition_type(field))
        } else {
            None
        },
        addition_source_item: if tag.ends_with("Addition") {
            fields
                .get(19)
                .and_then(|field| parse_form_search_addition_source_item(field, table_name_by_id))
        } else {
            None
        },
        picture_ref: if tag == "Button" && form_button_layout_is_extended(&fields) {
            fields
                .get(25)
                .and_then(|field| parse_form_child_item_picture_value(field, object_refs))
                .map(|(reference, _)| reference)
        } else if tag == "PictureField" {
            parse_form_picture_field_value(picture_field_options.as_deref(), object_refs)
                .map(|(reference, _)| reference)
        } else if tag == "PictureDecoration" {
            parse_form_picture_decoration_picture_value(&fields, object_refs)
                .map(|(reference, _)| reference)
        } else if tag == "Popup" {
            fields
                .get(20)
                .and_then(|field| parse_form_popup_picture_value(field, object_refs))
                .map(|(reference, _)| reference)
        } else {
            None
        },
        picture_load_transparent: if tag == "Button" && form_button_layout_is_extended(&fields) {
            fields
                .get(25)
                .and_then(|field| parse_form_child_item_picture_value(field, object_refs))
                .map(|(_, load_transparent)| load_transparent)
                .unwrap_or(false)
        } else if tag == "PictureField" {
            parse_form_picture_field_value(picture_field_options.as_deref(), object_refs)
                .map(|(_, load_transparent)| load_transparent)
                .unwrap_or(false)
        } else if tag == "PictureDecoration" {
            parse_form_picture_decoration_picture_value(&fields, object_refs)
                .map(|(_, load_transparent)| load_transparent)
                .unwrap_or(false)
        } else if tag == "Popup" {
            fields
                .get(20)
                .and_then(|field| parse_form_popup_picture_value(field, object_refs))
                .map(|(_, load_transparent)| load_transparent)
                .unwrap_or(false)
        } else {
            false
        },
        picture_size: if tag == "PictureDecoration" {
            parse_form_picture_decoration_picture_size(&fields)
        } else {
            None
        },
        picture_file_name: if tag == "PictureDecoration" {
            parse_form_picture_decoration_file_name(&fields)
        } else {
            None
        },
        title,
        tooltip,
        input_hint,
        choice_list: if tag == "RadioButtonField" {
            parse_form_radio_button_choice_list(radio_button_options.as_deref(), object_refs)
        } else if tag == "InputField" {
            parse_form_input_field_choice_list(input_field_extended_options.as_deref(), object_refs)
        } else {
            Vec::new()
        },
        extended_tooltip: parse_form_child_item_extended_tooltip(&fields),
        events: {
            let mut events = parse_form_child_item_event_fields(&fields);
            if tag == "InputField" {
                if let Some(extended_options) = input_field_extended_options.as_deref() {
                    append_unique_form_body_events(
                        &mut events,
                        parse_form_nested_child_item_event_records(extended_options),
                    );
                }
            }
            if matches!(tag, "LabelDecoration" | "PictureDecoration")
                && let Some(options) = fields
                    .get(18)
                    .and_then(|field| split_1c_braced_fields(field.trim(), 0))
            {
                append_unique_form_body_events(
                    &mut events,
                    parse_form_nested_child_item_event_records(&options),
                );
            }
            events
        },
        data_path,
        command_name,
        command_source: if tag == "CommandBar" {
            parse_form_command_bar_source(&fields)
        } else if tag == "ButtonGroup" {
            parse_form_button_group_command_source(&fields)
        } else {
            None
        },
        child_items,
    })
}

pub(super) fn is_form_field_direct_service_parent(tag: &str) -> bool {
    matches!(
        tag,
        "InputField"
            | "LabelField"
            | "LabelDecoration"
            | "PictureField"
            | "PictureDecoration"
            | "CheckBoxField"
            | "RadioButtonField"
            | "TextDocumentField"
            | "SearchStringAddition"
            | "ViewStatusAddition"
            | "SearchControlAddition"
    )
}

pub(super) fn append_form_table_service_child_items(
    child_items: &mut Vec<FormChildItem>,
    fields: &[&str],
    main_data_path: Option<&str>,
    parent_data_path: Option<&str>,
    parent_tag: Option<&str>,
    attribute_names_by_id: &BTreeMap<String, String>,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    data_path_by_binding_key: &BTreeMap<String, String>,
    bound_table_path_by_binding_key: &BTreeMap<String, String>,
    table_column_names_by_binding_key: &BTreeMap<String, BTreeMap<String, String>>,
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
) {
    append_form_child_items_by_tag(
        child_items,
        fields,
        &[
            "ContextMenu",
            "AutoCommandBar",
            "SearchStringAddition",
            "ViewStatusAddition",
            "SearchControlAddition",
        ],
        main_data_path,
        parent_data_path,
        parent_tag,
        attribute_names_by_id,
        table_name_by_id,
        table_column_names_by_id,
        data_path_by_binding_key,
        bound_table_path_by_binding_key,
        table_column_names_by_binding_key,
        commands,
        object_refs,
    );
}

pub(super) fn append_form_child_items_by_tag(
    child_items: &mut Vec<FormChildItem>,
    fields: &[&str],
    tags: &[&str],
    main_data_path: Option<&str>,
    parent_data_path: Option<&str>,
    parent_tag: Option<&str>,
    attribute_names_by_id: &BTreeMap<String, String>,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    data_path_by_binding_key: &BTreeMap<String, String>,
    bound_table_path_by_binding_key: &BTreeMap<String, String>,
    table_column_names_by_binding_key: &BTreeMap<String, BTreeMap<String, String>>,
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
) {
    for field in fields {
        let Some(item) = parse_form_child_item_with_context(
            field,
            main_data_path,
            parent_data_path,
            parent_tag,
            attribute_names_by_id,
            table_name_by_id,
            table_column_names_by_id,
            data_path_by_binding_key,
            bound_table_path_by_binding_key,
            table_column_names_by_binding_key,
            commands,
            object_refs,
        ) else {
            continue;
        };
        if !tags.contains(&item.tag) {
            continue;
        }
        if child_items.iter().any(|existing| {
            existing.tag == item.tag && (existing.id == item.id || existing.name == item.name)
        }) {
            continue;
        }
        child_items.push(item);
    }
}

pub(super) fn is_form_table_service_child_item(tag: &str) -> bool {
    matches!(
        tag,
        "ContextMenu"
            | "AutoCommandBar"
            | "SearchStringAddition"
            | "ViewStatusAddition"
            | "SearchControlAddition"
    )
}

pub(super) fn parse_form_text_document_context_menu(
    fields: &[&str],
    main_data_path: Option<&str>,
    parent_data_path: Option<&str>,
    parent_tag: Option<&str>,
    attribute_names_by_id: &BTreeMap<String, String>,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    data_path_by_binding_key: &BTreeMap<String, String>,
    bound_table_path_by_binding_key: &BTreeMap<String, String>,
    table_column_names_by_binding_key: &BTreeMap<String, BTreeMap<String, String>>,
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormChildItem> {
    if fields.get(41).map(|field| field.trim()) != Some("1") {
        return None;
    }
    parse_form_child_item_with_context(
        fields.get(42)?,
        main_data_path,
        parent_data_path,
        parent_tag,
        attribute_names_by_id,
        table_name_by_id,
        table_column_names_by_id,
        data_path_by_binding_key,
        bound_table_path_by_binding_key,
        table_column_names_by_binding_key,
        commands,
        object_refs,
    )
}

pub(super) struct FormUsualGroupExtendedOptions {
    pub(super) group: Option<&'static str>,
    pub(super) behavior: Option<&'static str>,
    pub(super) representation: Option<&'static str>,
    pub(super) horizontal_stretch: Option<bool>,
}

pub(super) struct FormColumnGroupOptions {
    pub(super) group: Option<&'static str>,
    pub(super) show_in_header: Option<bool>,
}

pub(super) struct FormLabelFieldOptions {
    pub(super) width: Option<String>,
    pub(super) height: Option<String>,
    pub(super) auto_max_width: Option<bool>,
    pub(super) max_width: Option<String>,
    pub(super) format: Vec<(String, String)>,
    pub(super) font_xml: Option<String>,
    pub(super) text_color: Option<String>,
    pub(super) hyperlink_style: bool,
}

pub(super) struct FormLabelDecorationOptions {
    pub(super) hyperlink: bool,
    pub(super) font_xml: Option<String>,
    pub(super) group_horizontal_align: Option<&'static str>,
}

pub(super) fn parse_form_column_group_options(fields: &[&str]) -> Option<FormColumnGroupOptions> {
    let options = split_1c_braced_fields(fields.get(20)?.trim(), 0)?;
    if !matches!(options.first()?.trim(), "2" | "5") {
        return None;
    }
    Some(FormColumnGroupOptions {
        group: options
            .get(1)
            .and_then(|field| parse_form_column_group_group(field)),
        show_in_header: options
            .get(3)
            .and_then(|field| parse_form_child_item_show_title(field)),
    })
}

pub(super) fn parse_form_standalone_undefined_marker(field: &str) -> Option<bool> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    (fields.len() == 1
        && fields
            .first()
            .and_then(|field| parse_1c_string(field))
            .is_some_and(|marker| marker == "U"))
    .then_some(true)
}

pub(super) fn form_table_ordinary_layout_variant(fields: &[&str]) -> bool {
    fields.get(43).map(|field| field.trim()) == Some("{0}")
        || (fields.get(55).map(|field| field.trim()) == Some("13")
            && fields
                .get(56)
                .and_then(|field| parse_form_standalone_undefined_marker(field.trim()))
                == Some(true)
            && fields.get(57).map(|field| field.trim()) == Some("19"))
}

pub(super) fn form_table_has_hierarchical_navigation(item: &FormChildItem) -> bool {
    item.tag == "Table"
        && (item.show_root == Some(true)
            || item.allow_root_choice.is_some()
            || item.top_level_parent_nil == Some(true))
}

pub(super) fn parse_form_label_field_options(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormLabelFieldOptions> {
    let options_index = 39 + form_input_field_top_level_offset(fields);
    let options = split_1c_braced_fields(fields.get(options_index)?.trim(), 0)?;
    if options.first()?.trim() != "11" {
        return None;
    }
    let width = options
        .get(1)
        .map(|field| field.trim())
        .filter(|value| *value != "0" && value.parse::<u32>().is_ok())
        .map(str::to_string);
    let max_width = options
        .get(7)
        .map(|field| field.trim())
        .filter(|value| *value != "0" && value.parse::<u32>().is_ok())
        .map(str::to_string);
    let text_color = options
        .get(8)
        .and_then(|field| parse_form_label_field_text_color(field, object_refs));
    let hyperlink_style = width.is_none() && max_width.is_some();
    Some(FormLabelFieldOptions {
        width: if hyperlink_style { None } else { width },
        height: if hyperlink_style {
            None
        } else {
            options
                .get(15)
                .map(|field| field.trim())
                .filter(|value| *value != "0" && value.parse::<u32>().is_ok())
                .map(str::to_string)
        },
        auto_max_width: if hyperlink_style {
            None
        } else {
            match options.get(2).map(|field| field.trim()) {
                Some("0")
                    if options.get(1).map(|field| field.trim()) != Some("0")
                        || (fields.get(7).map(|field| field.trim()) == Some("0")
                            && options.get(15).map(|field| field.trim()) == Some("0")
                            && options.get(16).map(|field| field.trim()) == Some("0")) =>
                {
                    Some(false)
                }
                _ => None,
            }
        },
        max_width: if hyperlink_style { None } else { max_width },
        format: options
            .get(6)
            .map(|field| parse_form_localized_strings(field))
            .unwrap_or_default(),
        font_xml: options
            .get(10)
            .and_then(|field| parse_form_font_tuple_xml(field)),
        text_color: if hyperlink_style { None } else { text_color },
        hyperlink_style,
    })
}

pub(super) fn parse_form_label_field_text_color(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("3") {
        return None;
    }
    match fields.get(1).map(|value| value.trim()) {
        Some("3") => {
            if let Some(reference) = parse_moxel_style_ref_slot(field, object_refs).flatten() {
                return Some(reference);
            }
            parse_form_object_reference(fields.get(2)?.trim(), object_refs).map(|reference| {
                reference
                    .strip_prefix("StyleItem.")
                    .map(|name| format!("style:{name}"))
                    .unwrap_or(reference)
            })
        }
        Some("2") | Some("0") => parse_moxel_style_ref_slot(field, object_refs).flatten(),
        _ => None,
    }
}

pub(super) fn parse_form_label_decoration_options(
    fields: &[&str],
) -> Option<FormLabelDecorationOptions> {
    let options = split_1c_braced_fields(fields.get(18)?.trim(), 0)?;
    if options.first()?.trim() != "5" {
        return None;
    }
    Some(FormLabelDecorationOptions {
        hyperlink: options.get(1).map(|field| field.trim()) == Some("1"),
        font_xml: fields
            .get(15)
            .and_then(|field| parse_form_font_tuple_xml(field)),
        group_horizontal_align: fields
            .get(32)
            .and_then(|field| parse_form_label_decoration_group_horizontal_align(field)),
    })
}

pub(super) fn parse_form_usual_group_extended_options(
    fields: &[&str],
) -> Option<FormUsualGroupExtendedOptions> {
    let options = split_1c_braced_fields(fields.get(20)?.trim(), 0)?;
    match options.first()?.trim() {
        "29" => Some(FormUsualGroupExtendedOptions {
            group: parse_form_usual_group_property_bag_group(&options),
            behavior: parse_form_usual_group_property_bag_behavior(fields, &options),
            representation: options
                .get(3)
                .and_then(|field| parse_form_child_item_representation(field)),
            horizontal_stretch: parse_form_usual_group_horizontal_stretch(fields),
        }),
        "38" => {
            let group = parse_form_extended_group(
                options.get(1)?.trim(),
                options.get(22).map(|value| value.trim()),
                options.get(27).map(|value| value.trim()),
                options.get(36).map(|value| value.trim()),
            );
            let representation = options
                .get(3)
                .and_then(|field| parse_form_child_item_representation(field));
            let behavior = parse_form_extended_group_behavior(
                fields.get(16).map(|value| value.trim()),
                fields.get(17).map(|value| value.trim()),
                options.get(10).map(|value| value.trim()),
                options.get(11).map(|value| value.trim()),
                options.get(24).map(|value| value.trim()),
                options.get(28).map(|value| value.trim()),
            );
            Some(FormUsualGroupExtendedOptions {
                group,
                behavior,
                representation,
                horizontal_stretch: None,
            })
        }
        _ => None,
    }
}

pub(super) fn parse_form_usual_group_property_bag_group(options: &[&str]) -> Option<&'static str> {
    match options.get(27).map(|value| value.trim()) {
        Some("1") => Some("Horizontal"),
        Some("3") => Some("AlwaysHorizontal"),
        Some("4") => Some("HorizontalIfPossible"),
        _ => match options.get(1).map(|value| value.trim()) {
            Some("0") => Some("Vertical"),
            _ => None,
        },
    }
}

pub(super) fn parse_form_usual_group_property_bag_behavior(
    fields: &[&str],
    options: &[&str],
) -> Option<&'static str> {
    if options.get(13).map(|value| value.trim()) == Some("1") {
        return Some("Usual");
    }
    if fields.get(15).map(|value| value.trim()) == Some("2")
        && options.get(13).map(|value| value.trim()) == Some("1")
    {
        return Some("Usual");
    }
    None
}

pub(super) fn parse_form_usual_group_horizontal_stretch(fields: &[&str]) -> Option<bool> {
    match fields.get(14).map(|value| value.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_usual_group_width(fields: &[&str]) -> Option<String> {
    let value = fields.get(12)?.trim();
    (value != "0" && value.parse::<u32>().is_ok()).then(|| value.to_string())
}

pub(super) fn parse_form_usual_group_vertical_stretch(fields: &[&str]) -> Option<bool> {
    match fields.get(15).map(|value| value.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_extended_group(
    orientation: &str,
    group_code: Option<&str>,
    layout_code: Option<&str>,
    mirror_code: Option<&str>,
) -> Option<&'static str> {
    match (orientation, group_code, layout_code, mirror_code) {
        ("0", Some("0"), Some("0"), Some("0")) => Some("Vertical"),
        ("1", Some("1"), Some("1"), Some("1")) => Some("Horizontal"),
        ("1", Some("1"), Some("3"), Some("3")) => Some("AlwaysHorizontal"),
        ("1", Some("2"), Some("2"), Some("2")) => Some("HorizontalIfPossible"),
        _ => None,
    }
}

pub(super) fn parse_form_child_item_representation(field: &str) -> Option<&'static str> {
    match field.trim() {
        "0" => Some("None"),
        "1" => Some("StrongSeparation"),
        "2" => Some("WeakSeparation"),
        "3" => Some("NormalSeparation"),
        _ => None,
    }
}

pub(super) fn parse_form_extended_group_behavior(
    style_field: Option<&str>,
    size_field: Option<&str>,
    flag10: Option<&str>,
    flag11: Option<&str>,
    flag24: Option<&str>,
    flag28: Option<&str>,
) -> Option<&'static str> {
    if style_field.is_some_and(|field| field.starts_with("{4,3"))
        && size_field.is_some_and(|field| field.starts_with("{8,2"))
        && flag10 == Some("1")
        && flag11 == Some("0")
        && flag24 == Some("2")
        && flag28 == Some("2")
    {
        return Some("PopUp");
    }
    if flag10 == Some("1") && flag11 == Some("1") && flag24 == Some("1") && flag28 == Some("1") {
        return Some("Collapsible");
    }
    if flag10 == Some("0") && flag11 == Some("0") && flag24 == Some("0") && flag28 == Some("0") {
        return Some("Usual");
    }
    None
}

pub(super) fn parse_form_child_item_group(field: &str) -> Option<&'static str> {
    match field.trim() {
        "0" => Some("Vertical"),
        "1" => Some("Horizontal"),
        "2" => Some("AlwaysHorizontal"),
        "3" => Some("HorizontalIfPossible"),
        "4" => Some("InCell"),
        _ => None,
    }
}

pub(super) fn parse_form_column_group_group(field: &str) -> Option<&'static str> {
    match field.trim() {
        "0" => Some("Horizontal"),
        "2" => Some("InCell"),
        _ => None,
    }
}

pub(super) fn parse_form_child_item_show_title(field: &str) -> Option<bool> {
    match field.trim() {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_usual_group_show_title(fields: &[&str]) -> Option<bool> {
    let show_title = fields
        .get(9)
        .and_then(|field| parse_form_child_item_show_title(field))?;
    if show_title {
        return Some(true);
    }
    let options = fields
        .get(20)
        .and_then(|field| split_1c_braced_fields(field.trim(), 0));
    match options
        .as_deref()
        .and_then(|values| values.first())
        .map(|value| value.trim())
    {
        Some("29")
            if options
                .as_deref()
                .and_then(|values| values.get(4))
                .map(|value| value.trim())
                == Some("1") =>
        {
            None
        }
        _ => Some(false),
    }
}

pub(super) fn parse_form_page_scroll_on_compress(tag: &str, fields: &[&str]) -> Option<bool> {
    if tag != "Page" {
        return None;
    }
    fields
        .get(8)
        .filter(|field| field.trim_start().starts_with('{'))
        .and_then(|_| fields.get(11))
        .and_then(|field| parse_form_child_item_show_title(field))
}

pub(super) fn parse_form_child_item_show_in_header(fields: &[&str]) -> Option<bool> {
    let index = 20 + form_input_field_top_level_offset(fields);
    match fields.get(index).map(|field| field.trim())? {
        "0" => Some(false),
        _ => None,
    }
}

pub(super) fn parse_form_child_item_visible(fields: &[&str]) -> Option<bool> {
    let index = 43 + form_input_field_top_level_offset(fields);
    match fields.get(index).map(|field| field.trim())? {
        "0" => Some(false),
        _ => None,
    }
}

pub(super) fn form_button_layout_is_extended(fields: &[&str]) -> bool {
    fields.len() > 20
}

pub(super) fn form_input_field_layout_is_extended(fields: &[&str]) -> bool {
    fields.len() > 20
}

pub(super) fn form_input_field_extended_options<'a>(fields: &'a [&'a str]) -> Option<Vec<&'a str>> {
    fields.iter().skip(39).find_map(|field| {
        let options = split_1c_braced_fields(field.trim(), 0)?;
        matches!(options.first().copied(), Some("36" | "38")).then_some(options)
    })
}

pub(super) fn parse_form_radio_button_options<'a>(fields: &'a [&'a str]) -> Option<Vec<&'a str>> {
    fields.iter().skip(39).find_map(|field| {
        let options = split_1c_braced_fields(field.trim(), 0)?;
        (options.first().map(|value| value.trim()) == Some("8")).then_some(options)
    })
}

pub(super) fn form_input_field_top_level_offset(fields: &[&str]) -> usize {
    fields
        .get(6)
        .and_then(|field| parse_1c_quoted_string_with_len(field.trim()))
        .filter(|(value, _)| !value.is_empty())
        .map(|_| 0)
        .unwrap_or(1)
}

pub(super) fn parse_form_button_type(field: &str) -> Option<&'static str> {
    match field.trim() {
        "0" => Some("UsualButton"),
        "1" => Some("CommandBarButton"),
        "2" => Some("Hyperlink"),
        _ => None,
    }
}

pub(super) fn parse_form_extended_button_type(field: &str) -> Option<&'static str> {
    match field.trim() {
        "0" => Some("CommandBarButton"),
        "1" => Some("UsualButton"),
        "2" => Some("Hyperlink"),
        _ => None,
    }
}

pub(super) fn parse_form_button_representation(field: &str) -> Option<&'static str> {
    match field.trim() {
        "0" => Some("Text"),
        "1" => Some("Picture"),
        "2" => Some("PictureAndText"),
        "3" => Some("None"),
        _ => None,
    }
}

pub(super) fn parse_form_button_group_horizontal_align(field: &str) -> Option<&'static str> {
    match field.trim() {
        "0" => Some("Left"),
        "1" => Some("Center"),
        "2" => Some("Right"),
        _ => None,
    }
}

pub(super) fn parse_form_label_decoration_group_horizontal_align(
    field: &str,
) -> Option<&'static str> {
    parse_form_button_group_horizontal_align(field)
}

pub(super) fn parse_form_button_group_representation(field: &str) -> Option<&'static str> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    match (
        fields.first().map(|value| value.trim()),
        fields.get(2).map(|value| value.trim()),
        fields.get(3).map(|value| value.trim()),
    ) {
        (Some("2"), Some("2"), Some("2")) => Some("Compact"),
        _ => None,
    }
}

pub(super) fn parse_form_pages_representation(field: &str) -> Option<&'static str> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    match fields.get(1).map(|value| value.trim()) {
        Some("0") => Some("None"),
        Some("1") => Some("TabsOnTop"),
        Some("2") => Some("TabsOnBottom"),
        Some("5") => Some("Swipe"),
        _ => None,
    }
}

pub(super) fn parse_form_button_location_in_command_bar(field: &str) -> Option<&'static str> {
    match field.trim() {
        "1" => Some("InAdditionalSubmenu"),
        "2" => Some("InCommandBar"),
        "3" => Some("InCommandBarAndInAdditionalSubmenu"),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_title_location(field: &str) -> Option<&'static str> {
    match field.trim() {
        "0" => Some("None"),
        "2" => Some("Left"),
        "3" => Some("Top"),
        "4" => Some("Right"),
        _ => None,
    }
}

pub(super) fn is_form_numeric_label_field_data_path(path: &str) -> bool {
    matches!(path, "СоставЗаказа.Количество" | "СоставЗаказа.Сумма")
}

pub(super) fn parse_form_table_has_drag_events(fields: &[&str]) -> bool {
    parse_form_child_item_event_fields(fields)
        .into_iter()
        .any(|event| matches!(event.name.as_str(), "Drag" | "DragCheck"))
}

pub(super) fn parse_form_view_status_addition_horizontal_location(
    fields: &[&str],
) -> Option<&'static str> {
    let options = split_1c_braced_fields(fields.get(13)?.trim(), 0)?;
    (options.get(11).map(|field| field.trim()) == Some("0")).then_some("Left")
}

pub(super) fn parse_form_command_bar_horizontal_location(field: &str) -> Option<&'static str> {
    parse_form_auto_command_bar_horizontal_align(field)
}

pub(super) fn parse_form_input_field_edit_mode(field: &str) -> Option<&'static str> {
    match field.trim() {
        "0" => Some("Directly"),
        "2" => Some("EnterOnInput"),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_auto_edit_mode(field: &str) -> Option<bool> {
    match field.trim() {
        "2" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_format(
    extended_options: Option<&[&str]>,
) -> Vec<(String, String)> {
    extended_options
        .and_then(|options| options.get(29))
        .map(|field| parse_form_localized_strings(field))
        .unwrap_or_default()
}

pub(super) fn parse_form_input_field_edit_format(
    extended_options: Option<&[&str]>,
) -> Vec<(String, String)> {
    extended_options
        .and_then(|options| options.get(30))
        .map(|field| parse_form_localized_strings(field))
        .unwrap_or_default()
}

pub(super) fn parse_form_input_field_font_xml(extended_options: Option<&[&str]>) -> Option<String> {
    extended_options
        .and_then(|options| options.get(40))
        .and_then(|field| parse_form_font_tuple_xml(field))
}

pub(super) fn parse_form_font_tuple_xml(field: &str) -> Option<String> {
    let trimmed = field.trim();
    let fields = split_1c_braced_fields(trimmed, 0)?;
    if fields.first()?.trim() != "7" {
        return None;
    }
    match fields.get(1).map(|value| value.trim()) {
        Some("1" | "2") => {}
        _ => return None,
    }
    let wrapped = format!("{{\"#\",00000000-0000-0000-0000-000000000000,1,{trimmed},0}}");
    let value_xml = parse_style_font_value_xml(&wrapped);
    let attrs = value_xml
        .strip_prefix(r#"<Value xsi:type="v8ui:Font""#)?
        .strip_suffix("/>")?;
    Some(format!("<Font{attrs}/>"))
}

pub(super) fn parse_form_input_field_skip_on_input(field: &str) -> Option<bool> {
    match field.trim() {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_button_auto_max_width(field: Option<&str>) -> Option<bool> {
    match field.map(|value| value.trim()) {
        Some("0") => Some(false),
        _ => None,
    }
}

pub(super) fn parse_form_button_max_width(field: Option<&str>) -> Option<String> {
    let value = field?.trim();
    (value != "0" && value.parse::<u32>().is_ok()).then(|| value.to_string())
}

pub(super) fn parse_form_input_field_width(extended_options: Option<&[&str]>) -> Option<String> {
    let value = extended_options?.get(2)?.trim();
    (value != "0" && value.parse::<u32>().is_ok()).then(|| value.to_string())
}

pub(super) fn parse_form_input_field_height(extended_options: Option<&[&str]>) -> Option<String> {
    let value = extended_options?.get(3)?.trim();
    (value != "0" && value.parse::<u32>().is_ok()).then(|| value.to_string())
}

pub(super) fn parse_form_input_field_auto_max_width(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?.get(49).map(|field| field.trim())? {
        "0" => Some(false),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_max_width(
    extended_options: Option<&[&str]>,
) -> Option<String> {
    let value = extended_options?.get(50)?.trim();
    (value != "0" && value.parse::<u32>().is_ok()).then(|| value.to_string())
}

pub(super) fn parse_form_input_field_auto_max_height(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?.get(52).map(|field| field.trim())? {
        "0" => Some(false),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_max_height(
    extended_options: Option<&[&str]>,
) -> Option<String> {
    let value = extended_options?.get(53)?.trim();
    (value != "0" && value.parse::<u32>().is_ok()).then(|| value.to_string())
}

pub(super) fn parse_form_input_field_horizontal_stretch(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?.get(4).map(|field| field.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_vertical_stretch(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?.get(5).map(|field| field.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_password_mode(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?.get(7).map(|field| field.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_multi_line(extended_options: Option<&[&str]>) -> Option<bool> {
    match extended_options?.get(8).map(|field| field.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_wrap(extended_options: Option<&[&str]>) -> Option<bool> {
    match extended_options?.get(6).map(|field| field.trim())? {
        "0" => Some(false),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_text_edit(extended_options: Option<&[&str]>) -> Option<bool> {
    match extended_options?.get(41).map(|field| field.trim())? {
        "0" => Some(false),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_auto_cell_height(fields: &[&str]) -> Option<bool> {
    let index = 28 + form_input_field_top_level_offset(fields);
    match fields.get(index).map(|field| field.trim())? {
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_drop_list_button(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?.get(47).map(|field| field.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_clear_button(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?.get(13).map(|field| field.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_open_button(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?.get(15).map(|field| field.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_create_button(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?.get(45).map(|field| field.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_choice_button(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?.get(12).map(|field| field.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_choice_list_button(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?.get(11).map(|field| field.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_spin_button(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?.get(14).map(|field| field.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_list_choice_mode(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?.get(19).map(|field| field.trim())? {
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_quick_choice(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?.get(23).map(|field| field.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_choose_type(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?.get(32).map(|field| field.trim())? {
        "0" => Some(false),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_auto_choice_incomplete(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?.get(28).map(|field| field.trim())? {
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_auto_mark_incomplete(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    match extended_options?.get(31).map(|field| field.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_mark_required_complete(
    extended_options: Option<&[&str]>,
) -> Option<bool> {
    parse_form_input_field_auto_mark_incomplete(extended_options)
}

pub(super) fn parse_form_input_field_choice_button_representation(
    extended_options: Option<&[&str]>,
) -> Option<&'static str> {
    match extended_options?.get(46).map(|field| field.trim())? {
        "1" => Some("ShowInDropList"),
        "2" => Some("ShowInDropListAndInInputField"),
        "3" => Some("ShowInInputField"),
        _ => None,
    }
}

pub(super) fn parse_form_input_field_choice_folders_and_items(
    extended_options: Option<&[&str]>,
) -> Option<&'static str> {
    metadata_choice_folders_and_items_xml(extended_options?.get(24)?.trim())
}

pub(super) fn parse_form_checkbox_type(fields: &[&str]) -> Option<&'static str> {
    let options_index = 39 + form_input_field_top_level_offset(fields);
    let options = split_1c_braced_fields(fields.get(options_index)?.trim(), 0)?;
    if options.first()?.trim() != "11" {
        return None;
    }
    match options.get(11).map(|field| field.trim())? {
        "2" => Some("Auto"),
        _ => None,
    }
}

pub(super) fn parse_form_radio_button_type(
    extended_options: Option<&[&str]>,
) -> Option<&'static str> {
    match extended_options?.get(7).map(|field| field.trim())? {
        "0" => Some("Auto"),
        "1" => Some("RadioButtons"),
        "2" => Some("Tumbler"),
        _ => None,
    }
}

pub(super) fn parse_form_radio_button_columns_count(
    extended_options: Option<&[&str]>,
) -> Option<u32> {
    let value = extended_options?.get(2)?.trim();
    let columns_count = value.parse::<u32>().ok()?;
    (columns_count > 0).then_some(columns_count)
}

pub(super) fn parse_form_radio_button_font_xml(
    extended_options: Option<&[&str]>,
) -> Option<String> {
    extended_options
        .and_then(|options| options.get(4))
        .and_then(|field| parse_form_font_tuple_xml(field))
}

pub(super) fn parse_form_radio_button_choice_list(
    extended_options: Option<&[&str]>,
    object_refs: &BTreeMap<String, String>,
) -> Vec<FormChoiceListItem> {
    let Some(field) = extended_options.and_then(|options| options.get(1)) else {
        return Vec::new();
    };
    let Some(fields) = split_1c_braced_fields(field.trim(), 0) else {
        return Vec::new();
    };
    let Some(item_count) = fields
        .get(1)
        .and_then(|value| value.trim().parse::<usize>().ok())
    else {
        return Vec::new();
    };

    (0..item_count)
        .filter_map(|index| fields.get(3 + index * 2).copied())
        .filter_map(|field| parse_form_radio_button_choice_list_item(field, object_refs))
        .collect()
}

pub(super) fn parse_form_input_field_choice_list(
    extended_options: Option<&[&str]>,
    object_refs: &BTreeMap<String, String>,
) -> Vec<FormChoiceListItem> {
    let Some(field) = extended_options.and_then(|options| options.get(1)) else {
        return Vec::new();
    };
    let Some(fields) = split_1c_braced_fields(field.trim(), 0) else {
        return Vec::new();
    };
    let Some(item_count) = fields
        .get(1)
        .and_then(|value| value.trim().parse::<usize>().ok())
    else {
        return Vec::new();
    };

    (0..item_count)
        .filter_map(|index| fields.get(3 + index * 2).copied())
        .filter_map(|field| parse_form_input_field_choice_list_item(field, object_refs))
        .collect()
}

pub(super) fn parse_form_input_field_choice_list_item(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<FormChoiceListItem> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if parse_1c_string(fields.first()?.trim())? != "#" {
        return None;
    }
    let payload_fields = split_1c_braced_fields(fields.get(2)?.trim(), 0)?;
    let value = parse_form_radio_button_choice_list_value(&payload_fields, object_refs)?;
    let presentation = payload_fields
        .get(5)
        .map(|field| parse_form_localized_strings(field))
        .unwrap_or_default();
    Some(FormChoiceListItem {
        presentation,
        value,
    })
}

pub(super) fn parse_form_radio_button_choice_list_item(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<FormChoiceListItem> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let payload_fields = split_1c_braced_fields(fields.get(2)?.trim(), 0)?;
    let value = parse_form_radio_button_choice_list_value(&payload_fields, object_refs)?;
    let presentation = payload_fields
        .get(5)
        .map(|field| parse_form_localized_strings(field))
        .unwrap_or_default();
    Some(FormChoiceListItem {
        presentation,
        value,
    })
}

pub(super) fn parse_form_radio_button_choice_list_value(
    payload_fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormChoiceListValue> {
    let value_fields = split_1c_braced_fields(payload_fields.get(2)?.trim(), 0)?;
    match parse_1c_string(value_fields.first()?.trim())?.as_str() {
        "N" => Some(FormChoiceListValue::Decimal(
            value_fields.get(1)?.trim().to_string(),
        )),
        "S" => parse_1c_quoted_string(value_fields.get(1)?.trim()).map(FormChoiceListValue::String),
        "U" => parse_design_time_reference(payload_fields.get(4)?.trim(), object_refs)
            .map(FormChoiceListValue::DesignTimeRef),
        _ => None,
    }
}

pub(super) fn parse_form_search_addition_type(field: &str) -> Option<&'static str> {
    match field.trim() {
        "0" => Some("SearchStringRepresentation"),
        "1" => Some("ViewStatusRepresentation"),
        "2" => Some("SearchControl"),
        _ => None,
    }
}

pub(super) fn parse_form_search_addition_source_item(
    field: &str,
    table_name_by_id: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let table_id = fields.first()?.trim();
    table_name_by_id.get(table_id).cloned()
}

pub(super) fn parse_form_table_representation(field: &str) -> Option<&'static str> {
    match field.trim() {
        "1" => Some("List"),
        _ => None,
    }
}

pub(super) fn parse_form_table_row_selection_mode(field: &str) -> Option<&'static str> {
    match field.trim() {
        "1" => Some("Cell"),
        _ => None,
    }
}

pub(super) fn parse_form_table_row_input_mode(field: Option<&str>) -> Option<&'static str> {
    match field.map(|value| value.trim()) {
        Some("2") => Some("AfterCurrentRow"),
        _ => None,
    }
}

pub(super) fn parse_form_table_file_drag_mode(field: &str) -> Option<&'static str> {
    match field.trim() {
        "2" => Some("AsFile"),
        _ => None,
    }
}

pub(super) fn parse_form_button_group_command_source(fields: &[&str]) -> Option<&'static str> {
    let source = split_1c_braced_fields(fields.get(20)?.trim(), 0)?;
    let form_ref = split_1c_braced_fields(source.get(1)?.trim(), 0)?;
    match (
        source.first().map(|field| field.trim()),
        form_ref.first().map(|field| field.trim()),
        source.get(2).map(|field| field.trim()),
        source.get(3).map(|field| field.trim()),
    ) {
        (Some("2"), Some("0"), Some("2"), Some("0")) => Some("Form"),
        _ => None,
    }
}

pub(super) fn parse_form_command_bar_source(fields: &[&str]) -> Option<&'static str> {
    let source = split_1c_braced_fields(fields.get(20)?.trim(), 0)?;
    match (
        source.first().map(|field| field.trim()),
        fields.get(21).map(|field| field.trim()),
    ) {
        (Some("1"), Some("5")) => Some("Form"),
        _ => None,
    }
}

pub(super) fn parse_form_table_property_bag_bool(fields: &[&str], key: &str) -> Option<bool> {
    let value = form_table_property_bag_value(fields, key)?;
    let fields = split_1c_braced_fields(value.trim(), 0)?;
    if fields.first().and_then(|field| parse_1c_string(field))? != "B" {
        return None;
    }
    match fields.get(1).map(|field| field.trim())? {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

pub(super) fn parse_form_table_property_bag_number(fields: &[&str], key: &str) -> Option<String> {
    let value = form_table_property_bag_value(fields, key)?;
    let fields = split_1c_braced_fields(value.trim(), 0)?;
    if fields.first().and_then(|field| parse_1c_string(field))? != "N" {
        return None;
    }
    fields
        .get(1)
        .map(|field| field.trim().to_string())
        .filter(|value| value.parse::<u32>().is_ok())
}

pub(super) fn parse_form_table_property_bag_string(fields: &[&str], key: &str) -> Option<String> {
    let value = form_table_property_bag_value(fields, key)?;
    let fields = split_1c_braced_fields(value.trim(), 0)?;
    if fields.first().and_then(|field| parse_1c_string(field))? != "S" {
        return None;
    }
    fields.get(1).and_then(|field| parse_1c_string(field))
}

pub(super) fn parse_form_table_property_bag_undefined(fields: &[&str], key: &str) -> Option<bool> {
    let value = form_table_property_bag_value(fields, key)?;
    parse_form_standalone_undefined_marker(value)
}

pub(super) fn parse_form_table_period(fields: &[&str]) -> Option<FormTablePeriod> {
    let value = form_table_property_bag_value(fields, "7")?;
    let fields = split_1c_braced_fields(value.trim(), 0)?;
    let payload = split_1c_braced_fields(fields.get(2)?.trim(), 0)?;
    match (
        fields.first().and_then(|field| parse_1c_string(field)),
        fields.get(1).map(|field| field.trim()),
        payload.first().map(|field| field.trim()),
        payload
            .get(1)
            .and_then(|field| format_1c_date_time(field.trim())),
        payload
            .get(2)
            .and_then(|field| format_1c_date_time(field.trim())),
    ) {
        (
            Some(marker),
            Some(FORM_STANDARD_PERIOD_UUID),
            Some("0"),
            Some(start_date),
            Some(end_date),
        ) if marker == "#" => Some(FormTablePeriod {
            variant: "Custom",
            start_date,
            end_date,
        }),
        _ => None,
    }
}

pub(super) fn apply_form_table_user_settings_groups(
    items: &mut [FormChildItem],
    group_names_by_table_id: &BTreeMap<String, String>,
) {
    for item in items {
        if item.tag == "Table"
            && let Some(group_name) = group_names_by_table_id.get(&item.id)
        {
            item.user_settings_group = Some(group_name.clone());
        }
        apply_form_table_user_settings_groups(&mut item.child_items, group_names_by_table_id);
    }
}

pub(super) fn parse_form_table_update_on_data_change(fields: &[&str]) -> Option<&'static str> {
    let value = form_table_property_bag_value(fields, "14")?;
    let fields = split_1c_braced_fields(value.trim(), 0)?;
    match (
        fields.first().and_then(|field| parse_1c_string(field)),
        fields.get(1).map(|field| field.trim()),
        fields.get(2).map(|field| field.trim()),
    ) {
        (Some(marker), Some(FORM_UPDATE_ON_DATA_CHANGE_UUID), Some("0")) if marker == "#" => {
            Some("Auto")
        }
        _ => None,
    }
}

pub(super) fn parse_form_table_choice_folders_and_items(fields: &[&str]) -> Option<&'static str> {
    let value = form_table_property_bag_value(fields, "8")?;
    let fields = split_1c_braced_fields(value.trim(), 0)?;
    match (
        fields.first().and_then(|field| parse_1c_string(field)),
        fields.get(1).map(|field| field.trim()),
        fields.get(2).map(|field| field.trim()),
    ) {
        (Some(marker), Some(FORM_USE_FOR_FOLDERS_AND_ITEMS_UUID), Some("0")) if marker == "#" => {
            Some("Items")
        }
        (Some(marker), Some(FORM_USE_FOR_FOLDERS_AND_ITEMS_UUID), Some("1")) if marker == "#" => {
            Some("Folders")
        }
        _ => None,
    }
}

pub(super) fn form_table_property_bag_value<'a>(fields: &[&'a str], key: &str) -> Option<&'a str> {
    fields.windows(2).find_map(|window| {
        (window[0].trim() == key && window[1].trim_start().starts_with('{')).then_some(window[1])
    })
}

pub(super) fn form_table_excluded_command_name(uuid: &str) -> Option<&'static str> {
    match uuid {
        "04ac7211-e74f-4776-9749-35a9282b1d52" => Some("UndoPosting"),
        "0ae4bea5-23be-42a7-b69e-97b11b29c453" => Some("Copy"),
        "0f8d6d98-2f8b-405a-b8b3-0538e9d95da5" => Some("ChangeHistory"),
        "11761e12-cf32-4826-a175-b23213e3b229" => Some("Change"),
        "18248aa8-e621-4e19-a611-54fb8923644c" => Some("CheckAll"),
        "182a793b-22a5-4625-b316-6a5be7f88078" => Some("LoadDynamicListSettings"),
        "2bbe4e12-06d2-409b-a972-eea585125d83" => Some("SortListAsc"),
        "33b7b9cd-6979-4435-8c58-d9bc8250edec" => Some("DynamicListStandardSettings"),
        "37740564-9e86-44a0-bea9-3f485a5a3f91" => Some("MoveUp"),
        "403bc6e6-b98e-4181-9f43-9c75cbbf82cf" => Some("Refresh"),
        "44ad3ec9-f3c2-4913-9224-5f9fb6418743" => Some("CancelSearch"),
        "58b2a785-23f6-4b0e-a324-9a1323285595" => Some("SortListDesc"),
        "59b4387d-f5be-4658-901f-bd3068217469" => Some("Pickup"),
        "714d44cc-63da-4431-b33a-428e398d2a08" => Some("FindByCurrentValue"),
        "7b683784-b474-441a-ba63-3d757bd0ffd4" => Some("FindByCurrentValue"),
        "825c1c15-ef8f-47ab-b002-e6b84b3e5b10" => Some("OutputList"),
        "88078230-1f6b-415f-99e4-ad2ff73810cf" => Some("CopyToClipboard"),
        "8af6ebff-cd02-4bfe-a984-44a292623708" => Some("Copy"),
        "8d772f97-c0ef-47c0-9cb0-efea28c61341" => Some("Delete"),
        "8969c93a-23e5-4bef-941d-aaef315858d2" => Some("Choose"),
        "95b4bc12-2ece-4d7a-b3e2-6f9293620a06" => Some("SaveDynamicListSettings"),
        "9ef79140-3de6-436a-8dda-610bb963f5db" => Some("EndEdit"),
        "a2f737a8-0114-4e86-a214-45e5c213fa65" => Some("SetDeletionMark"),
        "b0016a68-ec64-4e6d-b905-c71fd62efc4c" => Some("Add"),
        "b41f5bbc-ba5d-4888-8cd1-db246a371418" => Some("Change"),
        "c0519548-2a9a-44de-a25e-faf01e089d4d" => Some("Find"),
        "daa306cd-a78a-4e74-a14c-739daba624cb" => Some("SetDateInterval"),
        "e3dd8850-fc3c-41b1-bbb3-7c66af082608" => Some("SetDateInterval"),
        "e7216412-03ac-4a81-99c2-1d7c28e88e31" => Some("SetDeletionMark"),
        "ec576e13-1e76-4c33-98aa-a33204514227" => Some("ShowMultipleSelection"),
        "fa51b106-eae6-44c7-8054-76cbb3100603" => Some("MoveDown"),
        "01833a5a-6553-4c49-b445-095018107bb5" => Some("HierarchicalList"),
        "05468165-f954-45a5-84f2-6641c51f9f23" => Some("Tree"),
        "0d0249a4-2b2f-4fc0-a66f-b36f9494b3cc" => Some("List"),
        "49602716-fea6-497f-8047-726404038857" => Some("OutputList"),
        "51c99108-107c-43e1-8918-e48835bf2495" => Some("SelectAll"),
        _ => None,
    }
}

pub(super) fn form_table_excluded_command_rank(command: &str) -> usize {
    match command {
        "Add" => 0,
        "CancelSearch" => 1,
        "Change" => 2,
        "Choose" => 3,
        "ChangeHistory" => 4,
        "CheckAll" => 5,
        "Copy" => 6,
        "CopyToClipboard" => 7,
        "Create" => 8,
        "Delete" => 9,
        "DynamicListStandardSettings" => 10,
        "EndEdit" => 11,
        "Find" => 12,
        "FindByCurrentValue" => 13,
        "HierarchicalList" => 14,
        "List" => 15,
        "LoadDynamicListSettings" => 16,
        "MoveDown" => 17,
        "MoveUp" => 18,
        "OutputList" => 19,
        "Pickup" => 20,
        "Post" => 21,
        "Refresh" => 22,
        "SaveDynamicListSettings" => 23,
        "SearchEverywhere" => 24,
        "SearchHistory" => 25,
        "SelectAll" => 26,
        "SetDateInterval" => 27,
        "SetDeletionMark" => 28,
        "ShowMultipleSelection" => 29,
        "ShowRowRearrangement" => 30,
        "SortListAsc" => 31,
        "SortListDesc" => 32,
        "Tree" => 33,
        "UndoPosting" => 34,
        _ => usize::MAX,
    }
}

pub(super) fn parse_form_table_command_set_excluded_commands(fields: &[&str]) -> Vec<&'static str> {
    for field in fields {
        let field = field.trim();
        let nested_field = if field.starts_with('{') {
            scan_1c_braced_value(field, 0)
                .map(|end| &field[..end])
                .unwrap_or(field)
        } else {
            field
        };
        let Some(nested) = split_1c_braced_fields(nested_field, 0) else {
            continue;
        };
        let Some(count) = nested
            .first()
            .and_then(|value| value.trim().parse::<usize>().ok())
        else {
            continue;
        };
        let uuids: Vec<&str> = nested.iter().skip(1).map(|uuid| uuid.trim()).collect();
        if count == 0 || count != uuids.len() {
            continue;
        }
        match uuids.as_slice() {
            [
                "0ae4bea5-23be-42a7-b69e-97b11b29c453",
                "0f8d6d98-2f8b-405a-b8b3-0538e9d95da5",
                "182a793b-22a5-4625-b316-6a5be7f88078",
                "33b7b9cd-6979-4435-8c58-d9bc8250edec",
                "403bc6e6-b98e-4181-9f43-9c75cbbf82cf",
                "8d772f97-c0ef-47c0-9cb0-efea28c61341",
                "95b4bc12-2ece-4d7a-b3e2-6f9293620a06",
                "b41f5bbc-ba5d-4888-8cd1-db246a371418",
            ] => {
                return vec![
                    "Change",
                    "Copy",
                    "Create",
                    "Delete",
                    "DynamicListStandardSettings",
                    "LoadDynamicListSettings",
                    "Refresh",
                    "SaveDynamicListSettings",
                ];
            }
            [
                "0ae4bea5-23be-42a7-b69e-97b11b29c453",
                "2bbe4e12-06d2-409b-a972-eea585125d83",
                "37740564-9e86-44a0-bea9-3f485a5a3f91",
                "58b2a785-23f6-4b0e-a324-9a1323285595",
                "8d772f97-c0ef-47c0-9cb0-efea28c61341",
                "9ef79140-3de6-436a-8dda-610bb963f5db",
                "b0016a68-ec64-4e6d-b905-c71fd62efc4c",
                "fa51b106-eae6-44c7-8054-76cbb3100603",
            ] => {
                return vec![
                    "Add",
                    "Copy",
                    "Delete",
                    "EndEdit",
                    "MoveDown",
                    "MoveUp",
                    "SortListAsc",
                    "SortListDesc",
                ];
            }
            [
                "0ae4bea5-23be-42a7-b69e-97b11b29c453",
                "51c99108-107c-43e1-8918-e48835bf2495",
                "59b4387d-f5be-4658-901f-bd3068217469",
                "714d44cc-63da-4431-b33a-428e398d2a08",
                "7b683784-b474-441a-ba63-3d757bd0ffd4",
                "88078230-1f6b-415f-99e4-ad2ff73810cf",
                "8af6ebff-cd02-4bfe-a984-44a292623708",
                "8d772f97-c0ef-47c0-9cb0-efea28c61341",
                "9ef79140-3de6-436a-8dda-610bb963f5db",
                "b0016a68-ec64-4e6d-b905-c71fd62efc4c",
                "b41f5bbc-ba5d-4888-8cd1-db246a371418",
                "d96b0c03-b209-4d01-a3fc-17a14f873b64",
                "e7216412-03ac-4a81-99c2-1d7c28e88e31",
            ] => {
                return vec![
                    "Add",
                    "Change",
                    "Copy",
                    "CopyToClipboard",
                    "Delete",
                    "EndEdit",
                    "FindByCurrentValue",
                    "Pickup",
                    "SearchEverywhere",
                    "SearchHistory",
                    "SelectAll",
                    "ShowMultipleSelection",
                    "ShowRowRearrangement",
                ];
            }
            [
                "0ae4bea5-23be-42a7-b69e-97b11b29c453",
                "37740564-9e86-44a0-bea9-3f485a5a3f91",
                "51c99108-107c-43e1-8918-e48835bf2495",
                "7b683784-b474-441a-ba63-3d757bd0ffd4",
                "88078230-1f6b-415f-99e4-ad2ff73810cf",
                "8af6ebff-cd02-4bfe-a984-44a292623708",
                "8d772f97-c0ef-47c0-9cb0-efea28c61341",
                "9ef79140-3de6-436a-8dda-610bb963f5db",
                "b0016a68-ec64-4e6d-b905-c71fd62efc4c",
                "b41f5bbc-ba5d-4888-8cd1-db246a371418",
                "e7216412-03ac-4a81-99c2-1d7c28e88e31",
                "fa51b106-eae6-44c7-8054-76cbb3100603",
            ] => {
                return vec![
                    "Add",
                    "Change",
                    "Copy",
                    "Delete",
                    "EndEdit",
                    "MoveDown",
                    "MoveUp",
                    "SearchEverywhere",
                    "SelectAll",
                    "ShowMultipleSelection",
                    "ShowRowRearrangement",
                    "SortListDesc",
                ];
            }
            [
                "0ae4bea5-23be-42a7-b69e-97b11b29c453",
                "37740564-9e86-44a0-bea9-3f485a5a3f91",
                "44ad3ec9-f3c2-4913-9224-5f9fb6418743",
                "714d44cc-63da-4431-b33a-428e398d2a08",
                "7b683784-b474-441a-ba63-3d757bd0ffd4",
                "88078230-1f6b-415f-99e4-ad2ff73810cf",
                "8af6ebff-cd02-4bfe-a984-44a292623708",
                "8d772f97-c0ef-47c0-9cb0-efea28c61341",
                "9ef79140-3de6-436a-8dda-610bb963f5db",
                "b0016a68-ec64-4e6d-b905-c71fd62efc4c",
                "b41f5bbc-ba5d-4888-8cd1-db246a371418",
                "c0519548-2a9a-44de-a25e-faf01e089d4d",
                "d96b0c03-b209-4d01-a3fc-17a14f873b64",
                "fa51b106-eae6-44c7-8054-76cbb3100603",
            ] => {
                return vec![
                    "Add",
                    "CancelSearch",
                    "Change",
                    "Copy",
                    "CopyToClipboard",
                    "Delete",
                    "EndEdit",
                    "Find",
                    "FindByCurrentValue",
                    "MoveDown",
                    "MoveUp",
                    "SearchEverywhere",
                    "SearchHistory",
                    "ShowRowRearrangement",
                ];
            }
            [
                "0ae4bea5-23be-42a7-b69e-97b11b29c453",
                "18248aa8-e621-4e19-a611-54fb8923644c",
                "2bbe4e12-06d2-409b-a972-eea585125d83",
                "37740564-9e86-44a0-bea9-3f485a5a3f91",
                "58b2a785-23f6-4b0e-a324-9a1323285595",
                "59b4387d-f5be-4658-901f-bd3068217469",
                "8d772f97-c0ef-47c0-9cb0-efea28c61341",
                "9ef79140-3de6-436a-8dda-610bb963f5db",
                "b0016a68-ec64-4e6d-b905-c71fd62efc4c",
                "b41f5bbc-ba5d-4888-8cd1-db246a371418",
                "fa51b106-eae6-44c7-8054-76cbb3100603",
            ] => {
                return vec![
                    "Add",
                    "Change",
                    "CheckAll",
                    "Copy",
                    "Delete",
                    "EndEdit",
                    "MoveDown",
                    "MoveUp",
                    "Pickup",
                    "SortListAsc",
                    "SortListDesc",
                ];
            }
            [
                "0ae4bea5-23be-42a7-b69e-97b11b29c453",
                "2bbe4e12-06d2-409b-a972-eea585125d83",
                "37740564-9e86-44a0-bea9-3f485a5a3f91",
                "44ad3ec9-f3c2-4913-9224-5f9fb6418743",
                "58b2a785-23f6-4b0e-a324-9a1323285595",
                "8af6ebff-cd02-4bfe-a984-44a292623708",
                "9ef79140-3de6-436a-8dda-610bb963f5db",
                "c0519548-2a9a-44de-a25e-faf01e089d4d",
                "fa51b106-eae6-44c7-8054-76cbb3100603",
            ] => {
                return vec![
                    "CancelSearch",
                    "Copy",
                    "EndEdit",
                    "Find",
                    "MoveDown",
                    "MoveUp",
                    "ShowRowRearrangement",
                    "SortListAsc",
                    "SortListDesc",
                ];
            }
            [
                "0ae4bea5-23be-42a7-b69e-97b11b29c453",
                "2bbe4e12-06d2-409b-a972-eea585125d83",
                "37740564-9e86-44a0-bea9-3f485a5a3f91",
                "58b2a785-23f6-4b0e-a324-9a1323285595",
                "8af6ebff-cd02-4bfe-a984-44a292623708",
                "8d772f97-c0ef-47c0-9cb0-efea28c61341",
                "9ef79140-3de6-436a-8dda-610bb963f5db",
                "b0016a68-ec64-4e6d-b905-c71fd62efc4c",
                "b41f5bbc-ba5d-4888-8cd1-db246a371418",
                "fa51b106-eae6-44c7-8054-76cbb3100603",
            ] => {
                return vec![
                    "Add",
                    "Change",
                    "Copy",
                    "Delete",
                    "EndEdit",
                    "MoveDown",
                    "MoveUp",
                    "ShowRowRearrangement",
                    "SortListAsc",
                    "SortListDesc",
                ];
            }
            [
                "0ae4bea5-23be-42a7-b69e-97b11b29c453",
                "2bbe4e12-06d2-409b-a972-eea585125d83",
                "37740564-9e86-44a0-bea9-3f485a5a3f91",
                "44ad3ec9-f3c2-4913-9224-5f9fb6418743",
                "58b2a785-23f6-4b0e-a324-9a1323285595",
                "8d772f97-c0ef-47c0-9cb0-efea28c61341",
                "9ef79140-3de6-436a-8dda-610bb963f5db",
                "b0016a68-ec64-4e6d-b905-c71fd62efc4c",
                "b41f5bbc-ba5d-4888-8cd1-db246a371418",
                "c0519548-2a9a-44de-a25e-faf01e089d4d",
                "fa51b106-eae6-44c7-8054-76cbb3100603",
            ] => {
                return vec![
                    "Add",
                    "CancelSearch",
                    "Change",
                    "Copy",
                    "Delete",
                    "EndEdit",
                    "Find",
                    "MoveDown",
                    "MoveUp",
                    "SortListAsc",
                    "SortListDesc",
                ];
            }
            [
                "0ae4bea5-23be-42a7-b69e-97b11b29c453",
                "2bbe4e12-06d2-409b-a972-eea585125d83",
                "37740564-9e86-44a0-bea9-3f485a5a3f91",
                "49602716-fea6-497f-8047-726404038857",
                "51c99108-107c-43e1-8918-e48835bf2495",
                "58b2a785-23f6-4b0e-a324-9a1323285595",
                "88078230-1f6b-415f-99e4-ad2ff73810cf",
                "8af6ebff-cd02-4bfe-a984-44a292623708",
                "9ef79140-3de6-436a-8dda-610bb963f5db",
                "b0016a68-ec64-4e6d-b905-c71fd62efc4c",
                "b41f5bbc-ba5d-4888-8cd1-db246a371418",
                "e7216412-03ac-4a81-99c2-1d7c28e88e31",
                "fa51b106-eae6-44c7-8054-76cbb3100603",
            ] => {
                return vec![
                    "Add",
                    "Change",
                    "Copy",
                    "CopyToClipboard",
                    "EndEdit",
                    "MoveDown",
                    "MoveUp",
                    "OutputList",
                    "SelectAll",
                    "ShowMultipleSelection",
                    "ShowRowRearrangement",
                    "SortListAsc",
                    "SortListDesc",
                ];
            }
            [
                "0ae4bea5-23be-42a7-b69e-97b11b29c453",
                "2bbe4e12-06d2-409b-a972-eea585125d83",
                "37740564-9e86-44a0-bea9-3f485a5a3f91",
                "44ad3ec9-f3c2-4913-9224-5f9fb6418743",
                "49602716-fea6-497f-8047-726404038857",
                "51c99108-107c-43e1-8918-e48835bf2495",
                "58b2a785-23f6-4b0e-a324-9a1323285595",
                "88078230-1f6b-415f-99e4-ad2ff73810cf",
                "8d772f97-c0ef-47c0-9cb0-efea28c61341",
                "9ef79140-3de6-436a-8dda-610bb963f5db",
                "b0016a68-ec64-4e6d-b905-c71fd62efc4c",
                "b41f5bbc-ba5d-4888-8cd1-db246a371418",
                "c0519548-2a9a-44de-a25e-faf01e089d4d",
                "fa51b106-eae6-44c7-8054-76cbb3100603",
            ] => {
                return vec![
                    "Add",
                    "CancelSearch",
                    "Change",
                    "Copy",
                    "CopyToClipboard",
                    "Delete",
                    "EndEdit",
                    "Find",
                    "MoveDown",
                    "MoveUp",
                    "OutputList",
                    "SelectAll",
                    "SortListAsc",
                    "SortListDesc",
                ];
            }
            [
                "0ae4bea5-23be-42a7-b69e-97b11b29c453",
                "2bbe4e12-06d2-409b-a972-eea585125d83",
                "37740564-9e86-44a0-bea9-3f485a5a3f91",
                "44ad3ec9-f3c2-4913-9224-5f9fb6418743",
                "49602716-fea6-497f-8047-726404038857",
                "51c99108-107c-43e1-8918-e48835bf2495",
                "58b2a785-23f6-4b0e-a324-9a1323285595",
                "88078230-1f6b-415f-99e4-ad2ff73810cf",
                "8af6ebff-cd02-4bfe-a984-44a292623708",
                "8d772f97-c0ef-47c0-9cb0-efea28c61341",
                "9ef79140-3de6-436a-8dda-610bb963f5db",
                "b0016a68-ec64-4e6d-b905-c71fd62efc4c",
                "b41f5bbc-ba5d-4888-8cd1-db246a371418",
                "c0519548-2a9a-44de-a25e-faf01e089d4d",
                "e7216412-03ac-4a81-99c2-1d7c28e88e31",
                "fa51b106-eae6-44c7-8054-76cbb3100603",
            ] => {
                return vec![
                    "Add",
                    "CancelSearch",
                    "Change",
                    "Copy",
                    "CopyToClipboard",
                    "Delete",
                    "EndEdit",
                    "Find",
                    "MoveDown",
                    "MoveUp",
                    "OutputList",
                    "SelectAll",
                    "ShowMultipleSelection",
                    "ShowRowRearrangement",
                    "SortListAsc",
                    "SortListDesc",
                ];
            }
            [
                "01833a5a-6553-4c49-b445-095018107bb5",
                "05468165-f954-45a5-84f2-6641c51f9f23",
                "0ae4bea5-23be-42a7-b69e-97b11b29c453",
                "0d0249a4-2b2f-4fc0-a66f-b36f9494b3cc",
                "2bbe4e12-06d2-409b-a972-eea585125d83",
                "37740564-9e86-44a0-bea9-3f485a5a3f91",
                "49602716-fea6-497f-8047-726404038857",
                "51c99108-107c-43e1-8918-e48835bf2495",
                "58b2a785-23f6-4b0e-a324-9a1323285595",
                "88078230-1f6b-415f-99e4-ad2ff73810cf",
                "8af6ebff-cd02-4bfe-a984-44a292623708",
                "8d772f97-c0ef-47c0-9cb0-efea28c61341",
                "9ef79140-3de6-436a-8dda-610bb963f5db",
                "b0016a68-ec64-4e6d-b905-c71fd62efc4c",
                "b41f5bbc-ba5d-4888-8cd1-db246a371418",
                "e7216412-03ac-4a81-99c2-1d7c28e88e31",
                "fa51b106-eae6-44c7-8054-76cbb3100603",
            ] => {
                return vec![
                    "Add",
                    "Change",
                    "Copy",
                    "CopyToClipboard",
                    "Delete",
                    "EndEdit",
                    "HierarchicalList",
                    "List",
                    "MoveDown",
                    "MoveUp",
                    "OutputList",
                    "SelectAll",
                    "ShowMultipleSelection",
                    "ShowRowRearrangement",
                    "SortListAsc",
                    "SortListDesc",
                    "Tree",
                ];
            }
            [
                "0ae4bea5-23be-42a7-b69e-97b11b29c453",
                "2bbe4e12-06d2-409b-a972-eea585125d83",
                "37740564-9e86-44a0-bea9-3f485a5a3f91",
                "44ad3ec9-f3c2-4913-9224-5f9fb6418743",
                "49602716-fea6-497f-8047-726404038857",
                "51c99108-107c-43e1-8918-e48835bf2495",
                "58b2a785-23f6-4b0e-a324-9a1323285595",
                "714d44cc-63da-4431-b33a-428e398d2a08",
                "7b683784-b474-441a-ba63-3d757bd0ffd4",
                "88078230-1f6b-415f-99e4-ad2ff73810cf",
                "8af6ebff-cd02-4bfe-a984-44a292623708",
                "8d772f97-c0ef-47c0-9cb0-efea28c61341",
                "9ef79140-3de6-436a-8dda-610bb963f5db",
                "b0016a68-ec64-4e6d-b905-c71fd62efc4c",
                "b41f5bbc-ba5d-4888-8cd1-db246a371418",
                "c0519548-2a9a-44de-a25e-faf01e089d4d",
                "d96b0c03-b209-4d01-a3fc-17a14f873b64",
                "e7216412-03ac-4a81-99c2-1d7c28e88e31",
                "fa51b106-eae6-44c7-8054-76cbb3100603",
            ] => {
                return vec![
                    "Add",
                    "CancelSearch",
                    "Change",
                    "Copy",
                    "CopyToClipboard",
                    "Delete",
                    "EndEdit",
                    "Find",
                    "FindByCurrentValue",
                    "MoveDown",
                    "MoveUp",
                    "OutputList",
                    "SearchEverywhere",
                    "SearchHistory",
                    "SelectAll",
                    "ShowMultipleSelection",
                    "ShowRowRearrangement",
                    "SortListAsc",
                    "SortListDesc",
                ];
            }
            [
                "0ae4bea5-23be-42a7-b69e-97b11b29c453",
                "37740564-9e86-44a0-bea9-3f485a5a3f91",
                "8af6ebff-cd02-4bfe-a984-44a292623708",
                "9ef79140-3de6-436a-8dda-610bb963f5db",
                "b0016a68-ec64-4e6d-b905-c71fd62efc4c",
                "b41f5bbc-ba5d-4888-8cd1-db246a371418",
                "fa51b106-eae6-44c7-8054-76cbb3100603",
            ] => {
                return vec![
                    "Add",
                    "Change",
                    "Copy",
                    "EndEdit",
                    "MoveDown",
                    "MoveUp",
                    "ShowRowRearrangement",
                ];
            }
            [
                "0ae4bea5-23be-42a7-b69e-97b11b29c453",
                "2bbe4e12-06d2-409b-a972-eea585125d83",
                "37740564-9e86-44a0-bea9-3f485a5a3f91",
                "58b2a785-23f6-4b0e-a324-9a1323285595",
                "9ef79140-3de6-436a-8dda-610bb963f5db",
                "b0016a68-ec64-4e6d-b905-c71fd62efc4c",
                "b41f5bbc-ba5d-4888-8cd1-db246a371418",
                "fa51b106-eae6-44c7-8054-76cbb3100603",
            ] => {
                return vec![
                    "Add",
                    "Change",
                    "Copy",
                    "EndEdit",
                    "MoveDown",
                    "MoveUp",
                    "SortListAsc",
                    "SortListDesc",
                ];
            }
            [
                "04ac7211-e74f-4776-9749-35a9282b1d52",
                "0ae4bea5-23be-42a7-b69e-97b11b29c453",
                "0f8d6d98-2f8b-405a-b8b3-0538e9d95da5",
                "11761e12-cf32-4826-a175-b23213e3b229",
                "182a793b-22a5-4625-b316-6a5be7f88078",
                "33b7b9cd-6979-4435-8c58-d9bc8250edec",
                "403bc6e6-b98e-4181-9f43-9c75cbbf82cf",
                "44ad3ec9-f3c2-4913-9224-5f9fb6418743",
                "714d44cc-63da-4431-b33a-428e398d2a08",
                "7b683784-b474-441a-ba63-3d757bd0ffd4",
                "825c1c15-ef8f-47ab-b002-e6b84b3e5b10",
                "88078230-1f6b-415f-99e4-ad2ff73810cf",
                "95b4bc12-2ece-4d7a-b3e2-6f9293620a06",
                "a2f737a8-0114-4e86-a214-45e5c213fa65",
                "b41f5bbc-ba5d-4888-8cd1-db246a371418",
                "daa306cd-a78a-4e74-a14c-739daba624cb",
                "e3dd8850-fc3c-41b1-bbb3-7c66af082608",
                "e7216412-03ac-4a81-99c2-1d7c28e88e31",
                "ec576e13-1e76-4c33-98aa-a33204514227",
            ] => {
                return vec![
                    "CancelSearch",
                    "Change",
                    "ChangeHistory",
                    "Copy",
                    "CopyToClipboard",
                    "Create",
                    "Delete",
                    "DynamicListStandardSettings",
                    "FindByCurrentValue",
                    "LoadDynamicListSettings",
                    "OutputList",
                    "Post",
                    "Refresh",
                    "SaveDynamicListSettings",
                    "SearchEverywhere",
                    "SetDateInterval",
                    "SetDeletionMark",
                    "ShowMultipleSelection",
                    "UndoPosting",
                ];
            }
            _ => {}
        }
        let mapped: Option<Vec<_>> = uuids
            .iter()
            .map(|uuid| form_table_excluded_command_name(uuid))
            .collect();
        if let Some(mut mapped) = mapped {
            mapped.sort_by_key(|command| form_table_excluded_command_rank(command));
            return mapped;
        }
    }
    Vec::new()
}

pub(super) fn form_child_item_tag(wrapper: &str, fields: &[&str]) -> Option<&'static str> {
    match wrapper {
        "22" => match fields.get(5).map(|value| value.trim())? {
            "0" => Some("CommandBar"),
            "1" => Some("Popup"),
            "2" => Some("ColumnGroup"),
            "3" => Some("Pages"),
            "4" => Some("Page"),
            "5" => Some("UsualGroup"),
            "6" => Some("ButtonGroup"),
            "8" => Some("ContextMenu"),
            "9" => Some("AutoCommandBar"),
            _ => None,
        },
        "12" => {
            let kind = fields.get(5).map(|value| value.trim())?;
            if kind == "0" {
                let name = fields.get(6).and_then(|value| parse_1c_string(value));
                let has_title = fields
                    .get(7)
                    .map(|field| !parse_form_localized_strings(field).is_empty())
                    .unwrap_or(false);
                if has_title && !name.as_deref().is_some_and(is_form_extended_tooltip_name) {
                    Some("LabelDecoration")
                } else {
                    None
                }
            } else if kind == "1" {
                Some("PictureDecoration")
            } else {
                None
            }
        }
        "31" | "34" => Some("Button"),
        "37" | "48" => match fields
            .get(5 + form_input_field_top_level_offset(fields))
            .map(|value| value.trim())?
        {
            "1" => Some("LabelField"),
            "2" => Some("InputField"),
            "3" => Some("CheckBoxField"),
            "4" => Some("PictureField"),
            "5" => Some("RadioButtonField"),
            "7" => Some("TextDocumentField"),
            _ => None,
        },
        "5" | "6" => match fields.get(5).map(|value| value.trim())? {
            "0" => Some("SearchStringAddition"),
            "1" => Some("ViewStatusAddition"),
            "2" => Some("SearchControlAddition"),
            _ => None,
        },
        "73" => Some("Table"),
        "55" => Some("Table"),
        _ => None,
    }
}

pub(super) fn parse_form_child_item_name(wrapper: &str, fields: &[&str]) -> Option<String> {
    let indexes: &[usize] = match wrapper {
        "73" | "55" | "31" | "34" => &[5],
        "37" | "48" => &[6, 7],
        _ => &[6],
    };
    indexes.iter().find_map(|index| {
        parse_1c_quoted_string_with_len(fields.get(*index)?.trim())
            .map(|(value, _)| value)
            .filter(|value| !value.is_empty())
    })
}

pub(super) fn parse_form_child_item_title(wrapper: &str, fields: &[&str]) -> Vec<(String, String)> {
    let indexes: &[usize] = match wrapper {
        "73" | "55" => &[9],
        "31" | "34" => &[6],
        "37" | "48" => &[9, 10],
        _ => &[7],
    };
    indexes
        .iter()
        .find_map(|index| {
            let values = fields
                .get(*index)
                .map(|field| parse_form_localized_strings(field))
                .unwrap_or_default();
            (!values.is_empty()).then_some(values)
        })
        .unwrap_or_default()
}

pub(super) fn parse_form_child_item_tooltip(
    wrapper: &str,
    fields: &[&str],
) -> Vec<(String, String)> {
    let indexes: &[usize] = match wrapper {
        "22" => &[8],
        "37" | "48" => &[10, 11],
        _ => &[],
    };
    indexes
        .iter()
        .find_map(|index| {
            let values = fields
                .get(*index)
                .map(|field| parse_form_localized_strings(field))
                .unwrap_or_default();
            (!values.is_empty()).then_some(values)
        })
        .unwrap_or_default()
}

pub(super) fn parse_form_child_item_input_hint(
    wrapper: &str,
    fields: &[&str],
) -> Vec<(String, String)> {
    let indexes: &[usize] = match wrapper {
        "37" | "48" => &[10, 11],
        _ => &[],
    };
    indexes
        .iter()
        .find_map(|index| {
            let values = fields
                .get(*index)
                .map(|field| parse_form_localized_strings(field))
                .unwrap_or_default();
            (!values.is_empty()).then_some(values)
        })
        .unwrap_or_default()
}

pub(super) fn parse_form_input_field_input_hint(
    extended_options: Option<&[&str]>,
) -> Vec<(String, String)> {
    extended_options
        .and_then(|options| options.get(44))
        .map(|field| parse_form_localized_strings(field))
        .filter(|values| !values.is_empty())
        .unwrap_or_default()
}

pub(super) fn parse_form_child_item_extended_tooltip(fields: &[&str]) -> Option<(String, String)> {
    fields.iter().find_map(|field| {
        let nested = split_1c_braced_fields(field.trim(), 0)?;
        if nested.first().map(|value| value.trim()) != Some("12") {
            return None;
        }
        if nested.get(5).map(|value| value.trim()) == Some("1") {
            return None;
        }
        let identity = split_1c_braced_fields(nested.get(1)?.trim(), 0)?;
        let id = identity.first()?.trim();
        if id == "0" {
            return None;
        }
        let name = nested.get(6).and_then(|value| parse_1c_string(value))?;
        is_form_extended_tooltip_name(&name).then(|| (name, id.to_string()))
    })
}

pub(super) fn is_form_extended_tooltip_name(name: &str) -> bool {
    name.ends_with("ExtendedTooltip") || name.ends_with("РасширеннаяПодсказка")
}

pub(super) fn parse_form_picture_decoration_file_name(fields: &[&str]) -> Option<&'static str> {
    fields.iter().find_map(|field| {
        let payload = extract_base64_payload(field)?;
        let content = decode_base64_mime(payload)?;
        is_form_item_picture_content(&content).then(|| ext_picture_file_name(&content))
    })
}

pub(super) fn parse_form_picture_decoration_picture_value(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Option<(String, bool)> {
    fields
        .iter()
        .find_map(|field| parse_form_popup_picture_value(field.trim(), object_refs))
}

pub(super) fn parse_form_picture_decoration_picture_size(fields: &[&str]) -> Option<&'static str> {
    let options = split_1c_braced_fields(fields.get(18)?.trim(), 0)?;
    options
        .get(3)
        .and_then(|field| field.trim().parse::<usize>().ok())
        .and_then(moxel_picture_size_mode)
}

pub(super) fn parse_form_picture_field_options<'a>(fields: &'a [&'a str]) -> Option<Vec<&'a str>> {
    fields.iter().skip(39).find_map(|field| {
        let options = split_1c_braced_fields(field.trim(), 0)?;
        (options.first().map(|value| value.trim()) == Some("10")).then_some(options)
    })
}

pub(super) fn parse_form_picture_field_value(
    options: Option<&[&str]>,
    object_refs: &BTreeMap<String, String>,
) -> Option<(String, bool)> {
    options
        .and_then(|options| options.get(5))
        .and_then(|field| parse_form_child_item_picture_value(field, object_refs))
}

pub(super) fn parse_form_picture_field_file_drag_mode(
    options: Option<&[&str]>,
) -> Option<&'static str> {
    match options?.get(17).map(|field| field.trim()) {
        Some("1") => Some("AsFile"),
        _ => None,
    }
}

pub(super) fn parse_form_picture_decoration_file_drag_mode(
    fields: &[&str],
) -> Option<&'static str> {
    let options = split_1c_braced_fields(fields.get(18)?.trim(), 0)?;
    match options.get(11).map(|field| field.trim()) {
        Some("0") => Some("AsFile"),
        _ => None,
    }
}

pub(super) fn parse_form_child_item_picture_value(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<(String, bool)> {
    parse_common_command_picture_value(field, object_refs).and_then(
        |(reference, load_transparent)| reference.map(|reference| (reference, load_transparent)),
    )
}

pub(super) fn parse_form_popup_picture_value(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<(String, bool)> {
    let nested = split_1c_braced_fields(field.trim(), 0)?;
    parse_form_child_item_picture_value(nested.get(1)?.trim(), object_refs)
}

pub(super) fn parse_form_child_item_event_fields(fields: &[&str]) -> Vec<FormBodyEvent> {
    let mut events = Vec::new();
    for field in fields {
        let field = field.trim();
        if !field.starts_with('{') {
            continue;
        }
        let Some(nested) = split_1c_braced_fields(field, 0) else {
            continue;
        };
        append_unique_form_body_events(&mut events, parse_form_child_item_event_record(&nested));
    }
    for window in fields.windows(2) {
        if let Some(event) = parse_form_child_item_event_pair(window[0], window[1]) {
            append_unique_form_body_events(&mut events, vec![event]);
        }
    }
    events
}

pub(super) fn parse_form_child_item_event_record(fields: &[&str]) -> Vec<FormBodyEvent> {
    let Some(count) = fields
        .first()
        .and_then(|value| value.trim().parse::<usize>().ok())
    else {
        return Vec::new();
    };
    if count == 0 || fields.len() < 1 + count * 2 {
        return Vec::new();
    }
    let mut events = Vec::new();
    for item_index in 0..count {
        let event_index = 1 + item_index * 2;
        let handler_index = event_index + 1;
        if let Some(event) = fields
            .get(event_index)
            .zip(fields.get(handler_index))
            .and_then(|(event_field, handler_field)| {
                parse_form_child_item_event_pair(event_field, handler_field)
            })
        {
            events.push(event);
        }
    }
    events
}

pub(super) fn append_unique_form_body_events(
    target: &mut Vec<FormBodyEvent>,
    extra: Vec<FormBodyEvent>,
) {
    for event in extra {
        if target
            .iter()
            .any(|existing| existing.name == event.name && existing.handler == event.handler)
        {
            continue;
        }
        target.push(event);
    }
}

pub(super) fn parse_form_nested_child_item_event_records(fields: &[&str]) -> Vec<FormBodyEvent> {
    let mut events = Vec::new();
    collect_form_nested_child_item_event_records(fields, &mut events);
    events
}

pub(super) fn collect_form_nested_child_item_event_records(
    fields: &[&str],
    events: &mut Vec<FormBodyEvent>,
) {
    for field in fields {
        let field = field.trim();
        if !field.starts_with('{') {
            continue;
        }
        let Some(nested) = split_1c_braced_fields(field, 0) else {
            continue;
        };
        append_unique_form_body_events(events, parse_form_child_item_event_record(&nested));
        collect_form_nested_child_item_event_records(&nested, events);
    }
}

pub(super) fn parse_form_child_item_event_pair(
    event_field: &str,
    handler_field: &str,
) -> Option<FormBodyEvent> {
    let event = parse_form_child_item_event_identifier(event_field)?;
    let (handler, _) = parse_1c_quoted_string_with_len(handler_field.trim())?;
    let handler = handler.trim();
    if handler.is_empty() || !is_probable_form_event_handler(handler) {
        return None;
    }
    Some(FormBodyEvent {
        name: event,
        handler: handler.to_string(),
    })
}

pub(super) fn parse_form_child_item_event_identifier(field: &str) -> Option<String> {
    let field = field.trim();
    let identifier = parse_1c_quoted_string_with_len(field)
        .map(|(value, _)| value)
        .unwrap_or_else(|| field.to_string());
    let identifier = identifier.trim();
    match identifier {
        "ActivationProcessing" => Some("ActivationProcessing".to_string()),
        "AdditionalDetailProcessing" => Some("AdditionalDetailProcessing".to_string()),
        "AutoComplete" => Some("AutoComplete".to_string()),
        "OnGetDataAtServer" => Some("OnGetDataAtServer".to_string()),
        "OnChange" => Some("OnChange".to_string()),
        "StartChoice" => Some("StartChoice".to_string()),
        "1960479b-4d89-4eba-8b39-0aa802020558" => Some("StartChoice".to_string()),
        "StartListChoice" => Some("StartListChoice".to_string()),
        "ValueChoice" => Some("ValueChoice".to_string()),
        "0d8cf5b0-55eb-4d1e-960a-22c160210945" => Some("ValueChoice".to_string()),
        "Click" => Some("Click".to_string()),
        "OnClick" => Some("OnClick".to_string()),
        "11707a99-4eb9-4373-bc8c-84891483a034" => Some("Click".to_string()),
        "9874537f-454c-40ae-83e9-3b9cefbc6d08" => Some("Click".to_string()),
        "Clearing" => Some("Clearing".to_string()),
        "ChoiceProcessing" => Some("ChoiceProcessing".to_string()),
        "b50dc41b-c15a-4ebe-a17f-d01e51c47de6" => Some("Clearing".to_string()),
        "f72043b8-2d79-414e-bc4e-3972fe9dbca1" => Some("ChoiceProcessing".to_string()),
        "URLProcessing" => Some("URLProcessing".to_string()),
        "d710ea07-5c96-4c43-ab6e-e138d3653780" => Some("URLProcessing".to_string()),
        "URLGetProcessing" => Some("URLGetProcessing".to_string()),
        "URLListGetProcessing" => Some("URLListGetProcessing".to_string()),
        "TextEditEnd" => Some("TextEditEnd".to_string()),
        "EditTextChange" => Some("EditTextChange".to_string()),
        "ac5a9c5a-5f1d-4fc5-b88c-a187038c16d1" => Some("Opening".to_string()),
        "178a97c4-0ffe-4fcc-93e6-505369939da5" => Some("AutoComplete".to_string()),
        "OnActivateCell" => Some("OnActivateCell".to_string()),
        "f228b12f-d892-4925-b338-695617357b32" => Some("OnActivateCell".to_string()),
        "OnActivateField" => Some("OnActivateField".to_string()),
        "OnActivateRow" => Some("OnActivateRow".to_string()),
        "60edb81d-887b-478e-94ee-7fef2b13393d" => Some("OnActivateRow".to_string()),
        "fe115cc8-9e33-4684-a166-bd5136fe7a9f" => Some("OnChange".to_string()),
        "97365900-eadf-4dfd-a9aa-fbb9ecabd079" => Some("OnGetDataAtServer".to_string()),
        "BeforeAddRow" => Some("BeforeAddRow".to_string()),
        "2391e7b8-7235-45d7-ab7e-6ff3dc086396" => Some("BeforeAddRow".to_string()),
        "Creating" => Some("Creating".to_string()),
        "OnCurrentPageChange" => Some("OnCurrentPageChange".to_string()),
        "OnCurrentParentChange" => Some("OnCurrentParentChange".to_string()),
        "OnEditEnd" => Some("OnEditEnd".to_string()),
        "01d80ddd-dce5-4db3-beb5-f63c97cb05b9" => Some("OnEditEnd".to_string()),
        "BeforeEditEnd" => Some("BeforeEditEnd".to_string()),
        "BeforeDeleteRow" => Some("BeforeDeleteRow".to_string()),
        "2ccfdec5-583d-4eca-8319-e55de492665a" => Some("BeforeDeleteRow".to_string()),
        "OnStartEdit" => Some("OnStartEdit".to_string()),
        "b3c10170-c5ff-4cba-b537-679e1c872b45" => Some("OnStartEdit".to_string()),
        "Selection" => Some("Selection".to_string()),
        "1282f000-23b6-4887-87f4-9e8e79db3d32" => Some("Selection".to_string()),
        "BeforeRowChange" => Some("BeforeRowChange".to_string()),
        "ab930362-ff94-4dcb-ad16-188805d23e3c" => Some("BeforeRowChange".to_string()),
        "AfterDeleteRow" => Some("AfterDeleteRow".to_string()),
        "de65638d-a806-4a76-bc10-f62bbc86e0e7" => Some("AfterDeleteRow".to_string()),
        "BeforeCollapse" => Some("BeforeCollapse".to_string()),
        "BeforeExpand" => Some("BeforeExpand".to_string()),
        "BeforePrint" => Some("BeforePrint".to_string()),
        "DetailProcessing" => Some("DetailProcessing".to_string()),
        "DocumentComplete" => Some("DocumentComplete".to_string()),
        "Drag" => Some("Drag".to_string()),
        "8ad48496-8d0b-4f6c-ae48-99d95227884b" => Some("Drag".to_string()),
        "DragCheck" => Some("DragCheck".to_string()),
        "0d644ff6-443b-4390-86fa-7f9105e42711" => Some("DragCheck".to_string()),
        "DragEnd" => Some("DragEnd".to_string()),
        "cb286ab3-3a1c-40d2-a232-6e64f624ccec" => Some("DragEnd".to_string()),
        "DragStart" => Some("DragStart".to_string()),
        "6d4d6747-a823-4f61-ab31-a426572f2c6c" => Some("DragStart".to_string()),
        "MultipleValuesDelete" => Some("MultipleValuesDelete".to_string()),
        "NavigationProcessing" => Some("NavigationProcessing".to_string()),
        "NewWriteProcessing" => Some("NewWriteProcessing".to_string()),
        "OnChangeAreaContent" => Some("OnChangeAreaContent".to_string()),
        "OnChangeDisplaySettings" => Some("OnChangeDisplaySettings".to_string()),
        "OnIntervalEditEnd" => Some("OnIntervalEditEnd".to_string()),
        "OnPeriodOutput" => Some("OnPeriodOutput".to_string()),
        "RefreshRequestProcessing" => Some("RefreshRequestProcessing".to_string()),
        "Tuning" => Some("Tuning".to_string()),
        "70636369-514c-4662-977e-1c3976c9756c" => Some("Tuning".to_string()),
        _ => parse_form_event_identifier(identifier),
    }
}

pub(super) fn parse_form_child_item_data_path(
    tag: &str,
    fields: &[&str],
    name: &str,
    id: &str,
    main_data_path: Option<&str>,
    parent_data_path: Option<&str>,
    attribute_names_by_id: &BTreeMap<String, String>,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    data_path_by_binding_key: &BTreeMap<String, String>,
    bound_table_path_by_binding_key: &BTreeMap<String, String>,
    table_column_names_by_binding_key: &BTreeMap<String, BTreeMap<String, String>>,
) -> Option<String> {
    let parse_bound = |field: &&str| {
        parse_form_bound_data_path(
            field,
            name,
            attribute_names_by_id,
            table_name_by_id,
            table_column_names_by_id,
            bound_table_path_by_binding_key,
            table_column_names_by_binding_key,
        )
        .or_else(|| {
            parse_form_bound_data_binding_key(field)
                .and_then(|binding_key| data_path_by_binding_key.get(&binding_key).cloned())
        })
    };
    let input_field_offset = matches!(
        tag,
        "InputField"
            | "LabelField"
            | "CheckBoxField"
            | "PictureField"
            | "RadioButtonField"
            | "TextDocumentField"
    )
    .then(|| {
        form_input_field_layout_is_extended(fields)
            .then(|| form_input_field_top_level_offset(fields))
    })
    .flatten()
    .unwrap_or(0);
    match tag {
        "Table" => fields
            .get(11)
            .and_then(parse_bound)
            .or_else(|| main_data_path.map(ToOwned::to_owned)),
        "InputField" | "LabelField" | "CheckBoxField" | "PictureField" | "RadioButtonField" => {
            [11usize, 12]
                .iter()
                .filter_map(|index| fields.get(*index + input_field_offset))
                .find_map(parse_bound)
                .or_else(|| parent_data_path.map(|parent| format!("{parent}.{name}")))
        }
        "TextDocumentField" => [11usize, 12]
            .iter()
            .filter_map(|index| fields.get(*index + input_field_offset))
            .find_map(parse_bound),
        "Button" => fields.get(9).and_then(|field| {
            parse_form_button_data_path(field, table_name_by_id, table_column_names_by_id)
        }),
        _ => table_name_by_id.get(id).cloned(),
    }
}

pub(super) fn parse_form_bound_data_path(
    field: &str,
    name: &str,
    attribute_names_by_id: &BTreeMap<String, String>,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
    bound_table_path_by_binding_key: &BTreeMap<String, String>,
    table_column_names_by_binding_key: &BTreeMap<String, BTreeMap<String, String>>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    match fields.first().map(|value| value.trim()) {
        Some("1") => parse_form_attribute_data_path(field, name, attribute_names_by_id),
        Some("2") => {
            let table = fields
                .get(1)
                .and_then(|field| split_1c_braced_fields(field, 0))?;
            let table_id = table.first()?.trim();
            let table_name = attribute_names_by_id
                .get(table_id)
                .or_else(|| table_name_by_id.get(table_id))?;
            let column = parse_form_binding_key(fields.get(2)?.trim())?;
            if let Some(table_path) = bound_table_path_by_binding_key.get(&column) {
                return Some(table_path.clone());
            }
            let field_name = table_column_names_by_id
                .get(table_id)
                .and_then(|columns| columns.get(&column))
                .cloned()
                .or_else(|| (column == "8").then(|| "Ссылка".to_string()))?;
            let field_name = normalize_form_table_column_name(table_name, &field_name);
            Some(format!("{table_name}.{field_name}"))
        }
        Some("3") => {
            let table_key = parse_form_binding_key(fields.get(2)?.trim())?;
            let table_path = bound_table_path_by_binding_key.get(&table_key)?;
            let column_key = parse_form_binding_key(fields.get(3)?.trim())?;
            let field_name = table_column_names_by_binding_key
                .get(&table_key)
                .and_then(|columns| columns.get(&column_key))
                .cloned()
                .or_else(|| (column_key == "8").then(|| "Ссылка".to_string()))?;
            let field_name = normalize_form_table_column_name(table_path, &field_name);
            Some(format!("{table_path}.{field_name}"))
        }
        _ => None,
    }
}

pub(super) fn parse_form_ordinary_table_row_picture_data_path(
    fields: &[&str],
    data_path: Option<&str>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
) -> Option<String> {
    let table_name = data_path?;
    let table_id = parse_form_attribute_binding_id(fields.get(11)?.trim())?;
    let column_id = parse_form_attribute_binding_id(fields.get(43)?.trim())?;
    let column_name = table_column_names_by_id
        .get(&table_id)?
        .get(&column_id)?
        .as_str();
    let column_name = normalize_form_table_column_name(table_name, column_name);
    Some(format!("{table_name}.{column_name}"))
}

pub(super) fn normalize_form_table_column_name(table_name: &str, field_name: &str) -> String {
    [Some(table_name), table_name.rsplit('.').next()]
        .into_iter()
        .flatten()
        .find_map(|prefix| {
            field_name.strip_prefix(prefix).filter(|suffix| {
                suffix
                    .chars()
                    .next()
                    .is_some_and(|first| !first.is_lowercase())
            })
        })
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| field_name.to_string())
}

pub(super) fn parse_form_binding_key(field: &str) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let first = fields.first()?.trim();
    match fields.len() {
        1 => Some(first.to_string()),
        2 => Some(format!("{}|{}", first, fields.get(1)?.trim())),
        _ => None,
    }
}

pub(super) fn parse_form_bound_data_binding_key(field: &str) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    match fields.first().map(|value| value.trim()) {
        Some("2") => parse_form_binding_key(fields.get(2)?.trim()),
        Some("3") => parse_form_binding_key(fields.get(3)?.trim()),
        _ => None,
    }
}

pub(super) fn parse_form_bound_attribute_id(field: &str) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    match fields.first().map(|value| value.trim()) {
        Some("2") | Some("3") => split_1c_braced_fields(fields.get(1)?.trim(), 0)?
            .first()
            .map(|value| value.trim().to_string()),
        _ => None,
    }
}

pub(super) fn infer_form_bound_property_name(names: &BTreeSet<String>) -> Option<String> {
    let mut iter = names.iter();
    let first = iter.next()?.as_str();
    let mut prefix = first.to_string();
    for name in iter {
        prefix = common_prefix(&prefix, name);
        if prefix.is_empty() {
            break;
        }
    }
    if prefix.is_empty() {
        names.iter().min_by_key(|name| name.len()).cloned()
    } else {
        Some(prefix)
    }
}

pub(super) fn common_prefix(left: &str, right: &str) -> String {
    left.chars()
        .zip(right.chars())
        .take_while(|(left, right)| left == right)
        .map(|(ch, _)| ch)
        .collect()
}

pub(super) fn parse_form_table_binding(field: &str) -> Option<(String, String)> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|field| field.trim()) != Some("2") {
        return None;
    }
    let attribute_id = split_1c_braced_fields(fields.get(1)?.trim(), 0)?
        .first()?
        .trim()
        .to_string();
    let table_key = parse_form_binding_key(fields.get(2)?.trim())?;
    Some((attribute_id, table_key))
}

pub(super) fn parse_form_nested_table_column_binding(
    field: &str,
) -> Option<(String, String, String)> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("3") {
        return None;
    }
    let attribute_id = split_1c_braced_fields(fields.get(1)?.trim(), 0)?
        .first()?
        .trim()
        .to_string();
    let table_key = parse_form_binding_key(fields.get(2)?.trim())?;
    let column_key = parse_form_binding_key(fields.get(3)?.trim())?;
    Some((attribute_id, table_key, column_key))
}

pub(super) fn form_child_item_binding_fields<'a>(tag: &str, fields: &'a [&'a str]) -> Vec<&'a str> {
    let input_field_offset = matches!(
        tag,
        "InputField"
            | "LabelField"
            | "CheckBoxField"
            | "PictureField"
            | "RadioButtonField"
            | "TextDocumentField"
    )
    .then(|| {
        form_input_field_layout_is_extended(fields)
            .then(|| form_input_field_top_level_offset(fields))
    })
    .flatten()
    .unwrap_or(0);
    match tag {
        "Table" => fields.get(11).copied().into_iter().collect(),
        "InputField" | "LabelField" | "CheckBoxField" | "PictureField" | "RadioButtonField"
        | "TextDocumentField" => [11usize, 12]
            .iter()
            .filter_map(|index| fields.get(*index + input_field_offset).copied())
            .collect(),
        _ => Vec::new(),
    }
}

pub(super) fn parse_form_attribute_binding_id(field: &str) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|field| field.trim()) != Some("1") {
        return None;
    }
    let ids = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
    Some(ids.first()?.trim().to_string())
}

pub(super) fn parse_form_attribute_data_path(
    field: &str,
    name: &str,
    attribute_names_by_id: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|field| field.trim()) != Some("1") {
        return None;
    }
    let ids = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
    let attribute_id = ids.first()?.trim();
    attribute_names_by_id
        .get(attribute_id)
        .cloned()
        .or_else(|| Some(name.to_string()))
}

pub(super) fn parse_form_button_command_name(
    field: &str,
    button_name: &str,
    commands: &[FormCommand],
    object_refs: &BTreeMap<String, String>,
    table_name_by_id: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    let kind = fields.first()?.trim();
    let uuid = parse_non_zero_uuid(fields.get(1)?.trim())?;
    if kind == "0" {
        return form_standard_button_command_name(&uuid)
            .or_else(|| form_standard_command_name(&uuid))
            .map(ToOwned::to_owned)
            .or_else(|| object_refs.get(&uuid).cloned());
    }
    if kind == "10" || kind == "21" {
        let standard = form_table_standard_command_suffix(&uuid)?;
        let table_name = if table_name_by_id.len() == 1 {
            table_name_by_id.values().next()?.as_str()
        } else {
            form_standard_command_table_name(button_name, table_name_by_id)?
        };
        return Some(format!("Form.Item.{table_name}.StandardCommand.{standard}"));
    }
    commands
        .iter()
        .find(|command| command.id == kind && command.reference_uuid == uuid)
        .map(|command| format!("Form.Command.{}", command.name))
        .or_else(|| {
            let standard = form_table_standard_command_suffix(&uuid)?;
            let table_name = if table_name_by_id.len() == 1 {
                table_name_by_id.values().next()?.as_str()
            } else {
                form_standard_command_table_name(button_name, table_name_by_id)?
            };
            Some(format!("Form.Item.{table_name}.StandardCommand.{standard}"))
        })
}

pub(super) fn form_standard_command_name(uuid: &str) -> Option<&'static str> {
    match uuid {
        FORM_COMMAND_CUSTOMIZE_FORM_UUID => Some("Form.StandardCommand.CustomizeForm"),
        "4f834c38-add1-45e4-a9f3-cefe3efac5c9" => Some("Form.StandardCommand.Create"),
        "3772996b-41f4-4c47-a5a8-ea397db424ae" => Some("Form.StandardCommand.Close"),
        "39bb0fe9-771d-4dd5-8a6e-2d16984523af" => Some("Form.StandardCommand.Help"),
        "32df4349-2607-4c2b-a4b9-bca4a1a28bd7" => Some("Form.StandardCommand.WriteAndClose"),
        "fe558fde-99b3-45d0-a060-9fc2905309f6" => Some("Form.StandardCommand.Write"),
        _ => None,
    }
}

pub(super) fn form_standard_button_command_name(uuid: &str) -> Option<&'static str> {
    match uuid {
        "239f0103-8de9-4fdf-b485-eb5531da7e51" => Some("Form.StandardCommand.SaveValues"),
        "71e0226e-ebb2-4e33-8745-0a94a01bbf15" => Some("Form.StandardCommand.RestoreValues"),
        _ => None,
    }
}

pub(super) fn form_table_standard_command_suffix(uuid: &str) -> Option<&'static str> {
    match uuid {
        "37740564-9e86-44a0-bea9-3f485a5a3f91" => Some("MoveUp"),
        "2bbe4e12-06d2-409b-a972-eea585125d83" => Some("SortListAsc"),
        "c0519548-2a9a-44de-a25e-faf01e089d4d" => Some("Find"),
        "44ad3ec9-f3c2-4913-9224-5f9fb6418743" => Some("CancelSearch"),
        "49602716-fea6-497f-8047-726404038857" => Some("OutputList"),
        "5048cc44-702b-44e3-8445-9af75c02724d" => Some("UncheckAll"),
        "58b2a785-23f6-4b0e-a324-9a1323285595" => Some("SortListDesc"),
        "8d772f97-c0ef-47c0-9cb0-efea28c61341" => Some("Delete"),
        "fa51b106-eae6-44c7-8054-76cbb3100603" => Some("MoveDown"),
        _ => None,
    }
}

pub(super) fn form_standard_command_table_name<'a>(
    button_name: &str,
    table_name_by_id: &'a BTreeMap<String, String>,
) -> Option<&'a String> {
    if table_name_by_id.len() == 1 {
        return table_name_by_id.values().next();
    }
    match button_name {
        "Найти" | "ОтменитьПоиск" | "ВывестиСписок" | "Удалить" => {
            table_name_by_id.values().next()
        }
        _ => None,
    }
}

pub(super) fn collect_form_table_column_names_for_table(
    fields: &[&str],
    columns: &mut BTreeMap<String, String>,
) {
    for field in fields {
        let field = field.trim();
        if !field.starts_with('{') {
            continue;
        }
        let Some(nested) = split_1c_braced_fields(field, 0) else {
            continue;
        };
        let wrapper = nested.first().map(|value| value.trim()).unwrap_or_default();
        if matches!(
            form_child_item_tag(wrapper, &nested),
            Some("InputField" | "LabelField")
        ) && let Some(identity) = nested
            .get(1)
            .and_then(|field| split_1c_braced_fields(field, 0))
            && let (Some(id), Some(name)) = (
                identity.first().map(|value| value.trim().to_string()),
                parse_form_child_item_name(wrapper, &nested),
            )
        {
            columns.insert(id, name);
        }
        collect_form_table_column_names_for_table(&nested, columns);
    }
}

pub(super) fn parse_form_button_data_path(
    field: &str,
    table_name_by_id: &BTreeMap<String, String>,
    table_column_names_by_id: &BTreeMap<String, BTreeMap<String, String>>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("2") {
        return None;
    }
    let table = fields
        .get(1)
        .and_then(|field| split_1c_braced_fields(field, 0))?;
    let table_id = table.first()?.trim();
    let table_name = table_name_by_id.get(table_id)?;
    let column = fields
        .get(2)
        .and_then(|field| split_1c_braced_fields(field, 0))
        .and_then(|fields| fields.first().map(|value| value.trim().to_string()))?;
    let field_name = if column == "8" {
        "Ссылка".to_string()
    } else {
        table_column_names_by_id
            .get(table_id)
            .and_then(|columns| columns.get(&column))
            .cloned()?
    };
    let field_name = normalize_form_table_column_name(table_name, &field_name);
    Some(format!("Items.{table_name}.CurrentData.{field_name}"))
}

pub(super) fn extract_form_command_interface(
    trailing: &[String],
    object_refs: &BTreeMap<String, String>,
) -> Option<FormCommandInterface> {
    trailing
        .get(3)
        .and_then(|field| parse_form_command_interface_container(field, object_refs))
        .or_else(|| {
            trailing
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != 3)
                .find_map(|(_, field)| parse_form_command_interface_container(field, object_refs))
        })
}

pub(super) fn parse_form_command_interface_container(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<FormCommandInterface> {
    let fields = split_1c_braced_fields(field, 0)?;
    if fields.first().map(|value| value.trim()) != Some("0") {
        return None;
    }
    let mut command_bar = Vec::new();
    let mut navigation_panel = Vec::new();
    for field in fields.iter().skip(2) {
        if let Some(item) = parse_form_command_interface_item(field, object_refs) {
            if item
                .command_group
                .as_deref()
                .is_some_and(|group| group.starts_with("FormNavigationPanel"))
            {
                navigation_panel.push(item);
            } else {
                command_bar.push(item);
            }
        }
    }
    if command_bar.is_empty() && navigation_panel.is_empty() {
        return None;
    }
    Some(FormCommandInterface {
        command_bar,
        navigation_panel,
    })
}

pub(super) fn parse_form_command_interface_item(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<FormCommandInterfaceItem> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("3") {
        return None;
    }
    let command = parse_form_command_interface_command(fields.get(2)?, object_refs)?;
    let item_type = parse_form_command_interface_item_type(fields.get(4).copied())?;
    let command_group = fields
        .get(5)
        .and_then(|field| parse_form_command_group_reference(field, object_refs));
    let index = fields
        .get(6)
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|index| *index > 0);
    let default_visible = match fields.get(7).map(|value| value.trim()) {
        Some("0") => Some(false),
        Some("1") => Some(true),
        _ => None,
    }
    .filter(|visible| !*visible);
    Some(FormCommandInterfaceItem {
        command,
        item_type,
        command_group,
        index,
        default_visible,
        visible_common: fields
            .get(8)
            .and_then(|value| parse_form_nested_common_bool(value))
            .filter(|common| !*common),
    })
}

pub(super) fn parse_form_command_interface_item_type(field: Option<&str>) -> Option<&'static str> {
    match field.map(str::trim) {
        Some("0") => Some("Auto"),
        Some("1") => Some("Added"),
        _ => None,
    }
}

pub(super) fn parse_form_command_interface_command(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    match fields.first().map(|value| value.trim()) {
        Some("0") => {
            let Some(target) = fields.get(1).map(|value| value.trim()) else {
                return Some("0".to_string());
            };
            if target == "0" || target == "00000000-0000-0000-0000-000000000000" {
                Some("0".to_string())
            } else {
                parse_non_zero_uuid(target).and_then(|uuid| object_refs.get(&uuid).cloned())
            }
        }
        Some("2") => {
            let uuid = parse_non_zero_uuid(fields.get(1)?.trim())?;
            object_refs
                .get(&uuid)
                .map(|reference| format!("{reference}.StandardCommand.CreateBasedOn"))
        }
        _ => parse_form_object_reference(field, object_refs),
    }
}

pub(super) fn parse_form_object_reference(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("0") {
        return None;
    }
    let uuid = parse_non_zero_uuid(fields.get(1)?.trim())?;
    object_refs.get(&uuid).cloned()
}

pub(super) fn parse_form_command_group_reference(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)?;
    if fields.first().map(|value| value.trim()) != Some("0") {
        return None;
    }
    let uuid = parse_non_zero_uuid(fields.get(1)?.trim())?;
    form_standard_command_group_name(&uuid)
        .map(ToOwned::to_owned)
        .or_else(|| object_refs.get(&uuid).cloned())
}

pub(super) fn form_standard_command_group_name(uuid: &str) -> Option<&'static str> {
    match uuid {
        "c59e11f3-6bcb-404a-9d76-1416c12be354" => Some("CommandGroup.Органайзер"),
        "dc2ade0f-383e-4c78-85f2-c0dabc0e2dc0" => Some("FormCommandBarCreateBasedOn"),
        "cb50f5c0-8013-4262-93a2-f0db379d6b6b" => Some("FormCommandBarImportant"),
        "eacad741-96b9-4b3a-bf79-dde9ecead1a1" => Some("FormNavigationPanelGoTo"),
        "8ab1540c-0bfa-4fa6-a1e1-5d5069efc7d8" => Some("FormNavigationPanelSeeAlso"),
        "dc11a6be-de1f-4b64-a7a5-9b17bf4ec9f2" => Some("FormNavigationPanelImportant"),
        _ => None,
    }
}

pub(super) fn parse_form_nested_common_bool(field: &str) -> Option<bool> {
    if field.contains(r#"{"B",1}"#) {
        Some(true)
    } else if field.contains(r#"{"B",0}"#) {
        Some(false)
    } else {
        None
    }
}

pub(super) fn dedup_form_item_assets(assets: Vec<FormItemAsset>) -> Vec<FormItemAsset> {
    let mut seen = BTreeSet::<(String, String)>::new();
    let mut deduped = Vec::new();
    for asset in assets {
        if seen.insert((asset.item_name.clone(), asset.file_name.clone())) {
            deduped.push(asset);
        }
    }
    deduped
}

pub(super) fn format_form_body_xml(
    properties: &FormBodyProperties,
    auto_command_bar: Option<&FormAutoCommandBar>,
    events: &[FormBodyEvent],
    child_items: &[FormChildItem],
    attributes: &[FormAttribute],
    attributes_section: &FormAttributesSection,
    parameters: &[FormParameter],
    commands: &[FormCommand],
    command_interface: &Option<FormCommandInterface>,
) -> String {
    let mut xml = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Form xmlns=\"http://v8.1c.ru/8.3/xcf/logform\" xmlns:app=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:dcssch=\"http://v8.1c.ru/8.1/data-composition-system/schema\" xmlns:dcsset=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" xmlns:lf=\"http://v8.1c.ru/8.2/managed-application/logform\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n\
"
    .to_string();
    xml.push_str(&format_form_localized_section(
        "Title",
        &properties.title,
        1,
    ));
    if let Some(width) = &properties.width {
        xml.push_str(&format!("\t<Width>{}</Width>\r\n", escape_xml_text(width)));
    }
    if let Some(height) = &properties.height {
        xml.push_str(&format!(
            "\t<Height>{}</Height>\r\n",
            escape_xml_text(height)
        ));
    }
    if let Some(window_opening_mode) = properties.window_opening_mode {
        xml.push_str(&format!(
            "\t<WindowOpeningMode>{}</WindowOpeningMode>\r\n",
            escape_xml_text(window_opening_mode)
        ));
    }
    if let Some(value) = properties.enter_key_behavior {
        xml.push_str(&format!(
            "\t<EnterKeyBehavior>{}</EnterKeyBehavior>\r\n",
            escape_xml_text(value)
        ));
    }
    if properties.save_window_settings == Some(false) {
        xml.push_str("\t<SaveWindowSettings>false</SaveWindowSettings>\r\n");
    }
    if properties.auto_url == Some(false) {
        xml.push_str("\t<AutoURL>false</AutoURL>\r\n");
    }
    if let Some(value) = properties.save_data_in_settings {
        xml.push_str(&format!(
            "\t<SaveDataInSettings>{}</SaveDataInSettings>\r\n",
            escape_xml_text(value)
        ));
    }
    if let Some(value) = properties.auto_save_data_in_settings {
        xml.push_str(&format!(
            "\t<AutoSaveDataInSettings>{}</AutoSaveDataInSettings>\r\n",
            escape_xml_text(value)
        ));
    }
    if properties.auto_title == Some(false) {
        xml.push_str("\t<AutoTitle>false</AutoTitle>\r\n");
    }
    if let Some(group) = properties.group.filter(|group| *group != "Vertical") {
        xml.push_str(&format!("\t<Group>{}</Group>\r\n", escape_xml_text(group)));
    }
    if let Some(value) = properties.scaling_mode {
        xml.push_str(&format!(
            "\t<ScalingMode>{}</ScalingMode>\r\n",
            escape_xml_text(value)
        ));
    }
    if let Some(value) = properties.auto_time {
        xml.push_str(&format!(
            "\t<AutoTime>{}</AutoTime>\r\n",
            escape_xml_text(value)
        ));
    }
    if let Some(value) = properties.use_posting_mode {
        xml.push_str(&format!(
            "\t<UsePostingMode>{}</UsePostingMode>\r\n",
            escape_xml_text(value)
        ));
    }
    if let Some(value) = properties.repost_on_write {
        xml.push_str(&format!(
            "\t<RepostOnWrite>{}</RepostOnWrite>\r\n",
            if value { "true" } else { "false" }
        ));
    }
    if properties.auto_fill_check == Some(false) {
        xml.push_str("\t<AutoFillCheck>false</AutoFillCheck>\r\n");
    }
    if let Some(value) = properties.use_for_folders_and_items {
        xml.push_str(&format!(
            "\t<UseForFoldersAndItems>{}</UseForFoldersAndItems>\r\n",
            escape_xml_text(value)
        ));
    }
    if properties.customizable == Some(false) {
        xml.push_str("\t<Customizable>false</Customizable>\r\n");
    }
    if let Some(command_bar_location) = properties.command_bar_location {
        xml.push_str(&format!(
            "\t<CommandBarLocation>{}</CommandBarLocation>\r\n",
            escape_xml_text(command_bar_location)
        ));
    }
    if !properties.command_set_excluded_commands.is_empty() {
        xml.push_str("\t<CommandSet>\r\n");
        for command in &properties.command_set_excluded_commands {
            xml.push_str(&format!(
                "\t\t<ExcludedCommand>{}</ExcludedCommand>\r\n",
                escape_xml_text(command)
            ));
        }
        xml.push_str("\t</CommandSet>\r\n");
    }
    if let Some(vertical_scroll) = properties.vertical_scroll {
        xml.push_str(&format!(
            "\t<VerticalScroll>{}</VerticalScroll>\r\n",
            escape_xml_text(vertical_scroll)
        ));
    }
    if let Some(horizontal_align) = properties.horizontal_align {
        xml.push_str(&format!(
            "\t<HorizontalAlign>{}</HorizontalAlign>\r\n",
            escape_xml_text(horizontal_align)
        ));
    }
    if let Some(conversations_representation) = properties.conversations_representation {
        xml.push_str(&format!(
            "\t<ConversationsRepresentation>{}</ConversationsRepresentation>\r\n",
            escape_xml_text(conversations_representation)
        ));
    }
    if properties.show_title == Some(false) {
        xml.push_str("\t<ShowTitle>false</ShowTitle>\r\n");
    }
    if let Some(show_command_bar) = properties.show_command_bar {
        xml.push_str(&format!(
            "\t<ShowCommandBar>{}</ShowCommandBar>\r\n",
            if show_command_bar { "true" } else { "false" }
        ));
    }
    if properties.show_close_button == Some(false) {
        xml.push_str("\t<ShowCloseButton>false</ShowCloseButton>\r\n");
    }
    if let Some(value) = &properties.report_result {
        xml.push_str(&format!(
            "\t<ReportResult>{}</ReportResult>\r\n",
            escape_xml_text(value)
        ));
    }
    if let Some(value) = &properties.details_data {
        xml.push_str(&format!(
            "\t<DetailsData>{}</DetailsData>\r\n",
            escape_xml_text(value)
        ));
    }
    if let Some(value) = properties.report_form_type {
        xml.push_str(&format!(
            "\t<ReportFormType>{}</ReportFormType>\r\n",
            escape_xml_text(value)
        ));
    }
    if let Some(value) = properties.auto_show_state {
        xml.push_str(&format!(
            "\t<AutoShowState>{}</AutoShowState>\r\n",
            escape_xml_text(value)
        ));
    }
    if let Some(value) = properties.report_result_view_mode {
        xml.push_str(&format!(
            "\t<ReportResultViewMode>{}</ReportResultViewMode>\r\n",
            escape_xml_text(value)
        ));
    }
    if let Some(value) = properties.view_mode_application_on_set_report_result {
        xml.push_str(&format!(
            "\t<ViewModeApplicationOnSetReportResult>{}</ViewModeApplicationOnSetReportResult>\r\n",
            escape_xml_text(value)
        ));
    }
    if let Some(command_bar) = auto_command_bar {
        if command_bar.horizontal_align.is_some()
            || command_bar.autofill == Some(false)
            || !command_bar.child_items.is_empty()
        {
            xml.push_str(&format!(
                "\t<AutoCommandBar name=\"{}\" id=\"{}\">\r\n",
                escape_xml_text(&command_bar.name),
                escape_xml_text(&command_bar.id)
            ));
            if let Some(horizontal_align) = command_bar.horizontal_align {
                xml.push_str(&format!(
                    "\t\t<HorizontalAlign>{}</HorizontalAlign>\r\n",
                    escape_xml_text(horizontal_align)
                ));
            }
            if command_bar.autofill == Some(false) {
                xml.push_str("\t\t<Autofill>false</Autofill>\r\n");
            }
            xml.push_str(&format_form_child_items_xml(&command_bar.child_items, 2));
            xml.push_str("\t</AutoCommandBar>\r\n");
        } else {
            xml.push_str(&format!(
                "\t<AutoCommandBar name=\"{}\" id=\"{}\"/>\r\n",
                escape_xml_text(&command_bar.name),
                escape_xml_text(&command_bar.id)
            ));
        }
    }
    if !events.is_empty() {
        xml.push_str("\t<Events>\r\n");
        for event in events {
            xml.push_str(&format!(
                "\t\t<Event name=\"{}\">{}</Event>\r\n",
                escape_xml_text(&event.name),
                escape_xml_text(&event.handler)
            ));
        }
        xml.push_str("\t</Events>\r\n");
    }
    xml.push_str(&format_form_child_items_xml(child_items, 1));
    xml.push_str(&format_form_attributes_section_xml(
        attributes,
        attributes_section,
    ));
    if !commands.is_empty() {
        xml.push_str("\t<Commands>\r\n");
        for command in commands {
            xml.push_str(&format!(
                "\t\t<Command name=\"{}\" id=\"{}\">\r\n",
                escape_xml_text(&command.name),
                escape_xml_text(&command.id)
            ));
            xml.push_str(&format_form_localized_section("Title", &command.title, 3));
            xml.push_str(&format_form_localized_section(
                "ToolTip",
                &command.tooltip,
                3,
            ));
            if let Some(shortcut) = command.shortcut.as_deref() {
                xml.push_str(&format!(
                    "\t\t\t<Shortcut>{}</Shortcut>\r\n",
                    escape_xml_element_text(shortcut)
                ));
            }
            if let Some(reference) = command.picture_ref.as_deref() {
                xml.push_str("\t\t\t<Picture>\r\n");
                xml.push_str(&format!(
                    "\t\t\t\t<xr:Ref>{}</xr:Ref>\r\n",
                    escape_xml_text(reference)
                ));
                xml.push_str(&format!(
                    "\t\t\t\t<xr:LoadTransparent>{}</xr:LoadTransparent>\r\n",
                    xml_bool(command.picture_load_transparent)
                ));
                xml.push_str("\t\t\t</Picture>\r\n");
            }
            if !command.action.is_empty() {
                xml.push_str(&format!(
                    "\t\t\t<Action>{}</Action>\r\n",
                    escape_xml_text(&command.action)
                ));
            }
            if let Some(representation) = command.representation {
                xml.push_str(&format!(
                    "\t\t\t<Representation>{}</Representation>\r\n",
                    escape_xml_text(representation)
                ));
            }
            if command.modifies_saved_data == Some(true) {
                xml.push_str("\t\t\t<ModifiesSavedData>true</ModifiesSavedData>\r\n");
            }
            if !command.functional_options.is_empty() {
                xml.push_str("\t\t\t<FunctionalOptions>\r\n");
                for item in &command.functional_options {
                    xml.push_str(&format!(
                        "\t\t\t\t<Item>{}</Item>\r\n",
                        escape_xml_text(item)
                    ));
                }
                xml.push_str("\t\t\t</FunctionalOptions>\r\n");
            }
            if let Some(current_row_use) = command.current_row_use {
                xml.push_str(&format!(
                    "\t\t\t<CurrentRowUse>{}</CurrentRowUse>\r\n",
                    escape_xml_text(current_row_use)
                ));
            }
            xml.push_str("\t\t</Command>\r\n");
        }
        xml.push_str("\t</Commands>\r\n");
    }
    xml.push_str(&format_form_parameters_xml(parameters));
    if let Some(command_interface) = command_interface {
        xml.push_str(&format_form_command_interface_xml(command_interface));
    }
    xml.push_str("</Form>");
    xml
}

pub(super) fn format_form_child_items_xml(items: &[FormChildItem], indent: usize) -> String {
    if items.is_empty() {
        return String::new();
    }
    let tab = "\t".repeat(indent);
    let mut xml = format!("{tab}<ChildItems>\r\n");
    for item in items {
        xml.push_str(&format_form_child_item_xml(item, indent + 1, false));
    }
    xml.push_str(&format!("{tab}</ChildItems>\r\n"));
    xml
}

pub(super) fn format_form_child_item_xml(
    item: &FormChildItem,
    indent: usize,
    table_addition_child: bool,
) -> String {
    if item.tag == "ContextMenu" {
        return format_form_context_menu_xml(item, indent);
    }
    let tab = "\t".repeat(indent);
    let early_title_for_field = matches!(
        item.tag,
        "InputField"
            | "LabelField"
            | "CheckBoxField"
            | "PictureField"
            | "RadioButtonField"
            | "TextDocumentField"
            | "ColumnGroup"
    );
    let usual_group_title_first = matches!(item.tag, "UsualGroup" | "ButtonGroup");
    let mut direct_context_menu_xml = String::new();
    let mut direct_regular_children = Vec::new();
    if is_form_field_direct_service_parent(item.tag) {
        for child in &item.child_items {
            if child.tag == "ContextMenu" {
                direct_context_menu_xml.push_str(&format_form_child_item_xml(
                    child,
                    indent + 1,
                    false,
                ));
            } else {
                direct_regular_children.push(child.clone());
            }
        }
    }
    let mut xml = format!(
        "{tab}<{} name=\"{}\" id=\"{}\">\r\n",
        item.tag,
        escape_xml_text(&item.name),
        escape_xml_text(&item.id)
    );
    if item.tag.ends_with("Addition") {
        if item.addition_source_item.is_some() || item.item_type.is_some() {
            xml.push_str(&format!("{tab}\t<AdditionSource>\r\n"));
            if let Some(source_item) = &item.addition_source_item {
                xml.push_str(&format!(
                    "{tab}\t\t<Item>{}</Item>\r\n",
                    escape_xml_text(source_item)
                ));
            }
            if let Some(item_type) = item.item_type {
                xml.push_str(&format!(
                    "{tab}\t\t<Type>{}</Type>\r\n",
                    escape_xml_text(item_type)
                ));
            }
            xml.push_str(&format!("{tab}\t</AdditionSource>\r\n"));
        }
    } else if let Some(item_type) = item.item_type {
        xml.push_str(&format!(
            "{tab}\t<Type>{}</Type>\r\n",
            escape_xml_text(item_type)
        ));
    }
    if item.tag == "Button"
        && let Some(representation) = item.button_representation.filter(|value| *value != "None")
    {
        xml.push_str(&format!(
            "{tab}\t<Representation>{}</Representation>\r\n",
            escape_xml_text(representation)
        ));
    }
    if matches!(item.tag, "Button" | "LabelDecoration")
        && let Some(group_horizontal_align) = item.group_horizontal_align
    {
        xml.push_str(&format!(
            "{tab}\t<GroupHorizontalAlign>{}</GroupHorizontalAlign>\r\n",
            escape_xml_text(group_horizontal_align)
        ));
    }
    if item.tag != "CommandBar"
        && let Some(horizontal_location) = item.horizontal_location
    {
        xml.push_str(&format!(
            "{tab}\t<HorizontalLocation>{}</HorizontalLocation>\r\n",
            escape_xml_text(horizontal_location)
        ));
    }
    if item.tag == "Button"
        && let Some(width) = &item.width
    {
        xml.push_str(&format!(
            "{tab}\t<Width>{}</Width>\r\n",
            escape_xml_text(width)
        ));
    }
    if item.tag != "Button"
        && let Some(command_name) = &item.command_name
    {
        xml.push_str(&format!(
            "{tab}\t<CommandName>{}</CommandName>\r\n",
            escape_xml_text(command_name)
        ));
    }
    if item.tag == "AutoCommandBar" && item.autofill == Some(false) {
        xml.push_str(&format!("{tab}\t<Autofill>false</Autofill>\r\n"));
    }
    if item.tag == "Table" {
        let hierarchical_table = form_table_has_hierarchical_navigation(item);
        if let Some(representation) = item.table_representation {
            if !hierarchical_table {
                xml.push_str(&format!(
                    "{tab}\t<Representation>{}</Representation>\r\n",
                    escape_xml_text(representation)
                ));
            }
        }
        if item.auto_mark_incomplete == Some(true) {
            xml.push_str(&format!(
                "{tab}\t<AutoMarkIncomplete>true</AutoMarkIncomplete>\r\n"
            ));
        }
        if let Some(skip_on_input) = item.skip_on_input
            && (skip_on_input || should_emit_explicit_table_skip_on_input(item))
        {
            xml.push_str(&format!(
                "{tab}\t<SkipOnInput>{}</SkipOnInput>\r\n",
                if skip_on_input { "true" } else { "false" }
            ));
        }
        if hierarchical_table && item.use_alternation_row_color == Some(true) {
            xml.push_str(&format!(
                "{tab}\t<UseAlternationRowColor>true</UseAlternationRowColor>\r\n"
            ));
        }
        if item.read_only == Some(true) {
            xml.push_str(&format!("{tab}\t<ReadOnly>true</ReadOnly>\r\n"));
        }
        if item.change_row_set == Some(false) {
            xml.push_str(&format!("{tab}\t<ChangeRowSet>false</ChangeRowSet>\r\n"));
        }
        if let Some(height) = &item.height {
            xml.push_str(&format!(
                "{tab}\t<Height>{}</Height>\r\n",
                escape_xml_text(height)
            ));
        }
        if item.auto_max_height == Some(false) {
            xml.push_str(&format!("{tab}\t<AutoMaxHeight>false</AutoMaxHeight>\r\n"));
        }
        if let Some(height) = &item.height_in_table_rows {
            xml.push_str(&format!(
                "{tab}\t<HeightInTableRows>{}</HeightInTableRows>\r\n",
                escape_xml_text(height)
            ));
        }
        if item.default_item == Some(true) && !hierarchical_table {
            xml.push_str(&format!("{tab}\t<DefaultItem>true</DefaultItem>\r\n"));
        }
        if item.change_row_order == Some(false) {
            xml.push_str(&format!(
                "{tab}\t<ChangeRowOrder>false</ChangeRowOrder>\r\n"
            ));
        }
        if !hierarchical_table && item.auto_max_width == Some(false) {
            xml.push_str(&format!("{tab}\t<AutoMaxWidth>false</AutoMaxWidth>\r\n"));
        }
        if let Some(row_input_mode) = item.row_input_mode {
            xml.push_str(&format!(
                "{tab}\t<RowInputMode>{}</RowInputMode>\r\n",
                escape_xml_text(row_input_mode)
            ));
        }
        if !hierarchical_table
            && let Some(use_alternation_row_color) = item.use_alternation_row_color
        {
            xml.push_str(&format!(
                "{tab}\t<UseAlternationRowColor>{}</UseAlternationRowColor>\r\n",
                if use_alternation_row_color {
                    "true"
                } else {
                    "false"
                }
            ));
        }
        if item.auto_insert_new_row == Some(true) {
            xml.push_str(&format!(
                "{tab}\t<AutoInsertNewRow>true</AutoInsertNewRow>\r\n"
            ));
        }
        if item.enable_start_drag == Some(true) {
            xml.push_str(&format!(
                "{tab}\t<EnableStartDrag>true</EnableStartDrag>\r\n"
            ));
        }
        if item.enable_drag == Some(true) && (!hierarchical_table || item.read_only == Some(true)) {
            xml.push_str(&format!("{tab}\t<EnableDrag>true</EnableDrag>\r\n"));
        }
        if !hierarchical_table && let Some(file_drag_mode) = item.file_drag_mode {
            xml.push_str(&format!(
                "{tab}\t<FileDragMode>{}</FileDragMode>\r\n",
                escape_xml_text(file_drag_mode)
            ));
        }
        if let Some(data_path) = &item.data_path {
            xml.push_str(&format!(
                "{tab}\t<DataPath>{}</DataPath>\r\n",
                escape_xml_text(data_path)
            ));
        }
        if let Some(row_picture_data_path) = &item.row_picture_data_path {
            xml.push_str(&format!(
                "{tab}\t<RowPictureDataPath>{}</RowPictureDataPath>\r\n",
                escape_xml_text(row_picture_data_path)
            ));
        }
        if let Some(reference) = &item.rows_picture_ref {
            xml.push_str(&format!(
                "{tab}\t<RowsPicture>\r\n\
{tab}\t\t<xr:Ref>{}</xr:Ref>\r\n\
{tab}\t\t<xr:LoadTransparent>{}</xr:LoadTransparent>\r\n\
{tab}\t</RowsPicture>\r\n",
                escape_xml_text(reference),
                xml_bool(item.rows_picture_load_transparent)
            ));
        }
        xml.push_str(&format_form_localized_section(
            "Title",
            &item.title,
            indent + 1,
        ));
        if !item.command_set_excluded_commands.is_empty() {
            xml.push_str(&format!("{tab}\t<CommandSet>\r\n"));
            for command in &item.command_set_excluded_commands {
                xml.push_str(&format!(
                    "{tab}\t\t<ExcludedCommand>{}</ExcludedCommand>\r\n",
                    escape_xml_text(command)
                ));
            }
            xml.push_str(&format!("{tab}\t</CommandSet>\r\n"));
        }
        if let Some(auto_refresh) = item.auto_refresh {
            xml.push_str(&format!(
                "{tab}\t<AutoRefresh>{}</AutoRefresh>\r\n",
                if auto_refresh { "true" } else { "false" }
            ));
        }
        if let Some(auto_refresh_period) = &item.auto_refresh_period {
            xml.push_str(&format!(
                "{tab}\t<AutoRefreshPeriod>{}</AutoRefreshPeriod>\r\n",
                escape_xml_text(auto_refresh_period)
            ));
        }
        if let Some(period) = &item.period {
            xml.push_str(&format!(
                "{tab}\t<Period>\r\n\
{tab}\t\t<v8:variant xsi:type=\"v8:StandardPeriodVariant\">{}</v8:variant>\r\n\
{tab}\t\t<v8:startDate>{}</v8:startDate>\r\n\
{tab}\t\t<v8:endDate>{}</v8:endDate>\r\n\
{tab}\t</Period>\r\n",
                escape_xml_text(period.variant),
                escape_xml_text(&period.start_date),
                escape_xml_text(&period.end_date)
            ));
        }
        if let Some(choice_folders_and_items) = item.choice_folders_and_items {
            xml.push_str(&format!(
                "{tab}\t<ChoiceFoldersAndItems>{}</ChoiceFoldersAndItems>\r\n",
                escape_xml_text(choice_folders_and_items)
            ));
        }
        if let Some(restore_current_row) = item.restore_current_row {
            xml.push_str(&format!(
                "{tab}\t<RestoreCurrentRow>{}</RestoreCurrentRow>\r\n",
                if restore_current_row { "true" } else { "false" }
            ));
        }
        if item.top_level_parent_nil == Some(true) {
            xml.push_str(&format!("{tab}\t<TopLevelParent xsi:nil=\"true\"/>\r\n"));
        }
        if item.show_root == Some(true) {
            xml.push_str(&format!("{tab}\t<ShowRoot>true</ShowRoot>\r\n"));
        }
        if item.allow_root_choice == Some(false) {
            xml.push_str(&format!(
                "{tab}\t<AllowRootChoice>false</AllowRootChoice>\r\n"
            ));
        }
        if let Some(update_on_data_change) = item.update_on_data_change {
            xml.push_str(&format!(
                "{tab}\t<UpdateOnDataChange>{}</UpdateOnDataChange>\r\n",
                escape_xml_text(update_on_data_change)
            ));
        }
        if let Some(user_settings_group) = &item.user_settings_group {
            xml.push_str(&format!(
                "{tab}\t<UserSettingsGroup>{}</UserSettingsGroup>\r\n",
                escape_xml_text(user_settings_group)
            ));
        }
        if let Some(allow_getting_current_row_url) = item.allow_getting_current_row_url {
            xml.push_str(&format!(
                "{tab}\t<AllowGettingCurrentRowURL>{}</AllowGettingCurrentRowURL>\r\n",
                if allow_getting_current_row_url {
                    "true"
                } else {
                    "false"
                }
            ));
        }
    }
    if item.tag != "Table"
        && let Some(data_path) = &item.data_path
    {
        xml.push_str(&format!(
            "{tab}\t<DataPath>{}</DataPath>\r\n",
            escape_xml_text(data_path)
        ));
    }
    if item.visible == Some(false) {
        xml.push_str(&format!("{tab}\t<Visible>false</Visible>\r\n"));
    }
    if item.user_visible_common == Some(false) {
        xml.push_str(&format!(
            "{tab}\t<UserVisible>\r\n{tab}\t\t<xr:Common>false</xr:Common>\r\n{tab}\t</UserVisible>\r\n"
        ));
    }
    let read_only_before_title = item.tag != "Table"
        && item.read_only == Some(true)
        && matches!(item.tag, "InputField" | "LabelField");
    if read_only_before_title {
        xml.push_str(&format!("{tab}\t<ReadOnly>true</ReadOnly>\r\n"));
    }
    if early_title_for_field {
        xml.push_str(&format_form_title_section(item, indent + 1));
        if matches!(item.tag, "InputField" | "LabelField" | "CheckBoxField")
            && item.show_in_header == Some(false)
        {
            xml.push_str(&format!("{tab}\t<ShowInHeader>false</ShowInHeader>\r\n"));
        }
    }
    if item.tag == "Table"
        && !form_table_has_hierarchical_navigation(item)
        && item.row_filter_nil == Some(true)
    {
        xml.push_str(&format!("{tab}\t<RowFilter xsi:nil=\"true\"/>\r\n"));
    }
    if item.default_button == Some(true) {
        xml.push_str(&format!("{tab}\t<DefaultButton>true</DefaultButton>\r\n"));
    }
    if item.tag != "Table" && item.read_only == Some(true) && !read_only_before_title {
        xml.push_str(&format!("{tab}\t<ReadOnly>true</ReadOnly>\r\n"));
    }
    if item.tag != "LabelDecoration"
        && item.tag != "Table"
        && let Some(skip_on_input) = item.skip_on_input
    {
        xml.push_str(&format!(
            "{tab}\t<SkipOnInput>{}</SkipOnInput>\r\n",
            if skip_on_input { "true" } else { "false" }
        ));
    }
    if let Some(title_location) = item.title_location {
        xml.push_str(&format!(
            "{tab}\t<TitleLocation>{}</TitleLocation>\r\n",
            escape_xml_text(title_location)
        ));
    }
    if matches!(item.tag, "InputField" | "PictureField") {
        xml.push_str(&format_form_localized_section(
            "ToolTip",
            &item.tooltip,
            indent + 1,
        ));
        if let Some(tooltip_representation) = item.tooltip_representation {
            xml.push_str(&format!(
                "{tab}\t<ToolTipRepresentation>{}</ToolTipRepresentation>\r\n",
                escape_xml_text(tooltip_representation)
            ));
        }
    }
    if let Some(horizontal_align) = item.horizontal_align {
        xml.push_str(&format!(
            "{tab}\t<HorizontalAlign>{}</HorizontalAlign>\r\n",
            escape_xml_text(horizontal_align)
        ));
    }
    if let Some(edit_mode) = item.edit_mode {
        xml.push_str(&format!(
            "{tab}\t<EditMode>{}</EditMode>\r\n",
            escape_xml_text(edit_mode)
        ));
    }
    if item.tag == "PictureField"
        && let Some(reference) = &item.picture_ref
    {
        xml.push_str(&format!(
            "{tab}\t<ValuesPicture>\r\n\
{tab}\t\t<xr:Ref>{}</xr:Ref>\r\n\
{tab}\t\t<xr:LoadTransparent>{}</xr:LoadTransparent>\r\n\
{tab}\t</ValuesPicture>\r\n",
            escape_xml_text(reference),
            xml_bool(item.picture_load_transparent)
        ));
        if let Some(file_drag_mode) = item.file_drag_mode {
            xml.push_str(&format!(
                "{tab}\t<FileDragMode>{}</FileDragMode>\r\n",
                escape_xml_text(file_drag_mode)
            ));
        }
    }
    if let Some(check_box_type) = item.check_box_type {
        xml.push_str(&format!(
            "{tab}\t<CheckBoxType>{}</CheckBoxType>\r\n",
            escape_xml_text(check_box_type)
        ));
    }
    if let Some(radio_button_type) = item.radio_button_type {
        xml.push_str(&format!(
            "{tab}\t<RadioButtonType>{}</RadioButtonType>\r\n",
            escape_xml_text(radio_button_type)
        ));
    }
    if let Some(columns_count) = item.columns_count {
        xml.push_str(&format!(
            "{tab}\t<ColumnsCount>{columns_count}</ColumnsCount>\r\n"
        ));
    }
    if item.cell_hyperlink == Some(true) {
        xml.push_str(&format!("{tab}\t<CellHyperlink>true</CellHyperlink>\r\n"));
    }
    if item.show_in_footer == Some(false) {
        xml.push_str(&format!("{tab}\t<ShowInFooter>false</ShowInFooter>\r\n"));
    }
    if let Some(footer_horizontal_align) = item.footer_horizontal_align {
        xml.push_str(&format!(
            "{tab}\t<FooterHorizontalAlign>{}</FooterHorizontalAlign>\r\n",
            escape_xml_text(footer_horizontal_align)
        ));
    }
    if item.tag != "Button"
        && let Some(width) = &item.width
    {
        xml.push_str(&format!(
            "{tab}\t<Width>{}</Width>\r\n",
            escape_xml_text(width)
        ));
    }
    if item.tag != "Table" && item.auto_max_width == Some(false) {
        xml.push_str(&format!("{tab}\t<AutoMaxWidth>false</AutoMaxWidth>\r\n"));
    }
    if item.tag != "Table"
        && let Some(height) = &item.height
    {
        xml.push_str(&format!(
            "{tab}\t<Height>{}</Height>\r\n",
            escape_xml_text(height)
        ));
    }
    if item.tag == "LabelDecoration"
        && let Some(skip_on_input) = item.skip_on_input
    {
        xml.push_str(&format!(
            "{tab}\t<SkipOnInput>{}</SkipOnInput>\r\n",
            if skip_on_input { "true" } else { "false" }
        ));
    }
    if let Some(max_width) = &item.max_width {
        xml.push_str(&format!(
            "{tab}\t<MaxWidth>{}</MaxWidth>\r\n",
            escape_xml_text(max_width)
        ));
    }
    if item.tag == "Button"
        && let Some(command_name) = &item.command_name
    {
        xml.push_str(&format!(
            "{tab}\t<CommandName>{}</CommandName>\r\n",
            escape_xml_text(command_name)
        ));
    }
    if item.tag == "Button"
        && let Some(reference) = &item.picture_ref
    {
        xml.push_str(&format!(
            "{tab}\t<Picture>\r\n\
{tab}\t\t<xr:Ref>{}</xr:Ref>\r\n\
{tab}\t\t<xr:LoadTransparent>{}</xr:LoadTransparent>\r\n\
{tab}\t</Picture>\r\n",
            escape_xml_text(reference),
            xml_bool(item.picture_load_transparent)
        ));
    }
    if item.tag != "LabelDecoration" && item.hiperlink == Some(true) {
        xml.push_str(&format!("{tab}\t<Hiperlink>true</Hiperlink>\r\n"));
    }
    if item.tag != "PictureDecoration"
        && let Some(text_color) = &item.text_color
    {
        xml.push_str(&format!(
            "{tab}\t<TextColor>{}</TextColor>\r\n",
            escape_xml_text(text_color)
        ));
    }
    if item.tag != "Table" && item.auto_max_height == Some(false) {
        xml.push_str(&format!("{tab}\t<AutoMaxHeight>false</AutoMaxHeight>\r\n"));
    }
    if let Some(max_height) = &item.max_height {
        xml.push_str(&format!(
            "{tab}\t<MaxHeight>{}</MaxHeight>\r\n",
            escape_xml_text(max_height)
        ));
    }
    if let Some(horizontal_stretch) = item.horizontal_stretch
        && !usual_group_title_first
    {
        xml.push_str(&format!(
            "{tab}\t<HorizontalStretch>{}</HorizontalStretch>\r\n",
            if horizontal_stretch { "true" } else { "false" }
        ));
    }
    if item.tag != "Table"
        && let Some(choice_folders_and_items) = item.choice_folders_and_items
    {
        xml.push_str(&format!(
            "{tab}\t<ChoiceFoldersAndItems>{}</ChoiceFoldersAndItems>\r\n",
            escape_xml_text(choice_folders_and_items)
        ));
    }
    if let Some(vertical_stretch) = item.vertical_stretch {
        xml.push_str(&format!(
            "{tab}\t<VerticalStretch>{}</VerticalStretch>\r\n",
            if vertical_stretch { "true" } else { "false" }
        ));
    }
    if let Some(password_mode) = item.password_mode {
        xml.push_str(&format!(
            "{tab}\t<PasswordMode>{}</PasswordMode>\r\n",
            if password_mode { "true" } else { "false" }
        ));
    }
    if let Some(multi_line) = item.multi_line {
        xml.push_str(&format!(
            "{tab}\t<MultiLine>{}</MultiLine>\r\n",
            if multi_line { "true" } else { "false" }
        ));
    }
    if item.wrap == Some(false) {
        xml.push_str(&format!("{tab}\t<Wrap>false</Wrap>\r\n"));
    }
    if item.auto_choice_incomplete == Some(true) {
        xml.push_str(&format!(
            "{tab}\t<AutoChoiceIncomplete>true</AutoChoiceIncomplete>\r\n"
        ));
    }
    if item.tag != "Table"
        && let Some(auto_mark_incomplete) = item.auto_mark_incomplete
    {
        xml.push_str(&format!(
            "{tab}\t<AutoMarkIncomplete>{}</AutoMarkIncomplete>\r\n",
            if auto_mark_incomplete {
                "true"
            } else {
                "false"
            }
        ));
    }
    if item.auto_cell_height == Some(true) {
        xml.push_str(&format!("{tab}\t<AutoCellHeight>true</AutoCellHeight>\r\n"));
    }
    if let Some(drop_list_button) = item.drop_list_button {
        xml.push_str(&format!(
            "{tab}\t<DropListButton>{}</DropListButton>\r\n",
            if drop_list_button { "true" } else { "false" }
        ));
    }
    if let Some(clear_button) = item.clear_button {
        xml.push_str(&format!(
            "{tab}\t<ClearButton>{}</ClearButton>\r\n",
            if clear_button { "true" } else { "false" }
        ));
    }
    if let Some(open_button) = item.open_button {
        xml.push_str(&format!(
            "{tab}\t<OpenButton>{}</OpenButton>\r\n",
            if open_button { "true" } else { "false" }
        ));
    }
    if let Some(create_button) = item.create_button {
        xml.push_str(&format!(
            "{tab}\t<CreateButton>{}</CreateButton>\r\n",
            if create_button { "true" } else { "false" }
        ));
    }
    if let Some(choice_button) = item.choice_button {
        xml.push_str(&format!(
            "{tab}\t<ChoiceButton>{}</ChoiceButton>\r\n",
            if choice_button { "true" } else { "false" }
        ));
    }
    if let Some(choice_list_button) = item.choice_list_button {
        xml.push_str(&format!(
            "{tab}\t<ChoiceListButton>{}</ChoiceListButton>\r\n",
            if choice_list_button { "true" } else { "false" }
        ));
    }
    if let Some(spin_button) = item.spin_button {
        xml.push_str(&format!(
            "{tab}\t<SpinButton>{}</SpinButton>\r\n",
            if spin_button { "true" } else { "false" }
        ));
    }
    if item.list_choice_mode == Some(true) {
        xml.push_str(&format!("{tab}\t<ListChoiceMode>true</ListChoiceMode>\r\n"));
    }
    if !item.choice_list.is_empty() {
        xml.push_str(&format_form_choice_list_xml(&item.choice_list, indent + 1));
    }
    if let Some(quick_choice) = item.quick_choice {
        xml.push_str(&format!(
            "{tab}\t<QuickChoice>{}</QuickChoice>\r\n",
            if quick_choice { "true" } else { "false" }
        ));
    }
    if item.choose_type == Some(false) {
        xml.push_str(&format!("{tab}\t<ChooseType>false</ChooseType>\r\n"));
    }
    if item.text_edit == Some(false) {
        xml.push_str(&format!("{tab}\t<TextEdit>false</TextEdit>\r\n"));
    }
    if let Some(choice_button_representation) = item.choice_button_representation {
        xml.push_str(&format!(
            "{tab}\t<ChoiceButtonRepresentation>{}</ChoiceButtonRepresentation>\r\n",
            escape_xml_text(choice_button_representation)
        ));
    }
    if !item.input_hint.is_empty() {
        xml.push_str(&format_form_localized_section(
            "InputHint",
            &item.input_hint,
            indent + 1,
        ));
    }
    if usual_group_title_first {
        xml.push_str(&format_form_localized_section(
            "Title",
            &item.title,
            indent + 1,
        ));
        if item.tag == "ButtonGroup" {
            xml.push_str(&format_form_localized_section(
                "ToolTip",
                &item.tooltip,
                indent + 1,
            ));
        }
        if item.tag == "UsualGroup" {
            if let Some(horizontal_stretch) = item.horizontal_stretch {
                xml.push_str(&format!(
                    "{tab}\t<HorizontalStretch>{}</HorizontalStretch>\r\n",
                    if horizontal_stretch { "true" } else { "false" }
                ));
            }
        } else if item.horizontal_stretch == Some(true) {
            xml.push_str(&format!(
                "{tab}\t<HorizontalStretch>true</HorizontalStretch>\r\n"
            ));
        }
    }
    if let Some(group) = item
        .group
        .filter(|group| !(item.tag == "Page" && *group == "Vertical"))
    {
        xml.push_str(&format!(
            "{tab}\t<Group>{}</Group>\r\n",
            escape_xml_text(group)
        ));
    }
    if let Some(scroll_on_compress) =
        form_bool_when_not_native_default(item.scroll_on_compress, false)
    {
        xml.push_str(&format!(
            "{tab}\t<ScrollOnCompress>{}</ScrollOnCompress>\r\n",
            if scroll_on_compress { "true" } else { "false" }
        ));
    }
    if let Some(behavior) = item.behavior {
        xml.push_str(&format!(
            "{tab}\t<Behavior>{}</Behavior>\r\n",
            escape_xml_text(behavior)
        ));
    }
    if let Some(representation) = item.representation.filter(|representation| {
        !(*representation == "WeakSeparation"
            && item.tag == "UsualGroup"
            && item.show_title == Some(false)
            && item.behavior == Some("Usual"))
    }) {
        let tag_name = if item.tag == "Pages" {
            "PagesRepresentation"
        } else {
            "Representation"
        };
        xml.push_str(&format!(
            "{tab}\t<{tag_name}>{}</{tag_name}>\r\n",
            escape_xml_text(representation)
        ));
    }
    if item.show_title == Some(false) {
        xml.push_str(&format!("{tab}\t<ShowTitle>false</ShowTitle>\r\n"));
    }
    if item.tag == "ColumnGroup" && item.show_in_header == Some(true) {
        xml.push_str(&format!("{tab}\t<ShowInHeader>true</ShowInHeader>\r\n"));
    }
    if !item.format.is_empty() {
        xml.push_str(&format_form_localized_section(
            "Format",
            &item.format,
            indent + 1,
        ));
    }
    if !item.edit_format.is_empty() {
        xml.push_str(&format_form_localized_section(
            "EditFormat",
            &item.edit_format,
            indent + 1,
        ));
    }
    if let Some(font_xml) = &item.font_xml {
        xml.push_str(&format!("{tab}\t{font_xml}\r\n"));
    }
    if item.tag == "PictureDecoration"
        && let Some(text_color) = &item.text_color
    {
        xml.push_str(&format!(
            "{tab}\t<TextColor>{}</TextColor>\r\n",
            escape_xml_text(text_color)
        ));
    }
    if !early_title_for_field && !usual_group_title_first && item.tag != "Table" {
        xml.push_str(&format_form_title_section(item, indent + 1));
        if item.tag == "LabelDecoration" && item.hiperlink == Some(true) {
            xml.push_str(&format!("{tab}\t<Hyperlink>true</Hyperlink>\r\n"));
        }
        if item.tag == "PictureDecoration" {
            if let Some(picture_size) = item.picture_size {
                xml.push_str(&format!(
                    "{tab}\t<PictureSize>{}</PictureSize>\r\n",
                    escape_xml_text(picture_size)
                ));
            }
            if let Some(reference) = &item.picture_ref {
                xml.push_str(&format!(
                    "{tab}\t<Picture>\r\n\
{tab}\t\t<xr:Ref>{}</xr:Ref>\r\n\
{tab}\t\t<xr:LoadTransparent>{}</xr:LoadTransparent>\r\n\
{tab}\t</Picture>\r\n",
                    escape_xml_text(reference),
                    xml_bool(item.picture_load_transparent)
                ));
            }
            if let Some(file_drag_mode) = item.file_drag_mode {
                xml.push_str(&format!(
                    "{tab}\t<FileDragMode>{}</FileDragMode>\r\n",
                    escape_xml_text(file_drag_mode)
                ));
            }
        }
    }
    if item.tag == "CommandBar"
        && let Some(horizontal_location) = item.horizontal_location
    {
        xml.push_str(&format!(
            "{tab}\t<HorizontalLocation>{}</HorizontalLocation>\r\n",
            escape_xml_text(horizontal_location)
        ));
    }
    if item.tag == "Button"
        && let Some(location) = item.location_in_command_bar
    {
        xml.push_str(&format!(
            "{tab}\t<LocationInCommandBar>{}</LocationInCommandBar>\r\n",
            escape_xml_text(location)
        ));
    }
    if let Some(command_source) = item.command_source {
        xml.push_str(&format!(
            "{tab}\t<CommandSource>{}</CommandSource>\r\n",
            escape_xml_text(command_source)
        ));
    }
    if item.tag == "Popup"
        && let Some(reference) = &item.picture_ref
    {
        xml.push_str(&format!(
            "{tab}\t<Picture>\r\n\
{tab}\t\t<xr:Ref>{}</xr:Ref>\r\n\
{tab}\t\t<xr:LoadTransparent>{}</xr:LoadTransparent>\r\n\
{tab}\t</Picture>\r\n",
            escape_xml_text(reference),
            xml_bool(item.picture_load_transparent)
        ));
    }
    if !matches!(item.tag, "InputField" | "PictureField" | "ButtonGroup") {
        xml.push_str(&format_form_localized_section(
            "ToolTip",
            &item.tooltip,
            indent + 1,
        ));
    }
    if !direct_context_menu_xml.is_empty() {
        xml.push_str(&direct_context_menu_xml);
    }
    if item.tag != "Table"
        && let Some((name, id)) = &item.extended_tooltip
    {
        xml.push_str(&format!(
            "{tab}\t<ExtendedTooltip name=\"{}\" id=\"{}\"/>\r\n",
            escape_xml_text(name),
            escape_xml_text(id)
        ));
    }
    if let Some(file_name) = item.picture_file_name {
        xml.push_str(&format!(
            "{tab}\t<Picture>\r\n\
{tab}\t\t<xr:Abs>{}</xr:Abs>\r\n\
{tab}\t\t<xr:LoadTransparent>false</xr:LoadTransparent>\r\n\
{tab}\t</Picture>\r\n",
            escape_xml_text(file_name)
        ));
    }
    if item.tag != "Table" && !item.events.is_empty() {
        xml.push_str(&format!("{tab}\t<Events>\r\n"));
        for event in &item.events {
            xml.push_str(&format!(
                "{tab}\t\t<Event name=\"{}\">{}</Event>\r\n",
                escape_xml_text(&event.name),
                escape_xml_text(&event.handler)
            ));
        }
        xml.push_str(&format!("{tab}\t</Events>\r\n"));
    }
    if item.tag == "Table" {
        let mut context_menu_children = Vec::new();
        let mut auto_command_bar_children = Vec::new();
        let mut addition_children = Vec::new();
        let mut regular_children = Vec::new();
        for child in &item.child_items {
            match child.tag {
                "ContextMenu" => context_menu_children.push(child.clone()),
                "AutoCommandBar" => auto_command_bar_children.push(child.clone()),
                "SearchStringAddition" | "ViewStatusAddition" | "SearchControlAddition" => {
                    addition_children.push(child.clone())
                }
                _ if is_form_table_service_child_item(child.tag) => {
                    addition_children.push(child.clone())
                }
                _ => {
                    regular_children.push(child.clone());
                }
            }
        }
        for child in &context_menu_children {
            xml.push_str(&format_form_child_item_xml(child, indent + 1, false));
        }
        for child in &auto_command_bar_children {
            xml.push_str(&format_form_child_item_xml(child, indent + 1, false));
        }
        if let Some((name, id)) = &item.extended_tooltip {
            xml.push_str(&format!(
                "{tab}\t<ExtendedTooltip name=\"{}\" id=\"{}\"/>\r\n",
                escape_xml_text(name),
                escape_xml_text(id)
            ));
        }
        for child in &addition_children {
            xml.push_str(&format_form_child_item_xml(child, indent + 1, false));
        }
        if !item.events.is_empty() {
            xml.push_str(&format!("{tab}\t<Events>\r\n"));
            for event in &item.events {
                xml.push_str(&format!(
                    "{tab}\t\t<Event name=\"{}\">{}</Event>\r\n",
                    escape_xml_text(&event.name),
                    escape_xml_text(&event.handler)
                ));
            }
            xml.push_str(&format!("{tab}\t</Events>\r\n"));
        }
        xml.push_str(&format_form_child_items_xml(&regular_children, indent + 1));
    } else if item.tag.ends_with("Addition") && !is_form_field_direct_service_parent(item.tag) {
        for child in &item.child_items {
            xml.push_str(&format_form_child_item_xml(child, indent + 1, false));
        }
    } else if is_form_field_direct_service_parent(item.tag) {
        if !table_addition_child {
            xml.push_str(&format_form_child_items_xml(
                &direct_regular_children,
                indent + 1,
            ));
        }
    } else if !table_addition_child {
        xml.push_str(&format_form_child_items_xml(&item.child_items, indent + 1));
    }
    xml.push_str(&format!("{tab}</{}>\r\n", item.tag));
    xml
}

pub(super) fn format_form_choice_list_xml(items: &[FormChoiceListItem], indent: usize) -> String {
    let tab = "\t".repeat(indent);
    let mut xml = format!("{tab}<ChoiceList>\r\n");
    for item in items {
        xml.push_str(&format!(
            "{tab}\t<xr:Item>\r\n\
{tab}\t\t<xr:Presentation/>\r\n\
{tab}\t\t<xr:CheckState>0</xr:CheckState>\r\n\
{tab}\t\t<xr:Value xsi:type=\"FormChoiceListDesTimeValue\">\r\n"
        ));
        if !item.presentation.is_empty() {
            xml.push_str(&format_form_localized_section(
                "Presentation",
                &item.presentation,
                indent + 3,
            ));
        }
        match &item.value {
            FormChoiceListValue::Decimal(value) => xml.push_str(&format!(
                "{tab}\t\t\t<Value xsi:type=\"xs:decimal\">{}</Value>\r\n",
                escape_xml_text(value)
            )),
            FormChoiceListValue::String(value) => xml.push_str(&format!(
                "{tab}\t\t\t<Value xsi:type=\"xs:string\">{}</Value>\r\n",
                escape_xml_text(value)
            )),
            FormChoiceListValue::DesignTimeRef(value) => xml.push_str(&format!(
                "{tab}\t\t\t<Value xsi:type=\"xr:DesignTimeRef\">{}</Value>\r\n",
                escape_xml_text(value)
            )),
        }
        xml.push_str(&format!(
            "{tab}\t\t</xr:Value>\r\n\
{tab}\t</xr:Item>\r\n"
        ));
    }
    xml.push_str(&format!("{tab}</ChoiceList>\r\n"));
    xml
}

pub(super) fn should_emit_explicit_table_skip_on_input(item: &FormChildItem) -> bool {
    item.tag == "Table"
        && !form_table_has_hierarchical_navigation(item)
        && item.skip_on_input == Some(false)
        && (item.row_picture_data_path.is_some() || item.rows_picture_ref.is_some())
}

pub(super) fn format_form_context_menu_xml(item: &FormChildItem, indent: usize) -> String {
    let tab = "\t".repeat(indent);
    if item.autofill == Some(false) || !item.child_items.is_empty() {
        let mut xml = format!(
            "{tab}<ContextMenu name=\"{}\" id=\"{}\">\r\n",
            escape_xml_text(&item.name),
            escape_xml_text(&item.id)
        );
        if item.autofill == Some(false) {
            xml.push_str(&format!("{tab}\t<Autofill>false</Autofill>\r\n"));
        }
        xml.push_str(&format_form_child_items_xml(&item.child_items, indent + 1));
        xml.push_str(&format!("{tab}</ContextMenu>\r\n"));
        xml
    } else {
        format!(
            "{tab}<ContextMenu name=\"{}\" id=\"{}\"/>\r\n",
            escape_xml_text(&item.name),
            escape_xml_text(&item.id)
        )
    }
}

#[cfg(test)]
pub(super) fn format_form_attributes_xml(attributes: &[FormAttribute]) -> String {
    format_form_attributes_section_xml(attributes, &FormAttributesSection::default())
}

pub(super) fn format_form_attributes_section_xml(
    attributes: &[FormAttribute],
    attributes_section: &FormAttributesSection,
) -> String {
    if attributes.is_empty() && attributes_section.conditional_appearance_xml.is_none() {
        return "\t<Attributes/>\r\n".to_string();
    }
    let mut xml = "\t<Attributes>\r\n".to_string();
    xml.push_str(&format_form_attributes_items_xml(attributes));
    if let Some(conditional_appearance_xml) = &attributes_section.conditional_appearance_xml {
        xml.push_str(&indent_xml_fragment(
            &split_adjacent_xml_tags(conditional_appearance_xml),
            "\t\t",
        ));
    }
    xml.push_str("\t</Attributes>\r\n");
    xml
}

pub(super) fn format_form_attributes_items_xml(attributes: &[FormAttribute]) -> String {
    let mut xml = String::new();
    for attribute in attributes {
        xml.push_str(&format!(
            "\t\t<Attribute name=\"{}\" id=\"{}\">\r\n",
            escape_xml_text(&attribute.name),
            escape_xml_text(&attribute.id)
        ));
        xml.push_str(&format_form_localized_section("Title", &attribute.title, 3));
        if attribute.settings.is_some() {
            xml.push_str("\t\t\t<Type>\r\n");
            xml.push_str("\t\t\t\t<v8:Type>cfg:DynamicList</v8:Type>\r\n");
            xml.push_str("\t\t\t</Type>\r\n");
        } else if !attribute.value_types.is_empty() {
            xml.push_str(&format_form_metadata_types_xml(&attribute.value_types));
        } else if attribute.explicit_empty_type {
            xml.push_str("\t\t\t<Type/>\r\n");
        }
        if let Some(fill_check) = attribute.fill_check {
            xml.push_str(&format!(
                "\t\t\t<FillCheck>{}</FillCheck>\r\n",
                escape_xml_text(fill_check)
            ));
        }
        if !attribute.columns.is_empty() || !attribute.additional_columns.is_empty() {
            xml.push_str("\t\t\t<Columns>\r\n");
            for column in &attribute.columns {
                xml.push_str(&format_form_attribute_column_xml(column, "\t\t\t\t"));
            }
            for additional in &attribute.additional_columns {
                xml.push_str(&format!(
                    "\t\t\t\t<AdditionalColumns table=\"{}\">\r\n",
                    escape_xml_text(&additional.table)
                ));
                for column in &additional.columns {
                    xml.push_str(&format_form_attribute_column_xml(column, "\t\t\t\t\t"));
                }
                xml.push_str("\t\t\t\t</AdditionalColumns>\r\n");
            }
            xml.push_str("\t\t\t</Columns>\r\n");
        }
        if attribute.main_attribute {
            xml.push_str("\t\t\t<MainAttribute>true</MainAttribute>\r\n");
        }
        if attribute.saved_data {
            xml.push_str("\t\t\t<SavedData>true</SavedData>\r\n");
        }
        if !attribute.use_always.is_empty() {
            xml.push_str("\t\t\t<UseAlways>\r\n");
            for field in &attribute.use_always {
                xml.push_str(&format!(
                    "\t\t\t\t<Field>{}</Field>\r\n",
                    escape_xml_text(field)
                ));
            }
            xml.push_str("\t\t\t</UseAlways>\r\n");
        }
        if !attribute.save_fields.is_empty() {
            xml.push_str("\t\t\t<Save>\r\n");
            for field in &attribute.save_fields {
                xml.push_str(&format!(
                    "\t\t\t\t<Field>{}</Field>\r\n",
                    escape_xml_text(field)
                ));
            }
            xml.push_str("\t\t\t</Save>\r\n");
        }
        if !attribute.functional_options.is_empty() {
            xml.push_str("\t\t\t<FunctionalOptions>\r\n");
            for item in &attribute.functional_options {
                xml.push_str(&format!(
                    "\t\t\t\t<Item>{}</Item>\r\n",
                    escape_xml_text(item)
                ));
            }
            xml.push_str("\t\t\t</FunctionalOptions>\r\n");
        }
        if let Some(settings) = &attribute.settings {
            xml.push_str("\t\t\t<Settings xsi:type=\"DynamicList\">\r\n");
            if settings.manual_query {
                xml.push_str("\t\t\t\t<ManualQuery>true</ManualQuery>\r\n");
            }
            if settings.dynamic_data_read_explicit {
                xml.push_str(&format!(
                    "\t\t\t\t<DynamicDataRead>{}</DynamicDataRead>\r\n",
                    if settings.dynamic_data_read {
                        "true"
                    } else {
                        "false"
                    }
                ));
            }
            if let Some(query_text) = &settings.query_text {
                xml.push_str(&format!(
                    "\t\t\t\t<QueryText>{}</QueryText>\r\n",
                    escape_xml_text(query_text)
                ));
            }
            if let Some(server_state_xml) = &settings.server_state_xml {
                xml.push_str(&indent_xml_fragment(server_state_xml, "\t\t\t\t"));
            } else {
                for field in &settings.explicit_fields {
                    xml.push_str("\t\t\t\t<Field xsi:type=\"dcssch:DataSetFieldField\">\r\n");
                    xml.push_str(&format!(
                        "\t\t\t\t\t<dcssch:dataPath>{}</dcssch:dataPath>\r\n",
                        escape_xml_text(&field.data_path)
                    ));
                    xml.push_str(&format!(
                        "\t\t\t\t\t<dcssch:field>{}</dcssch:field>\r\n",
                        escape_xml_text(&field.field)
                    ));
                    xml.push_str("\t\t\t\t</Field>\r\n");
                }
            }
            if let Some(main_table) = &settings.main_table {
                xml.push_str(&format!(
                    "\t\t\t\t<MainTable>{}</MainTable>\r\n",
                    escape_xml_text(main_table)
                ));
            }
            xml.push_str(&format_form_list_settings_xml(&settings.list_settings));
            xml.push_str("\t\t\t</Settings>\r\n");
        } else if let Some(spreadsheet_document_settings) = &attribute.spreadsheet_document_settings
        {
            xml.push_str(
                "\t\t\t<Settings xmlns:mxl=\"http://v8.1c.ru/8.2/data/spreadsheet\" xsi:type=\"mxl:SpreadsheetDocument\">\r\n",
            );
            xml.push_str(&indent_xml_fragment(
                spreadsheet_document_settings,
                "\t\t\t",
            ));
            xml.push_str("\t\t\t</Settings>\r\n");
        } else if let Some(type_description_settings) = &attribute.type_description_settings {
            if type_description_settings.is_empty() {
                xml.push_str("\t\t\t<Settings xsi:type=\"v8:TypeDescription\"/>\r\n");
            } else {
                xml.push_str("\t\t\t<Settings xsi:type=\"v8:TypeDescription\">\r\n");
                xml.push_str(&format_type_description_value_types_xml(
                    type_description_settings,
                    "\t\t\t\t",
                ));
                xml.push_str("\t\t\t</Settings>\r\n");
            }
        }
        xml.push_str("\t\t</Attribute>\r\n");
    }
    xml
}

pub(super) fn format_form_attribute_column_xml(
    column: &FormAttributeColumn,
    indent: &str,
) -> String {
    let mut xml = format!(
        "{indent}<Column name=\"{}\" id=\"{}\">\r\n",
        escape_xml_text(&column.name),
        escape_xml_text(&column.id)
    );
    xml.push_str(&format_form_localized_section(
        "Title",
        &column.title,
        indent.chars().count() + 1,
    ));
    if !column.value_types.is_empty() {
        let nested_indent = format!("{indent}\t");
        xml.push_str(&format_form_metadata_types_xml_with_indent(
            &column.value_types,
            &nested_indent,
        ));
    } else if column.explicit_empty_type {
        xml.push_str(&format!("{indent}\t<Type/>\r\n"));
    }
    if !column.functional_options.is_empty() {
        xml.push_str(&format!("{indent}\t<FunctionalOptions>\r\n"));
        for item in &column.functional_options {
            xml.push_str(&format!(
                "{indent}\t\t<Item>{}</Item>\r\n",
                escape_xml_text(item)
            ));
        }
        xml.push_str(&format!("{indent}\t</FunctionalOptions>\r\n"));
    }
    xml.push_str(&format!("{indent}</Column>\r\n"));
    xml
}

pub(super) fn format_form_list_settings_xml(settings: &FormListSettings) -> String {
    if !form_list_settings_standard_section_has_output(settings.filter.as_ref())
        && !form_list_settings_order_has_output(settings.order.as_ref())
        && !form_list_settings_standard_section_has_output(settings.conditional_appearance.as_ref())
        && settings.items_view_mode.is_none()
        && settings.items_user_setting_id.is_none()
    {
        return String::new();
    }
    let mut xml = "\t\t\t\t<ListSettings>\r\n".to_string();
    if form_list_settings_standard_section_has_output(settings.filter.as_ref())
        && let Some(filter) = &settings.filter
    {
        if let Some(raw_xml) = &filter.raw_xml {
            xml.push_str(&indent_xml_fragment(raw_xml, "\t\t\t\t\t"));
        } else {
            xml.push_str(&format_form_list_settings_standard_section_xml(
                "filter", filter,
            ));
        }
    }
    if form_list_settings_order_has_output(settings.order.as_ref())
        && let Some(order) = &settings.order
    {
        if let Some(raw_xml) = &order.raw_xml {
            xml.push_str(&indent_xml_fragment(raw_xml, "\t\t\t\t\t"));
        } else {
            xml.push_str("\t\t\t\t\t<dcsset:order>\r\n");
            for item in &order.items {
                xml.push_str("\t\t\t\t\t\t<dcsset:item xsi:type=\"dcsset:OrderItemField\">\r\n");
                xml.push_str(&format!(
                    "\t\t\t\t\t\t\t<dcsset:field>{}</dcsset:field>\r\n",
                    escape_xml_text(&item.field)
                ));
                if let Some(order_type) = &item.order_type {
                    xml.push_str(&format!(
                        "\t\t\t\t\t\t\t<dcsset:orderType>{}</dcsset:orderType>\r\n",
                        escape_xml_text(order_type)
                    ));
                }
                xml.push_str("\t\t\t\t\t\t</dcsset:item>\r\n");
            }
            if let Some(view_mode) = &order.view_mode {
                xml.push_str(&format!(
                    "\t\t\t\t\t\t<dcsset:viewMode>{}</dcsset:viewMode>\r\n",
                    escape_xml_text(view_mode)
                ));
            }
            if let Some(user_setting_id) = &order.user_setting_id {
                xml.push_str(&format!(
                    "\t\t\t\t\t\t<dcsset:userSettingID>{}</dcsset:userSettingID>\r\n",
                    escape_xml_text(user_setting_id)
                ));
            }
            xml.push_str("\t\t\t\t\t</dcsset:order>\r\n");
        }
    }
    if form_list_settings_standard_section_has_output(settings.conditional_appearance.as_ref())
        && let Some(conditional_appearance) = &settings.conditional_appearance
    {
        if let Some(raw_xml) = &conditional_appearance.raw_xml {
            xml.push_str(&indent_xml_fragment(raw_xml, "\t\t\t\t\t"));
        } else {
            xml.push_str(&format_form_list_settings_standard_section_xml(
                "conditionalAppearance",
                conditional_appearance,
            ));
        }
    }
    if let Some(items_view_mode) = &settings.items_view_mode {
        xml.push_str(&format!(
            "\t\t\t\t\t<dcsset:itemsViewMode>{}</dcsset:itemsViewMode>\r\n",
            escape_xml_text(items_view_mode)
        ));
    }
    if let Some(items_user_setting_id) = &settings.items_user_setting_id {
        xml.push_str(&format!(
            "\t\t\t\t\t<dcsset:itemsUserSettingID>{}</dcsset:itemsUserSettingID>\r\n",
            escape_xml_text(items_user_setting_id)
        ));
    }
    xml.push_str("\t\t\t\t</ListSettings>\r\n");
    xml
}

pub(super) fn form_list_settings_standard_section_has_output(
    section: Option<&FormListSettingsStandardSection>,
) -> bool {
    section.is_some_and(|section| {
        section.raw_xml.is_some()
            || section.view_mode.is_some()
            || section.user_setting_id.is_some()
    })
}

pub(super) fn form_list_settings_order_has_output(order: Option<&FormListSettingsOrder>) -> bool {
    order.is_some_and(|order| {
        order.raw_xml.is_some()
            || !order.items.is_empty()
            || order.view_mode.is_some()
            || order.user_setting_id.is_some()
    })
}

pub(super) fn format_form_list_settings_standard_section_xml(
    name: &str,
    section: &FormListSettingsStandardSection,
) -> String {
    let mut xml = format!("\t\t\t\t\t<dcsset:{name}>\r\n");
    if let Some(view_mode) = &section.view_mode {
        xml.push_str(&format!(
            "\t\t\t\t\t\t<dcsset:viewMode>{}</dcsset:viewMode>\r\n",
            escape_xml_text(view_mode)
        ));
    }
    if let Some(user_setting_id) = &section.user_setting_id {
        xml.push_str(&format!(
            "\t\t\t\t\t\t<dcsset:userSettingID>{}</dcsset:userSettingID>\r\n",
            escape_xml_text(user_setting_id)
        ));
    }
    xml.push_str(&format!("\t\t\t\t\t</dcsset:{name}>\r\n"));
    xml
}

pub(super) fn indent_xml_fragment(fragment: &str, indent: &str) -> String {
    let mut xml = String::new();
    let mut level = 0usize;
    for line in fragment
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if line.starts_with("</") {
            level = level.saturating_sub(1);
        }
        xml.push_str(indent);
        for _ in 0..level {
            xml.push('\t');
        }
        xml.push_str(line);
        xml.push_str("\r\n");
        if line.starts_with('<')
            && !line.starts_with("</")
            && !line.starts_with("<?")
            && !line.starts_with("<!")
            && !line.ends_with("/>")
            && !line.contains("</")
        {
            level += 1;
        }
    }
    xml
}

pub(super) fn split_adjacent_xml_tags(fragment: &str) -> String {
    fragment.replace("><", ">\n<")
}

pub(super) fn form_bool_when_not_native_default(
    value: Option<bool>,
    native_default: bool,
) -> Option<bool> {
    value.filter(|value| *value != native_default)
}

pub(super) fn format_form_parameters_xml(parameters: &[FormParameter]) -> String {
    if parameters.is_empty() {
        return String::new();
    }
    let mut xml = "\t<Parameters>\r\n".to_string();
    for parameter in parameters {
        xml.push_str(&format!(
            "\t\t<Parameter name=\"{}\">\r\n",
            escape_xml_text(&parameter.name)
        ));
        if !parameter.value_types.is_empty() {
            xml.push_str(&format_form_metadata_types_xml(&parameter.value_types));
        } else if parameter.explicit_empty_type {
            xml.push_str("\t\t\t<Type/>\r\n");
        }
        if parameter.key_parameter {
            xml.push_str("\t\t\t<KeyParameter>true</KeyParameter>\r\n");
        }
        xml.push_str("\t\t</Parameter>\r\n");
    }
    xml.push_str("\t</Parameters>\r\n");
    xml
}

pub(super) fn format_form_localized_section(
    name: &str,
    values: &[(String, String)],
    indent: usize,
) -> String {
    if values.is_empty() {
        return String::new();
    }
    let tab = "\t".repeat(indent);
    let mut xml = format!("{tab}<{}>\r\n", name);
    for (lang, content) in values {
        xml.push_str(&format!(
            "{tab}\t<v8:item>\r\n{tab}\t\t<v8:lang>{}</v8:lang>\r\n{tab}\t\t<v8:content>{}</v8:content>\r\n{tab}\t</v8:item>\r\n",
            escape_xml_text(lang),
            escape_xml_text(content)
        ));
    }
    xml.push_str(&format!("{tab}</{}>\r\n", name));
    xml
}

pub(super) fn format_form_title_section(item: &FormChildItem, indent: usize) -> String {
    if item.title.is_empty() {
        return String::new();
    }
    if !matches!(item.tag, "LabelDecoration" | "PictureDecoration") {
        return format_form_localized_section("Title", &item.title, indent);
    }
    let tab = "\t".repeat(indent);
    let mut xml = format!("{tab}<Title formatted=\"false\">\r\n");
    for (lang, content) in &item.title {
        xml.push_str(&format!(
            "{tab}\t<v8:item>\r\n{tab}\t\t<v8:lang>{}</v8:lang>\r\n{tab}\t\t<v8:content>{}</v8:content>\r\n{tab}\t</v8:item>\r\n",
            escape_xml_text(lang),
            escape_xml_text(content)
        ));
    }
    xml.push_str(&format!("{tab}</Title>\r\n"));
    xml
}

pub(super) fn format_form_command_interface_xml(
    command_interface: &FormCommandInterface,
) -> String {
    let mut xml = "\t<CommandInterface>\r\n".to_string();
    if !command_interface.command_bar.is_empty() {
        xml.push_str("\t\t<CommandBar>\r\n");
        for item in &command_interface.command_bar {
            xml.push_str("\t\t\t<Item>\r\n");
            xml.push_str(&format!(
                "\t\t\t\t<Command>{}</Command>\r\n",
                escape_xml_text(&item.command)
            ));
            xml.push_str(&format!(
                "\t\t\t\t<Type>{}</Type>\r\n",
                escape_xml_text(item.item_type)
            ));
            if let Some(command_group) = item.command_group.as_deref() {
                xml.push_str(&format!(
                    "\t\t\t\t<CommandGroup>{}</CommandGroup>\r\n",
                    escape_xml_text(command_group)
                ));
            }
            if let Some(index) = item.index {
                xml.push_str(&format!("\t\t\t\t<Index>{index}</Index>\r\n"));
            }
            if let Some(default_visible) = item.default_visible {
                xml.push_str(&format!(
                    "\t\t\t\t<DefaultVisible>{}</DefaultVisible>\r\n",
                    xml_bool(default_visible)
                ));
            }
            if let Some(common) = item.visible_common {
                xml.push_str("\t\t\t\t<Visible>\r\n");
                xml.push_str(&format!(
                    "\t\t\t\t\t<xr:Common>{}</xr:Common>\r\n",
                    xml_bool(common)
                ));
                xml.push_str("\t\t\t\t</Visible>\r\n");
            }
            xml.push_str("\t\t\t</Item>\r\n");
        }
        xml.push_str("\t\t</CommandBar>\r\n");
    }
    if !command_interface.navigation_panel.is_empty() {
        xml.push_str("\t\t<NavigationPanel>\r\n");
        for item in &command_interface.navigation_panel {
            xml.push_str("\t\t\t<Item>\r\n");
            xml.push_str(&format!(
                "\t\t\t\t<Command>{}</Command>\r\n",
                escape_xml_text(&item.command)
            ));
            xml.push_str(&format!(
                "\t\t\t\t<Type>{}</Type>\r\n",
                escape_xml_text(item.item_type)
            ));
            if let Some(command_group) = item.command_group.as_deref() {
                xml.push_str(&format!(
                    "\t\t\t\t<CommandGroup>{}</CommandGroup>\r\n",
                    escape_xml_text(command_group)
                ));
            }
            if let Some(index) = item.index {
                xml.push_str(&format!("\t\t\t\t<Index>{index}</Index>\r\n"));
            }
            if let Some(default_visible) = item.default_visible {
                xml.push_str(&format!(
                    "\t\t\t\t<DefaultVisible>{}</DefaultVisible>\r\n",
                    xml_bool(default_visible)
                ));
            }
            if let Some(common) = item.visible_common {
                xml.push_str("\t\t\t\t<Visible>\r\n");
                xml.push_str(&format!(
                    "\t\t\t\t\t<xr:Common>{}</xr:Common>\r\n",
                    xml_bool(common)
                ));
                xml.push_str("\t\t\t\t</Visible>\r\n");
            }
            xml.push_str("\t\t\t</Item>\r\n");
        }
        xml.push_str("\t\t</NavigationPanel>\r\n");
    }
    xml.push_str("\t</CommandInterface>\r\n");
    xml
}

pub(crate) fn unpack_form_body_module_text(blob: &[u8]) -> Option<Vec<u8>> {
    let body = parse_form_body_blob(blob).ok()?;
    form_body_module_text_bytes(&body)
}

pub(super) fn form_body_module_text_bytes(body: &ParsedFormBodyBlob) -> Option<Vec<u8>> {
    if body.module_text.is_empty() {
        return None;
    }
    let mut bytes = Vec::with_capacity(3 + body.module_text.len());
    bytes.extend_from_slice(b"\xEF\xBB\xBF");
    bytes.extend_from_slice(body.module_text.as_bytes());
    Some(bytes)
}
